//! Props of "region: natural cave system" (`Style::Cave`): stalactites
//! hanging from the ceilings, stalagmites and columns on the floors, boulders
//! and scree, crystal clusters in the cracks, pale cave fungus, flowstone
//! curtains over the drops, roots breaking through from the surface, and the
//! odd sign of the war above (a helmet and what's left of its owner, a rusty
//! pipe, a dangling cable).

use super::*;

pub(super) const HOOKS: Hooks = Hooks { spacing, ground, edge, wall, ceiling };

/// (gap between props of the same kind, chance a free spot gets one), in prop units.
fn spacing(kind: Kind) -> (f32, f32) {
    match kind {
        Kind::Ground => (45.0, 0.85),
        Kind::Edge => (35.0, 0.8),
        Kind::Wall => (45.0, 0.75),
        Kind::Ceiling => (28.0, 0.9),
    }
}

#[derive(Clone, Copy)]
enum G {
    Stalagmites,
    Boulder,
    Scree,
    Mushrooms,
    Crystals,
    Pool,
    Helmet,
    Pipe,
}

impl G {
    /// (half width, height, may lie along a slope).
    fn size(self) -> (f32, f32, bool) {
        match self {
            G::Stalagmites => (14.0, 40.0, false),
            G::Boulder => (18.0, 26.0, true),
            G::Scree => (14.0, 8.0, true),
            G::Mushrooms => (10.0, 16.0, true),
            G::Crystals => (10.0, 22.0, true),
            G::Pool => (16.0, 6.0, false),
            G::Helmet => (16.0, 10.0, true),
            G::Pipe => (8.0, 30.0, true),
        }
    }
}

fn pick<T: Copy>(p: &mut Props, table: &[(T, f32)]) -> T {
    let total: f32 = table.iter().map(|e| e.1).sum();
    let mut r = p.rng.range(0.0, total);
    for e in table {
        r -= e.1;
        if r <= 0.0 {
            return e.0;
        }
    }
    table[0].0
}

fn side(p: &mut Props) -> f32 {
    if p.rng.chance(0.5) { 1.0 } else { -1.0 }
}

/// A top surface with headroom at `cell` (`x` = its world x). True if something was placed.
fn ground(p: &mut Props, cell: IVec2, x: f32) -> bool {
    floor(p, cell, x)
}

fn floor(p: &mut Props, cell: IVec2, x: f32) -> bool {
    // A low ceiling right above: stalactite and stalagmite grown into one.
    if let Some(h) = headroom(p, cell) {
        if p.rng.chance(0.55) {
            if let Some((y, _, indoor)) = p.fit(cell, 10.0, 6.0, false) {
                p.depth = if indoor { INDOOR } else { BACK };
                column(p, Vec2::new(x, y), h - (y - p.t.cell_min(cell).y - PX));
                p.depth = BACK;
                return true;
            }
        }
    }
    let first = pick(
        p,
        &[
            (G::Stalagmites, 4.0),
            (G::Boulder, 2.5),
            (G::Mushrooms, 2.0),
            (G::Crystals, 1.5),
            (G::Scree, 1.0),
            (G::Pool, 1.0),
            (G::Helmet, 0.5),
            (G::Pipe, 0.4),
        ],
    );
    for g in [first, G::Scree, G::Mushrooms] {
        let (half, head, tilt) = g.size();
        let Some((y, angle, indoor)) = p.fit(cell, half, head, tilt) else { continue };
        let at = Vec2::new(x, y);
        p.tilt = angle;
        p.depth = if indoor { INDOOR } else { BACK };
        match g {
            G::Stalagmites => stalagmites(p, at),
            G::Boulder => boulder(p, at),
            G::Scree => scree(p, at),
            G::Mushrooms => mushrooms(p, at),
            G::Crystals => crystal_patch(p, at),
            G::Pool => pool(p, at),
            G::Helmet => remains(p, at),
            G::Pipe => pipe_up(p, at),
        }
        p.tilt = 0.0;
        p.depth = BACK;
        return true;
    }
    false
}

/// Free height (world units) from the top of `cell` to a ceiling right above
/// it, when it's low enough for a column (and the ceiling is wide enough).
fn headroom(p: &Props, cell: IVec2) -> Option<f32> {
    let max = cells(170.0);
    let k = (1..=max).find(|&k| p.t.solid(cell - IVec2::Y * k))?;
    let h = (k - 1) as f32 * PX;
    let wide = [-1, 1].into_iter().all(|dx| p.t.solid(cell + IVec2::new(dx, -k)) || p.t.solid(cell + IVec2::new(dx, -k - 1)));
    (h > 60.0 && wide).then_some(h)
}

