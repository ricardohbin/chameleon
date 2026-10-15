//! The level: the world sketch (`assets/world.aseprite`) turned into terrain,
//! plus the moving pieces placed on it (swinging girders, crumbling slabs,
//! sinking plates).
//!
//! Release builds embed the baked sketch (`assets/world.baked`, written by
//! `cargo run -- bake` once Claude has interpreted the changes), so they need
//! no asset files. Debug builds read the .aseprite from disk instead, rebuild
//! the level live whenever it is saved, and warn while it differs from the bake.

use bevy::prelude::*;

use crate::{
    shapes::*,
    sketch,
    terrain::{Terrain, PX},
    terrain_art::{self, TerrainPart},
    terrain_props,
    unstable::{Chain, Motion},
};

/// The world's lost colors — gone with the war, to be earned back.
#[allow(dead_code)]
pub mod lost_colors {
    use bevy::prelude::Color;

    pub const MOSS: Color = Color::srgb(0.40, 0.66, 0.30);
    pub const PINK: Color = Color::srgb(0.93, 0.48, 0.64);
    pub const TEAL: Color = Color::srgb(0.22, 0.68, 0.66);
    pub const ORANGE: Color = Color::srgb(0.96, 0.60, 0.24);
    pub const PURPLE: Color = Color::srgb(0.58, 0.44, 0.86);
    pub const YELLOW: Color = Color::srgb(0.93, 0.83, 0.32);
}

pub const CONCRETE: Color = Color::srgb(0.40, 0.38, 0.36);
const RUST: Color = Color::srgb(0.44, 0.27, 0.18);
const STEEL: Color = Color::srgb(0.40, 0.42, 0.45);
const SOOT: Color = Color::srgb(0.09, 0.09, 0.10);

const SLAB_H: f32 = 18.0;

/// The reviewed state of the sketch (see `sketch_tool`).
const SKETCH: &[u8] = include_bytes!("../assets/world.baked");
#[cfg(debug_assertions)]
const SKETCH_PATH: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/assets/world.aseprite");

/// The moving pieces, placed by hand on the sketch, in sketch pixels from the
/// origin marker (the magenta cross; y down). `at` is the middle of the
/// platform's top surface. (The sewer pit is a "feature: large gap that
/// requires dash to cross", so it gets no bridge.)
const PINS: &[Pin] = &[
    // Crumbling building blocks (checked with a reachability search at a 17 px jump): a
    // step back out of the floor, and a route up to the Venom Mother's chamber (the
    // crumbling one re-forms after 3 s; without it the route needs a full jump).
    Pin { at: (381, -16), width: 10, kind: PinKind::Slab },
    Pin { at: (431, -18), width: 12, kind: PinKind::Slab },
    Pin { at: (415, -27), width: 10, kind: PinKind::Crumble },
    Pin { at: (451, -39), width: 10, kind: PinKind::Slab },
];

struct Pin {
    at: (i32, i32),
    /// In sketch pixels.
    width: i32,
    kind: PinKind,
}

impl Pin {
    /// World position of the middle of the platform's top.
    fn place(&self) -> Vec2 {
        Vec2::new(self.at.0 as f32, -self.at.1 as f32) * PX
    }
}

/// Where the moving pieces are (for the dev tour).
pub fn pin_spots() -> Vec<(String, Vec2)> {
    PINS.iter().map(|p| (format!("{:?}", p.kind).to_lowercase(), p.place())).collect()
}

#[derive(Clone, Copy, Debug)]
enum PinKind {
    /// Concrete slab, stays put.
    Slab,
    /// Concrete slab that breaks under you.
    Crumble,
    /// Scrap plate that sinks under your weight (7 px: never where the next jump needs every pixel).
    #[allow(dead_code)] // none placed right now
    Plate,
    /// Steel beam hanging from chains, swinging.
    #[allow(dead_code)] // none placed right now
    Girder,
}

/// Sent after the sketch was reloaded and the level rebuilt (debug builds,
/// and F8 in any build).
#[derive(Message)]
pub struct WorldReloaded {
    /// A boss marker (or the origin) moved: the fight should restart.
    pub bosses_moved: bool,
}

/// F8: the game starts over. Sent in `PreUpdate` by `restart`, after the world
/// was rebuilt from a fresh read of the sketch (with `WorldReloaded { bosses_moved: true }`,
/// so bosses and mobs come back too). Systems that reset their own state on it
/// run `.after(level::restart)`, where the new `Terrain` is already in.
#[derive(Message)]
pub struct Restart;

