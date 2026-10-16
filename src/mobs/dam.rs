//! The broken dam's vermin (`Style::Dam`): the giant crab's ecosystem, its
//! kin and what lives off the drowned. They bleed the crab's indigo
//! hemolymph, except the leech, full of someone else's blood.
//! - **Gallery louse** (giant isopod, fodder): crawls upside down along the
//!   ceilings of the inspection galleries and overhangs, lets go on the
//!   chameleon passing below (a twitch first), curls into a ball as it falls,
//!   and on the floor rolls at it, ricocheting off walls, then lies dizzy.
//! - **Seep leech**: inches along (rear sucker, stretch, front sucker, pull:
//!   the anchors stay put), rears up swaying, lunges and **latches on**: the
//!   chameleon drags it (slower walk) while it swells with blood (the red
//!   shows through, on the color layer); full, it bites (1) and drops off
//!   gorged. A hit or a dash tears it off.
//! - **Spillway crab**: keeps its crusher claw up in front of its face like
//!   a shield (slashes and venom from the front glance off), dances in and
//!   out at boxing distance, snaps (claw raised and opened = the tell, the
//!   guard is down) and is left with the claw jammed in the ground. Hit it
//!   from behind or from above; it needs a moment to swing its guard around
//!   when the chameleon jumps over it. Off guard (unaware), it can be hit.
//! - **Rubble hermit** (veteran): a hermit crab wearing a broken lump of the
//!   dam, sitting among the rubble like any other chunk. Walk right up to it
//!   (or slash it) and it bursts out with an overhead claw slam. Its shell
//!   shrugs off everything from behind or above: only its soft front, out of
//!   the shell, can be hurt, and every wound sends it back in for a moment,
//!   to come out slamming.
//!
//! Minds live on `Mob::dam`; bodies in `mob_art/dam.rs`.

use bevy::prelude::*;

use super::{hop, walk, Mob, MobKind, MobSpec, Moves, Notice, Shock, State, GRAVITY};
use crate::{
    gore::Fluid,
    level::Platform,
    player::Player,
    terrain::{Terrain, PX},
};

// --- Louse ---

/// It lets go when the chameleon is this close (x, with a little lead on its run) below it.
const DROP_RANGE: f32 = 55.0;
const DROP_LEAD: f32 = 0.2;
/// A ball bounces off walls this many times before it unrolls.
const BOUNCES: u32 = 2;
/// Ball radius in its drawing units (for the spin).
const BALL: f32 = 6.0;

// --- Leech ---

/// Body length (drawing units), contracted and stretched: one inch forward per cycle.
pub(crate) const INCH_MIN: f32 = 14.0;
pub(crate) const INCH_MAX: f32 = 26.0;
/// It rears up to lunge from this close (x), and its lunge takes about this long.
const LUNGE_RANGE: f32 = 120.0;
const LUNGE_TIME: f32 = 0.42;
/// Latched on: seconds until it's full (and bites), and the walk speed it leaves the chameleon.
const DRAIN: f32 = 2.6;
const LATCH_SPEED: f32 = 110.0;
/// A gorged leech crawls this much slower (and never attacks again).
const GORGED_PACE: f32 = 0.5;

// --- Crab ---

/// Seconds it takes to swing its guard to the other side.
const GUARD_TURN: f32 = 0.4;
/// It sizes the chameleon up from this far (x), snaps from this close.
const KEEP: f32 = 115.0;
const SNAP_RANGE: f32 = 85.0;
/// The claw tip hurts within this of the chameleon.
const CLAW_REACH: f32 = 30.0;
/// Footwork: a beat lasts this long, moving all but the last `STILL` of it.
const BEAT: (f32, f32) = (0.35, 0.75);
const STILL: f32 = 0.2;

// --- Hermit ---

/// It bursts out of hiding when the chameleon comes this close.
const AMBUSH: f32 = 120.0;
/// It slams from this close (x); the slam lands this far in front, this wide (half).
const SLAM_RANGE: f32 = 95.0;
const SLAM_AT: f32 = 50.0;
const SLAM_HALF: f32 = 44.0;
/// The slam lands this far into the attack.
const SLAM_HIT: f32 = 0.1;
/// Tucked into its shell this long after a wound.
const RETRACT: f32 = 1.2;
/// Out of sight this long, it goes home and pretends to be rubble again.
const HIDE_AFTER: f32 = 3.5;

