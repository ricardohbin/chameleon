//! Unstable terrain: crumbling slabs (found on the terrain by `level::loose`),
//! swinging girders, sinking plates and periodic earthquakes that shake
//! everything loose.

use bevy::prelude::*;

use crate::{
    camera::Shake,
    level::{self, Platform, Restart},
    player::{self, Player, HALF},
    shapes::{fill, rock_pts, scale_color, Rng},
    terrain::Terrain,
    terrain_art::TerrainPart,
};

const RESPAWN_DELAY: f32 = 3.0;
const SINK_DEPTH: f32 = 70.0;

pub struct UnstablePlugin;

impl Plugin for UnstablePlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(Tremor::default())
            .add_systems(
                PreUpdate,
                (|mut r: MessageReader<Restart>, mut tremor: ResMut<Tremor>| {
                    if r.read().count() > 0 {
                        *tremor = Tremor::default();
                    }
                })
                .after(level::restart),
            )
            .add_systems(Update, (move_platforms, update_chains).chain().before(player::move_player))
            .add_systems(Update, (show_remnants, fall_chunks));
    }
}

#[derive(Component)]
pub enum Motion {
    Crumble { origin: Option<Vec2>, state: CrumbleState, delay: f32 },
    Swing { anchor: Vec2, len: f32, amp: f32, speed: f32, phase: f32 },
    Sink { origin: Vec2, offset: f32 },
}

impl Motion {
    /// Breaks `delay` seconds after the chameleon steps on it.
    pub fn crumble_in(delay: f32) -> Self {
        Motion::Crumble { origin: None, state: CrumbleState::Idle, delay }
    }
}

#[derive(Clone, Copy)]
pub enum CrumbleState {
    Idle,
    Shaking(f32),
    Falling { t: f32, vel: f32 },
    Gone(f32),
}

/// A chain from a fixed anchor to a point on a (moving) platform.
#[derive(Component)]
pub struct Chain {
    pub platform: Entity,
    pub anchor: Vec2,
    /// Attachment point relative to the platform center.
    pub attach: Vec2,
}

/// What a collapsed platform leaves on the ground it was fixed to (broken
/// concrete, rebar, bent steel...): shown once the platform is falling or gone.
#[derive(Component)]
pub struct Remnant {
    pub owner: Entity,
}

/// A piece of a platform breaking away.
#[derive(Component)]
struct Chunk {
    vel: Vec2,
    spin: f32,
    age: f32,
    landed: bool,
}

/// Earthquakes: every so often the ground rumbles and slabs start to break.
#[derive(Resource)]
struct Tremor {
    next: f32,
    left: f32,
    rng: Rng,
}

impl Default for Tremor {
    fn default() -> Self {
        Tremor { next: 10.0, left: 0.0, rng: Rng(0xBADC0DE) }
    }
}

fn move_platforms(
    time: Res<Time>,
    mut tremor: ResMut<Tremor>,
    party: Res<crate::boss::Celebration>,
    mut shake: ResMut<Shake>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    player: Single<(&Player, &Transform)>,
    mut platforms: Query<(Entity, &mut Platform, &mut Motion, &mut Transform, &mut Visibility), Without<Player>>,
) {
    let dt = time.delta_secs().min(1.0 / 30.0);
    let t = time.elapsed_secs();
    let (p, ptf) = *player;

    // After the ending the ground is at peace: no more earthquakes.
    if party.0 {
        tremor.left = 0.0;
        tremor.next = tremor.next.max(5.0);
    }
    tremor.next -= dt;
    if tremor.next <= 0.0 {
        tremor.left = tremor.rng.range(1.2, 2.2);
        tremor.next = tremor.rng.range(9.0, 16.0);
    }
    let quaking = tremor.left > 0.0;
    if quaking {
        tremor.left -= dt;
        shake.at_least(0.55);
    }
    let rng = &mut tremor.rng;

    for (e, mut plat, mut motion, mut tf, mut vis) in &mut platforms {
        let old = tf.translation.truncate();
        // Standing *on* it: feet on its top with the body over it (brushing its edge is not enough).
        let standing = p.ground == Some(e) && (ptf.translation.x - old.x).abs() < plat.half.x + HALF.x * 0.3;
        let mut new = old;

        match &mut *motion {
            Motion::Crumble { origin, state, delay } => {
                let origin = *origin.get_or_insert(old);
                *state = match *state {
                    CrumbleState::Idle => {
                        let near = (origin - ptf.translation.truncate()).length() < 700.0;
                        if standing || (quaking && near && rng.chance(dt * 0.8)) {
                            CrumbleState::Shaking(0.0)
                        } else {
                            CrumbleState::Idle
                        }
                    }
                    CrumbleState::Shaking(s) => {
                        let s = s + dt;
                        let a = 0.8 + s * 4.0;
                        new = origin + Vec2::new(rng.range(-a, a), rng.range(-a, a) * 0.4);
                        if s > *delay {
                            plat.solid = false;
                            shake.add(0.12);
                            shed(&mut commands, &mut meshes, &mut materials, rng, old, plat.half, plat.color);
                            CrumbleState::Falling { t: 0.0, vel: 0.0 }
                        } else {
                            CrumbleState::Shaking(s)
                        }
                    }
                    CrumbleState::Falling { t, vel } => {
                        let vel = vel + 1600.0 * dt;
                        new.y -= vel * dt;
                        let spin = if e.index_u32() % 2 == 0 { 1.0 } else { -1.0 };
                        tf.rotate_z(spin * dt * 1.2);
                        if t > 1.6 {
                            *vis = Visibility::Hidden;
                            CrumbleState::Gone(0.0)
                        } else {
                            CrumbleState::Falling { t: t + dt, vel }
                        }
                    }
                    CrumbleState::Gone(g) if g > RESPAWN_DELAY => {
                        new = origin;
                        tf.rotation = Quat::from_rotation_z(plat.tilt);
                        plat.solid = true;
                        *vis = Visibility::Inherited;
                        CrumbleState::Idle
                    }
                    CrumbleState::Gone(g) => CrumbleState::Gone(g + dt),
                };
            }
            Motion::Swing { anchor, len, amp, speed, phase } => {
                let quake = if quaking { 1.8 } else { 1.0 };
                let a = *amp * quake * (t * *speed + *phase).sin();
                new = *anchor + Vec2::new(a.sin(), -a.cos()) * *len;
            }
            Motion::Sink { origin, offset } => {
                let (target, rate) = if standing { (-SINK_DEPTH, 35.0) } else { (0.0, 20.0) };
                *offset += (target - *offset).clamp(-rate * dt, rate * dt);
                new = *origin + Vec2::Y * *offset;
                if quaking {
                    new += Vec2::new(rng.range(-1.5, 1.5), rng.range(-1.5, 1.5));
                }
                let wobble = (t * 2.5 + origin.x).sin() * 0.03 * (*offset / -SINK_DEPTH);
                tf.rotation = Quat::from_rotation_z(wobble);
            }
        }

        // A teleport back to the origin isn't motion the player should follow.
        plat.delta = if plat.solid { new - old } else { Vec2::ZERO };
        tf.translation = new.extend(tf.translation.z);
    }
}

