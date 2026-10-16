//! The old sewer: the Plague Frog's brood and the filth it breeds in.
//!
//! - **Bile leeches** (`Moves::Leech`, fodder): inch along the floor; close to
//!   the chameleon they rear up (the tell) and spring at it. A leech that lands
//!   on it **latches on**: it rides along, slows the walk (`LATCH_DRAG`, via
//!   `Player::webbed`) and drains a health point every `LATCH_DRAIN`. Shake it
//!   off (turn around, jump: `SHAKE_OFF` times; a dash or the water at once) or
//!   slash it off your own back. A missed leap leaves it stranded a moment.
//! - **Gullet toads** (`Moves::TongueLash`): squat and hop about; with a clear
//!   line they gape (throat pumping: the tell) and shoot their tongue at where
//!   the chameleon stood. Caught, it is hurt and **yanked in** next to the toad,
//!   which then gulps (open). The tongue stops at walls; moving off the line
//!   dodges it.
//! - **Plague bloats** (`Moves::Belch`): poison-swollen toads waddling slowly at
//!   the chameleon. Close up they swell (stubborn: hits don't stop it), then
//!   belch a small violet cloud (`Miasma`) at where it stood, and sag empty
//!   for a while (the opening). Killed swollen, they burst into one last
//!   poisonous cloud; otherwise into a harmless puff.
//! - **Drain newts** (`Moves::CeilingCreep`, veterans): creep upside down along
//!   ceilings and stalk the chameleon from above; right over it they peel off
//!   (head dips: the tell) and drop on it, sprawl a moment where they land,
//!   then hunt on the ground with quick lunges. Slashed on the ceiling, they
//!   fall. They shed their tail when they die.
//!
//! Frog kin bleed violet; the leeches bleed what they sucked (red).

use std::f32::consts::PI;

use bevy::{prelude::*, sprite_render::AlphaMode2d};

use super::{flinch, hop, idle, walk, Mob, MobKind, MobSpec, MobStuff, Moves, Notice, State, GRAVITY};
use crate::{
    camera::{self, HitStop, Shake},
    fumes::Cloud,
    gore::{Bleed, Fluid},
    level::Platform,
    mob_art,
    player::{self, Hurt, Player},
    shapes::Rng,
    terrain::{Terrain, PX, STEP_UP},
};

// --- Bile leech ---
/// Springs at the chameleon from this close.
const LEECH_LEAP: f32 = 190.0;
/// The leap takes about this long.
const LEECH_LEAP_TIME: f32 = 0.5;
/// Latched on: a health point drained every this long (the first one too).
const LATCH_DRAIN: f32 = 2.4;
/// Share of the walk speed taken per leech latched on (up to `LATCH_DRAG_MAX`), as cobwebs do.
const LATCH_DRAG: f32 = 0.3;
const LATCH_DRAG_MAX: f32 = 0.8;
/// Turning around and jumps that shake a leech off.
const SHAKE_OFF: u32 = 3;
/// Rests this long after a leap before it may leap again.
const LEECH_COOLDOWN: (f32, f32) = (0.6, 1.4);

// --- Gullet toad ---
/// The tongue's reach, how fast it shoots out and snaps back.
const TONGUE_REACH: f32 = 280.0;
const TONGUE_BACK: f32 = 900.0;
/// It shoots a little past where it aims.
const TONGUE_OVERSHOOT: f32 = 40.0;
/// Caught when the tongue passes this close to the chameleon's center.
const TONGUE_GRAB: f32 = 20.0;
/// The yank toward the toad.
const TONGUE_PULL: f32 = 560.0;
/// The mouth, in its drawing (where the tongue comes out).
pub(crate) const TOAD_MOUTH: Vec2 = Vec2::new(8.0, -0.5);
/// Between tongue lashes.
const TOAD_COOLDOWN: (f32, f32) = (1.2, 2.4);

// --- Plague bloat ---
/// Belches when the chameleon is this close, the cloud at most this far from the mouth.
const BELCH_RANGE: f32 = 210.0;
const BELCH_REACH: f32 = 170.0;
/// Its mouth, in its drawing.
const BLOAT_MOUTH: Vec2 = Vec2::new(9.0, -3.0);
const BLOAT_COOLDOWN: (f32, f32) = (2.0, 3.5);
/// Its clouds: how fast they slow down, how big they get, how long they linger.
const MIASMA_DRAG: f32 = 3.0;
const MIASMA_RADIUS: f32 = 72.0;
const MIASMA_LIFE: f32 = 3.6;
/// Killed swollen: the burst cloud lingers this long (otherwise a harmless puff).
const BURST_LIFE: f32 = 3.0;
const PUFF_LIFE: f32 = 1.2;

// --- Drain newt ---
/// From the ceiling it spots the chameleon this far to either side, below it.
const NEWT_SPOT: f32 = 320.0;
/// Leads the chameleon by its velocity × this while stalking from above.
const NEWT_LEAD: f32 = 0.25;
/// Drops when the chameleon is this close to right below it.
const NEWT_DROP: f32 = 26.0;
/// Needs at least this much fall under it (else no point dropping).
const NEWT_MIN_DROP: f32 = 110.0;
/// Lunges at the chameleon from this close on the ground.
const NEWT_LUNGE: f32 = 150.0;

/// The smallest step a crawler takes (smaller ones would catch on pixel steps).
const MIN_STEP: f32 = 4.0;

