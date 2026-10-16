//! The sunken street's vermin: the giant grasshopper's kin, and what lives in
//! the collapsed soil under the asphalt. Each comes at the chameleon its own way:
//! - **Locusts** (`Moves::BandSweep`) fly in bands of up to `BAND`. Calm, they
//!   mill about over their spot. Hunting, the whole band runs on one clock
//!   (`band clock`): it gathers into a vertical line to one side of the
//!   chameleon (one locust per `LANE`, the lowest skimming the floor), holds it
//!   buzzing (the tell, `windup`), then sweeps across through where the
//!   chameleon stood and on past it, rests, and comes back the other way. The
//!   line is a fence: slash the locust in your lane before it comes, and the
//!   sweep passes through the gap (lanes are only dealt again next cycle).
//! - **Rubble crickets** (`Moves::WallKick`) chirp when they first spot the
//!   chameleon: wings raised and buzzing for `CHIRP`, then the call wakes every
//!   mob within `CALL_RANGE` (hit it mid-chirp and the call is lost). Then they
//!   crouch and leap, banking off a wall past the chameleon (or behind
//!   themselves) when there is one: they cling there for `CLING` (the second
//!   tell), then kick off at where the chameleon is now, up to `KICKS` times.
//!   Landing leaves them open.
//! - **Mole crickets** (`Moves::Tunnel`) live under the loose soil: only a
//!   mound of dirt shows, and blows pass over it. Once aware they tunnel along
//!   the ground until they are right under the chameleon, heave the soil up
//!   (the tell), then burst out snapping their shovel claws. Out, they are
//!   stranded for `recover` (the only time they can be hurt), then dig back in.
//! - **Wreck katydids** (`Moves::LeafGlide`) lie still on the wrecks, a dead
//!   leaf among the rubble, until the chameleon passes close. Then the leaf
//!   unfolds (the tell), they leap straight up, open their wings at the top of
//!   the leap and glide down along the line to where the chameleon is then
//!   (committed: moving dodges it; they can't climb). Landed, they are open;
//!   later they walk back and play dead leaf again.

use bevy::prelude::*;

use super::{fly, walk, Mob, MobKind, MobSpec, Moves, Notice, State, GRAVITY};
use crate::{
    gore::Fluid,
    player::{self, Player},
    terrain::{Terrain, PX, STEP_UP},
};

/// Locusts per band, and how far apart they rest (stacked up from the spot).
const BAND: usize = 4;
const BAND_GAP: f32 = 24.0;
/// The line: the lowest locust flies this far above the chameleon's feet, the next ones `LANE` higher each.
const LANE_BASE: f32 = 14.0;
const LANE: f32 = 34.0;
/// The line forms this far to the side of the chameleon, and a sweep runs on this far past its spot.
const SWEEP_START: f32 = 300.0;
const OVERSHOOT: f32 = 240.0;
/// Gathering into line takes this much of a band's cycle (then the hold = `windup`,
/// the sweep = `attack_time`, the rest = `recover`).
const GATHER: f32 = 1.5;
/// The chameleon this much further than the leash from home: the band gives up.
const GIVE_UP: f32 = 250.0;

/// Crickets: the call (wings raised, buzzing), and how far it carries.
const CHIRP: f32 = 0.8;
const CALL_RANGE: f32 = 480.0;
/// Chirps per second when it has nothing better to do (just song, no call).
const IDLE_CHIRP: f32 = 0.12;
/// A wall this far past the chameleon is worth banking off, if the cricket can reach it.
const BANK_REACH: f32 = 240.0;
const BANK_RANGE: f32 = 480.0;
/// A wall this close behind the cricket may be kicked off too (at `BEHIND_CHANCE`).
const BEHIND_REACH: f32 = 170.0;
const BEHIND_CHANCE: f32 = 0.6;
/// Clinging to a wall before kicking off, and kicks per attack.
const CLING: f32 = 0.25;
const KICKS: u8 = 2;

/// Mole crickets: within this much (x) of the chameleon it stops and heaves up
/// the soil; it only hunts a chameleon about as high as its ground.
const UNDER: f32 = 20.0;
const REACH_Y: f32 = 110.0;
/// Seconds to come all the way out, and to dig back in.
const EMERGE: f32 = 0.12;
const DIG: f32 = 0.6;
/// Less out of the soil than this, blows pass over it.
const HIDDEN: f32 = 0.35;

