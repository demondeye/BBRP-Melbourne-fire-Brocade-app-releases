//! Map — an interactive map of the city with every postal written on it.
//!
//! Drag to move, scroll to zoom, double-click to zoom in, right-click to drop a
//! waypoint. Type a postal in the search box to jump to it. The map pictures
//! are built into the app (satellite and atlas, two levels of detail), and the
//! postals come from `src/postals.rs`.

use crate::postals::{PARTLY_HIDDEN, POSTALS};
use crate::streets::{self, Place};
use crate::ui::theme::*;
use eframe::egui::{self, Align2, Color32, ColorImage, CornerRadius, Pos2, Rect, Sense, Stroke, StrokeKind, TextureHandle, TextureOptions, Ui, Vec2};
use std::collections::HashMap;

/// The whole map is this many map units across (and down).
const WORLD: f32 = 4096.0;
/// The most zoomed-in view: screen points per map unit.
const MAX_SCALE: f32 = 8.0;
/// Postal number height on the map, in map units (same size as in game).
const LABEL_SIZE: f32 = 11.0;
/// Tiles decoded per frame, so panning stays smooth.
const DECODE_PER_FRAME: usize = 10;
/// Tiles kept in memory before old ones are dropped.
const KEEP_TILES: usize = 450;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Layer {
    Satellite,
    Atlas,
}

struct Pack {
    n: u32,
    bytes: &'static [u8],
}

impl Pack {
    fn tile(&self, x: u32, y: u32) -> Option<&'static [u8]> {
        let i = (x * self.n + y) as usize;
        let at = 4 + i * 8;
        let rd = |o: usize| u32::from_le_bytes(self.bytes[o..o + 4].try_into().unwrap()) as usize;
        let (off, len) = (rd(at), rd(at + 4));
        if len == 0 {
            return None;
        }
        let data = 4 + (self.n * self.n) as usize * 8;
        Some(&self.bytes[data + off..data + off + len])
    }
}

fn pack(layer: Layer, z: u8) -> Pack {
    let bytes: &'static [u8] = match (layer, z) {
        (Layer::Satellite, 4) => include_bytes!("../../../assets/map/satellite-z4.bin"),
        (Layer::Satellite, _) => include_bytes!("../../../assets/map/satellite-z5.bin"),
        (Layer::Atlas, 4) => include_bytes!("../../../assets/map/atlas-z4.bin"),
        (Layer::Atlas, _) => include_bytes!("../../../assets/map/atlas-z5.bin"),
    };
    Pack { n: u32::from_le_bytes(bytes[0..4].try_into().unwrap()), bytes }
}

type TileKey = (Layer, u8, u32, u32);

pub struct MapUi {
    /// Map point in the middle of the window.
    center: Pos2,
    /// Screen points per map unit. 0 = not set up yet (fit on first show).
    scale: f32,
    layer: Layer,
    show_postals: bool,
    search: String,
    message: Option<String>,
    /// Postal index being shown, and when it was picked (for the pulse).
    found: Option<(usize, f64)>,
    /// Right-click pin, in map units.
    waypoint: Option<Pos2>,
    /// Smooth fly-to: (from centre, from scale, to centre, to scale, start time).
    flight: Option<(Pos2, f32, Pos2, f32, f64)>,
    /// Where each postal number was drawn last frame (for clicking on one).
    label_rects: Vec<(Rect, usize)>,
    /// Street details for the pinged postal and the waypoint, worked out once.
    found_place: Option<(usize, Option<Place>)>,
    waypoint_place: Option<(Pos2, Option<Place>)>,
    tiles: HashMap<TileKey, (TextureHandle, u64)>,
    frame: u64,
}

impl Default for MapUi {
    fn default() -> Self {
        Self {
            center: Pos2::new(WORLD / 2.0, WORLD / 2.0),
            scale: 0.0,
            layer: Layer::Satellite,
            show_postals: true,
            search: String::new(),
            message: None,
            found: None,
            waypoint: None,
            flight: None,
            label_rects: Vec::new(),
            found_place: None,
            waypoint_place: None,
            tiles: HashMap::new(),
            frame: 0,
        }
    }
}

