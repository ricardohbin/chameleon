//! Rats: the gutter's pack vermin, suspicious and cowardly until cornered.
//!
//! Every frame a rat reads how much the chameleon is paying attention to it
//! (`attention`, 0..1: the body facing it counts most, the head looking at it
//! less; less again while the chameleon is busy slashing elsewhere or eating,
//! in the air, or stunned; coming straight at the rat is hard to ignore). Then:
//! - **Lurking**: it picks a spot on its walkable ground (`Mind::floor`) at a
//!   distance that shrinks when it's ignored (striking range) and grows when
//!   watched; dark spots (interiors) and spots behind the chameleon are
//!   better. Out of sight, it hides in the dark near home.
//! - **Sneaking**: in, stop-and-go, with sniffing pauses; away, scurrying.
//!   When the chameleon suddenly turns to it, it freezes, then backs off.
//!   It keeps an eye on the chameleon unless it's running away.
//! - **Hit and run**: close and ignored, it may dart in, bite, and bolt. A
//!   rush breaks off when the chameleon turns to face it.
//! - **Cornered** (no room to flee and the chameleon close): it attacks even
//!   when watched; stared at, it rears up first (`MobSpec::windup`), then
//!   fights without breaking off.

use bevy::prelude::*;

use super::{walk, Mob, State};
use crate::{chameleon::ChameleonAnim, player::Player, sketch::Cell, terrain::Terrain};

/// Attention: how much the body facing the rat counts, and the head looking at it.
const BODY: f32 = 0.7;
const HEAD: f32 = 0.3;
/// Attention kept while the chameleon slashes elsewhere or eats, and while it's in the air.
const BUSY: f32 = 0.35;
const AIRBORNE: f32 = 0.7;
/// Lurking distance from the chameleon: ignored → watched (plus up to
/// ±`SPREAD` per rat, so a pack doesn't stack up).
const SNEAK_DIST: f32 = 80.0;
const WARY_DIST: f32 = 300.0;
const SPREAD: f32 = 30.0;
/// Lurk spot scores: a dark spot, and the penalty for standing in front of a watching chameleon.
const DARK: f32 = 0.6;
const EXPOSED: f32 = 0.6;
/// Speeds (× `attack_speed`): sneaking in, scurrying away, bolting after a bite.
const SNEAK: f32 = 0.3;
const SCURRY: f32 = 0.65;
const BOLT: f32 = 0.95;
/// Sniffing pauses while sneaking (per second), and the freeze when caught.
const SNIFF: f32 = 0.7;
const FREEZE: f32 = 0.35;
/// Attention jumping by this much in a frame startles it.
const STARTLE: f32 = 0.35;
/// It may strike from this close, at `AMBUSH` per second when fully ignored
/// (× (1 − attention)²), plus `DESPERATE` per second when cornered.
const STRIKE: f32 = 170.0;
const AMBUSH: f32 = 2.5;
const DESPERATE: f32 = 1.6;
/// Rushes break off at `NERVE` × attention² per second (not when cornered).
const NERVE: f32 = 7.0;
/// Cornered: less than `CORNER_ROOM` to flee, with the chameleon closer than `CORNER_DIST`.
const CORNER_ROOM: f32 = 60.0;
const CORNER_DIST: f32 = 180.0;
/// The smallest step it takes (see `Mind::stride`).
const MIN_STEP: f32 = 4.0;
/// Its ground is mapped in steps of this much (and at most `leash` each way).
const FLOOR_STEP: f32 = 12.0;

/// What a rat has in mind (kept on its `Mob`).
#[derive(Default)]
pub struct Mind {
    /// Its walkable ground, left to right (feet positions); mapped on the first frame.
    floor: Vec<Vec2>,
    /// Where it's lurking (x), and when it thinks again about where.
    lurk: Option<f32>,
    replan: f32,
    /// Frozen (sniffing, startled) for this long.
    pause: f32,
    /// Last frame's attention, to notice the chameleon turning to it.
    attention: f32,
    /// Attacking cornered: no breaking off.
    fury: bool,
    /// Movement saved up until it makes a whole step.
    carry: f32,
}

impl Mind {
    /// Bumped into something: think again about where to go.
    pub fn blocked(&mut self) {
        self.replan = 0.0;
        self.carry = 0.0;
    }

    /// Moves in steps of at least `MIN_STEP`: creeping a fraction of a unit
    /// per frame, the body box would catch on the edge of a pixel step.
    pub fn stride(&mut self, dx: f32) -> f32 {
        self.carry += dx;
        if self.carry.abs() < MIN_STEP {
            return 0.0;
        }
        std::mem::take(&mut self.carry)
    }
}

