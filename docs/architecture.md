# Kiến trúc kỹ thuật

## 1. Tổng quan

```text
┌──────────────────────────── Tauri application ────────────────────────────┐
│                                                                           │
│  React UI                                                                 │
│  ├─ Capture controls                                                      │
│  ├─ Editable transcript                                                   │
│  ├─ Streaming answer                                                      │
│  └─ Settings/status                                                       │
│            │ typed IPC                                                    │
│            ▼                                                              │
│  Rust native core                                                         │
│  ├─ WindowProtectionManager ── Win32 SetWindowDisplayAffinity             │
│  ├─ AudioCaptureManager ────── WASAPI loopback / microphone               │
│  ├─ TranscriptionSession ───── OpenAI streaming transcription             │
│  ├─ ResponseSession ────────── OpenAI response streaming                  │
│  ├─ SecretStore ────────────── Credential Manager / DPAPI                 │
│  ├─ HotkeyManager                                                     │
│  └─ AppState state machine                                                │
│                                                                           │
└─────────────────────────────────┬─────────────────────────────────────────┘
                                  │ TLS
                                  ▼
                              OpenAI API
```

Không có service hoặc backend do sản phẩm vận hành.

## 2. Lựa chọn công nghệ

### Tauri 2 + Rust

- Bundle nhỏ hơn Electron và có native core phù hợp với Win32/WASAPI.
- UI WebView không được trực tiếp giữ API key hoặc tự gọi OpenAI.
- Mọi thao tác nhạy cảm đi qua command/event IPC có schema rõ ràng.

### React + TypeScript

- Một main window chứa transcript và answer nhằm hạn chế native popup.
- Modal, dropdown và tooltip ưu tiên render trong DOM của main window.
- State UI phản chiếu native state; native core là nguồn sự thật cho capture/network.

## 3. Thành phần native

### 3.1 WindowProtectionManager

Trách nhiệm:

- Nhận HWND ngay khi Tauri tạo top-level window.
- Gọi `SetWindowDisplayAffinity(hwnd, WDA_EXCLUDEFROMCAPTURE)` trước khi show.
- Đọc lại affinity nếu API cho phép và lưu trạng thái từng HWND.
- Re-apply khi window được recreate.
- Tổng hợp trạng thái `Protected`, `Unavailable`, `Error` cho UI.

Không được coi thuộc tính này là DRM hoặc cơ chế chống mọi loại capture.

### 3.2 AudioCaptureManager

Interface đề xuất:

```rust
enum CaptureSource {
    SystemAudio { endpoint_id: String },
    Microphone { endpoint_id: String },
}

enum CaptureState {
    Stopped,
    Starting,
    Listening,
    Paused,
    Stopping,
    Error(String),
}
```

- `SystemAudio`: WASAPI shared-mode loopback trên render endpoint.
- `Microphone`: WASAPI shared-mode capture trên capture endpoint.
- Chuẩn hóa PCM theo format transcription yêu cầu ở native core.
- Dùng bounded queue để tránh tăng RAM vô hạn khi mạng chậm.
- Pause ngừng đưa audio frame vào queue; không tiếp tục buffer ngầm.
- Stop giải phóng COM/audio client và xóa queued audio.

MVP không yêu cầu process-specific browser capture. Windows 11 có thể bổ sung sau bằng `VIRTUAL_AUDIO_DEVICE_PROCESS_LOOPBACK`.

### 3.3 TranscriptionSession

- Chỉ tồn tại khi capture ở Listening hoặc Paused.
- Nhận PCM frame từ AudioCaptureManager.
- Gửi audio trực tiếp tới OpenAI qua kết nối streaming phù hợp.
- Phát event `partial`, `final`, `error`, `connection_state` về UI.
- UI chỉ append event `final` vào editor; `partial` nằm ở vùng riêng.
- Khi Stop, flush final segment trong khoảng timeout hữu hạn rồi đóng kết nối.

### 3.4 ResponseSession

- Nhận immutable snapshot gồm transcript, system prompt và model.
- Chỉ một request active.
- Stream text delta về UI.
- Hỗ trợ cancellation token.
- Không tự động dùng transcript mới phát sinh sau thời điểm Submit.

### 3.5 SecretStore

- API key chỉ được nhận trong native command khi người dùng Save/Test.
- Lưu trong Windows Credential Manager hoặc blob DPAPI gắn với user hiện tại.
- UI chỉ nhận `has_api_key: boolean` và masked suffix nếu cần.
- Không log header, key hoặc request body chứa transcript.
- Delete key xóa credential và credential cache trong RAM.

## 4. State machine

Capture và AI response là hai state machine độc lập:

```text
Capture: Stopped → Starting → Listening ⇄ Paused → Stopping → Stopped
                         └───────────────────────→ Error

Response: Idle → Submitting → Streaming → Completed
                 └───────────────→ Failed/Cancelled
```

Người dùng có thể Submit khi Capture đang Listening, Paused hoặc Stopped. Việc Submit không thay đổi capture state.

## 5. IPC contract

Commands tối thiểu:

- `list_audio_devices`
- `start_capture(source, language)`
- `pause_capture`
- `resume_capture`
- `stop_capture`
- `save_api_key`
- `delete_api_key`
- `test_openai_connection`
- `submit_prompt(snapshot, model, system_prompt)`
- `cancel_response`
- `set_hotkeys`
- `exit_app`

Events tối thiểu:

- `capture_state_changed`
- `transcript_partial`
- `transcript_final`
- `response_started`
- `response_delta`
- `response_completed`
- `response_failed`
- `screen_protection_changed`

Mọi command phải validate input ở native core; không tin dữ liệu từ WebView.

## 6. Client-side networking

- Request được tạo trong Rust native core qua TLS.
- Cấu hình timeout và cancellation rõ ràng.
- Retry tự động chỉ áp dụng cho lỗi kết nối trước khi server bắt đầu trả dữ liệu.
- Không retry Submit sau khi đã nhận response delta để tránh tạo hai câu trả lời/tăng chi phí.
- Không gửi audio khi Paused hoặc Stopped.
- Transcript chỉ được đưa vào response request sau thao tác Submit.

## 7. Khả năng tương thích

### Windows 10 2004+

- Dùng endpoint-wide WASAPI Loopback cho System Audio.
- Dùng `WDA_EXCLUDEFROMCAPTURE` cho top-level windows.
- Không tuyên bố chỉ thu riêng Chrome/Edge.

### Windows 11

- Cùng đường code tương thích như Windows 10 cho MVP.
- Process-specific loopback là enhancement riêng, không phải điều kiện release.

## 8. Logging

- Log local ở mức tối thiểu: lifecycle, mã lỗi, trạng thái device/network.
- Không log API key, audio, transcript, prompt hoặc answer.
- Có giới hạn kích thước và số file log.
- Bản release mặc định không bật debug logging chi tiết.

## 9. Packaging

- MSI hoặc NSIS x64 có ký code khi phát hành thật.
- Manifest khai báo Windows 10 build tối thiểu và microphone capability cần thiết.
- Installer không cài driver âm thanh hoặc service nền.
- Uninstall xóa binary/preferences; credential OpenAI phải được xóa hoặc có lựa chọn rõ ràng trong quy trình uninstall.
