//! Ground material of "region: cold war military bunker" (`Style::Bunker`):
//! poured blocks of reinforced concrete (formwork tie holes, spalls showing
//! the rebar mesh) patched with riveted steel plates, hazard paint along the
//! lit edges; the corridors behind are formwork walls with steel columns,
//! conduit runs, a striped warning band and fallout-shelter plaques.

use super::*;

/// Cold, slightly green-gray poured concrete.
const CONC: Rgb = [0.41, 0.42, 0.40];
/// Riveted steel plate.
const PLATE: Rgb = [0.33, 0.35, 0.37];
/// Olive drab paint (on the plates, conduits and columns).
const OLIVE: Rgb = [0.34, 0.37, 0.26];
/// Hazard yellow.
const HAZARD: Rgb = [0.80, 0.68, 0.20];
const RUSTY: Rgb = [0.42, 0.26, 0.17];

/// The overall color of the ground (what the chameleon blends into).
pub const GROUND: Rgb = [0.39, 0.40, 0.38];

/// Distance to the nearest edge of a cell of size `w`×`h`, plus the cell's
/// fractional position.
fn cell_edge(x: f32, y: f32, w: f32, h: f32) -> (f32, f32, f32) {
    let (fx, fy) = ((x / w).rem_euclid(1.0), (y / h).rem_euclid(1.0));
    ((fx.min(1.0 - fx) * w).min(fy.min(1.0 - fy) * h), fx, fy)
}

/// The bare material of solid ground, before lighting (`u`, `v` in units of 5 world units).
pub fn material(u: f32, v: f32) -> Rgb {
    let blotch = 0.86 + 0.28 * vnoise(u * 0.12, v * 0.12, 404);
    // Big poured blocks in staggered courses.
    let (bw, bh) = (30.0, 14.0);
    let row = (v / bh).floor() as i32;
    let x = u + if row.rem_euclid(2) == 0 { 0.0 } else { bw / 2.0 };
    let col = (x / bw).floor() as i32;
    let (edge, fx, fy) = cell_edge(x, v, bw, bh);
    if edge < 0.45 {
        return scale(SOOT, 1.3);
    }
    // Chamfered seams: lit along the top, shadowed along the bottom.
    let bevel = if edge < 1.1 {
        if fy < 0.5 { 1.18 } else if fx < 0.5 { 1.06 } else { 0.72 }
    } else {
        1.0
    };
    let id = hash(col, row, 401);
    let c = if id < 0.22 {
        // A riveted steel plate, olive paint flaking off.
        let paint = vnoise(u * 0.3, v * 0.3, 405) > 0.42;
        let base = if paint { OLIVE } else { PLATE };
        // Rivets along the border.
        let (lx, ly) = (fx * bw, fy * bh);
        let inset = 1.6;
        let near_border = (lx - inset).abs() < 0.6 || (bw - lx - inset).abs() < 0.6 || (ly - inset).abs() < 0.6 || (bh - ly - inset).abs() < 0.6;
        let along = if (lx - inset).abs() < 0.6 || (bw - lx - inset).abs() < 0.6 { ly } else { lx };
        let rivet = near_border && ((along / 3.0).fract() - 0.5).abs() < 0.17;
        if rivet {
            scale(PLATE, if (ly / 3.0).fract() < 0.5 { 1.5 } else { 0.8 })
        } else {
            // Rust running down from the rivet lines.
            let streak = vnoise(u * 0.7, v * 0.05, 406);
            let brushed = 0.93 + 0.1 * vnoise(u * 0.1, v * 2.0, 407);
            mix(scale(base, brushed), RUSTY, ((streak - 0.6) * 2.0).clamp(0.0, 0.7))
        }
    } else {
        // Formwork tie holes in two rows.
        let tie = [(0.2, 0.3), (0.5, 0.3), (0.8, 0.3), (0.2, 0.7), (0.5, 0.7), (0.8, 0.7)]
            .iter()
            .any(|&(tx, ty)| Vec2::new((fx - tx) * bw, (fy - ty) * bh).length() < 0.55);
        if tie {
            return scale(SOOT, 1.6);
        }
        // Spalls: the face broken off, the rebar mesh behind it.
        let spall = vnoise(u * 0.07, v * 0.07, 408);
        if spall > 0.72 {
            let (ru, rv) = ((u / 4.0).rem_euclid(1.0), (v / 4.0).rem_euclid(1.0));
            if ru < 0.14 || rv < 0.14 {
                return scale(RUSTY, 0.9 + 0.3 * hash(u as i32, v as i32, 409));
            }
            return scale(CONC, 0.45 * blotch);
        }
        if spall > 0.69 {
            return scale(CONC, 0.7);
        }
        // Hairline cracks.
        if (vnoise(u * 0.08, v * 0.08, 410) - 0.5).abs() < 0.01 {
            return SOOT;
        }
        // Pour lines (formwork boards).
        let board = if (v / 3.5).rem_euclid(1.0) < 0.08 { 0.88 } else { 1.0 };
        scale(CONC, (0.92 + 0.12 * hash(col, row, 402)) * board)
    };
    scale(c, blotch * bevel)
}

