//! Props of "region: buried grand central subway station" (`Style::Subway`).
//! On the floors: long waiting-room benches (sagging, snapped), rows of ticket
//! turnstiles (one knocked flat), fallen split-flap departure boards, the
//! brass four-faced clock on its column (or rolled off its snapped stump),
//! abandoned luggage, crushed sections of a train car and heaps of marble.
//! The electrified tracks feature gets its ballast, sleepers, running rail and
//! the boarded third rail on insulators, sparking here and there.
//! Platform edges keep their bullnose lip, with cables, a dangling handrail or
//! a bent rail hanging over the drop; walls carry mosaic station plaques,
//! dead wall lamps and conduit runs; ceilings hang dead pendant lamps, a
//! chandelier now and then, and directional signs swinging off one rod.

use std::f32::consts::{FRAC_PI_2, PI, TAU};

use super::*;

pub(super) const HOOKS: Hooks = Hooks { spacing, ground, edge, wall, ceiling };

/// (gap between props of the same kind, chance a free spot gets one), in prop units.
fn spacing(kind: Kind) -> (f32, f32) {
    match kind {
        Kind::Ground => (55.0, 0.8),
        Kind::Edge => (40.0, 0.8),
        Kind::Wall => (55.0, 0.75),
        Kind::Ceiling => (38.0, 0.75),
    }
}

#[derive(Clone, Copy)]
enum Thing {
    Bench,
    Turnstiles,
    Board,
    Clock,
    Luggage,
    Car,
    Marble,
}

impl Thing {
    /// (half width, height) in prop units.
    fn size(self) -> (f32, f32) {
        match self {
            Thing::Bench => (30.0, 28.0),
            Thing::Turnstiles => (30.0, 26.0),
            Thing::Board => (34.0, 34.0),
            Thing::Clock => (12.0, 58.0),
            Thing::Luggage => (14.0, 16.0),
            Thing::Car => (58.0, 48.0),
            Thing::Marble => (16.0, 14.0),
        }
    }
}

/// A top surface with headroom at `cell` (`x` = its world x). True if something was placed.
fn ground(p: &mut Props, cell: IVec2, x: f32) -> bool {
    tracks_once(p);
    use Thing::*;
    let table: &[(Thing, f32)] = &[(Bench, 2.0), (Turnstiles, 1.5), (Board, 1.3), (Clock, 0.8), (Luggage, 1.5), (Car, 1.0), (Marble, 2.0)];
    let total: f32 = table.iter().map(|e| e.1).sum();
    let mut r = p.rng.range(0.0, total);
    let pick = table.iter().find(|e| {
        r -= e.1;
        r <= 0.0
    });
    let mut pick = pick.unwrap_or(&table[0]).0;
    let here = Vec2::new(x, p.t.cell_min(cell).y + PX);
    // Keep the track bed clear.
    if track_rects(p).iter().any(|r| r.inflate(4.0 * PX).contains(here)) {
        return false;
    }
    // Cars are long: only where nothing else stands near.
    if matches!(pick, Car) && p.taken.iter().any(|(q, k)| *k == Kind::Ground && q.distance(here) < 110.0 * S) {
        pick = Bench;
    }
    for g in [pick, Marble] {
        let (half, head) = g.size();
        let tilt = matches!(g, Marble | Luggage | Board | Car);
        let Some((y, angle, indoor)) = p.fit(cell, half, head, tilt) else { continue };
        let at = Vec2::new(x, y);
        p.tilt = angle;
        p.depth = if indoor { INDOOR } else { BACK };
        match g {
            Bench => bench(p, at),
            Turnstiles => turnstiles(p, at),
            Board => board(p, at),
            Clock => clock(p, at),
            Luggage => luggage(p, at),
            Car => car(p, at),
            Marble => marble(p, at),
        }
        p.tilt = 0.0;
        p.depth = BACK;
        return true;
    }
    false
}

/// Where the ground ends at a drop (`flip` = the open side).
fn edge(p: &mut Props, at: Vec2, flip: f32) -> bool {
    tracks_once(p);
    let e = p.root(at, flip, 0.0);
    // The platform's bullnose lip, with its row of warning studs.
    p.bar(e, &[Vec2::new(-16.0, -1.2), Vec2::new(2.5, -1.2)], 2.6, M::Rim, 0.02);
    p.bar(e, &[Vec2::new(-14.0, -3.4), Vec2::new(1.5, -3.4)], 1.8, M::Soot, 0.01);
    let studs: Vec<[Vec2; 3]> = (0..5).map(|i| {
        let x = -14.0 + i as f32 * 3.2;
        [Vec2::new(x - 0.8, -0.2), Vec2::new(x + 0.8, -0.2), Vec2::new(x, 0.8)]
    }).collect();
    p.part(e, triangles(&studs), M::Enamel, 0.03);
    match p.rng.below(3) {
        0 => {
            let n = 1 + p.rng.below(3);
            p.cables(e, Vec2::new(-2.0, -4.0), n, 0.0);
        }
        1 => handrail(p, e),
        _ => bent_rail(p, e),
    }
    true
}

/// A straight vertical face (`flip` = the open side).
fn wall(p: &mut Props, at: Vec2, flip: f32) -> bool {
    tracks_once(p);
    let inset = |w: f32| at + Vec2::new(-flip * (w * S + PX * 2.0), 0.0);
    let roll = p.rng.next();
    if roll < 0.45 {
        let half = Vec2::new(28.0, 8.0);
        let c = inset(half.x);
        if p.inside(c, half * S) {
            let e = p.root(c, flip, 0.0);
            plaque(p, e, half);
            return true;
        }
    }
    let e = p.root(at, flip, 0.0);
    if roll < 0.75 {
        wall_lamp(p, e);
    } else {
        conduit(p, e);
    }
    true
}

/// The underside of a thick overhang.
fn ceiling(p: &mut Props, at: Vec2) -> bool {
    tracks_once(p);
    let e = p.root(at, 1.0, 0.0);
    let roll = p.rng.next();
    if roll < 0.4 {
        pendant(p, e);
    } else if roll < 0.55 {
        chandelier(p, e);
    } else if roll < 0.85 {
        hanging_sign(p, e);
    } else {
        let n = 1 + p.rng.below(3);
        let x = p.rng.range(-6.0, 6.0);
        p.cables(e, Vec2::new(x, 2.0), n, -1.0);
    }
    true
}

// --- Helpers ---

fn rect(c: Vec2, half: Vec2) -> Vec<Vec2> {
    vec![c + Vec2::new(-half.x, -half.y), c + Vec2::new(-half.x, half.y), c + Vec2::new(half.x, half.y), c + Vec2::new(half.x, -half.y)]
}

fn line(p: &mut Props, e: Entity, a: Vec2, b: Vec2, w: f32, m: M, z: f32) {
    p.part(e, stroke(&[a, b], |_| w, 1.0, 0.6, |_| 1.0), m, z);
}

