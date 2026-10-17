//! The buried grand central station (`Style::Subway`): the rat king's court.
//! Rat packs (`rat.rs`) are the heart of it; around them live:
//! - **Plague ticks** (`Moves::Latch`): fat rat ticks questing on the floor,
//!   front legs waving. Close enough, one rears up (the tell) and leaps at the
//!   chameleon; landing on it, it holds on and rides along, feeding and
//!   swelling, and bites deep (1 damage) once it's full, then drops off sated.
//!   Slash it off before that. A missed leap leaves it on its back, legs
//!   kicking (the window).
//! - **Ballast moles** (`Moves::Burrow`): naked mole rats tunneling under the
//!   floor. Down there only a moving heap of gravel shows, and nothing touches
//!   them (claw, spit, live rail). They feel footsteps: with the chameleon on
//!   the ground about their level, they tunnel to where it's walking to; the
//!   gravel heaves and shakes (the tell), then they erupt with a bite right
//!   there, committed. Out, they're dazed a moment (the window) before digging
//!   back in. Unaware, one pokes its head out for a look now and then. One
//!   lured into coming up on the live rail gets shocked like anything else.
//! - **Rat knots** (`Moves::Tumble`): a rat king in the making, rats whose
//!   tails knotted together into one squirming ball. It can't agree where to
//!   go and lurches about, until it sees the chameleon on its level: it rocks
//!   back squealing (the tell), then rolls at it, gathering speed, committed
//!   to the way it set off. It can't steer or stop short: it overshoots,
//!   brakes, and sits dazed while the rats bicker (the window). Rolling into a
//!   wall stuns it longer. It rolls through hits. Killed, the knot tears
//!   apart and two wounded rats scramble out alive (`split`).

use bevy::prelude::*;

use super::{fits, hop, walk, Mob, MobKind, MobSpec, Moves, Notice, State, GRAVITY};
use crate::{
    gore::Fluid,
    level::Platform,
    mob_art,
    player::{self, Player},
    terrain::Terrain,
};

pub(super) static PLAGUE_TICK: MobSpec = MobSpec {
    moves: Moves::Latch,
    hp: 1,
    scale: 1.5,
    radius: 11.0,
    foot: 3.6 * 1.5,
    // Full of rat blood (and soon of the chameleon's).
    blood: Fluid::Red,
    // Crawling closer, questing.
    speed: 14.0,
    sense: 260.0,
    windup: 0.55,
    // Fastest leap.
    attack_speed: 420.0,
    // Feeding, once it holds on, before the bite.
    attack_time: 2.2,
    recover: 1.1,
    leash: 220.0,
    damage: 1,
    stubborn: false,
    initiative: 0.9,
    veteran: false,
    // 1 hp, but it rides you and bites.
    food: 0.35,
};

pub(super) static BALLAST_MOLE: MobSpec = MobSpec {
    moves: Moves::Burrow,
    hp: 2,
    scale: 1.6,
    radius: 16.0,
    foot: 5.0 * 1.6,
    blood: Fluid::Red,
    // Tunneling about.
    speed: 45.0,
    sense: 420.0,
    // The gravel heaving before it comes up.
    windup: 0.6,
    // Tunneling after the chameleon (it can be outrun).
    attack_speed: 170.0,
    // Coming up: the bite.
    attack_time: 0.35,
    // Dazed out in the open.
    recover: 1.4,
    leash: 360.0,
    damage: 1,
    // Once it heaves, it comes up whatever happens.
    stubborn: true,
    initiative: 0.8,
    veteran: false,
    // 2 hp, out of reach most of the time.
    food: 0.75,
};

pub(super) static RAT_KNOT: MobSpec = MobSpec {
    moves: Moves::Tumble,
    hp: 4,
    scale: 1.6,
    radius: 24.0,
    foot: mob_art::KNOT_FOOT * 1.6,
    blood: Fluid::Red,
    // Lurching about.
    speed: 40.0,
    sense: 380.0,
    // Rocking back, squealing.
    windup: 0.8,
    // Top rolling speed.
    attack_speed: 380.0,
    // Rolling before it gives up and brakes.
    attack_time: 2.0,
    // Dazed, bickering.
    recover: 1.5,
    leash: 380.0,
    damage: 1,
    stubborn: true,
    initiative: 1.2,
    veteran: true,
    // 4 hp, rolls through hits (the rats that get out are nothing to eat).
    food: 1.25,
};

