//! Settings, laid out like macOS System Settings: a sidebar with search and
//! coloured icons, back / forward arrows, and grouped rows on the right.

use crate::core::{Core, ServerStatus};
use crate::fieldguide::data::guide;
use crate::relay::PORT;
use crate::settings::{import_wallpaper, FILL_COLORS, FITS, WALLPAPERS};
use crate::ui::icons;
use crate::ui::theme::{self, *};
use crate::ui::wallpaper;
use eframe::egui::{self, Align, Align2, Color32, CornerRadius, Id, Layout, Margin, Pos2, Rect, RichText, Sense, Stroke, StrokeKind, Ui, UiBuilder, Vec2};

pub struct SettingsUi {
    pub current: usize,
    back: Vec<usize>,
    forward: Vec<usize>,
    search: String,
    message: String,
    copied: Option<(String, f64)>,
}

impl Default for SettingsUi {
    fn default() -> Self {
        Self { current: WALLPAPER_SECTION, back: vec![], forward: vec![], search: String::new(), message: String::new(), copied: None }
    }
}

impl SettingsUi {
    /// Go to a section, remembering where we were (for the back arrow).
    pub fn go(&mut self, section: usize) {
        if section != self.current {
            self.back.push(self.current);
            self.forward.clear();
            self.current = section;
            self.message.clear();
        }
    }
}

struct Section {
    name: &'static str,
    tint: [u8; 3],
    icon: &'static str,
    /// Extra words the search box matches.
    keywords: &'static str,
    /// Sidebar group (a gap is left between groups).
    group: u8,
}

// Add a settings section: write a pane function, add a line here and to `pane()`.
const SECTIONS: [Section; 6] = [
    Section { name: "Account", tint: [0x5a, 0x6b, 0x85], icon: icons::ACCOUNT, keywords: "name user password local", group: 0 },
    Section { name: "Appearance", tint: [0x1d, 0x1d, 0x1f], icon: r#"<circle cx="12" cy="12" r="8"/><path d="M12 4a8 8 0 0 1 0 16z" fill="white"/>"#, keywords: "dark light mode theme auto window windowed borderless fullscreen full screen display monitor screen", group: 1 },
    Section { name: "Wallpaper", tint: [0x2e, 0x7a, 0xb5], icon: icons::WALLPAPER, keywords: "background picture photo desktop fill fit stretch centre center tile colour color", group: 1 },
    Section { name: "Stream Deck", tint: [0x1f, 0x8a, 0x6c], icon: icons::STREAMDECK, keywords: "plugin keys address control connection", group: 2 },
    Section { name: "Software Update", tint: [0x6b, 0x6b, 0x70], icon: icons::UPDATE, keywords: "update version install new", group: 3 },
    Section { name: "About", tint: [0x1d, 0x0f, 0x5e], icon: r#"<circle cx="12" cy="12" r="9"/><path d="M12 11v6M12 7.5v.5"/>"#, keywords: "version info", group: 3 },
];

pub const ACCOUNT_SECTION: usize = 0;
pub const WALLPAPER_SECTION: usize = 2;
/// The Software Update section's place in the list.
pub const UPDATE_SECTION: usize = 4;

const TOOLBAR_H: f32 = 52.0;

pub fn show(ui: &mut Ui, st: &mut SettingsUi, core: &mut Core, now: f64, radius: u8) {
    let rect = ui.max_rect();
    let narrow = rect.width() <= 560.0;
    let name = core.user.clone().unwrap_or_default();
    let side_w = if narrow { 0.0 } else { 220.0 };
    let side = Rect::from_min_size(rect.min, Vec2::new(side_w, rect.height()));
    let pane = Rect::from_min_max(Pos2::new(rect.left() + side_w, rect.top()), rect.max);

    if !narrow {
        sidebar(ui, st, core, side, radius, &name);
    }

    // ── Toolbar: ‹ › and the section name ──
    let bar = Rect::from_min_size(pane.min, Vec2::new(pane.width(), TOOLBAR_H));
    let mut tb = ui.new_child(UiBuilder::new().max_rect(bar.shrink2(Vec2::new(16.0, 10.0))).layout(Layout::left_to_right(Align::Center)));
    let pill = Rect::from_min_size(tb.cursor().min, Vec2::new(64.0, 30.0));
    tb.painter().rect(pill, CornerRadius::same(15), group_bg(), Stroke::new(1.0_f32, white(0.12)), StrokeKind::Inside);
    tb.painter().vline(pill.center().x, (pill.top() + 7.0)..=(pill.bottom() - 7.0), Stroke::new(1.0_f32, white(0.15)));
    for (i, glyph) in ["‹", "›"].iter().enumerate() {
        let r = Rect::from_min_size(pill.min + Vec2::new(i as f32 * 32.0, 0.0), Vec2::new(32.0, 30.0));
        let enabled = if i == 0 { !st.back.is_empty() } else { !st.forward.is_empty() };
        let resp = tb.interact(r, Id::new(("settings-nav", i)), if enabled { Sense::click() } else { Sense::hover() });
        if enabled && resp.hovered() {
            tb.painter().rect_filled(r.shrink(2.0), 13.0, white(0.1));
        }
        tb.painter().text(r.center() - Vec2::new(0.0, 2.0), Align2::CENTER_CENTER, glyph, font(22.0, 400), if enabled { ink() } else { white(0.3) });
        if enabled && resp.clicked() {
            if i == 0 {
                let prev = st.back.pop().unwrap();
                st.forward.push(st.current);
                st.current = prev;
            } else {
                let next = st.forward.pop().unwrap();
                st.back.push(st.current);
                st.current = next;
            }
        }
    }
    tb.add_space(76.0);
    tb.label(RichText::new(SECTIONS[st.current].name).font(font(15.0, 600)));
    if narrow {
        // No sidebar in a small window: pick the section from a menu instead.
        tb.with_layout(Layout::right_to_left(Align::Center), |ui| {
            egui::ComboBox::from_id_salt("settings-sections").selected_text("Sections").width(130.0).show_ui(ui, |ui| {
                for (i, sec) in SECTIONS.iter().enumerate() {
                    if ui.selectable_label(st.current == i, sec.name).clicked() {
                        st.go(i);
                    }
                }
            });
        });
    }

    // ── The section itself ──
    let body = Rect::from_min_max(Pos2::new(pane.left(), bar.bottom()), pane.max);
    let mut pn = ui.new_child(UiBuilder::new().max_rect(body));
    pn.set_clip_rect(body.intersect(ui.clip_rect()));
    egui::ScrollArea::vertical().id_salt(("settings-pane", st.current)).auto_shrink([false, false]).show(&mut pn, |ui| {
        egui::Frame::NONE.inner_margin(Margin { left: 20, right: 20, top: 4, bottom: 24 }).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.spacing_mut().item_spacing.y = 8.0;
            match st.current {
                ACCOUNT_SECTION => account_pane(ui, &name),
                1 => appearance_pane(ui, core),
                WALLPAPER_SECTION => wallpaper_pane(ui, st, core, &name),
                3 => streamdeck_pane(ui, st, core, now),
                UPDATE_SECTION => update_pane(ui, core),
                _ => about_pane(ui),
            }
        });
    });
}