/// Its spec, for `MobKind::spec`.
pub(super) static BILE_LEECH: MobSpec = MobSpec {
    moves: Moves::Leech,
    hp: 1,
    scale: 1.5,
    radius: 12.0,
    foot: 2.6 * 1.5,
    // Full of someone else's blood.
    blood: Fluid::Red,
    speed: 26.0,
    sense: 320.0,
    windup: 0.45,
    attack_speed: 420.0,
    attack_time: 2.0,
    recover: 0.9,
    leash: 260.0,
    damage: 1,
    stubborn: false,
    initiative: 1.2,
    veteran: false,
    // 1 hp; a drain if it latches on.
    food: 0.25,
};
pub(super) static GULLET_TOAD: MobSpec = MobSpec {
    moves: Moves::TongueLash,
    hp: 2,
    scale: 1.6,
    radius: 15.0,
    foot: 5.0 * 1.6,
    blood: Fluid::Violet,
    // Idle hops.
    speed: 90.0,
    sense: 380.0,
    windup: 0.55,
    // The tongue.
    attack_speed: 1300.0,
    attack_time: 0.0,
    recover: 1.1,
    leash: 200.0,
    damage: 1,
    stubborn: false,
    initiative: 0.9,
    veteran: false,
    // 2 hp, a ranged grab.
    food: 0.75,
};
pub(super) static PLAGUE_BLOAT: MobSpec = MobSpec {
    moves: Moves::Belch,
    hp: 3,
    scale: 1.7,
    radius: 20.0,
    foot: 9.0 * 1.7,
    blood: Fluid::Violet,
    speed: 18.0,
    sense: 380.0,
    windup: 1.0,
    attack_speed: 0.0,
    attack_time: 0.0,
    recover: 1.6,
    leash: 220.0,
    damage: 1,
    // Keeps swelling through hits.
    stubborn: true,
    initiative: 1.6,
    veteran: false,
    // 3 hp, poison clouds, stubborn.
    food: 1.0,
};
pub(super) static DRAIN_NEWT: MobSpec = MobSpec {
    moves: Moves::CeilingCreep,
    hp: 3,
    scale: 1.6,
    radius: 16.0,
    foot: 4.5 * 1.6,
    blood: Fluid::Violet,
    speed: 40.0,
    sense: 420.0,
    windup: 0.45,
    attack_speed: 380.0,
    attack_time: 0.35,
    recover: 1.1,
    leash: 320.0,
    damage: 1,
    stubborn: false,
    initiative: 0.5,
    veteran: true,
    // 3 hp, an ambush from above, then lunges.
    food: 1.0,
};

/// What a sewer mob has in mind (kept on its `Mob`).
#[derive(Default)]
pub(crate) struct Mind {
    /// Movement saved up until it makes a whole step (`MIN_STEP`).
    carry: f32,
    /// Seconds before it may attack again.
    cooldown: f32,
    /// Leeches: the inchworm phase.
    pub(crate) inch: f32,
    /// Leeches: where it holds on to the chameleon (x along its facing, from its center).
    pub(crate) latched: Option<Vec2>,
    /// Leeches: time to the next sip, how many times it was shaken, what it saw last frame.
    drain: f32,
    shakes: u32,
    last_facing: f32,
    was_grounded: bool,
    /// Leeches: health points drunk (it swells).
    pub(crate) fed: f32,
    /// Toads: the tongue's length out and its direction (world), how far it goes.
    pub(crate) tongue: f32,
    pub(crate) aim: Vec2,
    reach: f32,
    /// Bloats: 0..1 swollen (eased; < 0 = sagging empty after a belch).
    pub(crate) swell: f32,
    /// Bloats: a cloud to belch this frame: (from, velocity).
    belch: Option<(Vec2, Vec2)>,
    /// Newts: off the ceiling for good, falling now, already bit during this drop.
    pub(crate) dropped: bool,
    falling: bool,
    bit: bool,
    /// Newts: 0 = upside down on the ceiling, 1 = right way up (eased: it turns over as it falls).
    pub(crate) turned: f32,
}

pub(super) fn plugin(app: &mut App) {
    app.add_systems(Update, leech_drag.after(crate::webs::stick).before(player::move_player)).add_systems(
        Update,
        (belch, miasma).chain().after(super::think).before(mob_art::dismember),
    );
}

/// The kinds of movement this region adds (run by `think` here).
pub(crate) fn owns(moves: Moves) -> bool {
    matches!(moves, Moves::Leech | Moves::TongueLash | Moves::Belch | Moves::CeilingCreep)
}

type Platforms<'w, 's, 'q, 'r> = Query<'w, 's, (&'q Platform, &'r Transform), (Without<Mob>, Without<Player>)>;

/// What a sewer mob sees of, and does to, the world this frame.
pub(super) struct World<'a, 'b, 'w, 's, 'q, 'r> {
    pub terrain: &'a Terrain,
    pub platforms: &'a Platforms<'w, 's, 'q, 'r>,
    pub p: &'a mut Player,
    pub ppos: Vec2,
    pub bleed: &'a mut MessageWriter<'b, Bleed>,
    pub shake: &'a mut Shake,
    pub stop: &'a mut HitStop,
    pub t: f32,
    pub dt: f32,
}

