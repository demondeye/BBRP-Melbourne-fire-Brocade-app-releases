//! Software updates, like a system update.
//!
//! The app asks GitHub for the newest release of `UPDATE_REPO`. If its version
//! (the release tag, e.g. "v0.3.0") is newer than this copy, a popup and
//! Settings → Software Update offer to install it. Installing downloads the
//! new MelbourneFireBrigade.exe, swaps it in for this one and restarts the app.
//!
//! To publish an update: raise `version` in Cargo.toml, build, and make a
//! GitHub release tagged with that version with MelbourneFireBrigade.exe attached
//! (release.ps1 does all of that). See README.md.

use crate::core::{lock, Shared};
use eframe::egui;
use serde::Deserialize;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

/// The GitHub repository updates come from ("owner/name"): the app's own code
/// repo, with each version published under its Releases. It must be public.
pub const UPDATE_REPO: &str = "demondeye/BBRP-Melbourne-fire-Brocade-app-releases";
/// The file attached to each release.
pub const ASSET_NAME: &str = "MelbourneFireBrigade.exe";
/// How often to look for updates in the background.
const CHECK_EVERY_SECS: u64 = 3 * 60 * 60;

pub const CURRENT: &str = env!("CARGO_PKG_VERSION");

#[derive(Clone, PartialEq)]
pub struct Release {
    pub version: String,
    pub notes: String,
    pub url: String,
    pub size: u64,
    pub sha256: Option<String>,
}

#[derive(Clone, PartialEq)]
pub enum UpdateStatus {
    Idle,
    Checking,
    UpToDate,
    Available(Release),
    Downloading(Release, f32),
    /// Downloaded and checked; the app installs it straight away.
    Ready(Release, PathBuf),
    Installing(Release),
    Failed(String),
}

impl UpdateStatus {
    pub fn release(&self) -> Option<&Release> {
        match self {
            UpdateStatus::Available(r) | UpdateStatus::Downloading(r, _) | UpdateStatus::Ready(r, _) | UpdateStatus::Installing(r) => Some(r),
            _ => None,
        }
    }
    pub fn busy(&self) -> bool {
        matches!(self, UpdateStatus::Checking | UpdateStatus::Downloading(..) | UpdateStatus::Ready(..) | UpdateStatus::Installing(..))
    }
}

#[derive(Deserialize)]
struct GhRelease {
    tag_name: String,
    #[serde(default)]
    body: Option<String>,
    #[serde(default)]
    draft: bool,
    #[serde(default)]
    prerelease: bool,
    #[serde(default)]
    assets: Vec<GhAsset>,
}
#[derive(Deserialize)]
struct GhAsset {
    name: String,
    browser_download_url: String,
    size: u64,
    #[serde(default)]
    digest: Option<String>,
}

/// "v1.2.3" → (1, 2, 3)
pub fn parse_version(v: &str) -> Option<(u64, u64, u64)> {
    let v = v.trim().trim_start_matches(['v', 'V']);
    let core = v.split(['-', '+']).next()?;
    let mut parts = core.split('.').map(|p| p.parse::<u64>().ok());
    Some((parts.next()??, parts.next().flatten().unwrap_or(0), parts.next().flatten().unwrap_or(0)))
}

fn is_newer(candidate: &str, current: &str) -> bool {
    match (parse_version(candidate), parse_version(current)) {
        (Some(a), Some(b)) => a > b,
        _ => false,
    }
}

fn agent() -> ureq::Agent {
    ureq::AgentBuilder::new().timeout_connect(std::time::Duration::from_secs(10)).timeout_read(std::time::Duration::from_secs(60)).build()
}

fn latest_url() -> String {
    // MFB_UPDATE_URL (for testing) points at a different release feed.
    std::env::var("MFB_UPDATE_URL").unwrap_or_else(|_| format!("https://api.github.com/repos/{UPDATE_REPO}/releases/latest"))
}