/// Where the ground ends at a drop (`flip` = the open side).
fn edge(p: &mut Props, at: Vec2, flip: f32) -> bool {
    let e = p.root(at, flip, 0.0);
    match pick(p, &[(0, 4.0), (1, 2.5), (2, 2.0), (3, 0.5)]) {
        0 => curtain(p, e),
        1 => p.roots(e, Vec2::new(-2.0, -3.0), 0.0),
        2 => {
            // Loose rocks about to go over.
            for i in 0..2 + p.rng.below(2) {
                let r = p.rng.range(3.0, 6.0);
                let c = Vec2::new(p.rng.range(-8.0, 2.0) + i as f32 * 3.0, r * 0.5);
                rock(p, e, c, r, 0.02 * i as f32);
            }
            if p.rng.chance(0.6) {
                curtain(p, e);
            }
        }
        _ => p.cables(e, Vec2::new(-3.0, -4.0), 1, 0.0),
    }
    true
}

/// A straight vertical face (`flip` = the open side).
fn wall(p: &mut Props, at: Vec2, flip: f32) -> bool {
    match pick(p, &[(0, 3.0), (1, 2.5), (2, 2.0), (3, 2.0), (4, 0.6)]) {
        0 => {
            // Crystals in a crack of the face.
            let c = at + Vec2::new(-flip * (6.0 * S + PX * 2.0), 0.0);
            let e = p.root(c, flip, 0.0);
            let half = Vec2::new(4.0, p.rng.range(9.0, 14.0));
            if p.inside(c, half * S) {
                crack_with_crystals(p, e, half);
            } else {
                let e = p.root(at, flip, 0.0);
                let a1 = p.rng.below(3);
                crystals(p, e, Vec2::new(-2.0, 0.0), 0.0, 3 + a1, 0.0);
            }
        }
        1 => {
            let e = p.root(at, flip, 0.0);
            brackets(p, e);
        }
        2 => {
            let e = p.root(at, flip, 0.0);
            let y = p.rng.range(-8.0, 8.0);
            p.roots(e, Vec2::new(-2.0, y), 0.0);
        }
        3 => {
            // Flowstone running down the face.
            let c = at + Vec2::new(-flip * (5.0 * S + PX * 1.5), 0.0);
            let half = Vec2::new(5.0, 18.0);
            if !p.inside(c, half * S) {
                return false;
            }
            let e = p.root(c, flip, 0.0);
            flowstone_streak(p, e, half);
        }
        _ => {
            let e = p.root(at, flip, 0.0);
            let r = p.rng.range(4.0, 6.0);
            let y = p.rng.range(-6.0, 6.0);
            p.pipe(e, at, flip, Vec2::new(0.0, y), r);
        }
    }
    true
}

/// The underside of a thick overhang.
fn ceiling(p: &mut Props, at: Vec2) -> bool {
    let flip = side(p);
    let e = p.root(at, flip, 0.0);
    match pick(p, &[(0, 6.0), (1, 1.5), (2, 1.0), (3, 0.4)]) {
        0 => stalactites(p, e),
        1 => {
            p.roots(e, Vec2::new(0.0, 2.0), -1.0);
            if p.rng.chance(0.5) {
                stalactites(p, e);
            }
        }
        2 => {
            let n = 2 + p.rng.below(3);
            crystals(p, e, Vec2::new(0.0, 2.0), std::f32::consts::PI, n, 0.0);
        }
        _ => {
            let a1 = p.rng.range(-6.0, 6.0);
            p.cables(e, Vec2::new(a1, 2.0), 1, -1.0);
            stalactites(p, e);
        }
    }
    true
}

// --- Drawing ---