/// Bits of the platform breaking off as it gives way.
fn shed(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<ColorMaterial>,
    rng: &mut Rng,
    at: Vec2,
    half: Vec2,
    color: Color,
) {
    let n = (half.x / 5.0).clamp(4.0, 10.0) as i32;
    for _ in 0..n {
        let size = rng.range(2.5, 6.5);
        let mesh = meshes.add(fill(&rock_pts(rng, size, 6), |p| 0.7 + 0.3 * (p.y / size)));
        let mat = materials.add(scale_color(color, rng.range(0.55, 1.0)));
        let pos = at + Vec2::new(rng.range(-half.x, half.x), rng.range(-half.y, half.y));
        let vel = Vec2::new(rng.range(-80.0, 80.0), rng.range(30.0, 190.0));
        commands.spawn((
            Mesh2d(mesh),
            MeshMaterial2d(mat),
            Transform::from_translation(pos.extend(0.3)),
            Chunk { vel, spin: rng.range(-9.0, 9.0), age: 0.0, landed: false },
            TerrainPart,
        ));
    }
}

fn fall_chunks(
    time: Res<Time>,
    terrain: Res<Terrain>,
    mut commands: Commands,
    mut chunks: Query<(Entity, &mut Chunk, &mut Transform)>,
) {
    let dt = time.delta_secs().min(1.0 / 30.0);
    for (e, mut c, mut tf) in &mut chunks {
        c.age += dt;
        if !c.landed {
            c.vel.y -= 1600.0 * dt;
            tf.translation += (c.vel * dt).extend(0.0);
            tf.rotate_z(c.spin * dt);
            if c.vel.y < 0.0 && terrain.solid_at(tf.translation.truncate() - Vec2::Y * 2.0) {
                c.landed = true;
            }
        }
        if c.age > if c.landed { 3.0 } else { 2.2 } {
            commands.entity(e).try_despawn();
        }
    }
}

/// The broken contact points show once the platform is coming down, and hide again when it re-forms.
fn show_remnants(motions: Query<&Motion>, mut remnants: Query<(&Remnant, &mut Visibility)>) {
    for (r, mut vis) in &mut remnants {
        let broken = matches!(
            motions.get(r.owner),
            Ok(Motion::Crumble { state: CrumbleState::Falling { .. } | CrumbleState::Gone(_), .. })
        );
        let want = if broken { Visibility::Inherited } else { Visibility::Hidden };
        if *vis != want {
            *vis = want;
        }
    }
}

fn update_chains(
    platforms: Query<&Transform, (With<Platform>, Without<Chain>)>,
    mut chains: Query<(&Chain, &mut Transform), Without<Platform>>,
) {
    for (chain, mut tf) in &mut chains {
        let Ok(ptf) = platforms.get(chain.platform) else { continue };
        let end = ptf.translation.truncate() + chain.attach;
        let d = end - chain.anchor;
        tf.translation = ((chain.anchor + end) / 2.0).extend(tf.translation.z);
        tf.rotation = Quat::from_rotation_z(d.to_angle() - std::f32::consts::FRAC_PI_2);
        tf.scale.y = d.length();
    }
}