/// A blow from above (slashing downward at it) gets past a guard.
const FROM_ABOVE: f32 = -0.55;

pub(super) static LOUSE: MobSpec = MobSpec {
    moves: Moves::Cling,
    hp: 1,
    scale: 1.4,
    radius: 12.0,
    foot: 5.0 * 1.4,
    blood: Fluid::Indigo,
    speed: 45.0,
    sense: 340.0,
    windup: 0.4,
    // Rolling.
    attack_speed: 320.0,
    attack_time: 1.4,
    recover: 0.9,
    leash: 260.0,
    damage: 1,
    stubborn: false,
    initiative: 0.9,
    veteran: false,
    // 1 hp, a drop and a roll.
    food: 0.25,
};
pub(super) static LEECH: MobSpec = MobSpec {
    moves: Moves::Inch,
    hp: 2,
    scale: 1.5,
    radius: 13.0,
    foot: 3.0 * 1.5,
    // Full of someone else's.
    blood: Fluid::Red,
    speed: 40.0,
    sense: 320.0,
    windup: 0.55,
    attack_speed: 430.0,
    attack_time: 1.0,
    recover: 1.1,
    leash: 240.0,
    damage: 1,
    stubborn: false,
    initiative: 1.2,
    veteran: false,
    // 2 hp, slow, but it rides you.
    food: 0.5,
};
pub(super) static CRAB: MobSpec = MobSpec {
    moves: Moves::Scuttle,
    hp: 3,
    scale: 1.6,
    radius: 18.0,
    foot: 7.0 * 1.6,
    blood: Fluid::Indigo,
    speed: 60.0,
    sense: 380.0,
    windup: 0.45,
    attack_speed: 380.0,
    attack_time: 0.22,
    recover: 0.9,
    leash: 300.0,
    damage: 1,
    stubborn: false,
    initiative: 0.8,
    veteran: false,
    // 3 hp behind a guard.
    food: 0.75,
};
pub(super) static HERMIT: MobSpec = MobSpec {
    moves: Moves::Shell,
    hp: 5,
    scale: 2.0,
    radius: 22.0,
    foot: 7.0 * 2.0,
    blood: Fluid::Indigo,
    speed: 32.0,
    sense: 420.0,
    windup: 0.65,
    attack_speed: 0.0,
    attack_time: 0.3,
    recover: 1.1,
    leash: 220.0,
    damage: 1,
    stubborn: true,
    initiative: 0.3,
    veteran: true,
    // 5 hp, hides, armored from three sides.
    food: 1.5,
};

/// The broken dam's mobs (`Style::Dam`), whoever owns the region: the
/// crab's kin and what lives off the drowned. Carrion flies swarm over the
/// reservoir's dead too.
pub(super) fn menu() -> Vec<(MobKind, f32)> {
    vec![
        (MobKind::GalleryLouse, 0.3),
        (MobKind::SeepLeech, 0.2),
        (MobKind::SpillwayCrab, 0.22),
        (MobKind::RubbleHermit, 0.13),
        (MobKind::Fly, 0.15),
    ]
}

/// Whether these moves are run here.
pub(super) fn runs(moves: Moves) -> bool {
    matches!(moves, Moves::Cling | Moves::Inch | Moves::Scuttle | Moves::Shell)
}

/// What a dam mob has in mind (kept on its `Mob`).
#[derive(Default)]
pub struct Mind {
    /// First frame done.
    ready: bool,
    /// Louse: still on its ceiling.
    pub(crate) clinging: bool,
    /// Louse: 0 = stretched out, 1 = rolled into a ball; and how far the ball has turned.
    pub(crate) curl: f32,
    pub(crate) roll: f32,
    bounces: u32,
    /// Leech: inching cycle (one inch per unit), and movement saved up into whole steps.
    pub(crate) inch: f32,
    carry: f32,
    /// Leech: latched on the chameleon at this offset (x × the chameleon's facing).
    pub(crate) latch: Option<Vec2>,
    /// Leech: 0..1 full of blood; gorged = full, done.
    pub(crate) swell: f32,
    pub(crate) gorged: bool,
    /// Crab: how long the chameleon has been on its unguarded side (swinging the guard around).
    pub(crate) turn: f32,
    /// Crab: footwork beat (time left, direction).
    beat: (f32, f32),
    /// Crab and hermit: seconds before it attacks again; this attack has hit.
    cool: f32,
    hit: bool,
    /// Hermit: pretending to be rubble.
    pub(crate) hidden: bool,
    /// Hermit: 0 = tucked in its shell, 1 = out.
    pub(crate) out: f32,
    /// Hermit: seconds left tucked in after a wound; seconds without seeing the chameleon.
    pub(crate) retract: f32,
    lost: f32,
}

