//! Props of "region: broken dam" (`Style::Dam`): safety railings along the
//! crest, sluice wheels and fallen spillway gates, rusted penstock sections,
//! warning signs, debris logs jammed in the breach; ladders bolted to the
//! faces, penstocks out of the walls, calcite icicles under the overhangs.

use super::*;

pub(super) const HOOKS: Hooks = Hooks { spacing, ground, edge, wall, ceiling };

/// (gap between props of the same kind, chance a free spot gets one), in prop units.
fn spacing(kind: Kind) -> (f32, f32) {
    match kind {
        Kind::Ground => (55.0, 0.8),
        Kind::Edge => (35.0, 0.85),
        Kind::Wall => (45.0, 0.8),
        Kind::Ceiling => (40.0, 0.7),
    }
}

#[derive(Clone, Copy)]
enum Dam {
    Railing,
    Wheel,
    Log,
    Penstock,
    Sign,
    Gate,
    Rubble,
}

impl Dam {
    /// (half width, height) in prop units.
    fn size(self) -> (f32, f32) {
        match self {
            Dam::Railing => (30.0, 26.0),
            Dam::Wheel => (12.0, 48.0),
            Dam::Log => (40.0, 18.0),
            Dam::Penstock => (32.0, 22.0),
            Dam::Sign => (10.0, 40.0),
            Dam::Gate => (30.0, 46.0),
            Dam::Rubble => (16.0, 14.0),
        }
    }
}

/// A top surface with headroom at `cell` (`x` = its world x). True if something was placed.
fn ground(p: &mut Props, cell: IVec2, x: f32) -> bool {
    use Dam::*;
    let table: &[(Dam, f32)] = &[(Railing, 2.5), (Wheel, 1.5), (Log, 2.0), (Penstock, 1.5), (Sign, 0.5), (Gate, 1.0), (Rubble, 1.5)];
    let total: f32 = table.iter().map(|e| e.1).sum();
    let mut r = p.rng.range(0.0, total);
    let pick = table.iter().find(|e| {
        r -= e.1;
        r <= 0.0
    });
    let picks = [pick.unwrap_or(&table[0]).0, Rubble];
    for g in picks {
        let (half, head) = g.size();
        let tilt = matches!(g, Log | Penstock | Rubble | Gate);
        let Some((y, angle, indoor)) = p.fit(cell, half, head, tilt) else { continue };
        let at = Vec2::new(x, y);
        p.tilt = angle;
        p.depth = if indoor { INDOOR } else { BACK };
        match g {
            Railing => railing(p, at),
            Wheel => sluice_wheel(p, at),
            Log => log(p, at),
            Penstock => penstock(p, at),
            Sign => sign(p, at),
            Gate => gate(p, at),
            Rubble => p.rubble(at),
        }
        p.tilt = 0.0;
        p.depth = BACK;
        return true;
    }
    false
}

/// Where the ground ends at a drop (`flip` = the open side).
fn edge(p: &mut Props, at: Vec2, flip: f32) -> bool {
    let e = p.root(at, flip, 0.0);
    match p.rng.below(3) {
        0 => broken_railing(p, e),
        1 => jammed_log(p, e),
        _ => {
            p.rebar_burst(e);
            if p.rng.chance(0.5) {
                let hotfix_roll_1 = p.rng.range(2.0, 8.0); // HOTFIX: evaluate before borrowing Props mutably (E0499).
                let hotfix_roll_2 = p.rng.range(20.0, 50.0); // HOTFIX: evaluate before borrowing Props mutably (E0499).
                chain(p, e, Vec2::new(hotfix_roll_1, -2.0), hotfix_roll_2);
            }
        }
    }
    true
}

