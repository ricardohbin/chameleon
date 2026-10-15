//! Props of "region: sinkhole sunken street" (`Style::Sinkhole`): the street
//! that fell in, still recognizable. Outside: street lamps and traffic lights
//! leaning at drunk angles, hydrants, bent road signs, jersey barriers, cars
//! and a bus swallowed nose-first, broken asphalt with its lane paint, chunks
//! of shop facade still standing. On the walls: storefronts (shop windows,
//! doorways, rolled shutters, signboards, awnings), projecting shop signs,
//! severed pipes and cable bundles. Indoors (the buildings that went down
//! with the street): desks, filing cabinets, office chairs, shop counters and
//! shelving, loose paper, fluorescent lights and blinds hanging from the
//! ceiling. Under overhangs: a traffic light hanging by its wires, cables.

use super::*;

pub(super) const HOOKS: Hooks = Hooks { spacing, ground, edge, wall, ceiling };

/// (gap between props of the same kind, chance a free spot gets one), in prop units.
fn spacing(kind: Kind) -> (f32, f32) {
    match kind {
        Kind::Ground => (55.0, 0.8),
        Kind::Edge => (35.0, 0.85),
        Kind::Wall => (45.0, 0.75),
        Kind::Ceiling => (45.0, 0.65),
    }
}

#[derive(Clone, Copy, PartialEq)]
enum Street {
    Lamp,
    Signal,
    Hydrant,
    Car,
    Bus,
    Sign,
    Barrier,
    Chunks,
    Cone,
    Facade,
    Desk,
    Cabinet,
    Chair,
    Counter,
    Shelf,
    Papers,
}

impl Street {
    /// (half width, height) in prop units.
    fn size(self) -> (f32, f32) {
        match self {
            Street::Lamp => (6.0, 110.0),
            Street::Signal => (8.0, 90.0),
            Street::Hydrant => (7.0, 20.0),
            Street::Car => (30.0, 30.0),
            Street::Bus => (48.0, 50.0),
            Street::Sign => (8.0, 55.0),
            Street::Barrier => (18.0, 16.0),
            Street::Chunks => (16.0, 14.0),
            Street::Cone => (5.0, 12.0),
            Street::Facade => (22.0, 50.0),
            Street::Desk => (16.0, 30.0),
            Street::Cabinet => (6.0, 30.0),
            Street::Chair => (8.0, 22.0),
            Street::Counter => (22.0, 22.0),
            Street::Shelf => (12.0, 40.0),
            Street::Papers => (12.0, 6.0),
        }
    }
}

fn sign_of(p: &mut Props) -> f32 {
    if p.rng.chance(0.5) { 1.0 } else { -1.0 }
}

fn pick(p: &mut Props, table: &[(Street, f32)]) -> Street {
    let total: f32 = table.iter().map(|e| e.1).sum();
    let mut r = p.rng.range(0.0, total);
    table
        .iter()
        .find(|e| {
            r -= e.1;
            r <= 0.0
        })
        .unwrap_or(&table[0])
        .0
}

/// A top surface with headroom at `cell` (`x` = its world x). True if something was placed.
fn ground(p: &mut Props, cell: IVec2, x: f32) -> bool {
    use Street::*;
    // Inside a building (passable structure behind): what survived of its shops and offices.
    let indoor = p.fit(cell, 4.0, 20.0, true).is_some_and(|f| f.2);
    let first = if indoor {
        pick(p, &[(Desk, 2.5), (Cabinet, 2.0), (Chair, 1.5), (Counter, 2.0), (Shelf, 2.0), (Car, 0.6), (Chunks, 0.5)])
    } else {
        pick(p, &[(Lamp, 2.0), (Signal, 1.2), (Hydrant, 1.4), (Car, 1.8), (Bus, 0.6), (Sign, 1.3), (Barrier, 1.0), (Chunks, 1.5), (Facade, 2.0), (Counter, 0.4), (Cabinet, 0.4)])
    };
    let small = if indoor { [Chair, Papers] } else { [Chunks, Cone] };
    for g in [first, small[0], small[1]] {
        let (half, head) = g.size();
        let tilt = matches!(g, Car | Bus | Barrier | Chunks | Cone | Papers);
        let Some((y, angle, indoor)) = p.fit(cell, half, head, tilt) else { continue };
        let at = Vec2::new(x, y);
        p.tilt = angle;
        p.depth = if indoor { INDOOR } else { BACK };
        match g {
            Lamp => street_lamp(p, at),
            Signal => traffic_light(p, at),
            Hydrant => hydrant(p, at),
            Car => car(p, at),
            Bus => bus(p, at),
            Sign => road_sign(p, at),
            Barrier => barrier(p, at),
            Chunks => asphalt_chunks(p, at),
            Cone => cone(p, at),
            Facade => facade_chunk(p, at),
            Desk => desk(p, at),
            Cabinet => filing_cabinet(p, at),
            Chair => office_chair(p, at),
            Counter => shop_counter(p, at),
            Shelf => shop_shelf(p, at),
            Papers => papers(p, at),
        }
        p.tilt = 0.0;
        p.depth = BACK;
        return true;
    }
    false
}

/// Where the ground ends at a drop (`flip` = the open side).
fn edge(p: &mut Props, at: Vec2, flip: f32) -> bool {
    if p.rng.chance(0.4) {
        // A shop front in the building face under the broken edge.
        let half = Vec2::new(p.rng.range(12.0, 16.0), p.rng.range(10.0, 13.0));
        let c = at + Vec2::new(-flip * (half.x * S + PX * 2.0), -(half.y + 14.0) * S);
        if p.inside(c, (half + Vec2::Y * 8.0) * S) {
            let e = p.root(c, flip, 0.0);
            storefront(p, e, half, OVER);
        }
    }
    let e = p.root(at, flip, 0.0);
    asphalt_lip(p, e);
    if p.rng.chance(0.5) {
        let r = p.rng.range(3.0, 5.5);
        let y = -p.rng.range(12.0, 20.0);
        let k1 = p.rng.range(-0.2, 0.15);
        severed_pipe(p, e, Vec2::new(-4.0, y), r, k1);
    }
    if p.rng.chance(0.55) {
        let n = 1 + p.rng.below(3);
        let k1 = p.rng.range(8.0, 14.0);
        p.cables(e, Vec2::new(-2.0, -k1), n, 0.0);
    }
    true
}

/// A straight vertical face (`flip` = the open side).
fn wall(p: &mut Props, at: Vec2, flip: f32) -> bool {
    let roll = p.rng.next();
    if roll < 0.4 {
        // A storefront that went down with its building, still in the wall's face.
        let half = Vec2::new(p.rng.range(12.0, 17.0), p.rng.range(10.0, 14.0));
        let c = at + Vec2::new(-flip * (half.x * S + PX * 2.0), 0.0);
        if p.inside(c, (half + Vec2::Y * 8.0) * S) {
            let e = p.root(c, flip, 0.0);
            storefront(p, e, half, OVER);
            return true;
        }
    }
    let e = p.root(at, flip, 0.0);
    if roll < 0.15 && p.rng.chance(0.5) {
        blade_sign(p, e);
    } else if roll < 0.4 {
        let k1 = p.rng.range(-4.0, 10.0);
        awning(p, e, Vec2::new(-4.0, k1), 22.0, 0.0);
    } else if roll < 0.62 {
        // A utility line cut by the collapse, sometimes two stacked.
        let r = p.rng.range(3.5, 7.0);
        let k1 = p.rng.range(-8.0, 8.0);
        let k2 = p.rng.range(-0.15, 0.1);
        severed_pipe(p, e, Vec2::new(-4.0, k1), r, k2);
        if p.rng.chance(0.4) {
            let r2 = p.rng.range(2.5, 4.0);
            let k1 = p.rng.range(-20.0, -12.0);
            let k2 = p.rng.range(-0.3, 0.1);
            severed_pipe(p, e, Vec2::new(-4.0, k1), r2, k2);
        }
    } else if roll < 0.75 {
        // A cable bundle spilling out of a broken duct.
        let k1 = p.rng.range(-6.0, 8.0);
        cable_spill(p, e, Vec2::new(-2.0, k1));
    } else if roll < 0.87 {
        blade_sign(p, e);
    } else {
        // A slab of road jammed in the wall, sticking out.
        let a = p.rng.range(-0.5, 0.3);
        let k1 = p.rng.range(-20.0, 20.0);
        let e2 = p.root(at + Vec2::Y * k1, flip, a * flip);
        jammed_slab(p, e2);
    }
    true
}

