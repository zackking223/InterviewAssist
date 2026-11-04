use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use tauri::{AppHandle, Emitter};
use tokio::sync::mpsc;
use tokio_tungstenite::{connect_async, tungstenite::Message};

use crate::audio::AudioMessage;

pub async fn run_transcription(
    app: AppHandle,
    server_url: String,
    language: String,
    mut audio: mpsc::UnboundedReceiver<AudioMessage>,
) {
    if let Err(error) = transcription_inner(&app, &server_url, &language, &mut audio).await {
        let _ = app.emit(
            "transcription-error",
            format!("Whisper server error: {error}. Check the server URL and network connection."),
        );
    }
}

async fn transcription_inner(
    app: &AppHandle,
    server_url: &str,
    language: &str,
    audio: &mut mpsc::UnboundedReceiver<AudioMessage>,
) -> Result<(), String> {
    if !(server_url.starts_with("ws://") || server_url.starts_with("wss://")) {
        return Err("server URL must start with ws:// or wss://".into());
    }
    let (socket, _) = connect_async(server_url)
        .await
        .map_err(|error| format!("cannot connect to {server_url}: {error}"))?;
    let (mut write, mut read) = socket.split();
    write
        .send(Message::Text(
            json!({"type": "session_update", "language": language})
                .to_string()
                .into(),
        ))
        .await
        .map_err(|error| error.to_string())?;

    loop {
        tokio::select! {
            input = audio.recv() => match input {
                Some(AudioMessage::Samples(samples)) => {
                    let bytes: Vec<u8> = samples
                        .into_iter()
                        .flat_map(i16::to_le_bytes)
                        .collect();
                    write.send(Message::Binary(bytes.into())).await.map_err(|error| error.to_string())?;
                }
                Some(AudioMessage::Flush) => {
                    write.send(Message::Text(json!({"type": "flush"}).to_string().into()))
                        .await.map_err(|error| error.to_string())?;
                }
                Some(AudioMessage::Stop) | None => {
                    let _ = write.send(Message::Text(json!({"type": "stop"}).to_string().into())).await;
                    let _ = write.close().await;
                    return Ok(());
                }
            },
            message = read.next() => match message {
                Some(Ok(Message::Text(text))) => handle_event(app, text.as_ref()),
                Some(Ok(Message::Close(_))) | None => return Err("connection closed".into()),
                Some(Err(error)) => return Err(error.to_string()),
                _ => {}
            }
        }
    }
}

fn handle_event(app: &AppHandle, text: &str) {
    let Ok(event) = serde_json::from_str::<Value>(text) else {
        return;
    };
    match event.get("type").and_then(Value::as_str) {
        Some("status") => {
            if let Some(message) = event.get("message").and_then(Value::as_str) {
                let _ = app.emit("transcript-partial", message);
            }
        }
        Some("partial") => {
            if let Some(value) = event.get("text").and_then(Value::as_str) {
                let _ = app.emit("transcript-partial", value);
            }
        }
        Some("final") => {
            if let Some(value) = event.get("text").and_then(Value::as_str) {
                let _ = app.emit("transcript-final", value);
            }
        }
        Some("error") => {
            let message = event
                .get("message")
                .and_then(Value::as_str)
                .unwrap_or("Whisper server returned an error");
            let _ = app.emit("transcription-error", message);
        }
        _ => {}
    }
}