fn sidebar(ui: &mut Ui, st: &mut SettingsUi, core: &Core, side: Rect, radius: u8, name: &str) {
    let p = ui.painter().clone();
    p.rect_filled(side, CornerRadius { nw: 0, ne: 0, sw: radius.saturating_sub(1), se: 0 }, sidebar_bg());
    p.vline(side.right(), side.y_range(), Stroke::new(1.0_f32, white(0.1)));
    let mut sb = ui.new_child(UiBuilder::new().max_rect(side.shrink2(Vec2::new(10.0, 12.0))));
    sb.set_clip_rect(side.intersect(ui.clip_rect()));

    // Search
    let (sr, _) = sb.allocate_exact_size(Vec2::new(sb.available_width(), 28.0), Sense::hover());
    sb.painter().rect(sr, CornerRadius::same(7), white(0.08), Stroke::NONE, StrokeKind::Inside);
    let mut ic = sb.new_child(UiBuilder::new().max_rect(Rect::from_min_size(sr.min + Vec2::new(8.0, 7.0), Vec2::splat(14.0))));
    icons::line_icon(&mut ic, "search", r#"<circle cx="11" cy="11" r="6"/><path d="M20 20l-4.5-4.5"/>"#, 14.0, white(0.5), 2.2);
    let edit = egui::TextEdit::singleline(&mut st.search)
        .id(Id::new("settings-search"))
        .frame(false)
        .font(font(13.0, 400))
        .hint_text(RichText::new("Search").color(white(0.45)))
        .margin(Vec2::ZERO);
    sb.put(Rect::from_min_max(sr.min + Vec2::new(28.0, 6.0), sr.max - Vec2::new(8.0, 5.0)), edit);
    sb.add_space(10.0);

    let q = st.search.trim().to_lowercase();
    let matches = |s: &Section| q.is_empty() || s.name.to_lowercase().contains(&q) || s.keywords.contains(&q);

    // The user, at the top (opens Account), like a Mac's account card.
    if q.is_empty() {
        let (r, resp) = sb.allocate_exact_size(Vec2::new(sb.available_width(), 50.0), Sense::click());
        let active = st.current == ACCOUNT_SECTION;
        if active {
            sb.painter().rect_filled(r, 8.0, select_blue());
        } else {
            theme::hover_fill(&sb, &resp, r, 8, white(0.08));
        }
        let c = Pos2::new(r.left() + 6.0 + 18.0, r.center().y);
        sb.painter().circle_filled(c, 18.0, if active { WHITE } else { rgb([0x5a, 0x6b, 0x85]) });
        let initial = name.chars().next().map(|c| c.to_uppercase().to_string()).unwrap_or_default();
        sb.painter().text(c, Align2::CENTER_CENTER, initial, display(15.0, 800), if active { select_blue() } else { WHITE });
        let tc = if active { WHITE } else { ink() };
        let title = theme::elide(&sb, name, &font(13.5, 600), r.width() - 56.0);
        sb.painter().text(Pos2::new(c.x + 26.0, r.center().y - 8.0), Align2::LEFT_CENTER, title, font(13.5, 600), tc);
        sb.painter().text(Pos2::new(c.x + 26.0, r.center().y + 9.0), Align2::LEFT_CENTER, "Local account", font(11.5, 400), if active { lit(0.8) } else { white(0.55) });
        if resp.clicked() {
            st.go(ACCOUNT_SECTION);
        }
        sb.add_space(8.0);
    }

    egui::ScrollArea::vertical().id_salt("settings-side").auto_shrink([false, false]).show(&mut sb, |ui| {
        ui.spacing_mut().item_spacing.y = 1.0;
        let mut last_group = None;
        let mut any = false;
        for (i, sec) in SECTIONS.iter().enumerate() {
            if (i == ACCOUNT_SECTION && q.is_empty()) || !matches(sec) {
                continue;
            }
            any = true;
            if last_group.is_some_and(|g| g != sec.group) {
                ui.add_space(10.0);
            }
            last_group = Some(sec.group);
            let active = st.current == i;
            let (r, resp) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 30.0), Sense::click());
            if active {
                ui.painter().rect_filled(r, 7.0, select_blue());
            } else {
                theme::hover_fill(ui, &resp, r, 7, white(0.08));
            }
            let ir = Rect::from_min_size(Pos2::new(r.left() + 6.0, r.center().y - 11.0), Vec2::splat(22.0));
            paint_icon_tile(ui, ir, sec.tint);
            let mut child = ui.new_child(UiBuilder::new().max_rect(Rect::from_center_size(ir.center(), Vec2::splat(14.0))));
            icons::line_icon(&mut child, sec.name, sec.icon, 14.0, WHITE, 2.2);
            ui.painter().text(Pos2::new(ir.right() + 9.0, r.center().y), Align2::LEFT_CENTER, sec.name, font(13.5, 400), if active { WHITE } else { ink() });
            // A badge on Software Update when a new version is waiting.
            if i == UPDATE_SECTION && core.update.release().is_some() {
                let c = Pos2::new(r.right() - 14.0, r.center().y);
                ui.painter().circle_filled(c, 9.0, Color32::from_rgb(239, 68, 68));
                ui.painter().text(c, Align2::CENTER_CENTER, "1", font(11.0, 700), WHITE);
            }
            if resp.clicked() {
                st.go(i);
            }
        }
        if !any {
            ui.add_space(10.0);
            ui.label(RichText::new("No results").font(font(13.0, 400)).color(white(0.5)));
        }
    });
}