/// The underside of a thick overhang.
fn ceiling(p: &mut Props, at: Vec2) -> bool {
    // Inside a building (`visit` set the indoor depth): offices and shops
    // still have their lights hanging from the ceiling.
    if p.depth == INDOOR {
        let e = p.root(at, 1.0, 0.0);
        if p.rng.chance(0.3) {
            blinds(p, e);
        } else {
            ceiling_light(p, e);
        }
        return true;
    }
    let k1 = sign_of(p);
    let e = p.root(at, k1, 0.0);
    let roll = p.rng.next();
    if roll < 0.3 {
        hanging_signal(p, e);
    } else if roll < 0.6 {
        // A broken pipe drooping out of the overhang.
        let r = p.rng.range(2.5, 4.0);
        let len = p.rng.range(14.0, 30.0);
        let sway = p.rng.range(-8.0, 8.0);
        let pts = [Vec2::new(0.0, 4.0), Vec2::new(sway * 0.2, -len * 0.5), Vec2::new(sway, -len)];
        p.bar(e, &pts, 2.0 * r, M::Rust, 0.0);
        let end = pts[2];
        broken_mouth(p, e, end, Vec2::new(sway, -len) - Vec2::new(sway * 0.2, -len * 0.5), r, M::Rust);
        let n = 1 + p.rng.below(2);
        let k1 = p.rng.range(-8.0, 8.0);
        p.cables(e, Vec2::new(k1, 2.0), n, -1.0);
    } else {
        let n = 2 + p.rng.below(3);
        let k1 = p.rng.range(-6.0, 6.0);
        p.cables(e, Vec2::new(k1, 2.0), n, -1.0);
        if p.rng.chance(0.4) {
            // A clump of asphalt still hanging on the wires.
            let c = Vec2::new(p.rng.range(-4.0, 4.0), -p.rng.range(20.0, 34.0));
            let k1 = p.rng.range(3.5, 5.0);
            chunk(p, e, c, k1, 0.06);
        }
    }
    true
}

// --- Drawing ---

/// A jagged lump of asphalt: dark, with a lit broken top and maybe lane paint.
fn chunk(p: &mut Props, e: Entity, c: Vec2, r: f32, z: f32) {
    let pts: Vec<Vec2> = rock_pts(&mut p.rng, r, 6).into_iter().map(|q| Vec2::new(q.x * 1.4, q.y * 0.8) + c).collect();
    p.shape(e, &pts, 0.5, M::Iron, z);
    // Top face: road surface, paint dash on some.
    let top = [c + Vec2::new(-r * 1.1, r * 0.55), c + Vec2::new(r * 1.1, r * 0.5)];
    p.part(e, stroke(&top, |_| 1.0, 1.0, 1.0, |_| 1.0), M::Steel, z + 0.005);
    if p.rng.chance(0.5) {
        let dash = [c + Vec2::new(-r * 0.5, r * 0.45), c + Vec2::new(r * 0.4, r * 0.42)];
        p.part(e, stroke(&dash, |_| 1.2, 1.0, 1.0, |_| 1.0), M::Enamel, z + 0.01);
    }
}

fn asphalt_chunks(p: &mut Props, at: Vec2) {
    let e = p.root(at - Vec2::Y * S * 2.0, 1.0, 0.0);
    let n = 2 + p.rng.below(3);
    for i in 0..n {
        let r = p.rng.range(3.5, 7.0);
        let c = Vec2::new(p.rng.range(-12.0, 12.0), r * 0.4);
        chunk(p, e, c, r, 0.02 * i as f32);
    }
    if p.rng.chance(0.45) {
        // A length of curb.
        let (x, w) = (p.rng.range(-10.0, 4.0), p.rng.range(12.0, 18.0));
        let a = p.rng.range(-0.15, 0.25);
        let d = Vec2::from_angle(a);
        let n = d.perp();
        let curb = [Vec2::new(x, -2.0), Vec2::new(x, -2.0) + n * 5.0, Vec2::new(x, -2.0) + n * 5.0 + d * w, Vec2::new(x, -2.0) + d * w];
        p.shape(e, &curb, 0.5, M::Concrete, -0.01);
        p.part(e, stroke(&[curb[1], curb[2]], |_| 1.4, 1.0, 1.0, |_| 1.0), M::Rim, -0.005);
    }
}

/// A modern street lamp, its pole leaning and the head drooping.
fn street_lamp(p: &mut Props, at: Vec2) {
    let lean = p.rng.range(0.08, 0.45) * sign_of(p);
    let flip = sign_of(p);
    let e = p.root(at - Vec2::Y * S * 4.0, flip, lean * flip);
    let h = p.rng.range(80.0, 105.0);
    // Base plinth, cracked.
    let base = [Vec2::new(-5.0, -2.0), Vec2::new(-4.0, 8.0), Vec2::new(4.0, 8.0), Vec2::new(5.0, -2.0)];
    p.shape(e, &base, 0.4, M::Concrete, 0.01);
    let bend = p.rng.range(-6.0, 4.0);
    let pole = [Vec2::new(0.0, 4.0), Vec2::new(0.0, h * 0.5), Vec2::new(bend * 0.4, h * 0.85), Vec2::new(bend, h)];
    p.part(e, stroke(&smooth_open(&pole, 8), |t| 3.4 - 1.4 * t, 1.0, 0.5, |_| 1.0), M::Steel, 0.0);
    // Cobra-head arm, out over the road.
    let top = Vec2::new(bend, h);
    let reach = p.rng.range(18.0, 26.0);
    let droop = p.rng.range(0.0, 14.0);
    let arm = [top, top + Vec2::new(reach * 0.4, 5.0), top + Vec2::new(reach, 3.0 - droop)];
    p.bar(e, &arm, 1.8, M::Steel, 0.0);
    let tip = arm[2];
    let head = [tip + Vec2::new(-3.0, 1.0), tip + Vec2::new(4.0, 2.5), tip + Vec2::new(10.0, 0.5), tip + Vec2::new(9.0, -2.0), tip + Vec2::new(0.0, -2.0)];
    p.shape(e, &smooth_closed(&head, 3), 0.55, M::Steel, 0.01);
    // Smashed lens.
    let lens = [tip + Vec2::new(1.0, -2.2), tip + Vec2::new(8.0, -2.2)];
    p.part(e, stroke(&lens, |_| 1.0, 1.0, 1.0, |t| if (t * 5.0) as i32 % 2 == 0 { 1.0 } else { 0.3 }), M::Rim, 0.02);
    // Access hatch hanging open, a wire out of it.
    let hatch = [Vec2::new(1.6, 14.0), Vec2::new(4.5, 13.0), Vec2::new(5.0, 7.0), Vec2::new(2.0, 8.0)];
    p.shape(e, &hatch, 0.6, M::Steel, 0.02);
    p.part(e, stroke(&smooth_open(&[Vec2::new(1.0, 12.0), Vec2::new(4.0, 9.0), Vec2::new(6.0, 2.0)], 5), |_| 0.8, 1.0, 1.0, |_| 1.0), M::Soot, 0.03);
}

/// A traffic signal: the pole, a mast arm, and the three-lamp head, all dead.
fn traffic_light(p: &mut Props, at: Vec2) {
    let lean = p.rng.range(0.05, 0.4) * sign_of(p);
    let flip = sign_of(p);
    let e = p.root(at - Vec2::Y * S * 4.0, flip, lean * flip);
    let h = p.rng.range(55.0, 75.0);
    p.part(e, stroke(&[Vec2::new(0.0, -2.0), Vec2::new(0.0, h)], |t| 3.6 - 0.8 * t, 1.0, 0.5, |_| 1.0), M::Iron, 0.0);
    // Push-button box.
    let bx = [Vec2::new(1.5, 22.0), Vec2::new(1.5, 28.0), Vec2::new(5.0, 28.0), Vec2::new(5.0, 22.0)];
    p.shape(e, &bx, 0.6, M::Iron, 0.01);
    let arm_len = p.rng.range(20.0, 34.0);
    let sag = p.rng.range(0.0, 10.0);
    let tip = Vec2::new(arm_len, h - sag);
    p.bar(e, &[Vec2::new(0.0, h), Vec2::new(arm_len * 0.5, h - sag * 0.3), tip], 2.0, M::Iron, 0.0);
    // Head hangs from the arm's tip (swung on its bracket).
    let swing = p.rng.range(-0.35, 0.35);
    signal_head(p, e, tip, swing, 0.02);
}

/// A three-lamp signal head hanging from `top`, rotated by `swing`.
fn signal_head(p: &mut Props, e: Entity, top: Vec2, swing: f32, z: f32) {
    let rot = |q: Vec2| top + Vec2::from_angle(swing).rotate(q);
    let (w, h) = (6.5, 20.0);
    let body: Vec<Vec2> = [Vec2::new(-w / 2.0, -2.0), Vec2::new(w / 2.0, -2.0), Vec2::new(w / 2.0, -h - 2.0), Vec2::new(-w / 2.0, -h - 2.0)].map(rot).to_vec();
    p.shape(e, &body, 0.5, M::Iron, z);
    let dead = p.rng.below(4);
    for i in 0..3 {
        let c = rot(Vec2::new(0.0, -5.5 - i as f32 * 6.0));
        // Visors over each lamp.
        let visor = [rot(Vec2::new(-3.2, -2.6 - i as f32 * 6.0)), rot(Vec2::new(4.6, -3.4 - i as f32 * 6.0))];
        p.part(e, stroke(&visor, |_| 1.0, 1.0, 1.0, |_| 1.0), M::Soot, z + 0.02);
        let lamp: Vec<Vec2> = ellipse_pts(2.2, 2.2, 10).into_iter().map(|q| q + c).collect();
        p.part(e, fill(&lamp, |_| if i == dead { 0.35 } else { 1.0 }), if i == dead { M::Soot } else { M::Steel }, z + 0.01);
    }
}

