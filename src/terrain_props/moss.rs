//! Hints of green in the other regions: the Venom Mother's vines are the
//! thick of it, but green was everywhere before the war, so here and there
//! (in clumps, with long bare stretches between) a region keeps what fits its
//! place: weeds in the cracks of the concrete, moss on the old walls, green
//! slime pooling and dripping in the sewers, algae on the dam, cave moss.
//! Each hint has its own green (a little lighter or darker, a little off in
//! hue) and, like the vines, a gray copy in the world and a living copy on the
//! color layer that fades in when the green is earned (`regrowth`).

use std::f32::consts::{PI, TAU};

use bevy::sprite_render::AlphaMode2d;

use crate::{camera, regrowth::Vine};

use super::*;

/// The least distance between two hints, world units.
const GAP: f32 = 170.0;
/// How much more often than the themes' chances (those are per eligible spot).
const DENSITY: f32 = 2.4;

struct Theme {
    hue: f32,
    sat: f32,
    light: f32,
    /// Chance for an eligible spot (before the clumping noise).
    chance: f32,
}

fn theme(style: Style) -> Theme {
    let (hue, sat, light, chance) = match style {
        Style::Concrete => (95.0, 0.45, 0.38, 0.10),
        Style::Trash => (100.0, 0.40, 0.33, 0.12),
        Style::Sewer => (85.0, 0.55, 0.36, 0.22),
        Style::Park => (110.0, 0.50, 0.40, 0.18),
        Style::Subway => (120.0, 0.22, 0.27, 0.10),
        Style::Treatment => (80.0, 0.60, 0.38, 0.22),
        Style::Sinkhole => (125.0, 0.50, 0.36, 0.20),
        Style::Catacombs => (135.0, 0.35, 0.28, 0.14),
        Style::Mall => (105.0, 0.45, 0.38, 0.10),
        Style::Dam => (150.0, 0.40, 0.30, 0.18),
        Style::Bunker => (100.0, 0.18, 0.27, 0.07),
        Style::Cave => (150.0, 0.50, 0.30, 0.22),
    };
    Theme { hue, sat, light, chance }
}

/// Maybe a hint of `tone` at the spot `s` (`at` = its point on the surface).
pub(super) fn hint(p: &mut Props, s: &Site, at: Vec2, style: Style, tone: Tone) {
    let yellow = tone == Tone::Yellow;
    let th = if yellow { theme_yellow(style) } else { theme(style) };
    let far = |q: &Vec2, d: f32| q.distance_squared(at) >= d * d;
    let others = if yellow { &p.yellow } else { &p.hints };
    if !others.iter().all(|q| far(q, GAP)) || !p.taken.iter().all(|(q, _)| far(q, 20.0 * S)) {
        return;
    }
    p.rng = Rng(s.key ^ if yellow { 0x7965_6c6c } else { 0x6d6f_7373 });
    p.rng.next();
    // Clumps: some stretches of the region get many, most none.
    let clump = (vines::noise(at, 600.0, if yellow { 17 } else { 9 }) * 2.0).powi(2).min(2.5);
    // The Gutter King's region (his sewer and the dump beside it) is where the yellow is thick.
    let home = if yellow { 1.0 + 3.0 * (1.0 - at.distance(p.t.roach.unwrap_or(at + Vec2::splat(1e5))) / 2600.0).clamp(0.0, 1.0) } else { 1.0 };
    if !p.rng.chance(th.chance * DENSITY * clump * home) {
        return;
    }
    let flip = if s.kind == Kind::Ground {
        if p.rng.chance(0.5) { 1.0 } else { -1.0 }
    } else {
        s.dir as f32
    };
    let (mut at, mut tilt, mut indoor) = (at, 0.0, false);
    if s.kind == Kind::Ground {
        let Some((y, angle, inside)) = p.fit(s.cell, 14.0, 24.0, true) else { return };
        (at.y, tilt, indoor) = (y, angle, inside);
    }
    let build = choose(&mut p.rng, if yellow { table_yellow(style, s.kind) } else { table(style, s.kind) });
    let meshes = build(&mut p.rng);
    let t = p.t;
    indoor |= t.cell(t.to_cell(at + Vec2::Y * 6.0)) == Cell::Back || t.cell(t.to_cell(at + Vec2::new(flip * 6.0, 0.0))) == Cell::Back;
    // A little off in hue, lighter or darker, each its own.
    let hue = th.hue + p.rng.range(-12.0, 12.0) * if yellow { 0.6 } else { 1.0 };
    let light = (th.light * p.rng.range(0.75, 1.3)).min(if yellow { 0.68 } else { 0.6 });
    let color = Color::hsl(hue, (th.sat * p.rng.range(0.8, 1.15)).min(1.0), light);
    let gray = if light > if yellow { 0.42 } else { 0.34 } { M::Concrete } else { M::Soot };

    let home_of = if yellow { t.roach } else { t.wasp };
    let dist = at.distance(home_of.unwrap_or(at));
    let boss = if yellow { crate::boss::BossKind::Roach } else { crate::boss::BossKind::Wasp };
    dress(p, at, flip, tilt, indoor, meshes, color, gray, boss, dist);
    if yellow {
        p.yellow.push(at);
    } else {
        p.hints.push(at);
    }
    p.taken.push((at, s.kind));
}

fn fuzz(q: Vec2) -> f32 {
    0.72 + 0.28 * ((q.x * 0.8 + q.y * 1.9).sin() * 0.5 + 0.5)
}

fn ellipses(rng: &mut Rng, centers: &[Vec2], r: (f32, f32)) -> Vec<Vec<Vec2>> {
    centers
        .iter()
        .map(|&c| {
            let rx = rng.range(r.0, r.1);
            let ry = rx * rng.range(0.6, 0.9);
            ellipse_pts(rx, ry, 8).into_iter().map(|q| q + c).collect()
        })
        .collect()
}