/// A cone of dripstone hanging from `base` (`down` = −1) or standing on it (+1).
fn dripstone(p: &mut Props, e: Entity, base: Vec2, w: f32, len: f32, down: f32, z: f32) {
    let lean = p.rng.range(-0.12, 0.12) * len;
    let ribs = p.rng.range(0.85, 1.12);
    let y = |t: f32| base.y + down * len * t;
    let mut left = Vec::new();
    let mut right = Vec::new();
    let n = 6;
    for i in 0..n {
        let t = i as f32 / n as f32;
        // Thick at the root, thin and a little bulging along its length.
        let bulge = 1.0 + 0.12 * ((t * 9.0 * ribs).sin());
        let half = w * (1.0 - t).powf(0.8) * bulge;
        let x = base.x + lean * t * t;
        left.push(Vec2::new(x - half, y(t) - down * if i == 0 { 3.0 } else { 0.0 }));
        right.push(Vec2::new(x + half, y(t) - down * if i == 0 { 3.0 } else { 0.0 }));
    }
    let tip = Vec2::new(base.x + lean, y(1.0));
    let mut outline = left.clone();
    outline.push(tip);
    outline.extend(right.iter().rev());
    let outline = smooth_closed(&outline, 3);
    // Lit along its left side, the far side in shadow.
    let (x0, x1) = (base.x - w, base.x + w + lean.abs());
    p.part(e, fill(&outline, |q| 0.4 + 0.5 * (1.0 - ((q.x - x0) / (x1 - x0)).clamp(0.0, 1.0)) + 0.1 * if down > 0.0 { ((q.y - base.y) / len).clamp(0.0, 1.0) } else { 0.0 }), M::Concrete, z);
    // Wet highlight down the lit flank.
    let hl: Vec<Vec2> = left.iter().skip(1).map(|q| *q + Vec2::new(w * 0.25 * (1.0 - (q.y - base.y).abs() / len), 0.0)).chain([tip.lerp(left[n - 1], 0.4)]).collect();
    if hl.len() >= 2 {
        p.part(e, stroke(&hl, |t| 1.3 * (1.0 - t) + 0.3, 1.0, 1.0, |_| 1.0), M::Rim, z + 0.005);
    }
}

fn stalactites(p: &mut Props, e: Entity) {
    let n = 2 + p.rng.below(4);
    for i in 0..n {
        let x = p.rng.range(-16.0, 16.0);
        let big = i == 0;
        let len = if big { p.rng.range(28.0, 60.0) } else { p.rng.range(8.0, 26.0) };
        let w = len * p.rng.range(0.13, 0.2) + 1.0;
        dripstone(p, e, Vec2::new(x, 2.0), w, len, -1.0, 0.01 * i as f32);
        if p.rng.chance(0.45) {
            // A drop of water about to fall, another on its way down.
            let tip = Vec2::new(x, 2.0 - len);
            water_drop(p, e, tip - Vec2::Y * 1.2, 0.9, 0.05);
            if p.rng.chance(0.5) {
                let fall = p.rng.range(10.0, 40.0);
                water_drop(p, e, tip - Vec2::Y * fall, 0.7, 0.05);
            }
        }
    }
    // Soda straws: thin hollow tubes between them.
    for _ in 0..p.rng.below(4) {
        let x = p.rng.range(-18.0, 18.0);
        let len = p.rng.range(5.0, 14.0);
        p.part(e, stroke(&[Vec2::new(x, 2.0), Vec2::new(x, 2.0 - len)], |_| 1.0, 1.0, 0.6, |_| 1.0), M::Rim, 0.04);
    }
}

fn water_drop(p: &mut Props, e: Entity, c: Vec2, r: f32, z: f32) {
    let pts = [c + Vec2::new(0.0, r * 1.8), c + Vec2::new(-r, -r * 0.2), c + Vec2::new(0.0, -r), c + Vec2::new(r, -r * 0.2)];
    p.part(e, fill(&smooth_closed(&pts, 3), |_| 1.0), M::Rim, z);
}

fn stalagmites(p: &mut Props, at: Vec2) {
    let a1 = side(p);
    let e = p.root(at - Vec2::Y * S * 2.0, a1, 0.0);
    let n = 1 + p.rng.below(4);
    for i in 0..n {
        let x = if i == 0 { 0.0 } else { p.rng.range(-13.0, 13.0) };
        let len = if i == 0 { p.rng.range(18.0, 38.0) } else { p.rng.range(6.0, 16.0) };
        let w = len * p.rng.range(0.18, 0.26) + 2.0;
        dripstone(p, e, Vec2::new(x, 0.0), w, len, 1.0, 0.01 * (n - i) as f32);
    }
    // A splash cup of calcite around the base.
    let base: Vec<Vec2> = ellipse_pts(p.rng.range(9.0, 14.0), 2.2, 16).into_iter().map(|q| q + Vec2::Y * 1.2).collect();
    p.shape(e, &base, 0.5, M::Concrete, 0.05);
}

