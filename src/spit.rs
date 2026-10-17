//! The Venom Mother's gift: once she's beaten, the chameleon spits venom
//! (right mouse button / G) toward the mouse. A glob is lobbed like a
//! grenade (it drops off quickly) and costs half the strength bar; it hurts the first boss or mob it touches (one hit, applied with the
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

/// Lobbed like a grenade: slow and heavy, it drops off quickly.
/// Hold to charge: the speed goes from `SPEED` (tapped) to `SPEED_MAX` (fully charged), and the
/// strength burnt while charging reaches `FULL_DRAIN` of the bar, in `CHARGE_TIME` seconds.
const SPEED: f32 = 100.0;
const SPEED_MAX: f32 = 1000.0;
const CHARGE_TIME: f32 = 1.2;
/// Share of the bar a full charge empties (net of the refill) from a full bar.
const FULL_DRAIN: f32 = 0.5;
const GRAVITY: f32 = 1400.0;
const LIFE: f32 = 1.4;
/// Spat from this far out along the aim, from the neck (about the snout).
const MOUTH: f32 = 22.0;
const COLOR: Color = Color::srgb(0.36, 0.78, 0.18);

pub struct SpitPlugin;

impl Plugin for SpitPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<SpitHit>()
            .init_resource::<SpitCharge>()
            .add_systems(Startup, load)
            .add_systems(PreUpdate, clear_on_restart.after(level::restart))
            .add_systems(Update, (spit, charge_fx, move_spit).chain().after(player::move_player).after(chameleon::update_anim_state));
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

/// How charged the spit being held is (0..1), `None` when not charging.
#[derive(Resource, Default)]
struct SpitCharge(Option<f32>);

/// A streak around the mouth while charging (`r` = distance from it): sucked in, or thrown out.
#[derive(Component)]
struct Streak {
    ang: f32,
    r: f32,
    /// Flying out from the mouth (the charge building up) instead of in.
    out: bool,
}

#[derive(Resource)]
struct SpitAssets {
    mesh: Handle<Mesh>,
    mat: Handle<ColorMaterial>,
    fx_mesh: Handle<Mesh>,
    fx_mat: Handle<ColorMaterial>,
}

fn load(mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>, mut materials: ResMut<Assets<ColorMaterial>>) {
    let glob = fill(&ellipse_pts(6.0, 4.5, 18), |p| 1.0 - 0.35 * (p - Vec2::new(2.0, 1.5)).length() / 7.0);
    commands.insert_resource(SpitAssets {
        mesh: meshes.add(glob),
        mat: materials.add(COLOR),
        fx_mesh: meshes.add(Rectangle::new(1.0, 1.0)),
        fx_mat: materials.add(ColorMaterial { color: Color::srgba(0.85, 1.0, 0.8, 0.3), alpha_mode: bevy::sprite_render::AlphaMode2d::Blend, ..default() }),
    });
}

fn spit(
    mut commands: Commands,
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    defeated: Res<Defeated>,
    assets: Res<SpitAssets>,
    mut charge: ResMut<SpitCharge>,
    player: Single<(&mut Player, &Transform, &mut ChameleonAnim)>,
) {
    let pressed = keys.just_pressed(KeyCode::KeyQ) || mouse.just_pressed(MouseButton::Right);
    let held = keys.pressed(KeyCode::KeyQ) || mouse.pressed(MouseButton::Right);
    let (mut p, tf, mut anim) = player.into_inner();
    if p.dead.is_some() || p.stun > 0.0 || !defeated.spit() {
        charge.0 = None;
        return;
    }
    let pill = p.pill.cost();
    if pressed {
        charge.0 = Some(0.0);
        return;
    }
    let Some(c) = charge.0 else { return };
    if held {
        // The strength is the fuel. Burning goes at the bar's refill rate plus the net drain that empties
        // `FULL_DRAIN` of the bar in `CHARGE_TIME` seconds (about 0.82 per second at the usual 0.4 + 0.42). A full bar
        // therefore charges fully in `CHARGE_TIME`; with the bar at zero it burns only what refills,
        // which is slower.
        let rate = p.pill.recover() / player::SWIM_RECOVER + pill * FULL_DRAIN / CHARGE_TIME;
        let burn = (rate * time.delta_secs()).min(p.swim).min((1.0 - c) * rate * CHARGE_TIME);
        p.swim -= burn;
        charge.0 = Some(c + burn / (rate * CHARGE_TIME));
        return;
    }
    // Released: the strength has been burnt already.
    charge.0 = None;
    anim.spit();
    let from = chameleon::neck(tf.translation.truncate(), p.facing) + p.aim * MOUTH;
    commands.spawn((
        Glob { vel: p.aim * (SPEED + (SPEED_MAX - SPEED) * c), life: LIFE },
        Mesh2d(assets.mesh.clone()),
        MeshMaterial2d(assets.mat.clone()),
        Transform::from_translation(from.extend(15.0)).with_rotation(Quat::from_rotation_z(p.aim.to_angle())),
        camera::color_layer(),
    ));
}