/// Whether it can see the chameleon (the line of sight is checked by the caller).
pub(super) fn seen(m: &Mob, pos: Vec2, ppos: Vec2) -> bool {
    let to = ppos - pos;
    let near_home = |more: f32| (ppos.x - m.home.x).abs() < m.spec.leash + more;
    match m.spec.moves {
        Moves::CeilingCreep if !m.sewer.dropped => to.y < -20.0 && to.x.abs() < NEWT_SPOT,
        Moves::TongueLash => to.y.abs() < 200.0 && near_home(TONGUE_REACH),
        _ => to.y.abs() < 110.0 && near_home(150.0),
    }
}

/// Knocked by a blow from `side`: its new velocity.
pub(super) fn knock(m: &mut Mob, side: f32) -> Vec2 {
    m.sewer.latched = None;
    match m.spec.moves {
        Moves::Leech | Moves::TongueLash => {
            m.airborne = true;
            Vec2::new(side * 200.0, 200.0)
        }
        // Slashed off the ceiling: it falls.
        Moves::CeilingCreep if !m.sewer.dropped => {
            m.sewer.dropped = true;
            m.sewer.falling = true;
            m.airborne = true;
            Vec2::new(side * 120.0, 0.0)
        }
        _ => Vec2::new(side * 240.0, 0.0),
    }
}

/// One frame of a sewer mob (not dead): returns its new center.
pub(super) fn think(m: &mut Mob, pos: Vec2, sees: bool, touching: bool, w: &mut World) -> Vec2 {
    m.sewer.cooldown -= w.dt;
    let next = match m.spec.moves {
        Moves::Leech => leech(m, pos, sees, touching, w),
        Moves::TongueLash => toad(m, pos, sees, w),
        Moves::Belch => bloat(m, pos, sees, w),
        Moves::CeilingCreep => newt(m, pos, sees, touching, w),
        _ => pos,
    };
    let scrabble = if matches!(m.state, State::Windup(_)) { 25.0 } else { 0.0 };
    m.stride += w.dt * (m.vel.x.abs() * 0.12 + scrabble);
    next
}

fn ease(rate: f32, dt: f32) -> f32 {
    1.0 - (-rate * dt).exp()
}

fn sign(x: f32) -> f32 {
    if x >= 0.0 { 1.0 } else { -1.0 }
}

/// Minding its own business, or caught by surprise (walkers).
fn about(m: &mut Mob, pos: Vec2, w: &World) {
    match m.notice {
        Notice::Shock { how, .. } => flinch(m, how, w.ppos - pos, w.dt),
        _ => idle(m, pos, w.t, w.dt),
    }
}

/// Crawl `dx` along the ground, in whole steps; turns back when blocked.
fn crawl(m: &mut Mob, pos: Vec2, dx: f32, terrain: &Terrain) -> Vec2 {
    m.sewer.carry += dx;
    if m.sewer.carry.abs() < MIN_STEP {
        return pos;
    }
    let dx = std::mem::take(&mut m.sewer.carry);
    let (feet, blocked) = walk(terrain, pos - Vec2::Y * m.spec.foot, dx, m.home.x, m.spec);
    if blocked {
        m.vel.x = 0.0;
        if m.state == State::Idle {
            m.wander = (-m.wander.0, m.rng.range(1.0, 2.0));
        }
    }
    feet + Vec2::Y * m.spec.foot
}

/// Hurts the chameleon without knocking it about (a drain, a cloud): it keeps its momentum and footing.
fn sap(p: &mut Player, ppos: Vec2, bleed: &mut MessageWriter<Bleed>, shake: &mut Shake) -> Hurt {
    let (stun, keep) = (p.stun, p.vel);
    let hurt = p.hurt(1, keep);
    if !matches!(hurt, Hurt::Ignored) {
        p.stun = stun;
        shake.add(0.15);
        let amount = if matches!(hurt, Hurt::Killed) { 90 } else { 14 };
        bleed.write(Bleed { pos: ppos, dir: Vec2::Y, spread: 1.2, speed: 260.0, amount, fluid: Fluid::Red, sticky: false });
    }
    hurt
}

// --- Bile leech ---

fn leech(m: &mut Mob, pos: Vec2, sees: bool, touching: bool, w: &mut World) -> Vec2 {
    let spec = m.spec;
    let dt = w.dt;
    m.sewer.inch += dt * if matches!(m.state, State::Idle) { 4.0 + m.vel.x.abs() * 0.05 } else { 9.0 };
    if let Some(at) = m.sewer.latched {
        return ride(m, at, w);
    }
    let to = w.ppos - pos;
    let side = sign(to.x);
    m.state = match m.state {
        State::Hurt(s) => {
            if s < 0.3 { State::Hurt(s + dt) } else { State::Recover(spec.recover * 0.5) }
        }
        State::Idle if sees && !m.airborne && to.length() < LEECH_LEAP && m.sewer.cooldown <= 0.0 => {
            m.plan = w.ppos;
            m.facing = side;
            m.vel = Vec2::ZERO;
            State::Windup(0.0)
        }
        State::Idle => {
            if !m.airborne {
                about(m, pos, w);
            }
            m.facing = if sees { side } else if m.vel.x.abs() > 4.0 { sign(m.vel.x) } else { m.facing };
            State::Idle
        }
        State::Windup(s) if s < spec.windup => {
            m.facing = side;
            State::Windup(s + dt)
        }
        State::Windup(_) => {
            // A ballistic spring at the chameleon's body (only follows it a little).
            let to = m.plan.lerp(w.ppos, 0.3) - pos;
            let time = LEECH_LEAP_TIME;
            let vx = (to.x / time).clamp(-spec.attack_speed, spec.attack_speed);
            let vy = ((to.y + 0.5 * GRAVITY * time * time) / time).clamp(250.0, 620.0);
            m.vel = Vec2::new(vx, vy);
            m.airborne = true;
            State::Attack(0.0)
        }
        State::Attack(_) if touching && m.airborne && !w.p.god && w.p.dead.is_none() => {
            latch(m, pos, w);
            return pos;
        }
        // Missed: stranded on the floor, writhing.
        State::Attack(s) if !m.airborne && s > 0.05 => State::Recover(0.0),
        State::Attack(s) => State::Attack(s + dt),
        State::Recover(s) => {
            if !m.airborne {
                m.vel.x = 0.0;
            }
            if s < spec.recover {
                State::Recover(s + dt)
            } else {
                m.sewer.cooldown = m.rng.range(LEECH_COOLDOWN.0, LEECH_COOLDOWN.1);
                State::Idle
            }
        }
        dead @ State::Dead { .. } => dead,
    };
    if m.airborne {
        hop(m, w.terrain, w.platforms, pos, dt)
    } else {
        // Inchworm: it only moves while it stretches.
        let push = m.sewer.inch.sin().max(0.0) * 2.0;
        crawl(m, pos, m.vel.x * push * dt, w.terrain)
    }
}

