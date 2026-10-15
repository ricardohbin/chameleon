//! Props of "region: cold war military bunker" (`Style::Bunker`): blast doors
//! with wheel locks, consoles with dead CRTs, bunk frames, lockers, sandbags,
//! ammo crates, oil drums, a toppled missile section, gas masks, caged
//! bulkhead lamps hanging on long conduits, ducts, hazard-striped steel lips.

use super::*;

pub(super) const HOOKS: Hooks = Hooks { spacing, ground, edge, wall, ceiling };

/// (gap between props of the same kind, chance a free spot gets one), in prop units.
fn spacing(kind: Kind) -> (f32, f32) {
    match kind {
        Kind::Ground => (52.0, 0.8),
        Kind::Edge => (35.0, 0.85),
        Kind::Wall => (50.0, 0.75),
        Kind::Ceiling => (45.0, 0.6),
    }
}

#[derive(Clone, Copy)]
enum G {
    Door,
    Console,
    Bunk,
    Lockers,
    Sandbags,
    Crates,
    Drums,
    Missile,
    Mask,
    Lamp,
}

impl G {
    /// (half width, height) in prop units.
    fn size(self) -> (f32, f32) {
        match self {
            G::Door => (40.0, 88.0),
            G::Console => (28.0, 56.0),
            G::Bunk => (28.0, 54.0),
            G::Lockers => (22.0, 58.0),
            G::Sandbags => (24.0, 22.0),
            G::Crates => (22.0, 34.0),
            G::Drums => (20.0, 28.0),
            G::Missile => (42.0, 34.0),
            G::Mask => (9.0, 10.0),
            G::Lamp => (4.0, 20.0),
        }
    }
}

/// A top surface with headroom at `cell` (`x` = its world x). True if something was placed.
fn ground(p: &mut Props, cell: IVec2, x: f32) -> bool {
    use G::*;
    let table: &[(G, f32)] = &[
        (Door, 0.8),
        (Console, 1.6),
        (Bunk, 1.2),
        (Lockers, 1.0),
        (Sandbags, 1.6),
        (Crates, 1.8),
        (Drums, 1.8),
        (Missile, 0.7),
        (Mask, 0.7),
        (Lamp, 1.6),
    ];
    let total: f32 = table.iter().map(|e| e.1).sum();
    // Neighbouring spots' RNGs start alike: pick from a hash of the spot instead.
    let mut r = terrain_art::hash(cell.x, cell.y, 0xb0b) * total;
    let pick = table.iter().find(|e| {
        r -= e.1;
        r <= 0.0
    });
    let pick = pick.unwrap_or(&table[0]).0;
    for g in [pick, Mask] {
        let (half, head) = g.size();
        let tilt = matches!(g, Sandbags | Mask | Missile);
        let Some((y, angle, indoor)) = p.fit(cell, half, head, tilt) else { continue };
        let at = Vec2::new(x, y);
        // Wide props keep clear of the ones already standing near them.
        if p.taken.iter().any(|&(q, k)| k == Kind::Ground && q.distance(at) < (half + 26.0) * S) {
            continue;
        }
        p.tilt = angle;
        p.depth = if indoor { INDOOR } else { BACK };
        let placed = match g {
            Door => blast_door(p, at),
            Console => console(p, at),
            Bunk => bunk(p, at),
            Lockers => lockers(p, at),
            Sandbags => sandbags(p, at),
            Crates => crates(p, at),
            Drums => drums(p, at),
            Missile => missile(p, at),
            Mask => {
                let flip = if p.rng.chance(0.5) { 1.0 } else { -1.0 };
                let e = p.root(at, flip, 0.0);
                let angle = p.rng.range(-0.3, 0.3);
                gas_mask(p, e, Vec2::ZERO, angle, 0.0);
                true
            }
            Lamp => hanging_lamp(p, cell, at),
        };
        p.tilt = 0.0;
        p.depth = BACK;
        if placed {
            return true;
        }
    }
    false
}

/// Where the ground ends at a drop (`flip` = the open side).
fn edge(p: &mut Props, at: Vec2, flip: f32) -> bool {
    let e = p.root(at, flip, 0.0);
    // A steel lip bolted over the broken edge, hazard striped.
    hazard_v(p, e, -10.0, 1.0, -7.0, 0.0, 0.0);
    p.shape(e, &rect(-10.0, -7.0, 1.5, -5.5), 0.5, M::Iron, 0.01);
    if p.rng.chance(0.6) {
        // A conduit snapped off the edge, cables spilling out.
        let y = -p.rng.range(8.0, 14.0);
        let len = p.rng.range(8.0, 16.0);
        let droop = p.rng.range(-5.0, 1.0);
        p.bar(e, &[Vec2::new(-8.0, y), Vec2::new(len * 0.6, y + droop * 0.3), Vec2::new(len, y + droop)], 3.4, M::Slime, 0.02);
        let n = 1 + p.rng.below(3);
        p.cables(e, Vec2::new(len - 3.0, y + droop), n, 0.0);
    } else {
        // Rebar mesh hanging out of the blasted concrete.
        for _ in 0..3 {
            let base = Vec2::new(-2.0, -p.rng.range(6.0, 16.0));
            let dir = Vec2::new(1.0, p.rng.range(-0.6, 0.3)).normalize();
            p.rebar(e, base, dir, 0.0);
        }
    }
    true
}

/// A straight vertical face (`flip` = the open side).
fn wall(p: &mut Props, at: Vec2, flip: f32) -> bool {
    if p.rng.chance(0.55) {
        // A caged bulkhead lamp on a bracket.
        let e = p.root(at, flip, 0.0);
        p.bar(e, &[Vec2::new(-3.0, 0.0), Vec2::new(7.0, 0.0)], 3.0, M::Iron, 0.0);
        p.shape(e, &rect(-2.0, -6.0, 1.5, 6.0), 0.5, M::Iron, 0.01);
        cage_lamp(p, e, Vec2::new(7.0, -1.0), 0.02);
    } else {
        // Conduit and a junction box on the face.
        let half = Vec2::new(7.0, 9.0);
        let c = at + Vec2::new(-flip * (half.x * S + PX * 2.0), 0.0);
        if !p.inside(c, half * S) {
            let e = p.root(at, flip, 0.0);
            cage_lamp(p, e, Vec2::new(4.0, -2.0), 0.0);
            return true;
        }
        let e = p.root(c, flip, 0.0);
        p.bar(e, &[Vec2::new(-2.0, half.y), Vec2::new(-2.0, half.y + 30.0)], 2.4, M::Slime, OVER);
        p.bar(e, &[Vec2::new(2.0, -half.y), Vec2::new(2.0, -half.y - 26.0)], 2.4, M::Slime, OVER);
        p.shape(e, &rect(-half.x, -half.y, half.x, half.y), 0.55, M::Slime, OVER + 0.01);
        p.shape(e, &rect(-half.x + 1.5, -half.y + 1.5, half.x - 1.5, half.y - 1.5), 0.8, M::Iron, OVER + 0.02);
        rivets(p, e, &[Vec2::new(-half.x + 1.0, half.y - 1.0), Vec2::new(half.x - 1.0, half.y - 1.0), Vec2::new(-half.x + 1.0, -half.y + 1.0), Vec2::new(half.x - 1.0, -half.y + 1.0)], 0.8, OVER + 0.03);
        trefoil(p, e, Vec2::ZERO, 4.0, OVER + 0.03);
    }
    true
}