/// Katydids: the leap (up, and a little toward the chameleon).
const LAUNCH: f32 = 560.0;
const LAUNCH_SIDE: f32 = 140.0;
/// Gliding can't climb: the line goes down at least this steeply (sine).
const GLIDE_SINK: f32 = 0.22;
/// Back this close to home, it lies down as a leaf again (once it forgot the chameleon).
const SETTLE: f32 = 8.0;

/// Ground movers step at least this much at a time (see `rat::Mind::stride`).
const MIN_STEP: f32 = 4.0;

pub(super) static LOCUST: MobSpec = MobSpec {
    moves: Moves::BandSweep,
    hp: 1,
    scale: 1.3,
    radius: 12.0,
    foot: 4.0,
    blood: Fluid::Brown,
    speed: 90.0,
    sense: 420.0,
    // Holding the line, buzzing.
    windup: 0.7,
    attack_speed: 520.0,
    attack_time: 1.1,
    recover: 0.9,
    leash: 650.0,
    damage: 1,
    stubborn: false,
    initiative: 1.2,
    veteran: false,
    // 1 hp, a band of 4 makes a tick.
    food: 0.25,
};
pub(super) static RUBBLE_CRICKET: MobSpec = MobSpec {
    moves: Moves::WallKick,
    hp: 2,
    scale: 1.5,
    radius: 14.0,
    foot: 6.5 * 1.5,
    blood: Fluid::Brown,
    speed: 100.0,
    sense: 400.0,
    windup: 0.35,
    // Max leap / kick speed.
    attack_speed: 520.0,
    attack_time: 2.0,
    recover: 0.9,
    leash: 360.0,
    damage: 1,
    stubborn: false,
    initiative: 0.6,
    veteran: false,
    // 2 hp, banked leaps, and the call.
    food: 0.75,
};
pub(super) static MOLE_CRICKET: MobSpec = MobSpec {
    moves: Moves::Tunnel,
    hp: 3,
    scale: 1.7,
    radius: 18.0,
    foot: 5.0 * 1.7,
    blood: Fluid::Brown,
    // Tunnelling.
    speed: 150.0,
    sense: 380.0,
    // The soil heaving.
    windup: 0.55,
    // Bursting up.
    attack_speed: 360.0,
    attack_time: 1.2,
    // Stranded out of the soil.
    recover: 1.4,
    leash: 340.0,
    damage: 1,
    stubborn: true,
    initiative: 0.8,
    veteran: false,
    // 3 hp, only hittable when it comes out.
    food: 1.0,
};
pub(super) static WRECK_KATYDID: MobSpec = MobSpec {
    moves: Moves::LeafGlide,
    hp: 3,
    scale: 1.6,
    radius: 16.0,
    foot: 7.0 * 1.6,
    // A leaf that bleeds green.
    blood: Fluid::Green,
    // Walking back home.
    speed: 40.0,
    // It waits for the chameleon to come close.
    sense: 300.0,
    // The leaf unfolding.
    windup: 0.45,
    // Gliding.
    attack_speed: 360.0,
    attack_time: 2.5,
    recover: 1.1,
    leash: 420.0,
    damage: 1,
    stubborn: false,
    initiative: 0.3,
    veteran: true,
    // 3 hp, an ambush from above.
    food: 1.0,
};

/// The sunken street, whatever boss owns it: the grasshopper's kin, and rats
/// (they overrun the street's buried shops and eat what the hoppers leave).
pub(super) fn menu() -> Vec<(MobKind, f32)> {
    vec![
        (MobKind::Locust, 0.3),
        (MobKind::RubbleCricket, 0.25),
        (MobKind::MoleCricket, 0.15),
        (MobKind::WreckKatydid, 0.15),
        (MobKind::Rat, 0.15),
    ]
}

/// The locusts of a band placed at `at` (an open spot): stacked up from it,
/// where there's room. They all share `at.x` as their home's x: that's how
/// they know each other (`Bands`).
pub(super) fn band(terrain: &Terrain, at: Vec2) -> Vec<Vec2> {
    (0..BAND)
        .map(|i| at + Vec2::Y * i as f32 * BAND_GAP)
        .filter(|&q| q == at || (!terrain.solid_at(q) && terrain.clearance(q) >= 2.0 * PX))
        .collect()
}

