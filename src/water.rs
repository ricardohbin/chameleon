//! Water: the empty space inside "feature: … water / flood / sludge / pool"
//! slices of the sketch is filled with it (every non-solid cell in the slice,
//! up to the slice's top edge). The water is drawn in front of the chameleon
//! (see through, or not, depending on what it is), has a surface that ripples
//! (springs per column, kicked by splashes and by swimming), droplets and
//! bubbles. The swimming rules live in `player.rs` (it reads `Waters::sample`
//! and each `Kind`'s `Traits`).
//!
//! Two kinds, picked by keyword in the slice name:
//! * **Sludge** ("sludge", "flood", "sewage"): thick, dark, almost opaque. Slow
//!   to move through, wears the swimmer out fast, holds a fall like mud, big
//!   lazy bubbles that pop.
//! * **Clear** ("water", "pool", "lake"): light, see-through, fast ripples,
//!   small bright droplets, easy to swim in.

use bevy::{
    asset::RenderAssetUsages, mesh::Indices, prelude::*, render::render_resource::PrimitiveTopology, sprite_render::AlphaMode2d,
};

use crate::{
    camera::MainCamera,
    player::{self, Player},
    shapes::{ellipse_pts, fill, Rng},
    terrain::{Terrain, PX},
};

/// In front of the chameleon (10) and the mobs (11).
const Z: f32 = 15.0;
/// Fixed step of the surface springs.
const STEP: f32 = 1.0 / 120.0;
/// Surface displacement limit (world units).
const MAX_WAVE: f32 = 26.0;
/// Surface line segments per grid column (fine chop needs more than the springs' resolution).
const SUB: usize = 4;
/// Rows under the surface line that the surface mesh owns (the body starts below them),
/// so the fill always follows the contour: they must cover the tallest trough.
const SURF_ROWS: usize = 5;
/// Bubbles and droplets alive at once.
const MAX_BUBBLES: usize = 90;
const MAX_DROPS: usize = 260;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    Sludge,
    Clear,
}

/// What a kind of water is like to be in (and to look at).
pub struct Traits {
    /// Wading (less than ~⅓ submerged): share of the walking speed lost at that depth.
    pub wade: f32,
    /// Swimming speed (fresh), vertical stroke speed, how quickly velocity follows its target.
    pub speed: f32,
    pub stroke: f32,
    pub accel: f32,
    /// Fastest float-up (toward the resting depth) and sink speed once exhausted.
    pub rise: f32,
    pub sink: f32,
    /// How much deeper the swimmer hangs as it tires (× body half-height, squared).
    pub sag: f32,
    /// Jump out of the water from the surface / push off the bottom, × a normal jump.
    pub hop: f32,
    pub jump: f32,
    /// Seconds of swimming / of just floating until the swimmer is exhausted.
    pub active: f32,
    pub idle: f32,
    /// Share of the vertical speed kept when falling into it (it stops the fall).
    pub entry: f32,
    // --- look ---
    /// Surface springs: stiffness, damping, spread to the neighbors.
    k: f32,
    damp: f32,
    spread: f32,
    /// sRGBA at the surface and deep down, and the depth (units) that takes.
    shallow: [f32; 4],
    deep: [f32; 4],
    scale: f32,
    /// The bright line along the surface: color, thickness above/below it.
    shine: [f32; 4],
    shine_w: f32,
    /// Idle ripple amplitude multiplier.
    calm: f32,
    /// Droplet gravity, and bubbles spawned per second over the visible water.
    gravity: f32,
    bubble_rate: f32,
}

const CLEAR: Traits = Traits {
    wade: 0.35,
    speed: 190.0,
    stroke: 240.0,
    accel: 1500.0,
    rise: 170.0,
    sink: 55.0,
    sag: 3.0,
    hop: 0.8,
    jump: 0.8,
    active: 8.0,
    idle: 15.0,
    entry: 0.35,
    k: 28.0,
    damp: 1.6,
    spread: 2600.0,
    shallow: [0.50, 0.76, 0.88, 0.40],
    deep: [0.05, 0.20, 0.32, 0.80],
    scale: 520.0,
    shine: [0.93, 0.98, 1.0, 0.60],
    shine_w: 1.6,
    calm: 1.0,
    gravity: 1500.0,
    bubble_rate: 2.5,
};

