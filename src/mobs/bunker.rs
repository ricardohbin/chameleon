//! The bunker's vermin ("region: cold war military bunker", `Style::Bunker`):
//! the brood and hangers-on of the giant beetle that rules it. All of them
//! bleed its orange.
//! - **Wireworms** (click-beetle larvae) live under the cracked floor: a little
//!   heap of crumbs moving along the ground is all you see. Under the
//!   chameleon, the crumbs shake and the mandibles peek out (the tell), then
//!   the worm shoots straight up out of the floor. It stays reared and
//!   thrashing for a moment (the only time a slash finds it), then sinks back.
//! - **Bombardiers** turn their back on the chameleon and back into range,
//!   rear first, scuttling off head first when it gets too close. The tail
//!   tips up and drips (the tell), then fires a short cone of scalding spray
//!   behind it. Spent, it steams for a while: the window. Jumping over it
//!   makes it turn around before it can aim again.
//! - **Ramhorns** (rhinoceros beetles): their horn and plated front glance
//!   slashes off. They paw the ground, then charge along it, committed. One
//!   that rams a wall is dazed (open from every side); one that misses skids
//!   and takes a long time to turn round, showing its back.
//! - **Sextons** (burying beetles) roll balls of carrion and rubble: they
//!   rear back and shove the ball at the chameleon. The ball rolls along the
//!   ground, faster downhill, bounces off walls and bursts on whatever it
//!   hits. A slash sends it back, and then it hurts mobs instead. The sexton
//!   gathers a new ball (`BALL_REGROW`), keeping its distance meanwhile.

use bevy::prelude::*;

use super::{flinch, idle, walk, Mob, MobKind, MobSpec, MobStuff, Moves, Notice, State, GRAVITY};
use crate::{
    boss,
    camera::{HitStop, Shake},
    chameleon::{self, ChameleonAnim},
    gore::{Bleed, Fluid},
    mob_art,
    player::Player,
    spit::SpitHit,
    terrain::{Terrain, STEP_UP},
};

pub(super) static WIREWORM: MobSpec = MobSpec {
    moves: Moves::Bore,
    hp: 1,
    scale: 1.5,
    radius: 13.0,
    foot: 5.0,
    blood: Fluid::Orange,
    // Meandering under its patch of floor.
    speed: 30.0,
    sense: 340.0,
    windup: 0.55,
    // Tunneling after the chameleon.
    attack_speed: 120.0,
    // Out of the floor in this long.
    attack_time: 0.15,
    // Stuck out, thrashing (the window), before it sinks back.
    recover: 0.9,
    leash: 220.0,
    damage: 1,
    stubborn: false,
    initiative: 0.8,
    veteran: false,
    // 1 hp, an ambush you can see coming.
    food: 0.25,
};
pub(super) static BOMBARDIER: MobSpec = MobSpec {
    moves: Moves::Walk,
    hp: 2,
    scale: 1.5,
    radius: 16.0,
    foot: 6.0 * 1.5,
    blood: Fluid::Orange,
    speed: 55.0,
    sense: 400.0,
    windup: 0.6,
    // Spray speed.
    attack_speed: 480.0,
    // The burst.
    attack_time: 0.45,
    // Spent, steaming.
    recover: 1.4,
    leash: 280.0,
    damage: 1,
    stubborn: false,
    initiative: 1.0,
    veteran: false,
    // 2 hp, a scalding cone.
    food: 0.75,
};
pub(super) static RAMHORN: MobSpec = MobSpec {
    moves: Moves::Walk,
    hp: 4,
    scale: 2.0,
    radius: 24.0,
    foot: 7.0 * 2.0,
    blood: Fluid::Orange,
    speed: 38.0,
    sense: 440.0,
    windup: 0.8,
    attack_speed: 470.0,
    // Longest charge.
    attack_time: 1.4,
    // Skidding to a stop.
    recover: 0.7,
    leash: 380.0,
    damage: 1,
    stubborn: true,
    initiative: 1.3,
    veteran: true,
    // 4 hp, armored, charges through hits.
    food: 1.5,
};
pub(super) static SEXTON: MobSpec = MobSpec {
    moves: Moves::Walk,
    hp: 3,
    scale: 1.6,
    radius: 18.0,
    foot: 6.0 * 1.6,
    blood: Fluid::Orange,
    speed: 45.0,
    sense: 460.0,
    windup: 0.7,
    // The shove: the ball's speed.
    attack_speed: 400.0,
    attack_time: 0.25,
    recover: 0.9,
    leash: 300.0,
    // What its ball does.
    damage: 1,
    stubborn: false,
    initiative: 1.0,
    veteran: false,
    // 3 hp, rolling hazards from afar.
    food: 1.0,
};