/// The postal nearest a map point.
pub fn nearest_postal(p: Pos2) -> Option<(&'static str, f32)> {
    POSTALS
        .iter()
        .map(|&(code, x, y)| (code, (Pos2::new(x, y) - p).length()))
        .min_by(|a, b| a.1.total_cmp(&b.1))
}

fn text_outlined(painter: &egui::Painter, pos: Pos2, text: &str, f: egui::FontId, fill: Color32, outline: Color32) {
    let galley = painter.layout_no_wrap(text.to_string(), f.clone(), fill);
    let at = pos - galley.size() / 2.0;
    let w = (f.size / 9.0).clamp(1.0, 2.5);
    for (dx, dy) in [(-w, 0.0), (w, 0.0), (0.0, -w), (0.0, w), (-w, -w), (w, w), (-w, w), (w, -w)] {
        painter.galley_with_override_text_color(at + Vec2::new(dx, dy), galley.clone(), outline);
    }
    painter.galley(at, galley, fill);
}

impl MapUi {
    fn min_scale(view: Rect) -> f32 {
        (view.width().min(view.height()) / WORLD) * 0.95
    }

    fn to_screen(&self, view: Rect, p: Pos2) -> Pos2 {
        view.center() + (p - self.center) * self.scale
    }
    fn to_map(&self, view: Rect, s: Pos2) -> Pos2 {
        self.center + (s - view.center()) / self.scale
    }

    fn clamp(&mut self, view: Rect) {
        let min = Self::min_scale(view);
        self.scale = self.scale.clamp(min, MAX_SCALE);
        let half = view.size() / (2.0 * self.scale);
        let cx = if half.x * 2.0 >= WORLD { WORLD / 2.0 } else { self.center.x.clamp(half.x, WORLD - half.x) };
        let cy = if half.y * 2.0 >= WORLD { WORLD / 2.0 } else { self.center.y.clamp(half.y, WORLD - half.y) };
        self.center = Pos2::new(cx, cy);
    }

    /// Zoom by `factor`, keeping the map point under `anchor` (screen) still.
    fn zoom_at(&mut self, view: Rect, anchor: Pos2, factor: f32) {
        let before = self.to_map(view, anchor);
        self.scale = (self.scale * factor).clamp(Self::min_scale(view), MAX_SCALE);
        let after = self.to_map(view, anchor);
        self.center += before - after;
        self.flight = None;
    }

    /// Go to a postal from outside (voice control).
    pub fn go_code(&mut self, code: &str, now: f64) {
        self.search = code.to_string();
        self.found = None;
        self.go_to_postal(now);
    }

    fn go_to_postal(&mut self, now: f64) {
        let code = self.search.trim().to_string();
        self.message = None;
        if code.is_empty() {
            return;
        }
        // Several postals can share a number: pressing Enter again moves to the next one.
        let matches: Vec<usize> = POSTALS.iter().enumerate().filter(|(_, p)| p.0 == code).map(|(i, _)| i).collect();
        if matches.is_empty() {
            self.message = Some(format!("Postal {code} isn't on the map."));
            return;
        }
        let next = match self.found {
            Some((i, _)) => matches.iter().position(|&m| m == i).map_or(matches[0], |k| matches[(k + 1) % matches.len()]),
            None => matches[0],
        };
        self.found = Some((next, now));
        let (_, x, y) = POSTALS[next];
        self.flight = Some((self.center, self.scale, Pos2::new(x, y), self.scale.max(1.6), now));
        if matches.len() > 1 {
            self.message = Some(format!("Postal {code} is on the map {} times — press Enter for the next one.", matches.len()));
        }
    }

    fn texture(&mut self, ctx: &egui::Context, key: TileKey, budget: &mut usize) -> Option<egui::TextureId> {
        let frame = self.frame;
        if let Some((tex, used)) = self.tiles.get_mut(&key) {
            *used = frame;
            return Some(tex.id());
        }
        if *budget == 0 {
            return None;
        }
        *budget -= 1;
        let (layer, z, x, y) = key;
        let bytes = pack(layer, z).tile(x, y)?;
        let img = image::load_from_memory_with_format(bytes, image::ImageFormat::Jpeg).ok()?.to_rgba8();
        let size = [img.width() as usize, img.height() as usize];
        let color = ColorImage::from_rgba_unmultiplied(size, img.as_raw());
        let tex = ctx.load_texture(format!("map-{layer:?}-{z}-{x}-{y}"), color, TextureOptions::LINEAR);
        let id = tex.id();
        self.tiles.insert(key, (tex, frame));
        Some(id)
    }

