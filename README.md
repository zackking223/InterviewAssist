# InterviewAssist

InterviewAssist là ứng dụng desktop Windows hỗ trợ live caption và gửi nội dung do người dùng chủ động chọn sang nhà cung cấp AI đã cấu hình để nhận câu trả lời. Ứng dụng chạy hoàn toàn ở phía client, không có backend của sản phẩm.

## Phạm vi MVP

- Windows 10 version 2004 (build 19041) đến Windows 11, x64.
- Hai nguồn thu loại trừ nhau: `System Audio` hoặc `Microphone`.
- Live caption tiếng Anh, tiếng Việt và tự nhận diện bằng Local Whisper (offline) hoặc OpenAI.
- Transcript có thể chọn, copy, sửa và gõ thêm.
- Chỉ gọi AI khi người dùng nhấn Submit hoặc hotkey.
- Hỗ trợ OpenAI, OpenRouter Free Router hoặc Google AI Studio/Gemini Free Tier cho câu trả lời; người dùng tự cung cấp API key tương ứng.
- Pause, Continue, Stop, Clear, Copy, Minimize và Exit hoàn toàn.
- Tất cả cửa sổ của ứng dụng được loại khỏi luồng share `Entire Screen` của Chrome/Edge khi hệ điều hành hỗ trợ.
- Không có tài khoản, backend, đồng bộ cloud, analytics hoặc lưu lịch sử mặc định.

## Tài liệu

- [Đặc tả sản phẩm](docs/product-requirements.md)
- [Kiến trúc kỹ thuật](docs/architecture.md)
- [Backlog triển khai](docs/task-breakdown.md)
- [Tiêu chí nghiệm thu](docs/acceptance-tests.md)
- [Bảo mật và quyền riêng tư](docs/security-privacy.md)
- [Thiết lập môi trường phát triển](DEVELOPMENT.md)
- [Release notes 0.1.0](docs/release-notes-0.1.0.md)

## Current build

- Native application: `D:\iat\release\interview-assist.exe`
- Windows x64 installer: `D:\iat\release\bundle\nsis\InterviewAssist_0.1.0_x64-setup.exe`
- The development build is unsigned.

## Build và chạy

### 1. Yêu cầu môi trường

- Windows 10 version 2004+ hoặc Windows 11 x64.
- Node.js 22+ và npm.
- Rust stable với target `x86_64-pc-windows-msvc`.
- Visual Studio 2022 Build Tools, workload `Desktop development with C++`.
- LLVM/Clang, CMake và Vulkan SDK (dùng để build Local Whisper có GPU acceleration).
- Microsoft Edge WebView2 Runtime.

Cài Rust và Visual C++ Build Tools bằng `winget`:

```powershell
winget install --id Rustlang.Rustup --exact
winget install --id Microsoft.VisualStudio.2022.BuildTools --exact --override "--wait --passive --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended"
winget install --id LLVM.LLVM --exact
winget install --id Kitware.CMake --exact
winget install --id KhronosGroup.VulkanSDK --exact
```

Sau khi cài, mở terminal PowerShell mới. Kiểm tra môi trường:

```powershell
node --version
npm --version
rustc --version
cargo --version
```

Nếu terminal hiện tại chưa tìm thấy Cargo:

```powershell
$env:Path = "$env:USERPROFILE\.cargo\bin;$env:Path"
```

Repository đã có `.cargo/config.toml` trỏ tới vị trí cài đặt mặc định của LLVM và CMake. Nếu bạn cài hai công cụ ở vị trí khác và build báo không tìm thấy chúng, đặt đường dẫn cho phiên hiện tại:

```powershell
$env:LIBCLANG_PATH = "C:\Program Files\LLVM\bin"
$env:Path = "C:\Program Files\CMake\bin;$env:Path"
$env:VULKAN_SDK = "C:\VulkanSDK\1.4.363.0"
```

### 2. Cài dependency

Chạy tại thư mục gốc repository:

```powershell
cd D:\InterviewAssist
npm install
```

### 3. Kiểm tra frontend

Lệnh này chỉ type-check và build frontend, không mở ứng dụng:

```powershell
npm run build
```

Output nằm trong `dist/`.

### 4. Kiểm tra native Rust

```powershell
cargo check --manifest-path src-tauri/Cargo.toml
cargo test --manifest-path src-tauri/Cargo.toml
```

Nếu báo thiếu `link.exe`, hãy mở `Developer PowerShell for VS 2022` rồi chạy lại, hoặc nạp môi trường Build Tools trước:

```powershell
& "${env:ProgramFiles(x86)}\Microsoft Visual Studio\2022\BuildTools\Common7\Tools\Launch-VsDevShell.ps1" -Arch amd64 -HostArch amd64
```

