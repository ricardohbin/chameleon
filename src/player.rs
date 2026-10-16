//! Player controller: input, gravity, collisions against the terrain grid
//! (walking up ramps and small steps) and the moving platforms.

use bevy::prelude::*;

use crate::{
    boss::Defeated,
    camera::Shake,
    chameleon,
    level::{self, Platform, Restart},
    terrain::{FLOOR_HEIGHT, STEP_UP, Terrain},
    terrain_art,
    water::{Splash, Waters, Wet},
};

/// Size of the chameleon relative to its drawing (affects visuals and collider).
pub const SCALE: f32 = 0.7;
/// Half extents of the player's collision box (its bottom is at `-HALF.y`).
/// In rig units it spans from the elbow/knee (-14) up to just inside the back
/// (22), so the feet sink into the ground and no corner pokes out of the sprite.
pub const HALF: Vec2 = Vec2::new(30.0 * SCALE, 18.0 * SCALE);
/// The rig's origin relative to the collider center (the box center is at rig y = +4).
pub const RIG_DROP: f32 = -4.0 * SCALE;
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
/// Wall walking (earned by beating the spider): pushing into a tall wall
/// gets the chameleon on it; W/S walk up and down at `CLIMB_SPEED`, Space
/// leaps off (`WALL_LEAP`), pushing away lets go. After letting go it can't
/// grab a wall again for `CLIMB_LOCK`.
const CLIMB_SPEED: f32 = 170.0;
const WALL_LEAP: Vec2 = Vec2::new(380.0, 640.0);
const CLIMB_LOCK: f32 = 0.25;
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
/// The grasshopper's big jump: fully charged it takes off at this speed (~3x as high,
/// still under the fall-damage height). Holding down charges it, full after
/// `CHARGE_TIME` seconds; let go sooner than `CHARGE_MIN` and nothing happens.
const BIG_JUMP_SPEED: f32 = 1300.0;
pub const CHARGE_TIME: f32 = 1.5;
const CHARGE_MIN: f32 = 0.2;
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
const FALL_SAFE_FLOORS: f32 = 5.0;
/// This far below the lowest ground you fell into the abyss.
const VOID_DEPTH: f32 = 600.0;
/// From this share of the body under the surface you swim instead of walking.
const SWIM_FRACTION: f32 = 0.3;
/// Seconds out of the water to get the whole swimming strength back.
const SWIM_RECOVER: f32 = 2.5;
/// Landing in water this deep (one storey) absorbs the whole fall; shallower, only part.
const WATER_SOFT_DEPTH: f32 = FLOOR_HEIGHT;
/// Splashes below this speed are just wading.
const SPLASH_SPEED: f32 = 150.0;

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
    /// 0..1: how stuck in cobwebs (`webs.rs`), set every frame; slows the walk.
    pub webbed: f32,
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
    /// Blows taken so far (each `hurt` / `shock` that landed): `sound.rs` hears the hits.
    pub wounds: u32,
    /// Seconds left untouchable in the middle of an attack (the air spin): like
    /// `invuln`, without the blinking.
    pub guard: f32,
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
    /// On a wall (wall walking): which side it is on (-1 left, 1 right).
    pub climbing: Option<f32>,
    /// The grasshopper's power: seconds spent charging the big jump (down held on
    /// the ground, feet planted); see `charging`.
    charge: Option<f32>,
    /// Burrowing through rubble (`burrow.rs`): a dig in progress, and the way through that `E` would take now.
    pub burrow: Option<crate::burrow::Burrow>,
    pub burrow_target: Option<crate::burrow::Path>,
    climb_lock: f32,
    /// Seconds left of the current dash.
    pub dash: f32,
    dash_ready: bool,
    dash_cooldown: f32,
    /// Which way the current dash goes.
    dash_dir: f32,
    /// Highest the feet have been since last touching ground (fall damage).
    fall_top: f32,
    /// Share of the body under water (0 = dry), and whether it is swimming (>= `SWIM_FRACTION`).
    pub wet: f32,
    pub swimming: bool,
    /// Swimming strength left, 1 = fresh. It runs down in the water (faster when swimming
    /// about) and comes back out of it. Tired, the chameleon hangs deeper and deeper,
    /// until its head is under and it starts to drown.
    pub swim: f32,
    /// Drowning progress, one HP at 1.
    harm: f32,
    /// How much of the fall carries through the water (shallow water absorbs less).
    carry: f32,
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
    /// Seconds into charging the big jump (0 = just started), if it is.
    pub fn charging(&self) -> Option<f32> {
        self.charge
    }

    /// Take `damage` and get knocked back with velocity `knock`.
    pub fn hurt(&mut self, damage: i32, knock: Vec2) -> Hurt {
        if self.dead.is_some() || self.invuln > 0.0 || self.guard > 0.0 || self.god {
            return Hurt::Ignored;
        }
        self.hp = (self.hp - damage).max(0);
        self.wounds += 1;
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

    /// An electric jolt (`electric.rs`): costs `damage`, a short twitch and a hop
    /// instead of a knockback, and no invulnerability, so the shocks keep coming
    /// for as long as the chameleon stays on the live rail.
    pub fn shock(&mut self, damage: i32) -> Hurt {
        if self.dead.is_some() || self.god {
            return Hurt::Ignored;
        }
        self.hp = (self.hp - damage).max(0);
        self.wounds += 1;
        self.shadow = (self.shadow - 1.0).max(0.0);
        self.digest = 0.0;
        self.vel.y = self.vel.y.max(170.0);
        self.vel.x *= 0.5;
        self.stun = 0.18;
        if self.hp == 0 {
            self.dead = Some(0.0);
            self.shadow = 0.0;
            Hurt::Killed
        } else {
            Hurt::Hurt
        }
    }

    /// Choking on foul air (`king_rat::foul_air`): costs `damage` and a cough
    /// (a short stagger), no knockback.
    pub fn choke(&mut self, damage: i32) -> Hurt {
        if self.dead.is_some() || self.god {
            return Hurt::Ignored;
        }
        self.hp = (self.hp - damage).max(0);
        self.wounds += 1;
        self.shadow = (self.shadow - 1.0).max(0.0);
        self.digest = 0.0;
        self.vel.x *= 0.4;
        self.stun = 0.25;
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

fn spawn(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<ColorMaterial>,
    terrain: &Terrain,
    god: bool,
) {
    let spawn = spawn_point(terrain);
    let player = commands
        .spawn((
            Player {
                facing: 1.0,
                safe_spot: spawn,
                fall_top: spawn.y - HALF.y,
                hp: MAX_HP,
                max_hp: MAX_HP,
                swim: 1.0,
                god,
                ..default()
            },
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
    mut splash: MessageWriter<Splash>,
    waters: Res<Waters>,
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
    if let Some(target) = window
        .cursor_position()
        .and_then(|c| camera.viewport_to_world_2d(camera_tf, c).ok())
    {
        p.aim_at = Some(target);
    }
    let neck = chameleon::neck(tf.translation.truncate(), p.facing);
    p.aim = p
        .aim_at
        .map_or(Vec2::ZERO, |at| at - neck)
        .normalize_or(Vec2::new(p.facing, 0.0));

    // Underground: `burrow.rs` moves it.
    if p.burrow.is_some() {
        return;
    }

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

    // --- Water: how deep we are, first contact (splash, the fall is held) ---
    let wet = if flying {
        None
    } else {
        waters.sample(&terrain, tf.translation.truncate(), HALF)
    };
    let wet_f = wet.as_ref().map_or(0.0, |w| w.fraction);
    let swimming = wet_f >= SWIM_FRACTION;
    let safe_before = p.safe_spot;
    if let Some(w) = &wet {
        if p.wet <= 0.0 {
            let speed = p.vel.length();
            let protect = (w.depth / WATER_SOFT_DEPTH).clamp(0.0, 1.0);
            p.carry = (p.fall_top - (tf.translation.y - HALF.y)).max(0.0) * (1.0 - protect);
            if speed > SPLASH_SPEED {
                splash.write(Splash {
                    at: Vec2::new(tf.translation.x, w.level),
                    pool: w.pool,
                    speed,
                });
                if speed > HARD_LANDING * 0.6 {
                    shake.add(0.2);
                }
            }
            if p.vel.y < 0.0 {
                p.vel.y *= w.kind.t().entry;
            }
        }
    }
    p.wet = wet_f;
    p.swimming = swimming;

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
    p.guard = (p.guard - dt).max(0.0);
    let dead = p.dead.is_some();
    if p.stun > 0.0 || dead || p.charging().is_some() {
        dir = 0.0;
    }
    if dir != 0.0 {
        p.facing = dir;
    }
    let accel = if dead {
        GROUND_ACCEL
    } else if p.stun > 0.0 {
        AIR_ACCEL * 0.15
    } else if p.grounded {
        GROUND_ACCEL
    } else {
        AIR_ACCEL
    };
    let feet = tf.translation.truncate() - Vec2::Y * HALF.y;
    // The ground's slope (+ = uphill to the right) and how steep it is between
    // walkable (0) and unclimbable (1).
    let on_terrain = p.grounded && p.ground.is_none();
    let slope = if on_terrain {
        terrain.slope_at(feet)
    } else {
        None
    };
    let slippery = terrain.slides_at(feet);
    let (walk, steep) = if slippery {
        (SLIPPERY_WALK, SLIPPERY_SLOPE)
    } else {
        (WALK_SLOPE, STEEP_SLOPE)
    };
    // Loose ground creeps downhill under you.
    let drift = if slippery {
        -slope.unwrap_or(0.0) * SLIPPERY_DRIFT
    } else {
        0.0
    };
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
    // Wading slows the walk (swimming has its own speed, below).
    // The frog's power: water doesn't slow, tire or drown it.
    let at_home = defeated.wade();
    let wade = wet.as_ref().filter(|_| !at_home).map_or(1.0, |w| {
        1.0 - w.kind.t().wade * (w.fraction / SWIM_FRACTION).min(1.0)
    });
    let top_speed = MAX_SPEED * wade * (1.0 - 0.55 * p.webbed).max(0.1);

    // --- Wall walking: push into a wall to get on it ---
    p.climb_lock = (p.climb_lock - dt).max(0.0);
    let here = tf.translation.truncate();
    let wall = |side: f32| terrain.box_hits(here + Vec2::X * side * 3.0, HALF);
    // A real wall, not a step to walk up.
    let tall = |side: f32| terrain.box_hits(here + Vec2::new(side * 3.0, STEP_UP + 4.0), HALF);
    let can_climb = defeated.climb() && !flying && !swimming && !dead && p.stun <= 0.0;
    if let Some(side) = p.climbing {
        if !can_climb || dir == -side || p.dash > 0.0 {
            p.climbing = None;
            p.climb_lock = CLIMB_LOCK;
        } else if !wall(side) {
            // Over the top: a hop onto the ledge (off the bottom: just let go).
            p.climbing = None;
            if p.vel.y > 0.0 {
                p.vel = Vec2::new(side * 180.0, 330.0);
            }
        }
    } else if can_climb && p.climb_lock <= 0.0 && dir != 0.0 && wall(dir) && tall(dir) {
        p.climbing = Some(dir);
        p.vel = Vec2::ZERO;
    }
    let climbing = p.climbing;
    if !flying {
        let vx0 = p.vel.x;
        p.vel.x = match slide {
            // Downhill, faster with steepness; steering only nudges it.
            Some(k) => move_towards(p.vel.x, k * SLIDE_SPEED + dir * 60.0, SLIDE_ACCEL * dt),
            // Steep but standable: uphill is slower and slower, and the slope pulls you down.
            None if hardness > 0.0 => {
                let s = slope.unwrap_or(0.0);
                let uphill = dir * s > 0.0;
                let effort = if uphill { 1.0 - hardness } else { 1.0 };
                move_towards(
                    p.vel.x,
                    dir * top_speed * effort - s.signum() * SLOPE_PULL * hardness * hardness
                        + drift,
                    accel * dt,
                )
            }
            None if drift != 0.0 && on_terrain => {
                move_towards(p.vel.x, dir * top_speed + drift, accel * dt)
            }
            // Just left a slope mid-slide: keep the speed instead of braking in the air.
            None if p.slipping > 0.0 && !p.grounded && dir == 0.0 => p.vel.x,
            None => move_towards(p.vel.x, dir * top_speed, accel * dt),
        };
        let swim_up = !dead
            && p.stun <= 0.0
            && keys.any_pressed([KeyCode::Space, KeyCode::KeyW, KeyCode::ArrowUp]);
        let swim_down =
            !dead && p.stun <= 0.0 && keys.any_pressed([KeyCode::KeyS, KeyCode::ArrowDown]);
        if let (true, Some(w)) = (swimming, &wet) {
            let t = w.kind.t();
            p.vel.x = if at_home {
                move_towards(vx0, dir * top_speed, accel * dt)
            } else {
                move_towards(vx0, dir * t.speed * (0.45 + 0.55 * p.swim), t.accel * dt)
            };
        }

        // --- Jumping (with coyote time, buffering and variable height) ---
        let jump_keys = [KeyCode::Space, KeyCode::KeyW, KeyCode::ArrowUp];
        p.coyote = if p.grounded {
            COYOTE_TIME
        } else {
            p.coyote - dt
        };
        p.jump_buffer = if keys.any_just_pressed(jump_keys) {
            JUMP_BUFFER
        } else {
            p.jump_buffer - dt
        };
        // On steep ground the jump tilts toward the slope's normal: no hopping up walls.
        let normal = slope.map_or(Vec2::Y, |s| Vec2::new(-s, 1.0).normalize());
        let jump = Vec2::Y.lerp(normal, hardness).normalize();
        let take_off = |p: &mut Player, speed: f32| {
            p.vel.y = jump.y * speed;
            if hardness > 0.0 {
                p.vel.x = jump.x * speed;
            }
            p.jump_buffer = 0.0;
            p.coyote = 0.0;
            p.grounded = false;
            p.ground = None;
        };
        // The big jump (grasshopper): holding down (S / arrow) on the ground charges it
        // (feet planted), letting go launches it; a tap (under `CHARGE_MIN`) does nothing.
        let down = keys.any_pressed([KeyCode::KeyS, KeyCode::ArrowDown]);
        let can_charge = defeated.leap() && !swimming && !dead && p.stun <= 0.0 && climbing.is_none() && p.grounded;
        let mut launched = false;
        if let Some(held) = p.charge {
            if !can_charge {
                // Walked off, stunned, knocked into the air...
                p.charge = None;
            } else if down {
                p.charge = Some(held + dt);
            } else {
                if held >= CHARGE_MIN {
                    let k = (held / CHARGE_TIME).clamp(0.0, 1.0);
                    let k = k * k * (3.0 - 2.0 * k);
                    take_off(p, JUMP_SPEED + (BIG_JUMP_SPEED - JUMP_SPEED) * k);
                    launched = true;
                    if k >= 1.0 {
                        shake.add(0.25);
                    }
                }
                p.charge = None;
            }
        } else if can_charge && down {
            p.charge = Some(0.0);
        }
        if p.jump_buffer > 0.0 && p.coyote > 0.0 && p.stun <= 0.0 && !dead && climbing.is_none() && !launched {
            let push = if swimming && !at_home {
                wet.as_ref().map_or(1.0, |w| w.kind.t().jump)
            } else {
                1.0
            };
            take_off(p, JUMP_SPEED * push);
            p.charge = None;
        }
        if keys.any_just_released(jump_keys) && p.vel.y > 0.0 && !launched {
            p.vel.y *= 0.5;
        }

        // --- Dash (keeps its speed afterwards, so it carries a jump much further) ---
        p.dash_ready |= p.grounded;
        p.dash = (p.dash - dt).max(0.0);
        p.dash_cooldown = (p.dash_cooldown - dt).max(0.0);
        if keys.any_just_pressed(DASH_KEYS)
            && defeated.dash()
            && p.dash_ready
            && p.dash_cooldown <= 0.0
            && p.stun <= 0.0
            && !dead
        {
            p.dash = DASH_TIME;
            p.dash_ready = false;
            p.dash_cooldown = DASH_COOLDOWN;
            p.dash_dir = if dir != 0.0 { dir } else { p.facing };
        }
        if p.dash > 0.0 {
            p.vel = Vec2::new(p.dash_dir * DASH_SPEED, 0.0);
        } else if let (true, Some(w)) = (swimming, &wet) {
            // Near the surface a jump hops out of the water (onto a ledge).
            let y = tf.translation.y;
            if p.jump_buffer > 0.0
                && !dead
                && p.stun <= 0.0
                && y > w.level - HALF.y * 1.3
                && bank_near(&terrain, tf.translation.truncate())
            {
                p.vel.y = JUMP_SPEED * w.kind.t().hop;
                p.swim = (p.swim - 0.05).max(0.0);
                p.jump_buffer = 0.0;
                p.grounded = false;
                p.ground = None;
            }
            p.vel.y = swim_dy(&p, w, y, (swim_up, swim_down), dead, dt);
        } else if let Some(side) = climbing {
            // On the wall: W/S (arrows) walk up and down it, Space leaps off.
            let up = keys.any_pressed([KeyCode::KeyW, KeyCode::ArrowUp]) as i32 - keys.any_pressed([KeyCode::KeyS, KeyCode::ArrowDown]) as i32;
            p.vel = Vec2::new(side * 60.0, up as f32 * CLIMB_SPEED * (1.0 - 0.55 * p.webbed).max(0.1));
            p.facing = side;
            if keys.just_pressed(KeyCode::Space) {
                p.vel = Vec2::new(-side * WALL_LEAP.x, WALL_LEAP.y);
                p.facing = -side;
                p.climbing = None;
                p.climb_lock = CLIMB_LOCK;
                p.jump_buffer = 0.0;
            }
        } else {
            p.vel.y = (p.vel.y - GRAVITY * dt).max(-MAX_FALL);
        }
        // Swimming wears you out (faster the more you move); a dry body recovers.
        if at_home && swimming {
            // Never tires in the water.
            p.swim = 1.0;
            p.harm = 0.0;
        } else if let (true, Some(w)) = (swimming, &wet) {
            let t = w.kind.t();
            let busy = dir != 0.0 || swim_up || swim_down;
            p.swim = (p.swim - dt / if busy { t.active } else { t.idle }).max(0.0);
            // Tired, the head ends up under; the longer and the more tired, the worse.
            let tired = if dead { 1.0 } else { 1.0 - p.swim };
            if tf.translation.y + HALF.y < w.level && tired > 0.5 && !dead {
                p.harm += dt * (0.5 + 3.0 * (tired - 0.5));
            } else {
                p.harm = (p.harm - dt * 0.8).max(0.0);
            }
            if p.harm >= 1.0 {
                if p.god {
                    p.harm = 0.0;
                } else if p.invuln <= 0.0 {
                    let keep = p.vel;
                    p.harm = 0.0;
                    if let Hurt::Hurt = p.hurt(1, keep) {
                        p.stun = 0.0;
                    }
                    shake.add(0.15);
                }
            }
        } else {
            p.harm = (p.harm - dt * 0.8).max(0.0);
            // Only dry ground rests you: hopping out of the water and back doesn't.
            if wet_f <= 0.0 && p.grounded {
                p.swim = (p.swim + dt / SWIM_RECOVER).min(1.0);
            }
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
            p.vel =
                (p.vel + direction.normalize() * FLIGHT_ACCEL * dt).clamp_length_max(FLIGHT_SPEED);
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
        let climb = terrain
            .box_top(next, HALF)
            .filter(|top| top - feet <= STEP_UP);
        match climb {
            // (No climbing up one-way slopes.)
            Some(top)
                if !flying
                    && (was_grounded || p.vel.y <= 0.0)
                    && slide.is_none()
                    && !terrain.box_hits(Vec2::new(next.x, top + HALF.y), HALF) =>
            {
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
            pos.x = if pos.x < c.x {
                c.x - plat.half.x - HALF.x
            } else {
                c.x + plat.half.x + HALF.x
            };
            p.vel.x = 0.0;
        }
    }

    let fall_speed = -p.vel.y;
    let prev_bottom = pos.y - HALF.y;
    let prev_ground = p.ground;
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
    p.slipping = if slide.is_some() {
        0.25
    } else {
        (p.slipping - dt).max(0.0)
    };
    // Walking (or sliding) down a slope: stick to it instead of hopping off every step.
    if !flying
        && !p.grounded
        && (was_grounded || p.slipping > 0.0)
        && p.vel.y <= 0.0
        && p.ground.is_none()
    {
        let reach = if p.slipping > 0.0 {
            STEP_UP + p.vel.x.abs() * dt * 2.5
        } else {
            STEP_UP
        };
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
        if !plat.solid || !overlaps(pos, c, plat.bound()) {
            continue;
        }
        let tilted = plat.tilt != 0.0;
        // Inclined: the surface under the body's middle, and only within its length.
        if tilted && (pos.x - c.x).abs() > plat.run() + HALF.x * 0.5 {
            continue;
        }
        let top = plat.top_at(c, pos.x);
        // A little tolerance so moving platforms don't drop us (and slopes can be walked up).
        let slack = 6.0
            + plat.tilt.tan().abs()
                * if prev_ground == Some(entity) {
                    20.0
                } else {
                    10.0
                };
        if tilted && pos.y - HALF.y > top + 0.5 {
            // Above the slanted surface, in the corner of its box.
        } else if p.vel.y <= 0.0 && prev_bottom >= top - slack {
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
    // Walking down an inclined platform: stay on it instead of hopping off.
    if !flying && !p.grounded && p.vel.y <= 0.0 {
        if let Some((entity, plat, ptf)) = prev_ground.and_then(|e| platforms.get(e).ok()) {
            let c = ptf.translation.truncate();
            if plat.solid && plat.tilt != 0.0 && (pos.x - c.x).abs() <= plat.run() {
                let top = plat.top_at(c, pos.x);
                let gap = pos.y - HALF.y - top;
                if (0.0..=6.0 + plat.tilt.tan().abs() * 20.0).contains(&gap) {
                    pos.y = top + HALF.y;
                    p.vel.y = 0.0;
                    p.grounded = true;
                    p.ground = Some(entity);
                }
            }
        }
    }
    if !flying && p.grounded && !was_grounded && fall_speed > HARD_LANDING {
        shake.add(0.3);
    }

    // Fall damage: from `FALL_SAFE_FLOORS` storeys up, one more HP per extra storey.
    // Soft ground (`Style::softness`) shortens the fall; platforms are hard; water
    // takes the fall (`carry`: all of it if deep enough).
    let feet_y = pos.y - HALF.y;
    // Holding on to a wall: no fall builds up.
    if p.climbing.is_some() {
        p.fall_top = feet_y;
    }
    if wet_f > 0.0 {
        p.fall_top = feet_y + p.carry;
        p.safe_spot = safe_before;
    }
    if flying {
        p.fall_top = feet_y;
        p.grounded = false;
        p.ground = None;
    } else if p.grounded {
        if !was_grounded && !dead {
            let softness = if p.ground.is_none() {
                terrain.style_at(pos).softness()
            } else {
                0.0
            };
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
            p.swim = 1.0;
            p.harm = 0.0;
            p.wet = 0.0;
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

/// Ground within reach beside (or just under) the swimmer: something to climb out onto.
/// Without it the water gives nothing to push off from.
fn bank_near(terrain: &Terrain, pos: Vec2) -> bool {
    [-1.0, 1.0].iter().any(|side| {
        (0..5).any(|i| {
            let y = pos.y - HALF.y + HALF.y * 2.5 * i as f32 / 4.0;
            terrain.solid_at(Vec2::new(pos.x + side * (HALF.x + 14.0), y))
        })
    })
}

/// Vertical velocity while swimming: floats toward its resting depth under the
/// surface (deeper the more tired it is, `Traits::sag`), strokes up (`up`, only
/// as high as its strength lets the head rise) or dives (`down`), and sinks once
/// exhausted. The water pulls the velocity toward that target (`Traits::accel`).
fn swim_dy(p: &Player, w: &Wet, y: f32, (up, down): (bool, bool), dead: bool, dt: f32) -> f32 {
    let t = w.kind.t();
    let tired = if dead { 1.0 } else { 1.0 - p.swim };
    let float_y = w.level - HALF.y * (0.2 + t.sag * tired * tired);
    let mut target = ((float_y - y) * 3.0).clamp(-300.0, t.rise);
    if tired >= 1.0 {
        target = target.min(-t.sink);
    }
    let stroke = t.stroke * (0.3 + 0.7 * p.swim);
    if up && y < w.level - HALF.y * (0.35 + 1.2 * tired * tired) {
        target = target.max(stroke);
    } else if down {
        target = -stroke * 0.8;
    }
    move_towards(p.vel.y, target, t.accel * dt)
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
    let drop = terrain
        .surface(feet + Vec2::new(edge, -1.0), 400.0)
        .map_or(f32::INFINITY, |g| feet.y - g);
    (drop > (edge - support).abs() * STEEP_SLOPE).then_some(away * 0.3)
}

fn land_on_terrain(p: &mut Player, terrain: &Terrain, pos: Vec2) {
    p.vel.y = 0.0;
    p.grounded = true;
    p.ground_color = Some(terrain_art::ground_color(terrain.style_at(pos)));
    // Safe to come back to if there's ground under both edges.
    let feet = pos.y - HALF.y - 1.0;
    if terrain.solid_at(Vec2::new(pos.x - HALF.x, feet))
        && terrain.solid_at(Vec2::new(pos.x + HALF.x, feet))
    {
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
