//! Ground material of "region: sinkhole sunken street" (`Style::Sinkhole`).
//!
//! A street that dropped into the earth: the ground is cut into broken slabs
//! (columns each sunk and tilted by their own amount, dark fractures between
//! them), and every slab shows the road's cross-section: a dark asphalt crust
//! with lane paint, a pale gravel base course, then dirt full of stones and
//! severed utility pipes and cable bundles. The strata repeat downward as
//! older roads buried under newer ones, so the deep walls of the hole read
//! the same way the top does.

use super::*;

const ASPHALT: Rgb = [0.14, 0.14, 0.15];
const PAINT: Rgb = [0.90, 0.84, 0.55];
const GRAVEL: Rgb = [0.56, 0.54, 0.50];
const DIRT: Rgb = [0.27, 0.22, 0.18];
const CLAY: Rgb = [0.35, 0.29, 0.22];
const PIPE: Rgb = [0.60, 0.60, 0.60];

/// The overall color of the ground (what the chameleon blends into).
pub const GROUND: Rgb = [0.27, 0.25, 0.23];

/// Height of one buried road (crust + base + fill), in material units.
const PERIOD: f32 = 44.0;

/// Which slab a point belongs to, and its depth below that slab's road top.
fn slab(u: f32, v: f32) -> (i32, f32, f32) {
    // Ragged fracture lines between slab columns.
    let wu = u + (vnoise(u * 0.02, v * 0.07, 41) - 0.5) * 14.0;
    let width = 34.0;
    let col = (wu / width).floor() as i32;
    let fx = (wu / width).fract();
    let sink = hash(col, 0, 42) * PERIOD;
    let tilt = (hash(col, 1, 43) - 0.5) * 0.35;
    let depth = v + sink + tilt * (fx - 0.5) * width;
    // Distance to the fracture, in units.
    let edge = fx.min(1.0 - fx) * width;
    (col, depth, edge)
}

