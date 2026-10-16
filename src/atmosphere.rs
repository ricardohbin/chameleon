//! The apocalyptic backdrop: ruined skyline, smoke plumes over fires,
//! falling ash, rising embers, fog and lightning.
//!
//! The distant layers echo the area of the world they're seen from: ruined
//! towers behind the buildings, junk heaps behind the dump, culverts and
//! pipes behind the sewer, dead trees and a fairground wheel behind the park.

use std::f32::consts::PI;

use bevy::{asset::RenderAssetUsages, mesh::Indices, prelude::*, render::render_resource::PrimitiveTopology, sprite_render::AlphaMode2d};

use crate::{
    camera::{MainCamera, Shake},
    level::WorldReloaded,
    shapes::*,
    terrain::{Style, Terrain, PX},
};

const SKY_TOP: Color = Color::srgb(0.07, 0.07, 0.08);
const SKY_HORIZON: Color = Color::srgb(0.52, 0.36, 0.27);
const SMOKE: Color = Color::srgb(0.10, 0.09, 0.09);
/// Dark haze over the ground below the horizon.
const HAZE: Color = Color::srgb(0.16, 0.14, 0.13);
const FIRE: Color = Color::srgb(1.0, 0.62, 0.25);
const ASH_COUNT: usize = 160;
const EMBER_COUNT: usize = 50;
/// Particles wrap around this box centered on the camera.
const WRAP: Vec2 = Vec2::new(560.0, 340.0);

pub struct AtmospherePlugin;

impl Plugin for AtmospherePlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(ClearColor(HAZE))
            .init_resource::<Glare>()
            .insert_resource(Lightning { next: 3.0, since: 99.0, rng: Rng(0xF1A5) })
            .add_systems(Startup, (setup_backdrop, spawn_lightning, spawn_particles))
            .add_systems(Update, (drift_particles, animate_smoke, animate_fire, lightning, tint_sky))
            .add_systems(PostUpdate, (rebuild_backdrop, backdrop_layers, parallax).chain().before(TransformSystems::Propagate));
    }
}

/// Position = origin + camera * factor. factor 0 = part of the world,
/// 1 = glued to the screen, in between = distant layers, negative = foreground.
#[derive(Component)]
pub(crate) struct Parallax {
    origin: Vec2,
    factor: Vec2,
}

/// The far layers (behind z -50) are drawn by the backdrop camera.
fn backdrop_layers(mut commands: Commands, new: Query<(Entity, &Transform), Added<Backdrop>>, kids: Query<&Children>) {
    for (e, tf) in &new {
        if tf.translation.z >= -50.0 {
            continue;
        }
        commands.entity(e).insert(crate::camera::backdrop_layer());
        for d in kids.iter_descendants(e) {
            commands.entity(d).insert(crate::camera::backdrop_layer());
        }
    }
}

/// Everything rebuilt when the world reloads (children go with their parents).
#[derive(Component)]
struct Backdrop;

#[derive(Component)]
struct Particle {
    vel: Vec2,
    spin: f32,
    wobble: f32,
}

/// One puff in a smoke plume (each has its own material to fade).
#[derive(Component)]
struct Puff {
    source: Vec2,
    age: f32,
    life: f32,
    seed: f32,
}

#[derive(Component)]
struct Fire {
    seed: f32,
    base_alpha: f32,
}

/// The sky gradient, tinted by the region the camera is in.
#[derive(Component)]
struct Sky;

#[derive(Component)]
struct Flash;

#[derive(Component)]
struct Bolt;

/// How strongly the sky is flashing (0..1), for what is drawn on the color
/// layer and so isn't covered by the flash overlay.
#[derive(Resource, Default)]
pub struct Glare(pub f32);

#[derive(Resource)]
struct Lightning {
    next: f32,
    since: f32,
    rng: Rng,
}

fn blend(materials: &mut Assets<ColorMaterial>, color: Color) -> Handle<ColorMaterial> {
    materials.add(ColorMaterial { color, alpha_mode: AlphaMode2d::Blend, ..default() })
}

fn setup_backdrop(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    terrain: Res<Terrain>,
) {
    spawn_backdrop(&mut commands, &mut meshes, &mut materials, &terrain);
}

/// The world changed: bounds, areas and ground heights may have moved.
fn rebuild_backdrop(
    mut commands: Commands,
    mut reloaded: MessageReader<WorldReloaded>,
    old: Query<Entity, With<Backdrop>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    terrain: Res<Terrain>,
) {
    if reloaded.read().count() == 0 {
        return;
    }
    for e in &old {
        commands.entity(e).despawn();
    }
    spawn_backdrop(&mut commands, &mut meshes, &mut materials, &terrain);
}

/// How high the camera sits above the ground the player stands on.
const CAM_ABOVE: f32 = 75.0;
/// How far the layers' ground and the fog reach down (deeper than any pit).
const DEPTH: f32 = 9000.0;

/// The height of the ground per world x, smoothed: what the backdrop stands on.
/// Each column averages every floor in it (building storeys, pit bottoms), so
/// the layers sit in the middle of wherever the camera can be.
struct Horizon {
    x0: f32,
    heights: Vec<f32>,
}

impl Horizon {
    const STEP: f32 = 50.0;
    /// Gaussian smoothing radius (sigma), in world units.
    const SMOOTH: f32 = 450.0;

    fn new(terrain: &Terrain) -> Horizon {
        let b = terrain.bounds;
        let n = (b.width() / Self::STEP).ceil() as usize + 1;
        let raw: Vec<Option<f32>> = (0..n)
            .map(|i| {
                let x = b.min.x + i as f32 * Self::STEP;
                let (mut sum, mut count, mut above) = (0.0, 0, false);
                let mut y = b.max.y;
                while y > b.min.y {
                    let solid = terrain.solid_at(Vec2::new(x, y));
                    if solid && !above {
                        sum += y;
                        count += 1;
                    }
                    above = solid;
                    y -= PX;
                }
                (count > 0).then(|| sum / count as f32)
            })
            .collect();
        let radius = (Self::SMOOTH * 3.0 / Self::STEP) as isize;
        let heights = (0..n as isize)
            .map(|i| {
                let (mut sum, mut weight) = (0.0, 0.0);
                for j in (i - radius).max(0)..(i + radius + 1).min(n as isize) {
                    if let Some(h) = raw[j as usize] {
                        let d = (j - i) as f32 * Self::STEP / Self::SMOOTH;
                        let w = (-0.5 * d * d).exp();
                        sum += h * w;
                        weight += w;
                    }
                }
                if weight > 1e-3 { sum / weight } else { terrain.spawn.y }
            })
            .collect();
        Horizon { x0: b.min.x, heights }
    }

    fn at(&self, x: f32) -> f32 {
        let last = self.heights.len() - 1;
        let f = ((x - self.x0) / Self::STEP).clamp(0.0, last as f32);
        let i = (f as usize).min(last.saturating_sub(1));
        let a = self.heights[i];
        let b = self.heights.get(i + 1).copied().unwrap_or(a);
        a + (b - a) * (f - i as f32)
    }