/// Feedback while charging: faint pale streaks, only from in front of the head, get sucked into the
/// mouth, fading as the charge fills while chaotic streaks burst outward in all directions, more of
/// them the fuller it is.
fn charge_fx(
    mut commands: Commands,
    time: Res<Time>,
    charge: Res<SpitCharge>,
    assets: Res<SpitAssets>,
    mut seed: Local<u32>,
    mut clocks: Local<[f32; 2]>,
    player: Single<(&Player, &Transform), Without<Streak>>,
    mut streaks: Query<(Entity, &mut Streak, &mut Transform)>,
) {
    let (p, tf) = *player;
    let dt = time.delta_secs();
    let mouth = chameleon::neck(tf.translation.truncate(), p.facing) + p.aim * MOUTH;
    let mut rand = || {
        *seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
        (*seed >> 8) as f32 / (1u32 << 24) as f32
    };
    let place = |tf: &mut Transform, ang: f32, r: f32, len: f32, thick: f32| {
        tf.translation = (mouth + Vec2::from_angle(ang) * r).extend(16.0);
        tf.rotation = Quat::from_rotation_z(ang);
        tf.scale = Vec3::new(len, thick, 1.0);
    };
    let c = charge.0;
    let amount = c.unwrap_or(0.0);
    let pull = 1.0 - amount;
    if c.is_some() {
        clocks[0] += dt * 70.0 * pull;
        clocks[1] += dt * 70.0 * amount;
    }
    while clocks[0] >= 1.0 {
        clocks[0] -= 1.0;
        // Only from the front: a cone around the aim.
        let (ang, r) = (p.aim.to_angle() + (rand() - 0.5) * 1.1, 30.0 + rand() * 16.0);
        let mut t = Transform::default();
        place(&mut t, ang, r, 8.0, 1.3);
        commands.spawn((Streak { ang, r, out: false }, Mesh2d(assets.fx_mesh.clone()), MeshMaterial2d(assets.fx_mat.clone()), t, camera::color_layer()));
    }
    while clocks[1] >= 1.0 {
        clocks[1] -= 1.0;
        let (ang, r) = (rand() * std::f32::consts::TAU, 6.0 + rand() * 8.0);
        let mut t = Transform::default();
        place(&mut t, ang, r, 8.0, 1.3);
        commands.spawn((Streak { ang, r, out: true }, Mesh2d(assets.fx_mesh.clone()), MeshMaterial2d(assets.fx_mat.clone()), t, camera::color_layer()));
    }
    for (e, mut s, mut stf) in &mut streaks {
        let dir = if s.out { 1.0 } else { -1.0 };
        s.r += dir * 150.0 * dt;
        let gone = if s.out { s.r > 40.0 } else { s.r < 5.0 };
        if gone || c.is_none() {
            commands.entity(e).despawn();
            continue;
        }
        // Inward ones shrink as they near the mouth and as the charge fills; outward ones grow with it.
        let k = if s.out { amount } else { pull };
        let len = if s.out { 9.0 } else { (s.r * 0.35).clamp(2.0, 9.0) };
        place(&mut stf, s.ang, s.r, len * k.max(0.2), 1.3 * k);
    }
}

pub fn move_spit(
    mut commands: Commands,
    time: Res<Time>,
    mut bleed: MessageWriter<Bleed>,
    mut hits: MessageWriter<SpitHit>,
    mut globs: Query<(Entity, &mut Glob, &mut Transform)>,
    bosses: Query<(Entity, &Boss, &Transform), Without<Glob>>,
    mobs: Query<(Entity, &Mob, &Transform), (Without<Glob>, Without<crate::terrain_props::Asleep>)>,
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
