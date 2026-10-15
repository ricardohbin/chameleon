//! Command-line tools for the world sketch, run before the game starts:
//!
//! - `cargo run -- bake`: `assets/world.aseprite` → `assets/world.baked`, the
//!   state Claude last interpreted (and what release builds embed).
//! - `cargo run -- diff`: what changed in the sketch since the bake, as a report
//!   and `screenshots/world-diff.png`. `diff <old> <new>` compares any two
//!   sketches (.aseprite or baked).
//!
//! - `cargo run -- mark <boss> <x,y>`: paints a boss marker into
//!   `assets/world.aseprite` at sketch pixels `x,y` from the origin (as the diff
//!   prints them), on a new layer of its own (the user's layers are left as
//!   they are), after backing the file up beside it. Bosses: spider,
//!   dragonfly, wasp, roach.
//!
//! Both versions are aligned on their origin markers, so resizing or extending
//! the canvas doesn't count as a change. Coordinates in the report are sketch
//! pixels relative to the origin (y down).

use std::collections::HashMap;

use bevy::math::{IRect, IVec2, Vec2};

use crate::sketch::{self, Cell, MarkerKind, Sketch};

pub const ASEPRITE: &str = "assets/world.aseprite";
pub const BAKED: &str = "assets/world.baked";
const DIFF_PNG: &str = "screenshots/world-diff.png";

fn path(rel: &str) -> String {
    format!("{}/{rel}", env!("CARGO_MANIFEST_DIR"))
}

/// Runs `bake` or `diff` if that's the first argument; false = start the game.
pub fn run_cli() -> bool {
    let result = match std::env::args().nth(1).as_deref() {
        Some("bake") => bake(),
        Some("diff") => diff(),
        Some("mark") => mark(),
        _ => return false,
    };
    if let Err(e) = result {
        eprintln!("error: {e}");
        std::process::exit(1);
    }
    true
}

/// `rel` is relative to the project (absolute paths work too).
fn read(rel: &str) -> Result<Sketch, String> {
    let file = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(rel);
    let bytes = std::fs::read(file).map_err(|e| format!("{rel}: {e}"))?;
    sketch::decode(&bytes).map_err(|e| format!("{rel}: {e}"))
}

fn bake() -> Result<(), String> {
    let sketch = read(ASEPRITE)?;
    let baked = sketch.bake();
    let check = sketch::decode(&baked)?;
    if !Diff::new(&sketch, &check).is_empty() || check.size != sketch.size {
        return Err("baked sketch doesn't read back the same".into());
    }
    std::fs::write(path(BAKED), &baked).map_err(|e| format!("{BAKED}: {e}"))?;
    println!(
        "baked {BAKED} ({} bytes): {}×{}, {} markers, {} slices",
        baked.len(),
        sketch.size.x,
        sketch.size.y,
        sketch.markers.len(),
        sketch.slices.len()
    );
    Ok(())
}

fn diff() -> Result<(), String> {
    let args: Vec<String> = std::env::args().skip(2).collect();
    let (old_path, new_path) = match &args[..] {
        [] => (BAKED, ASEPRITE),
        [a, b] => (a.as_str(), b.as_str()),
        _ => return Err("usage: diff [<old> <new>]".into()),
    };
    let (old, new) = (read(old_path)?, read(new_path)?);
    let diff = Diff::new(&old, &new);
    println!("world sketch: {new_path} vs {old_path} (sketch px from the origin marker, y down)");
    print!("{}", diff.report());
    let (w, h, rgb) = diff.image();
    let out = path(DIFF_PNG);
    std::fs::create_dir_all(path("screenshots")).map_err(|e| e.to_string())?;
    std::fs::write(&out, png(w, h, &rgb)).map_err(|e| format!("{DIFF_PNG}: {e}"))?;
    println!("image: {DIFF_PNG} ({w}×{h})");
    Ok(())
}

