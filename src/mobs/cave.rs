//! The cave system's own vermin (`Style::Cave`, the Burrower's region): the
//! giant centipede's kin and brood, and what shares the dark rock with it.
//! - **Hundredfoot**: a small centipede that crawls along any surface (floor,
//!   wall, ceiling, round the corners: it feels for the nearest rock and
//!   follows its face), its body a chain of links laid along the path its
//!   head took. From a ceiling it lets go over the chameleon and drops on it,
//!   from a wall it springs at it, on the floor it rears and rushes. After a
//!   drop or a leap it lies dazed where it lands (the chance to hit it).
//! - **Curlback**: an armored millipede. It ambles about on its many legs;
//!   seeing the chameleon, it rolls up into a ball (the tell) and bowls at
//!   where it was: it can't steer, it speeds up downhill and slows uphill,
//!   bounces off walls, flies off ledges. Rolled up, blows glance off its
//!   armor (and bat it back a little); once it stops it stays curled a moment,
//!   then unrolls, dazed: then it can be hurt. Hurt, it curls up at once.
//! - **Burrowling**: the Burrower's brood, pale and blind, living inside the
//!   rock under the floor. Buried it can't be hurt; it feels the chameleon's
//!   steps and follows them as a heap of rubble moving under the surface.
//!   Right under it, the ground cracks and swells (the tell), then it bursts
//!   out and bites, sways there a while (the chance to hit it), and sinks back.
//! - **Broodcoil**: a mother centipede coiled around her eggs, never leaving
//!   them. Close enough, she rears back and lashes her head out along the
//!   floor; the eggs hatch hundredfoots while the chameleon is around (sooner
//!   when she is hurt), a few at a time and only so many in all.
//!
//! Bodies and animation live in `mob_art/cave.rs`.

use std::{
    collections::{HashMap, VecDeque},
    f32::consts::{PI, TAU},
};

use bevy::prelude::*;

use super::{fits, walk, Mob, MobKind, MobSpec, Moves, Notice, Shock, State, GRAVITY, LEAP_TIME};
use crate::{
    gore::Fluid,
    mob_art,
    player::Player,
    terrain::{Terrain, PX, STEP_UP},
};

/// How a cave mob gets around (its `Moves::Cave`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Gait {
    /// Crawls along any surface of the rock; drops, springs or rushes.
    Crawl,
    /// Walks; rolls up into an armored ball and bowls along the ground.
    Roll,
    /// Travels inside the rock under the floor; bursts out of it.
    Burrow,
    /// Stays coiled on its spot; lashes out, hatches its eggs.
    Coil,
}

pub(super) static HUNDREDFOOT: MobSpec = MobSpec {
    moves: Moves::Cave(Gait::Crawl),
    hp: 1,
    scale: 1.4,
    radius: 13.0,
    foot: 4.0 * 1.4,
    blood: Fluid::Purple,
    speed: 75.0,
    sense: 380.0,
    windup: 0.45,
    // Rushing along the floor.
    attack_speed: 300.0,
    attack_time: 0.4,
    recover: 1.0,
    leash: 360.0,
    damage: 1,
    stubborn: false,
    initiative: 0.6,
    veteran: false,
    // 1 hp, but it comes from the walls and ceilings.
    food: 0.35,
};
pub(super) static CURLBACK: MobSpec = MobSpec {
    moves: Moves::Cave(Gait::Roll),
    hp: 3,
    scale: 1.8,
    radius: 18.0,
    foot: 4.0 * 1.8,
    // Millipedes ooze a pale yellow defensive fluid.
    blood: Fluid::Yellow,
    speed: 30.0,
    sense: 360.0,
    windup: 0.6,
    // Launch speed of the roll; it lasts at most `attack_time`.
    attack_speed: 380.0,
    attack_time: 3.0,
    recover: 1.7,
    leash: 420.0,
    damage: 1,
    stubborn: true,
    initiative: 1.2,
    veteran: false,
    // 3 hp, armored while it rolls.
    food: 1.0,
};
pub(super) static BURROWLING: MobSpec = MobSpec {
    moves: Moves::Cave(Gait::Burrow),
    hp: 2,
    scale: 1.5,
    radius: 16.0,
    foot: 4.0 * 1.5,
    blood: Fluid::Purple,
    speed: 45.0,
    sense: 420.0,
    // The ground cracking under the chameleon.
    windup: 0.55,
    // Following the chameleon's steps under the floor.
    attack_speed: 170.0,
    attack_time: 0.3,
    recover: 1.3,
    leash: 380.0,
    damage: 1,
    stubborn: true,
    initiative: 0.5,
    veteran: false,
    // 2 hp, out of reach most of the time.
    food: 0.75,
};
pub(super) static BROODCOIL: MobSpec = MobSpec {
    moves: Moves::Cave(Gait::Coil),
    hp: 4,
    scale: 2.0,
    radius: 24.0,
    foot: 5.0 * 2.0,
    blood: Fluid::Purple,
    speed: 0.0,
    sense: 520.0,
    windup: 0.65,
    attack_speed: 0.0,
    attack_time: 0.35,
    recover: 1.1,
    leash: 0.0,
    damage: 1,
    stubborn: true,
    initiative: 0.9,
    veteran: true,
    // 4 hp, a long lash and a brood (the hatchlings give nothing).
    food: 1.25,
};