/// A traffic light dangling from the overhang on its own cables.
fn hanging_signal(p: &mut Props, e: Entity) {
    let len = p.rng.range(16.0, 34.0);
    let sway = p.rng.range(-6.0, 6.0);
    let end = Vec2::new(sway, -len);
    for dx in [-1.2, 1.2] {
        let pts = smooth_open(&[Vec2::new(dx, 3.0), Vec2::new(dx + sway * 0.2, -len * 0.5), end + Vec2::X * dx * 0.5], 6);
        p.part(e, stroke(&pts, |_| 1.1, 0.8, 0.5, |_| 1.0), M::Soot, 0.0);
    }
    signal_head(p, e, end, sway * -0.02, 0.01);
}

/// A fire hydrant, sometimes knocked askew, a cap missing.
fn hydrant(p: &mut Props, at: Vec2) {
    let tilt = if p.rng.chance(0.4) { p.rng.range(-0.35, 0.35) } else { 0.0 };
    let k1 = sign_of(p);
    let e = p.root(at - Vec2::Y * S * 2.0, k1, tilt);
    let body = [Vec2::new(-4.0, -2.0), Vec2::new(-4.0, 13.0), Vec2::new(4.0, 13.0), Vec2::new(4.0, -2.0)];
    p.shape(e, &body, 0.45, M::Enamel, 0.0);
    // Flange at the base and under the bonnet.
    for y in [1.0, 12.5] {
        p.part(e, stroke(&[Vec2::new(-5.5, y), Vec2::new(5.5, y)], |_| 2.2, 1.0, 0.6, |_| 1.0), M::Enamel, 0.01);
    }
    let dome: Vec<Vec2> = (0..=8).map(|i| {
        let a = i as f32 / 8.0 * std::f32::consts::PI;
        Vec2::new(a.cos() * 4.0, 13.5 + a.sin() * 3.5)
    }).collect();
    p.shape(e, &dome, 0.6, M::Enamel, 0.01);
    p.part(e, stroke(&[Vec2::new(0.0, 16.5), Vec2::new(0.0, 19.0)], |_| 2.2, 1.0, 0.6, |_| 1.0), M::Iron, 0.02);
    // Side nozzles: one capped, one open (black mouth).
    p.part(e, stroke(&[Vec2::new(-4.0, 8.0), Vec2::new(-7.0, 8.0)], |_| 3.4, 1.0, 0.6, |_| 1.0), M::Enamel, 0.02);
    p.part(e, stroke(&[Vec2::new(4.0, 8.0), Vec2::new(7.0, 8.0)], |_| 3.4, 1.0, 0.6, |_| 1.0), M::Enamel, 0.02);
    let mouth: Vec<Vec2> = ellipse_pts(0.8, 1.3, 8).into_iter().map(|q| q + Vec2::new(7.0, 8.0)).collect();
    p.part(e, fill(&mouth, |_| 1.0), M::Soot, 0.03);
    // Rust bleeding down.
    p.part(e, stroke(&[Vec2::new(-1.5, 11.0), Vec2::new(-1.2, 3.0)], |t| 1.6 * (1.0 - t) + 0.3, 1.0, 1.0, |_| 1.0), M::Rust, 0.03);
}

/// A sedan swallowed nose-first by the ground, its tail in the air.
fn car(p: &mut Props, at: Vec2) {
    let flip = sign_of(p);
    let dive = p.rng.range(0.35, 0.8);
    // Pivot sunk below the surface so the nose is buried.
    let k1 = p.rng.range(12.0, 18.0);
    let e = p.root(at - Vec2::Y * S * k1, flip, -dive * flip);
    let k1 = p.rng.chance(0.5);
    car_body(p, e, k1);
}

/// A car in its local frame: +X is the front, wheels on y = 0.
fn car_body(p: &mut Props, e: Entity, light: bool) {
    let m = if light { M::Enamel } else { M::Steel };
    let (l, h) = (30.0, 12.0);
    let crush = p.rng.range(0.0, 4.0);
    let body = [
        Vec2::new(-l, 3.0),
        Vec2::new(-l - 1.0, h - 2.0),
        Vec2::new(-l + 2.0, h),
        Vec2::new(-14.0, h + 1.0),
        Vec2::new(-10.0, h + 10.0 - crush),
        Vec2::new(8.0, h + 10.0 - crush * 0.5),
        Vec2::new(14.0, h + 1.0),
        Vec2::new(l - 1.0, h - 1.0),
        Vec2::new(l, 4.0),
        Vec2::new(l - 2.0, 2.0),
    ];
    p.shape(e, &body, 0.45, m, 0.0);
    // Windows: blown out, black, a pillar between.
    let win = [Vec2::new(-12.0, h + 1.5), Vec2::new(-9.0, h + 8.5 - crush), Vec2::new(7.0, h + 8.5 - crush * 0.5), Vec2::new(12.0, h + 1.5)];
    p.shape(e, &win, 0.6, M::Soot, 0.01);
    p.part(e, stroke(&[Vec2::new(-1.0, h + 1.5), Vec2::new(-1.0, h + 8.8 - crush * 0.7)], |_| 1.6, 1.0, 0.6, |_| 1.0), m, 0.02);
    // Door seams, sill, bumper, lights.
    p.part(e, stroke(&[Vec2::new(-l + 1.0, 6.0), Vec2::new(l - 1.0, 6.0)], |_| 0.7, 1.0, 1.0, |_| 1.0), M::Soot, 0.02);
    for x in [-12.0, 1.0] {
        p.part(e, stroke(&[Vec2::new(x, 3.0), Vec2::new(x + 0.5, h)], |_| 0.6, 1.0, 1.0, |_| 1.0), M::Soot, 0.02);
    }
    p.part(e, stroke(&[Vec2::new(-l + 2.0, h - 0.6), Vec2::new(-14.0, h + 0.4), Vec2::new(14.0, h + 0.4), Vec2::new(l - 2.0, h - 1.6)], |_| 1.0, 1.0, 1.0, |_| 1.0), M::Rim, 0.015);
    p.part(e, stroke(&[Vec2::new(-l - 1.5, 3.0), Vec2::new(-l - 1.5, 7.0)], |_| 2.0, 1.0, 0.6, |_| 1.0), M::Iron, 0.02);
    let tail = [Vec2::new(-l - 0.5, 8.0), Vec2::new(-l + 2.5, 8.0), Vec2::new(-l + 2.5, 10.5), Vec2::new(-l, 10.5)];
    p.shape(e, &tail, 0.7, M::Soot, 0.02);
    // Wheels (one torn off: just the hub).
    let gone = p.rng.below(3);
    for (i, x) in [-19.0, 19.0].into_iter().enumerate() {
        let c = Vec2::new(x, 3.0);
        let arch: Vec<Vec2> = ellipse_pts(7.0, 6.5, 14).into_iter().map(|q| q + c).collect();
        p.part(e, fill(&arch, |_| 1.0), M::Soot, 0.03);
        if i == gone {
            p.ring(e, c, 2.5, 1.6, M::Rust, 0.04, |_| 1.0);
        } else {
            p.tire(e, c, 6.0, 0.04);
        }
    }
    // Dents and rust blooms.
    for _ in 0..2 {
        let c = Vec2::new(p.rng.range(-l + 4.0, l - 4.0), p.rng.range(4.0, h - 1.0));
        let k1 = p.rng.range(1.5, 3.0);
        let pts: Vec<Vec2> = rock_pts(&mut p.rng, k1, 7).into_iter().map(|q| q + c).collect();
        p.part(e, fill(&pts, |_| 0.8), M::Rust, 0.025);
    }
}

/// The back end of a bus, plunged into the ground.
fn bus(p: &mut Props, at: Vec2) {
    let flip = sign_of(p);
    let dive = p.rng.range(0.35, 0.8);
    let k1 = p.rng.range(14.0, 22.0);
    let e = p.root(at - Vec2::Y * S * k1, flip, -dive * flip);
    let (l, h) = (52.0, 30.0);
    // Only the rear half is drawn: the front is in the ground.
    let body = [Vec2::new(-l, 3.0), Vec2::new(-l - 1.0, h - 2.0), Vec2::new(-l + 2.0, h), Vec2::new(l, h), Vec2::new(l, 3.0)];
    p.shape(e, &body, 0.45, M::Enamel, 0.0);
    // A dark band of windows, some still glazed (light), most gone.
    let mut x = -l + 4.0;
    while x < l - 8.0 {
        let w = 9.0;
        let pane = [Vec2::new(x, 14.0), Vec2::new(x, 25.0), Vec2::new(x + w, 25.0), Vec2::new(x + w, 14.0)];
        let glazed = p.rng.chance(0.25);
        p.shape(e, &pane, 0.6, if glazed { M::Steel } else { M::Soot }, 0.01);
        if glazed {
            let crack = [Vec2::new(x + 2.0, 24.0), Vec2::new(x + 5.0, 19.0), Vec2::new(x + 4.0, 15.0)];
            p.part(e, stroke(&crack, |_| 0.5, 1.0, 1.0, |_| 1.0), M::Soot, 0.02);
        }
        x += w + 2.0;
    }
    // Livery stripe, roof edge, rear bumper.
    p.part(e, stroke(&[Vec2::new(-l, 10.0), Vec2::new(l, 10.0)], |_| 2.6, 0.5, 0.5, |_| 1.0), M::Iron, 0.015);
    p.part(e, stroke(&[Vec2::new(-l + 2.0, h - 0.6), Vec2::new(l, h - 0.6)], |_| 1.4, 1.0, 1.0, |_| 1.0), M::Rim, 0.015);
    p.part(e, stroke(&[Vec2::new(-l - 1.5, 3.0), Vec2::new(-l - 1.5, 9.0)], |_| 2.4, 1.0, 0.6, |_| 1.0), M::Iron, 0.02);
    // Engine grille at the back.
    for i in 0..4 {
        let y = 15.0 + i as f32 * 2.5;
        p.part(e, stroke(&[Vec2::new(-l + 1.0, y), Vec2::new(-l + 8.0, y)], |_| 0.8, 1.0, 1.0, |_| 1.0), M::Soot, 0.02);
    }
    let c = Vec2::new(-l + 14.0, 3.0);
    let arch: Vec<Vec2> = ellipse_pts(8.5, 8.0, 14).into_iter().map(|q| q + c).collect();
    p.part(e, fill(&arch, |_| 1.0), M::Soot, 0.03);
    p.tire(e, c, 7.5, 0.04);
}

