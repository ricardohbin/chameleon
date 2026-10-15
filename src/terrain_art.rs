//! Paints the terrain grid into textures: smooth edges (the sketch pixels are
//! interpolated), light from above, and a material per area style
//! (cracked concrete, a mosaic of junk, sewer brick, dead park soil).
//! Passable structures are painted as dark interiors behind the action.

use std::sync::Arc;

use bevy::{
    asset::RenderAssetUsages,
    prelude::*,
    render::render_resource::{Extent3d, TextureDimension, TextureFormat},
    image::ImageSampler,
    tasks::{futures::check_ready, AsyncComputeTaskPool, Task},
};

use crate::{
    outline,
    sketch::Cell,
    terrain::{Style, Terrain, PX},
};

// One module per newer region, each with its own material.
mod subway;
mod treatment;
mod sinkhole;
mod catacombs;
mod mall;
mod dam;
mod bunker;
mod cave;

/// Texels per sketch pixel (about 3 world units each).
const TEX: usize = 3;
/// Patterns and light are designed in units of 5 world units, whatever `PX` is.
const K: f32 = PX / 5.0;
/// Tile size in sketch pixels (two textures each: ground and interiors).
const TILE: i32 = 256;
pub const Z: f32 = -1.0;
/// The interiors' layer, just behind the solid ground's.
const Z_BACK: f32 = -0.005;

/// How far apart the solid ground (what collides) and the interiors behind
/// the action are in brightness, in stops: the ground is brightened by half of
/// it and the interiors darkened by the other half. F6 / F7 lower / raise it.
/// Each tile is painted as two layers, so this is just a tint, applied at once.
#[derive(Resource)]
pub struct Contrast(pub f32);

impl Default for Contrast {
    fn default() -> Self {
        Contrast(DEFAULT_CONTRAST)
    }
}

/// 0 = as painted.
const DEFAULT_CONTRAST: f32 = 0.5;
const CONTRAST_STEP: f32 = 0.25;
const CONTRAST_RANGE: (f32, f32) = (-4.0, 8.0);

/// Which layer of a painted tile a sprite is (its tint follows `Contrast`).
#[derive(Component, Clone, Copy)]
pub enum Layer {
    Front,
    Back,
}

impl Contrast {
    /// A layer's sprite tint (linear, above 1 brightens).
    fn tint(&self, layer: Layer) -> Color {
        let k = 2f32.powf(match layer {
            Layer::Front => self.0 / 2.0,
            Layer::Back => -self.0 / 2.0,
        });
        Color::linear_rgb(k, k, k)
    }
}

/// F6 / F7: less / more contrast between the ground and the interiors.
pub fn adjust_contrast(keys: Res<ButtonInput<KeyCode>>, mut contrast: ResMut<Contrast>) {
    let step = if keys.just_pressed(KeyCode::F6) {
        -CONTRAST_STEP
    } else if keys.just_pressed(KeyCode::F7) {
        CONTRAST_STEP
    } else {
        return;
    };
    contrast.0 = (contrast.0 + step).clamp(CONTRAST_RANGE.0, CONTRAST_RANGE.1);
    info!("ground / interior contrast {:+.2} stops", contrast.0);
}

/// Tints the tile layers: new ones, or all of them when the contrast changed.
pub fn apply_contrast(contrast: Res<Contrast>, mut sprites: Query<(&Layer, &mut Sprite)>) {
    for (&layer, mut sprite) in &mut sprites {
        if contrast.is_changed() || sprite.is_added() {
            sprite.color = contrast.tint(layer);
        }
    }
}

type Rgb = [f32; 3];