/// Debug builds: warns when the sketch on disk has changes the bake doesn't have.
#[cfg(debug_assertions)]
pub fn warn_if_new(sketch: &Sketch) {
    let baked = match read(BAKED) {
        Ok(b) => b,
        Err(e) => return bevy::log::warn!("world sketch: no baked state ({e}) — run `cargo run -- bake`"),
    };
    let diff = Diff::new(&baked, sketch);
    if !diff.is_empty() {
        bevy::log::warn!(
            "world sketch: not interpreted yet ({}) — run `cargo run -- diff`",
            diff.summary()
        );
    }
}

/// A sketch seen from its origin marker.
struct View<'a> {
    s: &'a Sketch,
    origin: IVec2,
}

impl<'a> View<'a> {
    fn new(s: &'a Sketch) -> Self {
        // Same rule as `Terrain::new`.
        let marker = |kind| s.markers.iter().find(|m| m.kind == kind).map(|m| m.at);
        let origin = marker(MarkerKind::Origin)
            .or_else(|| marker(MarkerKind::Player))
            .map_or(s.size / 2, |p| p.floor().as_ivec2());
        View { s, origin }
    }

    /// The canvas, origin-relative.
    fn canvas(&self) -> IRect {
        IRect::from_corners(-self.origin, self.s.size - self.origin)
    }

    fn cell(&self, p: IVec2) -> Cell {
        let q = p + self.origin;
        if q.x < 0 || q.y < 0 || q.x >= self.s.size.x || q.y >= self.s.size.y {
            return Cell::Air;
        }
        self.s.cells[(q.y * self.s.size.x + q.x) as usize]
    }

    fn markers(&self) -> impl Iterator<Item = (MarkerKind, Vec2)> + '_ {
        self.s.markers.iter().map(|m| (m.kind, m.at - self.origin.as_vec2()))
    }

    fn slices(&self) -> impl Iterator<Item = (&'a str, IRect)> + '_ {
        let o = self.origin;
        self.s.slices.iter().map(move |s| (s.name.as_str(), IRect::from_corners(s.rect.min - o, s.rect.max - o)))
    }

    /// Names of the slices overlapping `r`.
    fn slices_at(&self, r: IRect) -> Vec<&'a str> {
        self.slices().filter(|(_, s)| !s.intersect(r).is_empty()).map(|(n, _)| n).collect()
    }
}

/// Counts per kind of change: + solid, − solid, + passable, − passable.
type Counts = [u32; 4];

fn changes(a: Cell, b: Cell) -> Counts {
    let solid = |c| c == Cell::Solid;
    let back = |c| c == Cell::Back;
    [
        (solid(b) && !solid(a)) as u32,
        (solid(a) && !solid(b)) as u32,
        (back(b) && !back(a)) as u32,
        (back(a) && !back(b)) as u32,
    ]
}

struct Cluster {
    rect: IRect,
    counts: Counts,
}

impl Cluster {
    fn total(&self) -> u32 {
        self.counts.iter().sum()
    }
}

enum MarkerChange {
    Moved(MarkerKind, Vec2, Vec2),
    Added(MarkerKind, Vec2),
    Removed(MarkerKind, Vec2),
}

struct Diff<'a> {
    old: View<'a>,
    new: View<'a>,
    /// Everything drawn in either version, padded (for the image).
    content: IRect,
    clusters: Vec<Cluster>,
    markers: Vec<MarkerChange>,
    slices: Vec<String>,
}

/// Changed cells are grouped in blocks this big; touching blocks form a cluster.
const BLOCK: i32 = 8;

