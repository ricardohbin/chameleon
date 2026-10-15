//! Ground material of "region: buried grand central subway station" (`Style::Subway`).
//! Cross-sections of a buried station: walls of glazed subway tile with mosaic
//! trim bands, blocks of veined marble, bare backfill where the cladding fell
//! off, all split by big cracks. The lit top is a polished marble floor; the
//! passable interiors are the dark hall behind: arcades of tiled vaults.

use super::*;

/// Glazed cream tile.
const TILE: Rgb = [0.60, 0.58, 0.52];
const GROUT: Rgb = [0.15, 0.14, 0.13];
/// Veined marble blocks.
const MARBLE: Rgb = [0.56, 0.54, 0.50];
/// The mosaic trim bands (a dark green that reads darker than the tile).
const TRIM: Rgb = [0.24, 0.30, 0.26];
/// Behind the cladding: rough concrete and earth.
const BACKFILL: Rgb = [0.26, 0.24, 0.22];

/// The overall color of the ground (what the chameleon blends into).
pub const GROUND: Rgb = [0.50, 0.48, 0.44];

/// The bare material of solid ground, before lighting (`u`, `v` in units of 5 world units).
pub fn material(u: f32, v: f32) -> Rgb {
    // Big cracks through everything.
    let crack = (vnoise(u * 0.06, v * 0.06, 141) - 0.5).abs() < 0.011 || (vnoise(u * 0.035, v * 0.1, 142) - 0.5).abs() < 0.008;
    if crack {
        return SOOT;
    }
    let grime = 0.8 + 0.3 * vnoise(u * 0.12, v * 0.12, 143);
    // Where the cladding fell off: backfill with gravel.
    let bare = vnoise(u * 0.045, v * 0.06, 144) + 0.15 * vnoise(u * 0.4, v * 0.4, 145);
    if bare > 0.78 {
        let stone = hash((u * 0.8) as i32, (v * 0.8) as i32, 146);
        return scale(BACKFILL, grime * if stone > 0.8 { 1.35 } else { 0.85 + 0.2 * stone });
    }
    // A broken rim of cladding around the bare patch.
    if bare > 0.74 {
        return scale(GROUT, 1.2);
    }
    // Zones of marble blocks among the tiled walls.
    if vnoise(u * 0.02 + 7.3, v * 0.03, 147) > 0.62 {
        return scale(marble(u, v), grime);
    }
    scale(tiles(u, v), grime)
}

/// Glazed tile in running bond, a mosaic trim band every storey.
fn tiles(u: f32, v: f32) -> Rgb {
    let period = 36.0;
    let band = v.rem_euclid(period);
    if band < 3.6 {
        // Mosaic: a checker of small squares between two dark lines.
        if band < 0.6 || band > 3.0 {
            return GROUT;
        }
        let (i, j) = ((u / 1.2).floor() as i32, ((band - 0.6) / 1.2).floor() as i32);
        let fx = (u / 1.2).fract();
        if fx < 0.18 {
            return scale(GROUT, 1.4);
        }
        let light = (i + j) % 2 == 0 && hash(i, j, 148) > 0.25;
        return if light { scale(TILE, 0.8) } else { scale(TRIM, 0.9 + 0.2 * hash(i, j, 149)) };
    }
    let (w, h) = (4.0, 2.0);
    let row = (v / h).floor() as i32;
    let x = u / w + if row % 2 == 0 { 0.0 } else { 0.5 };
    let (fx, fy) = (x.fract(), (v / h).fract());
    if fx < 0.14 || fy < 0.27 {
        return GROUT;
    }
    let col = x.floor() as i32;
    let id = hash(col, row, 150);
    if id > 0.93 {
        // A missing tile: the dark mortar bed behind.
        return scale(BACKFILL, 0.7);
    }
    // Glaze: brighter near the top of each tile, a few darker replacement tiles.
    let glaze = if fy < 0.5 { 1.15 } else { 0.95 };
    let k = if id < 0.06 { 0.7 } else { 0.9 + 0.15 * hash(col, row, 151) };
    scale(TILE, k * glaze)
}

/// Big ashlar blocks of marble, veined.
fn marble(u: f32, v: f32) -> Rgb {
    let (w, h) = (14.0, 6.0);
    let row = (v / h).floor() as i32;
    let x = u / w + hash(row, 3, 152) * 0.7;
    let (fx, fy) = (x.fract(), (v / h).fract());
    if fx * w < 0.6 || fy * h < 0.6 {
        return GROUT;
    }
    let col = x.floor() as i32;
    let tone = 0.85 + 0.25 * hash(col, row, 153);
    // Veins: thin noise contours, warped.
    let wu = u + vnoise(u * 0.1, v * 0.1, 154) * 6.0;
    let vein = (vnoise(wu * 0.07, v * 0.18, 155) - 0.5).abs();
    let k = if vein < 0.02 { 0.55 } else if vein < 0.05 { 0.85 } else { 1.0 };
    scale(MARBLE, tone * k)
}

/// The lit top edge: polished marble floor slabs.
pub fn rim(u: f32) -> Rgb {
    let slab = (u / 9.0).floor() as i32;
    if (u / 9.0).fract() * 9.0 < 0.7 {
        return scale(RIM, 0.55);
    }
    scale(RIM, 0.92 + 0.12 * hash(slab, 0, 156))
}

/// Passable structures: the station hall behind, an arcade of tiled vaults
/// between marble pilasters, in the dark.
pub fn interior(u: f32, v: f32) -> Rgb {
    let grime = 0.8 + 0.3 * vnoise(u * 0.2, v * 0.04, 157);
    // Water streaks running down.
    let streak = 0.85 + 0.15 * vnoise(u * 0.7, v * 0.015, 158);
    let (bay, storey) = (44.0, 56.0);
    let x = u.rem_euclid(bay);
    let y = v.rem_euclid(storey);
    let pier = 5.0;
    if x < pier {
        // Pilaster: lit edge on one side, fluting.
        let edge = if x < 0.8 { 1.25 } else if x > pier - 0.8 { 0.7 } else { 1.0 };
        let flute = if (x * 1.2).fract() < 0.25 { 0.85 } else { 1.0 };
        return scale(MARBLE, 0.36 * edge * flute * grime);
    }
    // The arch springs from the piers: everything above its curve is the vault.
    let half = (bay - pier) / 2.0;
    let dx = x - pier - half;
    let spring = 22.0;
    let arch_top = spring - (half * half - dx * dx).max(0.0).sqrt();
    if y < arch_top - 1.2 {
        // Vault: coffered, very dark.
        let coffer = ((u / 4.0).fract() < 0.1) || ((v / 4.0).fract() < 0.1);
        return scale(BACKFILL, 0.55 * grime * if coffer { 0.6 } else { 1.0 });
    }
    if y < arch_top + 0.6 {
        // The arch's rim of stone.
        return scale(MARBLE, 0.42 * grime);
    }
    if (y - storey + 4.0).abs() < 1.6 {
        // A trim band at dado height.
        return scale(TRIM, 0.55 * grime);
    }
    // Recessed tile wall.
    let row = (v / 2.5).floor() as i32;
    let tx = u / 5.0 + if row % 2 == 0 { 0.0 } else { 0.5 };
    let grout = tx.fract() < 0.12 || (v / 2.5).fract() < 0.22;
    let k = if grout { 0.6 } else { 0.9 + 0.15 * hash(tx.floor() as i32, row, 159) };
    scale(TILE, 0.3 * k * grime * streak)
}
