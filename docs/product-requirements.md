# Đặc tả sản phẩm MVP

## 1. Mục tiêu

Tạo một ứng dụng Windows gọn nhẹ có thể thu một nguồn âm thanh, tạo live caption, cho phép người dùng chỉnh sửa nội dung và chỉ gửi nội dung đó tới OpenAI khi có thao tác Submit rõ ràng.

## 2. Nền tảng hỗ trợ

- Tối thiểu: Windows 10 version 2004, build 19041.
- Tối đa: các bản Windows 11 còn được hỗ trợ.
- Kiến trúc MVP: x64.
- Browser kiểm thử chính: Chrome và Edge.
- Tình huống screen sharing: browser gọi `getDisplayMedia` và người dùng chọn `Entire Screen`.

Windows trước build 19041 không thuộc phạm vi vì không đáp ứng đúng hành vi loại cửa sổ khỏi capture bằng `WDA_EXCLUDEFROMCAPTURE`.

## 3. Người dùng và luồng chính

1. Người dùng mở ứng dụng từ desktop hoặc system tray.
2. Lần đầu sử dụng, người dùng nhập OpenAI API key và kiểm tra kết nối.
3. Người dùng chọn ngôn ngữ: `Auto`, `English` hoặc `Tiếng Việt`.
4. Người dùng chọn đúng một nguồn: `System Audio` hoặc `Microphone`.
5. Người dùng nhấn Start; caption xuất hiện theo thời gian thực.
6. Người dùng có thể Pause/Continue, copy hoặc chỉnh sửa transcript.
7. Người dùng nhấn Submit hoặc hotkey; ứng dụng chụp snapshot nội dung hiện tại và gửi snapshot đó tới OpenAI.
8. Câu trả lời được stream vào vùng Answer và có thể copy.
9. Người dùng tiếp tục nghe, tạo câu hỏi mới, thu nhỏ xuống tray hoặc Exit.

## 4. Chế độ audio

### 4.1 System Audio

- Thu âm thanh từ output device đang được chọn bằng WASAPI Loopback.
- Hoạt động khi người dùng dùng tai nghe; không cần bật loa vật lý.
- Không mở hoặc thu microphone.
- Trên Windows 10, có thể bao gồm notification và âm thanh từ ứng dụng khác dùng cùng output device.
- UI phải gọi đúng tên `System Audio`, không tuyên bố luôn chỉ thu riêng browser.

### 4.2 Microphone

- Chỉ thu microphone mà người dùng chọn.
- Không mở hoặc thu system audio.
- Hệ điều hành có thể yêu cầu quyền microphone.

### 4.3 Quy tắc chuyển nguồn

- Hai nguồn loại trừ nhau và không bao giờ được active đồng thời.
- Chuyển nguồn khi đang Listening phải dừng session hiện tại, flush đoạn caption cuối, rồi mới mở nguồn mới.
- Nếu mở nguồn mới thất bại, trạng thái chuyển về Stopped; không tự quay lại thu nguồn cũ.

## 5. Trạng thái capture

| Trạng thái | Ý nghĩa | Thao tác hợp lệ |
|---|---|---|
| Stopped | Không có audio session | Start, đổi nguồn, Settings, Exit |
| Listening | Đang thu và transcribe | Pause, Stop, Submit |
| Paused | Session giữ cấu hình nhưng không gửi audio mới | Continue, Stop, Submit |
| Error | Session gặp lỗi | Retry/Start, Stop, Settings, Exit |

- Pause không xóa transcript.
- Continue nối tiếp vào transcript hiện tại.
- Stop đóng audio device và transcription connection.
- Không tự động Start khi ứng dụng khởi động.

## 6. Live caption và input editor

- Hỗ trợ `Auto`, `English`, `Tiếng Việt`.
- Partial transcript đang hình thành được hiển thị riêng, có kiểu chữ mờ.
- Final transcript được append vào editor.
- Editor hỗ trợ select, copy, cut, paste, undo, redo, sửa và gõ tay.
- Không để partial transcript ghi đè lên phần người dùng vừa sửa.
- Clear transcript cần một thao tác rõ ràng; không xóa Answer.
- Transcript không tự động gửi sang AI trong bất kỳ trường hợp nào.

## 7. Submit và Answer