/// Ask GitHub for the newest release. Ok(None) = already up to date.
fn fetch_latest() -> Result<Option<Release>, String> {
    if UPDATE_REPO.is_empty() && std::env::var("MFB_UPDATE_URL").is_err() {
        return Err("Software updates aren't switched on yet.".into());
    }
    let resp = agent()
        .get(&latest_url())
        .set("User-Agent", "MelbourneFireBrigade-Updater")
        .set("Accept", "application/vnd.github+json")
        .call();
    let gh: GhRelease = match resp {
        Ok(r) => r.into_json().map_err(|_| "The update information couldn't be read.".to_string())?,
        // No releases published yet.
        Err(ureq::Error::Status(404, _)) => return Ok(None),
        Err(ureq::Error::Status(code, _)) => return Err(format!("GitHub answered with an error ({code}). Try again later.")),
        Err(_) => return Err("Couldn't reach the update server. Check the internet connection.".into()),
    };
    if gh.draft || gh.prerelease || !is_newer(&gh.tag_name, CURRENT) {
        return Ok(None);
    }
    let asset = gh
        .assets
        .iter()
        .find(|a| a.name.eq_ignore_ascii_case(ASSET_NAME))
        .or_else(|| gh.assets.iter().find(|a| a.name.to_lowercase().ends_with(".exe")))
        .ok_or_else(|| format!("Version {} was published without {ASSET_NAME} attached.", gh.tag_name))?;
    Ok(Some(Release {
        version: gh.tag_name.trim_start_matches(['v', 'V']).to_string(),
        notes: gh.body.unwrap_or_default().trim().to_string(),
        url: asset.browser_download_url.clone(),
        size: asset.size,
        sha256: asset.digest.as_deref().and_then(|d| d.strip_prefix("sha256:")).map(str::to_lowercase),
    }))
}

/// Look for an update in the background. `manual` = the user pressed the button
/// (only then are connection problems shown).
pub fn check(core: &Shared, ctx: &egui::Context, manual: bool) {
    {
        let mut c = lock(core);
        if c.update.busy() {
            return;
        }
        if manual || !matches!(c.update, UpdateStatus::Available(_)) {
            c.update = UpdateStatus::Checking;
        }
    }
    let core = core.clone();
    let ctx = ctx.clone();
    std::thread::spawn(move || {
        let result = fetch_latest();
        let mut c = lock(&core);
        c.update_checked_at = Some(chrono::Local::now());
        if c.update.busy() && !matches!(c.update, UpdateStatus::Checking) {
            return; // an install started meanwhile
        }
        c.update = match result {
            Ok(Some(r)) => {
                // Checked by hand from Settings: it's already on screen there, no popup needed.
                if manual {
                    c.update_dismissed = Some(r.version.clone());
                }
                UpdateStatus::Available(r)
            }
            Ok(None) => UpdateStatus::UpToDate,
            Err(e) if manual => UpdateStatus::Failed(e),
            Err(_) => UpdateStatus::Idle,
        };
        drop(c);
        ctx.request_repaint();
    });
}

/// Check shortly after start-up, then every few hours.
pub fn start_background_checks(core: Shared, ctx: egui::Context) {
    if UPDATE_REPO.is_empty() && std::env::var("MFB_UPDATE_URL").is_err() {
        return;
    }
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_secs(5));
        loop {
            check(&core, &ctx, false);
            std::thread::sleep(std::time::Duration::from_secs(CHECK_EVERY_SECS));
        }
    });
}

fn downloads_dir() -> PathBuf {
    let d = crate::storage::data_dir().join("updates");
    let _ = std::fs::create_dir_all(&d);
    d
}

