//! The desktop: wallpaper, menu bar, windows, dock, launcher, menus, dialogs.

use super::apps::{self, Apps};
use super::icons;
use super::theme::{self, *};
use super::wm::{self, WindowManager, APPS, DOCK};
use crate::core::{Core, UiEvent};
use eframe::egui::{
    self, emath::TSTransform, Align2, Area, Color32, CornerRadius, CursorIcon, Id, Key, LayerId, Order, Pos2, Rect, Sense, Stroke, StrokeKind, Ui, UiBuilder, Vec2,
};

#[derive(Clone, Copy, PartialEq)]
pub enum Power {
    Restart,
    Shutdown,
}

#[derive(Clone, Copy, PartialEq)]
pub enum ConfirmAction {
    Power(Power),
    ClearShifts,
}

pub struct Confirm {
    pub title: String,
    pub message: String,
    pub label: String,
    pub action: ConfirmAction,
    pub at: f64,
}

#[derive(Clone, Copy)]
enum CtxKind {
    Desktop,
    Text(Id),
}
struct CtxMenu {
    pos: Pos2,
    kind: CtxKind,
}

pub enum DesktopOut {
    None,
    Logout,
    Power(Power),
}

pub struct Desktop {
    pub wm: WindowManager,
    launcher: Option<f64>,
    launcher_q: String,
    menu_open: bool,
    dock_menu: Option<(&'static str, f32)>,
    ctx_menu: Option<CtxMenu>,
    toast: Option<(String, f64)>,
    pub confirm: Option<Confirm>,
    pub apps: Apps,
    entered_at: f64,
    fieldguide_was_open: bool,
    raised: Option<&'static str>,
    update_popup_at: Option<f64>,
}

const DOCK_LAYER: &str = "dock";
const MENUBAR_LAYER: &str = "menubar";

fn win_layer(id: &str) -> LayerId {
    LayerId::new(Order::Middle, Id::new(("window", id)))
}

impl Desktop {
    pub fn new(now: f64) -> Self {
        Self {
            wm: WindowManager::new(),
            launcher: None,
            launcher_q: String::new(),
            menu_open: false,
            dock_menu: None,
            ctx_menu: None,
            toast: None,
            confirm: None,
            apps: Apps::default(),
            entered_at: now,
            fieldguide_was_open: false,
            raised: None,
            update_popup_at: None,
        }
    }

    fn open(&mut self, id: &'static str, area: Rect, now: f64) {
        self.wm.open(id, (area.width(), area.height()), now);
        self.wm.touch(now);
    }

    pub fn show(&mut self, ctx: &egui::Context, core: &mut Core, now: f64) -> DesktopOut {
        let mut out = DesktopOut::None;
        let screen = ctx.screen_rect();
        let area = Rect::from_min_max(Pos2::new(screen.left(), screen.top() + MENUBAR_H), screen.max);
        let narrow = screen.width() < 1024.0;
        // Light or dark windows, from the user's Appearance setting.
        let light = match core.settings.appearance.as_str() {
            "light" => true,
            "auto" => ctx.system_theme() == Some(egui::Theme::Light),
            _ => false,
        };
        theme::set_light(ctx, light);
        let enter = ((now - self.entered_at) / 0.5).clamp(0.0, 1.0) as f32;

        // Things the Stream Deck asked for.
        for event in std::mem::take(&mut core.events) {
            match event {
                UiEvent::OpenApp(id) => self.open(id, area, now),
                UiEvent::Toast(text) => self.toast = Some((text, now)),
            }
        }
        // The Field Guide always starts fresh: closing its window clears every
        // tick and choice, so the next time it opens nothing is filled in.
        let fg_open = self.wm.get("fieldguide").is_some();
        if self.fieldguide_was_open && !fg_open {
            core.guide = Default::default();
        }
        self.fieldguide_was_open = fg_open;
        self.wm.tick(now);

        if ctx.input(|i| i.key_pressed(Key::Escape)) {
            if self.confirm.is_some() {
                self.confirm = None;
            } else {
                self.launcher = None;
                self.menu_open = false;
                self.dock_menu = None;
                self.ctx_menu = None;
                self.apps.escape();
            }
        }

        // ── Wallpaper (and right-click on the empty desktop) ──
        egui::CentralPanel::default().frame(egui::Frame::NONE).show(ctx, |ui| {
            super::wallpaper::paint(ui, ui.painter(), screen, &core.settings, true);
            if enter < 1.0 {
                ui.painter().rect_filled(screen, 0.0, theme::rgba(0, 0, 0, 1.0 - enter));
            }
            let resp = ui.interact(area, Id::new("desktop-area"), Sense::click());
            if resp.secondary_clicked() {
                if let Some(pos) = resp.interact_pointer_pos() {
                    self.ctx_menu = Some(CtxMenu { pos, kind: CtxKind::Desktop });
                }
            }
        });

        // Clicking anywhere in a window brings it to the front.
        if ctx.input(|i| i.pointer.any_pressed()) {
            if let Some(pos) = ctx.input(|i| i.pointer.interact_pos()) {
                if let Some(layer) = ctx.layer_id_at(pos) {
                    if let Some(w) = self.wm.windows.iter().find(|w| win_layer(w.id) == layer && !w.minimized) {
                        let id = w.id;
                        if self.wm.active() != Some(id) {
                            self.wm.focus(id);
                        }
                    }
                }
            }
        }

        // ── Windows ──
        let mut order: Vec<wm::Win> = self.wm.windows.clone();
        order.sort_by_key(|w| w.z);
        let active = self.wm.active();
        for win in &order {
            self.draw_window(ctx, core, win, active == Some(win.id), area, narrow, now);
        }
        // Keep the screen's stacking in step with ours: raise the front window when it changes.
        // (Only one layer is raised per frame, so the order is never shuffled.)
        let front = self.wm.active();
        if front != self.raised {
            if let Some(id) = front {
                ctx.move_to_top(win_layer(id));
            }
            self.raised = front;
        }

        // ── Launcher ──
        if let Some(opened) = self.launcher {
            self.draw_launcher(ctx, screen, area, opened, now);
        }

        // ── Dock ──
        self.draw_dock(ctx, screen, area, now, enter);

        // ── Menu bar ──
        if let Some(o) = self.draw_menubar(ctx, core, screen, now) {
            out = o;
        }

        // ── Toast ──
        if let Some((text, at)) = self.toast.clone() {
            let age = (now - at) as f32;
            if age > 3.5 {
                self.toast = None;
            } else {
                let slide = ease_out(age / 0.35);
                let alpha = slide * (1.0 - ((age - 3.1) / 0.4).clamp(0.0, 1.0));
                Area::new(Id::new("toast")).order(Order::Debug).interactable(false).fixed_pos(Pos2::ZERO).show(ctx, |ui| {
                    let f = font(14.0, 500);
                    let g = ui.fonts(|fo| fo.layout_no_wrap(text.clone(), f, Color32::PLACEHOLDER));
                    let size = g.size() + Vec2::new(32.0, 20.0);
                    let x = screen.right() - 14.0 - size.x + (1.0 - slide) * 30.0;
                    let r = Rect::from_min_size(Pos2::new(x, area.top() + 12.0), size);
                    let p = ui.painter();
                    theme::shadow(p, r, 14, 12, 30, 0.35 * alpha);
                    card(p, r, 14, fade(menu_bg(), alpha), fade(white(0.18), alpha));
                    p.galley(r.center() - g.size() / 2.0, g, fade(ink(), alpha));
                });
                ctx.request_repaint();
            }
        }

        // ── "Are you sure?" dialog ──
        if let Some(action) = self.draw_confirm(ctx, screen, now) {
            match action {
                ConfirmAction::Power(p) => out = DesktopOut::Power(p),
                ConfirmAction::ClearShifts => {
                    if let Some(name) = core.user.clone() {
                        core.sheet.clear(&name);
                    }
                }
            }
        }

        // ── "New update is ready to install" ──
        self.draw_update_popup(ctx, core, screen, area, now);

        // ── Right-click menu ──
        self.draw_context_menu(ctx, area, now);

        out
    }

