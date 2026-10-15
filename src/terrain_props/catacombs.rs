//! Props of "region: catacombs" (`Style::Catacombs`): heaps of skulls and
//! bones, candle stubs drowned in their own wax, rusted iron gates, stone
//! crosses; burial niches and inscribed plaques cut into the walls; bones
//! sticking out of broken ledges; roots and dripping water from the ceilings.

use super::*;

pub(super) const HOOKS: Hooks = Hooks { spacing, ground, edge, wall, ceiling };

/// (gap between props of the same kind, chance a free spot gets one), in prop units.
fn spacing(kind: Kind) -> (f32, f32) {
    match kind {
        Kind::Ground => (50.0, 0.85),
        Kind::Edge => (35.0, 0.85),
        Kind::Wall => (45.0, 0.8),
        Kind::Ceiling => (40.0, 0.7),
    }
}

#[derive(Clone, Copy)]
enum G {
    Skulls,
    Bones,
    Candles,
    Gate,
    Cross,
}

impl G {
    /// (half width, height) in prop units.
    fn size(self) -> (f32, f32) {
        match self {
            G::Skulls => (16.0, 18.0),
            G::Bones => (16.0, 8.0),
            G::Candles => (10.0, 20.0),
            G::Gate => (22.0, 52.0),
            G::Cross => (10.0, 36.0),
        }
    }
}

/// A top surface with headroom at `cell` (`x` = its world x). True if something was placed.
fn ground(p: &mut Props, cell: IVec2, x: f32) -> bool {
    let table = [(G::Skulls, 3.0), (G::Bones, 1.5), (G::Candles, 2.5), (G::Gate, 1.5), (G::Cross, 1.5)];
    let total: f32 = table.iter().map(|e| e.1).sum();
    let mut r = p.rng.range(0.0, total);
    let pick = table.iter().find(|e| {
        r -= e.1;
        r <= 0.0
    });
    for g in [pick.unwrap_or(&table[0]).0, G::Bones] {
        let (half, head) = g.size();
        let tilt = matches!(g, G::Skulls | G::Bones | G::Candles | G::Cross);
        let Some((y, angle, indoor)) = p.fit(cell, half, head, tilt) else { continue };
        let at = Vec2::new(x, y);
        p.tilt = angle;
        p.depth = if indoor { INDOOR } else { BACK };
        match g {
            G::Skulls => skull_heap(p, at),
            G::Bones => scattered_bones(p, at),
            G::Candles => candles(p, at),
            G::Gate => gate(p, at),
            G::Cross => cross(p, at),
        }
        // Webs draped over whatever stands there, thicker toward the heart of the region.
        let k = heart(p, at);
        if p.rng.chance(0.08 + 0.7 * k) {
            let top = (head * 0.8).min(30.0);
            let e = p.root(at - Vec2::Y * S, 1.0, 0.0);
            let r = p.rng.range(0.7, 1.0) * top * (1.0 + 0.4 * k);
            let (x, span) = (p.rng.range(-3.0, 3.0), p.rng.range(1.6, 2.4));
            web_fan(p, e, Vec2::new(x, top), -std::f32::consts::FRAC_PI_2, span, r, k);
            if k > 0.5 && p.rng.chance(0.5) {
                let x = p.rng.range(-4.0, 4.0);
                spider(p, e, Vec2::new(x, top + 1.0), 0.0);
            }
        }
        p.tilt = 0.0;
        p.depth = BACK;
        return true;
    }
    false
}

/// How deep into its region `at` is, 0 at the borders to 1 at the center
/// (the webs thicken toward it).
pub(super) fn heart(p: &Props, at: Vec2) -> f32 {
    let Some(a) = p.t.area_at(at) else { return 0.0 };
    let d = (at - a.rect.center()) / a.rect.half_size().max(Vec2::splat(1.0));
    let k = (1.0 - d.length() / 1.1).clamp(0.0, 1.0);
    k * k * (3.0 - 2.0 * k)
}

/// A thin line as two triangles.
pub(super) fn thread(tris: &mut Vec<[Vec2; 3]>, a: Vec2, b: Vec2, w: f32) {
    let n = (b - a).normalize_or(Vec2::X).perp() * w * 0.5;
    tris.push([a + n, a - n, b + n]);
    tris.push([a - n, b - n, b + n]);
}

/// A sagging thread from `a` to `b`.
pub(super) fn sagging(tris: &mut Vec<[Vec2; 3]>, a: Vec2, b: Vec2, sag: f32, w: f32) {
    let mid = (a + b) * 0.5 - Vec2::Y * sag;
    let pts = smooth_open(&[a, mid, b], 3);
    for s in pts.windows(2) {
        thread(tris, s[0], s[1], w);
    }
}

