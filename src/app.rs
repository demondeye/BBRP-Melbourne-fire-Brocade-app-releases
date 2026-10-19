//! Boot → login → desktop. Shut Down (or closing the app) is like switching off:
//! next time it always begins at boot and nobody stays logged in.

use crate::auth;
use crate::core::{lock, Shared};
use crate::settings::UserSettings;
use crate::ui::desktop::{Desktop, DesktopOut, Power};
use crate::ui::icons;
use crate::ui::theme::{self, *};
use eframe::egui::{self, Align2, CornerRadius, Id, Key, Pos2, Rect, Sense, Stroke, StrokeKind, UiBuilder, Vec2};

/// How long the boot screen lasts, in seconds. It cannot be skipped.
const BOOT_SECS_NORMAL: f64 = 20.0;
/// How long the "logging in" loading step lasts before the desktop appears.
const LOGIN_SECS_NORMAL: f64 = 4.0;

/// For testing only: MFB_FAST=1 shortens the boot and login waits.
fn fast() -> bool {
    std::env::var("MFB_FAST").is_ok()
}
fn boot_secs() -> f64 {
    if fast() { 1.0 } else { BOOT_SECS_NORMAL }
}
fn login_secs() -> f64 {
    if fast() { 0.4 } else { LOGIN_SECS_NORMAL }
}
/// Restart / shut down: the screen fades to black with a spinner for this long.
const POWER_DOWN_SECS: f64 = 4.0;
/// How long the "Installing update" screen shows before the app restarts.
const INSTALL_SECS: f64 = 3.0;

enum Stage {
    /// Booting since this time.
    Boot(f64),
    Login(LoginForm),
    Desktop(Box<Desktop>),
}

#[derive(Default)]
struct LoginForm {
    account: Option<auth::Account>,
    settings: Option<UserSettings>,
    name: String,
    password: String,
    confirm: String,
    error: String,
    shake_at: Option<f64>,
    resetting: bool,
    loading: Option<(f64, String)>,
    opened_at: f64,
}

pub struct App {
    core: Shared,
    stage: Stage,
    power_down: Option<(f64, Power)>,
    /// Installing an update: when it started, the version and the downloaded file.
    installing: Option<(f64, String, std::path::PathBuf)>,
    install_done: bool,
    /// Frames drawn so far (used to make sure the window opens maximised).
    frames: u32,
    max_check_at: u32,
    max_tries: u32,
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>, core: Shared) -> Self {
        egui_extras::install_image_loaders(&cc.egui_ctx);
        theme::install_fonts(&cc.egui_ctx);
        theme::install_style(&cc.egui_ctx);
        crate::relay::start(core.clone(), cc.egui_ctx.clone());
        crate::update::cleanup();
        crate::update::start_background_checks(core.clone(), cc.egui_ctx.clone());
        // A desktop app may always play sound, so it starts straight away.
        crate::chime::play();
        let now = cc.egui_ctx.input(|i| i.time);
        Self { core, stage: Stage::Boot(now), power_down: None, installing: None, install_done: false, frames: 0, max_check_at: u32::MAX, max_tries: 0 }
    }

    fn to_login(&mut self, now: f64) {
        // (The login screen sits on the wallpaper; reset the window look for next time.)
        let account = auth::get_account();
        let settings = account.as_ref().map(|a| UserSettings::load(&a.name));
        self.stage = Stage::Login(LoginForm { account, settings, opened_at: now, ..Default::default() });
    }
}

