//! Time Clock: clock in / clock out. The same button can be pressed from a Stream Deck key.

use super::label;
use crate::core::Core;
use crate::timeclock::{now_ms, Shift};
use crate::ui::desktop::{Confirm, ConfirmAction};
use crate::ui::format::{self, day_ms, duration, hours, month_key, month_label_key, shift_key, time_ms};
use crate::ui::icons;
use crate::ui::theme::{self, *};
use chrono::{Datelike, Local, NaiveDate, NaiveTime, TimeZone};
use eframe::egui::{self, Align, Align2, CornerRadius, Layout, Margin, Pos2, Rect, RichText, Sense, Stroke, StrokeKind, Ui, Vec2};

#[derive(Default)]
pub struct TimeClockUi {
    deleting: Option<i64>,
    editing: Option<Editor>,
    /// One month is shown at a time; None = always the current month (so it rolls over by itself).
    view_key: Option<String>,
    pub picking: bool,
    picker_year: i32,
}

struct Editor {
    start: i64,
    date: String,
    from: String,
    to: String,
}

struct Month {
    key: String,
    shifts: Vec<Shift>,
    total: i64,
    running: Option<i64>,
}

/// Shifts grouped by the month they started in, newest month first. The current
/// month is always there (even before its first shift). A shift counts towards
/// the month it started in — including one still running.
fn by_month(shifts: &[Shift], now: i64, active_since: Option<i64>) -> Vec<Month> {
    let mut months: Vec<Month> = vec![Month { key: month_key(now), shifts: vec![], total: 0, running: None }];
    if let Some(since) = active_since {
        let key = month_key(since);
        match months.iter_mut().find(|m| m.key == key) {
            Some(m) => m.running = Some(now - since),
            None => months.push(Month { key, shifts: vec![], total: 0, running: Some(now - since) }),
        }
    }
    for s in shifts {
        let key = month_key(s.start);
        let i = match months.iter().position(|m| m.key == key) {
            Some(i) => i,
            None => {
                months.push(Month { key, shifts: vec![], total: 0, running: None });
                months.len() - 1
            }
        };
        months[i].shifts.push(*s);
        months[i].total += s.end - s.start;
    }
    months.sort_by(|a, b| b.key.cmp(&a.key));
    months
}

