# Bảo mật và quyền riêng tư

## 1. Mô hình dữ liệu

Ứng dụng không có backend riêng. Các luồng dữ liệu duy nhất:

```text
Audio source → Native app → OpenAI transcription API
Edited text → Native app → OpenAI response API
```

“100% client-side” nghĩa là không có server của InterviewAssist. Dữ liệu vẫn rời máy để được OpenAI xử lý.

## 2. Dữ liệu nhạy cảm

| Dữ liệu | Vị trí | Thời gian tồn tại mặc định |
|---|---|---|
| OpenAI API key | Windows Credential Manager/DPAPI | Đến khi người dùng xóa |
| Audio frame | RAM bounded queue | Vài giây hoặc đến khi gửi/xóa |
| Transcript | RAM/UI | Đến khi Clear hoặc Exit |
| AI Answer | RAM/UI | Đến khi Clear hoặc Exit |
| Preferences | AppData | Đến khi uninstall/xóa settings |

Không lưu audio, transcript hoặc answer xuống disk trong MVP.

## 3. Quy tắc bắt buộc

- API key không đi vào React state lâu dài, localStorage, sessionStorage hoặc URL.
- API key không được log, crash-report hoặc đưa vào error message.
- Native networking tự thêm Authorization header.
- IPC không có command `get_api_key` trả secret về UI.
- Clipboard chỉ được ghi khi người dùng nhấn Copy.
- Không tự động Submit transcript.
- Pause phải ngừng gửi audio, không chỉ dừng cập nhật UI.
- Exit phải zero/drop buffer và hủy request đang active trong phạm vi thư viện cho phép.

## 4. Logging

Được log:

- App version và OS build.
- State transition.
- Audio device ID đã hash hoặc tên không nhạy cảm nếu thực sự cần chẩn đoán.
- HTTP status, request ID và mã lỗi đã redact.

Không được log:

- API key/Authorization header.
- Raw audio hoặc PCM dump.
- Transcript, prompt, system prompt hoặc answer.
- Clipboard content.

## 5. Threat model tối thiểu

### Key bị đọc từ WebView

Giảm thiểu bằng cách giữ secret và network request trong Rust native core. UI chỉ biết key đã được cấu hình hay chưa.

### Key xuất hiện trong log/crash dump

Tạo type secret có `Debug`/`Display` đã redact, cấm log request headers và review panic/error paths.

### Audio tiếp tục truyền khi Pause

Pause tại producer/native queue; có integration test đếm frame/network bytes sau Pause.

### Window chưa được bảo vệ

Áp dụng affinity trước khi show, theo dõi từng HWND và không hiển thị trạng thái Protected nếu có window thất bại.

### Người dùng hiểu sai phạm vi bảo vệ

Settings/About phải ghi rõ phạm vi Chrome/Edge Entire Screen và các trường hợp không được bảo đảm.

## 6. Đồng ý và sử dụng hợp lệ

- UI onboarding phải nhắc người dùng tuân thủ yêu cầu đồng ý ghi âm tại nơi họ sinh sống và quy định của cuộc gọi.
- Không dùng từ ngữ quảng bá như “undetectable”, “bypass proctoring” hoặc “invisible everywhere”.
- Không tuyên bố `WDA_EXCLUDEFROMCAPTURE` là cơ chế bảo mật tuyệt đối.

## 7. Checklist trước release

- Dependency audit không có lỗ hổng critical chưa xử lý.
- DevTools và verbose logging bị tắt trong production.
- Key không xuất hiện khi tìm kiếm toàn bộ AppData/log sau phiên test.
- Không có file audio/transcript/answer sau Exit.
- Delete key xóa credential thành công.
- Uninstall behavior đối với credential được mô tả rõ và kiểm thử.