/// A small rounded square in a colour, for sidebar icons (lighter at the top, like a Mac).
fn paint_icon_tile(ui: &Ui, r: Rect, tint: [u8; 3]) {
    wallpaper::gradient_rect(ui.painter(), r, 6.0, 180.0, theme::mix(tint, 0.75, [255, 255, 255], 1.0), rgb(tint));
}

// ── Grouped rows, Mac style ──────────────────────────────────────────────

/// A titled group of rows in a rounded box.
fn group(ui: &mut Ui, title: Option<&str>, rows: impl FnOnce(&mut Ui)) {
    if let Some(t) = title {
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            ui.add_space(4.0);
            ui.label(RichText::new(t).font(font(13.0, 600)));
        });
    }
    egui::Frame::NONE
        .fill(group_bg())
        .stroke(Stroke::new(1.0_f32, white(0.1)))
        .corner_radius(CornerRadius::same(10))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.spacing_mut().item_spacing.y = 0.0;
            rows(ui);
        });
}

/// One row: a label on the left, `right` on the right. `first` rows get no line above.
fn row(ui: &mut Ui, first: bool, label: &str, sub: Option<&str>, right: impl FnOnce(&mut Ui)) {
    if !first {
        let y = ui.cursor().top();
        let x = ui.max_rect().x_range();
        ui.painter().hline((x.min + 12.0)..=(x.max - 12.0), y, Stroke::new(1.0_f32, white(0.1)));
    }
    egui::Frame::NONE.inner_margin(Margin::symmetric(12, 9)).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.set_min_height(22.0);
        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing.y = 1.0;
                ui.label(RichText::new(label).font(font(13.5, 400)));
                if let Some(sub) = sub {
                    ui.label(RichText::new(sub).font(font(11.5, 400)).color(white(0.55)));
                }
            });
            ui.with_layout(Layout::right_to_left(Align::Center), right);
        });
    });
}

fn note(ui: &mut Ui, text: &str) {
    ui.horizontal(|ui| {
        ui.add_space(4.0);
        ui.add(egui::Label::new(RichText::new(text).font(font(12.0, 400)).color(white(0.5))).wrap());
    });
}

/// A small Mac-style push button ("Add Photo…").
fn mac_button(ui: &mut Ui, label: &str) -> bool {
    theme::button(ui, label, Btn::Glass, 13.0, None, Vec2::new(12.0, 5.0)).clicked()
}

// ── Appearance ────────────────────────────────────────────────────────────