/// Weeds: blades in the cracks.
fn tufts(rng: &mut Rng) -> Vec<Mesh> {
    let tris: Vec<[Vec2; 3]> = (0..5 + rng.below(5))
        .map(|_| {
            let (x, h, bend, w) = (rng.range(-7.0, 7.0), rng.range(5.0, 13.0), rng.range(-4.0, 4.0), rng.range(0.9, 1.5));
            [Vec2::new(x - w, -0.5), Vec2::new(x + w, -0.5), Vec2::new(x + bend, h)]
        })
        .collect();
    vec![triangles(&tris)]
}

/// A low cushion of moss (`x` = where its middle sits).
fn mound_at(rng: &mut Rng, x: f32) -> Vec<Mesh> {
    let (n, w, hump) = (5 + rng.below(4), rng.range(6.0, 12.0), rng.range(2.0, 4.0));
    let centers: Vec<Vec2> = (0..n)
        .map(|i| {
            let t = i as f32 / (n - 1) as f32 * 2.0 - 1.0;
            Vec2::new(x + t * w + rng.range(-1.0, 1.0), 1.5 + hump * (1.0 - t * t) + rng.range(0.0, 1.5))
        })
        .collect();
    vec![blobs(&ellipses(rng, &centers, (2.5, 4.5)), fuzz)]
}

/// Moss on a wall's face.
fn patch(rng: &mut Rng) -> Vec<Mesh> {
    let centers: Vec<Vec2> = (0..6 + rng.below(5)).map(|_| Vec2::new(rng.range(1.5, 3.5), rng.range(-12.0, 12.0))).collect();
    vec![blobs(&ellipses(rng, &centers, (2.2, 4.0)), fuzz)]
}

/// Green slime (or algae) standing on the floor.
fn pool(rng: &mut Rng) -> Vec<Mesh> {
    let rx = rng.range(8.0, 14.0);
    let outline: Vec<Vec2> = ellipse_pts(rx, 1.6, 12).into_iter().map(|q| q + Vec2::new(0.0, 0.6)).collect();
    vec![fill(&outline, |q| 0.8 + 0.2 * (q.x * 0.5).sin().abs())]
}

/// A strand of slime off a ceiling.
fn drip(rng: &mut Rng) -> Vec<Mesh> {
    let len = rng.range(10.0, 40.0);
    let x = rng.range(-3.0, 3.0);
    let path = smooth_open(&[Vec2::new(x, 1.0), Vec2::new(x + 0.4, -len * 0.5), Vec2::new(x, -len)], 4);
    let drop: Vec<Vec2> = ellipse_pts(1.5, 2.4, 8).into_iter().map(|q| q + Vec2::new(x, -len - 1.5)).collect();
    vec![stroke(&path, |t| 1.8 * (1.0 - 0.5 * t), 1.0, 0.7, |_| 1.0), fill(&drop, |_| 1.0)]
}

/// Moss hanging in strands, from `x0..x1` across, `l0..l1` long.
fn hanging(rng: &mut Rng, x0: f32, x1: f32, l0: f32, l1: f32) -> Vec<Mesh> {
    let mut out = Vec::new();
    let mut leaves = Vec::new();
    for _ in 0..2 + rng.below(3) {
        let (x, len, sway) = (rng.range(x0, x1), rng.range(l0, l1), rng.range(-2.5, 2.5));
        let path = smooth_open(&[Vec2::new(x, 1.0), Vec2::new(x + sway * 0.4, -len * 0.5), Vec2::new(x + sway, -len)], 5);
        for k in [0.4, 0.7, 1.0] {
            leaves.push(path[((path.len() - 1) as f32 * k) as usize]);
        }
        out.push(stroke(&path, |t| 1.2 * (1.0 - 0.6 * t) + 0.3, 1.0, 0.7, |_| 1.0));
    }
    out.push(blobs(&ellipses(rng, &leaves, (1.6, 2.8)), fuzz));
    out
}

type Build = fn(&mut Rng) -> Vec<Mesh>;

fn choose(rng: &mut Rng, table: &[(Build, f32)]) -> Build {
    let mut r = rng.range(0.0, table.iter().map(|e| e.1).sum());
    for e in table {
        r -= e.1;
        if r <= 0.0 {
            return e.0;
        }
    }
    table[0].0
}

