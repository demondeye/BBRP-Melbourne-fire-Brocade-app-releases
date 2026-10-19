//! Design tokens — from the Glass Style Guide template (fire red / ember palette) —
//! plus the small drawing helpers every screen uses.

use eframe::egui::{
    self, epaint::Shadow, Color32, CornerRadius, FontData, FontDefinitions, FontFamily, FontId, Pos2, Rect, Response, Sense, Stroke, StrokeKind, Ui, Vec2,
};
use std::sync::Arc;

pub const WHITE: Color32 = Color32::WHITE;
pub const ACCENT: Color32 = Color32::from_rgb(0x5e, 0x0f, 0x14);
pub const ICE: Color32 = Color32::from_rgb(0xf5, 0xb7, 0xa8);
pub const RED_TEXT: Color32 = Color32::from_rgb(0xfc, 0xa5, 0xa5);
pub const GREEN: Color32 = Color32::from_rgb(0x6e, 0xe7, 0xa0);

pub const MENUBAR_H: f32 = 30.0;

// ── Light / dark mode ─────────────────────────────────────────────────────
// Windows, menus and dialogs follow the user's Appearance setting. Things that
// sit on the wallpaper (menu bar, dock, login, boot) use `lit()` and stay white.

static LIGHT: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

pub fn is_light() -> bool {
    LIGHT.load(std::sync::atomic::Ordering::Relaxed)
}

/// Switch the look; restyles egui's own widgets (text boxes, dropdowns) too.
pub fn set_light(ctx: &egui::Context, light: bool) {
    if is_light() == light && ctx.style().visuals.dark_mode != light {
        return;
    }
    LIGHT.store(light, std::sync::atomic::Ordering::Relaxed);
    install_style(ctx);
}

/// Main text colour.
pub fn ink() -> Color32 {
    if is_light() { Color32::from_rgb(0x1d, 0x1d, 0x1f) } else { WHITE }
}
/// Literal white at an alpha (for things drawn over the wallpaper).
pub fn lit(a: f32) -> Color32 {
    rgba(255, 255, 255, a)
}
/// Window background.
pub fn window_bg() -> Color32 {
    if is_light() { Color32::from_rgb(0xf4, 0xf4, 0xf6) } else { Color32::from_rgb(52, 20, 22) }
}
/// Sidebars and title bars.
pub fn sidebar_bg() -> Color32 {
    if is_light() { Color32::from_rgb(0xe8, 0xe8, 0xec) } else { rgba(0, 0, 0, 0.2) }
}
/// Menus, popups and dialogs.
pub fn menu_bg() -> Color32 {
    if is_light() { rgba(250, 250, 252, 0.98) } else { rgba(46, 12, 12, 0.95) }
}
/// Grouped boxes (Mac "inset grouped" rows).
pub fn group_bg() -> Color32 {
    if is_light() { WHITE } else { lit(0.07) }
}
/// Content cards inside windows (white in light mode, faint glass in dark).
pub fn card_fill() -> Color32 {
    if is_light() { WHITE } else { lit(0.1) }
}
/// Mac-style selection blue.
pub fn select_blue() -> Color32 {
    if is_light() { Color32::from_rgb(0xd6, 0x2d, 0x20) } else { Color32::from_rgb(0xc8, 0x28, 0x1e) }
}
pub fn red_text() -> Color32 {
    if is_light() { Color32::from_rgb(0xd7, 0x2c, 0x2c) } else { RED_TEXT }
}
pub fn ice_text() -> Color32 {
    if is_light() { Color32::from_rgb(0xb8, 0x26, 0x1a) } else { ICE }
}
/// A page accent colour used for text: darkened in light mode so it stays readable.
pub fn accent_text(c: [u8; 3]) -> Color32 {
    if is_light() { lerp_color(rgb(c), Color32::BLACK, 0.4) } else { rgb(c) }
}
pub fn green() -> Color32 {
    if is_light() { Color32::from_rgb(0x22, 0xa8, 0x55) } else { GREEN }
}
pub const RADIUS_CARD: u8 = 24;
pub const RADIUS_WINDOW: u8 = 14;