/// The region's ecosystem, whatever boss owns the slice.
pub(super) fn menu() -> Vec<(MobKind, f32)> {
    vec![
        (MobKind::Rat, 0.3),
        (MobKind::PlagueTick, 0.2),
        (MobKind::BallastMole, 0.18),
        (MobKind::RatKnot, 0.12),
        // Rat fleas.
        (MobKind::Flea, 0.12),
        // Carrion flies over what the rats leave.
        (MobKind::Fly, 0.08),
    ]
}

/// Ticks leap from this close (and not at a chameleon far below them).
const LEAP_RANGE: f32 = 190.0;
const LEAP_TIME: f32 = 0.5;
/// Where a tick can hold on: within this much of the chameleon's center
/// (x either way; y from `HOLD_LOW` below to `HOLD.y` above).
const HOLD: Vec2 = Vec2::new(player::HALF.x * 0.7, player::HALF.y * 0.9);
const HOLD_LOW: f32 = player::HALF.y * 0.3;
/// A tick digests: its swelling goes down by this much per second.
const DIGEST: f32 = 0.06;
/// Moles: below this far out of the ground nothing can touch them.
const DUG_IN: f32 = 0.4;
/// Moles feel the chameleon's steps within this height of their level.
const FEEL_HEIGHT: f32 = 110.0;
/// Moles come up where the chameleon will be after this share of the windup.
const LEAD: f32 = 0.8;
/// Moles heave when that spot is this close.
const STRIKE_X: f32 = 22.0;
/// Moles bite what's over the hole up to this high.
const BITE_HEIGHT: f32 = 70.0;
/// Rest under the ground after a strike before hunting again.
const REST: f32 = 1.2;
/// Unaware moles: chance to stop and peek out each time they choose where to go, and how far out.
const PEEK_CHANCE: f32 = 0.3;
const PEEK: f32 = 0.55;
/// The bulge while it heaves.
const BULGE: f32 = 0.15;
/// How fast a mole comes up and goes back down (rates).
const ERUPT: f32 = 22.0;
const SINK: f32 = 4.0;
/// Knots roll at the chameleon from this far on its level (± `ROLL_HEIGHT`).
const ROLL_RANGE: f32 = 300.0;
const ROLL_HEIGHT: f32 = 70.0;
/// Rolling: speeding up, braking, and how far past the chameleon before it brakes.
const ROLL_ACCEL: f32 = 1000.0;
const BRAKE: f32 = 520.0;
const OVERSHOOT: f32 = 50.0;
/// Into a wall faster than this: stunned `CRASH_DAZE` longer.
const CRASH_SPEED: f32 = 160.0;
const CRASH_DAZE: f32 = 0.9;
/// Rats that get out of a dead knot, and how much life they have left.
const SURVIVOR_GAP: f32 = 18.0;
const SURVIVOR_HP: i32 = 1;
/// The smallest step a crawler takes (see `rat::Mind::stride`).
const MIN_STEP: f32 = 4.0;

/// What the subway's vermin have in mind (kept on their `Mob`).
#[derive(Default)]
pub(super) struct Mind {
    /// Moles: how far out of the ground, 0 (under it) .. 1 (out).
    out: f32,
    /// Moles: poking its head out for a look (unaware), and rest before hunting again.
    peek: f32,
    rest: f32,
    /// Ticks: where it holds on to the chameleon (x along the chameleon's
    /// facing, from its center), while it feeds.
    latch: Option<Vec2>,
    /// Ticks: on its back after a missed leap.
    flipped: bool,
    /// Ticks: 0..1, how full of blood.
    gorge: f32,
    /// Ticks: the body's tilt while holding on (mouthparts into the hide).
    tilt: f32,
    /// Knots: how far the ball has turned (radians), the way it rolls.
    spin: f32,
    roll: f32,
    /// Moles and knots: already bit on this strike / roll.
    struck: bool,
    /// Knots: the survivors already got out of the dead knot.
    split: bool,
    /// Movement saved up until it makes a whole step.
    carry: f32,
}

impl Mind {
    /// Moves in steps of at least `MIN_STEP` (creeping by fractions, the body
    /// box catches on the edges of pixel steps).
    fn stride(&mut self, dx: f32) -> f32 {
        self.carry += dx;
        if self.carry.abs() < MIN_STEP {
            return 0.0;
        }
        std::mem::take(&mut self.carry)
    }
}

impl Mob {
    /// A burrower under the ground: no claw, spit or live rail reaches it.
    pub(crate) fn dug_in(&self) -> bool {
        self.spec.moves == Moves::Burrow && self.subway.out < DUG_IN && !matches!(self.state, State::Dead { .. })
    }

