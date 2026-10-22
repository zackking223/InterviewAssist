# InterviewAssist 0.1.0 — MVP build

## Implemented

- Windows 10 version 2004+ and Windows 11 desktop shell using Tauri 2.
- `WDA_EXCLUDEFROMCAPTURE` applied before the main window is shown.
- Always-on-top, hidden taskbar entry, system tray, Show/Hide and explicit Exit.
- Mutually exclusive System Audio and Microphone capture using CPAL/WASAPI.
- WASAPI output-device loopback works with headphones and does not require external speakers.
- Audio downmix and resampling to mono PCM16 at 24 kHz.
- Start, Pause, Continue and Stop state handling.
- OpenAI Realtime transcription using `gpt-live-transcribe` with English/Vietnamese hints.
- Editable partial/final live captions.
- Explicit-only Submit to OpenAI Responses API with streaming answer and cancellation.
- OpenAI API key storage through Windows Credential Manager.
- Settings for API key, response model and system prompt.
- Default global hotkeys for capture, submit, Show/Hide and Exit.
- NSIS current-user installer for Windows x64.

## Automatically verified

- TypeScript type-check and Vite production build.
- Rust `cargo check` with the MSVC toolchain.
- Rust unit test for stereo downmix/resampling.
- Release-mode Rust compilation and NSIS packaging.
- npm dependency audit reported zero known vulnerabilities.

## Requires manual validation

The application was not launched automatically. The following tests require a human-operated session:

- Entering a real OpenAI API key and testing account/model access.
- Live transcription latency and quality with representative English/Vietnamese audio.
- System Audio behavior across built-in, USB and Bluetooth endpoints.
- Microphone permission-denied and device-removal scenarios.
- Chrome/Edge Entire Screen sharing on Windows 10 and Windows 11.
- Global hotkey conflicts with software already installed on the target machine.

## Known scope limits

- System Audio captures the selected output endpoint; on Windows 10 this may include sounds from applications other than the browser.
- Screen-share exclusion is scoped to supported Chrome/Edge Entire Screen capture paths and is not a guarantee against OBS, driver capture, remote desktop or physical cameras.
- The installer is not code-signed in this development build, so Windows may display an unknown-publisher warning.