/// A road sign on a bent post: a stop sign or a street-name blade.
fn road_sign(p: &mut Props, at: Vec2) {
    let flip = sign_of(p);
    let e = p.root(at - Vec2::Y * S * 3.0, flip, 0.0);
    let h = p.rng.range(36.0, 48.0);
    let kink = h * p.rng.range(0.3, 0.6);
    let bend = p.rng.range(6.0, 18.0);
    let top = Vec2::new(bend, kink + (h - kink) * 0.8);
    p.bar(e, &[Vec2::new(0.0, -3.0), Vec2::new(0.0, kink), top], 1.8, M::Steel, 0.0);
    let face_angle = p.rng.range(-0.6, 0.6);
    let rot = |q: Vec2| top + Vec2::from_angle(face_angle).rotate(q);
    if p.rng.chance(0.55) {
        // Octagon, white border, bullet holes.
        let r = 8.0;
        let oct: Vec<Vec2> = (0..8).map(|i| rot(Vec2::from_angle((i as f32 + 0.5) * std::f32::consts::TAU / 8.0) * r)).collect();
        p.shape(e, &oct, 0.7, M::Enamel, 0.01);
        let inner: Vec<Vec2> = (0..8).map(|i| rot(Vec2::from_angle((i as f32 + 0.5) * std::f32::consts::TAU / 8.0) * (r - 1.6))).collect();
        p.shape(e, &inner, 0.6, M::Iron, 0.02);
        // Letters as a pale bar.
        p.part(e, stroke(&[rot(Vec2::new(-4.0, 0.0)), rot(Vec2::new(4.0, 0.0))], |_| 2.2, 1.0, 1.0, |t| if (t * 7.0) as i32 % 2 == 0 { 1.0 } else { 0.4 }), M::Enamel, 0.03);
    } else {
        let blade = [rot(Vec2::new(-11.0, -2.5)), rot(Vec2::new(-11.0, 2.5)), rot(Vec2::new(11.0, 2.5)), rot(Vec2::new(11.0, -2.5))];
        p.shape(e, &blade, 0.6, M::Iron, 0.01);
        p.part(e, stroke(&[rot(Vec2::new(-8.0, 0.0)), rot(Vec2::new(7.0, 0.0))], |_| 1.4, 1.0, 1.0, |t| if (t * 9.0) as i32 % 3 == 2 { 0.3 } else { 1.0 }), M::Enamel, 0.02);
    }
    for _ in 0..2 {
        let c = rot(Vec2::new(p.rng.range(-5.0, 5.0), p.rng.range(-3.0, 3.0)));
        let hole: Vec<Vec2> = ellipse_pts(0.7, 0.7, 6).into_iter().map(|q| q + c).collect();
        p.part(e, fill(&hole, |_| 1.0), M::Soot, 0.04);
    }
}

/// A concrete jersey barrier, cracked, with a hazard stripe.
fn barrier(p: &mut Props, at: Vec2) {
    let tilt = p.rng.range(-0.12, 0.12);
    let k1 = sign_of(p);
    let e = p.root(at - Vec2::Y * S * 2.0, k1, tilt);
    let w = p.rng.range(14.0, 18.0);
    let outline = [Vec2::new(-w, -2.0), Vec2::new(-w, 3.0), Vec2::new(-w * 0.55, 6.0), Vec2::new(-w * 0.4, 14.0), Vec2::new(w * 0.4, 14.0), Vec2::new(w * 0.55, 6.0), Vec2::new(w, 3.0), Vec2::new(w, -2.0)];
    p.shape(e, &outline, 0.45, M::Concrete, 0.0);
    p.part(e, stroke(&[Vec2::new(-w * 0.4, 13.4), Vec2::new(w * 0.4, 13.4)], |_| 1.2, 1.0, 1.0, |_| 1.0), M::Rim, 0.01);
    // Diagonal hazard stripes.
    let mut tris = Vec::new();
    let mut x = -w * 0.45;
    while x < w * 0.35 {
        tris.push([Vec2::new(x, 8.0), Vec2::new(x + 2.2, 8.0), Vec2::new(x + 4.4, 11.0)]);
        tris.push([Vec2::new(x, 8.0), Vec2::new(x + 4.4, 11.0), Vec2::new(x + 2.2, 11.0)]);
        x += 5.0;
    }
    p.part(e, triangles(&tris), M::Enamel, 0.01);
    let x0 = p.rng.range(-w * 0.3, w * 0.3);
    let crack: Vec<Vec2> = (0..4).map(|i| Vec2::new(x0 + p.rng.range(-2.0, 2.0), 14.0 - i as f32 * 4.5)).collect();
    p.part(e, stroke(&crack, |t| 1.4 * (1.0 - t) + 0.3, 1.0, 1.0, |_| 1.0), M::Soot, 0.02);
    // A chunk knocked out of the top corner, rebar showing.
    if p.rng.chance(0.5) {
        p.rebar(e, Vec2::new(w * 0.35, 12.0), Vec2::new(0.6, 0.8).normalize(), 0.015);
    }
}

/// A traffic cone, knocked over or standing.
fn cone(p: &mut Props, at: Vec2) {
    let fallen = p.rng.chance(0.5);
    let k1 = sign_of(p);
    let e = p.root(at - Vec2::Y * S * if fallen { 0.5 } else { 1.0 }, k1, if fallen { -1.45 } else { 0.0 });
    let base = [Vec2::new(-5.0, 0.0), Vec2::new(-5.0, 1.5), Vec2::new(5.0, 1.5), Vec2::new(5.0, 0.0)];
    let y = if fallen { 4.0 } else { 0.0 };
    let off = Vec2::new(0.0, y);
    p.shape(e, &base.map(|q| q + off), 0.5, M::Plastic, 0.0);
    let body = [Vec2::new(-3.5, 1.5), Vec2::new(-0.8, 12.0), Vec2::new(0.8, 12.0), Vec2::new(3.5, 1.5)];
    p.shape(e, &body.map(|q| q + off), 0.5, M::Plastic, 0.01);
    p.part(e, stroke(&[Vec2::new(-2.2, 6.0) + off, Vec2::new(2.2, 6.0) + off], |_| 2.0, 1.0, 1.0, |_| 1.0), M::Enamel, 0.02);
}

/// Broken road surface overhanging a drop: asphalt crust, gravel under it, a curb and paint.
fn asphalt_lip(p: &mut Props, e: Entity) {
    let out = p.rng.range(4.0, 11.0);
    let thick = p.rng.range(4.0, 6.0);
    let droop = p.rng.range(0.0, 4.0);
    let crust = [
        Vec2::new(-12.0, 0.6),
        Vec2::new(out * 0.5, 0.4 - droop * 0.3),
        Vec2::new(out, -droop),
        Vec2::new(out - 1.5, -droop - thick * 0.6),
        Vec2::new(out - 3.0, -droop - thick),
        Vec2::new(-12.0, -thick),
    ];
    p.shape(e, &crust, 0.5, M::Iron, 0.0);
    // Road surface catching the light, a lane line ending at the break.
    p.part(e, stroke(&[crust[0], crust[1], crust[2]], |_| 1.2, 1.0, 1.0, |_| 1.0), M::Steel, 0.01);
    if p.rng.chance(0.6) {
        let a = Vec2::new(-9.0, 0.0);
        p.part(e, stroke(&[a, Vec2::new(out * 0.4, -droop * 0.3)], |_| 1.0, 1.0, 1.0, |_| 1.0), M::Enamel, 0.02);
    }
    // Gravel base crumbling out underneath.
    for _ in 0..3 + p.rng.below(3) {
        let c = Vec2::new(p.rng.range(-8.0, out - 3.0), -thick - p.rng.range(0.5, 4.0));
        let k1 = p.rng.range(0.8, 1.8);
        let pts: Vec<Vec2> = rock_pts(&mut p.rng, k1, 5).into_iter().map(|q| q + c).collect();
        p.shape(e, &pts, 0.5, M::Concrete, 0.015);
    }
    // Wire mesh poking out of the break.
    if p.rng.chance(0.5) {
        for _ in 0..2 {
            let base = Vec2::new(out - 3.0, -droop - p.rng.range(1.0, thick));
            let dir = Vec2::new(1.0, p.rng.range(-0.8, 0.3)).normalize();
            p.rebar(e, base, dir, -0.01);
        }
    }
}