pub struct LevelPlugin;

impl Plugin for LevelPlugin {
    fn build(&self, app: &mut App) {
        let terrain = match load() {
            Ok(t) => t,
            Err(e) => panic!("world sketch: {e}"),
        };
        app.insert_resource(terrain)
            .add_message::<WorldReloaded>()
            .init_resource::<terrain_art::Contrast>()
            .add_systems(
                Update,
                (terrain_art::finish_tiles, terrain_props::place_props, (terrain_art::adjust_contrast, terrain_art::apply_contrast).chain()),
            )
            .add_message::<Restart>()
            .add_systems(Startup, spawn_level)
            .add_systems(PreUpdate, restart.after(bevy::input::InputSystems));
        #[cfg(debug_assertions)]
        // Before the painters, so a reload's despawns land first: a tile (or
        // the canvas) finishing that same frame is then simply gone, not
        // painted over the new level or touched after it was despawned.
        app.add_systems(Update, hot_reload.before(terrain_art::finish_tiles).before(terrain_props::place_props));
    }
}

/// Debug builds prefer the sketch on disk (it may be newer than the build).
fn load() -> Result<Terrain, String> {
    #[cfg(debug_assertions)]
    if let Ok(bytes) = std::fs::read(SKETCH_PATH) {
        let sketch = sketch::decode(&bytes)?;
        crate::sketch_tool::warn_if_new(&sketch);
        return Ok(Terrain::new(sketch));
    }
    sketch::decode(SKETCH).map(Terrain::new)
}

/// Collision box the player stands on (the moving pieces; the static world is
/// the `Terrain` grid). `color` is what the chameleon blends into.
#[derive(Component)]
pub struct Platform {
    pub half: Vec2,
    pub color: Color,
    /// One-way platforms can be jumped through from below.
    pub one_way: bool,
    /// False while crumbled away.
    pub solid: bool,
    /// Safe to respawn on.
    pub stable: bool,
    /// How far it moved this frame (so the player rides along).
    pub delta: Vec2,
}

impl Platform {
    fn new(half: Vec2, color: Color, one_way: bool, stable: bool) -> Self {
        Self { half, color, one_way, solid: true, stable, delta: Vec2::ZERO }
    }
}

struct Mats {
    concrete: Handle<ColorMaterial>,
    concrete_dark: Handle<ColorMaterial>,
    rust: Handle<ColorMaterial>,
    steel: Handle<ColorMaterial>,
    steel_dark: Handle<ColorMaterial>,
    soot: Handle<ColorMaterial>,
    white: Handle<ColorMaterial>,
    rim: Handle<ColorMaterial>,
}

fn spawn_level(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    mut images: ResMut<Assets<Image>>,
    terrain: Res<Terrain>,
) {
    build(&mut commands, &mut meshes, &mut materials, &mut images, &terrain, terrain.spawn);
}

fn build(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<ColorMaterial>,
    images: &mut Assets<Image>,
    terrain: &Terrain,
    // Where the terrain art is painted first.
    focus: Vec2,
) {
    terrain_art::spawn(commands, images, terrain, focus);
    terrain_props::spawn(commands, materials, terrain);

    let mats = Mats {
        concrete: materials.add(CONCRETE),
        concrete_dark: materials.add(scale_color(CONCRETE, 0.6)),
        rust: materials.add(RUST),
        steel: materials.add(STEEL),
        steel_dark: materials.add(scale_color(STEEL, 0.55)),
        soot: materials.add(SOOT),
        white: materials.add(Color::WHITE),
        rim: materials.add(Color::srgb(0.70, 0.67, 0.62)),
    };
    let mut b = Builder { commands, meshes, mats, rng: Rng(0xC0FFEE) };
    for pin in PINS {
        let top = pin.place();
        let w = pin.width as f32 * PX;
        if terrain.solid_at(top + Vec2::Y * 2.0) {
            warn!("world sketch: the {:?} pinned at {:?} is buried in the ground", pin.kind, pin.at);
        }
        match pin.kind {
            PinKind::Slab => b.slab(top - Vec2::Y * SLAB_H / 2.0, w, None),
            PinKind::Crumble => b.slab(top - Vec2::Y * SLAB_H / 2.0, w, Some(Motion::crumble())),
            PinKind::Plate => b.plate(top - Vec2::Y * 7.0, w),
            PinKind::Girder => b.girder(top - Vec2::Y * 8.0, w),
        }
    }
}