/// A chain from `a` to `b` (links as a dashed stroke).
fn chain(p: &mut Props, e: Entity, a: Vec2, b: Vec2, z: f32) {
    let links = ((b - a).length() / 2.2).max(2.0);
    p.part(e, stroke(&[a, b], |_| 1.3, 1.0, 0.7, move |t| if (t * links) as i32 % 2 == 0 { 1.0 } else { 0.45 }), M::Iron, z);
}

/// A few bright sparks bursting from `at`.
fn sparks(p: &mut Props, e: Entity, at: Vec2, z: f32) {
    let mut tris = Vec::new();
    for _ in 0..5 + p.rng.below(5) {
        let a = p.rng.range(0.15, PI - 0.15);
        let len = p.rng.range(3.0, 9.0);
        let d = Vec2::from_angle(a);
        let tip = at + d * len;
        let side = d.perp() * 0.35;
        tris.push([at + side, at - side, tip]);
    }
    p.part(e, triangles(&tris), M::Rim, z);
    let glow: Vec<Vec2> = ellipse_pts(1.6, 1.3, 10).into_iter().map(|q| q + at).collect();
    p.part(e, fill(&glow, |_| 1.0), M::Rim, z + 0.001);
}

// --- The tracks ---

/// The tracks are drawn whole, the first time any subway site is visited
/// (a marker far outside the world in `taken` remembers it).
fn tracks_once(p: &mut Props) {
    const MARK: f32 = 1.0e9;
    if p.taken.iter().any(|(q, _)| q.x == MARK) {
        return;
    }
    p.taken.push((Vec2::new(MARK, MARK), Kind::Ceiling));
    for (i, r) in track_rects(p).into_iter().enumerate() {
        p.rng = Rng(0x5eed_7a11 ^ (i as u32 + 1).wrapping_mul(0x9e37_79b1));
        p.rng.next();
        track_bed(p, r);
    }
}

/// The rects of the track features in subway areas.
fn track_rects(p: &Props) -> Vec<Rect> {
    let t = p.t;
    t.features
        .iter()
        .filter(|f| {
            let n = f.name.to_lowercase();
            (n.contains("track") || n.contains("rail")) && t.style_at(f.rect.center()) == Style::Subway
        })
        .map(|f| f.rect)
        .collect()
}

/// Ballast, sleepers, the running rail and the third rail along the floor of `r`.
fn track_bed(p: &mut Props, r: Rect) {
    let t = p.t;
    let step = 4.0;
    let n = ((r.width() / step) as usize).max(2);
    let reach = r.height() + 6.0 * PX;
    let probe_y = r.center().y;
    let ys: Vec<Option<f32>> = (0..=n)
        .map(|i| {
            let x = r.min.x + i as f32 * step;
            t.surface(Vec2::new(x, probe_y), reach).filter(|&y| y > r.min.y - 4.0 * PX && y < r.max.y + 4.0 * PX)
        })
        .collect();
    // Runs of continuous floor.
    let mut runs: Vec<Vec<Vec2>> = Vec::new();
    let mut cur: Vec<Vec2> = Vec::new();
    for (i, y) in ys.iter().enumerate() {
        let x = r.min.x + i as f32 * step;
        match y {
            Some(y) if cur.last().is_none_or(|q: &Vec2| (q.y - y).abs() <= 2.0 * PX) => cur.push(Vec2::new(x, *y)),
            Some(y) => {
                runs.push(std::mem::take(&mut cur));
                cur.push(Vec2::new(x, *y));
            }
            None => runs.push(std::mem::take(&mut cur)),
        }
    }
    runs.push(cur);
    let mut sign = true;
    for run in runs.into_iter().filter(|r| r.len() >= 8) {
        // Smooth the 1-px stairs into ramps.
        let smooth: Vec<Vec2> = (0..run.len())
            .map(|i| {
                let (a, b) = (i.saturating_sub(4), (i + 4).min(run.len() - 1));
                let y = run[a..=b].iter().map(|q| q.y).sum::<f32>() / (b - a + 1) as f32;
                Vec2::new(run[i].x, y)
            })
            .collect();
        // Chunks of ~100 world units, each its own depth (indoors or not).
        for chunk in smooth.chunks(26) {
            if chunk.len() < 3 {
                continue;
            }
            track_chunk(p, chunk, sign);
            sign = false;
        }
    }
}

