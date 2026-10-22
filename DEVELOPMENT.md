# Development setup

## Prerequisites

- Windows 10 version 2004+ or Windows 11.
- Node.js 22+ and npm.
- Rust stable MSVC toolchain.
- Visual Studio 2022 Build Tools with `Desktop development with C++`.
- WebView2 Runtime (included with supported Windows releases/Edge installations).

Rust can be installed with:

```powershell
winget install --id Rustlang.Rustup --exact
```

The required Microsoft linker can be installed through Visual Studio Installer by selecting `Desktop development with C++`. A complete Visual Studio IDE is not required.

## Commands

Install JavaScript dependencies:

```powershell
npm install
```

Run the UI in a browser without native integrations:

```powershell
npm run dev
```

Build the frontend:

```powershell
npm run build
```

Run the Windows desktop app:

```powershell
npm run tauri dev
```

Build the installer:

```powershell
npm run tauri build
```

## Current implementation status

- Frontend production build: passing.
- Tauri/Rust source: scaffolded.
- Native build on the current development machine: passing with Visual C++ Build Tools 2022.
- Audio, credential storage, Realtime transcription and Responses streaming adapters: implemented and compile-tested.
- Live OpenAI and screen-sharing validation still require a human-operated test session.

## Browser preview behavior

When running `npm run dev` in a normal browser, native Tauri commands are unavailable. The UI therefore displays screen-share protection as unavailable. This is expected and must not be treated as a Windows protection failure.