/// The underside of a thick overhang.
fn ceiling(p: &mut Props, at: Vec2) -> bool {
    let e = p.root(at, 1.0, 0.0);
    if p.rng.chance(0.6) {
        let len = p.rng.range(10.0, 30.0);
        p.bar(e, &[Vec2::new(0.0, 3.0), Vec2::new(0.0, -len)], 1.6, M::Iron, 0.0);
        cage_lamp_hanging(p, e, Vec2::new(0.0, -len), 0.01);
    } else {
        let n = 2 + p.rng.below(2);
        p.cables(e, Vec2::new(0.0, 2.0), n, -1.0);
    }
    true
}

// --- Helpers ---

fn rect(x0: f32, y0: f32, x1: f32, y1: f32) -> Vec<Vec2> {
    vec![Vec2::new(x0, y0), Vec2::new(x0, y1), Vec2::new(x1, y1), Vec2::new(x1, y0)]
}

fn flip_coin(p: &mut Props) -> f32 {
    if p.rng.chance(0.5) { 1.0 } else { -1.0 }
}

/// Small round rivet heads, one mesh.
fn rivets(p: &mut Props, e: Entity, at: &[Vec2], r: f32, z: f32) {
    let dots: Vec<Vec<Vec2>> = at.iter().map(|&c| ellipse_pts(r, r, 6).into_iter().map(|q| q + c).collect()).collect();
    p.part(e, blobs(&dots, |_| 0.8), M::Rim, z);
}

/// A row of rivets from `a` to `b`, `step` apart.
fn rivet_line(p: &mut Props, e: Entity, a: Vec2, b: Vec2, step: f32, z: f32) {
    let n = ((b - a).length() / step).floor().max(1.0) as usize;
    let pts: Vec<Vec2> = (0..=n).map(|i| a.lerp(b, i as f32 / n as f32)).collect();
    rivets(p, e, &pts, 0.7, z);
}

/// A horizontal band of diagonal hazard stripes.
fn hazard_h(p: &mut Props, e: Entity, x0: f32, x1: f32, y0: f32, y1: f32, z: f32) {
    p.shape(e, &rect(x0, y0, x1, y1), 0.8, M::Enamel, z);
    let (h, w) = (y1 - y0, 3.0);
    let mut tris = Vec::new();
    let mut x = x0 - h;
    while x < x1 {
        let c = |v: f32| v.clamp(x0, x1);
        let (a, b, cc, d) = (Vec2::new(c(x), y0), Vec2::new(c(x + w), y0), Vec2::new(c(x + w + h), y1), Vec2::new(c(x + h), y1));
        tris.push([a, b, cc]);
        tris.push([a, cc, d]);
        x += 2.0 * w;
    }
    p.part(e, triangles(&tris), M::Soot, z + 0.005);
}

/// A vertical band of diagonal hazard stripes.
fn hazard_v(p: &mut Props, e: Entity, x0: f32, x1: f32, y0: f32, y1: f32, z: f32) {
    p.shape(e, &rect(x0, y0, x1, y1), 0.8, M::Enamel, z);
    let (wd, w) = (x1 - x0, 3.0);
    let mut tris = Vec::new();
    let mut y = y0 - wd;
    while y < y1 {
        let c = |v: f32| v.clamp(y0, y1);
        let (a, b, cc, d) = (Vec2::new(x0, c(y)), Vec2::new(x0, c(y + w)), Vec2::new(x1, c(y + w + wd)), Vec2::new(x1, c(y + wd)));
        tris.push([a, b, cc]);
        tris.push([a, cc, d]);
        y += 2.0 * w;
    }
    p.part(e, triangles(&tris), M::Soot, z + 0.005);
}

/// The radiation trefoil (dark on whatever is under it).
fn trefoil(p: &mut Props, e: Entity, c: Vec2, r: f32, z: f32) {
    let mut outlines = vec![ellipse_pts(r * 0.2, r * 0.2, 8).into_iter().map(|q| q + c).collect::<Vec<_>>()];
    for k in 0..3 {
        let a0 = std::f32::consts::FRAC_PI_2 + k as f32 * std::f32::consts::TAU / 3.0 - 0.52;
        let mut blade = Vec::new();
        for i in 0..=6 {
            blade.push(c + Vec2::from_angle(a0 + i as f32 / 6.0 * 1.04) * r);
        }
        for i in (0..=6).rev() {
            blade.push(c + Vec2::from_angle(a0 + i as f32 / 6.0 * 1.04) * r * 0.35);
        }
        outlines.push(blade);
    }
    p.part(e, blobs(&outlines, |_| 1.0), M::Soot, z);
}

// --- Ground props ---

