//! Ground material of "region: abandoned shopping center" (`Style::Mall`).
//! The ground is the mall's own floors seen in section: glossy checkered tiles
//! (a light streak of reflection running across them, cracked and grimy, some
//! popped out down to the trowelled adhesive), stacked storey by storey on
//! concrete slabs. Interiors are the shop fronts: a drop-ceiling grid with
//! missing panels, a fascia band, and storefronts behind half-rolled shutters.

use super::*;

/// The light and dark tiles of the checker.
const IVORY: Rgb = [0.60, 0.57, 0.53];
const SLATE: Rgb = [0.28, 0.27, 0.26];
const GROUT: Rgb = [0.12, 0.12, 0.12];
const ADHESIVE: Rgb = [0.30, 0.29, 0.27];
const TRIM: Rgb = [0.80, 0.78, 0.74];

/// One tile, in units of 5 world units.
const TILE_W: f32 = 4.0;
/// One storey of the building (`terrain::FLOOR_HEIGHT` in the same units).
const STOREY: f32 = 96.0 / 5.0;

/// The overall color of the ground (what the chameleon blends into).
pub const GROUND: Rgb = [0.44, 0.42, 0.39];

/// The bare material of solid ground, before lighting (`u`, `v` in units of 5 world units).
pub fn material(u: f32, v: f32) -> Rgb {
    let blotch = 0.8 + 0.35 * vnoise(u * 0.12, v * 0.12, 64);
    // Storeys: a slab band between the tiled floors.
    let sv = (v / STOREY).fract() * STOREY;
    if sv < 2.2 {
        let pour = if (u / 9.0 + hash((v / STOREY) as i32, 0, 65) * 3.0).fract() < 0.03 { 0.6 } else { 1.0 };
        let edge = if sv < 0.35 || sv > 1.85 { 0.55 } else { 1.0 };
        return scale(CONCRETE, 0.75 * blotch * pour * edge);
    }
    // Long cracks running through everything.
    let crack = (vnoise(u * 0.07, v * 0.07, 66) - 0.5).abs() < 0.010 || (vnoise(u * 0.11, v * 0.04, 67) - 0.5).abs() < 0.007;
    if crack {
        return SOOT;
    }
    let (gx, gy) = (u / TILE_W, v / TILE_W);
    let (tx, ty) = (gx.floor() as i32, gy.floor() as i32);
    let (fx, fy) = (gx.fract(), gy.fract());
    let id = hash(tx, ty, 68);
    if id > 0.9 {
        // Popped tile: the comb marks of the adhesive underneath, soot in the hole.
        let comb = if ((u + v * 0.4) * 1.6).fract() < 0.35 { 0.7 } else { 1.0 };
        let rim = if fx.min(1.0 - fx).min(fy.min(1.0 - fy)) < 0.08 { 0.5 } else { 1.0 };
        return scale(ADHESIVE, comb * rim * blotch);
    }
    if fx < 0.05 || fy < 0.05 {
        return scale(GROUT, blotch);
    }
    let light = (tx + ty).rem_euclid(2) == 0;
    let base = if light { IVORY } else { SLATE };
    // Glossy: each tile brighter toward its top-left, plus a reflected light streak.
    let gloss = 1.06 - 0.14 * (fx + fy) * 0.5;
    let streak = ((u - v * 0.7) / 26.0 + 0.3 * vnoise(v * 0.05, 0.0, 69)).fract();
    let shine = if streak < 0.05 { 1.35 } else if streak < 0.09 { 1.15 } else { 1.0 };
    // Some tiles shattered from a corner (a star of hairline cracks).
    let shatter = id > 0.78 && {
        let (cx, cy) = (hash(tx, ty, 70), hash(tx, ty, 71));
        let a = (fy - cy).atan2(fx - cx);
        let spoke = (a * 7.0 / std::f32::consts::TAU + hash(tx, ty, 72)).fract();
        spoke < 0.06
    };
    if shatter {
        return scale(base, 0.45);
    }
    // Grime pooling along the grout.
    let near_grout = (fx.min(1.0 - fx).min(fy.min(1.0 - fy)) * 8.0).min(1.0);
    let grime = 0.8 + 0.2 * near_grout;
    scale(base, gloss * shine * grime * (0.85 + 0.25 * blotch) * (0.92 + 0.12 * id))
}

/// The lit top edge: a stainless nosing strip with seams at the tile joints.
pub fn rim(u: f32) -> Rgb {
    let seam = if (u / TILE_W).fract() < 0.06 { 0.6 } else { 1.0 };
    scale(TRIM, seam * (0.85 + 0.15 * hash(u as i32, 0, 73)))
}

/// Passable structures: shop fronts, storey by storey.
pub fn interior(u: f32, v: f32) -> Rgb {
    let grime = 0.8 + 0.3 * vnoise(u * 0.2, v * 0.05, 74);
    let storey = (v / STOREY).floor() as i32;
    let sv = (v / STOREY).fract() * STOREY;
    let k = if sv < 2.6 {
        // Drop ceiling: a grid of panels, some gone (the dark void above).
        let (px, py) = (u / 3.0, (sv + 0.2) / 1.3);
        let (cx, cy) = (px.floor() as i32, py.floor() as i32);
        if px.fract() < 0.1 || py.fract() < 0.12 {
            0.30
        } else if hash(cx, cy + storey * 7, 75) > 0.6 {
            0.05
        } else {
            0.20 + 0.04 * hash(cx, cy, 76)
        }
    } else if sv < 3.6 {
        // Fascia: the sign band, its letters mostly torn off.
        let cell = ((u / 1.2).floor() as i32, storey);
        if sv > 2.8 && sv < 3.4 && hash(cell.0, cell.1, 77) > 0.55 { 0.30 } else { 0.16 }
    } else {
        // Storefronts between pillars, shutters rolled down to different heights.
        let bay_w = 15.0;
        let bay = (u / bay_w).floor() as i32;
        let fx = (u / bay_w).fract();
        let h = STOREY - 3.6;
        let y = sv - 3.6;
        if fx < 0.05 {
            0.24
        } else {
            let down = hash(bay, storey, 78) * 1.1;
            if y < down * h {
                // Shutter slats; a few dented dark.
                let slat = (y * 2.2).fract() < 0.28;
                let dent = hash(bay, (y * 2.2) as i32, 79) > 0.93;
                if slat { 0.09 } else if dent { 0.12 } else { 0.21 }
            } else {
                // The looted shop behind: shelves, glass glints, the tiled floor.
                let shelf = (y / 3.4).fract() < 0.08 && fx > 0.15 && fx < 0.85;
                let glint = hash((u * 1.5) as i32, (v * 1.5) as i32, 80) > 0.997;
                if glint {
                    0.30
                } else if shelf {
                    0.13
                } else if y > h - 1.4 {
                    if ((u / 1.4).floor() as i32 + (y / 0.7) as i32).rem_euclid(2) == 0 { 0.14 } else { 0.07 }
                } else {
                    0.06
                }
            }
        }
    };
    scale([0.40, 0.39, 0.37], k * grime * 1.4)
}
