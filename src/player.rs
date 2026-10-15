//! Player controller: input, gravity, collisions against the terrain grid
//! (walking up ramps and small steps) and the moving platforms.

use bevy::prelude::*;

use crate::{
    boss::Defeated,
    camera::Shake,
    chameleon,
    level::{self, Platform, Restart},
    terrain::{Terrain, FLOOR_HEIGHT, STEP_UP},
    terrain_art,
};

/// Size of the chameleon relative to its drawing (affects visuals and collider).
pub const SCALE: f32 = 0.7;
/// Half extents of the player's collision box (feet are at `-HALF.y`).
pub const HALF: Vec2 = Vec2::new(30.0 * SCALE, 26.0 * SCALE);
/// Starting max health (each boss carcass eaten adds a tick, see `feeding.rs`).
pub const MAX_HP: i32 = 5;
/// Invulnerable (and blinking) for this long after a hit.
const INVULN: f32 = 1.0;
/// Seconds lying dead before respawning.
const DEATH_TIME: f32 = 2.8;
/// Seconds to digest one tick of shadow health (food eaten) into a real one.
const DIGEST_TIME: f32 = 2.5;

const MAX_SPEED: f32 = 240.0;
/// On slippery slopes (slices marked "slippery" / "sliding": loose trash that
/// slides down with you, worse than solid ground): walking gets hard from
/// `SLIPPERY_WALK` (~22°), you slide from `SLIPPERY_SLOPE` (45°), and below
/// that the ground still carries you downhill at `SLIPPERY_DRIFT` per unit of slope.
const SLIPPERY_WALK: f32 = 0.4;
const SLIPPERY_SLOPE: f32 = 1.0;
const SLIPPERY_DRIFT: f32 = 90.0;
/// Dash (earned by beating the Gutter King): a burst along the way you're
/// walking (else the facing), gravity off, once per time in the air.
/// (Jump + dash crosses ~55 sketch px: "feature: … gap that requires dash".)
const DASH_SPEED: f32 = 1100.0;
const DASH_TIME: f32 = 0.25;
const DASH_COOLDOWN: f32 = 0.45;
const DASH_KEYS: [KeyCode; 3] = [KeyCode::ShiftLeft, KeyCode::ShiftRight, KeyCode::KeyK];
/// Anywhere else, slopes (rise per run) up to `WALK_SLOPE` (45°) are walked
/// normally; above it, walking uphill gets harder and the slope pulls you
/// down, until `STEEP_SLOPE` (60°), where sliding is unavoidable. Jumps tilt
/// toward the slope's normal over the same range (straight away from it at 60°).
const WALK_SLOPE: f32 = 1.15;
const STEEP_SLOPE: f32 = 1.85;
/// How hard a slope at `STEEP_SLOPE` pulls you downhill (scaled by hardness²,
/// so uphill crawls to a stop just short of it).
const SLOPE_PULL: f32 = 120.0;
const SLIDE_SPEED: f32 = 520.0;
const SLIDE_ACCEL: f32 = 1600.0;
const GROUND_ACCEL: f32 = 2200.0;
const AIR_ACCEL: f32 = 1300.0;
const GRAVITY: f32 = 2000.0;
const JUMP_SPEED: f32 = 740.0;
const FLIGHT_ACCEL: f32 = 2000.0;
const FLIGHT_SPEED: f32 = 700.0;
const FLIGHT_BRAKE: f32 = 10.0;
const MAX_FALL: f32 = 1400.0;
/// Grace time to still jump after walking off a ledge.
const COYOTE_TIME: f32 = 0.1;
/// A jump pressed slightly before landing still counts.
const JUMP_BUFFER: f32 = 0.12;
/// Landing faster than this shakes the camera.
const HARD_LANDING: f32 = 950.0;
/// Falls shorter than this many storeys (`FLOOR_HEIGHT`) are harmless; from there
/// on it costs 1 HP, plus 1 more for every further storey.
const FALL_SAFE_FLOORS: f32 = 2.0;
/// This far below the lowest ground you fell into the abyss.
const VOID_DEPTH: f32 = 600.0;

pub struct PlayerPlugin;

impl Plugin for PlayerPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<Respawned>()
            .add_systems(Startup, spawn_player)
            .add_systems(PreUpdate, restart.after(level::restart))
            .add_systems(Update, (toggle_god, move_player, digest));
    }
}