/// The region's ecosystem: the beetle's brood, plus the vermin that survive
/// anything (roaches) and raid the rations (rats).
pub(super) fn menu() -> Vec<(MobKind, f32)> {
    vec![
        (MobKind::Wireworm, 0.25),
        (MobKind::Bombardier, 0.2),
        (MobKind::Sexton, 0.13),
        (MobKind::Ramhorn, 0.12),
        (MobKind::Roachling, 0.15),
        (MobKind::Rat, 0.15),
    ]
}

/// Wireworm: how high (drawing units) its head stands out of the floor, fully reared.
pub const REACH: f32 = 30.0;
/// Wireworm: how far out it peeks in its tell, and from how far out slashes find it (shares of `REACH`).
const PEEK: f32 = 0.2;
const OPEN_RISE: f32 = 0.3;
/// Wireworm: strikes once it's this close under the chameleon (x).
const UNDER: f32 = 22.0;
/// Wireworm: the strike hits within this much (plus its radius) of its column.
const COLUMN: f32 = 14.0;
/// Wireworm: stays at the top of the strike this long; sinks back in `SINK` s.
const HOLD_TOP: f32 = 0.1;
const SINK: f32 = 0.35;
/// Wireworm: seconds before it can strike again.
const REARM: (f32, f32) = (0.8, 1.6);

/// Bombardier: where the spray comes out (drawing units, facing +X).
pub const NOZZLE: Vec2 = Vec2::new(-14.0, 0.5);
/// Bombardier: the spray cone (reach, half-angle), and the chameleon's size in it.
const SPRAY_REACH: f32 = 150.0;
const SPRAY_FAN: f32 = 0.42;
const SCALD_R: f32 = 14.0;
/// Bombardier: the spray turns at most this far (radians) off straight back.
const SPRAY_TURN: f32 = 0.6;
/// Bombardier: distance it likes to spray from; backing in and scuttling off (× speed).
const SPRAY_GAP: f32 = 105.0;
const BACK_UP: f32 = 0.9;
const SCUTTLE: f32 = 1.7;
/// Bombardier: seconds to turn its back again; between bursts.
const QUICK_TURN: f32 = 0.25;
const RELOAD: (f32, f32) = (1.2, 2.2);

/// Ramhorn: the horn's reach in front (drawing units).
const HORN: f32 = 23.0;
/// Ramhorn: seconds to turn round (its back is open meanwhile).
const SLOW_TURN: f32 = 0.9;
/// Ramhorn: dazed this long after ramming a wall at least `RAM_SPEED` fast.
const DAZE: f32 = 1.8;
const RAM_SPEED: f32 = 200.0;
/// Ramhorn: seconds to get up to full charging speed; rest between charges.
const SPEED_UP: f32 = 0.25;
const BREATHER: (f32, f32) = (0.4, 0.9);

/// Sexton: its ball (drawing units): center in front of it, and radius.
pub const BALL_AT: Vec2 = Vec2::new(15.0, 2.0);
pub const BALL_R: f32 = 8.0;
/// Sexton: seconds to gather a new ball.
pub const BALL_REGROW: f32 = 3.5;
/// Sexton: backs off while gathering, if closer than this; shoves from up to this far.
const KEEP_AWAY: f32 = 130.0;
const KICK_RANGE: f32 = 360.0;
const BREATHER_SEXTON: (f32, f32) = (0.5, 1.2);

/// Rolling balls: life, pull downhill, rolling drag, kept speed per wall bounce
/// (and how many bounces it takes), speed when slashed back.
const BALL_LIFE: f32 = 7.0;
const BALL_PULL: f32 = 900.0;
const ROLL_DRAG: f32 = 0.25;
const BOUNCE: f32 = 0.55;
const MAX_BOUNCES: u32 = 2;
const BALL_RETURN: f32 = 520.0;
/// Slower than this for `STALL` s: it falls apart.
const BALL_SLOW: f32 = 30.0;
const STALL: f32 = 0.5;
/// The chameleon's size against a ball.
const BALL_HIT_R: f32 = 14.0;

/// The smallest step a walker takes: creeping a fraction of a unit per frame,
/// the body box catches on the edge of a pixel step (see `rat::Mind::stride`).
const MIN_STEP: f32 = 4.0;