/// What each region grows on each kind of surface, with its weights: the
/// common mosses and weeds, and what is specific of the place.
fn table(style: Style, kind: Kind) -> &'static [(Build, f32)] {
    match (style, kind) {
        // Ruined buildings: saplings out of the rubble, creepers up the walls.
        (Style::Concrete, Kind::Ground) => &[(tufts, 3.0), (mound, 1.0), (sapling, 2.0), (bush, 1.0)],
        (Style::Concrete | Style::Mall | Style::Trash | Style::Park, Kind::Wall) => &[(creeper, 3.0), (patch, 1.0)],
        // The dump: weeds through the tires, scrubby bushes.
        (Style::Trash, Kind::Ground) => &[(tufts, 3.0), (mound, 1.0), (bush, 2.0), (tire_weeds, 2.0)],
        // The sewer: slime pools, falls and curtains.
        (Style::Sewer, Kind::Ground) => &[(pool, 3.0), (bubble_pool, 2.0), (mound, 2.0), (tufts, 1.0)],
        (Style::Sewer | Style::Treatment, Kind::Wall) => &[(slime_fall, 3.0), (streaks, 2.0), (patch, 1.0)],
        // The park: the greenest of the dead ones.
        (Style::Park, Kind::Ground) => &[(tufts, 3.0), (bush, 3.0), (sapling, 2.0), (mound, 1.0)],
        // The subway: moss in the tile joints, weeds in the ballast.
        (Style::Subway, Kind::Ground) => &[(weeds_row, 3.0), (tufts, 2.0), (pool, 1.0), (mound, 1.0)],
        (Style::Subway, Kind::Wall) => &[(stripes, 3.0), (patch, 1.0)],
        // The treatment plant: algae blooms.
        (Style::Treatment, Kind::Ground) => &[(bubble_pool, 3.0), (pool, 2.0), (reeds, 1.0), (mound, 1.0)],
        // The sinkhole and the cave: ferns and roots.
        (Style::Sinkhole, Kind::Ground) => &[(fern, 3.0), (mound, 2.0), (tufts, 2.0), (bush, 1.0)],
        // Underground, only what grows without light: moss.
        (Style::Cave, Kind::Ground) => &[(mound, 4.0), (carpet, 3.0), (mossy_stalagmite, 2.0)],
        (Style::Sinkhole, Kind::Wall) => &[(patch, 2.0), (creeper, 2.0), (stripes, 1.0)],
        (Style::Cave | Style::Catacombs, Kind::Wall) => &[(patch, 3.0), (stripes, 1.0)],
        // The catacombs: moss on old stone.
        (Style::Catacombs, Kind::Ground) => &[(mound, 3.0), (carpet, 2.0), (mossy_stalagmite, 1.0)],
        // The shopping center: weeds in the tile cracks.
        (Style::Mall, Kind::Ground) => &[(weeds_row, 3.0), (tufts, 2.0), (sapling, 1.0)],
        // The dam: reeds and algae streaks.
        (Style::Dam, Kind::Ground) => &[(reeds, 3.0), (mound, 2.0), (pool, 1.0), (tufts, 1.0)],
        (Style::Dam, Kind::Wall) => &[(streaks, 3.0), (slime_fall, 1.0), (patch, 1.0)],
        // The bunker: lichen.
        (Style::Bunker, Kind::Ground) => &[(tufts, 3.0), (mound, 1.0)],
        (Style::Bunker, Kind::Wall) => &[(rosettes, 3.0), (patch, 1.0)],

        (Style::Sewer | Style::Treatment | Style::Dam, Kind::Edge) => &[(slime_curtain, 3.0), (hang_edge, 1.0)],
        (_, Kind::Edge) => &[(hang_edge, 2.0), (mound_edge, 2.0)],
        (Style::Sewer | Style::Treatment | Style::Subway | Style::Dam, Kind::Ceiling) => &[(drips, 3.0), (drip, 2.0)],
        (Style::Cave | Style::Catacombs | Style::Sinkhole, Kind::Ceiling) => &[(roots_moss, 3.0), (hang, 2.0)],
        (Style::Mall, Kind::Ceiling) => &[(hang_long, 3.0), (hang, 1.0)],
        (_, Kind::Ceiling) => &[(hang, 2.0), (drip, 1.0)],
    }
}

fn hang_edge(rng: &mut Rng) -> Vec<Mesh> {
    hanging(rng, 1.5, 3.5, 6.0, 22.0)
}
fn hang(rng: &mut Rng) -> Vec<Mesh> {
    hanging(rng, -6.0, 6.0, 8.0, 26.0)
}
fn hang_long(rng: &mut Rng) -> Vec<Mesh> {
    hanging(rng, -8.0, 8.0, 22.0, 52.0)
}
fn roots_moss(rng: &mut Rng) -> Vec<Mesh> {
    hanging(rng, -5.0, 5.0, 18.0, 42.0)
}
fn mound_edge(rng: &mut Rng) -> Vec<Mesh> {
    mound_at(rng, -5.0)
}

/// A small tree out of the rubble.
fn sapling(rng: &mut Rng) -> Vec<Mesh> {
    let (h, lean) = (rng.range(12.0, 22.0), rng.range(-3.0, 3.0));
    let trunk = smooth_open(&[Vec2::ZERO, Vec2::new(lean * 0.4, h * 0.5), Vec2::new(lean, h)], 4);
    let mut out = vec![stroke(&trunk, |t| 1.8 * (1.0 - 0.7 * t) + 0.3, 1.0, 0.6, |_| 1.0)];
    let mut tops = vec![Vec2::new(lean, h)];
    for k in [0.45, 0.7] {
        let from = trunk[((trunk.len() - 1) as f32 * k) as usize];
        let end = from + Vec2::new(rng.range(-7.0, 7.0), rng.range(3.0, 7.0));
        out.push(stroke(&smooth_open(&[from, (from + end) * 0.5 + Vec2::Y, end], 3), |t| 1.0 - 0.6 * t + 0.2, 1.0, 0.6, |_| 1.0));
        tops.push(end);
    }
    out.push(blobs(&ellipses(rng, &tops, (3.0, 5.0)), fuzz));
    out
}

/// A scrubby round bush.
fn bush(rng: &mut Rng) -> Vec<Mesh> {
    let (w, h) = (rng.range(6.0, 10.0), rng.range(6.0, 10.0));
    let centers: Vec<Vec2> = (0..7 + rng.below(4))
        .map(|_| {
            let a = rng.range(0.1, PI - 0.1);
            let r = rng.range(0.3, 1.0);
            Vec2::new(a.cos() * w * r, 2.0 + a.sin() * h * r)
        })
        .collect();
    vec![blobs(&ellipses(rng, &centers, (3.0, 5.0)), fuzz)]
}