impl eframe::App for App {
    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        [0.02, 0.05, 0.09, 1.0]
    }

    fn raw_input_hook(&mut self, ctx: &egui::Context, raw_input: &mut egui::RawInput) {
        // Cut / Copy / Paste chosen from the right-click menu are replayed as key presses.
        if let Stage::Desktop(d) = &mut self.stage {
            // Remember the selection before a right-click moves the cursor.
            let right_press = raw_input.events.iter().any(|e| matches!(e, egui::Event::PointerButton { button: egui::PointerButton::Secondary, pressed: true, .. }));
            if right_press {
                if let Some(id) = ctx.memory(|m| m.focused()) {
                    if let Some(range) = egui::TextEdit::load_state(ctx, id).and_then(|s| s.cursor.char_range()) {
                        d.apps.saved_selection = Some((id, range));
                    }
                }
            }
            if let Some((id, events)) = d.apps.pending_input.take() {
                ctx.memory_mut(|m| m.request_focus(id));
                if let Some((sid, range)) = d.apps.saved_selection.take() {
                    if sid == id {
                        if let Some(mut state) = egui::TextEdit::load_state(ctx, id) {
                            state.cursor.set_char_range(Some(range));
                            state.store(ctx, id);
                        }
                    }
                }
                raw_input.events.extend(events);
            }
        }
    }

    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        let now = ctx.input(|i| i.time);
        // Open the way this PC is set (Settings → Appearance → Window): the mode and
        // the screen. Once the window is on screen it's placed exactly (display.rs);
        // a moment later the real size is checked, and if it still doesn't fill the
        // screen it's placed again (Windows sometimes ignores the first attempt).
        let mode = crate::display::mode();
        if std::env::var("MFB_WINDOWED").is_err() {
            if self.frames == 1 {
                crate::display::request(mode);
                self.max_check_at = 20;
            } else if self.frames == self.max_check_at && self.max_tries < 3 && !crate::display::busy() {
                let (inner, monitor) = ctx.input(|i| (i.viewport().inner_rect, i.viewport().monitor_size));
                let fills = match (inner, monitor) {
                    (Some(r), Some(m)) => r.width() >= m.x * 0.9 && r.height() >= m.y * 0.8,
                    _ => true,
                };
                if !fills {
                    self.max_tries += 1;
                    crate::display::request(mode);
                    self.max_check_at = self.frames + 20;
                }
            }
            if self.frames < 150 {
                ctx.request_repaint();
            }
        }
        crate::display::tick(ctx, frame);
        self.frames = self.frames.saturating_add(1);
        // F11 flips fullscreen on and off for now (the saved mode isn't changed).
        if ctx.input(|i| i.key_pressed(Key::F11)) {
            let full = ctx.input(|i| i.viewport().fullscreen.unwrap_or(false));
            if full {
                let back = if mode == crate::display::Mode::Fullscreen { crate::display::Mode::Windowed } else { mode };
                crate::display::request(back);
            } else {
                crate::display::request(crate::display::Mode::Fullscreen);
            }
        }
        // Keep clocks ticking even when nothing moves.
        ctx.request_repaint_after(std::time::Duration::from_millis(500));

        match &mut self.stage {
            Stage::Boot(at) => {
                let at = *at;
                egui::CentralPanel::default().frame(egui::Frame::NONE.fill(egui::Color32::from_rgb(20, 6, 7))).show(ctx, |ui| {
                    let screen = ui.max_rect();
                    let age = now - at;
                    let fade_in = ((age / 0.6).min(1.0)) as f32;
                    let c = screen.center();
                    let logo = Rect::from_center_size(c - Vec2::new(0.0, 24.0), Vec2::splat(88.0));
                    ui.set_opacity(((age / 0.4).min(1.0)) as f32);
                    icons::logo_at(ui, logo);
                    let bar = Rect::from_center_size(Pos2::new(c.x, logo.bottom() + 44.0 + 2.5), Vec2::new(190.0f32.min(screen.width() * 0.6), 5.0));
                    ui.painter().rect_filled(bar, 3.0, lit(0.18 * fade_in));
                    let t = (age / (boot_secs() - 0.6)).clamp(0.0, 1.0) as f32;
                    let frac = boot_progress(t);
                    if frac > 0.0 {
                        ui.painter().rect_filled(Rect::from_min_size(bar.min, Vec2::new(bar.width() * frac, 5.0)), 3.0, fade(WHITE, fade_in));
                    }
                    ui.ctx().request_repaint();
                });
                if now - at >= boot_secs() {
                    self.to_login(now);
                }
            }
            Stage::Login(form) => {
                if let Some(user) = login_screen(ctx, form, now) {
                    let mut core = lock(&self.core);
                    core.sign_in(&user);
                    drop(core);
                    self.stage = Stage::Desktop(Box::new(Desktop::new(now)));
                }
            }
            Stage::Desktop(desktop) => {
                let mut core = lock(&self.core);
                let out = desktop.show(ctx, &mut core, now);
                drop(core);
                match out {
                    DesktopOut::None => {}
                    DesktopOut::Logout => {
                        desktop.wm.close_all();
                        lock(&self.core).sign_out();
                        self.to_login(now);
                    }
                    DesktopOut::Power(p) => {
                        if self.power_down.is_none() {
                            self.power_down = Some((now, p));
                        }
                    }
                }
            }
        }

        // Restart / shut down: fade to black, then log out and boot again (Restart) or close the app (Shut Down).
        if let Some((at, kind)) = self.power_down {
            let age = now - at;
            egui::Area::new(Id::new("powerdown")).order(egui::Order::Debug).fixed_pos(Pos2::ZERO).show(ctx, |ui| {
                let screen = ctx.screen_rect();
                let (_, _) = ui.allocate_exact_size(screen.size(), Sense::click_and_drag());
                ui.painter().rect_filled(screen, 0.0, rgba(0, 0, 0, (age / 1.2).min(1.0) as f32));
                let a = ((age - 1.2) / 0.6).clamp(0.0, 1.0) as f32;
                if a > 0.0 {
                    let c = screen.center();
                    ui.painter().circle_stroke(c, 13.5, Stroke::new(3.0_f32, lit(0.18 * a)));
                    let start = (now * std::f64::consts::TAU / 0.9) as f32;
                    let pts: Vec<Pos2> = (0..=16).map(|i| c + Vec2::angled(start + i as f32 / 16.0 * std::f32::consts::FRAC_PI_2) * 13.5).collect();
                    ui.painter().add(egui::Shape::line(pts, Stroke::new(3.0_f32, lit(0.85 * a))));
                }
            });
            ctx.move_to_top(egui::LayerId::new(egui::Order::Debug, Id::new("powerdown")));
            ctx.request_repaint();
            if age >= POWER_DOWN_SECS {
                self.power_down = None;
                if let Stage::Desktop(d) = &mut self.stage {
                    d.wm.close_all();
                }
                lock(&self.core).sign_out();
                if kind == Power::Restart {
                    self.stage = Stage::Boot(now);
                    crate::chime::play();
                } else {
                    // Shut Down: this "computer" turns off, so the app closes.
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
            }
        }

        // Update buttons pressed this frame (done here, once the screen has let go of the state).
        let request = lock(&self.core).update_request.take();
        match request {
            Some(crate::core::UpdateRequest::Check) => crate::update::check(&self.core, ctx, true),
            Some(crate::core::UpdateRequest::Install) => crate::update::download(&self.core, ctx),
            None => {}
        }

        // A downloaded update starts installing straight away, like a system update.
        if self.installing.is_none() {
            let mut core = lock(&self.core);
            if let crate::update::UpdateStatus::Ready(r, path) = core.update.clone() {
                core.update = crate::update::UpdateStatus::Installing(r.clone());
                self.installing = Some((now, r.version, path));
            }
        }
        if let Some((at, version, path)) = self.installing.clone() {
            let age = now - at;
            egui::Area::new(Id::new("installing")).order(egui::Order::Debug).fixed_pos(Pos2::ZERO).show(ctx, |ui| {
                let screen = ctx.screen_rect();
                let (_, _) = ui.allocate_exact_size(screen.size(), Sense::click_and_drag());
                let a = (age / 0.8).min(1.0) as f32;
                ui.painter().rect_filled(screen, 0.0, fade(egui::Color32::from_rgb(20, 6, 7), a));
                let c = screen.center();
                ui.set_opacity(((age - 0.4) / 0.6).clamp(0.0, 1.0) as f32);
                let logo = Rect::from_center_size(c - Vec2::new(0.0, 50.0), Vec2::splat(72.0));
                icons::logo_at(ui, logo);
                let p = ui.painter();
                p.text(Pos2::new(c.x, logo.bottom() + 26.0), Align2::CENTER_TOP, "Installing update", display(20.0, 700), WHITE);
                p.text(Pos2::new(c.x, logo.bottom() + 56.0), Align2::CENTER_TOP, format!("Melbourne Fire Brigade {version} · the app will restart"), font(13.0, 400), lit(0.6));
                let bar = Rect::from_center_size(Pos2::new(c.x, logo.bottom() + 98.0), Vec2::new(190.0, 5.0));
                p.rect_filled(bar, 3.0, lit(0.18));
                let t = ((age - 0.4) / (INSTALL_SECS - 0.4)).clamp(0.0, 1.0) as f32;
                p.rect_filled(Rect::from_min_size(bar.min, Vec2::new(bar.width() * boot_progress(t), 5.0)), 3.0, WHITE);
            });
            ctx.request_repaint();
            if age >= INSTALL_SECS && !self.install_done {
                match crate::update::install_and_relaunch(&path) {
                    Ok(()) => {
                        if let Stage::Desktop(d) = &mut self.stage {
                            d.wm.persist();
                        }
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                        // Stop here; the new version is starting.
                        self.install_done = true;
                    }
                    Err(e) => {
                        lock(&self.core).update = crate::update::UpdateStatus::Failed(e);
                        self.installing = None;
                    }
                }
            }
        }
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        if let Stage::Desktop(d) = &mut self.stage {
            d.wm.persist();
        }
    }
}

