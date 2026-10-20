//! Street names for any spot on the map: the nearest street, and whether the
//! spot is at a corner or an intersection (and of which streets).
//!
//! The roads come from the game's own road network (every named road point),
//! lined up with the map pictures, packed in `assets/map/roads.bin`:
//!   u32 street count, then per street: u8 name length, name, u8 freeway, u16 size
//!   u32 point count, then per point: u16 street, f32 x, f32 y   (map units)

use std::sync::OnceLock;

pub struct Street {
    pub name: String,
    pub freeway: bool,
    /// Bigger = bigger road (lanes, then Freeway > Highway > Boulevard > Avenue > Street…).
    pub size: u16,
}

struct Roads {
    streets: Vec<Street>,
    /// (street, x, y)
    points: Vec<(u16, f32, f32)>,
}

fn roads() -> &'static Roads {
    static ROADS: OnceLock<Roads> = OnceLock::new();
    ROADS.get_or_init(|| {
        let b: &[u8] = include_bytes!("../assets/map/roads.bin");
        let mut at = 0usize;
        let mut take = |n: usize| {
            let s = &b[at..at + n];
            at += n;
            s
        };
        let u32_ = |s: &[u8]| u32::from_le_bytes(s.try_into().unwrap());
        let count = u32_(take(4)) as usize;
        let mut streets = Vec::with_capacity(count);
        for _ in 0..count {
            let len = take(1)[0] as usize;
            let name = String::from_utf8_lossy(take(len)).into_owned();
            let freeway = take(1)[0] == 1;
            let size = u16::from_le_bytes(take(2).try_into().unwrap());
            streets.push(Street { name, freeway, size });
        }
        let n = u32_(take(4)) as usize;
        let mut points = Vec::with_capacity(n);
        for _ in 0..n {
            let s = u16::from_le_bytes(take(2).try_into().unwrap());
            let x = f32::from_le_bytes(take(4).try_into().unwrap());
            let y = f32::from_le_bytes(take(4).try_into().unwrap());
            points.push((s, x, y));
        }
        Roads { streets, points }
    })
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    /// Two streets cross: four ways.
    Intersection,
    /// One street ends at the other: three ways.
    TIntersection,
    /// Two streets meet and both end there: the road turns the corner.
    Corner,
}

impl Kind {
    pub fn label(self) -> &'static str {
        match self {
            Kind::Intersection => "Intersection",
            Kind::TIntersection => "T-intersection",
            Kind::Corner => "Corner",
        }
    }
}

#[derive(Clone, Debug)]
pub struct Crossing {
    /// Biggest street first.
    pub streets: [String; 2],
    pub kind: Kind,
    /// Where they meet, in map units.
    pub at: (f32, f32),
}

#[derive(Clone, Debug)]
pub struct Place {
    /// The nearest street (not a freeway, unless a freeway is all there is).
    pub street: String,
    /// The nearest corner or intersection, if there is one close by.
    pub crossing: Option<Crossing>,
}

/// How far to look for a corner or intersection, in map units (1 unit ≈ 3 m).
const REACH: f32 = 45.0;

fn dist(a: (f32, f32), b: (f32, f32)) -> f32 {
    ((a.0 - b.0).powi(2) + (a.1 - b.1).powi(2)).sqrt()
}

/// How many ways a street leaves a point (1 = it ends there, 2 = it carries on through).
fn ways_out(street: u16, at: (f32, f32)) -> usize {
    let mut dirs: Vec<f32> = Vec::new();
    for &(s, x, y) in &roads().points {
        if s != street {
            continue;
        }
        let d = dist((x, y), at);
        if d <= 4.0 || d >= 40.0 {
            continue;
        }
        let a = (y - at.1).atan2(x - at.0).to_degrees();
        if !dirs.iter().any(|b| ((a - b + 540.0) % 360.0 - 180.0).abs() < 50.0) {
            dirs.push(a);
        }
    }
    dirs.len()
}