/// A straight vertical face (`flip` = the open side).
fn wall(p: &mut Props, at: Vec2, flip: f32) -> bool {
    let roll = p.rng.next();
    if roll < 0.35 {
        let e = p.root(at, flip, 0.0);
        ladder(p, e);
    } else if roll < 0.6 {
        let e = p.root(at, flip, 0.0);
        let r = p.rng.range(7.0, 11.0);
        let y = p.rng.range(-6.0, 6.0);
        p.pipe(e, at, flip, Vec2::new(0.0, y), r);
    } else if roll < 0.75 {
        let half = Vec2::new(9.0, 7.0);
        let c = at + Vec2::new(-flip * (half.x * S + PX * 2.0), 0.0);
        if p.inside(c, half * S) {
            let e = p.root(c, flip, 0.0);
            plate(p, e, half);
        } else {
            let e = p.root(at, flip, 0.0);
            ladder(p, e);
        }
    } else {
        // A fissure on the face, rebar spilling out of it.
        let e = p.root(at, flip, 0.0);
        for _ in 0..2 + p.rng.below(3) {
            let base = Vec2::new(-2.0, p.rng.range(-12.0, 12.0));
            let dir = Vec2::new(1.0, p.rng.range(-0.6, 0.3)).normalize();
            p.rebar(e, base, dir, 0.0);
        }
    }
    true
}

/// The underside of a thick overhang.
fn ceiling(p: &mut Props, at: Vec2) -> bool {
    let e = p.root(at, 1.0, 0.0);
    stalactites(p, e);
    if p.rng.chance(0.45) {
        let x = p.rng.range(-10.0, 10.0);
        let len = p.rng.range(18.0, 46.0);
        chain(p, e, Vec2::new(x, 2.0), len);
    } else if p.rng.chance(0.4) {
        cage_lamp(p, e);
    }
    true
}

// --- Drawing ---

fn straight(p: &mut Props, e: Entity, a: Vec2, b: Vec2, w: f32, m: M, z: f32) {
    p.part(e, stroke(&[a, b], |_| w, 1.0, 0.55, |_| 1.0), m, z);
}

/// A safety railing: posts and two rails, one bay bent down, some posts gone.
fn railing(p: &mut Props, at: Vec2) {
    let flip = if p.rng.chance(0.5) { 1.0 } else { -1.0 };
    let e = p.root(at - Vec2::Y * S * 2.0, flip, 0.0);
    let n = 4 + p.rng.below(2);
    let span = 58.0 / (n - 1) as f32;
    let bent = p.rng.below(n - 1);
    let mut tops = Vec::new();
    for i in 0..n {
        let x = -29.0 + span * i as f32;
        let lean = p.rng.range(-1.5, 1.5) + if i == bent + 1 { p.rng.range(4.0, 9.0) } else { 0.0 };
        let h = 22.0 - if i == bent + 1 { p.rng.range(4.0, 8.0) } else { 0.0 };
        let top = Vec2::new(x + lean, h);
        straight(p, e, Vec2::new(x, -2.0), top, 2.2, M::Steel, 0.0);
        // Base plate bolted to the crest.
        straight(p, e, Vec2::new(x - 3.0, 0.6), Vec2::new(x + 3.0, 0.6), 1.6, M::Iron, 0.01);
        tops.push(top);
    }
    for (k, frac) in [1.0f32, 0.5].into_iter().enumerate() {
        for i in 0..n - 1 {
            let (a, b) = (Vec2::new(tops[i].x, tops[i].y * frac), Vec2::new(tops[i + 1].x, tops[i + 1].y * frac));
            if i == bent && k == 1 {
                // Snapped: both halves hang.
                let m = a.lerp(b, 0.5);
                p.bar(e, &[a, a.lerp(m, 0.6), m + Vec2::new(-2.0, -7.0)], 1.6, M::Steel, 0.02);
                continue;
            }
            let sag = if i == bent { 5.0 } else { 0.5 };
            p.bar(e, &[a, a.lerp(b, 0.5) - Vec2::Y * sag, b], 1.8, M::Steel, 0.02);
        }
    }
    // Hazard tape stripes on the top rail's first bay.
    let (a, b) = (tops[0], tops[1]);
    let stripes = 9.0;
    p.part(e, stroke(&[a + Vec2::Y * 0.2, b + Vec2::Y * 0.2], |_| 1.0, 1.0, 1.0, move |t| if (t * stripes) as i32 % 2 == 0 { 1.0 } else { 0.2 }), M::Enamel, 0.03);
}