/// The lit top edge: hazard paint, worn through in places.
pub fn rim(u: f32) -> Rgb {
    let worn = vnoise(u * 0.08, 0.0, 411) > 0.7;
    let stripe = ((u / 4.0).floor() as i32).rem_euclid(2) == 0;
    let k = 0.88 + 0.12 * hash(u as i32, 0, 9);
    if worn {
        scale(RIM, 0.85 * k)
    } else if stripe {
        scale(HAZARD, k)
    } else {
        scale(SOOT, 2.0)
    }
}

/// Passable structures: the corridor walls (dark interiors behind the action).
pub fn interior(u: f32, v: f32) -> Rgb {
    let grime = 0.82 + 0.3 * vnoise(u * 0.15, v * 0.04, 420);
    // Bays between steel columns, one conduit run + warning band per bay height.
    let (bay_w, bay_h) = (72.0, 64.0);
    let (bx, by) = ((u / bay_w).floor() as i32, (v / bay_h).floor() as i32);
    let (lx, ly) = (u.rem_euclid(bay_w), v.rem_euclid(bay_h));

    // Steel I-beam column.
    if lx < 5.0 {
        let flange = lx < 1.0 || lx > 4.0;
        let rivet = flange && ((ly / 4.0).fract() - 0.5).abs() < 0.12;
        let k = if rivet { 1.5 } else if flange { 1.15 - 0.3 * (lx / 5.0) } else { 0.7 };
        return scale(OLIVE, 0.5 * k * grime);
    }
    // Conduit run: one fat pipe and two thin ones, on brackets.
    for &(top, h) in &[(9.0, 3.2), (13.2, 1.6), (15.6, 1.6)] {
        if ly >= top && ly < top + h {
            let t = (ly - top) / h;
            // Round: highlight near the top, dark underneath.
            let round = 0.45 + 0.75 * (t * std::f32::consts::PI).sin() * (1.0 - t * 0.6);
            let base = if h > 2.0 { OLIVE } else { PLATE };
            return scale(base, 0.62 * round * grime);
        }
    }
    // Brackets holding the run.
    if ly > 8.0 && ly < 18.0 && (lx % 18.0) < 1.0 {
        return scale(SOOT, 1.4);
    }
    // Warning band: diagonal hazard stripes, chipped.
    if ly > 40.0 && ly < 43.0 {
        let chipped = vnoise(u * 0.4, v * 0.4, 421) > 0.68;
        let stripe = (((u + ly) / 3.0).floor() as i32).rem_euclid(2) == 0;
        return if stripe && !chipped { scale(HAZARD, 0.42 * grime) } else { scale(SOOT, 1.3) };
    }
    // Fallout shelter plaque in some bays, between the run and the band.
    let sign = hash(bx, by, 422);
    if sign > 0.45 {
        let c = Vec2::new(bay_w * (0.3 + 0.4 * hash(bx, by, 423)), 29.0);
        let d = Vec2::new(lx, ly) - c;
        let half = 6.5;
        if d.x.abs() < half && d.y.abs() < half {
            if d.x.abs() > half - 0.6 || d.y.abs() > half - 0.6 {
                return scale(SOOT, 1.2);
            }
            // Trefoil: a dot and three blades.
            let r = d.length();
            let a = (-d.y).atan2(d.x).to_degrees().rem_euclid(120.0);
            let mark = r < 1.0 || (r > 1.6 && r < 4.6 && (a - 30.0).abs() < 30.0);
            let rust = vnoise(u * 0.8, v * 0.8, 424) > 0.7;
            return if mark { scale(SOOT, 1.5) } else if rust { scale(RUSTY, 0.5) } else { scale(HAZARD, 0.52) };
        }
    }
    // Formwork concrete: panel seams and tie holes, water streaks.
    let (pw, ph) = (24.0, 12.0);
    let (edge, fx, fy) = cell_edge(u + 7.0, v, pw, ph);
    let streak = vnoise(u * 0.5, v * 0.02, 425);
    let wet = 1.0 - ((streak - 0.6) * 1.6).clamp(0.0, 0.35);
    let k = if edge < 0.35 {
        0.55
    } else if [(0.25, 0.5), (0.75, 0.5)].iter().any(|&(tx, ty)| Vec2::new((fx - tx) * pw, (fy - ty) * ph).length() < 0.45) {
        0.45
    } else {
        0.92 + 0.12 * hash(((u + 7.0) / pw).floor() as i32, (v / ph).floor() as i32, 426)
    };
    scale(CONC, 0.36 * k * wet * grime)
}