/// Stalactite and stalagmite grown into one pillar, `h` world units tall.
fn column(p: &mut Props, at: Vec2, h: f32) {
    let a1 = side(p);
    let e = p.root(at - Vec2::Y * S * 2.0, a1, 0.0);
    let h = h / S + 4.0;
    let waist = p.rng.range(0.35, 0.65);
    let (wb, wt) = (p.rng.range(8.0, 12.0), p.rng.range(7.0, 11.0));
    let neck = p.rng.range(1.5, 3.5);
    let n = 10;
    let width = |t: f32| {
        // Flared at both ends, pinched where the two halves met.
        let d = (t - waist).abs() / waist.max(1.0 - waist);
        let flare = if t < waist { wb } else { wt };
        neck + (flare - neck) * d.powf(1.6)
    };
    let mut left = Vec::new();
    let mut right = Vec::new();
    for i in 0..=n {
        let t = i as f32 / n as f32;
        let wob = (t * 17.0).sin() * 0.8;
        left.push(Vec2::new(-width(t) + wob, t * h));
        right.push(Vec2::new(width(t) + wob * 0.5, t * h));
    }
    // Two halves, so each is star-shaped for the fan fill.
    let k = (waist * n as f32).round() as usize;
    for (a, b, z) in [(0, k, 0.0), (k, n, 0.01)] {
        let mut o: Vec<Vec2> = left[a..=b].to_vec();
        o.extend(right[a..=b].iter().rev());
        p.part(e, fill(&o, |q| 0.4 + 0.5 * (1.0 - ((q.x + wb) / (2.0 * wb)).clamp(0.0, 1.0))), M::Concrete, z);
    }
    let hl: Vec<Vec2> = left.iter().map(|q| *q + Vec2::X * 1.5).collect();
    p.part(e, stroke(&hl, |_| 1.0, 1.0, 1.0, |t| 0.6 + 0.4 * (t * 23.0).sin().abs()), M::Rim, 0.02);
    // Flowstone ribs down the body.
    for _ in 0..2 {
        let x = p.rng.range(-2.0, 3.0);
        let pts: Vec<Vec2> = (0..=n).map(|i| {
            let t = i as f32 / n as f32;
            Vec2::new(x * width(t) / wb, t * h)
        }).collect();
        p.part(e, stroke(&pts, |_| 0.8, 0.6, 0.6, |_| 1.0), M::Soot, 0.03);
    }
}

/// A rock chunk with a crack and a lit top.
fn rock(p: &mut Props, e: Entity, c: Vec2, r: f32, z: f32) {
    let pts: Vec<Vec2> = rock_pts(&mut p.rng, r, 8).into_iter().map(|q| q + c).collect();
    p.shape(e, &pts, 0.35, M::Concrete, z);
    let top: Vec<Vec2> = pts.iter().copied().filter(|q| q.y > c.y + r * 0.2).collect();
    if top.len() >= 2 {
        let mut top = top;
        top.sort_by(|a, b| a.x.total_cmp(&b.x));
        let top: Vec<Vec2> = top.into_iter().map(|q| q - Vec2::Y * 0.8).collect();
        p.part(e, stroke(&top, |_| 1.2, 1.0, 0.7, |_| 1.0), M::Rim, z + 0.003);
    }
}

fn boulder(p: &mut Props, at: Vec2) {
    let a1 = side(p);
    let e = p.root(at - Vec2::Y * S * 4.0, a1, 0.0);
    let r = p.rng.range(13.0, 20.0);
    let c = Vec2::new(0.0, r * 0.6);
    let pts: Vec<Vec2> = rock_pts(&mut p.rng, r, 10).into_iter().map(|q| q * Vec2::new(1.15, 1.0) + c).collect();
    p.shape(e, &pts, 0.3, M::Concrete, 0.0);
    // Bedding lines across it (it fell out of the same beds).
    let angle = p.rng.range(-0.4, 0.4);
    for k in 0..3 {
        let y = c.y + (k as f32 - 1.0) * r * 0.35;
        let d = Vec2::from_angle(angle) * r * 0.75;
        let mid = Vec2::new(0.0, y);
        p.part(e, stroke(&[mid - d, mid + d], |t| 1.0 * (1.0 - (t - 0.5).abs() * 1.6), 0.7, 0.7, |_| 1.0), M::Soot, 0.01);
    }
    // A fresh crack and the lit crown.
    let x0 = p.rng.range(-r * 0.4, r * 0.4);
    let crack: Vec<Vec2> = (0..4).map(|i| Vec2::new(x0 + p.rng.range(-3.0, 3.0), c.y + r * 0.8 - i as f32 * r * 0.4)).collect();
    p.part(e, stroke(&crack, |t| 1.6 * (1.0 - t) + 0.3, 1.0, 1.0, |_| 1.0), M::Soot, 0.02);
    let crown: Vec<Vec2> = (0..5).map(|i| c + Vec2::from_angle(0.6 + i as f32 * 0.45) * r * 0.78).collect();
    p.part(e, stroke(&smooth_open(&crown, 4), |t| 2.4 * (1.0 - (t - 0.5).abs() * 1.8), 1.0, 0.8, |_| 1.0), M::Rim, 0.015);
    // Scree heaped against it.
    for i in 0..3 + p.rng.below(3) {
        let r2 = p.rng.range(2.0, 4.5);
        let x = p.rng.range(r * 0.6, r * 1.3) * if i % 2 == 0 { 1.0 } else { -1.0 };
        rock(p, e, Vec2::new(x, r2 * 0.4), r2, 0.03 + 0.005 * i as f32);
    }
    if p.rng.chance(0.4) {
        let flip = side(p);
        let a1 = p.rng.below(2);
        mushroom_cluster(p, e, Vec2::new(flip * r * 0.9, 0.0), 2 + a1, 0.04);
    }
}