- Submit được kích hoạt bằng nút hoặc hotkey.
- Payload là snapshot của editor tại thời điểm Submit.
- Không Submit nếu nội dung chỉ gồm whitespace.
- Trong khi request đang chạy, nội dung editor vẫn có thể tiếp tục thay đổi mà không làm đổi payload đã gửi.
- Answer được stream dần và có nút Copy.
- Chỉ cho phép một request AI active trong MVP. Submit lần nữa khi đang Answering phải được disable.
- Có nút Cancel response để hủy request đang stream.
- Lỗi xác thực, quota, rate limit, timeout và mất mạng phải có thông báo riêng, không làm mất transcript.

## 8. OpenAI Settings

- Provider duy nhất: OpenAI API.
- Người dùng nhập API key và có thể Replace/Delete key.
- API key không được hiển thị lại toàn bộ sau khi lưu.
- Có thao tác Test connection.
- Model trả lời là cấu hình ứng dụng có giá trị mặc định; có thể đổi trong Settings.
- Prompt hệ thống có giá trị mặc định ngắn, có thể chỉnh sửa và reset.
- Ghi rõ ChatGPT subscription không tự động bao gồm API key hoặc API billing.

## 9. Giao diện và window behavior

Giao diện chính phải có:

- Trạng thái bảo vệ screen sharing: `Protected`, `Unavailable` hoặc `Error`.
- Chọn nguồn audio và input device tương ứng.
- Chọn ngôn ngữ.
- Start/Pause/Continue/Stop.
- Transcript editor và partial-caption indicator.
- Submit/Clear/Copy.
- Answer panel, Copy answer và Cancel response.
- Settings, Minimize và Exit.

Window behavior:

- Main window luôn nổi khi tùy chọn Always on top được bật.
- Minimize và nút đóng cửa sổ mặc định đưa app xuống system tray.
- Menu tray có Show/Hide, Start/Pause và Exit.
- Exit đóng audio session, hủy network request, xóa dữ liệu trong RAM và kết thúc process.
- Không hiện icon trên taskbar khi đang chạy ở tray.

## 10. Hotkey mặc định

| Hotkey | Hành động |
|---|---|
| `Ctrl+Shift+L` | Start/Pause/Continue |
| `Ctrl+Enter` | Submit |
| `Ctrl+Shift+C` | Copy Answer |
| `Ctrl+Shift+H` | Show/Hide |
| `Ctrl+Shift+Q` | Exit |

- Hotkey phải configurable.
- Nếu đăng ký hotkey thất bại vì trùng, hiển thị cảnh báo và không ghi nhận cấu hình mới.

## 11. Bảo vệ khi browser share Entire Screen

- Mọi top-level window do app tạo phải được áp dụng `WDA_EXCLUDEFROMCAPTURE`.
- Bao gồm main window, Settings, dialog, popup, caption/answer window nếu được tách riêng.
- Ưu tiên dialog/menu render bên trong một window để giảm số native window cần bảo vệ.
- App phải kiểm tra kết quả gọi Win32 API cho từng window.
- Chỉ hiển thị `Protected` khi tất cả window hiện có đã áp dụng thành công.
- Window mới tạo sau đó phải được bảo vệ trước khi show.
- Nếu thất bại, hiển thị `Screen-share protection unavailable`; không âm thầm tiếp tục với nhãn Protected.

Cam kết chỉ áp dụng cho Chrome/Edge share Entire Screen trên nền tảng đã kiểm thử. Không cam kết cho OBS, screenshot, ứng dụng họp desktop, remote desktop, máy ảo, camera vật lý hoặc capture cấp driver.

## 12. Dữ liệu và lưu trữ

- Transcript, Answer và audio buffer mặc định chỉ nằm trong RAM.
- Không lưu file audio.
- Không lưu lịch sử phiên.
- API key được lưu bằng Windows Credential Manager hoặc DPAPI.
- Preferences không nhạy cảm có thể lưu local dưới AppData.
- Không có backend, analytics hoặc telemetry trong MVP.

## 13. Ngoài phạm vi

- Nhận diện người nói/diarization.
- Thu đồng thời System Audio và Microphone.
- Tự động phát hiện câu hỏi và tự động Submit.
- OCR hoặc đọc nội dung màn hình.
- AI provider ngoài OpenAI.
- Đăng nhập, subscription, thanh toán hoặc license server.
- Lưu, tìm kiếm, xuất hoặc đồng bộ lịch sử.
- Auto update trong bản MVP đầu tiên.