fn appearance_pane(ui: &mut Ui, core: &mut Core) {
    let current = core.settings.appearance.clone();
    let mut pick = None;
    group(ui, None, |ui| {
        row(ui, true, "Appearance", Some("Windows, menus and dialogs. The menu bar and dock stay on the wallpaper."), |_| {});
        egui::Frame::NONE.inner_margin(Margin { left: 12, right: 12, top: 0, bottom: 14 }).show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 18.0;
                for (id, label) in [("auto", "Auto"), ("light", "Light"), ("dark", "Dark")] {
                    let (r, resp) = ui.allocate_exact_size(Vec2::new(96.0, 92.0), Sense::click());
                    let on = current == id;
                    let shot = Rect::from_min_size(r.min + Vec2::new(4.0, 4.0), Vec2::new(88.0, 58.0));
                    // A tiny window in that look (Auto is half and half).
                    let draw = |p: &egui::Painter, rect: Rect, light: bool| {
                        let (bg, side, bar) = if light {
                            (Color32::from_rgb(244, 244, 246), Color32::from_rgb(222, 222, 228), Color32::from_rgb(214, 45, 32))
                        } else {
                            (Color32::from_rgb(52, 20, 22), Color32::from_rgb(36, 14, 16), Color32::from_rgb(200, 40, 30))
                        };
                        p.rect_filled(rect, 4.0, bg);
                        p.rect_filled(Rect::from_min_size(rect.min, Vec2::new(rect.width() * 0.3, rect.height())), CornerRadius { nw: 4, sw: 4, ne: 0, se: 0 }, side);
                        p.rect_filled(Rect::from_min_size(rect.min + Vec2::new(rect.width() * 0.38, 10.0), Vec2::new(rect.width() * 0.5, 6.0)), 3.0, bar);
                    };
                    let p = ui.painter().clone();
                    wallpaper::paint_preset_rounded(&p, shot, &WALLPAPERS[0], true, 8.0);
                    let win = shot.shrink2(Vec2::new(12.0, 10.0));
                    match id {
                        "auto" => {
                            draw(&p.with_clip_rect(Rect::from_min_max(win.min, Pos2::new(win.center().x, win.bottom()))), win, true);
                            draw(&p.with_clip_rect(Rect::from_min_max(Pos2::new(win.center().x, win.top()), win.max)), win, false);
                        }
                        "light" => draw(&p, win, true),
                        _ => draw(&p, win, false),
                    }
                    p.rect_stroke(shot, CornerRadius::same(8), Stroke::new(if on { 3.0_f32 } else { 1.0 }, if on { select_blue() } else { white(0.2) }), StrokeKind::Outside);
                    p.text(Pos2::new(r.center().x, shot.bottom() + 10.0), Align2::CENTER_TOP, label, font(12.5, if on { 600 } else { 400 }), ink());
                    if resp.hovered() {
                        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                    }
                    if resp.clicked() {
                        pick = Some(id);
                    }
                }
            });
        });
    });
    note(ui, "Auto follows the light or dark setting in Windows.");

    // How the app's own window shows (for this PC, remembered every time it opens).
    let current_mode = crate::display::mode();
    group(ui, Some("Window"), |ui| {
        row(ui, true, "Window mode", None, |ui| {
            let label = crate::display::MODES.iter().find(|(m, _)| *m == current_mode).map_or("Windowed", |m| m.1);
            egui::ComboBox::from_id_salt("window-mode").selected_text(label).width(190.0).show_ui(ui, |ui| {
                for (m, name) in crate::display::MODES {
                    if ui.selectable_label(current_mode == m, name).clicked() && current_mode != m {
                        crate::display::set_mode(m);
                    }
                }
            });
        });
        // Which screen the app sits on (each connected monitor is detected).
        let screens = crate::display::monitors();
        let target = crate::display::target_monitor();
        row(ui, false, "Display", None, |ui| {
            let label = target.as_ref().map_or("This screen".to_string(), |m| m.label.clone());
            egui::ComboBox::from_id_salt("window-monitor").selected_text(label).width(190.0).show_ui(ui, |ui| {
                if screens.is_empty() {
                    ui.label("This screen");
                }
                for m in &screens {
                    let on = target.as_ref().is_some_and(|t| t.id == m.id);
                    if ui.selectable_label(on, &m.label).clicked() && !on {
                        crate::display::set_monitor(Some(m.id.clone()));
                    }
                }
            });
        });
    });
    note(ui, "Saved on this PC and used every time Melbourne Fire Brigade opens. If the chosen display is unplugged, the main one is used. F11 switches fullscreen on and off for now.");
    if let Some(id) = pick {
        let mut s = core.settings.clone();
        s.appearance = id.to_string();
        core.save_settings(s);
    }
}

// ── Wallpaper ─────────────────────────────────────────────────────────────