    #[allow(clippy::too_many_arguments)]
    fn draw_window(&mut self, ctx: &egui::Context, core: &mut Core, win: &wm::Win, is_active: bool, area: Rect, narrow: bool, now: f64) {
        let Some(app) = wm::app(win.id) else { return };
        let layer = win_layer(win.id);
        let min_t = ctx.animate_bool_with_time(Id::new(("min", win.id)), win.minimized, 0.3);
        if win.minimized && min_t >= 0.999 {
            return;
        }
        let open_t = ease_out(((now - win.opened_at) / 0.28) as f32);
        if open_t < 1.0 {
            ctx.request_repaint();
        }
        let dock_h = 52.0 + 34.0;
        let rect = if narrow {
            Rect::from_min_size(area.min, Vec2::new(area.width(), (area.height() - dock_h).max(200.0)))
        } else if win.maximized {
            area
        } else {
            Rect::from_min_size(area.min + Vec2::new(win.x, win.y), Vec2::new(win.w, win.h))
        };
        let full = narrow || win.maximized;
        let radius = if full { 0 } else { RADIUS_WINDOW };

        // Opening / minimising animation.
        let ease_min = ease_out(min_t);
        let scale = (0.96 + 0.04 * open_t) * (1.0 - 0.8 * ease_min);
        let offset = Vec2::new(0.0, (1.0 - open_t) * 8.0 + ease_min * area.height() * 0.6);
        let c = rect.center().to_vec2();
        ctx.set_transform_layer(layer, TSTransform::new(c * (1.0 - scale) + offset, scale));
        let opacity = open_t * (1.0 - min_t);

        let mut actions = WinActions::default();
        Area::new(layer.id)
            .order(Order::Middle)
            .fixed_pos(rect.min)
            .constrain(false)
            .interactable(!win.minimized)
            .show(ctx, |ui| {
                ui.set_opacity(opacity);
                let (rect, _) = ui.allocate_exact_size(rect.size(), Sense::click());
                let p = ui.painter().clone();
                if !full {
                    theme::shadow(&p, rect, radius, if is_active { 30 } else { 20 }, if is_active { 80 } else { 50 }, if is_active { 0.5 } else { 0.35 });
                }
                p.rect(rect, CornerRadius::same(radius), window_bg(), Stroke::new(if full { 0.0_f32 } else { 1.0 }, if is_active { white(0.28) } else { white(0.18) }), StrokeKind::Inside);

                // Title bar
                let bar = Rect::from_min_size(rect.min, Vec2::new(rect.width(), 38.0));
                p.rect_filled(bar.shrink2(Vec2::new(1.0, 0.0)).translate(Vec2::new(0.0, 1.0)), CornerRadius { nw: radius.saturating_sub(1), ne: radius.saturating_sub(1), sw: 0, se: 0 }, sidebar_bg());
                p.hline(bar.x_range(), bar.bottom(), Stroke::new(1.0_f32, white(0.1)));
                let bar_resp = ui.interact(bar, Id::new(("bar", win.id)), Sense::click_and_drag());
                if bar_resp.double_clicked() && !narrow {
                    actions.maximize = true;
                }
                if bar_resp.dragged() && !full {
                    actions.moved = Some(bar_resp.drag_delta());
                }
                let title_font = font(13.0, 600);
                let title = theme::elide(ui, app.name, &title_font, (rect.width() - 160.0).max(20.0));
                p.text(bar.center(), Align2::CENTER_CENTER, title, title_font, if is_active { white(0.9) } else { white(0.55) });

                let colors = [(239u8, 113u8, 113u8), (251, 191, 36), (74, 222, 128)];
                let size = if narrow { 16.0 } else { 13.0 };
                let gap = if narrow { 12.0 } else { 8.0 };
                for (i, name) in ["close", "min", "max"].iter().enumerate() {
                    if narrow && *name == "max" {
                        continue;
                    }
                    let center = Pos2::new(bar.left() + 12.0 + size / 2.0 + i as f32 * (size + gap), bar.center().y);
                    let hit = Rect::from_center_size(center, Vec2::splat(size + 4.0));
                    let r = ui.interact(hit, Id::new(("btn", win.id, *name)), Sense::click());
                    let (cr, cg, cb) = colors[i];
                    let mut col = if is_active { Color32::from_rgb(cr, cg, cb) } else { white(0.25) };
                    if r.hovered() {
                        col = lerp_color(col, WHITE, 0.2);
                        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
                    }
                    p.circle_filled(center, size / 2.0, col);
                    if r.clicked() {
                        match *name {
                            "close" => actions.close = true,
                            "min" => actions.minimize = true,
                            _ => actions.maximize = true,
                        }
                    }
                }

                // Body
                let body = Rect::from_min_max(Pos2::new(rect.left(), bar.bottom()), rect.max);
                let mut child = ui.new_child(UiBuilder::new().max_rect(body).layout(egui::Layout::top_down(egui::Align::Min)));
                child.set_clip_rect(body.intersect(ui.clip_rect()));
                child.set_min_size(body.size());
                let mut confirm = None;
                apps::show_app(&mut child, win.id, &mut self.apps, core, &mut confirm, &mut actions.open, now, radius);
                if let Some(c) = confirm {
                    self.confirm = Some(Confirm { at: now, ..c });
                }

                // Resize handle
                if !full {
                    let handle = Rect::from_min_max(rect.max - Vec2::splat(18.0), rect.max);
                    let r = ui.interact(handle, Id::new(("resize", win.id)), Sense::drag());
                    if r.hovered() || r.dragged() {
                        ui.ctx().set_cursor_icon(CursorIcon::ResizeNwSe);
                    }
                    if r.dragged() {
                        actions.resized = Some(r.drag_delta());
                    }
                }
            });

        // Apply what happened.
        if actions.close {
            self.wm.close(win.id);
        }
        if actions.minimize {
            self.wm.minimize(win.id);
        }
        if actions.maximize {
            self.wm.toggle_maximize(win.id, now);
        }
        if let Some(d) = actions.moved {
            let (aw, ah) = (area.width(), area.height());
            if let Some(w) = self.wm.get_mut(win.id) {
                w.x = (w.x + d.x).max(80.0 - w.w).min(aw - 80.0);
                w.y = (w.y + d.y).max(0.0).min(ah - 60.0);
            }
            self.wm.touch(now);
        }
        if let Some(d) = actions.resized {
            if let Some(w) = self.wm.get_mut(win.id) {
                w.w = (w.w + d.x).max(320.0);
                w.h = (w.h + d.y).max(220.0);
            }
            self.wm.touch(now);
        }
        if let Some(id) = actions.open {
            self.open(id, area, now);
        }
    }