pub fn show(ui: &mut Ui, st: &mut TimeClockUi, core: &mut Core, confirm: &mut Option<Confirm>, now_f: f64) {
    let _ = now_f;
    ui.ctx().request_repaint_after(std::time::Duration::from_millis(250));
    let Some(name) = core.user.clone() else { return };
    let now = now_ms();
    let on_duty = core.sheet.on_duty();
    let months = by_month(&core.sheet.shifts, now, core.sheet.active_since);
    let current_key = month_key(now);
    let shown_key = st.view_key.clone().unwrap_or(current_key.clone());
    let oldest_key = months.last().map(|m| m.key.clone()).unwrap_or(current_key.clone());
    let this_month = months.iter().find(|m| m.key == current_key).map_or(0, |m| m.total + m.running.unwrap_or(0));

    super::panel(ui, "timeclock", "Time Clock", &day_ms(now), |ui| {
        let w = ui.available_width();
        // ── The clock ──
        egui::Frame::NONE
            .fill(card_fill())
            .stroke(Stroke::new(1.0_f32, white(0.18)))
            .corner_radius(CornerRadius::same(RADIUS_CARD))
            .inner_margin(Margin::symmetric(16, 22))
            .show(ui, |ui| {
                ui.set_width(w - 32.0);
                ui.vertical_centered(|ui| {
                    ui.spacing_mut().item_spacing.y = 14.0;
                    let status = if let Some(since) = core.sheet.active_since { format!("On duty since {}", time_ms(since)) } else { "Off duty".to_string() };
                    let g = ui.fonts(|f| f.layout_no_wrap(status.clone(), font(13.0, 400), white(0.7)));
                    let (r, _) = ui.allocate_exact_size(Vec2::new(g.size().x + 16.0, g.size().y), Sense::hover());
                    theme::dot(ui.painter(), Pos2::new(r.left() + 4.0, r.center().y), on_duty, true);
                    ui.painter().galley(Pos2::new(r.left() + 16.0, r.top()), g, white(0.7));

                    let timer = duration(core.sheet.active_since.map_or(0, |s| now - s));
                    ui.label(RichText::new(timer).font(display(if w < 400.0 { 38.0 } else { 46.0 }, 700)));

                    let label = if on_duty { "Clock out" } else { "Clock in" };
                    let bw = if w < 400.0 { w - 64.0 } else { 200.0 };
                    let kind = if on_duty { Btn::Danger } else { Btn::Primary };
                    if theme::button(ui, label, kind, 15.0, Some(bw), Vec2::new(28.0, 12.0)).clicked() {
                        core.sheet.toggle(&name);
                    }
                    let job = {
                        let mut j = egui::text::LayoutJob::default();
                        j.append("This month: ", 0.0, egui::TextFormat::simple(font(13.0, 400), white(0.7)));
                        j.append(&hours(this_month), 0.0, egui::TextFormat::simple(font(13.0, 600), ink()));
                        j
                    };
                    ui.label(job);
                });
            });
        ui.add_space(22.0);

        // ── Shifts ──
        ui.horizontal(|ui| {
            ui.label(RichText::new("Shifts").font(display(16.0, 700)));
            if !core.sheet.shifts.is_empty() {
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    let g = ui.fonts(|f| f.layout_no_wrap("Clear all".into(), font(12.0, 400), red_text()));
                    let (r, resp) = ui.allocate_exact_size(g.size() + Vec2::new(20.0, 8.0), Sense::click());
                    theme::hover_fill(ui, &resp, r, 255, rgba(239, 68, 68, 0.15));
                    ui.painter().galley(r.center() - g.size() / 2.0, g, rgba(252, 165, 165, 0.85));
                    if resp.clicked() {
                        let n = core.sheet.shifts.len();
                        *confirm = Some(Confirm {
                            title: "Clear all shifts?".into(),
                            message: format!("This deletes all {n} recorded shift{} for {name}. It can't be undone.", if n == 1 { "" } else { "s" }),
                            label: "Clear all".into(),
                            action: ConfirmAction::ClearShifts,
                            at: 0.0,
                        });
                    }
                });
            }
        });
        ui.add_space(4.0);

        if core.sheet.shifts.is_empty() && !on_duty {
            label(ui, "No shifts recorded yet.", font(14.0, 400), white(0.7));
            return;
        }

        let viewing = months.iter().find(|m| m.key == shown_key);
        let is_current = shown_key == current_key;
        let (fill, border) = if is_current { (rgba(245, 183, 168, 0.12), rgba(245, 183, 168, 0.4)) } else { (white(0.06), white(0.18)) };
        egui::Frame::NONE.fill(fill).stroke(Stroke::new(1.0_f32, border)).corner_radius(CornerRadius::same(20)).inner_margin(Margin::same(6)).show(ui, |ui| {
            ui.set_width(ui.available_width());
            // Month heading: ‹  This month ▾  12h 05m  ›
            ui.horizontal(|ui| {
                ui.add_space(6.0);
                if nav(ui, "‹", shown_key > oldest_key) {
                    let k = shift_key(&shown_key, -1);
                    st.view_key = (k != current_key).then_some(k);
                }
                let count = viewing.map_or(0, |m| m.shifts.len());
                let total = viewing.map_or(0, |m| m.total + m.running.unwrap_or(0));
                let right_w = 32.0 + 12.0 + ui.fonts(|f| f.layout_no_wrap(hours(total), display(18.0, 700), ink()).size().x) + 14.0;
                ui.allocate_ui_with_layout(Vec2::new((ui.available_width() - right_w).max(60.0), 40.0), Layout::top_down(Align::Min), |ui| {
                    ui.spacing_mut().item_spacing.y = 0.0;
                    let title = if is_current { "This month".to_string() } else { month_label_key(&shown_key) };
                    let g = ui.fonts(|f| f.layout_no_wrap(title, display(15.0, 700), ink()));
                    let (r, resp) = ui.allocate_exact_size(Vec2::new(g.size().x + 30.0, g.size().y + 4.0), Sense::click());
                    let r = r.translate(Vec2::new(-8.0, 0.0));
                    theme::hover_fill(ui, &resp, r, 10, white(0.12));
                    if st.picking {
                        ui.painter().rect_filled(r, 10.0, white(0.12));
                    }
                    ui.painter().galley(Pos2::new(r.left() + 8.0, r.top() + 2.0), g, ink());
                    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(Rect::from_min_size(Pos2::new(r.right() - 20.0, r.center().y - 7.0), Vec2::splat(14.0))));
                    icons::line_icon(&mut child, "chev", icons::CHEVRON, 14.0, ink(), 2.4);
                    if resp.clicked() {
                        st.picking = !st.picking;
                        st.picker_year = shown_key[..4].parse().unwrap_or(Local::now().year());
                    }
                    let mut small = String::new();
                    if is_current {
                        small += &format!("{} · ", month_label_key(&shown_key));
                    }
                    small += &format!("{count} shift{}", if count == 1 { "" } else { "s" });
                    if viewing.is_some_and(|m| m.running.is_some()) {
                        small += " · on duty now";
                    }
                    ui.label(RichText::new(small).font(font(12.0, 400)).color(white(0.6)));
                });
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.add_space(6.0);
                    if nav(ui, "›", shown_key < current_key) {
                        let k = shift_key(&shown_key, 1);
                        st.view_key = (k != current_key).then_some(k);
                    }
                    ui.label(RichText::new(hours(total)).font(display(18.0, 700)));
                });
            });

            if st.picking {
                month_picker(ui, st, &months, &shown_key, &current_key, &oldest_key);
            }
            if !is_current {
                let g = ui.fonts(|f| f.layout_no_wrap("Back to this month".into(), font(12.0, 400), ice_text()));
                ui.horizontal(|ui| {
                    ui.add_space(12.0);
                    let (r, resp) = ui.allocate_exact_size(g.size(), Sense::click());
                    if resp.hovered() {
                        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                        ui.painter().hline(r.x_range(), r.bottom(), Stroke::new(1.0_f32, ice_text()));
                    }
                    ui.painter().galley(r.min, g, ice_text());
                    if resp.clicked() {
                        st.view_key = None;
                    }
                });
            }
            ui.add_space(2.0);
            match viewing {
                Some(m) if !m.shifts.is_empty() => {
                    ui.spacing_mut().item_spacing.y = 10.0;
                    for s in &m.shifts {
                        shift_pill(ui, st, core, &name, *s);
                    }
                }
                _ => {
                    ui.horizontal(|ui| {
                        ui.add_space(12.0);
                        ui.label(RichText::new(format!("No finished shifts {}.", if is_current { "yet this month" } else { "in this month" })).font(font(13.0, 400)).color(white(0.7)));
                    });
                    ui.add_space(6.0);
                }
            }
        });
    });
}