/// rgba() with alpha 0..1, like CSS. Premultiplied in gamma space so
/// see-through colours blend exactly the way a browser blends them.
pub fn rgba(r: u8, g: u8, b: u8, a: f32) -> Color32 {
    let a = a.clamp(0.0, 1.0);
    let m = |c: u8| (c as f32 * a).round() as u8;
    Color32::from_rgba_premultiplied(m(r), m(g), m(b), (a * 255.0).round() as u8)
}
/// Back to straight (unpremultiplied) rgba, gamma space.
pub fn unmul(c: Color32) -> [u8; 4] {
    let a = c.a();
    if a == 0 {
        return [0, 0, 0, 0];
    }
    let u = |v: u8| ((v as f32 * 255.0 / a as f32).round()).min(255.0) as u8;
    [u(c.r()), u(c.g()), u(c.b()), a]
}
/// The main text colour at an alpha: white in dark mode, near-black in light mode.
/// Used for faint fills, borders and secondary text inside windows.
pub fn white(a: f32) -> Color32 {
    if is_light() { rgba(0, 0, 0, a * 0.62) } else { rgba(255, 255, 255, a) }
}
pub fn rgb(c: [u8; 3]) -> Color32 {
    Color32::from_rgb(c[0], c[1], c[2])
}
pub fn hex(c: u32) -> [u8; 3] {
    [(c >> 16) as u8, (c >> 8) as u8, c as u8]
}
/// `color-mix(in srgb, c p%, transparent)` — the colour at alpha p.
pub fn tint(c: [u8; 3], p: f32) -> Color32 {
    rgba(c[0], c[1], c[2], p)
}
/// `color-mix(in srgb, a p%, b)` with b given as rgb + alpha (premultiplied mix, like CSS).
pub fn mix(a: [u8; 3], p: f32, b: [u8; 3], b_alpha: f32) -> Color32 {
    let alpha = p + (1.0 - p) * b_alpha;
    if alpha <= 0.0 {
        return Color32::TRANSPARENT;
    }
    let ch = |i: usize| ((a[i] as f32 * p + b[i] as f32 * (1.0 - p) * b_alpha) / alpha).round().clamp(0.0, 255.0) as u8;
    rgba(ch(0), ch(1), ch(2), alpha)
}
pub fn lerp_color(a: Color32, b: Color32, t: f32) -> Color32 {
    let t = t.clamp(0.0, 1.0);
    let l = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round() as u8;
    Color32::from_rgba_premultiplied(l(a.r(), b.r()), l(a.g(), b.g()), l(a.b(), b.b()), l(a.a(), b.a()))
}
pub fn fade(c: Color32, opacity: f32) -> Color32 {
    c.gamma_multiply(opacity.clamp(0.0, 1.0))
}

/// Same easing as CSS `cubic-bezier(0.22, 1, 0.36, 1)` (close enough).
pub fn ease_out(t: f32) -> f32 {
    1.0 - (1.0 - t.clamp(0.0, 1.0)).powi(4)
}
/// A little overshoot, like `cubic-bezier(0.34, 1.56, 0.64, 1)`.
pub fn ease_back(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    let c1 = 1.70158 * 1.2;
    let c3 = c1 + 1.0;
    1.0 + c3 * (t - 1.0).powi(3) + c1 * (t - 1.0).powi(2)
}

// ── Fonts: Outfit for text, Syne for headings ────────────────────────────

