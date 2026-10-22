use crate::audio::AudioMessage;
use futures_util::StreamExt;
use serde::Serialize;
use sha1::{Digest, Sha1};
use std::path::PathBuf;
use tauri::{AppHandle, Emitter, Manager};
use tokio::{io::AsyncWriteExt, sync::mpsc::UnboundedReceiver};
use whisper_rs::{
    FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters, WhisperState,
};

const MODEL_ROOT: &str = "https://huggingface.co/ggerganov/whisper.cpp/resolve/main";

struct ModelSpec {
    id: &'static str,
    name: &'static str,
    file: &'static str,
    size_label: &'static str,
    description: &'static str,
    sha1: &'static str,
}

const MODELS: &[ModelSpec] = &[
    ModelSpec {
        id: "small",
        name: "Whisper Small",
        file: "ggml-small.bin",
        size_label: "466 MiB",
        description: "Nhanh, chất lượng khá",
        sha1: "55356645c2b361a969dfd0ef2c5a50d530afd8d5",
    },
    ModelSpec {
        id: "medium-q5",
        name: "Whisper Medium Q5",
        file: "ggml-medium-q5_0.bin",
        size_label: "514 MiB",
        description: "Chính xác hơn Small, đã nén",
        sha1: "7718d4c1ec62ca96998f058114db98236937490e",
    },
    ModelSpec {
        id: "large-v3-turbo-q5",
        name: "Large v3 Turbo Q5",
        file: "ggml-large-v3-turbo-q5_0.bin",
        size_label: "547 MiB",
        description: "Khuyên dùng nếu CPU đủ nhanh",
        sha1: "e050f7970618a659205450ad97eb95a18d69c9ee",
    },
    ModelSpec {
        id: "large-v3-turbo",
        name: "Large v3 Turbo",
        file: "ggml-large-v3-turbo.bin",
        size_label: "1.5 GiB",
        description: "Chất lượng cao · chậm khi chỉ chạy CPU",
        sha1: "4af2b29d7ec73d781377bfd1758ca957a807e941",
    },
];

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalModelStatus {
    id: String,
    name: String,
    size_label: String,
    description: String,
    installed: bool,
    path: String,
    size_bytes: u64,
}

fn model_spec(id: &str) -> Result<&'static ModelSpec, String> {
    MODELS
        .iter()
        .find(|model| model.id == id)
        .ok_or_else(|| "Unknown local Whisper model".into())
}

fn model_path(app: &AppHandle, spec: &ModelSpec) -> Result<PathBuf, String> {
    Ok(app
        .path()
        .app_data_dir()
        .map_err(|error| error.to_string())?
        .join("models")
        .join(spec.file))
}

pub fn statuses(app: &AppHandle) -> Result<Vec<LocalModelStatus>, String> {
    MODELS
        .iter()
        .map(|spec| {
            let path = model_path(app, spec)?;
            let metadata = std::fs::metadata(&path).ok();
            Ok(LocalModelStatus {
                id: spec.id.into(),
                name: spec.name.into(),
                size_label: spec.size_label.into(),
                description: spec.description.into(),
                installed: metadata.is_some(),
                size_bytes: metadata.map(|value| value.len()).unwrap_or(0),
                path: path.display().to_string(),
            })
        })
        .collect()
}

