//! The looted mall (`Style::Mall`): the giant praying mantis's ecosystem.
//! Its kin hunt among the mannequins and dead palms; what they eat flutters
//! around the dead neon.
//! - **Mantis nymphs** (`Moves::Stalk`): still-hunters. They sway like a leaf
//!   and creep in, forelegs folded; a moving chameleon in reach gets the
//!   strike: forelegs raised and opened (the tell), then a fast, long-reach
//!   snap with a little lunge, then the forelegs hang out (the opening).
//!   Facing the chameleon they hold a **guard**: slashes from the front are
//!   parried (no harm, a riposte). Bait the strike, or come from behind or
//!   above (turning around takes them a moment); spit gets through.
//! - **Ghost mantises** (`Moves::Stalk`, veterans): disguised as a heap of
//!   dead leaves until something moves close by. Then they rise and flare
//!   their leaf wings (the tell) and pounce, a long leap ending in a snap;
//!   a long recovery after it. Calm again, they walk back and fold down.
//! - **Neon moths** (`Moves::Flutter`): circle their dead light; drawn to the
//!   chameleon's pale head, they spiral in around it, hang quivering (the
//!   tell), dart through, and flutter off to spiral in again.
//! - **Oothecae** (`Moves::Hang`, run by `mobs::nests`): egg cases glued to
//!   cables dangling from the ceiling; they hatch **mantis hatchlings**
//!   (`Moves::Hop`: drop, crouch, leap) while the chameleon is near.
//!
//! Mantises and moths **hunt by motion**: a chameleon holding still for a
//! moment (`STILL`) is lost to them (camouflage); a slash counts as moving.

use bevy::prelude::*;

use super::{fly, walk, Mob, MobKind, MobSpec, Moves, Notice, Shock, State};
use crate::{chameleon::ChameleonAnim, gore::Fluid, player::Player, terrain::Terrain};

/// The chameleon counts as moving above this speed (or while slashing).
const MOTION: f32 = 30.0;
/// Held still this long, the chameleon is lost to motion hunters; they only
/// strike something that moved within the last `FRESH` seconds.
const STILL: f32 = 1.0;
const FRESH: f32 = 0.25;
/// Nymphs: the strike reaches this far ahead of the center (and this far up or down).
const REACH: f32 = 82.0;
const REACH_UP: f32 = 50.0;
/// Nymphs: creeping in, they stop this far from the chameleon (a share of `REACH`).
const HOLD: f32 = 0.75;
/// Nymphs guard against blows from the front with the chameleon this close.
const GUARD: f32 = 140.0;
/// A parried slash: the riposte starts this far into the windup (share).
const RIPOSTE: f32 = 0.35;
/// The chameleon behind it this long before it turns around.
const TURN: f32 = 0.35;
/// Ghost mantises: reveal when something moves this close, pounce from this far.
const REVEAL: f32 = 200.0;
const POUNCE: f32 = 170.0;
/// Ghost mantises: the snap at the end of the pounce reaches this far ahead.
const SNAP: f32 = 58.0;
/// Ghost mantises: seconds without prey before they go home and fold down again.
const CALM: f32 = 3.0;
/// Rising / folding speed (per second, 0..1).
const RISE: f32 = 3.0;
/// Moths: the spiral around the chameleon's head (start and end radii,
/// shrink speed, turn speed), and the circle around their own light.
const RING_OUT: f32 = 130.0;
const RING_IN: f32 = 55.0;
const SPIRAL_IN: f32 = 40.0;
const ORBIT: f32 = 3.2;
const LIGHT_RING: f32 = 36.0;

/// What a mantis or a moth has in mind (kept on its `Mob`; the art reads it).
#[derive(Default)]
pub struct Mind {
    /// Seconds since the chameleon last moved.
    pub(crate) still: f32,
    /// Ghost mantises: folded down as dead leaves.
    pub(crate) disguised: bool,
    /// Ghost mantises: 0 = folded down, 1 = up on its legs.
    pub(crate) rise: f32,
    /// Nymphs: holding the guard (forelegs up in front of the face).
    pub(crate) guard: bool,
    /// Just parried a blow (seconds left of the clash pose).
    pub(crate) parried: f32,
    /// Where the head looks (angle above the horizontal, toward the facing).
    pub(crate) look: f32,
    /// Moths: the spiral's radius now.
    ring: f32,
    /// The chameleon has been behind it this long.
    turn: f32,
    /// Ghost mantises: seconds without prey.
    calm: f32,
    /// This strike already landed.
    struck: bool,
}

