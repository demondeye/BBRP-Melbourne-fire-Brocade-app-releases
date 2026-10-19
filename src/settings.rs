//! Per-user settings, keyed by the user's name, plus the built-in wallpapers.
//! Stored on this PC for now. When the database is added, load/save the
//! user's settings document here instead.

use crate::storage;
use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Wallpaper {
    /// "preset" or "image"
    #[serde(rename = "type")]
    pub kind: String,
    pub id: Option<String>,
}

#[derive(Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct UserSettings {
    pub wallpaper: Wallpaper,
    /// Path of the user's own picture (a resized copy kept in the app's folder).
    #[serde(rename = "customImage")]
    pub custom_image: Option<String>,
    /// How the user's own picture is sized: fill, fit, stretch, center or tile.
    pub fit: String,
    /// Colour shown around the picture when it doesn't cover the screen (Fit, Centre).
    #[serde(rename = "fillColor")]
    pub fill_color: [u8; 3],
    /// Window look: "dark", "light" or "auto" (follow Windows).
    pub appearance: String,
}

impl Default for Wallpaper {
    fn default() -> Self {
        Self { kind: "preset".into(), id: Some("ember".into()) }
    }
}
impl Default for UserSettings {
    fn default() -> Self {
        Self { wallpaper: Wallpaper::default(), custom_image: None, fit: "fill".into(), fill_color: [13, 27, 46], appearance: "dark".into() }
    }
}

impl UserSettings {
    pub fn load(name: &str) -> Self {
        storage::load(&storage::user_file("settings", name, "json")).unwrap_or_default()
    }
    pub fn save(&self, name: &str) -> bool {
        storage::save(&storage::user_file("settings", name, "json"), self)
    }
    pub fn using_image(&self) -> bool {
        self.wallpaper.kind == "image" && self.custom_image.is_some()
    }
    pub fn preset(&self) -> &'static WallpaperPreset {
        let id = self.wallpaper.id.as_deref().unwrap_or("ember");
        WALLPAPERS.iter().find(|w| w.id == id).unwrap_or(&WALLPAPERS[0])
    }
}

/// A gradient: CSS-style angle in degrees, then colour stops (0..=1).
pub struct WallpaperPreset {
    pub id: &'static str,
    pub name: &'static str,
    pub angle: f32,
    pub stops: &'static [(f32, u32)],
}

// Built-in wallpapers. Add one by adding a line here.
pub const WALLPAPERS: [WallpaperPreset; 6] = [
    WallpaperPreset { id: "ember", name: "Ember", angle: 155.0, stops: &[(0.0, 0x160808), (0.32, 0x5a1818), (0.66, 0xb23a2a), (1.0, 0xf0a35e)] },
    WallpaperPreset { id: "rink", name: "Rink", angle: 155.0, stops: &[(0.0, 0x081828), (0.25, 0x0d2d50), (0.55, 0x174f84), (0.78, 0x2e7ab5), (1.0, 0x6ab8d8)] },
    WallpaperPreset { id: "midnight", name: "Midnight", angle: 155.0, stops: &[(0.0, 0x04070f), (0.35, 0x120a3a), (0.65, 0x1d0f5e), (1.0, 0x4a3aa8)] },
    WallpaperPreset { id: "aurora", name: "Aurora", angle: 155.0, stops: &[(0.0, 0x04201a), (0.30, 0x0b4a3d), (0.62, 0x1f8a6c), (1.0, 0x8fe0c4)] },
    WallpaperPreset { id: "dusk", name: "Dusk", angle: 155.0, stops: &[(0.0, 0x150c26), (0.32, 0x45215a), (0.66, 0xa8486f), (1.0, 0xf0a35e)] },
    WallpaperPreset { id: "slate", name: "Slate", angle: 155.0, stops: &[(0.0, 0x0e1114), (0.35, 0x252c35), (0.70, 0x4e5a66), (1.0, 0x98a5af)] },
];

/// The picture sizing options, like a Mac: (id, label).
pub const FITS: [(&str, &str); 5] = [("fill", "Fill Screen"), ("fit", "Fit to Screen"), ("stretch", "Stretch to Fill"), ("center", "Centre"), ("tile", "Tile")];

/// Colours offered for the space around a picture.
pub const FILL_COLORS: [[u8; 3]; 8] = [[30, 10, 11], [0, 0, 0], [40, 44, 52], [29, 15, 94], [23, 79, 132], [31, 138, 108], [168, 72, 111], [235, 235, 240]];

/// Shrinks a chosen picture so it is quick to load, and keeps a copy for this user.
pub fn import_wallpaper(name: &str, source: &std::path::Path) -> Result<String, String> {
    let img = image::open(source).map_err(|_| "That file couldn't be opened as a picture.".to_string())?;
    let max_side = 2560u32;
    let img = if img.width().max(img.height()) > max_side { img.resize(max_side, max_side, image::imageops::FilterType::Lanczos3) } else { img };
    let stamp = chrono::Utc::now().timestamp_millis();
    let out = storage::user_file("wallpapers", &format!("{name}-{stamp}"), "jpg");
    img.to_rgb8()
        .save_with_format(&out, image::ImageFormat::Jpeg)
        .map_err(|_| "That picture couldn't be saved on this PC.".to_string())?;
    Ok(out.to_string_lossy().into_owned())
}
