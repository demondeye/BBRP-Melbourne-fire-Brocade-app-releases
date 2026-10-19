//! How the app's own window shows on this PC: windowed, borderless or
//! fullscreen, and on which screen. Saved for the PC (not per user), so the app
//! opens the same way every time, before anyone has logged in.
//!
//! On Windows the window is placed with the Windows API directly (exact pixels
//! on the chosen monitor), because the general-purpose "maximise" / "resize"
//! requests aren't reliable there, especially with display scaling.

use crate::storage;
use eframe::egui::{self, ViewportCommand};
use serde::{Deserialize, Serialize};
use std::sync::Mutex;

#[derive(Clone, Copy, PartialEq, Serialize, Deserialize, Default, Debug)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    /// A normal maximised window with a title bar.
    #[default]
    Windowed,
    /// No title bar, covering the whole screen (taskbar too).
    Borderless,
    /// Fullscreen.
    Fullscreen,
}

pub const MODES: [(Mode, &str); 3] = [(Mode::Windowed, "Windowed"), (Mode::Borderless, "Borderless Window"), (Mode::Fullscreen, "Fullscreen")];

#[derive(Clone, Serialize, Deserialize, Default)]
struct Saved {
    mode: Mode,
    /// The chosen screen (Windows device name, e.g. "\\.\DISPLAY2"); None = main screen.
    #[serde(default)]
    monitor: Option<String>,
}

/// A connected screen.
#[derive(Clone, PartialEq)]
pub struct Monitor {
    pub id: String,
    pub label: String,
    pub primary: bool,
    /// Whole screen and the part not covered by the taskbar: [left, top, right, bottom] in pixels.
    pub rect: [i32; 4],
    pub work: [i32; 4],
}

static SAVED: Mutex<Option<Saved>> = Mutex::new(None);
/// A placement in progress: (mode, step, frames to wait before the next step).
static PENDING: Mutex<Option<(Mode, u8, u8)>> = Mutex::new(None);

fn path() -> std::path::PathBuf {
    storage::data_dir().join("display.json")
}

fn saved() -> Saved {
    let mut s = SAVED.lock().unwrap_or_else(|e| e.into_inner());
    s.get_or_insert_with(|| storage::load::<Saved>(&path()).unwrap_or_default()).clone()
}

fn store(new: Saved) {
    storage::save(&path(), &new);
    *SAVED.lock().unwrap_or_else(|e| e.into_inner()) = Some(new);
}

/// The saved mode.
pub fn mode() -> Mode {
    saved().mode
}

/// The saved screen, if one was picked.
pub fn monitor_id() -> Option<String> {
    saved().monitor
}

/// Save a new mode and switch the window to it now.
pub fn set_mode(mode: Mode) {
    store(Saved { mode, ..saved() });
    request(mode);
}

/// Save the screen to use and move the window there now.
pub fn set_monitor(id: Option<String>) {
    let s = Saved { monitor: id, ..saved() };
    let mode = s.mode;
    store(s);
    request(mode);
}

/// Start putting the window into a mode (carried out over the next few frames by `tick`).
pub fn request(mode: Mode) {
    *PENDING.lock().unwrap_or_else(|e| e.into_inner()) = Some((mode, 0, 0));
}

pub fn busy() -> bool {
    PENDING.lock().unwrap_or_else(|e| e.into_inner()).is_some()
}

/// The screen the window should be on: the saved one if it's still connected, else the main one.
pub fn target_monitor() -> Option<Monitor> {
    let list = monitors();
    let want = monitor_id();
    list.iter().find(|m| Some(&m.id) == want.as_ref()).or_else(|| list.iter().find(|m| m.primary)).or(list.first()).cloned()
}

/// Call once per frame. Runs a requested placement step by step, leaving a few
/// frames between steps so each change has taken effect before the next.
pub fn tick(ctx: &egui::Context, frame: &eframe::Frame) {
    let mut pending = PENDING.lock().unwrap_or_else(|e| e.into_inner());
    let Some((mode, step, wait)) = *pending else { return };
    ctx.request_repaint();
    if wait > 0 {
        *pending = Some((mode, step, wait - 1));
        return;
    }
    match step {
        // 1. Leave fullscreen, set the title bar on or off.
        0 => {
            ctx.send_viewport_cmd(ViewportCommand::Fullscreen(false));
            ctx.send_viewport_cmd(ViewportCommand::Decorations(mode != Mode::Borderless));
            *pending = Some((mode, 1, 3));
        }
        // 2. Put the window on the right screen, at the right size.
        1 => {
            let placed = place(frame, mode);
            if !placed {
                // No Windows API (or it failed): fall back to the general requests.
                match mode {
                    Mode::Windowed => ctx.send_viewport_cmd(ViewportCommand::Maximized(true)),
                    Mode::Borderless => {
                        ctx.send_viewport_cmd(ViewportCommand::Maximized(false));
                        if let Some(m) = ctx.input(|i| i.viewport().monitor_size) {
                            ctx.send_viewport_cmd(ViewportCommand::OuterPosition(egui::pos2(0.0, 0.0)));
                            ctx.send_viewport_cmd(ViewportCommand::InnerSize(m));
                        }
                    }
                    Mode::Fullscreen => {}
                }
            }
            *pending = if mode == Mode::Fullscreen { Some((mode, 2, 3)) } else { None };
        }
        // 3. Fullscreen, on the screen the window is now on.
        _ => {
            ctx.send_viewport_cmd(ViewportCommand::Fullscreen(true));
            *pending = None;
        }
    }
}

