//! Sulfur fumes: the Gutter King's yellow, in the air. Vents (placed by
//! `terrain_props/moss.rs`, a crusted cone you can see from the start) leak
//! nothing until the king falls; then soft yellow clouds rise out of them,
//! the nearer ones first (`regrowth` wave), drifting and thinning as they go.
//! Drawn on the color layer, so they keep their color over the gray world.

use bevy::{
    asset::RenderAssetUsages,
    mesh::Indices,
    prelude::*,
    render::render_resource::PrimitiveTopology,
    sprite_render::AlphaMode2d,
};

use crate::{
    boss::BossKind,
    camera::{self, MainCamera},
    regrowth::Regrowth,
    shapes::Rng,
    terrain::{Style, Terrain},
};

/// Vents farther than this from the camera don't smoke (nobody sees them).
const SHOWN_WITHIN: f32 = 2400.0;

/// The toxic fumes' color (r, g, b ranges): gray for now, the boss's purple later.
const TOXIC_TINT: [(f32, f32); 3] = [(0.62, 0.78), (0.62, 0.78), (0.62, 0.78)];
/// How opaque a toxic cloud gets at its thickest.
const TOXIC_PEAK: f32 = 0.5;
/// Clouds per second per 100 000 square units of fumes in view.
const TOXIC_DENSITY: f32 = 2.2;

pub struct FumesPlugin;

impl Plugin for FumesPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, make_cloud).add_systems(Update, (emit, ambient, toxic_air, drift).chain());
    }
}

/// A vent that smokes once the Gutter King's color is back.
#[derive(Component)]
pub struct Fumer {
    /// Distance from the king's home (when the wave of color reaches it).
    pub dist: f32,
    pub timer: f32,
    pub seed: u32,
}

#[derive(Component)]
struct Puff {
    age: f32,
    life: f32,
    vel: Vec2,
    size: f32,
    phase: f32,
    strength: f32,
    /// How opaque it gets at its thickest.
    peak: f32,
    material: Handle<ColorMaterial>,
}

/// The soft cloud mesh (also the frog's poison clouds).
#[derive(Resource)]
pub(crate) struct Cloud(pub(crate) Handle<Mesh>);

/// A soft round cloud: opaque in the middle, clear at the rim.
fn make_cloud(mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>) {
    const N: usize = 20;
    let mut pos = vec![[0.0, 0.0, 0.0]];
    let mut colors = vec![[1.0, 1.0, 1.0, 1.0]];
    for (r, a) in [(0.5, 0.65), (1.0, 0.0)] {
        for i in 0..N {
            let t = i as f32 / N as f32 * std::f32::consts::TAU;
            pos.push([t.cos() * r, t.sin() * r * 0.8, 0.0]);
            colors.push([1.0, 1.0, 1.0, a]);
        }
    }
    let mut idx = Vec::new();
    for i in 0..N as u32 {
        let (j, k) = (1 + i, 1 + (i + 1) % N as u32);
        idx.extend([0, j, k]);
        idx.extend([j, N as u32 + j, N as u32 + k]);
        idx.extend([j, N as u32 + k, k]);
    }
    let n = pos.len();
    let mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default())
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, pos)
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, vec![[0.0, 0.0, 1.0]; n])
        .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, vec![[0.0, 0.0]; n])
        .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, colors)
        .with_inserted_indices(Indices::U32(idx));
    commands.insert_resource(Cloud(meshes.add(mesh)));
}

fn emit(
    time: Res<Time>,
    regrowth: Res<Regrowth>,
    cloud: Res<Cloud>,
    camera: Single<&Transform, With<MainCamera>>,
    mut vents: Query<(&mut Fumer, &GlobalTransform)>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    mut commands: Commands,
) {
    let dt = time.delta_secs();
    let eye = camera.translation.truncate();
    for (mut vent, tf) in &mut vents {
        let strength = regrowth.reveal(BossKind::Roach, vent.dist);
        let at = tf.translation().truncate();
        if strength <= 0.0 || at.distance(eye) > SHOWN_WITHIN {
            continue;
        }
        vent.timer -= dt;
        while vent.timer <= 0.0 {
            let mut rng = Rng(vent.seed.wrapping_mul(2654435761).wrapping_add((time.elapsed_secs() * 1000.0) as u32) | 1);
            rng.next();
            vent.timer += rng.range(0.2, 0.4);
            let tint = Color::srgba(rng.range(0.88, 1.0), rng.range(0.78, 0.9), rng.range(0.18, 0.4), 0.0);
            let material = materials.add(ColorMaterial { color: tint, alpha_mode: AlphaMode2d::Blend, ..default() });
            let size = rng.range(70.0, 130.0);
            commands.spawn((
                Mesh2d(cloud.0.clone()),
                MeshMaterial2d(material.clone()),
                Transform::from_translation((at + Vec2::new(rng.range(-8.0, 8.0), 0.0)).extend(-0.3 + rng.range(0.0, 0.05))).with_scale(Vec3::splat(size * 0.5)),
                camera::color_layer(),
                Puff {
                    age: 0.0,
                    life: rng.range(6.0, 10.0),
                    vel: Vec2::new(rng.range(-6.0, 6.0), rng.range(22.0, 40.0)),
                    size,
                    phase: rng.range(0.0, std::f32::consts::TAU),
                    strength,
                    peak: 0.3,
                    material,
                },
            ));
        }
    }
}