const RIM: Rgb = [0.70, 0.67, 0.62];
const SOOT: Rgb = [0.09, 0.09, 0.10];
const CONCRETE: Rgb = [0.40, 0.38, 0.36];
const JUNK: [Rgb; 6] = [
    [0.36, 0.33, 0.30],
    [0.26, 0.25, 0.25],
    [0.44, 0.30, 0.20],
    [0.52, 0.50, 0.46],
    [0.31, 0.33, 0.28],
    [0.40, 0.36, 0.30],
];
const RUBBER: Rgb = [0.13, 0.13, 0.13];
const BRICK: Rgb = [0.36, 0.30, 0.26];
const MORTAR: Rgb = [0.17, 0.16, 0.15];
const SLIME: Rgb = [0.20, 0.25, 0.15];
const SOIL: Rgb = [0.31, 0.27, 0.23];
const PAVING: Rgb = [0.47, 0.45, 0.42];

/// Marks everything spawned for the current terrain (despawned on reload).
#[derive(Component)]
pub struct TerrainPart;

/// Per-cell data the painter samples.
struct Fields {
    w: i32,
    h: i32,
    structure: Vec<f32>,
    /// Solid cells stacked above (including itself): how deep under the top surface.
    under: Vec<f32>,
    /// Distance to the nearest non-solid cell.
    depth: Vec<f32>,
    /// Distance to the nearest solid cell (for shadows on interiors).
    clear: Vec<f32>,
    style: Vec<Style>,
}

impl Fields {
    fn new(t: &Terrain) -> Fields {
        let size = t.size();
        let (w, h) = (size.x, size.y);
        let n = (w * h) as usize;
        let cells: Vec<Cell> = (0..n).map(|i| t.cell(IVec2::new(i as i32 % w, i as i32 / w))).collect();
        let solid: Vec<bool> = cells.iter().map(|&c| c == Cell::Solid).collect();
        let structure = cells.iter().map(|&c| (c != Cell::Air) as u8 as f32).collect();
        let mut under = vec![0.0; n];
        for x in 0..w as usize {
            let mut run = 0.0;
            for y in 0..h as usize {
                let i = y * w as usize + x;
                run = if solid[i] { run + 1.0 } else { 0.0 };
                under[i] = run;
            }
        }
        let depth = t.distance_field(|c| c != Cell::Solid).into_iter().map(|d| d as f32).collect();
        let clear = t.distance_field(|c| c == Cell::Solid).into_iter().map(|d| d as f32).collect();
        let style = (0..n)
            .map(|i| {
                if cells[i] == Cell::Air {
                    return Style::Concrete;
                }
                let c = IVec2::new(i as i32 % w, i as i32 / w);
                // Ragged borders between areas.
                let jitter = Vec2::new(vnoise(c.x as f32 * 0.08, c.y as f32 * 0.08, 91), vnoise(c.x as f32 * 0.08, c.y as f32 * 0.08, 92));
                t.style_at(t.cell_min(c) + (jitter - 0.5) * 30.0 * PX)
            })
            .collect();
        Fields { w, h, structure, under, depth, clear, style }
    }

    /// Bilinear sample at sketch-pixel coordinates (cell centers at +0.5).
    fn sample(&self, f: &[f32], u: f32, v: f32) -> f32 {
        let (x, y) = (u - 0.5, v - 0.5);
        let (x0, y0) = (x.floor(), y.floor());
        let (fx, fy) = (x - x0, y - y0);
        let at = |cx: i32, cy: i32| {
            if cx < 0 || cy < 0 || cx >= self.w || cy >= self.h { 0.0 } else { f[(cy * self.w + cx) as usize] }
        };
        let (x0, y0) = (x0 as i32, y0 as i32);
        let top = at(x0, y0) * (1.0 - fx) + at(x0 + 1, y0) * fx;
        let bottom = at(x0, y0 + 1) * (1.0 - fx) + at(x0 + 1, y0 + 1) * fx;
        top * (1.0 - fy) + bottom * fy
    }

    fn style(&self, u: f32, v: f32) -> Style {
        let (x, y) = ((u as i32).clamp(0, self.w - 1), (v as i32).clamp(0, self.h - 1));
        self.style[(y * self.w + x) as usize]
    }
}