/// An orb web anchored at `apex`, its spokes spread `span` radians around the
/// direction `dir`, `r` long; torn here and there. `k` = how thick and
/// intricate it is (more spokes and rings).
fn web_fan(p: &mut Props, e: Entity, apex: Vec2, dir: f32, span: f32, r: f32, k: f32) {
    let mut tris = Vec::new();
    let n = 4 + p.rng.below(2) + (k * 3.0) as usize;
    let rings = 2 + p.rng.below(2) + (k * 3.0) as usize;
    let spokes: Vec<Vec2> = (0..n)
        .map(|i| {
            let a = dir - span * 0.5 + span * (i as f32 + p.rng.range(-0.15, 0.15)) / (n - 1) as f32;
            apex + Vec2::from_angle(a) * r * p.rng.range(0.8, 1.0)
        })
        .collect();
    for &s in &spokes {
        thread(&mut tris, apex, s, 0.55);
    }
    // Frame threads along the border, then the spiral rings between spokes.
    for w in spokes.windows(2) {
        if p.rng.chance(0.85) {
            sagging(&mut tris, w[0], w[1], r * 0.07, 0.45);
        }
        for j in 1..rings {
            if p.rng.chance(0.8) {
                let f = j as f32 / rings as f32;
                sagging(&mut tris, apex + (w[0] - apex) * f, apex + (w[1] - apex) * f, r * 0.05 * f, 0.4);
            }
        }
    }
    p.part(e, triangles(&tris), M::Web, 0.05);
    // Sticky: slows whoever walks into it (`webs.rs`).
    p.commands.entity(e).insert(crate::webs::Web { center: apex + Vec2::from_angle(dir) * r * 0.5, radius: r * 0.6 });
}

/// Threads hanging from a ceiling, some with bundles of silk or a dust-grey
/// cocoon wrapped on them.
fn drape(p: &mut Props, e: Entity, k: f32) {
    let mut tris = Vec::new();
    for _ in 0..2 + p.rng.below(2) + (k * 4.0) as usize {
        let a = Vec2::new(p.rng.range(-14.0, 14.0), 1.0);
        let len = p.rng.range(8.0, 22.0 + 20.0 * k);
        let b = a + Vec2::new(p.rng.range(-4.0, 4.0), -len);
        sagging(&mut tris, a, b, 0.0, 0.5);
        // Silk beads along the strand.
        for i in 1..=p.rng.below(3) {
            let c = a.lerp(b, i as f32 / 3.5);
            thread(&mut tris, c + Vec2::new(0.0, 0.8), c - Vec2::new(0.0, 0.8), 1.4);
        }
    }
    p.part(e, triangles(&tris), M::Web, 0.05);
    if p.rng.chance(0.15 + 0.6 * k) {
        let x = p.rng.range(-8.0, 8.0);
        let len = p.rng.range(10.0, 18.0);
        let (rx, ry) = (p.rng.range(2.4, 3.6), p.rng.range(5.0, 8.0));
        let mut line = Vec::new();
        thread(&mut line, Vec2::new(x, 1.0), Vec2::new(x, -len), 0.5);
        p.part(e, triangles(&line), M::Web, 0.05);
        let body = ellipse_at(Vec2::new(x, -len - ry), rx, ry, 12);
        p.part(e, fill(&body, |q| 0.7 + 0.3 * ((q.x - x) / rx * 0.5 + 0.5)), M::Enamel, 0.06);
        // Bindings crossing the cocoon.
        let mut wrap = Vec::new();
        for i in 1..4 {
            let y = -len - ry * (0.35 + 0.4 * i as f32);
            thread(&mut wrap, Vec2::new(x - rx, y), Vec2::new(x + rx, y + 1.0), 0.5);
        }
        p.part(e, triangles(&wrap), M::Soot, 0.07);
    }
}

/// A spider waiting on a web, `at` its center.
pub(super) fn spider(p: &mut Props, e: Entity, at: Vec2, z: f32) {
    let body = ellipse_at(at, 1.4, 1.2, 8);
    p.part(e, fill(&body, |_| 1.0), M::Soot, 0.08 + z);
    let mut legs = Vec::new();
    for s in [-1.0, 1.0] {
        for i in 0..4 {
            let a = 0.5 + 0.35 * i as f32;
            let knee = at + Vec2::new(s * a.cos() * 2.6, a.sin() * 2.2);
            thread(&mut legs, at, knee, 0.4);
            thread(&mut legs, knee, knee + Vec2::new(s * 1.8, -2.6), 0.4);
        }
    }
    p.part(e, triangles(&legs), M::Soot, 0.08 + z);
}