impl<'a> Diff<'a> {
    fn new(old: &'a Sketch, new: &'a Sketch) -> Self {
        let (old, new) = (View::new(old), View::new(new));
        let area = old.canvas().union(new.canvas());

        let mut blocks: HashMap<IVec2, Cluster> = HashMap::new();
        let mut content: Option<IRect> = None;
        for y in area.min.y..area.max.y {
            for x in area.min.x..area.max.x {
                let p = IVec2::new(x, y);
                let (a, b) = (old.cell(p), new.cell(p));
                if a != Cell::Air || b != Cell::Air {
                    content = Some(content.map_or(IRect::from_corners(p, p + 1), |r| r.union_point(p + 1).union_point(p)));
                }
                if a == b {
                    continue;
                }
                let c = changes(a, b);
                let block = blocks
                    .entry(p.div_euclid(IVec2::splat(BLOCK)))
                    .or_insert(Cluster { rect: IRect::from_corners(p, p + 1), counts: [0; 4] });
                block.rect = block.rect.union_point(p).union_point(p + 1);
                for k in 0..4 {
                    block.counts[k] += c[k];
                }
            }
        }

        // Touching blocks (8-connected) → clusters.
        let mut clusters = Vec::new();
        let keys: Vec<IVec2> = blocks.keys().copied().collect();
        for key in keys {
            let Some(first) = blocks.remove(&key) else { continue };
            let (mut cluster, mut stack) = (first, vec![key]);
            while let Some(k) = stack.pop() {
                for dy in -1..=1 {
                    for dx in -1..=1 {
                        let n = k + IVec2::new(dx, dy);
                        if let Some(b) = blocks.remove(&n) {
                            cluster.rect = cluster.rect.union(b.rect);
                            for i in 0..4 {
                                cluster.counts[i] += b.counts[i];
                            }
                            stack.push(n);
                        }
                    }
                }
            }
            clusters.push(cluster);
        }
        clusters.sort_by_key(|c| (std::cmp::Reverse(c.total()), c.rect.min.y, c.rect.min.x));

        let mut content = content.unwrap_or(IRect::from_corners(IVec2::ZERO, IVec2::ONE));
        for (_, at) in old.markers().chain(new.markers()) {
            content = content.union_point(at.floor().as_ivec2()).union_point(at.floor().as_ivec2() + 1);
        }
        let content = content.inflate(16).intersect(area);

        let markers = diff_markers(&old, &new);
        let slices = diff_slices(&old, &new);
        Diff { old, new, content, clusters, markers, slices }
    }

    /// Canvas resizes don't count.
    fn is_empty(&self) -> bool {
        self.clusters.is_empty() && self.markers.is_empty() && self.slices.is_empty()
    }

    #[cfg(debug_assertions)]
    fn summary(&self) -> String {
        let cells: u32 = self.clusters.iter().map(Cluster::total).sum();
        format!(
            "{cells} cell changes in {} regions, {} marker and {} slice changes",
            self.clusters.len(),
            self.markers.len(),
            self.slices.len()
        )
    }

