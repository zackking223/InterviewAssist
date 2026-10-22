mod audio;
mod chatgpt;
mod google;
mod local_whisper;
mod openai;
mod openrouter;
mod secret_store;
mod window_protection;

use audio::{AudioSession, SourceKind};
use serde::{Deserialize, Serialize};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Mutex,
};
use tauri::{
    menu::{MenuBuilder, MenuItemBuilder},
    tray::TrayIconBuilder,
    AppHandle, Emitter, Manager, State, WindowEvent,
};

struct AppState {
    protection: Mutex<String>,
    audio: Mutex<Option<AudioSession>>,
    response: Mutex<Option<tauri::async_runtime::JoinHandle<()>>>,
    response_active: AtomicBool,
}

#[derive(Clone, Deserialize, Serialize)]
struct ChatMessage {
    role: String,
    content: String,
}

fn is_transient_google_error(error: &str) -> bool {
    let error = error.to_ascii_lowercase();
    [
        "high demand",
        "try again later",
        "resource_exhausted",
        "rate limit",
        "quota",
        "overloaded",
        "unavailable",
        "timeout",
        "timed out",
        "connection",
        "http 429",
        "http 500",
        "http 502",
        "http 503",
        "http 504",
    ]
    .iter()
    .any(|needle| error.contains(needle))
}

fn openrouter_label(model: &str) -> &'static str {
    if model == "openrouter/free" {
        "OpenRouter Auto Free"
    } else {
        "Gemma 4 Fast Free"
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            protection: Mutex::new("unavailable".into()),
            audio: Mutex::new(None),
            response: Mutex::new(None),
            response_active: AtomicBool::new(false),
        }
    }
}

#[tauri::command]
fn screen_protection_status(state: State<'_, AppState>) -> String {
    state
        .protection
        .lock()
        .map(|v| v.clone())
        .unwrap_or_else(|_| "error".into())
}

#[tauri::command]
fn has_api_key() -> bool {
    secret_store::has_api_key()
}

#[tauri::command]
async fn save_api_key(value: String, test: bool) -> Result<(), String> {
    if test {
        openai::test_connection(value.trim()).await?;
    }
    secret_store::save_api_key(&value)
}

#[tauri::command]
fn delete_api_key() -> Result<(), String> {
    secret_store::delete_api_key()
}

#[tauri::command]
fn has_openrouter_key() -> bool {
    secret_store::has_openrouter_key()
}

#[tauri::command]
async fn save_openrouter_key(value: String, test: bool) -> Result<(), String> {
    if test {
        openrouter::test_connection(value.trim()).await?;
    }
    secret_store::save_openrouter_key(&value)
}

#[tauri::command]
fn delete_openrouter_key() -> Result<(), String> {
    secret_store::delete_openrouter_key()
}

#[tauri::command]
fn has_google_key() -> bool {
    secret_store::has_google_key()
}

#[tauri::command]
async fn save_google_key(value: String, test: bool) -> Result<(), String> {
    if test {
        google::test_connection(value.trim()).await?;
    }
    secret_store::save_google_key(&value)
}

#[tauri::command]
fn delete_google_key() -> Result<(), String> {
    secret_store::delete_google_key()
}

#[tauri::command]
async fn test_openai_connection() -> Result<(), String> {
    openai::test_connection(&secret_store::get_api_key()?).await
}

#[tauri::command]
fn chatgpt_status() -> chatgpt::ChatGptStatus {
    chatgpt::status()
}

#[tauri::command]
async fn chatgpt_sign_in() -> Result<chatgpt::ChatGptStatus, String> {
    chatgpt::sign_in().await
}

#[tauri::command]
async fn chatgpt_sign_out() -> Result<(), String> {
    chatgpt::sign_out().await
}

#[tauri::command]
async fn chatgpt_models() -> Result<Vec<chatgpt::ChatGptModel>, String> {
    chatgpt::models().await
}

#[tauri::command]
fn local_model_status(app: AppHandle) -> Result<Vec<local_whisper::LocalModelStatus>, String> {
    local_whisper::statuses(&app)
}

#[tauri::command]
async fn download_local_model(
    app: AppHandle,
    model_id: String,
) -> Result<Vec<local_whisper::LocalModelStatus>, String> {
    local_whisper::download(app, model_id).await
}

#[tauri::command]
fn start_capture(
    app: AppHandle,
    state: State<'_, AppState>,
    source: String,
    language: String,
    caption_engine: String,
    local_model: String,
) -> Result<(), String> {
    let mut current = state
        .audio
        .lock()
        .map_err(|_| "audio state is unavailable")?;
    if current.is_some() {
        return Err("capture is already active".into());
    }
    let (sender, receiver) = tokio::sync::mpsc::unbounded_channel();
    match caption_engine.as_str() {
        "local" => local_whisper::start(app.clone(), language.clone(), local_model, receiver)?,
        "openai" => {
            let api_key = secret_store::get_api_key()?;
            let languages = match language.as_str() {
                "en" => vec!["en".into()],
                "vi" => vec!["vi".into()],
                _ => vec!["en".into(), "vi".into()],
            };
            tauri::async_runtime::spawn(openai::run_transcription(
                app.clone(),
                api_key,
                languages,
                receiver,
            ));
        }
        _ => return Err("invalid caption engine".into()),
    }
    let session = audio::start(parse_source(&source)?, sender)?;
    *current = Some(session);
    let _ = app.emit("capture-state-changed", "listening");
    Ok(())
}