const SLUDGE: Traits = Traits {
    wade: 0.7,
    speed: 120.0,
    stroke: 150.0,
    accel: 700.0,
    rise: 70.0,
    sink: 85.0,
    sag: 2.3,
    hop: 0.55,
    jump: 0.6,
    active: 4.5,
    idle: 8.0,
    entry: 0.12,
    k: 9.0,
    damp: 5.5,
    spread: 520.0,
    shallow: [0.26, 0.23, 0.09, 0.80],
    deep: [0.04, 0.035, 0.015, 0.92],
    scale: 400.0,
    shine: [0.70, 0.62, 0.32, 0.70],
    shine_w: 2.6,
    calm: 0.8,
    gravity: 1900.0,
    bubble_rate: 4.0,
};

impl Kind {
    pub fn t(self) -> &'static Traits {
        match self {
            Kind::Clear => &CLEAR,
            Kind::Sludge => &SLUDGE,
        }
    }

    /// Picked from a feature slice's name (sludge first: "flood water sludge" is sludge).
    fn from_name(name: &str) -> Option<Kind> {
        let n = name.to_lowercase();
        let has = |words: &[&str]| words.iter().any(|w| n.contains(w));
        if has(&["sludge", "flood", "sewage"]) {
            Some(Kind::Sludge)
        } else if has(&["water", "pool", "lake"]) {
            Some(Kind::Clear)
        } else {
            None
        }
    }

    /// Is this a water feature slice?
    pub fn is_water(feature_name: &str) -> bool {
        Kind::from_name(feature_name).is_some()
    }

    fn index(self) -> usize {
        self as usize
    }
}

/// One body of water.
pub struct Pool {
    pub kind: Kind,
    /// Cell-aligned box of the water (the slice, snapped to the grid).
    pub bounds: Rect,
    /// Height of the surface.
    pub level: f32,
    /// Columns whose surface is open (water on top, free space above): they ripple.
    open: Vec<bool>,
    /// Displacement and velocity of each column's surface.
    h: Vec<f32>,
    v: Vec<f32>,
}

impl Pool {
    fn new(t: &Terrain, kind: Kind, rect: Rect) -> Option<Pool> {
        let c0 = t.to_cell(Vec2::new(rect.min.x + 0.01, rect.max.y - 0.01));
        let c1 = t.to_cell(Vec2::new(rect.max.x - 0.01, rect.min.y + 0.01));
        let (cols, rows) = (c1.x - c0.x + 1, c1.y - c0.y + 1);
        if cols <= 0 || rows <= 0 {
            return None;
        }
        let left = t.cell_min(IVec2::new(c0.x, c1.y)).x;
        let bottom = t.cell_min(c1).y;
        let level = t.cell_min(c0).y + PX;
        let open = (0..cols)
            .map(|i| {
                let top = IVec2::new(c0.x + i, c0.y);
                !t.solid(top) && !t.solid(top - IVec2::Y)
            })
            .collect();
        Some(Pool {
            kind,
            bounds: Rect::new(left, bottom, left + cols as f32 * PX, level),
            level,
            open,
            h: vec![0.0; cols as usize],
            v: vec![0.0; cols as usize],
        })
    }

    /// Is there water at `p`?
    pub fn holds(&self, t: &Terrain, p: Vec2) -> bool {
        p.x >= self.bounds.min.x && p.x < self.bounds.max.x && p.y >= self.bounds.min.y && p.y < self.level && !t.solid_at(p)
    }

    fn column(&self, x: f32) -> Option<usize> {
        let i = ((x - self.bounds.min.x) / PX).floor();
        (i >= 0.0 && (i as usize) < self.h.len()).then_some(i as usize)
    }