    /// World x the camera is at when something at layer x is centered on screen.
    fn seen_from(x: f32, factor: Vec2) -> f32 {
        x / (1.0 - factor.x)
    }

    /// Layer origin for `x` such that, seen from the ground of the area below
    /// it, it shows `screen_y` above the camera.
    fn anchor(&self, x: f32, factor: Vec2, screen_y: f32) -> Vec2 {
        let ground = self.at(Self::seen_from(x, factor));
        Vec2::new(x, screen_y + (ground + CAM_ABOVE) * (1.0 - factor.y))
    }

    /// A band following the layer's ground line (`screen_y` above the camera):
    /// `top` color fading to `bottom` over `fade`, then `bottom` down to `DEPTH`.
    fn band(&self, (from, to): (f32, f32), factor: Vec2, screen_y: f32, fade: f32, top: Color, bottom: Color) -> Mesh {
        let (t, b) = (top.to_linear().to_f32_array(), bottom.to_linear().to_f32_array());
        let (mut pos, mut colors, mut indices) = (Vec::new(), Vec::new(), Vec::new());
        let n = ((to - from) / 40.0).ceil() as u32;
        for i in 0..=n {
            let x = from + i as f32 * 40.0;
            let y = self.anchor(x, factor, screen_y).y;
            for (dy, c) in [(0.0, t), (-fade, b), (-DEPTH, b)] {
                pos.push([x, y + dy, 0.0]);
                colors.push(c);
            }
            if i > 0 {
                let (p, q) = ((i - 1) * 3, i * 3);
                for k in 0..2 {
                    indices.extend([p + k, p + k + 1, q + k, q + k, p + k + 1, q + k + 1]);
                }
            }
        }
        color_mesh(pos, colors, indices)
    }
}

/// Vertical gradient through the given (y, color) stops, `width` wide.
fn gradient_column(width: f32, stops: &[(f32, Color)]) -> Mesh {
    let (mut pos, mut colors, mut indices) = (Vec::new(), Vec::new(), Vec::new());
    for (i, &(y, c)) in stops.iter().enumerate() {
        let c = c.to_linear().to_f32_array();
        pos.extend([[-width / 2.0, y, 0.0], [width / 2.0, y, 0.0]]);
        colors.extend([c, c]);
        if i > 0 {
            let p = (i as u32 - 1) * 2;
            indices.extend([p, p + 2, p + 1, p + 1, p + 2, p + 3]);
        }
    }
    color_mesh(pos, colors, indices)
}

fn color_mesh(pos: Vec<[f32; 3]>, colors: Vec<[f32; 4]>, indices: Vec<u32>) -> Mesh {
    let n = pos.len();
    Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default())
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, pos)
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, vec![[0.0, 0.0, 1.0]; n])
        .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, vec![[0.0, 0.0]; n])
        .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, colors)
        .with_inserted_indices(Indices::U32(indices))
}