/// What a bunker mob has in mind (kept on its `Mob`).
#[derive(Default)]
pub struct Mind {
    /// Wireworm: its feet under the floor (set on the first frame).
    feet: Option<Vec2>,
    /// Wireworm: how far out of the floor, 0 (buried) .. 1 (fully reared).
    rise: f32,
    /// Ramhorn: seconds left dazed after ramming a wall.
    dazed: f32,
    /// Turning round: seconds spent so far.
    turn: f32,
    /// Sexton: seconds until its next ball is ready.
    regrow: f32,
    /// Sexton: a ball shoved this frame (center, speed), for `balls` to launch.
    kick: Option<(Vec2, f32)>,
    /// 1 when a slash glances off its armor, fading.
    clank: f32,
    /// Bombardier: where the burst goes (unit).
    aim: Vec2,
    /// Already hit the chameleon with this attack.
    struck: bool,
    /// Movement saved up until it makes a whole step.
    carry: f32,
}

impl Mind {
    /// Wireworm: how far its head stands above its feet (world units).
    pub(crate) fn height(&self, spec: &MobSpec) -> f32 {
        spec.foot + self.rise * REACH * spec.scale
    }

    /// Ramhorn: dazed (open from every side).
    pub(crate) fn dazed(&self) -> bool {
        self.dazed > 0.0
    }

    /// 1 when a slash just glanced off its armor, fading.
    pub(crate) fn clank(&self) -> f32 {
        self.clank
    }

    /// Sexton: how much of its next ball it has gathered, 0..1.
    pub(crate) fn ball(&self) -> f32 {
        1.0 - (self.regrow / BALL_REGROW).clamp(0.0, 1.0)
    }
}

/// The bunker kinds, run by `think` here rather than the common code.
pub fn owns(kind: MobKind) -> bool {
    matches!(kind, MobKind::Wireworm | MobKind::Bombardier | MobKind::Ramhorn | MobKind::Sexton)
}

/// A wireworm under the floor: nothing there for claw or spit.
pub(crate) fn buried(m: &Mob) -> bool {
    m.kind == MobKind::Wireworm && m.bunker.rise < OPEN_RISE
}

/// What a blow does to a bunker mob.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Parry {
    /// It lands.
    Open,
    /// Buried: nothing there to hit.
    Hidden,
    /// Glances off the armor.
    Armor,
}

/// A blow from a chameleon at `ppos` against the mob at `pos`: does it land?
/// A buried wireworm can't be hit; a ramhorn's front glances blows off,
/// unless it's dazed or the chameleon is up above its back.
pub fn parry(m: &mut Mob, pos: Vec2, ppos: Vec2) -> Parry {
    match m.kind {
        MobKind::Wireworm if m.bunker.rise < OPEN_RISE => Parry::Hidden,
        MobKind::Ramhorn if m.bunker.dazed <= 0.0 && (ppos.x - pos.x) * m.facing > 0.0 && ppos.y < pos.y + m.spec.radius * 0.8 => {
            m.flash = 0.6;
            m.bunker.clank = 1.0;
            m.notice = Notice::Aware;
            Parry::Armor
        }
        _ => Parry::Open,
    }
}

/// What a bunker mob sees of the world this frame.
pub struct World<'a> {
    pub terrain: &'a Terrain,
    pub p: &'a Player,
    pub ppos: Vec2,
    pub t: f32,
    pub dt: f32,
}

/// What it did this frame (the new state is already set on the mob).
pub struct Step {
    /// Its new center.
    pub pos: Vec2,
    /// It hurt the chameleon, from here.
    pub bite: Option<Vec2>,
}

pub fn think(m: &mut Mob, pos: Vec2, sees: bool, w: &World, bleed: &mut MessageWriter<Bleed>, shake: &mut Shake) -> Step {
    m.bunker.clank = (m.bunker.clank - w.dt * 4.0).max(0.0);
    match m.kind {
        MobKind::Wireworm => wireworm(m, sees, w),
        MobKind::Bombardier => bombardier(m, pos, sees, w, bleed),
        MobKind::Ramhorn => ramhorn(m, pos, sees, w, shake),
        _ => sexton(m, pos, sees, w),
    }
}

fn ease(rate: f32, dt: f32) -> f32 {
    1.0 - (-rate * dt).exp()
}

fn sign(x: f32) -> f32 {
    if x >= 0.0 { 1.0 } else { -1.0 }
}