/// Lands on the chameleon: holds on where it touched (on the back, rather).
fn latch(m: &mut Mob, pos: Vec2, w: &mut World) {
    let p = &*w.p;
    let rel = pos - w.ppos;
    let half = player::HALF;
    let at = Vec2::new((rel.x * p.facing).clamp(-half.x * 0.7, half.x * 0.7), rel.y.clamp(half.y * 0.4, half.y * 0.85));
    m.sewer.latched = Some(at);
    m.sewer.drain = LATCH_DRAIN;
    m.sewer.shakes = 0;
    m.sewer.last_facing = p.facing;
    m.sewer.was_grounded = p.grounded;
    m.airborne = false;
    m.vel = Vec2::ZERO;
    m.state = State::Attack(0.0);
    w.shake.add(0.12);
    w.bleed.write(Bleed { pos, dir: Vec2::Y, spread: 0.8, speed: 160.0, amount: 4, fluid: Fluid::Red, sticky: false });
}

/// Riding the chameleon: drinks until shaken off. Returns its new center.
fn ride(m: &mut Mob, at: Vec2, w: &mut World) -> Vec2 {
    let p = &mut *w.p;
    let pos = w.ppos + Vec2::new(at.x * p.facing, at.y);
    // Shaken: turning around, jumping; a dash or the water at once.
    if p.facing != m.sewer.last_facing {
        m.sewer.shakes += 1;
        m.sewer.last_facing = p.facing;
    }
    if m.sewer.was_grounded && !p.grounded && p.vel.y > 150.0 {
        m.sewer.shakes += 1;
    }
    m.sewer.was_grounded = p.grounded;
    if p.dash > 0.0 || p.swimming || p.dead.is_some() || p.god || m.sewer.shakes >= SHAKE_OFF {
        m.sewer.latched = None;
        m.airborne = true;
        let out = if at.x * p.facing >= 0.0 { 1.0 } else { -1.0 };
        m.vel = Vec2::new(out * 170.0, 260.0);
        m.facing = -out;
        m.state = State::Recover(0.0);
        return pos;
    }
    // Head toward the middle of the body.
    m.facing = if at.x == 0.0 { p.facing } else { -sign(at.x * p.facing) };
    m.sewer.drain -= w.dt;
    if m.sewer.drain <= 0.0 {
        if matches!(sap(p, w.ppos, w.bleed, w.shake), Hurt::Ignored) {
            // Invulnerable just now: tries again soon.
            m.sewer.drain = 0.3;
        } else {
            m.sewer.drain = LATCH_DRAIN;
            m.sewer.fed += 1.0;
        }
    }
    if let State::Attack(s) = m.state {
        m.state = State::Attack(s + w.dt);
    }
    pos
}

/// Leeches latched on slow the chameleon down (after the cobwebs had their say).
fn leech_drag(mobs: Query<&Mob>, mut player: Single<&mut Player>) {
    let n = mobs.iter().filter(|m| m.sewer.latched.is_some() && !matches!(m.state, State::Dead { .. })).count();
    if n > 0 {
        player.webbed = player.webbed.max((n as f32 * LATCH_DRAG).min(LATCH_DRAG_MAX));
    }
}

// --- Gullet toad ---

/// Where the toad's tongue comes out (the rig keeps level while it lashes).
pub(crate) fn toad_mouth(m: &Mob, pos: Vec2) -> Vec2 {
    pos + Vec2::new(TOAD_MOUTH.x * m.facing, TOAD_MOUTH.y) * m.spec.scale
}