/// A tile being painted on another thread (`TerrainPart`, so a reload drops it
/// and the painting is cancelled).
#[derive(Component)]
pub struct TileTask(Task<(Image, Image)>, IVec2);

/// A flat stand-in for the tile starting at grid cell `.0`, so the ground
/// around the player shows on the very first frame; it goes when the painted
/// tile is ready.
#[derive(Component)]
pub struct Placeholder(IVec2);

/// Stand-ins cover this far around the focus (world units): more than a screen.
const PLACEHOLDER_REACH: Vec2 = Vec2::new(3000.0, 2000.0);

/// The fields and outlines being prepared on another thread, then the tiles
/// to paint with them (nearest to the focus first).
#[derive(Component)]
pub struct CanvasTask(Task<(Arc<Canvas>, Vec<Vec2>)>);

/// Everything a tile painter reads, shared by all the tiles of one terrain.
pub struct Canvas {
    fields: Fields,
    solid: Vec<Vec<Vec2>>,
    structure: Vec<Vec<Vec2>>,
    /// World position of grid cell (0, 0)'s top-left corner.
    corner: Vec2,
}

/// Paints the terrain as sprite tiles, progressively and off the main thread:
/// the shared fields are prepared on the task pool, then each tile is painted
/// there (nearest to `focus` first) and shows up when it's done
/// (`finish_tiles`). Collision never waits for it: that's the `Terrain` grid.
pub fn spawn(commands: &mut Commands, images: &mut Assets<Image>, terrain: &Terrain, focus: Vec2) {
    placeholders(commands, images, terrain, focus);
    let terrain = terrain.clone();
    let task = AsyncComputeTaskPool::get().spawn(async move { prepare(&terrain, focus) });
    commands.spawn((CanvasTask(task), TerrainPart));
}

/// Neutral gray stand-ins (one texel per cell: solid gray, interiors dark) for
/// the tiles near `focus`, built right away: they're cheap.
fn placeholders(commands: &mut Commands, images: &mut Assets<Image>, terrain: &Terrain, focus: Vec2) {
    let size = terrain.size();
    let lo = terrain.to_cell(focus + Vec2::new(-PLACEHOLDER_REACH.x, PLACEHOLDER_REACH.y)).max(IVec2::ZERO) / TILE;
    let hi = terrain.to_cell(focus + Vec2::new(PLACEHOLDER_REACH.x, -PLACEHOLDER_REACH.y)).min(size - 1) / TILE;
    let extent = TILE as f32 * PX;
    for ty in lo.y..=hi.y {
        for tx in lo.x..=hi.x {
            let first = IVec2::new(tx, ty) * TILE;
            let mut data = vec![0u8; (TILE * TILE * 4) as usize];
            let mut used = false;
            for y in 0..TILE {
                for x in 0..TILE {
                    let shade = match terrain.cell(first + IVec2::new(x, y)) {
                        Cell::Air => continue,
                        Cell::Solid => 100,
                        Cell::Back => 30,
                    };
                    used = true;
                    let k = ((y * TILE + x) * 4) as usize;
                    data[k..k + 4].copy_from_slice(&[shade, shade, shade, 255]);
                }
            }
            if !used {
                continue;
            }
            let mut image = Image::new(
                Extent3d { width: TILE as u32, height: TILE as u32, depth_or_array_layers: 1 },
                TextureDimension::D2,
                data,
                TextureFormat::Rgba8UnormSrgb,
                RenderAssetUsages::RENDER_WORLD,
            );
            image.sampler = ImageSampler::nearest();
            let corner = terrain.cell_min(first) + Vec2::Y * PX;
            commands.spawn((
                Sprite { custom_size: Some(Vec2::splat(extent)), ..Sprite::from_image(images.add(image)) },
                Transform::from_translation((corner + Vec2::new(extent, -extent) / 2.0).extend(Z - 0.01)),
                Placeholder(first),
                TerrainPart,
            ));
        }
    }
}

