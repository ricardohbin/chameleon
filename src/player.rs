//! Player controller: input, gravity and AABB collisions against platforms.

use bevy::prelude::*;

use crate::{camera::Shake, chameleon, level::Platform};

/// Size of the chameleon relative to its drawing (affects visuals and collider).
pub const SCALE: f32 = 0.7;
/// Half extents of the player's collision box (feet are at `-HALF.y`).
pub const HALF: Vec2 = Vec2::new(30.0 * SCALE, 26.0 * SCALE);
pub const SPAWN: Vec2 = Vec2::new(-650.0, 0.0);

const MAX_SPEED: f32 = 240.0;
const GROUND_ACCEL: f32 = 2200.0;
const AIR_ACCEL: f32 = 1300.0;
const GRAVITY: f32 = 2000.0;
const JUMP_SPEED: f32 = 740.0;
const MAX_FALL: f32 = 1400.0;
/// Grace time to still jump after walking off a ledge.
const COYOTE_TIME: f32 = 0.1;
/// A jump pressed slightly before landing still counts.
const JUMP_BUFFER: f32 = 0.12;
/// Landing faster than this shakes the camera.
const HARD_LANDING: f32 = 950.0;
/// Below this height you fell into the abyss.
const VOID_Y: f32 = -700.0;

pub struct PlayerPlugin;

impl Plugin for PlayerPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn_player)
            .add_systems(Update, move_player);
    }
}

#[derive(Component, Default)]
pub struct Player {
    pub vel: Vec2,
    pub grounded: bool,
    /// 1.0 = facing right, -1.0 = facing left.
    pub facing: f32,
    /// Platform we are standing on.
    pub ground: Option<Entity>,
    /// Top color of the platform we last stood on (used for camouflage).
    pub ground_color: Option<Color>,
    /// Last position on stable ground, used to respawn.
    safe_spot: Vec2,
    coyote: f32,
    jump_buffer: f32,
}

fn spawn_player(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
) {
    let player = commands
        .spawn((
            Player { facing: 1.0, safe_spot: SPAWN, ..default() },
            Transform::from_translation(SPAWN.extend(10.0)),
            Visibility::default(),
        ))
        .id();
    chameleon::spawn_chameleon(&mut commands, &mut meshes, &mut materials, player);
}

pub fn move_player(
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    mut shake: ResMut<Shake>,
    mut player: Single<(&mut Player, &mut Transform)>,
    platforms: Query<(Entity, &Platform, &Transform), Without<Player>>,
) {
    let dt = time.delta_secs().min(1.0 / 30.0);
    let (ref mut p, ref mut tf) = *player;

    // --- Horizontal input ---
    let mut dir = 0.0;
    if keys.any_pressed([KeyCode::ArrowLeft, KeyCode::KeyA]) {
        dir -= 1.0;
    }
    if keys.any_pressed([KeyCode::ArrowRight, KeyCode::KeyD]) {
        dir += 1.0;
    }
    if dir != 0.0 {
        p.facing = dir;
    }
    let accel = if p.grounded { GROUND_ACCEL } else { AIR_ACCEL };
    p.vel.x = move_towards(p.vel.x, dir * MAX_SPEED, accel * dt);

    // --- Jumping (with coyote time, buffering and variable height) ---
    let jump_keys = [KeyCode::Space, KeyCode::KeyW, KeyCode::ArrowUp];
    p.coyote = if p.grounded { COYOTE_TIME } else { p.coyote - dt };
    p.jump_buffer = if keys.any_just_pressed(jump_keys) { JUMP_BUFFER } else { p.jump_buffer - dt };
    if p.jump_buffer > 0.0 && p.coyote > 0.0 {
        p.vel.y = JUMP_SPEED;
        p.jump_buffer = 0.0;
        p.coyote = 0.0;
        p.grounded = false;
        p.ground = None;
    }
    if keys.any_just_released(jump_keys) && p.vel.y > 0.0 {
        p.vel.y *= 0.5;
    }
    p.vel.y = (p.vel.y - GRAVITY * dt).max(-MAX_FALL);

    let mut pos = tf.translation.truncate();

    // Ride whatever we stand on (swinging girders, sinking plates, shaking slabs).
    if let Some((_, plat, _)) = p.ground.and_then(|e| platforms.get(e).ok()) {
        if plat.solid {
            pos += plat.delta;
        }
    }

    // --- Move & collide, one axis at a time ---
    pos.x += p.vel.x * dt;
    for (_, plat, ptf) in &platforms {
        if plat.one_way || !plat.solid {
            continue;
        }
        let c = ptf.translation.truncate();
        if overlaps(pos, c, plat.half) {
            pos.x = if pos.x < c.x { c.x - plat.half.x - HALF.x } else { c.x + plat.half.x + HALF.x };
            p.vel.x = 0.0;
        }
    }

    let was_grounded = p.grounded;
    let fall_speed = -p.vel.y;
    let prev_bottom = pos.y - HALF.y;
    pos.y += p.vel.y * dt;
    p.grounded = false;
    p.ground = None;
    for (entity, plat, ptf) in &platforms {
        let c = ptf.translation.truncate();
        if !plat.solid || !overlaps(pos, c, plat.half) {
            continue;
        }
        let top = c.y + plat.half.y;
        // A little tolerance so moving platforms don't drop us.
        if p.vel.y <= 0.0 && prev_bottom >= top - 6.0 {
            pos.y = top + HALF.y;
            p.vel.y = 0.0;
            p.grounded = true;
            p.ground = Some(entity);
            p.ground_color = Some(plat.color);
            if plat.stable {
                let margin = (plat.half.x - HALF.x).max(0.0);
                p.safe_spot = Vec2::new(pos.x.clamp(c.x - margin, c.x + margin), top + HALF.y);
            }
        } else if !plat.one_way && p.vel.y > 0.0 {
            pos.y = c.y - plat.half.y - HALF.y;
            p.vel.y = 0.0;
        }
    }
    if p.grounded && !was_grounded && fall_speed > HARD_LANDING {
        shake.add(0.3);
    }

    // Fell into the abyss: back to the last safe ground.
    if pos.y < VOID_Y {
        pos = p.safe_spot + Vec2::Y * 30.0;
        p.vel = Vec2::ZERO;
        shake.add(0.4);
    }

    tf.translation.x = pos.x;
    tf.translation.y = pos.y;
}

fn overlaps(pos: Vec2, center: Vec2, half: Vec2) -> bool {
    let d = (pos - center).abs();
    d.x < HALF.x + half.x && d.y < HALF.y + half.y
}

fn move_towards(current: f32, target: f32, max_delta: f32) -> f32 {
    current + (target - current).clamp(-max_delta, max_delta)
}
