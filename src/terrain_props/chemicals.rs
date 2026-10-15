//! The Razorwing's plant: chemical drums everywhere in its lair, standing,
//! toppled and stacked, leaking into puddles, with stains running down the
//! walls and drips off the ceilings. Like the other bosses' colors, the
//! drums are gray props in the world and their chemical is drawn twice: gray,
//! and pink on the color layer, hidden until the Razorwing falls and
//! `regrowth` washes its pink back out from its home (`moss::dress`).

use crate::boss::BossKind;

use super::{moss, *};

/// Least distance between two drum groups, world units.
const GAP: f32 = 70.0;
/// Chance a free floor spot of the lair gets drums.
const CHANCE: f32 = 0.55;
/// Chance for a wall, ledge or ceiling of the lair to get a stain or drips.
const STAIN: f32 = 0.3;
/// Least distance between two stains.
const STAIN_GAP: f32 = 110.0;

/// The lair: the region slice the Razorwing's marker is in.
pub(super) fn in_lair(t: &Terrain, at: Vec2) -> bool {
    t.dragonfly.and_then(|d| t.area_at(d)).is_some_and(|a| a.rect.contains(at))
}

/// A pink of the chemical: around `#d1b0c6`, puddles a little deeper.
fn chemical(rng: &mut Rng, deep: bool) -> Color {
    let (sat, light) = if deep { (rng.range(0.38, 0.5), rng.range(0.6, 0.7)) } else { (rng.range(0.28, 0.38), rng.range(0.7, 0.78)) };
    Color::hsl(320.0 + rng.range(-6.0, 6.0), sat, light)
}

/// Maybe drums (floors) or stains (walls, ledges, ceilings) at the spot `s`
/// of the lair. True if something was placed.
pub(super) fn place(p: &mut Props, s: &Site, at: Vec2) -> bool {
    p.rng = Rng(s.key ^ 0x6368_656d);
    p.rng.next();
    match s.kind {
        Kind::Ground => drums(p, s, at),
        _ => stain(p, s, at),
    }
}

fn drums(p: &mut Props, s: &Site, at: Vec2) -> bool {
    let far = |q: &Vec2, d: f32| q.distance_squared(at) >= d * d;
    if !p.drums.iter().all(|q| far(q, GAP)) || !p.taken.iter().all(|(q, _)| far(q, 20.0 * S)) || !p.rng.chance(CHANCE) {
        return false;
    }
    // What: one standing, a pair, a stack, or one on its side.
    let kind = p.rng.below(10);
    let half = match kind {
        0..=3 => 9.0,
        4..=5 => 19.0,
        6..=7 => 19.0,
        _ => 14.0,
    };
    let Some((y, _, indoor)) = p.fit(s.cell, half + 14.0, 50.0, false) else { return false };
    let at = Vec2::new(at.x, y);
    let flip = if p.rng.chance(0.5) { 1.0 } else { -1.0 };
    p.depth = if indoor { INDOOR } else { BACK };
    let e = p.root(at, flip, 0.0);
    // The chemical: what leaks out, and the painted bands.
    let mut chem: Vec<Mesh> = Vec::new();
    match kind {
        0..=3 => {
            standing(p, e, 0.0, 0.0, &mut chem);
            let y = p.rng.range(5.0, 14.0);
            leak(p, Vec2::new(8.4, y), &mut chem);
        }
        4..=5 => {
            standing(p, e, -9.5, 0.0, &mut chem);
            standing(p, e, 9.5, 0.0, &mut chem);
            let y = p.rng.range(4.0, 12.0);
            leak(p, Vec2::new(17.9, y), &mut chem);
        }
        6..=7 => {
            standing(p, e, -9.5, 0.0, &mut chem);
            standing(p, e, 9.5, 0.0, &mut chem);
            let x = p.rng.range(-2.0, 2.0);
            standing(p, e, x, 24.5, &mut chem);
            let y = p.rng.range(4.0, 12.0);
            leak(p, Vec2::new(-17.9, y), &mut chem);
        }
        _ => toppled(p, e, &mut chem),
    }
    p.depth = BACK;
    let color = chemical(&mut p.rng, true);
    let dist = at.distance(p.t.dragonfly.unwrap_or(at));
    moss::dress(p, at, flip, 0.0, indoor, chem, color, M::Soot, BossKind::Dragonfly, dist, 0);
    p.drums.push(at);
    true
}

