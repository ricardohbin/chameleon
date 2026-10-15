//! Ground material of "region: natural cave system" (`Style::Cave`).
//! Limestone under the city: sedimentary beds folded by the ground's slow
//! heave (pale limestone, mid marl, dark laminated shale), broken by joints
//! that stop at the bedding planes, cut by bright calcite veins and dark
//! fractures, pitted with fossils and the odd crystal geode, streaked where
//! water has run down the rock for ten thousand years.

use super::*;

const LIME: Rgb = [0.56, 0.54, 0.50];
const MARL: Rgb = [0.40, 0.39, 0.37];
const SHALE: Rgb = [0.22, 0.22, 0.23];
const CALCITE: Rgb = [0.80, 0.78, 0.73];

/// The overall color of the ground (what the chameleon blends into).
pub const GROUND: Rgb = [0.44, 0.42, 0.39];

/// The folded coordinate across the beds (grows downward like `v`).
fn fold(u: f32, v: f32) -> f32 {
    v + (u * 0.019 + vnoise(u * 0.004, 0.0, 400) * 6.0).sin() * 9.0
        + (vnoise(u * 0.011, v * 0.011, 401) - 0.5) * 46.0
        + (vnoise(u * 0.07, v * 0.07, 402) - 0.5) * 2.5
}

/// One bed of rock: (its color, its index).
fn bed(f: f32) -> (Rgb, i32) {
    let i = (f / 3.4).floor() as i32;
    let kind = hash(i, 0, 410);
    let k = 0.86 + 0.24 * hash(i, 1, 411);
    let c = if kind < 0.55 {
        LIME
    } else if kind < 0.8 {
        MARL
    } else {
        SHALE
    };
    (scale(c, k), i)
}

/// The rock of the beds, without veins and fractures (also the interiors).
fn beds(u: f32, v: f32) -> Rgb {
    let f = fold(u, v);
    let (c, i) = bed(f);
    let t = (f / 3.4).fract();
    let mut c = c;
    // Shale is finely laminated.
    if hash(i, 0, 410) >= 0.8 && (f * 1.7).fract() < 0.35 {
        c = scale(c, 0.8);
    }
    // Bedding planes: a dark parting at the bottom of each bed, a lighter skin over it.
    if t > 0.93 {
        c = mix(c, SOOT, 0.65);
    } else if t < 0.06 {
        c = scale(c, 1.12);
    }
    c
}

/// The bare material of solid ground, before lighting (`u`, `v` in units of 5 world units).
pub fn material(u: f32, v: f32) -> Rgb {
    let f = fold(u, v);
    let (_, i) = bed(f);
    let mut c = beds(u, v);
    let blotch = 0.88 + 0.24 * vnoise(u * 0.18, v * 0.18, 404);
    c = scale(c, blotch);

    // Fossil pits and pores in the limestone.
    if hash(i, 0, 410) < 0.55 && hash((u * 1.4) as i32, (v * 1.4) as i32, 405) > 0.985 {
        c = scale(c, 0.55);
    }
    // Joints: near-vertical cracks inside a bed, each bed breaking on its own pattern.
    let w = 12.0 + hash(i, 2, 412) * 18.0;
    let jx = u + (v - i as f32 * 3.4) * (hash(i, 4, 426) - 0.5) * 0.8 + hash(i, 3, 413) * 30.0 + (vnoise(u * 0.1, v * 0.35, 414) - 0.5) * 2.5;
    let jd = (jx / w).fract() * w;
    if jd < 0.5 && hash((jx / w).floor() as i32, i, 415) > 0.68 {
        c = mix(c, SOOT, 0.75);
    } else if jd < 1.0 && hash((jx / w).floor() as i32, i, 415) > 0.68 {
        c = scale(c, 0.88);
    }
    // Water stains running down the face.
    let streak = vnoise(u * 0.45, v * 0.018, 416);
    if streak > 0.68 {
        c = scale(c, 1.0 - (streak - 0.68) * 1.4);
    }
    // Calcite veins (bright, crossing the beds) and a finer network.
    let (wu, wv) = (u + (vnoise(u * 0.3, v * 0.3, 427) - 0.5) * 4.0, v + (vnoise(u * 0.3, v * 0.3, 428) - 0.5) * 4.0);
    let vein = (vnoise(wu * 0.09 + wv * 0.04, wv * 0.1, 417) - 0.5).abs();
    let fine = (vnoise(wu * 0.22, wv * 0.19, 418) - 0.5).abs();
    let veined = vnoise(u * 0.02, v * 0.02, 419);
    if (vein < 0.012 && veined > 0.45) || (fine < 0.008 && veined > 0.62) {
        c = CALCITE;
    } else if vein < 0.022 && veined > 0.45 {
        c = mix(c, CALCITE, 0.35);
    }
    // Big fractures: dark, with a pale weathered halo.
    let crack = (vnoise(wu * 0.03, wv * 0.034, 420) - 0.5).abs();
    if crack < 0.006 {
        c = SOOT;
    } else if crack < 0.012 {
        c = mix(c, CALCITE, 0.25);
    }
    // Geodes: a dark rind around a pocket of faceted crystal.
    let cell = 16.0;
    let (gx, gy) = ((u / cell).floor() as i32, (v / cell).floor() as i32);
    if hash(gx, gy, 421) > 0.9 {
        let center = Vec2::new((gx as f32 + 0.3 + 0.4 * hash(gx, gy, 422)) * cell, (gy as f32 + 0.3 + 0.4 * hash(gx, gy, 423)) * cell);
        let r = 1.6 + 1.8 * hash(gx, gy, 424);
        let d = Vec2::new(u, v) - center;
        let squash = Vec2::new(d.x, d.y * 1.5);
        let dist = squash.length();
        if dist < r * 0.62 {
            let facet = ((d.y.atan2(d.x) / std::f32::consts::TAU + 0.5) * 7.0).floor() as i32;
            let k = 0.55 + 0.45 * hash(facet, gx * 31 + gy, 425);
            c = scale(CALCITE, k * (0.7 + 0.3 * dist / (r * 0.62)));
        } else if dist < r {
            c = scale(SOOT, 1.3);
        }
    }
    c
}

/// The lit top edge: a damp crust of pale calcite, broken here and there.
pub fn rim(u: f32) -> Rgb {
    let k = 0.82 + 0.18 * hash(u as i32, 0, 430);
    if hash((u * 0.4) as i32, 1, 431) > 0.85 {
        scale(LIME, 1.1 * k)
    } else {
        scale(mix(RIM, CALCITE, 0.5), k)
    }
}

/// Passable structures (the open cave behind the action): the far wall of
/// the cavern, the same folded beds but soft and deep in shadow, draped with
/// flowstone and streaked by seeping water.
pub fn interior(u: f32, v: f32) -> Rgb {
    let f = fold(u, v);
    let (c, _) = bed(f);
    let mut c = mix(GROUND, c, 0.4);
    let t = (f / 3.4).fract();
    if t > 0.94 {
        c = scale(c, 0.82);
    }
    let drape = 0.84 + 0.16 * ((u * 0.9 + vnoise(u * 0.1, v * 0.03, 432) * 6.0).sin() * 0.5 + 0.5);
    let wet = 0.85 + 0.25 * vnoise(u * 0.25, v * 0.03, 433);
    let blotch = 0.85 + 0.3 * vnoise(u * 0.05, v * 0.05, 434);
    scale(c, 0.34 * drape * wet * blotch)
}
