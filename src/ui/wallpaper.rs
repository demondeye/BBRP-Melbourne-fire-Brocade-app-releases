//! The desktop background for a user (used by the desktop, login screen and
//! the wallpaper previews in Settings).

use super::theme::rgba;
use crate::settings::{UserSettings, WallpaperPreset};
use eframe::egui::{self, epaint::Mesh, Color32, Painter, Pos2, Rect, Ui};

type Rgba = [f32; 4];

fn to_rgba(c: [u8; 3], a: f32) -> Rgba {
    [c[0] as f32, c[1] as f32, c[2] as f32, a]
}
/// Paint `top` over `bottom` (both straight alpha).
fn over(bottom: Rgba, top: Rgba) -> Rgba {
    let a = top[3] + bottom[3] * (1.0 - top[3]);
    if a <= 0.0 {
        return [0.0; 4];
    }
    let ch = |i: usize| (top[i] * top[3] + bottom[i] * bottom[3] * (1.0 - top[3])) / a;
    [ch(0), ch(1), ch(2), a]
}

/// Colour of a CSS linear-gradient at a point.
fn linear(angle: f32, stops: &[(f32, [u8; 3], f32)], rect: Rect, p: Pos2) -> Rgba {
    let a = angle.to_radians();
    let dir = egui::vec2(a.sin(), -a.cos());
    let len = (rect.width() * a.sin()).abs() + (rect.height() * a.cos()).abs();
    let t = ((p - rect.center()).dot(dir) / len + 0.5).clamp(0.0, 1.0);
    for w in stops.windows(2) {
        let (t0, c0, a0) = w[0];
        let (t1, c1, a1) = w[1];
        if t <= t1 {
            let f = if t1 > t0 { ((t - t0) / (t1 - t0)).clamp(0.0, 1.0) } else { 1.0 };
            return [
                c0[0] as f32 + (c1[0] as f32 - c0[0] as f32) * f,
                c0[1] as f32 + (c1[1] as f32 - c0[1] as f32) * f,
                c0[2] as f32 + (c1[2] as f32 - c0[2] as f32) * f,
                a0 + (a1 - a0) * f,
            ];
        }
    }
    let (_, c, a) = stops[stops.len() - 1];
    to_rgba(c, a)
}

/// CSS `radial-gradient(circle at cx cy, color, transparent end)`.
fn radial(rect: Rect, p: Pos2, cx: f32, cy: f32, color: [u8; 3], alpha: f32, end: f32) -> Rgba {
    let c = rect.min + egui::vec2(rect.width() * cx, rect.height() * cy);
    let far = [rect.left_top(), rect.right_top(), rect.left_bottom(), rect.right_bottom()].iter().map(|q| q.distance(c)).fold(0.0, f32::max);
    let t = (p.distance(c) / (far * end)).clamp(0.0, 1.0);
    to_rgba(color, alpha * (1.0 - t))
}

/// Paints a built-in wallpaper, with the soft highlights and glass sheen on top.

/// Same, with rounded corners.
pub fn paint_preset_rounded(painter: &Painter, rect: Rect, preset: &WallpaperPreset, sheen: bool, radius: f32) {
    let stops: Vec<(f32, [u8; 3], f32)> = preset.stops.iter().map(|(t, c)| (*t, super::theme::hex(*c), 1.0)).collect();
    let (nx, ny) = (36usize, 24usize);
    let mut mesh = Mesh::default();
    for j in 0..=ny {
        for i in 0..=nx {
            let p = round_corner(rect, radius, rect.min + egui::vec2(rect.width() * i as f32 / nx as f32, rect.height() * j as f32 / ny as f32));
            let mut c = linear(preset.angle, &stops, rect, p);
            c = over(c, radial(rect, p, 0.75, 0.45, [162, 203, 237], 0.28, 0.45));
            c = over(c, radial(rect, p, 0.15, 0.85, [255, 255, 255], 0.10, 0.40));
            if sheen {
                c = over(c, linear(135.0, &[(0.0, [162, 203, 237], 0.2), (0.55, [162, 203, 237], 0.0)], rect, p));
            }
            mesh.colored_vertex(p, rgba(c[0].round() as u8, c[1].round() as u8, c[2].round() as u8, c[3]));
        }
    }
    let w = (nx + 1) as u32;
    for j in 0..ny as u32 {
        for i in 0..nx as u32 {
            let a = j * w + i;
            mesh.add_triangle(a, a + 1, a + w);
            mesh.add_triangle(a + 1, a + w + 1, a + w);
        }
    }
    painter.add(mesh);
}

/// Pulls a point that falls outside a rounded corner back onto the curve.
fn round_corner(rect: Rect, r: f32, p: Pos2) -> Pos2 {
    if r <= 0.0 {
        return p;
    }
    let cx = if p.x < rect.left() + r { rect.left() + r } else if p.x > rect.right() - r { rect.right() - r } else { return p };
    let cy = if p.y < rect.top() + r { rect.top() + r } else if p.y > rect.bottom() - r { rect.bottom() - r } else { return p };
    let c = Pos2::new(cx, cy);
    let d = p - c;
    if d.length() > r { c + d.normalized() * r } else { p }
}

/// Paints a picture so it covers the whole rect (cropping what doesn't fit).
pub fn paint_image_cover_rounded(ui: &Ui, painter: &Painter, rect: Rect, path: &str, radius: u8) -> bool {
    paint_image(ui, painter, rect, path, "fill", [0, 0, 0], radius, 1.0)
}