fn spawn_backdrop(commands: &mut Commands, meshes: &mut Assets<Mesh>, materials: &mut Assets<ColorMaterial>, terrain: &Terrain) {
    let horizon = Horizon::new(terrain);
    // A layer moving at `factor` must cover this x range for the camera to
    // never run out of it anywhere in the world.
    let bounds = terrain.bounds;
    let span = |factor: f32| (bounds.min.x * (1.0 - factor) - 1300.0, bounds.max.x * (1.0 - factor) + 1300.0);
    // The area the camera is in when a layer piece at `x` is centered on screen.
    let seen_from = |x: f32, factor: Vec2| {
        let at = Horizon::seen_from(x, factor);
        terrain.style_at(Vec2::new(at, horizon.at(at)))
    };
    let mut wheel_done = false;
    let white = materials.add(Color::WHITE);
    let white_blend = blend(materials, Color::WHITE);
    let haze = HAZE;
    let mut rng = Rng(0xA5A5);

    // Sky, glued to the screen and tall enough for any zoom: dark smoke-choked
    // top, burning horizon, hazy dark ground below it (seen from deep pits).
    let sky_mat = materials.add(Color::WHITE);
    let sky = gradient_column(
        60000.0,
        &[
            (30000.0, SKY_TOP),
            (420.0, SKY_TOP),
            (-300.0, SKY_HORIZON),
            (-700.0, scale_color(SKY_HORIZON, 0.55)),
            (-1400.0, haze),
            (-30000.0, haze),
        ],
    );
    let id = layer(commands, meshes, sky, &sky_mat, Vec2::ZERO, -100.0, Vec2::ONE);
    commands.entity(id).insert(Sky);

    // The farthest layer, almost fixed to the sky: a hazy skyline that follows
    // the region, with wide gaps showing the sky between its pieces.
    far_pieces(commands, meshes, materials, &horizon, &mut rng, &span, &seen_from);

    // Ruined skylines, far to near, each on its own hazy ground. `screen_y`:
    // where its ground line shows, relative to the camera, standing on the ground.
    let skylines = [
        (Color::srgb(0.36, 0.28, 0.24), Vec2::new(0.9, 0.95), -95.0, -105.0, (60.0, 140.0), (120.0, 300.0), false),
        (Color::srgb(0.22, 0.18, 0.17), Vec2::new(0.75, 0.9), -90.0, -185.0, (70.0, 160.0), (160.0, 380.0), true),
        (Color::srgb(0.12, 0.11, 0.11), Vec2::new(0.5, 0.8), -70.0, -305.0, (90.0, 200.0), (220.0, 480.0), true),
    ];
    for (color, factor, z, screen_y, (wmin, wmax), (hmin, hmax), windows) in skylines {
        let mat = materials.add(color);
        let hole = materials.add(scale_color(SKY_HORIZON, 0.75));
        // In front of the pieces' feet, so a slope never shows a gap under them.
        let ground = horizon.band(span(factor.x), factor, screen_y, 260.0, scale_color(color, 0.8), haze.mix(&color, 0.4));
        layer(commands, meshes, ground, &white, Vec2::ZERO, z + 0.5, factor);
        let (mut x, extent) = span(factor.x);
        while x < extent {
            let style = seen_from(x, factor);
            // Sunk a little: the ground line slopes under wide pieces.
            let origin = horizon.anchor(x, factor, screen_y - 40.0);
            if matches!(style, Style::Trash | Style::Sewer | Style::Park) {
                let piece = Piece { mat: &mat, hole: &hole, origin, z, factor, size: (wmin, wmax, hmin + 40.0, hmax + 40.0) };
                let w = piece.spawn(commands, meshes, &mut rng, style, !windows && !wheel_done);
                wheel_done |= style == Style::Park && !windows;
                x += w + rng.range(-20.0, 80.0);
                continue;
            }
            let w = rng.range(wmin, wmax);
            let h = rng.range(hmin, hmax) + 40.0;
            let tilt = if rng.chance(0.2) { rng.range(-0.12, 0.12) } else { 0.0 };
            let id = layer(commands, meshes, fill(&ruin_outline(&mut rng, w, h), |p| 0.8 + 0.2 * (p.y / h)), &mat, origin, z, factor);
            commands.entity(id).insert(Transform::from_translation(origin.extend(z)).with_rotation(Quat::from_rotation_z(tilt)));
            if windows {
                // Blown-out windows showing the burning sky behind.
                let cols = (w / 22.0) as i32;
                let rows = (h * 0.55 / 26.0) as i32;
                for c in 0..cols {
                    for r in 0..rows {
                        if rng.chance(0.45) {
                            let p = Vec2::new(-w / 2.0 + 14.0 + c as f32 * 22.0, 60.0 + r as f32 * 26.0);
                            let win = Rectangle::new(rng.range(6.0, 10.0), rng.range(8.0, 14.0));
                            let mesh = meshes.add(win);
                            commands.spawn((Mesh2d(mesh), MeshMaterial2d(hole.clone()), Transform::from_translation(p.extend(0.01)), ChildOf(id)));
                        }
                    }
                }
            }
            // Twisted beams poking out of the broken tops.
            for _ in 0..rng.below(3) {
                let bx = rng.range(-w / 2.0 + 5.0, w / 2.0 - 5.0);
                let beam = smooth_open(&[Vec2::new(bx, h * 0.6), Vec2::new(bx + rng.range(-15.0, 15.0), h * 0.85), Vec2::new(bx + rng.range(-35.0, 35.0), h + rng.range(0.0, 30.0))], 5);
                let mesh = meshes.add(stroke(&beam, |_| 3.0, 1.0, 1.0, |_| 1.0));
                commands.spawn((Mesh2d(mesh), MeshMaterial2d(mat.clone()), Transform::from_xyz(0.0, 0.0, -0.01), ChildOf(id)));
            }
            x += w + rng.range(-20.0, 80.0);
        }
    }

    // Toppled power pylons with sagging cables, and broken cranes, standing
    // in the haze between the middle and near skylines.
    let mid = materials.add(Color::srgb(0.17, 0.15, 0.14));
    let factor = Vec2::new(0.62, 0.85);
    let (mut x, extent) = span(factor.x);
    let mut prev_top: Option<Vec2> = None;
    while x < extent {
        let h = rng.range(170.0, 240.0);
        let lean = rng.range(-0.15, 0.15);
        let base = horizon.anchor(x, factor, -260.0);
        let top = base + Vec2::new(lean * h, h);
        let pylon = [base + Vec2::new(-14.0, 0.0), base + Vec2::new(14.0, 0.0), top + Vec2::new(2.0, 0.0), top + Vec2::new(-2.0, 0.0)];
        layer(commands, meshes, fill(&pylon, |_| 1.0), &mid, Vec2::ZERO, -80.0, factor);
        let arm = [top + Vec2::new(-26.0, -4.0), top + Vec2::new(26.0, -4.0 + lean * 30.0), top + Vec2::new(26.0, -1.0 + lean * 30.0), top + Vec2::new(-26.0, -1.0)];
        layer(commands, meshes, fill(&arm, |_| 1.0), &mid, Vec2::ZERO, -80.0, factor);
        if let Some(prev) = prev_top {
            for dy in [0.0, -4.0] {
                let a = prev + Vec2::new(20.0, dy - 3.0);
                let b = top + Vec2::new(-20.0, dy - 3.0);
                let sag = smooth_open(&[a, (a + b) / 2.0 - Vec2::Y * rng.range(30.0, 70.0), b], 10);
                layer(commands, meshes, stroke(&sag, |_| 1.2, 1.0, 1.0, |_| 1.0), &mid, Vec2::ZERO, -80.0, factor);
            }
        }
        // Some cables snapped and dangle.
        prev_top = if rng.chance(0.7) { Some(top) } else { None };
        if rng.chance(0.25) {
            let mast = horizon.anchor(x + 120.0, factor, -260.0);
            let mh = rng.range(260.0, 340.0);
            let tip = mast + Vec2::Y * mh;
            layer(commands, meshes, Rectangle::new(8.0, mh).into(), &mid, mast + Vec2::Y * mh / 2.0, -81.0, factor);
            let jib_end = tip + Vec2::new(rng.range(120.0, 180.0), rng.range(-60.0, -10.0));
            layer(commands, meshes, stroke(&[tip, jib_end], |_| 6.0, 1.0, 1.0, |_| 1.0), &mid, Vec2::ZERO, -81.0, factor);
            let hook = smooth_open(&[jib_end, jib_end + Vec2::new(4.0, -60.0), jib_end + Vec2::new(-2.0, -110.0)], 6);
            layer(commands, meshes, stroke(&hook, |_| 1.5, 1.0, 1.0, |_| 1.0), &mid, Vec2::ZERO, -81.0, factor);
        }
        x += rng.range(260.0, 380.0);
    }

    // Fog banks over the middle and near grounds, thick all the way down.
    let fog_top = Color::srgba(0.30, 0.25, 0.22, 0.0);
    for (screen_y, h, alpha, z, factor) in [(-175.0, 260.0, 0.55, -85.0, Vec2::new(0.75, 0.9)), (-305.0, 300.0, 0.6, -60.0, Vec2::new(0.5, 0.8))] {
        let fog = horizon.band(span(factor.x), factor, screen_y + h / 2.0, h, fog_top, haze.with_alpha(alpha));
        layer(commands, meshes, fog, &white_blend, Vec2::ZERO, z, factor);
    }
    // And at the bottom of the screen, over the world (down past the bottom
    // edge when zoomed out).
    let thick = haze.with_alpha(0.75);
    let fog = gradient_column(60000.0, &[(-60.0, fog_top), (-320.0, thick), (-800.0, thick), (-1200.0, fog_top)]);
    layer(commands, meshes, fog, &white_blend, Vec2::ZERO, 35.0, Vec2::ONE);

    // Foreground wreckage silhouettes along the bottom of the screen, moving
    // faster than the world.
    let fg = materials.add(Color::srgb(0.03, 0.03, 0.035));
    let factor = Vec2::new(-0.3, 1.0);
    let (mut x, extent) = span(factor.x);
    while x < extent {
        let w = rng.range(120.0, 260.0);
        let h = rng.range(40.0, 90.0);
        let base = Vec2::new(x, -300.0);
        for mesh in foreground_pieces(&mut rng, seen_from(x, factor), w, h) {
            layer(commands, meshes, mesh, &fg, base, 45.0, factor);
        }
        x += rng.range(500.0, 900.0);
    }

    // Smoke plumes rising from fires on the near ground.
    let factor = Vec2::new(0.5, 0.8);
    let (mut x, extent) = span(factor.x);
    while x < extent {
        let source = horizon.anchor(x, factor, -215.0 + rng.range(-30.0, 40.0));
        for (r, a) in [(26.0, 0.25), (12.0, 0.7)] {
            let mat = blend(materials, FIRE.with_alpha(a));
            let id = layer(commands, meshes, fill(&ellipse_pts(r * 1.6, r, 20), |_| 1.0), &mat, source, -55.0, factor);
            commands.entity(id).insert(Fire { seed: rng.range(0.0, 100.0), base_alpha: a });
        }
        let life = rng.range(6.0, 9.0);
        for i in 0..14 {
            let mat = blend(materials, SMOKE.with_alpha(0.0));
            let puff = fill(&smooth_closed(&rock_pts(&mut rng, 1.0, 7), 5), |p| 0.8 + 0.4 * p.y);
            let id = layer(commands, meshes, puff, &mat, source, -54.0 + i as f32 * 0.01, factor);
            commands.entity(id).insert(Puff { source, age: life * i as f32 / 14.0, life, seed: rng.range(0.0, 100.0) });
        }
        x += rng.range(380.0, 700.0);
    }
}