/// A round ‹ › button. Returns true when clicked.
fn nav(ui: &mut Ui, glyph: &str, enabled: bool) -> bool {
    let (r, resp) = ui.allocate_exact_size(Vec2::splat(32.0), if enabled { Sense::click() } else { Sense::hover() });
    let alpha = if enabled { 1.0 } else { 0.3 };
    let hover = if enabled && resp.hovered() { 0.2 } else { 0.1 };
    ui.painter().circle_filled(r.center(), 16.0, fade(white(hover), alpha));
    ui.painter().text(r.center() - Vec2::new(0.0, 2.0), Align2::CENTER_CENTER, glyph, font(22.0, 400), fade(ink(), alpha));
    if enabled && resp.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    enabled && resp.clicked()
}

fn small_icon_button(ui: &mut Ui, id: &str, paths: &str, danger_hover: bool) -> bool {
    let (r, resp) = ui.allocate_exact_size(Vec2::splat(30.0), Sense::click());
    let hovered = resp.hovered();
    if hovered {
        ui.painter().circle_filled(r.center(), 15.0, if danger_hover { rgba(239, 68, 68, 0.18) } else { white(0.12) });
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    let color = if hovered { if danger_hover { red_text() } else { ink() } } else { white(0.55) };
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(Rect::from_center_size(r.center(), Vec2::splat(15.0))));
    icons::line_icon(&mut child, id, paths, 15.0, color, 2.0);
    resp.clicked()
}

