//! Tiny vector-art toolkit: smooth outlines and shaded meshes built in code.
//!
//! Every mesh carries per-vertex grayscale "shade" colors. `ColorMaterial`
//! multiplies them with the material color, so one material (e.g. the
//! chameleon skin) can be re-tinted at runtime while keeping its shading.

use bevy::{asset::RenderAssetUsages, mesh::Indices, prelude::*, render::render_resource::PrimitiveTopology};

/// Closed Catmull-Rom spline through `pts` (`steps` samples per segment).
pub fn smooth_closed(pts: &[Vec2], steps: usize) -> Vec<Vec2> {
    let n = pts.len();
    let mut out = Vec::with_capacity(n * steps);
    for i in 0..n {
        let (p0, p1, p2, p3) = (pts[(i + n - 1) % n], pts[i], pts[(i + 1) % n], pts[(i + 2) % n]);
        for s in 0..steps {
            out.push(catmull_rom(p0, p1, p2, p3, s as f32 / steps as f32));
        }
    }
    out
}

/// Open Catmull-Rom spline through `pts` (endpoints are included).
pub fn smooth_open(pts: &[Vec2], steps: usize) -> Vec<Vec2> {
    let n = pts.len();
    let mut out = Vec::with_capacity(n * steps);
    for i in 0..n - 1 {
        let p0 = pts[i.saturating_sub(1)];
        let p3 = pts[(i + 2).min(n - 1)];
        for s in 0..steps {
            out.push(catmull_rom(p0, pts[i], pts[i + 1], p3, s as f32 / steps as f32));
        }
    }
    out.push(pts[n - 1]);
    out
}

fn catmull_rom(p0: Vec2, p1: Vec2, p2: Vec2, p3: Vec2, t: f32) -> Vec2 {
    let (t2, t3) = (t * t, t * t * t);
    0.5 * (2.0 * p1
        + (p2 - p0) * t
        + (2.0 * p0 - 5.0 * p1 + 4.0 * p2 - p3) * t2
        + (3.0 * p1 - p0 - 3.0 * p2 + p3) * t3)
}

/// Points on an ellipse, counter-clockwise.
pub fn ellipse_pts(rx: f32, ry: f32, n: usize) -> Vec<Vec2> {
    (0..n)
        .map(|i| {
            let a = i as f32 / n as f32 * std::f32::consts::TAU;
            Vec2::new(rx * a.cos(), ry * a.sin())
        })
        .collect()
}

/// Fills a star-shaped outline with a triangle fan from its centroid.
/// `shade(p)` returns the brightness multiplier at each vertex.
pub fn fill(outline: &[Vec2], shade: impl Fn(Vec2) -> f32) -> Mesh {
    let center = outline.iter().copied().sum::<Vec2>() / outline.len() as f32;
    let mut pos = vec![center];
    pos.extend_from_slice(outline);
    let colors = pos.iter().map(|&p| gray(shade(p))).collect();
    let n = outline.len() as u32;
    let indices = (0..n).flat_map(|i| [0, 1 + i, 1 + (i + 1) % n]).collect();
    build(&pos, colors, indices)
}

/// Several star-shaped outlines in one mesh (e.g. a splat and its droplets).
pub fn blobs(outlines: &[Vec<Vec2>], shade: impl Fn(Vec2) -> f32) -> Mesh {
    let (mut pos, mut indices) = (Vec::new(), Vec::new());
    for outline in outlines {
        let base = pos.len() as u32;
        let n = outline.len() as u32;
        pos.push(outline.iter().copied().sum::<Vec2>() / n as f32);
        pos.extend_from_slice(outline);
        indices.extend((0..n).flat_map(|i| [base, base + 1 + i, base + 1 + (i + 1) % n]));
    }
    let colors = pos.iter().map(|&p| gray(shade(p))).collect();
    build(&pos, colors, indices)
}