#[tauri::command]
fn pause_capture(app: AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    state
        .audio
        .lock()
        .map_err(|_| "audio state is unavailable")?
        .as_ref()
        .ok_or("capture is not active")?
        .pause()?;
    let _ = app.emit("capture-state-changed", "paused");
    Ok(())
}

#[tauri::command]
fn resume_capture(app: AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    state
        .audio
        .lock()
        .map_err(|_| "audio state is unavailable")?
        .as_ref()
        .ok_or("capture is not active")?
        .resume()?;
    let _ = app.emit("capture-state-changed", "listening");
    Ok(())
}

#[tauri::command]
fn stop_capture(app: AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    if let Some(session) = state
        .audio
        .lock()
        .map_err(|_| "audio state is unavailable")?
        .take()
    {
        session.stop();
    }
    let _ = app.emit("capture-state-changed", "stopped");
    Ok(())
}

#[tauri::command]
fn submit_prompt(
    app: AppHandle,
    state: State<'_, AppState>,
    messages: Vec<ChatMessage>,
    model: String,
    openrouter_model: String,
    google_model: String,
    system_prompt: String,
    provider: String,
) -> Result<(), String> {
    if messages.is_empty()
        || messages
            .last()
            .is_none_or(|message| message.role != "user" || message.content.trim().is_empty())
    {
        return Err("input cannot be empty".into());
    }
    if messages
        .iter()
        .any(|message| !matches!(message.role.as_str(), "user" | "assistant"))
    {
        return Err("invalid chat message role".into());
    }
    if provider == "chatgpt" && !chatgpt::status().connected {
        return Err("ChatGPT connection was incomplete and has been cleared. Open Settings and choose Continue with ChatGPT once more.".into());
    }
    let api_key = match provider.as_str() {
        "openai" => secret_store::get_api_key()?,
        "openrouter" => secret_store::get_openrouter_key()?,
        "google" => secret_store::get_google_key()?,
        "chatgpt" => String::new(),
        _ => return Err("invalid answer provider".into()),
    };
    let openrouter_model = match openrouter_model.as_str() {
        "google/gemma-4-26b-a4b-it:free" | "openrouter/free" => openrouter_model,
        _ => "google/gemma-4-26b-a4b-it:free".into(),
    };
    let google_model = match google_model.as_str() {
        "gemini-3.8-flash" | "gemini-3.1-flash-lite" => google_model,
        _ => "gemini-3.8-flash".into(),
    };
    let fallback_key = if provider == "google" || provider == "chatgpt" {
        secret_store::get_openrouter_key().ok()
    } else {
        None
    };
    let mut task = state
        .response
        .lock()
        .map_err(|_| "response state is unavailable")?;
    if state.response_active.swap(true, Ordering::SeqCst) {
        return Err("a response is already streaming".into());
    }
    let event_app = app.clone();
    *task = Some(tauri::async_runtime::spawn(async move {
        let _ = event_app.emit("response-started", ());
        let result = match provider.as_str() {
            "chatgpt" => {
                match chatgpt::access_token().await {
                    Ok(token) => {
                        let result = openai::stream_response(event_app.clone(), token, model, system_prompt.clone(), messages.clone()).await;
                        match (result, fallback_key) {
                            (Err(error), Some(key)) => {
                                let _ = event_app.emit("response-provider-changed", openrouter_label(&openrouter_model));
                                openrouter::stream_free_response(event_app.clone(), key, openrouter_model, system_prompt, messages).await
                                    .map_err(|fallback_error| format!("ChatGPT plan request failed ({error}); OpenRouter fallback also failed: {fallback_error}"))
                            }
                            (Err(error), None) => Err(format!("ChatGPT plan answer error: {error}")),
                            (Ok(()), _) => Ok(()),
                        }
                    }
                    Err(error) => Err(format!("ChatGPT account error: {error}")),
                }
            }
            "openrouter" => openrouter::stream_free_response(
                event_app.clone(),
                api_key,
                openrouter_model,
                system_prompt,
                messages,
            )
            .await
            .map_err(|error| format!("OpenRouter answer error: {error}")),
            "google" => {
                let result = google::stream_response(
                    event_app.clone(),
                    api_key,
                    google_model,
                    system_prompt.clone(),
                    messages.clone(),
                )
                .await;
                match (result, fallback_key) {
                    (Err(error), Some(fallback_key)) if is_transient_google_error(&error) => {
                        let _ = event_app.emit(
                            "response-provider-changed",
                            openrouter_label(&openrouter_model),
                        );
                        openrouter::stream_free_response(
                            event_app.clone(),
                            fallback_key,
                            openrouter_model,
                            system_prompt,
                            messages,
                        )
                        .await
                        .map_err(|fallback_error| {
                            format!(
                                "Google Gemini was unavailable; OpenRouter fallback also failed: {fallback_error}"
                            )
                        })
                    }
                    (Err(error), None) if is_transient_google_error(&error) => Err(format!(
                        "Google Gemini is temporarily unavailable and no OpenRouter fallback key is configured: {error}"
                    )),
                    (Err(error), _) => Err(format!("Google Gemini answer error: {error}")),
                    (Ok(()), _) => Ok(()),
                }
            }
            _ => {
                openai::stream_response(event_app.clone(), api_key, model, system_prompt, messages)
                    .await
                    .map_err(|error| format!("OpenAI answer error: {error}"))
            }
        };
        if let Err(error) = result {
            let _ = event_app.emit("response-error", error);
        }
        event_app
            .state::<AppState>()
            .response_active
            .store(false, Ordering::SeqCst);
    }));
    Ok(())
}