/// An old tire grown over with weeds.
fn tire_weeds(rng: &mut Rng) -> Vec<Mesh> {
    let r = rng.range(8.0, 11.0);
    let mut ring: Vec<Vec2> = ellipse_pts(r, r * 0.35, 16).into_iter().map(|q| q + Vec2::new(0.0, r * 0.35)).collect();
    ring.push(ring[0]);
    let mut out = vec![stroke(&ring, |_| 1.6, 1.0, 0.8, |_| 1.0)];
    out.extend(tufts(rng));
    out
}

/// Slime with bubbles.
fn bubble_pool(rng: &mut Rng) -> Vec<Mesh> {
    let mut out = pool(rng);
    let bubbles: Vec<Vec2> = (0..4 + rng.below(4)).map(|_| Vec2::new(rng.range(-7.0, 7.0), rng.range(1.5, 3.5))).collect();
    out.push(blobs(&ellipses(rng, &bubbles, (0.9, 1.8)), |_| 1.0));
    out
}

/// Cattails and reeds.
fn reeds(rng: &mut Rng) -> Vec<Mesh> {
    let mut tris = Vec::new();
    let mut tops: Vec<Vec<Vec2>> = Vec::new();
    for _ in 0..5 + rng.below(4) {
        let (x, h, bend) = (rng.range(-8.0, 8.0), rng.range(14.0, 30.0), rng.range(-3.0, 3.0));
        tris.push([Vec2::new(x - 0.6, -0.5), Vec2::new(x + 0.6, -0.5), Vec2::new(x + bend, h)]);
        if rng.chance(0.4) {
            let c = Vec2::new(x + bend * 0.9, h - 2.5);
            tops.push(ellipse_pts(0.9, 2.6, 6).into_iter().map(|q| q + c).collect());
        }
    }
    let mut out = vec![triangles(&tris)];
    if !tops.is_empty() {
        out.push(blobs(&tops, |_| 1.0));
    }
    out
}

/// A fern: fronds arching out of one point, with leaflets.
fn fern(rng: &mut Rng) -> Vec<Mesh> {
    let mut out = Vec::new();
    let mut leaflets = Vec::new();
    for i in 0..6 + rng.below(4) {
        let side = if i % 2 == 0 { 1.0 } else { -1.0 };
        let (reach, h) = (rng.range(5.0, 13.0), rng.range(7.0, 15.0));
        let tip = Vec2::new(side * reach, h * rng.range(0.4, 0.9));
        let path = smooth_open(&[Vec2::ZERO, Vec2::new(side * reach * 0.35, h), tip], 5);
        for k in (2..path.len()).step_by(2) {
            let p = path[k];
            let d = (path[k] - path[k - 1]).normalize_or(Vec2::Y);
            let w = 3.2 * (1.0 - k as f32 / path.len() as f32) + 0.8;
            for sd in [-1.0, 1.0] {
                leaflets.push([p, p + d * 1.8 + d.perp() * sd * w, p + d * 0.4 + d.perp() * sd * w * 0.3]);
            }
        }
        out.push(stroke(&path, |t| 0.9 * (1.0 - t) + 0.2, 1.0, 0.7, |_| 1.0));
    }
    out.push(triangles(&leaflets));
    out
}

/// Weeds along a crack in the floor.
fn weeds_row(rng: &mut Rng) -> Vec<Mesh> {
    let tris: Vec<[Vec2; 3]> = (0..8 + rng.below(7))
        .map(|_| {
            let (x, h, bend, w) = (rng.range(-18.0, 18.0), rng.range(3.0, 7.0), rng.range(-2.0, 2.0), rng.range(0.6, 1.0));
            [Vec2::new(x - w, -0.5), Vec2::new(x + w, -0.5), Vec2::new(x + bend, h)]
        })
        .collect();
    vec![triangles(&tris)]
}

/// A creeper climbing the wall, leaves on the open side.
fn creeper(rng: &mut Rng) -> Vec<Mesh> {
    let (base, top) = (rng.range(-30.0, -12.0), rng.range(12.0, 32.0));
    let (phase, freq) = (rng.range(0.0, TAU), rng.range(0.12, 0.22));
    let pts: Vec<Vec2> = (0..=8)
        .map(|i| {
            let y = base + (top - base) * i as f32 / 8.0;
            Vec2::new(1.8 + 1.2 * (phase + freq * y).sin(), y)
        })
        .collect();
    let path = smooth_open(&pts, 4);
    let mut leaves = Vec::new();
    for k in (3..path.len()).step_by(3) {
        let side = if (k / 3) % 2 == 0 { 0.5 } else { -0.5 };
        let a = side + rng.range(-0.3, 0.3);
        let size = rng.range(4.0, 6.5);
        let r = Vec2::from_angle(a);
        let c = path[k] + r * size * 0.5;
        leaves.push(ellipse_pts(size * 0.5, size * 0.27, 8).into_iter().map(|q| c + r.rotate(q)).collect());
    }
    vec![stroke(&path, |t| 1.3 * (1.0 - 0.5 * t) + 0.3, 1.0, 0.6, |_| 1.0), blobs(&leaves, fuzz)]
}

/// Green slime running down a wall from a leaking pipe.
fn slime_fall(rng: &mut Rng) -> Vec<Mesh> {
    let (top, len) = (rng.range(8.0, 24.0), rng.range(25.0, 50.0));
    let (phase, x) = (rng.range(0.0, TAU), rng.range(1.0, 1.6));
    let path: Vec<Vec2> = (0..=10)
        .map(|i| {
            let y = top - len * i as f32 / 10.0;
            Vec2::new(x + 0.5 * (phase + y * 0.3).sin(), y)
        })
        .collect();
    let puddle: Vec<Vec2> = ellipse_pts(4.5, 1.8, 10).into_iter().map(|q| q + Vec2::new(x + 1.0, top - len)).collect();
    vec![stroke(&path, |t| 2.6 - 0.8 * t, 1.0, 0.7, |t| 0.8 + 0.2 * (t * 9.0).sin().abs()), fill(&puddle, |_| 1.0)]
}

