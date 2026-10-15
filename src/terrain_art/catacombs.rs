//! Ground material of "region: catacombs" (`Style::Catacombs`).
//! Ossuary walls: courses of rough-cut limestone, rows of long bones stacked
//! end-on (their knobbed heads facing out), a band of skulls, bones laid
//! lengthwise; elsewhere bare limestone blocks with the odd carved cross.
//! Interiors are rows of burial niches with bones piled in the dark.
//! Bones stay a step darker than the bone-white chameleon.

use super::*;

const BONE: Rgb = [0.60, 0.56, 0.47];
const LIME: Rgb = [0.45, 0.42, 0.36];
const GAP: Rgb = [0.07, 0.065, 0.06];

/// The overall color of the ground (what the chameleon blends into).
pub const GROUND: Rgb = [0.50, 0.47, 0.40];

/// The bare material of solid ground, before lighting (`u`, `v` in units of 5 world units).
pub fn material(u: f32, v: f32) -> Rgb {
    // Hand-stacked: courses sag and wander a little.
    let wv = v + (vnoise(u * 0.04, v * 0.01, 401) - 0.5) * 3.0;
    let wu = u + (vnoise(u * 0.03, v * 0.05, 402) - 0.5) * 2.0;
    // Damp: dark water streaks running down.
    let damp = 0.82 + 0.22 * vnoise(u * 0.5, v * 0.03, 409);
    let c = if vnoise(u * 0.012, v * 0.02, 403) < 0.33 {
        let row = (wv / 6.0).floor() as i32;
        block(wu, wv - row as f32 * 6.0, row, 6.0)
    } else {
        ossuary(wu, wv)
    };
    scale(c, damp)
}

/// One repeat of an ossuary wall (24 units tall).
fn ossuary(u: f32, v: f32) -> Rgb {
    const P: f32 = 24.0;
    let band = (v / P).floor() as i32;
    let y = v - band as f32 * P;
    let r = band * 8;
    match y {
        y if y < 6.0 => block(u, y, r, 6.0),
        y if y < 8.8 => knob(u, y - 6.0, r + 1),
        y if y < 11.6 => knob(u, y - 8.8, r + 2),
        y if y < 17.6 => skull(u, y - 11.6, r + 3),
        y if y < 20.4 => long_bone(u, y - 17.6, r + 4),
        y if y < 23.2 => knob(u, y - 20.4, r + 5),
        _ => GAP,
    }
}

/// A course of rough-cut limestone blocks (`y` within the course, `h` tall).
fn block(u: f32, y: f32, row: i32, h: f32) -> Rgb {
    let w = 9.0 + 5.0 * hash(row, 0, 411);
    let x = u + hash(row, 1, 412) * 13.0;
    let col = (x / w).floor() as i32;
    let fx = x / w - col as f32;
    let edge = (fx.min(1.0 - fx) * w).min(y.min(h - y));
    if edge < 0.45 {
        return scale(LIME, 0.28);
    }
    let id = hash(col, row, 413);
    // Pick marks: diagonal chisel strokes, pitted surface.
    let chisel = if ((u * 0.9 + y * 0.6 + id * 7.0).fract()) < 0.16 { 0.86 } else { 1.0 };
    let pit = if hash((u * 2.0) as i32, (y * 2.0) as i32 + row * 31, 414) > 0.94 { 0.6 } else { 1.0 };
    // Lit top edge of the block, shadowed bottom.
    let bevel = if y - 0.45 < 0.6 { 1.12 } else if h - y < 1.0 { 0.78 } else { 1.0 };
    // Now and then a carved cross or a line of inscription.
    let (cx, cy) = ((fx - 0.5) * w, y - h / 2.0);
    let carved = id > 0.93 && ((cx.abs() < 0.4 && cy.abs() < 2.1) || ((cy + 0.7).abs() < 0.4 && cx.abs() < 1.4));
    let letters = (0.86..0.93).contains(&id) && cy.abs() < 0.5 && cx.abs() < w * 0.32 && hash((u * 1.6) as i32, row, 415) > 0.45;
    if carved || letters {
        return scale(LIME, 0.3);
    }
    scale(LIME, (0.82 + 0.3 * id) * chisel * pit * bevel * (0.9 + 0.2 * vnoise(u * 0.4, y * 0.4 + row as f32, 416)))
}

/// A row of long bones stacked end-on: their round heads, packed tight.
fn knob(u: f32, y: f32, row: i32) -> Rgb {
    let w = 3.0;
    let x = u + if row % 2 == 0 { 0.0 } else { w / 2.0 } + hash(row, 2, 421) * 3.0;
    let col = (x / w).floor() as i32;
    let id = hash(col, row, 422);
    if id > 0.93 {
        // One pulled out: a hole into the dark.
        return GAP;
    }
    let off = Vec2::new(hash(col, row, 423) - 0.5, hash(col, row, 424) - 0.5) * 0.4;
    let d = Vec2::new((x / w - col as f32 - 0.5) * w, y - 1.4) - off;
    let r = 1.2 + 0.25 * id;
    let len = d.length();
    if len > r {
        return scale(BONE, 0.16);
    }
    // Rounded: lit up-left, dark rim below.
    let light = 1.0 - (d - Vec2::new(-0.4, -0.5)).length() / (r * 1.6);
    let k = 0.55 + 0.55 * light.max(0.0) - 0.25 * (len / r).powi(4);
    // Some heads broken open: marrow hole.
    if id > 0.82 && len < r * 0.35 {
        return scale(BONE, 0.25);
    }
    scale(BONE, k * (0.85 + 0.25 * hash(col, row, 425)))
}