pub async fn download(app: AppHandle, model_id: String) -> Result<Vec<LocalModelStatus>, String> {
    let spec = model_spec(&model_id)?;
    let destination = model_path(&app, spec)?;
    if destination.exists() {
        return statuses(&app);
    }
    let parent = destination.parent().ok_or("invalid model directory")?;
    tokio::fs::create_dir_all(parent)
        .await
        .map_err(|error| error.to_string())?;
    let temporary = destination.with_extension("bin.download");
    let response = reqwest::get(format!("{MODEL_ROOT}/{}", spec.file))
        .await
        .map_err(|error| error.to_string())?
        .error_for_status()
        .map_err(|error| error.to_string())?;
    let total = response.content_length().unwrap_or(0);
    let mut stream = response.bytes_stream();
    let mut file = tokio::fs::File::create(&temporary)
        .await
        .map_err(|error| error.to_string())?;
    let mut received = 0_u64;
    let mut hasher = Sha1::new();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|error| error.to_string())?;
        file.write_all(&chunk)
            .await
            .map_err(|error| error.to_string())?;
        hasher.update(&chunk);
        received += chunk.len() as u64;
        let percent = if total > 0 {
            (received * 100 / total).min(100)
        } else {
            0
        };
        let _ = app.emit(
            "local-model-download-progress",
            serde_json::json!({ "modelId": spec.id, "percent": percent }),
        );
    }
    file.flush().await.map_err(|error| error.to_string())?;
    drop(file);
    let actual = hex::encode(hasher.finalize());
    if actual != spec.sha1 {
        let _ = tokio::fs::remove_file(&temporary).await;
        return Err("Local model checksum verification failed. Please download it again.".into());
    }
    tokio::fs::rename(&temporary, &destination)
        .await
        .map_err(|error| error.to_string())?;
    let _ = app.emit(
        "local-model-download-progress",
        serde_json::json!({ "modelId": spec.id, "percent": 100 }),
    );
    statuses(&app)
}

pub fn start(
    app: AppHandle,
    language: String,
    model_id: String,
    mut receiver: UnboundedReceiver<AudioMessage>,
) -> Result<(), String> {
    let path = model_path(&app, model_spec(&model_id)?)?;
    if !path.exists() {
        return Err(
            "Local Whisper model is not installed. Open Settings and download it first.".into(),
        );
    }
    std::thread::Builder::new()
        .name("local-whisper".into())
        .spawn(move || {
            if let Err(error) = run(app.clone(), path, language, &mut receiver) {
                let _ = app.emit("transcription-error", error);
            }
        })
        .map_err(|error| error.to_string())?;
    Ok(())
}

fn run(
    app: AppHandle,
    path: PathBuf,
    language: String,
    receiver: &mut UnboundedReceiver<AudioMessage>,
) -> Result<(), String> {
    let _ = app.emit("transcript-partial", "Loading local model…");
    let context = WhisperContext::new_with_params(
        path.to_str().ok_or("model path is not valid UTF-8")?,
        WhisperContextParameters::default(),
    )
    .map_err(|error| error.to_string())?;
    let mut state = context.create_state().map_err(|error| error.to_string())?;
    let _ = app.emit("transcript-partial", "Model ready · waiting for speech…");
    let mut audio = Vec::<i16>::new();
    let mut silence_samples = 0_usize;
    let mut speaking = false;
    while let Some(message) = receiver.blocking_recv() {
        match message {
            AudioMessage::Flush => {
                flush_audio(&app, &mut state, &mut audio, &language)?;
                speaking = false;
                silence_samples = 0;
            }
            AudioMessage::Stop => {
                flush_audio(&app, &mut state, &mut audio, &language)?;
                break;
            }
            AudioMessage::Samples(samples) => {
                let loud = rms(&samples) >= 120.0;
                if loud {
                    if !speaking {
                        let _ = app.emit("transcript-partial", "Audio detected…");
                    }
                    speaking = true;
                    silence_samples = 0;
                } else if speaking {
                    silence_samples += samples.len();
                }
                if speaking {
                    audio.extend_from_slice(&samples);
                }

                // Finalize on a short pause or at 3.5 seconds. Never re-run the
                // same growing buffer: that caused CPU models to fall far behind.
                if speaking && (silence_samples >= 9_600 || audio.len() >= 84_000) {
                    flush_audio(&app, &mut state, &mut audio, &language)?;
                    speaking = false;
                    silence_samples = 0;
                }
            }
        }
    }
    Ok(())
}