/// Lightning: a full-screen flash and a bolt in the sky (kept across reloads).
fn spawn_lightning(mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>, mut materials: ResMut<Assets<ColorMaterial>>) {
    let flash_mat = blend(&mut materials, Color::srgba(0.9, 0.9, 1.0, 0.0));
    let flash = (Mesh2d(meshes.add(Rectangle::new(60000.0, 60000.0))), MeshMaterial2d(flash_mat), Transform::from_xyz(0.0, 0.0, 80.0));
    commands.spawn((flash, Parallax { origin: Vec2::ZERO, factor: Vec2::ONE }, Flash));
    let bolt_mat = materials.add(Color::srgb(0.95, 0.95, 1.0));
    let bolt = (Mesh2d(meshes.add(Rectangle::new(1.0, 1.0))), MeshMaterial2d(bolt_mat), Transform::from_xyz(0.0, 0.0, -96.0));
    commands.spawn((bolt, Parallax { origin: Vec2::ZERO, factor: Vec2::ONE }, Bolt, Visibility::Hidden, crate::camera::backdrop_layer()));
}

fn layer(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    mesh: Mesh,
    mat: &Handle<ColorMaterial>,
    origin: Vec2,
    z: f32,
    factor: Vec2,
) -> Entity {
    commands
        .spawn((
            Mesh2d(meshes.add(mesh)),
            MeshMaterial2d(mat.clone()),
            Transform::from_translation(origin.extend(z)),
            Parallax { origin, factor },
            Backdrop,
        ))
        .id()
}

/// A distant silhouette that isn't a ruined tower.
struct Piece<'a> {
    mat: &'a Handle<ColorMaterial>,
    /// For openings showing the sky behind.
    hole: &'a Handle<ColorMaterial>,
    origin: Vec2,
    z: f32,
    factor: Vec2,
    /// The layer's (min width, max width, min height, max height).
    size: (f32, f32, f32, f32),
}

impl Piece<'_> {
    /// Spawns one piece in the style of the area; returns its width.
    fn spawn(&self, commands: &mut Commands, meshes: &mut Assets<Mesh>, rng: &mut Rng, style: Style, landmark: bool) -> f32 {
        let (wmin, wmax, hmin, hmax) = self.size;
        let root = |commands: &mut Commands, meshes: &mut Assets<Mesh>, mesh: Mesh| layer(commands, meshes, mesh, self.mat, self.origin, self.z, self.factor);
        let child = |commands: &mut Commands, meshes: &mut Assets<Mesh>, parent: Entity, mesh: Mesh, mat: &Handle<ColorMaterial>, z: f32| {
            commands.spawn((Mesh2d(meshes.add(mesh)), MeshMaterial2d(mat.clone()), Transform::from_xyz(0.0, 0.0, z), ChildOf(parent)));
        };
        match style {
            Style::Trash => {
                // A heap of junk with things sticking out of it.
                let w = rng.range(wmin * 1.4, wmax * 2.0);
                let h = rng.range(hmin * 0.35, hmax * 0.5);
                let mut pts = vec![Vec2::new(-w / 2.0, 0.0)];
                for i in 1..24 {
                    let s = i as f32 / 24.0 * 2.0 - 1.0;
                    pts.push(Vec2::new(s * w / 2.0, h * (1.0 - s * s).powf(0.7) + rng.range(-6.0, 6.0)));
                }
                pts.push(Vec2::new(w / 2.0, 0.0));
                let id = root(commands, meshes, fill(&pts, |p| 0.85 + 0.15 * (p.y / h)));
                for _ in 0..rng.below(4) + 2 {
                    let s = rng.range(-0.6, 0.6);
                    let base = Vec2::new(s * w / 2.0, h * (1.0 - s * s).powf(0.7) - 6.0);
                    match rng.below(3) {
                        0 => {
                            // A wheel.
                            let r = rng.range(8.0, 16.0);
                            let ring: Vec<Vec2> = ellipse_pts(r, r, 20).iter().map(|p| *p + base + Vec2::Y * r * 0.6).collect();
                            let mut ring = ring.clone();
                            ring.push(ring[0]);
                            child(commands, meshes, id, stroke(&ring, |_| 3.0, 1.0, 1.0, |_| 1.0), self.mat, 0.01);
                        }
                        1 => {
                            // A bent bar or bed frame.
                            let tip = base + Vec2::new(rng.range(-30.0, 30.0), rng.range(25.0, 50.0));
                            let bar = smooth_open(&[base, (base + tip) / 2.0 + Vec2::X * rng.range(-10.0, 10.0), tip], 5);
                            child(commands, meshes, id, stroke(&bar, |_| 2.5, 1.0, 1.0, |_| 1.0), self.mat, 0.01);
                        }
                        _ => {
                            // A fridge or a car door, tipped over.
                            let (bw, bh) = (rng.range(12.0, 22.0), rng.range(20.0, 34.0));
                            let a = rng.range(-0.6, 0.6);
                            let corners = [Vec2::new(-bw, 0.0), Vec2::new(bw, 0.0), Vec2::new(bw, bh), Vec2::new(-bw, bh)]
                                .map(|c| Vec2::from_angle(a).rotate(c / Vec2::new(2.0, 1.0)) + base);
                            child(commands, meshes, id, fill(&corners, |_| 1.0), self.mat, 0.01);
                        }
                    }
                }
                w
            }
            Style::Sewer => {
                // A retaining wall with culverts, and pipes spilling out of it.
                let w = rng.range(wmin * 1.5, wmax * 1.8);
                let h = rng.range(hmin * 0.5, hmax * 0.6);
                let mut wall = ruin_outline(rng, w, h);
                for p in &mut wall {
                    p.y = p.y.min(h * 0.9);
                }
                let id = root(commands, meshes, fill(&wall, |p| 0.8 + 0.2 * (p.y / h)));
                let r = h * rng.range(0.22, 0.32);
                let cx = rng.range(-w / 4.0, w / 4.0);
                child(commands, meshes, id, fill(&ellipse_pts(r, r, 24).iter().map(|p| *p + Vec2::new(cx, r * 1.1)).collect::<Vec<_>>(), |_| 1.0), self.hole, 0.01);
                for _ in 0..rng.below(3) + 1 {
                    let y = rng.range(h * 0.3, h * 0.7);
                    let side = if rng.chance(0.5) { -1.0 } else { 1.0 };
                    let start = Vec2::new(side * w / 2.0 - side * 10.0, y);
                    let end = start + Vec2::new(side * rng.range(30.0, 60.0), rng.range(-25.0, 5.0));
                    let pipe = smooth_open(&[start, (start + end) / 2.0 + Vec2::Y * 4.0, end], 4);
                    child(commands, meshes, id, stroke(&pipe, |_| rng_width(r), 1.0, 1.0, |_| 1.0), self.mat, 0.01);
                }
                w
            }
            Style::Park if landmark => {
                // The fairground wheel, tilted and missing gondolas.
                let r = hmax * 0.55;
                let hub = Vec2::new(0.0, r + 20.0);
                let id = root(commands, meshes, fill(&[Vec2::new(-r * 0.6, 0.0), Vec2::new(-r * 0.5, 0.0), hub, hub + Vec2::new(-4.0, 6.0)], |_| 1.0));
                child(commands, meshes, id, fill(&[Vec2::new(r * 0.5, 0.0), Vec2::new(r * 0.6, 0.0), hub + Vec2::new(4.0, 6.0), hub], |_| 1.0), self.mat, 0.0);
                let mut rim: Vec<Vec2> = ellipse_pts(r, r * 0.96, 48).iter().map(|p| *p + hub).collect();
                rim.truncate(41); // a broken section
                child(commands, meshes, id, stroke(&rim, |_| 4.0, 1.0, 1.0, |_| 1.0), self.mat, 0.0);
                for i in 0..16 {
                    let a = i as f32 / 16.0 * std::f32::consts::TAU + 0.1;
                    let end = hub + Vec2::from_angle(a) * r;
                    child(commands, meshes, id, stroke(&[hub, end], |_| 1.4, 1.0, 1.0, |_| 1.0), self.mat, 0.0);
                    if i % 2 == 0 && rng.chance(0.6) {
                        let car = [Vec2::new(-7.0, -14.0), Vec2::new(7.0, -14.0), Vec2::new(5.0, 0.0), Vec2::new(-5.0, 0.0)].map(|c| c + end);
                        child(commands, meshes, id, fill(&car, |_| 1.0), self.mat, 0.0);
                    }
                }
                r * 2.4
            }
            Style::Park => {
                // A dead tree.
                let h = rng.range(hmin * 0.5, hmax * 0.7);
                let lean = rng.range(-0.2, 0.2);
                let trunk = smooth_open(&[Vec2::ZERO, Vec2::new(lean * h * 0.3, h * 0.5), Vec2::new(lean * h * 0.5, h * 0.75)], 5);
                let top = trunk[trunk.len() - 1];
                let id = root(commands, meshes, stroke(&trunk, |t| 9.0 - 5.0 * t, 1.0, 1.0, |_| 1.0));
                let mut branches = vec![(top, Vec2::Y.rotate(Vec2::from_angle(lean)), h * 0.35, 4.0)];
                while let Some((from, dir, len, width)) = branches.pop() {
                    for side in [-1.0, 1.0] {
                        let d = Vec2::from_angle(side * rng.range(0.3, 0.8)).rotate(dir);
                        let to = from + d * len * rng.range(0.6, 1.0);
                        let twig = smooth_open(&[from, (from + to) / 2.0 + d.perp() * rng.range(-4.0, 4.0), to], 3);
                        child(commands, meshes, id, stroke(&twig, |t| width * (1.0 - 0.6 * t), 1.0, 1.0, |_| 1.0), self.mat, 0.0);
                        if width > 1.5 {
                            branches.push((to, d, len * 0.6, width * 0.6));
                        }
                    }
                }
                h * 0.6 + rng.range(20.0, 80.0)
            }
            _ => 0.0,
        }
    }
}