/// What a sinkhole mob has in mind (kept on its `Mob`).
#[derive(Default)]
pub struct Mind {
    /// Set up on its first frame (mole crickets start buried, katydids as leaves).
    ready: bool,
    /// Mole crickets: 0 = under the soil .. 1 = all out.
    emerge: f32,
    /// Katydids: lying still as a dead leaf; gliding on open wings.
    disguised: bool,
    gliding: bool,
    /// Crickets: chirping for this long; the chirp is a call (wakes the
    /// neighbors when it ends); it has called (or heard a call) already.
    chirp: f32,
    calling: bool,
    called: bool,
    /// Crickets: clinging to a wall on this side (0 = not) for `hold` more
    /// seconds; kicks left this attack.
    wall: f32,
    hold: f32,
    kicks: u8,
    /// Locusts: its lane in the line, the side the sweep starts from, and the
    /// cycles its plan and its bite belong to.
    lane: usize,
    side: f32,
    planned: Option<i64>,
    bit: Option<i64>,
    /// Ground movement saved up until it makes a whole step.
    carry: f32,
}

impl Mind {
    /// 0 = under the soil .. 1 = all out (mole crickets).
    pub(crate) fn emerge(&self) -> f32 {
        self.emerge
    }

    pub(crate) fn disguised(&self) -> bool {
        self.disguised
    }

    pub(crate) fn gliding(&self) -> bool {
        self.gliding
    }

    pub(crate) fn chirping(&self) -> bool {
        self.chirp > 0.0
    }

    /// The side of the wall it clings to (0 = none).
    pub(crate) fn clinging(&self) -> f32 {
        self.wall
    }

    fn stride(&mut self, dx: f32) -> f32 {
        self.carry += dx;
        if self.carry.abs() < MIN_STEP {
            return 0.0;
        }
        std::mem::take(&mut self.carry)
    }
}

/// Under the soil: blows and spit pass over it.
pub(crate) fn hidden(m: &Mob) -> bool {
    m.spec.moves == Moves::Tunnel && m.sinkhole.emerge < HIDDEN && !matches!(m.state, State::Dead { .. })
}

/// Every locust band's members this frame: (home x = band id, home y, position).
pub(super) struct Bands(Vec<(f32, f32, Vec2)>);

impl Bands {
    pub(super) fn new<'a>(mobs: impl Iterator<Item = (&'a Mob, Vec2)>) -> Self {
        Bands(
            mobs.filter(|(m, _)| m.spec.moves == Moves::BandSweep && !matches!(m.state, State::Dead { .. }))
                .map(|(m, pos)| (m.home.x, m.home.y, pos))
                .collect(),
        )
    }

    /// For the locust at `pos` from home `home`: its rank from the bottom of
    /// its band (by height), and the band's mean x.
    fn place(&self, home: Vec2, pos: Vec2) -> (usize, f32) {
        let (mut rank, mut sum, mut n) = (0, 0.0, 0);
        for &(_, hy, q) in self.0.iter().filter(|(hx, ..)| *hx == home.x) {
            n += 1;
            sum += q.x;
            if (q.y, hy) < (pos.y, home.y) {
                rank += 1;
            }
        }
        (rank, if n > 0 { sum / n as f32 } else { pos.x })
    }
}

/// What a sinkhole mob sees of the world this frame.
pub(super) struct World<'a> {
    pub terrain: &'a Terrain,
    /// Landing height moving `from` → `to` with the bottom `foot` below (terrain and platforms).
    pub floor: &'a dyn Fn(Vec2, Vec2, f32) -> Option<f32>,
    pub p: &'a Player,
    pub ppos: Vec2,
    pub t: f32,
    pub dt: f32,
    pub bands: &'a Bands,
}

/// What it did this frame.
pub(super) struct Step {
    /// Its new center.
    pub pos: Vec2,
    /// It bit the chameleon.
    pub bite: bool,
    /// It called: everyone around is on guard.
    pub call: bool,
}

/// The movements this module runs.
pub(super) fn runs(moves: Moves) -> bool {
    matches!(moves, Moves::BandSweep | Moves::WallKick | Moves::Tunnel | Moves::LeafGlide)
}