fn shift_pill(ui: &mut Ui, st: &mut TimeClockUi, core: &mut Core, name: &str, s: Shift) {
    let deleting = st.deleting == Some(s.start);
    let editing = st.editing.as_ref().is_some_and(|e| e.start == s.start);
    let (fill, border) = if deleting {
        (rgba(239, 68, 68, 0.14), rgba(252, 165, 165, 0.35))
    } else if editing {
        (rgba(245, 183, 168, 0.24), white(0.25))
    } else {
        (rgba(245, 183, 168, 0.18), white(0.25))
    };
    let resp = egui::Frame::NONE
        .fill(fill)
        .stroke(Stroke::new(1.0_f32, border))
        .corner_radius(CornerRadius::same(if editing { 22 } else { 40 }))
        .inner_margin(Margin { left: 10, right: 10, top: 10, bottom: 10 })
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                let (r, _) = ui.allocate_exact_size(Vec2::splat(36.0), Sense::hover());
                ui.painter().circle_filled(r.center(), 18.0, WHITE);
                ui.painter().text(r.center(), Align2::CENTER_CENTER, format::local(s.start).day().to_string(), font(13.0, 700), ACCENT);
                ui.add_space(6.0);
                let buttons_w = if deleting { 140.0 } else if editing { 0.0 } else { 70.0 };
                ui.allocate_ui_with_layout(Vec2::new((ui.available_width() - buttons_w).max(40.0), 36.0), Layout::top_down(Align::Min), |ui| {
                    ui.spacing_mut().item_spacing.y = 0.0;
                    ui.add_space(1.0);
                    ui.label(RichText::new(format!("{} – {}", time_ms(s.start), time_ms(s.end))).font(font(14.0, 600)));
                    ui.label(RichText::new(format!("{} · {}", day_ms(s.start), duration(s.end - s.start))).font(font(12.0, 400)).color(white(0.6)));
                });
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.spacing_mut().item_spacing.x = 6.0;
                    if deleting {
                        if theme::button(ui, "Cancel", Btn::Subtle, 12.0, None, Vec2::new(12.0, 5.0)).clicked() {
                            st.deleting = None;
                        }
                        if theme::button(ui, "Delete", Btn::Danger, 12.0, None, Vec2::new(12.0, 5.0)).clicked() {
                            core.sheet.delete_shift(name, s.start);
                            st.deleting = None;
                        }
                    } else if !editing {
                        if small_icon_button(ui, "bin", icons::BIN, true) {
                            st.deleting = Some(s.start);
                            st.editing = None;
                        }
                        if small_icon_button(ui, "pencil", icons::PENCIL, false) {
                            let d = format::local(s.start);
                            let e = format::local(s.end);
                            st.editing = Some(Editor {
                                start: s.start,
                                date: d.format("%Y-%m-%d").to_string(),
                                from: d.format("%H:%M").to_string(),
                                to: e.format("%H:%M").to_string(),
                            });
                            st.deleting = None;
                        }
                    }
                });
            });
            if editing {
                shift_editor(ui, st, core, name);
            }
        });
    let _ = resp;
}

/// Edit a finished shift: its date, start and end time. An end time earlier
/// than the start means the shift ran past midnight.
fn shift_editor(ui: &mut Ui, st: &mut TimeClockUi, core: &mut Core, name: &str) {
    let Some(ed) = st.editing.as_mut() else { return };
    let parse = |date: &str, t: &str| -> Option<i64> {
        let d = NaiveDate::parse_from_str(date.trim(), "%Y-%m-%d").ok()?;
        let t = NaiveTime::parse_from_str(t.trim(), "%H:%M").ok()?;
        Local.from_local_datetime(&d.and_time(t)).earliest().map(|x| x.timestamp_millis())
    };
    let in_at = parse(&ed.date, &ed.from);
    let mut out_at = parse(&ed.date, &ed.to);
    let overnight = matches!((in_at, out_at), (Some(a), Some(b)) if b <= a);
    if overnight {
        out_at = out_at.map(|o| o + 24 * 60 * 60 * 1000);
    }
    let ok = matches!((in_at, out_at), (Some(a), Some(_)) if a <= now_ms());

    ui.add_space(4.0);
    let mut save = false;
    let mut cancel = false;
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 10.0;
        field(ui, "Date", &mut ed.date, "YYYY-MM-DD", 96.0, "ed-date");
        field(ui, "Start", &mut ed.from, "HH:MM", 54.0, "ed-from");
        field(ui, "End", &mut ed.to, "HH:MM", 54.0, "ed-to");
    });
    ui.horizontal(|ui| {
        let note = match (ok, in_at, out_at) {
            (true, Some(a), Some(b)) => format!("{}{}", duration(b - a), if overnight { " · ends the next day" } else { "" }),
            _ => "Check the date and times.".into(),
        };
        ui.label(RichText::new(note).font(font(12.0, 400)).color(white(0.7)));
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.spacing_mut().item_spacing.x = 6.0;
            if theme::button(ui, "Cancel", Btn::Subtle, 12.0, None, Vec2::new(12.0, 5.0)).clicked() {
                cancel = true;
            }
            if theme::button_enabled(ui, "Save", Btn::Primary, 12.0, None, Vec2::new(12.0, 5.0), ok).clicked() {
                save = true;
            }
        });
    });
    if save {
        if let (Some(a), Some(b)) = (in_at, out_at) {
            let old = ed.start;
            core.sheet.update_shift(name, old, Shift { start: a, end: b });
            st.editing = None;
        }
    } else if cancel {
        st.editing = None;
    }
}