/// The region's ecosystem.
pub(super) fn menu() -> Vec<(MobKind, f32)> {
    vec![
        (MobKind::Hundredfoot, 0.3),
        (MobKind::Burrowling, 0.2),
        (MobKind::Curlback, 0.2),
        (MobKind::Broodcoil, 0.1),
        // Cave spiders hang from the ceilings over the drops.
        (MobKind::Spider, 0.15),
        // Cave roaches: what everything down here eats.
        (MobKind::Roachling, 0.05),
    ]
}

// --- Crawling (hundredfoots) ---

/// Crawlers feel for rock this far around them; further, they lose their grip and fall.
const GRIP: f32 = 22.0;
/// The body: links behind the head, the distance between them, and the
/// spacing of the trail they are laid along (world units).
pub(crate) const LINKS: usize = 7;
pub(crate) const LINK: f32 = 6.0;
const TRAIL_STEP: f32 = 3.0;
const TRAIL_LEN: usize = (LINKS as f32 * LINK / TRAIL_STEP) as usize + 8;
/// On a ceiling, it drops when the chameleon is below it, this close sideways and down.
const DROP_X: f32 = 36.0;
const DROP_Y: f32 = 340.0;
/// On a wall, it springs from this far.
const SPRING: f32 = 170.0;
/// On the floor, it rushes from this close.
const RUSH: f32 = 110.0;
/// Seconds between two changes of its mind about which way to crawl.
const TURN_CD: f32 = 0.5;

// --- Rolling (curlbacks) ---

/// Radius of the rolled-up ball, in its drawing units (the feet are 4 below the center).
pub(crate) const BALL: f32 = 8.5;
/// Curling up or unrolling takes this long.
const CURL_TIME: f32 = 0.35;
/// It stays rolled up this long after the roll stops.
const CURL_HOLD: f32 = 0.5;
/// Rolled up past this, blows glance off.
const ARMORED: f32 = 0.5;
/// A roll hurts above this speed, and is over below `ROLL_STOP`.
const ROLL_HARM: f32 = 120.0;
const ROLL_STOP: f32 = 35.0;
/// Pull of a slope on the ball, rolling drag (per second), what a wall gives back, walls it takes.
const DOWNHILL: f32 = 900.0;
const ROLL_DRAG: f32 = 0.25;
const BOUNCE: f32 = 0.55;
const BOUNCES: u8 = 2;
/// The slope under the ball is measured this far each side.
const SLOPE_SPAN: f32 = 8.0;
/// A blow on the ball knocks it this fast away.
const BATTED: f32 = 200.0;

// --- Burrowing (burrowlings) ---

/// Out of the rock it stands this much higher (its center, world units).
pub(crate) const RAISE: f32 = 16.0;
/// Less out of the rock than this, nothing can reach it.
const HIDDEN: f32 = 0.35;
/// It cracks the ground when the chameleon is this close above it (sideways).
const UNDER: f32 = 16.0;
/// What its burst reaches: sideways, and up from the ground.
const BURST_W: f32 = 24.0;
const BURST_H: f32 = 56.0;
/// Coming out, going back in (seconds), and the rest after it is back in.
const RISE_TIME: f32 = 0.1;
const SINK_TIME: f32 = 0.35;
const REST: f32 = 0.8;

// --- Coiling (broodcoils) ---

/// The neck leaves the coil here, and the lash reaches this far (drawing units).
pub(crate) const NECK_BASE: Vec2 = Vec2::new(7.0, 1.0);
const STRIKE_X: f32 = 46.0;
/// It rears up when the chameleon is this close (world units, sideways).
const COIL_REACH: f32 = 100.0;
/// The lash bites what is this close to the neck.
const LASH: f32 = 20.0;
/// Eggs in a clutch, hatchlings out at once, and the time between two.
pub(crate) const CLUTCH: u8 = 5;
const HATCHLINGS: usize = 2;
const HATCH_EVERY: (f32, f32) = (4.0, 6.0);
/// Hurt, the next egg hatches within this long.
const STIRRED: f32 = 0.3;

/// What a cave mob has in mind (kept on its `Mob`).
#[derive(Default)]
pub struct Mind {
    /// Crawlers: the face of the rock it holds on to (unit normal, out of the rock; zero = none yet).
    pub(crate) n: Vec2,
    /// Crawlers: which way it crawls along that face (±1 along `n.perp()`), and when it may turn again.
    dir: f32,
    turn_cd: f32,
    /// Crawlers: where the head has been, newest first, with the face under it there.
    trail: VecDeque<(Vec2, Vec2)>,
    /// Crawlers: the head and the links behind it (positions and faces), for the art.
    pub(crate) links: [(Vec2, Vec2); LINKS + 1],
    /// Curlbacks: 0 = stretched out on its legs, 1 = a ball; how far the ball has turned (radians).
    pub(crate) curl: f32,
    pub(crate) roll: f32,
    bounces: u8,
    /// Burrowlings: 0 = in the rock, 1 = all the way out; rest before it can strike again.
    pub(crate) rise: f32,
    rest: f32,
    /// Its attack already bit (one bite per attack).
    struck: bool,
    /// Broodcoils: eggs hatched so far.
    pub(crate) hatched: u8,
}