    /// Moles: how far out of the ground, 0..1 (all of a corpse).
    pub(crate) fn mole_out(&self) -> f32 {
        if matches!(self.state, State::Dead { .. }) { 1.0 } else { self.subway.out }
    }

    /// Knots: how far the ball has turned, in its own (facing +X) frame.
    pub(crate) fn ball_spin(&self) -> f32 {
        self.subway.spin
    }

    /// Ticks: 0..1, how full of blood.
    pub(crate) fn tick_gorge(&self) -> f32 {
        self.subway.gorge
    }

    /// Ticks: holding on to the chameleon.
    pub(crate) fn tick_latched(&self) -> bool {
        self.subway.latch.is_some()
    }

    /// Ticks: on its back.
    pub(crate) fn tick_flipped(&self) -> bool {
        self.subway.flipped
    }

    /// Ticks: the body's tilt while holding on.
    pub(crate) fn tick_tilt(&self) -> f32 {
        self.subway.tilt
    }
}

/// What they see of the world this frame.
pub(super) struct Scene<'a> {
    pub terrain: &'a Terrain,
    pub p: &'a Player,
    pub ppos: Vec2,
    pub t: f32,
    pub dt: f32,
}

/// What it did this frame: its new center, and whether it bit the chameleon.
pub(super) struct Step {
    pub next: Vec2,
    pub bite: bool,
}

/// The movement patterns this module runs.
pub(super) fn runs(moves: Moves) -> bool {
    matches!(moves, Moves::Latch | Moves::Burrow | Moves::Tumble)
}

pub(super) fn think(
    m: &mut Mob,
    pos: Vec2,
    sees: bool,
    touching: bool,
    w: &Scene,
    platforms: &Query<(&Platform, &Transform), (Without<Mob>, Without<Player>)>,
) -> Step {
    let step = match m.spec.moves {
        Moves::Latch => tick(m, pos, sees, w, platforms),
        Moves::Burrow => mole(m, pos, sees, w),
        Moves::Tumble => knot(m, pos, sees, touching, w),
        _ => Step { next: pos, bite: false },
    };
    let scrabble = if matches!(m.state, State::Windup(_)) { 25.0 } else { 0.0 };
    m.stride += w.dt * (m.vel.x.abs() * 0.12 + scrabble);
    step
}

/// Along the ground by its velocity, in whole steps, stopping at walls,
/// drops and its leash. The new center, and whether it was blocked.
fn crawl(m: &mut Mob, pos: Vec2, terrain: &Terrain, dt: f32) -> (Vec2, bool) {
    let dx = m.subway.stride(m.vel.x * dt);
    if dx == 0.0 {
        return (pos, false);
    }
    let (feet, blocked) = walk(terrain, pos - Vec2::Y * m.spec.foot, dx, m.home.x, m.spec);
    if blocked {
        m.subway.carry = 0.0;
    }
    (feet + Vec2::Y * m.spec.foot, blocked)
}