/// A sluice gate hoist: concrete pedestal, gearbox, a big spoked hand wheel
/// and the threaded stem rising out of it.
fn sluice_wheel(p: &mut Props, at: Vec2) {
    let flip = if p.rng.chance(0.5) { 1.0 } else { -1.0 };
    let e = p.root(at - Vec2::Y * S * 2.0, flip, 0.0);
    let ped = [Vec2::new(-9.0, -3.0), Vec2::new(-8.0, 10.0), Vec2::new(8.0, 10.0), Vec2::new(9.0, -3.0)];
    p.shape(e, &ped, 0.45, M::Concrete, 0.0);
    straight(p, e, Vec2::new(-9.5, 10.0), Vec2::new(9.5, 10.0), 2.0, M::Rim, 0.01);
    let gear = [Vec2::new(-6.0, 10.0), Vec2::new(-5.5, 20.0), Vec2::new(5.5, 20.0), Vec2::new(6.0, 10.0)];
    p.shape(e, &gear, 0.5, M::Iron, 0.02);
    // Threaded stem, bent at the top.
    let h = p.rng.range(36.0, 48.0);
    let kink = p.rng.range(-5.0, 5.0);
    p.part(e, stroke(&smooth_open(&[Vec2::new(0.0, 18.0), Vec2::new(0.0, h * 0.8), Vec2::new(kink, h)], 6), |_| 2.4, 1.0, 0.5, |t| if (t * 22.0) as i32 % 2 == 0 { 1.0 } else { 0.7 }), M::Steel, 0.01);
    let c = Vec2::new(0.0, 24.0);
    let r = p.rng.range(10.0, 12.5);
    p.ring(e, c, r, 2.4, M::Rust, 0.03, |_| 1.0);
    p.ring(e, c, 2.2, 1.6, M::Iron, 0.04, |_| 1.0);
    let rot = p.rng.range(0.0, 1.0);
    for i in 0..3 {
        let d = Vec2::from_angle(i as f32 * std::f32::consts::FRAC_PI_3 + rot) * r;
        straight(p, e, c - d, c + d, 1.4, M::Rust, 0.03);
    }
    // Handle knob.
    let k = c + Vec2::from_angle(rot + 0.5) * r;
    straight(p, e, k, k + Vec2::new(0.0, 4.0), 2.0, M::Iron, 0.05);
    if p.rng.chance(0.5) {
        p.drip(e, Vec2::new(5.0, 10.0), 1.4, 5.0, OVER);
    }
}

