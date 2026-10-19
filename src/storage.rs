//! Everything is saved as small JSON files on this PC, in
//! %APPDATA%\Melbourne Fire Brigade (Windows) — the native stand-in for the
//! browser storage the web version used. Swap these for the database later.

use serde::{de::DeserializeOwned, Serialize};
use std::path::PathBuf;

pub fn data_dir() -> PathBuf {
    // MFB_DATA_DIR (for testing) keeps a separate set of data.
    let dir = match std::env::var("MFB_DATA_DIR") {
        Ok(d) => PathBuf::from(d),
        Err(_) => dirs::data_dir().unwrap_or_else(|| PathBuf::from(".")).join("Melbourne Fire Brigade"),
    };
    let _ = std::fs::create_dir_all(&dir);
    dir
}

/// A file name that is safe for any user name.
pub fn user_file(kind: &str, name: &str, ext: &str) -> PathBuf {
    let clean: String = name
        .trim()
        .to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '_' })
        .collect();
    let dir = data_dir().join(kind);
    let _ = std::fs::create_dir_all(&dir);
    dir.join(format!("{clean}.{ext}"))
}

pub fn load<T: DeserializeOwned>(path: &PathBuf) -> Option<T> {
    let text = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&text).ok()
}

/// Returns false if it could not be written.
pub fn save<T: Serialize>(path: &PathBuf, value: &T) -> bool {
    match serde_json::to_string_pretty(value) {
        Ok(text) => {
            let tmp = path.with_extension("tmp");
            std::fs::write(&tmp, text).is_ok() && std::fs::rename(&tmp, path).is_ok()
        }
        Err(_) => false,
    }
}