fn tick(m: &mut Mob, pos: Vec2, sees: bool, w: &Scene, platforms: &Query<(&Platform, &Transform), (Without<Mob>, Without<Player>)>) -> Step {
    let spec = m.spec;
    let dt = w.dt;
    let ease = |rate: f32| 1.0 - (-rate * dt).exp();
    let to = w.ppos - pos;
    let side = if to.x >= 0.0 { 1.0 } else { -1.0 };

    if let Some(at) = m.subway.latch {
        if let (State::Attack(s), None) = (m.state, w.p.dead) {
            // Riding along, feeding.
            let off = Vec2::new(at.x * w.p.facing, at.y);
            let next = w.ppos + off;
            m.vel = Vec2::ZERO;
            m.facing = if off.x > 0.0 { -1.0 } else { 1.0 };
            m.subway.tilt = Vec2::new(-off.x * m.facing, -off.y).to_angle().clamp(-1.0, 1.0);
            m.subway.gorge = (s / spec.attack_time).min(1.0);
            if s < spec.attack_time {
                m.state = State::Attack(s + dt);
                return Step { next, bite: false };
            }
            // Full: one deep bite, and it lets go (and lives where it falls).
            m.subway.latch = None;
            m.subway.tilt = 0.0;
            m.airborne = true;
            m.vel = Vec2::new(-m.facing * 80.0, 150.0);
            m.home = next;
            m.state = State::Recover(-spec.recover);
            return Step { next, bite: true };
        }
        // Knocked loose, or the chameleon is gone: it falls off.
        m.subway.latch = None;
        m.subway.tilt = 0.0;
        m.airborne = true;
        m.home = pos;
    }
    m.subway.gorge = (m.subway.gorge - DIGEST * dt).max(0.0);

    m.state = match m.state {
        State::Hurt(s) if s < 0.3 => State::Hurt(s + dt),
        State::Hurt(_) => State::Recover(0.0),
        State::Idle if sees && !m.airborne && to.length() < LEAP_RANGE && to.y > -60.0 => {
            m.facing = side;
            m.plan = w.ppos;
            State::Windup(0.0)
        }
        State::Idle => {
            if !m.airborne {
                // Questing: holds still with its front legs out, creeping
                // closer once it knows where the chameleon is.
                let want = if sees { side * spec.speed * 2.5 } else { 0.0 };
                m.vel.x += (want - m.vel.x) * ease(6.0);
                if sees {
                    m.facing = side;
                }
            }
            State::Idle
        }
        State::Windup(s) if s < spec.windup => {
            m.facing = side;
            m.vel.x = 0.0;
            State::Windup(s + dt)
        }
        State::Windup(_) => {
            // A leap at where the chameleon stood when it reared up (following a little).
            let aim = m.plan.lerp(w.ppos, 0.3) - pos;
            let vx = (aim.x / LEAP_TIME).clamp(-spec.attack_speed, spec.attack_speed);
            let vy = ((aim.y + 0.5 * GRAVITY * LEAP_TIME * LEAP_TIME) / LEAP_TIME).clamp(260.0, 640.0);
            m.vel = Vec2::new(vx, vy);
            m.airborne = true;
            State::Attack(0.0)
        }
        State::Attack(s) => {
            let on_it = (to.abs() - player::HALF).max_element() < spec.radius * 0.5;
            if m.airborne && on_it && w.p.dead.is_none() {
                let off = (pos - w.ppos).clamp(Vec2::new(-HOLD.x, -HOLD_LOW), HOLD);
                m.subway.latch = Some(Vec2::new(off.x * w.p.facing, off.y));
                State::Attack(0.0)
            } else if !m.airborne && s > 0.05 {
                // Missed: landed on its back.
                m.subway.flipped = true;
                State::Recover(0.0)
            } else {
                State::Attack(s + dt)
            }
        }
        State::Recover(s) if s < spec.recover => {
            if !m.airborne {
                m.vel.x *= 1.0 - 6.0 * dt;
            }
            State::Recover(s + dt)
        }
        State::Recover(_) => {
            m.subway.flipped = false;
            State::Idle
        }
        dead @ State::Dead { .. } => dead,
    };

    let mut next = pos;
    if !m.airborne && m.vel.x != 0.0 {
        let (n, blocked) = crawl(m, pos, w.terrain, dt);
        if blocked {
            m.vel.x = 0.0;
        }
        next = n;
    }
    Step { next: hop(m, w.terrain, platforms, next, dt), bite: false }
}