/// What a cave mob sees of the world this frame.
pub(super) struct World<'a> {
    pub terrain: &'a Terrain,
    pub p: &'a Player,
    pub ppos: Vec2,
    pub dt: f32,
}

/// What it did this frame: where it is now, and whether it bit the chameleon.
pub(super) struct Step {
    pub at: Vec2,
    pub bite: bool,
}

pub(super) fn think(m: &mut Mob, pos: Vec2, gait: Gait, sees: bool, w: &World) -> Step {
    match gait {
        Gait::Crawl => crawl(m, pos, sees, w),
        Gait::Roll => roll(m, pos, sees, w),
        Gait::Burrow => burrow(m, pos, sees, w),
        Gait::Coil => coil(m, pos, sees, w),
    }
}

/// What a blow does to it.
pub(super) enum Blow {
    Lands,
    /// Out of reach (in the rock).
    Misses,
    /// Off its armor.
    Glances,
}

/// A slash or a spit reached it (`away`: its side of the chameleon, ±).
pub(super) fn blow(m: &mut Mob, away: f32) -> Blow {
    match m.spec.moves {
        Moves::Cave(Gait::Burrow) if m.cave.rise < HIDDEN => Blow::Misses,
        Moves::Cave(Gait::Roll) if m.cave.curl > ARMORED => {
            m.flash = 0.5;
            m.notice = Notice::Aware;
            m.vel.x += away.signum() * BATTED;
            Blow::Glances
        }
        _ => Blow::Lands,
    }
}

/// A spot near `p` where a cave mob of this gait can live (its center), if any.
pub(super) fn site(terrain: &Terrain, gait: Gait, p: Vec2, rect: Rect, spec: &MobSpec) -> Option<Vec2> {
    if gait == Gait::Crawl {
        // On the nearest face of rock, whichever way it faces.
        let (d, n) = feel(terrain, p, 10.0 * PX)?;
        let at = p - n * (d - spec.foot);
        let room = !terrain.solid_at(at) && !terrain.solid_at(at + n * 24.0);
        return (rect.contains(at) && room && terrain.surface(at, 900.0).is_some()).then_some(at);
    }
    let feet = Vec2::new(p.x, terrain.surface(p, rect.height())?);
    let (left, right) = terrain.flat_span(feet, 24.0);
    let span = match gait {
        // Room to bowl along.
        Gait::Roll => 20.0 * PX,
        _ => 10.0 * PX,
    };
    // Burrowlings need rock under the floor to live in.
    let rock = gait != Gait::Burrow || (terrain.solid_at(feet - Vec2::Y * PX * 0.5) && terrain.solid_at(feet - Vec2::Y * PX * 1.5));
    (rect.contains(feet) && rock && fits(terrain, feet, spec) && right - left >= span).then_some(feet + Vec2::Y * spec.foot)
}

/// Patrol: a new heading now and then (−1, 0 = pausing, 1), back toward home
/// when `away` (its offset from home along its way) gets past most of the leash.
fn wander(m: &mut Mob, away: f32, dt: f32) -> f32 {
    m.wander.1 -= dt;
    if m.wander.1 <= 0.0 {
        m.wander = if m.rng.chance(0.3) {
            (0.0, m.rng.range(0.6, 1.6))
        } else if away.abs() > m.spec.leash * 0.6 {
            (-away.signum(), m.rng.range(1.0, 2.5))
        } else {
            (if m.rng.chance(0.5) { 1.0 } else { -1.0 }, m.rng.range(1.0, 2.5))
        };
    }
    m.wander.0
}

/// Caught by surprise: which way it bolts (0 = it freezes), from the chameleon's side `side`.
fn bolt(how: Shock, side: f32) -> f32 {
    match how {
        Shock::Freeze => 0.0,
        Shock::Flee | Shock::Panic => -side,
        Shock::Scatter(a) => {
            if a.cos() >= 0.0 {
                1.0
            } else {
                -1.0
            }
        }
    }
}

fn smooth(x: f32) -> f32 {
    let x = x.clamp(0.0, 1.0);
    x * x * (3.0 - 2.0 * x)
}

/// The nearest rock within `range` of `p`: its distance and the face's normal
/// there (out of the rock). A fan of short rays; the normal is averaged over
/// the nearest hits, so the stair-stepped cells of a slope read as one face.
fn feel(terrain: &Terrain, p: Vec2, range: f32) -> Option<(f32, Vec2)> {
    const RAYS: usize = 20;
    let mut hits = [f32::MAX; RAYS];
    let mut best = f32::MAX;
    for (i, hit) in hits.iter_mut().enumerate() {
        let d = Vec2::from_angle(i as f32 / RAYS as f32 * TAU);
        let mut r = 1.0;
        while r <= range {
            if terrain.solid_at(p + d * r) {
                *hit = r;
                best = best.min(r);
                break;
            }
            r += 2.0;
        }
    }
    if best == f32::MAX {
        return None;
    }
    let mut n = Vec2::ZERO;
    for (i, &h) in hits.iter().enumerate() {
        if h < best + 6.0 {
            n -= Vec2::from_angle(i as f32 / RAYS as f32 * TAU) / (1.0 + (h - best) * 0.3);
        }
    }
    Some((best, n.normalize_or(Vec2::Y)))
}