    fn draw_menubar(&mut self, ctx: &egui::Context, core: &mut Core, screen: Rect, now: f64) -> Option<DesktopOut> {
        let mut out = None;
        let bar = Rect::from_min_size(screen.min, Vec2::new(screen.width(), MENUBAR_H));
        let slide = ease_out((((now - self.entered_at) - 0.2) / 0.6).clamp(0.0, 1.0) as f32);
        let y = -(1.0 - slide) * MENUBAR_H;
        let mut logo_rect = Rect::NOTHING;
        Area::new(Id::new(MENUBAR_LAYER)).order(Order::Tooltip).fixed_pos(bar.min).show(ctx, |ui| {
            ui.set_opacity(slide);
            let bar = bar.translate(Vec2::new(0.0, y));
            let (_, _) = ui.allocate_exact_size(bar.size(), Sense::click());
            let p = ui.painter();
            p.rect_filled(bar, 0.0, rgba(0, 0, 0, 0.32));
            p.hline(bar.x_range(), bar.bottom() - 0.5, Stroke::new(1.0_f32, lit(0.1)));

            // System menu (the logo)
            logo_rect = Rect::from_min_size(Pos2::new(bar.left() + 10.0, bar.top() + 3.0), Vec2::new(34.0, 24.0));
            let r = ui.interact(logo_rect, Id::new("menubar-logo"), Sense::click());
            let t = ui.ctx().animate_bool_with_time(r.id, r.hovered() || self.menu_open, 0.2);
            if t > 0.0 {
                p.rect_filled(logo_rect, 6.0, lit(0.16 * t));
            }
            if r.hovered() {
                ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
            }
            icons::logo_at(ui, Rect::from_center_size(logo_rect.center(), Vec2::splat(18.0)));
            if r.clicked() {
                self.menu_open = !self.menu_open;
            }
            let app_name = self.wm.active().and_then(wm::app).map_or("Melbourne Fire Brigade", |a| a.name);
            p.text(Pos2::new(logo_rect.right() + 12.0, bar.center().y), Align2::LEFT_CENTER, app_name, font(13.0, 600), Color32::WHITE);

            // Clock and status on the right
            let now_local = chrono::Local::now();
            let time = super::format::time(now_local);
            let date = super::format::day(now_local);
            let f = font(13.0, 400);
            let mut x = bar.right() - 10.0;
            let r = p.text(Pos2::new(x, bar.center().y), Align2::RIGHT_CENTER, &time, f.clone(), lit(0.85));
            x = r.left() - 12.0;
            if screen.width() >= 768.0 {
                let r = p.text(Pos2::new(x, bar.center().y), Align2::RIGHT_CENTER, &date, f.clone(), lit(0.85));
                x = r.left() - 12.0;
                let r = p.text(Pos2::new(x, bar.center().y), Align2::RIGHT_CENTER, "Local only", f.clone(), lit(0.85));
                x = r.left() - 6.0;
            }
            let status_rect = Rect::from_center_size(Pos2::new(x - 4.0, bar.center().y), Vec2::splat(8.0));
            p.circle_filled(status_rect.center(), 4.0, lit(0.4));
            ui.interact(status_rect.expand(4.0), Id::new("status-tip"), Sense::hover()).on_hover_text("No database connected yet");
        });
        let _ = core;

        if self.menu_open {
            let pos = Pos2::new(screen.left() + 10.0, screen.top() + MENUBAR_H - 2.0);
            let user = core.user.clone().unwrap_or_default();
            let items: Vec<(&str, String, bool)> = vec![
                ("about", "About Melbourne Fire Brigade".into(), false),
                ("-", String::new(), false),
                ("settings", "Settings…".into(), false),
                ("update", if core.update.release().is_some() { "Update ready to install…".into() } else { "Software Update…".into() }, false),
                ("-", String::new(), false),
                ("restart", "Restart…".into(), false),
                ("shutdown", "Shut Down…".into(), false),
                ("-", String::new(), false),
                ("logout", format!("Log out {user}…"), true),
            ];
            let mut picked = None;
            let rect = menu_area(ctx, "menubar-dropdown", pos, 230.0, &items, &mut picked);
            let area = Rect::from_min_max(Pos2::new(screen.left(), screen.top() + MENUBAR_H), screen.max);
            if let Some(id) = picked {
                self.menu_open = false;
                match id {
                    "about" => self.open("about", area, now),
                    "settings" => self.open("settings", area, now),
                    "update" => {
                        self.apps.settings.go(super::apps::settings::UPDATE_SECTION);
                        self.open("settings", area, now);
                    }
                    "restart" => self.ask_power(Power::Restart, now),
                    "shutdown" => self.ask_power(Power::Shutdown, now),
                    "logout" => out = Some(DesktopOut::Logout),
                    _ => {}
                }
            } else if clicked_outside(ctx, &[rect, logo_rect]) {
                self.menu_open = false;
            }
        }
        out
    }