/// A blast door in a concrete surround, hazard-striped jambs, a wheel lock.
fn blast_door(p: &mut Props, at: Vec2) -> bool {
    let flip = flip_coin(p);
    let e = p.root(at - Vec2::Y * S * 0.5, flip, 0.0);
    let (w, h) = (34.0, 80.0);
    p.shape(e, &rect(-w - 6.0, 0.0, w + 6.0, h + 8.0), 0.45, M::Concrete, 0.0);
    p.part(e, stroke(&[Vec2::new(-w - 7.0, h + 7.5), Vec2::new(w + 7.0, h + 7.5)], |_| 2.2, 1.0, 0.6, |_| 1.0), M::Rim, 0.005);
    hazard_v(p, e, -w - 5.0, -w, 0.0, h, 0.01);
    hazard_v(p, e, w, w + 5.0, 0.0, h, 0.01);
    hazard_h(p, e, -w - 5.0, w + 5.0, h, h + 4.0, 0.012);
    // The leaf: thick steel, banded.
    let leaf = smooth_closed(&rect(-w + 1.0, 1.0, w - 1.0, h - 1.0), 1);
    p.shape(e, &leaf, 0.5, M::Steel, 0.02);
    p.part(e, stroke(&[Vec2::new(-w + 1.0, h - 1.0), Vec2::new(-w + 1.0, 1.0)], |_| 1.8, 1.0, 1.0, |_| 1.0), M::Rim, 0.025);
    for y in [h * 0.22, h * 0.78] {
        p.shape(e, &rect(-w + 2.0, y - 3.0, w - 2.0, y + 3.0), 0.6, M::Iron, 0.03);
        rivet_line(p, e, Vec2::new(-w + 4.0, y), Vec2::new(w - 4.0, y), 5.0, 0.035);
    }
    rivet_line(p, e, Vec2::new(-w + 3.0, h - 3.0), Vec2::new(w - 3.0, h - 3.0), 5.0, 0.035);
    rivet_line(p, e, Vec2::new(-w + 3.0, 3.0), Vec2::new(w - 3.0, 3.0), 5.0, 0.035);
    // Hinges on the far side.
    for y in [h * 0.15, h * 0.85] {
        p.shape(e, &rect(w - 3.0, y - 5.0, w + 3.0, y + 5.0), 0.5, M::Iron, 0.04);
    }
    // Wheel lock.
    let c = Vec2::new(-w * 0.15, h * 0.5);
    p.ring(e, c, 10.0, 2.2, M::Iron, 0.05, |_| 1.0);
    for k in 0..3 {
        let d = Vec2::from_angle(k as f32 * 1.047 + p.rng.range(0.0, 1.0)) * 10.0;
        p.part(e, stroke(&[c - d, c + d], |_| 1.4, 1.0, 0.6, |_| 1.0), M::Iron, 0.05);
    }
    p.part(e, fill(&ellipse_pts(2.6, 2.6, 10).into_iter().map(|q| q + c).collect::<Vec<_>>(), |_| 0.9), M::Steel, 0.06);
    // Dogging bolts to the jamb.
    p.bar(e, &[c + Vec2::new(-10.0, 0.0), Vec2::new(-w + 2.0, c.y)], 2.4, M::Steel, 0.04);
    // Scorched where something tried to burn through.
    if p.rng.chance(0.6) {
        let sc = Vec2::new(p.rng.range(-w * 0.6, w * 0.6), p.rng.range(h * 0.3, h * 0.7));
        let pts: Vec<Vec2> = rock_pts(&mut p.rng, 6.0, 9).into_iter().map(|q| q + sc).collect();
        p.part(e, fill(&pts, |_| 0.9), M::Soot, 0.045);
    }
    // A dead warning beacon over the door.
    let b = Vec2::new(0.0, h + 8.0);
    p.shape(e, &rect(-4.0, b.y, 4.0, b.y + 2.0), 0.6, M::Iron, 0.05);
    let dome: Vec<Vec2> = (0..=8).map(|i| b + Vec2::Y * 2.0 + Vec2::from_angle(i as f32 / 8.0 * std::f32::consts::PI) * 3.4).collect();
    p.shape(e, &dome, 0.6, M::Enamel, 0.05);
    true
}

/// A control console: olive desk, a slanted switch panel, dead CRTs.
fn console(p: &mut Props, at: Vec2) -> bool {
    let flip = flip_coin(p);
    let e = p.root(at - Vec2::Y * S * 0.5, flip, 0.0);
    let w = 26.0;
    p.shape(e, &rect(-w, 0.0, w, 22.0), 0.45, M::Slime, 0.0);
    p.shape(e, &rect(-w + 3.0, 3.0, -w + 18.0, 18.0), 0.7, M::Iron, 0.01);
    p.part(e, stroke(&[Vec2::new(-w + 6.0, 10.5), Vec2::new(-w + 15.0, 10.5)], |_| 1.2, 1.0, 1.0, |_| 1.0), M::Steel, 0.015);
    // Slanted panel with switches and dials.
    let panel = [Vec2::new(-w - 1.0, 22.0), Vec2::new(-w + 2.0, 29.0), Vec2::new(w - 2.0, 29.0), Vec2::new(w + 1.0, 22.0)];
    p.shape(e, &panel, 0.7, M::Iron, 0.02);
    let mut knobs = Vec::new();
    for i in 0..9 {
        let x = -w + 5.0 + i as f32 * 5.0;
        let up = p.rng.chance(0.5);
        p.part(e, stroke(&[Vec2::new(x, 25.0), Vec2::new(x + if up { -1.0 } else { 1.0 }, 27.5)], |_| 0.8, 1.0, 1.0, |_| 1.0), M::Steel, 0.03);
        knobs.push(ellipse_pts(0.9, 0.7, 6).into_iter().map(|q| q + Vec2::new(x, 24.5)).collect::<Vec<_>>());
    }
    p.part(e, blobs(&knobs, |_| 1.0), M::Enamel, 0.03);
    // Upper cabinet with two screens and a dial meter.
    p.shape(e, &rect(-w + 2.0, 29.0, w - 2.0, 54.0), 0.5, M::Slime, 0.01);
    p.part(e, stroke(&[Vec2::new(-w + 2.0, 53.5), Vec2::new(w - 2.0, 53.5)], |_| 1.4, 1.0, 1.0, |_| 1.0), M::Rim, 0.015);
    let cracked = p.rng.below(3);
    for (k, cx) in [-w * 0.45, w * 0.18].into_iter().enumerate() {
        let c = Vec2::new(cx, 42.0);
        let bezel = smooth_closed(&rect(c.x - 9.0, c.y - 8.0, c.x + 9.0, c.y + 8.0), 2);
        p.shape(e, &bezel, 0.6, M::Iron, 0.02);
        let screen = smooth_closed(&rect(c.x - 7.0, c.y - 6.0, c.x + 7.0, c.y + 6.0), 3);
        p.part(e, fill(&screen, |q| 0.8 + 0.6 * ((q - c).length() / 9.0)), M::Soot, 0.03);
        // Glare on the dead glass.
        p.part(e, stroke(&smooth_open(&[c + Vec2::new(-5.0, 2.0), c + Vec2::new(-3.5, 4.5), c + Vec2::new(-0.5, 5.0)], 4), |_| 0.9, 1.0, 1.0, |_| 0.6), M::Steel, 0.035);
        if k == cracked {
            let hit = c + Vec2::new(p.rng.range(-3.0, 3.0), p.rng.range(-3.0, 2.0));
            for _ in 0..4 {
                let d = Vec2::from_angle(p.rng.range(0.0, 6.28)) * p.rng.range(3.0, 6.0);
                let end = (hit + d).clamp(c - Vec2::new(6.5, 5.5), c + Vec2::new(6.5, 5.5));
                p.part(e, stroke(&[hit, end], |t| 0.7 * (1.0 - t) + 0.2, 1.0, 1.0, |_| 1.0), M::Rim, 0.036);
            }
        }
    }
    let m = Vec2::new(w * 0.68, 46.0);
    p.part(e, fill(&ellipse_pts(3.6, 3.6, 12).into_iter().map(|q| q + m).collect::<Vec<_>>(), |_| 0.9), M::Enamel, 0.03);
    let needle = Vec2::from_angle(p.rng.range(0.4, 2.7)) * 3.0;
    p.part(e, stroke(&[m, m + needle], |_| 0.6, 1.0, 1.0, |_| 1.0), M::Soot, 0.035);
    // Lamps in a row, all dark.
    let lamps: Vec<Vec<Vec2>> = (0..4).map(|i| ellipse_pts(0.9, 0.9, 6).into_iter().map(|q| q + Vec2::new(w * 0.52 + i as f32 * 2.4 - 3.6, 36.0)).collect()).collect();
    p.part(e, blobs(&lamps, |_| 1.0), M::Soot, 0.03);
    // Cables spilling from under the desk.
    for _ in 0..2 {
        let x = p.rng.range(0.0, w - 4.0);
        let end = Vec2::new(x + p.rng.range(6.0, 16.0), 0.6);
        p.part(e, stroke(&smooth_open(&[Vec2::new(x, 4.0), Vec2::new(x + 2.0, 0.8), end], 5), |_| 1.3, 0.8, 0.5, |_| 1.0), M::Soot, 0.04);
    }
    true
}