pub fn install_fonts(ctx: &egui::Context) {
    let mut fonts = FontDefinitions::default();
    let fallbacks: Vec<String> = fonts.families.get(&FontFamily::Proportional).cloned().unwrap_or_default();
    let faces: [(&str, &'static [u8]); 6] = [
        ("outfit-400", include_bytes!("../../assets/fonts/Outfit-400.ttf")),
        ("outfit-500", include_bytes!("../../assets/fonts/Outfit-500.ttf")),
        ("outfit-600", include_bytes!("../../assets/fonts/Outfit-600.ttf")),
        ("outfit-700", include_bytes!("../../assets/fonts/Outfit-700.ttf")),
        ("syne-700", include_bytes!("../../assets/fonts/Syne-700.ttf")),
        ("syne-800", include_bytes!("../../assets/fonts/Syne-800.ttf")),
    ];
    for (name, bytes) in faces {
        fonts.font_data.insert(name.into(), Arc::new(FontData::from_static(bytes)));
        let mut family = vec![name.to_string()];
        family.extend(fallbacks.iter().cloned());
        fonts.families.insert(FontFamily::Name(name.into()), family);
    }
    // Plain egui widgets (text boxes) use Outfit too.
    fonts.families.get_mut(&FontFamily::Proportional).unwrap().insert(0, "outfit-400".into());
    ctx.set_fonts(fonts);
}

/// Outfit at a weight (400, 500, 600 or 700).
pub fn font(size: f32, weight: u16) -> FontId {
    let name = match weight {
        0..=449 => "outfit-400",
        450..=549 => "outfit-500",
        550..=649 => "outfit-600",
        _ => "outfit-700",
    };
    FontId::new(size, FontFamily::Name(name.into()))
}
/// Syne, the display face (700 or 800).
pub fn display(size: f32, weight: u16) -> FontId {
    FontId::new(size, FontFamily::Name(if weight >= 800 { "syne-800" } else { "syne-700" }.into()))
}
pub fn mono(size: f32) -> FontId {
    FontId::monospace(size)
}

pub fn install_style(ctx: &egui::Context) {
    let light = is_light();
    ctx.set_theme(if light { egui::Theme::Light } else { egui::Theme::Dark });
    ctx.all_styles_mut(|s| {
        s.visuals = if light { egui::Visuals::light() } else { egui::Visuals::dark() };
        s.visuals.override_text_color = Some(ink());
        s.visuals.panel_fill = Color32::from_rgb(20, 6, 7);
        s.visuals.window_fill = window_bg();
        s.visuals.window_stroke = Stroke::new(1.0_f32, white(0.15));
        s.visuals.extreme_bg_color = Color32::TRANSPARENT;
        s.visuals.selection.bg_fill = if light { rgba(214, 45, 32, 0.3) } else { rgba(245, 183, 168, 0.45) };
        s.visuals.selection.stroke = Stroke::new(1.0_f32, ink());
        s.visuals.text_cursor.stroke = Stroke::new(1.5_f32, ink());
        s.visuals.widgets.inactive.bg_stroke = Stroke::NONE;
        s.visuals.widgets.hovered.bg_stroke = Stroke::NONE;
        s.visuals.widgets.active.bg_stroke = Stroke::NONE;
        s.visuals.widgets.noninteractive.bg_stroke = Stroke::NONE;
        for w in [&mut s.visuals.widgets.inactive, &mut s.visuals.widgets.hovered, &mut s.visuals.widgets.active, &mut s.visuals.widgets.open] {
            w.corner_radius = CornerRadius::same(7);
            w.fg_stroke = Stroke::new(1.0_f32, ink());
        }
        s.spacing.item_spacing = Vec2::new(8.0, 8.0);
        s.spacing.scroll = egui::style::ScrollStyle::thin();
        s.spacing.scroll.bar_width = 6.0;
        s.spacing.button_padding = Vec2::new(10.0, 5.0);
        s.visuals.widgets.inactive.bg_fill = white(0.12);
        s.visuals.widgets.inactive.weak_bg_fill = white(0.12);
        s.visuals.widgets.hovered.bg_fill = white(0.2);
        s.visuals.widgets.hovered.weak_bg_fill = white(0.2);
        s.visuals.widgets.active.bg_fill = white(0.26);
        s.visuals.widgets.active.weak_bg_fill = white(0.26);
        s.visuals.widgets.open.weak_bg_fill = white(0.2);
        s.visuals.popup_shadow = Shadow { offset: [0, 8], blur: 24, spread: 0, color: rgba(0, 0, 0, 0.3) };
        s.interaction.selectable_labels = false;
        s.text_styles.insert(egui::TextStyle::Body, font(14.0, 400));
        s.text_styles.insert(egui::TextStyle::Button, font(13.0, 500));
        s.text_styles.insert(egui::TextStyle::Small, font(12.0, 400));
        s.text_styles.insert(egui::TextStyle::Monospace, mono(12.5));
    });
}

// ── Drawing helpers ───────────────────────────────────────────────────────

pub fn shadow(painter: &egui::Painter, rect: Rect, radius: u8, offset_y: i8, blur: u8, alpha: f32) {
    let s = Shadow { offset: [0, offset_y], blur, spread: 0, color: rgba(0, 0, 0, alpha) };
    painter.add(s.as_shape(rect, CornerRadius::same(radius)));
}

pub fn card(painter: &egui::Painter, rect: Rect, radius: u8, fill: Color32, border: Color32) {
    painter.rect(rect, CornerRadius::same(radius), fill, Stroke::new(1.0_f32, border), StrokeKind::Inside);
}

/// Dashed outline (for "info" sections).
pub fn dashed_rect(painter: &egui::Painter, rect: Rect, radius: f32, color: Color32) {
    let r = radius.min(rect.width() / 2.0).min(rect.height() / 2.0);
    let rect = rect.shrink(0.5);
    let mut pts = Vec::new();
    let corner = |c: Pos2, a0: f32, pts: &mut Vec<Pos2>| {
        for i in 0..=6 {
            let a = a0 + i as f32 / 6.0 * std::f32::consts::FRAC_PI_2;
            pts.push(c + Vec2::angled(a) * r);
        }
    };
    use std::f32::consts::PI;
    corner(Pos2::new(rect.right() - r, rect.top() + r), -PI / 2.0, &mut pts);
    corner(Pos2::new(rect.right() - r, rect.bottom() - r), 0.0, &mut pts);
    corner(Pos2::new(rect.left() + r, rect.bottom() - r), PI / 2.0, &mut pts);
    corner(Pos2::new(rect.left() + r, rect.top() + r), PI, &mut pts);
    pts.push(pts[0]);
    painter.extend(egui::Shape::dashed_line(&pts, Stroke::new(1.0_f32, color), 4.0, 3.0));
}

/// Text cut to fit a width, with an ellipsis.
pub fn elide(ui: &Ui, s: &str, font: &FontId, max_w: f32) -> String {
    let w = |t: &str| ui.fonts(|f| f.layout_no_wrap(t.to_string(), font.clone(), WHITE).size().x);
    if w(s) <= max_w {
        return s.to_string();
    }
    let mut chars: Vec<char> = s.chars().collect();
    while !chars.is_empty() {
        chars.pop();
        let t: String = chars.iter().collect::<String>() + "…";
        if w(&t) <= max_w {
            return t;
        }
    }
    String::new()
}

#[derive(Clone, Copy, PartialEq)]
pub enum Btn {
    /// White pill with navy text.
    Primary,
    /// Frosted pill with a light border.
    Glass,
    /// Faint pill, no border.
    Subtle,
    /// Red tinted pill.
    Danger,
}

/// A pill button. `width` = None sizes it to the text.
pub fn button(ui: &mut Ui, label: &str, kind: Btn, size: f32, width: Option<f32>, pad: Vec2) -> Response {
    button_enabled(ui, label, kind, size, width, pad, true)
}

pub fn button_enabled(ui: &mut Ui, label: &str, kind: Btn, size: f32, width: Option<f32>, pad: Vec2, enabled: bool) -> Response {
    let weight = if kind == Btn::Primary { 600 } else { 500 };
    let f = font(size, weight);
    let galley = ui.fonts(|fo| fo.layout_no_wrap(label.to_string(), f.clone(), Color32::PLACEHOLDER));
    let w = width.unwrap_or(galley.size().x + pad.x * 2.0);
    let h = galley.size().y + pad.y * 2.0;
    let sense = if enabled { Sense::click() } else { Sense::hover() };
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, h), sense);
    if ui.is_rect_visible(rect) {
        let hover = ui.ctx().animate_bool_with_time(resp.id, resp.hovered() && enabled, 0.2);
        let (fill, fill_hover, stroke, color) = match kind {
            Btn::Primary if is_light() => (select_blue(), lerp_color(select_blue(), Color32::BLACK, 0.12), Color32::TRANSPARENT, WHITE),
            Btn::Primary => (WHITE, WHITE, Color32::TRANSPARENT, ACCENT),
            Btn::Glass if is_light() => (WHITE, Color32::from_rgb(0xf2, 0xf2, 0xf4), white(0.22), ink()),
            Btn::Glass => (white(0.18), white(0.26), white(0.25), ink()),
            Btn::Subtle => (white(0.10), white(0.18), Color32::TRANSPARENT, ink()),
            Btn::Danger => (rgba(239, 68, 68, 0.14), rgba(239, 68, 68, 0.24), rgba(239, 68, 68, 0.35), red_text()),
        };
        let lift = if kind == Btn::Primary { hover * 1.0 } else { 0.0 };
        let r = rect.translate(Vec2::new(0.0, -lift));
        let alpha = if enabled { 1.0 } else { 0.4 };
        ui.painter().rect(
            r,
            CornerRadius::same(255),
            fade(lerp_color(fill, fill_hover, hover), alpha),
            Stroke::new(1.0_f32, fade(stroke, alpha)),
            StrokeKind::Inside,
        );
        ui.painter().galley(r.center() - galley.size() / 2.0, galley, fade(color, alpha));
        if enabled && resp.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }
    }
    resp
}

