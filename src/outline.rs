//! Turns a pixel mask into smooth outlines and rasterizes them antialiased.
//!
//! The mask's boundary is traced into closed loops along pixel corners, then
//! simplified (Douglas-Peucker): 1-pixel staircases collapse into straight
//! slopes (the sketch's ramps), while 2+ pixel steps and corners survive.

use std::collections::HashMap;

use bevy::math::{IVec2, Vec2};

/// How far (in pixels) a simplified outline may stray from the pixel edges.
const TOLERANCE: f32 = 0.6;

/// Closed loops around the `true` pixels of a `w`×`h` mask (pixel corners at
/// integer coordinates, y down). Outer boundaries and holes alike.
pub fn trace(w: i32, h: i32, inside: impl Fn(i32, i32) -> bool) -> Vec<Vec<Vec2>> {
    // Directed edges, inside on the right (y down): top, right, bottom, left.
    let mut edges: Vec<(IVec2, IVec2)> = Vec::new();
    for y in 0..h {
        for x in 0..w {
            if !inside(x, y) {
                continue;
            }
            let p = IVec2::new(x, y);
            if !inside(x, y - 1) {
                edges.push((p, p + IVec2::X));
            }
            if !inside(x + 1, y) {
                edges.push((p + IVec2::X, p + IVec2::ONE));
            }
            if !inside(x, y + 1) {
                edges.push((p + IVec2::ONE, p + IVec2::Y));
            }
            if !inside(x - 1, y) {
                edges.push((p + IVec2::Y, p));
            }
        }
    }
    let mut from: HashMap<IVec2, Vec<usize>> = HashMap::new();
    for (i, e) in edges.iter().enumerate() {
        from.entry(e.0).or_default().push(i);
    }
    let mut used = vec![false; edges.len()];
    let mut loops = Vec::new();
    for start in 0..edges.len() {
        if used[start] {
            continue;
        }
        let mut pts = Vec::new();
        let mut cur = start;
        loop {
            used[cur] = true;
            let (a, b) = edges[cur];
            pts.push(a);
            let dir = b - a;
            // Where two loops touch at a corner, keep turning right so they stay apart.
            let next = from[&b].iter().copied().filter(|&i| !used[i]).min_by_key(|&i| {
                let d = edges[i].1 - edges[i].0;
                let cross = dir.x * d.y - dir.y * d.x;
                if cross > 0 { 0 } else if cross == 0 { 1 } else { 2 }
            });
            match next {
                Some(n) => cur = n,
                None => break,
            }
        }
        loops.push(simplify(&pts));
    }
    loops
}

/// Douglas-Peucker on a closed loop of pixel corners.
fn simplify(pts: &[IVec2]) -> Vec<Vec2> {
    // Drop collinear corners first.
    let n = pts.len();
    let corners: Vec<Vec2> = (0..n)
        .filter(|&i| {
            let (a, b, c) = (pts[(i + n - 1) % n], pts[i], pts[(i + 1) % n]);
            (b - a) != (c - b)
        })
        .map(|i| pts[i].as_vec2())
        .collect();
    // 1-pixel stairs are ramps: a 1-pixel edge between two edges going the same
    // way becomes its midpoint, so the outline runs straight through the steps.
    let n = corners.len();
    let edge = |i: usize| corners[(i + 1) % n] - corners[i % n];
    let stair = |i: usize| {
        let (prev, e, next) = (edge(i + n - 1), edge(i), edge(i + 1));
        e.length_squared() == 1.0 && prev.normalize() == next.normalize()
    };
    let mut ramped = Vec::with_capacity(n);
    for i in 0..n {
        if !stair(i + n - 1) && !stair(i) {
            ramped.push(corners[i]);
        }
        if stair(i) {
            ramped.push(corners[i] + edge(i) / 2.0);
        }
    }
    let corners = ramped;
    if corners.len() < 4 {
        return corners;
    }
    // Split the loop at its two most distant corners and simplify each half.
    let far = (1..corners.len())
        .max_by(|&a, &b| corners[a].distance_squared(corners[0]).total_cmp(&corners[b].distance_squared(corners[0])))
        .unwrap();
    let mut keep = vec![false; corners.len()];
    keep[0] = true;
    keep[far] = true;
    let mut stack = vec![(0, far), (far, corners.len())];
    while let Some((a, b)) = stack.pop() {
        let (pa, pb) = (corners[a], corners[b % corners.len()]);
        let mut best = (0.0, 0);
        for i in a + 1..b {
            let d = dist_to_segment(corners[i], pa, pb);
            if d > best.0 {
                best = (d, i);
            }
        }
        if best.0 > TOLERANCE {
            keep[best.1] = true;
            stack.push((a, best.1));
            stack.push((best.1, b));
        }
    }
    corners.iter().zip(keep).filter(|(_, k)| *k).map(|(p, _)| *p).collect()
}

fn dist_to_segment(p: Vec2, a: Vec2, b: Vec2) -> f32 {
    let ab = b - a;
    let t = ((p - a).dot(ab) / ab.length_squared().max(1e-6)).clamp(0.0, 1.0);
    p.distance(a + ab * t)
}

/// Antialiased coverage (0..1) of the loops (even-odd) over a grid of
/// `w`×`h` samples, each `1 / scale` pixels wide, starting at `origin`.
pub fn rasterize(loops: &[Vec<Vec2>], origin: Vec2, scale: f32, w: usize, h: usize) -> Vec<f32> {
    const SUB: usize = 4;
    let mut cov = vec![0.0f32; w * h];
    // Edges in sample space, keeping only those that reach this area.
    let segs: Vec<(Vec2, Vec2)> = loops
        .iter()
        .flat_map(|l| (0..l.len()).map(move |i| (l[i], l[(i + 1) % l.len()])))
        .map(|(a, b)| ((a - origin) * scale, (b - origin) * scale))
        .filter(|(a, b)| a.y.max(b.y) >= 0.0 && a.y.min(b.y) <= h as f32 && a.y != b.y)
        .collect();
    let mut xs: Vec<f32> = Vec::new();
    for row in 0..h {
        let line = &mut cov[row * w..(row + 1) * w];
        for s in 0..SUB {
            let y = row as f32 + (s as f32 + 0.5) / SUB as f32;
            xs.clear();
            for (a, b) in &segs {
                let (lo, hi) = if a.y < b.y { (a, b) } else { (b, a) };
                if y >= lo.y && y < hi.y {
                    xs.push(lo.x + (y - lo.y) / (hi.y - lo.y) * (hi.x - lo.x));
                }
            }
            xs.sort_by(f32::total_cmp);
            for span in xs.chunks_exact(2) {
                let (x0, x1) = (span[0].max(0.0), span[1].min(w as f32));
                if x1 <= x0 {
                    continue;
                }
                let (i0, i1) = (x0 as usize, (x1 as usize).min(w - 1));
                let k = 1.0 / SUB as f32;
                if i0 == i1 {
                    line[i0] += (x1 - x0) * k;
                    continue;
                }
                line[i0] += (i0 as f32 + 1.0 - x0) * k;
                for c in &mut line[i0 + 1..i1] {
                    *c += k;
                }
                if i1 < w {
                    line[i1] += (x1 - i1 as f32) * k;
                }
            }
        }
    }
    cov
}