/// A metal bunk frame, the upper berth collapsed onto the lower one.
fn bunk(p: &mut Props, at: Vec2) -> bool {
    let flip = flip_coin(p);
    let e = p.root(at - Vec2::Y * S * 0.5, flip, 0.0);
    let (w, low, high) = (26.0, 11.0, 34.0);
    let fallen = p.rng.chance(0.6);
    // Posts.
    for x in [-w, w] {
        let top = if fallen && x > 0.0 { low + 6.0 } else { high + 14.0 };
        p.bar(e, &[Vec2::new(x, 0.0), Vec2::new(x, top)], 2.2, M::Iron, 0.03);
    }
    // Lower berth: frame, sagging springs, a thin mattress.
    p.bar(e, &[Vec2::new(-w, low), Vec2::new(w, low)], 1.8, M::Iron, 0.03);
    let springs: Vec<Vec2> = (0..=24).map(|i| Vec2::new(-w + 2.0 + i as f32 * (2.0 * w - 4.0) / 24.0, low - 1.0 - if i % 2 == 0 { 0.0 } else { 1.4 })).collect();
    p.part(e, stroke(&springs, |_| 0.5, 1.0, 1.0, |_| 1.0), M::Steel, 0.02);
    let sag = |x: f32| low + 1.0 - 2.0 * (1.0 - (x / w).powi(2));
    let top: Vec<Vec2> = (0..=10).map(|i| { let x = -w + 1.0 + i as f32 * (2.0 * w - 2.0) / 10.0; Vec2::new(x, sag(x) + 4.0) }).collect();
    let mut mat = top.clone();
    mat.push(Vec2::new(w - 1.0, low));
    mat.push(Vec2::new(-w + 1.0, low));
    p.shape(e, &smooth_closed(&mat, 2), 0.55, M::Plastic, 0.01);
    // Stains on it.
    let sc = Vec2::new(p.rng.range(-w * 0.5, w * 0.5), low + 2.5);
    p.part(e, fill(&ellipse_pts(5.0, 1.4, 10).into_iter().map(|q| q + sc).collect::<Vec<_>>(), |_| 0.8), M::Rust, 0.015);
    // Upper berth.
    if fallen {
        let (a, b) = (Vec2::new(-w, high), Vec2::new(w - 2.0, low + 6.0));
        p.bar(e, &[a, b], 1.8, M::Iron, 0.02);
        let n = 16;
        let zig: Vec<Vec2> = (0..=n).map(|i| a.lerp(b, i as f32 / n as f32) - Vec2::Y * if i % 2 == 0 { 0.0 } else { 1.4 }).collect();
        p.part(e, stroke(&zig, |_| 0.5, 1.0, 1.0, |_| 1.0), M::Steel, 0.015);
        // A blanket sliding off.
        let bl = [a + Vec2::new(4.0, 1.0), a.lerp(b, 0.5) + Vec2::new(0.0, 2.0), a.lerp(b, 0.55) + Vec2::new(2.0, -8.0), a.lerp(b, 0.3) + Vec2::new(0.0, -10.0)];
        p.shape(e, &smooth_closed(&bl, 3), 0.5, M::Slime, 0.016);
    } else {
        p.bar(e, &[Vec2::new(-w, high), Vec2::new(w, high)], 1.8, M::Iron, 0.03);
        let springs: Vec<Vec2> = (0..=24).map(|i| Vec2::new(-w + 2.0 + i as f32 * (2.0 * w - 4.0) / 24.0, high - 1.0 - if i % 2 == 0 { 0.0 } else { 1.4 })).collect();
        p.part(e, stroke(&springs, |_| 0.5, 1.0, 1.0, |_| 1.0), M::Steel, 0.02);
        // A blanket hanging over the side.
        let x = p.rng.range(-w * 0.6, w * 0.3);
        let bl = [Vec2::new(x, high + 1.0), Vec2::new(x + 14.0, high + 1.0), Vec2::new(x + 13.0, high - 12.0), Vec2::new(x + 1.0, high - 9.0)];
        p.shape(e, &bl, 0.55, M::Slime, 0.025);
    }
    // Ladder rungs at the head end.
    for y in [low + 7.0, low + 14.0, low + 21.0] {
        if !fallen || y < low + 8.0 {
            p.bar(e, &[Vec2::new(-w, y), Vec2::new(-w + 6.0, y)], 1.2, M::Iron, 0.03);
        }
    }
    true
}