fn flush_audio(
    app: &AppHandle,
    state: &mut WhisperState,
    audio: &mut Vec<i16>,
    language: &str,
) -> Result<(), String> {
    if audio.len() >= 7_200 {
        let _ = app.emit("transcript-partial", "Transcribing locally…");
        let text = transcribe(state, audio, language)?;
        if !text.is_empty() {
            let _ = app.emit("transcript-final", text);
        }
    } else {
        let _ = app.emit("transcript-partial", "Model ready · waiting for speech…");
    }
    audio.clear();
    Ok(())
}

fn transcribe(state: &mut WhisperState, pcm_24k: &[i16], language: &str) -> Result<String, String> {
    let pcm_16k = resample_24k_to_16k(pcm_24k);
    let threads = std::thread::available_parallelism()
        .map(|v| v.get())
        .unwrap_or(4)
        .min(12);
    let selected_language = match language {
        "en" => "en",
        "vi" => "vi",
        _ => detect_english_or_vietnamese(state, &pcm_16k, threads)?,
    };
    let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
    params.set_n_threads(threads as i32);
    params.set_print_progress(false);
    params.set_print_realtime(false);
    params.set_print_special(false);
    params.set_translate(false);
    params.set_no_context(true);
    params.set_single_segment(true);
    params.set_suppress_blank(true);
    params.set_temperature_inc(0.0);
    // Native auto mode considers 100 languages. Live mode intentionally
    // compares only EN and VI, then forces decoding to the stronger one.
    params.set_language(Some(selected_language));
    state
        .full(params, &pcm_16k)
        .map_err(|error| error.to_string())?;
    let count = state.full_n_segments();
    let mut text = String::new();
    for index in 0..count {
        let segment = state
            .get_segment(index)
            .ok_or("Whisper returned an invalid segment")?;
        text.push_str(&segment.to_str_lossy().map_err(|error| error.to_string())?);
    }
    Ok(collapse_repeated_halves(text.trim()))
}

fn detect_english_or_vietnamese(
    state: &mut WhisperState,
    pcm_16k: &[f32],
    threads: usize,
) -> Result<&'static str, String> {
    state
        .pcm_to_mel(pcm_16k, threads)
        .map_err(|error| error.to_string())?;
    let (_, probabilities) = state
        .lang_detect(0, threads)
        .map_err(|error| error.to_string())?;
    let en = whisper_rs::get_lang_id("en").ok_or("Whisper has no English language id")? as usize;
    let vi = whisper_rs::get_lang_id("vi").ok_or("Whisper has no Vietnamese language id")? as usize;
    let english_score = probabilities.get(en).copied().unwrap_or(0.0);
    let vietnamese_score = probabilities.get(vi).copied().unwrap_or(0.0) * 1.25;
    Ok(if vietnamese_score >= english_score {
        "vi"
    } else {
        "en"
    })
}

fn collapse_repeated_halves(value: &str) -> String {
    let mut words: Vec<&str> = value.split_whitespace().collect();
    loop {
        let half = words.len() / 2;
        if words.len() < 4 || words.len() % 2 != 0 {
            break;
        }
        let repeated = words[..half]
            .iter()
            .zip(&words[half..])
            .all(|(left, right)| left.eq_ignore_ascii_case(right));
        if !repeated {
            break;
        }
        words.truncate(half);
    }
    words.join(" ")
}

fn rms(samples: &[i16]) -> f64 {
    if samples.is_empty() {
        return 0.0;
    }
    let sum: f64 = samples.iter().map(|v| (*v as f64) * (*v as f64)).sum();
    (sum / samples.len() as f64).sqrt()
}

fn resample_24k_to_16k(input: &[i16]) -> Vec<f32> {
    let output_len = input.len() * 2 / 3;
    (0..output_len)
        .map(|index| {
            let position = index as f64 * 1.5;
            let left = position.floor() as usize;
            let fraction = (position - left as f64) as f32;
            let a = input[left] as f32;
            let b = input.get(left + 1).copied().unwrap_or(input[left]) as f32;
            (a + (b - a) * fraction) / 32768.0
        })
        .collect()
}