fn wallpaper_pane(ui: &mut Ui, st: &mut SettingsUi, core: &mut Core, name: &str) {
    let settings = core.settings.clone();
    let using_image = settings.using_image();
    let mut new_settings: Option<crate::settings::UserSettings> = None;
    let mut add_photo = false;

    // Top: the current wallpaper next to its options, like a Mac.
    ui.horizontal_top(|ui| {
        ui.spacing_mut().item_spacing.x = 14.0;
        let pw = 176.0;
        let (r, _) = ui.allocate_exact_size(Vec2::new(pw, pw * 10.0 / 16.0), Sense::hover());
        let p = ui.painter().clone();
        theme::shadow(&p, r, 8, 4, 12, 0.25);
        let scale = r.width() / ui.ctx().screen_rect().width().max(1.0);
        wallpaper::paint_scaled(ui, &p, r, &settings, true, 8, scale);
        p.rect_stroke(r, CornerRadius::same(8), Stroke::new(1.0_f32, white(0.15)), StrokeKind::Outside);

        ui.vertical(|ui| {
            ui.set_width(ui.available_width());
            group(ui, None, |ui| {
                let label = if using_image { "Your picture" } else { settings.preset().name };
                row(ui, true, label, None, |ui| {
                    if using_image {
                        let current = FITS.iter().find(|(id, _)| *id == settings.fit).map_or("Fill Screen", |f| f.1);
                        egui::ComboBox::from_id_salt("wallpaper-fit").selected_text(current).width(140.0).show_ui(ui, |ui| {
                            for (id, label) in FITS {
                                if ui.selectable_label(settings.fit == id, label).clicked() {
                                    let mut s = settings.clone();
                                    s.fit = id.to_string();
                                    new_settings = Some(s);
                                }
                            }
                        });
                    } else {
                        ui.label(RichText::new("Fill Screen").font(font(13.0, 400)).color(white(0.45)));
                    }
                });
                if using_image && matches!(settings.fit.as_str(), "fit" | "center") {
                    row(ui, false, "Colour around picture", None, |_| {});
                    egui::Frame::NONE.inner_margin(Margin { left: 12, right: 12, top: 0, bottom: 10 }).show(ui, |ui| {
                        ui.horizontal_wrapped(|ui| {
                            ui.spacing_mut().item_spacing.x = 6.0;
                            for c in FILL_COLORS.iter() {
                                let (r, resp) = ui.allocate_exact_size(Vec2::splat(20.0), Sense::click());
                                ui.painter().circle_filled(r.center(), 8.5, rgb(*c));
                                let on = settings.fill_color == *c;
                                ui.painter().circle_stroke(r.center(), if on { 10.0 } else { 8.5 }, Stroke::new(if on { 2.0_f32 } else { 1.0 }, if on { select_blue() } else { white(0.25) }));
                                if resp.hovered() {
                                    ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                                }
                                if resp.clicked() {
                                    let mut s = settings.clone();
                                    s.fill_color = *c;
                                    new_settings = Some(s);
                                }
                            }
                            // Any other colour
                            let mut custom = settings.fill_color;
                            if ui.color_edit_button_srgb(&mut custom).on_hover_text("Other colour").changed() {
                                let mut s = settings.clone();
                                s.fill_color = custom;
                                new_settings = Some(s);
                            }
                        });
                    });
                }
            });
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if settings.custom_image.is_some() && mac_button(ui, "Remove Photo") {
                        let mut s = settings.clone();
                        if let Some(old) = s.custom_image.take() {
                            let _ = std::fs::remove_file(old);
                        }
                        s.wallpaper.kind = "preset".into();
                        new_settings = Some(s);
                    }
                    if mac_button(ui, "Add Photo…") {
                        add_photo = true;
                    }
                });
            });
        });
    });
    if !st.message.is_empty() {
        ui.label(RichText::new(&st.message).font(font(13.0, 400)).color(red_text()));
    }

    // Built-in wallpapers
    section_header(ui, "Built-in Wallpapers");
    thumb_grid(ui, WALLPAPERS.len(), |ui, i, r| {
        let wp = &WALLPAPERS[i];
        let selected = !using_image && settings.preset().id == wp.id;
        let resp = ui.interact(r, Id::new(("wp", i)), Sense::click());
        thumb(ui, r, wp.name, selected, resp.hovered(), |ui, ir| wallpaper::paint_preset_rounded(ui.painter(), ir, wp, true, 8.0));
        if resp.clicked() {
            let mut s = settings.clone();
            s.wallpaper = crate::settings::Wallpaper { kind: "preset".into(), id: Some(wp.id.into()) };
            new_settings = Some(s);
        }
    });

    // Your photos: an "Add Photo…" tile, then the user's picture.
    section_header(ui, "Your Photos");
    let own = settings.custom_image.clone();
    let count = 1 + own.is_some() as usize;
    thumb_grid(ui, count, |ui, i, r| {
        let resp = ui.interact(r, Id::new(("wp-own", i)), Sense::click());
        if i == 0 {
            thumb(ui, r, "Add Photo…", false, resp.hovered(), |ui, ir| {
                ui.painter().rect_filled(ir, 8.0, white(if resp.hovered() { 0.1 } else { 0.06 }));
                let mut ic = ui.new_child(UiBuilder::new().max_rect(Rect::from_center_size(ir.center(), Vec2::splat(26.0))));
                icons::line_icon(&mut ic, "add-photo", r#"<rect x="3" y="5" width="15" height="13" rx="2"/><path d="M3 15l4-4 3 3 2-2 6 6M19 3v6M16 6h6"/>"#, 26.0, white(0.5), 1.8);
            });
            if resp.clicked() {
                add_photo = true;
            }
        } else if let Some(path) = &own {
            thumb(ui, r, "Your picture", using_image, resp.hovered(), |ui, ir| {
                if !wallpaper::paint_image_cover_rounded(ui, ui.painter(), ir, path, 8) {
                    ui.painter().rect_filled(ir, 8.0, white(0.1));
                }
            });
            if resp.clicked() {
                let mut s = settings.clone();
                s.wallpaper.kind = "image".into();
                new_settings = Some(s);
            }
        }
    });
    ui.add_space(6.0);
    note(ui, &format!("Saved for {name} on this PC. It will move to the database when that's connected."));

    if add_photo {
        st.message.clear();
        let file = rfd::FileDialog::new().set_title("Choose a picture").add_filter("Pictures", &["png", "jpg", "jpeg", "gif", "bmp", "webp"]).pick_file();
        if let Some(file) = file {
            match import_wallpaper(name, &file) {
                Ok(path) => {
                    let mut s = new_settings.take().unwrap_or_else(|| settings.clone());
                    if let Some(old) = &s.custom_image {
                        let _ = std::fs::remove_file(old);
                    }
                    s.custom_image = Some(path);
                    s.wallpaper.kind = "image".into();
                    new_settings = Some(s);
                }
                Err(e) => st.message = e,
            }
        }
    }
    if let Some(s) = new_settings {
        if !core.save_settings(s) {
            st.message = "That couldn't be saved on this PC.".into();
        }
    }
}