fn rng_width(r: f32) -> f32 {
    (r * 0.45).clamp(5.0, 14.0)
}

/// A building silhouette with a broken, jagged top. Base at y = 0.
/// The near, blurry silhouettes at the bottom of the screen, themed to the
/// area the camera is in: ruined floors, junk heaps, pipes, dead shrubs,
/// bones and columns, stalagmites, sandbags.
fn foreground_pieces(rng: &mut Rng, style: Style, w: f32, h: f32) -> Vec<Mesh> {
    // A silhouette standing on the ground line, with a foot reaching below the screen.
    let footed = |mut top: Vec<Vec2>| {
        let last = top.len() - 1;
        top[0].y = -500.0;
        top[last].y = -500.0;
        fill(&top, |_| 1.0)
    };
    // Lumpy ground: random heights along the top.
    let heap = |rng: &mut Rng, w: f32, h: f32, lump: f32| {
        let mut pts = vec![Vec2::new(-w / 2.0, 0.0)];
        let mut x = -w / 2.0;
        while x < w / 2.0 {
            let edge = 1.0 - (x / (w / 2.0)).abs().powi(2);
            pts.push(Vec2::new(x, h * (0.25 + 0.75 * edge) * rng.range(1.0 - lump, 1.0)));
            x += rng.range(10.0, 26.0);
        }
        pts.push(Vec2::new(w / 2.0, 0.0));
        pts
    };
    let mut out = Vec::new();
    match style {
        Style::Trash | Style::Sinkhole => {
            out.push(footed(heap(rng, w, h, 0.6)));
            if style == Style::Trash {
                // A tire in the pile.
                let (r, x) = (rng.range(22.0, 38.0), rng.range(-w / 4.0, w / 4.0));
                let ring: Vec<Vec2> = (0..=24).map(|i| Vec2::new(x, h * 0.7) + Vec2::from_angle(i as f32 / 24.0 * std::f32::consts::TAU) * r).collect();
                out.push(stroke(&ring, |_| r * 0.35, 1.0, 1.0, |_| 1.0));
            } else {
                // Roots coming through the rubble.
                for _ in 0..3 {
                    let x = rng.range(-w / 3.0, w / 3.0);
                    let root = smooth_open(&[Vec2::new(x, h * 0.6), Vec2::new(x + rng.range(-30.0, 30.0), h + 40.0), Vec2::new(x + rng.range(-80.0, 80.0), h + 90.0)], 6);
                    out.push(stroke(&root, |t| 9.0 * (1.0 - t) + 1.5, 1.0, 1.0, |_| 1.0));
                }
            }
        }
        Style::Sewer | Style::Treatment | Style::Subway => {
            // A run of pipe along the ground with flanges, on a block.
            let (y, r) = (rng.range(30.0, 60.0), rng.range(18.0, 30.0));
            out.push(footed(vec![Vec2::new(-w / 2.0, 0.0), Vec2::new(-w / 2.0, y - r), Vec2::new(w / 2.0, y - r), Vec2::new(w / 2.0, 0.0)]));
            let body = [Vec2::new(-w / 2.0, y - r), Vec2::new(w / 2.0, y - r), Vec2::new(w / 2.0, y + r), Vec2::new(-w / 2.0, y + r)];
            out.push(fill(&body, |_| 1.0));
            for k in [0.25, 0.75] {
                let x = -w / 2.0 + w * k;
                out.push(fill(&[Vec2::new(x - 7.0, y - r - 7.0), Vec2::new(x + 7.0, y - r - 7.0), Vec2::new(x + 7.0, y + r + 7.0), Vec2::new(x - 7.0, y + r + 7.0)], |_| 1.0));
            }
            if rng.chance(0.6) {
                let x = rng.range(-w / 4.0, w / 4.0);
                out.push(fill(&[Vec2::new(x - r * 0.6, y), Vec2::new(x + r * 0.6, y), Vec2::new(x + r * 0.6, y + h + 60.0), Vec2::new(x - r * 0.6, y + h + 60.0)], |_| 1.0));
            }
        }
        Style::Park => {
            out.push(footed(heap(rng, w * 0.8, h * 0.4, 0.3)));
            // Dead shrub: bent twigs.
            for _ in 0..4 + rng.below(3) {
                let x = rng.range(-w / 3.0, w / 3.0);
                let (dx, top) = (rng.range(-70.0, 70.0), rng.range(80.0, 170.0));
                let twig = smooth_open(&[Vec2::new(x, h * 0.2), Vec2::new(x + dx * 0.3, top * 0.55), Vec2::new(x + dx, top)], 6);
                out.push(stroke(&twig, |t| 6.0 * (1.0 - t) + 1.0, 1.0, 1.0, |_| 1.0));
            }
        }
        Style::Catacombs => {
            // A pile of rubble and a broken column.
            out.push(footed(heap(rng, w, h * 0.7, 0.5)));
            let (x, cw, ch) = (rng.range(-w / 4.0, w / 4.0), rng.range(26.0, 40.0), rng.range(100.0, 190.0));
            let shaft = [Vec2::new(x - cw / 2.0, 0.0), Vec2::new(x - cw / 2.0, ch), Vec2::new(x - cw * 0.2, ch + 12.0), Vec2::new(x + cw / 2.0, ch - 24.0), Vec2::new(x + cw / 2.0, 0.0)];
            out.push(fill(&shaft, |_| 1.0));
            out.push(fill(&[Vec2::new(x - cw * 0.8, 0.0), Vec2::new(x - cw * 0.8, 16.0), Vec2::new(x + cw * 0.8, 16.0), Vec2::new(x + cw * 0.8, 0.0)], |_| 1.0));
        }
        Style::Cave => {
            // Stalagmites on a rocky floor.
            out.push(footed(heap(rng, w, h * 0.35, 0.4)));
            for _ in 0..3 + rng.below(4) {
                let (x, sw, sh) = (rng.range(-w / 2.0, w / 2.0), rng.range(24.0, 60.0), rng.range(70.0, 210.0));
                out.push(fill(&[Vec2::new(x - sw, -20.0), Vec2::new(x - sw * 0.2, sh * 0.7), Vec2::new(x + rng.range(-8.0, 8.0), sh), Vec2::new(x + sw * 0.25, sh * 0.65), Vec2::new(x + sw, -20.0)], |_| 1.0));
            }
        }
        Style::Bunker => {
            // Stacked concrete blocks and a coil of barbed wire.
            let mut y = 0.0;
            let mut half = w / 2.0;
            let mut blocks = vec![];
            for _ in 0..2 + rng.below(2) {
                let bh = rng.range(24.0, 44.0);
                let off = rng.range(-12.0, 12.0);
                blocks.push([Vec2::new(off - half, y), Vec2::new(off + half, y), Vec2::new(off + half, y + bh), Vec2::new(off - half, y + bh)]);
                y += bh + 2.0;
                half *= rng.range(0.6, 0.85);
            }
            let mut base = blocks[0].to_vec();
            base[0].y = -500.0;
            base[1].y = -500.0;
            out.push(fill(&base, |_| 1.0));
            for b in &blocks[1..] {
                out.push(fill(b, |_| 1.0));
            }
            let wire: Vec<Vec2> = (0..=16).map(|i| Vec2::new(-w / 2.0 + w * i as f32 / 16.0, y + 14.0 + if i % 2 == 0 { 10.0 } else { -4.0 })).collect();
            out.push(stroke(&wire, |_| 2.5, 1.0, 1.0, |_| 1.0));
        }
        Style::Concrete | Style::Mall | Style::Dam => {
            out.push(footed(ruin_outline(rng, w, h)));
            if rng.chance(0.6) {
                let bx = rng.range(-w / 3.0, w / 3.0);
                let bar = smooth_open(&[Vec2::new(bx, 60.0), Vec2::new(bx + rng.range(-20.0, 20.0), 130.0), Vec2::new(bx + rng.range(-60.0, 60.0), 160.0)], 6);
                out.push(stroke(&bar, |t| 5.0 - 3.0 * t, 1.0, 1.0, |_| 1.0));
            }
        }
    }
    out
}

