//! The Field Guide app. Each page has its own accent colour (see data.rs).

use crate::core::Core;
use crate::fieldguide::data::{guide, shows_for, Kind, Page, Section, GROUPS};
use crate::fieldguide::store::FieldGuideState;
use crate::ui::icons::emoji_image;
use crate::ui::theme::{self, *};
use crate::ui::wallpaper::gradient_rect;
use eframe::egui::{self, Align, Align2, Color32, CornerRadius, Layout, Margin, Pos2, Rect, RichText, Sense, Shape, Stroke, StrokeKind, Ui, UiBuilder, Vec2};
use rand::Rng;

#[derive(Default)]
pub struct FieldGuideUi {
    last_page: String,
    last_complete: bool,
    party: Option<(f64, Vec<Bit>)>,
    last_focus: Option<(String, usize)>,
}

struct Bit {
    left: f32,
    width: f32,
    color: Color32,
    delay: f32,
    duration: f32,
    spin: f32,
    drift: f32,
}

const CONFETTI: [u32; 7] = [0xf2c14e, 0x4fc3f7, 0xef6a6a, 0xc58af0, 0x86d98a, 0xff9a6b, 0xffffff];

/// What a click in the guide asked for (applied after drawing).
enum Act {
    Run(&'static str, Option<String>),
    Toggle(String),
}

pub fn show(ui: &mut Ui, st: &mut FieldGuideUi, core: &mut Core, now: f64, radius: u8) {
    let rect = ui.max_rect();
    let narrow = rect.width() <= 600.0;
    let state = core.guide.clone();
    let page = guide().page(&state.page).unwrap_or(&guide().pages[0]);
    let progress = state.progress(page.id);

    // Confetti the moment the last step on a page is ticked.
    let page_changed = st.last_page != page.id;
    if progress.complete && !page_changed && !st.last_complete {
        let mut rng = rand::thread_rng();
        let bits = (0..36)
            .map(|i| Bit {
                left: rng.gen::<f32>(),
                width: 6.0 + rng.gen::<f32>() * 7.0,
                color: rgb(hex(CONFETTI[i % CONFETTI.len()])),
                delay: rng.gen::<f32>() * 0.35,
                duration: 1.4 + rng.gen::<f32>() * 0.9,
                spin: 360.0 + rng.gen::<f32>() * 540.0,
                drift: (rng.gen::<f32>() - 0.5) * 120.0,
            })
            .collect();
        st.party = Some((now, bits));
    }
    if page_changed {
        st.party = None;
    }
    st.last_complete = progress.complete;
    st.last_page = page.id.to_string();

    let (side, pane) = if narrow {
        let side = Rect::from_min_size(rect.min, Vec2::new(rect.width(), 52.0));
        (side, Rect::from_min_max(Pos2::new(rect.left(), side.bottom()), rect.max))
    } else {
        let side = Rect::from_min_size(rect.min, Vec2::new(236.0, rect.height()));
        (side, Rect::from_min_max(Pos2::new(side.right(), rect.top()), rect.max))
    };

    let mut acts: Vec<Act> = Vec::new();
    sidebar(ui, side, narrow, radius, &state, &mut acts);

    // ── Page ──
    let accent = page.color;
    let mut pn = ui.new_child(UiBuilder::new().max_rect(pane));
    pn.set_clip_rect(pane.intersect(ui.clip_rect()));
    let mut scroll = egui::ScrollArea::vertical().id_salt("fg-pane").auto_shrink([false, false]);
    if page_changed {
        scroll = scroll.vertical_scroll_offset(0.0);
    }
    // A step ticked from the Stream Deck is brought into view.
    let focus_now = state.focus.clone().map(|f| (f, progress.done.len()));
    let scroll_to = if focus_now.is_some() && focus_now != st.last_focus { state.focus.clone() } else { None };
    st.last_focus = focus_now;

    scroll.show(&mut pn, |ui| {
        egui::Frame::NONE.inner_margin(if narrow { Margin::same(14) } else { Margin { left: 22, right: 22, top: 18, bottom: 28 } }).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.spacing_mut().item_spacing.y = 12.0;
            hero(ui, page, &state, narrow, &mut acts);
            ui.add_space(6.0);
            let ctx = Ctx { accent, state: &state, scroll_to: scroll_to.as_deref() };
            match &page.kind {
                Kind::Home => home(ui, &state, &mut acts, now),
                Kind::Columns(cols) => {
                    let mut mica_asked = false;
                    let w = ui.available_width();
                    let fit = (((w + 16.0) / (250.0 + 16.0)).floor() as usize).max(1);
                    if cols.len() > 1 && fit >= cols.len() {
                        ui.spacing_mut().item_spacing.x = 16.0;
                        ui.columns(cols.len(), |uis| {
                            for (ci, (c, cu)) in cols.iter().zip(uis.iter_mut()).enumerate() {
                                cu.spacing_mut().item_spacing.y = 12.0;
                                for (si, s) in c.sections.iter().enumerate() {
                                    section(cu, &ctx, s, &ci.to_string(), si, &mut mica_asked, &mut acts);
                                }
                            }
                        });
                    } else {
                        for (ci, c) in cols.iter().enumerate() {
                            for (si, s) in c.sections.iter().enumerate() {
                                section(ui, &ctx, s, &ci.to_string(), si, &mut mica_asked, &mut acts);
                            }
                        }
                    }
                }
                Kind::Choice { question, options, intro, outro } => {
                    let mut mica_asked = false;
                    for (si, s) in intro.iter().enumerate() {
                        section(ui, &ctx, s, "i", si, &mut mica_asked, &mut acts);
                    }
                    let chosen = state.chosen.get(page.id).copied();
                    let labels: Vec<(String, String)> = options.iter().enumerate().map(|(i, o)| ((i + 1).to_string(), o.name.to_string())).collect();
                    let picked = choice_card(ui, accent, question, &labels, chosen, 170.0, false);
                    if let Some(i) = picked {
                        acts.push(Act::Run("option", Some((i + 1).to_string())));
                    }
                    if let Some(c) = chosen {
                        for (si, s) in options[c].sections.iter().enumerate() {
                            section(ui, &ctx, s, &format!("o{c}"), si, &mut mica_asked, &mut acts);
                        }
                    }
                    for (si, s) in outro.iter().enumerate() {
                        section(ui, &ctx, s, "x", si, &mut mica_asked, &mut acts);
                    }
                }
                Kind::Tables(tables) => {
                    for t in tables {
                        info_card(ui, accent, |ui| {
                            ui.label(RichText::new(t.title.to_uppercase()).font(font(12.0, 700)).color(accent_text(accent)));
                            table(ui, &t.head, &t.rows);
                        });
                    }
                }
            }
            if page.group == "injury" {
                ui.add_space(6.0);
                ui.horizontal_wrapped(|ui| {
                    ui.label(RichText::new("Next:").font(font(13.0, 400)).color(white(0.6)));
                    for id in ["vitals", "pain"] {
                        let p = guide().page(id).unwrap();
                        if footer_button(ui, p) {
                            acts.push(Act::Run("open", Some(id.to_string())));
                        }
                    }
                });
            }
        });
    });

    // Confetti over the top of the page.
    if let Some((at, bits)) = &st.party {
        let age = (now - at) as f32;
        if age > 2.6 {
            st.party = None;
        } else {
            let p = ui.painter().with_clip_rect(pane.intersect(ui.clip_rect()));
            for b in bits {
                let t = ((age - b.delay) / b.duration).clamp(0.0, 1.0);
                if t <= 0.0 || t >= 1.0 {
                    continue;
                }
                let e = t * t; // ease-in
                let c = Pos2::new(pane.left() + b.left * pane.width() + b.drift * e, pane.top() - 20.0 + 520.0 * e);
                let rot = (b.spin * e).to_radians();
                let (hw, hh) = (b.width / 2.0, b.width * 0.45 / 2.0);
                let rotv = |x: f32, y: f32| c + Vec2::new(x * rot.cos() - y * rot.sin(), x * rot.sin() + y * rot.cos());
                p.add(Shape::convex_polygon(vec![rotv(-hw, -hh), rotv(hw, -hh), rotv(hw, hh), rotv(-hw, hh)], fade(b.color, 1.0 - e), Stroke::NONE));
            }
            ui.ctx().request_repaint();
        }
    }

    for act in acts {
        match act {
            Act::Run(cmd, arg) => {
                core.guide.run(cmd, arg.as_deref());
            }
            Act::Toggle(key) => core.guide.toggle_step(&key),
        }
    }
}