/// A broken pipe end: dark bore and a ragged lip, facing along `dir`.
fn broken_mouth(p: &mut Props, e: Entity, c: Vec2, dir: Vec2, r: f32, m: M) {
    let d = dir.normalize_or(Vec2::X);
    let n = d.perp();
    let bore: Vec<Vec2> = (0..10).map(|i| {
        let a = i as f32 / 10.0 * std::f32::consts::TAU;
        c + d * a.cos() * r * 0.35 + n * a.sin() * r * 0.8
    }).collect();
    p.part(e, fill(&bore, |_| 1.0), M::Soot, 0.02);
    // Jagged teeth around the break.
    let mut tris = Vec::new();
    for i in 0..5 {
        let s = -1.0 + 2.0 * (i as f32 + 0.5) / 5.0;
        let b = c + n * s * r;
        let tooth = p.rng.range(0.5, 2.2);
        tris.push([b - n * r * 0.2, b + n * r * 0.2, b + d * tooth]);
    }
    p.part(e, triangles(&tris), m, 0.025);
}

/// A utility pipe cut by the collapse, out of the wall along local +X.
fn severed_pipe(p: &mut Props, e: Entity, at: Vec2, r: f32, droop: f32) {
    let len = p.rng.range(8.0, 22.0);
    let dir = Vec2::from_angle(droop);
    let mouth = at + dir * len;
    let m = match p.rng.below(3) {
        0 => M::Rust,
        1 => M::Steel,
        _ => M::Concrete,
    };
    p.part(e, stroke(&[at - dir * 8.0, mouth], |_| 2.0 * r, 1.0, 0.4, |_| 1.0), m, 0.0);
    // Joint collar.
    let f = at + dir * (len * 0.35);
    let n = dir.perp();
    p.part(e, stroke(&[f - n * (r + 1.2), f + n * (r + 1.2)], |_| 2.6, 0.9, 0.6, |_| 1.0), M::Iron, 0.01);
    broken_mouth(p, e, mouth, dir, r, m);
    // Something still leaking out: a dark trickle (water main) or nothing.
    if p.rng.chance(0.4) {
        let start = mouth + Vec2::new(0.5, -r * 0.6);
        let len = p.rng.range(8.0, 20.0);
        let pts = [start, start + Vec2::new(2.0, -2.0), start + Vec2::new(2.5, -len)];
        p.part(e, stroke(&smooth_open(&pts, 5), |t| 2.0 - 1.4 * t, 1.0, 0.6, |_| 1.0), M::Soot, OVER);
    }
    // Wires spilling alongside some.
    if p.rng.chance(0.35) {
        p.cables(e, at + Vec2::new(0.0, -r - 1.0), 1, 0.0);
    }
}

/// A burst cable duct: a bundle of cut cables fanning out of the wall.
fn cable_spill(p: &mut Props, e: Entity, at: Vec2) {
    let duct = [at + Vec2::new(-6.0, -4.0), at + Vec2::new(-6.0, 4.0), at + Vec2::new(3.0, 4.5), at + Vec2::new(3.5, -4.5)];
    p.shape(e, &duct, 0.5, M::Concrete, 0.0);
    let hole: Vec<Vec2> = ellipse_pts(1.6, 3.5, 10).into_iter().map(|q| q + at + Vec2::X * 3.2).collect();
    p.part(e, fill(&hole, |_| 1.0), M::Soot, 0.01);
    for i in 0..3 + p.rng.below(4) {
        let start = at + Vec2::new(3.0, p.rng.range(-2.5, 2.5));
        let reach = p.rng.range(6.0, 18.0);
        let fall = p.rng.range(8.0, 40.0);
        let pts = smooth_open(&[start, start + Vec2::new(reach * 0.5, 1.0), start + Vec2::new(reach, -fall * 0.4), start + Vec2::new(reach + p.rng.range(-3.0, 3.0), -fall)], 6);
        let w = p.rng.range(1.0, 2.2);
        let m = if i % 3 == 1 { M::Rubber } else { M::Soot };
        p.part(e, stroke(&pts, |_| w, 0.9, 0.5, |_| 1.0), m, 0.02 + i as f32 * 0.002);
        // Bright copper at the cut.
        let end = pts[pts.len() - 1];
        p.part(e, stroke(&[end, end - Vec2::Y * 2.0], |_| w * 0.6, 1.0, 1.0, |_| 1.0), M::Bronze, 0.03);
    }
}

/// A slab of road jammed into a wall, sticking out at an angle (local +X).
fn jammed_slab(p: &mut Props, e: Entity) {
    let len = p.rng.range(14.0, 26.0);
    let t = p.rng.range(5.0, 8.0);
    let outline = [Vec2::new(-6.0, 0.0), Vec2::new(len, 0.5), Vec2::new(len + 1.5, -t * 0.4), Vec2::new(len - 2.0, -t * 0.7), Vec2::new(len - 1.0, -t), Vec2::new(-6.0, -t)];
    p.shape(e, &outline, 0.5, M::Iron, 0.0);
    // Gravel base underneath the asphalt.
    let base = [Vec2::new(-6.0, -t), Vec2::new(len - 1.0, -t), Vec2::new(len - 3.0, -t - 3.0), Vec2::new(-6.0, -t - 3.0)];
    p.shape(e, &base, 0.6, M::Concrete, -0.005);
    p.part(e, stroke(&[Vec2::new(-6.0, 0.0), Vec2::new(len, 0.5)], |_| 1.2, 1.0, 1.0, |_| 1.0), M::Steel, 0.01);
    if p.rng.chance(0.6) {
        p.part(e, stroke(&[Vec2::new(-2.0, -0.2), Vec2::new(len * 0.6, 0.1)], |_| 1.0, 1.0, 1.0, |_| 1.0), M::Enamel, 0.02);
    }
}

// --- Shops and offices ---

/// Blocky lettering on a sign: pale glyph bars, some missing.
fn lettering(p: &mut Props, e: Entity, c: Vec2, w: f32, h: f32, m: M, z: f32) {
    let n = ((w / (h * 0.8)) as usize).clamp(2, 9);
    let step = w / n as f32;
    let mut tris = Vec::new();
    for i in 0..n {
        if p.rng.chance(0.2) {
            continue;
        }
        let x = c.x - w / 2.0 + step * (i as f32 + 0.2);
        let gw = step * 0.6;
        let y0 = c.y - h / 2.0;
        let y1 = if p.rng.chance(0.3) { c.y + h / 2.0 } else { c.y + h * 0.3 };
        tris.push([Vec2::new(x, y0), Vec2::new(x + gw, y0), Vec2::new(x + gw, y1)]);
        tris.push([Vec2::new(x, y0), Vec2::new(x + gw, y1), Vec2::new(x, y1)]);
    }
    if !tris.is_empty() {
        p.part(e, triangles(&tris), m, z);
    }
}

/// A striped canvas awning seen from the side, out from `at` along local +X, torn and sagging.
fn awning(p: &mut Props, e: Entity, at: Vec2, w: f32, z: f32) {
    let sag = p.rng.range(3.0, 9.0);
    let torn = p.rng.chance(0.5);
    let stripes = 6 + p.rng.below(4);
    let st = stripes as f32;
    let top = [at, at + Vec2::new(w * 0.5, -sag * 0.4), at + Vec2::new(w, -sag)];
    let low = [at + Vec2::new(w, -sag - 5.0), at + Vec2::new(w * 0.5, -sag * 0.4 - 3.5), at + Vec2::new(0.0, -3.0)];
    let mut outline: Vec<Vec2> = top.to_vec();
    outline.extend(low);
    p.part(e, fill(&outline, move |q| if ((q.x - at.x) / w * st) as i32 % 2 == 0 { 1.0 } else { 0.5 }), M::Plastic, z);
    // Valance teeth along the underside, ragged where it tore.
    let mut tris = Vec::new();
    for i in 0..stripes {
        let a = low[2].lerp(low[1], 0.0).lerp(low[0], i as f32 / st);
        let b = low[2].lerp(low[0], (i as f32 + 1.0) / st);
        let hang = if torn && i * 3 > stripes * 2 { p.rng.range(4.0, 12.0) } else { 2.0 };
        tris.push([a, b, a.lerp(b, 0.5) - Vec2::Y * hang]);
    }
    p.part(e, triangles(&tris), M::Plastic, z + 0.005);
    // Frame arm.
    p.bar(e, &[at + Vec2::new(0.0, -12.0), at.lerp(top[2], 0.6)], 1.0, M::Iron, z + 0.01);
}