fn prepare(terrain: &Terrain, focus: Vec2) -> (Arc<Canvas>, Vec<Vec2>) {
    let fields = Fields::new(terrain);
    let size = terrain.size();
    let solid = outline::trace(size.x, size.y, |x, y| terrain.solid(IVec2::new(x, y)));
    let structure = outline::trace(size.x, size.y, |x, y| terrain.cell(IVec2::new(x, y)) != Cell::Air);
    let corner = terrain.cell_min(IVec2::ZERO) + Vec2::Y * PX;
    let mut tiles = Vec::new();
    for ty in 0..(size.y + TILE - 1) / TILE {
        for tx in 0..(size.x + TILE - 1) / TILE {
            let (x0, y0) = (tx * TILE, ty * TILE);
            let used = (y0..(y0 + TILE).min(size.y)).any(|y| (x0..(x0 + TILE).min(size.x)).any(|x| fields.structure[(y * size.x + x) as usize] > 0.0));
            if used {
                tiles.push(Vec2::new(x0 as f32, y0 as f32));
            }
        }
    }
    let canvas = Canvas { fields, solid, structure, corner };
    tiles.sort_by(|a, b| canvas.center(*a).distance_squared(focus).total_cmp(&canvas.center(*b).distance_squared(focus)));
    (Arc::new(canvas), tiles)
}

impl Canvas {
    /// World center of the tile whose first cell is `tile`.
    fn center(&self, tile: Vec2) -> Vec2 {
        let extent = TILE as f32 * PX;
        self.corner + Vec2::new(tile.x, -tile.y) * PX + Vec2::new(extent, -extent) / 2.0
    }
}

/// Starts painting the tiles of a prepared canvas, and shows the tiles whose
/// painting is done.
pub fn finish_tiles(
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
    mut canvases: Query<(Entity, &mut CanvasTask)>,
    mut tasks: Query<(Entity, &mut TileTask)>,
    placeholders: Query<(Entity, &Placeholder)>,
) {
    for (e, mut task) in &mut canvases {
        let Some((canvas, tiles)) = check_ready(&mut task.0) else { continue };
        commands.entity(e).try_despawn();
        let pool = AsyncComputeTaskPool::get();
        for tile in tiles {
            let c = canvas.clone();
            let task = pool.spawn(async move { paint_tile(&c, tile.x as i32, tile.y as i32) });
            commands.spawn((TileTask(task, tile.as_ivec2()), Transform::from_translation(canvas.center(tile).extend(Z)), Visibility::default(), TerrainPart));
        }
    }
    let extent = TILE as f32 * PX;
    for (e, mut task) in &mut tasks {
        if let Some((front, back)) = check_ready(&mut task.0) {
            for (stand_in, _) in placeholders.iter().filter(|(_, p)| p.0 == task.1) {
                commands.entity(stand_in).try_despawn();
            }
            let back = commands
                .spawn((
                    Sprite { custom_size: Some(Vec2::splat(extent)), ..Sprite::from_image(images.add(back)) },
                    Layer::Back,
                    Transform::from_xyz(0.0, 0.0, Z_BACK),
                ))
                .id();
            commands
                .entity(e)
                .try_remove::<TileTask>()
                .try_insert((Sprite { custom_size: Some(Vec2::splat(extent)), ..Sprite::from_image(images.add(front)) }, Layer::Front))
                .add_child(back);
        }
    }
}