/// What a dam mob sees of the world this frame.
pub struct World<'a, 'w, 's, 'd, 'e> {
    pub terrain: &'a Terrain,
    pub platforms: &'a Query<'w, 's, (&'d Platform, &'e Transform), (Without<Mob>, Without<Player>)>,
    pub p: &'a Player,
    pub ppos: Vec2,
    pub dt: f32,
}

/// What it did this frame that touches the outside world.
#[derive(Default)]
pub struct Step {
    /// It hurt the chameleon from here.
    pub bite: Option<Vec2>,
    /// It is riding the chameleon, which drags it along (`drag`).
    pub drag: bool,
    /// Screen shake from a heavy blow.
    pub shake: f32,
}

/// A latched leech weighs the chameleon down: its walk is capped.
pub(super) fn drag(p: &mut Player) {
    p.vel.x = p.vel.x.clamp(-LATCH_SPEED, LATCH_SPEED);
}

/// Knocked back by a blow that got through (slash or venom).
pub(super) fn knock(mind: &Mind, kind: MobKind, side: f32) -> Vec2 {
    match kind {
        // Too heavy to move; or still stuck to its ceiling.
        MobKind::RubbleHermit => Vec2::ZERO,
        MobKind::GalleryLouse if mind.clinging => Vec2::ZERO,
        MobKind::SeepLeech if mind.latch.is_some() => Vec2::new(side * 200.0, 240.0),
        _ => Vec2::new(side * 240.0, 0.0),
    }
}

impl Mob {
    /// A spillway crab holding its claw up in front: aware of the chameleon,
    /// not winding up, recovering or swinging its guard around.
    pub(crate) fn guarding(&self) -> bool {
        self.spec.moves == Moves::Scuttle
            && self.notice == Notice::Aware
            && matches!(self.state, State::Idle | State::Attack(_))
            && self.dam.turn == 0.0
    }
}

/// Whether a blow from the chameleon at `ppos` going `aim` glances off
/// (the crab's claw guard, the hermit's shell). A glance makes it flash, and
/// the crab answers at once; testing a lump of rubble wakes the hermit in it.
pub(super) fn glances(m: &mut Mob, pos: Vec2, ppos: Vec2, aim: Vec2) -> bool {
    let front = (ppos.x - pos.x) * m.facing > 0.0;
    let above = aim.y < FROM_ABOVE && ppos.y > pos.y + 10.0;
    let glance = match m.spec.moves {
        Moves::Scuttle => {
            let glance = m.guarding() && front && !above;
            if glance {
                // Shoved back a little, and it answers.
                m.vel.x = -m.facing * 150.0;
                m.dam.cool = 0.0;
            }
            glance
        }
        Moves::Shell if m.dam.hidden => {
            // Not rubble after all.
            m.dam.hidden = false;
            m.notice = Notice::Aware;
            m.facing = if ppos.x >= pos.x { 1.0 } else { -1.0 };
            m.plan = ppos;
            m.state = State::Windup(m.spec.windup * 0.5);
            true
        }
        Moves::Shell => m.dam.retract > 0.0 || !front || above,
        _ => false,
    };
    if glance {
        m.flash = 0.45;
    }
    glance
}

/// A spot for a louse near `p`: its center, upside down under a ceiling
/// long enough to crawl along, with a floor below to drop onto.
pub(super) fn ceiling_site(terrain: &Terrain, p: Vec2, rect: Rect, spec: &MobSpec) -> Option<Vec2> {
    let mut c = terrain.to_cell(p);
    for _ in 0..30 {
        c.y -= 1;
        if terrain.solid(c) {
            let ceiling = terrain.cell_min(c).y;
            let at = Vec2::new(p.x, ceiling - spec.foot);
            let along = (-3..=3).all(|i| {
                let x = p.x + i as f32 * PX;
                terrain.solid_at(Vec2::new(x, ceiling + PX * 0.5)) && !terrain.solid_at(Vec2::new(x, ceiling - PX * 1.5))
            });
            let room = terrain.clearance(at - Vec2::Y * 40.0) >= 2.5 * PX;
            let floor = terrain.surface(at, 600.0).is_some();
            return (rect.contains(at) && along && room && floor).then_some(at);
        }
    }
    None
}

