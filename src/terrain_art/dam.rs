//! Ground material of "region: broken dam" (`Style::Dam`).
//!
//! A massive gravity dam: huge poured blocks (monoliths) stacked in lifts,
//! each pour its own tone, formwork panel lines and tie holes on the faces,
//! calcite leaching white out of the lift joints and running down, dark wet
//! seeps and moss, and giant fissures with rusted rebar across them.

use super::*;

/// Pale poured concrete (lighter and smoother than the city's).
const DAM: Rgb = [0.50, 0.48, 0.44];
/// Calcite leached out of the joints.
const CALCITE: Rgb = [0.86, 0.85, 0.80];
/// Wet seeps and algae.
const MOSS: Rgb = [0.22, 0.27, 0.17];
const RUST: Rgb = [0.42, 0.25, 0.16];

/// Monolith width and lift height (in units of 5 world units).
const BLOCK_W: f32 = 64.0;
const LIFT_H: f32 = 22.0;

/// The overall color of the ground (what the chameleon blends into).
pub const GROUND: Rgb = DAM;

/// The bare material of solid ground, before lighting (`u`, `v` in units of 5 world units).
pub fn material(u: f32, v: f32) -> Rgb {
    // --- Giant fissures: long, mostly vertical cracks, rust bleeding around them.
    let f = vnoise(u * 0.022, v * 0.0065, 61) - 0.5;
    let wob = (vnoise(u * 0.3, v * 0.3, 62) - 0.5) * 0.006;
    let fd = (f + wob).abs();
    if fd < 0.016 {
        // Rebar bridging the gap every few units.
        let bar = (v / 4.5).fract() < 0.16 && fd < 0.012;
        return if bar { scale(RUST, 0.8 + 0.4 * hash(u as i32, (v / 4.5) as i32, 63)) } else { scale(SOOT, 0.7) };
    }

    // --- Monoliths stacked in lifts, every pour a slightly different tone.
    let lift = (v / LIFT_H).floor() as i32;
    let ju = u + (vnoise(0.0, v * 0.05, 64) - 0.5) * 1.5;
    let block = (ju / BLOCK_W).floor() as i32;
    let (bx, by) = ((ju / BLOCK_W).fract() * BLOCK_W, (v / LIFT_H).fract() * LIFT_H);
    let tone = 0.86 + 0.22 * hash(block, lift, 66);
    let blotch = 0.88 + 0.24 * vnoise(u * 0.12, v * 0.12, 67);
    let mut c = scale(DAM, tone * blotch);

    // Formwork: panel seams and tie holes.
    let (pw, ph) = (16.0, 5.5);
    let (px, py) = ((bx / pw).fract() * pw, (by / ph).fract() * ph);
    if py < 0.3 || px < 0.25 {
        c = scale(c, 0.88);
    }
    let (tx, ty) = (((bx + pw / 4.0) / (pw / 2.0)).fract() * (pw / 2.0) - pw / 4.0, py - ph / 2.0);
    if tx * tx + ty * ty < 0.22 {
        c = scale(c, 0.45);
    }

    // Calcite leaching out of the lift joint above, running down.
    let streak = vnoise(u * 0.55, lift as f32 * 7.3, 68);
    let reach = LIFT_H * (0.25 + 0.9 * vnoise(u * 0.9, lift as f32 * 3.1, 69));
    if streak > 0.6 && by < reach {
        let k = ((streak - 0.6) * 4.0).min(1.0) * (1.0 - by / reach).powf(0.7);
        c = mix(c, CALCITE, 0.75 * k);
    }

    // Wet seeps, darkened with algae, down from the joints and cracks.
    let seep = vnoise(u * 0.35, v * 0.012, 70);
    if seep > 0.64 {
        c = mix(c, MOSS, ((seep - 0.64) * 4.5).min(0.75));
    }
    // Rust bleeding out of the fissures.
    if fd < 0.045 {
        let k = 1.0 - (fd - 0.016) / 0.029;
        c = mix(c, scale(RUST, 0.8), 0.5 * k * vnoise(u * 0.5, v * 0.2, 71));
    }
    // Spalled patches: the skin gone, coarse aggregate showing.
    if vnoise(u * 0.07, v * 0.07, 72) > 0.74 {
        let stone = hash((u * 1.5) as i32, (v * 1.5) as i32, 73);
        c = scale(c, 0.62 + 0.35 * stone);
    }

    // Joints last: dark lift lines and monolith seams.
    if by < 0.55 || bx < 0.6 {
        c = scale(SOOT, 1.4);
    } else if by < 1.1 {
        // The lip under a lift joint catches calcite.
        c = mix(c, CALCITE, 0.35);
    }
    c
}

/// The lit top edge: a weathered, wet crest, tufts of moss on it.
pub fn rim(u: f32) -> Rgb {
    let k = 0.88 + 0.14 * hash(u as i32, 0, 74);
    if vnoise(u * 0.4, 0.0, 75) > 0.72 {
        scale(mix(RIM, MOSS, 0.55), k)
    } else {
        scale([0.74, 0.73, 0.69], k)
    }
}

/// Inspection galleries inside the dam: dark concrete tunnels with a painted
/// dado band, formwork seams and water weeping down the walls.
pub fn interior(u: f32, v: f32) -> Rgb {
    let grime = 0.8 + 0.3 * vnoise(u * 0.2, v * 0.04, 76);
    let (bx, by) = ((u / 16.0).fract() * 16.0, (v / 5.5).fract() * 5.5);
    let mut k = if by < 0.3 || bx < 0.25 { 0.82 } else { 1.0 };
    // Painted band along the gallery walls, flaking off.
    let band = (v / 26.0).fract();
    if band > 0.62 && band < 0.78 && vnoise(u * 0.3, v * 0.3, 77) > 0.35 {
        k *= 1.7;
    } else if band >= 0.78 && band < 0.8 {
        k *= 0.45;
    }
    let seep = vnoise(u * 0.5, v * 0.015, 78);
    let base = if seep > 0.66 { mix(DAM, MOSS, 0.6) } else { DAM };
    scale(base, 0.27 * k * grime)
}