    fn ask_power(&mut self, p: Power, now: f64) {
        let restart = p == Power::Restart;
        self.confirm = Some(Confirm {
            title: if restart { "Restart now?" } else { "Shut down now?" }.into(),
            message: format!("You will be logged out and all open apps will close{}", if restart { ", then the system will start again." } else { ", then Melbourne Fire Brigade will turn off." }),
            label: if restart { "Restart" } else { "Shut Down" }.into(),
            action: ConfirmAction::Power(p),
            at: now,
        });
    }

    fn draw_dock(&mut self, ctx: &egui::Context, screen: Rect, area: Rect, now: f64, _enter: f32) {
        let icon = if screen.width() < 768.0 { 40.0 } else if screen.width() < 1024.0 { 46.0 } else if screen.width() >= 1920.0 { 64.0 } else { 52.0 };
        let gap = if screen.width() >= 1920.0 { 12.0 } else { 8.0 };
        let pad = if screen.width() >= 1920.0 { Vec2::new(14.0, 10.0) } else { Vec2::new(10.0, 8.0) };
        let mut ids: Vec<&'static str> = DOCK.to_vec();
        for w in &self.wm.windows {
            if !DOCK.contains(&w.id) {
                ids.push(w.id);
            }
        }
        let count = ids.len() as f32 + 1.0;
        let width = pad.x * 2.0 + count * icon + (count - 1.0) * gap + 1.0 + gap;
        let height = icon + pad.y * 2.0;
        let rise = ease_out((((now - self.entered_at) - 0.5) / 0.7).clamp(0.0, 1.0) as f32);
        let dock = Rect::from_min_size(Pos2::new(screen.center().x - width / 2.0, screen.bottom() - 10.0 - height + (1.0 - rise) * height * 1.4), Vec2::new(width, height));
        if rise < 1.0 {
            ctx.request_repaint();
        }

        let mut clicked: Option<&'static str> = None;
        let mut launcher_clicked = false;
        let mut menu_for: Option<(&'static str, f32)> = None;
        Area::new(Id::new(DOCK_LAYER)).order(Order::Tooltip).fixed_pos(dock.min).constrain(false).show(ctx, |ui| {
            ui.set_opacity(rise);
            let (_, _) = ui.allocate_exact_size(dock.size(), Sense::click());
            let p = ui.painter().clone();
            theme::shadow(&p, dock, RADIUS_CARD, 0, 80, 0.3);
            card(&p, dock, RADIUS_CARD, rgba(255, 255, 255, 0.18), lit(0.25));

            let mut x = dock.left() + pad.x;
            let y = dock.top() + pad.y;
            for (i, id) in std::iter::once("launcher").chain(ids.iter().copied()).enumerate() {
                let slot = Rect::from_min_size(Pos2::new(x, y), Vec2::splat(icon));
                let resp = ui.interact(slot, Id::new(("dock", id)), Sense::click());
                let hover = ui.ctx().animate_bool_with_time(resp.id, resp.hovered() && self.dock_menu.is_none() && screen.width() >= 768.0, 0.2);
                let s = 1.0 + 0.22 * ease_out(hover);
                let r = Rect::from_center_size(slot.center_bottom() - Vec2::new(0.0, icon * s / 2.0 + 4.0 * hover), Vec2::splat(icon * s));
                let corner = (14.0 * s * icon / 52.0) as u8;
                theme::shadow(&p, r, corner, 6, 16, 0.3);
                if id == "launcher" {
                    p.rect(r, CornerRadius::same(corner), lit(0.92), Stroke::new(1.0_f32, lit(0.3)), StrokeKind::Inside);
                    let mut child = ui.new_child(UiBuilder::new().max_rect(Rect::from_center_size(r.center(), Vec2::splat(24.0 * s))));
                    icons::launcher_dots(&mut child, 24.0 * s, ACCENT);
                } else if let Some(app) = wm::app(id) {
                    paint_app_icon(ui, r, corner, app.tint);
                    let mut child = ui.new_child(UiBuilder::new().max_rect(Rect::from_center_size(r.center(), Vec2::splat(26.0 * s * icon / 52.0))));
                    icons::line_icon(&mut child, id, icons::app_icon_paths(id), 26.0 * s * icon / 52.0, Color32::WHITE, 1.8);
                    if self.wm.get(id).is_some() {
                        p.circle_filled(Pos2::new(slot.center().x, slot.bottom() + 4.0), 2.0, lit(0.85));
                    }
                }
                if resp.hovered() {
                    ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
                    if self.dock_menu.is_none() && screen.width() >= 768.0 {
                        let name = if id == "launcher" { "Launcher" } else { wm::app(id).map_or("", |a| a.name) };
                        tip(&p, ui, Pos2::new(slot.center().x, r.top() - 12.0), name);
                    }
                }
                if resp.clicked() {
                    if id == "launcher" { launcher_clicked = true } else { clicked = Some(id) }
                }
                if resp.secondary_clicked() && id != "launcher" {
                    menu_for = Some((id, slot.center().x));
                }
                x += icon + gap;
                if i == 0 {
                    p.vline(x - gap / 2.0 + 0.5, (dock.top() + 4.0 + pad.y)..=(dock.bottom() - 4.0 - pad.y), Stroke::new(1.0_f32, lit(0.25)));
                    x += 1.0 + gap;
                }
            }
        });

        if launcher_clicked {
            self.launcher = if self.launcher.is_some() { None } else { Some(now) };
            self.launcher_q.clear();
        }
        if let Some(id) = clicked {
            // Left click: open → focus → minimise → restore, like a desktop dock.
            match self.wm.get(id) {
                None => self.open(id, area, now),
                Some(w) if w.minimized => {
                    self.open(id, area, now);
                }
                Some(_) if self.wm.active() == Some(id) => self.wm.minimize(id),
                Some(_) => self.wm.focus(id),
            }
            self.launcher = None;
        }
        if let Some(m) = menu_for {
            self.dock_menu = Some(m);
        }

        // Right-click menu on a dock icon
        if let Some((id, cx)) = self.dock_menu {
            let Some(app) = wm::app(id) else { self.dock_menu = None; return };
            let win = self.wm.get(id).cloned();
            let mut items: Vec<(&str, String, bool)> = vec![("title", app.name.to_string(), false), ("-", String::new(), false)];
            match &win {
                None => items.push(("open", "Open".into(), false)),
                Some(w) => {
                    if w.minimized {
                        items.push(("open", "Show".into(), false));
                    } else {
                        items.push(("front", "Bring to front".into(), false));
                        items.push(("min", "Minimise".into(), false));
                        items.push(("max", if w.maximized { "Restore size" } else { "Maximise" }.into(), false));
                    }
                    items.push(("-", String::new(), false));
                    items.push(("quit", "Quit".into(), true));
                }
            }
            let h = menu_height(&items);
            let pos = Pos2::new(cx - 90.0, dock.top() - 12.0 - h);
            let mut picked = None;
            let rect = menu_area(ctx, "dock-menu", pos, 180.0, &items, &mut picked);
            if let Some(action) = picked {
                self.dock_menu = None;
                match action {
                    "open" => self.open(id, area, now),
                    "front" => self.wm.focus(id),
                    "min" => self.wm.minimize(id),
                    "max" => self.wm.toggle_maximize(id, now),
                    "quit" => self.wm.close(id),
                    _ => {}
                }
            } else if clicked_outside(ctx, &[rect]) && menu_for.is_none() {
                self.dock_menu = None;
            }
        }
    }

    fn draw_launcher(&mut self, ctx: &egui::Context, screen: Rect, area: Rect, opened: f64, now: f64) {
        let age = (now - opened) as f32;
        let fade_in = (age / 0.2).clamp(0.0, 1.0);
        if age < 0.6 {
            ctx.request_repaint();
        }
        let mut launch: Option<&'static str> = None;
        let mut close = false;
        Area::new(Id::new("launcher")).order(Order::Foreground).fixed_pos(screen.min).show(ctx, |ui| {
            ui.set_opacity(fade_in);
            let (_, bg) = ui.allocate_exact_size(screen.size(), Sense::click());
            ui.painter().rect_filled(screen, 0.0, rgba(24, 8, 8, 0.72));
            if bg.clicked() {
                close = true;
            }
            let narrow = screen.width() < 768.0;
            let top = area.top() + if narrow { 24.0 } else { 48.0 };
            let sw = 320.0f32.min(screen.width() - 48.0);
            let mut child = ui.new_child(UiBuilder::new().max_rect(Rect::from_min_size(Pos2::new(screen.center().x - sw / 2.0, top), Vec2::new(sw, 40.0))));
            let resp = theme::text_input(&mut child, &mut self.launcher_q, "Search", false, sw, Id::new("launcher-search"));
            if age < 0.05 {
                resp.request_focus();
            }
            let q = self.launcher_q.trim().to_lowercase();
            let apps: Vec<&wm::AppInfo> = APPS.iter().filter(|a| a.name.to_lowercase().contains(&q)).collect();
            if resp.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)) {
                if let Some(a) = apps.first() {
                    launch = Some(a.id);
                }
            }

            let grid_w = 760.0f32.min(screen.width() - 48.0);
            let cols = if narrow { 3 } else { ((grid_w + 12.0) / (110.0 + 12.0)).floor().max(1.0) as usize };
            let cell_w = (grid_w - (cols as f32 - 1.0) * 12.0) / cols as f32;
            let icon = if narrow { 58.0 } else { 68.0 };
            let cell_h = icon + 10.0 + 18.0;
            let gx = screen.center().x - grid_w / 2.0;
            let gy = top + 40.0 + if narrow { 24.0 } else { 36.0 };
            if apps.is_empty() {
                ui.painter().text(Pos2::new(screen.center().x, gy + 10.0), Align2::CENTER_TOP, "No apps found", font(14.0, 400), lit(0.55));
            }
            for (i, a) in apps.iter().enumerate() {
                let (row, col) = (i / cols, i % cols);
                let appear = ease_out(((age - i as f32 * 0.03) / 0.35).clamp(0.0, 1.0));
                let cell = Rect::from_min_size(Pos2::new(gx + col as f32 * (cell_w + 12.0), gy + row as f32 * (cell_h + 28.0) + (1.0 - appear) * 12.0), Vec2::new(cell_w, cell_h));
                let r = ui.interact(cell, Id::new(("launch", a.id)), Sense::click());
                let hover = ui.ctx().animate_bool_with_time(r.id, r.hovered(), 0.2);
                let s = icon * (1.0 + 0.08 * hover);
                let ir = Rect::from_center_size(Pos2::new(cell.center().x, cell.top() + icon / 2.0), Vec2::splat(s));
                let p = ui.painter().clone();
                theme::shadow(&p, ir, 18, 6, 16, 0.3 * appear);
                paint_app_icon(ui, ir, 18, a.tint);
                let mut child = ui.new_child(UiBuilder::new().max_rect(Rect::from_center_size(ir.center(), Vec2::splat(34.0))));
                child.set_opacity(appear);
                icons::line_icon(&mut child, a.id, icons::app_icon_paths(a.id), 34.0, Color32::WHITE, 1.8);
                p.text(Pos2::new(cell.center().x, cell.top() + icon + 10.0), Align2::CENTER_TOP, a.name, font(13.0, 400), fade(Color32::WHITE, appear));
                if r.hovered() {
                    ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
                }
                if r.clicked() {
                    launch = Some(a.id);
                }
            }
        });
        if let Some(id) = launch {
            self.open(id, area, now);
            self.launcher = None;
        } else if close {
            self.launcher = None;
        }
    }

