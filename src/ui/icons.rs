//! Icons: the same line drawings as the web version (drawn as SVG), the logo,
//! and the colour emoji pictures used by the Field Guide.

use eframe::egui::{self, Color32, Image, Ui, Vec2};

fn hex(c: Color32) -> String {
    format!("#{:02x}{:02x}{:02x}", c.r(), c.g(), c.b())
}

/// A 24×24 line icon (path data in the web version's format).
pub fn line_icon(ui: &mut Ui, name: &str, paths: &str, size: f32, color: Color32, stroke_width: f32) -> egui::Response {
    let svg = format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" width="{s}" height="{s}" fill="none" stroke="{c}" stroke-width="{w}" stroke-linecap="round" stroke-linejoin="round">{paths}</svg>"#,
        s = size,
        c = hex(color),
        w = stroke_width
    );
    let uri = format!("bytes://icon/{name}-{}-{size}-{stroke_width}.svg", hex(color));
    ui.add(Image::from_bytes(uri, svg.into_bytes()).fit_to_exact_size(Vec2::splat(size)))
}

/// The app icons (from the registry in the web version).
pub fn app_icon_paths(id: &str) -> &'static str {
    match id {
        "timeclock" => r#"<circle cx="12" cy="12" r="9"/><path d="M12 7v5l3 2"/>"#,
        "fieldguide" => r#"<path d="M5 4h11a3 3 0 0 1 3 3v13H8a3 3 0 0 1-3-3z"/><path d="M5 17a3 3 0 0 1 3-3h11M12 7v4M10 9h4"/>"#,
        "records" => r#"<path d="M6 3h9l4 4v14H6z"/><path d="M14 3v5h5M9 13h6M9 17h6"/>"#,
        "notes" => r#"<path d="M5 4h14v12l-5 5H5z"/><path d="M14 21v-5h5M8 9h8M8 13h5"/>"#,
        "settings" => r#"<circle cx="12" cy="12" r="3"/><path d="M12 3v3M12 18v3M3 12h3M18 12h3M5.6 5.6l2.1 2.1M16.3 16.3l2.1 2.1M5.6 18.4l2.1-2.1M16.3 7.7l2.1-2.1"/>"#,
        "about" => r#"<circle cx="12" cy="12" r="9"/><path d="M12 11v6M12 7.5v.5"/>"#,
        "map" => r#"<path d="M3 6l6-2 6 2 6-2v14l-6 2-6-2-6 2z"/><path d="M9 4v14M15 6v14"/>"#,
        _ => r#"<rect x="4" y="4" width="16" height="16" rx="3"/>"#,
    }
}

pub const ARROW: &str = r#"<path d="M5 12h14M13 6l6 6-6 6"/>"#;
pub const PENCIL: &str = r#"<path d="M4 20h4L19 9l-4-4L4 16z M13 7l4 4"/>"#;
pub const BIN: &str = r#"<path d="M4 7h16M9 7V4h6v3M6 7l1 13h10l1-13"/>"#;
pub const CHEVRON: &str = r#"<path d="M6 9l6 6 6-6"/>"#;
pub const WALLPAPER: &str = r#"<rect x="3" y="5" width="18" height="14" rx="2"/><path d="M3 16l5-5 4 4 3-3 6 6"/>"#;
pub const STREAMDECK: &str = r#"<rect x="3" y="5" width="18" height="14" rx="2"/><path d="M7 9h2M11 9h2M15 9h2M7 13h2M11 13h2M15 13h2"/>"#;
pub const UPDATE: &str = r#"<circle cx="12" cy="12" r="9"/><path d="M12 7v9M8 12.5l4 4 4-4"/>"#;
pub const ACCOUNT: &str = r#"<circle cx="12" cy="8" r="4"/><path d="M4 21c1.5-4 4.5-6 8-6s6.5 2 8 6"/>"#;

/// The launcher's nine dots.
pub fn launcher_dots(ui: &mut Ui, size: f32, color: Color32) {
    let mut circles = String::new();
    for y in [4, 12, 20] {
        for x in [4, 12, 20] {
            circles += &format!(r#"<circle cx="{x}" cy="{y}" r="2.2"/>"#);
        }
    }
    let svg = format!(r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" width="{size}" height="{size}" fill="{}">{circles}</svg>"#, hex(color));
    ui.add(Image::from_bytes(format!("bytes://icon/launcher-{size}.svg"), svg.into_bytes()).fit_to_exact_size(Vec2::splat(size)));
}

/// Original fictional mark — a flame in a white disc.
pub fn logo(ui: &mut Ui, size: f32) -> egui::Response {
    const SVG: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" width="64" height="64" viewBox="0 0 64 64"><circle cx="32" cy="32" r="30" fill="#fff"/><path d="M32 10c2 9 13 14 13 27a13 13 0 0 1-26 0c0-7 4-11 7-14 0 5 2 8 5 9-1-9 1-16 1-22z" fill="#c8281e"/><path d="M32 33c1 4 6 6 6 11a6 6 0 0 1-12 0c0-4 3-6 4-8 0 2 1 3 2 3z" fill="#f0a35e"/></svg>"##;
    ui.add(Image::from_bytes("bytes://logo.svg", SVG.as_bytes()).fit_to_exact_size(Vec2::splat(size)))
}

pub fn logo_at(ui: &Ui, rect: egui::Rect) {
    const SVG: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" width="64" height="64" viewBox="0 0 64 64"><circle cx="32" cy="32" r="30" fill="#fff"/><path d="M32 10c2 9 13 14 13 27a13 13 0 0 1-26 0c0-7 4-11 7-14 0 5 2 8 5 9-1-9 1-16 1-22z" fill="#c8281e"/><path d="M32 33c1 4 6 6 6 11a6 6 0 0 1-12 0c0-4 3-6 4-8 0 2 1 3 2 3z" fill="#f0a35e"/></svg>"##;
    Image::from_bytes("bytes://logo.svg", SVG.as_bytes()).paint_at(ui, rect);
}

fn emoji_bytes(code: &str) -> &'static [u8] {
    macro_rules! e {
        ($($c:literal),*) => {
            match code {
                $($c => include_bytes!(concat!("../../assets/emoji/", $c, ".png")),)*
                _ => include_bytes!("../../assets/emoji/1f4cb.png"),
            }
        };
    }
    e!("1f9b4", "1f30a", "1fa79", "1f48a", "1f3af", "1f525", "1f697", "1f9e0", "1f493", "1f634", "1f489", "1f4fb", "1f399", "1f4ac", "1f691", "1f3e0", "1f4cb", "1f389")
}

/// A colour emoji picture (Twemoji), by its code.
pub fn emoji_image(code: &str) -> Image<'static> {
    Image::from_bytes(format!("bytes://emoji/{code}.png"), emoji_bytes(code))
}
