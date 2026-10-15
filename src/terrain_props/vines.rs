//! Vines of the Venom Mother's region: climbing walls, hanging from the lips
//! of ledges and from ceilings. They are the vegetation the war killed, so
//! each one is drawn twice: bare and gray in the world, and lush green on the
//! color layer, hidden until the wasp falls and `regrowth` washes the green
//! back over them.

use std::f32::consts::{PI, TAU};

use crate::{camera, regrowth::Vine};

use super::*;

/// Share of the edges, walls and ceilings that get a vine where the growth is thickest.
const SHARE: f32 = 0.85;

/// Smooth random value in 0..1 at a lattice point.
fn lattice(x: i32, y: i32, salt: i32) -> f32 {
    (seed(x.wrapping_mul(31).wrapping_add(salt), y.wrapping_add(salt * 7)) >> 8) as f32 / (1u32 << 24) as f32
}

/// Continuous value noise (two octaves), 0..1, features ~`scale` world units wide.
pub(super) fn noise(at: Vec2, scale: f32, salt: i32) -> f32 {
    let octave = |k: f32, salt: i32| {
        let q = at / (scale * k);
        let (i, f) = (q.floor().as_ivec2(), q - q.floor());
        let f = f * f * (Vec2::splat(3.0) - 2.0 * f);
        let at = |dx, dy| lattice(i.x + dx, i.y + dy, salt);
        let (top, bottom) = (at(0, 0).lerp(at(1, 0), f.x), at(0, 1).lerp(at(1, 1), f.x));
        top.lerp(bottom, f.y)
    };
    (octave(1.0, salt) * 0.7 + octave(0.35, salt + 101) * 0.3).clamp(0.0, 1.0)
}

/// The region is cut into blocks of `BLOCK` world units (a screen is several
/// of them) and each one should hold at least `MIN_PER_BLOCK` vines: where
/// nature leaves fewer, the chance rises, and it rises more next to a vine
/// already there, so the minimum grows as patches rather than lone strands.
const BLOCK: f32 = 400.0;
const MIN_PER_BLOCK: f32 = 10.0;
/// The chance a spot gets in an empty block, and next to other vines of a thin one.
const STARVED: f32 = 0.5;
const NEXT_TO: f32 = 180.0;

/// The chance forced on `at` by the vines missing in its block (0 when it has enough).
pub(super) fn starved(p: &Props, at: Vec2) -> f32 {
    let block = (at / BLOCK).floor();
    let mut here = 0.0;
    let mut close = false;
    for v in p.vined.iter().filter(|v| (**v / BLOCK).floor() == block) {
        here += 1.0;
        close |= v.distance_squared(at) < NEXT_TO * NEXT_TO;
    }
    let missing = (1.0 - here / MIN_PER_BLOCK).max(0.0);
    (STARVED * missing * if close { 1.8 } else { 1.0 }).min(0.9)
}

/// How much open sky is over `at` and around it (0..1): rays going up, the
/// ones right above weighing most. A thick roof or ceiling over the spot blocks
/// its own ray, but the rays of the open ground beside it still count, so the
/// nooks next to the sky stay lush and the deep interiors don't.
fn sky(t: &Terrain, at: Vec2) -> f32 {
    let clear = |dx: f32| {
        let top = at + Vec2::new(dx, 4.0);
        // Out of the ground or ceiling the spot is attached to (up to a few cells)...
        let Some(start) = (0..=12).find(|&k| !t.solid_at(top + Vec2::Y * k as f32 * PX)) else { return false };
        // ...then nothing else over it.
        (start..start + 40).all(|k| !t.solid_at(top + Vec2::Y * k as f32 * PX))
    };
    let rays = [(-160.0, 1.0), (-80.0, 2.0), (-30.0, 3.0), (0.0, 4.0), (30.0, 3.0), (80.0, 2.0), (160.0, 1.0)];
    rays.iter().filter(|r| clear(r.0)).map(|r| r.1).sum::<f32>() / rays.iter().map(|r| r.1).sum::<f32>()
}