/// The bare material of solid ground, before lighting (`u`, `v` in units of 5 world units).
pub fn material(u: f32, v: f32) -> Rgb {
    let (col, depth, edge) = slab(u, v);
    // Fractures between slabs: soot-black, a little wider at random.
    if edge < 0.45 + 0.5 * vnoise(u * 0.1, v * 0.3, 44) {
        return SOOT;
    }
    let cycle = (depth / PERIOD).floor() as i32;
    let d = depth - cycle as f32 * PERIOD;
    let blotch = 0.85 + 0.3 * vnoise(u * 0.15, v * 0.15, 45);
    // Not every buried layer was paved.
    let paved = hash(col, cycle, 46) < 0.8;
    let crust = 4.0 + 2.0 * hash(col, cycle, 47);
    let base = crust + 6.0 + 3.0 * hash(col, cycle, 48);
    if paved && d < crust {
        // Asphalt: dark, fine aggregate glinting, worn lane paint near the top.
        let fleck = hash((u * 2.0) as i32, (v * 2.0) as i32, 49);
        let mut c = scale(ASPHALT, blotch * if fleck > 0.9 { 1.5 } else { 1.0 });
        let lane = hash(col, cycle, 50);
        if d > 0.5 && d < 2.0 && lane < 0.8 {
            // Dashed center line (some slabs: a solid edge line).
            let dash = lane < 0.35 || ((u + hash(col, 2, 51) * 20.0) / 9.0).fract() < 0.55;
            if dash && vnoise(u * 0.5, v * 0.5, 52) > 0.25 {
                c = scale(PAINT, 0.8 + 0.25 * vnoise(u * 0.8, 0.0, 53));
            }
        }
        // Manhole covers sitting in the crust.
        let mx = (u / 26.0).floor() as i32;
        if hash(mx, cycle * 7 + col, 54) > 0.82 {
            let cx = (mx as f32 + 0.5) * 26.0;
            let dx = (u - cx).abs();
            if dx < 4.0 && d < crust - 0.5 {
                let grid = ((u * 1.5).fract() < 0.3) ^ ((d * 1.5).fract() < 0.3);
                return scale(PIPE, if dx > 3.4 { 0.45 } else if grid { 0.55 } else { 0.75 });
            }
        }
        return c;
    }
    if paved && d < crust + 0.6 {
        // Bitumen seam under the crust.
        return scale(SOOT, 1.6);
    }
    if paved && d < base {
        // Base course: packed gravel, light and speckled.
        let stone = hash((u * 1.3) as i32, (v * 1.3) as i32, 55);
        return scale(GRAVEL, blotch * (0.75 + 0.45 * stone));
    }
    // Fill: dirt with stones, deeper reaching clay.
    let k = (d / PERIOD).clamp(0.0, 1.0);
    let mut c = scale(mix(DIRT, CLAY, k * vnoise(u * 0.07, v * 0.07, 56)), blotch);
    // Stones.
    let (sx, sy) = ((u / 3.0).floor() as i32, (v / 3.0).floor() as i32);
    if hash(sx, sy, 57) > 0.88 {
        let p = Vec2::new((u / 3.0).fract() - 0.5, (v / 3.0).fract() - 0.5);
        if p.length() < 0.25 + 0.15 * hash(sx, sy, 58) {
            c = scale(GRAVEL, 0.7 + 0.4 * hash(sx, sy, 59) - p.y);
        }
    }
    // Severed utility lines in the fill: pipes (hollow rings) and cable bundles.
    let (cx, cy) = ((u / 14.0).floor() as i32, ((d + 30.0) / PERIOD).floor() as i32);
    let h = hash(cx + col * 31, cy + cycle * 17, 60);
    if h > 0.55 {
        let center = Vec2::new((cx as f32 + 0.5) * 14.0 + (hash(cx, cycle, 61) - 0.5) * 4.0, base + 6.0 + hash(cx, cycle, 62) * (PERIOD - base - 14.0));
        let p = Vec2::new(u, d) - center;
        let r = 2.0 + 2.2 * hash(cx, cycle, 63);
        let dist = p.length();
        if h > 0.8 {
            // Pipe: lit ring, black bore.
            if dist < r * 0.6 {
                return SOOT;
            }
            if dist < r {
                return scale(PIPE, 0.6 + 0.5 * (-p.y / r).max(0.0) + 0.2 * (p.x / r));
            }
        } else if dist < r * 0.9 {
            // Cable bundle: little dark discs.
            let q = p * 1.6;
            let cell = (q.x.floor() as i32, q.y.floor() as i32);
            let f = Vec2::new(q.x.fract().abs() - 0.5, q.y.fract().abs() - 0.5);
            return if f.length() < 0.38 { scale(SOOT, 1.3 + 1.5 * hash(cell.0, cell.1, 64)) } else { scale(SOOT, 0.9) };
        }
    }
    c
}

/// The lit top edge: an asphalt lip, with flecks of curb paint.
pub fn rim(u: f32) -> Rgb {
    let k = 0.85 + 0.15 * hash(u as i32, 0, 9);
    let paint = ((u / 7.0).fract() < 0.45) && hash((u / 40.0) as i32, 3, 65) > 0.45;
    if paint {
        scale(mix(RIM, PAINT, 0.4), 1.05 * k)
    } else {
        scale(RIM, 0.8 * k)
    }
}

/// Passable structures: utility tunnels dug through the fill, pipes and
/// conduits running along them, timber shoring, all in the dark.
pub fn interior(u: f32, v: f32) -> Rgb {
    let grime = 0.8 + 0.3 * vnoise(u * 0.2, v * 0.04, 66);
    let mut c = scale(DIRT, 0.42 * grime);
    // Pipe runs along the tunnel walls.
    let band = (v / 12.0).floor() as i32;
    let fv = (v / 12.0).fract();
    if hash(band, 0, 67) > 0.45 && fv > 0.2 && fv < 0.45 {
        let t = (fv - 0.2) / 0.25;
        let shade = 0.35 + 0.35 * (1.0 - (t - 0.3).abs() * 2.0).max(0.0);
        c = scale(PIPE, shade * grime);
        // Collars.
        if ((u + hash(band, 1, 68) * 10.0) / 16.0).fract() < 0.06 {
            c = scale(PIPE, 0.55 * grime);
        }
    } else if hash(band, 2, 69) > 0.6 && fv > 0.6 && fv < 0.68 {
        // A sagging cable.
        c = scale(SOOT, 0.8);
    }
    // Timber shoring.
    if (u / 22.0).fract() < 0.1 {
        c = scale(DIRT, 0.55 * grime);
    }
    c
}