pub(super) fn think(m: &mut Mob, pos: Vec2, sees: bool, touching: bool, w: &World) -> Step {
    if !m.sinkhole.ready {
        m.sinkhole.ready = true;
        m.sinkhole.emerge = if m.spec.moves == Moves::Tunnel { 0.0 } else { 1.0 };
        m.sinkhole.disguised = m.spec.moves == Moves::LeafGlide;
    }
    let mut step = Step { pos, bite: false, call: false };
    match m.spec.moves {
        Moves::BandSweep => locust(m, pos, touching, w, &mut step),
        Moves::WallKick => cricket(m, pos, sees, touching, w, &mut step),
        Moves::Tunnel => mole(m, pos, sees, touching, w, &mut step),
        Moves::LeafGlide => katydid(m, pos, sees, touching, w, &mut step),
        _ => {}
    }
    step
}

/// A cricket's call reached the mob at `pos` (if any did): it is on guard.
pub(super) fn hear(m: &mut Mob, pos: Vec2, calls: &[Vec2]) {
    if matches!(m.state, State::Dead { .. }) || !calls.iter().any(|c| c.distance(pos) < CALL_RANGE) {
        return;
    }
    m.notice = Notice::Aware;
    m.lost = 0.0;
    // Crickets that heard don't call again.
    m.sinkhole.called = true;
}

// --- Locusts ---

fn locust(m: &mut Mob, pos: Vec2, touching: bool, w: &World, step: &mut Step) {
    let spec = m.spec;
    let dt = w.dt;
    let ease = |rate: f32| 1.0 - (-rate * dt).exp();
    // The band clock: every locust of a band reads the same phase.
    let cycle = GATHER + spec.windup + spec.attack_time + spec.recover;
    let clock = w.t + (m.home.x * 0.0137).rem_euclid(1.0) * cycle;
    let n = (clock / cycle).floor() as i64;
    let s = clock - n as f32 * cycle;
    let (rank, mean) = w.bands.place(m.home, pos);
    let hunting = m.notice == Notice::Aware && w.p.dead.is_none() && w.ppos.distance(m.home) < spec.leash + GIVE_UP;
    let lane_y = |lane: usize, at: Vec2| at.y - player::HALF.y + LANE_BASE + lane as f32 * LANE;
    let buzz = Vec2::new((w.t * 11.0 + m.seed).sin(), (w.t * 14.0 + m.seed).cos()) * 6.0;
    let mut goal = None;
    m.state = match m.state {
        State::Hurt(h) => {
            m.vel *= 1.0 - 4.0 * dt;
            if h < 0.3 { State::Hurt(h + dt) } else { State::Recover(0.0) }
        }
        _ if !hunting => {
            mill(m, pos, w.t, dt);
            State::Idle
        }
        _ if s < GATHER => {
            // Into line, on the band's side of the chameleon.
            m.sinkhole.side = if mean >= w.ppos.x { 1.0 } else { -1.0 };
            m.sinkhole.lane = rank;
            goal = Some(Vec2::new(w.ppos.x + m.sinkhole.side * SWEEP_START, lane_y(rank, w.ppos)));
            State::Recover(0.0)
        }
        _ if s < GATHER + spec.windup => {
            // Holding the line, aimed at where the chameleon is as it forms.
            if m.sinkhole.planned != Some(n) {
                m.sinkhole.planned = Some(n);
                m.plan = w.ppos;
                m.sinkhole.lane = rank;
            }
            goal = Some(Vec2::new(m.plan.x + m.sinkhole.side * SWEEP_START, lane_y(m.sinkhole.lane, m.plan)) + buzz * 0.5);
            State::Windup(s - GATHER)
        }
        _ if s < GATHER + spec.windup + spec.attack_time && m.sinkhole.planned == Some(n) => {
            // The sweep: straight across its lane, through and past the spot.
            let side = m.sinkhole.side;
            if (m.plan.x - pos.x) * side < OVERSHOOT {
                m.vel.x += (-side * spec.attack_speed - m.vel.x) * ease(10.0);
                m.vel.y = (lane_y(m.sinkhole.lane, m.plan) - pos.y) * 5.0;
            } else {
                m.vel *= 1.0 - 4.0 * dt;
            }
            if touching && m.sinkhole.bit != Some(n) {
                m.sinkhole.bit = Some(n);
                step.bite = true;
            }
            State::Attack(s - GATHER - spec.windup)
        }
        _ => {
            m.vel *= 1.0 - 2.5 * dt;
            State::Recover(0.0)
        }
    };
    if let Some(goal) = goal {
        let desired = ((goal - pos) * 4.0).clamp_length_max(spec.speed * 3.5);
        m.vel = m.vel.lerp(desired + buzz * 4.0, ease(5.0));
    }
    m.facing = match m.state {
        State::Windup(_) | State::Attack(_) | State::Recover(_) if hunting => -m.sinkhole.side,
        _ if m.vel.x.abs() > 10.0 => m.vel.x.signum(),
        _ => m.facing,
    };
    let (mut next, _) = fly(w.terrain, pos, m.vel * dt, spec.radius * 0.8);
    let off = next - m.home;
    if off.length() > spec.leash {
        next = m.home + off.clamp_length_max(spec.leash);
    }
    step.pos = next;
}

