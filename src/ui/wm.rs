//! Apps and the window manager. One window per app.
//!
//! Each app's last position, size and maximised state is remembered on this
//! PC, so a window reopens the way it was left — after closing it, logging
//! out, restarting or quitting.

use crate::storage;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub struct AppInfo {
    pub id: &'static str,
    pub name: &'static str,
    pub tint: [u8; 3],
    pub w: f32,
    pub h: f32,
}

// Add a new app: write its screen, then register it here (and in DOCK to pin it).
pub const APPS: [AppInfo; 7] = [
    AppInfo { id: "timeclock", name: "Time Clock", tint: [0xb2, 0x3a, 0x2a], w: 460.0, h: 560.0 },
    AppInfo { id: "fieldguide", name: "Field Guide", tint: [0xe0, 0x7a, 0x1f], w: 900.0, h: 600.0 },
    AppInfo { id: "map", name: "Map", tint: [0x2f, 0x7d, 0x4a], w: 1000.0, h: 680.0 },
    AppInfo { id: "records", name: "Incident Reports", tint: [0xd8, 0x8a, 0x6a], w: 720.0, h: 460.0 },
    AppInfo { id: "notes", name: "Notes", tint: [0xf5, 0xb7, 0xa8], w: 480.0, h: 400.0 },
    AppInfo { id: "settings", name: "Settings", tint: [0x5a, 0x6b, 0x85], w: 760.0, h: 520.0 },
    AppInfo { id: "about", name: "About", tint: [0x5e, 0x0f, 0x14], w: 420.0, h: 340.0 },
];

pub const DOCK: [&str; 5] = ["timeclock", "map", "records", "notes", "settings"];

pub fn app(id: &str) -> Option<&'static AppInfo> {
    APPS.iter().find(|a| a.id == id)
}

#[derive(Clone)]
pub struct Win {
    pub id: &'static str,
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    pub z: u64,
    pub minimized: bool,
    pub maximized: bool,
    /// When it was opened (for the opening animation).
    pub opened_at: f64,
}

#[derive(Clone, Copy, Serialize, Deserialize)]
struct Saved {
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    maximized: bool,
}

pub struct WindowManager {
    pub windows: Vec<Win>,
    z_top: u64,
    saved: HashMap<String, Saved>,
    dirty_at: Option<f64>,
}

fn saved_path() -> std::path::PathBuf {
    storage::data_dir().join("windows.json")
}

impl WindowManager {
    pub fn new() -> Self {
        Self { windows: Vec::new(), z_top: 10, saved: storage::load(&saved_path()).unwrap_or_default(), dirty_at: None }
    }

    /// Remember how every window is right now.
    pub fn persist(&mut self) {
        if self.windows.is_empty() {
            return;
        }
        for w in &self.windows {
            self.saved.insert(w.id.to_string(), Saved { x: w.x, y: w.y, w: w.w, h: w.h, maximized: w.maximized });
        }
        storage::save(&saved_path(), &self.saved);
        self.dirty_at = None;
    }
    /// Save shortly after a change (so dragging doesn't write on every pixel).
    pub fn touch(&mut self, now: f64) {
        self.dirty_at = Some(now);
    }
    pub fn tick(&mut self, now: f64) {
        if self.dirty_at.is_some_and(|t| now - t > 0.25) {
            self.persist();
        }
    }

    pub fn get(&self, id: &str) -> Option<&Win> {
        self.windows.iter().find(|w| w.id == id)
    }
    pub fn get_mut(&mut self, id: &str) -> Option<&mut Win> {
        self.windows.iter_mut().find(|w| w.id == id)
    }

    /// Open an app, or bring it back if it's already open. `area` is the desktop size.
    pub fn open(&mut self, id: &'static str, area: (f32, f32), now: f64) {
        self.z_top += 1;
        let z = self.z_top;
        if let Some(w) = self.get_mut(id) {
            w.minimized = false;
            w.z = z;
            return;
        }
        let Some(app) = app(id) else { return };
        let n = (self.windows.len() % 6) as f32;
        let last = self.saved.get(id).copied();
        let (aw, ah) = area;
        let max_w = (aw - 40.0).max(320.0);
        let max_h = (ah - 120.0).max(220.0);
        let w = last.map_or(app.w, |l| l.w).min(max_w);
        let h = last.map_or(app.h, |l| l.h).min(max_h);
        // Keep a remembered position on screen if the window is smaller now.
        let (x, y) = match last {
            Some(l) => (l.x.max(80.0 - w).min(aw - 80.0), l.y.max(0.0).min(ah - 60.0)),
            None => (90.0 + n * 34.0, 60.0 + n * 28.0),
        };
        self.windows.push(Win { id, x, y, w, h, z, minimized: false, maximized: last.is_some_and(|l| l.maximized), opened_at: now });
    }

    pub fn focus(&mut self, id: &str) {
        self.z_top += 1;
        let z = self.z_top;
        if let Some(w) = self.get_mut(id) {
            w.z = z;
        }
    }
    pub fn close(&mut self, id: &str) {
        self.persist(); // remember how it was left before it goes
        self.windows.retain(|w| w.id != id);
    }
    pub fn close_all(&mut self) {
        self.persist();
        self.windows.clear();
    }
    pub fn minimize(&mut self, id: &str) {
        if let Some(w) = self.get_mut(id) {
            w.minimized = true;
        }
    }
    pub fn toggle_maximize(&mut self, id: &str, now: f64) {
        if let Some(w) = self.get_mut(id) {
            w.maximized = !w.maximized;
        }
        self.touch(now);
    }

    /// The front-most window that isn't minimised.
    pub fn active(&self) -> Option<&'static str> {
        self.windows.iter().filter(|w| !w.minimized).max_by_key(|w| w.z).map(|w| w.id)
    }
}
