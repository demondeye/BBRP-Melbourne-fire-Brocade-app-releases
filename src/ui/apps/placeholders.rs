//! Placeholder apps — replace with real screens (and the database) later.

use super::{label, panel};
use crate::ui::icons;
use crate::ui::theme::*;
use eframe::egui::{self, CornerRadius, Sense, Stroke, StrokeKind, Ui, Vec2};

pub fn records(ui: &mut Ui) {
    panel(ui, "records", "Incident Reports", "Waiting for database", |ui| {
        label(ui, "No reports yet. This app will read from Firebase once it's connected.", font(14.0, 400), white(0.7));
    });
}

pub fn fieldguide(ui: &mut Ui) {
    panel(ui, "fieldguide", "Field Guide", "Coming soon", |ui| {
        label(ui, "The fire service field guide will go here once its cheat sheet is ready.", font(14.0, 400), white(0.7));
    });
}

pub fn notes(ui: &mut Ui, text: &mut String) {
    let rect = ui.max_rect();
    let narrow = rect.width() < 420.0;
    egui::Frame::NONE.inner_margin(if narrow { egui::Margin::symmetric(16, 18) } else { egui::Margin::symmetric(24, 22) }).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("Notes").font(display(if narrow { 24.0 } else { 30.0 }, 700)));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(egui::RichText::new("Not saved yet").font(font(13.0, 400)).color(white(0.55)));
            });
        });
        ui.add_space(10.0);
        let size = Vec2::new(ui.available_width(), (rect.bottom() - ui.cursor().top() - 22.0).max(120.0));
        let (box_rect, _) = ui.allocate_exact_size(size, Sense::hover());
        ui.painter().rect(box_rect, CornerRadius::same(16), card_fill(), Stroke::new(1.0_f32, white(0.18)), StrokeKind::Inside);
        let inner = box_rect.shrink2(Vec2::new(16.0, 14.0));
        let mut child = ui.new_child(egui::UiBuilder::new().max_rect(inner));
        let resp = egui::ScrollArea::vertical().id_salt("notes-scroll").auto_shrink([false, false]).show(&mut child, |ui| {
            ui.add_sized(
                Vec2::new(inner.width(), inner.height()),
                egui::TextEdit::multiline(text)
                    .id(egui::Id::new("notes-text"))
                    .frame(false)
                    .font(font(14.0, 400))
                    .hint_text(egui::RichText::new("Type a note…").color(white(0.45)))
                    .margin(Vec2::ZERO),
            )
        });
        if resp.inner.has_focus() {
            ui.painter().rect_stroke(box_rect.expand(2.0), CornerRadius::same(18), Stroke::new(2.0_f32, ice_text()), StrokeKind::Outside);
        }
    });
}

pub fn about(ui: &mut Ui) {
    let rect = ui.max_rect();
    ui.allocate_new_ui(egui::UiBuilder::new().max_rect(rect), |ui| {
        ui.vertical_centered(|ui| {
            let content_h = 72.0 + 10.0 + 30.0 + 8.0 + 18.0 + 16.0 + 40.0;
            ui.add_space(((rect.height() - content_h) / 2.0).max(16.0));
            icons::logo(ui, 72.0);
            ui.add_space(6.0);
            ui.label(egui::RichText::new("Melbourne Fire Brigade").font(display(24.0, 800)));
            ui.label(egui::RichText::new(format!("Version {}", env!("CARGO_PKG_VERSION"))).font(font(13.0, 400)).color(white(0.7)));
            ui.add_space(8.0);
            ui.set_max_width(280.0);
            ui.add(egui::Label::new(egui::RichText::new("A fictional roleplay service. Not affiliated with any real fire or emergency service.").font(font(12.0, 400)).color(white(0.5))).wrap());
        });
    });
}