// ── Windows ──────────────────────────────────────────────────────────────

#[cfg(windows)]
mod win {
    use super::Monitor;
    use windows_sys::Win32::Foundation::{BOOL, HWND, LPARAM, RECT};
    use windows_sys::Win32::Graphics::Gdi::{EnumDisplayMonitors, GetMonitorInfoW, HDC, HMONITOR, MONITORINFO, MONITORINFOEXW};
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        SetWindowPos, ShowWindow, HWND_TOP, SWP_FRAMECHANGED, SWP_NOOWNERZORDER, SWP_SHOWWINDOW, SW_MAXIMIZE, SW_RESTORE,
    };

    unsafe extern "system" fn collect(hmon: HMONITOR, _hdc: HDC, _rect: *mut RECT, data: LPARAM) -> BOOL {
        let list = &mut *(data as *mut Vec<Monitor>);
        let mut info: MONITORINFOEXW = std::mem::zeroed();
        info.monitorInfo.cbSize = std::mem::size_of::<MONITORINFOEXW>() as u32;
        if GetMonitorInfoW(hmon, &mut info as *mut MONITORINFOEXW as *mut MONITORINFO) != 0 {
            let len = info.szDevice.iter().position(|c| *c == 0).unwrap_or(info.szDevice.len());
            let id = String::from_utf16_lossy(&info.szDevice[..len]);
            let r = info.monitorInfo.rcMonitor;
            let w = info.monitorInfo.rcWork;
            let primary = info.monitorInfo.dwFlags & 1 != 0;
            // "\\.\DISPLAY2" → "Display 2"
            let number: String = id.chars().rev().take_while(|c| c.is_ascii_digit()).collect::<Vec<_>>().into_iter().rev().collect();
            let name = if number.is_empty() { format!("Display {}", list.len() + 1) } else { format!("Display {number}") };
            let label = format!("{name} · {}×{}{}", r.right - r.left, r.bottom - r.top, if primary { " (main)" } else { "" });
            list.push(Monitor { id, label, primary, rect: [r.left, r.top, r.right, r.bottom], work: [w.left, w.top, w.right, w.bottom] });
        }
        1
    }

    pub fn monitors() -> Vec<Monitor> {
        let mut list: Vec<Monitor> = Vec::new();
        unsafe {
            EnumDisplayMonitors(std::ptr::null_mut(), std::ptr::null(), Some(collect), &mut list as *mut Vec<Monitor> as LPARAM);
        }
        list.sort_by(|a, b| a.label.cmp(&b.label));
        list
    }

    /// Move the window onto a screen: maximised there (windowed), covering it
    /// completely (borderless), or just moved there (before going fullscreen).
    pub fn place(hwnd: HWND, mode: super::Mode, m: &Monitor) -> bool {
        unsafe {
            ShowWindow(hwnd, SW_RESTORE);
            let flags = SWP_FRAMECHANGED | SWP_SHOWWINDOW | SWP_NOOWNERZORDER;
            let [l, t, r, b] = if mode == super::Mode::Borderless { m.rect } else { m.work };
            let ok = SetWindowPos(hwnd, HWND_TOP, l, t, r - l, b - t, flags) != 0;
            if mode == super::Mode::Windowed {
                ShowWindow(hwnd, SW_MAXIMIZE);
            }
            ok
        }
    }
}

/// All connected screens (Windows only; elsewhere the list is empty).
pub fn monitors() -> Vec<Monitor> {
    #[cfg(windows)]
    {
        win::monitors()
    }
    #[cfg(not(windows))]
    {
        Vec::new()
    }
}

#[cfg(windows)]
fn place(frame: &eframe::Frame, mode: Mode) -> bool {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    let Some(m) = target_monitor() else { return false };
    let Ok(handle) = frame.window_handle() else { return false };
    match handle.as_raw() {
        RawWindowHandle::Win32(h) => win::place(h.hwnd.get() as _, mode, &m),
        _ => false,
    }
}

#[cfg(not(windows))]
fn place(_frame: &eframe::Frame, _mode: Mode) -> bool {
    false
}