fn toad(m: &mut Mob, pos: Vec2, sees: bool, w: &mut World) -> Vec2 {
    let spec = m.spec;
    let dt = w.dt;
    let mouth = toad_mouth(m, pos);
    let to = w.ppos - mouth;
    let side = sign(w.ppos.x - pos.x);
    m.state = match m.state {
        State::Hurt(s) => {
            if s < 0.3 { State::Hurt(s + dt) } else { State::Recover(spec.recover * 0.5) }
        }
        State::Idle
            if sees && !m.airborne && to.length() < TONGUE_REACH && m.sewer.cooldown <= 0.0 && w.terrain.raycast(mouth, w.ppos).is_none() =>
        {
            m.plan = w.ppos;
            m.facing = side;
            State::Windup(0.0)
        }
        State::Idle => {
            if !m.airborne && !matches!(m.notice, Notice::Shock { .. }) {
                // Little hops about; seen, toward the chameleon until in reach.
                let toward = (sees && to.length() > TONGUE_REACH * 0.8).then_some(side);
                if sees {
                    m.facing = side;
                }
                if toward.is_some() || !sees {
                    toad_hop(m, pos, toward, dt);
                }
            }
            State::Idle
        }
        State::Windup(s) if s < spec.windup => State::Windup(s + dt),
        State::Windup(_) => {
            // Shot at where it stood (following only a little).
            let target = m.plan.lerp(w.ppos, 0.25);
            m.sewer.aim = (target - mouth).normalize_or(Vec2::X * m.facing);
            m.sewer.reach = ((target - mouth).length() + TONGUE_OVERSHOOT).min(TONGUE_REACH);
            m.sewer.tongue = 0.0;
            State::Attack(0.0)
        }
        State::Attack(s) => {
            let len = (m.sewer.tongue + spec.attack_speed * dt).min(m.sewer.reach);
            let from = mouth + m.sewer.aim * m.sewer.tongue;
            let tip = mouth + m.sewer.aim * len;
            if let Some(hit) = w.terrain.raycast(from, tip) {
                // Slaps a wall.
                m.sewer.tongue = (hit - mouth).length();
                State::Recover(0.0)
            } else {
                m.sewer.tongue = len;
                let near = w.ppos - mouth;
                let along = near.dot(m.sewer.aim).clamp(0.0, len);
                let caught = (mouth + m.sewer.aim * along).distance(w.ppos) < TONGUE_GRAB;
                if caught && w.p.dead.is_none() {
                    grab(mouth, w);
                    State::Recover(0.0)
                } else if len >= m.sewer.reach {
                    State::Recover(0.0)
                } else {
                    State::Attack(s + dt)
                }
            }
        }
        State::Recover(s) => {
            if s < spec.recover {
                State::Recover(s + dt)
            } else {
                m.sewer.cooldown = m.rng.range(TOAD_COOLDOWN.0, TOAD_COOLDOWN.1);
                State::Idle
            }
        }
        dead @ State::Dead { .. } => dead,
    };
    if !matches!(m.state, State::Attack(_)) {
        m.sewer.tongue = (m.sewer.tongue - TONGUE_BACK * dt).max(0.0);
    }
    hop(m, w.terrain, w.platforms, pos, dt)
}

/// A little hop now and then: toward `toward`, else about its spot.
fn toad_hop(m: &mut Mob, pos: Vec2, toward: Option<f32>, dt: f32) {
    m.wander.1 -= dt;
    if m.wander.1 > 0.0 {
        return;
    }
    let away = pos.x - m.home.x;
    let dir = toward.unwrap_or(if away.abs() > m.spec.leash * 0.5 {
        -sign(away)
    } else if m.rng.chance(0.5) {
        1.0
    } else {
        -1.0
    });
    m.vel = Vec2::new(dir * m.spec.speed * m.rng.range(0.6, 1.1), m.rng.range(220.0, 300.0));
    m.facing = dir;
    m.airborne = true;
    m.wander.1 = m.rng.range(1.2, 2.6);
}

/// The tongue caught the chameleon: hurt and yanked in.
fn grab(mouth: Vec2, w: &mut World) {
    let pull = (mouth - w.ppos).normalize_or_zero() * TONGUE_PULL + Vec2::Y * 140.0;
    match w.p.hurt(1, pull) {
        Hurt::Ignored => {}
        hurt => {
            w.shake.add(0.3);
            w.stop.add(0.05);
            let amount = if matches!(hurt, Hurt::Killed) { 120 } else { 16 };
            w.bleed.write(Bleed { pos: w.ppos, dir: pull, spread: 0.8, speed: 340.0, amount, fluid: Fluid::Red, sticky: false });
        }
    }
}

// --- Plague bloat ---

fn bloat(m: &mut Mob, pos: Vec2, sees: bool, w: &mut World) -> Vec2 {
    let spec = m.spec;
    let dt = w.dt;
    let to = w.ppos - pos;
    let side = sign(to.x);
    m.state = match m.state {
        State::Hurt(s) => {
            m.vel.x *= 1.0 - 6.0 * dt;
            if s < 0.3 { State::Hurt(s + dt) } else { State::Recover(spec.recover * 0.5) }
        }
        State::Idle if sees && to.length() < BELCH_RANGE && m.sewer.cooldown <= 0.0 => {
            m.plan = w.ppos;
            m.facing = side;
            m.vel = Vec2::ZERO;
            State::Windup(0.0)
        }
        State::Idle => {
            if sees && !matches!(m.notice, Notice::Shock { .. }) {
                // Waddles at the chameleon.
                let go = if to.x.abs() > BELCH_RANGE * 0.6 { side } else { 0.0 };
                m.vel.x += (go * spec.speed * 1.6 - m.vel.x) * ease(6.0, dt);
                m.facing = side;
            } else {
                about(m, pos, w);
                m.facing = if m.vel.x.abs() > 4.0 { sign(m.vel.x) } else { m.facing };
            }
            State::Idle
        }
        State::Windup(s) if s < spec.windup => {
            m.facing = side;
            State::Windup(s + dt)
        }
        State::Windup(_) => {
            let mouth = pos + Vec2::new(BLOAT_MOUTH.x * m.facing, BLOAT_MOUTH.y) * spec.scale;
            let target = mouth + (m.plan.lerp(w.ppos, 0.3) - mouth).clamp_length_max(BELCH_REACH);
            m.sewer.belch = Some((mouth, (target - mouth) * MIASMA_DRAG));
            State::Recover(0.0)
        }
        State::Attack(_) => State::Recover(0.0),
        State::Recover(s) => {
            m.vel.x = 0.0;
            if s < spec.recover {
                State::Recover(s + dt)
            } else {
                m.sewer.cooldown = m.rng.range(BLOAT_COOLDOWN.0, BLOAT_COOLDOWN.1);
                State::Idle
            }
        }
        dead @ State::Dead { .. } => dead,
    };
    // Swells with the windup, sags empty after the belch.
    let swell = match m.state {
        State::Windup(_) => m.windup(),
        State::Recover(s) => -0.25 * (1.0 - s / spec.recover).max(0.0),
        _ => 0.0,
    };
    m.sewer.swell += (swell - m.sewer.swell) * ease(10.0, dt);
    crawl(m, pos, m.vel.x * dt, w.terrain)
}

