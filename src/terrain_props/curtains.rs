//! Web curtains: an orb web spun across a corridor from floor to ceiling,
//! sticky (`webs.rs`) until the chameleon tears it down by slashing, pushing
//! through or thrashing in it. Placed in the tunnels that are flat and low
//! enough to be closed off by one, thickest in the heart of the catacombs.

use super::{catacombs, *};
use crate::webs::Curtain;

/// Corridor height (floor to ceiling) a curtain can close, in world units.
const HEIGHT: (f32, f32) = (60.0, 260.0);
/// Least distance between two curtains.
const GAP: f32 = 260.0;
/// Drawn over the chameleon, so it looks caught in the silk.
const Z: f32 = 12.0;
/// Strands are torn off in this many groups as the curtain wears out.
pub const GROUPS: usize = 4;

/// Chance that a flat stretch of corridor gets a curtain (per candidate cell).
fn chance(style: Style) -> f32 {
    match style {
        Style::Catacombs => 0.2,
        Style::Cave | Style::Subway | Style::Bunker | Style::Sewer | Style::Treatment => 0.05,
        _ => 0.0,
    }
}

/// Maybe closes the corridor at the ground spot `at` (cell `s.cell`).
pub(super) fn corridor(p: &mut Props, s: &Site, at: Vec2, style: Style) {
    let base = chance(style);
    if base == 0.0 || p.curtains.iter().any(|c| c.distance(at) < GAP) {
        return;
    }
    p.rng = Rng(s.key ^ 0x7e3b_91a5 | 1);
    p.rng.next();
    let k = if style == Style::Catacombs { catacombs::heart(p, at) } else { 0.0 };
    if !p.rng.chance((base * (1.0 + 5.0 * k * k)).min(0.9)) {
        return;
    }
    let t = p.t;
    let up = IVec2::Y;
    // Cells of air over a floor cell, up to the ceiling.
    let max = cells(HEIGHT.1) + 1;
    let roof = |c: IVec2| (1..=max).find(|&k| t.solid(c - up * k));
    let Some(k0) = roof(s.cell) else { return };
    if ((k0 - 1) as f32) * PX < HEIGHT.0 {
        return;
    }
    // A tunnel, not a gap under an overhang: floor and ceiling run level on both sides.
    for dx in [-3, -2, -1, 1, 2, 3] {
        let col = s.cell + IVec2::X * dx;
        if !t.solid(col) || t.solid(col - up) || roof(col).is_none_or(|k| (k - k0).abs() > 1) {
            return;
        }
    }
    let ceiling = t.cell_min(s.cell - up * k0).y;
    let h = ceiling - at.y;
    if h < HEIGHT.0 {
        return;
    }
    p.curtains.push(at);
    // Everything is drawn hanging from the ceiling point (the curtain swings from there).
    let rx = (h * 0.28).clamp(26.0, 54.0);
    let ry = h * 0.5 - 1.0;
    let hub = Vec2::new(p.rng.range(-3.0, 3.0), -h * 0.5 + p.rng.range(-4.0, 4.0));
    let n = 10 + p.rng.below(4);
    let turn = p.rng.range(0.0, std::f32::consts::TAU);
    let dirs: Vec<Vec2> = (0..n)
        .map(|i| {
            let a = turn + (i as f32 + p.rng.range(-0.15, 0.15)) / n as f32 * std::f32::consts::TAU;
            Vec2::new(a.cos() * rx, a.sin() * ry)
        })
        .collect();
    let mut groups: Vec<Vec<[Vec2; 3]>> = vec![Vec::new(); GROUPS];
    // Spokes (the skeleton: they go last) and the frame threads out to the corners.
    for &d in &dirs {
        let g = GROUPS - 1 - p.rng.below(2);
        catacombs::thread(&mut groups[g], hub, hub + d, 2.0);
    }
    for (sx, y) in [(-1.0, 0.0), (1.0, 0.0), (-1.0, -h), (1.0, -h)] {
        let corner = Vec2::new(sx * rx * 1.7, y);
        let near = dirs.iter().map(|&d| hub + d).min_by(|a, b| a.distance(corner).total_cmp(&b.distance(corner))).unwrap_or(hub);
        catacombs::sagging(&mut groups[GROUPS - 1], corner, near, 3.0, 1.6);
    }
    // Spiral rings between the spokes, torn here and there.
    let rings = 5 + p.rng.below(3);
    for j in 0..rings {
        let f = 0.2 + 0.8 * (j + 1) as f32 / rings as f32;
        for i in 0..n {
            if p.rng.chance(0.1 + 0.1 * f) {
                continue;
            }
            let (a, b) = (hub + dirs[i] * f, hub + dirs[(i + 1) % n] * f);
            let g = p.rng.below(GROUPS - 1);
            catacombs::sagging(&mut groups[g], a, b, a.distance(b) * 0.06, 1.5);
        }
    }
    let root = p.commands.spawn((Transform::from_xyz(at.x, ceiling, Z), Visibility::default(), TerrainPart)).id();
    let base = p.colors.get(&p.mats[M::Silk as usize]).cloned().unwrap_or_default();
    let silk = p.colors.add(base);
    let mut parents = Vec::new();
    for tris in &groups {
        let g = p.commands.spawn((Transform::default(), Visibility::default(), ChildOf(root))).id();
        let mesh = p.meshes.add(triangles(tris));
        p.commands.spawn((Mesh2d(mesh), MeshMaterial2d(silk.clone()), Transform::default(), ChildOf(g)));
        parents.push(g);
    }
    if k > 0.3 && p.rng.chance(0.5) {
        // The weaver, waiting at the hub (the last to go with the skeleton).
        let holder = p.commands.spawn((Transform::from_translation((hub + Vec2::Y * ry * 0.3).extend(0.2)).with_scale(Vec3::splat(S * 1.6)), Visibility::default(), ChildOf(parents[GROUPS - 1]))).id();
        catacombs::spider(p, holder, Vec2::ZERO, 0.0);
    }
    let center = Vec2::new(at.x, ceiling - h * 0.5);
    p.commands.entity(root).insert(Curtain::new(center, Vec2::new(rx * 0.7 + 8.0, h * 0.5), parents, silk));
}