impl Mind {
    pub fn new(kind: MobKind) -> Self {
        Self { disguised: kind == MobKind::GhostMantis, ring: RING_OUT, ..default() }
    }
}

pub(super) static MANTIS_NYMPH: MobSpec = MobSpec {
    moves: Moves::Stalk,
    hp: 2,
    scale: 1.7,
    radius: 15.0,
    foot: 7.0 * 1.7,
    blood: Fluid::Green,
    // Creeping.
    speed: 34.0,
    sense: 380.0,
    windup: 0.4,
    // The lunge with the strike.
    attack_speed: 260.0,
    attack_time: 0.14,
    recover: 0.9,
    leash: 220.0,
    damage: 1,
    stubborn: false,
    initiative: 0.5,
    veteran: false,
    // 2 hp, out-reaches the claw and parries it from the front.
    food: 0.75,
};
pub(super) static GHOST_MANTIS: MobSpec = MobSpec {
    hp: 4,
    scale: 2.1,
    radius: 20.0,
    foot: 7.0 * 2.1,
    speed: 48.0,
    // Disguised: only something moving close by gives it away.
    sense: REVEAL,
    windup: 0.6,
    attack_speed: 560.0,
    attack_time: 0.28,
    recover: 1.2,
    leash: 260.0,
    stubborn: true,
    initiative: 0.3,
    veteran: true,
    // 4 hp, hidden, pounces from afar, stubborn.
    food: 1.5,
    ..MANTIS_NYMPH
};
pub(super) static NEON_MOTH: MobSpec = MobSpec {
    moves: Moves::Flutter,
    hp: 1,
    scale: 1.5,
    radius: 12.0,
    foot: 4.0,
    // Dusty pale ichor.
    blood: Fluid::Yellow,
    speed: 110.0,
    sense: 360.0,
    windup: 0.45,
    attack_speed: 400.0,
    attack_time: 0.5,
    recover: 1.1,
    leash: 400.0,
    damage: 1,
    stubborn: false,
    initiative: 1.2,
    veteran: false,
    // 1 hp, telegraphed darts.
    food: 0.25,
};
pub(super) static OOTHECA: MobSpec = MobSpec {
    moves: Moves::Hang,
    hp: 3,
    scale: 1.6,
    radius: 16.0,
    foot: 14.0,
    blood: Fluid::Green,
    speed: 0.0,
    sense: 520.0,
    windup: 0.0,
    attack_speed: 0.0,
    // Shudders this long when a hatchling crawls out.
    attack_time: 0.6,
    recover: 0.0,
    leash: 0.0,
    damage: 0,
    stubborn: true,
    initiative: 0.0,
    veteran: false,
    // 3 hp, harmless itself, a meal of eggs.
    food: 0.75,
};
pub(super) static MANTIS_HATCHLING: MobSpec = MobSpec {
    moves: Moves::Hop,
    hp: 1,
    scale: 0.95,
    radius: 10.0,
    foot: 7.0 * 0.95,
    blood: Fluid::Green,
    speed: 90.0,
    sense: 480.0,
    windup: 0.35,
    attack_speed: 380.0,
    attack_time: 2.0,
    recover: 0.7,
    leash: 260.0,
    damage: 1,
    stubborn: false,
    initiative: 0.2,
    veteran: true,
    // Hatched, not found: nothing to farm.
    food: 0.0,
};

/// The mall's ecosystem: kinds and their share of the mobs.
pub(super) fn menu() -> Vec<(MobKind, f32)> {
    vec![
        (MobKind::MantisNymph, 0.25),
        (MobKind::NeonMoth, 0.25),
        (MobKind::GhostMantis, 0.13),
        (MobKind::Ootheca, 0.12),
        // Food-court cockroaches and mall rats: what the mantises live on.
        (MobKind::Roachling, 0.15),
        (MobKind::Rat, 0.1),
    ]
}