/// The tile's solid ground (front) and its interiors (back), as two images.
fn paint_tile(c: &Canvas, x0: i32, y0: i32) -> (Image, Image) {
    let px = TILE as usize * TEX;
    // Two extra texel rows above each tile, to find the lit top edges.
    const PAD: usize = 2;
    let origin = Vec2::new(x0 as f32, y0 as f32 - PAD as f32 / TEX as f32);
    let cover_solid = outline::rasterize(&c.solid, origin, TEX as f32, px, px + PAD);
    let cover_back = outline::rasterize(&c.structure, origin, TEX as f32, px, px + PAD);
    let mut front = vec![0u8; px * px * 4];
    let mut back = vec![0u8; px * px * 4];
    for j in 0..px {
        for i in 0..px {
            let u = x0 as f32 + (i as f32 + 0.5) / TEX as f32;
            let v = y0 as f32 + (j as f32 + 0.5) / TEX as f32;
            let at = |dj: usize| cover_solid[(j + PAD - dj) * px + i].min(1.0);
            // Open air 1 or 2 texels above: the lit rim.
            let rim = if at(1) < 0.5 { 1.0 } else if at(2) < 0.5 { 0.5 } else { 0.0 };
            let (f, b) = paint(&c.fields, u, v, at(0), cover_back[(j + PAD) * px + i].min(1.0), rim);
            let k = (j * px + i) * 4;
            for n in 0..4 {
                front[k + n] = to_u8(f[n]);
                back[k + n] = to_u8(b[n]);
            }
        }
    }
    let image = |data| {
        Image::new(
            Extent3d { width: px as u32, height: px as u32, depth_or_array_layers: 1 },
            TextureDimension::D2,
            data,
            TextureFormat::Rgba8UnormSrgb,
            RenderAssetUsages::RENDER_WORLD,
        )
    };
    (image(front), image(back))
}

fn to_u8(v: f32) -> u8 {
    (v.clamp(0.0, 1.0) * 255.0 + 0.5) as u8
}

/// A texel of the solid ground (front) and of the interiors behind it (back).
/// The back's alpha is what the front leaves of the structure's coverage, so
/// the front drawn over it gives the same picture as one layer would, and no
/// interior shows through the ground's antialiased edges.
fn paint(f: &Fields, u: f32, v: f32, cover_solid: f32, cover_back: f32, rim_k: f32) -> ([f32; 4], [f32; 4]) {
    let cover_back = cover_back.max(cover_solid);
    if cover_back <= 0.0 && cover_solid <= 0.0 {
        return ([0.0; 4], [0.0; 4]);
    }
    let style = f.style(u, v);

    let back_alpha = if cover_solid < 1.0 { ((cover_back - cover_solid) / (1.0 - cover_solid)).clamp(0.0, 1.0) } else { 0.0 };
    let back = if back_alpha > 0.0 {
        let shadow = 0.45 + 0.55 * (f.sample(&f.clear, u, v) * K / 4.0).min(1.0);
        let [r, g, b] = scale(interior(style, u * K, v * K), shadow);
        [r, g, b, back_alpha]
    } else {
        [0.0; 4]
    };
    let solid = if cover_solid > 0.0 {
        let under = f.sample(&f.under, u, v);
        let depth = f.sample(&f.depth, u, v);
        let top = (1.0 - under * K / 26.0).clamp(0.0, 1.0).powf(1.5);
        let side = (1.0 - depth * K / 6.0).clamp(0.0, 1.0) * 0.35;
        let light = 0.16 + 0.84 * top.max(side);
        mix(scale(material(style, u * K, v * K), light), rim(style, u * K), rim_k)
    } else {
        [0.0; 3]
    };
    ([solid[0], solid[1], solid[2], cover_solid], back)
}