/// A shop window, a doorway or a rolled shutter in a wall face, signboard (and awning) above.
fn storefront(p: &mut Props, e: Entity, half: Vec2, z: f32) {
    let (l, r, b, t) = (-half.x, half.x, -half.y, half.y);
    let hole = [Vec2::new(l, b), Vec2::new(l, t), Vec2::new(r, t), Vec2::new(r, b)];
    p.shape(e, &hole, 0.6, M::Soot, z);
    let kind = p.rng.below(3);
    match kind {
        0 => {
            // Shop window: mullion, cracked glass catching light, a display shelf with leftovers.
            let mid = p.rng.range(-half.x * 0.3, half.x * 0.3);
            p.part(e, stroke(&[Vec2::new(mid, b), Vec2::new(mid, t)], |_| 1.6, 1.0, 0.6, |_| 1.0), M::Iron, z + 0.02);
            let mut tris = Vec::new();
            for _ in 0..3 {
                let c = Vec2::new(if p.rng.chance(0.5) { l + 1.0 } else { r - 1.0 }, if p.rng.chance(0.5) { b + 5.0 } else { t - 1.0 });
                let inward = (-c).signum();
                let dx = p.rng.range(4.0, 10.0);
                let dy = p.rng.range(5.0, 12.0);
                tris.push([c, c + Vec2::new(inward.x * dx, 0.0), c + Vec2::new(0.0, inward.y * dy)]);
            }
            p.part(e, triangles(&tris), M::Steel, z + 0.01);
            let shelf = b + 5.0;
            p.part(e, stroke(&[Vec2::new(l, shelf), Vec2::new(r, shelf)], |_| 1.4, 1.0, 1.0, |_| 1.0), M::Wood, z + 0.015);
            for _ in 0..2 + p.rng.below(3) {
                let x = p.rng.range(l + 3.0, r - 6.0);
                let (w, h) = (p.rng.range(2.5, 5.0), p.rng.range(2.5, 7.0));
                let bx = [Vec2::new(x, shelf + 0.6), Vec2::new(x, shelf + h), Vec2::new(x + w, shelf + h), Vec2::new(x + w, shelf + 0.6)];
                p.shape(e, &bx, 0.5, M::Enamel, z + 0.016);
            }
            if p.rng.chance(0.5) {
                // A mannequin's torso.
                let x = p.rng.range(l + 6.0, r - 6.0);
                let torso = [Vec2::new(x - 2.5, shelf + 1.0), Vec2::new(x - 3.5, shelf + 9.0), Vec2::new(x - 1.0, shelf + 11.0), Vec2::new(x + 1.0, shelf + 11.0), Vec2::new(x + 3.5, shelf + 9.0), Vec2::new(x + 2.5, shelf + 1.0)];
                p.shape(e, &smooth_closed(&torso, 3), 0.6, M::Enamel, z + 0.017);
            }
        }
        1 => {
            // Doorway: a glass door hanging crooked, push bar, a card left in the glass.
            let lean = p.rng.range(-4.0, 4.0);
            let door = [Vec2::new(l + 2.0, b), Vec2::new(l + 2.0 + lean, t - 1.0), Vec2::new(r - 6.0 + lean, t - 1.0), Vec2::new(r - 6.0, b)];
            let mut frame = door.to_vec();
            frame.push(door[0]);
            p.part(e, stroke(&frame, |_| 1.8, 1.0, 0.6, |_| 1.0), M::Iron, z + 0.02);
            let bar_y = (b + t) / 2.0;
            p.part(e, stroke(&[Vec2::new(l + 4.0 + lean * 0.5, bar_y), Vec2::new(r - 8.0 + lean * 0.5, bar_y)], |_| 1.4, 1.0, 1.0, |_| 1.0), M::Steel, z + 0.025);
            p.part(e, stroke(&[Vec2::new(l + 4.0, t - 4.0), Vec2::new(l + 10.0, t - 9.0)], |_| 0.8, 1.0, 1.0, |_| 1.0), M::Steel, z + 0.02);
            let card = [Vec2::new(r - 14.0, bar_y + 3.0), Vec2::new(r - 14.0, bar_y + 7.0), Vec2::new(r - 8.0, bar_y + 7.0), Vec2::new(r - 8.0, bar_y + 3.0)];
            p.shape(e, &card, 0.8, M::Enamel, z + 0.03);
        }
        _ => {
            // Roller shutter, jammed part-open, ribbed, a tag sprayed on it.
            let open = p.rng.range(0.2, 0.7);
            let low = t - (t - b) * (1.0 - open);
            let (j0, j1) = (p.rng.range(-2.0, 2.0), p.rng.range(-3.0, 1.0));
            let sh = [Vec2::new(l, low + j0), Vec2::new(l, t), Vec2::new(r, t), Vec2::new(r, low + j1)];
            let span = (t - low).max(1.0);
            p.part(e, fill(&sh, move |q| if ((q.y - low) / span * 10.0) as i32 % 2 == 0 { 0.85 } else { 0.6 }), M::Steel, z + 0.01);
            p.part(e, stroke(&[sh[0], sh[3]], |_| 1.4, 1.0, 1.0, |_| 1.0), M::Iron, z + 0.02);
            let x0 = p.rng.range(l + 3.0, 0.0);
            let mut tag = Vec::new();
            for i in 0..6 {
                let j = p.rng.range(-1.0, 1.0);
                tag.push(Vec2::new(x0 + i as f32 * 3.0, (low + t) / 2.0 + if i % 2 == 0 { 2.0 } else { -2.0 } + j));
            }
            p.part(e, stroke(&smooth_open(&tag, 3), |_| 1.0, 1.0, 1.0, |_| 1.0), M::Soot, z + 0.03);
        }
    }
    // Frame and signboard.
    let mut frame = hole.to_vec();
    frame.push(hole[0]);
    p.part(e, stroke(&frame, |_| 2.4, 1.0, 0.5, |_| 1.0), M::Iron, z + 0.04);
    let sign_h = p.rng.range(6.0, 9.0);
    let sign = [Vec2::new(l - 2.0, t + 1.5), Vec2::new(l - 2.0, t + 1.5 + sign_h), Vec2::new(r + 2.0, t + 1.5 + sign_h), Vec2::new(r + 2.0, t + 1.5)];
    p.shape(e, &sign, 0.6, M::Iron, z + 0.04);
    lettering(p, e, Vec2::new(0.0, t + 1.5 + sign_h / 2.0), 2.0 * half.x - 4.0, sign_h * 0.55, M::Enamel, z + 0.05);
    if kind != 2 && p.rng.chance(0.6) {
        // Front-on awning: a striped valance over the top of the opening, torn away on one side.
        let drop = p.rng.range(4.0, 7.0);
        let cut = p.rng.range(0.4, 1.0);
        let n = 8;
        let mut tris = Vec::new();
        for i in 0..n {
            if (i as f32 / n as f32) > cut {
                continue;
            }
            let x0 = l + (r - l) * i as f32 / n as f32;
            let x1 = l + (r - l) * (i + 1) as f32 / n as f32;
            let d = drop + if i % 2 == 0 { 0.0 } else { 1.5 };
            tris.push([Vec2::new(x0, t + 1.0), Vec2::new(x1, t + 1.0), Vec2::new(x1, t - d)]);
            tris.push([Vec2::new(x0, t + 1.0), Vec2::new(x1, t - d), Vec2::new(x0, t - d)]);
        }
        if !tris.is_empty() {
            let mesh = triangles(&tris);
            p.part(e, mesh, M::Plastic, z + 0.06);
        }
    }
}

/// A projecting shop sign on a bracket, out from the wall (local +X).
fn blade_sign(p: &mut Props, e: Entity) {
    let y = p.rng.range(0.0, 14.0);
    let swing = p.rng.range(-0.5, 0.2);
    p.bar(e, &[Vec2::new(-3.0, y), Vec2::new(16.0, y)], 1.4, M::Iron, 0.0);
    let hang = Vec2::new(14.0, y);
    let rot = |q: Vec2| hang + Vec2::from_angle(swing).rotate(q);
    let board = [rot(Vec2::new(-11.0, -1.0)), rot(Vec2::new(-11.0, -10.0)), rot(Vec2::new(0.0, -10.0)), rot(Vec2::new(0.0, -1.0))];
    p.part(e, stroke(&[Vec2::new(4.0, y), board[0]], |_| 0.6, 1.0, 1.0, |_| 1.0), M::Iron, 0.01);
    p.part(e, stroke(&[hang, board[3]], |_| 0.6, 1.0, 1.0, |_| 1.0), M::Iron, 0.01);
    p.shape(e, &board, 0.6, M::Enamel, 0.01);
    let inner = [rot(Vec2::new(-9.5, -2.5)), rot(Vec2::new(-9.5, -8.5)), rot(Vec2::new(-1.5, -8.5)), rot(Vec2::new(-1.5, -2.5))];
    p.shape(e, &inner, 0.6, M::Iron, 0.02);
    let c = rot(Vec2::new(-5.5, -5.5));
    let glyph: Vec<Vec2> = ellipse_pts(2.2, 2.2, 10).into_iter().map(|q| q + c).collect();
    p.part(e, fill(&glyph, |_| 1.0), M::Rim, 0.03);
}