/// A row of steel lockers, one door hanging open.
fn lockers(p: &mut Props, at: Vec2) -> bool {
    let flip = flip_coin(p);
    let e = p.root(at - Vec2::Y * S * 0.5, flip, 0.0);
    let n = 3;
    let (lw, h) = (13.0, 52.0);
    let x0 = -lw * n as f32 / 2.0;
    let open = p.rng.below(n);
    for i in 0..n {
        let (a, b) = (x0 + i as f32 * lw, x0 + (i + 1) as f32 * lw);
        let lean = if i == n - 1 && p.rng.chance(0.4) { 2.0 } else { 0.0 };
        if i == open {
            p.shape(e, &rect(a + 0.5, 1.0, b - 0.5, h - 1.0), 0.6, M::Soot, 0.0);
            // Something hanging inside: a gas mask on a hook.
            if p.rng.chance(0.7) {
                gas_mask(p, e, Vec2::new((a + b) / 2.0, h * 0.62), 0.0, 0.01);
            }
            // The door swung out, seen edge-on.
            p.shape(e, &[Vec2::new(a, 1.0), Vec2::new(a - 5.0, 3.0), Vec2::new(a - 5.0, h - 3.0), Vec2::new(a, h - 1.0)], 0.55, M::Steel, 0.03);
        } else {
            p.shape(e, &[Vec2::new(a, 0.0), Vec2::new(a + lean, h), Vec2::new(b + lean, h), Vec2::new(b, 0.0)], 0.5, M::Steel, 0.01);
            // Vents and a handle.
            for k in 0..4 {
                let y = h - 7.0 - k as f32 * 2.0;
                p.part(e, stroke(&[Vec2::new(a + 3.0, y), Vec2::new(b - 3.0, y)], |_| 0.8, 1.0, 1.0, |_| 1.0), M::Soot, 0.02);
            }
            p.part(e, stroke(&[Vec2::new(b - 3.0, h * 0.45), Vec2::new(b - 3.0, h * 0.55)], |_| 1.2, 1.0, 1.0, |_| 1.0), M::Iron, 0.02);
            if p.rng.chance(0.5) {
                // A dent.
                let c = Vec2::new((a + b) / 2.0, p.rng.range(12.0, 30.0));
                let pts: Vec<Vec2> = rock_pts(&mut p.rng, 3.5, 7).into_iter().map(|q| q + c).collect();
                p.part(e, fill(&pts, |q| 0.6 + 0.4 * ((q.y - c.y) / 3.5 + 0.5).clamp(0.0, 1.0)), M::Iron, 0.015);
            }
        }
        p.part(e, stroke(&[Vec2::new(a, 0.5), Vec2::new(a, h)], |_| 0.8, 1.0, 1.0, |_| 1.0), M::Iron, 0.025);
    }
    p.part(e, stroke(&[Vec2::new(x0, h - 0.5), Vec2::new(-x0, h - 0.5)], |_| 1.6, 1.0, 0.6, |_| 1.0), M::Rim, 0.03);
    true
}

/// One sandbag lying lengthways at `c`.
fn sandbag(p: &mut Props, e: Entity, c: Vec2, w: f32, h: f32, z: f32) {
    let pts = [
        c + Vec2::new(-w, -h * 0.5),
        c + Vec2::new(-w - 1.0, h * 0.1),
        c + Vec2::new(-w * 0.6, h * 0.5),
        c + Vec2::new(w * 0.6, h * 0.55),
        c + Vec2::new(w + 1.0, h * 0.1),
        c + Vec2::new(w, -h * 0.5),
    ];
    let out = smooth_closed(&pts, 4);
    p.shape(e, &out, 0.45, M::Bronze, z);
    // Tied end and a seam.
    p.part(e, stroke(&[c + Vec2::new(w - 2.5, -h * 0.4), c + Vec2::new(w - 2.0, h * 0.4)], |_| 0.8, 1.0, 1.0, |_| 1.0), M::Soot, z + 0.002);
    p.part(e, stroke(&[c + Vec2::new(-w * 0.7, h * 0.3), c + Vec2::new(w * 0.5, h * 0.35)], |_| 0.6, 1.0, 1.0, |_| 1.0), M::Rim, z + 0.002);
}

/// A low wall of sandbags, slumping, one torn and spilling.
fn sandbags(p: &mut Props, at: Vec2) -> bool {
    let flip = flip_coin(p);
    let e = p.root(at - Vec2::Y * S * 1.0, flip, 0.0);
    let (w, h) = (6.5, 6.0);
    let rows = [4usize, 3, 2];
    let mut z = 0.0;
    for (r, &n) in rows.iter().enumerate() {
        let n = if r == 2 && p.rng.chance(0.4) { 1 } else { n };
        for i in 0..n {
            let x = (i as f32 - (n as f32 - 1.0) / 2.0) * (2.0 * w + 0.5) + p.rng.range(-1.0, 1.0);
            let y = h * 0.5 + r as f32 * (h - 1.0);
            z += 0.004;
            sandbag(p, e, Vec2::new(x, y), w, h, z);
        }
    }
    // Torn bag and the sand spilled from it.
    let x = p.rng.range(12.0, 22.0);
    let pile = [Vec2::new(x - 9.0, 0.0), Vec2::new(x - 4.0, 2.5), Vec2::new(x + 1.0, 3.2), Vec2::new(x + 6.0, 1.6), Vec2::new(x + 10.0, 0.0)];
    p.shape(e, &smooth_closed(&pile, 3), 0.6, M::Concrete, z + 0.01);
    let flat = [Vec2::new(x - 6.0, 2.0), Vec2::new(x - 2.0, 4.6), Vec2::new(x + 4.0, 4.2), Vec2::new(x + 7.0, 2.4)];
    p.shape(e, &smooth_closed(&flat, 3), 0.5, M::Bronze, z + 0.02);
    true
}

