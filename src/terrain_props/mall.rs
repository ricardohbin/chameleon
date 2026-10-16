//! Props of "region: abandoned shopping center" (`Style::Mall`): a looted mall.
//! On the floors: shopping carts (some on their side), toppled and standing
//! mannequins (limbs off, heads rolled away), dead potted palms, food-court
//! tables and chairs, fallen sign letters, heaps of display glass, and now and
//! then a collapsed escalator. Mezzanine edges keep their bent chrome
//! handrails and shattered glass balustrades; walls are storefronts behind
//! half-rolled shutters or have signs hanging off one bracket; ceilings shed
//! their drop-ceiling grid and fluorescent fixtures dangling on their wires.

use std::f32::consts::{FRAC_PI_2, PI};

use super::*;

pub(super) const HOOKS: Hooks = Hooks { spacing, ground, edge, wall, ceiling };

/// (gap between props of the same kind, chance a free spot gets one), in prop units.
fn spacing(kind: Kind) -> (f32, f32) {
    match kind {
        Kind::Ground => (50.0, 0.8),
        Kind::Edge => (35.0, 0.85),
        Kind::Wall => (55.0, 0.75),
        Kind::Ceiling => (40.0, 0.7),
    }
}

#[derive(Clone, Copy)]
enum Thing {
    Cart,
    Mannequin,
    Fallen,
    Palm,
    FoodCourt,
    Letter,
    Glass,
    Escalator,
}

impl Thing {
    /// (half width, height) in prop units.
    fn size(self) -> (f32, f32) {
        match self {
            Thing::Cart => (18.0, 28.0),
            Thing::Mannequin => (7.0, 50.0),
            Thing::Fallen => (26.0, 12.0),
            Thing::Palm => (9.0, 46.0),
            Thing::FoodCourt => (22.0, 24.0),
            Thing::Letter => (14.0, 28.0),
            Thing::Glass => (14.0, 6.0),
            Thing::Escalator => (46.0, 44.0),
        }
    }
}