/// Where the ground ends at a drop (`flip` = the open side).
fn edge(p: &mut Props, at: Vec2, flip: f32) -> bool {
    let e = p.root(at, flip, 0.0);
    // Long bones jutting out of the broken stack.
    for _ in 0..2 + p.rng.below(3) {
        let y = -p.rng.range(2.0, 16.0);
        let len = p.rng.range(10.0, 18.0);
        let droop = p.rng.range(-5.0, 2.0);
        let a = Vec2::new(-8.0, y);
        let r0 = p.rng.range(2.2, 3.0);
        bone(p, e, a, a + Vec2::new(len, droop), r0, 0.0);
    }
    if p.rng.chance(0.4) {
        // A skull tipping over the lip.
        let c = Vec2::new(p.rng.range(-2.0, 3.0), 4.5);
        let r0 = p.rng.range(5.0, 6.0);
        let r1 = p.rng.range(-0.5, 0.2);
        skull(p, e, c, r0, r1, 0.02);
    } else if p.rng.chance(0.5) {
        // A candle someone left at the edge, its wax run down the face.
        let r0 = p.rng.range(6.0, 11.0);
        candle(p, e, Vec2::new(-4.0, -0.5), r0, 3.2, 0.03);
        let len = p.rng.range(10.0, 22.0);
        wax_run(p, e, Vec2::new(-2.0, -1.0), len);
    }
    if p.rng.chance(0.35) {
        p.roots(e, Vec2::new(-2.0, -4.0), 0.0);
    }
    // Silk strung from the lip down into the drop.
    let k = heart(p, at);
    if p.rng.chance(0.1 + 0.6 * k) {
        let r = p.rng.range(12.0, 20.0) * (1.0 + 0.4 * k);
        let (tilt, span) = (p.rng.range(-0.3, 0.2), p.rng.range(1.2, 1.8));
        web_fan(p, e, Vec2::new(-2.0, 0.0), -1.0 + tilt, span, r, k);
    }
    true
}

/// A straight vertical face (`flip` = the open side).
fn wall(p: &mut Props, at: Vec2, flip: f32) -> bool {
    let placed = wall_face(p, at, flip);
    let k = heart(p, at);
    if p.rng.chance(0.12 + 0.75 * k) {
        // A web spun out from the wall, hanging toward the floor.
        let up = p.rng.range(2.0, 12.0) * S;
        let e = p.root(at + Vec2::new(0.0, up), flip, 0.0);
        let r = p.rng.range(14.0, 22.0) * (1.0 + 0.5 * k);
        let dir = p.rng.range(-0.7, 0.1);
        let span = p.rng.range(1.5, 2.1);
        web_fan(p, e, Vec2::new(-1.0, 0.0), dir, span, r, k);
        if k > 0.45 && p.rng.chance(0.4) {
            let d = Vec2::from_angle(dir) * r * 0.55;
            spider(p, e, d, 0.0);
        }
    }
    placed
}

fn wall_face(p: &mut Props, at: Vec2, flip: f32) -> bool {
    let inset = |w: f32| at + Vec2::new(-flip * (w * S + PX * 2.0), 0.0);
    let roll = p.rng.next();
    if roll < 0.55 {
        let half = Vec2::new(p.rng.range(9.0, 12.0), p.rng.range(9.0, 12.0));
        let c = inset(half.x);
        if p.inside(c, half * S) {
            let e = p.root(c, flip, 0.0);
            niche(p, e, half);
            return true;
        }
    } else if roll < 0.85 {
        let half = Vec2::new(p.rng.range(10.0, 14.0), p.rng.range(6.0, 8.0));
        let c = inset(half.x);
        if p.inside(c, half * S) {
            let e = p.root(c, flip, 0.0);
            plaque(p, e, half);
            return true;
        }
    }
    // An iron ring hammered into the wall, a length of chain hanging from it.
    let e = p.root(at, flip, 0.0);
    let y = p.rng.range(-4.0, 8.0);
    p.bar(e, &[Vec2::new(-6.0, y), Vec2::new(2.0, y)], 2.0, M::Iron, 0.0);
    let ring = Vec2::new(3.5, y - 2.5);
    p.ring(e, ring, 3.0, 1.4, M::Rust, 0.01, |_| 1.0);
    let len = p.rng.range(8.0, 22.0);
    chain(p, e, ring - Vec2::Y * 3.0, len, 0.02);
    true
}

/// The underside of a thick overhang.
fn ceiling(p: &mut Props, at: Vec2) -> bool {
    let e = p.root(at, 1.0, 0.0);
    if p.rng.chance(0.6) {
        let r0 = p.rng.range(-6.0, 6.0);
        p.roots(e, Vec2::new(r0, 2.0), -1.0);
    }
    if p.rng.chance(0.25) {
        // An iron lantern hung on a chain, long dead.
        let x = p.rng.range(-8.0, 8.0);
        let len = p.rng.range(10.0, 26.0);
        chain(p, e, Vec2::new(x, 1.0), len, 0.0);
        lantern(p, e, Vec2::new(x, -len - 1.0));
    }
    water(p, e);
    // Cobwebs in the vault, the densest toward the heart of the region.
    let k = heart(p, at);
    if p.rng.chance(0.15 + 0.75 * k) {
        let x = p.rng.range(-6.0, 6.0);
        let r = p.rng.range(14.0, 24.0) * (1.0 + 0.6 * k);
        let span = p.rng.range(1.8, 2.7);
        let tilt = p.rng.range(-0.3, 0.3);
        web_fan(p, e, Vec2::new(x, 1.0), -std::f32::consts::FRAC_PI_2 + tilt, span, r, k);
        if k > 0.4 && p.rng.chance(0.35) {
            spider(p, e, Vec2::new(x, 1.0 - r * 0.45), 0.0);
        }
    }
    if p.rng.chance(0.1 + 0.7 * k) {
        drape(p, e, k);
    }
    true
}

// --- Drawing ---