/// Hundredfoots: crawl the rock, strike from wherever they are.
fn crawl(m: &mut Mob, pos: Vec2, sees: bool, w: &World) -> Step {
    let spec = m.spec;
    let dt = w.dt;
    let to = w.ppos - pos;
    let side = if to.x >= 0.0 { 1.0 } else { -1.0 };
    let touching = w.p.dead.is_none() && to.length() < spec.radius + 12.0;
    if m.cave.dir == 0.0 {
        m.cave.dir = m.facing;
    }
    m.cave.turn_cd -= dt;
    if m.state == State::Hurt(0.0) {
        // Knocked off the rock.
        m.airborne = true;
        m.vel.y = 180.0;
    }
    let n = m.cave.n;
    let gripping = !m.airborne && n != Vec2::ZERO;
    let mut bite = false;
    m.state = match m.state {
        State::Hurt(s) if s < 0.3 => State::Hurt(s + dt),
        State::Hurt(_) => State::Recover(spec.recover * 0.5),
        State::Idle if sees && gripping && in_reach(n, to) => {
            m.plan = w.ppos;
            State::Windup(0.0)
        }
        State::Windup(s) if s < spec.windup => State::Windup(s + dt),
        State::Windup(_) => {
            strike(m, pos, w);
            State::Attack(0.0)
        }
        State::Attack(_) if touching => {
            bite = true;
            if m.airborne {
                m.vel.x *= -0.3;
            } else {
                m.cave.dir = -m.cave.dir;
            }
            State::Recover(0.0)
        }
        State::Attack(s) if (!m.airborne && s > spec.attack_time) || s > 3.0 => State::Recover(0.0),
        State::Attack(s) => State::Attack(s + dt),
        State::Recover(s) if s < spec.recover => State::Recover(s + dt),
        State::Recover(_) => State::Idle,
        other => other,
    };

    let speed = match m.state {
        State::Idle => {
            let t = m.cave.n.perp();
            let (dir, speed) = match m.notice {
                Notice::Shock { how, .. } => {
                    let d = bolt(how, side);
                    (if d == 0.0 { 0.0 } else { t.dot(Vec2::new(d, 0.0)).signum() }, spec.speed * 2.0)
                }
                // Toward the chameleon (over it, on a ceiling), holding still once there.
                _ if sees => {
                    let along = t.dot(to);
                    (if along.abs() > 12.0 { along.signum() } else { 0.0 }, spec.speed * 1.5)
                }
                _ => (wander(m, t.dot(pos - m.home), dt), spec.speed),
            };
            if dir != 0.0 && dir != m.cave.dir && m.cave.turn_cd <= 0.0 {
                m.cave.dir = dir;
                m.cave.turn_cd = TURN_CD;
            }
            if dir == 0.0 { 0.0 } else { speed }
        }
        State::Attack(_) => spec.attack_speed,
        // Dazed where it landed, then off.
        State::Recover(s) if s > spec.recover * 0.6 => spec.speed,
        _ => 0.0,
    };
    let next = if m.airborne { fall(m, pos, w) } else { creep(m, pos, speed, w) };
    if m.vel.x.abs() > 4.0 {
        m.facing = m.vel.x.signum();
    }
    m.stride += dt * m.vel.length().min(400.0) * 0.12;
    lay_trail(m, next);
    Step { at: next, bite }
}

/// Close enough to strike from where it holds on.
fn in_reach(n: Vec2, to: Vec2) -> bool {
    if n.y < -0.6 {
        to.x.abs() < DROP_X && to.y < -16.0 && to.y > -DROP_Y
    } else if n.y > 0.6 {
        to.length() < RUSH && to.y.abs() < 40.0
    } else {
        to.length() < SPRING && to.y < 60.0 && n.dot(to) > 0.0
    }
}

/// The telegraph is over: drop off a ceiling, spring off a wall, rush along the floor.
fn strike(m: &mut Mob, pos: Vec2, w: &World) {
    let spec = m.spec;
    let n = m.cave.n;
    let to = m.plan.lerp(w.ppos, 0.3) - pos;
    if n.y < -0.6 {
        m.vel = Vec2::new(to.x * 2.0, -60.0);
        m.airborne = true;
    } else if n.y > 0.6 {
        let along = n.perp().dot(to);
        if along != 0.0 {
            m.cave.dir = along.signum();
        }
    } else {
        let vx = (to.x / LEAP_TIME).clamp(-spec.attack_speed * 1.4, spec.attack_speed * 1.4);
        let vy = ((to.y + 0.5 * GRAVITY * LEAP_TIME * LEAP_TIME) / LEAP_TIME).clamp(60.0, 560.0);
        m.vel = Vec2::new(vx, vy);
        m.airborne = true;
    }
}