/// The band of skulls (6 units tall): crania with dark sockets, nose and teeth.
fn skull(u: f32, y: f32, row: i32) -> Rgb {
    let w = 6.0;
    let x = u + hash(row, 3, 431) * 6.0;
    let col = (x / w).floor() as i32;
    let id = hash(col, row, 432);
    let lean = (hash(col, row, 433) - 0.5) * 0.6;
    let p = Vec2::new((x / w - col as f32 - 0.5) * w + lean, y);
    let cranium = (p.x / 2.6).powi(2) + ((p.y - 2.5) / 2.3).powi(2) < 1.0;
    let jaw = p.x.abs() < 1.7 - (p.y - 3.6).max(0.0) * 0.3 && (3.0..5.6).contains(&p.y);
    if !cranium && !jaw {
        // Bones crammed in behind.
        return scale(BONE, 0.18 + 0.1 * hash((u * 1.5) as i32, (y * 1.5) as i32 + row, 434));
    }
    let eye = ((p - Vec2::new(-1.05, 3.0)) / Vec2::new(0.8, 0.75)).length() < 1.0 || ((p - Vec2::new(1.05, 3.0)) / Vec2::new(0.8, 0.75)).length() < 1.0;
    let nose = (3.7..4.5).contains(&p.y) && p.x.abs() < 0.12 + (p.y - 3.7) * 0.4;
    let teeth = (4.85..5.15).contains(&p.y) || ((4.85..5.6).contains(&p.y) && ((p.x * 2.2).fract().abs() < 0.2));
    if eye || nose {
        return GAP;
    }
    if id > 0.85 && (p.x - 0.8).abs() < 0.2 && p.y < 2.2 {
        // A crack across the dome.
        return scale(BONE, 0.3);
    }
    let shade = 1.05 - 0.45 * (p.y / 5.6) - 0.15 * (p.x / 2.6).abs().powi(2);
    scale(BONE, shade * (0.85 + 0.2 * id) * if teeth { 0.55 } else { 1.0 })
}

/// A row of long bones laid lengthwise, knobbed ends touching.
fn long_bone(u: f32, y: f32, row: i32) -> Rgb {
    let w = 8.0 + 4.0 * hash(row, 4, 441);
    let x = u + hash(row, 5, 442) * 10.0;
    let col = (x / w).floor() as i32;
    let fx = (x / w - col as f32) * w;
    let end = fx.min(w - fx);
    let dy = y - 1.4 + (hash(col, row, 443) - 0.5) * 0.4;
    // Shaft, swelling into two lobes at each end.
    let half = if end < 1.4 { 0.55 + 0.75 * (1.0 - end / 1.4).powf(0.5) } else { 0.55 };
    let notch = end < 0.9 && dy.abs() < 0.18;
    if dy.abs() > half || notch || end < 0.15 {
        return scale(BONE, 0.14);
    }
    let k = 1.0 - 0.5 * ((dy + 0.2) / half).abs().powi(2);
    scale(BONE, k * (0.85 + 0.25 * hash(col, row, 444)))
}

/// The lit top edge: trodden limestone dust with bone chips.
pub fn rim(u: f32) -> Rgb {
    let chip = hash((u * 1.5) as i32, 0, 451) > 0.85;
    scale(if chip { BONE } else { [0.58, 0.55, 0.48] }, 0.85 + 0.15 * hash(u as i32, 0, 9))
}

/// Passable structures: dark walls cut with rows of burial niches, bones piled inside.
pub fn interior(u: f32, v: f32) -> Rgb {
    let grime = 0.8 + 0.3 * vnoise(u * 0.2, v * 0.04, 11);
    let (pw, ph) = (11.0, 12.0);
    let row = (v / ph).floor() as i32;
    let x = u + if row % 2 == 0 { 0.0 } else { pw / 2.0 };
    let col = (x / pw).floor() as i32;
    let p = Vec2::new((x / pw - col as f32 - 0.5) * pw, v - row as f32 * ph);
    // Arched opening: 7 wide, from y 2.5 down to 10.5.
    let (hw, top, bottom) = (3.5, 2.5, 10.5);
    let spring = top + hw;
    let inside = p.x.abs() < hw && p.y < bottom && (p.y > spring || (p - Vec2::new(0.0, spring)).length() < hw);
    if inside && hash(col, row, 462) > 0.8 {
        // Sealed long ago: a plain slab, a carved cross on it.
        let cross = (p.x.abs() < 0.35 && (4.5..9.0).contains(&p.y)) || ((p.y - 6.0).abs() < 0.35 && p.x.abs() < 1.5);
        return scale(LIME, grime * if cross { 0.12 } else { 0.3 + 0.04 * vnoise(u, v, 463) });
    }
    if inside {
        // Bones heaped in the back of the niche, darkness above them.
        let heap = bottom - 3.0 - 1.2 * (1.0 - (p.x / hw).powi(2)) + (hash(col, row, 461) - 0.5) * 1.5;
        if p.y > heap {
            let k = knob(u * 1.3, (p.y - heap) % 2.8, row * 13 + col);
            return scale(k, 0.3 * grime);
        }
        return scale(GAP, 0.7);
    }
    // Lit lip of the arch.
    let lip = p.x.abs() < hw + 0.7 && p.y < bottom + 0.6 && (p.y > spring || (p - Vec2::new(0.0, spring)).length() < hw + 0.7);
    let wall = block(u, v - (v / 6.0).floor() * 6.0, (v / 6.0).floor() as i32 + 1000, 6.0);
    scale(wall, grime * if lip { 0.42 } else { 0.28 })
}
