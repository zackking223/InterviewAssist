use std::time::Duration;

use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use futures_util::{SinkExt, StreamExt};
use reqwest::header::{AUTHORIZATION, CONTENT_TYPE};
use serde_json::{json, Value};
use tauri::{AppHandle, Emitter};
use tokio::sync::mpsc;
use tokio_tungstenite::{
    connect_async,
    tungstenite::{client::IntoClientRequest, Message},
};

use crate::audio::AudioMessage;
use crate::ChatMessage;

pub async fn test_connection(api_key: &str) -> Result<(), String> {
    let response = reqwest::Client::new()
        .get("https://api.openai.com/v1/models")
        .header(AUTHORIZATION, format!("Bearer {api_key}"))
        .timeout(Duration::from_secs(15))
        .send()
        .await
        .map_err(|error| error.to_string())?;
    if response.status().is_success() {
        Ok(())
    } else {
        Err(api_error(response).await)
    }
}

pub async fn run_transcription(
    app: AppHandle,
    api_key: String,
    languages: Vec<String>,
    mut audio: mpsc::UnboundedReceiver<AudioMessage>,
) {
    loop {
        match transcription_inner(&app, &api_key, languages.clone(), &mut audio).await {
            Ok(()) => return,
            Err(error) => {
                let lowered = error.to_ascii_lowercase();
                let reconnectable = lowered.contains("reset")
                    || lowered.contains("without closing handshake")
                    || lowered.contains("connection closed")
                    || lowered.contains("broken pipe");
                if !reconnectable {
                    let _ = app.emit(
                        "transcription-error",
                        format!("OpenAI caption error: {error}"),
                    );
                    return;
                }
                let _ = app.emit(
                    "transcription-error",
                    "OpenAI caption WebSocket disconnected; reconnecting…",
                );
                tokio::time::sleep(Duration::from_millis(900)).await;
                if audio.is_closed() {
                    return;
                }
            }
        }
    }
}

async fn transcription_inner(
    app: &AppHandle,
    api_key: &str,
    languages: Vec<String>,
    audio: &mut mpsc::UnboundedReceiver<AudioMessage>,
) -> Result<(), String> {
    // The WebSocket is a transcription session. The speech-to-text model is
    // configured inside session.audio.input.transcription below; it is not a
    // Realtime conversation model and must not be passed as the URL model.
    let mut request = "wss://api.openai.com/v1/realtime?intent=transcription"
        .into_client_request()
        .map_err(|error| error.to_string())?;
    request.headers_mut().insert(
        AUTHORIZATION,
        format!("Bearer {api_key}")
            .parse()
            .map_err(|_| "invalid API key header")?,
    );
    let (socket, _) = connect_async(request)
        .await
        .map_err(|error| format!("transcription connection failed: {error}"))?;
    let (mut write, mut read) = socket.split();

    let transcription = if languages.is_empty() {
        json!({ "model": "gpt-live-transcribe", "delay": "low" })
    } else {
        json!({ "model": "gpt-live-transcribe", "languages": languages, "delay": "low" })
    };
    write
        .send(Message::Text(
            json!({
                "type": "session.update",
                "session": {
                    "type": "transcription",
                    "audio": { "input": {
                        "format": { "type": "audio/pcm", "rate": 24000 },
                        "transcription": transcription,
                        "turn_detection": null
                    }}
                }
            })
            .to_string()
            .into(),
        ))
        .await
        .map_err(|error| error.to_string())?;

    let mut speech_active = false;
    let mut silence_samples = 0usize;
    let mut buffered_samples = 0usize;
    loop {
        tokio::select! {
            input = audio.recv() => match input {
                Some(AudioMessage::Samples(samples)) => {
                    buffered_samples += samples.len();
                    let rms = root_mean_square(&samples);
                    if rms > 450.0 {
                        speech_active = true;
                        silence_samples = 0;
                    } else if speech_active {
                        silence_samples += samples.len();
                    }
                    let bytes: Vec<u8> = samples.iter().flat_map(|sample| sample.to_le_bytes()).collect();
                    write.send(Message::Text(json!({
                        "type": "input_audio_buffer.append",
                        "audio": BASE64.encode(bytes)
                    }).to_string().into())).await.map_err(|error| error.to_string())?;

                    if speech_active && silence_samples >= 16_800 {
                        write.send(Message::Text(json!({"type":"input_audio_buffer.commit"}).to_string().into()))
                            .await.map_err(|error| error.to_string())?;
                        speech_active = false;
                        silence_samples = 0;
                        buffered_samples = 0;
                    }
                }
                Some(AudioMessage::Flush) => {
                    if buffered_samples > 0 {
                        write.send(Message::Text(json!({"type":"input_audio_buffer.commit"}).to_string().into()))
                            .await.map_err(|error| error.to_string())?;
                    }
                    speech_active = false;
                    silence_samples = 0;
                    buffered_samples = 0;
                }
                Some(AudioMessage::Stop) | None => {
                    if buffered_samples > 0 {
                        let _ = write.send(Message::Text(json!({"type":"input_audio_buffer.commit"}).to_string().into())).await;
                    }
                    let _ = write.close().await;
                    return Ok(());
                }
            },
            message = read.next() => match message {
                Some(Ok(Message::Text(text))) => handle_transcription_event(app, text.as_ref()),
                Some(Ok(Message::Close(_))) | None => return Ok(()),
                Some(Err(error)) => return Err(error.to_string()),
                _ => {}
            }
        }
    }
}