/// Crawling along the face of the rock at `speed`, `foot` off it: the face
/// under it turns as it goes, so it follows the rock round corners, up walls
/// and onto ceilings.
fn creep(m: &mut Mob, pos: Vec2, speed: f32, w: &World) -> Vec2 {
    let spec = m.spec;
    let terrain = w.terrain;
    let dt = w.dt;
    if terrain.solid_at(pos) {
        // Squeezed into the rock: out the way its face looks.
        return pos + m.cave.n.normalize_or(Vec2::Y) * 2.0;
    }
    let Some((d, n)) = feel(terrain, pos, GRIP) else {
        // Nothing to hold on to.
        m.airborne = true;
        return pos;
    };
    let k = 1.0 - (-14.0 * dt).exp();
    m.cave.n = if m.cave.n == Vec2::ZERO { n } else { m.cave.n.lerp(n, k).normalize_or(n) };
    let n = m.cave.n;
    let radial = ((spec.foot - d) * 12.0).clamp(-240.0, 240.0);
    m.vel = n.perp() * m.cave.dir * speed + n * radial;
    let next = pos + m.vel * dt;
    let off = next - m.home;
    let leashed = off.length() > spec.leash && off.length() > (pos - m.home).length();
    if (leashed && !matches!(m.state, State::Attack(_))) || terrain.solid_at(next) {
        // The end of its range, or a crack too tight: back the other way.
        m.cave.dir = -m.cave.dir;
        return pos;
    }
    next
}

/// Off the rock: falls until it touches rock again, and holds on to it.
fn fall(m: &mut Mob, pos: Vec2, w: &World) -> Vec2 {
    let spec = m.spec;
    let terrain = w.terrain;
    m.vel.y = (m.vel.y - GRAVITY * w.dt).max(-900.0);
    let next = pos + m.vel * w.dt;
    if next.y < terrain.bounds.min.y - 600.0 {
        m.airborne = false;
        m.cave.n = Vec2::ZERO;
        m.vel = Vec2::ZERO;
        m.cave.trail.clear();
        return m.home;
    }
    if terrain.solid_at(next) {
        let n = feel(terrain, pos, GRIP).map_or(-m.vel.normalize_or(Vec2::NEG_Y), |(_, n)| n);
        land(m, n);
        return pos;
    }
    match feel(terrain, next, spec.foot + 2.0) {
        Some((_, n)) if m.vel.dot(n) < 0.0 => {
            land(m, n);
            next
        }
        _ => next,
    }
}

fn land(m: &mut Mob, n: Vec2) {
    m.airborne = false;
    m.cave.n = n;
    m.vel = Vec2::ZERO;
    if matches!(m.state, State::Attack(_)) {
        m.state = State::Recover(0.0);
    }
}

/// Where the head went, and the links laid along it.
fn lay_trail(m: &mut Mob, head: Vec2) {
    let n = if m.cave.n == Vec2::ZERO { Vec2::Y } else { m.cave.n };
    let dir = if m.cave.dir == 0.0 { 1.0 } else { m.cave.dir };
    let trail = &mut m.cave.trail;
    if trail.is_empty() {
        // New: the body lies along the face behind the head.
        let back = -n.perp() * dir;
        trail.extend((0..TRAIL_LEN).map(|i| (head + back * i as f32 * TRAIL_STEP, n)));
    }
    let last = trail[0].0;
    let d = head - last;
    let l = d.length();
    if l >= TRAIL_STEP {
        for i in 1..=(l / TRAIL_STEP) as usize {
            trail.push_front((last + d * (i as f32 * TRAIL_STEP / l), n));
        }
        trail.truncate(TRAIL_LEN);
    }
    let links = &mut m.cave.links;
    links[0] = (head, n);
    let (mut cur, mut next, mut left, mut k) = (head, 0, LINK, 1);
    while k <= LINKS {
        let Some(&(to, tn)) = trail.get(next) else {
            links[k] = (cur, links[k - 1].1);
            k += 1;
            continue;
        };
        let l = (to - cur).length();
        if l >= left {
            cur += (to - cur) / l * left;
            links[k] = (cur, tn);
            k += 1;
            left = LINK;
        } else {
            left -= l;
            cur = to;
            next += 1;
        }
    }
}