fn ellipse_at(c: Vec2, rx: f32, ry: f32, n: usize) -> Vec<Vec2> {
    ellipse_pts(rx, ry, n).into_iter().map(|q| q + c).collect()
}

/// A skull of radius `r` centered at `c`, turned by `angle` (local +Y up).
fn skull(p: &mut Props, e: Entity, c: Vec2, r: f32, angle: f32, z: f32) {
    let rot = Vec2::from_angle(angle);
    let at = |q: Vec2| c + rot.rotate(q * r);
    let dome: Vec<Vec2> = ellipse_pts(1.0, 0.88, 18).into_iter().map(|q| at(q + Vec2::Y * 0.15)).collect();
    p.shape(e, &dome, 0.55, M::Enamel, z);
    // Maxilla and teeth.
    let jaw: Vec<Vec2> = [Vec2::new(-0.55, -0.35), Vec2::new(-0.5, -0.95), Vec2::new(0.5, -0.95), Vec2::new(0.55, -0.35)].into_iter().map(at).collect();
    p.shape(e, &smooth_closed(&jaw, 3), 0.5, M::Enamel, z + 0.001);
    let teeth: Vec<Vec2> = [Vec2::new(-0.42, -0.72), Vec2::new(0.42, -0.72)].into_iter().map(at).collect();
    p.part(e, stroke(&teeth, |_| r * 0.16, 1.0, 1.0, |t| if (t * 7.0) as i32 % 2 == 0 { 1.0 } else { 0.3 }), M::Soot, z + 0.002);
    // Sockets and nose.
    for s in [-1.0, 1.0] {
        let eye: Vec<Vec2> = ellipse_pts(0.29, 0.27, 12).into_iter().map(|q| at(q + Vec2::new(s * 0.37, -0.12))).collect();
        p.part(e, fill(&eye, |_| 1.0), M::Soot, z + 0.003);
    }
    let nose = [at(Vec2::new(0.0, -0.3)), at(Vec2::new(-0.12, -0.58)), at(Vec2::new(0.12, -0.58))];
    p.part(e, triangles(&[nose]), M::Soot, z + 0.003);
    // Highlight on the brow.
    let hl = [at(Vec2::new(-0.6, 0.55)), at(Vec2::new(-0.1, 0.82))];
    p.part(e, stroke(&hl, |t| r * 0.12 * (1.0 - (t - 0.5).abs()), 1.0, 1.0, |_| 1.0), M::Rim, z + 0.004);
}

/// A long bone from `a` to `b`, knobbed ends `w` thick.
fn bone(p: &mut Props, e: Entity, a: Vec2, b: Vec2, w: f32, z: f32) {
    let d = (b - a).normalize_or(Vec2::X);
    let n = d.perp();
    p.part(e, stroke(&[a + d * w * 0.4, b - d * w * 0.4], |t| w * (0.75 - 0.15 * (1.0 - (2.0 * t - 1.0).powi(2))), 1.0, 0.5, |_| 1.0), M::Enamel, z);
    for (end, k) in [(a, 1.0), (b, -1.0)] {
        for s in [-1.0, 1.0] {
            let lobe = ellipse_at(end + d * k * w * 0.25 + n * s * w * 0.32, w * 0.42, w * 0.42, 10);
            p.part(e, fill(&lobe, |q| 0.6 + 0.4 * ((q - end).dot(n) * s / w + 0.5).clamp(0.0, 1.0)), M::Enamel, z + 0.001);
        }
    }
}

/// Skulls heaped on a bed of bones.
fn skull_heap(p: &mut Props, at: Vec2) {
    let flip = if p.rng.chance(0.5) { 1.0 } else { -1.0 };
    let e = p.root(at - Vec2::Y * S * 2.0, flip, 0.0);
    for i in 0..3 {
        let x = p.rng.range(-14.0, 6.0);
        let a = Vec2::new(x, p.rng.range(0.5, 3.0));
        let len = p.rng.range(14.0, 22.0);
        let r0 = p.rng.range(-3.0, 4.0);
        bone(p, e, a, a + Vec2::new(len, r0), 2.6, 0.001 * i as f32);
    }
    let n = 2 + p.rng.below(3);
    for i in 0..n {
        let r = p.rng.range(4.5, 6.0);
        let top = i == n - 1 && n > 2;
        let c = if top { Vec2::new(p.rng.range(-3.0, 3.0), r * 2.4) } else { Vec2::new(-12.0 + i as f32 * 24.0 / n as f32 + p.rng.range(-2.0, 2.0), r * 0.9) };
        let angle = p.rng.range(-0.5, 0.5);
        skull(p, e, c, r, angle, 0.01 + 0.01 * i as f32);
    }
    if p.rng.chance(0.4) {
        // A candle stuck on top of the heap.
        let r0 = p.rng.range(-8.0, 8.0);
        let r1 = p.rng.range(5.0, 9.0);
        candle(p, e, Vec2::new(r0, 10.0), r1, 2.8, 0.06);
    }
}

