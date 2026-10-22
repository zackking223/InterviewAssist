use futures_util::StreamExt;
use serde_json::{json, Value};
use std::time::Duration;
use tauri::{AppHandle, Emitter};

use crate::ChatMessage;

const BASE_URL: &str = "https://generativelanguage.googleapis.com/v1beta";

fn normalize_api_key(api_key: &str) -> Result<&str, String> {
    let key = api_key.trim();
    if key.is_empty() {
        Err("Google AI Studio API key cannot be empty".into())
    } else {
        Ok(key)
    }
}

pub async fn test_connection(api_key: &str) -> Result<(), String> {
    let response = reqwest::Client::new()
        .get(format!("{BASE_URL}/models"))
        .header("x-goog-api-key", normalize_api_key(api_key)?)
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

pub async fn stream_response(
    app: AppHandle,
    api_key: String,
    model: String,
    system_prompt: String,
    messages: Vec<ChatMessage>,
) -> Result<(), String> {
    let thinking_level = if model == "gemini-3.1-flash-lite" {
        "minimal"
    } else {
        "low"
    };
    let contents = messages
        .into_iter()
        .map(|message| {
            json!({
                "role": if message.role == "assistant" { "model" } else { "user" },
                "parts": [{ "text": message.content }]
            })
        })
        .collect::<Vec<_>>();
    let response = reqwest::Client::new()
        .post(format!(
            "{BASE_URL}/models/{model}:streamGenerateContent?alt=sse"
        ))
        .header("x-goog-api-key", normalize_api_key(&api_key)?)
        .json(&json!({
            "system_instruction": { "parts": [{ "text": system_prompt }] },
            "contents": contents,
            "generationConfig": {
                "maxOutputTokens": 4096,
                "thinkingConfig": { "thinkingLevel": thinking_level }
            }
        }))
        .timeout(Duration::from_secs(120))
        .send()
        .await
        .map_err(|error| error.to_string())?;
    if !response.status().is_success() {
        return Err(api_error(response).await);
    }

    let mut chunks = response.bytes_stream();
    let mut buffer = String::new();
    let mut finish_reason: Option<String> = None;
    while let Some(chunk) = chunks.next().await {
        buffer.push_str(&String::from_utf8_lossy(
            &chunk.map_err(|error| error.to_string())?,
        ));
        while let Some(index) = buffer.find('\n') {
            let line = buffer[..index].trim_end_matches('\r').to_string();
            buffer.drain(..=index);
            let Some(data) = line.strip_prefix("data: ") else {
                continue;
            };
            let Ok(event) = serde_json::from_str::<Value>(data) else {
                continue;
            };
            if let Some(message) = event.pointer("/error/message").and_then(Value::as_str) {
                return Err(message.to_owned());
            }
            if let Some(reason) = event
                .pointer("/candidates/0/finishReason")
                .and_then(Value::as_str)
            {
                finish_reason = Some(reason.to_owned());
            }
            if let Some(parts) = event
                .pointer("/candidates/0/content/parts")
                .and_then(Value::as_array)
            {
                for part in parts {
                    if part.get("thought").and_then(Value::as_bool) == Some(true) {
                        continue;
                    }
                    if let Some(text) = part.get("text").and_then(Value::as_str) {
                        let _ = app.emit("response-delta", text);
                    }
                }
            }
        }
    }
    if let Some(reason) = finish_reason.filter(|reason| reason != "STOP") {
        return Err(match reason.as_str() {
            "MAX_TOKENS" => {
                "Response was truncated because Gemini reached its output-token limit".to_string()
            }
            "SAFETY" => "Gemini stopped the response because of its safety filters".to_string(),
            _ => format!("Gemini stopped before completing the response ({reason})"),
        });
    }
    let _ = app.emit("response-completed", ());
    Ok(())
}

async fn api_error(response: reqwest::Response) -> String {
    let status = response.status();
    let body = response.json::<Value>().await.ok();
    let message = body
        .as_ref()
        .and_then(|value| value.pointer("/error/message"))
        .and_then(Value::as_str)
        .map(str::to_owned)
        .unwrap_or_else(|| "Google Gemini request failed".into());
    format!("{message} (HTTP {status})")
}

#[cfg(test)]
mod tests {
    use super::normalize_api_key;

    #[test]
    fn trims_and_rejects_empty_keys() {
        assert_eq!(normalize_api_key("  test-key  ").unwrap(), "test-key");
        assert!(normalize_api_key("  ").is_err());
    }
}