fn section_header(ui: &mut Ui, title: &str) {
    ui.add_space(12.0);
    ui.horizontal(|ui| {
        ui.add_space(2.0);
        ui.label(RichText::new(title).font(font(13.5, 600)));
    });
    ui.add_space(2.0);
}

fn thumb_grid(ui: &mut Ui, count: usize, mut cell: impl FnMut(&mut Ui, usize, Rect)) {
    let gap = 12.0;
    let avail = ui.available_width();
    let cols = ((avail + gap) / (118.0 + gap)).floor().clamp(1.0, 5.0) as usize;
    let w = (avail - gap * (cols as f32 - 1.0)) / cols as f32;
    let h = w * 10.0 / 16.0 + 6.0 + 16.0;
    let rows = count.div_ceil(cols);
    let (area, _) = ui.allocate_exact_size(Vec2::new(avail, rows as f32 * h + (rows.saturating_sub(1)) as f32 * gap), Sense::hover());
    for i in 0..count {
        let (row, col) = (i / cols, i % cols);
        let r = Rect::from_min_size(area.min + Vec2::new(col as f32 * (w + gap), row as f32 * (h + gap)), Vec2::new(w, h));
        cell(ui, i, r);
    }
}

fn thumb(ui: &mut Ui, r: Rect, label: &str, selected: bool, hovered: bool, paint: impl FnOnce(&mut Ui, Rect)) {
    let ir = Rect::from_min_size(r.min, Vec2::new(r.width(), r.width() * 10.0 / 16.0));
    paint(ui, ir);
    let p = ui.painter();
    if selected {
        p.rect_stroke(ir.expand(1.0), CornerRadius::same(9), Stroke::new(3.0_f32, select_blue()), StrokeKind::Outside);
    } else {
        p.rect_stroke(ir, CornerRadius::same(8), Stroke::new(1.0_f32, white(if hovered { 0.35 } else { 0.15 })), StrokeKind::Outside);
    }
    p.text(Pos2::new(r.center().x, ir.bottom() + 6.0), Align2::CENTER_TOP, label, font(12.0, 400), if selected { ink() } else { white(0.75) });
    if hovered {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
}

fn copy_button(ui: &mut Ui, st: &mut SettingsUi, text: &str, label: &str, now: f64, small: bool) {
    let copied = st.copied.as_ref().is_some_and(|(t, at)| t == text && now - at < 1.5);
    let pad = if small { Vec2::new(14.0, 6.0) } else { Vec2::new(18.0, 8.0) };
    if theme::button(ui, if copied { "Copied" } else { label }, Btn::Glass, 13.0, None, pad).clicked() {
        ui.ctx().copy_text(text.to_string());
        st.copied = Some((text.to_string(), now));
    }
    if copied {
        ui.ctx().request_repaint_after(std::time::Duration::from_millis(300));
    }
}

fn streamdeck_pane(ui: &mut Ui, st: &mut SettingsUi, core: &mut Core, now: f64) {
    ui.add(egui::Label::new(RichText::new("Install the Melbourne Fire Brigade plugin in the Stream Deck app and the keys just work. There's nothing to set up: they control whoever is signed in here.").font(font(14.0, 400)).color(white(0.75))).wrap());

    section_header(ui, "Connection");
    let connected = core.server == ServerStatus::Running;
    egui::Frame::NONE.fill(group_bg()).stroke(Stroke::new(1.0_f32, white(0.1))).corner_radius(CornerRadius::same(10)).inner_margin(Margin::symmetric(16, 12)).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.horizontal_wrapped(|ui| {
            ui.label(RichText::new(if connected { "Ready for your Stream Deck" } else { "Not ready" }).font(font(14.0, 500)).color(ink()));
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let text = if connected { "Connected" } else { "Not connected" };
                ui.label(RichText::new(text).font(font(12.0, 400)));
                let (r, _) = ui.allocate_exact_size(Vec2::splat(8.0), Sense::hover());
                theme::dot(ui.painter(), r.center(), connected, false);
            });
        });
    });
    if let ServerStatus::Failed(msg) = &core.server {
        ui.label(RichText::new(msg).font(font(13.0, 400)).color(red_text()));
    }
    note(ui, "Keys only work on this PC, while Melbourne Fire Brigade is open and someone is signed in.");

    section_header(ui, "Addresses for your keys");
    note(ui, "In the Stream Deck app, add a \"Website\" action, paste an address, and tick \"GET request in background\". Sending a key opens the Field Guide if it's closed. Melbourne Fire Brigade has to be open for the keys to work.");
    ui.add_space(6.0);

    let mut commands: Vec<(String, String)> = vec![
        ("Clock in / clock out (one key does both)".into(), "timeclock/toggle".into()),
        ("Tick the next step".into(), "fieldguide/step".into()),
        ("Un-tick the last step".into(), "fieldguide/unstep".into()),
        ("Start this page again".into(), "fieldguide/reset".into()),
        ("Next option (pages that ask which situation applies)".into(), "fieldguide/option".into()),
        ("Pick option 1".into(), "fieldguide/option/1".into()),
        ("Pick option 2".into(), "fieldguide/option/2".into()),
        ("Pick option 3".into(), "fieldguide/option/3".into()),
        ("MICA available: yes".into(), "fieldguide/mica/yes".into()),
        ("MICA available: no".into(), "fieldguide/mica/no".into()),
        ("Next page".into(), "fieldguide/next".into()),
        ("Previous page".into(), "fieldguide/prev".into()),
        ("Home".into(), "fieldguide/home".into()),
    ];
    for p in guide().pages.iter().filter(|p| p.id != "home") {
        commands.push((format!("Open: {}", p.title), format!("fieldguide/open/{}", p.id)));
    }
    egui::Frame::NONE.fill(group_bg()).stroke(Stroke::new(1.0_f32, white(0.1))).corner_radius(CornerRadius::same(10)).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.spacing_mut().item_spacing.y = 0.0;
        for (i, (label, path)) in commands.iter().enumerate() {
            if i > 0 {
                let y = ui.cursor().top();
                ui.painter().hline(ui.max_rect().x_range(), y, Stroke::new(1.0_f32, white(0.1)));
            }
            egui::Frame::NONE.inner_margin(Margin::symmetric(14, 10)).show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.horizontal(|ui| {
                    let bw = 130.0;
                    ui.allocate_ui_with_layout(Vec2::new((ui.available_width() - bw).max(80.0), 0.0), Layout::top_down(Align::Min), |ui| {
                        ui.spacing_mut().item_spacing.y = 2.0;
                        ui.add(egui::Label::new(RichText::new(label).font(font(14.0, 500))).wrap());
                        ui.add(egui::Label::new(RichText::new(format!("/api/{path}")).font(mono(12.5)).color(white(0.55))).wrap());
                    });
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        let url = format!("http://localhost:{PORT}/api/{path}");
                        copy_button(ui, st, &url, "Copy address", now, true);
                    });
                });
            });
        }
    });
}