/// Bones strewn over the floor, maybe a lone skull or a jaw.
fn scattered_bones(p: &mut Props, at: Vec2) {
    let e = p.root(at - Vec2::Y * S * 1.0, 1.0, 0.0);
    for i in 0..2 + p.rng.below(3) {
        let a = Vec2::new(p.rng.range(-16.0, 6.0), p.rng.range(0.8, 2.0));
        let dir = Vec2::from_angle(p.rng.range(-0.25, 0.25));
        let len = p.rng.range(9.0, 18.0);
        let r0 = p.rng.range(2.0, 2.8);
        bone(p, e, a, a + dir * len, r0, 0.002 * i as f32);
    }
    if p.rng.chance(0.65) {
        let r = p.rng.range(4.0, 5.0);
        let r0 = p.rng.range(-8.0, 8.0);
        let r1 = p.rng.range(-1.6, 1.6);
        skull(p, e, Vec2::new(r0, r * 0.8), r, r1, 0.02);
    } else {
        // Ribs, curled like claws.
        for i in 0..2 + p.rng.below(2) {
            let x = p.rng.range(-10.0, 10.0);
            let pts = [Vec2::new(x, 0.5), Vec2::new(x + 3.0, 5.5), Vec2::new(x + 8.0, 6.5), Vec2::new(x + 11.0, 3.0)];
            p.part(e, stroke(&smooth_open(&pts, 5), |t| 1.8 - 0.8 * t, 1.0, 0.55, |_| 1.0), M::Enamel, 0.01 + 0.001 * i as f32);
        }
    }
}

/// A candle stub `h` tall standing at `base`, wax dribbled down it.
fn candle(p: &mut Props, e: Entity, base: Vec2, h: f32, w: f32, z: f32) {
    let top = base + Vec2::Y * h;
    // Puddle of wax at its foot.
    let pool = ellipse_at(base + Vec2::Y * 0.4, w * 1.4, 1.3, 14);
    p.part(e, fill(&pool, |q| 0.6 + 0.4 * (q.y - base.y).max(0.0)), M::Enamel, z);
    p.part(e, stroke(&[base, top], |_| w, 1.0, 0.55, |_| 1.0), M::Enamel, z + 0.001);
    // Melted crown, drips down the side.
    let crown = [top + Vec2::new(-w * 0.5, -0.5), top + Vec2::new(-w * 0.2, 0.6), top + Vec2::new(w * 0.3, 0.2), top + Vec2::new(w * 0.5, -0.8)];
    p.part(e, stroke(&smooth_open(&crown, 3), |_| 1.0, 1.0, 0.7, |_| 1.0), M::Rim, z + 0.002);
    for _ in 0..1 + p.rng.below(2) {
        let x = p.rng.range(-w * 0.45, w * 0.45);
        let len = p.rng.range(h * 0.25, h * 0.7);
        p.part(e, stroke(&[top + Vec2::new(x, -0.3), top + Vec2::new(x + 0.2, -len)], |t| 1.1 - 0.4 * t, 1.0, 1.0, |_| 1.0), M::Rim, z + 0.003);
    }
    p.part(e, stroke(&[top, top + Vec2::new(0.3, 1.8)], |_| 0.6, 1.0, 1.0, |_| 1.0), M::Soot, z + 0.003);
    if p.rng.chance(0.5) {
        // Still burning: a small pale flame.
        let f = top + Vec2::Y * 2.0;
        let flame = [f + Vec2::new(-0.9, 0.0), f + Vec2::new(0.0, 3.6), f + Vec2::new(0.9, 0.0), f + Vec2::new(0.0, -0.8)];
        p.part(e, fill(&smooth_closed(&flame, 3), |_| 1.0), M::Rim, z + 0.004);
    }
}

/// A trail of wax run down a face from `from`.
fn wax_run(p: &mut Props, e: Entity, from: Vec2, len: f32) {
    let wob = p.rng.range(-1.0, 1.0);
    let pts = [from, from + Vec2::new(0.8, -len * 0.4), from + Vec2::new(wob, -len * 0.8), from + Vec2::new(wob + 0.4, -len)];
    p.part(e, stroke(&smooth_open(&pts, 5), |t| 2.2 - 1.2 * t, 1.0, 0.7, |_| 1.0), M::Enamel, OVER);
    let blob = ellipse_at(from + Vec2::new(wob + 0.4, -len - 0.6), 0.9, 1.2, 8);
    p.part(e, fill(&blob, |_| 0.9), M::Enamel, OVER);
}

/// A cluster of candle stubs, often on a skull.
fn candles(p: &mut Props, at: Vec2) {
    let e = p.root(at - Vec2::Y * S * 0.5, 1.0, 0.0);
    if p.rng.chance(0.5) {
        let r = 5.5;
        let r0 = p.rng.range(-0.2, 0.2);
        skull(p, e, Vec2::new(0.0, r * 0.8), r, r0, 0.0);
        let r0 = p.rng.range(5.0, 8.0);
        candle(p, e, Vec2::new(0.0, r * 1.6), r0, 3.0, 0.02);
        let x = p.rng.range(6.0, 9.0);
        let r0 = p.rng.range(4.0, 9.0);
        candle(p, e, Vec2::new(x, 0.0), r0, 3.0, 0.03);
    } else {
        for i in 0..2 + p.rng.below(3) {
            let x = -8.0 + i as f32 * 5.0 + p.rng.range(-1.0, 1.0);
            let r0 = p.rng.range(4.0, 14.0);
            let r1 = p.rng.range(2.6, 3.6);
            candle(p, e, Vec2::new(x, 0.0), r0, r1, 0.01 * i as f32);
        }
    }
}