struct Ctx<'a> {
    accent: [u8; 3],
    state: &'a FieldGuideState,
    scroll_to: Option<&'a str>,
}

fn sidebar(ui: &mut Ui, side: Rect, narrow: bool, radius: u8, state: &FieldGuideState, acts: &mut Vec<Act>) {
    let p = ui.painter().clone();
    p.rect_filled(side, CornerRadius { nw: 0, ne: 0, sw: if narrow { 0 } else { radius.saturating_sub(1) }, se: 0 }, sidebar_bg());
    if narrow {
        p.hline(side.x_range(), side.bottom(), Stroke::new(1.0_f32, white(0.1)));
    } else {
        p.vline(side.right(), side.y_range(), Stroke::new(1.0_f32, white(0.1)));
    }
    let mut sb = ui.new_child(UiBuilder::new().max_rect(side));
    sb.set_clip_rect(side.intersect(ui.clip_rect()));
    let mut item = |ui: &mut Ui, page: &Page, w: Option<f32>| {
        let active = state.page == page.id;
        let pr = state.progress(page.id);
        let f = font(13.5, if active { 600 } else { 400 });
        let badge = if page.id != "home" && !pr.done.is_empty() { Some(if pr.complete { "✓".to_string() } else { pr.done.len().to_string() }) } else { None };
        let width = w.unwrap_or_else(|| ui.fonts(|fo| fo.layout_no_wrap(page.title.into(), f.clone(), ink()).size().x) + 60.0 + if badge.is_some() { 30.0 } else { 0.0 });
        // Long titles wrap onto a second line in the sidebar (one line in the narrow strip).
        let text_w = width - 8.0 - 26.0 - 9.0 - 8.0 - if badge.is_some() { 30.0 } else { 0.0 };
        let galley = ui.fonts(|fo| if w.is_some() { fo.layout(page.title.to_string(), f.clone(), ink(), text_w) } else { fo.layout_no_wrap(page.title.to_string(), f.clone(), ink()) });
        let height = (galley.size().y + 14.0).max(36.0);
        let (r, resp) = ui.allocate_exact_size(Vec2::new(width, height), Sense::click());
        let c = page.color;
        if active {
            ui.painter().rect_filled(r, 10.0, mix(c, 0.30, [255, 255, 255], 0.08));
            if narrow {
                ui.painter().rect_filled(Rect::from_min_max(Pos2::new(r.left(), r.bottom() - 3.0), r.max), 0.0, rgb(c));
            } else {
                ui.painter().rect_filled(Rect::from_min_size(r.min, Vec2::new(3.0, r.height())), CornerRadius { nw: 10, sw: 10, ne: 0, se: 0 }, rgb(c));
            }
        } else {
            theme::hover_fill(ui, &resp, r, 10, white(0.1));
        }
        let hover = ui.ctx().animate_bool_with_time(resp.id.with("b"), resp.hovered(), 0.25);
        let br = Rect::from_center_size(Pos2::new(r.left() + 8.0 + 13.0, r.center().y), Vec2::splat(26.0 * (1.0 + 0.18 * hover)));
        ui.painter().rect_filled(br, 8.0, tint(c, 0.28));
        emoji_image(page.icon).paint_at(ui, Rect::from_center_size(br.center(), Vec2::splat(15.0 * (1.0 + 0.18 * hover))));
        ui.painter().galley(Pos2::new(r.left() + 8.0 + 26.0 + 9.0, r.center().y - galley.size().y / 2.0), galley, ink());
        if let Some(b) = badge {
            let g = ui.fonts(|fo| fo.layout_no_wrap(b, font(11.0, 600), ink()));
            let bw = (g.size().x + 12.0).max(20.0);
            let bb = Rect::from_center_size(Pos2::new(r.right() - 8.0 - bw / 2.0, r.center().y), Vec2::new(bw, 18.0));
            ui.painter().rect_filled(bb, 10.0, tint(c, 0.45));
            ui.painter().galley(bb.center() - g.size() / 2.0, g, ink());
        }
        if resp.clicked() {
            acts.push(if page.id == "home" { Act::Run("home", None) } else { Act::Run("open", Some(page.id.to_string())) });
        }
    };

    if narrow {
        egui::ScrollArea::horizontal().id_salt("fg-strip").show(&mut sb, |ui| {
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 4.0;
                ui.add_space(10.0);
                for page in &guide().pages {
                    item(ui, page, None);
                }
                ui.add_space(10.0);
            });
        });
    } else {
        egui::ScrollArea::vertical().id_salt("fg-side").auto_shrink([false, false]).show(&mut sb, |ui| {
            egui::Frame::NONE.inner_margin(Margin::symmetric(10, 12)).show(ui, |ui| {
                ui.spacing_mut().item_spacing.y = 0.0;
                let w = ui.available_width();
                item(ui, &guide().pages[0], Some(w));
                for g in GROUPS.iter() {
                    ui.add_space(14.0);
                    ui.horizontal(|ui| {
                        ui.add_space(8.0);
                        ui.label(RichText::new(g.name.to_uppercase()).font(font(11.0, 600)).color(white(0.45)));
                    });
                    ui.add_space(6.0);
                    for page in guide().pages_in(g.id) {
                        item(ui, page, Some(w));
                    }
                }
            });
        });
    }
}