/// What a rat sees of the world this frame.
pub struct World<'a> {
    pub terrain: &'a Terrain,
    pub p: &'a Player,
    pub anim: &'a ChameleonAnim,
    pub ppos: Vec2,
    pub t: f32,
    pub dt: f32,
}

/// What it does this frame: the new state is already set on the mob.
pub struct Step {
    /// Horizontal speed wanted, × `attack_speed`.
    pub want: f32,
    /// Which way to face, when not simply where it moves.
    pub facing: Option<f32>,
    /// It bit the chameleon.
    pub bite: bool,
}

pub fn think(m: &mut Mob, pos: Vec2, sees: bool, touching: bool, w: &World) -> Step {
    let spec = m.spec;
    let dt = w.dt;
    let ease = |rate: f32| 1.0 - (-rate * dt).exp();
    let feet = pos - Vec2::Y * spec.foot;
    if m.rat.floor.is_empty() {
        m.rat.floor = map_floor(w.terrain, feet, m);
        // Its range is centered on the ground it can walk, not where it was put.
        let (lo, hi) = (m.rat.floor[0].x, m.rat.floor[m.rat.floor.len() - 1].x);
        m.home.x = (lo + hi) / 2.0;
    }

    let to_player = w.ppos - pos;
    let dist = to_player.length();
    let side = if to_player.x >= 0.0 { 1.0 } else { -1.0 };
    let a = if sees { attention(pos, w) } else { 0.0 };
    let startled = a - m.rat.attention > STARTLE && dist < WARY_DIST;
    m.rat.attention = a;
    let cornered = sees && dist < CORNER_DIST && walk(w.terrain, feet, -side * CORNER_ROOM, m.home.x, spec).1;

    let mut bite = false;
    let (state, want, facing) = match m.state {
        State::Hurt(s) if s < 0.3 => (State::Hurt(s + dt), 0.0, None),
        State::Hurt(_) => (State::Recover(0.0), -side, None),

        // Cornered and stared at: a warning, then it fights.
        State::Windup(s) if s < spec.windup => (State::Windup(s + dt), 0.0, Some(side)),
        State::Windup(_) => {
            m.rat.fury = true;
            (State::Attack(0.0), side, Some(side))
        }

        State::Attack(_) if touching => {
            bite = true;
            m.rat.fury = false;
            (State::Recover(0.0), -side, None)
        }
        State::Attack(s) if s > spec.attack_time => {
            m.rat.fury = false;
            (State::Recover(0.0), -side, None)
        }
        // Caught in the act: it breaks off (unless it has nowhere to go).
        State::Attack(s) if !m.rat.fury && s > 0.12 && m.rng.chance(ease(NERVE * a * a)) => (State::Recover(0.0), -side, None),
        State::Attack(s) => (State::Attack(s + dt), side, Some(side)),

        // Hit and run: bolt, slowing down, and turn to watch once at a safe distance.
        State::Recover(s) if s < spec.recover => {
            let k = 1.0 - s / spec.recover;
            let look = (s > spec.recover * 0.5 && sees).then_some(side);
            (State::Recover(s + dt), -side * BOLT * k.powf(1.5), look)
        }
        State::Recover(_) => {
            m.rat.replan = 0.0;
            (State::Idle, 0.0, None)
        }

        _ => {
            // Strike?
            let dare = AMBUSH * (1.0 - a).powi(2) + if cornered { DESPERATE } else { 0.0 };
            if sees && dist < STRIKE && m.rat.pause <= 0.0 && m.rng.chance(ease(dare)) {
                if cornered && a > 0.5 {
                    (State::Windup(0.0), 0.0, Some(side))
                } else {
                    (State::Attack(0.0), side, Some(side))
                }
            } else {
                lurk(m, pos, sees, startled, side, a, w)
            }
        }
    };
    m.state = state;
    Step { want, facing, bite }
}