/// A small poison cloud belched by a bloat (its puffs are children). Hurts while the chameleon is inside.
#[derive(Component)]
struct Miasma {
    vel: Vec2,
    age: f32,
    life: f32,
    harmful: bool,
    material: Handle<ColorMaterial>,
}

#[derive(Component)]
struct Puff {
    offset: Vec2,
    phase: f32,
    size: f32,
}

/// Bloats belch their clouds; the ones that just died burst (poisonous only if they were swollen).
fn belch(mut commands: Commands, mut materials: ResMut<Assets<ColorMaterial>>, cloud: Res<Cloud>, mut mobs: Query<(&mut Mob, &Transform)>) {
    for (mut m, tf) in &mut mobs {
        if m.kind != MobKind::PlagueBloat {
            continue;
        }
        let pos = tf.translation.truncate();
        let mut rng = m.rng.clone();
        if let Some((from, vel)) = m.sewer.belch.take() {
            spawn_miasma(&mut commands, &mut materials, &cloud, &mut rng, from, vel, MIASMA_LIFE, true);
        }
        if m.dismember && matches!(m.state, State::Dead { .. }) {
            let swollen = m.sewer.swell > 0.5;
            let life = if swollen { BURST_LIFE } else { PUFF_LIFE };
            spawn_miasma(&mut commands, &mut materials, &cloud, &mut rng, pos, Vec2::ZERO, life, swollen);
        }
        m.rng = rng;
    }
}

#[allow(clippy::too_many_arguments)]
fn spawn_miasma(commands: &mut Commands, materials: &mut Assets<ColorMaterial>, cloud: &Cloud, rng: &mut Rng, at: Vec2, vel: Vec2, life: f32, harmful: bool) {
    // A harmless puff is grayer.
    let tint = if harmful {
        Color::srgba(rng.range(0.55, 0.72), rng.range(0.0, 0.12), rng.range(0.85, 1.0), 0.0)
    } else {
        Color::srgba(0.55, 0.45, 0.62, 0.0)
    };
    let material = materials.add(ColorMaterial { color: tint, alpha_mode: AlphaMode2d::Blend, ..default() });
    let parent = commands
        .spawn((Transform::from_translation(at.extend(13.0)), Visibility::default(), Miasma { vel, age: 0.0, life, harmful, material: material.clone() }, MobStuff))
        .id();
    for i in 0..5 {
        let offset = if i == 0 { Vec2::ZERO } else { Vec2::from_angle(i as f32 * PI / 2.0 + rng.range(-0.4, 0.4)) * rng.range(0.3, 0.55) };
        commands.spawn((
            Mesh2d(cloud.0.clone()),
            MeshMaterial2d(material.clone()),
            Transform::from_xyz(0.0, 0.0, i as f32 * 0.01),
            camera::color_layer(),
            Puff { offset, phase: rng.range(0.0, PI * 2.0), size: rng.range(0.75, 1.1) },
            ChildOf(parent),
        ));
    }
}