    fn report(&self) -> String {
        let mut out = String::new();
        let (a, b) = (self.old.canvas(), self.new.canvas());
        if a == b {
            out += &format!("canvas: {}×{} (unchanged)\n", a.width(), a.height());
        } else {
            let sides: Vec<String> = [
                ("left", a.min.x - b.min.x),
                ("right", b.max.x - a.max.x),
                ("top", a.min.y - b.min.y),
                ("bottom", b.max.y - a.max.y),
            ]
            .into_iter()
            .filter(|(_, d)| *d != 0)
            .map(|(side, d)| format!("{side} {d:+}"))
            .collect();
            out += &format!(
                "canvas: {}×{} → {}×{} ({}; not a change)\n",
                a.width(),
                a.height(),
                b.width(),
                b.height(),
                sides.join(", ")
            );
        }
        if self.is_empty() {
            out += "no changes: the baked state matches the sketch.\n";
            return out;
        }

        if self.clusters.is_empty() {
            out += "cells: unchanged\n";
        } else {
            out += &format!("cells: {} changed regions\n", self.clusters.len());
        }
        const SHOWN: usize = 40;
        for (i, c) in self.clusters.iter().enumerate().take(SHOWN) {
            let r = c.rect;
            let counts: Vec<String> = ["+solid", "−solid", "+passable", "−passable"]
                .iter()
                .zip(c.counts)
                .filter(|(_, n)| *n > 0)
                .map(|(label, n)| format!("{label} {n}"))
                .collect();
            let mut areas = self.new.slices_at(r);
            for n in self.old.slices_at(r) {
                if !areas.contains(&n) {
                    areas.push(n);
                }
            }
            let areas = if areas.is_empty() {
                "outside every slice".to_string()
            } else {
                areas.iter().map(|n| format!("\"{n}\"")).collect::<Vec<_>>().join(", ")
            };
            out += &format!(
                "  #{:<3} ({}, {})..({}, {}) {}×{}: {} — {areas}\n",
                i + 1,
                r.min.x,
                r.min.y,
                r.max.x,
                r.max.y,
                r.width(),
                r.height(),
                counts.join(", ")
            );
        }
        if self.clusters.len() > SHOWN {
            out += &format!("  … and {} smaller regions\n", self.clusters.len() - SHOWN);
        }

        if !self.markers.is_empty() {
            out += "markers:\n";
            for m in &self.markers {
                out += &match m {
                    MarkerChange::Moved(k, a, b) => format!(
                        "  moved {k:?} ({:.1}, {:.1}) → ({:.1}, {:.1}) by ({:+.1}, {:+.1})\n",
                        a.x,
                        a.y,
                        b.x,
                        b.y,
                        b.x - a.x,
                        b.y - a.y
                    ),
                    MarkerChange::Added(k, p) => format!("  added {k:?} at ({:.1}, {:.1})\n", p.x, p.y),
                    MarkerChange::Removed(k, p) => format!("  removed {k:?} from ({:.1}, {:.1})\n", p.x, p.y),
                };
            }
        }
        if !self.slices.is_empty() {
            out += "slices:\n";
            for s in &self.slices {
                out += &format!("  {s}\n");
            }
        }
        out += "legend (image): green +solid, red −solid, cyan +passable, purple −passable, white boxes = regions,\n";
        out += "  markers as crosses (hollow squares = where they were), blue = slice outlines.\n";
        out
    }

    /// RGB image of the content area: unchanged structure dim, changes colored.
    fn image(&self) -> (u32, u32, Vec<u8>) {
        let r = self.content;
        let (w, h) = (r.width(), r.height());
        let mut rgb = vec![0u8; (w * h * 3) as usize];
        let mut put = |p: IVec2, c: [u8; 3]| {
            let q = p - r.min;
            if q.x >= 0 && q.y >= 0 && q.x < w && q.y < h {
                let i = ((q.y * w + q.x) * 3) as usize;
                rgb[i..i + 3].copy_from_slice(&c);
            }
        };

        for y in r.min.y..r.max.y {
            for x in r.min.x..r.max.x {
                let p = IVec2::new(x, y);
                let (a, b) = (self.old.cell(p), self.new.cell(p));
                let color = match (a, b) {
                    (Cell::Air, Cell::Air) => continue,
                    _ if a == b && a == Cell::Solid => [72, 72, 72],
                    _ if a == b => [36, 36, 36],
                    (_, Cell::Solid) => [60, 230, 90],
                    (Cell::Solid, _) => [240, 60, 60],
                    (_, Cell::Back) => [60, 200, 230],
                    _ => [180, 80, 230],
                };
                put(p, color);
            }
        }

        let outline = |rect: IRect, c: [u8; 3], put: &mut dyn FnMut(IVec2, [u8; 3])| {
            for x in rect.min.x..rect.max.x {
                put(IVec2::new(x, rect.min.y), c);
                put(IVec2::new(x, rect.max.y - 1), c);
            }
            for y in rect.min.y..rect.max.y {
                put(IVec2::new(rect.min.x, y), c);
                put(IVec2::new(rect.max.x - 1, y), c);
            }
        };
        for (_, s) in self.new.slices() {
            outline(s, [40, 70, 140], &mut put);
        }
        for c in &self.clusters {
            outline(c.rect.inflate(3), [255, 255, 255], &mut put);
        }

        let tint = |k: MarkerKind| match k {
            MarkerKind::Origin => [255, 0, 255],
            MarkerKind::Player => [255, 40, 40],
            MarkerKind::Roach => [255, 230, 0],
            MarkerKind::Wasp => [0, 255, 0],
            MarkerKind::Dragonfly => [255, 120, 200],
            MarkerKind::Spider => [90, 140, 255],
        };
        for m in &self.markers {
            if let MarkerChange::Moved(k, at, _) | MarkerChange::Removed(k, at) = m {
                outline(IRect::from_center_half_size(at.floor().as_ivec2(), IVec2::splat(3)), tint(*k), &mut put);
            }
        }
        for (k, at) in self.new.markers() {
            let p = at.floor().as_ivec2();
            for d in -4..=4 {
                put(p + IVec2::new(d, 0), tint(k));
                put(p + IVec2::new(0, d), tint(k));
            }
        }
        (w as u32, h as u32, rgb)
    }
}