fn drift(
    time: Res<Time>,
    mut commands: Commands,
    mut puffs: Query<(Entity, &mut Puff, &mut Transform)>,
    mut materials: ResMut<Assets<ColorMaterial>>,
) {
    let dt = time.delta_secs();
    let now = time.elapsed_secs();
    for (e, mut puff, mut tf) in &mut puffs {
        puff.age += dt;
        let t = puff.age / puff.life;
        if t >= 1.0 {
            commands.entity(e).despawn();
            continue;
        }
        // Swells as it goes up, swaying with a slow breeze.
        let sway = (now * 0.5 + puff.phase).sin() * 10.0;
        tf.translation += ((puff.vel + Vec2::X * sway) * dt).extend(0.0);
        tf.scale = Vec3::splat(puff.size * 0.5 * (0.6 + 1.5 * t));
        if let Some(mut m) = materials.get_mut(&puff.material) {
            let fade = (t * std::f32::consts::PI).sin().powf(0.8);
            m.color = m.color.with_alpha(puff.peak * puff.strength * fade);
        }
    }
}

/// The "feature: toxic fumes" slices: dense banks of fumes drifting inside the
/// marked area only (and nowhere else), whatever the region's style.
fn toxic_air(
    time: Res<Time>,
    terrain: Res<Terrain>,
    cloud: Res<Cloud>,
    camera: Single<&Transform, With<MainCamera>>,
    mut owed: Local<f32>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    mut commands: Commands,
) {
    let eye = camera.translation.truncate();
    let view = Rect::from_center_half_size(eye, Vec2::new(1300.0, 700.0));
    let zones: Vec<Rect> = terrain.toxic_zones().map(|z| z.intersect(view)).filter(|z| !z.is_empty()).collect();
    let area: f32 = zones.iter().map(|z| z.width() * z.height()).sum();
    if area <= 0.0 {
        *owed = 0.0;
        return;
    }
    *owed += (area / 100_000.0 * TOXIC_DENSITY).min(14.0) * time.delta_secs();
    let mut rng = Rng(((time.elapsed_secs() * 6007.0) as u32).wrapping_mul(2654435761) | 1);
    while *owed >= 1.0 {
        *owed -= 1.0;
        rng.next();
        // Pick a zone in proportion to its visible area.
        let mut pick = rng.range(0.0, area);
        let zone = zones.iter().find(|z| {
            pick -= z.width() * z.height();
            pick <= 0.0
        });
        let Some(zone) = zone.or(zones.last()) else { continue };
        let at = Vec2::new(rng.range(zone.min.x, zone.max.x), rng.range(zone.min.y, zone.max.y));
        if terrain.solid_at(at) {
            continue;
        }
        let tint = Color::srgba(rng.range(TOXIC_TINT[0].0, TOXIC_TINT[0].1), rng.range(TOXIC_TINT[1].0, TOXIC_TINT[1].1), rng.range(TOXIC_TINT[2].0, TOXIC_TINT[2].1), 0.0);
        let material = materials.add(ColorMaterial { color: tint, alpha_mode: AlphaMode2d::Blend, ..default() });
        commands.spawn((
            Mesh2d(cloud.0.clone()),
            MeshMaterial2d(material.clone()),
            Transform::from_translation(at.extend(-0.22 + rng.range(0.0, 0.05))),
            camera::color_layer(),
            Puff {
                age: 0.0,
                life: rng.range(8.0, 13.0),
                vel: Vec2::new(rng.range(-14.0, 14.0), rng.range(-3.0, 10.0)),
                size: rng.range(220.0, 420.0),
                phase: rng.range(0.0, std::f32::consts::TAU),
                strength: 1.0,
                peak: TOXIC_PEAK,
                material,
            },
        ));
    }
}