/// The sketch file's modification time and size: a change means a save.
#[cfg(debug_assertions)]
type Stamp = (std::time::SystemTime, u64);

/// Rebuilds the level whenever the sketch is saved (debug builds only).
#[cfg(debug_assertions)]
fn hot_reload(
    mut commands: Commands,
    time: Res<Time<Real>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    mut images: ResMut<Assets<Image>>,
    parts: Query<Entity, With<TerrainPart>>,
    player: Query<&Transform, With<crate::player::Player>>,
    old: Res<Terrain>,
    mut reloaded: MessageWriter<WorldReloaded>,
    mut seen: Local<Option<Stamp>>,
    mut pending: Local<Option<Stamp>>,
    mut failed: Local<Option<Stamp>>,
    mut wait: Local<f32>,
) {
    *wait -= time.delta_secs();
    if *wait > 0.0 {
        return;
    }
    *wait = 0.5;
    // Missing for a moment while an editor swaps the file in: just look again later.
    let Ok(stamp) = std::fs::metadata(SKETCH_PATH).and_then(|m| Ok((m.modified()?, m.len()))) else { return };
    let Some(current) = *seen else {
        *seen = Some(stamp);
        return;
    };
    if stamp == current {
        *pending = None;
        return;
    }
    // A save in progress keeps changing the stamp: wait until it holds still for a poll.
    if *pending != Some(stamp) {
        *pending = Some(stamp);
        return;
    }
    let terrain = match std::fs::read(SKETCH_PATH).map_err(|e| e.to_string()).and_then(|b| sketch::decode(&b)) {
        Ok(s) => {
            crate::sketch_tool::warn_if_new(&s);
            Terrain::new(s)
        }
        Err(e) => {
            // Locked or half-written: keep it pending and retry every poll
            // (forgetting the change here is what used to lose saves).
            if failed.replace(stamp) != Some(stamp) {
                warn!("world sketch: {e} (retrying)");
            }
            return;
        }
    };
    (*seen, *pending, *failed) = (Some(stamp), None, None);
    let bosses_moved = old.roach != terrain.roach || old.wasp != terrain.wasp;
    let focus = player.single().map_or(terrain.spawn, |tf| tf.translation.truncate());
    replace_world(&mut commands, &mut meshes, &mut materials, &mut images, &parts, terrain, focus);
    reloaded.write(WorldReloaded { bosses_moved });
    info!("world sketch reloaded{}", if bosses_moved { " (bosses moved: fight restarts)" } else { "" });
}

/// F8: start over. Reads the sketch again (debug builds: from disk, so a stale
/// level is picked up even without a save), rebuilds the world and tells
/// everyone to reset (`Restart`).
pub fn restart(
    mut commands: Commands,
    keys: Res<ButtonInput<KeyCode>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    mut images: ResMut<Assets<Image>>,
    parts: Query<Entity, With<TerrainPart>>,
    mut reloaded: MessageWriter<WorldReloaded>,
    mut restarted: MessageWriter<Restart>,
    mut loads: MessageReader<crate::save::LoadGame>,
) {
    // F9 (a loaded game) starts over too; `save.rs` then puts the saved state back.
    let loaded_at = loads.read().last().map(|l| l.at);
    if !keys.just_pressed(KeyCode::F8) && loaded_at.is_none() {
        return;
    }
    match load() {
        // The chameleon starts over at the spawn, or where the save puts it.
        Ok(terrain) => {
            let focus = loaded_at.unwrap_or(terrain.spawn);
            replace_world(&mut commands, &mut meshes, &mut materials, &mut images, &parts, terrain, focus)
        }
        Err(e) => warn!("world sketch: {e} (restarting on the current one)"),
    }
    reloaded.write(WorldReloaded { bosses_moved: true });
    restarted.write(Restart);
    info!("restart");
}

/// Swaps in a new `Terrain`: the old level art and moving pieces go, the new ones are built.
fn replace_world(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<ColorMaterial>,
    images: &mut Assets<Image>,
    parts: &Query<Entity, With<TerrainPart>>,
    terrain: Terrain,
    focus: Vec2,
) {
    for e in parts {
        commands.entity(e).try_despawn();
    }
    build(commands, meshes, materials, images, &terrain, focus);
    commands.insert_resource(terrain);
}

struct Builder<'a, 'w, 's> {
    commands: &'a mut Commands<'w, 's>,
    meshes: &'a mut Assets<Mesh>,
    mats: Mats,
    rng: Rng,
}

