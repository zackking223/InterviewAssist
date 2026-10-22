# Backlog triển khai MVP

## Quy ước

- `P0`: bắt buộc trước khi release.
- `P1`: cần có nhưng có thể làm sau đường critical path.
- Mỗi task chỉ được đánh Done khi có test hoặc bằng chứng kiểm thử tương ứng.
- Dependency ghi bằng ID; task không có dependency có thể thực hiện song song.

## Milestone 0 — Khởi tạo

### IA-001 — Bootstrap Tauri + React (`P0`)

- Khởi tạo Tauri 2, Rust, React và TypeScript.
- Thiết lập lint, format, unit test và release/dev config.
- Tắt DevTools trong production.
- Tạo cấu trúc module native/UI.

**Dependency:** Không.

**Status:** Done — frontend and native release builds pass.

### IA-002 — CI build Windows x64 (`P0`)

- Build debug/release trên Windows.
- Chạy Rust tests, TypeScript tests, lint và type-check.
- Lưu installer/build artifact.

**Dependency:** IA-001.

## Milestone 1 — Window shell và screen-share protection

### IA-101 — Main window và system tray (`P0`)

- Main window, Show/Hide, minimize-to-tray.
- Không hiện taskbar khi ẩn xuống tray.
- Menu tray: Show/Hide, Start/Pause, Exit.
- Exit thật sự kết thúc process sau cleanup.

**Dependency:** IA-001.

**Status:** Implemented and compile-tested; manual tray lifecycle validation remains in the acceptance pass.

### IA-102 — WindowProtectionManager (`P0`)

- Lấy HWND của mọi top-level window.
- Áp dụng `WDA_EXCLUDEFROMCAPTURE` trước khi window hiển thị.
- Trả trạng thái Protected/Unavailable/Error cho UI.
- Có test/manual harness tạo thêm window để kiểm tra bảo vệ.

**Dependency:** IA-001.

**Status:** Implemented and compile-tested; Chrome/Edge Windows 10/11 verification remains manual.

### IA-103 — UI status và safe window policy (`P0`)

- Hiển thị trạng thái screen protection rõ ràng.
- Render modal/menu/tooltip trong main window khi có thể.
- Window mới phải qua WindowProtectionManager trước khi show.

**Dependency:** IA-101, IA-102.

### IA-104 — Global hotkeys (`P1`)

- Đăng ký các hotkey mặc định.
- Cho phép đổi hotkey, phát hiện conflict và rollback cấu hình lỗi.
- Hotkey Exit cần cleanup giống nút Exit.

**Dependency:** IA-101.

## Milestone 2 — Audio capture

### IA-201 — Liệt kê audio devices (`P0`)

- Liệt kê render endpoint và capture endpoint.
- Theo dõi default-device change và device removal.
- Không hiển thị microphone trong danh sách System Audio và ngược lại.

**Dependency:** IA-001.

### IA-202 — WASAPI System Audio capture (`P0`)

- Shared-mode loopback từ render endpoint.
- Chuẩn hóa format PCM.
- Bounded buffer, cancellation và cleanup.
- Kiểm thử với loa, USB headset và Bluetooth headset.

**Dependency:** IA-201.

### IA-203 — WASAPI Microphone capture (`P0`)

- Shared-mode capture từ microphone endpoint.
- Xử lý quyền microphone và device unavailable.
- Chuẩn hóa format giống System Audio pipeline.

**Dependency:** IA-201.

### IA-204 — Audio source state machine (`P0`)

- Đảm bảo System Audio và Microphone loại trừ nhau.
- Start, Pause, Continue và Stop.
- Pause không buffer ngầm; Stop giải phóng device.
- Chuyển nguồn theo đúng quy tắc trong PRD.

**Dependency:** IA-202, IA-203.

## Milestone 3 — OpenAI và secret storage

### IA-301 — SecretStore (`P0`)

- Save/Get/Delete API key bằng Credential Manager hoặc DPAPI.
- Không trả key đầy đủ cho UI.
- Redact key khỏi log/error.

**Dependency:** IA-001.

### IA-302 — OpenAI settings (`P0`)