fn track_chunk(p: &mut Props, pts: &[Vec2], sign: bool) {
    let t = p.t;
    let o = pts[0];
    let mid = pts[pts.len() / 2];
    let c = t.to_cell(mid + Vec2::Y * PX * 0.5);
    let indoor = (1..=4).any(|k| t.cell(c - IVec2::Y * k) == Cell::Back);
    p.depth = if indoor { INDOOR } else { BACK };
    let e = p.root(o, 1.0, 0.0);
    let local: Vec<Vec2> = pts.iter().map(|q| (*q - o) / S).collect();
    let up = |dy: f32| -> Vec<Vec2> { local.iter().map(|q| *q + Vec2::Y * dy).collect() };
    let (x0, x1) = (local[0].x, local[local.len() - 1].x);
    let y_at = |x: f32| -> f32 {
        let i = local.partition_point(|q| q.x < x).min(local.len() - 1);
        local[i].y
    };
    // Ballast: a low bank of stones.
    let mut bank = up(1.6);
    bank.push(Vec2::new(x1, local[local.len() - 1].y - 3.0));
    bank.push(Vec2::new(x0, local[0].y - 3.0));
    p.shape(e, &bank, 0.5, M::Concrete, 0.0);
    let mut x = x0 + p.rng.range(0.0, 2.0);
    while x < x1 {
        let rr = p.rng.range(0.8, 1.6);
        let pts: Vec<Vec2> = rock_pts(&mut p.rng, rr, 6).into_iter().map(|q| q + Vec2::new(x, y_at(x) + 1.2)).collect();
        let hotfix_roll_1 = p.rng.chance(0.3); // HOTFIX: evaluate before borrowing Props mutably (E0499).
        p.shape(e, &pts, 0.5, if hotfix_roll_1 { M::Steel } else { M::Concrete }, 0.005);
        x += p.rng.range(1.5, 3.0);
    }
    // Sleepers, a few scorched or split.
    let mut x = x0 + p.rng.range(1.0, 5.0);
    while x < x1 - 2.0 {
        let y = y_at(x);
        let m = if p.rng.chance(0.15) { M::Soot } else { M::Wood };
        let h = if p.rng.chance(0.1) { 1.8 } else { 3.2 };
        p.shape(e, &rect(Vec2::new(x, y + 1.6 + h / 2.0 - 1.0), Vec2::new(3.2, h / 2.0)), 0.5, m, 0.01);
        x += p.rng.range(8.0, 10.0);
    }
    // The running rail: foot, web and the polished head.
    p.part(e, stroke(&up(3.6), |_| 1.0, 1.0, 0.6, |_| 1.0), M::Iron, 0.02);
    p.part(e, stroke(&up(4.6), |_| 1.4, 0.7, 0.5, |_| 1.0), M::Iron, 0.021);
    p.part(e, stroke(&up(5.6), |_| 1.1, 1.0, 0.8, |_| 1.0), M::Rim, 0.022);
    // The third rail on ceramic insulators, under its wooden cover board.
    let mut x = x0 + p.rng.range(2.0, 8.0);
    while x < x1 {
        let y = y_at(x);
        p.shape(e, &rect(Vec2::new(x, y + 7.0), Vec2::new(1.4, 1.6)), 0.6, M::Enamel, 0.03);
        for k in 0..2 {
            line(p, e, Vec2::new(x - 1.6, y + 6.2 + k as f32 * 1.1), Vec2::new(x + 1.6, y + 6.2 + k as f32 * 1.1), 0.4, M::Soot, 0.031);
        }
        // Bracket up to the cover board.
        p.bar(e, &[Vec2::new(x + 2.0, y + 6.0), Vec2::new(x + 2.6, y + 10.0), Vec2::new(x + 1.5, y + 13.4)], 0.8, M::Iron, 0.03);
        x += p.rng.range(20.0, 28.0);
    }
    p.part(e, stroke(&up(9.2), |_| 2.2, 1.0, 0.5, |_| 1.0), M::Steel, 0.04);
    p.part(e, stroke(&up(10.1), |_| 0.6, 1.0, 1.0, |_| 1.0), M::Rim, 0.041);
    // The cover board, broken off in places so the rail shows.
    let mut a = x0;
    while a < x1 {
        let len = p.rng.range(14.0, 30.0);
        let b = (a + len).min(x1);
        if p.rng.chance(0.75) {
            let seg: Vec<Vec2> = local.iter().filter(|q| q.x >= a && q.x <= b).map(|q| *q + Vec2::Y * 13.6).collect();
            if seg.len() >= 2 {
                p.part(e, stroke(&seg, |_| 1.8, 1.0, 0.5, |_| 1.0), M::Wood, 0.05);
            }
        }
        a = b + p.rng.range(2.0, 6.0);
    }
    // Live: arcs where the shoe gear once ran.
    let mut x = x0 + p.rng.range(4.0, 16.0);
    while x < x1 {
        if p.rng.chance(0.55) {
            let y = y_at(x);
            sparks(p, e, Vec2::new(x, y + 10.0), OVER);
            let scorch: Vec<Vec2> = ellipse_pts(p.rng.range(3.0, 5.0), 1.2, 10).into_iter().map(|q| q + Vec2::new(x, y + 2.0)).collect();
            p.part(e, fill(&scorch, |_| 1.0), M::Soot, 0.015);
        }
        x += p.rng.range(14.0, 24.0);
    }
    if sign {
        danger_sign(p, e, Vec2::new(x0 + 4.0, local[0].y));
    }
    p.depth = BACK;
}

/// A warning plate on a post at the start of the tracks: a lightning bolt.
fn danger_sign(p: &mut Props, e: Entity, base: Vec2) {
    p.bar(e, &[base + Vec2::new(0.0, -2.0), base + Vec2::new(0.4, 24.0)], 1.6, M::Iron, 0.06);
    let c = base + Vec2::new(0.4, 28.0);
    let plate = [c + Vec2::new(0.0, -6.0), c + Vec2::new(-6.0, 0.0), c + Vec2::new(0.0, 6.0), c + Vec2::new(6.0, 0.0)];
    p.shape(e, &plate, 0.7, M::Enamel, 0.07);
    let mut rim = plate.to_vec();
    rim.push(plate[0]);
    p.part(e, stroke(&rim, |_| 0.9, 1.0, 1.0, |_| 1.0), M::Soot, 0.071);
    let bolt = [[c + Vec2::new(1.0, 4.2), c + Vec2::new(-2.2, -0.4), c + Vec2::new(0.3, 0.2)], [c + Vec2::new(-0.3, -0.2), c + Vec2::new(2.2, 0.4), c + Vec2::new(-1.0, -4.2)]];
    p.part(e, triangles(&bolt), M::Soot, 0.072);
}

// --- Floor props ---

/// A long waiting-room bench: carved ends, a thick seat, a slatted back;
/// one end sagging, slats snapped.
fn bench(p: &mut Props, at: Vec2) {
    let flip = if p.rng.chance(0.5) { 1.0 } else { -1.0 };
    let sag = if p.rng.chance(0.6) { p.rng.range(0.06, 0.16) } else { 0.0 };
    let e = p.root(at - Vec2::Y * S * 2.0, flip, -sag);
    let w = 27.0;
    for (k, x) in [-w + 2.0, w - 2.0].into_iter().enumerate() {
        if k == 1 && sag > 0.0 {
            // That end gave way: the panel lies flat under the seat.
            p.shape(e, &rect(Vec2::new(x - 4.0, 1.5), Vec2::new(6.0, 1.5)), 0.5, M::Wood, 0.0);
            continue;
        }
        let panel = [Vec2::new(x - 4.0, 0.0), Vec2::new(x - 3.0, 12.0), Vec2::new(x - 5.0, 14.0), Vec2::new(x - 4.0, 17.0), Vec2::new(x + 1.0, 17.0), Vec2::new(x + 3.0, 30.0), Vec2::new(x + 5.0, 30.0), Vec2::new(x + 4.0, 0.0)];
        p.shape(e, &panel, 0.45, M::Wood, 0.0);
        // Armrest scroll.
        p.ring(e, Vec2::new(x - 3.0, 16.0), 1.8, 1.0, M::Iron, 0.01, |_| 1.0);
    }
    p.shape(e, &rect(Vec2::new(0.0, 12.5), Vec2::new(w, 1.8)), 0.5, M::Wood, 0.02);
    line(p, e, Vec2::new(-w, 14.2), Vec2::new(w, 14.2), 0.9, M::Rim, 0.025);
    // Which back slat is snapped (3 or 4 = none).
    let snap = p.rng.below(5);
    for (i, y) in [18.5, 22.5, 26.5].into_iter().enumerate() {
        let lean = 2.0 * (y - 14.0) / 14.0;
        if i == snap {
            let cut = p.rng.range(-8.0, 10.0);
            let hotfix_roll_1 = p.rng.range(-1.0, 0.5); // HOTFIX: evaluate before borrowing Props mutably (E0499).
            line(p, e, Vec2::new(-w + lean, y), Vec2::new(cut, y + hotfix_roll_1), 2.2, M::Wood, 0.02);
            line(p, e, Vec2::new(cut + 4.0, y - 3.0), Vec2::new(w + lean, y - 1.0), 2.0, M::Wood, 0.019);
        } else {
            line(p, e, Vec2::new(-w + lean, y), Vec2::new(w + lean, y), 2.2, M::Wood, 0.02);
        }
    }
    if p.rng.chance(0.5) {
        // A coat left on it.
        let c = Vec2::new(p.rng.range(-14.0, 10.0), 14.0);
        let coat = [c, c + Vec2::new(4.0, 7.0), c + Vec2::new(10.0, 6.0), c + Vec2::new(12.0, 0.0), c + Vec2::new(13.0, -9.0), c + Vec2::new(9.0, -8.0), c + Vec2::new(7.0, -1.0)];
        p.shape(e, &coat, 0.4, M::Iron, 0.03);
    }
}