/// Calm locusts: slow wide loops over their spot.
fn mill(m: &mut Mob, pos: Vec2, t: f32, dt: f32) {
    let s = m.seed;
    let drift = Vec2::new(
        (t * 0.8 + s).sin() * 60.0 + (t * 2.1 + s * 1.7).sin() * 18.0,
        (t * 1.2 + s).cos() * 26.0 + (t * 3.3 + s).sin() * 8.0,
    );
    let desired = ((m.home + drift - pos) * 2.5).clamp_length_max(m.spec.speed * 1.5);
    m.vel = m.vel.lerp(desired, 1.0 - (-4.0 * dt).exp());
}

// --- Rubble crickets ---

fn cricket(m: &mut Mob, pos: Vec2, sees: bool, touching: bool, w: &World, step: &mut Step) {
    let spec = m.spec;
    let dt = w.dt;
    let to = w.ppos - pos;
    let side = if to.x >= 0.0 { 1.0 } else { -1.0 };
    if m.sinkhole.wall != 0.0 {
        if !matches!(m.state, State::Attack(_)) {
            // Knocked off the wall.
            m.sinkhole.wall = 0.0;
        } else if m.sinkhole.hold > 0.0 {
            m.sinkhole.hold -= dt;
            m.facing = side;
            return;
        } else {
            // Kick off at where the chameleon is now.
            m.sinkhole.wall = 0.0;
            let time = (to.length() / spec.attack_speed).clamp(0.25, 0.6);
            m.vel = ballistic(to, time, spec.attack_speed);
            m.state = State::Attack(0.0);
        }
    }
    m.state = match m.state {
        State::Hurt(h) => {
            // A blow cuts the call short.
            m.sinkhole.chirp = 0.0;
            m.sinkhole.calling = false;
            if !m.airborne {
                m.vel.x *= 1.0 - 6.0 * dt;
            }
            if h < 0.3 { State::Hurt(h + dt) } else { State::Recover(spec.recover * 0.5) }
        }
        State::Idle if m.sinkhole.chirp > 0.0 => {
            m.sinkhole.chirp -= dt;
            if m.sinkhole.chirp <= 0.0 && m.sinkhole.calling {
                m.sinkhole.calling = false;
                m.sinkhole.called = true;
                step.call = true;
            }
            if sees {
                m.facing = side;
            }
            State::Idle
        }
        State::Idle if sees && !m.airborne => {
            m.facing = side;
            if m.sinkhole.called {
                m.plan = w.ppos;
                State::Windup(0.0)
            } else {
                m.sinkhole.chirp = CHIRP;
                m.sinkhole.calling = true;
                State::Idle
            }
        }
        State::Idle => {
            if !m.airborne && m.notice == Notice::Unaware {
                if m.rng.chance(dt * IDLE_CHIRP) {
                    m.sinkhole.chirp = CHIRP * 0.7;
                } else {
                    hop_about(m, pos, dt);
                }
            }
            State::Idle
        }
        State::Windup(s) if s < spec.windup => {
            m.facing = side;
            State::Windup(s + dt)
        }
        State::Windup(_) => {
            pounce(m, pos, w);
            State::Attack(0.0)
        }
        State::Attack(_) if touching && m.airborne => {
            step.bite = true;
            m.vel = Vec2::new(-m.vel.x * 0.4, 200.0);
            State::Recover(0.0)
        }
        State::Attack(s) if (!m.airborne && s > 0.05) || s > spec.attack_time => State::Recover(0.0),
        State::Attack(s) => State::Attack(s + dt),
        State::Recover(s) => {
            if !m.airborne {
                m.vel = Vec2::ZERO;
            }
            if s < spec.recover { State::Recover(s + dt) } else { State::Idle }
        }
        dead @ State::Dead { .. } => dead,
    };
    let (next, contact) = air(m, pos, GRAVITY, w);
    if let (Contact::Wall(dir), State::Attack(_)) = (contact, m.state)
        && m.sinkhole.kicks > 0
    {
        m.sinkhole.kicks -= 1;
        m.sinkhole.wall = dir;
        m.sinkhole.hold = CLING;
        m.vel = Vec2::ZERO;
    }
    if matches!(m.state, State::Attack(_)) && m.sinkhole.wall == 0.0 && m.vel.x.abs() > 10.0 {
        m.facing = m.vel.x.signum();
    }
    step.pos = next;
}