fn hero(ui: &mut Ui, page: &Page, state: &FieldGuideState, narrow: bool, acts: &mut Vec<Act>) {
    let pr = state.progress(page.id);
    let total = pr.keys.len();
    let chosen = state.chosen.get(page.id);
    let mica = state.mica.get(page.id);
    let c = page.color;
    let w = ui.available_width();
    let emoji = if narrow { 48.0 } else { 62.0 };
    let pad = if narrow { 12.0 } else { 16.0 };
    let h = emoji + pad * 2.0;
    let (r, _) = ui.allocate_exact_size(Vec2::new(w, h), Sense::hover());
    let p = ui.painter().clone();
    gradient_rect(&p, r, 22.0, 120.0, tint(c, 0.38), tint(c, 0.08));
    p.rect_stroke(r, CornerRadius::same(22), Stroke::new(1.0_f32, tint(c, 0.45)), StrokeKind::Inside);

    // Emoji tile, with a little bounce when the page opens.
    let opened = ui.ctx().animate_value_with_time(egui::Id::new(("fg-hero", page.id)), 1.0, 0.6);
    let bounce = ease_back(opened);
    let er = Rect::from_min_size(Pos2::new(r.left() + pad, r.top() + pad), Vec2::splat(emoji));
    p.rect_filled(er, CornerRadius::same(if narrow { 16 } else { 20 }), white(0.16));
    let es = emoji * 0.55 * (0.5 + 0.5 * bounce);
    emoji_image(page.icon).paint_at(ui, Rect::from_center_size(er.center(), Vec2::splat(es)));

    let has_reset = total > 0 || page.options().is_some();
    let reset_w = if has_reset { 80.0 } else { 0.0 };
    let tx = er.right() + 16.0;
    let tw = (r.right() - pad - reset_w - tx).max(60.0);
    let title_font = display(if narrow { 19.0 } else { 24.0 }, 700);
    let title = ui.fonts(|f| f.layout(page.title.to_string(), title_font, ink(), tw));
    let title_h = title.size().y;
    let line2_h = 22.0;
    let ty = r.center().y - (title_h + 4.0 + line2_h) / 2.0;
    p.galley(Pos2::new(tx, ty), title, ink());
    let y2 = ty + title_h + 4.0;
    if pr.needs_choice && total == 0 {
        p.text(Pos2::new(tx, y2 + line2_h / 2.0), Align2::LEFT_CENTER, "Pick what applies to start", font(13.5, 400), white(0.75));
    } else if total > 0 {
        let label = if pr.complete { "All done!".to_string() } else { format!("{} / {}", pr.done.len(), total) };
        let g = ui.fonts(|f| f.layout_no_wrap(label, font(13.0, 600), ink()));
        let bar_w = (tw - g.size().x - 12.0 - if pr.complete { 22.0 } else { 0.0 }).min(320.0).max(40.0);
        let bar = Rect::from_min_size(Pos2::new(tx, y2 + line2_h / 2.0 - 4.0), Vec2::new(bar_w, 8.0));
        p.rect_filled(bar, 4.0, white(0.18));
        let frac = ui.ctx().animate_value_with_time(egui::Id::new(("fg-bar", page.id)), pr.done.len() as f32 / total as f32, 0.45);
        if frac > 0.0 {
            p.rect_filled(Rect::from_min_size(bar.min, Vec2::new(bar_w * frac, 8.0)), 4.0, rgb(c));
        }
        let gx = bar.right() + 12.0;
        let gw = g.size().x;
        p.galley(Pos2::new(gx, bar.center().y - g.size().y / 2.0), g, ink());
        if pr.complete {
            emoji_image("1f389").paint_at(ui, Rect::from_min_size(Pos2::new(gx + gw + 4.0, bar.center().y - 8.0), Vec2::splat(16.0)));
        }
    } else {
        let t = if page.id == "home" { "Just a guide - don't forget to have fun when you can!" } else { "Quick reference" };
        let g = ui.fonts(|f| f.layout(t.to_string(), font(13.5, 400), white(0.75), tw));
        p.galley(Pos2::new(tx, y2 + 2.0), g, white(0.75));
    }
    if has_reset {
        let enabled = !pr.done.is_empty() || chosen.is_some() || mica.is_some();
        let br = Rect::from_min_size(Pos2::new(r.right() - pad - 70.0, r.center().y - 16.0), Vec2::new(70.0, 32.0));
        let mut child = ui.new_child(UiBuilder::new().max_rect(br));
        if theme::button_enabled(&mut child, "Reset", Btn::Glass, 13.0, Some(70.0), Vec2::new(16.0, 7.0), enabled).clicked() {
            acts.push(Act::Run("reset", None));
        }
    }
}