/// One ammo crate (olive, stenciled, clasps, rope handle).
fn crate_box(p: &mut Props, e: Entity, c: Vec2, w: f32, h: f32, z: f32, open: bool) {
    p.shape(e, &rect(c.x - w, c.y, c.x + w, c.y + h), 0.5, M::Slime, z);
    // Lid line, edge battens.
    p.part(e, stroke(&[Vec2::new(c.x - w, c.y + h - 3.0), Vec2::new(c.x + w, c.y + h - 3.0)], |_| 0.8, 1.0, 1.0, |_| 1.0), M::Soot, z + 0.002);
    for x in [c.x - w + 1.2, c.x + w - 1.2] {
        p.part(e, stroke(&[Vec2::new(x, c.y), Vec2::new(x, c.y + h)], |_| 2.0, 1.0, 0.6, |_| 1.0), M::Wood, z + 0.003);
    }
    p.part(e, stroke(&[Vec2::new(c.x - w, c.y + h - 0.6), Vec2::new(c.x + w, c.y + h - 0.6)], |_| 1.2, 1.0, 1.0, |_| 1.0), M::Rim, z + 0.004);
    // Clasps.
    for x in [c.x - w * 0.5, c.x + w * 0.5] {
        p.shape(e, &rect(x - 1.2, c.y + h - 5.5, x + 1.2, c.y + h - 1.5), 0.6, M::Iron, z + 0.005);
    }
    // Stencil: a block of light dashes.
    let mut dashes = Vec::new();
    let mut x = c.x - w * 0.55;
    let y = c.y + h * 0.35;
    while x < c.x + w * 0.5 {
        let l = p.rng.range(1.0, 2.6);
        dashes.push(rect(x, y, x + l, y + 2.2));
        x += l + 0.9;
    }
    p.part(e, blobs(&dashes, |_| 0.8), M::Enamel, z + 0.005);
    if open {
        // Lid knocked off: shells inside.
        let mut shells = Vec::new();
        for i in 0..((w * 2.0 / 3.2) as i32) {
            let sx = c.x - w + 2.4 + i as f32 * 3.2;
            shells.push(vec![Vec2::new(sx - 1.1, c.y + h - 2.0), Vec2::new(sx - 1.1, c.y + h + 3.0), Vec2::new(sx, c.y + h + 5.0), Vec2::new(sx + 1.1, c.y + h + 3.0), Vec2::new(sx + 1.1, c.y + h - 2.0)]);
        }
        p.part(e, blobs(&shells, |q| 0.6 + 0.4 * ((q.y - c.y - h) / 5.0).clamp(0.0, 1.0)), M::Bronze, z - 0.001);
    }
}

/// A stack of ammo crates.
fn crates(p: &mut Props, at: Vec2) -> bool {
    let flip = flip_coin(p);
    let e = p.root(at - Vec2::Y * S * 0.5, flip, 0.0);
    let (w, h) = (10.0, 11.0);
    crate_box(p, e, Vec2::new(-w - 0.5, 0.0), w, h, 0.0, false);
    let second = p.rng.chance(0.7);
    if second {
        crate_box(p, e, Vec2::new(w + 0.5, 0.0), w, h, 0.01, false);
    }
    let x = p.rng.range(-w * 0.8, if second { w * 0.6 } else { -w * 0.4 });
    let open = p.rng.chance(0.4);
    crate_box(p, e, Vec2::new(x, h), w, h, 0.03, open);
    if !open && p.rng.chance(0.4) {
        let dx = p.rng.range(-4.0, 4.0);
        let angle = p.rng.range(-0.3, 0.3);
        gas_mask(p, e, Vec2::new(x + dx, 2.0 * h + 0.5), angle, 0.05);
    }
    true
}

/// One upright drum at `c` (its foot).
fn drum(p: &mut Props, e: Entity, c: Vec2, r: f32, h: f32, z: f32, m: M, sign: bool) {
    p.shape(e, &rect(c.x - r, c.y, c.x + r, c.y + h), 0.45, m, z);
    for y in [h * 0.33, h * 0.66] {
        p.part(e, stroke(&[Vec2::new(c.x - r, c.y + y), Vec2::new(c.x + r, c.y + y)], |_| 1.4, 1.0, 0.5, |_| 1.0), M::Iron, z + 0.002);
    }
    p.part(e, stroke(&[Vec2::new(c.x - r - 0.5, c.y + h), Vec2::new(c.x + r + 0.5, c.y + h)], |_| 1.6, 1.0, 1.0, |_| 1.0), M::Rim, z + 0.003);
    p.part(e, stroke(&[Vec2::new(c.x - r + 1.2, c.y), Vec2::new(c.x - r + 1.2, c.y + h)], |_| 1.4, 1.0, 1.0, |_| 0.8), M::Steel, z + 0.002);
    if sign {
        let s = Vec2::new(c.x + r * 0.15, c.y + h * 0.5);
        p.part(e, fill(&ellipse_pts(3.6, 3.6, 12).into_iter().map(|q| q + s).collect::<Vec<_>>(), |_| 0.85), M::Enamel, z + 0.004);
        trefoil(p, e, s, 3.0, z + 0.005);
    }
    // Rust bleeding through.
    let x = p.rng.range(c.x - r * 0.6, c.x + r * 0.6);
    p.part(e, stroke(&[Vec2::new(x, c.y + h - 2.0), Vec2::new(x + 0.4, c.y + h * 0.3)], |t| 1.8 * (1.0 - t) + 0.3, 1.0, 1.0, |_| 1.0), M::Rust, z + 0.003);
}

/// Oil drums: a couple upright, maybe one toppled and leaking.
fn drums(p: &mut Props, at: Vec2) -> bool {
    let flip = flip_coin(p);
    let e = p.root(at - Vec2::Y * S * 0.5, flip, 0.0);
    let (r, h) = (7.0, 22.0);
    let mats = [M::Rust, M::Slime, M::Steel];
    let m0 = mats[p.rng.below(3)];
    let sign0 = p.rng.chance(0.5);
    drum(p, e, Vec2::new(-r - 0.5, 0.0), r, h, 0.0, m0, sign0);
    if p.rng.chance(0.6) {
        let m1 = mats[p.rng.below(3)];
        let shorter = p.rng.range(0.0, 2.0);
        drum(p, e, Vec2::new(r + 0.5, 0.0), r, h - shorter, 0.01, m1, false);
    } else {
        // Toppled: lying on its side, the open end toward us, oil pooled.
        let c = Vec2::new(r + 4.0 + h / 2.0, r);
        p.shape(e, &rect(c.x - h / 2.0, 0.0, c.x + h / 2.0, 2.0 * r), 0.45, m0, 0.01);
        for x in [c.x - h * 0.17, c.x + h * 0.17] {
            p.part(e, stroke(&[Vec2::new(x, 0.0), Vec2::new(x, 2.0 * r)], |_| 1.4, 1.0, 0.5, |_| 1.0), M::Iron, 0.012);
        }
        let end: Vec<Vec2> = ellipse_pts(2.6, r, 14).into_iter().map(|q| q + Vec2::new(c.x + h / 2.0, r)).collect();
        p.part(e, fill(&end, |_| 0.9), M::Iron, 0.013);
        let hole: Vec<Vec2> = ellipse_pts(1.0, 1.6, 8).into_iter().map(|q| q + Vec2::new(c.x + h / 2.0 + 0.6, r * 0.4)).collect();
        p.part(e, fill(&hole, |_| 1.0), M::Soot, 0.014);
        let pool: Vec<Vec2> = ellipse_pts(p.rng.range(9.0, 14.0), 1.4, 16).into_iter().map(|q| q + Vec2::new(c.x + h / 2.0 + 6.0, 0.3)).collect();
        p.part(e, fill(&pool, |_| 0.8), M::Soot, 0.015);
    }
    true
}