/// An iron gate: spiked bars, two rails, one bar bent aside, hanging off its hinge.
fn gate(p: &mut Props, at: Vec2) {
    let flip = if p.rng.chance(0.5) { 1.0 } else { -1.0 };
    let lean = p.rng.range(-0.08, 0.04);
    let e = p.root(at - Vec2::Y * S * 2.0, flip, lean);
    let (hw, h) = (p.rng.range(16.0, 21.0), p.rng.range(40.0, 50.0));
    let n = 5 + p.rng.below(2);
    let bent = p.rng.below(n);
    let sag = p.rng.range(0.0, 6.0);
    for i in 0..n {
        let x = -hw + 2.0 * hw * i as f32 / (n - 1) as f32;
        let y_top = h - sag * (x + hw) / (2.0 * hw);
        let pts: Vec<Vec2> = if i == bent && i != 0 && i != n - 1 {
            let k = p.rng.range(5.0, 8.0) * if p.rng.chance(0.5) { 1.0 } else { -1.0 };
            vec![Vec2::new(x, -3.0), Vec2::new(x + k * 0.3, y_top * 0.35), Vec2::new(x + k, y_top * 0.6), Vec2::new(x + k * 0.6, y_top)]
        } else {
            vec![Vec2::new(x, -3.0), Vec2::new(x, y_top)]
        };
        let tip = pts[pts.len() - 1];
        p.bar(e, &pts, 1.9, M::Iron, 0.0);
        let spike = [tip + Vec2::new(-1.8, 0.0), tip + Vec2::new(0.0, 4.5), tip + Vec2::new(1.8, 0.0)];
        p.part(e, triangles(&[spike]), M::Iron, 0.005);
    }
    for (k, y) in [h * 0.18, h * 0.82].into_iter().enumerate() {
        let a = Vec2::new(-hw - 1.5, y);
        let b = Vec2::new(hw + 1.5, y - sag * if k == 1 { 1.0 } else { 0.3 });
        p.part(e, stroke(&[a, b], |_| 2.4, 1.0, 0.5, |_| 1.0), M::Iron, 0.01);
    }
    // A cross welded on the middle, the lock hanging open, rust bleeding down.
    let c = Vec2::new(0.0, h * 0.55);
    p.part(e, stroke(&[c - Vec2::Y * 7.0, c + Vec2::Y * 7.0], |_| 1.6, 1.0, 0.6, |_| 1.0), M::Rust, 0.02);
    p.part(e, stroke(&[c + Vec2::new(-4.0, 3.0), c + Vec2::new(4.0, 3.0)], |_| 1.6, 1.0, 0.6, |_| 1.0), M::Rust, 0.02);
    let lock = Vec2::new(hw - 3.0, h * 0.42);
    p.ring(e, lock + Vec2::Y * 2.5, 1.8, 0.9, M::Iron, 0.02, |_| 1.0);
    let body = [lock + Vec2::new(-2.0, 1.0), lock + Vec2::new(-2.0, -2.5), lock + Vec2::new(2.0, -2.5), lock + Vec2::new(2.0, 1.0)];
    p.shape(e, &body, 0.5, M::Rust, 0.025);
}

/// A stone cross, sunk and leaning, chipped.
fn cross(p: &mut Props, at: Vec2) {
    let lean = p.rng.range(-0.25, 0.25);
    let e = p.root(at - Vec2::Y * S * 4.0, 1.0, lean);
    let (h, arm, w) = (p.rng.range(26.0, 34.0), p.rng.range(8.0, 10.0), 4.2);
    let ay = h * 0.7;
    let outline = [
        Vec2::new(-w / 2.0, -2.0),
        Vec2::new(-w / 2.0, ay - w / 2.0),
        Vec2::new(-arm, ay - w / 2.0),
        Vec2::new(-arm, ay + w / 2.0),
        Vec2::new(-w / 2.0, ay + w / 2.0),
        Vec2::new(-w / 2.0, h),
        Vec2::new(w / 2.0, h),
        Vec2::new(w / 2.0, ay + w / 2.0),
        Vec2::new(arm - p.rng.range(0.0, 3.0), ay + w / 2.0),
        Vec2::new(arm, ay - w / 2.0 + 0.5),
        Vec2::new(w / 2.0, ay - w / 2.0),
        Vec2::new(w / 2.0, -2.0),
    ];
    // Filled as two bars (the fan fill wants star-shaped outlines).
    let post = [outline[0], outline[5], outline[6], outline[11]];
    let beam = [outline[2], outline[3], outline[8], outline[9]];
    p.shape(e, &post, 0.5, M::Concrete, 0.0);
    p.shape(e, &beam, 0.6, M::Concrete, 0.001);
    p.part(e, stroke(&[Vec2::new(-w / 2.0, h - 0.6), Vec2::new(w / 2.0, h - 0.6)], |_| 1.4, 1.0, 1.0, |_| 1.0), M::Rim, 0.002);
    p.part(e, stroke(&[Vec2::new(-arm, ay + w / 2.0 - 0.6), Vec2::new(arm, ay + w / 2.0 - 0.6)], |_| 1.2, 1.0, 1.0, |_| 1.0), M::Rim, 0.002);
    let x0 = p.rng.range(-1.0, 1.0);
    let crack: Vec<Vec2> = (0..4).map(|i| Vec2::new(x0 + p.rng.range(-1.0, 1.0), ay - 2.0 - i as f32 * 5.0)).collect();
    p.part(e, stroke(&crack, |t| 1.0 * (1.0 - t) + 0.3, 1.0, 1.0, |_| 1.0), M::Soot, 0.003);
    if p.rng.chance(0.5) {
        // A rosary of iron beads hung on an arm.
        let a = Vec2::new(arm - 2.0, ay + w / 2.0);
        let pts = smooth_open(&[a, a + Vec2::new(1.0, -6.0), a + Vec2::new(-2.0, -9.0), a + Vec2::new(-4.0, -1.0)], 5);
        p.part(e, stroke(&pts, |_| 0.5, 1.0, 1.0, |_| 1.0), M::Iron, 0.004);
    }
}