pub(super) fn think(m: &mut Mob, tf: &mut Transform, sees: bool, touching: bool, w: &World) -> Step {
    let pos = tf.translation.truncate();
    let mut step = Step::default();
    let next = match m.spec.moves {
        Moves::Cling => louse(m, pos, sees, touching, w, &mut step),
        Moves::Inch => leech(m, pos, sees, touching, w, &mut step),
        Moves::Scuttle => crab(m, pos, sees, touching, w, &mut step),
        Moves::Shell => hermit(m, pos, sees, w, &mut step),
        _ => pos,
    };
    tf.translation.x = next.x;
    tf.translation.y = next.y;
    step
}

fn sign(x: f32) -> f32 {
    if x >= 0.0 { 1.0 } else { -1.0 }
}

fn shocked(m: &Mob) -> Option<Shock> {
    match m.notice {
        Notice::Shock { how, .. } => Some(how),
        _ => None,
    }
}

/// Patrol direction (0 = pausing), as walkers do.
fn wander(m: &mut Mob, pos: Vec2, dt: f32) -> f32 {
    m.wander.1 -= dt;
    if m.wander.1 <= 0.0 {
        let away = pos.x - m.home.x;
        m.wander = if m.rng.chance(0.35) {
            (0.0, m.rng.range(0.6, 1.6))
        } else if away.abs() > m.spec.leash * 0.6 {
            (-sign(away), m.rng.range(1.0, 2.5))
        } else {
            (if m.rng.chance(0.5) { 1.0 } else { -1.0 }, m.rng.range(1.0, 2.5))
        };
    }
    m.wander.0
}

/// Walk `vel.x` along the ground; returns the new center and whether it was blocked.
fn stroll(m: &Mob, w: &World, pos: Vec2) -> (Vec2, bool) {
    let feet = pos - Vec2::Y * m.spec.foot;
    let (feet, blocked) = walk(w.terrain, feet, m.vel.x * w.dt, m.home.x, m.spec);
    (feet + Vec2::Y * m.spec.foot, blocked)
}

// --- Gallery louse ---