/// The bare material of solid ground, before lighting.
fn material(style: Style, u: f32, v: f32) -> Rgb {
    let grain = 0.94 + 0.12 * hash(u as i32 * 7 + (u.fract() * 2.0) as i32, v as i32 * 5 + (v.fract() * 2.0) as i32, 3);
    let blotch = 0.85 + 0.3 * vnoise(u * 0.15, v * 0.15, 4);
    let c = match style {
        Style::Concrete => {
            // Hairline cracks: thin bands along a noise contour.
            let crack = (vnoise(u * 0.09, v * 0.09, 5) - 0.5).abs() < 0.012 || (vnoise(u * 0.05, v * 0.13, 6) - 0.5).abs() < 0.008;
            if crack {
                SOOT
            } else {
                // Old floors: faint strata lines.
                let layer = ((v + vnoise(u * 0.05, 0.0, 7) * 6.0) / 11.0).fract();
                scale(CONCRETE, blotch * if layer < 0.06 { 0.7 } else { 1.0 })
            }
        }
        Style::Trash => junk(u, v),
        Style::Sewer => brick(u, v, 1.0),
        Style::Park => {
            let pebble = hash(u as i32, v as i32, 8) > 0.93;
            scale(if pebble { PAVING } else { SOIL }, blotch * if pebble { 0.8 } else { 1.0 })
        }
        Style::Subway => subway::material(u, v),
        Style::Treatment => treatment::material(u, v),
        Style::Sinkhole => sinkhole::material(u, v),
        Style::Catacombs => catacombs::material(u, v),
        Style::Mall => mall::material(u, v),
        Style::Dam => dam::material(u, v),
        Style::Bunker => bunker::material(u, v),
        Style::Cave => cave::material(u, v),
    };
    scale(c, grain)
}

/// The overall color of an area's ground (what the chameleon blends into).
pub fn ground_color(style: Style) -> Color {
    let [r, g, b] = match style {
        Style::Concrete => CONCRETE,
        Style::Trash => JUNK[2],
        Style::Sewer => mix(BRICK, SLIME, 0.5),
        Style::Park => SOIL,
        Style::Subway => subway::GROUND,
        Style::Treatment => treatment::GROUND,
        Style::Sinkhole => sinkhole::GROUND,
        Style::Catacombs => catacombs::GROUND,
        Style::Mall => mall::GROUND,
        Style::Dam => dam::GROUND,
        Style::Bunker => bunker::GROUND,
        Style::Cave => cave::GROUND,
    };
    Color::srgb(r, g, b)
}

/// The lit top edge.
fn rim(style: Style, u: f32) -> Rgb {
    let k = 0.85 + 0.15 * hash(u as i32, 0, 9);
    match style {
        Style::Concrete | Style::Park => scale(RIM, k),
        Style::Trash => scale(RIM, 0.75 * k),
        Style::Sewer => scale(mix(RIM, SLIME, 0.4), k),
        Style::Subway => subway::rim(u),
        Style::Treatment => treatment::rim(u),
        Style::Sinkhole => sinkhole::rim(u),
        Style::Catacombs => catacombs::rim(u),
        Style::Mall => mall::rim(u),
        Style::Dam => dam::rim(u),
        Style::Bunker => bunker::rim(u),
        Style::Cave => cave::rim(u),
    }
}

/// Passable structures: interiors in the dark, behind everything that moves.
fn interior(style: Style, u: f32, v: f32) -> Rgb {
    let grime = 0.8 + 0.3 * vnoise(u * 0.2, v * 0.04, 11);
    match style {
        Style::Concrete | Style::Park => {
            // Peeling wallpaper stripes and water streaks.
            let stripe = if (u / 3.0).fract() < 0.5 { 1.0 } else { 0.9 };
            scale(CONCRETE, 0.38 * stripe * grime)
        }
        Style::Trash => scale(junk(u, v), 0.4 * grime),
        Style::Sewer => scale(brick(u, v, 0.4), grime),
        Style::Subway => subway::interior(u, v),
        Style::Treatment => treatment::interior(u, v),
        Style::Sinkhole => sinkhole::interior(u, v),
        Style::Catacombs => catacombs::interior(u, v),
        Style::Mall => mall::interior(u, v),
        Style::Dam => dam::interior(u, v),
        Style::Bunker => bunker::interior(u, v),
        Style::Cave => cave::interior(u, v),
    }
}

