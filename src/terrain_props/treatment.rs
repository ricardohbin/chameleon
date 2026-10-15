//! Props of "region: water treatment facility" (`Style::Treatment`): pipe
//! risers with handwheel valves and pressure gauges running up to the ceiling,
//! chlorine tanks on their cradles, catwalk railings along the tank edges,
//! filtration grates, chemical drums, warning signs and sludge.
//!
//! The halls are mostly passable structure (Back cells), where the generic
//! site finder only reports ground (walls/ceilings need open air), so the
//! ground props also reach up the hall: risers climb to the ceiling, railings
//! line the drops they find themselves.

use super::*;

pub(super) const HOOKS: Hooks = Hooks { spacing, ground, edge, wall, ceiling };

/// (gap between props of the same kind, chance a free spot gets one), in prop units.
fn spacing(kind: Kind) -> (f32, f32) {
    match kind {
        Kind::Ground => (48.0, 0.85),
        Kind::Edge => (35.0, 0.85),
        Kind::Wall => (50.0, 0.75),
        Kind::Ceiling => (45.0, 0.6),
    }
}

#[derive(Clone, Copy)]
enum Prop {
    /// Catwalk railing along a drop (`dir` = the open side).
    Railing(i32),
    Riser,
    Tank,
    Grate,
    Drums,
    Sign,
    Sludge,
}

impl Prop {
    /// (half width, height) in prop units.
    fn size(self) -> (f32, f32) {
        match self {
            Prop::Railing(_) => (24.0, 26.0),
            Prop::Riser => (8.0, 50.0),
            Prop::Tank => (32.0, 34.0),
            Prop::Grate => (16.0, 34.0),
            Prop::Drums => (18.0, 22.0),
            Prop::Sign => (8.0, 38.0),
            Prop::Sludge => (16.0, 6.0),
        }
    }
}