fn louse(m: &mut Mob, pos: Vec2, sees: bool, touching: bool, w: &World, step: &mut Step) -> Vec2 {
    let spec = m.spec;
    let dt = w.dt;
    let ease = |rate: f32| 1.0 - (-rate * dt).exp();
    if !m.dam.ready {
        m.dam.ready = true;
        m.dam.clinging = w.terrain.solid_at(pos + Vec2::Y * (spec.foot + 4.0));
        m.airborne = !m.dam.clinging && w.terrain.surface(pos - Vec2::Y * spec.foot, 2.0 * PX).is_none();
    }
    let to = w.ppos - pos;
    let side = sign(to.x);
    let alive = w.p.dead.is_none();
    // Rolled up while it falls, winds up on the floor and rolls.
    let balled = !m.dam.clinging
        && match m.state {
            State::Windup(_) | State::Attack(_) => true,
            _ => m.airborne,
        };
    let target = if balled { 1.0 } else { 0.0 };
    m.dam.curl += (target - m.dam.curl) * ease(if balled { 10.0 } else { 2.5 });

    if m.dam.clinging {
        let below = alive && sees && to.y < -20.0 && (to.x + w.p.vel.x * DROP_LEAD).abs() < DROP_RANGE;
        m.state = match m.state {
            State::Idle if below => {
                m.facing = side;
                State::Windup(0.0)
            }
            State::Idle => {
                if shocked(m).is_none() {
                    // Crawling along the ceiling, turning back where it ends.
                    let dir = wander(m, pos, dt);
                    if dir != 0.0 {
                        let nx = pos.x + dir * spec.speed * dt;
                        let front = nx + dir * spec.radius * 0.6;
                        let roof = pos.y + spec.foot + 4.0;
                        let holds = w.terrain.solid_at(Vec2::new(front, roof)) && !w.terrain.solid_at(Vec2::new(front, pos.y));
                        if holds && (nx - m.home.x).abs() < spec.leash {
                            m.facing = dir;
                            m.stride += dt * spec.speed * 0.12;
                            return Vec2::new(nx, pos.y);
                        }
                        m.wander = (-dir, m.rng.range(1.0, 2.0));
                    }
                }
                State::Idle
            }
            State::Windup(s) if s < spec.windup => State::Windup(s + dt),
            // Let go, drifting a little toward the chameleon.
            State::Windup(_) | State::Hurt(_) => {
                m.dam.clinging = false;
                m.airborne = true;
                m.dam.hit = false;
                m.vel = Vec2::new((to.x * 2.0).clamp(-120.0, 120.0), 0.0);
                m.home.x = pos.x;
                State::Attack(0.0)
            }
            other => other,
        };
        return pos;
    }

    if m.airborne {
        // Falling: hurts whatever it lands on.
        if touching && alive && !m.dam.hit && matches!(m.state, State::Attack(_)) {
            step.bite = Some(pos);
            m.dam.hit = true;
        }
        m.dam.roll -= dt * 9.0;
        let next = hop(m, w.terrain, w.platforms, pos, dt);
        if !m.airborne {
            // Landed: its ground is here now. A moment dizzy, then it unrolls.
            m.home.x = next.x;
            if matches!(m.state, State::Attack(_)) {
                m.state = State::Recover(0.0);
            }
        }
        return next;
    }

    let near = sees && to.y.abs() < 90.0;
    m.dam.cool -= dt;
    m.state = match m.state {
        State::Hurt(s) => {
            m.vel.x *= 1.0 - 6.0 * dt;
            if s < 0.3 { State::Hurt(s + dt) } else { State::Recover(spec.recover * 0.5) }
        }
        State::Idle if near && m.dam.cool <= 0.0 => {
            m.facing = side;
            m.plan = w.ppos;
            m.vel.x = 0.0;
            State::Windup(0.0)
        }
        State::Idle => {
            let dir = match shocked(m) {
                Some(Shock::Freeze) => 0.0,
                Some(_) => -side,
                None => wander(m, pos, dt),
            };
            m.vel.x += (dir * spec.speed - m.vel.x) * ease(8.0);
            if m.vel.x.abs() > 5.0 {
                m.facing = sign(m.vel.x);
            }
            State::Idle
        }
        State::Windup(s) if s < spec.windup => State::Windup(s + dt),
        State::Windup(_) => {
            // Rolls where it saw the chameleon when it curled up.
            m.facing = sign(m.plan.x - pos.x);
            m.vel.x = m.facing * spec.attack_speed;
            m.dam.bounces = 0;
            m.dam.hit = false;
            State::Attack(0.0)
        }
        State::Attack(_) if touching && alive && !m.dam.hit => {
            step.bite = Some(pos);
            m.dam.hit = true;
            m.vel.x *= -0.4;
            State::Recover(0.0)
        }
        State::Attack(s) if s > spec.attack_time => State::Recover(0.0),
        State::Attack(s) => State::Attack(s + dt),
        State::Recover(s) => {
            m.vel.x *= 1.0 - 4.0 * dt;
            if s < spec.recover {
                State::Recover(s + dt)
            } else {
                m.dam.cool = m.rng.range(0.6, 1.4);
                State::Idle
            }
        }
        dead @ State::Dead { .. } => dead,
    };
    let (next, blocked) = stroll(m, w, pos);
    if blocked {
        match m.state {
            // Ricochets off walls (and its leash), then gives up.
            State::Attack(_) if m.dam.bounces < BOUNCES => {
                m.dam.bounces += 1;
                m.vel.x *= -0.8;
                m.facing = sign(m.vel.x);
                step.shake = 0.05;
            }
            State::Attack(_) => {
                m.vel.x = 0.0;
                m.state = State::Recover(0.0);
            }
            State::Idle => {
                m.vel.x = 0.0;
                m.wander = (-m.wander.0, m.rng.range(1.0, 2.0));
            }
            _ => m.vel.x = 0.0,
        }
    }
    let moved = (next.x - pos.x).abs();
    if m.dam.curl > 0.5 {
        m.dam.roll -= moved / (BALL * spec.scale);
    } else {
        m.stride += moved * 0.12;
    }
    next
}

// --- Seep leech ---

/// 0..1 how stretched out the leech is at this point of its inching cycle.
pub(crate) fn stretch(inch: f32) -> f32 {
    0.5 - 0.5 * (inch * std::f32::consts::TAU).cos()
}

