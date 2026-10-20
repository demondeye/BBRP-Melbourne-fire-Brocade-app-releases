//! State shared between the screen and the Stream Deck server. Commands from a
//! Stream Deck key run straight against this, so they work even while the app
//! is minimised; anything the screen should show is queued in `events`.

use crate::fieldguide::store::FieldGuideState;
use crate::settings::UserSettings;
use crate::timeclock::TimeSheet;
use std::sync::{Arc, Mutex};

pub enum UiEvent {
    OpenApp(&'static str),
    Toast(String),
    /// Voice control heard a postal: open the map and go there.
    MapPostal(String),
}

#[derive(Clone, PartialEq)]
pub enum ServerStatus {
    Starting,
    Running,
    Failed(String),
}

pub struct Core {
    /// The signed-in user (only kept in memory — closing the app logs out).
    pub user: Option<String>,
    pub settings: UserSettings,
    pub sheet: TimeSheet,
    pub guide: FieldGuideState,
    pub events: Vec<UiEvent>,
    pub server: ServerStatus,
    pub update: crate::update::UpdateStatus,
    pub update_checked_at: Option<chrono::DateTime<chrono::Local>>,
    /// "Later" was pressed on the popup for this version (Settings still shows it).
    pub update_dismissed: Option<String>,
    /// Asked for from the screen; carried out by the app once the screen is drawn.
    pub update_request: Option<UpdateRequest>,
    /// What voice control is doing (shown on the map and in Settings).
    pub voice: crate::voice::VoiceStatus,
}

#[derive(Clone, Copy, PartialEq)]
pub enum UpdateRequest {
    Check,
    Install,
}

pub type Shared = Arc<Mutex<Core>>;

impl Core {
    pub fn new() -> Shared {
        Arc::new(Mutex::new(Core {
            user: None,
            settings: UserSettings::default(),
            sheet: TimeSheet::default(),
            guide: FieldGuideState::default(),
            events: Vec::new(),
            server: ServerStatus::Starting,
            update: crate::update::UpdateStatus::Idle,
            update_checked_at: None,
            update_dismissed: None,
            update_request: None,
            voice: crate::voice::VoiceStatus::default(),
        }))
    }

    pub fn sign_in(&mut self, name: &str) {
        self.user = Some(name.to_string());
        self.settings = UserSettings::load(name);
        self.sheet = TimeSheet::load(name);
        self.guide = FieldGuideState::default();
    }

    pub fn sign_out(&mut self) {
        self.user = None;
        self.guide = FieldGuideState::default();
        self.events.clear();
    }

    pub fn save_settings(&mut self, settings: UserSettings) -> bool {
        let Some(name) = self.user.clone() else { return false };
        let ok = settings.save(&name);
        self.settings = settings;
        ok
    }
}

/// Lock that survives a panic elsewhere.
pub fn lock(shared: &Shared) -> std::sync::MutexGuard<'_, Core> {
    shared.lock().unwrap_or_else(|e| e.into_inner())
}