/// Walks by its velocity along the ground, in whole steps. The new center, and whether it was blocked.
fn stride(m: &mut Mob, pos: Vec2, w: &World) -> (Vec2, bool) {
    m.stride += w.dt * m.vel.x.abs() * 0.12;
    m.bunker.carry += m.vel.x * w.dt;
    if m.bunker.carry.abs() < MIN_STEP {
        return (pos, false);
    }
    let dx = std::mem::take(&mut m.bunker.carry);
    let (feet, blocked) = walk(w.terrain, pos - Vec2::Y * m.spec.foot, dx, m.home.x, m.spec);
    (feet + Vec2::Y * m.spec.foot, blocked)
}

/// Not fighting: about its business, or (just surprised) frozen or running off.
fn about(m: &mut Mob, pos: Vec2, w: &World) {
    match m.notice {
        Notice::Shock { how, .. } => flinch(m, how, w.ppos - pos, w.dt),
        _ => idle(m, pos, w.t, w.dt),
    }
}

/// Knocked back by a blow, then a short recovery.
fn hurt(m: &mut Mob, s: f32, dt: f32) -> State {
    m.vel.x *= 1.0 - 6.0 * dt;
    if s < 0.3 { State::Hurt(s + dt) } else { State::Recover(m.spec.recover * 0.5) }
}

/// A walker bumped into something: patrols turn back, charges stop.
fn bumped(m: &mut Mob) {
    m.vel.x = 0.0;
    if m.state == State::Idle {
        m.wander = (-m.wander.0, m.rng.range(1.0, 2.0));
    }
}

// --- Wireworm ---

fn wireworm(m: &mut Mob, sees: bool, w: &World) -> Step {
    let spec = m.spec;
    let dt = w.dt;
    let mut feet = *m.bunker.feet.get_or_insert(m.home - Vec2::Y * spec.foot);
    let dx = w.ppos.x - feet.x;
    let above = w.ppos.y - feet.y;
    // On (or just over) its floor.
    let over = above > -20.0 && above < 160.0;
    let mut bite = None;
    let mut want = 0.0;
    m.state = match m.state {
        State::Hurt(s) => {
            if s < 0.2 { State::Hurt(s + dt) } else { State::Recover(0.0) }
        }
        State::Idle => {
            m.bunker.rise = (m.bunker.rise - dt / SINK).max(0.0);
            m.cooldown -= dt;
            if sees && over && dx.abs() < UNDER && m.cooldown <= 0.0 && m.bunker.rise <= 0.0 {
                m.bunker.struck = false;
                State::Windup(0.0)
            } else {
                if sees && over {
                    // Tunnels after it (also while rearming: it stays under you).
                    want = if dx.abs() < UNDER * 0.5 { 0.0 } else { sign(dx) * spec.attack_speed };
                } else if !matches!(m.notice, Notice::Shock { .. }) {
                    // Wanders about under its patch of floor.
                    m.wander.1 -= dt;
                    if m.wander.1 <= 0.0 {
                        let away = feet.x - m.home.x;
                        m.wander = if m.rng.chance(0.5) {
                            (0.0, m.rng.range(1.0, 3.0))
                        } else if away.abs() > spec.leash * 0.5 {
                            (-sign(away), m.rng.range(1.0, 2.0))
                        } else {
                            (if m.rng.chance(0.5) { 1.0 } else { -1.0 }, m.rng.range(0.8, 2.0))
                        };
                    }
                    want = m.wander.0 * spec.speed;
                }
                State::Idle
            }
        }
        // The tell: the crumbs shake, the mandibles peek out. It commits to the spot.
        State::Windup(s) => {
            m.bunker.rise = PEEK * (s / spec.windup).min(1.0);
            if s < spec.windup { State::Windup(s + dt) } else { State::Attack(0.0) }
        }
        State::Attack(s) => {
            m.bunker.rise = (PEEK + s / spec.attack_time).min(1.0);
            let top = m.bunker.height(spec);
            if !m.bunker.struck && w.p.dead.is_none() && dx.abs() < spec.radius + COLUMN && above > -10.0 && above < top + 18.0 {
                bite = Some(feet);
                m.bunker.struck = true;
            }
            if s < spec.attack_time + HOLD_TOP { State::Attack(s + dt) } else { State::Recover(0.0) }
        }
        // Reared up and thrashing, then back under.
        State::Recover(s) => {
            if s < spec.recover {
                m.bunker.rise = 1.0;
                State::Recover(s + dt)
            } else {
                m.bunker.rise = (m.bunker.rise - dt / SINK).max(0.0);
                if m.bunker.rise <= 0.0 {
                    m.cooldown = m.rng.range(REARM.0, REARM.1);
                    State::Idle
                } else {
                    State::Recover(s + dt)
                }
            }
        }
        dead @ State::Dead { .. } => dead,
    };

    // Under the floor it tunnels; out of it, it's stuck where it is.
    if m.bunker.rise <= 0.0 {
        m.vel.x += (want - m.vel.x) * ease(10.0, dt);
        let fake = feet + Vec2::Y * spec.foot;
        let (moved, blocked) = stride(m, fake, w);
        feet = moved - Vec2::Y * spec.foot;
        if blocked {
            bumped(m);
        }
        if m.vel.x.abs() > 5.0 {
            m.facing = sign(m.vel.x);
        }
    } else {
        m.vel.x = 0.0;
        m.bunker.carry = 0.0;
        if sees {
            m.facing = sign(dx);
        }
    }
    m.bunker.feet = Some(feet);
    Step { pos: feet + Vec2::Y * m.bunker.height(spec), bite }
}