/// The clouds drift to a stop, swell, swirl and fade; inside one the chameleon is poisoned
/// (one tick per invulnerability window, no knockback).
fn miasma(
    time: Res<Time>,
    mut commands: Commands,
    mut materials: ResMut<Assets<ColorMaterial>>,
    mut bleed: MessageWriter<Bleed>,
    mut shake: ResMut<Shake>,
    terrain: Res<Terrain>,
    player: Single<(&mut Player, &Transform), Without<Miasma>>,
    mut clouds: Query<(Entity, &mut Miasma, &mut Transform, &Children), Without<Player>>,
    mut puffs: Query<(&Puff, &mut Transform), (Without<Miasma>, Without<Player>)>,
) {
    let dt = time.delta_secs();
    let now = time.elapsed_secs();
    let (mut p, ptf) = player.into_inner();
    let ppos = ptf.translation.truncate();
    for (e, mut c, mut tf, children) in &mut clouds {
        c.age += dt;
        let k = c.age / c.life;
        if k >= 1.0 {
            commands.entity(e).despawn();
            continue;
        }
        let step = c.vel * dt;
        if terrain.solid_at(tf.translation.truncate() + step) {
            c.vel = Vec2::ZERO;
        } else {
            tf.translation += step.extend(0.0);
        }
        c.vel *= (-MIASMA_DRAG * dt).exp();
        let size = if c.harmful { 1.0 } else { 0.7 };
        let radius = MIASMA_RADIUS * size * (0.35 + 0.65 * (c.age / 0.5).min(1.0)) * (1.0 + 0.06 * (now * 1.9 + (e.to_bits() % 89) as f32).sin());
        let fade = (c.age / 0.2).min(1.0) * (1.0 - ((k - 0.7) / 0.3).clamp(0.0, 1.0));
        if let Some(mut mat) = materials.get_mut(&c.material) {
            mat.color = mat.color.with_alpha(0.34 * fade);
        }
        for child in children.iter() {
            let Ok((puff, mut ptf)) = puffs.get_mut(child) else { continue };
            let swirl = Vec2::from_angle(now * 0.8 + puff.phase).rotate(puff.offset);
            ptf.translation.x = swirl.x * radius;
            ptf.translation.y = swirl.y * radius * 0.75;
            ptf.scale = Vec3::splat(radius * puff.size * (1.0 + 0.1 * (now * 2.5 + puff.phase).sin()));
        }
        if c.harmful && fade > 0.5 && p.dead.is_none() && ppos.distance(tf.translation.truncate()) < radius * 0.8 {
            sap(&mut p, ppos, &mut bleed, &mut shake);
        }
    }
}

// --- Drain newt ---

/// The underside of the ceiling over `p` (within `reach` up); when `p` is
/// inside solid ground, the underside of that.
fn ceiling(terrain: &Terrain, p: Vec2, reach: f32) -> Option<f32> {
    let mut c = terrain.to_cell(p);
    let steps = (reach / PX).ceil() as i32;
    if terrain.solid(c) {
        for _ in 0..steps {
            c.y += 1;
            if !terrain.solid(c) {
                return Some(terrain.cell_min(c).y + PX);
            }
        }
        None
    } else {
        for _ in 0..steps {
            c.y -= 1;
            if terrain.solid(c) {
                return Some(terrain.cell_min(c).y);
            }
        }
        None
    }
}

/// A newt clinging under the ceiling at `back` has room for its body (the box
/// starts a step down, so the stair-stepped underside of a slope doesn't count).
fn hugs(terrain: &Terrain, back: Vec2, spec: &MobSpec) -> bool {
    let half = Vec2::new(spec.radius * 0.6, spec.foot * 0.6);
    !terrain.box_hits(back - Vec2::Y * (STEP_UP + half.y), half)
}

/// Where a newt can cling near `p` (its center, upside down under the ceiling), if anywhere.
pub(super) fn ceiling_site(terrain: &Terrain, p: Vec2, rect: Rect, spec: &MobSpec) -> Option<Vec2> {
    let top = ceiling(terrain, p, 40.0 * PX)?;
    let at = Vec2::new(p.x, top - spec.foot);
    let floor = terrain.surface(at, 900.0)?;
    // Some ceiling to creep along each way, and a fall to drop down.
    let creep = [-24.0, 24.0].into_iter().all(|dx| ceiling(terrain, Vec2::new(p.x + dx, top - STEP_UP), 2.0 * STEP_UP).is_some_and(|y| (y - top).abs() <= STEP_UP));
    let open = terrain.clearance(at - Vec2::Y * 40.0) >= 2.0 * PX;
    (rect.contains(at) && hugs(terrain, Vec2::new(p.x, top), spec) && creep && open && top - floor >= NEWT_MIN_DROP).then_some(at)
}

/// Creep `dx` along the ceiling, in whole steps; turns back at walls, edges and the leash.
fn creep(m: &mut Mob, pos: Vec2, dx: f32, terrain: &Terrain) -> Vec2 {
    m.sewer.carry += dx;
    if m.sewer.carry.abs() < MIN_STEP {
        return pos;
    }
    let dx = std::mem::take(&mut m.sewer.carry);
    let spec = m.spec;
    let n = (dx.abs() / 4.0).ceil().max(1.0) as i32;
    let step = dx / n as f32;
    let mut back = pos + Vec2::Y * spec.foot;
    let mut blocked = false;
    for _ in 0..n {
        let nx = back.x + step;
        let leashed = (nx - m.home.x).abs() > spec.leash && (nx - m.home.x).abs() > (back.x - m.home.x).abs();
        let next = ceiling(terrain, Vec2::new(nx, back.y - STEP_UP), 2.0 * STEP_UP).filter(|y| (y - back.y).abs() <= STEP_UP + 0.1);
        match next {
            Some(y) if !leashed && hugs(terrain, Vec2::new(nx, y), spec) => back = Vec2::new(nx, y),
            _ => {
                blocked = true;
                break;
            }
        }
    }
    if blocked {
        m.vel.x = 0.0;
        if m.state == State::Idle {
            m.wander = (-m.wander.0, m.rng.range(1.0, 2.0));
        }
    }
    back - Vec2::Y * spec.foot
}

