//! Only one copy of Melbourne Fire Brigade runs at a time. Opening it again brings
//! the open window to the front instead (so two copies can never fight over the
//! Stream Deck connection).

/// Returns false if another copy is already running (and has been brought forward),
/// in which case this copy should quit straight away.
pub fn claim() -> bool {
    #[cfg(windows)]
    {
        win::claim()
    }
    #[cfg(not(windows))]
    {
        true
    }
}

#[cfg(windows)]
mod win {
    use windows_sys::Win32::Foundation::{CloseHandle, GetLastError, ERROR_ALREADY_EXISTS};
    use windows_sys::Win32::System::Threading::CreateMutexW;
    use windows_sys::Win32::UI::WindowsAndMessaging::{FindWindowW, IsIconic, SetForegroundWindow, ShowWindow, SW_RESTORE};

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }

    pub fn claim() -> bool {
        // Started by an update: the old copy is still closing, so wait for it.
        let after_update = std::env::args().any(|a| a == "--after-update");
        let name = wide("Local\\MelbourneFireBrigade.SingleInstance");
        let tries = if after_update { 60 } else { 1 };
        for i in 0..tries {
            unsafe {
                let handle = CreateMutexW(std::ptr::null(), 0, name.as_ptr());
                if !handle.is_null() && GetLastError() != ERROR_ALREADY_EXISTS {
                    // Ours: keep it for as long as the app runs.
                    let _ = handle; // never closed, so the name stays taken while we run
                    return true;
                }
                if !handle.is_null() {
                    CloseHandle(handle);
                }
            }
            if i + 1 < tries {
                std::thread::sleep(std::time::Duration::from_millis(250));
            }
        }
        // Another copy is open: bring it to the front.
        unsafe {
            let title = wide("Melbourne Fire Brigade");
            let hwnd = FindWindowW(std::ptr::null(), title.as_ptr());
            if !hwnd.is_null() {
                if IsIconic(hwnd) != 0 {
                    ShowWindow(hwnd, SW_RESTORE);
                }
                SetForegroundWindow(hwnd);
            }
        }
        false
    }
}