/// Chance of a vine at `at` (a spot of `kind`, `flip` = its open side). It's
/// sparse in general and thickest where the sky is open and next to it (light);
/// noise patches set where the air is damp, faces turned away from the sun (it
/// hangs low on the left, see `atmosphere`) keep their moisture, and damp pools
/// low in the region.
pub(super) fn share(p: &Props, at: Vec2, kind: Kind, flip: f32) -> f32 {
    let t = p.t;
    let open = sky(t, at);
    // Faces looking at the sun (toward -x) dry; ones looking away keep the damp.
    let turned_away = if kind == Kind::Wall { 0.5 + 0.5 * flip.signum() } else { 0.5 };
    let low = t.wasp.and_then(|w| t.area_at(w)).map_or(0.5, |a| 1.0 - ((at.y - a.rect.min.y) / a.rect.height().max(1.0)).clamp(0.0, 1.0));
    let damp = noise(at, 420.0, 3) + 0.5 * (open - 0.4) + 0.12 * (turned_away - 0.5) + 0.14 * (low - 0.5);
    let k = ((damp - 0.46) / 0.16).clamp(0.0, 1.0);
    SHARE * k * k * (3.0 - 2.0 * k)
}

const STEM: Color = Color::srgb(0.17, 0.30, 0.10);
const LEAF: Color = Color::srgb(0.40, 0.72, 0.20);
/// The vines covering the walls of interiors, in the shade.
const STEM_DARK: Color = Color::srgb(0.08, 0.16, 0.05);
const LEAF_DARK: Color = Color::srgb(0.17, 0.36, 0.10);

/// A vine's shapes in prop units, before they're drawn in both versions.
struct Growth {
    /// (path, width at the root)
    stems: Vec<(Vec<Vec2>, f32)>,
    leaves: Vec<Vec<Vec2>>,
}

/// Plants a vine at a spot of `kind` (`flip` = the open side). Several
/// strands grow together so they hang as a curtain. `back` = covering the
/// wall of an interior: darker, hanging at most that long.
pub(super) fn plant(p: &mut Props, at: Vec2, flip: f32, kind: Kind, back: Option<f32>, dark: bool) {
    let limit = back.unwrap_or(f32::MAX);
    let mut g = Growth { stems: Vec::new(), leaves: Vec::new() };
    let strands = if dark { 3 + p.rng.below(3) } else { 2 + p.rng.below(3) };
    // Thinner at the fringe of a patch.
    let strands = if share(p, at, kind, flip) < 0.5 * SHARE { strands.div_ceil(2) } else { strands };
    for i in 0..strands {
        let spread = if kind == Kind::Ceiling { p.rng.range(-9.0, 9.0) } else { 0.0 };
        let off = Vec2::new(spread, if i == 0 { 0.0 } else { p.rng.range(-4.0, 4.0) });
        let one = grow(p, kind, limit);
        g.stems.extend(one.stems.into_iter().map(|(path, w)| (path.into_iter().map(|q| q + off).collect(), w)));
        g.leaves.extend(one.leaves.into_iter().map(|l| l.into_iter().map(|q| q + off).collect()));
    }
    let stems: Vec<Mesh> = g.stems.iter().map(|(path, w)| stroke(path, |t| w * (1.0 - 0.75 * t) + 0.35, 1.0, 0.6, |_| 1.0)).collect();
    let leaves = blobs(&g.leaves, |q| 0.62 + 0.38 * ((q.x * 0.9 + q.y * 1.7).sin() * 0.5 + 0.5));
    let stems: Vec<Handle<Mesh>> = stems.into_iter().map(|m| p.meshes.add(m)).collect();
    let leaves = p.meshes.add(leaves);

    // Dead and gray in the world...
    if back.is_some() {
        p.depth = INDOOR;
    }
    // Each vine has its own depth (no z-fighting where they overlap).
    let jitter = (seed(at.x as i32, at.y as i32) >> 8) as f32 / (1u32 << 24) as f32 * 0.08;
    p.depth += jitter * 0.1;
    let dead = p.root(at, flip, 0.0);
    for s in &stems {
        p.commands.spawn((Mesh2d(s.clone()), MeshMaterial2d(p.mats[M::Soot as usize].clone()), Transform::default(), ChildOf(dead)));
    }
    let gray_leaf = if dark { M::Soot } else { M::Concrete };
    p.commands.spawn((Mesh2d(leaves.clone()), MeshMaterial2d(p.mats[gray_leaf as usize].clone()), Transform::from_xyz(0.0, 0.0, 0.01), ChildOf(dead)));

    // ...alive on the color layer, fading in when its turn comes.
    let (stem, leaf) = if dark { (STEM_DARK, LEAF_DARK) } else { (STEM, LEAF) };
    let blend = |color| ColorMaterial { color, alpha_mode: bevy::sprite_render::AlphaMode2d::Blend, ..default() };
    let (stem_mat, leaf_mat) = (p.colors.add(blend(stem.with_alpha(0.0))), p.colors.add(blend(leaf.with_alpha(0.0))));
    let dist = at.distance(p.t.wasp.unwrap_or(at));
    // The color layer draws over everything gray: the vines of the foreground always over those of the walls.
    p.depth = if dark { -0.6 } else { -0.4 } + jitter;
    let alive = p.root(at, flip, 0.0);
    p.commands.entity(alive).insert((
        Visibility::Hidden,
        Vine { boss: crate::boss::BossKind::Wasp, dist, stem: stem_mat.clone(), leaf: leaf_mat.clone(), shown: 0.0, base: [stem, leaf], lit: 0.0 },
    ));
    for s in stems {
        p.commands.spawn((Mesh2d(s), MeshMaterial2d(stem_mat.clone()), Transform::default(), camera::color_layer(), ChildOf(alive)));
    }
    p.commands.spawn((Mesh2d(leaves), MeshMaterial2d(leaf_mat), Transform::from_xyz(0.0, 0.0, 0.01), camera::color_layer(), ChildOf(alive)));
    p.depth = BACK;
}