fn scree(p: &mut Props, at: Vec2) {
    let e = p.root(at - Vec2::Y * S * 2.0, 1.0, 0.0);
    for i in 0..4 + p.rng.below(4) {
        let r = p.rng.range(2.0, 5.5);
        let c = Vec2::new(p.rng.range(-13.0, 13.0), r * 0.35);
        rock(p, e, c, r, 0.01 * i as f32);
    }
}

/// A clump of pale cave fungus: thin stems, drooping caps.
fn mushroom_cluster(p: &mut Props, e: Entity, at: Vec2, n: usize, z: f32) {
    for i in 0..n {
        let x = at.x + p.rng.range(-7.0, 7.0);
        let h = p.rng.range(4.0, 14.0);
        let lean = p.rng.range(-3.0, 3.0);
        let top = Vec2::new(x + lean, at.y + h);
        let stem = smooth_open(&[Vec2::new(x, at.y - 2.0), Vec2::new(x + lean * 0.3, at.y + h * 0.5), top], 4);
        let zz = z + 0.004 * i as f32;
        p.part(e, stroke(&stem, |t| 1.6 - 0.6 * t, 1.0, 0.6, |_| 1.0), M::Enamel, zz);
        let r = p.rng.range(2.5, 5.0) * (0.6 + h / 20.0);
        let droop = p.rng.range(0.4, 0.9);
        let cap = [
            top + Vec2::new(-r, -r * droop),
            top + Vec2::new(-r * 0.75, r * 0.15),
            top + Vec2::new(-r * 0.2, r * 0.5),
            top + Vec2::new(r * 0.4, r * 0.45),
            top + Vec2::new(r * 0.85, r * 0.05),
            top + Vec2::new(r, -r * droop),
            top + Vec2::new(0.0, -r * 0.15),
        ];
        let cap = smooth_closed(&cap, 3);
        let y0 = top.y - r;
        p.part(e, fill(&cap, |q| 0.55 + 0.45 * ((q.y - y0) / (1.5 * r)).clamp(0.0, 1.0)), M::Enamel, zz + 0.002);
        // Dark gills under the cap.
        p.part(e, stroke(&[top + Vec2::new(-r * 0.8, -r * droop * 0.8), top + Vec2::new(0.0, -r * 0.1), top + Vec2::new(r * 0.8, -r * droop * 0.8)], |_| 0.9, 0.5, 0.5, |_| 1.0), M::Soot, zz + 0.003);
    }
}

fn mushrooms(p: &mut Props, at: Vec2) {
    let a1 = side(p);
    let e = p.root(at - Vec2::Y * S, a1, 0.0);
    let n = 3 + p.rng.below(4);
    mushroom_cluster(p, e, Vec2::ZERO, n, 0.0);
    // Rotting litter they grow from: a twig, a pebble.
    if p.rng.chance(0.5) {
        let a1 = p.rng.range(-8.0, 8.0);
        let a2 = p.rng.range(2.5, 4.0);
        rock(p, e, Vec2::new(a1, 1.0), a2, -0.01);
    }
}

/// A fan of crystal prisms out of `base`, pointing along `angle` + π/2 (up).
fn crystals(p: &mut Props, e: Entity, base: Vec2, angle: f32, n: usize, z: f32) {
    for i in 0..n {
        let a = angle + p.rng.range(-0.75, 0.75);
        let len = if i == 0 { p.rng.range(12.0, 20.0) } else { p.rng.range(5.0, 13.0) };
        let w = len * p.rng.range(0.16, 0.24);
        let dir = Vec2::from_angle(a + std::f32::consts::FRAC_PI_2);
        let across = dir.perp();
        let b = base + across * p.rng.range(-3.0, 3.0) - dir * 2.0;
        let shoulder = b + dir * len * 0.78;
        let tip = b + dir * len;
        let zz = z + 0.004 * i as f32;
        // Two facets: one catching the light, one in shadow.
        let lit = [b - across * w, shoulder - across * w, tip, shoulder, b];
        let dark = [b, shoulder, tip, shoulder + across * w, b + across * w];
        p.part(e, fill(&lit, |_| 1.0), M::Rim, zz);
        p.part(e, fill(&dark, |_| 0.75), M::Steel, zz + 0.001);
        p.part(e, stroke(&[b + dir * 1.0, shoulder], |_| 0.5, 1.0, 1.0, |_| 1.0), M::Enamel, zz + 0.002);
    }
    let socket: Vec<Vec2> = ellipse_pts(5.0, 2.5, 10).into_iter().map(|q| q + base).collect();
    p.part(e, fill(&socket, |_| 0.8), M::Soot, z + 0.03);
}