// --- Bombardier ---

/// The spray goes out behind it, turned toward the chameleon as far as it can.
fn spray_dir(tip: Vec2, ppos: Vec2, facing: f32) -> Vec2 {
    let back = Vec2::new(-facing, 0.0);
    let a = back.angle_to((ppos - tip).normalize_or(back)).clamp(-SPRAY_TURN, SPRAY_TURN);
    Vec2::from_angle(a).rotate(back)
}

fn bombardier(m: &mut Mob, pos: Vec2, sees: bool, w: &World, bleed: &mut MessageWriter<Bleed>) -> Step {
    let spec = m.spec;
    let dt = w.dt;
    let to = w.ppos - pos;
    let side = sign(to.x);
    let tip = pos + Vec2::new(NOZZLE.x * m.facing, NOZZLE.y) * spec.scale;
    let back = Vec2::new(-m.facing, -0.4);
    let drip = |bleed: &mut MessageWriter<Bleed>, speed: f32| {
        bleed.write(Bleed { pos: tip, dir: back, spread: 0.6, speed, amount: 1, fluid: spec.blood, sticky: false });
    };
    let mut bite = None;
    m.state = match m.state {
        State::Hurt(s) => hurt(m, s, dt),
        State::Idle if sees => {
            m.cooldown -= dt;
            if m.facing != -side {
                // Turning its back on the chameleon.
                m.vel.x *= 1.0 - 8.0 * dt;
                m.bunker.turn += dt;
                if m.bunker.turn > QUICK_TURN {
                    m.facing = -side;
                    m.bunker.turn = 0.0;
                }
                State::Idle
            } else {
                m.bunker.turn = 0.0;
                let gap = to.x.abs();
                if gap < SPRAY_GAP + 25.0 && to.y.abs() < 70.0 && m.cooldown <= 0.0 {
                    m.vel.x = 0.0;
                    m.bunker.struck = false;
                    State::Windup(0.0)
                } else {
                    // Backs into range rear first; too close, it scuttles off head first.
                    let want = if gap > SPRAY_GAP + 25.0 {
                        side * BACK_UP
                    } else if gap < SPRAY_GAP - 50.0 {
                        -side * SCUTTLE
                    } else {
                        0.0
                    };
                    m.vel.x += (want * spec.speed - m.vel.x) * ease(8.0, dt);
                    State::Idle
                }
            }
        }
        State::Idle => {
            m.bunker.turn = 0.0;
            about(m, pos, w);
            if m.vel.x.abs() > 8.0 {
                m.facing = sign(m.vel.x);
            }
            State::Idle
        }
        // The tell: the tail tips up, hissing and dripping.
        State::Windup(s) => {
            m.vel.x *= 1.0 - 8.0 * dt;
            if m.rng.chance(dt * 14.0) {
                drip(bleed, 60.0);
            }
            if s < spec.windup {
                State::Windup(s + dt)
            } else {
                m.bunker.aim = spray_dir(tip, w.ppos, m.facing);
                State::Attack(0.0)
            }
        }
        // The burst: scalding pops along its aim, pushing it forward a little.
        State::Attack(s) => {
            let dir = m.bunker.aim;
            let speed = spec.attack_speed * m.rng.range(0.55, 1.0);
            bleed.write(Bleed { pos: tip, dir, spread: SPRAY_FAN * 0.6, speed, amount: 2, fluid: spec.blood, sticky: true });
            if !m.bunker.struck && w.p.dead.is_none() && chameleon::in_fan(tip, dir, SPRAY_REACH, SPRAY_FAN, w.ppos, SCALD_R) {
                bite = Some(tip);
                m.bunker.struck = true;
            }
            m.vel.x = m.facing * 30.0;
            if s < spec.attack_time {
                State::Attack(s + dt)
            } else {
                m.cooldown = m.rng.range(RELOAD.0, RELOAD.1);
                State::Recover(0.0)
            }
        }
        // Spent: steaming and dripping.
        State::Recover(s) => {
            m.vel.x *= 1.0 - 6.0 * dt;
            if m.rng.chance(dt * 3.0) {
                drip(bleed, 30.0);
            }
            if s < spec.recover { State::Recover(s + dt) } else { State::Idle }
        }
        dead @ State::Dead { .. } => dead,
    };
    let (next, blocked) = stride(m, pos, w);
    if blocked {
        bumped(m);
    }
    if matches!(m.state, State::Windup(_)) {
        m.stride += dt * 25.0;
    }
    Step { pos: next, bite }
}