/// A drowned tree trunk washed down through the breach, branches snapped.
fn log(p: &mut Props, at: Vec2) {
    let flip = if p.rng.chance(0.5) { 1.0 } else { -1.0 };
    let angle = p.rng.range(-0.08, 0.12);
    let e = p.root(at - Vec2::Y * S * 3.0, flip, angle);
    let len = p.rng.range(62.0, 80.0);
    let r = p.rng.range(5.5, 7.5);
    let trunk = smooth_open(&[Vec2::new(-len / 2.0, r), Vec2::new(0.0, r + 1.0), Vec2::new(len / 2.0, r * 0.9)], 6);
    p.part(e, stroke(&trunk, |t| 2.0 * r * (1.0 - 0.35 * t), 1.0, 0.4, |t| 0.9 + 0.1 * ((t * 31.0).sin())), M::Wood, 0.0);
    // Bark furrows.
    for k in [-0.3, 0.25] {
        let line: Vec<Vec2> = trunk.iter().map(|q| *q + Vec2::Y * r * k).collect();
        p.part(e, stroke(&line, |_| 0.7, 0.5, 0.5, |t| if (t * 13.0).fract() < 0.7 { 1.0 } else { 0.0 }), M::Soot, 0.01);
    }
    // The root plate at the thick end.
    let base = Vec2::new(-len / 2.0, r);
    for i in 0..5 {
        let a = std::f32::consts::FRAC_PI_2 + (i as f32 - 2.0) * 0.55;
        let d = Vec2::from_angle(a + std::f32::consts::FRAC_PI_2);
        let l = p.rng.range(8.0, 15.0);
        let hotfix_roll_1 = p.rng.range(-2.0, 2.0); // HOTFIX: evaluate before borrowing Props mutably (E0499).
        p.part(e, stroke(&smooth_open(&[base, base + d * l * 0.5 + Vec2::new(0.0, hotfix_roll_1), base + d * l], 4), |t| 3.2 * (1.0 - t) + 0.5, 1.0, 0.5, |_| 1.0), M::Wood, 0.015);
    }
    // Snapped branch stubs.
    for _ in 0..2 + p.rng.below(2) {
        let x = p.rng.range(-len / 4.0, len / 2.0 - 6.0);
        let from = Vec2::new(x, r + r * 0.8);
        let tip = from + Vec2::new(p.rng.range(3.0, 9.0), p.rng.range(5.0, 14.0));
        p.part(e, stroke(&[from, tip], |t| 2.6 * (1.0 - t) + 0.6, 1.0, 0.5, |_| 1.0), M::Wood, 0.02);
    }
    // Snapped end: pale splinters.
    let end = Vec2::new(len / 2.0, r * 0.9);
    let tris = [[end + Vec2::new(-1.0, r * 0.6), end + Vec2::new(4.0, r * 0.3), end + Vec2::new(-1.0, 0.0)], [end + Vec2::new(-1.0, 0.0), end + Vec2::new(3.0, -r * 0.4), end + Vec2::new(-1.0, -r * 0.6)]];
    p.part(e, triangles(&tris), M::Rim, 0.03);
}

/// A rusted penstock section, riveted bands, its broken end gaping.
fn penstock(p: &mut Props, at: Vec2) {
    let flip = if p.rng.chance(0.5) { 1.0 } else { -1.0 };
    let hotfix_roll_1 = p.rng.range(-0.05, 0.05); // HOTFIX: evaluate before borrowing Props mutably (E0499).
    let e = p.root(at - Vec2::Y * S * 4.0, flip, hotfix_roll_1);
    let (len, r) = (p.rng.range(52.0, 64.0), p.rng.range(9.0, 11.0));
    let (a, b) = (Vec2::new(-len / 2.0, r), Vec2::new(len / 2.0, r));
    p.part(e, stroke(&[a, b], |_| 2.0 * r, 1.0, 0.35, |_| 1.0), M::Rust, 0.0);
    // Riveted bands.
    for k in 0..4 {
        let x = -len / 2.0 + 4.0 + k as f32 * (len - 10.0) / 3.0;
        straight(p, e, Vec2::new(x, -0.5), Vec2::new(x, 2.0 * r + 0.5), 2.6, M::Iron, 0.01);
        let mut dots = Vec::new();
        for j in 0..5 {
            let y = 2.0 + j as f32 * (2.0 * r - 4.0) / 4.0;
            let q = Vec2::new(x + 2.2, y);
            dots.push([q + Vec2::new(-0.6, -0.6), q + Vec2::new(0.6, -0.6), q + Vec2::new(0.0, 0.7)]);
        }
        p.part(e, triangles(&dots), M::Steel, 0.015);
    }
    // Torn open end: jagged dark mouth.
    let mut mouth = Vec::new();
    for i in 0..12 {
        let t = i as f32 / 12.0 * std::f32::consts::TAU;
        let jag = if i % 2 == 0 { 1.0 } else { p.rng.range(0.6, 1.3) };
        mouth.push(b + Vec2::new(t.cos() * r * 0.4 * jag, t.sin() * r));
    }
    p.part(e, fill(&mouth, |_| 1.0), M::Soot, 0.02);
    p.ring_ellipse(e, b, r * 0.4, r, M::Rust);
    // Rust streaks along its belly.
    for _ in 0..2 {
        let x = p.rng.range(-len / 2.5, len / 3.0);
        straight(p, e, Vec2::new(x, r * 1.4), Vec2::new(x + 0.6, r * 0.2), 1.6, M::Soot, 0.012);
    }
}