/// Algae streaks down a wall.
fn streaks(rng: &mut Rng) -> Vec<Mesh> {
    let mut tris = Vec::new();
    for _ in 0..4 + rng.below(4) {
        let (x, y0, len, w) = (rng.range(0.4, 1.2), rng.range(-6.0, 22.0), rng.range(10.0, 34.0), rng.range(1.0, 2.6));
        let xs = rng.range(-1.0, 1.0);
        tris.push([Vec2::new(x, y0), Vec2::new(x + w, y0), Vec2::new(x + w * 0.5 + xs, y0 - len)]);
        tris.push([Vec2::new(x, y0), Vec2::new(x + w * 0.5 + xs, y0 - len), Vec2::new(x - 0.1, y0 - len * 0.6)]);
    }
    vec![triangles(&tris)]
}

/// Moss in the joints of tiled walls.
fn stripes(rng: &mut Rng) -> Vec<Mesh> {
    let mut tris = Vec::new();
    let mut knots = Vec::new();
    let (x0, y0) = (rng.range(0.0, 1.0), rng.range(-8.0, 8.0));
    for i in 0..2 + rng.below(2) {
        let y = y0 + i as f32 * 9.0;
        let len = rng.range(8.0, 16.0);
        let (a, b, c, d) = (Vec2::new(x0, y), Vec2::new(x0 + 1.4, y), Vec2::new(x0 + 1.4, y + len), Vec2::new(x0, y + len));
        tris.push([a, b, c]);
        tris.push([a, c, d]);
        knots.push(Vec2::new(x0 + 0.7, y + rng.range(0.0, len)));
    }
    vec![triangles(&tris), blobs(&ellipses(rng, &knots, (1.8, 3.2)), fuzz)]
}

/// Lichen rosettes.
fn rosettes(rng: &mut Rng) -> Vec<Mesh> {
    let mut discs = Vec::new();
    for _ in 0..2 + rng.below(3) {
        let c = Vec2::new(1.2, rng.range(-12.0, 12.0));
        let r = rng.range(2.5, 4.5);
        discs.push(ellipse_pts(r * 0.5, r, 10).into_iter().map(|q| q * 0.7 + c).collect());
        for k in 0..6 {
            let a = k as f32 / 6.0 * TAU;
            let at = c + Vec2::new(a.cos() * r * 0.35, a.sin() * r);
            discs.push(ellipse_pts(r * 0.3, r * 0.3, 6).into_iter().map(|q| q + at).collect());
        }
    }
    vec![blobs(&discs, fuzz)]
}

/// A curtain of slime hanging off a lip.
fn slime_curtain(rng: &mut Rng) -> Vec<Mesh> {
    let (len, width, folds) = (rng.range(14.0, 34.0), rng.range(5.0, 9.0), 3 + rng.below(3));
    let mut outline = vec![Vec2::new(-4.0, 2.0), Vec2::new(width * 0.6, 1.5), Vec2::new(width, -len * 0.3)];
    for i in 0..=folds * 2 {
        let t = i as f32 / (folds * 2) as f32;
        let x = width - t * (width + 3.0);
        let y = -len * (0.75 + 0.25 * (1.0 - (t * 2.0 - 1.0).abs())) + if i % 2 == 0 { -3.0 } else { 2.0 };
        outline.push(Vec2::new(x, y));
    }
    outline.push(Vec2::new(-3.0, -len * 0.4));
    let outline = smooth_closed(&outline, 2);
    vec![fill(&outline, |q| 0.6 + 0.4 * ((q.y + len) / (len + 2.0)).clamp(0.0, 1.0))]
}

/// Several drips side by side.
fn drips(rng: &mut Rng) -> Vec<Mesh> {
    let mut out = Vec::new();
    for i in 0..2 + rng.below(3) {
        let len = rng.range(8.0, 36.0);
        let x = -9.0 + i as f32 * 6.0 + rng.range(-1.5, 1.5);
        let path = smooth_open(&[Vec2::new(x, 1.0), Vec2::new(x + 0.3, -len * 0.5), Vec2::new(x, -len)], 4);
        let drop: Vec<Vec2> = ellipse_pts(1.4, 2.2, 8).into_iter().map(|q| q + Vec2::new(x, -len - 1.4)).collect();
        out.push(stroke(&path, |t| 1.6 * (1.0 - 0.5 * t), 1.0, 0.7, |_| 1.0));
        out.push(fill(&drop, |_| 1.0));
    }
    out
}

fn mound(rng: &mut Rng) -> Vec<Mesh> {
    mound_at(rng, -1.0)
}

/// Which color a hint carries.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Tone {
    /// The Venom Mother's: life.
    Green,
    /// The Gutter King's: sludge, dry things, sulfur and what people painted to warn each other.
    Yellow,
}

fn theme_yellow(style: Style) -> Theme {
    let (hue, sat, light, chance) = match style {
        Style::Concrete => (48.0, 0.65, 0.50, 0.10),
        Style::Trash => (50.0, 0.70, 0.45, 0.20),
        Style::Sewer => (52.0, 0.70, 0.42, 0.30),
        Style::Park => (46.0, 0.65, 0.52, 0.20),
        Style::Subway => (50.0, 0.50, 0.42, 0.14),
        Style::Treatment => (56.0, 0.70, 0.46, 0.22),
        Style::Sinkhole => (44.0, 0.55, 0.44, 0.14),
        Style::Catacombs => (42.0, 0.75, 0.55, 0.14),
        Style::Mall => (45.0, 0.70, 0.55, 0.12),
        Style::Dam => (48.0, 0.70, 0.50, 0.14),
        Style::Bunker => (50.0, 0.55, 0.42, 0.14),
        Style::Cave => (54.0, 0.65, 0.46, 0.18),
    };
    Theme { hue, sat, light, chance }
}