/// A toppled missile section: a banded tube with fins, broken open at one end.
fn missile(p: &mut Props, at: Vec2) -> bool {
    let flip = flip_coin(p);
    let lean = p.rng.range(0.06, 0.14);
    let e = p.root(at - Vec2::Y * S * 1.5, flip, lean);
    let (l, r) = (64.0, 9.0);
    let (x0, x1) = (-l / 2.0, l / 2.0);
    let y = r;
    // Fins at the tail (x0), one flat in front, one up, one down behind the body.
    let fin_up = [Vec2::new(x0 + 2.0, y + r - 1.0), Vec2::new(x0 - 5.0, y + r + 14.0), Vec2::new(x0 + 4.0, y + r + 14.0), Vec2::new(x0 + 16.0, y + r - 1.0)];
    p.shape(e, &fin_up, 0.6, M::Steel, 0.0);
    // Body with its tail nozzle.
    let nozzle = [Vec2::new(x0 - 6.0, y - r * 0.7), Vec2::new(x0 - 6.0, y + r * 0.7), Vec2::new(x0, y + r * 0.85), Vec2::new(x0, y - r * 0.85)];
    p.shape(e, &nozzle, 0.4, M::Iron, 0.005);
    let body = smooth_closed(&rect(x0, y - r, x1, y + r), 1);
    p.part(e, fill(&body, |q| 0.5 + 0.55 * (1.0 - ((q.y - y - r * 0.3) / r).abs()).clamp(0.0, 1.0)), M::Enamel, 0.01);
    p.part(e, stroke(&[Vec2::new(x0, y + r * 0.45), Vec2::new(x1 - 2.0, y + r * 0.45)], |_| 1.6, 1.0, 1.0, |_| 0.9), M::Rim, 0.012);
    // Bands and a stencil.
    for bx in [x0 + 6.0, x0 + 22.0] {
        p.part(e, stroke(&[Vec2::new(bx, y - r), Vec2::new(bx, y + r)], |_| 2.6, 1.0, 1.0, |_| 1.0), M::Soot, 0.015);
    }
    rivet_line(p, e, Vec2::new(x0 + 14.0, y - r + 1.5), Vec2::new(x0 + 14.0, y + r - 1.5), 3.0, 0.016);
    let mut letters = Vec::new();
    let mut lx = x0 + 28.0;
    while lx < x1 - 12.0 {
        let w = p.rng.range(1.2, 2.6);
        letters.push(rect(lx, y - 2.5, lx + w, y + 1.5));
        lx += w + 1.0;
    }
    p.part(e, blobs(&letters, |_| 1.0), M::Soot, 0.016);
    // The flat fin in front, hitting the floor.
    let fin = [Vec2::new(x0 + 1.0, y - 2.0), Vec2::new(x0 - 4.0, y - r - 3.0), Vec2::new(x0 + 6.0, y - r - 3.0), Vec2::new(x0 + 15.0, y - 2.0)];
    p.shape(e, &fin, 0.55, M::Steel, 0.02);
    // Broken front: jagged, dark inside, wires out.
    let mut jag = vec![Vec2::new(x1 - 4.0, y + r)];
    for i in 1..6 {
        let t = i as f32 / 6.0;
        jag.push(Vec2::new(x1 + p.rng.range(-3.0, 3.0), y + r - 2.0 * r * t));
    }
    jag.push(Vec2::new(x1 - 4.0, y - r));
    jag.push(Vec2::new(x1 - 8.0, y));
    p.part(e, fill(&jag, |_| 1.0), M::Soot, 0.02);
    for _ in 0..3 {
        let s = Vec2::new(x1 - 2.0, y + p.rng.range(-r * 0.6, r * 0.6));
        let end = s + Vec2::new(p.rng.range(5.0, 12.0), p.rng.range(-8.0, 3.0));
        p.part(e, stroke(&smooth_open(&[s, s.lerp(end, 0.5) + Vec2::Y * 2.0, end], 4), |_| 0.8, 1.0, 1.0, |_| 1.0), M::Steel, 0.022);
    }
    true
}

/// A gas mask lying at `c`: rubber face, two goggle eyes, a filter canister.
fn gas_mask(p: &mut Props, e: Entity, c: Vec2, angle: f32, z: f32) {
    let rot = |q: Vec2| c + Vec2::from_angle(angle).rotate(q);
    let face: Vec<Vec2> = [Vec2::new(-6.0, 2.0), Vec2::new(-5.0, 7.5), Vec2::new(0.0, 9.5), Vec2::new(5.0, 7.5), Vec2::new(6.0, 2.0), Vec2::new(2.5, -0.5), Vec2::new(-2.5, -0.5)].map(rot).to_vec();
    p.shape(e, &smooth_closed(&face, 4), 0.45, M::Rubber, z);
    for s in [-1.0, 1.0] {
        let eye = rot(Vec2::new(s * 2.8, 5.8));
        p.part(e, fill(&ellipse_pts(2.2, 2.2, 10).into_iter().map(|q| q + eye).collect::<Vec<_>>(), |_| 1.0), M::Iron, z + 0.002);
        p.part(e, fill(&ellipse_pts(1.5, 1.5, 10).into_iter().map(|q| q + eye).collect::<Vec<_>>(), |_| 1.0), M::Soot, z + 0.003);
        p.part(e, stroke(&[eye + Vec2::new(-0.8, 0.4), eye + Vec2::new(0.0, 1.0)], |_| 0.5, 1.0, 1.0, |_| 1.0), M::Rim, z + 0.004);
    }
    let can = rot(Vec2::new(0.0, 0.6));
    p.part(e, fill(&ellipse_pts(2.4, 2.4, 10).into_iter().map(|q| q + can).collect::<Vec<_>>(), |_| 0.9), M::Steel, z + 0.004);
    p.ring(e, can, 1.6, 0.5, M::Soot, z + 0.005, |_| 1.0);
    // A strap.
    let strap = [rot(Vec2::new(-5.5, 6.0)), rot(Vec2::new(-8.5, 3.0)), rot(Vec2::new(-9.0, -0.5))];
    p.part(e, stroke(&smooth_open(&strap, 4), |_| 0.8, 1.0, 1.0, |_| 1.0), M::Rubber, z + 0.001);
}