/// Download the update (with progress). When it's done the status becomes Ready
/// and the app installs it.
pub fn download(core: &Shared, ctx: &egui::Context) {
    let release = {
        let mut c = lock(core);
        let Some(r) = c.update.release().cloned() else { return };
        if c.update.busy() {
            return;
        }
        c.update = UpdateStatus::Downloading(r.clone(), 0.0);
        r
    };
    let core = core.clone();
    let ctx = ctx.clone();
    std::thread::spawn(move || {
        let result = (|| -> Result<PathBuf, String> {
            let resp = agent()
                .get(&release.url)
                .set("User-Agent", "MelbourneFireBrigade-Updater")
                .set("Accept", "application/octet-stream")
                .call()
                .map_err(|_| "The download didn't start. Check the internet connection and try again.".to_string())?;
            let total = resp.header("Content-Length").and_then(|v| v.parse::<u64>().ok()).unwrap_or(release.size).max(1);
            let path = downloads_dir().join(format!("MelbourneFireBrigade-{}.exe", release.version));
            let mut file = std::fs::File::create(&path).map_err(|_| "Couldn't save the update on this PC.".to_string())?;
            let mut reader = resp.into_reader();
            let mut hasher = <sha2::Sha256 as sha2::Digest>::new();
            let mut buf = vec![0u8; 64 * 1024];
            let mut got: u64 = 0;
            loop {
                let n = reader.read(&mut buf).map_err(|_| "The download was interrupted. Try again.".to_string())?;
                if n == 0 {
                    break;
                }
                file.write_all(&buf[..n]).map_err(|_| "Couldn't save the update on this PC.".to_string())?;
                sha2::Digest::update(&mut hasher, &buf[..n]);
                got += n as u64;
                lock(&core).update = UpdateStatus::Downloading(release.clone(), (got as f32 / total as f32).min(1.0));
                ctx.request_repaint();
            }
            file.flush().ok();
            drop(file);
            if release.size > 0 && got != release.size {
                return Err("The download was incomplete. Try again.".into());
            }
            if let Some(want) = &release.sha256 {
                let have: String = sha2::Digest::finalize(hasher).iter().map(|b| format!("{b:02x}")).collect();
                if &have != want {
                    let _ = std::fs::remove_file(&path);
                    return Err("The download didn't match the published update, so it wasn't installed.".into());
                }
            }
            Ok(path)
        })();
        lock(&core).update = match result {
            Ok(path) => UpdateStatus::Ready(release, path),
            Err(e) => UpdateStatus::Failed(e),
        };
        ctx.request_repaint();
    });
}

/// Swap the downloaded copy in for the running app and start it.
/// (Windows lets a running program be renamed, just not deleted.)
pub fn install_and_relaunch(downloaded: &Path) -> Result<(), String> {
    let exe = std::env::current_exe().map_err(|_| "Couldn't find where Melbourne Fire Brigade is installed.".to_string())?;
    let dir = exe.parent().ok_or("Couldn't find where Melbourne Fire Brigade is installed.")?;
    let name = exe.file_name().and_then(|n| n.to_str()).unwrap_or(ASSET_NAME).to_string();
    let stem = name.trim_end_matches(".exe");
    let staged = dir.join(format!("{stem}.new.exe"));
    let old = dir.join(format!("{stem}.old.exe"));

    let no_access = |_| format!("Melbourne Fire Brigade can't update itself in {}. Move it to a folder you own (like Documents or the Desktop) and try again.", dir.display());
    std::fs::copy(downloaded, &staged).map_err(no_access)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&staged, std::fs::Permissions::from_mode(0o755));
    }
    let _ = std::fs::remove_file(&old);
    std::fs::rename(&exe, &old).map_err(no_access)?;
    if let Err(e) = std::fs::rename(&staged, &exe) {
        let _ = std::fs::rename(&old, &exe); // put the current version back
        return Err(no_access(e));
    }
    let _ = std::fs::remove_file(downloaded);
    std::process::Command::new(&exe)
        .arg("--after-update")
        .current_dir(dir)
        .spawn()
        .map_err(|_| "The update was installed, but the app couldn't restart. Open Melbourne Fire Brigade again.".to_string())?;
    Ok(())
}

/// Tidy up after an update: remove the previous version and old downloads.
pub fn cleanup() {
    if let Ok(exe) = std::env::current_exe() {
        if let (Some(dir), Some(name)) = (exe.parent(), exe.file_name().and_then(|n| n.to_str())) {
            let stem = name.trim_end_matches(".exe");
            // The old copy may still be closing; try for a little while.
            let old = dir.join(format!("{stem}.old.exe"));
            std::thread::spawn(move || {
                for _ in 0..20 {
                    if !old.exists() || std::fs::remove_file(&old).is_ok() {
                        break;
                    }
                    std::thread::sleep(std::time::Duration::from_millis(500));
                }
            });
        }
    }
    if let Ok(entries) = std::fs::read_dir(downloads_dir()) {
        for e in entries.flatten() {
            let _ = std::fs::remove_file(e.path());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn versions() {
        assert!(is_newer("v0.3.0", "0.2.0"));
        assert!(is_newer("1.0", "0.9.9"));
        assert!(!is_newer("v0.2.0", "0.2.0"));
        assert!(!is_newer("v0.1.5", "0.2.0"));
        assert!(is_newer("0.2.10", "0.2.9"));
        assert!(!is_newer("garbage", "0.2.0"));
    }
}