/// Pairs markers of the same kind, nearest first.
fn diff_markers(old: &View, new: &View) -> Vec<MarkerChange> {
    let mut out = Vec::new();
    for kind in [MarkerKind::Origin, MarkerKind::Player, MarkerKind::Roach, MarkerKind::Wasp, MarkerKind::Dragonfly, MarkerKind::Spider] {
        let mut a: Vec<Vec2> = old.markers().filter(|m| m.0 == kind).map(|m| m.1).collect();
        let mut b: Vec<Vec2> = new.markers().filter(|m| m.0 == kind).map(|m| m.1).collect();
        while !a.is_empty() && !b.is_empty() {
            let mut best = (f32::MAX, 0, 0);
            for (i, p) in a.iter().enumerate() {
                for (j, q) in b.iter().enumerate() {
                    let d = p.distance(*q);
                    if d < best.0 {
                        best = (d, i, j);
                    }
                }
            }
            let (p, q) = (a.swap_remove(best.1), b.swap_remove(best.2));
            if best.0 > 1e-3 {
                out.push(MarkerChange::Moved(kind, p, q));
            }
        }
        out.extend(a.into_iter().map(|p| MarkerChange::Removed(kind, p)));
        out.extend(b.into_iter().map(|p| MarkerChange::Added(kind, p)));
    }
    out
}

fn diff_slices(old: &View, new: &View) -> Vec<String> {
    let fmt = |r: IRect| format!("({}, {}) {}×{}", r.min.x, r.min.y, r.width(), r.height());
    let mut gone: Vec<(&str, IRect)> = old.slices().collect();
    let mut fresh = Vec::new();
    let mut out = Vec::new();
    for (name, b) in new.slices() {
        let Some(i) = gone.iter().position(|(n, _)| *n == name) else {
            fresh.push((name, b));
            continue;
        };
        let (_, a) = gone.remove(i);
        if a.size() != b.size() {
            out.push(format!("resized \"{name}\": {} → {}", fmt(a), fmt(b)));
        } else if a.min != b.min {
            let d = b.min - a.min;
            out.push(format!("moved \"{name}\" by ({:+}, {:+}) to {}", d.x, d.y, fmt(b)));
        }
    }
    for (name, b) in fresh {
        if let Some(i) = gone.iter().position(|(_, a)| *a == b) {
            let (old_name, _) = gone.remove(i);
            out.push(format!("renamed \"{old_name}\" → \"{name}\""));
        } else {
            out.push(format!("added \"{name}\" {}", fmt(b)));
        }
    }
    for (name, a) in gone {
        out.push(format!("removed \"{name}\" {}", fmt(a)));
    }
    out
}