fn account_pane(ui: &mut Ui, name: &str) {
    let rows = [("Name", name.to_string()), ("Account type", "Local".into()), ("Stored", "This PC only".into())];
    egui::Frame::NONE.fill(group_bg()).stroke(Stroke::new(1.0_f32, white(0.1))).corner_radius(CornerRadius::same(10)).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.spacing_mut().item_spacing.y = 0.0;
        for (i, (k, v)) in rows.iter().enumerate() {
            if i > 0 {
                let y = ui.cursor().top();
                ui.painter().hline(ui.max_rect().x_range(), y, Stroke::new(1.0_f32, white(0.18)));
            }
            egui::Frame::NONE.inner_margin(Margin::symmetric(16, 12)).show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.horizontal(|ui| {
                    ui.label(RichText::new(*k).font(font(14.0, 400)).color(white(0.65)));
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        ui.label(RichText::new(v).font(font(14.0, 600)));
                    });
                });
            });
        }
    });
    ui.add_space(6.0);
    note(ui, "Sign-in and syncing between PCs arrive with the database.");
}

fn about_pane(ui: &mut Ui) {
    ui.vertical_centered(|ui| {
        ui.add_space(12.0);
        icons::logo(ui, 56.0);
        ui.add_space(4.0);
        ui.label(RichText::new("Melbourne Fire Brigade").font(display(20.0, 800)));
        ui.label(RichText::new(format!("Version {}", env!("CARGO_PKG_VERSION"))).font(font(13.0, 400)).color(white(0.7)));
        ui.add_space(4.0);
        ui.allocate_ui(Vec2::new(280.0, 60.0), |ui| {
            ui.add(egui::Label::new(RichText::new("A fictional roleplay service. Not affiliated with any real fire or emergency service.").font(font(12.0, 400)).color(white(0.5))).wrap());
        });
    });
}