- Form nhập, replace, delete key.
- Test connection, hiển thị lỗi auth/quota/network.
- Model và system prompt settings.

**Dependency:** IA-301.

### IA-303 — Streaming transcription client (`P0`)

- Kết nối trực tiếp từ Rust native core.
- Gửi PCM từ audio pipeline.
- Hỗ trợ Auto/English/Vietnamese.
- Chuẩn hóa partial/final/error events.
- Pause/Continue/Stop không tạo audio ngoài mong muốn.

**Dependency:** IA-204, IA-301.

### IA-304 — Streaming response client (`P0`)

- Gửi immutable snapshot chỉ sau Submit.
- Stream delta, complete và error events.
- Hỗ trợ Cancel và một request active.
- Không retry request đã bắt đầu stream.

**Dependency:** IA-301.

## Milestone 4 — Product UI

### IA-401 — Capture control panel (`P0`)

- Chọn nguồn, device và ngôn ngữ.
- Start/Pause/Continue/Stop với trạng thái disable hợp lệ.
- Hiển thị device/network error không làm mất transcript.

**Dependency:** IA-103, IA-204, IA-303.

### IA-402 — Editable live-caption editor (`P0`)

- Partial caption riêng và final transcript append an toàn.
- Select/copy/cut/paste/undo/redo/edit/type.
- Clear transcript.
- Bảo vệ nội dung user edit khỏi partial event đến muộn.

**Dependency:** IA-303.

### IA-403 — Submit và Answer panel (`P0`)

- Submit bằng button và `Ctrl+Enter`.
- Không gửi whitespace; disable khi request đang active.
- Stream answer, Copy và Cancel response.
- Không xóa transcript khi request lỗi.

**Dependency:** IA-304, IA-402.

### IA-404 — Settings và exit flows (`P0`)

- OpenAI settings, hotkey settings và audio defaults.
- Minimize, close-to-tray và explicit Exit.
- Unsaved editor content không cần persistence nhưng Exit phải rõ ràng.

**Dependency:** IA-101, IA-104, IA-302.

## Milestone 5 — Hardening và release

### IA-501 — Automated tests (`P0`)

- Unit test state machines, IPC validation và redaction.
- UI tests cho editor/submit/state transitions.
- Integration test mock OpenAI streaming/cancellation.

**Dependency:** IA-303, IA-304, IA-403.

### IA-502 — Windows compatibility matrix (`P0`)

- Test Windows 10 22H2 và Windows 11 hiện hành.
- Chrome và Edge share Entire Screen.
- DPI 100/125/150%, một và hai màn hình.
- Audio devices theo checklist nghiệm thu.

**Dependency:** IA-103, IA-401, IA-403.

### IA-503 — Security/privacy review (`P0`)

- Xác nhận key không vào WebView/localStorage/log.
- Xác nhận không lưu audio/transcript/answer.
- Kiểm tra TLS, dependency audit và production config.
- Kiểm tra cleanup credential khi user chọn Delete key.

**Dependency:** IA-301, IA-303, IA-304.

### IA-504 — Installer và release candidate (`P0`)

- Tạo installer x64.
- Kiểm thử install/upgrade/uninstall trên máy sạch.
- Hiển thị minimum OS và phạm vi screen protection.
- Chuẩn bị code signing cho bản phát hành.

**Dependency:** IA-002, IA-501, IA-502, IA-503.

## Critical path

```text
IA-001
  ├─ IA-101 → IA-102/103 ───────────────────────┐
  ├─ IA-201 → IA-202/203 → IA-204 → IA-303 ────┤
  └─ IA-301 → IA-302/304 ──────────────────────┤
                                                ▼
                               IA-401/402/403 → IA-501/502/503 → IA-504
```

## Definition of Done cho MVP

- Tất cả task P0 hoàn thành.
- Không còn lỗi crash/data exposure mức blocker hoặc critical.
- Toàn bộ test bắt buộc trong `acceptance-tests.md` đạt trên Windows 10 22H2 và Windows 11.
- Installer chạy được trên máy sạch không có dev tool.
- Tài liệu giới hạn screen capture và xử lý dữ liệu xuất hiện trong app/release notes.