/// What the Gutter King's color brings back to each place.
fn table_yellow(style: Style, kind: Kind) -> &'static [(Build, f32)] {
    match (style, kind) {
        (Style::Concrete, Kind::Ground) => &[(dry_grass, 4.0), (flowers, 1.0)],
        (Style::Trash, Kind::Ground) => &[(crystals, 3.0), (bubble_pool, 3.0), (pool, 2.0), (dry_grass, 1.0)],
        (Style::Sewer, Kind::Ground) => &[(bubble_pool, 4.0), (pool, 3.0), (crystals, 1.0)],
        (Style::Park, Kind::Ground) => &[(dry_grass, 4.0), (flowers, 4.0)],
        (Style::Subway, Kind::Ground) => &[(edge_line, 4.0), (dry_grass, 1.0), (pool, 1.0)],
        (Style::Treatment, Kind::Ground) => &[(crystals, 3.0), (bubble_pool, 3.0), (pool, 2.0)],
        (Style::Sinkhole, Kind::Ground) => &[(dry_grass, 3.0), (flowers, 2.0), (mound, 1.0)],
        (Style::Catacombs, Kind::Ground) => &[(candles, 4.0), (mound, 1.0)],
        (Style::Mall, Kind::Ground) => &[(dry_grass, 2.0), (flowers, 1.0)],
        (Style::Dam, Kind::Ground) => &[(reeds, 2.0), (dry_grass, 2.0), (pool, 1.0)],
        (Style::Bunker, Kind::Ground) => &[(dry_grass, 2.0), (edge_line, 2.0)],
        (Style::Cave, Kind::Ground) => &[(crystals, 4.0), (mound, 1.0), (carpet, 1.0)],

        (Style::Concrete, Kind::Wall) => &[(caution_sign, 2.0), (streaks, 2.0), (rosettes, 1.0)],
        (Style::Trash, Kind::Wall) => &[(streaks, 3.0), (slime_fall, 2.0), (rosettes, 1.0)],
        (Style::Sewer, Kind::Wall) => &[(slime_fall, 4.0), (streaks, 3.0), (patch, 1.0)],
        (Style::Park, Kind::Wall) => &[(creeper, 2.0), (patch, 1.0)],
        (Style::Subway, Kind::Wall) => &[(caution_sign, 2.0), (stripes, 2.0), (hazard_stripes, 2.0)],
        (Style::Treatment, Kind::Wall) => &[(slime_fall, 3.0), (streaks, 3.0), (caution_sign, 2.0)],
        (Style::Sinkhole | Style::Cave, Kind::Wall) => &[(patch, 2.0), (rosettes, 2.0)],
        (Style::Catacombs, Kind::Wall) => &[(rosettes, 2.0), (patch, 2.0)],
        (Style::Mall, Kind::Wall) => &[(sign_frame, 3.0), (streaks, 1.0)],
        (Style::Dam, Kind::Wall) => &[(hazard_stripes, 3.0), (caution_sign, 2.0), (streaks, 2.0)],
        (Style::Bunker, Kind::Wall) => &[(hazard_stripes, 3.0), (caution_sign, 3.0), (rosettes, 1.0)],

        (Style::Sewer | Style::Treatment | Style::Trash | Style::Dam, Kind::Edge) => &[(slime_curtain, 3.0), (hang_edge, 1.0)],
        (_, Kind::Edge) => &[(hang_edge, 2.0), (mound_edge, 1.0)],
        (Style::Sewer | Style::Treatment | Style::Subway | Style::Dam | Style::Trash, Kind::Ceiling) => &[(drips, 3.0), (drip, 2.0)],
        (Style::Cave | Style::Catacombs | Style::Sinkhole, Kind::Ceiling) => &[(crystals_down, 3.0), (hang, 1.0)],
        (_, Kind::Ceiling) => &[(hang, 2.0), (drip, 1.0)],
    }
}

/// Tall dry grass.
fn dry_grass(rng: &mut Rng) -> Vec<Mesh> {
    let tris: Vec<[Vec2; 3]> = (0..9 + rng.below(6))
        .map(|_| {
            let (x, h, bend, w) = (rng.range(-9.0, 9.0), rng.range(8.0, 18.0), rng.range(-5.0, 5.0), rng.range(0.6, 1.1));
            [Vec2::new(x - w, -0.5), Vec2::new(x + w, -0.5), Vec2::new(x + bend, h)]
        })
        .collect();
    vec![triangles(&tris)]
}

/// Little flowers (dandelions) on thin stems.
fn flowers(rng: &mut Rng) -> Vec<Mesh> {
    let mut tris = Vec::new();
    let mut heads: Vec<Vec<Vec2>> = Vec::new();
    for _ in 0..3 + rng.below(4) {
        let (x, h, bend) = (rng.range(-8.0, 8.0), rng.range(7.0, 15.0), rng.range(-2.0, 2.0));
        tris.push([Vec2::new(x - 0.4, -0.5), Vec2::new(x + 0.4, -0.5), Vec2::new(x + bend, h)]);
        let (c, r) = (Vec2::new(x + bend, h + 1.0), rng.range(1.5, 2.4));
        heads.push(ellipse_pts(r, r, 8).into_iter().map(|q| q + c).collect());
    }
    vec![triangles(&tris), blobs(&heads, |_| 1.0)]
}

/// Sulfur crystals: clusters of spikes.
fn crystals(rng: &mut Rng) -> Vec<Mesh> {
    spikes(rng, 1.0)
}