    /// Pushes the surface around `x` (negative = down) by `amount` of velocity.
    fn kick(&mut self, x: f32, amount: f32, radius: i32) {
        let Some(i) = self.column(x) else { return };
        for j in -radius..=radius {
            let k = i as i32 + j;
            if k >= 0 && (k as usize) < self.v.len() {
                let w = 0.5 + 0.5 * (j as f32 / (radius as f32 + 1.0) * std::f32::consts::PI).cos();
                self.v[k as usize] += amount * w;
            }
        }
    }

    fn step(&mut self, dt: f32) {
        let t = self.kind.t();
        let n = self.h.len();
        for i in 0..n {
            let l = if i > 0 { self.h[i - 1] } else { self.h[i] };
            let r = if i + 1 < n { self.h[i + 1] } else { self.h[i] };
            let a = -t.k * self.h[i] - t.damp * self.v[i] + t.spread * (l + r - 2.0 * self.h[i]);
            self.v[i] += a * dt;
        }
        for i in 0..n {
            self.h[i] = (self.h[i] + self.v[i] * dt).clamp(-MAX_WAVE, MAX_WAVE);
        }
    }

    /// The idle swell on top of the springs.
    fn ambient(&self, x: f32, t: f32) -> f32 {
        let c = self.kind.t().calm;
        match self.kind {
            Kind::Clear => c * (1.5 * (x * 0.021 + t * 1.6).sin() + 0.8 * (x * 0.047 - t * 2.1).sin()),
            Kind::Sludge => c * (1.8 * (x * 0.009 + t * 0.45).sin() + 0.8 * (x * 0.022 - t * 0.3).sin()),
        }
    }

    /// Visual only: short, fast wind-blown chop and ripples riding the swell,
    /// swelling and dying with slow gusts that drift along the surface.
    fn chop(&self, x: f32, t: f32) -> f32 {
        let gust = (0.7 + 0.35 * (x * 0.006 - t * 0.7).sin() + 0.25 * (x * 0.013 + t * 1.3).sin()).clamp(0.25, 1.3);
        let w = match self.kind {
            // Small, crisp, quick.
            Kind::Clear => {
                0.55 * (x * 0.31 - t * 5.2).sin()
                    + 0.38 * (x * 0.53 + t * 6.8 + (x * 0.05 + t).sin() * 1.5).sin()
                    + 0.26 * (x * 0.97 - t * 9.5).sin()
                    + 0.14 * (x * 1.7 + t * 13.0).sin()
            }
            // Thick: slower, rounder, with the odd bubbling wobble.
            Kind::Sludge => {
                0.5 * (x * 0.17 - t * 2.1).sin()
                    + 0.32 * (x * 0.29 + t * 2.9 + (x * 0.04 - t * 0.6).sin() * 1.2).sin()
                    + 0.18 * (x * 0.61 - t * 4.2).sin()
            }
        };
        w * gust * self.kind.t().calm * 1.3
    }
}

/// How much of a box is in water, and where.
pub struct Wet {
    pub pool: usize,
    pub kind: Kind,
    pub level: f32,
    /// Share of the box's height under the surface.
    pub fraction: f32,
    /// Water between the surface and the floor under the box.
    pub depth: f32,
}

#[derive(Resource)]
pub struct Waters {
    pub pools: Vec<Pool>,
    rng: Rng,
}

impl Default for Waters {
    fn default() -> Self {
        Waters { pools: Vec::new(), rng: Rng(0x5EED_1234) }
    }
}

impl Waters {
    /// The water around the box `center ± half`, if any of it is under the surface.
    pub fn sample(&self, t: &Terrain, center: Vec2, half: Vec2) -> Option<Wet> {
        const N: usize = 8;
        let feet = center.y - half.y;
        for (i, p) in self.pools.iter().enumerate() {
            if center.x < p.bounds.min.x || center.x >= p.bounds.max.x || feet >= p.level || center.y + half.y <= p.bounds.min.y {
                continue;
            }
            let inside = (0..N)
                .filter(|&k| p.holds(t, Vec2::new(center.x, feet + (k as f32 + 0.5) / N as f32 * 2.0 * half.y)))
                .count();
            if inside == 0 {
                continue;
            }
            let floor = t.surface(Vec2::new(center.x, feet.min(p.level - 1.0).max(p.bounds.min.y)), 4000.0).unwrap_or(p.bounds.min.y);
            return Some(Wet {
                pool: i,
                kind: p.kind,
                level: p.level,
                fraction: inside as f32 / N as f32,
                depth: (p.level - floor.max(p.bounds.min.y)).max(0.0),
            });
        }
        None
    }
}