/// The leap: at a wall past the chameleon if there's one in reach (to bank
/// off it onto its back), else often at a wall right behind (to kick off it),
/// else straight at it.
fn pounce(m: &mut Mob, pos: Vec2, w: &World) {
    let spec = m.spec;
    let target = m.plan.lerp(w.ppos, 0.3);
    let dir = if target.x >= pos.x { 1.0 } else { -1.0 };
    let high = target + Vec2::Y * 40.0;
    let beyond = w.terrain.raycast(high, high + Vec2::X * dir * BANK_REACH);
    let behind = w.terrain.raycast(pos + Vec2::Y * 30.0, pos + Vec2::new(-dir * BEHIND_REACH, 30.0));
    let aim = match (beyond, behind) {
        (Some(wall), _) if wall.distance(pos) < BANK_RANGE => wall,
        (_, Some(wall)) if m.rng.chance(BEHIND_CHANCE) => wall,
        _ => target,
    };
    let to = aim - pos;
    let time = (to.length() / spec.attack_speed * 1.3).clamp(0.35, 0.8);
    m.vel = ballistic(to, time, spec.attack_speed);
    m.sinkhole.kicks = KICKS;
    m.airborne = true;
}

/// The launch velocity that reaches `to` in `time` (speeds capped).
fn ballistic(to: Vec2, time: f32, max: f32) -> Vec2 {
    Vec2::new((to.x / time).clamp(-max, max), ((to.y + 0.5 * GRAVITY * time * time) / time).clamp(-max, 760.0))
}

/// Little hops about its spot, now and then.
fn hop_about(m: &mut Mob, pos: Vec2, dt: f32) {
    m.wander.1 -= dt;
    if m.wander.1 > 0.0 {
        return;
    }
    let away = pos.x - m.home.x;
    let dir = if away.abs() > m.spec.leash * 0.5 { -away.signum() } else if m.rng.chance(0.5) { 1.0 } else { -1.0 };
    m.vel = Vec2::new(dir * m.spec.speed * m.rng.range(0.6, 1.2), m.rng.range(240.0, 340.0));
    m.airborne = true;
    m.facing = dir;
    m.wander.1 = m.rng.range(1.0, 2.6);
}

// --- Mole crickets ---