/// A warning sign on a bent post: a pale panel with a black triangle and a bar.
fn sign(p: &mut Props, at: Vec2) {
    let flip = if p.rng.chance(0.5) { 1.0 } else { -1.0 };
    let hotfix_roll_1 = p.rng.range(-0.15, 0.15); // HOTFIX: evaluate before borrowing Props mutably (E0499).
    let e = p.root(at - Vec2::Y * S * 2.0, flip, hotfix_roll_1);
    let h = p.rng.range(26.0, 34.0);
    let lean = p.rng.range(-3.0, 3.0);
    let top = Vec2::new(lean, h);
    p.bar(e, &[Vec2::new(0.0, -4.0), Vec2::new(lean * 0.3, h * 0.5), top], 2.0, M::Steel, 0.0);
    let hotfix_roll_1 = p.rng.range(-0.2, 0.2); // HOTFIX: evaluate before borrowing Props mutably (E0499).
    sign_panel(p, e, top + Vec2::new(0.0, 2.0), Vec2::new(9.0, 8.0), hotfix_roll_1, 0.01);
}

/// The sign itself: panel, border, hazard triangle with "!", rust and bullet holes.
fn sign_panel(p: &mut Props, e: Entity, c: Vec2, half: Vec2, angle: f32, z: f32) {
    let rot = |v: Vec2| c + Vec2::from_angle(angle).rotate(v);
    let quad: Vec<Vec2> = [Vec2::new(-half.x, -half.y), Vec2::new(-half.x, half.y), Vec2::new(half.x, half.y), Vec2::new(half.x, -half.y)].into_iter().map(rot).collect();
    p.shape(e, &quad, 0.75, M::Enamel, z);
    let mut border = quad.clone();
    border.push(quad[0]);
    p.part(e, stroke(&border, |_| 1.2, 1.0, 1.0, |_| 1.0), M::Iron, z + 0.01);
    let s = half.y * 0.8;
    let tri = [rot(Vec2::new(-s, -s * 0.75)), rot(Vec2::new(0.0, s * 0.9)), rot(Vec2::new(s, -s * 0.75))];
    let mut outline = tri.to_vec();
    outline.push(tri[0]);
    p.part(e, stroke(&outline, |_| 1.6, 1.0, 1.0, |_| 1.0), M::Soot, z + 0.02);
    p.part(e, stroke(&[rot(Vec2::new(0.0, s * 0.4)), rot(Vec2::new(0.0, -s * 0.2))], |_| 1.4, 1.0, 1.0, |_| 1.0), M::Soot, z + 0.02);
    p.part(e, stroke(&[rot(Vec2::new(0.0, -s * 0.38)), rot(Vec2::new(0.0, -s * 0.52))], |_| 1.4, 1.0, 1.0, |_| 1.0), M::Soot, z + 0.02);
    // Rust bleeding from a corner, and bullet holes.
    let corner = rot(Vec2::new(half.x - 1.0, half.y - 1.0));
    let hotfix_roll_1 = p.rng.range(6.0, 12.0); // HOTFIX: evaluate before borrowing Props mutably (E0499).
    p.part(e, stroke(&[corner, corner - Vec2::Y * hotfix_roll_1], |t| 2.2 * (1.0 - t) + 0.3, 1.0, 1.0, |_| 1.0), M::Rust, z + 0.025);
    for _ in 0..p.rng.below(4) {
        let q = rot(Vec2::new(p.rng.range(-half.x + 2.0, half.x - 2.0), p.rng.range(-half.y + 2.0, half.y - 2.0)));
        let hole: Vec<Vec2> = ellipse_pts(0.8, 0.8, 6).into_iter().map(|v| v + q).collect();
        p.part(e, fill(&hole, |_| 1.0), M::Soot, z + 0.03);
    }
}

