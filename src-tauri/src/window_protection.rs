#[cfg(target_os = "windows")]
mod windows {
    use std::ffi::c_void;
    use tauri::WebviewWindow;

    const WDA_EXCLUDEFROMCAPTURE: u32 = 0x0000_0011;

    #[link(name = "user32")]
    extern "system" {
        fn SetWindowDisplayAffinity(hwnd: *mut c_void, affinity: u32) -> i32;
    }

    pub fn protect(window: &WebviewWindow) -> Result<(), String> {
        let hwnd = window
            .hwnd()
            .map_err(|error| format!("failed to resolve HWND: {error}"))?;

        let applied =
            unsafe { SetWindowDisplayAffinity(hwnd.0 as *mut c_void, WDA_EXCLUDEFROMCAPTURE) };
        if applied == 0 {
            return Err(format!(
                "SetWindowDisplayAffinity failed: {}",
                std::io::Error::last_os_error()
            ));
        }

        Ok(())
    }
}

#[cfg(target_os = "windows")]
pub use windows::protect;

#[cfg(not(target_os = "windows"))]
pub fn protect(_window: &tauri::WebviewWindow) -> Result<(), String> {
    Err("screen-share protection is only available on Windows".into())
}