fn crystal_patch(p: &mut Props, at: Vec2) {
    let a1 = side(p);
    let e = p.root(at - Vec2::Y * S * 1.5, a1, 0.0);
    let a1 = p.rng.range(5.0, 7.0);
    rock(p, e, Vec2::new(-4.0, 2.0), a1, 0.0);
    let n = 3 + p.rng.below(4);
    let a1 = p.rng.range(-0.3, 0.3);
    crystals(p, e, Vec2::new(2.0, 2.0), a1, n, 0.01);
}

/// A crack in a wall's face (over the terrain) with crystals growing out of it.
fn crack_with_crystals(p: &mut Props, e: Entity, half: Vec2) {
    let mut slit = Vec::new();
    for i in 0..=6 {
        let t = i as f32 / 6.0;
        slit.push(Vec2::new(half.x * (1.0 - (2.0 * t - 1.0).abs()) * p.rng.range(0.6, 1.0) + p.rng.range(-1.0, 1.0), half.y - 2.0 * half.y * t));
    }
    for i in (1..6).rev() {
        let t = i as f32 / 6.0;
        slit.push(Vec2::new(-half.x * 0.6 * (1.0 - (2.0 * t - 1.0).abs()), half.y - 2.0 * half.y * t));
    }
    p.part(e, fill(&slit, |_| 0.8), M::Soot, OVER);
    // Crystals poking out toward the open side.
    for k in 0..2 + p.rng.below(3) {
        let y = p.rng.range(-half.y * 0.6, half.y * 0.6);
        let n = 1 + p.rng.below(3);
        let a = -std::f32::consts::FRAC_PI_2 + p.rng.range(-0.3, 0.6);
        crystals(p, e, Vec2::new(0.0, y), a, n, OVER + 0.01 + 0.01 * k as f32);
    }
}

/// Bracket fungus stacked up a wall (local +X is the open side).
fn brackets(p: &mut Props, e: Entity) {
    let n = 2 + p.rng.below(3);
    let mut y = p.rng.range(-12.0, 0.0);
    for i in 0..n {
        let r = p.rng.range(4.0, 8.0);
        let shelf = [
            Vec2::new(-3.0, y - r * 0.3),
            Vec2::new(-3.0, y + r * 0.35),
            Vec2::new(r * 0.5, y + r * 0.45),
            Vec2::new(r, y + r * 0.1),
            Vec2::new(r * 0.85, y - r * 0.2),
            Vec2::new(r * 0.3, y - r * 0.32),
        ];
        let shelf = smooth_closed(&shelf, 3);
        p.part(e, fill(&shelf, |q| 0.5 + 0.5 * ((q.y - y + r * 0.3) / (r * 0.75)).clamp(0.0, 1.0)), M::Enamel, 0.01 * i as f32);
        // Growth rings.
        p.part(e, stroke(&[Vec2::new(-2.0, y + r * 0.1), Vec2::new(r * 0.5, y + r * 0.18), Vec2::new(r * 0.8, y)], |_| 0.6, 0.6, 0.6, |_| 1.0), M::Wood, 0.01 * i as f32 + 0.002);
        y += p.rng.range(6.0, 11.0);
    }
}

/// Pale flowstone running down a wall's face (over the terrain).
fn flowstone_streak(p: &mut Props, e: Entity, half: Vec2) {
    for k in 0..3 {
        let x = -half.x + half.x * k as f32 + p.rng.range(-1.0, 1.0);
        let len = half.y * 2.0 * p.rng.range(0.5, 1.0);
        let wob = p.rng.range(-1.5, 1.5);
        let pts = smooth_open(&[Vec2::new(x, half.y), Vec2::new(x + wob, half.y - len * 0.5), Vec2::new(x - wob * 0.5, half.y - len)], 6);
        let w = p.rng.range(2.0, 4.0);
        p.part(e, stroke(&pts, |t| w * (1.0 - t * 0.7), 1.0, 0.6, |t| 0.75 + 0.25 * (t * 19.0).sin().abs()), M::Rim, OVER + 0.01 * k as f32);
    }
}