fn mole(m: &mut Mob, pos: Vec2, sees: bool, w: &Scene) -> Step {
    let spec = m.spec;
    let dt = w.dt;
    let ease = |rate: f32| 1.0 - (-rate * dt).exp();
    let to = w.ppos - pos;
    // It feels footsteps: a chameleon on the ground, about on its level.
    let feel = sees && w.p.grounded && to.y.abs() < FEEL_HEIGHT;
    if feel || m.notice != Notice::Unaware {
        m.subway.peek = 0.0;
    }
    // Where the chameleon will be by the time it comes up.
    let ahead = w.ppos.x + w.p.vel.x * spec.windup * LEAD;
    let mut bite = false;
    let mut want = 0.0;
    let (mut out, mut rate) = (0.0, SINK);
    m.subway.rest -= dt;
    m.state = match m.state {
        State::Hurt(s) if s < 0.3 => {
            out = 1.0;
            State::Hurt(s + dt)
        }
        State::Hurt(_) => {
            // Ducks back under, and lies low a while.
            m.subway.rest = REST * 1.5;
            State::Idle
        }
        State::Idle if feel && m.subway.rest <= 0.0 && m.subway.out < DUG_IN => {
            let dx = ahead - pos.x;
            if dx.abs() < STRIKE_X {
                m.plan = pos;
                State::Windup(0.0)
            } else {
                want = dx.signum() * spec.attack_speed;
                State::Idle
            }
        }
        State::Idle => {
            m.subway.peek -= dt;
            m.wander.1 -= dt;
            if m.wander.1 <= 0.0 {
                let away = pos.x - m.home.x;
                m.wander = if m.notice == Notice::Unaware && m.rng.chance(PEEK_CHANCE) {
                    m.subway.peek = m.rng.range(0.8, 1.8);
                    (0.0, m.subway.peek + 0.4)
                } else if away.abs() > spec.leash * 0.6 {
                    (-away.signum(), m.rng.range(1.0, 2.5))
                } else if m.rng.chance(0.3) {
                    (0.0, m.rng.range(0.6, 1.5))
                } else {
                    (if m.rng.chance(0.5) { 1.0 } else { -1.0 }, m.rng.range(1.0, 2.5))
                };
            }
            if m.subway.peek > 0.0 {
                out = PEEK;
                rate = 5.0;
            } else {
                want = m.wander.0 * spec.speed;
            }
            State::Idle
        }
        State::Windup(s) => {
            // The gravel heaves and shakes: it's coming up right here.
            out = BULGE;
            if s < spec.windup {
                State::Windup(s + dt)
            } else {
                m.subway.struck = false;
                State::Attack(0.0)
            }
        }
        State::Attack(s) => {
            out = 1.0;
            rate = ERUPT;
            // Bites whatever is over the hole on its way up.
            let over = to.x.abs() < spec.radius + player::HALF.x * 0.6 && to.y > -player::HALF.y && to.y < BITE_HEIGHT;
            if over && !m.subway.struck && w.p.dead.is_none() {
                bite = true;
                m.subway.struck = true;
            }
            if s < spec.attack_time { State::Attack(s + dt) } else { State::Recover(0.0) }
        }
        State::Recover(s) => {
            // Dazed out in the open, shaking the dirt off; then back under.
            out = 1.0;
            if s < spec.recover {
                State::Recover(s + dt)
            } else {
                m.subway.rest = REST;
                State::Idle
            }
        }
        dead @ State::Dead { .. } => dead,
    };
    m.subway.out += (out - m.subway.out) * ease(rate);
    if want != 0.0 {
        m.facing = want.signum();
    } else if sees && m.subway.out > DUG_IN {
        m.facing = if to.x >= 0.0 { 1.0 } else { -1.0 };
    }

    // It only gets about under the ground (out of it, only knocked about).
    if matches!(m.state, State::Hurt(_)) {
        m.vel.x *= 1.0 - 6.0 * dt;
    } else {
        let k = if m.subway.out < DUG_IN { 1.0 } else { 0.0 };
        m.vel.x += (want * k - m.vel.x) * ease(8.0);
    }
    let (next, blocked) = crawl(m, pos, w.terrain, dt);
    if blocked {
        m.vel.x = 0.0;
        if m.state == State::Idle {
            m.wander = (-m.wander.0, m.rng.range(1.0, 2.0));
        }
    }
    Step { next, bite }
}