fn field(ui: &mut Ui, label: &str, value: &mut String, hint: &str, width: f32, id: &str) {
    ui.vertical(|ui| {
        ui.spacing_mut().item_spacing.y = 4.0;
        ui.label(RichText::new(label).font(font(11.0, 400)).color(white(0.65)));
        let (r, _) = ui.allocate_exact_size(Vec2::new(width + 20.0, 30.0), Sense::hover());
        ui.painter().rect(r, CornerRadius::same(10), if is_light() { WHITE } else { rgba(0, 0, 0, 0.25) }, Stroke::new(1.0_f32, white(0.25)), StrokeKind::Inside);
        let edit = egui::TextEdit::singleline(value)
            .id(egui::Id::new(id))
            .frame(false)
            .font(font(13.0, 400))
            .hint_text(RichText::new(hint).color(white(0.4)))
            .desired_width(width)
            .margin(Vec2::ZERO);
        ui.put(r.shrink2(Vec2::new(10.0, 7.0)), edit);
    });
}

/// Pick any month: a year at a time, twelve months in a grid. Months with
/// shifts show their total; months after the current one can't be picked.
fn month_picker(ui: &mut Ui, st: &mut TimeClockUi, months: &[Month], shown_key: &str, current_key: &str, oldest_key: &str) {
    let oldest_year: i32 = oldest_key[..4].parse().unwrap_or(2026);
    let current_year: i32 = current_key[..4].parse().unwrap_or(2026);
    let resp = egui::Frame::NONE
        .fill(menu_bg())
        .stroke(Stroke::new(1.0_f32, white(0.18)))
        .corner_radius(CornerRadius::same(18))
        .inner_margin(Margin::same(12))
        .outer_margin(Margin { left: 6, right: 6, top: 4, bottom: 8 })
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                if nav(ui, "‹", st.picker_year > oldest_year) {
                    st.picker_year -= 1;
                }
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if nav(ui, "›", st.picker_year < current_year) {
                        st.picker_year += 1;
                    }
                    ui.centered_and_justified(|ui| {
                        ui.label(RichText::new(st.picker_year.to_string()).font(display(17.0, 700)));
                    });
                });
            });
            ui.add_space(4.0);
            let gap = 6.0;
            let cell_w = (ui.available_width() - gap * 2.0) / 3.0;
            for row in 0..4 {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = gap;
                    for col in 0..3 {
                        let m = row * 3 + col + 1;
                        let key = format!("{}-{:02}", st.picker_year, m);
                        let total: i64 = months.iter().find(|x| x.key == key).map_or(0, |x| x.total + x.running.unwrap_or(0));
                        let enabled = key.as_str() <= current_key;
                        let (r, resp) = ui.allocate_exact_size(Vec2::new(cell_w, 46.0), if enabled { Sense::click() } else { Sense::hover() });
                        let shown = key == shown_key;
                        let mut fill = if total > 0 { rgba(245, 183, 168, 0.16) } else { white(0.06) };
                        if enabled && resp.hovered() {
                            fill = white(0.16);
                            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                        }
                        if shown {
                            fill = WHITE;
                        }
                        let a = if enabled { 1.0 } else { 0.3 };
                        let border = if key == current_key { rgba(245, 183, 168, 0.5) } else { egui::Color32::TRANSPARENT };
                        ui.painter().rect(r, CornerRadius::same(12), fade(fill, a), Stroke::new(1.0_f32, border), StrokeKind::Inside);
                        let tc = if shown { ACCENT } else { ink() };
                        ui.painter().text(Pos2::new(r.center().x, r.top() + 8.0), Align2::CENTER_TOP, format::month_short(m as u32), font(13.0, 600), fade(tc, a));
                        let sc = if shown { ACCENT } else if total > 0 { ice_text() } else { white(0.5) };
                        ui.painter().text(Pos2::new(r.center().x, r.bottom() - 8.0), Align2::CENTER_BOTTOM, if total > 0 { hours(total) } else { "–".into() }, font(11.0, 400), fade(sc, a));
                        if enabled && resp.clicked() {
                            st.view_key = (key != current_key).then_some(key);
                            st.picking = false;
                        }
                    }
                });
            }
        });
    // Clicking outside the picker closes it (the month heading toggles it itself).
    let rect = resp.response.rect;
    if ui.input(|i| i.pointer.any_pressed() && i.pointer.interact_pos().is_some_and(|p| !rect.expand(4.0).contains(p) && p.y > rect.top())) {
        st.picking = false;
    }
}
