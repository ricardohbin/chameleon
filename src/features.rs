//! Behaviors for the sketch's "feature: …" slices that aren't plain terrain
//! rules (sliding is in `player.rs`): junk breaking off a crumbling slope as
//! the chameleon slides down it.

use bevy::prelude::*;

use crate::{
    level::{self, Restart},
    player::{self, Player},
    shapes::*,
    terrain::Terrain,
};

/// Chunks per second shaken loose while sliding.
const DEBRIS_RATE: f32 = 14.0;
const DEBRIS_LIFE: f32 = 1.6;
const GRAVITY: f32 = 1400.0;

pub struct FeaturesPlugin;

impl Plugin for FeaturesPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, load_debris)
            .add_systems(PreUpdate, clear_debris.after(level::restart))
            .add_systems(Update, (shed_debris, tumble_debris).chain().after(player::move_player));
    }
}

#[derive(Resource)]
struct DebrisAssets {
    meshes: Vec<Handle<Mesh>>,
    mats: Vec<Handle<ColorMaterial>>,
    rng: Rng,
    /// Fractional chunks owed.
    due: f32,
}

#[derive(Component)]
struct Debris {
    vel: Vec2,
    spin: f32,
    age: f32,
    size: f32,
}

/// F8: chunks still tumbling go.
fn clear_debris(mut commands: Commands, mut restarted: MessageReader<Restart>, debris: Query<Entity, With<Debris>>) {
    if restarted.read().count() == 0 {
        return;
    }
    for e in &debris {
        commands.entity(e).despawn();
    }
}

fn load_debris(mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>, mut materials: ResMut<Assets<ColorMaterial>>) {
    let mut rng = Rng(0xDEB215);
    let meshes = (0..6).map(|_| meshes.add(fill(&rock_pts(&mut rng, 1.0, 6), |p| 0.75 + 0.25 * p.y))).collect();
    let mats = [(0.36, 0.33, 0.30), (0.44, 0.30, 0.20), (0.52, 0.50, 0.46), (0.18, 0.17, 0.17)]
        .into_iter()
        .map(|(r, g, b)| materials.add(Color::srgb(r, g, b)))
        .collect();
    commands.insert_resource(DebrisAssets { meshes, mats, rng, due: 0.0 });
}

/// Sliding down a crumbling feature knocks junk loose around the chameleon's feet.
fn shed_debris(
    mut commands: Commands,
    time: Res<Time>,
    terrain: Res<Terrain>,
    mut assets: ResMut<DebrisAssets>,
    player: Single<(&Player, &Transform)>,
) {
    let (p, tf) = *player;
    let feet = tf.translation.truncate() - Vec2::Y * player::HALF.y;
    // Sliding sheds the most; just walking on it knocks some loose too.
    let moving = p.slipping > 0.0 || (p.grounded && p.vel.x.abs() > 40.0);
    let crumbling = moving && p.dead.is_none() && terrain.features_at(feet).any(|f| f.crumbles);
    if !crumbling {
        assets.due = 0.0;
        return;
    }
    let rate = if p.slipping > 0.0 { 1.0 } else { 0.35 };
    assets.due += DEBRIS_RATE * rate * time.delta_secs() * (p.vel.x.abs() / 300.0).clamp(0.5, 2.0);
    while assets.due >= 1.0 {
        assets.due -= 1.0;
        let a = &mut *assets;
        let size = a.rng.range(3.0, 8.0);
        let at = feet + Vec2::new(a.rng.range(-player::HALF.x, player::HALF.x), a.rng.range(0.0, 6.0));
        let vel = Vec2::new(p.vel.x * a.rng.range(0.2, 0.7) + a.rng.range(-60.0, 60.0), a.rng.range(60.0, 240.0));
        let mesh = a.meshes[a.rng.below(a.meshes.len())].clone();
        let mat = a.mats[a.rng.below(a.mats.len())].clone();
        commands.spawn((
            Mesh2d(mesh),
            MeshMaterial2d(mat),
            Transform::from_translation(at.extend(9.0)).with_scale(Vec3::splat(size)),
            Debris { vel, spin: a.rng.range(-12.0, 12.0), age: 0.0, size },
        ));
    }
}

/// Chunks bounce down the slope, then crumble away.
fn tumble_debris(mut commands: Commands, time: Res<Time>, terrain: Res<Terrain>, mut debris: Query<(Entity, &mut Debris, &mut Transform)>) {
    let dt = time.delta_secs().min(1.0 / 30.0);
    for (e, mut d, mut tf) in &mut debris {
        d.age += dt;
        if d.age > DEBRIS_LIFE {
            commands.entity(e).despawn();
            continue;
        }
        d.vel.y -= GRAVITY * dt;
        let pos = tf.translation.truncate();
        let next = pos + d.vel * dt;
        if terrain.solid_at(next) {
            // Bounce off the ground, losing most of the energy.
            if terrain.solid_at(Vec2::new(next.x, pos.y)) {
                d.vel.x *= -0.4;
            }
            d.vel.y = d.vel.y.abs() * 0.3;
            d.spin *= 0.6;
        } else {
            tf.translation.x = next.x;
            tf.translation.y = next.y;
        }
        tf.rotate_z(d.spin * dt);
        let fade = 1.0 - ((d.age - DEBRIS_LIFE * 0.6) / (DEBRIS_LIFE * 0.4)).clamp(0.0, 1.0);
        tf.scale = Vec3::splat(d.size * fade);
    }
}