/// A row of ticket turnstiles; one torn off its bolts and lying flat.
fn turnstiles(p: &mut Props, at: Vec2) {
    let n = 2 + p.rng.below(2);
    let flat = p.rng.below(n + 1);
    let gap = 22.0;
    for i in 0..n {
        let x = (i as f32 - (n - 1) as f32 / 2.0) * gap;
        let down = i == flat;
        let base = at + Vec2::new(x * S, 0.0);
        let (pos, angle) = if down { (base + Vec2::new(0.0, -S * 1.0), FRAC_PI_2 * 0.96) } else { (base - Vec2::Y * S * 2.0, p.rng.range(-0.05, 0.05)) };
        let e = p.root(pos, 1.0, angle);
        let off = if down { Vec2::new(0.0, -5.0) } else { Vec2::ZERO };
        // Cabinet with a rounded top and a slanted ticket slot.
        let body = smooth_closed(&[Vec2::new(-5.0, 0.0) + off, Vec2::new(-5.0, 17.0) + off, Vec2::new(-3.5, 19.5) + off, Vec2::new(3.5, 19.5) + off, Vec2::new(5.0, 17.0) + off, Vec2::new(5.0, 0.0) + off], 2);
        p.shape(e, &body, 0.45, M::Steel, 0.0);
        line(p, e, Vec2::new(-4.5, 18.6) + off, Vec2::new(4.5, 18.6) + off, 1.2, M::Rim, 0.01);
        p.shape(e, &rect(Vec2::new(-1.5, 14.0) + off, Vec2::new(2.0, 0.6)), 1.0, M::Soot, 0.01);
        // Tripod arms from the hub on the side.
        let hub = Vec2::new(5.5, 13.0) + off;
        let spin = p.rng.range(0.0, TAU);
        for k in 0..3 {
            let a = spin + k as f32 * TAU / 3.0;
            let d = Vec2::new(a.cos(), a.sin() * 0.55);
            let len = if p.rng.chance(0.15) { 4.0 } else { 11.0 };
            line(p, e, hub, hub + d * len, 1.5, M::Steel, 0.02 + 0.001 * k as f32);
        }
        p.ring(e, hub, 1.2, 1.2, M::Iron, 0.03, |_| 1.0);
    }
}

/// A split-flap departure board, fallen: leaning into the floor, or still on its legs.
fn board(p: &mut Props, at: Vec2) {
    let (w, h) = (31.0, 15.0);
    let leaning = p.rng.chance(0.55);
    let e = if leaning {
        let a = p.rng.range(0.25, 0.45) * if p.rng.chance(0.5) { 1.0 } else { -1.0 };
        p.root(at - Vec2::Y * S * 4.0, 1.0, a)
    } else {
        let hotfix_roll_1 = p.rng.range(-0.05, 0.05); // HOTFIX: evaluate before borrowing Props mutably (E0499).
        p.root(at - Vec2::Y * S * 2.0, 1.0, hotfix_roll_1)
    };
    let c = Vec2::new(0.0, if leaning { h } else { h + 14.0 });
    if !leaning {
        for x in [-w * 0.6, w * 0.6] {
            p.bar(e, &[Vec2::new(x, 0.0), Vec2::new(x, c.y - h)], 2.2, M::Iron, 0.0);
        }
    }
    p.shape(e, &rect(c, Vec2::new(w + 2.0, h + 2.0)), 0.5, M::Iron, 0.01);
    p.shape(e, &rect(c, Vec2::new(w, h)), 0.8, M::Soot, 0.02);
    // Rows of flaps; some fell out, some show half a letter.
    let (cols, rows) = (13, 5);
    let (fw, fh) = (2.0 * w / cols as f32, 2.0 * h / rows as f32);
    let mut tris = Vec::new();
    let mut dim = Vec::new();
    for j in 0..rows {
        for i in 0..cols {
            let r = p.rng.next();
            if r < 0.18 || (i == 4 && j > 0) {
                continue;
            }
            let lo = Vec2::new(-w + i as f32 * fw + 0.4, c.y - h + j as f32 * fh + 0.4);
            let hi = lo + Vec2::new(fw - 0.8, fh - 0.8);
            let quad = [[lo, Vec2::new(hi.x, lo.y), hi], [lo, hi, Vec2::new(lo.x, hi.y)]];
            if r < 0.65 { tris.extend(quad) } else { dim.extend(quad) }
            // The split across the middle of every flap.
            let m = (lo.y + hi.y) / 2.0;
            let gap = [[Vec2::new(lo.x, m - 0.15), Vec2::new(hi.x, m - 0.15), Vec2::new(hi.x, m + 0.15)], [Vec2::new(lo.x, m - 0.15), Vec2::new(hi.x, m + 0.15), Vec2::new(lo.x, m + 0.15)]];
            dim.extend(gap);
        }
    }
    p.part(e, triangles(&tris), M::Enamel, 0.03);
    p.part(e, triangles(&dim), M::Iron, 0.031);
    // A crack across the glass.
    let x0 = p.rng.range(-w * 0.6, w * 0.3);
    let crack = [Vec2::new(x0, c.y + h), Vec2::new(x0 + 4.0, c.y + 3.0), Vec2::new(x0 + 2.0, c.y - 4.0), Vec2::new(x0 + 8.0, c.y - h)];
    p.part(e, stroke(&crack, |t| 1.0 - 0.5 * t, 1.0, 1.0, |_| 1.0), M::Rim, 0.04);
}

