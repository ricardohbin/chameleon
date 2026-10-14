//! Unstable terrain: crumbling slabs, swinging girders, sinking plates
//! and periodic earthquakes that shake everything loose.

use bevy::prelude::*;

use crate::{
    camera::Shake,
    level::Platform,
    player::{self, Player},
    shapes::Rng,
};

const CRUMBLE_DELAY: f32 = 0.6;
const RESPAWN_DELAY: f32 = 3.0;
const SINK_DEPTH: f32 = 70.0;

pub struct UnstablePlugin;

impl Plugin for UnstablePlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(Tremor { next: 10.0, left: 0.0, rng: Rng(0xBADC0DE) })
            .add_systems(Update, (move_platforms, update_chains).chain().before(player::move_player));
    }
}

#[derive(Component)]
pub enum Motion {
    Crumble { origin: Option<Vec2>, state: CrumbleState },
    Swing { anchor: Vec2, len: f32, amp: f32, speed: f32, phase: f32 },
    Sink { origin: Vec2, offset: f32 },
}

impl Motion {
    pub fn crumble() -> Self {
        Motion::Crumble { origin: None, state: CrumbleState::Idle }
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

/// Earthquakes: every so often the ground rumbles and slabs start to break.
#[derive(Resource)]
struct Tremor {
    next: f32,
    left: f32,
    rng: Rng,
}

fn move_platforms(
    time: Res<Time>,
    mut tremor: ResMut<Tremor>,
    mut shake: ResMut<Shake>,
    player: Single<(&Player, &Transform)>,
    mut platforms: Query<(Entity, &mut Platform, &mut Motion, &mut Transform, &mut Visibility), Without<Player>>,
) {
    let dt = time.delta_secs().min(1.0 / 30.0);
    let t = time.elapsed_secs();
    let (p, ptf) = *player;

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
        let standing = p.ground == Some(e);
        let mut new = old;

        match &mut *motion {
            Motion::Crumble { origin, state } => {
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
                        if s > CRUMBLE_DELAY {
                            plat.solid = false;
                            shake.add(0.12);
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
                        tf.rotation = Quat::IDENTITY;
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