/// A flat clickable row/area with a hover highlight; content is painted by the caller.
pub fn hover_fill(ui: &Ui, resp: &Response, rect: Rect, radius: u8, color: Color32) {
    let t = ui.ctx().animate_bool_with_time(resp.id.with("hf"), resp.hovered(), 0.2);
    if t > 0.0 {
        ui.painter().rect_filled(rect, CornerRadius::same(radius), fade(color, t));
    }
    if resp.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
}

/// A rounded text box like the web inputs. Returns the TextEdit response.
pub fn text_input(ui: &mut Ui, value: &mut String, hint: &str, password: bool, width: f32, id: egui::Id) -> Response {
    let h = 38.0;
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, h), Sense::hover());
    ui.painter().rect(rect, CornerRadius::same(255), lit(0.18), Stroke::new(1.0_f32, lit(0.25)), StrokeKind::Inside);
    let inner = rect.shrink2(Vec2::new(16.0, 9.0));
    let edit = egui::TextEdit::singleline(value)
        .id(id)
        .frame(false)
        .password(password)
        .font(font(14.0, 400))
        .text_color(WHITE)
        .hint_text(egui::RichText::new(hint).color(lit(0.55)).font(font(14.0, 400)))
        .desired_width(inner.width())
        .margin(Vec2::ZERO);
    let resp = ui.put(inner, edit);
    if resp.has_focus() {
        ui.painter().rect_stroke(rect.expand(2.0), CornerRadius::same(255), Stroke::new(2.0_f32, ICE), StrokeKind::Outside);
    }
    resp
}

/// The status dot used in the menu bar, Time Clock and Settings.
pub fn dot(painter: &egui::Painter, center: Pos2, on: bool, glow: bool) {
    if on && glow {
        painter.circle_filled(center, 7.0, rgba(110, 231, 160, 0.18));
    }
    painter.circle_filled(center, 4.0, if on { green() } else { white(0.4) });
}