/// Something hit the water: a splash at the surface (`speed` of the impact).
#[derive(Message)]
pub struct Splash {
    pub at: Vec2,
    pub pool: usize,
    pub speed: f32,
}

/// Everything the water spawns (rebuilt with the terrain).
#[derive(Component)]
struct WaterPart;

/// The animated top of a pool.
#[derive(Component)]
struct Surface {
    pool: usize,
    mesh: Handle<Mesh>,
}

#[derive(Component)]
struct Drop {
    vel: Vec2,
    pool: usize,
    size: f32,
    age: f32,
}

#[derive(Component)]
struct Bubble {
    pool: usize,
    rise: f32,
    sway: f32,
    phase: f32,
    x: f32,
}

#[derive(Resource)]
struct Gfx {
    disc: Handle<Mesh>,
    /// Per `Kind::index` (sludge, clear).
    drop: [Handle<ColorMaterial>; 2],
    bubble: [Handle<ColorMaterial>; 2],
    body: Handle<ColorMaterial>,
}

pub struct WaterPlugin;

impl Plugin for WaterPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Waters>().add_message::<Splash>().add_systems(Startup, setup).add_systems(
            Update,
            (
                rebuild,
                (swim_wake, splashes, simulate, droplets, ambient_bubbles, bubbles, animate_surface).chain(),
            )
                .chain()
                .after(player::move_player),
        );
    }
}

fn setup(mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>, mut materials: ResMut<Assets<ColorMaterial>>) {
    let blend = |color: Color| ColorMaterial { color, alpha_mode: AlphaMode2d::Blend, ..default() };
    commands.insert_resource(Gfx {
        disc: meshes.add(fill(&ellipse_pts(1.0, 1.0, 10), |_| 1.0)),
        drop: [materials.add(blend(Color::srgba(0.16, 0.14, 0.06, 0.95))), materials.add(blend(Color::srgba(0.82, 0.93, 1.0, 0.85)))],
        bubble: [materials.add(blend(Color::srgba(0.45, 0.42, 0.2, 0.55))), materials.add(blend(Color::srgba(0.9, 0.97, 1.0, 0.40)))],
        body: materials.add(blend(Color::WHITE)),
    });
}

// --- Building ---

fn rebuild(
    mut commands: Commands,
    terrain: Res<Terrain>,
    mut waters: ResMut<Waters>,
    gfx: Res<Gfx>,
    parts: Query<Entity, With<WaterPart>>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    if !terrain.is_changed() {
        return;
    }
    for e in &parts {
        commands.entity(e).despawn();
    }
    waters.pools = terrain
        .features
        .iter()
        .filter_map(|f| Pool::new(&terrain, Kind::from_name(&f.name)?, f.rect))
        .collect();
    for (i, pool) in waters.pools.iter().enumerate() {
        commands.spawn((
            Mesh2d(meshes.add(body_mesh(&terrain, pool))),
            MeshMaterial2d(gfx.body.clone()),
            Transform::from_xyz(0.0, 0.0, Z),
            WaterPart,
        ));
        let mesh = meshes.add(surface_mesh(pool, 0.0, 0.0, 0.0));
        commands.spawn((
            Mesh2d(mesh.clone()),
            MeshMaterial2d(gfx.body.clone()),
            Transform::from_xyz(0.0, 0.0, Z + 0.01),
            Surface { pool: i, mesh },
            WaterPart,
        ));
        info!(
            "water: {:?} pool, x {:.0}..{:.0}, surface at {:.0}, bottom at {:.0}",
            pool.kind, pool.bounds.min.x, pool.bounds.max.x, pool.level, pool.bounds.min.y
        );
    }
}