/// What a nest-like spawner releases (None: not a spawner of this region).
pub(super) fn brood(kind: MobKind) -> Option<MobKind> {
    (kind == MobKind::Ootheca).then_some(MobKind::MantisHatchling)
}

/// What a mantis or a moth sees of the world this frame.
pub(super) struct World<'a> {
    pub terrain: &'a Terrain,
    pub p: &'a Player,
    pub anim: &'a ChameleonAnim,
    pub ppos: Vec2,
    pub t: f32,
    pub dt: f32,
}

/// What it did this frame: where it is now, and whether it hit the chameleon.
pub(super) struct Step {
    pub next: Vec2,
    pub bite: bool,
}

pub(super) fn think(m: &mut Mob, pos: Vec2, sees: bool, touching: bool, w: &World) -> Step {
    let moving = w.p.vel.length() > MOTION || w.anim.slashing();
    m.mall.still = if moving { 0.0 } else { m.mall.still + w.dt };
    m.mall.parried = (m.mall.parried - w.dt).max(0.0);
    match m.spec.moves {
        Moves::Flutter => moth(m, pos, sees, touching, w),
        _ => mantis(m, pos, sees, w),
    }
}

/// A slash on a nymph holding its guard: parried. It strikes back at once
/// (part of its windup already done).
pub(super) fn parries(m: &mut Mob, pos: Vec2, ppos: Vec2) -> bool {
    let ahead = m.facing * (ppos.x - pos.x) > 0.0;
    if !(m.mall.guard && ahead && m.notice == Notice::Aware && matches!(m.state, State::Idle | State::Windup(_))) {
        return false;
    }
    m.flash = 0.6;
    m.mall.parried = 0.2;
    m.plan = ppos;
    m.state = State::Windup(m.spec.windup * RIPOSTE);
    true
}

fn sign(x: f32) -> f32 {
    if x >= 0.0 { 1.0 } else { -1.0 }
}