    fn draw_confirm(&mut self, ctx: &egui::Context, screen: Rect, now: f64) -> Option<ConfirmAction> {
        let c = self.confirm.as_ref()?;
        let mut result: Option<bool> = None;
        let age = (now - c.at) as f32;
        if age < 0.4 {
            ctx.request_repaint();
        }
        Area::new(Id::new("confirm")).order(Order::Debug).fixed_pos(screen.min).show(ctx, |ui| {
            let (_, bg) = ui.allocate_exact_size(screen.size(), Sense::click());
            ui.painter().rect_filled(screen, 0.0, rgba(0, 0, 0, 0.55 * (age / 0.2).clamp(0.0, 1.0)));
            if bg.clicked() {
                result = Some(false);
            }
            if let Some(confirmed) = confirm_card(ui, screen, &c.title, &c.message, &c.label, age) {
                result = Some(confirmed);
            }
        });
        match result {
            Some(true) => {
                let action = c.action;
                self.confirm = None;
                Some(action)
            }
            Some(false) => {
                self.confirm = None;
                None
            }
            None => None,
        }
    }

    fn draw_update_popup(&mut self, ctx: &egui::Context, core: &mut Core, screen: Rect, area: Rect, now: f64) {
        use crate::core::UpdateRequest;
        use crate::update::{UpdateStatus, CURRENT};
        let Some(release) = core.update.release().cloned() else {
            self.update_popup_at = None;
            return;
        };
        if core.update_dismissed.as_deref() == Some(release.version.as_str()) || self.confirm.is_some() {
            return;
        }
        let at = *self.update_popup_at.get_or_insert(now);
        let age = (now - at) as f32;
        if age < 0.4 {
            ctx.request_repaint();
        }
        let mut later = ctx.input(|i| i.key_pressed(Key::Escape));
        let mut install = false;
        let mut details = false;
        Area::new(Id::new("update-popup")).order(Order::Debug).fixed_pos(screen.min).show(ctx, |ui| {
            let (_, _) = ui.allocate_exact_size(screen.size(), Sense::click());
            ui.painter().rect_filled(screen, 0.0, rgba(0, 0, 0, 0.45 * (age / 0.2).clamp(0.0, 1.0)));
            let pop = theme::ease_back((age / 0.35).clamp(0.0, 1.0));
            let alpha = (age / 0.2).clamp(0.0, 1.0);
            ui.set_opacity(alpha);
            let w = 380.0f32.min(screen.width() - 40.0);
            let notes = super::apps::settings::clean_notes(&release.notes);
            let notes_g = ui.fonts(|f| f.layout(notes.clone(), font(13.0, 400), Color32::PLACEHOLDER, w - 48.0 - 20.0));
            let notes_h = if notes.is_empty() { 0.0 } else { notes_g.size().y.min(150.0) + 20.0 + 30.0 };
            let h = 24.0 + 56.0 + 26.0 + 22.0 + 40.0 + notes_h + 50.0 + 40.0;
            let rect = Rect::from_center_size(screen.center() + Vec2::new(0.0, (1.0 - pop) * 12.0), Vec2::new(w, h));
            let p = ui.painter().clone();
            theme::shadow(&p, rect, 20, 0, 80, 0.35);
            card(&p, rect, 20, menu_bg(), white(0.18));
            let mut cu = ui.new_child(UiBuilder::new().max_rect(rect.shrink(24.0)).layout(egui::Layout::top_down(egui::Align::Center)));
            cu.spacing_mut().item_spacing.y = 6.0;
            // Badge
            let (br, _) = cu.allocate_exact_size(Vec2::splat(52.0), Sense::hover());
            p.circle_filled(br.center(), 26.0, theme::rgb([0x17, 0x4f, 0x84]));
            p.circle_stroke(br.center(), 26.0, Stroke::new(1.0_f32, white(0.3)));
            let mut ic = cu.new_child(UiBuilder::new().max_rect(Rect::from_center_size(br.center(), Vec2::splat(28.0))));
            icons::line_icon(&mut ic, "update", icons::UPDATE, 28.0, WHITE, 1.8);
            cu.label(egui::RichText::new("New update is ready to install").font(display(18.0, 700)));
            cu.label(egui::RichText::new(format!("Melbourne Fire Brigade {}  ·  you have {CURRENT}", release.version)).font(font(13.0, 400)).color(white(0.65)));
            if !notes.is_empty() {
                cu.add_space(6.0);
                egui::Frame::NONE.fill(white(0.06)).corner_radius(CornerRadius::same(12)).inner_margin(egui::Margin::same(10)).show(&mut cu, |ui| {
                    ui.set_width(w - 48.0 - 20.0);
                    ui.with_layout(egui::Layout::top_down(egui::Align::Min), |ui| {
                        ui.label(egui::RichText::new("What's new").font(font(12.0, 600)).color(white(0.6)));
                        egui::ScrollArea::vertical().id_salt("update-notes").max_height(150.0).show(ui, |ui| {
                            ui.add(egui::Label::new(egui::RichText::new(&notes).font(font(13.0, 400))).wrap());
                        });
                    });
                });
            }
            cu.add_space(4.0);
            cu.add(egui::Label::new(egui::RichText::new("The app restarts to finish installing. You'll log in again; the Time Clock keeps running.").font(font(12.0, 400)).color(white(0.5))).wrap());
            cu.add_space(6.0);
            let bw = (w - 48.0 - 10.0) / 2.0;
            match &core.update {
                UpdateStatus::Downloading(_, frac) => {
                    cu.label(egui::RichText::new(format!("Downloading… {}%", (frac * 100.0).round())).font(font(13.0, 500)));
                    let (r, _) = cu.allocate_exact_size(Vec2::new(w - 48.0, 6.0), Sense::hover());
                    p.rect_filled(r, 3.0, white(0.18));
                    p.rect_filled(Rect::from_min_size(r.min, Vec2::new(r.width() * frac, 6.0)), 3.0, select_blue());
                }
                UpdateStatus::Available(_) => {
                    cu.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 10.0;
                        if theme::button(ui, "Later", Btn::Subtle, 14.0, Some(bw), Vec2::new(10.0, 10.0)).clicked() {
                            later = true;
                        }
                        if theme::button(ui, "Install now", Btn::Primary, 14.0, Some(bw), Vec2::new(10.0, 10.0)).clicked() {
                            install = true;
                        }
                    });
                    let g = cu.fonts(|f| f.layout_no_wrap("Details in Settings".into(), font(12.0, 400), ice_text()));
                    let (lr, lresp) = cu.allocate_exact_size(g.size(), Sense::click());
                    if lresp.hovered() {
                        cu.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                        p.hline(lr.x_range(), lr.bottom(), Stroke::new(1.0_f32, ice_text()));
                    }
                    p.galley(lr.min, g, ice_text());
                    details = lresp.clicked();
                }
                _ => {
                    cu.label(egui::RichText::new("Installing…").font(font(13.0, 500)));
                }
            }
        });
        if install {
            core.update_request = Some(UpdateRequest::Install);
        } else if (later || details) && matches!(core.update, UpdateStatus::Available(_)) {
            core.update_dismissed = Some(release.version.clone());
            if details {
                self.apps.settings.go(super::apps::settings::UPDATE_SECTION);
                self.open("settings", area, now);
            }
        }
    }

    fn draw_context_menu(&mut self, ctx: &egui::Context, area: Rect, now: f64) {
        // Right-clicking a text box opens Cut / Copy / Paste / Select All.
        if ctx.input(|i| i.pointer.secondary_clicked()) {
            if let Some(id) = ctx.memory(|m| m.focused()) {
                if egui::TextEdit::load_state(ctx, id).is_some() {
                    if let Some(pos) = ctx.input(|i| i.pointer.interact_pos()) {
                        if ctx.read_response(id).is_some_and(|r| r.rect.contains(pos)) {
                            self.ctx_menu = Some(CtxMenu { pos, kind: CtxKind::Text(id) });
                        }
                    }
                }
            }
        }
        let Some(menu) = &self.ctx_menu else { return };
        let pos = menu.pos;
        let items: Vec<(&str, String, bool)> = match &menu.kind {
            CtxKind::Desktop => vec![("launcher", "Open Launcher".into(), false), ("-", String::new(), false), ("settings", "Settings…".into(), false), ("about", "About Melbourne Fire Brigade".into(), false)],
            CtxKind::Text(_) => vec![("cut", "Cut".into(), false), ("copy", "Copy".into(), false), ("paste", "Paste".into(), false), ("-", String::new(), false), ("selectall", "Select All".into(), false)],
        };
        let h = menu_height(&items);
        let screen = ctx.screen_rect();
        let pos = Pos2::new(pos.x.min(screen.right() - 220.0 - 6.0).max(6.0), pos.y.min(screen.bottom() - h - 6.0).max(6.0));
        let mut picked = None;
        let rect = menu_area(ctx, "ctx-menu", pos, 210.0, &items, &mut picked);
        let kind = self.ctx_menu.as_ref().map_or(CtxKind::Desktop, |m| m.kind);
        if let Some(action) = picked {
            self.ctx_menu = None;
            match (&kind, action) {
                (CtxKind::Desktop, "launcher") => self.launcher = Some(now),
                (CtxKind::Desktop, id @ ("settings" | "about")) => {
                    let id = if id == "settings" { "settings" } else { "about" };
                    self.open(id, area, now);
                }
                (CtxKind::Text(id), cmd) => self.apps.text_command(ctx, *id, cmd),
                _ => {}
            }
        } else if clicked_outside(ctx, &[rect]) && !ctx.input(|i| i.pointer.secondary_clicked()) {
            self.ctx_menu = None;
        }
    }
}