impl Builder<'_, '_, '_> {
    fn root(&mut self, center: Vec2, platform: Platform, motion: Option<Motion>) -> Entity {
        let mut e = self.commands.spawn((platform, Transform::from_translation(center.extend(0.0)), Visibility::default(), TerrainPart));
        if let Some(motion) = motion {
            e.insert(motion);
        }
        e.id()
    }

    fn child(&mut self, mesh: Mesh, mat: &Handle<ColorMaterial>, tf: Transform, parent: Entity) {
        let mesh = self.meshes.add(mesh);
        self.commands.spawn((Mesh2d(mesh), MeshMaterial2d(mat.clone()), tf, ChildOf(parent)));
    }

    /// Broken concrete slab. With `Motion::Crumble` it falls when stepped on.
    fn slab(&mut self, center: Vec2, w: f32, motion: Option<Motion>) {
        let crumbles = motion.is_some();
        let e = self.root(center, Platform::new(Vec2::new(w / 2.0, SLAB_H / 2.0), CONCRETE, true, !crumbles), motion);
        let (hw, hh) = (w / 2.0, SLAB_H / 2.0);

        let mut outline = Vec::new();
        let mut x = -hw + self.rng.range(2.0, 8.0);
        while x < hw - 8.0 {
            outline.push(Vec2::new(x, hh + self.rng.range(-1.5, 1.0)));
            x += self.rng.range(10.0, 20.0);
        }
        outline.push(Vec2::new(hw - self.rng.range(2.0, 8.0), hh));
        let top_edge = outline.clone();
        outline.push(Vec2::new(hw + self.rng.range(-2.0, 3.0), self.rng.range(-3.0, 3.0)));
        let mut x = hw - self.rng.range(4.0, 12.0);
        while x > -hw + 8.0 {
            outline.push(Vec2::new(x, -hh - self.rng.range(-1.0, 5.0)));
            x -= self.rng.range(8.0, 18.0);
        }
        outline.push(Vec2::new(-hw + self.rng.range(-3.0, 2.0), self.rng.range(-3.0, 3.0)));
        // Reverse so the outline is counter-clockwise (only matters for consistency).
        outline.reverse();
        let mat = if crumbles { self.mats.concrete_dark.clone() } else { self.mats.concrete.clone() };
        self.child(fill(&outline, |p| 0.55 + 0.45 * ((p.y + hh) / SLAB_H).clamp(0.0, 1.0)), &mat, Transform::default(), e);

        self.rim(e, &top_edge);

        for _ in 0..if crumbles { 4 } else { 1 } {
            let start = Vec2::new(self.rng.range(-hw + 8.0, hw - 8.0), hh);
            self.crack(e, start, SLAB_H + 6.0);
        }
        for side in [-1.0, 1.0] {
            if self.rng.chance(0.7) {
                let base = Vec2::new(side * (hw - 3.0), self.rng.range(-4.0, 4.0));
                let dir = Vec2::new(side, self.rng.range(-0.8, 0.4)).normalize();
                self.rebar(e, base, dir);
            }
        }
    }

    /// Rusty scrap plate that sinks under the player's weight.
    fn plate(&mut self, center: Vec2, w: f32) {
        let motion = Motion::Sink { origin: center, offset: 0.0 };
        let h = 14.0;
        let e = self.root(center, Platform::new(Vec2::new(w / 2.0, h / 2.0), RUST, true, false), Some(motion));
        let outline = [
            Vec2::new(-w / 2.0, -h / 2.0 + 2.0),
            Vec2::new(w / 2.0 - 3.0, -h / 2.0),
            Vec2::new(w / 2.0 + 2.0, h / 2.0 - 1.0),
            Vec2::new(-w / 2.0 + 4.0, h / 2.0),
        ];
        let mat = self.mats.rust.clone();
        self.child(fill(&outline, |p| 0.6 + 0.4 * ((p.y + h / 2.0) / h)), &mat, Transform::default(), e);
        self.rim(e, &[outline[3], outline[2]]);
        let soot = self.mats.soot.clone();
        let mut x = -w / 2.0 + 6.0;
        while x < w / 2.0 - 4.0 {
            self.child(Circle::new(1.2).into(), &soot, Transform::from_xyz(x, h / 2.0 - 3.0, 0.1), e);
            x += 12.0;
        }
        for _ in 0..3 {
            let p = Vec2::new(self.rng.range(-w / 2.0 + 8.0, w / 2.0 - 8.0), self.rng.range(-3.0, 2.0));
            let dent = fill(&ellipse_pts(self.rng.range(3.0, 7.0), 2.0, 12), |_| 0.7);
            self.child(dent, &soot, Transform::from_translation(p.extend(0.05)), e);
        }
    }

    /// Steel I-beam hanging from two chains, swinging.
    fn girder(&mut self, center: Vec2, w: f32) {
        let len = 330.0;
        let anchor = center + Vec2::Y * len;
        let motion = Motion::Swing {
            anchor,
            len,
            amp: self.rng.range(0.12, 0.22),
            speed: self.rng.range(1.0, 1.5),
            phase: self.rng.range(0.0, 6.28),
        };
        let e = self.root(center, Platform::new(Vec2::new(w / 2.0, 8.0), STEEL, true, false), Some(motion));
        let (steel, dark, soot) = (self.mats.steel.clone(), self.mats.steel_dark.clone(), self.mats.soot.clone());
        // Gradient meshes carry full colors in their vertices, so they use a white material.
        let white = self.mats.white.clone();
        self.child(gradient_rect(Vec2::new(w, 10.0), STEEL, scale_color(STEEL, 0.6)), &white, Transform::default(), e);
        self.child(Rectangle::new(w, 3.0).into(), &steel, Transform::from_xyz(0.0, 6.5, 0.1), e);
        self.child(Rectangle::new(w, 3.0).into(), &dark, Transform::from_xyz(0.0, -6.5, 0.1), e);
        let mut x = -w / 2.0 + 12.0;
        while x < w / 2.0 - 8.0 {
            self.child(Circle::new(2.4).into(), &soot, Transform::from_xyz(x, 0.0, 0.05), e);
            x += 22.0;
        }
        let chain_mesh = self.meshes.add(chain_mesh());
        for side in [-1.0, 1.0] {
            let attach = Vec2::new(side * (w / 2.0 - 10.0), 8.0);
            self.commands.spawn((
                Mesh2d(chain_mesh.clone()),
                MeshMaterial2d(dark.clone()),
                Transform::from_xyz(0.0, 0.0, -1.0),
                Chain { platform: e, anchor: anchor + attach, attach },
                TerrainPart,
            ));
        }
    }

    /// Light catching the top edge, so walkable surfaces read against the dark.
    fn rim(&mut self, parent: Entity, top: &[Vec2]) {
        let path: Vec<Vec2> = top.iter().map(|p| *p - Vec2::Y * 1.0).collect();
        let rim = self.mats.rim.clone();
        self.child(stroke(&path, |_| 2.4, 1.0, 0.55, |_| 1.0), &rim, Transform::from_xyz(0.0, 0.0, 0.25), parent);
    }

    fn crack(&mut self, parent: Entity, start: Vec2, len: f32) {
        let n = 5;
        let mut pts = vec![start + Vec2::Y * 2.0];
        for i in 1..=n {
            let prev = pts[pts.len() - 1];
            pts.push(Vec2::new(prev.x + self.rng.range(-7.0, 7.0), start.y - len * i as f32 / n as f32));
        }
        let soot = self.mats.soot.clone();
        self.child(stroke(&pts, |t| 2.4 * (1.0 - t) + 0.4, 1.0, 1.0, |_| 1.0), &soot, Transform::from_xyz(0.0, 0.0, 0.2), parent);
    }

    fn rebar(&mut self, parent: Entity, base: Vec2, dir: Vec2) {
        let a = self.rng.range(8.0, 14.0);
        let b = self.rng.range(14.0, 24.0);
        let bend = dir.perp() * self.rng.range(-9.0, 9.0);
        let pts = smooth_open(&[base - dir * 6.0, base + dir * a, base + dir * b + bend], 5);
        let mat = self.mats.steel_dark.clone();
        self.child(stroke(&pts, |_| 1.8, 1.0, 0.7, |_| 1.0), &mat, Transform::from_xyz(0.0, 0.0, -0.1), parent);
    }
}

/// Unit-length vertical chain (scaled to length at runtime), with link bands.
fn chain_mesh() -> Mesh {
    let pts: Vec<Vec2> = (0..=120).map(|i| Vec2::new(0.0, i as f32 / 120.0 - 0.5)).collect();
    stroke(&pts, |_| 3.0, 1.0, 0.8, |t| if (t * 60.0) as i32 % 2 == 0 { 1.0 } else { 0.45 })
}

