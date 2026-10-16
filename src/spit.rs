//! The Venom Mother's gift: once she's beaten, the chameleon spits venom
//! (right mouse button / G) toward the mouse. A glob flies fast with a slight
//! drop, hurts the first boss or mob it touches (one hit, applied with the
//! slash's in `boss.rs` / `mobs.rs` through `SpitHit`) and splashes on walls.

use bevy::prelude::*;

use crate::{
    boss::{Boss, Defeated},
    camera,
    chameleon::{self, ChameleonAnim},
    gore::{Bleed, Fluid},
    level::{self, Platform, Restart},
    mobs::{Mob, State},
    player::{self, Player},
    shapes::{ellipse_pts, fill},
    terrain::Terrain,
};

const SPEED: f32 = 1050.0;
const GRAVITY: f32 = 450.0;
const COOLDOWN: f32 = 0.4;
const LIFE: f32 = 1.4;
/// Spat from this far out along the aim, from the neck (about the snout).
const MOUTH: f32 = 22.0;
const COLOR: Color = Color::srgb(0.36, 0.78, 0.18);

pub struct SpitPlugin;

impl Plugin for SpitPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<SpitHit>()
            .add_systems(Startup, load)
            .add_systems(PreUpdate, clear_on_restart.after(level::restart))
            .add_systems(Update, (spit, move_spit).chain().after(player::move_player).after(chameleon::update_anim_state));
    }
}

/// A glob hit this boss or mob, flying along `dir`.
#[derive(Message)]
pub struct SpitHit {
    pub target: Entity,
    pub dir: Vec2,
}

#[derive(Component)]
pub struct Glob {
    vel: Vec2,
    life: f32,
}

#[derive(Resource)]
struct SpitAssets {
    mesh: Handle<Mesh>,
    mat: Handle<ColorMaterial>,
}

fn load(mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>, mut materials: ResMut<Assets<ColorMaterial>>) {
    let glob = fill(&ellipse_pts(6.0, 4.5, 18), |p| 1.0 - 0.35 * (p - Vec2::new(2.0, 1.5)).length() / 7.0);
    commands.insert_resource(SpitAssets { mesh: meshes.add(glob), mat: materials.add(COLOR) });
}

fn spit(
    mut commands: Commands,
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    defeated: Res<Defeated>,
    assets: Res<SpitAssets>,
    mut cooldown: Local<f32>,
    player: Single<(&Player, &Transform, &mut ChameleonAnim)>,
) {
    *cooldown = (*cooldown - time.delta_secs()).max(0.0);
    let pressed = keys.just_pressed(KeyCode::KeyQ) || mouse.just_pressed(MouseButton::Right);
    let (p, tf, mut anim) = player.into_inner();
    if !pressed || !defeated.spit() || *cooldown > 0.0 || p.dead.is_some() || p.stun > 0.0 {
        return;
    }
    *cooldown = COOLDOWN;
    anim.spit();
    let from = chameleon::neck(tf.translation.truncate(), p.facing) + p.aim * MOUTH;
    commands.spawn((
        Glob { vel: p.aim * SPEED, life: LIFE },
        Mesh2d(assets.mesh.clone()),
        MeshMaterial2d(assets.mat.clone()),
        Transform::from_translation(from.extend(15.0)).with_rotation(Quat::from_rotation_z(p.aim.to_angle())),
        camera::color_layer(),
    ));
}

pub fn move_spit(
    mut commands: Commands,
    time: Res<Time>,
    mut bleed: MessageWriter<Bleed>,
    mut hits: MessageWriter<SpitHit>,
    mut globs: Query<(Entity, &mut Glob, &mut Transform)>,
    bosses: Query<(Entity, &Boss, &Transform), Without<Glob>>,
    mobs: Query<(Entity, &Mob, &Transform), Without<Glob>>,
    platforms: Query<(&Platform, &Transform), Without<Glob>>,
    terrain: Res<Terrain>,
) {
    let dt = time.delta_secs().min(1.0 / 30.0);
    let t = time.elapsed_secs();
    for (e, mut g, mut tf) in &mut globs {
        g.vel.y -= GRAVITY * dt;
        g.life -= dt;
        let from = tf.translation.truncate();
        let pos = from + g.vel * dt;
        tf.translation = pos.extend(tf.translation.z);
        tf.rotation = Quat::from_rotation_z(g.vel.to_angle());
        tf.scale = Vec3::new(1.2 + (t * 30.0).sin() * 0.12, 0.9 - (t * 30.0).sin() * 0.1, 1.0);
        let splash = |dir: Vec2, amount| Bleed { pos, dir, spread: 1.2, speed: 220.0, amount, fluid: Fluid::Venom, sticky: false };

        // The nearest living thing along this step's path.
        let near = |at: Vec2, radius: f32| dist_to_segment(at, from, pos) < radius;
        let target = bosses
            .iter()
            .filter(|(_, b, btf)| !b.dead && near(btf.translation.truncate(), b.spec.radius))
            .map(|(e, _, _)| e)
            .chain(
                mobs.iter()
                    .filter(|(_, m, mtf)| !matches!(m.state, State::Dead { .. }) && !m.untouchable() && near(mtf.translation.truncate(), m.spec.radius))
                    .map(|(e, _, _)| e),
            )
            .next();
        if let Some(target) = target {
            hits.write(SpitHit { target, dir: g.vel.normalize_or(Vec2::X) });
            bleed.write(splash(-g.vel, 10));
            commands.entity(e).despawn();
            continue;
        }
        let hit_wall = terrain.solid_at(pos)
            || platforms.iter().any(|(pl, ptf)| {
                let d = (pos - ptf.translation.truncate()).abs();
                pl.solid && d.x < pl.half.x && d.y < pl.half.y
            });
        if hit_wall || g.life <= 0.0 {
            bleed.write(splash(-g.vel, 12));
            commands.entity(e).despawn();
        } else if (t * 40.0).fract() < 0.25 {
            // A thin trail of drips.
            bleed.write(Bleed { dir: -g.vel, spread: 0.6, speed: 60.0, ..splash(Vec2::Y, 1) });
        }
    }
}

fn dist_to_segment(p: Vec2, a: Vec2, b: Vec2) -> f32 {
    let ab = b - a;
    let t = ((p - a).dot(ab) / ab.length_squared().max(1e-6)).clamp(0.0, 1.0);
    p.distance(a + ab * t)
}

/// F8: no venom left in the air.
fn clear_on_restart(mut commands: Commands, mut restarted: MessageReader<Restart>, globs: Query<Entity, With<Glob>>) {
    if restarted.read().count() > 0 {
        for e in &globs {
            commands.entity(e).despawn();
        }
    }
}