/// Curlbacks: amble, roll up, bowl at the chameleon, unroll dazed.
fn roll(m: &mut Mob, pos: Vec2, sees: bool, w: &World) -> Step {
    let spec = m.spec;
    let dt = w.dt;
    let to = w.ppos - pos;
    let side = if to.x >= 0.0 { 1.0 } else { -1.0 };
    let touching = w.p.dead.is_none() && to.length() < spec.radius + 14.0;
    let ease = |rate: f32| 1.0 - (-rate * dt).exp();
    let mut bite = false;
    m.state = match m.state {
        State::Hurt(s) if s < 0.3 => State::Hurt(s + dt),
        // Hurt: it rolls up at once (and at whoever did it).
        State::Hurt(_) if sees => {
            m.facing = side;
            m.plan = w.ppos;
            State::Windup(0.0)
        }
        State::Hurt(_) => State::Recover(CURL_HOLD),
        State::Idle if sees && !m.airborne && to.y.abs() < 120.0 => {
            m.facing = side;
            m.plan = w.ppos;
            State::Windup(0.0)
        }
        State::Windup(s) if s < spec.windup => State::Windup(s + dt),
        State::Windup(_) => {
            // Off at where the chameleon was: a ball can't steer.
            let aim = m.plan.lerp(w.ppos, 0.25).x - pos.x;
            m.facing = if aim >= 0.0 { 1.0 } else { -1.0 };
            m.vel = Vec2::new(m.facing * spec.attack_speed, 0.0);
            m.cave.bounces = 0;
            State::Attack(0.0)
        }
        State::Attack(_) if touching && m.vel.x.abs() > ROLL_HARM => {
            bite = true;
            m.vel = Vec2::new(-m.vel.x * 0.35, 220.0);
            m.airborne = true;
            State::Recover(0.0)
        }
        State::Attack(s) if s > spec.attack_time || m.cave.bounces > BOUNCES || (!m.airborne && s > 0.3 && m.vel.x.abs() < ROLL_STOP) => {
            State::Recover(0.0)
        }
        State::Attack(s) => State::Attack(s + dt),
        State::Recover(s) if s < spec.recover => State::Recover(s + dt),
        State::Recover(_) => State::Idle,
        other => other,
    };
    // Rolled up for the roll and a moment after; unrolls, dazed, for the rest of the recovery.
    let curled = match m.state {
        State::Windup(_) | State::Attack(_) => true,
        State::Recover(s) => s < CURL_HOLD,
        _ => false,
    };
    let goal = if curled { 1.0 } else { 0.0 };
    m.cave.curl += (goal - m.cave.curl).clamp(-dt / CURL_TIME, dt / CURL_TIME);

    let feet = pos - Vec2::Y * spec.foot;
    let next = if m.airborne {
        tumble(m, pos, w)
    } else if matches!(m.state, State::Attack(_)) {
        bowl(m, feet, w) + Vec2::Y * spec.foot
    } else {
        let want = match (m.state, m.notice) {
            (State::Idle, Notice::Shock { how, .. }) => bolt(how, side) * spec.speed * 2.5,
            (State::Idle, _) => wander(m, feet.x - m.home.x, dt) * spec.speed,
            _ => 0.0,
        };
        // A ball coming to rest rolls on a bit.
        m.vel.x += (want - m.vel.x) * ease(if m.cave.curl > ARMORED { 2.0 } else { 8.0 });
        let (f, blocked) = walk(w.terrain, feet, m.vel.x * dt, m.home.x, spec);
        if blocked {
            m.vel.x = 0.0;
            m.wander.0 = -m.wander.0;
        }
        if !curled && m.vel.x.abs() > 8.0 {
            m.facing = m.vel.x.signum();
        }
        f + Vec2::Y * spec.foot
    };
    if m.cave.curl > 0.9 {
        // Rolls without slipping.
        m.cave.roll -= (next.x - pos.x) / (BALL * spec.scale);
    } else {
        // Turns back upright as it unrolls (whole turns don't show).
        let r = (m.cave.roll + PI).rem_euclid(TAU) - PI;
        m.cave.roll = r * (1.0 - ease(10.0));
    }
    m.stride += dt * m.vel.x.abs() * 0.15 * (1.0 - m.cave.curl);
    Step { at: next, bite }
}

/// Rolling along the ground from `feet`: downhill it speeds up, uphill it
/// slows down, walls (and the end of its range) bounce it back, off a ledge it flies.
fn bowl(m: &mut Mob, feet: Vec2, w: &World) -> Vec2 {
    let terrain = w.terrain;
    let spec = m.spec;
    let dt = w.dt;
    let ground = |x: f32| terrain.surface(Vec2::new(x, feet.y + STEP_UP * 1.5), 3.0 * STEP_UP);
    if let (Some(a), Some(b)) = (ground(feet.x - SLOPE_SPAN), ground(feet.x + SLOPE_SPAN)) {
        let slope = (b - a) / (2.0 * SLOPE_SPAN);
        m.vel.x -= slope / (1.0 + slope * slope).sqrt() * DOWNHILL * dt;
    }
    m.vel.x *= 1.0 - ROLL_DRAG * dt;
    let dx = m.vel.x * dt;
    let n = (dx.abs() / 4.0).ceil().max(1.0) as i32;
    let step = dx / n as f32;
    let r = BALL * spec.scale * 0.8;
    let mut f = feet;
    for _ in 0..n {
        let nx = f.x + step;
        let away = (nx - m.home.x).abs() > spec.leash * 1.5 && (nx - m.home.x).abs() > (f.x - m.home.x).abs();
        let ny = terrain.surface(Vec2::new(nx, f.y + STEP_UP), 2.0 * STEP_UP);
        if ny.is_none() && !away {
            // Over the edge.
            m.airborne = true;
            m.vel.y = 0.0;
            return Vec2::new(nx, f.y);
        }
        let wall = ny.is_none_or(|ny| ny - f.y > STEP_UP + 0.1 || terrain.box_hits(Vec2::new(nx, ny + STEP_UP + r), Vec2::splat(r * 0.7)));
        if away || wall {
            m.vel.x = -m.vel.x * BOUNCE;
            m.cave.bounces += 1;
            break;
        }
        f = Vec2::new(nx, ny.unwrap_or(f.y));
    }
    f
}