fn leech(m: &mut Mob, pos: Vec2, sees: bool, touching: bool, w: &World, step: &mut Step) -> Vec2 {
    let spec = m.spec;
    let dt = w.dt;
    let p = w.p;
    if !m.dam.ready {
        m.dam.ready = true;
        m.dam.inch = m.rng.next();
    }
    let to = w.ppos - pos;
    let side = sign(to.x);
    let alive = p.dead.is_none();

    if let Some(local) = m.dam.latch {
        let hurt = matches!(m.state, State::Hurt(_));
        if hurt || !alive || p.dash > 0.0 || p.god.on() {
            // Torn off (by a blow, `knock` threw it) or flung off by a dash.
            m.dam.latch = None;
            m.airborne = true;
            if !hurt {
                m.vel = Vec2::new(-side * 220.0, 260.0);
                m.state = State::Recover(0.0);
            }
            m.home.x = pos.x;
        } else {
            m.dam.swell = (m.dam.swell + dt / DRAIN).min(1.0);
            step.drag = true;
            let at = w.ppos + Vec2::new(local.x * p.facing, local.y);
            // Its mouth on the chameleon.
            m.facing = sign(w.ppos.x - at.x);
            if m.dam.swell >= 1.0 {
                // Full: a last bite, and it drops off, sated.
                step.bite = Some(at);
                m.dam.latch = None;
                m.dam.gorged = true;
                m.airborne = true;
                m.vel = Vec2::new(-m.facing * 120.0, 160.0);
                m.home.x = at.x;
                m.state = State::Recover(-1.5);
            }
            return at;
        }
    }

    let near = sees && to.y.abs() < 110.0;
    let mut want = 0.0;
    m.state = match m.state {
        State::Hurt(s) if s < 0.3 => State::Hurt(s + dt),
        State::Hurt(_) => State::Recover(spec.recover * 0.5),
        State::Idle if near && !m.dam.gorged && !m.airborne && to.x.abs() < LUNGE_RANGE => {
            m.facing = side;
            m.plan = w.ppos;
            State::Windup(0.0)
        }
        State::Idle => {
            want = match shocked(m) {
                Some(Shock::Freeze) => 0.0,
                Some(_) => -side,
                // Gorged: away from it; hungry: toward it.
                None if near => if m.dam.gorged { -side } else { side },
                None => wander(m, pos, dt),
            };
            State::Idle
        }
        State::Windup(s) if s < spec.windup => {
            m.facing = side;
            State::Windup(s + dt)
        }
        State::Windup(_) => {
            // A lunge at where it reared up against (following only a little).
            let d = m.plan.lerp(w.ppos, 0.3) - pos;
            let vx = (d.x / LUNGE_TIME).clamp(-spec.attack_speed, spec.attack_speed);
            let vy = ((d.y + 0.5 * GRAVITY * LUNGE_TIME * LUNGE_TIME) / LUNGE_TIME).clamp(250.0, 600.0);
            m.vel = Vec2::new(vx, vy);
            m.airborne = true;
            State::Attack(0.0)
        }
        // Landed without catching anything: lies stretched out a moment.
        State::Attack(_) if !m.airborne => State::Recover(0.0),
        State::Attack(s) if s > spec.attack_time => State::Recover(0.0),
        State::Attack(s) => State::Attack(s + dt),
        State::Recover(s) => {
            if s < spec.recover {
                State::Recover(s + dt)
            } else {
                State::Idle
            }
        }
        dead @ State::Dead { .. } => dead,
    };

    if m.airborne {
        if matches!(m.state, State::Attack(_)) && touching && alive && !p.god.on() && p.dash <= 0.0 {
            // Got it: holds on to its flank.
            let x = ((pos.x - w.ppos.x) * p.facing).clamp(-14.0, 14.0);
            m.dam.latch = Some(Vec2::new(if x.abs() < 8.0 { 8.0 * sign(x) } else { x }, 3.0));
            m.dam.swell = 0.0;
            m.vel = Vec2::ZERO;
            m.airborne = false;
            return pos;
        }
        return hop(m, w.terrain, w.platforms, pos, dt);
    }
    if want == 0.0 {
        return pos;
    }
    // Inching: the body stretches and contracts, the end on the ground stays
    // put, so the center moves half the change in length.
    let pace = if m.dam.gorged { GORGED_PACE } else { 1.0 };
    let span = (INCH_MAX - INCH_MIN) * spec.scale;
    let before = stretch(m.dam.inch);
    m.dam.inch = (m.dam.inch + spec.speed * pace / span * dt).fract();
    let dx = 0.5 * (stretch(m.dam.inch) - before).abs() * span * want;
    m.facing = want;
    // Whole steps only (tiny ones catch on the edges of pixel steps).
    m.dam.carry += dx;
    if m.dam.carry.abs() < 3.0 {
        return pos;
    }
    let feet = pos - Vec2::Y * spec.foot;
    let (feet, blocked) = walk(w.terrain, feet, std::mem::take(&mut m.dam.carry), m.home.x, spec);
    if blocked {
        m.wander = (-want, m.rng.range(1.0, 2.0));
    }
    feet + Vec2::Y * spec.foot
}