// --- Ramhorn ---

fn ramhorn(m: &mut Mob, pos: Vec2, sees: bool, w: &World, shake: &mut Shake) -> Step {
    let spec = m.spec;
    let dt = w.dt;
    let to = w.ppos - pos;
    let side = sign(to.x);
    m.bunker.dazed = (m.bunker.dazed - dt).max(0.0);
    let mut bite = None;
    m.state = match m.state {
        State::Hurt(s) => hurt(m, s, dt),
        State::Idle if sees => {
            m.cooldown -= dt;
            if m.facing != side {
                // Heavy: turning round takes a while, showing its back.
                m.vel.x *= 1.0 - 6.0 * dt;
                m.bunker.turn += dt;
                if m.bunker.turn > SLOW_TURN {
                    m.facing = side;
                    m.bunker.turn = 0.0;
                }
                State::Idle
            } else if to.y.abs() < 60.0 && m.cooldown <= 0.0 {
                m.bunker.turn = 0.0;
                m.bunker.struck = false;
                State::Windup(0.0)
            } else {
                m.bunker.turn = 0.0;
                m.vel.x += (side * spec.speed - m.vel.x) * ease(6.0, dt);
                State::Idle
            }
        }
        State::Idle => {
            m.bunker.turn = 0.0;
            about(m, pos, w);
            if m.vel.x.abs() > 8.0 {
                m.facing = sign(m.vel.x);
            }
            State::Idle
        }
        // The tell: horn down, pawing the ground.
        State::Windup(s) => {
            m.vel.x = -m.facing * 12.0;
            m.stride += dt * 30.0;
            if s < spec.windup { State::Windup(s + dt) } else { State::Attack(0.0) }
        }
        // Committed: straight along the ground, tossing the chameleon and thundering on.
        State::Attack(s) => {
            m.vel.x = m.facing * spec.attack_speed * (0.4 + 0.6 * (s / SPEED_UP).min(1.0));
            let ahead = to.x * m.facing;
            if !m.bunker.struck && w.p.dead.is_none() && ahead > -spec.radius * 0.5 && ahead < HORN * spec.scale && to.y.abs() < spec.foot + 22.0 {
                bite = Some(pos);
                m.bunker.struck = true;
            }
            if s < spec.attack_time { State::Attack(s + dt) } else { State::Recover(0.0) }
        }
        // Skids to a stop (or sits dazed, legs flailing).
        State::Recover(s) => {
            m.vel.x *= 1.0 - 3.5 * dt;
            if m.bunker.dazed > 0.0 {
                m.stride += dt * 20.0;
                State::Recover(s)
            } else if s < spec.recover {
                State::Recover(s + dt)
            } else {
                m.cooldown = m.rng.range(BREATHER.0, BREATHER.1);
                State::Idle
            }
        }
        dead @ State::Dead { .. } => dead,
    };
    let speed = m.vel.x.abs();
    let (next, blocked) = stride(m, pos, w);
    if blocked {
        let charging = matches!(m.state, State::Attack(_) | State::Recover(_));
        let nose = pos + Vec2::new(m.facing * (spec.radius * 0.8 + 6.0), spec.foot * 0.2);
        if charging && speed > RAM_SPEED && w.terrain.box_hits(nose, Vec2::new(4.0, spec.foot * 0.5)) {
            // Rammed a wall: dazed, bounced back a little.
            m.bunker.dazed = DAZE;
            m.vel.x = -m.facing * 90.0;
            m.state = State::Recover(0.0);
            shake.add(0.35);
        } else {
            bumped(m);
            if matches!(m.state, State::Attack(_)) {
                m.state = State::Recover(0.0);
            }
        }
    }
    Step { pos: next, bite }
}