/// A chunk of a shop's facade still standing in the rubble: broken wall around a storefront.
fn facade_chunk(p: &mut Props, at: Vec2) {
    let lean = p.rng.range(-0.15, 0.15);
    let k1 = sign_of(p);
    let e = p.root(at - Vec2::Y * S * 6.0, k1, lean);
    let (w, h) = (p.rng.range(22.0, 27.0), p.rng.range(46.0, 56.0));
    // The wall, broken off jaggedly at the top and down one side.
    let mut outline = vec![Vec2::new(-w, -4.0), Vec2::new(-w, h * 0.7)];
    let n = 6;
    for i in 0..=n {
        let x = -w + 2.0 * w * i as f32 / n as f32;
        let bite = p.rng.range(0.0, 14.0);
        outline.push(Vec2::new(x, h - bite * (i as f32 / n as f32)));
    }
    outline.push(Vec2::new(w - 2.0, h * 0.4));
    outline.push(Vec2::new(w, -4.0));
    p.shape(e, &outline, 0.45, M::Concrete, 0.0);
    let top: Vec<Vec2> = outline[2..outline.len() - 2].iter().map(|q| *q - Vec2::Y).collect();
    p.part(e, stroke(&top, |_| 1.6, 1.0, 0.6, |_| 1.0), M::Rim, 0.005);
    let half = Vec2::new(w - 6.0, 13.0);
    let e2 = p.commands.spawn((Transform::from_xyz(0.0, 16.0, 0.01), Visibility::default(), ChildOf(e))).id();
    storefront(p, e2, half, 0.0);
    for _ in 0..2 {
        let base = Vec2::new(w - 2.0, p.rng.range(h * 0.2, h * 0.5));
        let dir = Vec2::new(1.0, p.rng.range(-0.3, 0.6)).normalize();
        p.rebar(e, base, dir, -0.01);
    }
}

/// An office desk with a dead monitor and papers, a drawer hanging open.
fn desk(p: &mut Props, at: Vec2) {
    let k1 = sign_of(p);
    let tilt = if p.rng.chance(0.3) { p.rng.range(-0.15, 0.15) } else { 0.0 };
    let e = p.root(at - Vec2::Y * S * 1.5, k1, tilt);
    let (w, h) = (p.rng.range(14.0, 18.0), 12.0);
    p.part(e, stroke(&[Vec2::new(-w, h), Vec2::new(w, h)], |_| 2.2, 1.0, 0.5, |_| 1.0), M::Wood, 0.01);
    // Drawer pedestal, a leg at the other end (buckled on some).
    let ped = [Vec2::new(w - 10.0, 0.0), Vec2::new(w - 10.0, h - 1.0), Vec2::new(w - 1.0, h - 1.0), Vec2::new(w - 1.0, 0.0)];
    p.shape(e, &ped, 0.5, M::Wood, 0.0);
    for i in 0..3 {
        let y = 0.5 + i as f32 * 3.6;
        p.part(e, stroke(&[Vec2::new(w - 9.5, y + 3.2), Vec2::new(w - 1.5, y + 3.2)], |_| 0.5, 1.0, 1.0, |_| 1.0), M::Soot, 0.005);
        p.part(e, stroke(&[Vec2::new(w - 6.5, y + 1.6), Vec2::new(w - 4.0, y + 1.6)], |_| 0.7, 1.0, 1.0, |_| 1.0), M::Steel, 0.006);
    }
    if p.rng.chance(0.5) {
        let d = [Vec2::new(w - 1.0, 5.0), Vec2::new(w - 1.0, 8.0), Vec2::new(w + 5.0, 8.0), Vec2::new(w + 5.0, 5.0)];
        p.shape(e, &d, 0.6, M::Wood, 0.02);
    }
    let leg_end = if p.rng.chance(0.3) { Vec2::new(-w + 5.0, 1.0) } else { Vec2::new(-w + 2.0, 0.0) };
    p.bar(e, &[Vec2::new(-w + 2.0, h - 1.0), leg_end], 1.6, M::Wood, 0.0);
    if p.rng.chance(0.7) {
        // A boxy monitor, screen cracked.
        let x = p.rng.range(-w + 5.0, w - 12.0);
        let mon = [Vec2::new(x - 5.0, h + 2.0), Vec2::new(x - 5.5, h + 11.0), Vec2::new(x + 5.5, h + 11.0), Vec2::new(x + 5.0, h + 2.0)];
        p.shape(e, &mon, 0.55, M::Plastic, 0.02);
        let scr = [Vec2::new(x - 3.8, h + 3.5), Vec2::new(x - 4.0, h + 9.6), Vec2::new(x + 4.0, h + 9.6), Vec2::new(x + 3.8, h + 3.5)];
        p.shape(e, &scr, 0.8, M::Soot, 0.03);
        p.part(e, stroke(&[Vec2::new(x - 2.0, h + 9.0), Vec2::new(x + 0.5, h + 6.0), Vec2::new(x + 3.0, h + 4.5)], |_| 0.4, 1.0, 1.0, |_| 1.0), M::Steel, 0.04);
        p.part(e, stroke(&[Vec2::new(x - 2.0, h + 1.1), Vec2::new(x + 2.0, h + 1.1)], |_| 1.4, 1.0, 1.0, |_| 1.0), M::Plastic, 0.02);
    }
    let x = p.rng.range(w - 10.0, w - 3.0);
    let pile = [Vec2::new(x - 3.0, h + 1.1), Vec2::new(x - 3.0, h + 3.0), Vec2::new(x + 3.5, h + 2.6), Vec2::new(x + 3.0, h + 1.1)];
    p.shape(e, &pile, 0.8, M::Enamel, 0.02);
}

/// A steel filing cabinet, one drawer hanging out with folders in it.
fn filing_cabinet(p: &mut Props, at: Vec2) {
    let k1 = sign_of(p);
    let tilt = if p.rng.chance(0.35) { p.rng.range(-0.3, 0.3) } else { 0.0 };
    let e = p.root(at - Vec2::Y * S * 2.0, k1, tilt);
    let (w, h) = (5.5, p.rng.range(20.0, 27.0));
    let body = [Vec2::new(-w, -1.0), Vec2::new(-w, h), Vec2::new(w, h), Vec2::new(w, -1.0)];
    p.shape(e, &body, 0.5, M::Steel, 0.0);
    p.part(e, stroke(&[Vec2::new(-w + 0.5, h - 0.5), Vec2::new(w - 0.5, h - 0.5)], |_| 1.0, 1.0, 1.0, |_| 1.0), M::Rim, 0.005);
    let n = 4;
    let open = p.rng.below(n);
    for i in 0..n {
        let y0 = (h - 1.0) * i as f32 / n as f32 + 0.5;
        let y1 = (h - 1.0) * (i + 1) as f32 / n as f32 - 0.3;
        if i == open {
            let out = p.rng.range(5.0, 9.0);
            p.shape(e, &[Vec2::new(-w + 0.5, y0), Vec2::new(-w + 0.5, y1), Vec2::new(w - 0.5, y1), Vec2::new(w - 0.5, y0)], 0.9, M::Soot, 0.01);
            let dr = [Vec2::new(w - 3.0, y0), Vec2::new(w - 3.0, y1 - 0.5), Vec2::new(w + out, y1 - 0.5), Vec2::new(w + out, y0)];
            p.shape(e, &dr, 0.5, M::Steel, 0.02);
            let mut tris = Vec::new();
            for k in 0..3 {
                let x = w + k as f32 * 2.0;
                let (jx, jy) = (p.rng.range(-1.0, 1.0), p.rng.range(1.5, 3.0));
                tris.push([Vec2::new(x, y1 - 0.5), Vec2::new(x + 2.0, y1 - 0.5), Vec2::new(x + 1.0 + jx, y1 + jy)]);
            }
            p.part(e, triangles(&tris), M::Enamel, 0.015);
        } else {
            p.part(e, stroke(&[Vec2::new(-w, y1), Vec2::new(w, y1)], |_| 0.5, 1.0, 1.0, |_| 1.0), M::Soot, 0.01);
            let hy = (y0 + y1) / 2.0 + 1.0;
            p.part(e, stroke(&[Vec2::new(-1.5, hy), Vec2::new(1.5, hy)], |_| 0.8, 1.0, 1.0, |_| 1.0), M::Iron, 0.01);
        }
    }
}

/// A swivel office chair, sometimes knocked on its side.
fn office_chair(p: &mut Props, at: Vec2) {
    let fallen = p.rng.chance(0.35);
    let k1 = sign_of(p);
    let e = p.root(at - Vec2::Y * S * if fallen { 0.0 } else { 1.0 }, k1, if fallen { 1.4 } else { 0.0 });
    let off = if fallen { Vec2::new(0.0, 6.0) } else { Vec2::ZERO };
    p.part(e, stroke(&[Vec2::new(-6.0, 1.2) + off, Vec2::new(6.0, 1.2) + off], |_| 1.2, 1.0, 0.6, |_| 1.0), M::Iron, 0.0);
    for x in [-6.0, 0.0, 6.0] {
        let c: Vec<Vec2> = ellipse_pts(1.0, 1.0, 6).into_iter().map(|q| q + Vec2::new(x, 0.6) + off).collect();
        p.part(e, fill(&c, |_| 1.0), M::Rubber, 0.01);
    }
    p.part(e, stroke(&[Vec2::new(0.0, 1.2) + off, Vec2::new(0.0, 8.0) + off], |_| 1.4, 1.0, 0.6, |_| 1.0), M::Steel, 0.0);
    let seat = [Vec2::new(-5.5, 8.0), Vec2::new(-6.0, 10.0), Vec2::new(5.5, 10.0), Vec2::new(6.0, 8.0)].map(|q| q + off);
    p.shape(e, &seat, 0.5, M::Rubber, 0.01);
    let back = [Vec2::new(-6.0, 10.0), Vec2::new(-7.5, 20.0), Vec2::new(-4.5, 21.0), Vec2::new(-3.5, 10.0)].map(|q| q + off);
    p.shape(e, &back, 0.5, M::Rubber, 0.02);
    if p.rng.chance(0.5) {
        // Foam bursting out of a slash in the back.
        let foam: Vec<Vec2> = rock_pts(&mut p.rng, 1.4, 6).into_iter().map(|q| q + Vec2::new(-5.5, 16.0) + off).collect();
        p.part(e, fill(&foam, |_| 0.9), M::Enamel, 0.03);
    }
}