// --- Spillway crab ---

fn crab(m: &mut Mob, pos: Vec2, sees: bool, touching: bool, w: &World, step: &mut Step) -> Vec2 {
    let spec = m.spec;
    let dt = w.dt;
    let ease = |rate: f32| 1.0 - (-rate * dt).exp();
    let to = w.ppos - pos;
    let side = sign(to.x);
    let alive = w.p.dead.is_none();
    let near = sees && to.y.abs() < 100.0;
    let busy = matches!(m.state, State::Windup(_) | State::Attack(_));
    // The guard swings around slowly when the chameleon gets past it.
    if near && side != m.facing && !busy {
        m.dam.turn += dt;
        if m.dam.turn >= GUARD_TURN {
            m.facing = side;
            m.dam.turn = 0.0;
        }
    } else {
        m.dam.turn = 0.0;
    }
    m.dam.cool -= dt;

    let mut want = None;
    m.state = match m.state {
        State::Hurt(s) => {
            m.vel.x *= 1.0 - 6.0 * dt;
            if s < 0.3 { State::Hurt(s + dt) } else { State::Recover(spec.recover * 0.5) }
        }
        State::Idle if near && m.dam.cool <= 0.0 && side == m.facing && to.x.abs() < SNAP_RANGE => {
            m.plan = w.ppos;
            State::Windup(0.0)
        }
        State::Idle => {
            let v = match shocked(m) {
                Some(Shock::Freeze) => 0.0,
                Some(_) => -side * spec.speed * 1.5,
                None if near => {
                    // Footwork: bursts in and out at boxing distance.
                    m.dam.beat.0 -= dt;
                    if m.dam.beat.0 <= 0.0 {
                        let gap = to.x.abs();
                        m.dam.beat = (
                            m.rng.range(BEAT.0, BEAT.1),
                            if gap > KEEP + 25.0 {
                                side
                            } else if gap < KEEP - 35.0 {
                                -side
                            } else {
                                m.rng.below(3) as f32 - 1.0
                            },
                        );
                    }
                    if m.dam.beat.0 > STILL { m.dam.beat.1 * spec.speed * 2.2 } else { 0.0 }
                }
                // Scuttling sideways about its spot: the guard stays where it is.
                None => wander(m, pos, dt) * spec.speed,
            };
            want = Some(v);
            State::Idle
        }
        State::Windup(s) if s < spec.windup => {
            // Draws back, claw high and open.
            want = Some(-m.facing * 15.0);
            State::Windup(s + dt)
        }
        State::Windup(_) => {
            m.facing = sign(m.plan.x - pos.x);
            m.vel.x = m.facing * spec.attack_speed;
            m.dam.hit = false;
            State::Attack(0.0)
        }
        State::Attack(s) => {
            let tip = pos + Vec2::new(m.facing * spec.radius * 1.3, 2.0);
            if !m.dam.hit && alive && (touching || tip.distance(w.ppos) < CLAW_REACH) {
                step.bite = Some(pos);
                m.dam.hit = true;
            }
            if s > spec.attack_time { State::Recover(0.0) } else { State::Attack(s + dt) }
        }
        State::Recover(s) => {
            // The claw jammed in the ground: open.
            m.vel.x *= 1.0 - 7.0 * dt;
            if s < spec.recover {
                State::Recover(s + dt)
            } else {
                m.dam.cool = m.rng.range(0.8, 1.6);
                State::Idle
            }
        }
        dead @ State::Dead { .. } => dead,
    };
    if let Some(v) = want {
        m.vel.x += (v - m.vel.x) * ease(14.0);
    }
    let (next, blocked) = stroll(m, w, pos);
    if blocked {
        match m.state {
            State::Attack(_) => {
                m.vel.x = 0.0;
                m.state = State::Recover(0.0);
            }
            State::Idle => {
                m.vel.x = 0.0;
                m.wander = (-m.wander.0, m.rng.range(1.0, 2.0));
                m.dam.beat.0 = 0.0;
            }
            _ => m.vel.x = 0.0,
        }
    }
    m.stride += (next.x - pos.x).abs() * 0.12;
    next
}