fn update_pane(ui: &mut Ui, core: &mut Core) {
    use crate::core::UpdateRequest;
    use crate::update::{UpdateStatus, CURRENT, UPDATE_REPO};

    // This version
    egui::Frame::NONE.fill(group_bg()).stroke(Stroke::new(1.0_f32, white(0.1))).corner_radius(CornerRadius::same(10)).inner_margin(Margin::symmetric(16, 14)).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.horizontal(|ui| {
            icons::logo(ui, 40.0);
            ui.add_space(4.0);
            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing.y = 0.0;
                ui.label(RichText::new("Melbourne Fire Brigade").font(font(15.0, 600)));
                ui.label(RichText::new(format!("Version {CURRENT}")).font(font(13.0, 400)).color(white(0.6)));
            });
        });
    });
    ui.add_space(8.0);

    // Status
    let status = core.update.clone();
    egui::Frame::NONE.fill(group_bg()).stroke(Stroke::new(1.0_f32, white(0.1))).corner_radius(CornerRadius::same(10)).inner_margin(Margin::symmetric(16, 14)).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.spacing_mut().item_spacing.y = 6.0;
        match &status {
            UpdateStatus::Idle | UpdateStatus::UpToDate => {
                let (title, sub) = if status == UpdateStatus::UpToDate {
                    ("Melbourne Fire Brigade is up to date".to_string(), format!("You have the newest version ({CURRENT})."))
                } else {
                    ("Updates are checked automatically".to_string(), "The app looks for a new version when it starts and every few hours.".to_string())
                };
                status_line(ui, &title, status == UpdateStatus::UpToDate);
                ui.label(RichText::new(sub).font(font(13.0, 400)).color(white(0.65)));
            }
            UpdateStatus::Checking => {
                ui.horizontal(|ui| {
                    ui.add(egui::Spinner::new().size(14.0).color(ink()));
                    ui.label(RichText::new("Checking for updates…").font(font(14.0, 600)));
                });
            }
            UpdateStatus::Available(r) | UpdateStatus::Downloading(r, _) | UpdateStatus::Ready(r, _) | UpdateStatus::Installing(r) => {
                ui.label(RichText::new(format!("Melbourne Fire Brigade {} is ready to install", r.version)).font(font(15.0, 600)));
                ui.label(RichText::new(format!("You have version {CURRENT}. The app restarts to finish installing — you'll log in again, and the Time Clock keeps running.")).font(font(13.0, 400)).color(white(0.65)));
                if !r.notes.is_empty() {
                    ui.add_space(4.0);
                    ui.label(RichText::new("What's new").font(font(13.0, 600)).color(white(0.6)));
                    egui::Frame::NONE.fill(white(0.06)).corner_radius(CornerRadius::same(10)).inner_margin(Margin::same(10)).show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.add(egui::Label::new(RichText::new(clean_notes(&r.notes)).font(font(13.0, 400))).wrap());
                    });
                }
                ui.add_space(4.0);
                match &status {
                    UpdateStatus::Downloading(_, p) => progress(ui, *p, &format!("Downloading… {}%", (p * 100.0).round())),
                    UpdateStatus::Ready(..) | UpdateStatus::Installing(_) => progress(ui, 1.0, "Installing…"),
                    _ => {
                        if theme::button(ui, "Install now", Btn::Primary, 13.0, None, Vec2::new(20.0, 8.0)).clicked() {
                            core.update_request = Some(UpdateRequest::Install);
                        }
                    }
                }
            }
            UpdateStatus::Failed(e) => {
                ui.label(RichText::new("The update didn't work").font(font(15.0, 600)));
                ui.add(egui::Label::new(RichText::new(e).font(font(13.0, 400)).color(red_text())).wrap());
            }
        }
    });
    ui.add_space(8.0);
    ui.horizontal_wrapped(|ui| {
        let busy = core.update.busy();
        if theme::button_enabled(ui, "Check for updates", Btn::Glass, 13.0, None, Vec2::new(18.0, 8.0), !busy).clicked() {
            core.update_request = Some(UpdateRequest::Check);
        }
        if let Some(at) = core.update_checked_at {
            ui.label(RichText::new(format!("Last checked {}", crate::ui::format::time(at))).font(font(12.0, 400)).color(white(0.5)));
        }
    });
    ui.add_space(4.0);
    note(ui, &format!("Updates come from github.com/{UPDATE_REPO}."));
}

fn status_line(ui: &mut Ui, title: &str, ok: bool) {
    ui.horizontal(|ui| {
        let (r, _) = ui.allocate_exact_size(Vec2::splat(10.0), Sense::hover());
        theme::dot(ui.painter(), r.center(), ok, ok);
        ui.label(RichText::new(title).font(font(15.0, 600)));
    });
}

fn progress(ui: &mut Ui, frac: f32, label: &str) {
    ui.label(RichText::new(label).font(font(13.0, 500)));
    let (r, _) = ui.allocate_exact_size(Vec2::new(ui.available_width().min(320.0), 6.0), Sense::hover());
    ui.painter().rect_filled(r, 3.0, white(0.18));
    ui.painter().rect_filled(Rect::from_min_size(r.min, Vec2::new(r.width() * frac.clamp(0.0, 1.0), 6.0)), 3.0, select_blue());
}

/// Release notes are written in Markdown on GitHub; show them as plain text.
pub fn clean_notes(notes: &str) -> String {
    notes
        .lines()
        .map(|l| {
            let t = l.trim_start();
            let t = t.trim_start_matches('#').trim_start();
            if let Some(rest) = t.strip_prefix("- ").or_else(|| t.strip_prefix("* ")) { format!("•  {rest}") } else { t.to_string() }
        })
        .map(|l| l.replace("**", "").replace('`', ""))
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_string()
}