/// In the air: falls, bounces off walls; a fast ball bounces a little when it lands.
fn tumble(m: &mut Mob, pos: Vec2, w: &World) -> Vec2 {
    let terrain = w.terrain;
    let spec = m.spec;
    m.vel.y -= GRAVITY * w.dt;
    let mut next = pos + m.vel * w.dt;
    let body = Vec2::new(spec.radius * 0.4, spec.foot * 0.4);
    if terrain.box_hits(Vec2::new(next.x, pos.y + 4.0), body) {
        next.x = pos.x;
        m.vel.x = -m.vel.x * BOUNCE;
        m.cave.bounces += 1;
    }
    if m.vel.y > 0.0 && terrain.box_hits(Vec2::new(next.x, next.y + body.y), Vec2::new(body.x, 2.0)) {
        next.y = pos.y;
        m.vel.y = 0.0;
    }
    if let Some(top) = terrain.landing(pos, next, spec.foot) {
        next.y = top + spec.foot;
        m.vel.y = if m.cave.curl > ARMORED && m.vel.y < -300.0 { -m.vel.y * 0.3 } else { 0.0 };
        m.airborne = m.vel.y > 0.0;
    }
    if next.y < terrain.bounds.min.y - 600.0 {
        next = m.home;
        m.vel = Vec2::ZERO;
        m.airborne = false;
    }
    next
}

/// Burrowlings: follow the chameleon's steps inside the rock, burst out under it.
fn burrow(m: &mut Mob, pos: Vec2, sees: bool, w: &World) -> Step {
    let spec = m.spec;
    let dt = w.dt;
    let ease = |rate: f32| 1.0 - (-rate * dt).exp();
    let feet = pos - Vec2::Y * (spec.foot + m.cave.rise * RAISE);
    let dx = w.ppos.x - feet.x;
    let above = w.ppos.y - feet.y;
    // It feels the chameleon's steps on its rock.
    let felt = sees && w.p.grounded && above > -10.0 && above < 90.0;
    let mut bite = false;
    m.cave.rest -= dt;
    m.state = match m.state {
        State::Hurt(s) if s < 0.3 => State::Hurt(s + dt),
        State::Hurt(_) => State::Recover(spec.recover * 0.6),
        State::Idle if felt && dx.abs() < UNDER && m.cave.rest <= 0.0 && m.cave.rise < 0.05 => {
            // Commits to the spot: the chameleon can step off the cracks.
            m.plan = w.ppos;
            State::Windup(0.0)
        }
        State::Windup(s) if s < spec.windup => State::Windup(s + dt),
        State::Windup(_) => {
            m.cave.struck = false;
            State::Attack(0.0)
        }
        State::Attack(s) => {
            if !m.cave.struck && w.p.dead.is_none() && dx.abs() < BURST_W && above > -6.0 && above < BURST_H {
                bite = true;
                m.cave.struck = true;
            }
            if s < spec.attack_time { State::Attack(s + dt) } else { State::Recover(0.0) }
        }
        State::Recover(s) if s < spec.recover || m.cave.rise > 0.0 => State::Recover(s + dt),
        State::Recover(_) => {
            m.cave.rest = REST;
            State::Idle
        }
        other => other,
    };
    let out = match m.state {
        State::Attack(_) | State::Hurt(_) => true,
        State::Recover(s) => s < spec.recover,
        _ => false,
    };
    m.cave.rise = if out { (m.cave.rise + dt / RISE_TIME).min(1.0) } else { (m.cave.rise - dt / SINK_TIME).max(0.0) };

    let want = match m.state {
        State::Idle if m.cave.rise < 0.05 => match m.notice {
            Notice::Shock { .. } => 0.0,
            _ if felt => (dx * 5.0).clamp(-spec.attack_speed, spec.attack_speed),
            _ => wander(m, feet.x - m.home.x, dt) * spec.speed,
        },
        _ => 0.0,
    };
    m.vel.x += (want - m.vel.x) * ease(8.0);
    let (feet, blocked) = tunnel(w.terrain, feet, m.vel.x * dt, m.home.x, spec);
    if blocked {
        m.vel.x = 0.0;
        m.wander.0 = -m.wander.0;
    }
    if m.vel.x.abs() > 8.0 {
        m.facing = m.vel.x.signum();
    } else if out && sees {
        m.facing = if dx >= 0.0 { 1.0 } else { -1.0 };
    }
    m.stride += dt * m.vel.x.abs() * 0.2;
    Step { at: feet + Vec2::Y * (spec.foot + m.cave.rise * RAISE), bite }
}

/// Burrowing `dx` along just under the top of the rock from `feet`: it keeps
/// to the top whatever the steps, but not up or down walls, not across gaps
/// or where the crust is too thin, not past the leash. Returns the new feet
/// and whether it was blocked.
fn tunnel(terrain: &Terrain, feet: Vec2, dx: f32, home_x: f32, spec: &MobSpec) -> (Vec2, bool) {
    let n = (dx.abs() / 4.0).ceil().max(1.0) as i32;
    let step = dx / n as f32;
    let mut f = feet;
    for _ in 0..n {
        let nx = f.x + step;
        if step != 0.0 && (nx - home_x).abs() > spec.leash && (nx - home_x).abs() > (f.x - home_x).abs() {
            return (f, true);
        }
        let probe = Vec2::new(nx, f.y - PX * 0.5);
        let reach = if terrain.solid_at(probe) { 3.0 * PX } else { 2.0 * PX };
        let Some(ny) = terrain.surface(probe, reach) else { return (f, true) };
        if (ny - f.y).abs() > 2.0 * PX + 0.1 || !terrain.solid_at(Vec2::new(nx, ny - PX * 1.5)) {
            return (f, true);
        }
        f = Vec2::new(nx, ny);
    }
    (f, false)
}