    fn drop_old_tiles(&mut self) {
        if self.tiles.len() <= KEEP_TILES {
            return;
        }
        let mut ages: Vec<(TileKey, u64)> = self.tiles.iter().map(|(k, v)| (*k, v.1)).collect();
        ages.sort_by_key(|a| a.1);
        for (k, _) in ages.into_iter().take(self.tiles.len() - KEEP_TILES) {
            self.tiles.remove(&k);
        }
    }

    /// Paint the tiles of one zoom level that are on screen. Returns true if some are still loading.
    fn paint_level(&mut self, ui: &Ui, painter: &egui::Painter, view: Rect, z: u8, budget: &mut usize) -> bool {
        let n = 1u32 << z;
        let size = WORLD / n as f32;
        let tl = self.to_map(view, view.min);
        let br = self.to_map(view, view.max);
        let x0 = ((tl.x / size).floor().max(0.0)) as u32;
        let y0 = ((tl.y / size).floor().max(0.0)) as u32;
        let x1 = ((br.x / size).ceil().min(n as f32)) as u32;
        let y1 = ((br.y / size).ceil().min(n as f32)) as u32;
        let mut pending = false;
        let uv = Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0));
        for x in x0..x1 {
            for y in y0..y1 {
                let a = self.to_screen(view, Pos2::new(x as f32 * size, y as f32 * size));
                let b = self.to_screen(view, Pos2::new((x + 1) as f32 * size, (y + 1) as f32 * size));
                // A hair of overlap hides seams between tiles.
                let r = Rect::from_min_max(a, b).expand(0.35);
                match self.texture(ui.ctx(), (self.layer, z, x, y), budget) {
                    Some(id) => {
                        painter.image(id, r, uv, Color32::WHITE);
                    }
                    None => pending = true,
                }
            }
        }
        pending
    }
}