#[derive(Default)]
struct WinActions {
    close: bool,
    minimize: bool,
    maximize: bool,
    moved: Option<Vec2>,
    resized: Option<Vec2>,
    open: Option<&'static str>,
}

/// An app icon tile: the tint fading to a lighter shade at the top.
pub fn paint_app_icon(ui: &Ui, r: Rect, corner: u8, tint: [u8; 3]) {
    let light = theme::mix(tint, 0.7, [255, 255, 255], 1.0);
    let base = theme::rgb(tint);
    let p = ui.painter();
    // A vertical gradient clipped to the rounded tile, drawn as thin slices.
    let mut mesh = egui::epaint::Mesh::default();
    let steps = 12;
    let rad = corner as f32;
    for j in 0..=steps {
        let t = j as f32 / steps as f32;
        let y = r.top() + r.height() * t;
        let inset = if y < r.top() + rad {
            rad - (rad * rad - (r.top() + rad - y).powi(2)).max(0.0).sqrt()
        } else if y > r.bottom() - rad {
            rad - (rad * rad - (y - (r.bottom() - rad)).powi(2)).max(0.0).sqrt()
        } else {
            0.0
        };
        let c = lerp_color(light, base, (t * 1.1).min(1.0));
        mesh.colored_vertex(Pos2::new(r.left() + inset, y), c);
        mesh.colored_vertex(Pos2::new(r.right() - inset, y), c);
        if j > 0 {
            let a = (j as u32 - 1) * 2;
            mesh.add_triangle(a, a + 1, a + 2);
            mesh.add_triangle(a + 1, a + 3, a + 2);
        }
    }
    p.add(mesh);
    p.rect_stroke(r, CornerRadius::same(corner), Stroke::new(1.0_f32, lit(0.3)), StrokeKind::Inside);
}