/// The brass four-faced clock on its column, or rolled off its snapped stump.
fn clock(p: &mut Props, at: Vec2) {
    let flip = if p.rng.chance(0.5) { 1.0 } else { -1.0 };
    let e = p.root(at - Vec2::Y * S * 2.0, flip, 0.0);
    let fallen = p.rng.chance(0.45);
    let top = if fallen { p.rng.range(14.0, 24.0) } else { 36.0 };
    // Fluted column on a plinth.
    p.shape(e, &rect(Vec2::new(0.0, 2.0), Vec2::new(8.0, 3.0)), 0.5, M::Concrete, 0.0);
    let mut col = vec![Vec2::new(-5.5, 4.0), Vec2::new(-5.0, top)];
    if fallen {
        col.push(Vec2::new(-1.5, top + 3.0));
        col.push(Vec2::new(1.0, top - 1.5));
        col.push(Vec2::new(5.0, top + 1.5));
    } else {
        col.push(Vec2::new(5.0, top));
    }
    col.push(Vec2::new(5.5, 4.0));
    p.shape(e, &col, 0.5, M::Enamel, 0.01);
    for x in [-3.0, 0.0, 3.0] {
        line(p, e, Vec2::new(x, 5.0), Vec2::new(x, top - 2.0), 0.6, M::Concrete, 0.015);
    }
    let globe = if fallen {
        Vec2::new(p.rng.range(16.0, 22.0), 8.5)
    } else {
        p.shape(e, &rect(Vec2::new(0.0, top + 1.5), Vec2::new(7.0, 1.5)), 0.6, M::Bronze, 0.02);
        Vec2::new(0.0, top + 11.5)
    };
    // Brass ball with a face; the acorn finial on top (snapped off if fallen).
    let ball: Vec<Vec2> = ellipse_pts(9.0, 9.0, 24).into_iter().map(|q| q + globe).collect();
    p.shape(e, &ball, 0.45, M::Bronze, 0.03);
    let face: Vec<Vec2> = ellipse_pts(5.8, 6.2, 20).into_iter().map(|q| q + globe + Vec2::new(-0.8, 0.3)).collect();
    p.shape(e, &face, 0.85, M::Enamel, 0.04);
    let fc = globe + Vec2::new(-0.8, 0.3);
    let mut ticks = Vec::new();
    for i in 0..12 {
        let d = Vec2::from_angle(i as f32 * TAU / 12.0);
        let (a, b) = (fc + d * 4.4, fc + d * 5.4);
        ticks.push([a + d.perp() * 0.3, a - d.perp() * 0.3, b]);
    }
    p.part(e, triangles(&ticks), M::Soot, 0.045);
    // Stopped when the roof came down.
    let (ha, ma) = (p.rng.range(0.0, TAU), p.rng.range(0.0, TAU));
    line(p, e, fc, fc + Vec2::from_angle(ha) * 2.8, 0.9, M::Soot, 0.05);
    line(p, e, fc, fc + Vec2::from_angle(ma) * 4.3, 0.6, M::Soot, 0.05);
    if !fallen {
        let f = globe + Vec2::Y * 9.0;
        let acorn = [f + Vec2::new(-1.8, 0.0), f + Vec2::new(-1.2, 3.0), f + Vec2::new(0.0, 5.0), f + Vec2::new(1.2, 3.0), f + Vec2::new(1.8, 0.0)];
        p.shape(e, &acorn, 0.5, M::Bronze, 0.035);
    } else {
        let crack = [globe + Vec2::new(4.0, 8.0), globe + Vec2::new(2.0, 3.0), globe + Vec2::new(4.5, -1.0)];
        p.part(e, stroke(&crack, |t| 1.0 - 0.5 * t, 1.0, 1.0, |_| 1.0), M::Soot, 0.046);
    }
}

/// Suitcases and a hatbox, dropped when the station was buried.
fn luggage(p: &mut Props, at: Vec2) {
    let e = p.root(at - Vec2::Y * S * 1.5, 1.0, 0.0);
    let n = 2 + p.rng.below(2);
    let mut x = -10.0;
    for i in 0..n {
        let (w, h) = (p.rng.range(5.0, 9.0), p.rng.range(4.0, 7.0));
        let stacked = i == n - 1 && n == 3;
        let c = if stacked { Vec2::new(p.rng.range(-6.0, 4.0), 2.0 * 6.0 + h - 1.0) } else { Vec2::new(x + w, h) };
        let body = smooth_closed(&rect(c, Vec2::new(w, h)), 2);
        let m = if p.rng.chance(0.5) { M::Wood } else { M::Iron };
        p.shape(e, &body, 0.5, m, 0.01 * i as f32);
        // Straps and the handle.
        for s in [-0.5, 0.5] {
            line(p, e, Vec2::new(c.x + s * w, c.y - h), Vec2::new(c.x + s * w, c.y + h), 0.9, M::Soot, 0.01 * i as f32 + 0.003);
        }
        let hdl = smooth_open(&[c + Vec2::new(-2.0, h), c + Vec2::new(-1.5, h + 2.0), c + Vec2::new(1.5, h + 2.0), c + Vec2::new(2.0, h)], 3);
        p.part(e, stroke(&hdl, |_| 0.9, 1.0, 0.6, |_| 1.0), M::Soot, 0.01 * i as f32 + 0.004);
        if p.rng.chance(0.4) {
            // Burst open: a sleeve hanging out.
            let s = [c + Vec2::new(w * 0.6, -h * 0.2), c + Vec2::new(w + 3.0, -h * 0.5), c + Vec2::new(w + 4.0, -h)];
            p.bar(e, &s, 2.0, M::Concrete, 0.01 * i as f32 + 0.005);
        }
        if !stacked {
            x += 2.0 * w + p.rng.range(1.0, 4.0);
        }
    }
}