### 5. Chạy ứng dụng ở chế độ development

Lệnh sau **sẽ mở ứng dụng desktop**:

```powershell
npm run tauri dev
```

Sau khi app mở:

1. Mở Settings.
2. Chọn `Local Whisper` hoặc `OpenAI` cho Live caption.
3. Với Local Whisper, chọn và tải một model multilingual: Small (466 MiB), Medium Q5 (514 MiB), Large v3 Turbo Q5 (547 MiB, khuyên dùng), hoặc Large v3 Turbo (1.5 GiB). Caption sau đó chạy offline.
4. Chọn Answer provider: ChatGPT plan, OpenRouter Free, Google AI Studio/Gemini Free Tier hoặc OpenAI API.
   - `ChatGPT plan`: nhấn **Continue with ChatGPT** và đăng nhập trong browser. Tài khoản ChatGPT Plus/Pro đủ điều kiện có thể dùng quota của gói, không cần API key riêng.
   - Các provider còn lại: nhập API key tương ứng. OpenAI API key vẫn cần nếu dùng OpenAI cloud caption vì luồng ChatGPT plan không hỗ trợ transcription/audio.
5. Chọn System Audio hoặc Microphone và thiết bị tương ứng, rồi nhấn Start.
6. Chỉnh transcript rồi nhấn Submit để gửi sang answer provider đã chọn.

API key và thông tin đăng nhập ChatGPT được lưu trong Windows Credential Manager, không cần tạo file `.env`.

### 6. Build file cài đặt Windows

Lệnh này chỉ compile và đóng gói, không tự mở app:

```powershell
npm run tauri build
```

Artifacts:

- Executable: `D:\iat\release\interview-assist.exe`
- NSIS installer: `D:\iat\release\bundle\nsis\InterviewAssist_0.1.0_x64-setup.exe`

### 7. Chạy bản release không qua installer

Lệnh sau **sẽ mở ứng dụng desktop**:

```powershell
D:\iat\release\interview-assist.exe
```

### 8. Dừng và thoát

- Trong app: nhấn nút `X` hoặc dùng `Ctrl+Shift+Q` để Exit hoàn toàn.
- Nếu chỉ ẩn cửa sổ: dùng Minimize hoặc `Ctrl+Shift+H`; app vẫn còn trong system tray.
- Khi chạy bằng `npm run tauri dev`, có thể quay lại terminal và nhấn `Ctrl+C` để dừng tiến trình development.

## Quyết định kỹ thuật ban đầu

- Desktop shell: Tauri 2.
- Native core: Rust + Win32/WASAPI.
- UI: React + TypeScript.
- Secret storage: Windows Credential Manager hoặc DPAPI thông qua native core.
- Network: native core gọi trực tiếp OpenAI, OpenRouter hoặc Google Gemini; API key không được chuyển vào WebView hoặc lưu trong localStorage.
- ChatGPT plan: đăng nhập OAuth chính thức bằng **Continue with ChatGPT**; app tự đăng ký client local, xác minh ID token, tự refresh token và lấy danh sách model theo tài khoản. Responses dùng `store: false`, `stream: true`; nếu lỗi và có OpenRouter key, app tự fallback sang OpenRouter.
- Free router: native core gọi trực tiếp OpenRouter với model `openrouter/free`; OpenRouter key được lưu riêng trong Windows Credential Manager.
- Google AI Studio: native core gọi trực tiếp Gemini streaming API; Google key được lưu riêng trong Windows Credential Manager.
- Khi Gemini gặp lỗi tạm thời như high demand, rate limit, quota hoặc mất kết nối, app tự chuyển sang OpenRouter nếu OpenRouter key đã được cấu hình; OpenRouter tiếp tục có fallback từ model nhanh sang Free Router.
- Local caption: whisper.cpp qua `whisper-rs` với Vulkan GPU acceleration; model nằm trong thư mục app data của Windows và audio không rời máy.

## Ngoài phạm vi MVP

- macOS/Linux và Windows trước version 2004.
- OBS, capture cấp driver, remote desktop và ứng dụng họp native.
- Tách riêng âm thanh theo process browser trên mọi bản Windows 10.
- Provider AI ngoài OpenAI.
- Tự động gửi transcript sang AI.
- Ghi âm, lưu lịch sử hoặc đồng bộ dữ liệu.

## Nguyên tắc sử dụng

Người dùng chịu trách nhiệm tuân thủ quy định của cuộc gọi, yêu cầu đồng ý ghi âm và chính sách của tổ chức. Sản phẩm không được mô tả là công cụ vượt qua hệ thống giám sát hay bảo đảm vô hình trước mọi cơ chế capture.