/// A radial spillway gate torn off its trunnions, leaning: a curved skin
/// plate with ribs and the two arms meeting at the pin.
fn gate(p: &mut Props, at: Vec2) {
    let flip = if p.rng.chance(0.5) { 1.0 } else { -1.0 };
    let hotfix_roll_1 = p.rng.range(-0.12, 0.12); // HOTFIX: evaluate before borrowing Props mutably (E0499).
    let e = p.root(at - Vec2::Y * S * 3.0, flip, hotfix_roll_1);
    let rad = p.rng.range(40.0, 48.0);
    let pin = Vec2::new(-rad * 0.55, 22.0);
    let (a0, a1) = (-0.62f32, 0.55f32);
    let arc: Vec<Vec2> = (0..=16).map(|i| pin + Vec2::from_angle(a0 + (a1 - a0) * i as f32 / 16.0) * rad).collect();
    // Skin plate.
    p.part(e, stroke(&arc, |_| 5.0, 1.0, 0.4, |t| if (t * 6.0).fract() < 0.1 { 0.6 } else { 1.0 }), M::Rust, 0.01);
    // Horizontal girders behind it.
    for k in [0.2f32, 0.5, 0.8] {
        let q = arc[(k * 16.0) as usize];
        straight(p, e, q, q.lerp(pin, 0.18), 3.0, M::Iron, 0.0);
    }
    // Two arms to the trunnion pin.
    for k in [0.15f32, 0.85] {
        let q = arc[(k * 16.0) as usize];
        straight(p, e, q.lerp(pin, 0.1), pin, 3.4, M::Iron, 0.005);
    }
    p.ring(e, pin, 2.6, 2.0, M::Steel, 0.02, |_| 1.0);
    // Torn hinge plate on the pin.
    let tris = [[pin + Vec2::new(-3.0, -2.0), pin + Vec2::new(-8.0, -4.0), pin + Vec2::new(-5.0, 3.0)]];
    p.part(e, triangles(&tris), M::Rust, 0.015);
    // Water stains streaked down the skin.
    let q = arc[10];
    p.part(e, stroke(&[q + Vec2::new(-1.0, 0.0), q + Vec2::new(-1.5, -10.0)], |t| 2.0 * (1.0 - t) + 0.4, 1.0, 1.0, |_| 1.0), M::Soot, 0.02);
}

/// A railing post snapped at the lip, bent out over the drop, a rail dangling.
fn broken_railing(p: &mut Props, e: Entity) {
    let x = -p.rng.range(3.0, 7.0);
    let out = p.rng.range(6.0, 14.0);
    let top = Vec2::new(x + out, p.rng.range(10.0, 18.0));
    p.bar(e, &[Vec2::new(x, -2.0), Vec2::new(x + 1.0, 6.0), top], 2.2, M::Steel, 0.0);
    let len = p.rng.range(16.0, 30.0);
    let hotfix_roll_1 = p.rng.range(-3.0, 3.0); // HOTFIX: evaluate before borrowing Props mutably (E0499).
    p.bar(e, &[top, top + Vec2::new(4.0, -len * 0.4), top + Vec2::new(2.0 + hotfix_roll_1, -len)], 1.8, M::Steel, 0.01);
    if p.rng.chance(0.6) {
        let back = Vec2::new(x - 24.0, 21.0);
        p.bar(e, &[back, Vec2::new(x - 10.0, 18.0), top], 1.8, M::Steel, 0.01);
    }
}