/// A ribbon along `path`. `width(t)` gives the width at t in 0..=1.
/// `shade_left` / `shade_right` shade the two edges (left = `tangent.perp()`).
/// `pattern(t)` multiplies the shade along the length (stripes, bands...).
pub fn stroke(
    path: &[Vec2],
    width: impl Fn(f32) -> f32,
    shade_left: f32,
    shade_right: f32,
    pattern: impl Fn(f32) -> f32,
) -> Mesh {
    let n = path.len();
    let mut pos = Vec::with_capacity(n * 2);
    let mut colors = Vec::with_capacity(n * 2);
    for i in 0..n {
        let t = i as f32 / (n - 1) as f32;
        let tangent = (path[(i + 1).min(n - 1)] - path[i.saturating_sub(1)]).normalize_or(Vec2::X);
        let offset = tangent.perp() * width(t) * 0.5;
        let k = pattern(t);
        pos.push(path[i] + offset);
        pos.push(path[i] - offset);
        colors.push(gray(shade_left * k));
        colors.push(gray(shade_right * k));
    }
    let indices = (0..n as u32 - 1)
        .flat_map(|i| {
            let (a, b, c, d) = (2 * i, 2 * i + 1, 2 * i + 2, 2 * i + 3);
            [a, b, c, b, d, c]
        })
        .collect();
    build(&pos, colors, indices)
}

/// A flat triangle-list mesh from explicit triangles (all fully lit).
pub fn triangles(tris: &[[Vec2; 3]]) -> Mesh {
    let pos: Vec<Vec2> = tris.iter().flatten().copied().collect();
    let colors = vec![gray(1.0); pos.len()];
    let indices = (0..pos.len() as u32).collect();
    build(&pos, colors, indices)
}

/// Rectangle with a vertical gradient, `top` and `bottom` are full colors.
pub fn gradient_rect(size: Vec2, top: Color, bottom: Color) -> Mesh {
    let h = size / 2.0;
    let pos = [Vec2::new(-h.x, h.y), Vec2::new(h.x, h.y), Vec2::new(-h.x, -h.y), Vec2::new(h.x, -h.y)];
    let (t, b) = (top.to_linear().to_f32_array(), bottom.to_linear().to_f32_array());
    build(&pos, vec![t, t, b, b], vec![0, 2, 1, 1, 2, 3])
}

/// The strip between two polylines of the same length (`top[i]` faces
/// `bottom[i]`), shaded per vertex.
pub fn band(top: &[Vec2], bottom: &[Vec2], shade: impl Fn(Vec2) -> f32) -> Mesh {
    let pos: Vec<Vec2> = top.iter().zip(bottom).flat_map(|(&t, &b)| [t, b]).collect();
    let colors = pos.iter().map(|&p| gray(shade(p))).collect();
    let indices = (0..top.len().saturating_sub(1) as u32)
        .flat_map(|i| {
            let (a, b, c, d) = (2 * i, 2 * i + 1, 2 * i + 2, 2 * i + 3);
            [a, b, c, b, d, c]
        })
        .collect();
    build(&pos, colors, indices)
}

fn gray(v: f32) -> [f32; 4] {
    [v, v, v, 1.0]
}

fn build(pos: &[Vec2], colors: Vec<[f32; 4]>, indices: Vec<u32>) -> Mesh {
    let n = pos.len();
    Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default())
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, pos.iter().map(|p| [p.x, p.y, 0.0]).collect::<Vec<_>>())
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, vec![[0.0, 0.0, 1.0]; n])
        .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, vec![[0.0, 0.0]; n])
        .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, colors)
        .with_inserted_indices(Indices::U32(indices))
}

/// Irregular rock / rubble chunk outline.
pub fn rock_pts(rng: &mut Rng, r: f32, n: usize) -> Vec<Vec2> {
    (0..n)
        .map(|i| {
            let a = (i as f32 + rng.range(-0.3, 0.3)) / n as f32 * std::f32::consts::TAU;
            Vec2::new(a.cos(), a.sin() * 0.8) * r * rng.range(0.65, 1.0)
        })
        .collect()
}

pub fn scale_color(c: Color, k: f32) -> Color {
    let s = c.to_srgba();
    Color::srgba((s.red * k).min(1.0), (s.green * k).min(1.0), (s.blue * k).min(1.0), s.alpha)
}

/// Deterministic xorshift RNG, so generated content is the same every run.
#[derive(Clone)]
pub struct Rng(pub u32);

impl Rng {
    pub fn next(&mut self) -> f32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 17;
        self.0 ^= self.0 << 5;
        (self.0 >> 8) as f32 / (1u32 << 24) as f32
    }

    pub fn range(&mut self, lo: f32, hi: f32) -> f32 {
        lo + (hi - lo) * self.next()
    }

    pub fn below(&mut self, n: usize) -> usize {
        ((self.next() * n as f32) as usize).min(n - 1)
    }

    pub fn chance(&mut self, p: f32) -> bool {
        self.next() < p
    }
}