/// A flowstone curtain draped over a drop (local +X is the open side).
fn curtain(p: &mut Props, e: Entity) {
    let len = p.rng.range(14.0, 34.0);
    let width = p.rng.range(5.0, 9.0);
    let folds = 3 + p.rng.below(3);
    // Draped from the lip, its bottom hem scalloped.
    let mut outline = vec![Vec2::new(-4.0, 2.0), Vec2::new(width * 0.6, 1.5)];
    outline.push(Vec2::new(width, -len * 0.3));
    for i in 0..=folds * 2 {
        let t = i as f32 / (folds * 2) as f32;
        let x = width - t * (width + 3.0);
        let y = -len * (0.75 + 0.25 * (1.0 - (t * 2.0 - 1.0).abs())) + if i % 2 == 0 { -3.0 } else { 2.0 };
        outline.push(Vec2::new(x, y));
    }
    outline.push(Vec2::new(-3.0, -len * 0.4));
    let outline = smooth_closed(&outline, 2);
    p.part(e, fill(&outline, |q| 0.5 + 0.5 * ((q.y + len) / (len + 2.0)).clamp(0.0, 1.0)), M::Concrete, 0.0);
    // Folds: alternating light and dark streaks.
    for i in 0..folds {
        let t = (i as f32 + 0.5) / folds as f32;
        let x = width - t * (width + 3.0);
        let bottom = -len * (0.7 + 0.25 * (1.0 - (t * 2.0 - 1.0).abs()));
        let m = if i % 2 == 0 { M::Rim } else { M::Soot };
        p.part(e, stroke(&smooth_open(&[Vec2::new(x * 0.7, 0.0), Vec2::new(x, bottom * 0.5), Vec2::new(x, bottom)], 4), |t| 1.4 * (1.0 - t) + 0.4, 1.0, 0.7, |_| 1.0), m, 0.01);
    }
    // A drip hanging off the lowest point.
    if p.rng.chance(0.6) {
        let x = width * 0.2;
        let a1 = p.rng.range(4.0, 16.0);
        water_drop(p, e, Vec2::new(x, -len - a1), 0.8, 0.02);
    }
}

/// A still pool of dark water under the drip, its rim crusted with calcite.
fn pool(p: &mut Props, at: Vec2) {
    let e = p.root(at, 1.0, 0.0);
    let rx = p.rng.range(10.0, 15.0);
    let pts: Vec<Vec2> = ellipse_pts(rx, 2.6, 18).into_iter().map(|q| q + Vec2::Y * 0.3).collect();
    p.part(e, fill(&pts, |q| 0.6 + 0.4 * (q.y.max(0.0) / 2.6)), M::Iron, OVER);
    // Rimstone dam around it.
    let dam: Vec<Vec2> = (0..=16).map(|i| {
        let a = std::f32::consts::PI * (1.0 + i as f32 / 16.0);
        Vec2::new(a.cos() * (rx + 1.0), 0.3 + a.sin() * 3.0)
    }).collect();
    p.part(e, stroke(&dam, |_| 1.4, 1.0, 0.6, |_| 1.0), M::Rim, OVER + 0.01);
    // Ripples from the last drop.
    let c = Vec2::new(p.rng.range(-rx * 0.4, rx * 0.4), 0.5);
    for r in [2.0, 4.5] {
        let ring: Vec<Vec2> = (0..=16).map(|i| c + Vec2::new((i as f32 / 16.0 * std::f32::consts::TAU).cos() * r, (i as f32 / 16.0 * std::f32::consts::TAU).sin() * r * 0.25)).collect();
        p.part(e, stroke(&ring, |_| 0.5, 1.0, 1.0, |_| 0.7), M::Rim, OVER + 0.02);
    }
    if p.rng.chance(0.5) {
        let x = c.x + p.rng.range(-3.0, 3.0);
        let a1 = p.rng.range(14.0, 40.0);
        water_drop(p, e, Vec2::new(x, a1), 0.8, OVER + 0.02);
    }
}