#[derive(Component, Default)]
pub struct Player {
    pub vel: Vec2,
    pub grounded: bool,
    /// 1.0 = facing right, -1.0 = facing left: the body faces where it walks
    /// (the head turns to the mouse, see `chameleon.rs`).
    pub facing: f32,
    /// Where the mouse points in the world, once it has been over the window.
    pub aim_at: Option<Vec2>,
    /// Unit direction from the neck (`chameleon::neck`) to `aim_at` (else the facing).
    pub aim: Vec2,
    /// Platform we are standing on.
    pub ground: Option<Entity>,
    /// Top color of the platform we last stood on (used for camouflage).
    pub ground_color: Option<Color>,
    /// Seconds left without control after being hit.
    pub stun: f32,
    pub hp: i32,
    /// Current max health: `MAX_HP` plus one per boss carcass eaten (kept through deaths).
    pub max_hp: i32,
    /// Shadow health: ticks promised by remains eaten (`feeding.rs`), not yet
    /// health. They fill one at a time; `hp + shadow` never passes `max_hp`.
    pub shadow: f32,
    /// 0..1: how much of the next shadow tick has filled (it only counts once full).
    pub digest: f32,
    /// Seconds left of post-hit invulnerability.
    pub invuln: f32,
    /// F3 invulnerability: no damage, no knockback (enemies pass right through).
    pub god: bool,
    /// F4: gravity-free movement with mouse-directed jump impulses.
    pub flying: bool,
    /// Seconds since death, `None` while alive.
    pub dead: Option<f32>,
    /// Visual offset easing out the hop of climbing (or dropping) a ramp step.
    pub step: f32,
    /// Seconds left of sliding momentum (kept through the hops down a steep slope).
    pub slipping: f32,
    /// Seconds left of the current dash.
    pub dash: f32,
    dash_ready: bool,
    dash_cooldown: f32,
    /// Which way the current dash goes.
    dash_dir: f32,
    /// Highest the feet have been since last touching ground (fall damage).
    fall_top: f32,
    /// Last position on stable ground, used to respawn.
    safe_spot: Vec2,
    coyote: f32,
    jump_buffer: f32,
}

/// Sent when the chameleon comes back after dying (enemies reset).
#[derive(Message)]
pub struct Respawned;

pub enum Hurt {
    Ignored,
    Hurt,
    Killed,
}

impl Player {
    /// Take `damage` and get knocked back with velocity `knock`.
    pub fn hurt(&mut self, damage: i32, knock: Vec2) -> Hurt {
        if self.dead.is_some() || self.invuln > 0.0 || self.god {
            return Hurt::Ignored;
        }
        self.hp = (self.hp - damage).max(0);
        // The tick being digested is lost whole; the rest start filling again.
        self.shadow = (self.shadow - 1.0).max(0.0);
        self.digest = 0.0;
        self.vel = knock;
        self.stun = 0.45;
        self.invuln = INVULN;
        if self.hp == 0 {
            self.dead = Some(0.0);
            self.shadow = 0.0;
            Hurt::Killed
        } else {
            Hurt::Hurt
        }
    }

    /// Put the chameleon back on the ground at `pos` (a loaded game): also its respawn spot.
    pub fn settle_at(&mut self, pos: Vec2) {
        self.safe_spot = pos;
        self.forget_fall(pos);
    }

    /// Teleported to `pos`: the height fallen from before doesn't count.
    pub fn forget_fall(&mut self, pos: Vec2) {
        self.fall_top = pos.y - HALF.y;
    }

    /// How much shadow health still fits under `max_hp`.
    pub fn hunger(&self) -> f32 {
        ((self.max_hp - self.hp) as f32 - self.shadow).max(0.0)
    }

    /// Eat `food` ticks of shadow health (what doesn't fit is wasted).
    pub fn feed(&mut self, food: f32) {
        self.shadow += food.min(self.hunger());
    }
}

/// F3: toggle invulnerability.
fn toggle_god(keys: Res<ButtonInput<KeyCode>>, mut player: Single<&mut Player>) {
    if keys.just_pressed(KeyCode::F3) {
        player.god = !player.god;
        info!("invulnerability {}", if player.god { "on" } else { "off" });
    }
}