/// An arched burial niche cut into the wall, a skull and bones inside (over the face).
fn niche(p: &mut Props, e: Entity, half: Vec2) {
    let arch: Vec<Vec2> = {
        let mut v = vec![Vec2::new(half.x, -half.y), Vec2::new(half.x, half.y - half.x)];
        for i in 1..10 {
            let a = i as f32 / 10.0 * std::f32::consts::PI;
            v.push(Vec2::new(a.cos() * half.x, half.y - half.x + a.sin() * half.x));
        }
        v.push(Vec2::new(-half.x, half.y - half.x));
        v.push(Vec2::new(-half.x, -half.y));
        v
    };
    p.part(e, fill(&arch, |q| 0.4 + 0.6 * (1.0 - (q.y + half.y) / (2.0 * half.y))), M::Soot, OVER);
    let mut rim = arch.clone();
    rim.push(arch[0]);
    p.part(e, stroke(&rim, |_| 2.6, 1.0, 0.5, |_| 1.0), M::Concrete, OVER + 0.02);
    // Sill.
    p.part(e, stroke(&[Vec2::new(-half.x - 2.5, -half.y - 1.0), Vec2::new(half.x + 2.5, -half.y - 1.0)], |_| 2.6, 1.0, 0.5, |_| 1.0), M::Rim, OVER + 0.03);
    // Bones piled inside, a skull on top looking out.
    for i in 0..2 {
        let y = -half.y + 1.5 + i as f32 * 2.5;
        let a = Vec2::new(-half.x + 1.5, y);
        let r0 = p.rng.range(-1.0, 1.0);
        bone(p, e, a, Vec2::new(half.x - 1.5, y + r0), 2.2, OVER + 0.005 + 0.001 * i as f32);
    }
    let r = (half.x * 0.42).min(5.0);
    if p.rng.chance(0.8) {
        let r0 = p.rng.range(-2.0, 2.0);
        let r1 = p.rng.range(-0.25, 0.25);
        skull(p, e, Vec2::new(r0, -half.y + 5.5 + r * 0.8), r, r1, OVER + 0.01);
    }
    if p.rng.chance(0.4) {
        let x = if p.rng.chance(0.5) { half.x - 2.5 } else { -half.x + 2.5 };
        let r0 = p.rng.range(4.0, 7.0);
        candle(p, e, Vec2::new(x, -half.y + 0.5), r0, 2.4, OVER + 0.015);
        let len = p.rng.range(8.0, 18.0);
        wax_run(p, e, Vec2::new(x, -half.y - 2.0), len);
    }
}