fn crystals_down(rng: &mut Rng) -> Vec<Mesh> {
    spikes(rng, -1.0)
}

fn spikes(rng: &mut Rng, up: f32) -> Vec<Mesh> {
    let tris: Vec<[Vec2; 3]> = (0..4 + rng.below(4))
        .map(|_| {
            let (x, h, w, lean) = (rng.range(-7.0, 7.0), rng.range(5.0, 16.0), rng.range(1.4, 3.2), rng.range(-2.5, 2.5));
            [Vec2::new(x - w, 0.0), Vec2::new(x + w, 0.0), Vec2::new(x + lean, h * up)]
        })
        .collect();
    vec![triangles(&tris)]
}

/// A few candles, flames and all.
fn candles(rng: &mut Rng) -> Vec<Mesh> {
    let mut tris = Vec::new();
    let mut flames: Vec<Vec<Vec2>> = Vec::new();
    for i in 0..2 + rng.below(3) {
        let (x, h) = (-6.0 + i as f32 * 5.0 + rng.range(-1.0, 1.0), rng.range(4.0, 9.0));
        let (a, b, c, d) = (Vec2::new(x - 1.1, 0.0), Vec2::new(x + 1.1, 0.0), Vec2::new(x + 1.1, h), Vec2::new(x - 1.1, h));
        tris.push([a, b, c]);
        tris.push([a, c, d]);
        flames.push(ellipse_pts(0.9, 1.9, 8).into_iter().map(|q| q + Vec2::new(x, h + 2.4)).collect());
    }
    vec![triangles(&tris), blobs(&flames, |_| 1.0)]
}

/// A triangular warning sign on a wall (the exclamation mark is cut out of the plate: it's drawn as the frame and the mark).
fn caution_sign(rng: &mut Rng) -> Vec<Mesh> {
    let y = rng.range(-8.0, 8.0);
    let (a, b, c) = (Vec2::new(6.0, y + 10.0), Vec2::new(0.6, y - 1.0), Vec2::new(11.4, y - 1.0));
    let frame = smooth_open(&[a, b, c, a], 1);
    let bar = [Vec2::new(5.3, y + 1.8), Vec2::new(6.7, y + 1.8), Vec2::new(6.4, y + 6.5)];
    let bar2 = [Vec2::new(5.3, y + 1.8), Vec2::new(6.4, y + 6.5), Vec2::new(5.6, y + 6.5)];
    let dot: Vec<Vec2> = ellipse_pts(0.8, 0.8, 6).into_iter().map(|q| q + Vec2::new(6.0, y + 0.3)).collect();
    vec![stroke(&frame, |_| 1.6, 1.0, 1.0, |_| 1.0), triangles(&[bar, bar2]), fill(&dot, |_| 1.0)]
}

/// Yellow-and-black warning bands on a wall (the black is the wall).
fn hazard_stripes(rng: &mut Rng) -> Vec<Mesh> {
    let (y0, h, w) = (rng.range(-10.0, 4.0), rng.range(8.0, 14.0), rng.range(7.0, 12.0));
    let mut tris = Vec::new();
    let n = (w / 3.0) as i32;
    for i in 0..n {
        let x = 0.6 + i as f32 * 3.0;
        let (a, b, c, d) = (Vec2::new(x, y0), Vec2::new(x + 1.5, y0), Vec2::new(x + 1.5 + 3.0, y0 + h), Vec2::new(x + 3.0, y0 + h));
        tris.push([a, b, c]);
        tris.push([a, c, d]);
    }
    vec![triangles(&tris)]
}

/// The tactile line at the edge of a platform.
fn edge_line(rng: &mut Rng) -> Vec<Mesh> {
    let half = rng.range(14.0, 24.0);
    let (a, b, c, d) = (Vec2::new(-half, 0.0), Vec2::new(half, 0.0), Vec2::new(half, 1.3), Vec2::new(-half, 1.3));
    let bumps: Vec<Vec<Vec2>> = (0..(half / 2.5) as i32)
        .map(|i| ellipse_pts(0.7, 0.5, 6).into_iter().map(|q| q + Vec2::new(-half + 1.5 + i as f32 * 5.0, 2.0)).collect())
        .collect();
    vec![triangles(&[[a, b, c], [a, c, d]]), blobs(&bumps, |_| 1.0)]
}

/// A lit-up shop sign, just its frame and a few bulbs.
fn sign_frame(rng: &mut Rng) -> Vec<Mesh> {
    let (w, h, y) = (rng.range(6.0, 9.0), rng.range(4.0, 6.0), rng.range(-6.0, 6.0));
    let x0 = 1.0;
    let frame = [Vec2::new(x0, y - h), Vec2::new(x0 + 2.0 * w, y - h), Vec2::new(x0 + 2.0 * w, y + h), Vec2::new(x0, y + h), Vec2::new(x0, y - h)];
    let bulbs: Vec<Vec<Vec2>> = (0..4)
        .map(|i| ellipse_pts(0.9, 0.9, 6).into_iter().map(|q| q + Vec2::new(x0 + 2.0 + i as f32 * (2.0 * w - 4.0) / 3.0, y)).collect())
        .collect();
    vec![stroke(&frame, |_| 1.2, 1.0, 1.0, |_| 1.0), blobs(&bulbs, |_| 1.0)]
}