// --- Sexton ---

fn sexton(m: &mut Mob, pos: Vec2, sees: bool, w: &World) -> Step {
    let spec = m.spec;
    let dt = w.dt;
    let to = w.ppos - pos;
    let side = sign(to.x);
    m.bunker.regrow = (m.bunker.regrow - dt).max(0.0);
    m.state = match m.state {
        State::Hurt(s) => hurt(m, s, dt),
        State::Idle if sees => {
            m.cooldown -= dt;
            m.facing = side;
            let gap = to.x.abs();
            let ready = m.bunker.regrow <= 0.0;
            if ready && m.cooldown <= 0.0 && to.y.abs() < 60.0 && gap < KICK_RANGE {
                m.vel.x = 0.0;
                State::Windup(0.0)
            } else {
                // Gathering a new ball, it keeps its distance (backing off, still
                // facing the chameleon); with one, it edges closer.
                let want = if !ready && gap < KEEP_AWAY {
                    -side
                } else if ready && gap > KICK_RANGE {
                    side * 0.8
                } else {
                    0.0
                };
                m.vel.x += (want * spec.speed - m.vel.x) * ease(8.0, dt);
                State::Idle
            }
        }
        State::Idle => {
            about(m, pos, w);
            if m.vel.x.abs() > 8.0 {
                m.facing = sign(m.vel.x);
            }
            State::Idle
        }
        // The tell: rears back, the ball pulled in.
        State::Windup(s) => {
            m.vel.x *= 1.0 - 8.0 * dt;
            if s < spec.windup {
                State::Windup(s + dt)
            } else {
                let at = pos + Vec2::new(BALL_AT.x * m.facing, BALL_AT.y) * spec.scale;
                m.bunker.kick = Some((at, m.facing * spec.attack_speed));
                m.bunker.regrow = BALL_REGROW;
                State::Attack(0.0)
            }
        }
        // The shove: a lurch after the ball.
        State::Attack(s) => {
            m.vel.x = m.facing * 90.0 * (1.0 - s / spec.attack_time).max(0.0);
            if s < spec.attack_time { State::Attack(s + dt) } else { State::Recover(0.0) }
        }
        State::Recover(s) => {
            m.vel.x *= 1.0 - 6.0 * dt;
            if s < spec.recover {
                State::Recover(s + dt)
            } else {
                m.cooldown = m.rng.range(BREATHER_SEXTON.0, BREATHER_SEXTON.1);
                State::Idle
            }
        }
        dead @ State::Dead { .. } => dead,
    };
    let (next, blocked) = stride(m, pos, w);
    if blocked {
        bumped(m);
    }
    Step { pos: next, bite: None }
}

// --- Rolling balls ---

/// A ball of carrion and rubble shoved by a sexton.
#[derive(Component)]
pub struct Ball {
    vel: Vec2,
    airborne: bool,
    life: f32,
    /// Seconds spent crawling along.
    slow: f32,
    bounces: u32,
    /// Slashed back: hurts mobs now, not the chameleon.
    returned: bool,
    /// Last slash that hit it.
    hit_by: u32,
}