/// Nymphs and ghost mantises.
fn mantis(m: &mut Mob, pos: Vec2, sees: bool, w: &World) -> Step {
    let spec = m.spec;
    let dt = w.dt;
    let ease = |rate: f32| 1.0 - (-rate * dt).exp();
    let ghost = m.kind == MobKind::GhostMantis;
    let to = w.ppos - pos;
    let dist = to.length();
    let side = sign(to.x);
    let ahead = m.facing * to.x > 0.0;
    // Something moving: the only thing a mantis hunts.
    let prey = sees && m.mall.still < STILL;
    let fresh = sees && m.mall.still < FRESH;
    let mut bite = false;
    let mut want = 0.0;

    // Behind it: it takes a moment to turn around.
    if prey && !ahead && m.state == State::Idle && !m.mall.disguised {
        m.mall.turn += dt;
        if m.mall.turn > TURN {
            m.facing = side;
            m.mall.turn = 0.0;
        }
    } else {
        m.mall.turn = 0.0;
    }
    m.mall.calm = if prey { 0.0 } else { m.mall.calm + dt };

    m.state = match m.state {
        State::Hurt(s) if s < 0.3 => {
            m.mall.disguised = false;
            State::Hurt(s + dt)
        }
        // A ghost hit answers with a display and a pounce; a nymph backs off a little.
        State::Hurt(_) if ghost && sees => {
            m.facing = side;
            m.plan = w.ppos;
            State::Windup(0.0)
        }
        State::Hurt(_) => State::Recover(spec.recover * 0.5),
        State::Windup(s) if s < spec.windup => {
            // Ghosts track the chameleon while they flare; nymphs are committed.
            if ghost {
                m.facing = side;
                m.plan = w.ppos;
            }
            State::Windup(s + dt)
        }
        State::Windup(_) => {
            m.mall.struck = false;
            State::Attack(0.0)
        }
        State::Attack(s) if s < spec.attack_time => {
            want = m.facing;
            // The snap: a box ahead of it (late in the pounce for ghosts).
            let (reach, live) = if ghost { (SNAP, s > spec.attack_time * 0.4) } else { (REACH, true) };
            let fx = to.x * m.facing;
            if live && !m.mall.struck && w.p.dead.is_none() && fx > -spec.radius && fx < reach && to.y.abs() < REACH_UP {
                m.mall.struck = true;
                bite = true;
            }
            State::Attack(s + dt)
        }
        State::Attack(_) => State::Recover(0.0),
        State::Recover(s) if s < spec.recover => State::Recover(s + dt),
        State::Recover(_) => State::Idle,
        State::Idle => {
            let level = to.y.abs() < REACH_UP;
            if ghost && m.mall.disguised {
                // Leaves, until something moves close by.
                if fresh && dist < REVEAL {
                    m.mall.disguised = false;
                    m.facing = side;
                    m.plan = w.ppos;
                    State::Windup(0.0)
                } else {
                    State::Idle
                }
            } else if ghost && fresh && ahead && level && dist < POUNCE && m.mall.rise > 0.9 {
                m.plan = w.ppos;
                State::Windup(0.0)
            } else if !ghost && fresh && ahead && level && to.x.abs() < REACH {
                m.plan = w.ppos;
                State::Windup(0.0)
            } else if prey && ahead {
                // Stalking: creeps in, swaying like a leaf, stepping on the sway.
                let hold = if ghost { POUNCE * 0.7 } else { REACH * HOLD };
                let sway = ((w.t * 1.7 + m.seed).sin() > 0.2) as i32 as f32;
                if to.x.abs() > hold {
                    want = side * spec.speed / spec.attack_speed * sway;
                }
                State::Idle
            } else {
                // Lost it: back to its spot (ghosts after a while), where a ghost folds down again.
                let home = (pos.x - m.home.x).abs() < 8.0;
                if !prey && !home && (!ghost || m.mall.calm > CALM) {
                    want = sign(m.home.x - pos.x) * spec.speed / spec.attack_speed * 0.6;
                }
                if ghost && m.mall.calm > if home { CALM } else { CALM * 2.5 } {
                    m.mall.disguised = true;
                }
                State::Idle
            }
        }
        dead @ State::Dead { .. } => dead,
    };

    m.mall.guard = !ghost && sees && ahead && dist < GUARD && matches!(m.state, State::Idle | State::Windup(_));
    let up = if m.mall.disguised { 0.0 } else { 1.0 };
    m.mall.rise += (up - m.mall.rise).clamp(-RISE * dt, RISE * dt);
    let look = if sees { (to.y / to.x.abs().max(20.0)).atan().clamp(-0.6, 0.6) } else { 0.0 };
    m.mall.look += (look - m.mall.look) * ease(8.0);

    // Walking: knocked back, creeping, lunging; the pounce is a long leap along the ground.
    match m.state {
        State::Hurt(_) | State::Recover(_) => m.vel.x *= 1.0 - 6.0 * dt,
        State::Attack(_) => m.vel.x = want * spec.attack_speed,
        _ => m.vel.x += (want * spec.attack_speed - m.vel.x) * ease(8.0),
    }
    if want != 0.0 && m.state == State::Idle {
        m.facing = sign(want);
    }
    let feet = pos - Vec2::Y * spec.foot;
    let (feet, blocked) = walk(w.terrain, feet, m.vel.x * dt, m.home.x, spec);
    if blocked {
        m.vel.x = 0.0;
        if matches!(m.state, State::Attack(_)) && ghost {
            m.state = State::Recover(0.0);
        }
    }
    m.stride += dt * m.vel.x.abs() * 0.12;
    Step { next: feet + Vec2::Y * spec.foot, bite }
}

