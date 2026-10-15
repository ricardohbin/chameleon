//! Ground material of "region: water treatment facility" (`Style::Treatment`).
//!
//! Solid ground is cast concrete poured in formwork lifts: panel seams, rows of
//! tie holes bleeding rust, pale limescale streaks crusting down the faces,
//! dark tide lines left by the flood and patches of sludge. The rims are the
//! rusted steel copings of the settling tanks. Interiors are the plant's halls:
//! glazed tiles (many fallen off), a dark painted dado under a stripe, and
//! pipe runs with flanges along the walls.

use super::*;

/// Cast concrete, a little green from the damp.
const CAST: Rgb = [0.43, 0.44, 0.40];
/// Limescale / efflorescence crust.
const LIME: Rgb = [0.70, 0.71, 0.66];
const RUST: Rgb = [0.42, 0.25, 0.15];
const SLUDGE: Rgb = [0.15, 0.16, 0.11];
/// Glazed wall tiles.
const TILE: Rgb = [0.60, 0.63, 0.60];
/// Industrial paint on the lower walls.
const PAINT: Rgb = [0.26, 0.33, 0.31];
/// Steel of the tank copings and pipes.
const STEEL: Rgb = [0.52, 0.54, 0.55];

/// The overall color of the ground (what the chameleon blends into).
pub const GROUND: Rgb = [0.44, 0.45, 0.41];

/// Formwork panel size (units of 5 world units).
const PANEL: (f32, f32) = (24.0, 12.0);

/// The bare material of solid ground, before lighting (`u`, `v` in units of 5 world units, `v` down).
pub fn material(u: f32, v: f32) -> Rgb {
    let (pw, ph) = PANEL;
    let (col, row) = ((u / pw).floor() as i32, (v / ph).floor() as i32);
    let (fx, fy) = ((u / pw).fract(), (v / ph).fract());
    // Each pour came out a little different.
    let pour = 0.88 + 0.18 * hash(col, row, 201);
    let mut c = scale(CAST, pour * (0.86 + 0.26 * vnoise(u * 0.12, v * 0.12, 202)));

    // Sludge caked in hollows.
    let sludge = vnoise(u * 0.06, v * 0.06, 205);
    if sludge > 0.72 {
        c = mix(c, SLUDGE, ((sludge - 0.72) * 6.0).min(0.85));
    }

    // Tide lines: a dark wet band under a crisp crust of scale.
    let tide = ((v + vnoise(u * 0.03, 0.5, 204) * 4.0) / 37.0).fract();
    if tide < 0.10 {
        c = scale(c, 0.62 + 0.3 * tide / 0.10);
    } else if tide > 0.975 {
        c = mix(c, LIME, 0.55);
    }

    // Limescale weeping down the face in long streaks.
    let weep = vnoise(u * 0.7, v * 0.025, 203) * (0.6 + 0.4 * vnoise(u * 0.05, v * 0.05, 206));
    if weep > 0.5 {
        c = mix(c, LIME, ((weep - 0.5) * 2.2).min(0.6));
    }

    // Panel seams: recessed lines.
    if fy * ph < 0.4 || fx * pw < 0.35 {
        return scale(c, 0.55);
    }

    // Tie holes, two per panel, each bleeding rust (or scale) down the panel.
    for (k, hx) in [0.25_f32, 0.75].into_iter().enumerate() {
        let d = Vec2::new((fx - hx) * pw, (fy - 0.42) * ph);
        if d.length() < 0.75 {
            return scale(SOOT, 0.8);
        }
        let id = hash(col * 2 + k as i32, row, 207);
        let len = 2.0 + id * 6.0;
        if d.y > 0.0 && d.y < len {
            let w = 0.7 * (1.0 - d.y / len) + 0.15 + 0.2 * vnoise(u * 2.0, v * 0.5, 208);
            if d.x.abs() < w {
                let stain = if id > 0.35 { RUST } else { LIME };
                c = mix(c, stain, 0.7 * (1.0 - d.y / len));
            }
        }
    }
    c
}

/// The lit top edge: a rusted steel coping, riveted.
pub fn rim(u: f32) -> Rgb {
    let rust = 0.15 + 0.45 * vnoise(u * 0.25, 0.5, 209);
    let c = mix(scale(STEEL, 1.25), RUST, rust);
    let rivet = (u / 4.0).fract() < 0.14;
    scale(c, if rivet { 0.6 } else { 0.9 + 0.12 * hash(u as i32, 0, 210) })
}

/// Passable structures (dark interiors behind the action): tiled hall walls.
pub fn interior(u: f32, v: f32) -> Rgb {
    // Wall bands (top to bottom): pipe run, tiles, a stripe, painted dado.
    const BAND: f32 = 34.0;
    let b = (v / BAND).fract() * BAND;
    let bi = (v / BAND).floor() as i32;
    let grime = 0.75 + 0.3 * vnoise(u * 0.4, v * 0.03, 211);
    let c = if (4.0..7.5).contains(&b) {
        // A pipe along the wall, lit from above, flanged.
        let t = (b - 4.0) / 3.5;
        let round = 1.0 - (t - 0.3).abs() * 1.1;
        let flange = ((u + bi as f32 * 13.0) / 30.0).fract() < 0.05;
        let base = mix(STEEL, RUST, 0.3 + 0.5 * vnoise(u * 0.15, bi as f32, 212));
        scale(base, 0.5 * round.max(0.25) * if flange { 1.25 } else { 1.0 })
    } else if b < 21.0 {
        tiles(u, v)
    } else if b < 22.2 {
        // The stripe.
        scale(TILE, 0.42)
    } else {
        // Painted dado, peeling to concrete, with water marks.
        let peel = vnoise(u * 0.15, v * 0.15, 213) > 0.68;
        let base = if peel { scale(CAST, 0.4) } else { scale(PAINT, 0.8 + 0.2 * vnoise(u * 0.5, v * 0.5, 214)) };
        let wet = (b - 22.2) / (BAND - 22.2);
        scale(base, 1.0 - 0.3 * wet)
    };
    // Limescale and rust running down from everything.
    let streak = vnoise(u * 0.9, v * 0.02, 215);
    let c = if streak > 0.7 { mix(c, scale(RUST, 0.6), (streak - 0.7) * 2.0) } else { c };
    scale(c, 0.8 * grime)
}

/// Glazed tiles with dark grout, whole patches fallen off.
fn tiles(u: f32, v: f32) -> Rgb {
    const T: f32 = 2.6;
    let (tx, ty) = ((u / T).floor() as i32, (v / T).floor() as i32);
    let (fx, fy) = ((u / T).fract(), (v / T).fract());
    let gone = vnoise(u * 0.06, v * 0.06, 216) > 0.66 && hash(tx, ty, 217) > 0.25 || hash(tx, ty, 218) > 0.95;
    if gone {
        // Bare render behind, mortar dabs.
        return scale(CAST, 0.4 + 0.08 * hash(tx, ty, 219));
    }
    if fx < 0.1 || fy < 0.1 {
        return scale(SOOT, 1.6);
    }
    // Glaze catching light on the upper edge of each tile.
    let glaze = if fy < 0.25 { 1.12 } else { 1.0 };
    scale(TILE, 0.46 * glaze * (0.85 + 0.2 * hash(tx, ty, 220)))
}