fn textured(rect: Rect, radius: u8, texture: egui::TextureId, uv: Rect) -> egui::epaint::RectShape {
    let mut shape = egui::epaint::RectShape::filled(rect, radius, Color32::WHITE);
    shape.brush = Some(std::sync::Arc::new(egui::epaint::Brush { fill_texture_id: texture, uv }));
    shape
}

/// Paints the user's picture, sized like a Mac desktop picture:
/// - `fill`    covers the whole area, cropping what doesn't fit
/// - `fit`     shows the whole picture, with `bg` around it
/// - `stretch` stretches it to the area's shape
/// - `center`  shows it at its real size in the middle, with `bg` around it
/// - `tile`    repeats it at its real size
/// `scale` shrinks "real size" for small previews (preview width ÷ screen width).
#[allow(clippy::too_many_arguments)]
pub fn paint_image(ui: &Ui, painter: &Painter, rect: Rect, path: &str, fit: &str, bg: [u8; 3], radius: u8, scale: f32) -> bool {
    let uri = format!("file://{}", path.replace('\\', "/"));
    let options = if fit == "tile" { egui::TextureOptions::LINEAR_REPEAT } else { egui::TextureOptions::LINEAR };
    let texture = match ui.ctx().try_load_texture(&uri, options, egui::SizeHint::default()) {
        Ok(egui::load::TexturePoll::Ready { texture }) => texture,
        Ok(egui::load::TexturePoll::Pending { .. }) => {
            ui.ctx().request_repaint();
            return false;
        }
        Err(_) => return false,
    };
    let (iw, ih) = (texture.size.x.max(1.0), texture.size.y.max(1.0));
    // The picture's real size on screen, in points.
    let ppp = ui.ctx().pixels_per_point();
    let real = egui::vec2(iw / ppp, ih / ppp) * scale;
    let full_uv = Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0));
    let clipped = painter.with_clip_rect(rect.intersect(painter.clip_rect()));
    match fit {
        "stretch" => {
            painter.add(textured(rect, radius, texture.id, full_uv));
        }
        "fit" => {
            painter.rect_filled(rect, radius, super::theme::rgb(bg));
            let k = (rect.width() / iw).min(rect.height() / ih);
            let r = Rect::from_center_size(rect.center(), egui::vec2(iw * k, ih * k));
            painter.add(textured(r, 0, texture.id, full_uv));
        }
        "center" => {
            painter.rect_filled(rect, radius, super::theme::rgb(bg));
            let r = Rect::from_center_size(rect.center(), real);
            clipped.add(textured(r, 0, texture.id, full_uv));
        }
        "tile" => {
            // Tiles start from the top-left corner, like a Mac.
            let uv = Rect::from_min_max(Pos2::ZERO, Pos2::new(rect.width() / real.x.max(1.0), rect.height() / real.y.max(1.0)));
            painter.add(textured(rect, radius, texture.id, uv));
        }
        _ => {
            let k = (rect.width() / iw).max(rect.height() / ih);
            let (vw, vh) = (rect.width() / k / iw, rect.height() / k / ih);
            let uv = Rect::from_min_max(Pos2::new((1.0 - vw) / 2.0, (1.0 - vh) / 2.0), Pos2::new((1.0 + vw) / 2.0, (1.0 + vh) / 2.0));
            painter.add(textured(rect, radius, texture.id, uv));
        }
    }
    true
}

/// The background for these settings: the user's picture, or a built-in wallpaper.
pub fn paint(ui: &Ui, painter: &Painter, rect: Rect, settings: &UserSettings, sheen: bool) {
    paint_scaled(ui, painter, rect, settings, sheen, 0, 1.0)
}

/// Same, for previews: rounded corners, and real-size pictures shrunk by `scale`.
pub fn paint_scaled(ui: &Ui, painter: &Painter, rect: Rect, settings: &UserSettings, sheen: bool, radius: u8, scale: f32) {
    if settings.using_image() {
        painter.rect_filled(rect, radius, super::theme::rgb(settings.fill_color));
        if paint_image(ui, painter, rect, settings.custom_image.as_deref().unwrap_or_default(), &settings.fit, settings.fill_color, radius, scale) {
            return;
        }
    }
    paint_preset_rounded(painter, rect, settings.preset(), sheen, radius as f32);
}

/// A rounded rectangle filled with a CSS-style linear gradient (angle in degrees).
pub fn gradient_rect(painter: &Painter, rect: Rect, radius: f32, angle: f32, from: Color32, to: Color32) {
    let [r0, g0, b0, a0] = super::theme::unmul(from);
    let [r1, g1, b1, a1] = super::theme::unmul(to);
    let stops = [(0.0, [r0, g0, b0], a0 as f32 / 255.0), (1.0, [r1, g1, b1], a1 as f32 / 255.0)];
    let (nx, ny) = (28usize, 14usize);
    let mut mesh = Mesh::default();
    for j in 0..=ny {
        for i in 0..=nx {
            let p = round_corner(rect, radius, rect.min + egui::vec2(rect.width() * i as f32 / nx as f32, rect.height() * j as f32 / ny as f32));
            let c = linear(angle, &stops, rect, p);
            mesh.colored_vertex(p, rgba(c[0].round() as u8, c[1].round() as u8, c[2].round() as u8, c[3]));
        }
    }
    let w = (nx + 1) as u32;
    for j in 0..ny as u32 {
        for i in 0..nx as u32 {
            let a = j * w + i;
            mesh.add_triangle(a, a + 1, a + w);
            mesh.add_triangle(a + 1, a + w + 1, a + w);
        }
    }
    painter.add(mesh);
}