/// A debris log jammed into the lip, sticking out over the drop.
fn jammed_log(p: &mut Props, e: Entity) {
    let r = p.rng.range(4.5, 6.5);
    let rise = p.rng.range(-4.0, 10.0);
    let (a, b) = (Vec2::new(-28.0, -r * 1.3), Vec2::new(p.rng.range(18.0, 30.0), -r * 0.5 + rise));
    p.part(e, stroke(&smooth_open(&[a, a.lerp(b, 0.5) + Vec2::Y, b], 4), |t| 2.0 * r * (1.0 - 0.3 * t), 1.0, 0.4, |_| 1.0), M::Wood, 0.0);
    let tris = [[b + Vec2::new(-1.0, r * 0.7), b + Vec2::new(4.0, r * 0.2), b + Vec2::new(-1.0, -r * 0.6)]];
    p.part(e, triangles(&tris), M::Rim, 0.01);
    let mid = a.lerp(b, 0.7);
    p.part(e, stroke(&[mid + Vec2::Y * r, mid + Vec2::new(4.0, r + 9.0)], |t| 2.2 * (1.0 - t) + 0.5, 1.0, 0.5, |_| 1.0), M::Wood, 0.01);
    // Weed and trash snagged on it, hanging.
    let snag = a.lerp(b, 0.85);
    let len = p.rng.range(8.0, 18.0);
    p.part(e, stroke(&smooth_open(&[snag, snag + Vec2::new(1.5, -len * 0.5), snag + Vec2::new(-0.5, -len)], 4), |t| 2.6 * (1.0 - t) + 0.4, 1.0, 0.7, |_| 1.0), M::Slime, 0.02);
}

/// A rusty chain hanging from `from`.
fn chain(p: &mut Props, e: Entity, from: Vec2, len: f32) {
    let sway = p.rng.range(-4.0, 4.0);
    let path = smooth_open(&[from, from + Vec2::new(sway * 0.3, -len * 0.5), from + Vec2::new(sway, -len)], 8);
    p.part(e, stroke(&path, |_| 2.0, 1.0, 0.5, |t| if (t * len / 2.5) as i32 % 2 == 0 { 1.0 } else { 0.45 }), M::Rust, 0.03);
    let end = path[path.len() - 1];
    if p.rng.chance(0.5) {
        // A hook at the end.
        p.bar(e, &[end, end + Vec2::new(0.0, -3.0), end + Vec2::new(2.5, -4.0), end + Vec2::new(3.0, -1.5)], 1.4, M::Iron, 0.04);
    }
}

/// A maintenance ladder bolted to the face, rungs rusted, a few missing.
fn ladder(p: &mut Props, e: Entity) {
    let (x0, x1) = (3.5, 12.5);
    let (top, bottom) = (p.rng.range(10.0, 30.0), -p.rng.range(40.0, 90.0));
    // Twisted near the bottom where it tore off a bracket.
    let twist = p.rng.range(-3.0, 4.0);
    for x in [x0, x1] {
        let pts = [Vec2::new(x, top), Vec2::new(x, (top + bottom) / 2.0), Vec2::new(x + twist, bottom)];
        p.bar(e, &pts, 1.8, M::Steel, 0.02);
    }
    let mut y = top - 3.0;
    let mut i = 0;
    while y > bottom + 2.0 {
        let k = (top - y) / (top - bottom);
        let off = twist * (k - 0.5).max(0.0) * 2.0;
        if p.rng.chance(0.88) {
            straight(p, e, Vec2::new(x0 + off, y), Vec2::new(x1 + off, y), 1.3, if i % 3 == 0 { M::Rust } else { M::Steel }, 0.025);
        }
        y -= 6.0;
        i += 1;
    }
    // Brackets into the concrete.
    let mut y = top - 6.0;
    while y > bottom + 10.0 {
        for x in [x0, x1] {
            straight(p, e, Vec2::new(-3.0, y), Vec2::new(x, y), 1.6, M::Iron, 0.015);
        }
        y -= p.rng.range(24.0, 34.0);
    }
    // Safety hoops at the top.
    if p.rng.chance(0.5) {
        let c = Vec2::new((x0 + x1) / 2.0, top - 4.0);
        let hoop: Vec<Vec2> = (0..=12).map(|i| c + Vec2::new((i as f32 / 12.0 * std::f32::consts::PI).cos() * 7.0, -(i as f32 / 12.0 * std::f32::consts::PI).sin() * 3.0 + 4.0)).collect();
        p.part(e, stroke(&hoop, |_| 1.2, 1.0, 0.6, |_| 1.0), M::Steel, 0.03);
    }
}