/// A top surface with headroom at `cell` (`x` = its world x). True if something was placed.
fn ground(p: &mut Props, cell: IVec2, x: f32) -> bool {
    let mut picks = Vec::new();
    // Edge sites put railing stubs right at the brink; a full railing back
    // along the floor now and then.
    if let Some(dir) = ledge(p, cell)
        && p.rng.chance(0.3)
    {
        picks.push(Prop::Railing(dir));
    }
    let table: &[(Prop, f32)] = &[(Prop::Riser, 2.2), (Prop::Tank, 0.8), (Prop::Grate, 1.4), (Prop::Drums, 1.6), (Prop::Sign, 1.0), (Prop::Sludge, 1.2)];
    let total: f32 = table.iter().map(|e| e.1).sum();
    let mut r = p.rng.range(0.0, total);
    let pick = table.iter().find(|e| {
        r -= e.1;
        r <= 0.0
    });
    picks.push(pick.unwrap_or(&table[0]).0);
    picks.push(Prop::Sludge);

    for g in picks {
        let (half, head) = g.size();
        // The railing stands back from the drop, along the floor.
        let (fit_cell, px) = match g {
            Prop::Railing(dir) => {
                let edge = ledge_x(p, cell, dir);
                let inward = edge - dir as f32 * (half * S + PX * 0.5);
                (p.t.to_cell(Vec2::new(inward, p.t.cell_min(cell).y + PX * 0.5)), inward)
            }
            _ => (cell, x),
        };
        let tilt = matches!(g, Prop::Sludge | Prop::Drums);
        let Some((y, angle, indoor)) = p.fit(fit_cell, half, head, tilt) else { continue };
        let at = Vec2::new(px, y);
        p.tilt = angle;
        p.depth = if indoor { INDOOR } else { BACK };
        let placed = match g {
            Prop::Railing(dir) => {
                railing(p, at, dir as f32, half);
                true
            }
            Prop::Riser => riser(p, at),
            Prop::Tank => {
                chlorine_tank(p, at);
                true
            }
            Prop::Grate => {
                leaning_grate(p, at);
                true
            }
            Prop::Drums => {
                drums(p, at);
                true
            }
            Prop::Sign => {
                sign_post(p, at);
                true
            }
            Prop::Sludge => {
                sludge(p, at);
                true
            }
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
    if p.rng.chance(0.55) {
        // Railing posts at the brink, one rail snapped and hanging over.
        railing_stub(p, e);
    } else {
        // An outfall pipe jutting over the drop, sludge pouring out.
        let r = p.rng.range(5.0, 7.5);
        let y = -r - p.rng.range(3.0, 10.0);
        outfall(p, e, at, flip, Vec2::new(-4.0, y), r);
    }
    true
}

/// A straight vertical face (`flip` = the open side).
fn wall(p: &mut Props, at: Vec2, flip: f32) -> bool {
    let half = Vec2::new(13.0, 13.0);
    let c = at + Vec2::new(-flip * (half.x * S + PX * 2.0), 0.0);
    if p.rng.chance(0.45) && p.inside(c, half * S) {
        let e = p.root(c, flip, 0.0);
        wall_grate(p, e, half);
    } else {
        let e = p.root(at, flip, 0.0);
        let r = p.rng.range(5.0, 8.0);
        let y = p.rng.range(-6.0, 8.0);
        outfall(p, e, at, flip, Vec2::new(-4.0, y), r);
    }
    true
}

/// The underside of a thick overhang.
fn ceiling(p: &mut Props, at: Vec2) -> bool {
    let e = p.root(at, 1.0, 0.0);
    // A pipe run on hangers, and limescale stalactites.
    let drop = p.rng.range(6.0, 12.0);
    let r = p.rng.range(3.5, 5.0);
    for x in [-16.0, 16.0] {
        p.bar(e, &[Vec2::new(x, 2.0), Vec2::new(x, -drop)], 1.2, M::Iron, 0.0);
    }
    let y = -drop - r;
    let hotfix_roll_1 = p.rng.range(-1.5, 1.5); // HOTFIX: evaluate before borrowing Props mutably (E0499).
    p.part(e, stroke(&[Vec2::new(-26.0, y), Vec2::new(26.0, y + hotfix_roll_1)], |_| 2.0 * r, 1.0, 0.4, |_| 1.0), M::Rust, 0.01);
    let hotfix_roll_1 = p.rng.range(-8.0, 8.0); // HOTFIX: evaluate before borrowing Props mutably (E0499).
    flange(p, e, Vec2::new(hotfix_roll_1, y), r, true, 0.02);
    for _ in 0..1 + p.rng.below(3) {
        let x = p.rng.range(-14.0, 14.0);
        let len = p.rng.range(4.0, 12.0);
        stalactite(p, e, Vec2::new(x, 1.0), len, OVER);
    }
    true
}

// --- Placing helpers ---

/// The open side if `cell`'s floor ends at a drop within a few cells.
fn ledge(p: &Props, cell: IVec2) -> Option<i32> {
    let t = p.t;
    for dir in [-1, 1] {
        for k in 1..=cells(40.0) {
            let c = cell + IVec2::new(dir * k, 0);
            if t.solid(c - IVec2::Y) {
                break; // A wall or a step up, not a drop.
            }
            if !t.solid(c) {
                if (0..=cells(30.0)).all(|d| !t.solid(c + IVec2::Y * d)) {
                    return Some(dir);
                }
                break;
            }
        }
    }
    None
}

/// World x of the brink of the drop found by `ledge`.
fn ledge_x(p: &Props, cell: IVec2, dir: i32) -> f32 {
    let t = p.t;
    let mut c = cell;
    while t.solid(c + IVec2::new(dir, 0)) {
        c.x += dir;
    }
    t.cell_min(c).x + if dir > 0 { PX } else { 0.0 }
}

/// Clear height (world units) over the floor at `x`, up to the ceiling, if there is one in reach.
fn headroom(p: &Props, x: f32, floor: f32) -> Option<f32> {
    let t = p.t;
    let c = t.to_cell(Vec2::new(x, floor + PX * 0.5));
    (0..cells(640.0)).map(|k| c - IVec2::Y * k).find(|&c| t.solid(c)).map(|c| t.cell_min(c).y - floor)
}

// --- Drawing ---

/// A flange ring around a pipe of radius `r` (`across` = on a horizontal pipe).
fn flange(p: &mut Props, e: Entity, c: Vec2, r: f32, across: bool, z: f32) {
    let (a, b) = if across { (c - Vec2::Y * (r + 1.5), c + Vec2::Y * (r + 1.5)) } else { (c - Vec2::X * (r + 1.5), c + Vec2::X * (r + 1.5)) };
    p.part(e, stroke(&[a, b], |_| 3.0, 1.0, 0.55, |_| 1.0), M::Iron, z);
    // Bolt heads.
    let n = (a - b).normalize();
    let mut tris = Vec::new();
    for t in [0.15, 0.85] {
        let q = a.lerp(b, t);
        tris.push([q + n.perp() * 0.9, q - n.perp() * 0.9, q + n * 0.9]);
    }
    p.part(e, triangles(&tris), M::Steel, z + 0.005);
}

/// A handwheel valve (wheel seen face on).
fn handwheel(p: &mut Props, e: Entity, c: Vec2, r: f32, z: f32) {
    p.ring(e, c, r, 2.4, M::Steel, z, |t| if (t * 16.0) as i32 % 2 == 0 { 1.0 } else { 0.75 });
    let spin = p.rng.range(0.0, 1.0);
    for i in 0..3 {
        let d = Vec2::from_angle(i as f32 * std::f32::consts::FRAC_PI_3 + spin) * r;
        p.part(e, stroke(&[c - d, c + d], |_| 1.1, 1.0, 0.8, |_| 1.0), M::Steel, z);
    }
    let hub: Vec<Vec2> = ellipse_pts(1.8, 1.8, 10).into_iter().map(|q| q + c).collect();
    p.part(e, fill(&hub, |_| 0.9), M::Steel, z + 0.01);
}

/// A pressure gauge: enamel dial, bezel, needle stuck somewhere.
fn gauge(p: &mut Props, e: Entity, c: Vec2, r: f32, z: f32) {
    let face: Vec<Vec2> = ellipse_pts(r, r, 18).into_iter().map(|q| q + c).collect();
    p.part(e, fill(&face, |q| 0.75 + 0.25 * ((q.y - c.y) / r + 1.0) / 2.0), M::Enamel, z);
    p.ring(e, c, r, 1.4, M::Iron, z + 0.01, |_| 1.0);
    // Ticks along the dial's arc.
    let mut tris = Vec::new();
    for i in 0..7 {
        let a = std::f32::consts::PI * (1.25 - i as f32 * 0.25);
        let d = Vec2::from_angle(a);
        let (o, q) = (c + d * r * 0.85, c + d * r * 0.6);
        tris.push([o + d.perp() * 0.35, o - d.perp() * 0.35, q]);
    }
    p.part(e, triangles(&tris), M::Soot, z + 0.02);
    // Pinned past the end of the scale, or dead at zero.
    let a = if p.rng.chance(0.5) { p.rng.range(-0.9, -0.6) } else { p.rng.range(3.6, 3.9) };
    let tip = c + Vec2::from_angle(a) * r * 0.8;
    p.part(e, stroke(&[c, tip], |t| 1.0 - 0.7 * t, 1.0, 1.0, |_| 1.0), M::Soot, z + 0.03);
    // Cracked glass.
    if p.rng.chance(0.5) {
        let crack = [c + Vec2::new(-r * 0.6, r * 0.5), c + Vec2::new(-r * 0.1, r * 0.1), c + Vec2::new(r * 0.2, -r * 0.6)];
        p.part(e, stroke(&crack, |_| 0.4, 1.0, 1.0, |_| 1.0), M::Rim, z + 0.035);
    }
}

/// A limescale stalactite hanging from `at`.
fn stalactite(p: &mut Props, e: Entity, at: Vec2, len: f32, z: f32) {
    let w = p.rng.range(1.4, 2.6);
    let pts = [at + Vec2::new(-w, 0.5), at + Vec2::new(-w * 0.4, -len * 0.6), at + Vec2::new(0.0, -len), at + Vec2::new(w * 0.5, -len * 0.5), at + Vec2::new(w, 0.5)];
    p.part(e, fill(&smooth_closed(&pts, 3), |q| 0.65 + 0.35 * ((q.y - at.y) / len + 1.0).clamp(0.0, 1.0)), M::Enamel, z);
}

/// A pipe riser from the floor to the ceiling: flanges, a handwheel valve,
/// a gauge on a branch, and limescale weeping from a leaking joint.
fn riser(p: &mut Props, at: Vec2) -> bool {
    let Some(room) = headroom(p, at.x, at.y) else { return false };
    if room < 70.0 {
        return false;
    }
    let flip = if p.rng.chance(0.5) { 1.0 } else { -1.0 };
    let e = p.root(at - Vec2::Y * S * 2.0, flip, 0.0);
    let top = room / S + 4.0;
    let r = p.rng.range(4.0, 6.0);
    let m = if p.rng.chance(0.5) { M::Steel } else { M::Rust };
    // Foot: a concrete plinth.
    let plinth = [Vec2::new(-r - 4.0, -2.0), Vec2::new(-r - 3.0, 5.0), Vec2::new(r + 3.0, 5.0), Vec2::new(r + 4.0, -2.0)];
    p.shape(e, &plinth, 0.5, M::Concrete, 0.02);
    p.part(e, stroke(&[Vec2::new(0.0, 0.0), Vec2::new(0.0, top)], |_| 2.0 * r, 1.0, 0.4, |t| 1.0 - 0.15 * (t * 9.0).sin().abs()), m, 0.0);
    // Flanges up the run.
    let mut y = p.rng.range(8.0, 12.0);
    while y < top - 6.0 {
        flange(p, e, Vec2::new(0.0, y), r, false, 0.01);
        y += p.rng.range(22.0, 34.0);
    }
    // The valve at about chameleon height, its stem sticking out sideways.
    let vy = p.rng.range(12.0, 34.0).min(top * 0.5);
    let body = [Vec2::new(-r - 2.0, vy - 4.0), Vec2::new(-r - 2.5, vy + 4.0), Vec2::new(r + 2.5, vy + 4.0), Vec2::new(r + 2.0, vy - 4.0)];
    p.shape(e, &body, 0.45, M::Iron, 0.02);
    p.part(e, stroke(&[Vec2::new(r, vy), Vec2::new(r + 7.0, vy)], |_| 1.8, 1.0, 0.6, |_| 1.0), M::Iron, 0.025);
    let wheel_r = p.rng.range(7.0, 9.0);
    handwheel(p, e, Vec2::new(r + 7.0, vy), wheel_r, 0.03);
    // A gauge on a little branch higher up.
    if top > 45.0 && p.rng.chance(0.8) {
        let gy = vy + p.rng.range(16.0, 22.0);
        p.part(e, stroke(&[Vec2::new(-r, gy), Vec2::new(-r - 6.0, gy), Vec2::new(-r - 6.0, gy + 3.0)], |_| 1.6, 1.0, 0.6, |_| 1.0), M::Iron, 0.02);
        let gr = p.rng.range(4.0, 5.5);
        gauge(p, e, Vec2::new(-r - 6.0, gy + 3.0 + gr), gr, 0.03);
    }
    // A leaking joint: a crust of scale running down the pipe.
    if p.rng.chance(0.7) {
        let ly = p.rng.range(vy + 8.0, top.max(vy + 10.0));
        let side = if p.rng.chance(0.5) { -1.0 } else { 1.0 };
        let x = side * r * 0.5;
        let len = p.rng.range(10.0, 24.0).min(ly - 2.0);
        p.part(e, stroke(&[Vec2::new(x, ly), Vec2::new(x + side * 0.6, ly - len * 0.5), Vec2::new(x, ly - len)], |t| 3.2 * (1.0 - t) + 0.6, 1.0, 0.7, |_| 1.0), M::Enamel, 0.04);
        let hotfix_roll_1 = p.rng.range(3.0, 6.0); // HOTFIX: evaluate before borrowing Props mutably (E0499).
        stalactite(p, e, Vec2::new(x, ly - len + 1.0), hotfix_roll_1, 0.04);
    }
    true
}

/// A horizontal chlorine tank on two saddles, valves on the dome, a hazard diamond.
fn chlorine_tank(p: &mut Props, at: Vec2) {
    let flip = if p.rng.chance(0.5) { 1.0 } else { -1.0 };
    let e = p.root(at - Vec2::Y * S * 2.0, flip, 0.0);
    let (len, r) = (p.rng.range(44.0, 54.0), p.rng.range(10.0, 12.5));
    let cy = r + 7.0;
    // Saddles.
    for x in [-len * 0.3, len * 0.3] {
        let saddle = [Vec2::new(x - 5.0, -2.0), Vec2::new(x - 6.0, cy - r * 0.4), Vec2::new(x, cy - r * 0.75), Vec2::new(x + 6.0, cy - r * 0.4), Vec2::new(x + 5.0, -2.0)];
        p.shape(e, &saddle, 0.4, M::Iron, 0.0);
    }
    // The shell: a capsule, darker underneath.
    let mut shell = Vec::new();
    for i in 0..=10 {
        let a = -std::f32::consts::FRAC_PI_2 + i as f32 / 10.0 * std::f32::consts::PI;
        shell.push(Vec2::new(len / 2.0 + a.cos() * r * 0.45, cy + a.sin() * r));
    }
    for i in 0..=10 {
        let a = std::f32::consts::FRAC_PI_2 + i as f32 / 10.0 * std::f32::consts::PI;
        shell.push(Vec2::new(-len / 2.0 + a.cos() * r * 0.45, cy + a.sin() * r));
    }
    p.part(e, fill(&shell, |q| 0.35 + 0.65 * ((q.y - cy) / r * 0.5 + 0.5).clamp(0.0, 1.0).powf(0.7)), M::Enamel, 0.01);
    // Highlight along the top.
    p.part(e, stroke(&[Vec2::new(-len / 2.0 + 2.0, cy + r * 0.72), Vec2::new(len / 2.0 - 2.0, cy + r * 0.72)], |_| 1.6, 1.0, 1.0, |_| 1.0), M::Rim, 0.015);
    // Weld seams / bands.
    for x in [-len * 0.18, len * 0.18] {
        let band: Vec<Vec2> = (0..=8).map(|i| Vec2::new(x, cy - r + 2.0 * r * i as f32 / 8.0)).collect();
        p.part(e, stroke(&band, |_| 1.2, 0.6, 0.6, |_| 1.0), M::Iron, 0.02);
    }
    // Rust blooms and streaks.
    for _ in 0..3 + p.rng.below(3) {
        let c = Vec2::new(p.rng.range(-len / 2.0 + 2.0, len / 2.0 - 2.0), cy + p.rng.range(-r * 0.7, r * 0.6));
        let rr = p.rng.range(1.5, 3.5);
        let pts: Vec<Vec2> = rock_pts(&mut p.rng, rr, 8).into_iter().map(|q| q + c).collect();
        p.part(e, fill(&pts, |_| 0.8), M::Rust, 0.025);
        let l = p.rng.range(4.0, 10.0).min(c.y - (cy - r) + 0.5);
        p.part(e, stroke(&[c, c - Vec2::Y * l], |t| rr * (1.0 - t) + 0.3, 1.0, 1.0, |_| 1.0), M::Rust, 0.024);
    }
    // Hazard diamond with a skull-ish blot.
    let dc = Vec2::new(-len * 0.02, cy + 0.5);
    let d = 5.5;
    let diamond = [dc + Vec2::new(0.0, d), dc + Vec2::new(d, 0.0), dc + Vec2::new(0.0, -d), dc + Vec2::new(-d, 0.0)];
    p.part(e, fill(&diamond, |_| 0.95), M::Soot, 0.03);
    let inner = diamond.map(|q| dc + (q - dc) * 0.72);
    p.part(e, fill(&inner, |_| 1.0), M::Rim, 0.031);
    let skull: Vec<Vec2> = ellipse_pts(1.6, 1.4, 10).into_iter().map(|q| q + dc + Vec2::Y * 0.6).collect();
    p.part(e, fill(&skull, |_| 1.0), M::Soot, 0.032);
    p.part(e, stroke(&[dc - Vec2::new(1.8, 2.0), dc + Vec2::new(1.8, -0.6)], |_| 0.5, 1.0, 1.0, |_| 1.0), M::Soot, 0.032);
    p.part(e, stroke(&[dc + Vec2::new(1.8, -2.0), dc - Vec2::new(1.8, 0.6)], |_| 0.5, 1.0, 1.0, |_| 1.0), M::Soot, 0.032);
    // Valve manifold on the dome.
    let vx = len * 0.28;
    p.part(e, stroke(&[Vec2::new(vx, cy + r - 1.0), Vec2::new(vx, cy + r + 6.0)], |_| 3.0, 1.0, 0.5, |_| 1.0), M::Steel, 0.0);
    handwheel(p, e, Vec2::new(vx, cy + r + 8.5), 3.5, 0.005);
    if p.rng.chance(0.6) {
        gauge(p, e, Vec2::new(vx - 9.0, cy + r + 3.0), 3.2, 0.005);
        p.part(e, stroke(&[Vec2::new(vx - 9.0, cy + r - 1.0), Vec2::new(vx - 9.0, cy + r + 0.5)], |_| 1.4, 1.0, 0.5, |_| 1.0), M::Iron, 0.0);
    }
    // A dark leak pooled under it.
    let pool: Vec<Vec2> = ellipse_pts(p.rng.range(12.0, 18.0), 1.8, 16).into_iter().map(|q| q + Vec2::new(p.rng.range(-6.0, 6.0), 2.2)).collect();
    p.part(e, fill(&pool, |_| 0.6), M::Soot, OVER);
}

/// A section of catwalk railing standing back from a drop (`flip` = the open side):
/// posts, a hand rail and a knee rail, a kick plate, one bay bent out.
fn railing(p: &mut Props, at: Vec2, flip: f32, half: f32) {
    let e = p.root(at - Vec2::Y * S * 1.0, flip, 0.0);
    let (h, mid) = (p.rng.range(20.0, 24.0), 11.0);
    let n = 3 + p.rng.below(2);
    let span = 2.0 * half / (n - 1) as f32;
    let bent = p.rng.below(n - 1);
    let sag = p.rng.range(4.0, 9.0);
    let mut tops = Vec::new();
    for i in 0..n {
        let x = -half + span * i as f32;
        // The post on the open side of the bent bay leans out over the drop.
        let lean = if i == bent + 1 { p.rng.range(2.0, 6.0) } else { p.rng.range(-0.8, 0.8) };
        let top = Vec2::new(x + lean, h - if i == bent + 1 { lean * 0.4 } else { 0.0 });
        p.bar(e, &[Vec2::new(x, -2.0), Vec2::new(x + lean * 0.4, h * 0.5), top], 2.2, M::Steel, 0.0);
        // Base plate.
        p.part(e, stroke(&[Vec2::new(x - 2.5, 0.2), Vec2::new(x + 2.5, 0.2)], |_| 1.4, 1.0, 0.6, |_| 1.0), M::Iron, 0.005);
        tops.push(top);
    }
    for (k, w) in tops.windows(2).enumerate() {
        let (a, b) = (w[0], w[1]);
        for (dy, width) in [(0.0, 1.8), (h - mid, 1.4)] {
            let (a, b) = (a - Vec2::Y * dy, b - Vec2::Y * dy);
            let path = if k == bent {
                if dy > 0.0 && p.rng.chance(0.5) {
                    // Knee rail snapped, its end dangling.
                    smooth_open(&[a, a.lerp(b, 0.55) - Vec2::Y * 2.0, a.lerp(b, 0.6) - Vec2::new(-1.0, 9.0)], 6)
                } else {
                    smooth_open(&[a, (a + b) / 2.0 - Vec2::Y * sag, b], 8)
                }
            } else {
                vec![a, b]
            };
            p.part(e, stroke(&path, |_| width, 1.0, 0.5, |t| 1.0 - 0.25 * ((t * 7.0).sin() > 0.6) as i32 as f32), M::Steel, 0.01);
        }
    }
    // Kick plate along the floor, rust eating its bottom.
    let kick = [Vec2::new(-half, -1.0), Vec2::new(-half, 3.0), Vec2::new(half, 3.0), Vec2::new(half, -1.0)];
    p.shape(e, &kick, 0.5, M::Rust, 0.015);
    // A warning tag zip-tied to the hand rail.
    if p.rng.chance(0.5) {
        let x = p.rng.range(-half * 0.6, half * 0.6);
        let c = Vec2::new(x, h - 5.0);
        let plate = [c + Vec2::new(-4.0, 3.5), c + Vec2::new(4.0, 3.5), c + Vec2::new(4.5, -3.5), c + Vec2::new(-3.5, -3.8)];
        p.shape(e, &plate, 0.7, M::Enamel, 0.02);
        excl(p, e, c, 2.6, 0.025);
    }
}

/// Posts at a brink with a snapped rail hanging down.
fn railing_stub(p: &mut Props, e: Entity) {
    let h = p.rng.range(18.0, 22.0);
    for x in [-14.0, -2.0] {
        let hotfix_roll_1 = p.rng.range(-1.0, 1.0); // HOTFIX: evaluate before borrowing Props mutably (E0499).
        p.bar(e, &[Vec2::new(x, -2.0), Vec2::new(x + hotfix_roll_1, h)], 2.2, M::Steel, 0.0);
    }
    p.part(e, stroke(&[Vec2::new(-14.0, h), Vec2::new(-2.0, h)], |_| 1.8, 1.0, 0.5, |_| 1.0), M::Steel, 0.01);
    let len = p.rng.range(16.0, 28.0);
    let hang = smooth_open(&[Vec2::new(-2.0, h), Vec2::new(3.0, h - 2.0), Vec2::new(5.0, h - len * 0.5), Vec2::new(4.0 + p.rng.range(-3.0, 3.0), h - len)], 6);
    p.part(e, stroke(&hang, |_| 1.8, 1.0, 0.5, |_| 1.0), M::Steel, 0.01);
}

/// An outfall pipe out of the wall (local +X), sludge spilling out and down.
fn outfall(p: &mut Props, e: Entity, root: Vec2, flip: f32, at: Vec2, r: f32) {
    let len = p.rng.range(10.0, 20.0);
    let mouth = at + Vec2::new(len, p.rng.range(-2.0, 0.5));
    p.part(e, stroke(&[at - Vec2::X * 8.0, mouth], |_| 2.0 * r, 1.0, 0.4, |_| 1.0), M::Rust, 0.0);
    flange(p, e, at.lerp(mouth, 0.5), r, true, 0.01);
    let lip: Vec<Vec2> = ellipse_pts(r * 0.4, r, 14).into_iter().map(|q| q + mouth).collect();
    p.part(e, fill(&lip, |_| 1.0), M::Soot, 0.02);
    p.ring_ellipse(e, mouth, r * 0.4, r, M::Rust);
    // Sludge rope falling to whatever is below, or a hanging crust.
    let lip_low = mouth + Vec2::new(r * 0.1, -r * 0.8);
    let world = root + lip_low * Vec2::new(flip, 1.0) * S;
    match p.t.surface(world - Vec2::Y * 2.0, 400.0) {
        Some(gy) if gy < world.y - 4.0 => {
            let bottom = (gy - root.y) / S;
            let pts = [lip_low, lip_low + Vec2::new(2.0, -3.0), Vec2::new(lip_low.x + 2.5, (lip_low.y + bottom) / 2.0), Vec2::new(lip_low.x + 2.5, bottom)];
            p.part(e, stroke(&smooth_open(&pts, 6), |t| 3.0 - 1.4 * t, 0.5, 0.3, |_| 1.0), M::Soot, OVER);
            let heap: Vec<Vec2> = ellipse_pts(p.rng.range(8.0, 12.0), 2.6, 16).into_iter().map(|q| q + Vec2::new(lip_low.x + 2.5, bottom + 0.5)).collect();
            p.part(e, fill(&heap, |q| 0.4 + 0.3 * (q.y - bottom).max(0.0) / 2.6), M::Soot, OVER + 0.01);
        }
        _ => {
            let hotfix_roll_1 = p.rng.range(5.0, 9.0); // HOTFIX: evaluate before borrowing Props mutably (E0499).
            stalactite(p, e, lip_low, hotfix_roll_1, OVER)
        },
    }
    // Scale crusted down the wall under it.
    let streak = [at + Vec2::new(-3.0, -r), at + Vec2::new(-3.5, -r - p.rng.range(12.0, 26.0))];
    p.part(e, stroke(&streak, |t| 4.0 * (1.0 - t) + 0.5, 1.0, 1.0, |_| 1.0), M::Enamel, OVER - 0.01);
}

/// A louvered intake grate set in a wall face (drawn over the terrain).
fn wall_grate(p: &mut Props, e: Entity, half: Vec2) {
    let back = [Vec2::new(-half.x, -half.y), Vec2::new(-half.x, half.y), Vec2::new(half.x, half.y), Vec2::new(half.x, -half.y)];
    p.part(e, fill(&back, |q| 0.6 + 0.4 * (1.0 - (q.y + half.y) / (2.0 * half.y))), M::Soot, OVER);
    let mut frame = back.to_vec();
    frame.push(back[0]);
    p.part(e, stroke(&frame, |_| 2.6, 1.0, 0.5, |_| 1.0), M::Rust, OVER + 0.02);
    let n = 6;
    for i in 0..n {
        let y = -half.y + 2.0 * half.y * (i as f32 + 0.5) / n as f32;
        let missing = p.rng.chance(0.15);
        if missing {
            continue;
        }
        p.part(e, stroke(&[Vec2::new(-half.x + 1.0, y + 1.0), Vec2::new(half.x - 1.0, y - 0.5)], |_| 1.8, 1.0, 0.4, |_| 1.0), M::Iron, OVER + 0.01);
    }
    // Sludge pushed through the bottom slats, scale down the wall.
    let x = p.rng.range(-half.x * 0.5, half.x * 0.5);
    let len = p.rng.range(10.0, 24.0);
    p.part(e, stroke(&[Vec2::new(x, -half.y + 1.0), Vec2::new(x + 0.5, -half.y - len)], |t| 5.0 * (1.0 - t) + 1.0, 1.0, 0.8, |_| 1.0), M::Enamel, OVER + 0.03);
}

/// A filtration grate pulled out of its channel and propped against the wall,
/// clogged with sludge.
fn leaning_grate(p: &mut Props, at: Vec2) {
    let flip = if p.rng.chance(0.5) { 1.0 } else { -1.0 };
    let angle = p.rng.range(0.12, 0.3);
    let (w, h) = (p.rng.range(22.0, 28.0), p.rng.range(28.0, 34.0));
    let e = p.root(at - Vec2::Y * S * 2.0, flip, angle);
    let (l, r) = (-w / 2.0, w / 2.0);
    // Bars (dense), some bent.
    let n = 9;
    for i in 0..n {
        let x = l + w * (i as f32 + 0.5) / n as f32;
        let bend = if p.rng.chance(0.15) { p.rng.range(-3.0, 3.0) } else { 0.0 };
        p.part(e, stroke(&smooth_open(&[Vec2::new(x, 1.0), Vec2::new(x + bend, h * 0.5), Vec2::new(x, h - 1.0)], 4), |_| 1.1, 1.0, 0.5, |_| 1.0), M::Iron, 0.01);
    }
    // Frame.
    let frame = [Vec2::new(l, 0.0), Vec2::new(l, h), Vec2::new(r, h), Vec2::new(r, 0.0), Vec2::new(l, 0.0)];
    p.part(e, stroke(&frame, |_| 2.6, 1.0, 0.5, |_| 1.0), M::Rust, 0.02);
    p.part(e, stroke(&[Vec2::new(l, h * 0.5), Vec2::new(r, h * 0.5)], |_| 1.8, 1.0, 0.5, |_| 1.0), M::Rust, 0.02);
    // Clogs: rags and sludge caught in the bars, dripping.
    for _ in 0..2 + p.rng.below(3) {
        let c = Vec2::new(p.rng.range(l + 3.0, r - 3.0), p.rng.range(2.0, h * 0.7));
        let rr = p.rng.range(2.5, 4.5);
        let pts: Vec<Vec2> = rock_pts(&mut p.rng, rr, 8).into_iter().map(|q| q + c).collect();
        p.part(e, fill(&pts, |q| 0.5 + 0.5 * ((q.y - c.y) / rr * 0.5 + 0.5)), M::Soot, 0.03);
        if p.rng.chance(0.5) {
            let hotfix_roll_1 = p.rng.range(3.0, 6.0); // HOTFIX: evaluate before borrowing Props mutably (E0499).
            p.drip(e, c - Vec2::Y * rr * 0.6, 1.2, hotfix_roll_1, 0.035);
        }
    }
}

/// Chemical drums: one standing, one tipped over and spilling.
fn drums(p: &mut Props, at: Vec2) {
    let flip = if p.rng.chance(0.5) { 1.0 } else { -1.0 };
    let e = p.root(at - Vec2::Y * S * 1.5, flip, 0.0);
    let (w, h) = (p.rng.range(12.0, 14.0), p.rng.range(18.0, 21.0));
    // Standing drum.
    let x0 = -w * 0.6;
    let body = [Vec2::new(x0 - w / 2.0, 0.0), Vec2::new(x0 - w / 2.0, h), Vec2::new(x0 + w / 2.0, h), Vec2::new(x0 + w / 2.0, 0.0)];
    p.shape(e, &body, 0.45, M::Plastic, 0.0);
    for y in [h * 0.33, h * 0.66] {
        p.part(e, stroke(&[Vec2::new(x0 - w / 2.0, y), Vec2::new(x0 + w / 2.0, y)], |_| 1.3, 1.0, 0.5, |_| 1.0), M::Iron, 0.01);
    }
    let lid: Vec<Vec2> = ellipse_pts(w / 2.0, 1.6, 14).into_iter().map(|q| q + Vec2::new(x0, h)).collect();
    p.part(e, fill(&lid, |_| 0.9), M::Rim, 0.015);
    // Label with an exclamation mark.
    let lc = Vec2::new(x0, h * 0.5);
    let label = [lc + Vec2::new(-3.5, -3.0), lc + Vec2::new(-3.5, 3.0), lc + Vec2::new(3.5, 3.0), lc + Vec2::new(3.5, -3.0)];
    p.shape(e, &label, 0.8, M::Enamel, 0.02);
    excl(p, e, lc, 2.2, 0.025);
    // Tipped drum, open end toward +X, sludge spilling out.
    if p.rng.chance(0.75) {
        let c = Vec2::new(w * 0.7, w / 2.0);
        let side = [c + Vec2::new(-h / 2.0, -w / 2.0), c + Vec2::new(-h / 2.0, w / 2.0), c + Vec2::new(h / 2.0, w / 2.0), c + Vec2::new(h / 2.0, -w / 2.0)];
        p.shape(e, &side, 0.4, M::Rust, 0.0);
        for dx in [-h * 0.17, h * 0.17] {
            p.part(e, stroke(&[c + Vec2::new(dx, -w / 2.0), c + Vec2::new(dx, w / 2.0)], |_| 1.3, 1.0, 0.5, |_| 1.0), M::Iron, 0.01);
        }
        let mouth: Vec<Vec2> = ellipse_pts(1.8, w / 2.0, 14).into_iter().map(|q| q + c + Vec2::X * h / 2.0).collect();
        p.part(e, fill(&mouth, |_| 1.0), M::Soot, 0.02);
        let spill: Vec<Vec2> = ellipse_pts(p.rng.range(9.0, 14.0), 1.8, 16).into_iter().map(|q| q + c + Vec2::new(h / 2.0 + 7.0, -w / 2.0 + 1.0)).collect();
        p.part(e, fill(&spill, |q| 0.5 + 0.4 * (q.y - (c.y - w / 2.0)).max(0.0) / 2.0), M::Slime, OVER);
    }
}

/// An exclamation mark centered at `c`, `s` tall-ish.
fn excl(p: &mut Props, e: Entity, c: Vec2, s: f32, z: f32) {
    p.part(e, stroke(&[c + Vec2::new(0.0, s), c - Vec2::new(0.0, s * 0.25)], |t| 0.9 - 0.4 * t, 1.0, 1.0, |_| 1.0), M::Soot, z);
    let dot: Vec<Vec2> = ellipse_pts(0.45, 0.45, 8).into_iter().map(|q| q + c - Vec2::Y * s * 0.75).collect();
    p.part(e, fill(&dot, |_| 1.0), M::Soot, z);
}

/// A warning sign on a bent post: a triangle with an exclamation mark, bullet-holed.
fn sign_post(p: &mut Props, at: Vec2) {
    let flip = if p.rng.chance(0.5) { 1.0 } else { -1.0 };
    let hotfix_roll_1 = p.rng.range(-0.12, 0.12); // HOTFIX: evaluate before borrowing Props mutably (E0499).
    let e = p.root(at - Vec2::Y * S * 2.0, flip, hotfix_roll_1);
    let h = p.rng.range(26.0, 34.0);
    let top = Vec2::new(p.rng.range(-2.0, 2.0), h);
    p.bar(e, &[Vec2::new(0.0, -3.0), Vec2::new(top.x * 0.3, h * 0.5), top], 2.2, M::Iron, 0.0);
    let c = top + Vec2::new(0.0, 2.0);
    let s = p.rng.range(9.0, 11.0);
    let tilt = p.rng.range(-0.25, 0.25);
    let rot = |q: Vec2| c + Vec2::from_angle(tilt).rotate(q);
    let tri = [rot(Vec2::new(0.0, s)), rot(Vec2::new(s * 0.95, -s * 0.6)), rot(Vec2::new(-s * 0.95, -s * 0.6))];
    p.part(e, fill(&tri, |q| 0.75 + 0.25 * ((q.y - c.y) / s + 0.6).clamp(0.0, 1.0)), M::Enamel, 0.01);
    let mut border = tri.to_vec();
    border.push(tri[0]);
    p.part(e, stroke(&border, |_| 1.4, 0.4, 0.4, |_| 1.0), M::Soot, 0.015);
    excl(p, e, rot(Vec2::new(0.0, -s * 0.05)), s * 0.38, 0.02);
    // Bullet holes and rust weeping from them.
    for _ in 0..p.rng.below(4) {
        let q = rot(Vec2::new(p.rng.range(-s * 0.4, s * 0.4), p.rng.range(-s * 0.4, s * 0.2)));
        let hole: Vec<Vec2> = ellipse_pts(0.7, 0.7, 6).into_iter().map(|d| d + q).collect();
        p.part(e, fill(&hole, |_| 1.0), M::Soot, 0.03);
        let hotfix_roll_1 = p.rng.range(2.0, 5.0); // HOTFIX: evaluate before borrowing Props mutably (E0499).
        p.part(e, stroke(&[q, q - Vec2::Y * hotfix_roll_1], |t| 1.0 - 0.7 * t, 1.0, 1.0, |_| 1.0), M::Rust, 0.025);
    }
}

/// A sludge pool with a limescale crust and a bit of pipe sunk in it.
fn sludge(p: &mut Props, at: Vec2) {
    let e = p.root(at, 1.0, 0.0);
    let rx = p.rng.range(10.0, 16.0);
    let pool: Vec<Vec2> = ellipse_pts(rx, 2.6, 18).into_iter().map(|q| q + Vec2::Y * 0.4).collect();
    p.part(e, fill(&pool, |q| 0.45 + 0.35 * (q.y.max(0.0) / 2.6)), M::Soot, OVER);
    let crust = smooth_open(&[Vec2::new(-rx, 0.6), Vec2::new(-rx * 0.4, 3.2), Vec2::new(rx * 0.3, 3.0), Vec2::new(rx, 0.6)], 5);
    p.part(e, stroke(&crust, |_| 1.0, 1.0, 1.0, |t| 0.7 + 0.3 * (t * 9.0).sin().abs()), M::Enamel, OVER + 0.01);
    if p.rng.chance(0.55) {
        // A pipe elbow sticking out.
        let x = p.rng.range(-rx * 0.5, rx * 0.5);
        let up = p.rng.range(6.0, 11.0);
        let pts = [Vec2::new(x, -4.0), Vec2::new(x, up), Vec2::new(x + 6.0, up + 1.0)];
        p.part(e, stroke(&smooth_open(&pts, 4), |_| 5.0, 1.0, 0.45, |_| 1.0), M::Rust, 0.0);
        let lip: Vec<Vec2> = ellipse_pts(1.0, 2.5, 10).into_iter().map(|q| q + Vec2::new(x + 6.5, up + 1.0)).collect();
        p.part(e, fill(&lip, |_| 1.0), M::Soot, 0.01);
    }
}