fn handle_transcription_event(app: &AppHandle, text: &str) {
    let Ok(event) = serde_json::from_str::<Value>(text) else {
        return;
    };
    match event.get("type").and_then(Value::as_str) {
        Some("conversation.item.input_audio_transcription.delta") => {
            if let Some(delta) = event.get("delta").and_then(Value::as_str) {
                let _ = app.emit("transcript-partial", delta);
            }
        }
        Some("conversation.item.input_audio_transcription.completed") => {
            if let Some(transcript) = event.get("transcript").and_then(Value::as_str) {
                let _ = app.emit("transcript-final", transcript);
            }
        }
        Some("error") => {
            let message = event
                .pointer("/error/message")
                .and_then(Value::as_str)
                .unwrap_or("OpenAI transcription error");
            let _ = app.emit("transcription-error", message);
        }
        _ => {}
    }
}

pub async fn stream_response(
    app: AppHandle,
    api_key: String,
    model: String,
    system_prompt: String,
    messages: Vec<ChatMessage>,
) -> Result<(), String> {
    let response = reqwest::Client::new()
        .post("https://api.openai.com/v1/responses")
        .header(AUTHORIZATION, format!("Bearer {api_key}"))
        .header(CONTENT_TYPE, "application/json")
        .json(&json!({
            "model": model,
            "instructions": system_prompt,
            "input": messages,
            "stream": true,
            "store": false
        }))
        .timeout(Duration::from_secs(120))
        .send()
        .await
        .map_err(|error| error.to_string())?;

    if !response.status().is_success() {
        return Err(api_error(response).await);
    }

    let mut bytes = response.bytes_stream();
    let mut buffer = String::new();
    while let Some(chunk) = bytes.next().await {
        let chunk = chunk.map_err(|error| error.to_string())?;
        buffer.push_str(&String::from_utf8_lossy(&chunk));
        while let Some(index) = buffer.find('\n') {
            let line = buffer[..index].trim_end_matches('\r').to_string();
            buffer.drain(..=index);
            if let Some(data) = line.strip_prefix("data: ") {
                if data == "[DONE]" {
                    continue;
                }
                if let Ok(event) = serde_json::from_str::<Value>(data) {
                    match event.get("type").and_then(Value::as_str) {
                        Some("response.output_text.delta") => {
                            if let Some(delta) = event.get("delta").and_then(Value::as_str) {
                                let _ = app.emit("response-delta", delta);
                            }
                        }
                        Some("response.completed") => {
                            let _ = app.emit("response-completed", ());
                        }
                        Some("error") | Some("response.failed") => {
                            let message = event
                                .pointer("/error/message")
                                .and_then(Value::as_str)
                                .unwrap_or("OpenAI response failed");
                            return Err(message.into());
                        }
                        _ => {}
                    }
                }
            }
        }
    }
    Ok(())
}

async fn api_error(response: reqwest::Response) -> String {
    let status = response.status();
    let body = response.json::<Value>().await.ok();
    body.as_ref()
        .and_then(|value| value.pointer("/error/message"))
        .and_then(Value::as_str)
        .map(str::to_owned)
        .unwrap_or_else(|| format!("OpenAI request failed with HTTP {status}"))
}

fn root_mean_square(samples: &[i16]) -> f64 {
    if samples.is_empty() {
        return 0.0;
    }
    let sum: f64 = samples.iter().map(|sample| (*sample as f64).powi(2)).sum();
    (sum / samples.len() as f64).sqrt()
}