/// A crushed section of a train car: ribbed steel, a window band, a door, the
/// roof buckled down where the vault fell on it, a bogie half in the floor.
fn car(p: &mut Props, at: Vec2) {
    let flip = if p.rng.chance(0.5) { 1.0 } else { -1.0 };
    let hotfix_roll_1 = p.rng.range(-0.06, 0.06); // HOTFIX: evaluate before borrowing Props mutably (E0499).
    let e = p.root(at - Vec2::Y * S * 6.0, flip, hotfix_roll_1);
    let (w, h) = (54.0, 40.0);
    let dent = p.rng.range(8.0, 16.0);
    let dx = p.rng.range(-10.0, 18.0);
    // Wheels of the bogie first (behind the body).
    for x in [-w * 0.7, -w * 0.4] {
        p.ring(e, Vec2::new(x, 5.0), 4.5, 2.6, M::Iron, 0.0, |_| 1.0);
    }
    // Body outline: torn ends, buckled roof.
    let mut body = vec![Vec2::new(-w, 4.0), Vec2::new(-w - 1.0, h * 0.5), Vec2::new(-w + 1.5, h * 0.62), Vec2::new(-w, h - 4.0), Vec2::new(-w + 4.0, h)];
    for i in 1..8 {
        let x = -w + 4.0 + i as f32 * (2.0 * w - 8.0) / 8.0;
        let d = (x - dx) / 22.0;
        body.push(Vec2::new(x, h - dent * (-d * d).exp()));
    }
    body.extend([Vec2::new(w - 4.0, h - 1.0), Vec2::new(w + 1.5, h - 7.0), Vec2::new(w - 1.0, h * 0.45), Vec2::new(w + 2.0, h * 0.3), Vec2::new(w, 4.0)]);
    p.shape(e, &body, 0.45, M::Steel, 0.01);
    // Corrugated skirt.
    let mut ribs = Vec::new();
    let mut x = -w + 2.0;
    while x < w - 2.0 {
        ribs.push([Vec2::new(x, 5.0), Vec2::new(x + 0.7, 5.0), Vec2::new(x + 0.35, h * 0.38)]);
        ribs.push([Vec2::new(x, 5.0), Vec2::new(x + 0.35, h * 0.38), Vec2::new(x - 0.2, h * 0.38)]);
        x += 2.6;
    }
    p.part(e, triangles(&ribs), M::Iron, 0.015);
    line(p, e, Vec2::new(-w, h * 0.42), Vec2::new(w, h * 0.42), 1.4, M::Rim, 0.016);
    // Window band (black, glass shards in the corners), the door in the middle.
    let wy = h * 0.66;
    let roof_at = |x: f32| h - dent * (-((x - dx) / 22.0).powi(2)).exp();
    let door_x = p.rng.range(-w * 0.2, w * 0.3);
    let mut x = -w + 6.0;
    while x < w - 10.0 {
        let ww = 9.0;
        if (x + ww / 2.0 - door_x).abs() > 10.0 {
            let top = (wy + 6.0).min(roof_at(x + ww / 2.0) - 4.0);
            if top > wy - 4.0 {
                let pane = [Vec2::new(x, wy - 5.0), Vec2::new(x, top), Vec2::new(x + ww, top), Vec2::new(x + ww, wy - 5.0)];
                p.shape(e, &pane, 0.8, M::Soot, 0.02);
                if p.rng.chance(0.6) {
                    let s = [[Vec2::new(x, top), Vec2::new(x + p.rng.range(2.0, 5.0), top), Vec2::new(x, top - p.rng.range(2.0, 6.0))]];
                    p.part(e, triangles(&s), M::Rim, 0.025);
                }
            }
        }
        x += ww + 3.0;
    }
    let door_top = (h - 6.0).min(roof_at(door_x) - 3.0);
    p.shape(e, &rect(Vec2::new(door_x, (5.0 + door_top) / 2.0), Vec2::new(8.0, (door_top - 5.0) / 2.0)), 0.6, M::Iron, 0.02);
    line(p, e, Vec2::new(door_x, 5.0), Vec2::new(door_x, door_top), 0.7, M::Soot, 0.022);
    for s in [-1.0, 1.0] {
        let wc = Vec2::new(door_x + s * 4.0, door_top - 7.0);
        p.shape(e, &rect(wc, Vec2::new(2.2, 4.0)), 0.8, M::Soot, 0.023);
    }
    // Rubble from the vault on the dented roof.
    for _ in 0..3 + p.rng.below(3) {
        let x = dx + p.rng.range(-12.0, 12.0);
        let r = p.rng.range(3.0, 6.0);
        let pts: Vec<Vec2> = rock_pts(&mut p.rng, r, 7).into_iter().map(|q| q + Vec2::new(x, roof_at(x) + r * 0.4)).collect();
        p.shape(e, &pts, 0.45, M::Concrete, 0.03);
    }
    // Rust bleeding from the seams.
    for _ in 0..2 {
        let x = p.rng.range(-w * 0.8, w * 0.8);
        let y = roof_at(x) - 2.0;
        let hotfix_roll_1 = p.rng.range(8.0, 18.0); // HOTFIX: evaluate before borrowing Props mutably (E0499).
        p.part(e, stroke(&[Vec2::new(x, y), Vec2::new(x + 0.4, y - hotfix_roll_1)], |t| 2.2 * (1.0 - t) + 0.3, 1.0, 1.0, |_| 1.0), M::Rust, 0.03);
    }
}

/// A heap of broken marble and tile shards.
fn marble(p: &mut Props, at: Vec2) {
    let e = p.root(at - Vec2::Y * S * 2.0, 1.0, 0.0);
    for i in 0..3 + p.rng.below(3) {
        let r = p.rng.range(3.5, 7.5);
        let c = Vec2::new(p.rng.range(-12.0, 12.0), r * 0.35);
        let pts: Vec<Vec2> = rock_pts(&mut p.rng, r, 6).into_iter().map(|q| q + c).collect();
        p.shape(e, &pts, 0.45, if i % 2 == 0 { M::Enamel } else { M::Concrete }, 0.01 * i as f32);
        if i % 2 == 0 {
            // A vein across the chunk.
            let v = [c + Vec2::new(-r * 0.6, p.rng.range(-1.0, 1.0)), c + Vec2::new(0.0, p.rng.range(-1.5, 1.5)), c + Vec2::new(r * 0.5, p.rng.range(-1.0, 1.0))];
            p.part(e, stroke(&smooth_open(&v, 3), |_| 0.5, 1.0, 1.0, |_| 1.0), M::Concrete, 0.01 * i as f32 + 0.005);
        }
    }
    // Tile shards scattered around.
    let mut tris = Vec::new();
    for _ in 0..5 {
        let c = Vec2::new(p.rng.range(-18.0, 18.0), p.rng.range(1.5, 3.0));
        let a = p.rng.range(0.0, TAU);
        let (d, q) = (Vec2::from_angle(a) * 1.8, Vec2::from_angle(a).perp() * 1.0);
        tris.push([c - d - q, c + d - q, c + d + q]);
        tris.push([c - d - q, c + d + q, c - d + q]);
    }
    p.part(e, triangles(&tris), M::Enamel, 0.06);
}

// --- Edges ---

/// The platform's handrail, snapped and hanging over the edge.
fn handrail(p: &mut Props, e: Entity) {
    let post = Vec2::new(-6.0, 0.0);
    p.bar(e, &[post, post + Vec2::new(0.0, 14.0)], 1.6, M::Steel, 0.0);
    let droop = p.rng.range(16.0, 30.0);
    let rail = [post + Vec2::new(-14.0, 14.0), post + Vec2::new(0.0, 14.0), post + Vec2::new(7.0, 11.0), post + Vec2::new(10.0, 14.0 - droop * 0.6), post + Vec2::new(9.0 + p.rng.range(-3.0, 3.0), 14.0 - droop)];
    p.bar(e, &rail, 1.4, M::Steel, 0.01);
}

/// A rail torn off its sleepers, bent down over the drop, a sleeper still bolted on.
fn bent_rail(p: &mut Props, e: Entity) {
    let len = p.rng.range(22.0, 38.0);
    let pts = [Vec2::new(-18.0, 1.0), Vec2::new(-2.0, 1.0), Vec2::new(5.0, -3.0), Vec2::new(8.0, -len * 0.6), Vec2::new(6.0 + p.rng.range(-3.0, 4.0), -len)];
    p.bar(e, &pts, 2.4, M::Iron, 0.0);
    let head: Vec<Vec2> = pts.iter().map(|q| *q + Vec2::new(-0.6, 0.6)).collect();
    p.bar(e, &head, 0.8, M::Rim, 0.005);
    let s = Vec2::new(7.0, -len * 0.55);
    let sleeper = [s + Vec2::new(-1.5, -3.5), s + Vec2::new(-1.0, 3.5), s + Vec2::new(2.5, 3.2), s + Vec2::new(2.0, -3.8)];
    p.shape(e, &sleeper, 0.5, M::Wood, 0.01);
}