/// A top surface with headroom at `cell` (`x` = its world x). True if something was placed.
fn ground(p: &mut Props, cell: IVec2, x: f32) -> bool {
    use Thing::*;
    let table: &[(Thing, f32)] = &[
        (Cart, 2.5),
        (Mannequin, 1.1),
        (Fallen, 1.8),
        (Palm, 1.5),
        (FoodCourt, 1.6),
        (Letter, 1.4),
        (Glass, 1.2),
        (Escalator, 0.5),
    ];
    let total: f32 = table.iter().map(|e| e.1).sum();
    let mut r = p.rng.range(0.0, total);
    let pick = table.iter().find(|e| {
        r -= e.1;
        r <= 0.0
    });
    for g in [pick.unwrap_or(&table[0]).0, Glass] {
        let (half, head) = g.size();
        let tilt = matches!(g, Fallen | Glass | Letter | Cart);
        let Some((y, angle, indoor)) = p.fit(cell, half, head, tilt) else { continue };
        let at = Vec2::new(x, y);
        p.tilt = angle;
        p.depth = if indoor { INDOOR } else { BACK };
        match g {
            Cart => cart(p, at),
            Mannequin => standing_mannequin(p, at),
            Fallen => fallen_mannequin(p, at),
            Palm => palm(p, at),
            FoodCourt => food_court(p, at),
            Letter => letter(p, at),
            Glass => glass(p, at),
            Escalator => escalator(p, at),
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
    handrail(p, e);
    if p.rng.chance(0.35) {
        let a = -0.3 - p.rng.range(0.0, 0.5);
        tube(p, e, Vec2::new(2.0, -2.0), a, 0.05);
    }
    true
}

/// A straight vertical face (`flip` = the open side).
fn wall(p: &mut Props, at: Vec2, flip: f32) -> bool {
    let half = Vec2::new(p.rng.range(15.0, 20.0), p.rng.range(16.0, 20.0));
    let c = at + Vec2::new(-flip * (half.x * S + PX * 2.0), 0.0);
    if p.rng.chance(0.6) && p.inside(c, half * S) {
        let e = p.root(c, flip, 0.0);
        storefront(p, e, half);
    } else {
        let e = p.root(at, flip, 0.0);
        hanging_sign(p, e);
    }
    true
}

/// The underside of a thick overhang.
fn ceiling(p: &mut Props, at: Vec2) -> bool {
    // Over a passable interior the props must stand in front of it.
    let below = p.t.cell(p.t.to_cell(at - Vec2::Y * PX * 1.5));
    p.depth = if below == Cell::Back { INDOOR } else { BACK };
    let f = flip(p);
    let e = p.root(at, f, 0.0);
    if p.rng.chance(0.6) {
        fixture(p, e);
    } else {
        ceiling_grid(p, e);
    }
    if p.rng.chance(0.4) {
        let x = p.rng.range(-10.0, 10.0);
        p.cables(e, Vec2::new(x, 2.0), 1, -1.0);
    }
    p.depth = BACK;
    true
}

// --- Helpers ---

/// Broken shop glass once the Weaver's blue is back.
const GLASS_BLUE: Color = Color::srgb(0.62, 0.80, 0.98);

/// Points of a part placed at `base`, turned by `rot`.
fn place(pts: &[Vec2], base: Vec2, rot: f32) -> Vec<Vec2> {
    let r = Vec2::from_angle(rot);
    pts.iter().map(|&q| base + r.rotate(q)).collect()
}

fn line(p: &mut Props, e: Entity, a: Vec2, b: Vec2, w: f32, m: M, z: f32) {
    p.part(e, stroke(&[a, b], |_| w, 1.0, 0.6, |_| 1.0), m, z);
}

fn flip(p: &mut Props) -> f32 {
    if p.rng.chance(0.5) { 1.0 } else { -1.0 }
}

/// Little glass triangles scattered around `c`, `spread` wide.
fn shards(p: &mut Props, e: Entity, c: Vec2, spread: f32, n: usize, z: f32) {
    let mut tris = Vec::new();
    for _ in 0..n {
        let o = c + Vec2::new(p.rng.range(-spread, spread), p.rng.range(0.0, 2.0));
        let a = p.rng.range(0.0, PI);
        let (l, w) = (p.rng.range(2.0, 5.0), p.rng.range(0.8, 2.0));
        let d = Vec2::from_angle(a);
        tris.push([o, o + d * l, o + d.perp() * w]);
    }
    p.part_blue(e, triangles(&tris), M::Rim, z, Some(GLASS_BLUE));
}

// --- Floors ---

/// A shopping cart: a wire basket on a chassis; upright and rolled off, or on its side.
fn cart(p: &mut Props, at: Vec2) {
    let side = p.rng.chance(0.4);
    let f = flip(p);
    let (base, rot) = if side { (at + Vec2::Y * S * 9.0, f * FRAC_PI_2 * p.rng.range(0.9, 1.0)) } else { (at - Vec2::Y * S, 0.0) };
    let e = p.root(base, f, rot);
    // Basket: taller at the handle end (−x), tapering to the nose.
    let (bl, bt, nl, nb) = (Vec2::new(-13.0, 10.0), Vec2::new(-15.0, 24.0), Vec2::new(12.0, 21.0), Vec2::new(9.0, 11.0));
    let rows = 4;
    for i in 0..=rows {
        let t = i as f32 / rows as f32;
        line(p, e, bl.lerp(bt, t), nb.lerp(nl, t), if i == 0 || i == rows { 1.2 } else { 0.6 }, M::Steel, 0.02);
    }
    let cols = 8;
    for i in 0..=cols {
        let t = i as f32 / cols as f32;
        line(p, e, bl.lerp(nb, t), bt.lerp(nl, t), if i == 0 || i == cols { 1.2 } else { 0.6 }, M::Steel, 0.02);
    }
    // Dented nose: one wire snapped and sprung out.
    if p.rng.chance(0.5) {
        let a = nb.lerp(nl, 0.5);
        p.bar(e, &[a, a + Vec2::new(4.0, 2.0), a + Vec2::new(6.0, 6.0)], 0.7, M::Steel, 0.03);
    }
    // Handle and chassis.
    p.bar(e, &[bt, bt + Vec2::new(-2.5, 2.0), bt + Vec2::new(-4.0, 3.5)], 1.4, M::Steel, 0.03);
    line(p, e, Vec2::new(-12.0, 3.0), Vec2::new(10.0, 3.0), 1.6, M::Steel, 0.01);
    line(p, e, Vec2::new(-12.0, 3.0), bl, 1.4, M::Steel, 0.01);
    line(p, e, Vec2::new(8.0, 3.0), nb, 1.2, M::Steel, 0.01);
    // Casters (one gone).
    let gone = p.rng.below(3);
    for (i, x) in [-10.0, 8.0].into_iter().enumerate() {
        line(p, e, Vec2::new(x, 3.0), Vec2::new(x, 1.8), 1.0, M::Iron, 0.0);
        if i != gone {
            let pts: Vec<Vec2> = ellipse_pts(1.8, 1.8, 10).into_iter().map(|q| q + Vec2::new(x, 1.6)).collect();
            p.part(e, fill(&pts, |_| 0.8), M::Rubber, 0.01);
        }
    }
    // Something left in it: a torn bag.
    if !side && p.rng.chance(0.4) {
        let c = Vec2::new(p.rng.range(-6.0, 3.0), 18.0);
        let r = p.rng.range(4.0, 5.5);
        p.bag(e, c, r, -0.01);
    }
}

/// Pieces of a mannequin, each placed by (base, rot) in the root's frame.
/// `pose` = the whole figure standing at `base` turned by `rot`;
/// `arm` false = an arm torn off; `head` = where the head is (None = gone).
struct Figure {
    base: Vec2,
    rot: f32,
    legs: bool,
    arm: bool,
    head: Option<(Vec2, f32)>,
}

fn figure(p: &mut Props, e: Entity, f: &Figure, z: f32) {
    let at = |pts: &[Vec2]| place(pts, f.base, f.rot);
    if f.legs {
        for (x0, x1) in [(-2.4, -2.0), (2.4, 2.9)] {
            let leg = at(&[Vec2::new(x1, 0.5), Vec2::new((x0 + x1) / 2.0, 11.0), Vec2::new(x0, 22.0)]);
            p.part(e, stroke(&smooth_open(&leg, 4), |t| 2.6 + 1.6 * t, 1.0, 0.55, |_| 1.0), M::Enamel, z);
        }
    }
    let hips = at(&[Vec2::new(-4.6, 20.5), Vec2::new(4.6, 20.5), Vec2::new(4.2, 25.0), Vec2::new(-4.2, 25.0)]);
    p.shape(e, &hips, 0.6, M::Enamel, z + 0.01);
    let torso = at(&[
        Vec2::new(-3.6, 24.5),
        Vec2::new(3.6, 24.5),
        Vec2::new(4.2, 29.0),
        Vec2::new(5.8, 35.5),
        Vec2::new(5.0, 37.5),
        Vec2::new(-5.0, 37.5),
        Vec2::new(-5.8, 35.5),
        Vec2::new(-4.2, 29.0),
    ]);
    p.shape(e, &torso, 0.55, M::Enamel, z + 0.02);
    // Joint seams.
    let waist = at(&[Vec2::new(-4.0, 25.0), Vec2::new(4.0, 25.0)]);
    p.part(e, stroke(&waist, |_| 0.5, 1.0, 1.0, |_| 1.0), M::Soot, z + 0.03);
    let neck = at(&[Vec2::new(0.0, 37.0), Vec2::new(0.0, 40.0)]);
    p.part(e, stroke(&neck, |_| 1.8, 1.0, 0.6, |_| 1.0), M::Enamel, z + 0.01);
    // Arms: one hanging stiff, one gone (a dark socket).
    let arm = at(&[Vec2::new(5.4, 36.0), Vec2::new(6.4, 30.0), Vec2::new(6.8, 24.0), Vec2::new(5.5, 19.0)]);
    p.part(e, stroke(&smooth_open(&arm, 4), |t| 2.2 - 0.6 * t, 1.0, 0.55, |_| 1.0), M::Enamel, z + 0.03);
    if f.arm {
        let arm = at(&[Vec2::new(-5.4, 36.0), Vec2::new(-7.0, 31.0), Vec2::new(-9.5, 27.0), Vec2::new(-10.0, 24.5)]);
        p.part(e, stroke(&smooth_open(&arm, 4), |t| 2.2 - 0.6 * t, 1.0, 0.55, |_| 1.0), M::Enamel, z + 0.03);
    } else {
        let socket: Vec<Vec2> = at(&ellipse_pts(1.3, 1.1, 8).into_iter().map(|q| q + Vec2::new(-4.6, 35.6)).collect::<Vec<_>>());
        p.part(e, fill(&socket, |_| 1.0), M::Soot, z + 0.04);
    }
    if let Some((c, r)) = f.head {
        head(p, e, c, r, z + 0.04);
    }
}

/// A blank egg of a head with two dark dents for eyes and a crack.
fn head(p: &mut Props, e: Entity, c: Vec2, rot: f32, z: f32) {
    let egg: Vec<Vec2> = ellipse_pts(3.2, 4.1, 16).into_iter().map(|q| Vec2::new(q.x * (1.0 - 0.12 * (q.y / 4.1)), q.y)).collect();
    let egg = place(&egg, c, rot);
    p.shape(e, &egg, 0.6, M::Enamel, z);
    let eyes: Vec<Vec<Vec2>> = [-1.3, 1.5]
        .into_iter()
        .map(|x| place(&ellipse_pts(0.7, 0.45, 8).into_iter().map(|q| q + Vec2::new(x, 0.6)).collect::<Vec<_>>(), c, rot))
        .collect();
    p.part(e, blobs(&eyes, |_| 1.0), M::Soot, z + 0.01);
    let crack = place(&[Vec2::new(0.4, 4.0), Vec2::new(-0.3, 2.4), Vec2::new(0.6, 1.4), Vec2::new(0.1, -0.4)], c, rot);
    p.part(e, stroke(&crack, |t| 0.5 * (1.0 - t) + 0.15, 1.0, 1.0, |_| 1.0), M::Soot, z + 0.02);
}

/// A mannequin still on its stand, creepily turned toward the room.
fn standing_mannequin(p: &mut Props, at: Vec2) {
    let f = flip(p);
    let e = p.root(at - Vec2::Y * S, f, 0.0);
    let plate = [Vec2::new(-6.0, -1.0), Vec2::new(-5.0, 1.2), Vec2::new(5.0, 1.2), Vec2::new(6.0, -1.0)];
    p.shape(e, &plate, 0.4, M::Iron, 0.0);
    let legs = p.rng.chance(0.6);
    if !legs {
        // Just a torso on a pole.
        line(p, e, Vec2::new(0.0, 1.0), Vec2::new(0.0, 21.0), 1.4, M::Iron, -0.01);
    } else {
        line(p, e, Vec2::new(0.0, 1.0), Vec2::new(-2.2, 8.0), 1.0, M::Iron, -0.01);
    }
    let head = p.rng.chance(0.7).then(|| (Vec2::new(p.rng.range(0.5, 1.5), 44.0), p.rng.range(-0.5, 0.2)));
    let lean = p.rng.range(-0.06, 0.06);
    let fig = Figure { base: Vec2::ZERO, rot: lean, legs, arm: p.rng.chance(0.5), head };
    figure(p, e, &fig, 0.01);
    if fig.head.is_none() {
        // The head at its feet.
        let x = p.rng.range(6.0, 10.0);
        head_on_floor(p, e, Vec2::new(x, 3.0));
    }
}

fn head_on_floor(p: &mut Props, e: Entity, c: Vec2) {
    let rot = p.rng.range(1.2, 2.2) * if p.rng.chance(0.5) { 1.0 } else { -1.0 };
    head(p, e, c, rot, 0.06);
}

/// A mannequin face down on the floor, a leg and the head come off.
fn fallen_mannequin(p: &mut Props, at: Vec2) {
    let f = flip(p);
    let e = p.root(at - Vec2::Y * S * 1.5, f, 0.0);
    // Lying along x: feet at −x, head toward +x, 5 up (its half thickness).
    let base = Vec2::new(-20.0, 5.0);
    let leg_off = p.rng.chance(0.6);
    let fig = Figure { base, rot: -FRAC_PI_2, legs: !leg_off, arm: p.rng.chance(0.4), head: None };
    figure(p, e, &fig, 0.01);
    if leg_off {
        // One leg still on, the other kicked aside at an angle.
        for (b, r) in [(Vec2::new(-20.0, 3.0), -FRAC_PI_2 - 0.05), (Vec2::new(-24.0, 2.0), -FRAC_PI_2 + p.rng.range(0.4, 1.0))] {
            let leg = place(&[Vec2::new(0.0, 0.5), Vec2::new(-0.3, 11.0), Vec2::new(-0.6, 21.0)], b, r);
            p.part(e, stroke(&smooth_open(&leg, 4), |t| 2.6 + 1.6 * t, 1.0, 0.55, |_| 1.0), M::Enamel, 0.0);
        }
    }
    let x = p.rng.range(26.0, 32.0);
    head_on_floor(p, e, Vec2::new(x, 3.6));
    if p.rng.chance(0.5) {
        // A wig, dark and matted.
        let c = Vec2::new(p.rng.range(-4.0, 8.0), 1.0);
        let pts: Vec<Vec2> = rock_pts(&mut p.rng, 3.5, 9).into_iter().map(|q| Vec2::new(q.x * 1.4, q.y * 0.5) + c).collect();
        p.shape(e, &pts, 0.6, M::Soot, 0.05);
    }
}

/// A dead palm (or ficus) in a cracked planter, its fronds hanging down.
fn palm(p: &mut Props, at: Vec2) {
    let f = flip(p);
    let e = p.root(at - Vec2::Y * S * 2.0, f, 0.0);
    let pot = [Vec2::new(-8.0, 0.0), Vec2::new(-10.0, 13.0), Vec2::new(10.0, 13.0), Vec2::new(8.0, 0.0)];
    p.shape(e, &pot, 0.45, M::Concrete, 0.02);
    line(p, e, Vec2::new(-10.5, 13.0), Vec2::new(10.5, 13.0), 1.8, M::Rim, 0.03);
    // A crack down the planter.
    let x = p.rng.range(-5.0, 5.0);
    p.part(e, stroke(&[Vec2::new(x, 13.0), Vec2::new(x + 1.5, 8.0), Vec2::new(x - 0.5, 3.0)], |t| 1.0 - 0.6 * t, 1.0, 1.0, |_| 1.0), M::Soot, 0.04);
    if p.rng.chance(0.6) {
        let h = p.rng.range(30.0, 42.0);
        let lean = p.rng.range(-6.0, 6.0);
        let trunk = [Vec2::new(0.0, 11.0), Vec2::new(lean * 0.3, h * 0.5), Vec2::new(lean, h)];
        p.part(e, stroke(&smooth_open(&trunk, 6), |t| 3.4 - 1.2 * t, 1.0, 0.5, |t| if (t * 14.0).fract() < 0.3 { 0.65 } else { 1.0 }), M::Wood, 0.0);
        let top = Vec2::new(lean, h);
        for i in 0..6 {
            let a = PI * (0.1 + 0.8 * i as f32 / 5.0) + p.rng.range(-0.15, 0.15);
            let d = Vec2::from_angle(a);
            let len = p.rng.range(10.0, 16.0);
            // Fronds: up and out, then drooping hard.
            let pts = [top, top + d * len * 0.4 + Vec2::Y * 2.0, top + Vec2::new(d.x * len * 0.8, -len * 0.5)];
            p.part(e, stroke(&smooth_open(&pts, 5), |t| 2.4 * (1.0 - t) + 0.3, 1.0, 0.5, |t| if (t * 9.0).fract() < 0.4 { 0.6 } else { 1.0 }), M::Wood, 0.01);
        }
    } else {
        // Bare ficus: a few dry sticks.
        for _ in 0..4 {
            let x = p.rng.range(-3.0, 3.0);
            let tip = Vec2::new(x * 3.0 + p.rng.range(-5.0, 5.0), p.rng.range(24.0, 36.0));
            let pts = [Vec2::new(x, 11.0), Vec2::new(x * 1.5, 20.0), tip];
            p.part(e, stroke(&smooth_open(&pts, 5), |t| 1.6 * (1.0 - t) + 0.3, 1.0, 0.5, |_| 1.0), M::Wood, 0.0);
        }
    }
    // Dead leaves on the floor.
    let mut tris = Vec::new();
    for _ in 0..4 {
        let c = Vec2::new(p.rng.range(-16.0, 16.0), 0.5);
        tris.push([c, c + Vec2::new(3.0, 0.8), c + Vec2::new(1.4, -0.6)]);
    }
    p.part(e, triangles(&tris), M::Wood, 0.05);
}

/// A molded plastic chair at `base` turned by `rot` (seat toward +x).
fn chair(p: &mut Props, e: Entity, base: Vec2, rot: f32, z: f32) {
    let pt = |q: Vec2| place(&[q], base, rot)[0];
    line(p, e, pt(Vec2::new(0.0, 0.0)), pt(Vec2::new(1.0, 9.0)), 1.2, M::Steel, z);
    line(p, e, pt(Vec2::new(9.0, 0.0)), pt(Vec2::new(8.0, 9.0)), 1.2, M::Steel, z);
    let seat = place(&[Vec2::new(-0.5, 8.8), Vec2::new(9.5, 8.8), Vec2::new(9.8, 10.4), Vec2::new(-0.5, 10.6)], base, rot);
    p.shape(e, &seat, 0.6, M::Plastic, z + 0.01);
    let back = place(&[Vec2::new(-0.5, 10.0), Vec2::new(-2.0, 19.5), Vec2::new(0.5, 20.0), Vec2::new(1.6, 10.0)], base, rot);
    p.shape(e, &back, 0.6, M::Plastic, z + 0.01);
}

/// A food-court table on its pedestal (or knocked over) with chairs strewn about.
fn food_court(p: &mut Props, at: Vec2) {
    let f = flip(p);
    let e = p.root(at - Vec2::Y * S, f, 0.0);
    if p.rng.chance(0.55) {
        let disk = [Vec2::new(-6.0, 0.0), Vec2::new(-5.0, 1.6), Vec2::new(5.0, 1.6), Vec2::new(6.0, 0.0)];
        p.shape(e, &disk, 0.4, M::Iron, 0.0);
        line(p, e, Vec2::new(0.0, 1.0), Vec2::new(0.0, 17.0), 2.0, M::Iron, 0.0);
        let top = [Vec2::new(-12.0, 16.6), Vec2::new(12.0, 16.6), Vec2::new(12.0, 18.6), Vec2::new(-12.0, 18.6)];
        p.shape(e, &top, 0.5, M::Enamel, 0.01);
        // A chair stacked upside down on top (closing time, years ago).
        if p.rng.chance(0.5) {
            chair(p, e, Vec2::new(-6.0, 38.0), PI, 0.02);
        }
    } else {
        // The table on its side: the top standing on edge, the pedestal sticking out.
        let top = [Vec2::new(-2.0, 0.0), Vec2::new(0.0, 0.0), Vec2::new(0.0, 23.0), Vec2::new(-2.0, 23.0)];
        p.shape(e, &top, 0.5, M::Enamel, 0.01);
        line(p, e, Vec2::new(0.0, 11.5), Vec2::new(15.0, 11.0), 2.0, M::Iron, 0.0);
        line(p, e, Vec2::new(15.0, 5.5), Vec2::new(15.0, 16.5), 1.6, M::Iron, 0.0);
    }
    chair(p, e, Vec2::new(14.0, 0.0), 0.0, 0.03);
    // One on its back.
    let r = FRAC_PI_2 + p.rng.range(-0.05, 0.1);
    chair(p, e, Vec2::new(-12.0, 2.0), r, 0.03);
}

/// Letter strokes in a 1×1 box (x right, y up).
fn glyph(c: u8) -> &'static [&'static [(f32, f32)]] {
    match c {
        b'M' => &[&[(0.0, 0.0), (0.0, 1.0), (0.5, 0.45), (1.0, 1.0), (1.0, 0.0)]],
        b'A' => &[&[(0.0, 0.0), (0.5, 1.0), (1.0, 0.0)], &[(0.22, 0.4), (0.78, 0.4)]],
        b'L' => &[&[(0.0, 1.0), (0.0, 0.0), (0.85, 0.0)]],
        b'S' => &[&[(1.0, 0.9), (0.5, 1.0), (0.0, 0.78), (0.1, 0.55), (0.9, 0.45), (1.0, 0.2), (0.5, 0.0), (0.0, 0.1)]],
        b'O' => &[&[(0.5, 0.0), (0.85, 0.1), (1.0, 0.35), (1.0, 0.65), (0.85, 0.9), (0.5, 1.0), (0.15, 0.9), (0.0, 0.65), (0.0, 0.35), (0.15, 0.1), (0.5, 0.0)]],
        b'P' => &[&[(0.0, 0.0), (0.0, 1.0), (0.75, 1.0), (1.0, 0.78), (0.75, 0.55), (0.0, 0.55)]],
        b'E' => &[&[(0.9, 1.0), (0.0, 1.0), (0.0, 0.0), (0.9, 0.0)], &[(0.0, 0.5), (0.7, 0.5)]],
        _ => &[&[(0.0, 0.0), (1.0, 0.0)]],
    }
}

/// One giant letter of the mall's sign: a thick channel letter with marquee bulbs
/// (most of them dead), drawn in a `size` box from `o`.
fn sign_letter(p: &mut Props, e: Entity, c: u8, o: Vec2, size: Vec2, z: f32) {
    for path in glyph(c) {
        let pts: Vec<Vec2> = path.iter().map(|&(x, y)| o + Vec2::new(x, y) * size).collect();
        let w = size.x * 0.28;
        p.part(e, stroke(&pts, |_| w + 1.6, 0.5, 0.3, |_| 1.0), M::Iron, z);
        p.part(e, stroke(&pts, |_| w, 1.0, 0.7, |_| 1.0), M::Enamel, z + 0.01);
        // Bulbs along the stroke.
        let mut bulbs = Vec::new();
        let mut dead = Vec::new();
        for pair in pts.windows(2) {
            let n = ((pair[1] - pair[0]).length() / 3.5).max(1.0) as usize;
            for i in 0..n {
                let q = pair[0].lerp(pair[1], (i as f32 + 0.5) / n as f32);
                let b: Vec<Vec2> = ellipse_pts(0.7, 0.7, 6).into_iter().map(|d| d + q).collect();
                if p.rng.chance(0.25) { bulbs.push(b) } else { dead.push(b) }
            }
        }
        if !bulbs.is_empty() {
            p.part(e, blobs(&bulbs, |_| 1.0), M::Rim, z + 0.02);
        }
        if !dead.is_empty() {
            p.part(e, blobs(&dead, |_| 0.7), M::Soot, z + 0.02);
        }
    }
}

const WORD: &[u8] = b"MALLSOPE";

/// A sign letter fallen off the facade, stuck in the floor at an angle.
fn letter(p: &mut Props, at: Vec2) {
    let c = WORD[p.rng.below(WORD.len())];
    let angle = p.rng.range(-0.45, 0.45);
    let e = p.root(at - Vec2::Y * S * 3.0, 1.0, angle);
    let size = Vec2::new(p.rng.range(16.0, 20.0), p.rng.range(20.0, 24.0));
    sign_letter(p, e, c, Vec2::new(-size.x / 2.0, 1.0), size, 0.0);
    // The mounting bracket, wrenched.
    let x = p.rng.range(-4.0, 4.0);
    p.bar(e, &[Vec2::new(x, size.y), Vec2::new(x + 2.0, size.y + 4.0), Vec2::new(x + 6.0, size.y + 5.0)], 1.2, M::Rust, -0.01);
}

/// A heap of display-window glass, a bent strip of frame in it.
fn glass(p: &mut Props, at: Vec2) {
    let e = p.root(at - Vec2::Y * S * 0.8, 1.0, 0.0);
    let n = 7 + p.rng.below(5);
    shards(p, e, Vec2::new(0.0, 0.5), 12.0, n, 0.02);
    // A few big pieces standing up.
    let mut tris = Vec::new();
    for _ in 0..2 {
        let x = p.rng.range(-8.0, 8.0);
        let h = p.rng.range(5.0, 9.0);
        tris.push([Vec2::new(x - 2.5, 0.0), Vec2::new(x + 2.0, 0.0), Vec2::new(x + p.rng.range(-2.0, 2.0), h)]);
    }
    p.part_blue(e, triangles(&tris), M::Rim, 0.01, Some(GLASS_BLUE));
    if p.rng.chance(0.6) {
        let y = p.rng.range(1.0, 3.0);
        p.bar(e, &[Vec2::new(-12.0, y), Vec2::new(-2.0, y + 1.5), Vec2::new(4.0, y + 4.0), Vec2::new(9.0, y + 1.0)], 1.4, M::Steel, 0.0);
    }
}

/// A section of escalator, fallen and propped on a heap of rubble: the side panel,
/// the stair treads along its top, the rubber handrail torn and hanging.
fn escalator(p: &mut Props, at: Vec2) {
    let f = flip(p);
    let e = p.root(at - Vec2::Y * S * 2.0, f, 0.0);
    let a = p.rng.range(0.42, 0.55);
    let (len, h) = (88.0, 9.0);
    let d = Vec2::from_angle(a);
    let n = d.perp();
    let start = Vec2::new(-44.0, 0.0);
    let end = start + d * len;
    // The truss under it, the broken upper end jagged.
    let side = [start - n * 1.0, start + d * (len - 4.0) - n * 1.0, end + n * 0.5 - d * 1.0, end + n * (h * 0.6), end + n * h - d * 2.0, start + n * h];
    p.shape(e, &side, 0.45, M::Steel, 0.02);
    // Panel seams.
    for k in 1..6 {
        let q = start + d * len * k as f32 / 6.0;
        line(p, e, q - n * 0.5, q + n * (h - 0.5), 0.5, M::Iron, 0.03);
    }
    // Treads: a sawtooth on top.
    let mut steps = Vec::new();
    let mut s = 3.0;
    while s < len - 6.0 {
        let q = start + d * s + n * h;
        steps.push([q, q + Vec2::new(5.0, 0.0), q + Vec2::new(0.0, 3.0)]);
        s += 5.0 / a.cos();
    }
    p.part(e, triangles(&steps), M::Iron, 0.01);
    // Balustrade glass: a few panes left, smashed.
    let mut tris = Vec::new();
    for k in 0..4 {
        if !p.rng.chance(0.6) {
            continue;
        }
        let q = start + d * (8.0 + k as f32 * 20.0) + n * h;
        let hgt = p.rng.range(6.0, 12.0);
        tris.push([q, q + d * 14.0, q + d * p.rng.range(2.0, 12.0) + Vec2::Y * hgt]);
    }
    p.part(e, triangles(&tris), M::Rim, -0.01);
    // Handrail: follows the top, then hangs off the broken end.
    let rail_y = h + 13.0;
    let r0 = start + n * rail_y;
    let r1 = start + d * (len - 10.0) + n * rail_y;
    let drop = p.rng.range(18.0, 30.0);
    let rail = [r0 - d * 4.0 - n * 6.0, r0, r1, r1 + d * 6.0 - n * 2.0, r1 + d * 8.0 - Vec2::Y * drop];
    p.part(e, stroke(&smooth_open(&rail, 6), |_| 2.4, 0.8, 0.4, |_| 1.0), M::Rubber, 0.0);
    // Rubble heap under the high end.
    let base = Vec2::new(start.x + d.x * len * 0.85, 0.0);
    for i in 0..4 {
        let r = p.rng.range(5.0, 9.0);
        let c = base + Vec2::new(p.rng.range(-8.0, 8.0), r * 0.4 + i as f32 * 0.5);
        let pts: Vec<Vec2> = rock_pts(&mut p.rng, r, 7).into_iter().map(|q| q + c).collect();
        p.shape(e, &pts, 0.45, M::Concrete, 0.04 + 0.01 * i as f32);
    }
    // A bent steel prop from the rubble up to the truss so it doesn't float.
    let under = start + d * len * 0.85 - n * 1.0;
    p.bar(e, &[Vec2::new(base.x - 6.0, 2.0), Vec2::new(base.x - 3.0, under.y * 0.5), under - d * 3.0], 2.2, M::Iron, 0.03);
    p.bar(e, &[Vec2::new(base.x + 6.0, 2.0), Vec2::new(base.x + 4.0, under.y * 0.55), under + d * 2.0], 2.2, M::Iron, 0.03);
    line(p, e, Vec2::new(base.x - 4.5, under.y * 0.5), Vec2::new(base.x + 4.5, under.y * 0.55), 1.2, M::Iron, 0.03);
}

// --- Edges, walls, ceilings ---

/// A mezzanine handrail: chrome posts and rail over broken glass panels,
/// the end bent out over the drop.
fn handrail(p: &mut Props, e: Entity) {
    let h = 20.0;
    let posts = [-34.0, -18.0, -3.0];
    for (i, &x) in posts.iter().enumerate() {
        if i == 0 && p.rng.chance(0.3) {
            continue;
        }
        line(p, e, Vec2::new(x, -2.0), Vec2::new(x, h), 1.6, M::Steel, 0.02);
    }
    // Glass between the posts: shattered to jagged stumps.
    let mut tris = Vec::new();
    for w in posts.windows(2) {
        let (a, b) = (w[0] + 1.0, w[1] - 1.0);
        let mut x = a;
        while x < b {
            let nx = (x + p.rng.range(2.5, 5.0)).min(b);
            let top = if p.rng.chance(0.2) { h - 3.0 } else { p.rng.range(1.0, 8.0) };
            tris.push([Vec2::new(x, 0.0), Vec2::new(nx, 0.0), Vec2::new(p.rng.range(x, nx), top)]);
            x = nx;
        }
    }
    p.part(e, triangles(&tris), M::Rim, 0.01);
    // The rail: straight, then wrenched out and down over the edge.
    let out = p.rng.range(6.0, 14.0);
    let sag = p.rng.range(8.0, 26.0);
    let rail = [Vec2::new(-36.0, h), Vec2::new(-12.0, h + 0.5), Vec2::new(-3.0, h), Vec2::new(out * 0.6, h - sag * 0.3), Vec2::new(out, h - sag)];
    p.part(e, stroke(&smooth_open(&rail, 6), |_| 2.0, 1.0, 0.45, |_| 1.0), M::Steel, 0.03);
    // Glass falling off the edge.
    shards(p, e, Vec2::new(2.0, -6.0), 3.0, 3, 0.02);
}

/// A fluorescent tube (`angle` from the horizontal), cracked, its end dark.
fn tube(p: &mut Props, e: Entity, from: Vec2, angle: f32, z: f32) {
    let len = p.rng.range(18.0, 28.0);
    let d = Vec2::from_angle(angle);
    let wire = [from, from + Vec2::new(1.0, -4.0), from + Vec2::new(2.0, -7.0)];
    p.part(e, stroke(&smooth_open(&wire, 4), |_| 0.7, 1.0, 1.0, |_| 1.0), M::Soot, z);
    let a = from + Vec2::new(2.0, -7.0);
    let broken = p.rng.range(0.5, 1.0);
    p.part(e, stroke(&[a, a + d * len * broken], |_| 2.2, 1.0, 0.75, |_| 1.0), M::Enamel, z + 0.01);
    line(p, e, a - d * 1.2, a + d * 1.2, 2.8, M::Iron, z + 0.02);
    if broken > 0.95 {
        let b = a + d * len;
        line(p, e, b - d * 1.2, b + d * 1.2, 2.8, M::Iron, z + 0.02);
    }
}

/// A storefront on the wall's face (drawn over the terrain): a shutter rolled
/// down part way under its housing, the dark smashed shop window below, a sign band above.
fn storefront(p: &mut Props, e: Entity, half: Vec2) {
    let (l, r, b, t) = (-half.x, half.x, -half.y, half.y);
    let hole = [Vec2::new(l, b), Vec2::new(r, b), Vec2::new(r, t), Vec2::new(l, t)];
    p.part(e, fill(&hole, |q| 0.5 + 0.5 * ((q.y - b) / (t - b))), M::Soot, OVER);
    // Pillars and sign band.
    for x in [l - 1.5, r + 1.5] {
        line(p, e, Vec2::new(x, b - 1.0), Vec2::new(x, t + 1.0), 3.0, M::Concrete, OVER + 0.04);
    }
    let band = [Vec2::new(l - 3.0, t), Vec2::new(r + 3.0, t), Vec2::new(r + 3.0, t + 6.0), Vec2::new(l - 3.0, t + 6.0)];
    p.shape(e, &band, 0.55, M::Iron, OVER + 0.04);
    // What's left of the shop's name: a few small letters, some hanging askew.
    let n = 4 + p.rng.below(3);
    let w = (r - l) / n as f32;
    for i in 0..n {
        if !p.rng.chance(0.55) {
            continue;
        }
        let c = WORD[p.rng.below(WORD.len())];
        let o = Vec2::new(l + w * i as f32 + w * 0.15, t + 1.2);
        sign_letter(p, e, c, o, Vec2::new(w * 0.7, 3.6), OVER + 0.05);
    }
    // The shutter: housing, then slats down to `down`, a bottom bar, bent at one corner.
    let down = t - (t - b) * p.rng.range(0.25, 0.8);
    let housing = [Vec2::new(l, t - 3.5), Vec2::new(r, t - 3.5), Vec2::new(r, t), Vec2::new(l, t)];
    p.shape(e, &housing, 0.6, M::Steel, OVER + 0.03);
    let mut y = t - 3.5;
    let mut k = 0;
    while y > down {
        let ny = (y - 1.6).max(down);
        let slat = [Vec2::new(l + 0.5, ny), Vec2::new(r - 0.5, ny), Vec2::new(r - 0.5, y), Vec2::new(l + 0.5, y)];
        p.part(e, fill(&slat, |q| if q.y > (y + ny) / 2.0 { 0.95 } else { 0.55 }), M::Steel, OVER + 0.01 + 0.0001 * k as f32);
        y = ny;
        k += 1;
    }
    let bend = p.rng.range(-4.0, 1.0);
    p.part(e, stroke(&[Vec2::new(l + 0.5, down), Vec2::new(r * 0.3, down), Vec2::new(r - 0.5, down + bend)], |_| 1.6, 1.0, 0.5, |_| 1.0), M::Iron, OVER + 0.02);
    // Graffiti-like dents: dark scratches.
    for _ in 0..2 {
        let a = Vec2::new(p.rng.range(l + 2.0, r - 2.0), p.rng.range(down + 1.0, t - 4.0));
        let q = a + Vec2::new(p.rng.range(-5.0, 5.0), p.rng.range(-2.0, 2.0));
        line(p, e, a, q, 0.5, M::Soot, OVER + 0.025);
    }
    // Glass teeth in the window frame below.
    if down - b > 4.0 {
        let mut tris = Vec::new();
        for _ in 0..3 {
            let x = p.rng.range(l + 1.0, r - 3.0);
            tris.push([Vec2::new(x, b + 0.5), Vec2::new(x + p.rng.range(2.0, 4.0), b + 0.5), Vec2::new(x + 1.0, b + p.rng.range(3.0, (down - b).min(10.0)))]);
        }
        p.part(e, triangles(&tris), M::Rim, OVER + 0.02);
        line(p, e, Vec2::new(l, b + 0.3), Vec2::new(r, b + 0.3), 1.2, M::Steel, OVER + 0.03);
    }
    // Lit sill.
    line(p, e, Vec2::new(l - 3.0, b - 1.2), Vec2::new(r + 3.0, b - 1.2), 2.4, M::Rim, OVER + 0.05);
}

/// A shop sign hanging off the wall from one bracket.
fn hanging_sign(p: &mut Props, e: Entity) {
    let y = p.rng.range(-6.0, 10.0);
    // The bracket arm out of the wall.
    line(p, e, Vec2::new(-2.0, y), Vec2::new(14.0, y), 1.6, M::Iron, 0.02);
    line(p, e, Vec2::new(-2.0, y - 6.0), Vec2::new(10.0, y), 1.0, M::Iron, 0.02);
    let (w, h) = (p.rng.range(14.0, 20.0), p.rng.range(8.0, 11.0));
    // Only the inner chain holds: the board swings down from it.
    let pivot = Vec2::new(3.0, y - 3.0);
    line(p, e, Vec2::new(3.0, y), pivot, 0.6, M::Soot, 0.02);
    let a = -p.rng.range(0.7, 1.2);
    let board = place(&[Vec2::ZERO, Vec2::new(w, 0.0), Vec2::new(w, -h), Vec2::new(0.0, -h)], pivot, a);
    p.shape(e, &board, 0.5, M::Enamel, 0.03);
    // The other chain, snapped and dangling.
    let y2 = y - p.rng.range(3.0, 7.0);
    line(p, e, Vec2::new(13.0, y), Vec2::new(13.5, y2), 0.6, M::Soot, 0.02);
    // A letter or two still on the board.
    let c = WORD[p.rng.below(WORD.len())];
    let o = place(&[Vec2::new(w * 0.3, -h * 0.8)], pivot, a)[0];
    sign_letter(p, e, c, o, Vec2::new(4.0, 5.0), 0.04);
}

/// A fluorescent fixture hanging from the ceiling, one wire snapped.
fn fixture(p: &mut Props, e: Entity) {
    let len = p.rng.range(30.0, 40.0);
    let drop = p.rng.range(10.0, 24.0);
    let hang = p.rng.range(4.0, 12.0);
    let a = Vec2::new(-len / 2.0, -drop);
    // The fixture pivots down from the wire that held: steep if the other snapped.
    let snapped = p.rng.chance(0.65);
    let tilt = if snapped { -p.rng.range(0.7, 1.2) } else { p.rng.range(-0.15, 0.15) };
    let d = Vec2::from_angle(tilt);
    let b = a + d * len;
    p.part(e, stroke(&[Vec2::new(-len / 2.0, 2.0), a], |_| 0.9, 1.0, 0.7, |_| 1.0), M::Steel, 0.0);
    if snapped {
        p.part(e, stroke(&[Vec2::new(len / 2.0, 2.0), Vec2::new(len / 2.0 + 1.0, -hang)], |_| 0.9, 1.0, 0.7, |_| 1.0), M::Steel, 0.0);
    } else {
        p.part(e, stroke(&[Vec2::new(len / 2.0, 2.0), b], |_| 0.9, 1.0, 0.7, |_| 1.0), M::Steel, 0.0);
    }
    // Housing and tubes (one tube gone or hanging loose).
    let n = d.perp();
    let housing = [a + n * 1.5, b + n * 1.5, b - n * 1.0, a - n * 1.0];
    p.shape(e, &housing, 0.5, M::Steel, 0.01);
    let t0 = a - n * 1.8 + d * 1.5;
    let t1 = b - n * 1.8 - d * 1.5;
    p.part(e, stroke(&[t0, t1], |_| 2.2, 1.0, 0.8, |_| 1.0), M::Rim, 0.02);
    if p.rng.chance(0.5) {
        let end = t0 + Vec2::new(p.rng.range(-3.0, 3.0), -p.rng.range(10.0, 16.0));
        p.part(e, stroke(&[t0 - n * 1.6, end], |_| 2.2, 1.0, 0.8, |_| 1.0), M::Rim, 0.03);
    }
    // Glass of the diffuser in pieces below.
    if p.rng.chance(0.5) {
        let c = b.min(a) + Vec2::new(0.0, -6.0);
        shards(p, e, c, 4.0, 3, 0.03);
    }
}

/// Bits of drop-ceiling grid hanging down, a panel or two still caught in it.
fn ceiling_grid(p: &mut Props, e: Entity) {
    let n = 2 + p.rng.below(3);
    for i in 0..n {
        let x = -20.0 + 40.0 * i as f32 / n as f32 + p.rng.range(-3.0, 3.0);
        let a = -FRAC_PI_2 + p.rng.range(-0.9, 0.9);
        let len = p.rng.range(10.0, 26.0);
        let from = Vec2::new(x, 1.0);
        let to = from + Vec2::from_angle(a) * len;
        line(p, e, from, to, 1.0, M::Steel, 0.01);
        if p.rng.chance(0.45) {
            // A panel hanging off the bar by a corner.
            let s = p.rng.range(7.0, 10.0);
            let r = a + p.rng.range(-0.4, 0.4);
            let panel = place(&[Vec2::ZERO, Vec2::new(s, 0.0), Vec2::new(s, 1.4), Vec2::new(0.0, 1.4)], to, r);
            p.shape(e, &panel, 0.6, M::Enamel, 0.02);
        }
    }
    // A cross bar still spanning two of them.
    let y = -p.rng.range(3.0, 8.0);
    let y2 = y - p.rng.range(-4.0, 4.0);
    line(p, e, Vec2::new(-22.0, y), Vec2::new(22.0, y2), 1.0, M::Steel, 0.0);
}