/// Ivy over the wall of an interior, hanging from the cell `c` (a spot from
/// `facade_spots`) down through the dark cells below it.
pub(super) fn backdrop(p: &mut Props, c: IVec2) {
    let t = p.t;
    let room = (1..=cells(160.0)).take_while(|&k| t.cell(c + IVec2::Y * k) == Cell::Back).count();
    let limit = room as f32 * PX / S;
    if limit < 24.0 {
        return;
    }
    let at = t.cell_min(c) + Vec2::new(PX / 2.0, 0.0);
    if !p.rng.chance(share(p, at, Kind::Ceiling, 1.0) / SHARE) {
        return;
    }
    plant(p, at, 1.0, Kind::Ceiling, Some(limit), true);
}

fn grow(p: &mut Props, kind: Kind, limit: f32) -> Growth {
    let rng = &mut p.rng;
    let mut g = Growth { stems: Vec::new(), leaves: Vec::new() };
    // The main stem, then what sprouts from it.
    let (main, width) = match kind {
        // Hangs straight down from the ceiling.
        Kind::Ceiling => {
            let len = rng.range(55.0, 150.0).min(limit);
            (meander(rng, Vec2::new(0.0, 1.0), -len, 0.0, 3.0), 2.2)
        }
        // Drapes over the lip and hangs down the face.
        Kind::Edge => {
            let len = rng.range(45.0, 120.0);
            let mut down = meander(rng, Vec2::new(2.4, -1.0), -len, 2.4, 1.6);
            let mut path = vec![Vec2::new(-6.0, 1.6), Vec2::new(-1.5, 1.3)];
            path.append(&mut down);
            (smooth_open(&path, 3), 2.2)
        }
        // Climbs the face: from lower on the wall up.
        _ => {
            let (base, top) = (rng.range(-55.0, -20.0), rng.range(20.0, 60.0));
            (meander(rng, Vec2::new(2.2, base), top, 2.2, 1.4), 2.4)
        }
    };
    let hangs = kind != Kind::Wall;
    sprout(rng, kind, &main, hangs, &mut g.leaves);
    for _ in 0..3 + rng.below(3) {
        let at = main[main.len() / 5 + rng.below(main.len() * 2 / 3)];
        let side = if kind == Kind::Ceiling && rng.chance(0.5) { -1.0 } else { 1.0 };
        let size = rng.range(0.7, 1.3);
        let pts = if hangs {
            [at, at + Vec2::new(side * 3.0, -2.0) * size, at + Vec2::new(side * 5.5, -7.0) * size, at + Vec2::new(side * 5.0, -13.0) * size]
        } else {
            [at, at + Vec2::new(3.0, 1.5) * size, at + Vec2::new(6.0, 0.5) * size, at + Vec2::new(8.0, -3.0) * size]
        };
        let tendril = smooth_open(&pts, 4);
        sprout(rng, kind, &tendril, hangs, &mut g.leaves);
        g.stems.push((tendril, 1.1));
    }
    g.stems.push((main, width));
    g
}