/// A stone plaque with a carved cross and lines of an inscription nobody can read (over the face).
fn plaque(p: &mut Props, e: Entity, half: Vec2) {
    let slab = [Vec2::new(-half.x, -half.y), Vec2::new(-half.x, half.y), Vec2::new(half.x, half.y), Vec2::new(half.x, -half.y)];
    p.shape(e, &slab, 0.75, M::Concrete, OVER);
    p.part(e, stroke(&[Vec2::new(-half.x, half.y - 0.6), Vec2::new(half.x, half.y - 0.6)], |_| 1.4, 1.0, 1.0, |_| 1.0), M::Rim, OVER + 0.01);
    p.part(e, stroke(&[Vec2::new(-half.x, -half.y + 0.6), Vec2::new(half.x, -half.y + 0.6)], |_| 1.4, 0.4, 0.4, |_| 1.0), M::Soot, OVER + 0.01);
    // Cross on the left.
    let c = Vec2::new(-half.x + 4.5, 0.0);
    p.part(e, stroke(&[c - Vec2::Y * (half.y - 1.8), c + Vec2::Y * (half.y - 1.8)], |_| 1.2, 1.0, 1.0, |_| 1.0), M::Soot, OVER + 0.02);
    p.part(e, stroke(&[c + Vec2::new(-2.3, half.y * 0.3), c + Vec2::new(2.3, half.y * 0.3)], |_| 1.2, 1.0, 1.0, |_| 1.0), M::Soot, OVER + 0.02);
    // Rows of letters.
    let rows = 2 + (half.y > 7.0) as usize;
    let mut tris = Vec::new();
    for k in 0..rows {
        let y = half.y - 2.6 - k as f32 * (2.0 * half.y - 4.0) / rows as f32;
        let mut x = c.x + 4.0;
        while x < half.x - 1.5 {
            let gw = p.rng.range(0.9, 1.6);
            if p.rng.chance(0.85) {
                let (a, b) = (Vec2::new(x, y), Vec2::new(x + gw * 0.6, y - 1.6));
                tris.push([a, Vec2::new(x + 0.5, y), b]);
                tris.push([a, b, Vec2::new(x + gw * 0.6 - 0.5, y - 1.6)]);
            }
            x += gw + if p.rng.chance(0.2) { 1.2 } else { 0.4 };
        }
    }
    p.part(e, triangles(&tris), M::Soot, OVER + 0.02);
    // Broken corner.
    if p.rng.chance(0.5) {
        let k = Vec2::new(half.x, -half.y);
        let (a, b) = (p.rng.range(3.0, 6.0), p.rng.range(3.0, 6.0));
        p.part(e, triangles(&[[k, k + Vec2::new(-a, 0.0), k + Vec2::new(0.0, b)]]), M::Soot, OVER + 0.03);
    }
}

/// A chain hanging from `from`, `len` long.
fn chain(p: &mut Props, e: Entity, from: Vec2, len: f32, z: f32) {
    let sway = p.rng.range(-2.0, 2.0);
    let pts = smooth_open(&[from, from + Vec2::new(sway * 0.4, -len * 0.5), from + Vec2::new(sway, -len)], 6);
    p.part(e, stroke(&pts, |_| 1.6, 1.0, 0.5, |t| if (t * len / 1.6) as i32 % 2 == 0 { 1.0 } else { 0.45 }), M::Iron, z);
}

/// An iron cage lantern, its glass gone.
fn lantern(p: &mut Props, e: Entity, top: Vec2) {
    let (w, h) = (4.5, 9.0);
    let hood = [top + Vec2::new(-w - 1.0, -2.0), top + Vec2::new(0.0, 1.5), top + Vec2::new(w + 1.0, -2.0)];
    p.shape(e, &hood, 0.6, M::Iron, 0.01);
    let body = [top + Vec2::new(-w, -2.0), top + Vec2::new(-w * 0.8, -2.0 - h), top + Vec2::new(w * 0.8, -2.0 - h), top + Vec2::new(w, -2.0)];
    p.shape(e, &body, 0.5, M::Soot, 0.005);
    for x in [-w * 0.9, 0.0, w * 0.9] {
        p.part(e, stroke(&[top + Vec2::new(x, -2.0), top + Vec2::new(x * 0.85, -2.0 - h)], |_| 0.9, 1.0, 1.0, |_| 1.0), M::Iron, 0.01);
    }
    p.part(e, stroke(&[top + Vec2::new(-w, -2.5 - h), top + Vec2::new(w, -2.5 - h)], |_| 1.6, 1.0, 0.5, |_| 1.0), M::Iron, 0.01);
    // A candle stub left inside.
    candle(p, e, top + Vec2::new(0.0, -1.5 - h), 3.0, 2.0, 0.008);
}

/// Water seeping through the vault: wet streaks, hanging drops, drops falling.
fn water(p: &mut Props, e: Entity) {
    for _ in 0..1 + p.rng.below(3) {
        let at = Vec2::new(p.rng.range(-12.0, 12.0), 1.0);
        let (w, len) = (p.rng.range(1.0, 1.8), p.rng.range(3.0, 9.0));
        // A little calcite nub with the drop on its tip.
        let nub = [at + Vec2::new(-w * 1.3, 1.0), at + Vec2::new(0.0, -len * 0.6), at + Vec2::new(w * 1.3, 1.0)];
        p.part(e, fill(&nub, |q| 0.6 + 0.4 * ((q.y - at.y) / len + 1.0).clamp(0.0, 1.0)), M::Concrete, OVER);
        let tip = at - Vec2::Y * len * 0.6;
        let drop = [tip + Vec2::new(-w * 0.5, 0.0), tip + Vec2::new(0.0, -w * 1.6), tip + Vec2::new(w * 0.5, 0.0), tip + Vec2::new(0.0, w * 0.5)];
        p.part(e, fill(&smooth_closed(&drop, 3), |_| 1.0), M::Steel, OVER + 0.01);
        if p.rng.chance(0.5) {
            let f = tip - Vec2::Y * p.rng.range(8.0, 22.0);
            p.part(e, stroke(&[f + Vec2::Y * 2.5, f], |t| 0.4 + 0.6 * t, 1.0, 1.0, |_| 1.0), M::Steel, OVER + 0.01);
        }
    }
}