fn newt(m: &mut Mob, pos: Vec2, sees: bool, touching: bool, w: &mut World) -> Vec2 {
    let spec = m.spec;
    let dt = w.dt;
    let turned = if m.sewer.dropped { 1.0 } else { 0.0 };
    m.sewer.turned += (turned - m.sewer.turned) * ease(7.0, dt);
    let to = w.ppos - pos;
    let side = sign(to.x);
    let bite = |m: &mut Mob, w: &mut World| {
        m.sewer.bit = true;
        crate::boss::hurt_player(&mut *w.p, w.ppos, pos, spec.damage, &mut *w.bleed, (&mut *w.shake, &mut *w.stop));
    };

    if !m.sewer.dropped {
        // Upside down on the ceiling: creeping about, stalking from above.
        m.state = match m.state {
            State::Idle if sees => {
                let lead = w.ppos.x + w.p.vel.x * NEWT_LEAD;
                let dx = lead - pos.x;
                if dx.abs() < NEWT_DROP && w.terrain.surface(pos, 900.0).is_some() {
                    m.plan = w.ppos;
                    m.vel.x = 0.0;
                    State::Windup(0.0)
                } else {
                    m.vel.x += (sign(dx) * spec.speed * 2.2 - m.vel.x) * ease(8.0, dt);
                    m.facing = sign(dx);
                    State::Idle
                }
            }
            State::Idle => {
                if matches!(m.notice, Notice::Shock { .. }) {
                    // Clings stock still.
                    m.vel.x = 0.0;
                } else {
                    idle(m, pos, w.t, dt);
                    m.facing = if m.vel.x.abs() > 4.0 { sign(m.vel.x) } else { m.facing };
                }
                State::Idle
            }
            State::Windup(s) if s < spec.windup => State::Windup(s + dt),
            State::Windup(_) => {
                // Lets go, a little toward where the chameleon will be.
                let x = m.plan.lerp(w.ppos, 0.4).x;
                m.vel = Vec2::new(((x - pos.x) * 2.5).clamp(-160.0, 160.0), -40.0);
                m.sewer.dropped = true;
                m.sewer.falling = true;
                m.sewer.bit = false;
                m.airborne = true;
                State::Attack(0.0)
            }
            // (Hurt on the ceiling knocks it off: see `knock`.)
            other => other,
        };
        if !m.sewer.dropped {
            return creep(m, pos, m.vel.x * dt, w.terrain);
        }
    }

    // Down: the drop lands, then it hunts along the ground.
    if m.sewer.falling && !m.airborne {
        m.sewer.falling = false;
        // Its ground now.
        m.home = pos;
        m.wander = (0.0, m.rng.range(0.3, 0.8));
        if matches!(m.state, State::Attack(_)) {
            if touching && !m.sewer.bit {
                bite(m, w);
            }
            // Sprawled where it fell: the opening.
            m.state = State::Recover(0.0);
        }
    }
    m.state = match m.state {
        State::Hurt(s) => {
            if !m.airborne {
                m.vel.x *= 1.0 - 6.0 * dt;
            }
            if s < 0.3 { State::Hurt(s + dt) } else { State::Recover(spec.recover * 0.5) }
        }
        // Falling on it.
        State::Attack(s) if m.sewer.falling => {
            if touching && !m.sewer.bit {
                bite(m, w);
            }
            State::Attack(s + dt)
        }
        State::Attack(_) if touching => {
            bite(m, w);
            m.vel.x *= -0.3;
            State::Recover(0.0)
        }
        State::Attack(s) if s > spec.attack_time => State::Recover(0.0),
        State::Attack(s) => State::Attack(s + dt),
        State::Idle if sees && to.length() < NEWT_LUNGE && to.y.abs() < 60.0 && !m.airborne => {
            m.plan = w.ppos;
            m.facing = side;
            m.vel.x = -side * 15.0;
            State::Windup(0.0)
        }
        State::Idle => {
            if sees && !matches!(m.notice, Notice::Shock { .. }) {
                // Scuttles at it.
                m.vel.x += (side * spec.speed * 2.5 - m.vel.x) * ease(8.0, dt);
                m.facing = side;
            } else {
                about(m, pos, w);
                m.facing = if m.vel.x.abs() > 4.0 { sign(m.vel.x) } else { m.facing };
            }
            State::Idle
        }
        State::Windup(s) if s < spec.windup => State::Windup(s + dt),
        State::Windup(_) => {
            m.facing = sign(m.plan.lerp(w.ppos, 0.2).x - pos.x);
            m.vel.x = m.facing * spec.attack_speed;
            State::Attack(0.0)
        }
        State::Recover(s) => {
            m.vel.x *= 1.0 - 6.0 * dt;
            if s < spec.recover { State::Recover(s + dt) } else { State::Idle }
        }
        dead @ State::Dead { .. } => dead,
    };
    if m.airborne {
        hop(m, w.terrain, w.platforms, pos, dt)
    } else {
        let next = crawl(m, pos, m.vel.x * dt, w.terrain);
        if next == pos && m.vel.x == 0.0 && matches!(m.state, State::Attack(_)) {
            // A lunge into a wall ends it.
            m.state = State::Recover(0.0);
        }
        next
    }
}

/// The old sewer's ecosystem: kinds and their share of the mobs. Leeches as
/// fodder, the frog's kin, and the sewer's classic vermin.
pub(super) fn menu() -> Vec<(MobKind, f32)> {
    vec![
        (MobKind::BileLeech, 0.25),
        (MobKind::GulletToad, 0.15),
        (MobKind::PlagueBloat, 0.1),
        (MobKind::DrainNewt, 0.1),
        (MobKind::Fly, 0.15),
        (MobKind::Rat, 0.15),
        (MobKind::SewerRoach, 0.1),
    ]
}