/// Neon moths.
fn moth(m: &mut Mob, pos: Vec2, sees: bool, touching: bool, w: &World) -> Step {
    let spec = m.spec;
    let dt = w.dt;
    let ease = |rate: f32| 1.0 - (-rate * dt).exp();
    // The light it's drawn to: the chameleon's pale head.
    let head = crate::chameleon::neck(w.ppos, w.p.facing) + Vec2::Y * 6.0;
    let lured = sees && m.mall.still < STILL;
    let flutter = Vec2::new((w.t * 5.3 + m.seed).sin() * 12.0, (w.t * 7.9 + m.seed * 1.7).sin() * 16.0);
    let mut bite = false;
    let steer = |m: &mut Mob, target: Vec2, speed: f32, rate: f32| {
        let desired = ((target - pos) * 4.0).clamp_length_max(speed);
        m.vel = m.vel.lerp(desired, ease(rate));
    };

    m.state = match m.state {
        State::Hurt(s) if s < 0.3 => {
            m.vel *= 1.0 - 4.0 * dt;
            State::Hurt(s + dt)
        }
        State::Hurt(_) => State::Recover(spec.recover * 0.5),
        State::Idle => {
            let shock = match m.notice {
                Notice::Shock { how, .. } => Some(how),
                _ => None,
            };
            match shock {
                // Startled: flutters off.
                Some(Shock::Flee | Shock::Panic) => steer(m, pos + (pos - w.ppos).normalize_or(Vec2::Y) * 120.0, spec.speed * 1.8, 6.0),
                Some(Shock::Scatter(a)) => steer(m, pos + Vec2::from_angle(a) * 120.0, spec.speed * 1.8, 6.0),
                _ if lured => {
                    // Spiraling in around the head.
                    m.mall.ring = (m.mall.ring - SPIRAL_IN * dt).max(RING_IN);
                    m.orbit += dt * ORBIT * if (m.seed * 3.7).fract() < 0.5 { 1.0 } else { -1.0 };
                    let target = head + Vec2::from_angle(m.orbit) * Vec2::new(m.mall.ring, m.mall.ring * 0.6) + flutter;
                    steer(m, target, spec.speed * 2.2, 5.0);
                }
                _ => {
                    // Circling its dead light.
                    m.mall.ring = (m.mall.ring + SPIRAL_IN * dt).min(RING_OUT);
                    m.orbit += dt * ORBIT * 0.8;
                    let target = m.home + Vec2::from_angle(m.orbit) * LIGHT_RING + flutter;
                    steer(m, target, spec.speed, 4.0);
                }
            }
            if lured && shock.is_none() && m.mall.ring <= RING_IN + 1.0 && m.rng.chance(ease(1.5)) {
                m.plan = head;
                State::Windup(0.0)
            } else {
                State::Idle
            }
        }
        State::Windup(s) if s < spec.windup => {
            // Hangs quivering, eyes on the head.
            m.vel *= 1.0 - 8.0 * dt;
            m.plan = m.plan.lerp(head, ease(2.0));
            State::Windup(s + dt)
        }
        State::Windup(_) => {
            m.vel = (m.plan - pos).normalize_or(Vec2::X * m.facing) * spec.attack_speed;
            State::Attack(0.0)
        }
        State::Attack(_) if touching => {
            bite = true;
            m.vel = (pos - w.ppos).normalize_or(Vec2::Y) * 200.0 + Vec2::Y * 120.0;
            m.mall.ring = RING_OUT;
            State::Recover(0.0)
        }
        State::Attack(s) if s > spec.attack_time => {
            m.mall.ring = RING_OUT;
            State::Recover(0.0)
        }
        State::Attack(s) => State::Attack(s + dt),
        State::Recover(s) if s < spec.recover => {
            // Flutters up and away.
            steer(m, pos + Vec2::new(sign(pos.x - w.ppos.x) * 60.0, 50.0) + flutter, spec.speed, 3.0);
            State::Recover(s + dt)
        }
        State::Recover(_) => State::Idle,
        dead @ State::Dead { .. } => dead,
    };

    let (mut next, blocked) = fly(w.terrain, pos, m.vel * dt, spec.radius * 0.8);
    let off = next - m.home;
    let leashed = off.length() > spec.leash;
    if leashed {
        next = m.home + off.clamp_length_max(spec.leash);
    }
    if (blocked || leashed) && matches!(m.state, State::Attack(_)) {
        m.vel *= -0.2;
        m.mall.ring = RING_OUT;
        m.state = State::Recover(0.0);
    }
    if matches!(m.state, State::Windup(_)) {
        m.facing = sign(m.plan.x - pos.x);
    } else if m.vel.x.abs() > 10.0 {
        m.facing = sign(m.vel.x);
    }
    Step { next, bite }
}
