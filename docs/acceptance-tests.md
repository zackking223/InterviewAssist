# Tiêu chí nghiệm thu MVP

## 1. Môi trường bắt buộc

Kiểm thử tối thiểu trên:

- Windows 10 22H2 x64.
- Windows 11 x64 còn được hỗ trợ.
- Chrome stable hiện hành.
- Edge stable hiện hành.
- Một màn hình và hai màn hình.
- DPI 100%, 125% và 150%.

## 2. Screen sharing

### AT-SCREEN-01 — Chrome Entire Screen

1. Mở app và xác nhận trạng thái Protected.
2. Mở Chrome, bắt đầu `getDisplayMedia` và chọn Entire Screen.
3. Di chuyển main window qua vùng nội dung khác nhau.
4. Mở Settings và các dialog của app.

**Đạt khi:** Không có window, popup hoặc dialog nào của app xuất hiện trong preview/remote stream; desktop phía sau không bị che bằng hình chữ nhật đen.

### AT-SCREEN-02 — Edge Entire Screen

Lặp lại AT-SCREEN-01 bằng Edge.

### AT-SCREEN-03 — Protection failure

Giả lập hoặc inject lỗi khi áp dụng affinity.

**Đạt khi:** UI hiển thị Unavailable/Error và không hiển thị Protected.

### AT-SCREEN-04 — Window lifecycle

Ẩn/hiện, minimize/restore, thay DPI, chuyển màn hình và recreate window.

**Đạt khi:** Protection vẫn được áp dụng sau mọi lifecycle event.

## 3. System Audio

### AT-AUDIO-01 — Headset playback

1. Chọn System Audio.
2. Phát cuộc gọi/video trong browser qua tai nghe.
3. Không bật microphone.

**Đạt khi:** Caption nhận được nội dung; microphone không được mở; không cần phát qua loa vật lý.

### AT-AUDIO-02 — System scope disclosure

Phát đồng thời browser audio và notification từ ứng dụng khác.

**Đạt khi:** App không tuyên bố chỉ thu browser; UI mô tả đúng rằng System Audio có thể nhận cả hai.

### AT-AUDIO-03 — Pause/Continue

1. Start System Audio và tạo caption.
2. Pause rồi tiếp tục phát âm thanh.
3. Continue và phát câu mới.

**Đạt khi:** Âm thanh trong thời gian Pause không xuất hiện sau đó; transcript cũ còn nguyên; câu mới được append.

### AT-AUDIO-04 — Device changes

Đổi default output, rút USB headset hoặc ngắt Bluetooth khi đang Listening.

**Đạt khi:** App không crash, báo lỗi có thể hiểu và cho phép chọn/restart device.

## 4. Microphone

### AT-MIC-01 — Mutual exclusion

1. Chọn Microphone và Start.
2. Phát browser audio đồng thời nói vào mic.

**Đạt khi:** Chỉ microphone được mở; app không đồng thời mở System Audio session.

### AT-MIC-02 — Permission denied

Từ chối quyền microphone.

**Đạt khi:** App báo đúng lỗi, về Stopped/Error hợp lệ và không crash.

### AT-MIC-03 — Switch source

Chuyển từ Microphone đang Listening sang System Audio.

**Đạt khi:** Session mic được đóng trước khi loopback session bắt đầu; không có khoảng overlap hai nguồn.

## 5. Caption editor

### AT-CAPTION-01 — English/Vietnamese/Auto

Đọc các đoạn tiếng Anh, tiếng Việt và xen kẽ ở từng cấu hình.

**Đạt khi:** Caption được tạo đúng luồng; UI không treo; language setting được gửi đúng.

### AT-CAPTION-02 — Partial versus final

Sửa một phần transcript đã final trong khi partial caption mới đang đến.

**Đạt khi:** Partial không ghi đè hoặc làm mất phần người dùng sửa.

### AT-CAPTION-03 — Editing

Thử select, copy, cut, paste, undo, redo, xóa và gõ tay.

**Đạt khi:** Hành vi giống text editor thông thường.

## 6. Submit và AI response

### AT-AI-01 — Explicit submit only

Start caption và chờ ít nhất một phút nhưng không Submit.

**Đạt khi:** Không có response request nào được gửi tới OpenAI.

### AT-AI-02 — Immutable snapshot

1. Nhập `Question A` và Submit.
2. Ngay lập tức đổi editor thành `Question B`.

**Đạt khi:** Request đầu dùng Question A; editor vẫn giữ Question B.

### AT-AI-03 — Streaming and copy

Submit một câu hỏi hợp lệ, chờ answer stream rồi Copy.

**Đạt khi:** Text hiển thị dần, hoàn tất đúng trạng thái và clipboard chứa toàn bộ answer hiện có.

### AT-AI-04 — Empty/duplicate submit

- Submit whitespace.
- Nhấn Submit lần nữa khi response đang active.

**Đạt khi:** Không tạo request cho whitespace và không tạo request active thứ hai.

### AT-AI-05 — Cancellation and errors

Kiểm thử Cancel, key sai, hết quota, rate limit, timeout và mất mạng.

**Đạt khi:** Có thông báo phân biệt được; transcript không mất; request/socket được đóng khi Cancel.

## 7. API key và privacy

### AT-SEC-01 — Secret storage

Lưu key, đóng/mở app và kiểm tra preferences/log/localStorage.

**Đạt khi:** App dùng được key sau restart nhưng key đầy đủ không xuất hiện trong file preferences, WebView storage hoặc log.

### AT-SEC-02 — Delete key

Chọn Delete key rồi khởi động lại app.

**Đạt khi:** Credential không còn và app yêu cầu nhập key trước khi gọi OpenAI.

### AT-SEC-03 — No history persistence

Tạo transcript/answer rồi Exit và mở lại.

**Đạt khi:** Transcript và answer cũ không được phục hồi; không có audio file được tạo.

## 8. Tray, hotkey và thoát

### AT-APP-01 — Close-to-tray

Nhấn nút đóng window.

**Đạt khi:** Window biến mất, app còn trong tray, capture state không đổi ngoài ý muốn và không có icon taskbar.

### AT-APP-02 — Explicit Exit

Exit từ UI, tray và hotkey trong ba lần kiểm thử riêng.

**Đạt khi:** Audio/network session được đóng và không còn process của app.

### AT-APP-03 — Hotkey conflict

Cấu hình một hotkey đã bị ứng dụng khác chiếm.

**Đạt khi:** App báo conflict và giữ cấu hình hotkey cũ.

## 9. Release gate

- Tất cả test `AT-SCREEN-*`, `AT-SEC-*` và `AT-APP-02` bắt buộc đạt.
- Không có crash trong phiên soak test hai giờ gồm capture, pause/continue và nhiều lần Submit.
- RAM không tăng liên tục khi Listening hoặc Paused.
- Installer cài/chạy/gỡ được trên máy sạch.