fn home(ui: &mut Ui, state: &FieldGuideState, acts: &mut Vec<Act>, _now: f64) {
    for g in GROUPS.iter() {
        ui.add_space(4.0);
        ui.label(RichText::new(g.name.to_uppercase()).font(font(12.0, 700)).color(white(0.6)));
        let pages: Vec<&Page> = guide().pages_in(g.id).collect();
        let gap = 12.0;
        let w = ui.available_width();
        let narrow = w < 560.0;
        let cols = if narrow { 2 } else { (((w + gap) / (150.0 + gap)).floor() as usize).max(1) };
        let cw = (w - gap * (cols as f32 - 1.0)) / cols as f32;
        let ch = 132.0;
        let rows = pages.len().div_ceil(cols);
        let (area, _) = ui.allocate_exact_size(Vec2::new(w, rows as f32 * ch + (rows - 1) as f32 * gap), Sense::hover());
        for (i, page) in pages.iter().enumerate() {
            let (row, col) = (i / cols, i % cols);
            let cell = Rect::from_min_size(area.min + Vec2::new(col as f32 * (cw + gap), row as f32 * (ch + gap)), Vec2::new(cw, ch));
            let resp = ui.interact(cell, egui::Id::new(("tile", page.id)), Sense::click());
            let hover = ui.ctx().animate_bool_with_time(resp.id, resp.hovered(), 0.25);
            let r = cell.translate(Vec2::new(0.0, -4.0 * ease_out(hover)));
            let c = page.color;
            let p = ui.painter().clone();
            if hover > 0.0 {
                theme::shadow(&p, r, 22, 14, 28, 0.3 * hover);
            }
            gradient_rect(&p, r, 22.0, 150.0, tint(c, 0.42), tint(c, 0.12));
            p.rect_stroke(r, CornerRadius::same(22), Stroke::new(1.0_f32, lerp_color(tint(c, 0.5), rgb(c), hover)), StrokeKind::Inside);
            let es = 32.0 * (1.0 + 0.25 * hover);
            emoji_image(page.icon).paint_at(ui, Rect::from_center_size(Pos2::new(r.left() + 14.0 + 16.0, r.top() + 14.0 + 18.0), Vec2::splat(es)));

            let pr = state.progress(page.id);
            let label = if page.options().is_some() && pr.needs_choice && pr.done.is_empty() {
                format!("{} situations", page.options().unwrap().len())
            } else if pr.keys.is_empty() {
                "Reference".into()
            } else if pr.complete {
                "All done ✓".into()
            } else if !pr.done.is_empty() {
                format!("{} of {} done", pr.done.len(), pr.keys.len())
            } else {
                format!("{} steps", pr.keys.len())
            };
            let has_bar = !pr.keys.is_empty() || page.options().is_some();
            let mut y = r.bottom() - 14.0;
            if has_bar {
                let bar = Rect::from_min_max(Pos2::new(r.left() + 14.0, y - 5.0), Pos2::new(r.right() - 14.0, y));
                p.rect_filled(bar, 3.0, white(0.18));
                let frac = if pr.keys.is_empty() { 0.0 } else { pr.done.len() as f32 / pr.keys.len() as f32 };
                if frac > 0.0 {
                    p.rect_filled(Rect::from_min_size(bar.min, Vec2::new(bar.width() * frac, 5.0)), 3.0, rgb(c));
                }
                y -= 5.0 + 6.0;
            }
            let small = p.text(Pos2::new(r.left() + 14.0, y), Align2::LEFT_BOTTOM, label, font(12.0, 400), white(0.7));
            let title = ui.fonts(|f| f.layout(page.title.to_string(), font(14.0, 600), ink(), r.width() - 28.0));
            p.galley(Pos2::new(r.left() + 14.0, small.top() - 4.0 - title.size().y), title, ink());
            if resp.hovered() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
            }
            if resp.clicked() {
                acts.push(Act::Run("open", Some(page.id.to_string())));
            }
        }
        ui.add_space(6.0);
    }
    ui.add_space(12.0);
    ui.label(RichText::new("Compiled by Blaire, Flamingo and Leah. Happy treating!").font(font(12.0, 400)).color(white(0.45)));
}