fn tip(p: &egui::Painter, ui: &Ui, bottom_center: Pos2, text: &str) {
    let f = font(12.0, 400);
    let g = ui.fonts(|fo| fo.layout_no_wrap(text.to_string(), f, Color32::WHITE));
    let r = Rect::from_center_size(bottom_center - Vec2::new(0.0, (g.size().y + 8.0) / 2.0), g.size() + Vec2::new(20.0, 8.0));
    card(p, r, 8, rgba(46, 12, 12, 0.88), lit(0.18));
    p.galley(r.center() - g.size() / 2.0, g, Color32::WHITE);
}

fn menu_height(items: &[(&str, String, bool)]) -> f32 {
    12.0 + items.iter().map(|(id, _, _)| if *id == "-" { 11.0 } else { 31.0 }).sum::<f32>()
}

/// A dark glass menu. Returns its rect; sets `picked` to the chosen item's id.
fn menu_area(ctx: &egui::Context, layer: &str, pos: Pos2, width: f32, items: &[(&str, String, bool)], picked: &mut Option<&'static str>) -> Rect {
    let h = menu_height(items);
    let rect = Rect::from_min_size(pos, Vec2::new(width, h));
    Area::new(Id::new(layer)).order(Order::Debug).fixed_pos(pos).constrain(false).show(ctx, |ui| {
        let (_, _) = ui.allocate_exact_size(rect.size(), Sense::click());
        let p = ui.painter().clone();
        theme::shadow(&p, rect, 12, 16, 40, 0.4);
        card(&p, rect, 12, menu_bg(), white(0.18));
        let mut y = rect.top() + 6.0;
        for (id, label, danger) in items {
            if *id == "-" {
                p.hline((rect.left() + 10.0)..=(rect.right() - 10.0), y + 5.5, Stroke::new(1.0_f32, white(0.1)));
                y += 11.0;
                continue;
            }
            let row = Rect::from_min_size(Pos2::new(rect.left() + 6.0, y), Vec2::new(width - 12.0, 31.0));
            if *id == "title" {
                p.text(Pos2::new(row.left() + 10.0, row.center().y), Align2::LEFT_CENTER, label, font(13.0, 600), white(0.55));
            } else {
                let r = ui.interact(row, Id::new((layer, *id)), Sense::click());
                if r.hovered() {
                    p.rect_filled(row, 7.0, if *danger { rgba(239, 68, 68, 0.15) } else { white(0.12) });
                    ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
                }
                p.text(Pos2::new(row.left() + 10.0, row.center().y), Align2::LEFT_CENTER, label, font(13.0, 400), if *danger { red_text() } else { ink() });
                if r.clicked() {
                    *picked = Some(leak(id));
                }
            }
            y += 31.0;
        }
    });
    rect
}