/// A drum standing with its bottom at (`x`, `y`) (prop units): steel or
/// rusted, two ribs, a lid; its painted band goes in `chem`.
fn standing(p: &mut Props, e: Entity, x: f32, y: f32, chem: &mut Vec<Mesh>) {
    let o = Vec2::new(x, y);
    let body: Vec<Vec2> = [(-8.0, 0.0), (8.0, 0.0), (8.4, 1.0), (8.4, 23.0), (8.0, 24.0), (-8.0, 24.0), (-8.4, 23.0), (-8.4, 1.0)]
        .into_iter()
        .map(|(a, b)| Vec2::new(a, b) + o)
        .collect();
    let m = if p.rng.chance(0.5) { M::Rust } else { M::Steel };
    p.shape(e, &body, 0.5, m, 0.0);
    for rib in [8.0, 16.0] {
        p.bar(e, &[o + Vec2::new(-8.4, rib), o + Vec2::new(8.4, rib)], 1.1, M::Iron, 0.01);
    }
    p.shape(e, &ellipse_pts(8.0, 1.2, 12).into_iter().map(|q| q + o + Vec2::new(0.0, 24.0)).collect::<Vec<_>>(), 0.4, M::Iron, 0.015);
    // A dent.
    if p.rng.chance(0.5) {
        let c = o + Vec2::new(p.rng.range(-5.0, 5.0), p.rng.range(3.0, 21.0));
        p.shape(e, &ellipse_pts(2.4, 1.6, 8).into_iter().map(|q| q + c).collect::<Vec<_>>(), 0.2, M::Soot, 0.02);
    }
    // The band between the ribs, with a hazard diamond.
    let band = [o + Vec2::new(-8.4, 9.5), o + Vec2::new(8.4, 9.5), o + Vec2::new(8.4, 14.5), o + Vec2::new(-8.4, 14.5)];
    let d = o + Vec2::new(p.rng.range(-3.0, 3.0), 12.0);
    let diamond = [d + Vec2::new(0.0, 2.2), d + Vec2::new(2.2, 0.0), d + Vec2::new(0.0, -2.2), d + Vec2::new(-2.2, 0.0)];
    chem.push(fill(&band, |q| 0.8 + 0.2 * ((q.y - o.y - 9.5) / 5.0)));
    p.part(e, fill(&diamond, |_| 1.0), M::Soot, 0.03);
}

/// A drum on its side, its open end spilling onto the floor.
fn toppled(p: &mut Props, e: Entity, chem: &mut Vec<Mesh>) {
    let body: Vec<Vec2> = [(-13.0, 0.0), (11.0, 0.0), (12.0, 1.0), (12.0, 15.0), (11.0, 16.0), (-13.0, 16.0), (-13.6, 15.0), (-13.6, 1.0)]
        .into_iter()
        .map(|(a, b)| Vec2::new(a, b))
        .collect();
    let m = if p.rng.chance(0.5) { M::Rust } else { M::Steel };
    p.shape(e, &body, 0.45, m, 0.0);
    for rib in [-5.0, 4.0] {
        p.bar(e, &[Vec2::new(rib, 0.0), Vec2::new(rib, 16.0)], 1.1, M::Iron, 0.01);
    }
    // The open end, dark inside.
    p.shape(e, &ellipse_pts(2.0, 7.6, 14).into_iter().map(|q| q + Vec2::new(12.0, 8.0)).collect::<Vec<_>>(), 0.3, M::Soot, 0.02);
    chem.push(fill(&[Vec2::new(-1.0, 0.0), Vec2::new(2.5, 0.0), Vec2::new(2.5, 16.0), Vec2::new(-1.0, 16.0)], |_| 0.85));
    // Pouring out of the mouth into a wide puddle.
    let pour = smooth_open(&[Vec2::new(12.0, 6.0), Vec2::new(15.0, 4.0), Vec2::new(17.0, 1.0)], 4);
    chem.push(stroke(&pour, |t| 4.0 - 2.0 * t, 1.0, 0.7, |_| 1.0));
    puddle(p, Vec2::new(24.0, 0.0), 14.0, chem);
}