/// Out of the vents and everywhere around: once a color is back the air around
/// the camera carries it. The Gutter King's yellow: banks of sulfur fumes
/// drifting across the screen (thick in the dump, the plant and the
/// cave, thin elsewhere). The Venom Mother's green: a rain of glowing spores.
fn ambient(
    time: Res<Time>,
    regrowth: Res<Regrowth>,
    terrain: Res<Terrain>,
    cloud: Res<Cloud>,
    camera: Single<&Transform, With<MainCamera>>,
    mut timers: Local<[f32; 2]>,
    mut mists: Local<[f32; 1]>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    mut commands: Commands,
) {
    let dt = time.delta_secs();
    let eye = camera.translation.truncate();
    let fumy = matches!(terrain.style_at(eye), Style::Trash | Style::Treatment | Style::Cave | Style::Sinkhole);
    let mut rng = Rng(((time.elapsed_secs() * 7919.0) as u32).wrapping_mul(2246822519) | 1);
    // Low banks of pale green mist drifting over the ground of her region.
    let (_, near) = regrowth.felt_at(BossKind::Wasp, &terrain, eye);
    let mist = regrowth.wash(BossKind::Wasp) * (0.3 + 0.7 * near);
    mists[0] -= dt;
    while mist > 0.0 && mists[0] <= 0.0 {
        mists[0] += 1.0 / (1.1 * mist).max(0.01);
        rng.next();
        let at = eye + Vec2::new(rng.range(-1300.0, 1300.0), rng.range(-600.0, 300.0));
        if terrain.solid_at(at) {
            continue;
        }
        let tint = Color::srgba(rng.range(0.6, 0.78), rng.range(0.85, 0.95), rng.range(0.45, 0.6), 0.0);
        let material = materials.add(ColorMaterial { color: tint, alpha_mode: AlphaMode2d::Blend, ..default() });
        commands.spawn((
            Mesh2d(cloud.0.clone()),
            MeshMaterial2d(material.clone()),
            Transform::from_translation(at.extend(-0.24 + rng.range(0.0, 0.05))),
            camera::color_layer(),
            Puff {
                age: 0.0,
                life: rng.range(12.0, 18.0),
                vel: Vec2::new(rng.range(8.0, 26.0), rng.range(-2.0, 5.0)),
                size: rng.range(320.0, 560.0),
                phase: rng.range(0.0, std::f32::consts::TAU),
                strength: 1.0,
                peak: 0.16 * mist.min(1.0),
                material,
            },
        ));
    }
    for (i, boss) in [BossKind::Roach, BossKind::Wasp].into_iter().enumerate() {
        let (revealed, near) = regrowth.felt_at(boss, &terrain, eye);
        // Her green is a wash that passes; his yellow stays.
        let revealed = if boss == BossKind::Wasp { regrowth.wash(boss) } else { revealed };
        let strength = revealed * (0.35 + 0.65 * near);
        if strength <= 0.0 {
            continue;
        }
        // The king's air is heavy with fumes; the Venom Mother's carries mist and spores.
        let (rate, size, peak, life) = match boss {
            BossKind::Roach => (if fumy { 1.6 } else { 0.5 } * strength, (160.0, 320.0), if fumy { 0.2 } else { 0.1 }, (9.0, 14.0)),
            BossKind::Wasp => (7.0 * strength, (5.0, 10.0), 0.7, (5.0, 8.0)),
            BossKind::Dragonfly | BossKind::Spider | BossKind::Centipede | BossKind::Frog | BossKind::Grasshopper | BossKind::Rat | BossKind::Mantis | BossKind::Crab | BossKind::Beetle => unreachable!("only the king and the mother have airs"),
        };
        timers[i] -= dt;
        while timers[i] <= 0.0 {
            timers[i] += 1.0 / rate.max(0.01);
            rng.next();
            let at = eye + Vec2::new(rng.range(-1200.0, 1200.0), rng.range(-650.0, 650.0));
            if terrain.solid_at(at) {
                continue;
            }
            let (tint, vel) = match boss {
                BossKind::Roach => (Color::srgba(rng.range(0.9, 1.0), rng.range(0.78, 0.88), rng.range(0.2, 0.38), 0.0), Vec2::new(rng.range(-30.0, 30.0), rng.range(-4.0, 12.0))),
                BossKind::Wasp => (Color::srgba(rng.range(0.55, 0.8), rng.range(0.9, 1.0), rng.range(0.2, 0.45), 0.0), Vec2::new(rng.range(-12.0, 12.0), rng.range(8.0, 22.0))),
                BossKind::Dragonfly | BossKind::Spider | BossKind::Centipede | BossKind::Frog | BossKind::Grasshopper | BossKind::Rat | BossKind::Mantis | BossKind::Crab | BossKind::Beetle => unreachable!(),
            };
            let material = materials.add(ColorMaterial { color: tint, alpha_mode: AlphaMode2d::Blend, ..default() });
            commands.spawn((
                Mesh2d(cloud.0.clone()),
                MeshMaterial2d(material.clone()),
                Transform::from_translation(at.extend(-0.25 + rng.range(0.0, 0.05))),
                camera::color_layer(),
                Puff {
                    age: 0.0,
                    life: rng.range(life.0, life.1),
                    vel,
                    size: rng.range(size.0, size.1),
                    phase: rng.range(0.0, std::f32::consts::TAU),
                    strength: 1.0,
                    peak: peak * strength.min(1.0),
                    material,
                },
            ));
        }
    }
}