/// Shadow health fills slowly, one tick at a time; a full tick becomes health.
/// A last partial tick (less than one promised) fills up to what was promised
/// and waits there for more food.
fn digest(time: Res<Time>, mut player: Single<&mut Player>) {
    let p = &mut **player;
    p.shadow = p.shadow.min((p.max_hp - p.hp).max(0) as f32);
    if p.dead.is_some() || p.shadow <= 0.0 {
        p.digest = 0.0;
        return;
    }
    p.digest = (p.digest + time.delta_secs() / DIGEST_TIME).min(p.shadow.min(1.0));
    if p.digest >= 1.0 - 1e-4 {
        p.hp += 1;
        p.shadow = (p.shadow - 1.0).max(0.0);
        p.digest = 0.0;
    }
}

/// Where the chameleon (re)spawns: on the red marker of the sketch.
pub fn spawn_point(terrain: &Terrain) -> Vec2 {
    terrain.spawn + Vec2::Y * HALF.y
}

fn spawn_player(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    terrain: Res<Terrain>,
) {
    spawn(&mut commands, &mut meshes, &mut materials, &terrain, false);
}

/// F8: a brand new chameleon on the spawn marker (health, max health, earned
/// skin, blood on it, all gone). Invulnerability is a dev toggle: it stays.
fn restart(
    mut commands: Commands,
    mut restarted: MessageReader<Restart>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    terrain: Res<Terrain>,
    player: Single<(Entity, &Player)>,
) {
    if restarted.read().count() == 0 {
        return;
    }
    let (e, p) = *player;
    commands.entity(e).despawn();
    spawn(&mut commands, &mut meshes, &mut materials, &terrain, p.god);
}

fn spawn(commands: &mut Commands, meshes: &mut Assets<Mesh>, materials: &mut Assets<ColorMaterial>, terrain: &Terrain, god: bool) {
    let spawn = spawn_point(terrain);
    let player = commands
        .spawn((
            Player { facing: 1.0, safe_spot: spawn, hp: MAX_HP, max_hp: MAX_HP, god, ..default() },
            Transform::from_translation(spawn.extend(10.0)),
            Visibility::default(),
        ))
        .id();
    chameleon::spawn_chameleon(commands, meshes, materials, player);
}

