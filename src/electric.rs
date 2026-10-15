//! The electrified tracks ("feature: … tracks / third rail" slices in a subway
//! area; the rails themselves are drawn by `terrain_props/subway.rs`). The third
//! rail is still live: standing on the track bed (low enough to touch it) shocks
//! the chameleon every `SHOCK_EVERY` seconds, one HP each, with a burst of
//! sparks and a twitch instead of a knockback (`Player::shock`), so the hits come
//! in quick succession until it leaves the rails. Near the rail, stray arcs
//! snap off it now and then.

use std::collections::HashMap;

use bevy::{prelude::*, sprite_render::AlphaMode2d};

use crate::{
    camera::{self, HitStop, MainCamera, Shake},
    gore::{Bleed, Fluid},
    level::{self, Restart},
    mobs::{Mob, State},
    player::{self, Hurt, Player},
    shapes::*,
    terrain::{Style, Terrain, PX},
};

/// Seconds between shocks while touching the rail.
const SHOCK_EVERY: f32 = 0.4;
/// How high above the floor the live rail and its cover reach (world units).
const RAIL_TOP: f32 = 26.0;
/// Stray arcs snap within this distance of the camera along the track.
const VIEW: f32 = 1000.0;
const GRAVITY: f32 = 1500.0;
const Z: f32 = 14.0;

pub struct ElectricPlugin;

impl Plugin for ElectricPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, load_sparks)
            .add_systems(PreUpdate, clear_sparks.after(level::restart))
            .add_systems(Update, (shock, shock_mobs, arcs, fly_sparks).chain().after(player::move_player));
    }
}

#[derive(Resource)]
struct SparkAssets {
    mesh: Handle<Mesh>,
    mats: [Handle<ColorMaterial>; 3],
    rng: Rng,
    /// Seconds until the next shock.
    cooldown: f32,
    /// Fractional sparks / arcs owed.
    due: f32,
    arc: f32,
}

#[derive(Component)]
struct Spark {
    vel: Vec2,
    age: f32,
    life: f32,
    size: f32,
}

fn load_sparks(mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>, mut materials: ResMut<Assets<ColorMaterial>>) {
    let mesh = meshes.add(fill(&[Vec2::new(-0.18, 0.0), Vec2::new(0.18, 0.0), Vec2::new(0.0, 1.0)], |_| 1.0));
    let mut mat = |c: Color| materials.add(ColorMaterial { color: c, alpha_mode: AlphaMode2d::Blend, ..default() });
    let mats = [mat(Color::srgb(1.0, 0.97, 0.8)), mat(Color::srgb(0.75, 0.88, 1.0)), mat(Color::srgb(1.0, 0.85, 0.35))];
    commands.insert_resource(SparkAssets { mesh, mats, rng: Rng(0x5ba2c), cooldown: 0.0, due: 0.0, arc: 0.0 });
}

/// F8: sparks still flying go.
fn clear_sparks(mut commands: Commands, mut restarted: MessageReader<Restart>, sparks: Query<Entity, With<Spark>>, mut assets: ResMut<SparkAssets>) {
    if restarted.read().count() == 0 {
        return;
    }
    assets.cooldown = 0.0;
    for e in &sparks {
        commands.entity(e).despawn();
    }
}

/// Burst `n` sparks from `at`, mostly up and out.
fn burst(commands: &mut Commands, assets: &mut SparkAssets, at: Vec2, n: usize, speed: f32) {
    for _ in 0..n {
        let a = assets.rng.range(0.1, std::f32::consts::PI - 0.1);
        let v = Vec2::from_angle(a) * assets.rng.range(0.3, 1.0) * speed;
        let size = assets.rng.range(10.0, 26.0);
        let mat = assets.mats[assets.rng.below(assets.mats.len())].clone();
        commands.spawn((
            Mesh2d(assets.mesh.clone()),
            MeshMaterial2d(mat),
            Transform::from_translation((at + Vec2::new(assets.rng.range(-6.0, 6.0), 0.0)).extend(Z)),
            camera::color_layer(),
            Spark { vel: v, age: 0.0, life: assets.rng.range(0.18, 0.5), size },
        ));
    }
}