/// Idle: go to (or toward) a good spot, warily.
fn lurk(m: &mut Mob, pos: Vec2, sees: bool, startled: bool, side: f32, a: f32, w: &World) -> (State, f32, Option<f32>) {
    let dt = w.dt;
    let ease = |rate: f32| 1.0 - (-rate * dt).exp();
    if startled {
        m.rat.pause = FREEZE;
        m.rat.replan = FREEZE;
    }
    m.rat.replan -= dt;
    if m.rat.replan <= 0.0 || m.rat.lurk.is_none() {
        m.rat.lurk = Some(pick_spot(m, pos, sees, a, w));
        m.rat.replan = if sees { m.rng.range(0.4, 0.9) } else { m.rng.range(2.0, 4.0) };
    }
    let watch = sees.then_some(side);
    if m.rat.pause > 0.0 {
        m.rat.pause -= dt;
        return (State::Idle, 0.0, watch.or(Some(m.facing)));
    }
    let dx = m.rat.lurk.unwrap_or(pos.x) - pos.x;
    if dx.abs() < 8.0 {
        return (State::Idle, 0.0, watch);
    }
    let dir = dx.signum();
    let closer = sees && dir == side;
    if closer {
        // Sneaking in: stop-and-go, sniffing now and then.
        if m.rng.chance(ease(SNIFF)) {
            m.rat.pause = m.rng.range(0.2, 0.6);
        }
        let go = if (w.t * 2.6 + m.seed).fract() < 0.6 { 1.0 } else { 0.0 };
        (State::Idle, dir * SNEAK * go, watch)
    } else if sees && dx.abs() < 60.0 {
        // A little further back: backs off, still watching.
        (State::Idle, dir * SNEAK, watch)
    } else {
        // Scurrying off (or about, unseen).
        let speed = if sees { SCURRY } else { SNEAK };
        (State::Idle, dir * speed, None)
    }
}

/// The best spot on its ground right now.
fn pick_spot(m: &mut Mob, pos: Vec2, sees: bool, a: f32, w: &World) -> f32 {
    let p = w.p;
    let spread = ((m.seed * 7.31).fract() * 2.0 - 1.0) * SPREAD;
    let near = SNEAK_DIST + (WARY_DIST - SNEAK_DIST) * a + spread;
    let mut best = (f32::MIN, pos.x);
    for q in m.rat.floor.iter().step_by(2) {
        let mut score = 0.0;
        if dark(w.terrain, *q) {
            score += DARK;
        }
        if sees {
            // Only on its own side: it won't slip past the chameleon to get there.
            if (q.x - w.ppos.x) * (pos.x - w.ppos.x) <= 0.0 || (q.x - w.ppos.x).abs() < SNEAK_DIST * 0.5 {
                continue;
            }
            score -= ((*q - w.ppos).length() - near).abs() / 120.0;
            // Out in front of a chameleon looking its way: exposed.
            if p.facing * (q.x - w.ppos.x) > 0.0 {
                score -= EXPOSED * a;
            }
            score -= (q.x - pos.x).abs() / 600.0;
        } else {
            // Unseen: somewhere dark, not far from home.
            score -= (q.x - m.home.x).abs() / 500.0 + (q.x - pos.x).abs() / 800.0;
        }
        // A little indecision.
        score += m.rng.range(0.0, 0.12);
        if score > best.0 {
            best = (score, q.x);
        }
    }
    best.1
}

/// How much the chameleon is paying attention to the rat at `pos`, 0..1.
fn attention(pos: Vec2, w: &World) -> f32 {
    let p = w.p;
    if p.dead.is_some() || p.stun > 0.0 {
        return 0.0;
    }
    let to_rat = (pos - crate::chameleon::neck(w.ppos, p.facing)).normalize_or_zero();
    let body = if p.facing * (pos.x - w.ppos.x) > 0.0 { 1.0 } else { 0.0 };
    let head = p.aim.dot(to_rat).max(0.0).powi(2);
    let mut a = BODY * body + HEAD * head;
    if w.anim.busy() && !(w.anim.slashing() && w.anim.slash_dir().dot(to_rat) > 0.5) {
        a *= BUSY;
    }
    if !p.grounded {
        a *= AIRBORNE;
    }
    // Coming straight at it: hard to ignore.
    let charge = (p.vel.x * (pos.x - w.ppos.x).signum() / 240.0).clamp(0.0, 1.0);
    a.max(charge * 0.8)
}

/// In the dark: an interior (a passable structure) behind it.
fn dark(terrain: &Terrain, feet: Vec2) -> bool {
    terrain.cell(terrain.to_cell(feet + Vec2::Y * 10.0)) == Cell::Back
}

/// Its walkable ground around `feet`: as far as it can walk each way, up to its leash.
fn map_floor(terrain: &Terrain, feet: Vec2, m: &Mob) -> Vec<Vec2> {
    let mut floor = vec![feet];
    for dir in [-1.0, 1.0] {
        let mut f = feet;
        for _ in 0..(m.spec.leash / FLOOR_STEP) as usize {
            let (next, blocked) = walk(terrain, f, dir * FLOOR_STEP, feet.x, m.spec);
            if blocked {
                break;
            }
            f = next;
            floor.push(f);
        }
    }
    floor.sort_by(|a, b| a.x.total_cmp(&b.x));
    floor
}