// --- Walls ---

/// 3×5 bitmaps (rows top to bottom) of the station's name in mosaic.
const LETTERS: [[u8; 5]; 7] = [
    [0b111, 0b100, 0b100, 0b100, 0b111], // C
    [0b111, 0b100, 0b110, 0b100, 0b111], // E
    [0b101, 0b111, 0b111, 0b101, 0b101], // N
    [0b111, 0b010, 0b010, 0b010, 0b010], // T
    [0b110, 0b101, 0b110, 0b101, 0b101], // R
    [0b010, 0b101, 0b111, 0b101, 0b101], // A
    [0b100, 0b100, 0b100, 0b100, 0b111], // L
];

/// A mosaic station plaque on the tiled face ("CENTRAL"), tiles fallen out.
fn plaque(p: &mut Props, e: Entity, half: Vec2) {
    p.part(e, fill(&rect(Vec2::ZERO, half), |_| 0.9), M::Soot, OVER);
    let mut rim = rect(Vec2::ZERO, half + Vec2::splat(1.0));
    rim.push(rim[0]);
    p.part(e, stroke(&rim, |_| 2.2, 1.0, 0.6, |_| 1.0), M::Enamel, OVER + 0.01);
    // A border of small squares.
    let mut border = Vec::new();
    let mut x = -half.x + 1.5;
    while x < half.x - 1.0 {
        for y in [half.y - 1.6, -half.y + 1.6] {
            if p.rng.chance(0.85) {
                border.extend(square(Vec2::new(x, y), 0.55));
            }
        }
        x += 1.8;
    }
    p.part(e, triangles(&border), M::Bronze, OVER + 0.02);
    let cell = 1.6;
    let pitch = 4.0 * cell;
    let x0 = -(LETTERS.len() as f32 * pitch - cell) / 2.0;
    let mut tiles = Vec::new();
    for (k, l) in LETTERS.iter().enumerate() {
        for (j, row) in l.iter().enumerate() {
            for i in 0..3 {
                if row >> (2 - i) & 1 == 0 || p.rng.chance(0.12) {
                    continue;
                }
                let c = Vec2::new(x0 + k as f32 * pitch + (i as f32 + 0.5) * cell, (2.0 - j as f32) * cell);
                tiles.extend(square(c, cell * 0.42));
            }
        }
    }
    p.part(e, triangles(&tiles), M::Enamel, OVER + 0.02);
    // A chunk of the plaque fell off.
    if p.rng.chance(0.5) {
        let c = Vec2::new(p.rng.range(-half.x, half.x), if p.rng.chance(0.5) { half.y } else { -half.y });
        let hotfix_roll_1 = p.rng.range(3.0, 5.0); // HOTFIX: evaluate before borrowing Props mutably (E0499).
        let pts: Vec<Vec2> = rock_pts(&mut p.rng, hotfix_roll_1, 7).into_iter().map(|q| q + c).collect();
        p.part(e, fill(&pts, |_| 0.8), M::Soot, OVER + 0.03);
    }
    // Grime running down from it.
    let x = p.rng.range(-half.x * 0.8, half.x * 0.8);
    let hotfix_roll_1 = p.rng.range(8.0, 20.0); // HOTFIX: evaluate before borrowing Props mutably (E0499).
    p.part(e, stroke(&[Vec2::new(x, -half.y), Vec2::new(x + 0.4, -half.y - hotfix_roll_1)], |t| 3.0 * (1.0 - t) + 0.4, 1.0, 1.0, |_| 1.0), M::Soot, OVER - 0.01);
}

fn square(c: Vec2, r: f32) -> [[Vec2; 3]; 2] {
    let (a, b, cc, d) = (c + Vec2::new(-r, -r), c + Vec2::new(r, -r), c + Vec2::new(r, r), c + Vec2::new(-r, r));
    [[a, b, cc], [a, cc, d]]
}

/// A dead lamp on a curled wall bracket, its globe broken.
fn wall_lamp(p: &mut Props, e: Entity) {
    let y = p.rng.range(-4.0, 8.0);
    p.shape(e, &rect(Vec2::new(-1.0, y), Vec2::new(2.0, 4.0)), 0.5, M::Bronze, 0.0);
    let droop = p.rng.range(-2.0, 4.0);
    let arm = [Vec2::new(0.0, y + 1.0), Vec2::new(6.0, y + 3.0), Vec2::new(11.0, y + 1.0 - droop), Vec2::new(13.0, y - 2.0 - droop)];
    p.bar(e, &arm, 1.4, M::Bronze, 0.01);
    p.ring(e, Vec2::new(5.0, y - 1.5), 2.5, 1.0, M::Bronze, 0.01, |_| 1.0);
    let g = Vec2::new(13.0, y - 7.0 - droop);
    let globe: Vec<Vec2> = ellipse_pts(4.0, 4.6, 16).into_iter().map(|q| q + g).collect();
    if p.rng.chance(0.4) {
        // Only the socket and a jagged ring of glass left.
        p.shape(e, &rect(g + Vec2::Y * 4.0, Vec2::new(1.6, 1.4)), 0.6, M::Iron, 0.02);
        let shards = [[g + Vec2::new(-3.0, 3.0), g + Vec2::new(-1.0, 3.2), g + Vec2::new(-2.4, 0.0)], [g + Vec2::new(1.0, 3.2), g + Vec2::new(3.2, 2.8), g + Vec2::new(2.0, -0.8)]];
        p.part(e, triangles(&shards), M::Rim, 0.021);
    } else {
        p.shape(e, &globe, 0.5, M::Enamel, 0.02);
        p.shape(e, &rect(g + Vec2::Y * 4.6, Vec2::new(1.8, 1.0)), 0.6, M::Bronze, 0.021);
        let crack = [g + Vec2::new(-1.0, 3.0), g + Vec2::new(0.5, 0.5), g + Vec2::new(-0.5, -2.0)];
        p.part(e, stroke(&crack, |_| 0.5, 1.0, 1.0, |_| 1.0), M::Soot, 0.022);
    }
}

/// Conduit pipes run along the wall and torn off at its end, with a junction box.
fn conduit(p: &mut Props, e: Entity) {
    let y = p.rng.range(-6.0, 6.0);
    for k in 0..2 {
        let yy = y + k as f32 * 3.0;
        let end = Vec2::new(p.rng.range(4.0, 10.0), yy - p.rng.range(4.0, 14.0));
        let pts = [Vec2::new(-14.0, yy), Vec2::new(-1.0, yy), Vec2::new(end.x * 0.6, yy - 1.0), end];
        p.bar(e, &pts, 1.6, M::Steel, 0.0 + k as f32 * 0.005);
        p.cables(e, end, 1, 0.0);
    }
    p.shape(e, &rect(Vec2::new(-4.0, y + 1.5), Vec2::new(3.0, 3.5)), 0.5, M::Iron, 0.02);
}