/// "Is MICA available?" shows once, just above the first section that depends on it.
fn section(ui: &mut Ui, ctx: &Ctx, s: &'static Section, prefix: &str, si: usize, mica_asked: &mut bool, acts: &mut Vec<Act>) {
    let page = ctx.state.page.as_str();
    let mica = ctx.state.mica.get(page).copied();
    if s.mica.is_some() && !*mica_asked {
        *mica_asked = true;
        let labels = vec![("✓".to_string(), "Yes".to_string()), ("✕".to_string(), "No".to_string())];
        let chosen = mica.map(|m| if m { 0 } else { 1 });
        if let Some(i) = choice_card(ui, ctx.accent, "Is MICA available?", &labels, chosen, 92.0, true) {
            acts.push(Act::Run("mica", Some(if i == 0 { "yes" } else { "no" }.into())));
        }
    }
    if !shows_for(s, mica) {
        return;
    }
    let accent = ctx.accent;
    let done = ctx.state.progress(page).done;
    let keys = ctx.state.progress(page).keys;
    let next_up = keys.iter().find(|k| !done.contains(k)).cloned();
    let body = |ui: &mut Ui| {
        ui.horizontal(|ui| {
            ui.add_space(4.0);
            ui.label(RichText::new(s.title.to_uppercase()).font(font(12.0, 700)).color(accent_text(accent)));
        });
        ui.add_space(-2.0);
        if s.info {
            for text in &s.steps {
                bullet(ui, accent, text);
            }
        } else {
            ui.spacing_mut().item_spacing.y = 2.0;
            for (i, text) in s.steps.iter().enumerate() {
                let key = format!("{prefix}-{si}-{i}");
                let is_done = done.contains(&key);
                let is_next = next_up.as_deref() == Some(key.as_str());
                let r = step(ui, accent, i + 1, text, is_done, is_next, &key);
                if ctx.scroll_to == Some(key.as_str()) {
                    ui.scroll_to_rect(r.rect, Some(Align::Center));
                }
                if r.clicked() {
                    acts.push(Act::Toggle(key));
                }
            }
        }
    };
    if s.info { info_card(ui, accent, body) } else { step_card(ui, body) }
}