pub fn move_player(
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    mut shake: ResMut<Shake>,
    mut respawned: MessageWriter<Respawned>,
    mut player: Single<(&mut Player, &mut Transform)>,
    platforms: Query<(Entity, &Platform, &Transform), Without<Player>>,
    terrain: Res<Terrain>,
    window: Single<&Window>,
    camera: Single<(&Camera, &GlobalTransform), With<crate::camera::MainCamera>>,
    defeated: Res<Defeated>,
) {
    let dt = time.delta_secs().min(1.0 / 30.0);
    let (ref mut p, ref mut tf) = *player;
    let (camera, camera_tf) = *camera;

    // --- Aim: the mouse, in the world ---
    if let Some(target) = window.cursor_position().and_then(|c| camera.viewport_to_world_2d(camera_tf, c).ok()) {
        p.aim_at = Some(target);
    }
    let neck = chameleon::neck(tf.translation.truncate(), p.facing);
    p.aim = p.aim_at.map_or(Vec2::ZERO, |at| at - neck).normalize_or(Vec2::new(p.facing, 0.0));

    if keys.just_pressed(KeyCode::F4) {
        p.flying = !p.flying;
        p.vel = Vec2::ZERO;
        p.grounded = false;
        p.ground = None;
        p.slipping = 0.0;
        p.dash = 0.0;
        p.coyote = 0.0;
        p.jump_buffer = 0.0;
        p.forget_fall(tf.translation.truncate());
        info!("flying mode {}", if p.flying { "on" } else { "off" });
    }
    let flying = p.flying && p.dead.is_none();

    // --- Horizontal input ---
    let mut dir = 0.0;
    if keys.any_pressed([KeyCode::ArrowLeft, KeyCode::KeyA]) {
        dir -= 1.0;
    }
    if keys.any_pressed([KeyCode::ArrowRight, KeyCode::KeyD]) {
        dir += 1.0;
    }
    p.stun = (p.stun - dt).max(0.0);
    p.invuln = (p.invuln - dt).max(0.0);
    let dead = p.dead.is_some();
    if p.stun > 0.0 || dead {
        dir = 0.0;
    }
    if dir != 0.0 {
        p.facing = dir;
    }
    let accel = if dead { GROUND_ACCEL } else if p.stun > 0.0 { AIR_ACCEL * 0.15 } else if p.grounded { GROUND_ACCEL } else { AIR_ACCEL };
    let feet = tf.translation.truncate() - Vec2::Y * HALF.y;
    // The ground's slope (+ = uphill to the right) and how steep it is between
    // walkable (0) and unclimbable (1).
    let on_terrain = p.grounded && p.ground.is_none();
    let slope = if on_terrain { terrain.slope_at(feet) } else { None };
    let slippery = terrain.slides_at(feet);
    let (walk, steep) = if slippery { (SLIPPERY_WALK, SLIPPERY_SLOPE) } else { (WALK_SLOPE, STEEP_SLOPE) };
    // Loose ground creeps downhill under you.
    let drift = if slippery { -slope.unwrap_or(0.0) * SLIPPERY_DRIFT } else { 0.0 };
    let hardness = slope.map_or(0.0, |s| ((s.abs() - walk) / (steep - walk)).clamp(0.0, 1.0));
    // Too steep to stand on, or perched over a drop: slide (signed strength, + = right).
    let slide = match slope {
        Some(s) if on_terrain && s.abs() > steep => Some(-s.signum() * s.abs().min(1.5)),
        // A walkable slope under you grips, even at its very tip; only flat-ish
        // corners (or no readable ground) can tip you off.
        Some(s) if on_terrain && s.abs() > 0.3 => None,
        _ if on_terrain => perched(&terrain, feet),
        _ => None,
    };
    if !flying {
        p.vel.x = match slide {
            // Downhill, faster with steepness; steering only nudges it.
            Some(k) => move_towards(p.vel.x, k * SLIDE_SPEED + dir * 60.0, SLIDE_ACCEL * dt),
            // Steep but standable: uphill is slower and slower, and the slope pulls you down.
            None if hardness > 0.0 => {
                let s = slope.unwrap_or(0.0);
                let uphill = dir * s > 0.0;
                let effort = if uphill { 1.0 - hardness } else { 1.0 };
                move_towards(p.vel.x, dir * MAX_SPEED * effort - s.signum() * SLOPE_PULL * hardness * hardness + drift, accel * dt)
            }
            None if drift != 0.0 && on_terrain => move_towards(p.vel.x, dir * MAX_SPEED + drift, accel * dt),
            // Just left a slope mid-slide: keep the speed instead of braking in the air.
            None if p.slipping > 0.0 && !p.grounded && dir == 0.0 => p.vel.x,
            None => move_towards(p.vel.x, dir * MAX_SPEED, accel * dt),
        };

        // --- Jumping (with coyote time, buffering and variable height) ---
        let jump_keys = [KeyCode::Space, KeyCode::KeyW, KeyCode::ArrowUp];
        p.coyote = if p.grounded { COYOTE_TIME } else { p.coyote - dt };
        p.jump_buffer = if keys.any_just_pressed(jump_keys) { JUMP_BUFFER } else { p.jump_buffer - dt };
        if p.jump_buffer > 0.0 && p.coyote > 0.0 && p.stun <= 0.0 && !dead {
            // On steep ground the jump tilts toward the slope's normal: no hopping up walls.
            let normal = slope.map_or(Vec2::Y, |s| Vec2::new(-s, 1.0).normalize());
            let jump = Vec2::Y.lerp(normal, hardness).normalize();
            p.vel.y = jump.y * JUMP_SPEED;
            if hardness > 0.0 {
                p.vel.x = jump.x * JUMP_SPEED;
            }
            p.jump_buffer = 0.0;
            p.coyote = 0.0;
            p.grounded = false;
            p.ground = None;
        }
        if keys.any_just_released(jump_keys) && p.vel.y > 0.0 {
            p.vel.y *= 0.5;
        }

        // --- Dash (keeps its speed afterwards, so it carries a jump much further) ---
        p.dash_ready |= p.grounded;
        p.dash = (p.dash - dt).max(0.0);
        p.dash_cooldown = (p.dash_cooldown - dt).max(0.0);
        if keys.any_just_pressed(DASH_KEYS) && defeated.dash() && p.dash_ready && p.dash_cooldown <= 0.0 && p.stun <= 0.0 && !dead {
            p.dash = DASH_TIME;
            p.dash_ready = false;
            p.dash_cooldown = DASH_COOLDOWN;
            p.dash_dir = if dir != 0.0 { dir } else { p.facing };
        }
        if p.dash > 0.0 {
            p.vel = Vec2::new(p.dash_dir * DASH_SPEED, 0.0);
        } else {
            p.vel.y = (p.vel.y - GRAVITY * dt).max(-MAX_FALL);
        }
    } else {
        p.grounded = false;
        p.ground = None;
        let mut direction = Vec2::new(dir, 0.0);
        if p.stun <= 0.0 && !dead {
            if keys.any_pressed([KeyCode::ArrowUp, KeyCode::KeyW]) {
                direction.y += 1.0;
            }
            if keys.any_pressed([KeyCode::ArrowDown, KeyCode::KeyS]) {
                direction.y -= 1.0;
            }
        }
        if direction != Vec2::ZERO {
            // Accelerate along the pressed arrows, up to a top speed.
            p.vel = (p.vel + direction.normalize() * FLIGHT_ACCEL * dt).clamp_length_max(FLIGHT_SPEED);
        } else {
            // Released: brake quickly.
            p.vel *= (-FLIGHT_BRAKE * dt).exp();
            if p.vel.length_squared() < 1.0 {
                p.vel = Vec2::ZERO;
            }
        }
    }

    let mut pos = tf.translation.truncate();

    // Ride whatever we stand on (swinging girders, sinking plates, shaking slabs).
    if let Some((_, plat, _)) = p.ground.and_then(|e| platforms.get(e).ok()) {
        if plat.solid {
            pos += plat.delta;
        }
    }

    // --- Move & collide, one axis at a time ---
    let was_grounded = p.grounded;
    p.step *= (-dt * 20.0).exp();
    // Terrain: small steps so nothing is tunneled through; ramps and low steps are climbed.
    let dx = p.vel.x * dt;
    let steps = (dx.abs() / 2.0).ceil().max(1.0);
    for _ in 0..steps as i32 {
        let next = pos + Vec2::X * dx / steps;
        if !terrain.box_hits(next, HALF) {
            pos = next;
            continue;
        }
        let feet = next.y - HALF.y;
        let climb = terrain.box_top(next, HALF).filter(|top| top - feet <= STEP_UP);
        match climb {
            // (No climbing up one-way slopes.)
            Some(top) if !flying && (was_grounded || p.vel.y <= 0.0) && slide.is_none() && !terrain.box_hits(Vec2::new(next.x, top + HALF.y), HALF) => {
                if was_grounded {
                    p.step -= top + HALF.y - pos.y;
                }
                pos = Vec2::new(next.x, top + HALF.y);
            }
            _ => {
                p.vel.x = 0.0;
                break;
            }
        }
    }
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

    let fall_speed = -p.vel.y;
    let prev_bottom = pos.y - HALF.y;
    p.grounded = false;
    p.ground = None;
    let dy = p.vel.y * dt;
    let steps = (dy.abs() / 3.0).ceil().max(1.0);
    for _ in 0..steps as i32 {
        let next = pos + Vec2::Y * dy / steps;
        if !terrain.box_hits(next, HALF) {
            pos = next;
        } else if dy <= 0.0 {
            pos.y = terrain.box_top(next, HALF).unwrap_or(pos.y - HALF.y) + HALF.y;
            land_on_terrain(p, &terrain, pos);
            break;
        } else {
            pos.y = terrain.box_bottom(next, HALF).unwrap_or(pos.y + HALF.y) - HALF.y;
            p.vel.y = 0.0;
            break;
        }
    }
    p.slipping = if slide.is_some() { 0.25 } else { (p.slipping - dt).max(0.0) };
    // Walking (or sliding) down a slope: stick to it instead of hopping off every step.
    if !flying && !p.grounded && (was_grounded || p.slipping > 0.0) && p.vel.y <= 0.0 && p.ground.is_none() {
        let reach = if p.slipping > 0.0 { STEP_UP + p.vel.x.abs() * dt * 2.5 } else { STEP_UP };
        let probe = pos - Vec2::Y * reach;
        if terrain.box_hits(probe, HALF) {
            let y = terrain.box_top(probe, HALF).unwrap_or(pos.y - HALF.y) + HALF.y;
            p.step += pos.y - y;
            pos.y = y;
            land_on_terrain(p, &terrain, pos);
        }
    }
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
    if !flying && p.grounded && !was_grounded && fall_speed > HARD_LANDING {
        shake.add(0.3);
    }

    // Fall damage: from `FALL_SAFE_FLOORS` storeys up, one more HP per extra storey.
    // Soft ground (`Style::softness`) shortens the fall; platforms are hard.
    let feet_y = pos.y - HALF.y;
    if flying {
        p.fall_top = feet_y;
        p.grounded = false;
        p.ground = None;
    } else if p.grounded {
        if !was_grounded && !dead {
            let softness = if p.ground.is_none() { terrain.style_at(pos).softness() } else { 0.0 };
            let floors = (p.fall_top - feet_y).max(0.0) * (1.0 - softness) / FLOOR_HEIGHT;
            if floors >= FALL_SAFE_FLOORS && !p.god {
                p.invuln = 0.0;
                let damage = floors.floor() as i32 - FALL_SAFE_FLOORS as i32 + 1;
                p.hurt(damage, Vec2::ZERO);
                shake.add(0.4);
            }
        }
        p.fall_top = feet_y;
    } else {
        p.fall_top = p.fall_top.max(feet_y);
    }

    // Fell into the abyss: it costs a life, then back to the last safe ground.
    let void_y = terrain.bounds.min.y - VOID_DEPTH;
    if pos.y < void_y && !dead {
        p.invuln = 0.0;
        if let Hurt::Killed = p.hurt(1, Vec2::ZERO) {
            p.vel = Vec2::ZERO;
        } else {
            pos = p.safe_spot + Vec2::Y * 30.0;
            p.fall_top = pos.y - HALF.y;
            p.vel = Vec2::ZERO;
            p.stun = 0.0;
        }
        shake.add(0.4);
    }
    if pos.y < void_y {
        // Dead in the void: wait there, out of sight.
        pos.y = void_y;
        p.vel = Vec2::ZERO;
    }

    // Dead: lie there a while, then start over.
    if let Some(t) = p.dead {
        p.dead = Some(t + dt);
        if t + dt > DEATH_TIME {
            p.dead = None;
            p.hp = p.max_hp;
            p.vel = Vec2::ZERO;
            p.invuln = INVULN;
            p.safe_spot = spawn_point(&terrain);
            pos = p.safe_spot;
            p.fall_top = pos.y - HALF.y;
            respawned.write(Respawned);
        }
    }

    tf.translation.x = pos.x;
    tf.translation.y = pos.y;
}