/// A thin stream from a hole at `from` down to a puddle on the floor.
fn leak(p: &mut Props, from: Vec2, chem: &mut Vec<Mesh>) {
    let out = from.x.signum();
    let fall = smooth_open(&[from, from + Vec2::new(out * 2.5, -1.5), from + Vec2::new(out * 3.5, -from.y * 0.5), Vec2::new(from.x + out * 3.8, 0.5)], 5);
    chem.push(stroke(&fall, |t| 1.6 - 0.6 * t, 1.0, 0.7, |_| 1.0));
    let r = p.rng.range(8.0, 14.0);
    puddle(p, Vec2::new(from.x + out * (r * 0.6 + 2.0), 0.0), r, chem);
}

/// A puddle (with a few drops) around `c` on the floor.
fn puddle(p: &mut Props, c: Vec2, r: f32, chem: &mut Vec<Mesh>) {
    let outline: Vec<Vec2> = ellipse_pts(r, 1.5, 14).into_iter().map(|q| q + c + Vec2::new(0.0, 0.5)).collect();
    chem.push(fill(&outline, |q| 0.8 + 0.2 * (q.x * 0.6).sin().abs()));
    let drops: Vec<Vec<Vec2>> = (0..2 + p.rng.below(3))
        .map(|_| {
            let d = c + Vec2::new(p.rng.range(-r * 1.4, r * 1.4), 0.6);
            ellipse_pts(p.rng.range(1.0, 2.0), 0.7, 8).into_iter().map(|q| q + d).collect()
        })
        .collect();
    chem.push(blobs(&drops, |_| 0.9));
}

/// Chemical running down a wall or off a ledge, dripping off a ceiling.
fn stain(p: &mut Props, s: &Site, at: Vec2) -> bool {
    let far = |q: &Vec2, d: f32| q.distance_squared(at) >= d * d;
    if !p.stains.iter().all(|q| far(q, STAIN_GAP)) || !p.taken.iter().all(|(q, _)| far(q, 20.0 * S)) || !p.rng.chance(STAIN) {
        return false;
    }
    let meshes = match s.kind {
        Kind::Wall => {
            if p.rng.chance(0.5) {
                moss::streaks(&mut p.rng)
            } else {
                moss::slime_fall(&mut p.rng)
            }
        }
        Kind::Ceiling => {
            if p.rng.chance(0.5) {
                moss::drips(&mut p.rng)
            } else {
                moss::drip(&mut p.rng)
            }
        }
        _ => moss::slime_fall(&mut p.rng),
    };
    let flip = if s.kind == Kind::Ceiling { 1.0 } else { s.dir as f32 };
    let t = p.t;
    let indoor = t.cell(t.to_cell(at + Vec2::Y * 6.0)) == Cell::Back || t.cell(t.to_cell(at + Vec2::new(flip * 6.0, 0.0))) == Cell::Back;
    let color = chemical(&mut p.rng, false);
    let dist = at.distance(t.dragonfly.unwrap_or(at));
    moss::dress(p, at, flip, 0.0, indoor, meshes, color, M::Soot, BossKind::Dragonfly, dist, 0);
    p.stains.push(at);
    true
}