fn step_card(ui: &mut Ui, body: impl FnOnce(&mut Ui)) {
    egui::Frame::NONE.fill(card_fill()).stroke(Stroke::new(1.0_f32, white(0.18))).corner_radius(CornerRadius::same(18)).inner_margin(Margin::same(14)).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.spacing_mut().item_spacing.y = 8.0;
        body(ui);
    });
}

fn info_card(ui: &mut Ui, accent: [u8; 3], body: impl FnOnce(&mut Ui)) {
    let resp = egui::Frame::NONE.fill(mix(accent, 0.12, [255, 255, 255], 0.05)).corner_radius(CornerRadius::same(18)).inner_margin(Margin::same(14)).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.spacing_mut().item_spacing.y = 8.0;
        body(ui);
    });
    theme::dashed_rect(ui.painter(), resp.response.rect, 18.0, tint(accent, 0.4));
}

fn bullet(ui: &mut Ui, accent: [u8; 3], text: &str) {
    let w = ui.available_width();
    let g = ui.fonts(|f| f.layout(text.to_string(), font(14.0, 400), ink(), w - 22.0));
    let (r, _) = ui.allocate_exact_size(Vec2::new(w, g.size().y), Sense::hover());
    ui.painter().circle_filled(Pos2::new(r.left() + 7.0, r.top() + 10.0), 3.0, rgb(accent));
    ui.painter().galley(Pos2::new(r.left() + 18.0, r.top()), g, ink());
}

