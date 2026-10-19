//! The apps that run in windows.

#[allow(dead_code)]
pub mod fieldguide;
pub mod map;
pub mod placeholders;
pub mod settings;
pub mod timeclock;

use super::desktop::Confirm;
use super::theme::*;
use crate::core::Core;
use eframe::egui::{self, Event, Id, Key, Modifiers, Ui};

#[derive(Default)]
pub struct Apps {
    pub timeclock: timeclock::TimeClockUi,
    pub settings: settings::SettingsUi,
    #[allow(dead_code)]
    pub fieldguide: fieldguide::FieldGuideUi,
    pub map: map::MapUi,
    pub notes: String,
    /// Keyboard events to replay into a text box next frame (from the right-click menu).
    pub pending_input: Option<(Id, Vec<Event>)>,
    /// The text box selection from just before a right-click (a right-click moves the cursor).
    pub saved_selection: Option<(Id, egui::text::CCursorRange)>,
}

impl Apps {
    pub fn escape(&mut self) {
        self.timeclock.picking = false;
    }

    /// Cut / Copy / Paste / Select All from the right-click menu, done by
    /// replaying the matching keyboard shortcut into the text box.
    pub fn text_command(&mut self, _ctx: &egui::Context, id: Id, cmd: &str) {
        let key = |key: Key| Event::Key { key, physical_key: None, pressed: true, repeat: false, modifiers: Modifiers::COMMAND };
        let events = match cmd {
            "cut" => vec![Event::Cut],
            "copy" => vec![Event::Copy],
            "paste" => match arboard::Clipboard::new().and_then(|mut c| c.get_text()) {
                Ok(text) if !text.is_empty() => vec![Event::Paste(text)],
                _ => vec![],
            },
            "selectall" => vec![key(Key::A)],
            _ => vec![],
        };
        self.pending_input = Some((id, events));
    }
}

#[allow(clippy::too_many_arguments)]
pub fn show_app(ui: &mut Ui, id: &str, apps: &mut Apps, core: &mut Core, confirm: &mut Option<Confirm>, open: &mut Option<&'static str>, now: f64, radius: u8) {
    match id {
        "timeclock" => timeclock::show(ui, &mut apps.timeclock, core, confirm, now),
        // The fire Field Guide is waiting for its content; the ambulance one stays in the code for now.
        "fieldguide" => placeholders::fieldguide(ui),
        "map" => map::show(ui, &mut apps.map, now, radius),
        "settings" => settings::show(ui, &mut apps.settings, core, now, radius),
        "records" => placeholders::records(ui),
        "notes" => placeholders::notes(ui, &mut apps.notes),
        "about" => placeholders::about(ui),
        _ => {}
    }
    let _ = open;
}

/// A scrolling panel with the big Syne title and a grey subtitle.
pub fn panel(ui: &mut Ui, id: &str, title: &str, subtitle: &str, add: impl FnOnce(&mut Ui)) {
    let narrow = ui.max_rect().width() < 420.0;
    egui::ScrollArea::vertical().id_salt(id).auto_shrink([false, false]).show(ui, |ui| {
        egui::Frame::NONE.inner_margin(if narrow { egui::Margin::symmetric(16, 18) } else { egui::Margin::symmetric(24, 22) }).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new(title).font(display(if narrow { 24.0 } else { 30.0 }, 700)).color(ink()));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(egui::RichText::new(subtitle).font(font(13.0, 400)).color(white(0.55)));
                });
            });
            ui.add_space(10.0);
            add(ui);
        });
    });
}

pub fn label(ui: &mut Ui, text: impl Into<String>, f: egui::FontId, color: egui::Color32) -> egui::Response {
    ui.add(egui::Label::new(egui::RichText::new(text.into()).font(f).color(color)).wrap())
}