// --- Ceilings ---

/// A dead pendant lamp on a rod: enamel shade, broken bulb; some swing askew.
fn pendant(p: &mut Props, e: Entity) {
    let len = p.rng.range(12.0, 30.0);
    let swing = p.rng.range(-0.25, 0.25);
    let d = Vec2::new(swing.sin(), -swing.cos());
    let x = p.rng.range(-6.0, 6.0);
    let top = Vec2::new(x, 2.0);
    let b = top + d * len;
    chain(p, e, top, b, 0.0);
    let n = d.perp();
    // Shade: a wide cone.
    let shade = [b - n * 1.5, b + n * 1.5, b + d * 5.0 + n * 7.0, b + d * 6.0, b + d * 5.0 - n * 7.0];
    p.shape(e, &shade, 0.4, M::Enamel, 0.01);
    let rim = [b + d * 5.0 + n * 7.0, b + d * 5.0 - n * 7.0];
    p.part(e, stroke(&rim, |_| 0.9, 1.0, 1.0, |_| 1.0), M::Iron, 0.015);
    let bulb: Vec<Vec2> = ellipse_pts(1.8, 1.8, 10).into_iter().map(|q| q + b + d * 6.5).collect();
    p.part(e, fill(&bulb, |_| 0.7), M::Steel, 0.012);
}

/// A chandelier: a chain, a brass ring and its arms, a few globes left.
fn chandelier(p: &mut Props, e: Entity) {
    let len = p.rng.range(10.0, 20.0);
    let c = Vec2::new(0.0, -len);
    chain(p, e, Vec2::new(0.0, 2.0), c, 0.0);
    let tilt = p.rng.range(-0.15, 0.15);
    let rot = |q: Vec2| c + Vec2::from_angle(tilt).rotate(q);
    let hoop: Vec<Vec2> = ellipse_pts(14.0, 3.0, 24).into_iter().map(rot).collect();
    let mut hoop_closed = hoop.clone();
    hoop_closed.push(hoop[0]);
    p.part(e, stroke(&hoop_closed, |_| 1.2, 1.0, 0.6, |_| 1.0), M::Bronze, 0.01);
    let body = [rot(Vec2::new(-2.5, 1.0)), rot(Vec2::new(0.0, 4.0)), rot(Vec2::new(2.5, 1.0)), rot(Vec2::new(1.5, -6.0)), rot(Vec2::new(0.0, -8.0)), rot(Vec2::new(-1.5, -6.0))];
    p.shape(e, &body, 0.5, M::Bronze, 0.02);
    for i in 0..6 {
        let a = i as f32 / 6.0 * TAU;
        let q = Vec2::new(a.cos() * 14.0, a.sin() * 3.0);
        line(p, e, rot(Vec2::new(0.0, -2.0)), rot(q), 0.8, M::Bronze, 0.015);
        if p.rng.chance(0.55) {
            let g: Vec<Vec2> = ellipse_pts(2.2, 2.6, 10).into_iter().map(|v| rot(q + Vec2::Y * 2.8 + v)).collect();
            p.shape(e, &g, 0.5, M::Enamel, if a.sin() < 0.0 { 0.025 } else { 0.005 });
        }
    }
    // A strand of crystals still hanging.
    let s = rot(Vec2::new(p.rng.range(-10.0, 10.0), -1.0));
    let mut tris = Vec::new();
    for k in 0..4 {
        let q = s - Vec2::Y * (2.0 + k as f32 * 2.4);
        tris.push([q + Vec2::new(-0.7, 0.0), q + Vec2::new(0.7, 0.0), q - Vec2::Y * 1.6]);
    }
    p.part(e, triangles(&tris), M::Rim, 0.03);
}

/// A directional sign on two rods, one snapped so it hangs at an angle:
/// an arrow and a line of letters.
fn hanging_sign(p: &mut Props, e: Entity) {
    let len = p.rng.range(8.0, 16.0);
    let (w, h) = (20.0, 5.5);
    let snapped = p.rng.chance(0.6);
    let a = Vec2::new(-w + 3.0, -len);
    let ang = if snapped { -p.rng.range(0.4, 0.9) } else { p.rng.range(-0.05, 0.05) };
    line(p, e, Vec2::new(a.x, 2.0), a, 0.9, M::Iron, 0.0);
    let rot = |q: Vec2| a + Vec2::from_angle(ang).rotate(q - Vec2::new(-w + 3.0, 0.0));
    let c = Vec2::new(0.0, -h);
    if snapped {
        // The other rod, bent and dangling.
        let b = Vec2::new(w - 3.0, -len * 0.4);
        p.bar(e, &[Vec2::new(w - 3.0, 2.0), b, b + Vec2::new(2.0, -3.0)], 0.9, M::Iron, 0.0);
    } else {
        line(p, e, Vec2::new(w - 3.0, 2.0), Vec2::new(w - 3.0, -len), 0.9, M::Iron, 0.0);
    }
    let panel: Vec<Vec2> = rect(c, Vec2::new(w, h)).into_iter().map(rot).collect();
    p.shape(e, &panel, 0.7, M::Soot, 0.01);
    let mut frame = panel.clone();
    frame.push(panel[0]);
    p.part(e, stroke(&frame, |_| 0.9, 1.0, 1.0, |_| 1.0), M::Steel, 0.015);
    // An arrow and blocky letters (pale enamel).
    let dir = if p.rng.chance(0.5) { 1.0 } else { -1.0 };
    let ax = if dir > 0.0 { w - 6.0 } else { -w + 6.0 };
    let tip = Vec2::new(ax + dir * 3.0, c.y);
    let mut tris = vec![[rot(tip), rot(Vec2::new(ax - dir * 0.5, c.y + 3.0)), rot(Vec2::new(ax - dir * 0.5, c.y - 3.0))]];
    tris.extend(square(Vec2::ZERO, 1.0).map(|t| t.map(|q| rot(Vec2::new(ax - dir * 3.0, c.y) + q * Vec2::new(2.5, 0.8)))));
    let mut x = if dir > 0.0 { -w + 2.5 } else { -w + 12.0 };
    let end = if dir > 0.0 { w - 11.0 } else { w - 2.5 };
    while x < end {
        if p.rng.chance(0.82) {
            let gw = p.rng.range(1.6, 2.8);
            for t in square(Vec2::ZERO, 1.0) {
                tris.push(t.map(|q| rot(Vec2::new(x + gw / 2.0, c.y) + q * Vec2::new(gw / 2.0, 2.0))));
            }
            x += gw + 0.9;
        } else {
            x += 2.5;
        }
    }
    p.part(e, triangles(&tris), M::Enamel, 0.02);
}