fn step(ui: &mut Ui, accent: [u8; 3], n: usize, text: &str, done: bool, next: bool, key: &str) -> egui::Response {
    let w = ui.available_width();
    let color = if done { white(0.6) } else { ink() };
    let g = ui.fonts(|f| f.layout(text.to_string(), font(14.0, 400), color, w - 20.0 - 24.0 - 11.0));
    let h = g.size().y.max(20.0) + 16.0;
    let (r, resp) = ui.allocate_exact_size(Vec2::new(w, h), Sense::click());
    let hover = ui.ctx().animate_bool_with_time(resp.id, resp.hovered(), 0.2);
    let r = r.translate(Vec2::new(3.0 * hover, 0.0));
    let p = ui.painter();
    let now = ui.input(|i| i.time);
    if next {
        p.rect(r, CornerRadius::same(14), tint(accent, 0.14), Stroke::new(1.0_f32, tint(accent, 0.45)), StrokeKind::Inside);
    }
    if hover > 0.0 {
        p.rect_filled(r, 14.0, white(0.1 * hover));
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    let c = Pos2::new(r.left() + 10.0 + 12.0, r.top() + 8.0 + 10.0);
    if next {
        // Gentle pulse around the next step's number.
        let t = ((now % 1.6) / 1.6) as f32;
        let pulse = (t * std::f32::consts::TAU).sin().max(0.0);
        p.circle_filled(c, 12.0 + 6.0 * pulse, tint(accent, 0.6 * (1.0 - pulse)));
        ui.ctx().request_repaint();
    }
    if done {
        let pop = ease_back(ui.ctx().animate_bool_with_time(egui::Id::new(("pop", key)), true, 0.4));
        p.circle_filled(c, 12.0 * pop.max(0.4), rgb(accent));
        p.text(c, Align2::CENTER_CENTER, "✓", font(14.0, 700), Color32::from_rgb(0x2e, 0x0b, 0x0d));
    } else {
        ui.ctx().animate_bool_with_time(egui::Id::new(("pop", key)), false, 0.0);
        if next {
            p.circle_filled(c, 11.0, window_bg());
        }
        p.circle_stroke(c, 11.0, Stroke::new(2.0_f32, tint(accent, 0.7)));
        p.text(c, Align2::CENTER_CENTER, n.to_string(), font(12.0, 700), accent_text(accent));
    }
    let tx = Pos2::new(r.left() + 10.0 + 24.0 + 11.0, r.top() + 8.0);
    let tw = g.size().x;
    let rows: Vec<(f32, f32, f32)> = g.rows.iter().map(|row| (row.rect.min.y, row.rect.max.y, row.rect.width())).collect();
    p.galley(tx, g, color);
    if done {
        for (top, bottom, width) in rows {
            let y = tx.y + (top + bottom) / 2.0 + 1.0;
            p.hline(tx.x..=tx.x + width.min(tw), y, Stroke::new(1.0_f32, tint(accent, 0.8)));
        }
    }
    resp
}

/// A card asking which one applies. Returns the index clicked.
fn choice_card(ui: &mut Ui, accent: [u8; 3], question: &str, options: &[(String, String)], chosen: Option<usize>, min_w: f32, compact: bool) -> Option<usize> {
    let mut picked = None;
    egui::Frame::NONE
        .fill(mix(accent, 0.16, [255, 255, 255], 0.04))
        .stroke(Stroke::new(1.0_f32, tint(accent, 0.5)))
        .corner_radius(CornerRadius::same(18))
        .inner_margin(if compact { Margin::symmetric(14, 10) } else { Margin::same(14) })
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            let w = ui.available_width();
            let draw_buttons = |ui: &mut Ui, w: f32, picked: &mut Option<usize>| {
                let gap = if compact { 8.0 } else { 10.0 };
                let cols = if compact { options.len() } else { ((((w + gap) / (min_w + gap)).floor() as usize).max(1)).min(options.len()) };
                let bw = if compact { min_w } else { (w - gap * (cols as f32 - 1.0)) / cols as f32 };
                for chunk in options.chunks(cols).enumerate() {
                    let (ci, row) = chunk;
                    let texts: Vec<_> = row.iter().map(|(_, t)| ui.fonts(|f| f.layout(t.clone(), font(14.0, 500), ink(), bw - 28.0 - 26.0 - 10.0))).collect();
                    let h = texts.iter().map(|g| g.size().y).fold(26.0f32, f32::max) + if compact { 16.0 } else { 24.0 };
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = gap;
                        for (j, ((badge, _), g)) in row.iter().zip(texts).enumerate() {
                            let i = ci * cols + j;
                            let is = chosen == Some(i);
                            let (r, resp) = ui.allocate_exact_size(Vec2::new(bw, h), Sense::click());
                            let hover = ui.ctx().animate_bool_with_time(resp.id, resp.hovered(), 0.2);
                            let r = r.translate(Vec2::new(0.0, -2.0 * hover));
                            let p = ui.painter();
                            let fill = if is { tint(accent, 0.38) } else { lerp_color(white(0.1), white(0.18), hover) };
                            p.rect(r, CornerRadius::same(16), fill, Stroke::new(2.0_f32, if is { rgb(accent) } else { Color32::TRANSPARENT }), StrokeKind::Inside);
                            let br = Rect::from_min_size(Pos2::new(r.left() + 14.0, r.center().y - 13.0), Vec2::splat(26.0));
                            p.rect_filled(br, 9.0, if is { rgb(accent) } else { tint(accent, 0.35) });
                            let bc = if is { Color32::from_rgb(0x2e, 0x0b, 0x0d) } else { ink() };
                            if badge == "✕" {
                                // Drawn by hand: the fonts don't have this mark.
                                let k = 4.5;
                                p.line_segment([br.center() - Vec2::splat(k), br.center() + Vec2::splat(k)], Stroke::new(2.0_f32, bc));
                                p.line_segment([br.center() + Vec2::new(-k, k), br.center() + Vec2::new(k, -k)], Stroke::new(2.0_f32, bc));
                            } else {
                                p.text(br.center(), Align2::CENTER_CENTER, badge, font(13.0, 700), bc);
                            }
                            p.galley(Pos2::new(br.right() + 10.0, r.center().y - g.size().y / 2.0), g, ink());
                            if resp.hovered() {
                                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                            }
                            if resp.clicked() {
                                *picked = Some(i);
                            }
                        }
                    });
                }
            };
            if compact {
                let buttons_w = options.len() as f32 * min_w + (options.len() as f32 - 1.0) * 8.0;
                if w - buttons_w >= 150.0 {
                    let (row, _) = ui.allocate_exact_size(Vec2::new(w, 42.0), Sense::hover());
                    ui.painter().text(Pos2::new(row.left(), row.center().y), Align2::LEFT_CENTER, question, display(15.0, 700), ink());
                    let mut child = ui.new_child(UiBuilder::new().max_rect(Rect::from_min_size(Pos2::new(row.right() - buttons_w, row.top()), Vec2::new(buttons_w, 42.0))).layout(Layout::top_down(Align::Min)));
                    draw_buttons(&mut child, buttons_w, &mut picked);
                } else {
                    ui.label(RichText::new(question).font(display(15.0, 700)));
                    draw_buttons(ui, w, &mut picked);
                }
            } else {
                ui.horizontal(|ui| {
                    ui.add_space(4.0);
                    ui.label(RichText::new(question).font(display(16.0, 700)));
                });
                ui.add_space(4.0);
                draw_buttons(ui, w, &mut picked);
            }
        });
    picked
}