fn ruin_outline(rng: &mut Rng, w: f32, h: f32) -> Vec<Vec2> {
    let mut pts = vec![Vec2::new(-w / 2.0, 0.0), Vec2::new(-w / 2.0, h * rng.range(0.6, 1.0))];
    let mut x = -w / 2.0 + rng.range(4.0, 12.0);
    while x < w / 2.0 - 4.0 {
        let collapse = if rng.chance(0.25) { rng.range(0.1, 0.35) * h } else { 0.0 };
        pts.push(Vec2::new(x, h - rng.range(0.0, 18.0) - collapse));
        x += rng.range(6.0, 18.0);
    }
    pts.push(Vec2::new(w / 2.0, h * rng.range(0.5, 1.0)));
    pts.push(Vec2::new(w / 2.0, 0.0));
    pts
}

fn spawn_particles(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
) {
    let mut rng = Rng(0xA54);
    let ash_mats = [
        blend(&mut materials, Color::srgba(0.12, 0.12, 0.12, 0.8)),
        blend(&mut materials, Color::srgba(0.55, 0.53, 0.50, 0.6)),
        blend(&mut materials, Color::srgba(0.30, 0.29, 0.28, 0.7)),
    ];
    let ember_mat = blend(&mut materials, Color::srgba(1.0, 0.75, 0.40, 0.9));
    let flake = meshes.add(triangles(&[[Vec2::new(-1.0, -0.6), Vec2::new(1.2, -0.3), Vec2::new(-0.2, 0.9)]]));
    let ember = meshes.add(Circle::new(1.0));

    for i in 0..ASH_COUNT + EMBER_COUNT {
        let is_ember = i >= ASH_COUNT;
        let pos = Vec2::new(rng.range(-WRAP.x, WRAP.x), rng.range(-WRAP.y, WRAP.y));
        let z = if rng.chance(0.3) { 30.0 } else { -10.0 };
        let (mesh, mat, vel, size) = if is_ember {
            (ember.clone(), ember_mat.clone(), Vec2::new(rng.range(10.0, 50.0), rng.range(25.0, 70.0)), rng.range(0.8, 1.8))
        } else {
            (flake.clone(), ash_mats[rng.below(3)].clone(), Vec2::new(rng.range(20.0, 70.0), -rng.range(20.0, 60.0)), rng.range(1.5, 3.5))
        };
        commands.spawn((
            Mesh2d(mesh),
            MeshMaterial2d(mat),
            Transform::from_translation(pos.extend(z)).with_scale(Vec3::splat(size)),
            Particle { vel, spin: rng.range(-4.0, 4.0), wobble: rng.range(0.0, 10.0) },
        ));
    }
}