// --- Rubble hermit ---

fn hermit(m: &mut Mob, pos: Vec2, sees: bool, w: &World, step: &mut Step) -> Vec2 {
    let spec = m.spec;
    let dt = w.dt;
    let ease = |rate: f32| 1.0 - (-rate * dt).exp();
    if !m.dam.ready {
        m.dam.ready = true;
        m.dam.hidden = true;
    }
    let to = w.ppos - pos;
    let side = sign(to.x);
    let alive = w.p.dead.is_none();
    let tucked = m.dam.hidden || m.dam.retract > 0.0;
    m.dam.out += (if tucked { 0.0 } else { 1.0 } - m.dam.out) * ease(if tucked { 14.0 } else { 5.0 });

    if m.dam.hidden {
        // Rubble, until something walks right up to it.
        let close = alive && !w.p.god.on() && to.length() < AMBUSH && to.y.abs() < 80.0;
        if close && w.terrain.raycast(pos + Vec2::Y * 4.0, w.ppos).is_none() {
            m.dam.hidden = false;
            m.notice = Notice::Aware;
            m.facing = side;
            m.plan = w.ppos;
            m.state = State::Windup(spec.windup * 0.5);
        } else {
            m.state = State::Idle;
            m.vel = Vec2::ZERO;
        }
        return pos;
    }
    m.dam.retract = (m.dam.retract - dt).max(0.0);
    let near = sees && to.y.abs() < 120.0;
    let mut want = 0.0;
    m.state = match m.state {
        // A wound: back into the shell.
        State::Hurt(_) => {
            m.dam.retract = RETRACT;
            State::Idle
        }
        State::Idle if m.dam.retract > 0.0 => {
            // Comes out slamming if the chameleon is still at it.
            if m.dam.retract <= dt && near && to.x.abs() < SLAM_RANGE + 20.0 {
                m.facing = side;
                m.plan = w.ppos;
                State::Windup(spec.windup * 0.6)
            } else {
                State::Idle
            }
        }
        State::Idle if near && to.x.abs() < SLAM_RANGE => {
            m.facing = side;
            m.plan = w.ppos;
            State::Windup(0.0)
        }
        State::Idle => {
            if near {
                m.dam.lost = 0.0;
                m.facing = side;
                want = side;
            } else {
                // Back home, to sit there as rubble again.
                m.dam.lost += dt;
                let back = m.home.x - pos.x;
                if back.abs() > 6.0 {
                    want = sign(back);
                    m.facing = want;
                } else if m.dam.lost > HIDE_AFTER {
                    m.dam.hidden = true;
                    m.notice = Notice::Unaware;
                }
            }
            State::Idle
        }
        State::Windup(s) if s < spec.windup => State::Windup(s + dt),
        State::Windup(_) => {
            m.dam.hit = false;
            State::Attack(0.0)
        }
        State::Attack(s) => {
            if s >= SLAM_HIT && !m.dam.hit {
                // The claw comes down in front of it.
                m.dam.hit = true;
                step.shake = 0.25;
                let dx = w.ppos.x - (pos.x + m.facing * SLAM_AT);
                let dy = w.ppos.y - pos.y;
                if alive && dx.abs() < SLAM_HALF && (-50.0..70.0).contains(&dy) {
                    step.bite = Some(pos);
                }
            }
            if s > spec.attack_time { State::Recover(0.0) } else { State::Attack(s + dt) }
        }
        State::Recover(s) => {
            if s < spec.recover {
                State::Recover(s + dt)
            } else {
                State::Idle
            }
        }
        dead @ State::Dead { .. } => dead,
    };
    m.vel.x += (want * spec.speed - m.vel.x) * ease(8.0);
    let (next, blocked) = stroll(m, w, pos);
    if blocked {
        m.vel.x = 0.0;
    }
    m.stride += (next.x - pos.x).abs() * 0.12;
    next
}
