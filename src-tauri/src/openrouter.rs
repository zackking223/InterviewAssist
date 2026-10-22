use futures_util::StreamExt;
use reqwest::{redirect::Policy, Client};
use serde_json::{json, Value};
use std::time::Duration;
use tauri::{AppHandle, Emitter};

use crate::ChatMessage;

const BASE_URL: &str = "https://openrouter.ai/api/v1";

fn normalize_api_key(api_key: &str) -> Result<&str, String> {
    let trimmed = api_key.trim();
    let normalized = if trimmed.eq_ignore_ascii_case("Bearer") {
        ""
    } else if trimmed
        .get(..7)
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case("Bearer "))
    {
        trimmed[7..].trim()
    } else {
        trimmed
    };
    if normalized.is_empty() {
        Err("OpenRouter API key cannot be empty".into())
    } else {
        Ok(normalized)
    }
}

fn client() -> Result<Client, String> {
    Client::builder()
        // Authorization headers are intentionally not forwarded through redirects.
        .redirect(Policy::none())
        .build()
        .map_err(|error| error.to_string())
}

fn fallback_models(model: String) -> Vec<String> {
    if model == "openrouter/free" {
        vec![model]
    } else {
        vec![model, "openrouter/free".into()]
    }
}

pub async fn test_connection(api_key: &str) -> Result<(), String> {
    let response = client()?
        .get(format!("{BASE_URL}/key"))
        .bearer_auth(normalize_api_key(api_key)?)
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

pub async fn stream_free_response(
    app: AppHandle,
    api_key: String,
    model: String,
    system_prompt: String,
    messages: Vec<ChatMessage>,
) -> Result<(), String> {
    let models = fallback_models(model);
    let response = client()?
        .post(format!("{BASE_URL}/chat/completions"))
        .bearer_auth(normalize_api_key(&api_key)?)
        .header("HTTP-Referer", "https://interviewassist.local")
        .header("X-Title", "InterviewAssist")
        .json(&json!({
            "models": models,
            "messages": std::iter::once(json!({ "role": "system", "content": system_prompt }))
                .chain(messages.into_iter().map(|message| json!(message)))
                .collect::<Vec<_>>(),
            "stream": true,
            "max_tokens": 4096,
            "provider": { "sort": "latency" }
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
            if data == "[DONE]" {
                let _ = app.emit("response-completed", ());
                return Ok(());
            }
            if let Ok(event) = serde_json::from_str::<Value>(data) {
                if let Some(delta) = event
                    .pointer("/choices/0/delta/content")
                    .and_then(Value::as_str)
                {
                    let _ = app.emit("response-delta", delta);
                }
                if let Some(message) = event.pointer("/error/message").and_then(Value::as_str) {
                    return Err(message.into());
                }
            }
        }
    }
    let _ = app.emit("response-completed", ());
    Ok(())
}

async fn api_error(response: reqwest::Response) -> String {
    let status = response.status();
    let body = response.json::<Value>().await.ok();
    body.as_ref()
        .and_then(|value| value.pointer("/error/message"))
        .and_then(Value::as_str)
        .map(str::to_owned)
        .unwrap_or_else(|| format!("OpenRouter request failed ({status})"))
}

#[cfg(test)]
mod tests {
    use super::{fallback_models, normalize_api_key};

    #[test]
    fn normalizes_whitespace_and_optional_bearer_prefix() {
        assert_eq!(normalize_api_key("  sk-or-test  ").unwrap(), "sk-or-test");
        assert_eq!(
            normalize_api_key("Bearer sk-or-test").unwrap(),
            "sk-or-test"
        );
        assert_eq!(
            normalize_api_key("bearer sk-or-test").unwrap(),
            "sk-or-test"
        );
    }

    #[test]
    fn rejects_empty_keys() {
        assert!(normalize_api_key("  ").is_err());
        assert!(normalize_api_key("Bearer  ").is_err());
    }

    #[test]
    fn fast_model_falls_back_to_free_router() {
        assert_eq!(
            fallback_models("google/gemma-4-26b-a4b-it:free".into()),
            vec!["google/gemma-4-26b-a4b-it:free", "openrouter/free"]
        );
        assert_eq!(
            fallback_models("openrouter/free".into()),
            vec!["openrouter/free"]
        );
    }
}