fn mole(m: &mut Mob, pos: Vec2, sees: bool, touching: bool, w: &World, step: &mut Step) {
    let spec = m.spec;
    let dt = w.dt;
    let to = w.ppos - pos;
    let side = if to.x >= 0.0 { 1.0 } else { -1.0 };
    let mut tunnel = 0.0;
    m.state = match m.state {
        State::Hurt(h) => {
            m.sinkhole.emerge = 1.0;
            if h < 0.3 { State::Hurt(h + dt) } else { State::Recover(spec.recover * 0.5) }
        }
        State::Idle if m.sinkhole.emerge > 0.0 => {
            // Digging back in.
            m.sinkhole.emerge = (m.sinkhole.emerge - dt / DIG).max(0.0);
            State::Idle
        }
        State::Idle if sees && to.y.abs() < REACH_Y => {
            m.facing = side;
            if to.x.abs() < UNDER {
                m.plan = w.ppos;
                State::Windup(0.0)
            } else {
                tunnel = side * spec.speed;
                State::Idle
            }
        }
        State::Idle => State::Idle,
        State::Windup(s) if s < spec.windup => State::Windup(s + dt),
        State::Windup(_) => {
            // Out through the soil, claws first.
            m.vel = Vec2::new(0.0, spec.attack_speed);
            m.airborne = true;
            State::Attack(0.0)
        }
        State::Attack(_) if touching => {
            step.bite = true;
            m.sinkhole.emerge = 1.0;
            State::Recover(0.0)
        }
        State::Attack(s) => {
            m.sinkhole.emerge = (m.sinkhole.emerge + dt / EMERGE).min(1.0);
            if (!m.airborne && s > 0.1) || s > spec.attack_time { State::Recover(0.0) } else { State::Attack(s + dt) }
        }
        State::Recover(s) => {
            // Stranded on the surface: the window.
            m.sinkhole.emerge = 1.0;
            if s < spec.recover { State::Recover(s + dt) } else { State::Idle }
        }
        dead @ State::Dead { .. } => dead,
    };
    if tunnel != 0.0 && !m.airborne {
        let feet = pos - Vec2::Y * spec.foot;
        let dx = m.sinkhole.stride(tunnel * dt);
        let (feet, blocked) = if dx != 0.0 { walk(w.terrain, feet, dx, m.home.x, spec) } else { (feet, false) };
        m.vel.x = if blocked { 0.0 } else { tunnel };
        step.pos = feet + Vec2::Y * spec.foot;
    } else {
        if !m.airborne {
            m.vel.x = 0.0;
        }
        step.pos = air(m, pos, GRAVITY, w).0;
    }
    m.stride += dt * m.vel.x.abs() * 0.12;
}

// --- Wreck katydids ---

fn katydid(m: &mut Mob, pos: Vec2, sees: bool, touching: bool, w: &World, step: &mut Step) {
    let spec = m.spec;
    let dt = w.dt;
    let side = if w.ppos.x >= pos.x { 1.0 } else { -1.0 };
    let mut stroll = 0.0;
    m.state = match m.state {
        State::Hurt(h) => {
            m.sinkhole.gliding = false;
            m.sinkhole.disguised = false;
            if !m.airborne {
                m.vel.x *= 1.0 - 6.0 * dt;
            }
            if h < 0.3 { State::Hurt(h + dt) } else { State::Recover(spec.recover * 0.5) }
        }
        State::Idle if sees && !m.airborne => {
            m.sinkhole.disguised = false;
            m.facing = side;
            m.plan = w.ppos;
            State::Windup(0.0)
        }
        State::Idle => {
            let back = m.home.x - pos.x;
            let shocked = matches!(m.notice, Notice::Shock { .. });
            if !m.airborne && !shocked && back.abs() > SETTLE {
                // Back to its wreck.
                stroll = back.signum() * spec.speed;
                m.facing = back.signum();
            } else if !m.airborne && m.notice == Notice::Unaware {
                m.sinkhole.disguised = true;
            }
            State::Idle
        }
        State::Windup(s) if s < spec.windup => {
            m.facing = side;
            State::Windup(s + dt)
        }
        State::Windup(_) => {
            m.vel = Vec2::new(m.facing * LAUNCH_SIDE, LAUNCH);
            m.airborne = true;
            State::Attack(0.0)
        }
        State::Attack(s) => {
            if !m.sinkhole.gliding && m.airborne && m.vel.y <= 0.0 && s > 0.05 {
                // The top of the leap: wings open, and it picks its line.
                let mut dir = (w.ppos - pos).normalize_or(Vec2::new(m.facing, 0.0));
                if dir.y > -GLIDE_SINK {
                    let sx = if dir.x == 0.0 { m.facing } else { dir.x.signum() };
                    dir = Vec2::new(sx * (1.0 - GLIDE_SINK * GLIDE_SINK).sqrt(), -GLIDE_SINK);
                }
                m.vel = dir * spec.attack_speed;
                m.facing = if dir.x >= 0.0 { 1.0 } else { -1.0 };
                m.sinkhole.gliding = true;
            }
            // The leap up is harmless; the glide down is the attack.
            if touching && m.sinkhole.gliding {
                step.bite = true;
                m.sinkhole.gliding = false;
                m.vel = Vec2::new(-m.vel.x * 0.3, 160.0);
                State::Recover(0.0)
            } else if (!m.airborne && s > 0.05) || s > spec.attack_time {
                m.sinkhole.gliding = false;
                State::Recover(0.0)
            } else {
                State::Attack(s + dt)
            }
        }
        State::Recover(s) => {
            if !m.airborne {
                m.vel = Vec2::ZERO;
            }
            if s < spec.recover { State::Recover(s + dt) } else { State::Idle }
        }
        dead @ State::Dead { .. } => dead,
    };
    if stroll != 0.0 && !m.airborne {
        let feet = pos - Vec2::Y * spec.foot;
        let dx = m.sinkhole.stride(stroll * dt);
        let (feet, blocked) = if dx != 0.0 { walk(w.terrain, feet, dx, m.home.x, spec) } else { (feet, false) };
        if blocked {
            // Can't get back: this spot will do.
            m.home.x = feet.x;
        }
        m.vel.x = stroll;
        step.pos = feet + Vec2::Y * spec.foot;
    } else {
        let mut gravity = GRAVITY;
        if m.sinkhole.gliding {
            // Riding the air on a straight line, swaying a little.
            gravity = 0.0;
            m.vel = m.vel.rotate(Vec2::from_angle((w.t * 5.0 + m.seed).sin() * 0.4 * dt));
        } else if !m.airborne && !matches!(m.state, State::Hurt(_)) {
            m.vel.x = 0.0;
        }
        let (next, contact) = air(m, pos, gravity, w);
        if matches!(contact, Contact::Wall(_)) {
            // Into a wall: wings fold, it drops.
            m.sinkhole.gliding = false;
        }
        step.pos = next;
    }
    m.stride += dt * m.vel.x.abs() * 0.12;
}