/// Color of the water `depth` units under the surface (linear RGBA).
fn shade(kind: Kind, depth: f32) -> [f32; 4] {
    let t = kind.t();
    let k = 1.0 - (-depth / t.scale).exp();
    let c: Vec<f32> = (0..4).map(|i| t.shallow[i] + (t.deep[i] - t.shallow[i]) * k).collect();
    Color::srgba(c[0], c[1], c[2], c[3]).to_linear().to_f32_array()
}

/// A triangle-list mesh with per-vertex RGBA.
fn mesh_of(pos: Vec<[f32; 3]>, colors: Vec<[f32; 4]>, mut indices: Vec<u32>) -> Mesh {
    let mut pos = pos;
    let mut colors = colors;
    if pos.is_empty() {
        // An empty mesh upsets the renderer: one degenerate triangle.
        pos = vec![[0.0; 3]; 3];
        colors = vec![[0.0; 4]; 3];
        indices = vec![0, 1, 2];
    }
    let n = pos.len();
    Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default())
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, pos)
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, vec![[0.0, 0.0, 1.0]; n])
        .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, vec![[0.0, 0.0]; n])
        .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, colors)
        .with_inserted_indices(Indices::U32(indices))
}

/// Everything under the surface: one quad per run of water cells in each row,
/// darker and thicker with depth. The top row of open columns is the surface's.
fn body_mesh(t: &Terrain, pool: &Pool) -> Mesh {
    let c0 = t.to_cell(Vec2::new(pool.bounds.min.x + 0.01, pool.level - 0.01));
    let cols = pool.open.len();
    let rows = ((pool.level - pool.bounds.min.y) / PX).round() as i32;
    let (mut pos, mut colors, mut indices) = (Vec::new(), Vec::new(), Vec::new());
    for r in 0..rows {
        let (top, bottom) = (pool.level - r as f32 * PX, pool.level - (r + 1) as f32 * PX);
        let (ct, cb) = (shade(pool.kind, r as f32 * PX), shade(pool.kind, (r + 1) as f32 * PX));
        let water = |i: usize| !t.solid(IVec2::new(c0.x + i as i32, c0.y + r)) && !(r < SURF_ROWS as i32 && pool.open[i]);
        let mut i = 0;
        while i < cols {
            if !water(i) {
                i += 1;
                continue;
            }
            let start = i;
            while i < cols && water(i) {
                i += 1;
            }
            let (x0, x1) = (pool.bounds.min.x + start as f32 * PX, pool.bounds.min.x + i as f32 * PX);
            let base = pos.len() as u32;
            pos.extend([[x0, top, 0.0], [x1, top, 0.0], [x0, bottom, 0.0], [x1, bottom, 0.0]]);
            colors.extend([ct, ct, cb, cb]);
            indices.extend([base, base + 2, base + 1, base + 1, base + 2, base + 3]);
        }
    }
    mesh_of(pos, colors, indices)
}