/// A caged bulkhead lamp, hanging from `top` (its cap).
fn cage_lamp_hanging(p: &mut Props, e: Entity, top: Vec2, z: f32) {
    let cap = [top + Vec2::new(-4.5, 0.0), top + Vec2::new(-3.0, 2.0), top + Vec2::new(3.0, 2.0), top + Vec2::new(4.5, 0.0)];
    p.shape(e, &cap, 0.5, M::Iron, z);
    let glass: Vec<Vec2> = (0..=10).map(|i| top + Vec2::from_angle(std::f32::consts::PI + i as f32 / 10.0 * std::f32::consts::PI) * Vec2::new(4.0, 6.0)).collect();
    let broken = p.rng.chance(0.3);
    p.part(e, fill(&glass, |q| 0.7 + 0.3 * ((q.y - top.y) / 6.0 + 1.0)), if broken { M::Soot } else { M::Enamel }, z + 0.002);
    // The cage.
    for k in [-0.6, 0.0, 0.6] {
        let x = k * 3.6;
        let bar = [top + Vec2::new(x * 1.2, 0.0), top + Vec2::new(x * 1.1, -4.0), top + Vec2::new(x * 0.5, -6.8)];
        p.part(e, stroke(&smooth_open(&bar, 3), |_| 0.7, 1.0, 1.0, |_| 1.0), M::Iron, z + 0.004);
    }
    let ring: Vec<Vec2> = (0..=8).map(|i| top + Vec2::new(-4.4 + i as f32 * 1.1, -3.2 - 0.6 * (1.0 - ((i as f32 - 4.0) / 4.0).powi(2)))).collect();
    p.part(e, stroke(&ring, |_| 0.7, 1.0, 1.0, |_| 1.0), M::Iron, z + 0.004);
}

/// A caged bulkhead lamp on a wall bracket, seen from the side (`c` = its base).
fn cage_lamp(p: &mut Props, e: Entity, c: Vec2, z: f32) {
    p.shape(e, &rect(c.x - 1.0, c.y - 4.0, c.x + 1.5, c.y + 4.0), 0.5, M::Iron, z);
    let glass: Vec<Vec2> = (0..=10).map(|i| c + Vec2::X * 1.5 + Vec2::from_angle(-std::f32::consts::FRAC_PI_2 + i as f32 / 10.0 * std::f32::consts::PI) * Vec2::new(5.0, 3.6)).collect();
    p.part(e, fill(&glass, |q| 0.75 + 0.25 * ((q.y - c.y) / 3.6 + 1.0) / 2.0), M::Enamel, z + 0.002);
    for k in [-2.4, 0.0, 2.4] {
        let bar = [c + Vec2::new(1.5, k * 1.4), c + Vec2::new(5.0, k * 1.1), c + Vec2::new(7.2, k * 0.3)];
        p.part(e, stroke(&smooth_open(&bar, 3), |_| 0.7, 1.0, 1.0, |_| 1.0), M::Iron, z + 0.004);
    }
}

/// A bulkhead lamp hanging on a long conduit from the corridor's ceiling, and
/// sometimes a run of duct along it. False if there's no ceiling overhead.
fn hanging_lamp(p: &mut Props, cell: IVec2, at: Vec2) -> bool {
    let t = p.t;
    let reach = cells(1000.0);
    let Some(k) = (1..=reach).find(|&k| t.solid(cell - IVec2::Y * k)) else { return false };
    let ceil_cell = cell - IVec2::Y * k;
    let ceil = t.cell_min(ceil_cell).y;
    let gap = ceil - at.y;
    if gap < 90.0 {
        return false;
    }
    // Low enough to be seen from the floor.
    let lamp_y = at.y + (gap - 20.0).min(p.rng.range(170.0, 250.0));
    let top = Vec2::new(at.x, ceil);
    let e = p.root(top, 1.0, 0.0);
    let len = (ceil - lamp_y) / S;
    // Conduit with a junction box halfway, a cable looping off it.
    p.bar(e, &[Vec2::new(0.0, 3.0), Vec2::new(0.0, -len)], 1.8, M::Iron, 0.0);
    let jb = -len * p.rng.range(0.3, 0.6);
    p.shape(e, &rect(-2.5, jb - 3.0, 2.5, jb + 3.0), 0.55, M::Slime, 0.01);
    if p.rng.chance(0.6) {
        let sag = p.rng.range(10.0, 30.0);
        let side = flip_coin(p);
        let path = smooth_open(&[Vec2::new(side * 2.0, jb), Vec2::new(side * 14.0, jb - sag), Vec2::new(side * 30.0, jb - sag * 0.4), Vec2::new(side * 40.0, jb + 4.0)], 6);
        p.part(e, stroke(&path, |_| 1.0, 0.8, 0.5, |_| 1.0), M::Soot, 0.005);
    }
    cage_lamp_hanging(p, e, Vec2::new(0.0, -len), 0.02);
    // A duct along the ceiling, if the ceiling is level for its whole run.
    let half = 50.0;
    let level = (-cells(half * S)..=cells(half * S)).all(|dx| {
        let c = ceil_cell + IVec2::X * dx;
        t.solid(c) && !t.solid(c + IVec2::Y)
    });
    if level && p.rng.chance(0.7) {
        p.shape(e, &rect(-half, -14.0, half, 0.5), 0.55, M::Steel, 0.03);
        let mut x = -half;
        while x <= half {
            p.part(e, stroke(&[Vec2::new(x, -15.0), Vec2::new(x, 0.5)], |_| 1.6, 1.0, 0.5, |_| 1.0), M::Iron, 0.035);
            x += 20.0;
        }
        // A vent grille.
        let gx = p.rng.range(-half * 0.6, half * 0.6);
        for i in 0..4 {
            let yy = -11.0 + i as f32 * 2.4;
            p.part(e, stroke(&[Vec2::new(gx - 5.0, yy), Vec2::new(gx + 5.0, yy)], |_| 0.9, 1.0, 1.0, |_| 1.0), M::Soot, 0.04);
        }
    }
    true
}