/// Touching the live rail shocks every `SHOCK_EVERY` seconds; sparks fly meanwhile.
fn shock(
    mut commands: Commands,
    time: Res<Time>,
    terrain: Res<Terrain>,
    mut assets: ResMut<SparkAssets>,
    mut shake: ResMut<Shake>,
    mut stop: ResMut<HitStop>,
    mut bleed: MessageWriter<Bleed>,
    player: Single<(&mut Player, &Transform)>,
) {
    let dt = time.delta_secs();
    let (mut p, tf) = player.into_inner();
    let ppos = tf.translation.truncate();
    let feet = ppos - Vec2::Y * player::HALF.y;
    assets.cooldown = (assets.cooldown - dt).max(0.0);
    if p.dead.is_some() {
        return;
    }
    let live = |q: Vec2| terrain.features_at(q).any(|f| f.electrified && terrain.style_at(f.rect.center()) == Style::Subway);
    // The floor under the chameleon, where a live rail could run.
    let floor = terrain.surface(feet + Vec2::Y * PX, 12.0 * PX);
    let Some(floor) = floor.filter(|_| live(feet + Vec2::Y * PX)) else {
        assets.due = 0.0;
        return;
    };
    let touching = feet.y - floor < RAIL_TOP;
    if touching {
        if assets.cooldown <= 0.0 {
            match p.shock(1) {
                Hurt::Ignored => {}
                outcome => {
                    let killed = matches!(outcome, Hurt::Killed);
                    assets.cooldown = SHOCK_EVERY;
                    shake.add(if killed { 0.7 } else { 0.22 });
                    stop.add(if killed { 0.25 } else { 0.03 });
                    bleed.write(Bleed {
                        pos: ppos,
                        dir: Vec2::Y,
                        spread: if killed { std::f32::consts::PI } else { 0.9 },
                        speed: if killed { 520.0 } else { 260.0 },
                        amount: if killed { 120 } else { 10 },
                        fluid: Fluid::Red,
                        sticky: false,
                    });
                    let a = &mut *assets;
                    burst(&mut commands, a, Vec2::new(feet.x, floor + 18.0), 18, 520.0);
                }
            }
        }
        // Crackling around the claws all along.
        assets.due += 22.0 * dt;
        while assets.due >= 1.0 {
            assets.due -= 1.0;
            let a = &mut *assets;
            let at = Vec2::new(feet.x + a.rng.range(-player::HALF.x, player::HALF.x), floor + 18.0);
            burst(&mut commands, a, at, 1, 380.0);
        }
    } else {
        assets.due = 0.0;
    }
}

/// Where the live rail runs at `x` inside `rect`: the floor height there, if there is track.
fn rail_at(terrain: &Terrain, rect: Rect, x: f32) -> Option<f32> {
    terrain
        .surface(Vec2::new(x, rect.center().y), rect.height() + 6.0 * PX)
        .filter(|&y| y > rect.min.y - 4.0 * PX && y < rect.max.y + 4.0 * PX)
}

/// Whether the player or the camera is anywhere about: stray arcs snap off the
/// rails on their own, touched or not, wherever the camera can see them.
fn arcs(
    mut commands: Commands,
    time: Res<Time>,
    terrain: Res<Terrain>,
    mut assets: ResMut<SparkAssets>,
    camera: Single<&Transform, With<MainCamera>>,
) {
    let eye = camera.translation.truncate();
    assets.arc += 2.5 * time.delta_secs();
    while assets.arc >= 1.0 {
        assets.arc -= 1.0;
        let a = &mut *assets;
        let seen: Vec<Rect> = terrain
            .features
            .iter()
            .filter(|f| f.electrified && f.rect.min.x < eye.x + VIEW && f.rect.max.x > eye.x - VIEW && terrain.style_at(f.rect.center()) == Style::Subway)
            .map(|f| f.rect)
            .collect();
        if seen.is_empty() {
            return;
        }
        let rect = seen[a.rng.below(seen.len())];
        let x = a.rng.range(rect.min.x.max(eye.x - VIEW), rect.max.x.min(eye.x + VIEW));
        let Some(y) = rail_at(&terrain, rect, x) else { continue };
        // Mostly a small snap; now and then a crackling run of them.
        let (n, reps) = if a.rng.chance(0.25) { (6 + a.rng.below(6), 2 + a.rng.below(3)) } else { (3 + a.rng.below(4), 1) };
        for k in 0..reps {
            let at = Vec2::new(x + k as f32 * a.rng.range(-14.0, 14.0), y + 18.0);
            burst(&mut commands, a, at, n, 280.0);
        }
    }
}