fn table(ui: &mut Ui, head: &[&str], rows: &[Vec<&str>]) {
    let w = ui.available_width();
    // First column sized to its longest entry; the rest share what's left.
    let first_w = rows.iter().map(|r| r[0]).chain(std::iter::once(head[0])).map(|t| ui.fonts(|f| f.layout_no_wrap(t.to_string(), font(13.5, 600), ink()).size().x)).fold(0.0, f32::max) + 16.0;
    let narrow_cols: Vec<f32> = (1..head.len()).map(|c| {
        let longest = rows.iter().map(|r| r[c]).chain(std::iter::once(head[c])).map(|t| ui.fonts(|f| f.layout_no_wrap(t.to_string(), font(13.5, 400), ink()).size().x)).fold(0.0, f32::max) + 16.0;
        longest
    }).collect();
    let rest = (w - first_w).max(60.0);
    let total_rest: f32 = narrow_cols.iter().sum();
    let widths: Vec<f32> = std::iter::once(first_w).chain(narrow_cols.iter().map(|c| if total_rest <= rest { c + (rest - total_rest) * c / total_rest } else { rest * c / total_rest })).collect();
    let row_ui = |ui: &mut Ui, cells: &[&str], header: bool, top_line: bool| {
        let galleys: Vec<_> = cells
            .iter()
            .enumerate()
            .map(|(i, t)| {
                let (f, c) = if header { (font(12.0, 600), white(0.6)) } else if i == 0 { (font(13.5, 600), ink()) } else { (font(13.5, 400), ink()) };
                ui.fonts(|fo| fo.layout(t.to_string(), f, c, widths[i] - 16.0))
            })
            .collect();
        let h = galleys.iter().map(|g| g.size().y).fold(0.0, f32::max) + if header { 12.0 } else { 16.0 };
        let (r, _) = ui.allocate_exact_size(Vec2::new(w, h), Sense::hover());
        if top_line {
            ui.painter().hline(r.x_range(), r.top(), Stroke::new(1.0_f32, white(0.1)));
        }
        let mut x = r.left();
        for (i, g) in galleys.into_iter().enumerate() {
            ui.painter().galley(Pos2::new(x + 8.0, r.top() + if header { 6.0 } else { 8.0 }), g, ink());
            x += widths[i];
        }
    };
    ui.spacing_mut().item_spacing.y = 0.0;
    row_ui(ui, head, true, false);
    for row in rows {
        row_ui(ui, row, false, true);
    }
}

fn footer_button(ui: &mut Ui, page: &Page) -> bool {
    let c = page.color;
    let g = ui.fonts(|f| f.layout_no_wrap(page.title.to_string(), font(13.0, 400), ink()));
    let (r, resp) = ui.allocate_exact_size(Vec2::new(g.size().x + 32.0 + 20.0, 32.0), Sense::click());
    let hover = ui.ctx().animate_bool_with_time(resp.id, resp.hovered(), 0.2);
    let r = r.translate(Vec2::new(0.0, -2.0 * hover));
    ui.painter().rect(r, CornerRadius::same(255), lerp_color(tint(c, 0.26), tint(c, 0.40), hover), Stroke::new(1.0_f32, tint(c, 0.55)), StrokeKind::Inside);
    emoji_image(page.icon).paint_at(ui, Rect::from_min_size(Pos2::new(r.left() + 16.0, r.center().y - 8.0), Vec2::splat(16.0)));
    ui.painter().galley(Pos2::new(r.left() + 16.0 + 20.0, r.center().y - g.size().y / 2.0), g, ink());
    if resp.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    resp.clicked()
}