/// Minimal PNG encoder: 8-bit RGB, no filtering.
fn png(w: u32, h: u32, rgb: &[u8]) -> Vec<u8> {
    let mut raw = Vec::with_capacity(rgb.len() + h as usize);
    for row in rgb.chunks(w as usize * 3) {
        raw.push(0);
        raw.extend_from_slice(row);
    }
    let mut ihdr = Vec::new();
    ihdr.extend(w.to_be_bytes());
    ihdr.extend(h.to_be_bytes());
    ihdr.extend([8, 2, 0, 0, 0]);

    let mut out = b"\x89PNG\r\n\x1a\n".to_vec();
    for (kind, data) in [
        (b"IHDR", ihdr),
        (b"IDAT", miniz_oxide::deflate::compress_to_vec_zlib(&raw, 6)),
        (b"IEND", Vec::new()),
    ] {
        out.extend((data.len() as u32).to_be_bytes());
        let start = out.len();
        out.extend(kind);
        out.extend(&data);
        out.extend(crc32(&out[start..]).to_be_bytes());
    }
    out
}

fn crc32(bytes: &[u8]) -> u32 {
    let mut crc = !0u32;
    for &b in bytes {
        crc ^= b as u32;
        for _ in 0..8 {
            crc = (crc >> 1) ^ (0xEDB8_8320 & (crc & 1).wrapping_neg());
        }
    }
    !crc
}

/// `mark <boss> <x,y>`: a 5×5 marker of the boss's sketch color at `x,y` (sketch
/// pixels from the origin marker), on a new top layer "marker: <boss>".
fn mark() -> Result<(), String> {
    let usage = "usage: mark <spider|dragonfly|wasp|roach> <x,y>  (sketch px from the origin, as `diff` prints them)";
    let mut args = std::env::args().skip(2);
    let boss = args.next().ok_or(usage)?;
    let (kind, rgb) = match boss.as_str() {
        "spider" => (MarkerKind::Spider, [0x7f, 0x9f, 0xd6]),
        "dragonfly" => (MarkerKind::Dragonfly, [0xd1, 0xb0, 0xc6]),
        "wasp" => (MarkerKind::Wasp, [0, 255, 0]),
        "roach" => (MarkerKind::Roach, [255, 230, 0]),
        _ => return Err(usage.into()),
    };
    let at = args.next().ok_or(usage)?;
    let (x, y) = at.split_once(',').ok_or(usage)?;
    let rel = IVec2::new(x.trim().parse().map_err(|_| usage)?, y.trim().parse().map_err(|_| usage)?);

    let file = path(ASEPRITE);
    let bytes = std::fs::read(&file).map_err(|e| format!("{ASEPRITE}: {e}"))?;
    let sketch = sketch::decode(&bytes)?;
    if let Some(old) = sketch.markers.iter().find(|m| m.kind == kind) {
        return Err(format!("the sketch already has a {boss} marker (canvas px {:.0}, {:.0}): move or erase it in Aseprite", old.at.x, old.at.y));
    }
    // The same origin the game uses (the first origin marker).
    let origin = sketch.markers.iter().find(|m| m.kind == MarkerKind::Origin).ok_or("the sketch has no origin marker")?.at.floor().as_ivec2();
    let center = origin + rel;
    if center.cmplt(IVec2::splat(2)).any() || center.cmpge(sketch.size - 2).any() {
        return Err(format!("{at} is off the canvas"));
    }

    let out = add_marker_layer(&bytes, &format!("marker: {boss}"), center - 2, 5, rgb)?;
    // It must read back with the marker where asked, and nothing else changed.
    let check = sketch::decode(&out)?;
    let got = check.markers.iter().find(|m| m.kind == kind).ok_or("the new marker doesn't read back")?;
    if (got.at - (center.as_vec2() + 0.5)).length() > 1.0 || check.size != sketch.size || check.slices.len() != sketch.slices.len() {
        return Err("the edited sketch doesn't read back as expected; nothing was written".into());
    }
    let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs());
    let backup = format!("{file}.bak-{stamp}");
    std::fs::write(&backup, &bytes).map_err(|e| format!("{backup}: {e}"))?;
    std::fs::write(&file, &out).map_err(|e| format!("{ASEPRITE}: {e}"))?;
    println!("marked {boss} at {at} from the origin (canvas px {}, {}) on a new layer \"marker: {boss}\"", center.x, center.y);
    println!("backup: {backup}");
    Ok(())
}