fn leak(s: &str) -> &'static str {
    // Menu ids are a small fixed set; map them back to static strings.
    const IDS: [&str; 20] = ["about", "settings", "restart", "shutdown", "logout", "open", "front", "min", "max", "quit", "launcher", "cut", "copy", "paste", "selectall", "title", "-", "update", "", ""];
    IDS.iter().find(|i| **i == s).copied().unwrap_or("")
}

/// True when the pointer was pressed this frame outside every given rect.
fn clicked_outside(ctx: &egui::Context, rects: &[Rect]) -> bool {
    ctx.input(|i| {
        i.pointer.any_pressed() && i.pointer.interact_pos().is_some_and(|p| !rects.iter().any(|r| r.contains(p)))
    })
}

/// The card part of the "Are you sure?" dialog. Some(true) = confirmed.
pub fn confirm_card(ui: &mut Ui, screen: Rect, title: &str, message: &str, label: &str, age: f32) -> Option<bool> {
    let mut result = None;
    let w = 340.0f32.min(screen.width() - 40.0);
    let msg = ui.fonts(|f| f.layout(message.to_string(), font(13.0, 400), Color32::PLACEHOLDER, w - 48.0));
    let h = 24.0 + 24.0 + 8.0 + msg.size().y + 20.0 + 40.0 + 24.0;
    let pop = theme::ease_back((age / 0.35).clamp(0.0, 1.0));
    let alpha = (age / 0.2).clamp(0.0, 1.0);
    let rect = Rect::from_center_size(screen.center() + Vec2::new(0.0, (1.0 - pop) * 12.0), Vec2::new(w, h) * (0.9 + 0.1 * pop));
    let p = ui.painter().clone();
    theme::shadow(&p, rect, 20, 0, 80, 0.3 * alpha);
    card(&p, rect, 20, fade(menu_bg(), alpha), fade(white(0.18), alpha));
    p.text(Pos2::new(rect.center().x, rect.top() + 24.0), Align2::CENTER_TOP, title, display(18.0, 700), fade(ink(), alpha));
    let mpos = Pos2::new(rect.center().x - msg.size().x / 2.0, rect.top() + 56.0);
    let msg_h = msg.size().y;
    p.galley(mpos, msg, fade(white(0.65), alpha));
    let by = rect.top() + 56.0 + msg_h + 20.0;
    let bw = (rect.width() - 48.0 - 10.0) / 2.0;
    let mut child = ui.new_child(UiBuilder::new().max_rect(Rect::from_min_size(Pos2::new(rect.left() + 24.0, by), Vec2::new(rect.width() - 48.0, 40.0))).layout(egui::Layout::left_to_right(egui::Align::Center)));
    child.spacing_mut().item_spacing.x = 10.0;
    child.set_opacity(alpha);
    if theme::button(&mut child, "Cancel", Btn::Subtle, 14.0, Some(bw), Vec2::new(10.0, 10.0)).clicked() {
        result = Some(false);
    }
    if theme::button(&mut child, label, Btn::Danger, 14.0, Some(bw), Vec2::new(10.0, 10.0)).clicked() {
        result = Some(true);
    }
    if ui.input(|i| i.key_pressed(Key::Enter)) {
        result = Some(true);
    }
    result
}