/// The top row of the open columns between `lo` and `hi` (world x), following
/// the waves, with a bright line along it.
fn surface_mesh(pool: &Pool, time: f32, lo: f32, hi: f32) -> Mesh {
    let t = pool.kind.t();
    let (mut pos, mut colors, mut indices) = (Vec::new(), Vec::new(), Vec::new());
    let n = pool.open.len();
    let first = (((lo - pool.bounds.min.x) / PX).floor().max(0.0)) as usize;
    let last = (((hi - pool.bounds.min.x) / PX).ceil().max(0.0) as usize).min(n);
    let deep: Vec<[f32; 4]> = (0..=SURF_ROWS).map(|r| shade(pool.kind, r as f32 * PX)).collect();
    let shine = Color::srgba(t.shine[0], t.shine[1], t.shine[2], t.shine[3]).to_linear().to_f32_array();
    let clear = [shine[0], shine[1], shine[2], 0.0];
    let mut i = first;
    while i < last {
        if !pool.open[i] {
            i += 1;
            continue;
        }
        let start = i;
        while i < last && pool.open[i] {
            i += 1;
        }
        // Nodes on the column edges start..=i, each takes the mean of the springs beside it;
        // between them the line is subdivided and carries the fine wind chop.
        let edge = |k: usize| {
            let a = if k > start { pool.h[k - 1] } else { pool.h[k.min(n - 1)] };
            let b = if k < i && k < n { pool.h[k] } else { a };
            (a + b) * 0.5
        };
        let mut first_node = true;
        for s in 0..=(i - start) * SUB {
            let k = start + s / SUB;
            let f = (s % SUB) as f32 / SUB as f32;
            let x = pool.bounds.min.x + (k as f32 + f) * PX;
            let swell = if f == 0.0 { edge(k) } else { edge(k) + (edge(k + 1) - edge(k)) * f };
            let y = pool.level + swell + pool.ambient(x, time) + pool.chop(x, time);
            let base = pos.len() as u32;
            // Body (surface → a few rows down, never above the contour), then the bright
            // line (a thin band over the edge).
            let stride = (SURF_ROWS + 4) as u32;
            pos.push([x, y, 0.0]);
            for r in 1..=SURF_ROWS {
                pos.push([x, (pool.level - r as f32 * PX).min(y), 0.0]);
            }
            pos.extend([[x, y + t.shine_w * 0.5, 0.0], [x, y, 0.0], [x, y - t.shine_w, 0.0]]);
            colors.extend(deep.iter().copied());
            colors.extend([clear, shine, clear]);
            if !std::mem::replace(&mut first_node, false) {
                let p = base - stride;
                for r in 0..SURF_ROWS as u32 {
                    indices.extend([p + r, p + r + 1, base + r, base + r, p + r + 1, base + r + 1]);
                }
                let sh = SURF_ROWS as u32 + 1;
                // upper shine strip, lower shine strip
                indices.extend([p + sh, p + sh + 1, base + sh, base + sh, p + sh + 1, base + sh + 1]);
                indices.extend([p + sh + 1, p + sh + 2, base + sh + 1, base + sh + 1, p + sh + 2, base + sh + 2]);
            }
        }
    }
    mesh_of(pos, colors, indices)
}

// --- Simulation ---

fn simulate(time: Res<Time>, mut waters: ResMut<Waters>, mut left: Local<f32>) {
    *left = (*left + time.delta_secs()).min(0.1);
    while *left >= STEP {
        *left -= STEP;
        for p in &mut waters.pools {
            p.step(STEP);
        }
    }
}

fn animate_surface(
    time: Res<Time>,
    waters: Res<Waters>,
    camera: Single<&Transform, With<MainCamera>>,
    surfaces: Query<&Surface>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    let (lo, hi) = (camera.translation.x - 1500.0, camera.translation.x + 1500.0);
    for s in &surfaces {
        let Some(pool) = waters.pools.get(s.pool) else { continue };
        if let Some(mut mesh) = meshes.get_mut(&s.mesh) {
            *mesh = surface_mesh(pool, time.elapsed_secs(), lo, hi);
        }
    }
}

/// The chameleon moving through the surface stirs it, and leaving the water
/// in a hurry splashes.
fn swim_wake(
    time: Res<Time>,
    terrain: Res<Terrain>,
    player: Single<(&Player, &Transform)>,
    mut waters: ResMut<Waters>,
    mut splash: MessageWriter<Splash>,
    mut was: Local<f32>,
) {
    let dt = time.delta_secs();
    let (p, tf) = *player;
    let pos = tf.translation.truncate();
    let Some(w) = waters.sample(&terrain, pos, player::HALF) else {
        if *was > 0.4 && p.vel.y > 220.0 {
            if let Some(i) = waters.pools.iter().position(|q| q.holds(&terrain, pos - Vec2::Y * player::HALF.y * 1.5)) {
                splash.write(Splash { at: Vec2::new(pos.x, waters.pools[i].level), pool: i, speed: p.vel.y * 0.6 });
            }
        }
        *was = 0.0;
        return;
    };
    *was = w.fraction;
    if (pos.y - w.level).abs() < player::HALF.y * 1.4 {
        let stir = p.vel.length() * dt * 3.0;
        waters.pools[w.pool].kick(pos.x, -stir, 3);
    }
}