/// Describe a spot on the map (map units).
pub fn describe(x: f32, y: f32) -> Option<Place> {
    let r = roads();
    let p = (x, y);
    // Closest distance to each street around the spot.
    let mut near: Vec<(u16, f32)> = Vec::new();
    let mut nearest_any: Option<(u16, f32)> = None;
    for &(s, px, py) in &r.points {
        let d = dist((px, py), p);
        if nearest_any.is_none_or(|n| d < n.1) {
            nearest_any = Some((s, d));
        }
        if d < REACH + 40.0 {
            match near.iter_mut().find(|n| n.0 == s) {
                Some(n) => n.1 = n.1.min(d),
                None => near.push((s, d)),
            }
        }
    }
    let (fallback, _) = nearest_any?;
    near.sort_by(|a, b| a.1.total_cmp(&b.1));
    let main = near.iter().find(|n| !r.streets[n.0 as usize].freeway).or(near.first()).map_or(fallback, |n| n.0);

    // Corners and intersections: pairs of nearby streets that meet close to the spot.
    let local: Vec<u16> = near.iter().filter(|n| !r.streets[n.0 as usize].freeway && n.1 < REACH + 10.0).take(8).map(|n| n.0).collect();
    let pts = |s: u16| -> Vec<(f32, f32)> { r.points.iter().filter(|q| q.0 == s && dist((q.1, q.2), p) < REACH + 40.0).map(|q| (q.1, q.2)).collect() };
    let mut best: Option<(f32, Crossing)> = None;
    for i in 0..local.len() {
        let pa = pts(local[i]);
        for &sb in &local[i + 1..] {
            let pb = pts(sb);
            let mut closest = (f32::MAX, (0.0, 0.0));
            for &a in &pa {
                for &b in &pb {
                    let d = dist(a, b);
                    if d < closest.0 {
                        closest = (d, ((a.0 + b.0) / 2.0, (a.1 + b.1) / 2.0));
                    }
                }
            }
            if closest.0 > 14.0 {
                continue;
            }
            let at = closest.1;
            let how_far = dist(at, p);
            if how_far > REACH {
                continue;
            }
            let ways = (ways_out(local[i], at) + ways_out(sb, at)).clamp(2, 4);
            let kind = match ways {
                4 => Kind::Intersection,
                3 => Kind::TIntersection,
                _ => Kind::Corner,
            };
            let (a, b) = (&r.streets[local[i] as usize], &r.streets[sb as usize]);
            let score = how_far - if local[i] == main || sb == main { 12.0 } else { 0.0 } - 0.15 * (a.size + b.size) as f32;
            let streets = if (b.size, &a.name) > (a.size, &b.name) { [b.name.clone(), a.name.clone()] } else { [a.name.clone(), b.name.clone()] };
            if best.as_ref().is_none_or(|bst| score < bst.0) {
                best = Some((score, Crossing { streets, kind, at }));
            }
        }
    }
    Some(Place { street: r.streets[main as usize].name.clone(), crossing: best.map(|b| b.1) })
}

/// Just the nearest street (cheap enough to run every frame under the mouse).
pub fn nearest_street(x: f32, y: f32) -> Option<&'static str> {
    let r = roads();
    let mut best: Option<(u16, f32)> = None;
    let mut best_land: Option<(u16, f32)> = None;
    for &(s, px, py) in &r.points {
        let d = dist((px, py), (x, y));
        if best.is_none_or(|b| d < b.1) {
            best = Some((s, d));
        }
        if !r.streets[s as usize].freeway && best_land.is_none_or(|b| d < b.1) {
            best_land = Some((s, d));
        }
    }
    let pick = match (best_land, best) {
        (Some(l), _) if l.1 < 85.0 => l,
        (_, Some(b)) => b,
        _ => return None,
    };
    Some(r.streets[pick.0 as usize].name.as_str())
}

#[cfg(test)]
mod tests {
    /// Prints the nearest street of every postal (MFB_LIST_STREETS=1).
    #[test]
    fn list_postal_streets() {
        if std::env::var("MFB_LIST_STREETS").is_err() {
            return;
        }
        let mut names: Vec<String> = crate::postals::POSTALS.iter().filter_map(|&(_, x, y)| super::describe(x, y).map(|p| p.street)).collect();
        names.sort();
        names.dedup();
        for n in names {
            println!("STREET\t{n}");
        }
        let mut crossing: Vec<String> = crate::postals::POSTALS.iter().filter_map(|&(_, x, y)| super::describe(x, y).and_then(|p| p.crossing)).flat_map(|c| c.streets).collect();
        crossing.sort();
        crossing.dedup();
        for n in crossing {
            println!("CROSS\t{n}");
        }
        let kinds: Vec<String> = crate::postals::POSTALS.iter().filter_map(|&(_, x, y)| super::describe(x, y)).map(|p| match &p.crossing { None => "none".to_string(), Some(c) => format!("{:?} main_in={}", c.kind, c.streets.contains(&p.street)) }).collect();
        let mut counts = std::collections::BTreeMap::new();
        for k in kinds { *counts.entry(k).or_insert(0) += 1; }
        println!("KINDS\t{counts:?}");
    }
}