fn drift_particles(
    time: Res<Time>,
    camera: Single<&Transform, (With<MainCamera>, Without<Particle>)>,
    mut particles: Query<(&Particle, &mut Transform)>,
) {
    let dt = time.delta_secs();
    let t = time.elapsed_secs();
    let cam = camera.translation.truncate();
    for (p, mut tf) in &mut particles {
        let sway = Vec2::new((t * 1.3 + p.wobble).sin() * 15.0, (t * 2.1 + p.wobble).cos() * 8.0);
        let mut pos = tf.translation.truncate() + (p.vel + sway) * dt;
        // Wrap around the camera so there's always ash on screen.
        let rel = pos - cam;
        pos = cam + Vec2::new(wrap(rel.x, WRAP.x), wrap(rel.y, WRAP.y));
        tf.translation = pos.extend(tf.translation.z);
        tf.rotate_z(p.spin * dt);
    }
}

fn wrap(v: f32, half: f32) -> f32 {
    (v + half).rem_euclid(half * 2.0) - half
}

fn animate_smoke(
    time: Res<Time>,
    mut puffs: Query<(&mut Puff, &mut Parallax, &mut Transform, &MeshMaterial2d<ColorMaterial>)>,
    mut materials: ResMut<Assets<ColorMaterial>>,
) {
    let dt = time.delta_secs();
    for (mut puff, mut layer, mut tf, mat) in &mut puffs {
        puff.age = (puff.age + dt) % puff.life;
        let k = puff.age / puff.life;
        let rise = Vec2::new(k * 160.0 + (k * 6.0 + puff.seed).sin() * 25.0, k * 380.0);
        layer.origin = puff.source + rise;
        tf.scale = Vec3::splat(18.0 + k * 110.0);
        tf.rotation = Quat::from_rotation_z(puff.seed + k * 1.5);
        if let Some(mut m) = materials.get_mut(&mat.0) {
            m.color = SMOKE.with_alpha((PI * k).sin().powf(0.7) * 0.55);
        }
    }
}

fn animate_fire(
    time: Res<Time>,
    fires: Query<(&Fire, &MeshMaterial2d<ColorMaterial>, &mut Transform)>,
    mut materials: ResMut<Assets<ColorMaterial>>,
) {
    let t = time.elapsed_secs();
    for (fire, mat, mut tf) in fires {
        let flicker = 0.75 + 0.25 * ((t * 9.0 + fire.seed).sin() * (t * 13.7 + fire.seed * 2.0).sin()).abs();
        tf.scale = Vec3::new(1.0, flicker, 1.0);
        if let Some(mut m) = materials.get_mut(&mat.0) {
            m.color = FIRE.with_alpha(fire.base_alpha * flicker);
        }
    }
}

fn lightning(
    time: Res<Time>,
    mut state: ResMut<Lightning>,
    mut shake: ResMut<Shake>,
    mut glare: ResMut<Glare>,
    terrain: Res<Terrain>,
    camera: Single<&Transform, With<MainCamera>>,
    mut open: Local<f32>,
    flash: Single<&MeshMaterial2d<ColorMaterial>, With<Flash>>,
    bolt: Single<(&Mesh2d, &mut Parallax, &mut Visibility), With<Bolt>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    let dt = time.delta_secs();
    let (bolt_mesh, mut bolt_pos, mut bolt_vis) = bolt.into_inner();
    state.next -= dt;
    state.since += dt;
    if state.next <= 0.0 {
        state.next = state.rng.range(4.0, 11.0);
        state.since = 0.0;
        bolt_pos.origin = Vec2::new(state.rng.range(-380.0, 380.0), 0.0);
        let mesh = bolt_mesh_gen(&mut state.rng);
        if let Some(mut m) = meshes.get_mut(&bolt_mesh.0) {
            *m = mesh;
        }
        if *open > 0.3 {
            shake.add(0.2);
        }
    }
    // Underground or indoors (a roof over the camera's whole box) there's no sky to flash.
    let center = camera.translation.truncate();
    let clear = [-900.0, -450.0, 0.0, 450.0, 900.0]
        .into_iter()
        .filter(|dx| (1..=150).all(|k| !terrain.solid_at(center + Vec2::new(*dx, k as f32 * PX))))
        .count() as f32
        / 5.0;
    *open += (clear - *open) * (1.0 - (-4.0 * dt).exp());
    // Double flicker: a strike, then a weaker echo.
    let s = state.since;
    let intensity = (-s * 14.0).exp().max(if s > 0.12 { 0.7 * (-(s - 0.12) * 12.0).exp() } else { 0.0 });
    let intensity = intensity * *open;
    glare.0 = intensity;
    if let Some(mut m) = materials.get_mut(&flash.0) {
        m.color = Color::srgba(0.9, 0.9, 1.0, intensity * 0.35);
    }
    *bolt_vis = if intensity > 0.3 { Visibility::Inherited } else { Visibility::Hidden };
}

/// A jagged bolt with a couple of forks, from high in the sky to the horizon.
fn bolt_mesh_gen(rng: &mut Rng) -> Mesh {
    let mut tris = Vec::new();
    let add_branch = |rng: &mut Rng, start: Vec2, steps: usize, width: f32, tris: &mut Vec<[Vec2; 3]>| {
        let mut p = start;
        for i in 0..steps {
            let next = p + Vec2::new(rng.range(-22.0, 22.0), -rng.range(18.0, 30.0));
            let w = width * (1.0 - i as f32 / steps as f32) + 0.6;
            let n = (next - p).normalize().perp() * w;
            tris.push([p + n, p - n, next + n]);
            tris.push([p - n, next - n, next + n]);
            p = next;
        }
        p
    };
    let mut p = Vec2::new(0.0, 330.0);
    for _ in 0..3 {
        p = add_branch(rng, p, 4, 2.2, &mut tris);
        let fork = p;
        add_branch(rng, fork, 3, 1.0, &mut tris);
    }
    triangles(&tris)
}

fn parallax(
    camera: Single<&Transform, With<MainCamera>>,
    mut layers: Query<(&Parallax, &mut Transform), Without<MainCamera>>,
) {
    let cam = camera.translation.truncate();
    for (layer, mut tf) in &mut layers {
        let p = layer.origin + cam * layer.factor;
        tf.translation.x = p.x;
        tf.translation.y = p.y;
    }
}