/// Broodcoils: guard the eggs, lash at what comes close.
fn coil(m: &mut Mob, pos: Vec2, sees: bool, w: &World) -> Step {
    let spec = m.spec;
    let dt = w.dt;
    let to = w.ppos - pos;
    let side = if to.x >= 0.0 { 1.0 } else { -1.0 };
    let mut bite = false;
    m.state = match m.state {
        State::Hurt(s) if s < 0.3 => {
            if s == 0.0 {
                // The clutch stirs.
                m.cooldown = m.cooldown.min(STIRRED);
            }
            State::Hurt(s + dt)
        }
        State::Hurt(_) => State::Recover(spec.recover * 0.6),
        State::Idle if sees && to.x.abs() < COIL_REACH && to.y.abs() < 60.0 => {
            m.facing = side;
            m.plan = w.ppos;
            m.cave.struck = false;
            State::Windup(0.0)
        }
        State::Idle => {
            if sees {
                m.facing = side;
            }
            State::Idle
        }
        State::Windup(s) if s < spec.windup => State::Windup(s + dt),
        State::Windup(_) => State::Attack(0.0),
        State::Attack(s) => {
            let world = |q: Vec2| pos + Vec2::new(q.x * m.facing, q.y) * spec.scale;
            let (base, head) = (world(NECK_BASE), world(coil_head(m, 0.0)));
            if !m.cave.struck && w.p.dead.is_none() && to_segment(w.ppos, base, head) < LASH {
                bite = true;
                m.cave.struck = true;
            }
            if s < spec.attack_time { State::Attack(s + dt) } else { State::Recover(0.0) }
        }
        State::Recover(s) if s < spec.recover => State::Recover(s + dt),
        State::Recover(_) => State::Idle,
        other => other,
    };
    Step { at: pos, bite }
}

fn to_segment(p: Vec2, a: Vec2, b: Vec2) -> f32 {
    let ab = b - a;
    let t = ((p - a).dot(ab) / ab.length_squared().max(1e-4)).clamp(0.0, 1.0);
    p.distance(a + ab * t)
}

/// Where a broodcoil's head is (its drawing units, facing +X): swaying over
/// the coil, reared back in the tell, lashed out along the floor, lying
/// there, drawn back.
pub(crate) fn coil_head(m: &Mob, tt: f32) -> Vec2 {
    let spec = m.spec;
    let rest = Vec2::new(13.0 + (tt * 1.3).sin() * 1.5, 8.0 + (tt * 0.9).sin() * 1.5);
    let reared = Vec2::new(3.0, 19.0);
    let struck = Vec2::new(STRIKE_X, 0.0);
    match m.state {
        State::Windup(s) => rest.lerp(reared, smooth(s / spec.windup)),
        State::Attack(s) => {
            let k = (s / (spec.attack_time * 0.5)).min(1.0);
            reared.lerp(struck, 1.0 - (1.0 - k).powi(3))
        }
        State::Recover(s) => (struck - Vec2::Y).lerp(rest, smooth((s / spec.recover - 0.5) / 0.5)),
        State::Hurt(s) => rest + Vec2::new(-3.0, 2.0) * (1.0 - s / 0.3).max(0.0),
        State::Dead { .. } => Vec2::new(15.0, -2.0),
        State::Idle => rest,
    }
}

/// Broodcoils' eggs hatch hundredfoots while the chameleon is around (sooner
/// when the mother is hurt): `HATCHLINGS` out at once, `CLUTCH` in all.
pub(super) fn hatch(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    time: Res<Time>,
    player: Single<(&Player, &Transform)>,
    mut mobs: Query<(Entity, &mut Mob, &Transform)>,
) {
    let dt = time.delta_secs();
    let (p, ptf) = *player;
    let ppos = ptf.translation.truncate();
    let mut brood: HashMap<Entity, usize> = HashMap::new();
    for (_, m, _) in &mobs {
        if let (Some(mother), false) = (m.parent, matches!(m.state, State::Dead { .. })) {
            *brood.entry(mother).or_default() += 1;
        }
    }
    for (e, mut m, tf) in &mut mobs {
        if m.kind != MobKind::Broodcoil || m.buried.is_some() || matches!(m.state, State::Dead { .. }) || m.notice != Notice::Aware {
            continue;
        }
        let pos = tf.translation.truncate();
        if p.dead.is_some() || pos.distance(ppos) > m.spec.sense {
            continue;
        }
        m.cooldown -= dt;
        if m.cooldown > 0.0 || brood.get(&e).copied().unwrap_or(0) >= HATCHLINGS || m.cave.hatched >= CLUTCH {
            continue;
        }
        m.cooldown = m.rng.range(HATCH_EVERY.0, HATCH_EVERY.1);
        m.cave.hatched += 1;
        let feet = pos - Vec2::Y * m.spec.foot;
        let at = feet + Vec2::new(m.rng.range(-8.0, 8.0), MobKind::Hundredfoot.spec().foot);
        mob_art::spawn(&mut commands, &mut meshes, &mut materials, MobKind::Hundredfoot, at, Some(e));
    }
}