/// A dropped helmet, and sometimes what's left of its owner.
fn remains(p: &mut Props, at: Vec2) {
    let flip = side(p);
    let e = p.root(at - Vec2::Y * S * 1.5, flip, 0.0);
    // The helmet, tipped over on its side.
    let tilt = p.rng.range(-0.5, 0.5);
    let hc = Vec2::new(p.rng.range(-4.0, 4.0), 1.0);
    let dome: Vec<Vec2> = (0..=12).map(|i| {
        let a = std::f32::consts::PI * i as f32 / 12.0;
        hc + Vec2::from_angle(tilt) * (a.cos() * 8.0) + Vec2::from_angle(tilt).perp() * (a.sin() * 6.5)
    }).collect();
    p.shape(e, &dome, 0.35, M::Iron, 0.02);
    let brim = [hc - Vec2::from_angle(tilt) * 10.0, hc + Vec2::from_angle(tilt) * 10.0];
    p.part(e, stroke(&brim, |_| 1.8, 1.0, 0.5, |_| 1.0), M::Iron, 0.025);
    let shine = [hc + Vec2::from_angle(tilt + 2.2) * 5.0, hc + Vec2::from_angle(tilt + 1.6) * 5.5];
    p.part(e, stroke(&shine, |_| 0.9, 1.0, 1.0, |_| 1.0), M::Steel, 0.03);
    // A bullet hole.
    let hole: Vec<Vec2> = ellipse_pts(1.0, 1.0, 8).into_iter().map(|q| q + hc + Vec2::from_angle(tilt + 1.2) * 3.5).collect();
    p.part(e, fill(&hole, |_| 1.0), M::Soot, 0.035);
    if p.rng.chance(0.6) {
        // Skull and a few long bones, picked clean.
        let sc = Vec2::new(-14.0, 4.0);
        let skull = smooth_closed(&[sc + Vec2::new(-4.5, -1.5), sc + Vec2::new(-4.0, 3.0), sc + Vec2::new(0.0, 5.0), sc + Vec2::new(4.0, 3.0), sc + Vec2::new(4.5, -1.0), sc + Vec2::new(2.5, -4.0), sc + Vec2::new(-2.0, -4.0)], 3);
        p.shape(e, &skull, 0.55, M::Enamel, 0.01);
        for dx in [-1.8, 1.4] {
            let eye: Vec<Vec2> = ellipse_pts(1.1, 1.3, 8).into_iter().map(|q| q + sc + Vec2::new(dx, 0.3)).collect();
            p.part(e, fill(&eye, |_| 1.0), M::Soot, 0.015);
        }
        p.part(e, stroke(&[sc + Vec2::new(-1.5, -3.0), sc + Vec2::new(1.5, -3.0)], |_| 0.6, 1.0, 1.0, |_| 1.0), M::Soot, 0.015);
        for _ in 0..2 + p.rng.below(2) {
            let c = Vec2::new(p.rng.range(4.0, 18.0), p.rng.range(0.5, 2.0));
            let d = Vec2::from_angle(p.rng.range(-0.3, 0.3)) * p.rng.range(5.0, 8.0);
            p.part(e, stroke(&[c - d, c + d], |t| 1.4 + 1.2 * (2.0 * t - 1.0).abs().powf(4.0), 1.0, 0.6, |_| 1.0), M::Enamel, 0.005);
        }
    }
}

/// A rusty pipe from the city above, broken off and poking up out of the floor.
fn pipe_up(p: &mut Props, at: Vec2) {
    let lean = p.rng.range(-0.35, 0.35);
    let a1 = side(p);
    let e = p.root(at, a1, lean);
    let (h, r) = (p.rng.range(14.0, 26.0), p.rng.range(4.0, 5.5));
    p.part(e, stroke(&[Vec2::new(0.0, -8.0), Vec2::new(0.0, h)], |_| 2.0 * r, 1.0, 0.4, |_| 1.0), M::Rust, 0.0);
    p.part(e, stroke(&[Vec2::new(-r - 1.5, h * 0.4), Vec2::new(r + 1.5, h * 0.4)], |_| 2.5, 0.9, 0.6, |_| 1.0), M::Iron, 0.01);
    // Jagged broken mouth.
    let mouth = [Vec2::new(-r, h), Vec2::new(-r * 0.4, h + 3.0), Vec2::new(0.2 * r, h + 1.0), Vec2::new(r * 0.6, h + 4.0), Vec2::new(r, h)];
    p.part(e, fill(&mouth, |_| 0.8), M::Rust, 0.01);
    let lip: Vec<Vec2> = ellipse_pts(r * 0.8, 1.2, 10).into_iter().map(|q| q + Vec2::Y * h).collect();
    p.part(e, fill(&lip, |_| 1.0), M::Soot, 0.02);
    // Calcite already crusting its foot.
    let crust: Vec<Vec2> = ellipse_pts(r + 4.0, 2.5, 12).into_iter().map(|q| q + Vec2::Y * 0.5).collect();
    p.shape(e, &crust, 0.5, M::Concrete, 0.03);
}