pub fn show(ui: &mut Ui, st: &mut MapUi, voice: &crate::voice::VoiceStatus, settings: &crate::settings::UserSettings, now: f64, radius: u8) {
    st.frame += 1;
    let view = ui.max_rect();
    let resp = ui.allocate_rect(view, Sense::click_and_drag());
    if st.scale == 0.0 {
        st.scale = MapUi::min_scale(view);
    }

    // ── Moving around ──────────────────────────────────────────────────
    if resp.dragged_by(egui::PointerButton::Primary) || resp.dragged_by(egui::PointerButton::Middle) {
        st.center -= resp.drag_delta() / st.scale;
        st.flight = None;
        ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
    } else if resp.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::Grab);
    }
    if let Some(hover) = resp.hover_pos() {
        let (scroll, zoom) = ui.input(|i| (i.smooth_scroll_delta.y, i.zoom_delta()));
        if scroll != 0.0 {
            st.zoom_at(view, hover, (scroll / 300.0).exp());
        }
        if zoom != 1.0 {
            st.zoom_at(view, hover, zoom);
        }
    }
    if resp.double_clicked() {
        if let Some(p) = resp.interact_pointer_pos() {
            st.flight = Some((st.center, st.scale, st.to_map(view, p), (st.scale * 2.0).min(MAX_SCALE), now));
        }
    }
    // Click a postal number to ping it.
    if resp.clicked() {
        if let Some(p) = resp.interact_pointer_pos() {
            if let Some(&(_, i)) = st.label_rects.iter().find(|(r, _)| r.expand(3.0).contains(p)) {
                st.found = Some((i, now));
                st.search = POSTALS[i].0.to_string();
                st.message = None;
            }
        }
    }
    if resp.secondary_clicked() {
        if let Some(p) = resp.interact_pointer_pos() {
            st.waypoint = Some(st.to_map(view, p));
        }
    }
    if let Some((c0, s0, c1, s1, t0)) = st.flight {
        let t = (((now - t0) / 0.6) as f32).clamp(0.0, 1.0);
        let e = ease_out(t);
        // Fly in screen space so the zoom feels even.
        st.scale = s0 * (s1 / s0).powf(e);
        st.center = c0 + (c1 - c0) * e;
        if t >= 1.0 {
            st.flight = None;
        }
        ui.ctx().request_repaint();
    }
    st.clamp(view);

    // ── The map ────────────────────────────────────────────────────────
    let painter = ui.painter_at(view);
    painter.rect_filled(view, CornerRadius::ZERO, Color32::from_rgb(19, 60, 104));
    let mut budget = DECODE_PER_FRAME;
    let ppp = ui.ctx().pixels_per_point();
    let mut pending = st.paint_level(ui, &painter, view, 4, &mut budget);
    if st.scale * ppp > 1.25 {
        pending |= st.paint_level(ui, &painter, view, 5, &mut budget);
    }
    if pending {
        ui.ctx().request_repaint();
    }
    st.drop_old_tiles();

    // ── Postals ────────────────────────────────────────────────────────
    if st.show_postals {
        let size = (LABEL_SIZE * st.scale).clamp(9.0, 20.0);
        let f = font(size, 700);
        // Hide numbers that would sit on top of one another when zoomed out.
        let mut placed: Vec<Rect> = Vec::new();
        st.label_rects.clear();
        let cw = size * 0.62;
        let found = st.found.map(|f| f.0);
        let order = found.into_iter().chain((0..POSTALS.len()).filter(|i| Some(*i) != found));
        for i in order {
            let (code, x, y) = POSTALS[i];
            let s = st.to_screen(view, Pos2::new(x, y));
            let r = Rect::from_center_size(s, Vec2::new(cw * code.len() as f32 + 2.0, size * 0.9));
            if !view.expand(40.0).intersects(r) || placed.iter().any(|o| o.intersects(r)) {
                continue;
            }
            placed.push(r);
            st.label_rects.push((r, i));
            let hit = Some(i) == found;
            let fill = if hit { Color32::from_rgb(0xff, 0xd2, 0x4a) } else { Color32::WHITE };
            text_outlined(&painter, s, code, f.clone(), fill, Color32::BLACK);
        }
    }

    if !st.show_postals {
        st.label_rects.clear();
    }
    if resp.hovered() && st.label_rects.iter().any(|(r, _)| resp.hover_pos().is_some_and(|h| r.expand(3.0).contains(h))) {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }

    // Street details, worked out when the pinged postal or the waypoint changes.
    if let Some((i, _)) = st.found {
        if st.found_place.as_ref().is_none_or(|f| f.0 != i) {
            let (_, x, y) = POSTALS[i];
            st.found_place = Some((i, streets::describe(x, y)));
            crate::voice::set_last_postal(POSTALS[i].0);
        }
    } else {
        st.found_place = None;
    }
    match st.waypoint {
        Some(w) if st.waypoint_place.as_ref().is_none_or(|f| f.0 != w) => st.waypoint_place = Some((w, streets::describe(w.x, w.y))),
        None => st.waypoint_place = None,
        _ => {}
    }

    // Mark the corner / intersection of the pinged postal.
    if let Some((_, Some(place))) = &st.found_place {
        if let Some(c) = &place.crossing {
            let s = st.to_screen(view, Pos2::new(c.at.0, c.at.1));
            painter.circle_filled(s, 7.0, Color32::from_rgb(0xff, 0xd2, 0x4a));
            painter.circle_stroke(s, 7.0, Stroke::new(2.5_f32, Color32::BLACK));
        }
    }

    // The postal you searched for pulses.
    if let Some((i, t0)) = st.found {
        let (_, x, y) = POSTALS[i];
        let s = st.to_screen(view, Pos2::new(x, y));
        let t = ((now - t0) % 1.6 / 1.6) as f32;
        let r = 14.0 + t * 28.0;
        painter.circle_stroke(s, r, Stroke::new(3.0_f32, fade(Color32::from_rgb(0xff, 0xd2, 0x4a), 1.0 - t)));
        if now - t0 < 8.0 {
            ui.ctx().request_repaint();
        }
    }

    // Waypoint pin.
    if let Some(w) = st.waypoint {
        let s = st.to_screen(view, w);
        let red = Color32::from_rgb(0xe0, 0x2b, 0x20);
        painter.circle_filled(s + Vec2::new(0.0, -18.0), 9.0, red);
        painter.add(egui::Shape::convex_polygon(vec![s + Vec2::new(-7.0, -14.0), s + Vec2::new(7.0, -14.0), s], red, Stroke::NONE));
        painter.circle_filled(s + Vec2::new(0.0, -18.0), 3.5, Color32::WHITE);
    }

    // ── Controls on top of the map ─────────────────────────────────────
    let pad = 12.0;
    let narrow = view.width() < 560.0;

    // Search, top left.
    let search_w = if narrow { view.width() - pad * 2.0 } else { 300.0 };
    let bar = Rect::from_min_size(view.min + Vec2::splat(pad), Vec2::new(search_w, 44.0));
    shadow(&painter, bar, 255, 4, 14, 0.35);
    painter.rect(bar, CornerRadius::same(255), window_bg(), Stroke::new(1.0_f32, white(0.18)), StrokeKind::Inside);
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(bar.shrink2(Vec2::new(6.0, 3.0))).layout(egui::Layout::left_to_right(egui::Align::Center)));
    child.add_space(10.0);
    child.label(egui::RichText::new("Postal").font(font(13.0, 600)).color(white(0.6)));
    child.add_space(4.0);
    let go_w = 52.0;
    let edit = egui::TextEdit::singleline(&mut st.search)
        .id(egui::Id::new("map-search"))
        .frame(false)
        .font(font(16.0, 600))
        .char_limit(5)
        .hint_text(egui::RichText::new("e.g. 361").color(white(0.4)).font(font(16.0, 400)))
        .desired_width((child.available_width() - go_w - 8.0).max(40.0))
        .margin(Vec2::ZERO);
    let er = child.add(edit);
    st.search.retain(|c| c.is_ascii_digit());
    let enter = er.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
    if er.changed() {
        st.message = None;
    }
    let go = button(&mut child, "Go", Btn::Primary, 13.0, Some(go_w), Vec2::new(0.0, 8.0)).clicked();
    if enter || go {
        st.go_to_postal(now);
        if enter {
            er.request_focus();
        }
    }
    if let Some(msg) = &st.message {
        let r = Rect::from_min_size(bar.left_bottom() + Vec2::new(0.0, 8.0), Vec2::new(search_w, 0.0));
        let g = painter.layout(msg.clone(), font(13.0, 500), WHITE, search_w - 24.0);
        let box_r = Rect::from_min_size(r.min, Vec2::new(search_w, g.size().y + 16.0));
        painter.rect_filled(box_r, CornerRadius::same(12), fade(Color32::from_rgb(0x2e, 0x0b, 0x0d), 0.92));
        painter.galley(box_r.min + Vec2::new(12.0, 8.0), g, WHITE);
    }

    // Voice control status, under the search box.
    {
        use crate::voice::VoiceState;
        let key = crate::voice::key_name(settings.voice_key);
        let red = Color32::from_rgb(0xe0, 0x2b, 0x20);
        let (dot, text): (Option<Color32>, String) = match &voice.state {
            VoiceState::Off => (None, String::new()),
            VoiceState::NoMic => (Some(white(0.4)), "Voice off — no microphone detected".into()),
            VoiceState::Downloading(p) => (Some(white(0.5)), format!("Getting voice control ready… {}%", (p * 100.0) as u32)),
            VoiceState::Waiting if settings.voice_mode == "both" => (Some(white(0.5)), format!("Say “Fire Com dispatch” or press {key}")),
            VoiceState::Waiting => (Some(white(0.5)), format!("Press {key} and say a postal")),
            VoiceState::Listening => (Some(red), "Listening for a postal…".into()),
            VoiceState::Speaking => (Some(Color32::from_rgb(0xff, 0xd2, 0x4a)), "Dispatch".into()),
            VoiceState::Problem(e) => (Some(red), format!("Voice: {e}")),
        };
        if let Some(dot) = dot {
            let mut text = text;
            if let Some(h) = &voice.heard {
                if !matches!(voice.state, VoiceState::Problem(_)) {
                    text = format!("{text}\nHeard {h}");
                }
            }
            let top = bar.bottom() + if st.message.is_some() { 70.0 } else { 10.0 };
            let g = painter.layout(text, font(12.5, 500), WHITE, search_w - 40.0);
            let r = Rect::from_min_size(Pos2::new(bar.left(), top), Vec2::new(g.size().x + 40.0, g.size().y + 14.0));
            painter.rect_filled(r, CornerRadius::same(12), fade(Color32::from_rgb(0x14, 0x06, 0x07), 0.85));
            let pulse = if voice.state == VoiceState::Listening {
                ui.ctx().request_repaint();
                0.6 + 0.4 * ((now * 6.0).sin() as f32 * 0.5 + 0.5)
            } else {
                1.0
            };
            painter.circle_filled(Pos2::new(r.left() + 16.0, r.top() + 14.0), 5.0, fade(dot, pulse));
            painter.galley(Pos2::new(r.left() + 30.0, r.top() + 7.0), g, WHITE);
        }
    }

    // Layer switch, top right (below the search on narrow windows).
    let seg_w = 210.0;
    let seg = if narrow {
        Rect::from_min_size(bar.left_bottom() + Vec2::new(0.0, if st.message.is_some() { 70.0 } else { 10.0 }), Vec2::new(seg_w, 36.0))
    } else {
        Rect::from_min_size(Pos2::new(view.right() - pad - seg_w, view.top() + pad + 4.0), Vec2::new(seg_w, 36.0))
    };
    shadow(&painter, seg, 255, 4, 14, 0.35);
    painter.rect(seg, CornerRadius::same(255), window_bg(), Stroke::new(1.0_f32, white(0.18)), StrokeKind::Inside);
    for (k, (layer, name)) in [(Layer::Satellite, "Satellite"), (Layer::Atlas, "Atlas")].into_iter().enumerate() {
        let r = Rect::from_min_size(seg.min + Vec2::new(3.0 + k as f32 * (seg_w - 6.0) / 2.0, 3.0), Vec2::new((seg_w - 6.0) / 2.0, 30.0));
        let rr = ui.interact(r, egui::Id::new(("map-layer", k)), Sense::click());
        let on = st.layer == layer;
        if on {
            painter.rect_filled(r, CornerRadius::same(255), WHITE);
        } else if rr.hovered() {
            painter.rect_filled(r, CornerRadius::same(255), white(0.12));
        }
        if rr.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }
        painter.text(r.center(), Align2::CENTER_CENTER, name, font(13.0, 600), if on { ACCENT } else { ink() });
        if rr.clicked() {
            st.layer = layer;
        }
    }

    // Zoom buttons and postal toggle, bottom right.
    let mut y = view.bottom() - pad;
    let round_btn = |ui: &mut Ui, painter: &egui::Painter, r: Rect, id: &str, text: &str, on: bool| -> bool {
        let rr = ui.interact(r, egui::Id::new(("map-btn", id)), Sense::click());
        shadow(painter, r, 255, 3, 10, 0.3);
        let fill = if on { WHITE } else if rr.hovered() { lerp_color(window_bg(), WHITE, 0.12) } else { window_bg() };
        painter.rect(r, CornerRadius::same(255), fill, Stroke::new(1.0_f32, white(0.18)), StrokeKind::Inside);
        painter.text(r.center(), Align2::CENTER_CENTER, text, font(if text.len() == 1 { 20.0 } else { 13.0 }, 600), if on { ACCENT } else { ink() });
        if rr.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }
        rr.clicked()
    };
    for (id, text) in [("out", "−"), ("in", "+")] {
        let r = Rect::from_min_size(Pos2::new(view.right() - pad - 40.0, y - 40.0), Vec2::splat(40.0));
        if round_btn(ui, &painter, r, id, text, false) {
            let f = if id == "in" { 1.6 } else { 1.0 / 1.6 };
            st.flight = Some((st.center, st.scale, st.center, (st.scale * f).clamp(MapUi::min_scale(view), MAX_SCALE), now));
        }
        y -= 48.0;
    }
    let r = Rect::from_min_size(Pos2::new(view.right() - pad - 96.0, y - 34.0), Vec2::new(96.0, 34.0));
    if round_btn(ui, &painter, r, "postals", "Postals", st.show_postals) {
        st.show_postals = !st.show_postals;
    }

    // Read-out, bottom left: the pinged postal's street and corner / intersection,
    // the waypoint, and what's under the mouse.
    let gold = Color32::from_rgb(0xff, 0xd2, 0x4a);
    let mut rows: Vec<(String, egui::FontId, Color32)> = Vec::new();
    let place_rows = |rows: &mut Vec<(String, egui::FontId, Color32)>, place: &Option<Place>| match place {
        Some(pl) => {
            rows.push((pl.street.clone(), font(15.0, 600), WHITE));
            match &pl.crossing {
                Some(c) => rows.push((format!("{} of {} & {}", c.kind.label(), c.streets[0], c.streets[1]), font(13.0, 500), white(0.85))),
                None => rows.push(("No corner or intersection close by".into(), font(13.0, 500), white(0.6))),
            }
        }
        None => rows.push(("No street found".into(), font(13.0, 500), white(0.6))),
    };
    if let Some((i, place)) = &st.found_place {
        rows.push((format!("Postal {}", POSTALS[*i].0), font(17.0, 700), gold));
        place_rows(&mut rows, place);
        if PARTLY_HIDDEN.contains(&POSTALS[*i].0) {
            rows.push(("Number partly hidden on the reference map — check it in game.".into(), font(12.0, 400), white(0.55)));
        }
    }
    if let Some((w, place)) = &st.waypoint_place {
        if !rows.is_empty() {
            rows.push((String::new(), font(6.0, 400), WHITE));
        }
        let code = nearest_postal(*w).map_or("—", |n| n.0);
        rows.push((format!("Waypoint · postal {code}"), font(15.0, 700), Color32::from_rgb(0xff, 0x7a, 0x6e)));
        place_rows(&mut rows, place);
    }
    if let Some(h) = resp.hover_pos() {
        let m = st.to_map(view, h);
        if let Some((code, _)) = nearest_postal(m) {
            if !rows.is_empty() {
                rows.push((String::new(), font(6.0, 400), WHITE));
            }
            let street = streets::nearest_street(m.x, m.y).map(|s| format!(" · {s}")).unwrap_or_default();
            rows.push((format!("Mouse: postal {code}{street}"), font(12.0, 500), white(0.7)));
        }
    }
    if rows.is_empty() {
        rows.push(("Click a postal for its street · drag to move · scroll to zoom · right-click for a waypoint".into(), font(13.0, 500), WHITE));
    }
    let wrap = (view.width() - 150.0).clamp(160.0, 460.0);
    let galleys: Vec<_> = rows.into_iter().map(|(t, f, c)| painter.layout(t, f, c, wrap)).collect();
    let w = galleys.iter().map(|g| g.size().x).fold(0.0, f32::max);
    let h: f32 = galleys.iter().map(|g| g.size().y + 2.0).sum();
    let box_r = Rect::from_min_size(Pos2::new(view.left() + pad, view.bottom() - pad - h - 20.0), Vec2::new(w + 28.0, h + 20.0));
    shadow(&painter, box_r, 14, 4, 14, 0.35);
    painter.rect_filled(box_r, CornerRadius::same(14), fade(Color32::from_rgb(0x14, 0x06, 0x07), 0.88));
    let mut y = box_r.top() + 10.0;
    for g in galleys {
        let gh = g.size().y;
        painter.galley(Pos2::new(box_r.left() + 14.0, y), g, WHITE);
        y += gh + 2.0;
    }
    let mut bx = box_r.right() + 8.0;
    if st.found.is_some() {
        let cr = Rect::from_min_size(Pos2::new(bx, box_r.bottom() - 34.0), Vec2::new(70.0, 34.0));
        if round_btn(ui, &painter, cr, "unping", "Close", false) {
            st.found = None;
        }
        bx += 78.0;
    }
    if st.waypoint.is_some() {
        let cr = Rect::from_min_size(Pos2::new(bx, box_r.bottom() - 34.0), Vec2::new(116.0, 34.0));
        if round_btn(ui, &painter, cr, "clear", "Clear waypoint", false) {
            st.waypoint = None;
        }
    }

    let _ = radius;
}