/// Launches the balls sexton shoved, rolls them along the ground (downhill
/// faster, bouncing off walls), sends them back when slashed, and bursts
/// them on whatever they hit (or once they stall, bounce too often or age).
pub(super) fn balls(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    time: Res<Time>,
    terrain: Res<Terrain>,
    mut shake: ResMut<Shake>,
    mut stop: ResMut<HitStop>,
    mut bleed: MessageWriter<Bleed>,
    mut hits: MessageWriter<SpitHit>,
    player: Single<(&mut Player, &Transform), (Without<Mob>, Without<Ball>)>,
    anim: Single<&ChameleonAnim>,
    mut mobs: Query<(Entity, &mut Mob, &Transform), Without<Ball>>,
    mut balls: Query<(Entity, &mut Ball, &mut Transform), (Without<Mob>, Without<Player>)>,
) {
    let dt = time.delta_secs().min(1.0 / 30.0);
    let (mut p, ptf) = player.into_inner();
    let ppos = ptf.translation.truncate();
    let r = BALL_R * SEXTON.scale;

    for (_, mut m, _) in &mut mobs {
        if let Some((at, speed)) = m.bunker.kick.take() {
            let e = mob_art::bunker::ball(&mut commands, &mut meshes, &mut materials, at, SEXTON.scale);
            let ball = Ball { vel: Vec2::new(speed, 0.0), airborne: false, life: BALL_LIFE, slow: 0.0, bounces: 0, returned: false, hit_by: 0 };
            commands.entity(e).insert((ball, MobStuff));
        }
    }

    for (e, mut b, mut tf) in &mut balls {
        let pos = tf.translation.truncate();
        b.life -= dt;
        // Slashed: sent back the way the claws swing.
        if anim.striking() && b.hit_by != anim.slash_id && anim.reaches(ppos, p.facing, pos, r) {
            b.hit_by = anim.slash_id;
            let x = anim.blow_dir(ppos, pos).x;
            let dir = if x.abs() > 0.2 { x.signum() } else { sign(pos.x - ppos.x) };
            b.vel = Vec2::new(dir * BALL_RETURN, if b.airborne { b.vel.y } else { 0.0 });
            b.returned = true;
            b.bounces = 0;
            b.life = BALL_LIFE;
            shake.add(0.15);
            stop.add(0.05);
            bleed.write(Bleed { pos, dir: Vec2::new(-dir, 0.6), spread: 0.8, speed: 240.0, amount: 6, fluid: Fluid::Red, sticky: true });
        }

        let feet = pos - Vec2::Y * r;
        let dir = sign(b.vel.x);
        let mut next = pos;
        let mut wall = false;
        if b.airborne {
            b.vel.y -= GRAVITY * dt;
            next = pos + b.vel * dt;
            if terrain.solid_at(Vec2::new(next.x + dir * r * 0.7, pos.y)) {
                wall = true;
                next.x = pos.x;
            }
            if let Some(top) = terrain.landing(pos, next, r) {
                next.y = top + r;
                b.vel.y = 0.0;
                b.airborne = false;
            }
        } else {
            // Rolls downhill, slows on the flat.
            let slope = terrain.slope_at(feet).unwrap_or(0.0);
            b.vel.x -= slope / (1.0 + slope * slope).sqrt() * BALL_PULL * dt;
            b.vel.x *= 1.0 - ROLL_DRAG * dt;
            let nx = pos.x + b.vel.x * dt;
            let probe = Vec2::new(nx, feet.y + STEP_UP);
            if terrain.solid_at(probe) || terrain.solid_at(Vec2::new(nx + dir * r * 0.7, pos.y + r * 0.5)) {
                wall = true;
            } else {
                match terrain.surface(probe, 2.0 * STEP_UP) {
                    Some(y) => next = Vec2::new(nx, y + r),
                    // Over the edge.
                    None => {
                        next = Vec2::new(nx, pos.y);
                        b.vel.y = 0.0;
                        b.airborne = true;
                    }
                }
            }
        }
        if wall {
            b.vel.x = -b.vel.x * BOUNCE;
            b.bounces += 1;
            shake.add(0.08);
            bleed.write(Bleed { pos: pos + Vec2::new(dir * r, 0.0), dir: Vec2::new(-dir, 0.5), spread: 0.7, speed: 160.0, amount: 3, fluid: Fluid::Red, sticky: true });
        }
        tf.translation.x = next.x;
        tf.translation.y = next.y;
        tf.rotate_z(-(next.x - pos.x) / r);

        let speed = b.vel.x.abs();
        b.slow = if !b.airborne && speed < BALL_SLOW { b.slow + dt } else { 0.0 };
        let mut burst = b.life <= 0.0 || b.slow > STALL || b.bounces > MAX_BOUNCES || next.y < terrain.bounds.min.y - 600.0;
        if !b.returned && p.dead.is_none() && speed > 60.0 && next.distance(ppos) < r + BALL_HIT_R {
            boss::hurt_player(&mut p, ppos, next, SEXTON.damage, &mut bleed, (&mut shake, &mut stop));
            burst = true;
        }
        if b.returned && !burst {
            let hit = mobs.iter().find(|(_, m, mtf)| {
                !matches!(m.state, State::Dead { .. })
                    && !(m.kind == MobKind::Wireworm && m.bunker.rise < OPEN_RISE)
                    && mtf.translation.truncate().distance(next) < r + m.spec.radius
            });
            if let Some((target, _, _)) = hit {
                hits.write(SpitHit { target, dir: Vec2::new(sign(b.vel.x), 0.2).normalize() });
                burst = true;
            }
        }
        if burst {
            // Falls apart: rotten meat and grit.
            bleed.write(Bleed { pos: next, dir: Vec2::Y, spread: 2.2, speed: 260.0, amount: 16, fluid: Fluid::Red, sticky: true });
            commands.entity(e).despawn();
        }
    }
}