#[tauri::command]
fn cancel_response(app: AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    if let Some(task) = state
        .response
        .lock()
        .map_err(|_| "response state is unavailable")?
        .take()
    {
        task.abort();
    }
    state.response_active.store(false, Ordering::SeqCst);
    let _ = app.emit("response-cancelled", ());
    Ok(())
}

#[tauri::command]
fn exit_app(app: AppHandle, state: State<'_, AppState>) {
    cleanup(&state);
    app.exit(0);
}

fn parse_source(source: &str) -> Result<SourceKind, String> {
    match source {
        "system" => Ok(SourceKind::SystemAudio),
        "microphone" => Ok(SourceKind::Microphone),
        _ => Err("invalid capture source".into()),
    }
}

fn cleanup(state: &State<'_, AppState>) {
    if let Ok(mut audio) = state.audio.lock() {
        if let Some(session) = audio.take() {
            session.stop();
        }
    }
    if let Ok(mut response) = state.response.lock() {
        if let Some(task) = response.take() {
            task.abort();
        }
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.show();
                let _ = window.unminimize();
                let _ = window.set_focus();
            }
        }))
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .manage(AppState::default())
        .invoke_handler(tauri::generate_handler![
            screen_protection_status,
            has_api_key,
            save_api_key,
            delete_api_key,
            has_openrouter_key,
            save_openrouter_key,
            delete_openrouter_key,
            has_google_key,
            save_google_key,
            delete_google_key,
            test_openai_connection,
            chatgpt_status,
            chatgpt_sign_in,
            chatgpt_sign_out,
            chatgpt_models,
            local_model_status,
            download_local_model,
            start_capture,
            pause_capture,
            resume_capture,
            stop_capture,
            submit_prompt,
            cancel_response,
            exit_app
        ])
        .setup(|app| {
            let main_window = app
                .get_webview_window("main")
                .ok_or("main window was not created")?;
            let protection_status = if window_protection::protect(&main_window).is_ok() {
                "protected"
            } else {
                "error"
            };
            if let Ok(mut status) = app.state::<AppState>().protection.lock() {
                *status = protection_status.into();
            }
            main_window.show()?;
            main_window.set_focus()?;
            let show = MenuItemBuilder::with_id("show", "Show / Hide").build(app)?;
            let exit = MenuItemBuilder::with_id("exit", "Exit").build(app)?;
            let menu = MenuBuilder::new(app).items(&[&show, &exit]).build()?;
            let mut tray = TrayIconBuilder::with_id("main-tray")
                .menu(&menu)
                .tooltip("InterviewAssist")
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "show" => {
                        if let Some(window) = app.get_webview_window("main") {
                            if window.is_visible().unwrap_or(false) {
                                let _ = window.hide();
                            } else {
                                let _ = window.show();
                                let _ = window.set_focus();
                            }
                        }
                    }
                    "exit" => {
                        cleanup(&app.state::<AppState>());
                        app.exit(0);
                    }
                    _ => {}
                });
            if let Some(icon) = app.default_window_icon().cloned() {
                tray = tray.icon(icon);
            }
            tray.build(app)?;
            Ok(())
        })
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = window.hide();
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running InterviewAssist");
}

#[cfg(test)]
mod tests {
    use super::{is_transient_google_error, openrouter_label};

    #[test]
    fn recognizes_google_errors_that_should_fallback() {
        assert!(is_transient_google_error(
            "This model is currently experiencing high demand. Please try again later. (HTTP 503)"
        ));
        assert!(is_transient_google_error("Quota exceeded (HTTP 429)"));
        assert!(!is_transient_google_error("API key not valid (HTTP 400)"));
    }

    #[test]
    fn labels_openrouter_fallback_model() {
        assert_eq!(
            openrouter_label("google/gemma-4-26b-a4b-it:free"),
            "Gemma 4 Fast Free"
        );
        assert_eq!(openrouter_label("openrouter/free"), "OpenRouter Auto Free");
    }
}