/// How each region colors the sky behind it (multiplies the gradient).
fn sky_tint(style: Style) -> Vec3 {
    match style {
        Style::Concrete => Vec3::new(1.0, 1.0, 1.0),
        Style::Trash => Vec3::new(1.1, 1.0, 0.6),
        Style::Sewer | Style::Treatment => Vec3::new(0.65, 1.05, 0.8),
        Style::Park => Vec3::new(0.85, 1.05, 0.7),
        Style::Mall => Vec3::new(1.05, 0.7, 1.1),
        Style::Dam => Vec3::new(0.65, 0.85, 1.2),
        _ => Vec3::new(0.8, 0.8, 0.85),
    }
}

fn tint_sky(
    time: Res<Time>,
    terrain: Res<Terrain>,
    camera: Single<&Transform, With<MainCamera>>,
    sky: Single<&MeshMaterial2d<ColorMaterial>, With<Sky>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    mut current: Local<Option<Vec3>>,
) {
    let target = sky_tint(terrain.style_at(camera.translation.truncate()));
    let now = current.get_or_insert(target);
    *now += (target - *now) * (1.0 - (-1.5 * time.delta_secs()).exp());
    if let Some(mut m) = materials.get_mut(&sky.0) {
        m.color = Color::linear_rgb(now.x, now.y, now.z);
    }
}

/// Distant silhouettes in the style of each region, far and hazy, with gaps
/// between them so the sky shows through.
fn far_pieces(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<ColorMaterial>,
    horizon: &Horizon,
    rng: &mut Rng,
    span: &impl Fn(f32) -> (f32, f32),
    seen_from: &impl Fn(f32, Vec2) -> Style,
) {
    let factor = Vec2::new(0.96, 0.97);
    let mat = materials.add(Color::srgb(0.30, 0.21, 0.18));
    let (mut x, extent) = span(factor.x);
    while x < extent {
        let style = seen_from(x, factor);
        let origin = horizon.anchor(x, factor, -40.0);
        // Pieces stand on a foot reaching well below the screen.
        let footed = |mut top: Vec<Vec2>| {
            let last = top.len() - 1;
            top[0].y = -900.0;
            top[last].y = -900.0;
            fill(&top, |p| 0.85 + 0.15 * (p.y / 300.0).clamp(0.0, 1.0))
        };
        let (w, meshes_out): (f32, Vec<Mesh>) = match style {
            Style::Concrete | Style::Mall => {
                // A cluster of thin broken towers.
                let w = rng.range(120.0, 260.0);
                let n = 2 + rng.below(3);
                let towers = (0..n)
                    .map(|i| {
                        let tw = rng.range(26.0, 50.0);
                        let th = rng.range(160.0, 400.0);
                        let mut o = ruin_outline(rng, tw, th);
                        for p in &mut o {
                            p.x += -w / 2.0 + w * (i as f32 + 0.5) / n as f32;
                        }
                        footed(o)
                    })
                    .collect();
                (w, towers)
            }
            Style::Trash | Style::Sinkhole => {
                // Mounds of junk, sometimes with a tall chimney of smoke.
                let w = rng.range(280.0, 520.0);
                let h = rng.range(70.0, 150.0);
                let mut pts = vec![Vec2::new(-w / 2.0, 0.0)];
                let mut px = -w / 2.0;
                while px < w / 2.0 {
                    let edge = 1.0 - (px / (w / 2.0)).abs().powi(2);
                    pts.push(Vec2::new(px, h * (0.2 + 0.8 * edge) * rng.range(0.75, 1.0)));
                    px += rng.range(16.0, 34.0);
                }
                pts.push(Vec2::new(w / 2.0, 0.0));
                (w, vec![footed(pts)])
            }
            Style::Sewer | Style::Treatment | Style::Subway => {
                // Cooling towers and tall stacks.
                let w = rng.range(160.0, 320.0);
                let (h, r) = (rng.range(120.0, 220.0), rng.range(36.0, 60.0));
                let tower = vec![Vec2::new(-r, 0.0), Vec2::new(-r * 0.62, h * 0.55), Vec2::new(-r * 0.72, h), Vec2::new(r * 0.72, h), Vec2::new(r * 0.62, h * 0.55), Vec2::new(r, 0.0)];
                let mut v = vec![footed(tower)];
                let sx = rng.range(r + 40.0, w);
                let sh = rng.range(220.0, 340.0);
                v.push(fill(&[Vec2::new(sx - 9.0, -900.0), Vec2::new(sx + 9.0, -900.0), Vec2::new(sx + 6.0, sh), Vec2::new(sx - 6.0, sh)], |_| 1.0));
                (w + 40.0, v)
            }
            Style::Park => {
                // Soft hills with a few dead trees.
                let w = rng.range(420.0, 800.0);
                let h = rng.range(60.0, 130.0);
                let hill: Vec<Vec2> = (0..=16)
                    .map(|i| {
                        let t = i as f32 / 16.0;
                        Vec2::new(-w / 2.0 + w * t, h * (std::f32::consts::PI * t).sin().powf(0.8))
                    })
                    .collect();
                let mut v = vec![footed(hill)];
                for _ in 0..2 + rng.below(3) {
                    let tx = rng.range(-w / 3.0, w / 3.0);
                    let th = rng.range(70.0, 130.0);
                    let trunk = smooth_open(&[Vec2::new(tx, h * 0.6), Vec2::new(tx + rng.range(-8.0, 8.0), h * 0.6 + th * 0.5), Vec2::new(tx + rng.range(-25.0, 25.0), h * 0.6 + th)], 5);
                    v.push(stroke(&trunk, |t| 4.0 * (1.0 - t) + 1.0, 1.0, 1.0, |_| 1.0));
                }
                (w, v)
            }
            Style::Dam => {
                // Cold jagged mountains.
                let w = rng.range(420.0, 800.0);
                let h = rng.range(200.0, 380.0);
                let mut pts = vec![Vec2::new(-w / 2.0, 0.0)];
                let mut px = -w / 2.0;
                while px < w / 2.0 {
                    let edge = 1.0 - (px / (w / 2.0)).abs();
                    pts.push(Vec2::new(px, h * edge.powf(0.9) * rng.range(0.6, 1.0)));
                    px += rng.range(30.0, 70.0);
                }
                pts.push(Vec2::new(w / 2.0, 0.0));
                (w, vec![footed(pts)])
            }
            _ => {
                // Underground regions: a faint crag, rarely seen.
                let w = rng.range(300.0, 500.0);
                let h = rng.range(80.0, 160.0);
                let pts = vec![Vec2::new(-w / 2.0, 0.0), Vec2::new(-w * 0.2, h), Vec2::new(0.0, h * 0.6), Vec2::new(w * 0.25, h * 0.9), Vec2::new(w / 2.0, 0.0)];
                (w, vec![footed(pts)])
            }
        };
        for mesh in meshes_out {
            layer(commands, meshes, mesh, &mat, origin + Vec2::X * w / 2.0, -98.0, factor);
        }
        // Wide, uneven gaps: the sky shows through between pieces.
        x += w + rng.range(120.0, 700.0);
    }
}