/// A mostly vertical wandering path from `from` to height `to_y`, hugging x = `x0`.
fn meander(rng: &mut Rng, from: Vec2, to_y: f32, x0: f32, amp: f32) -> Vec<Vec2> {
    let n = ((to_y - from.y).abs() / 8.0).ceil().max(3.0) as usize;
    let (phase, freq) = (rng.range(0.0, TAU), rng.range(0.10, 0.2));
    let pts: Vec<Vec2> = (0..=n)
        .map(|i| {
            let t = i as f32 / n as f32;
            let y = from.y + (to_y - from.y) * t;
            Vec2::new(from.x.lerp(x0, t) + amp * (phase + freq * y).sin() * t.sqrt(), y)
        })
        .collect();
    smooth_open(&pts, 4)
}

/// Leaves in alternating pairs along `path`, drooping when it hangs. Nothing
/// points into the wall (that would paint over its face on the color layer).
fn sprout(rng: &mut Rng, kind: Kind, path: &[Vec2], hangs: bool, out: &mut Vec<Vec<Vec2>>) {
    let step = 3;
    let mut side = if rng.chance(0.5) { 1.0 } else { -1.0 };
    for i in (step..path.len()).step_by(step) {
        let tangent = (path[i] - path[i - 1]).normalize_or(Vec2::NEG_Y);
        let along = tangent.y.atan2(tangent.x);
        let mut angle = along + side * rng.range(0.7, 1.2) - if hangs { 0.0 } else { side * 0.2 };
        if kind != Kind::Ceiling && Vec2::from_angle(angle).x < 0.2 {
            angle = PI - angle;
        }
        let size = rng.range(6.0, 9.0) * (1.0 - 0.35 * i as f32 / path.len() as f32);
        out.push(leaf(path[i], angle, size));
        side = -side;
    }
    // A leaf at the tip.
    if let Some(&tip) = path.last() {
        let along = (tip - path[path.len() - 2]).to_array();
        let mut angle = along[1].atan2(along[0]);
        if kind != Kind::Ceiling && Vec2::from_angle(angle).x < 0.2 {
            angle = PI - angle;
        }
        out.push(leaf(tip, angle, rng.range(5.5, 7.5)));
    }
}

/// A pointed leaf growing from `base` toward `angle`.
fn leaf(base: Vec2, angle: f32, size: f32) -> Vec<Vec2> {
    let (rot, c) = (Vec2::from_angle(angle), base + Vec2::from_angle(angle) * size * 0.5);
    ellipse_pts(size * 0.5, size * 0.25, 8).into_iter().map(|q| c + rot.rotate(q)).collect()
}

/// Vines draped over the face of a big block from its top, the side of a
/// ruined building seen from the open (a spot from `facade_spots`: a top cell).
pub(super) fn facade_vine(p: &mut Props, c: IVec2) {
    let t = p.t;
    let w = (t.cell_min(c) / PX).round().as_ivec2();
    p.rng = Rng(seed(w.x, w.y * 7 + 11));
    p.rng.next();
    let at = t.cell_min(c) + Vec2::new(PX / 2.0, PX);
    if p.vined.iter().any(|v| v.distance_squared(at) < 140.0 * 140.0) {
        return;
    }
    let room = (1..=cells(170.0)).take_while(|&k| t.solid(c + IVec2::Y * k)).count();
    let limit = (room as f32 * PX / S).min(130.0);
    if limit < 30.0 {
        return;
    }
    let k = share(p, at, Kind::Ceiling, 1.0).max(starved(p, at));
    if !p.rng.chance(k) {
        return;
    }
    plant(p, at, 1.0, Kind::Ceiling, Some(limit), false);
    p.vined.push(at);
}