/// The .aseprite file with a new top-level layer `name` holding one cel: a
/// `size`² square of `rgb` at canvas `corner`. Only the first frame changes:
/// the layer chunk goes after the last layer, the cel at the end (drawn last).
fn add_marker_layer(data: &[u8], name: &str, corner: IVec2, size: u16, rgb: [u8; 3]) -> Result<Vec<u8>, String> {
    let u16_at = |p: usize| u16::from_le_bytes([data[p], data[p + 1]]);
    let u32_at = |p: usize| u32::from_le_bytes([data[p], data[p + 1], data[p + 2], data[p + 3]]);
    if data.len() < 144 || u16_at(4) != 0xA5E0 || u16_at(12) != 32 {
        return Err("not an RGBA .aseprite file".into());
    }
    let frame = 128;
    let frame_len = u32_at(frame) as usize;
    if u16_at(frame + 4) != 0xF1FA || frame + frame_len > data.len() {
        return Err("bad first frame".into());
    }
    let chunks = match u32_at(frame + 12) as usize {
        0 => u16_at(frame + 6) as usize,
        n => n,
    };
    // Find the end of the last layer chunk, counting the layers.
    let (mut pos, mut layers, mut after_layers) = (frame + 16, 0u16, frame + 16);
    for _ in 0..chunks {
        let len = u32_at(pos) as usize;
        if len < 6 || pos + len > frame + frame_len {
            return Err("bad chunk".into());
        }
        if u16_at(pos + 4) == 0x2004 {
            layers += 1;
            after_layers = pos + len;
        }
        pos += len;
    }

    let mut layer = Vec::new();
    layer.extend(3u16.to_le_bytes()); // visible | editable
    layer.extend(0u16.to_le_bytes()); // normal layer
    layer.extend(0u16.to_le_bytes()); // top level
    layer.extend([0u8; 4]); // default size (ignored)
    layer.extend(0u16.to_le_bytes()); // blend: normal
    layer.push(255); // opacity
    layer.extend([0u8; 3]);
    layer.extend((name.len() as u16).to_le_bytes());
    layer.extend(name.as_bytes());

    let pixels: Vec<u8> = (0..size as usize * size as usize).flat_map(|_| [rgb[0], rgb[1], rgb[2], 255]).collect();
    let mut cel = Vec::new();
    cel.extend(layers.to_le_bytes()); // the new layer's index
    cel.extend((corner.x as i16).to_le_bytes());
    cel.extend((corner.y as i16).to_le_bytes());
    cel.push(255); // opacity
    cel.extend(2u16.to_le_bytes()); // compressed image
    cel.extend(0i16.to_le_bytes()); // z-index
    cel.extend([0u8; 5]);
    cel.extend(size.to_le_bytes());
    cel.extend(size.to_le_bytes());
    cel.extend(miniz_oxide::deflate::compress_to_vec_zlib(&pixels, 6));

    let chunk = |kind: u16, body: &[u8]| {
        let mut c = Vec::with_capacity(body.len() + 6);
        c.extend(((body.len() + 6) as u32).to_le_bytes());
        c.extend(kind.to_le_bytes());
        c.extend(body);
        c
    };
    let frame_end = frame + frame_len;
    let mut out = Vec::with_capacity(data.len() + 256);
    out.extend(&data[..after_layers]);
    out.extend(chunk(0x2004, &layer));
    out.extend(&data[after_layers..frame_end]);
    out.extend(chunk(0x2005, &cel));
    out.extend(&data[frame_end..]);

    let added = out.len() - data.len();
    let n = chunks + 2;
    out[frame..frame + 4].copy_from_slice(&((frame_len + added) as u32).to_le_bytes());
    out[frame + 6..frame + 8].copy_from_slice(&(n.min(0xFFFF) as u16).to_le_bytes());
    out[frame + 12..frame + 16].copy_from_slice(&(n as u32).to_le_bytes());
    let total = out.len() as u32;
    out[0..4].copy_from_slice(&total.to_le_bytes());
    Ok(out)
}