fn splashes(
    mut commands: Commands,
    mut events: MessageReader<Splash>,
    mut waters: ResMut<Waters>,
    gfx: Res<Gfx>,
    drops: Query<(), With<Drop>>,
) {
    let mut alive = drops.iter().count();
    for e in events.read() {
        let Some(kind) = waters.pools.get(e.pool).map(|p| p.kind) else { continue };
        let s = (e.speed / 900.0).clamp(0.1, 1.6);
        waters.pools[e.pool].kick(e.at.x, -(110.0 + 260.0 * s), 5);
        let (count, size, power) = match kind {
            Kind::Clear => (6.0 + 30.0 * s, (1.4, 3.4), 160.0 + 620.0 * s),
            Kind::Sludge => (3.0 + 14.0 * s, (3.0, 7.5), 90.0 + 360.0 * s),
        };
        for _ in 0..(count as usize).min(MAX_DROPS.saturating_sub(alive)) {
            let rng = &mut waters.rng;
            let angle = std::f32::consts::FRAC_PI_2 + rng.range(-0.75, 0.75);
            let v = Vec2::from_angle(angle) * power * rng.range(0.3, 1.0);
            let r = rng.range(size.0, size.1);
            let at = e.at + Vec2::new(rng.range(-14.0, 14.0), 0.0);
            commands.spawn((
                Mesh2d(gfx.disc.clone()),
                MeshMaterial2d(gfx.drop[kind.index()].clone()),
                Transform::from_translation(at.extend(Z + 0.02)).with_scale(Vec3::splat(r)),
                Drop { vel: v, pool: e.pool, size: r, age: 0.0 },
                WaterPart,
            ));
            alive += 1;
        }
    }
}

fn droplets(
    time: Res<Time>,
    mut commands: Commands,
    mut waters: ResMut<Waters>,
    mut drops: Query<(Entity, &mut Drop, &mut Transform)>,
) {
    let dt = time.delta_secs();
    for (e, mut d, mut tf) in &mut drops {
        let Some(pool) = waters.pools.get(d.pool) else {
            commands.entity(e).despawn();
            continue;
        };
        let (level, kind) = (pool.level, pool.kind);
        d.vel.y -= kind.t().gravity * dt;
        d.age += dt;
        tf.translation += (d.vel * dt).extend(0.0);
        // Stretches along its flight, shrinks as it ages.
        let stretch = 1.0 + (d.vel.length() / 1500.0).min(0.5);
        tf.scale = Vec3::new(d.size / stretch.sqrt(), d.size * stretch, 1.0) * (1.0 - (d.age / 3.0).powi(2)).max(0.1);
        tf.rotation = Quat::from_rotation_z(d.vel.to_angle() - std::f32::consts::FRAC_PI_2);
        if d.vel.y < 0.0 && tf.translation.y < level || d.age > 3.0 {
            if d.age <= 3.0 {
                let amount = -d.size * 14.0;
                waters.pools[d.pool].kick(tf.translation.x, amount, 2);
            }
            commands.entity(e).despawn();
        }
    }
}

// --- Bubbles ---

fn spawn_bubble(commands: &mut Commands, gfx: &Gfx, rng: &mut Rng, pool: usize, kind: Kind, at: Vec2, size: f32) {
    let (rise, sway) = match kind {
        Kind::Clear => (rng.range(45.0, 90.0), rng.range(3.0, 8.0)),
        Kind::Sludge => (rng.range(18.0, 38.0), rng.range(1.0, 3.5)),
    };
    commands.spawn((
        Mesh2d(gfx.disc.clone()),
        MeshMaterial2d(gfx.bubble[kind.index()].clone()),
        Transform::from_translation(at.extend(Z + 0.03)).with_scale(Vec3::splat(size)),
        Bubble { pool, rise, sway, phase: rng.range(0.0, 6.28), x: at.x },
        WaterPart,
    ));
}