/// Balanced on a corner over a drop (under a third of the feet on the ground,
/// and the ground past the other edge falls away steeper than `STEEP_SLOPE`):
/// which way to slip off. On a plain slope only one corner touches too, but
/// the ground under the other one is close: that's for the slope rules.
fn perched(terrain: &Terrain, feet: Vec2) -> Option<f32> {
    const SAMPLES: usize = 9;
    let (mut sum, mut count) = (0.0, 0);
    for i in 0..SAMPLES {
        let x = -HALF.x + 2.0 * HALF.x * i as f32 / (SAMPLES - 1) as f32;
        if terrain.solid_at(feet + Vec2::new(x, -2.0)) {
            sum += x;
            count += 1;
        }
    }
    if count == 0 || count * 3 >= SAMPLES {
        return None;
    }
    let away = if sum / count as f32 > 0.0 { -1.0 } else { 1.0 };
    let support = sum / count as f32;
    let edge = away * HALF.x;
    let drop = terrain.surface(feet + Vec2::new(edge, -1.0), 400.0).map_or(f32::INFINITY, |g| feet.y - g);
    (drop > (edge - support).abs() * STEEP_SLOPE).then_some(away * 0.3)
}

fn land_on_terrain(p: &mut Player, terrain: &Terrain, pos: Vec2) {
    p.vel.y = 0.0;
    p.grounded = true;
    p.ground_color = Some(terrain_art::ground_color(terrain.style_at(pos)));
    // Safe to come back to if there's ground under both edges.
    let feet = pos.y - HALF.y - 1.0;
    if terrain.solid_at(Vec2::new(pos.x - HALF.x, feet)) && terrain.solid_at(Vec2::new(pos.x + HALF.x, feet)) {
        p.safe_spot = pos;
    }
}

fn overlaps(pos: Vec2, center: Vec2, half: Vec2) -> bool {
    let d = (pos - center).abs();
    d.x < HALF.x + half.x && d.y < HALF.y + half.y
}

fn move_towards(current: f32, target: f32, max_delta: f32) -> f32 {
    current + (target - current).clamp(-max_delta, max_delta)
}