/// The boot bar's uneven pace (same keyframes as the web version).
fn boot_progress(t: f32) -> f32 {
    const KEYS: [(f32, f32); 6] = [(0.0, 0.0), (0.2, 0.18), (0.45, 0.40), (0.55, 0.44), (0.8, 0.82), (1.0, 1.0)];
    for w in KEYS.windows(2) {
        let ((t0, v0), (t1, v1)) = (w[0], w[1]);
        if t <= t1 {
            return v0 + (v1 - v0) * ((t - t0) / (t1 - t0));
        }
    }
    1.0
}

/// Returns the user's name once they've logged in (after the loading step).
fn login_screen(ctx: &egui::Context, f: &mut LoginForm, now: f64) -> Option<String> {
    let mut entered: Option<String> = None;
    if let Some((at, name)) = &f.loading {
        if now - at >= login_secs() {
            return Some(name.clone());
        }
        ctx.request_repaint();
    }
    egui::CentralPanel::default().frame(egui::Frame::NONE).show(ctx, |ui| {
        let screen = ui.max_rect();
        let settings = f.settings.clone().unwrap_or_default();
        crate::ui::wallpaper::paint(ui, ui.painter(), screen, &settings, true);
        let appear = ((now - f.opened_at) / 0.6).clamp(0.0, 1.0) as f32;
        ui.painter().rect_filled(screen, 0.0, rgba(24, 8, 8, 0.5 * appear));
        ui.set_opacity(appear);
        if appear < 1.0 {
            ui.ctx().request_repaint();
        }

        // Clock
        let local = chrono::Local::now();
        let big = if screen.height() <= 560.0 { 40.0 } else if screen.width() < 768.0 { 56.0 } else if screen.width() >= 1920.0 { 104.0 } else { 76.0 };
        let top = screen.top() + if screen.height() <= 560.0 { 16.0 } else { screen.height() * 0.07 };
        let p = ui.painter().clone();
        let d = p.text(Pos2::new(screen.center().x, top), Align2::CENTER_TOP, crate::ui::format::long_date(local), font(18.0, 500), lit(0.85));
        p.text(Pos2::new(screen.center().x, d.bottom() + 2.0), Align2::CENTER_TOP, crate::ui::format::time(local), display(big, 700), lit(0.85));

        // Card
        let card_w = if screen.width() >= 1920.0 { 300.0 } else { 260.0f32.min(screen.width() - 40.0) };
        let shake = f.shake_at.map_or(0.0, |at| {
            let t = ((now - at) / 0.42) as f32;
            if t >= 1.0 { 0.0 } else { ui.ctx().request_repaint(); 9.0 * (t * std::f32::consts::TAU * 2.0).sin() * (1.0 - t) }
        });
        let card_h = 360.0;
        let card = Rect::from_center_size(screen.center() + Vec2::new(shake, 20.0), Vec2::new(card_w, card_h));
        let mut cu = ui.new_child(UiBuilder::new().max_rect(card).layout(egui::Layout::top_down(egui::Align::Center)));
        cu.spacing_mut().item_spacing.y = 10.0;

        let display_name = match &f.account {
            Some(a) => a.name.clone(),
            None => f.name.trim().to_string(),
        };
        let (ar, _) = cu.allocate_exact_size(Vec2::splat(84.0), Sense::hover());
        theme::shadow(&p, ar, 42, 10, 30, 0.3);
        p.circle_filled(ar.center(), 42.0, WHITE);
        let initial = display_name.chars().next().map(|c| c.to_uppercase().to_string()).unwrap_or_else(|| "+".into());
        p.text(ar.center(), Align2::CENTER_CENTER, initial, display(34.0, 800), ACCENT);
        let title = if f.loading.is_some() { display_name.clone() } else if let Some(a) = &f.account { a.name.clone() } else { "Create local account".into() };
        cu.add_space(2.0);
        cu.label(egui::RichText::new(title).font(font(18.0, 600)));

        if let Some((at, _)) = &f.loading {
            cu.add_space(14.0);
            let (bar, _) = cu.allocate_exact_size(Vec2::new(190.0, 5.0), Sense::hover());
            p.rect_filled(bar, 3.0, lit(0.18));
            let t = ((now - at) / (login_secs() - 0.3)).clamp(0.0, 1.0) as f32;
            p.rect_filled(Rect::from_min_size(bar.min, Vec2::new(bar.width() * boot_progress(t), 5.0)), 3.0, WHITE);
        } else {
            let mut submit = false;
            let w = card_w;
            if f.account.is_none() {
                let r = theme::text_input(&mut cu, &mut f.name, "Name", false, w, Id::new("login-name"));
                if now - f.opened_at < 0.1 {
                    r.request_focus();
                }
            }
            // Password row (with the → button when logging in)
            let pr = theme::text_input(&mut cu, &mut f.password, if f.account.is_some() { "Enter password" } else { "Password" }, true, w, Id::new("login-pass"));
            if f.account.is_some() {
                if now - f.opened_at < 0.1 {
                    pr.request_focus();
                }
                let go = Rect::from_center_size(Pos2::new(pr.rect.right() + 16.0 - 5.0 - 14.0, pr.rect.center().y), Vec2::splat(28.0));
                let gr = cu.interact(go, Id::new("login-go"), Sense::click());
                p.circle_filled(go.center(), 14.0, lit(if gr.hovered() { 0.4 } else { 0.25 }));
                let mut child = cu.new_child(UiBuilder::new().max_rect(Rect::from_center_size(go.center(), Vec2::splat(16.0))));
                icons::line_icon(&mut child, "go", icons::ARROW, 16.0, WHITE, 2.4);
                if gr.hovered() {
                    cu.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                }
                submit |= gr.clicked();
            }
            submit |= pr.lost_focus() && cu.input(|i| i.key_pressed(Key::Enter));
            if f.account.is_none() {
                let cr = theme::text_input(&mut cu, &mut f.confirm, "Confirm password", true, w, Id::new("login-confirm"));
                submit |= cr.lost_focus() && cu.input(|i| i.key_pressed(Key::Enter));
                submit |= theme::button(&mut cu, "Create account", Btn::Primary, 14.0, Some(w), Vec2::new(10.0, 10.0)).clicked();
            }
            // Error line (always takes its space so nothing jumps)
            let (er, _) = cu.allocate_exact_size(Vec2::new(w, 18.0), Sense::hover());
            p.text(er.center(), Align2::CENTER_CENTER, &f.error, font(13.0, 400), RED_TEXT);

            if f.account.is_some() && !f.resetting {
                let g = cu.fonts(|fo| fo.layout_no_wrap("Forgot password?".into(), font(12.0, 400), WHITE));
                let (lr, lresp) = cu.allocate_exact_size(g.size(), Sense::click());
                p.galley(lr.min, g, if lresp.hovered() { WHITE } else { lit(0.6) });
                if lresp.hovered() {
                    cu.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                }
                if lresp.clicked() {
                    f.resetting = true;
                }
            }
            if f.resetting {
                egui::Frame::NONE.fill(rgba(50, 14, 14, 0.9)).stroke(Stroke::new(1.0_f32, lit(0.18))).corner_radius(CornerRadius::same(20)).inner_margin(egui::Margin::same(14)).show(&mut cu, |ui| {
                    ui.set_width(w - 28.0);
                    ui.vertical_centered(|ui| {
                        ui.add(egui::Label::new(egui::RichText::new("Remove the local account on this PC and set up a new one?").font(font(13.0, 400))).wrap());
                        ui.add_space(4.0);
                        ui.horizontal(|ui| {
                            let bw = (w - 28.0 - 8.0) / 2.0;
                            ui.spacing_mut().item_spacing.x = 8.0;
                            if theme::button(ui, "Cancel", Btn::Subtle, 13.0, Some(bw), Vec2::new(8.0, 8.0)).clicked() {
                                f.resetting = false;
                            }
                            if theme::button(ui, "Reset", Btn::Danger, 13.0, Some(bw), Vec2::new(8.0, 8.0)).clicked() {
                                auth::reset_account();
                                f.account = None;
                                f.settings = None;
                                f.resetting = false;
                                f.password.clear();
                                f.error.clear();
                            }
                        });
                    });
                });
            }

            if submit {
                let fail = |msg: &str, f: &mut LoginForm| {
                    f.error = msg.to_string();
                    f.shake_at = Some(now);
                };
                if f.account.is_some() {
                    match auth::login(&f.password) {
                        Some(name) => entered = Some(name),
                        None => {
                            f.password.clear();
                            fail("Incorrect password", f);
                            cu.memory_mut(|m| m.request_focus(Id::new("login-pass")));
                        }
                    }
                } else if f.name.trim().is_empty() {
                    fail("Enter a name", f);
                } else if f.password.chars().count() < 4 {
                    fail("Password needs at least 4 characters", f);
                } else if f.password != f.confirm {
                    fail("Passwords do not match", f);
                } else {
                    entered = Some(auth::create_account(f.name.trim(), &f.password));
                }
            }
        }

        p.text(Pos2::new(screen.center().x, screen.bottom() - screen.height() * 0.04), Align2::CENTER_BOTTOM, "Local account · stored on this PC only", font(12.0, 400), lit(0.5));
        let _ = StrokeKind::Inside;
    });
    // Correct details: show the loading step, then hand over to the desktop.
    if let Some(name) = entered {
        f.error.clear();
        f.loading = Some((now, name));
    }
    None
}