/// Bubbles rising through the water in view (sludge: big and slow, they pop).
fn ambient_bubbles(
    time: Res<Time>,
    mut commands: Commands,
    terrain: Res<Terrain>,
    mut waters: ResMut<Waters>,
    gfx: Res<Gfx>,
    camera: Single<&Transform, With<MainCamera>>,
    existing: Query<(), With<Bubble>>,
) {
    let dt = time.delta_secs();
    let mut alive = existing.iter().count();
    let cam = camera.translation.truncate();
    let waters = &mut *waters;
    for (i, pool) in waters.pools.iter().enumerate() {
        let view = Rect::from_center_half_size(cam, Vec2::new(1100.0, 650.0));
        let seen = Rect { min: view.min.max(pool.bounds.min), max: view.max.min(pool.bounds.max.with_y(pool.level)) };
        if seen.is_empty() || alive >= MAX_BUBBLES {
            continue;
        }
        if waters.rng.chance(pool.kind.t().bubble_rate * dt) {
            let at = Vec2::new(waters.rng.range(seen.min.x, seen.max.x), waters.rng.range(seen.min.y, seen.max.y));
            if pool.holds(&terrain, at) {
                let size = match pool.kind {
                    Kind::Clear => waters.rng.range(1.5, 3.5),
                    Kind::Sludge => waters.rng.range(3.5, 8.5),
                };
                spawn_bubble(&mut commands, &gfx, &mut waters.rng, i, pool.kind, at, size);
                alive += 1;
            }
        }
    }
}

fn bubbles(
    time: Res<Time>,
    mut commands: Commands,
    terrain: Res<Terrain>,
    mut waters: ResMut<Waters>,
    gfx: Res<Gfx>,
    player: Single<(&Player, &Transform)>,
    mut q: Query<(Entity, &mut Bubble, &mut Transform), Without<Player>>,
    mut timer: Local<f32>,
) {
    let dt = time.delta_secs();
    for (e, mut b, mut tf) in &mut q {
        let Some(pool) = waters.pools.get(b.pool) else {
            commands.entity(e).despawn();
            continue;
        };
        let (level, kind) = (pool.level, pool.kind);
        b.phase += dt * 3.0;
        tf.translation.y += b.rise * dt;
        tf.translation.x = b.x + b.phase.sin() * b.sway;
        let up = tf.translation.truncate() + Vec2::Y * tf.scale.y;
        if up.y >= level {
            // Pops at the surface: sludge bubbles throw a little.
            if kind == Kind::Sludge {
                let (x, size) = (tf.translation.x, tf.scale.x);
                waters.pools[b.pool].kick(x, -size * 12.0, 3);
                let rng = &mut waters.rng;
                for _ in 0..(size as usize / 3).max(1) {
                    let v = Vec2::new(rng.range(-40.0, 40.0), rng.range(80.0, 200.0));
                    commands.spawn((
                        Mesh2d(gfx.disc.clone()),
                        MeshMaterial2d(gfx.drop[kind.index()].clone()),
                        Transform::from_translation(Vec3::new(x, level, Z + 0.02)).with_scale(Vec3::splat(rng.range(1.5, 3.0))),
                        Drop { vel: v, pool: b.pool, size: 2.0, age: 0.0 },
                        WaterPart,
                    ));
                }
            }
            commands.entity(e).despawn();
        } else if !pool.holds(&terrain, up) {
            commands.entity(e).despawn();
        }
    }

    // The chameleon's own: more, the more tired it is.
    let (p, ptf) = *player;
    if p.swimming && p.dead.is_none() {
        *timer -= dt * (0.8 + 2.4 * (1.0 - p.swim));
        if *timer <= 0.0 {
            *timer = 0.4;
            let head = ptf.translation.truncate() + Vec2::new(p.facing * 22.0, 8.0);
            if let Some((i, pool)) = waters.pools.iter().enumerate().find(|(_, q)| q.holds(&terrain, head)) {
                let kind = pool.kind;
                let size = match kind {
                    Kind::Clear => waters.rng.range(1.5, 3.0),
                    Kind::Sludge => waters.rng.range(3.0, 6.0),
                };
                let mut rng = waters.rng.clone();
                spawn_bubble(&mut commands, &gfx, &mut rng, i, kind, head, size);
                waters.rng = rng;
            }
        }
    }
}