fn knot(m: &mut Mob, pos: Vec2, sees: bool, touching: bool, w: &Scene) -> Step {
    let spec = m.spec;
    let dt = w.dt;
    let ease = |rate: f32| 1.0 - (-rate * dt).exp();
    let to = w.ppos - pos;
    let side = if to.x >= 0.0 { 1.0 } else { -1.0 };
    let mut bite = false;
    m.state = match m.state {
        State::Hurt(s) if s < 0.3 => {
            m.vel.x *= 1.0 - 5.0 * dt;
            State::Hurt(s + dt)
        }
        State::Hurt(_) => State::Recover(spec.recover * 0.5),
        State::Idle if sees && to.y.abs() < ROLL_HEIGHT && to.x.abs() < ROLL_RANGE => {
            m.facing = side;
            m.plan = w.ppos;
            State::Windup(0.0)
        }
        State::Idle => {
            // The rats pull every which way: a lurch one way, then another
            // (toward the chameleon more often, once it knows where it is).
            m.wander.1 -= dt;
            if m.wander.1 <= 0.0 {
                let away = pos.x - m.home.x;
                let dir = if away.abs() > spec.leash * 0.6 {
                    -away.signum()
                } else if sees {
                    if m.rng.chance(0.7) { side } else { -side }
                } else if m.rng.chance(0.5) {
                    1.0
                } else {
                    -1.0
                };
                m.wander = (dir * m.rng.range(0.5, 1.2), m.rng.range(0.25, 0.8));
            }
            let tug = (w.t * 6.0 + m.seed).sin().max(0.0) * 2.0;
            m.vel.x += (m.wander.0 * spec.speed * tug - m.vel.x) * ease(6.0);
            if sees {
                m.facing = side;
            } else if m.vel.x.abs() > 10.0 {
                m.facing = m.vel.x.signum();
            }
            State::Idle
        }
        State::Windup(s) => {
            // Rocks back, squealing.
            m.vel.x += (-m.facing * 30.0 - m.vel.x) * ease(8.0);
            if s < spec.windup {
                State::Windup(s + dt)
            } else {
                let dx = m.plan.lerp(w.ppos, 0.25).x - pos.x;
                m.subway.roll = if dx >= 0.0 { 1.0 } else { -1.0 };
                m.facing = m.subway.roll;
                m.subway.struck = false;
                State::Attack(0.0)
            }
        }
        State::Attack(s) => {
            let dir = m.subway.roll;
            if touching && !m.subway.struck && w.p.dead.is_none() {
                bite = true;
                m.subway.struck = true;
            }
            let past = (w.ppos.x - pos.x) * dir < -OVERSHOOT;
            if s < spec.attack_time && !past {
                m.vel.x = (m.vel.x + dir * ROLL_ACCEL * dt).clamp(-spec.attack_speed, spec.attack_speed);
                State::Attack(s + dt)
            } else {
                // Can't stop short: it brakes, rolling on (and won't speed up again).
                let v = (m.vel.x.abs() - BRAKE * dt).max(0.0);
                m.vel.x = v * m.vel.x.signum();
                if v < 10.0 { State::Recover(0.0) } else { State::Attack(spec.attack_time.max(s) + dt) }
            }
        }
        State::Recover(s) => {
            m.vel.x *= 1.0 - 4.0 * dt;
            if s < spec.recover { State::Recover(s + dt) } else { State::Idle }
        }
        dead @ State::Dead { .. } => dead,
    };

    let (next, blocked) = crawl(m, pos, w.terrain, dt);
    if blocked {
        let ahead = pos + Vec2::new(m.vel.x.signum() * (spec.radius + 4.0), 0.0);
        let rolling = matches!(m.state, State::Attack(_));
        if rolling && m.vel.x.abs() > CRASH_SPEED && w.terrain.solid_at(ahead) {
            // Into a wall: bounces off, stunned.
            m.vel.x *= -0.3;
            m.state = State::Recover(-CRASH_DAZE);
        } else {
            // A drop or the end of its range: it stops dead there.
            m.vel.x = 0.0;
            match m.state {
                State::Attack(_) => m.state = State::Recover(0.0),
                State::Idle => m.wander = (-m.wander.0, m.rng.range(0.3, 0.8)),
                _ => {}
            }
        }
    }
    // The ball turns as it rolls (clockwise going forward, in its own frame).
    let ball = mob_art::KNOT_BALL * spec.scale;
    m.subway.spin -= (next.x - pos.x) * m.facing / ball;
    Step { next, bite }
}

/// A dead rat knot tears apart: two wounded rats scramble out of it alive
/// (born of it: nothing to eat), bolting from the chameleon.
pub(super) fn split(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    terrain: Res<Terrain>,
    mut mobs: Query<(Entity, &mut Mob, &Transform), Without<crate::terrain_props::Asleep>>,
) {
    let rat = MobKind::Rat.spec();
    for (e, mut m, tf) in &mut mobs {
        if m.spec.moves != Moves::Tumble || m.subway.split || !matches!(m.state, State::Dead { .. }) {
            continue;
        }
        m.subway.split = true;
        let pos = tf.translation.truncate();
        for dir in [-1.0f32, 1.0] {
            let q = pos + Vec2::new(dir * SURVIVOR_GAP, 0.0);
            let feet = [q, pos]
                .into_iter()
                .filter_map(|at| terrain.surface(at, 200.0).map(|y| Vec2::new(at.x, y)))
                .find(|&f| fits(&terrain, f, rat));
            let Some(feet) = feet else { continue };
            let born = mob_art::spawn(&mut commands, &mut meshes, &mut materials, MobKind::Rat, feet + Vec2::Y * rat.foot, Some(e));
            commands.queue(move |world: &mut World| {
                if let Some(mut r) = world.get_mut::<Mob>(born) {
                    r.hp = SURVIVOR_HP;
                    r.facing = dir;
                    r.vel = Vec2::new(dir * 260.0, 0.0);
                    r.state = State::Recover(0.0);
                }
            });
        }
    }
}