/// A shop counter with a cash register, drawer open and empty.
fn shop_counter(p: &mut Props, at: Vec2) {
    let k1 = sign_of(p);
    let e = p.root(at - Vec2::Y * S * 2.0, k1, 0.0);
    let (w, h) = (p.rng.range(18.0, 22.0), 13.0);
    let body = [Vec2::new(-w, -1.0), Vec2::new(-w, h), Vec2::new(w, h), Vec2::new(w, -1.0)];
    p.shape(e, &body, 0.45, M::Wood, 0.0);
    p.part(e, stroke(&[Vec2::new(-w - 1.5, h + 0.6), Vec2::new(w + 1.5, h + 0.6)], |_| 1.6, 1.0, 0.6, |_| 1.0), M::Rim, 0.01);
    for x in [-w * 0.33, w * 0.33] {
        p.part(e, stroke(&[Vec2::new(x, 0.0), Vec2::new(x, h - 1.0)], |_| 0.6, 1.0, 1.0, |_| 1.0), M::Soot, 0.01);
    }
    if p.rng.chance(0.5) {
        // Kicked in.
        let c = Vec2::new(p.rng.range(-w * 0.6, w * 0.6), p.rng.range(3.0, 8.0));
        let hole: Vec<Vec2> = rock_pts(&mut p.rng, 2.5, 7).into_iter().map(|q| q + c).collect();
        p.part(e, fill(&hole, |_| 1.0), M::Soot, 0.015);
    }
    let x = p.rng.range(-w + 6.0, w - 10.0);
    let reg = [Vec2::new(x - 4.0, h + 1.4), Vec2::new(x - 3.0, h + 7.0), Vec2::new(x + 4.0, h + 6.0), Vec2::new(x + 4.5, h + 1.4)];
    p.shape(e, &reg, 0.5, M::Iron, 0.02);
    let disp = [Vec2::new(x - 2.0, h + 7.0), Vec2::new(x - 2.0, h + 9.5), Vec2::new(x + 1.5, h + 9.5), Vec2::new(x + 1.5, h + 6.5)];
    p.shape(e, &disp, 0.6, M::Iron, 0.025);
    let drawer = [Vec2::new(x + 4.5, h + 1.4), Vec2::new(x + 4.5, h + 3.0), Vec2::new(x + 9.0, h + 3.0), Vec2::new(x + 9.0, h + 1.4)];
    p.shape(e, &drawer, 0.6, M::Steel, 0.02);
}

/// Store shelving, some shelves collapsed, a few cans and boxes left.
fn shop_shelf(p: &mut Props, at: Vec2) {
    let k1 = sign_of(p);
    let lean = if p.rng.chance(0.4) { p.rng.range(-0.25, 0.25) } else { 0.0 };
    let e = p.root(at - Vec2::Y * S * 2.0, k1, lean);
    let (w, h) = (p.rng.range(10.0, 13.0), p.rng.range(30.0, 38.0));
    let back = [Vec2::new(-w, 0.0), Vec2::new(-w, h), Vec2::new(w, h), Vec2::new(w, 0.0)];
    p.shape(e, &back, 0.8, M::Iron, -0.01);
    for x in [-w, w] {
        p.part(e, stroke(&[Vec2::new(x, -2.0), Vec2::new(x, h)], |_| 1.6, 1.0, 0.6, |_| 1.0), M::Steel, 0.0);
    }
    let n = 4;
    for i in 0..n {
        let y = 2.0 + (h - 4.0) * i as f32 / (n - 1) as f32;
        let fallen = i > 0 && p.rng.chance(0.3);
        let drop = if fallen { p.rng.range(5.0, 9.0) } else { 0.0 };
        p.part(e, stroke(&[Vec2::new(-w, y), Vec2::new(w, y - drop)], |_| 1.4, 1.0, 0.5, |_| 1.0), M::Steel, 0.01);
        if fallen {
            continue;
        }
        let mut x = -w + 1.0 + p.rng.range(0.0, 3.0);
        while x < w - 3.0 {
            if p.rng.chance(0.45) {
                let can = p.rng.chance(0.5);
                let (cw, ch) = if can { (2.2, 3.4) } else { (p.rng.range(3.0, 5.0), p.rng.range(3.0, 6.0)) };
                let item = [Vec2::new(x, y + 0.7), Vec2::new(x, y + ch), Vec2::new(x + cw, y + ch), Vec2::new(x + cw, y + 0.7)];
                p.shape(e, &item, 0.55, if can { M::Steel } else { M::Enamel }, 0.02);
                x += cw + 0.6;
            } else {
                x += p.rng.range(2.0, 4.0);
            }
        }
    }
}

/// Loose office paper and a binder on the floor.
fn papers(p: &mut Props, at: Vec2) {
    let e = p.root(at - Vec2::Y * S * 0.5, 1.0, 0.0);
    let mut tris = Vec::new();
    for _ in 0..4 + p.rng.below(4) {
        let c = Vec2::new(p.rng.range(-10.0, 10.0), 0.4);
        let a = p.rng.range(-0.3, 0.3);
        let d = Vec2::from_angle(a) * p.rng.range(2.0, 3.5);
        let n = d.perp() * 0.25;
        tris.push([c - d - n, c + d - n, c + d + n]);
        tris.push([c - d - n, c + d + n, c - d + n]);
    }
    p.part(e, triangles(&tris), M::Enamel, 0.0);
    if p.rng.chance(0.5) {
        let x = p.rng.range(-8.0, 6.0);
        let binder = [Vec2::new(x, -0.5), Vec2::new(x, 1.8), Vec2::new(x + 6.0, 1.8), Vec2::new(x + 6.0, -0.5)];
        p.shape(e, &binder, 0.6, M::Iron, 0.01);
    }
}

/// A fluorescent fixture hanging from the ceiling on chains, one end dropped.
fn ceiling_light(p: &mut Props, e: Entity) {
    let w = p.rng.range(9.0, 13.0);
    let la = p.rng.range(5.0, 12.0);
    let lb = if p.rng.chance(0.5) { p.rng.range(14.0, 26.0) } else { p.rng.range(5.0, 12.0) };
    let (a, b) = (Vec2::new(-w, -la), Vec2::new(w, -lb));
    for (top, end) in [(Vec2::new(-w, 1.0), a), (Vec2::new(w, 1.0), b)] {
        p.part(e, stroke(&[top, end], |_| 0.5, 1.0, 1.0, |t| if (t * 8.0) as i32 % 2 == 0 { 1.0 } else { 0.5 }), M::Iron, 0.0);
    }
    let d = (b - a).normalize();
    let n = d.perp();
    let housing = [a, b, b - n * 2.6, a - n * 2.6];
    p.shape(e, &housing, 0.6, M::Steel, 0.01);
    // The tube, dead white, snapped partway.
    let cut = p.rng.range(0.5, 1.0);
    p.part(e, stroke(&[a - n * 3.2 + d, a.lerp(b, cut) - n * 3.2], |_| 1.2, 1.0, 1.0, |_| 1.0), M::Rim, 0.02);
}

/// Venetian blinds over a window in the back wall, the lower slats hanging askew.
fn blinds(p: &mut Props, e: Entity) {
    let drop = p.rng.range(10.0, 22.0);
    let (w, h) = (p.rng.range(10.0, 14.0), p.rng.range(16.0, 24.0));
    let top = -drop;
    let win = [Vec2::new(-w, top - h - 2.0), Vec2::new(-w, top + 1.0), Vec2::new(w, top + 1.0), Vec2::new(w, top - h - 2.0)];
    p.shape(e, &win, 0.7, M::Soot, -0.01);
    let mut frame = win.to_vec();
    frame.push(win[0]);
    p.part(e, stroke(&frame, |_| 1.4, 1.0, 0.5, |_| 1.0), M::Iron, -0.005);
    p.part(e, stroke(&[Vec2::new(-w - 1.0, top + 1.0), Vec2::new(w + 1.0, top + 1.0)], |_| 1.6, 1.0, 0.6, |_| 1.0), M::Steel, 0.0);
    let skew = p.rng.range(-0.25, 0.25);
    let n = (h / 2.2) as usize;
    for i in 0..n {
        let y = top - 1.5 - i as f32 * 2.2;
        let s = if i > n / 2 { skew * (i - n / 2) as f32 * 1.5 } else { 0.0 };
        p.part(e, stroke(&[Vec2::new(-w, y), Vec2::new(w, y + s * w)], |_| 1.1, 1.0, 0.5, |_| 1.0), M::Enamel, 0.01);
    }
    p.part(e, stroke(&[Vec2::new(w - 1.5, top), Vec2::new(w - 1.0, top - h - 6.0)], |_| 0.4, 1.0, 1.0, |_| 1.0), M::Soot, 0.02);
}