// --- Shared ---

#[derive(Clone, Copy)]
enum Contact {
    None,
    Floor,
    /// Hit a wall on this side.
    Wall(f32),
}

/// Jumpers: ballistic in the air (under `gravity`), stuck to the ground
/// otherwise. Returns the new center and what it ran into.
fn air(m: &mut Mob, pos: Vec2, gravity: f32, w: &World) -> (Vec2, Contact) {
    let spec = m.spec;
    if !m.airborne {
        let feet = pos - Vec2::Y * spec.foot;
        match w.terrain.surface(feet + Vec2::Y * STEP_UP, 2.0 * STEP_UP) {
            Some(y) if m.vel.y <= 0.0 => return (Vec2::new(pos.x, y + spec.foot), Contact::Floor),
            _ => m.airborne = true,
        }
    }
    m.vel.y -= gravity * w.dt;
    let mut next = pos + m.vel * w.dt;
    // (The body box sits above the stair-stepped ground of slopes.)
    let body = Vec2::new(spec.radius * 0.4, spec.foot * 0.4);
    let mut contact = Contact::None;
    let off = |x: f32| (x - m.home.x).abs();
    if m.vel.x != 0.0 && w.terrain.box_hits(Vec2::new(next.x, pos.y + 4.0), body) {
        contact = Contact::Wall(m.vel.x.signum());
        next.x = pos.x;
        m.vel.x = 0.0;
    } else if off(next.x) > spec.leash && off(next.x) > off(pos.x) {
        next.x = pos.x;
        m.vel.x = 0.0;
    }
    if m.vel.y > 0.0 && w.terrain.box_hits(Vec2::new(next.x, next.y + body.y), Vec2::new(body.x, 2.0)) {
        next.y = pos.y;
        m.vel.y = 0.0;
    }
    if let Some(top) = (w.floor)(pos, next, spec.foot) {
        next.y = top + spec.foot;
        m.vel = Vec2::ZERO;
        m.airborne = false;
        contact = Contact::Floor;
    }
    if next.y < w.terrain.bounds.min.y - 600.0 {
        // Fell out of the world: back to its spot.
        next = m.home;
        m.vel = Vec2::ZERO;
        m.airborne = false;
    }
    (next, contact)
}