/// A heap of junk: a warped mosaic of planks, panels, rubber and tires.
fn junk(u: f32, v: f32) -> Rgb {
    let wu = u + (vnoise(u * 0.2, v * 0.2, 21) - 0.5) * 5.0;
    let wv = v + (vnoise(u * 0.2, v * 0.2, 22) - 0.5) * 5.0;
    let row_h = 3.6;
    let row = (wv / row_h).floor() as i32;
    let width = 3.5 + hash(row, 1, 23) * 5.0;
    let x = wu + hash(row, 2, 24) * 10.0;
    let col = (x / width).floor() as i32;
    let (fx, fy) = ((x / width).fract(), (wv / row_h).fract());
    let id = hash(col, row, 25);
    let edge = (fx.min(1.0 - fx) * width).min(fy.min(1.0 - fy) * row_h);
    if edge < 0.3 {
        return scale(SOOT, 1.5);
    }
    if id > 0.85 {
        // A tire.
        let d = Vec2::new((fx - 0.5) * width, (fy - 0.5) * row_h).length();
        let r = width.min(row_h) * 0.5;
        return if d < r * 0.45 { SOOT } else if d < r { scale(RUBBER, 1.0 + 0.4 * (d / r)) } else { scale(SOOT, 1.5) };
    }
    let c = JUNK[(id * JUNK.len() as f32) as usize % JUNK.len()];
    // Corrugation on some panels.
    let ribs = if hash(col, row, 26) > 0.6 && (wu * 1.5).fract() < 0.3 { 0.8 } else { 1.0 };
    scale(c, ribs * (0.85 + 0.3 * hash(col, row, 27)))
}

/// Sewer brick, with slime running down from the top.
fn brick(u: f32, v: f32, k: f32) -> Rgb {
    let row_h = 2.5;
    let row = (v / row_h).floor() as i32;
    let x = u / 5.0 + if row % 2 == 0 { 0.0 } else { 0.5 };
    let (fx, fy) = (x.fract(), (v / row_h).fract());
    let mortar = fx < 0.08 || fy < 0.18;
    let base = if mortar { MORTAR } else { scale(BRICK, 0.8 + 0.4 * hash(x.floor() as i32, row, 31)) };
    let streak = vnoise(u * 0.6, v * 0.02, 32);
    scale(mix(base, SLIME, (streak - 0.55).max(0.0) * 2.5), k)
}

fn scale(c: Rgb, k: f32) -> Rgb {
    [c[0] * k, c[1] * k, c[2] * k]
}

fn mix(a: Rgb, b: Rgb, t: f32) -> Rgb {
    [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t]
}

/// Deterministic hash of a lattice point, in 0..1.
pub fn hash(x: i32, y: i32, seed: u32) -> f32 {
    let mut h = (x as u32).wrapping_mul(0x8da6_b343) ^ (y as u32).wrapping_mul(0xd816_3841) ^ seed.wrapping_mul(0xcb1a_b31f);
    h ^= h >> 13;
    h = h.wrapping_mul(0x5bd1_e995);
    h ^= h >> 15;
    (h & 0xff_ffff) as f32 / 16_777_216.0
}

/// Smooth value noise in 0..1.
pub fn vnoise(x: f32, y: f32, seed: u32) -> f32 {
    let (x0, y0) = (x.floor(), y.floor());
    let (fx, fy) = (x - x0, y - y0);
    let (sx, sy) = (fx * fx * (3.0 - 2.0 * fx), fy * fy * (3.0 - 2.0 * fy));
    let (i, j) = (x0 as i32, y0 as i32);
    let top = hash(i, j, seed) * (1.0 - sx) + hash(i + 1, j, seed) * sx;
    let bottom = hash(i, j + 1, seed) * (1.0 - sx) + hash(i + 1, j + 1, seed) * sx;
    top * (1.0 - sy) + bottom * sy
}