/// Spawns a hint's two copies: gray in the world, and the living one on the color layer.
fn dress(p: &mut Props, at: Vec2, flip: f32, tilt: f32, indoor: bool, meshes: Vec<Mesh>, color: Color, gray: M, boss: crate::boss::BossKind, dist: f32) {
    let meshes: Vec<Handle<Mesh>> = meshes.into_iter().map(|m| p.meshes.add(m)).collect();
    let jitter = (seed(at.x as i32, at.y as i32) >> 8) as f32 / (1u32 << 24) as f32 * 0.08;
    p.tilt = tilt;
    p.depth = if indoor { INDOOR } else { BACK } + jitter * 0.1;
    let dead = p.root(at, flip, 0.0);
    for m in &meshes {
        p.commands.spawn((Mesh2d(m.clone()), MeshMaterial2d(p.mats[gray as usize].clone()), Transform::default(), ChildOf(dead)));
    }
    let mat = p.colors.add(ColorMaterial { color: color.with_alpha(0.0), alpha_mode: AlphaMode2d::Blend, ..default() });
    p.depth = -0.5 + jitter;
    let alive = p.root(at, flip, 0.0);
    p.commands.entity(alive).insert((
        Visibility::Hidden,
        Vine { boss, dist, stem: mat.clone(), leaf: mat.clone(), shown: 0.0, base: [color, color], lit: 0.0 },
    ));
    for m in meshes {
        p.commands.spawn((Mesh2d(m), MeshMaterial2d(mat.clone()), Transform::default(), camera::color_layer(), ChildOf(alive)));
    }
    p.tilt = 0.0;
    p.depth = BACK;
}

/// A sulfur vent: a crusted cone in the ground, visible from the start. It
/// leaks nothing until the Gutter King's color is back (`fumes`).
pub(super) fn vent(p: &mut Props, s: &Site, at: Vec2, style: Style) {
    // (chance per eligible floor spot, least distance between vents): the dump
    // smokes everywhere, the sewer and the plant here and there, the caves almost never.
    let (chance, gap) = match style {
        Style::Trash => (0.04, 320.0),
        Style::Treatment => (0.004, 700.0),
        Style::Sewer => (0.0025, 700.0),
        Style::Cave => (0.0004, 900.0),
        _ => return,
    };
    if s.kind != Kind::Ground || p.vents.iter().any(|q| q.distance_squared(at) < gap * gap) {
        return;
    }
    p.rng = Rng(s.key ^ 0x7665_6e74);
    p.rng.next();
    let near = 1.0 + 2.0 * (1.0 - at.distance(p.t.roach.unwrap_or(at + Vec2::splat(1e5))) / 2600.0).clamp(0.0, 1.0);
    if !p.rng.chance(chance * near) {
        return;
    }
    let Some((y, angle, indoor)) = p.fit(s.cell, 12.0, 14.0, true) else { return };
    let at = Vec2::new(at.x, y);
    let flip = if p.rng.chance(0.5) { 1.0 } else { -1.0 };

    // The cone itself (a gray prop like any other)...
    let cone = smooth_closed(&[Vec2::new(-10.0, -1.0), Vec2::new(-6.0, 4.0), Vec2::new(-3.0, 9.0), Vec2::new(3.0, 9.0), Vec2::new(6.0, 4.0), Vec2::new(10.0, -1.0)], 3);
    p.tilt = angle;
    p.depth = if indoor { INDOOR } else { BACK };
    let e = p.root(at, flip, 0.0);
    p.shape(e, &cone, 0.45, M::Rust, 0.0);
    p.shape(e, &ellipse_pts(2.4, 1.0, 8).into_iter().map(|q| q + Vec2::new(0.0, 9.0)).collect::<Vec<_>>(), 0.3, M::Soot, 0.02);
    p.tilt = 0.0;
    p.depth = BACK;

    // ...with a crust of sulfur around the mouth, which is what turns yellow.
    let mut crust = spikes(&mut p.rng, 1.0);
    let rim: Vec<Vec<Vec2>> = (0..5).map(|i| ellipse_pts(2.6, 1.8, 8).into_iter().map(|q| q + Vec2::new(-4.5 + i as f32 * 2.3, 8.6)).collect()).collect();
    crust.push(blobs(&rim, fuzz));
    let hue = 50.0 + p.rng.range(-5.0, 5.0);
    let color = Color::hsl(hue, 0.72, p.rng.range(0.42, 0.55));
    let dist = at.distance(p.t.roach.unwrap_or(at));
    dress(p, at, flip, angle, indoor, crust, color, M::Concrete, crate::boss::BossKind::Roach, dist);

    p.commands.spawn((
        Transform::from_translation((at + Vec2::new(0.0, 9.5 * S)).extend(0.0)),
        crate::fumes::Fumer { dist, timer: p.rng.range(0.0, 0.6), seed: s.key },
        crate::terrain_art::TerrainPart,
    ));
    p.vents.push(at);
}

/// A wide, low carpet of moss.
fn carpet(rng: &mut Rng) -> Vec<Mesh> {
    let centers: Vec<Vec2> = (0..10 + rng.below(4)).map(|i| Vec2::new(-17.0 + i as f32 * 3.0 + rng.range(-1.0, 1.0), rng.range(1.0, 2.6))).collect();
    vec![blobs(&ellipses(rng, &centers, (2.0, 3.6)), fuzz)]
}

/// A stalagmite wearing moss on its slopes.
fn mossy_stalagmite(rng: &mut Rng) -> Vec<Mesh> {
    let (w, h) = (rng.range(4.0, 7.0), rng.range(14.0, 26.0));
    let spike = [Vec2::new(-w, 0.0), Vec2::new(w, 0.0), Vec2::new(rng.range(-1.5, 1.5), h)];
    let coat: Vec<Vec2> = (0..6).map(|i| {
        let t = i as f32 / 5.0;
        Vec2::new((if i % 2 == 0 { -1.0 } else { 1.0 }) * w * (1.0 - t * 0.8) * 0.85, 1.5 + t * h * 0.55)
    }).collect();
    vec![triangles(&[spike]), blobs(&ellipses(rng, &coat, (1.8, 3.0)), fuzz)]
}