/// Anything else that touches the live rail gets the same jolts: ground mobs
/// flinch every `SHOCK_EVERY` seconds and burn out if they stay.
fn shock_mobs(
    mut commands: Commands,
    time: Res<Time>,
    terrain: Res<Terrain>,
    mut assets: ResMut<SparkAssets>,
    mut bleed: MessageWriter<Bleed>,
    mut zapped: Local<HashMap<Entity, f32>>,
    mut mobs: Query<(Entity, &mut Mob, &Transform)>,
) {
    let dt = time.delta_secs();
    zapped.retain(|e, t| {
        *t -= dt;
        *t > 0.0 && mobs.contains(*e)
    });
    for (e, mut m, tf) in &mut mobs {
        if matches!(m.state, State::Dead { .. }) || zapped.contains_key(&e) {
            continue;
        }
        let pos = tf.translation.truncate();
        let feet = pos - Vec2::Y * m.spec.foot;
        let probe = feet + Vec2::Y * PX;
        let Some(f) = terrain.features_at(probe).find(|f| f.electrified && terrain.style_at(f.rect.center()) == Style::Subway) else { continue };
        let Some(floor) = rail_at(&terrain, f.rect, feet.x) else { continue };
        if feet.y - floor >= RAIL_TOP {
            continue;
        }
        zapped.insert(e, SHOCK_EVERY);
        m.hp -= 1;
        m.flash = 1.0;
        let a = &mut *assets;
        burst(&mut commands, a, Vec2::new(feet.x, floor + 18.0), 14, 460.0);
        let gush = Bleed { pos, dir: Vec2::Y, spread: 0.9, speed: 260.0, amount: 10, fluid: m.spec.blood, sticky: false };
        if m.hp <= 0 {
            bleed.write(Bleed { spread: std::f32::consts::PI, speed: 560.0, amount: 60, ..gush });
            m.state = State::Dead { t: 0.0, landed: false };
            m.vel = Vec2::new(0.0, 320.0);
            m.dismember = true;
        } else {
            bleed.write(gush);
            if !matches!(m.state, State::Attack(_) | State::Windup(_)) || !m.spec.stubborn {
                m.vel = Vec2::new(m.vel.x * 0.3, 0.0);
                m.state = State::Hurt(0.0);
            }
        }
    }
}

/// Sparks arc out, fall and burn out.
fn fly_sparks(mut commands: Commands, time: Res<Time>, mut sparks: Query<(Entity, &mut Spark, &mut Transform)>) {
    let dt = time.delta_secs().min(1.0 / 30.0);
    for (e, mut s, mut tf) in &mut sparks {
        s.age += dt;
        if s.age >= s.life {
            commands.entity(e).despawn();
            continue;
        }
        s.vel.y -= GRAVITY * dt;
        tf.translation += (s.vel * dt).extend(0.0);
        // Pointing along the flight, shrinking as it dies.
        tf.rotation = Quat::from_rotation_z(s.vel.to_angle() - std::f32::consts::FRAC_PI_2);
        let fade = 1.0 - s.age / s.life;
        tf.scale = Vec3::new(s.size, s.size * (0.4 + 0.6 * fade), 1.0);
    }
}