/// A warning plate bolted to the face (drawn over the terrain).
fn plate(p: &mut Props, e: Entity, half: Vec2) {
    let hotfix_roll_1 = p.rng.range(-0.08, 0.08); // HOTFIX: evaluate before borrowing Props mutably (E0499).
    sign_panel(p, e, Vec2::ZERO, half, hotfix_roll_1, OVER);
    // Rust running down the wall from it.
    let x = p.rng.range(-half.x * 0.6, half.x * 0.6);
    let len = p.rng.range(10.0, 24.0);
    p.part(e, stroke(&[Vec2::new(x, -half.y), Vec2::new(x + 0.4, -half.y - len)], |t| 2.4 * (1.0 - t) + 0.3, 1.0, 1.0, |_| 1.0), M::Rust, OVER - 0.01);
}

/// Calcite icicles where water seeps through the overhang (pale, over the terrain).
fn stalactites(p: &mut Props, e: Entity) {
    let mut tris = Vec::new();
    for _ in 0..3 + p.rng.below(4) {
        let x = p.rng.range(-14.0, 14.0);
        let (w, len) = (p.rng.range(1.2, 3.0), p.rng.range(4.0, 14.0));
        tris.push([Vec2::new(x - w, 1.5), Vec2::new(x + w, 1.5), Vec2::new(x + p.rng.range(-0.8, 0.8), -len)]);
    }
    p.part(e, triangles(&tris), M::Enamel, OVER);
    // A wet stain spreading along the underside.
    let stain: Vec<Vec2> = ellipse_pts(p.rng.range(10.0, 16.0), 2.0, 14).into_iter().map(|q| q + Vec2::Y * 1.0).collect();
    p.part(e, fill(&stain, |_| 0.8), M::Slime, OVER - 0.01);
    if p.rng.chance(0.4) {
        let x = p.rng.range(-8.0, 8.0);
        let hotfix_roll_1 = p.rng.range(4.0, 8.0); // HOTFIX: evaluate before borrowing Props mutably (E0499).
        p.drip(e, Vec2::new(x, 0.5), 1.4, hotfix_roll_1, OVER + 0.01);
    }
}

/// A caged work lamp hanging on its cable, glass smashed.
fn cage_lamp(p: &mut Props, e: Entity) {
    let x = p.rng.range(-8.0, 8.0);
    let len = p.rng.range(12.0, 28.0);
    let end = Vec2::new(x + p.rng.range(-2.0, 2.0), -len);
    p.part(e, stroke(&[Vec2::new(x, 2.0), end], |_| 1.0, 0.8, 0.5, |_| 1.0), M::Soot, 0.03);
    let body = [end + Vec2::new(-3.5, 0.0), end + Vec2::new(-4.0, -8.0), end + Vec2::new(4.0, -8.0), end + Vec2::new(3.5, 0.0)];
    p.shape(e, &body, 0.5, M::Soot, 0.035);
    for dx in [-2.5, 0.0, 2.5] {
        straight(p, e, end + Vec2::new(dx, 0.0), end + Vec2::new(dx * 1.2, -8.0), 0.8, M::Iron, 0.04);
    }
    straight(p, e, end + Vec2::new(-4.2, -4.0), end + Vec2::new(4.2, -4.0), 0.8, M::Iron, 0.04);
    p.part(e, triangles(&[[end + Vec2::new(-1.5, -2.0), end + Vec2::new(1.0, -2.5), end + Vec2::new(-0.5, -6.0)]]), M::Rim, 0.045);
}
