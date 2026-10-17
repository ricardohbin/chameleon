//! The water treatment plant's vermin (`Style::Treatment`): the Razorwing's
//! brood, what it hunts, and what grew fat on the settling tanks' chemicals.
//!
//! - **Naiad** (`Moves::Mire`): a dragonfly nymph buried in the sludge caked
//!   on the floor, only its eyes showing. Aware, it creeps under the crust
//!   toward the chameleon (the moving mound is the tell), rears out of the
//!   floor (windup) and shoots its hinged lower jaw, the mask, out along the
//!   floor: a long, low reach (jump it, or stay out of it). Then it reels the
//!   mask back in slowly, out in the open: the window. Then it sinks again.
//!   Buried, claws and venom don't reach it.
//! - **Siphon** (`Moves::Siphon`): a plant mosquito, 1 hp. A wavering hover;
//!   aware, it sneaks round to the chameleon's back, hangs still (the tell)
//!   and lunges. Touching, it latches on: it weighs the walk down (`drag`)
//!   and drinks; full, it costs 1 hp and drops off bloated and sluggish.
//!   Slash it off (aim behind you) or shake it off with a dash or an air spin.
//! - **Sump leech** (`Moves::Inchworm`): loops along ceilings, inchworm
//!   fashion, to hang over the chameleon, swells and drips (the tell, the
//!   drips mark the spot), then lets go a drop of the pink caustic it gorged
//!   on. The drop burns, and so does the puddle it leaves for a few seconds
//!   (`acid`). Out of reach up there: jump at it, or spit.
//!
//! Their bodies are in `mob_art/treatment.rs`.

use bevy::prelude::*;

use super::{fly, walk, Deeds, Mob, MobKind, MobSpec, MobStuff, Moves, Notice, Shock, State};
use crate::{
    boss,
    camera::{self, HitStop, Shake},
    chameleon::ChameleonAnim,
    gore::{Bleed, Fluid},
    player::{self, Player},
    shapes::{ellipse_pts, fill, smooth_closed},
    terrain::{Terrain, PX, STEP_UP},
};

// --- Specs ---

/// Dragonfly nymph: buried ambusher with a shooting jaw.
pub(super) static NAIAD: MobSpec = MobSpec {
    moves: Moves::Mire,
    hp: 3,
    scale: 1.7,
    radius: 17.0,
    foot: 4.6 * 1.7,
    blood: Fluid::Pink,
    // Creeping under the crust.
    speed: 55.0,
    sense: 420.0,
    // Rearing out of the floor, mask quivering.
    windup: 0.55,
    attack_speed: 0.0,
    attack_time: JAW_OUT + JAW_HOLD,
    // Out of the floor, reeling the mask in: the window.
    recover: 1.3,
    leash: 220.0,
    damage: 1,
    stubborn: false,
    initiative: 0.5,
    // An ambush predator: it keeps its head.
    veteran: true,
    // 3 hp, unreachable buried, a long reach.
    food: 1.0,
};

/// Plant mosquito: latches on the chameleon's back and drinks.
pub(super) static SIPHON: MobSpec = MobSpec {
    moves: Moves::Siphon,
    hp: 1,
    scale: 1.5,
    radius: 10.0,
    foot: 4.0,
    // Full of someone's blood (soon the chameleon's).
    blood: Fluid::Red,
    speed: 70.0,
    sense: 380.0,
    windup: 0.45,
    attack_speed: 380.0,
    attack_time: 0.45,
    recover: 1.2,
    leash: 450.0,
    damage: 1,
    stubborn: false,
    initiative: 1.2,
    veteran: false,
    // 1 hp, a drain you must shake off.
    food: 0.25,
};

/// Ceiling leech gorged on the plant's chemicals: drops caustic.
pub(super) static SUMP_LEECH: MobSpec = MobSpec {
    moves: Moves::Inchworm,
    hp: 4,
    scale: 1.6,
    radius: 16.0,
    // Lying on the floor, dead.
    foot: 3.5 * 1.6,
    // It bleeds what it drank: the plant's pink.
    blood: Fluid::Pink,
    speed: 38.0,
    sense: 520.0,
    // Swelling, dripping: the tell.
    windup: 0.9,
    attack_speed: 0.0,
    attack_time: 0.0,
    recover: 2.2,
    leash: 260.0,
    damage: 1,
    // Clamped on with both suckers: hits don't stop the drop.
    stubborn: true,
    initiative: 1.0,
    veteran: true,
    // 4 hp, out of reach, makes the floor burn.
    food: 1.25,
};

/// The treatment plant's ecosystem: kinds and their share of the mobs.
/// - Naiads, the Razorwing's brood, ambush along the floors.
/// - Siphons: what the brood (and the boss) feed on, and what feeds on everyone.
/// - Sump leeches: the chemicals' own.
/// - Flies: carrion swarms over the sludge, prey of everything here.
/// - Spiders: web the pipe halls to catch the flies and siphons.
pub(super) fn menu() -> Vec<(MobKind, f32)> {
    vec![
        (MobKind::Siphon, 0.25),
        (MobKind::Naiad, 0.22),
        (MobKind::SumpLeech, 0.15),
        (MobKind::Fly, 0.23),
        (MobKind::Spider, 0.15),
    ]
}

// --- Tuning ---

/// Naiad: how far the mask shoots out (world units), how fast, and how long it stays out.
const JAW_REACH: f32 = 112.0;
const JAW_OUT: f32 = 0.1;
const JAW_HOLD: f32 = 0.14;
/// Share of the recover spent reeling the mask in.
const JAW_REEL: f32 = 0.6;
/// The mask catches the chameleon within this of its line.
const JAW_GRIP: f32 = player::HALF.y + 4.0;
/// Mouth height above the naiad's center, and how far in front.
const MOUTH: Vec2 = Vec2::new(14.0, -1.0);
/// It strikes when the chameleon is this close along the floor, and about level.
const STRIKE_X: f32 = JAW_REACH * 0.85;
const STRIKE_Y: f32 = 46.0;
/// Rising out of the floor and sinking back (share per second).
const RISE: f32 = 3.5;
const SINK: f32 = 1.6;
/// Below this much out of the floor it can't be hit.
const EXPOSED: f32 = 0.35;
/// Seconds between strikes.
const NAIAD_COOL: f32 = 0.8;

/// Siphon: where it goes for, relative to the chameleon (x behind its back).
const BACK: Vec2 = Vec2::new(-46.0, 44.0);
/// Close enough to its spot behind the chameleon to wind up.
const READY: f32 = 26.0;
/// Seconds of drinking before the bite costs a hit point.
const FEED: f32 = 2.0;
/// Gives up when the blood doesn't come (invulnerable chameleon) after this much more.
const FEED_GIVE_UP: f32 = 1.5;
/// How much each latched siphon weighs the walk down (as `Player::webbed`), and the cap.
const DRAG: f32 = 0.4;
const DRAG_MAX: f32 = 0.85;
/// Seconds to digest a full meal (it's sluggish meanwhile).
const DIGEST: f32 = 8.0;
/// Seconds between lunges.
const SIPHON_COOL: f32 = 1.4;

/// Leech: center below its ceiling, the steepest step it follows along it,
/// and how far up it looks for the ceiling.
pub(crate) const HANG: f32 = 5.0 * 1.6;
const CEILING_STEP: f32 = 2.0 * PX;
const CEILING_REACH: i32 = 6;
/// Over the chameleon within this (x), it drops.
const OVER: f32 = 22.0;
/// Seconds between drops.
const LEECH_COOL: f32 = 1.0;

/// Caustic drops: gravity, and the puddles they leave (half width, life, harmless fade at the end).
const DROP_GRAVITY: f32 = 1300.0;
const PUDDLE_HALF: f32 = 26.0;
const PUDDLE_LIFE: f32 = 4.2;
const PUDDLE_FADE: f32 = 0.8;
/// The chemical's color (on the color layer, like blood).
const ACID: Color = Color::srgba(0.86, 0.52, 0.74, 0.92);
const ACID_FOAM: Color = Color::srgba(0.97, 0.84, 0.92, 0.9);

/// What a treatment mob has in mind (kept on its `Mob`).
#[derive(Default)]
pub(crate) struct Mind {
    /// Naiad: 0 = buried, 1 = out of the floor.
    pub(crate) rise: f32,
    /// Naiad: how far the mask is out (world units).
    pub(crate) jaw: f32,
    /// Naiad: this strike already caught the chameleon.
    caught: bool,
    /// Siphon: latched on the chameleon at this offset (x along its facing).
    pub(crate) latch: Option<Vec2>,
    drink: f32,
    /// Siphon: 0..1, how full of blood.
    pub(crate) bloat: f32,
    /// Leech: 0..1, swelling before a drop.
    pub(crate) swell: f32,
    /// Leech: a drop to let go (picked up by `acid`).
    release: bool,
    /// Seconds before it may strike again.
    cool: f32,
}

impl Mob {
    /// Out of reach of claws and venom (a naiad under the floor).
    pub fn hidden(&self) -> bool {
        self.kind == MobKind::Naiad && self.treatment.rise < EXPOSED && !matches!(self.state, State::Dead { .. })
    }
}

/// Whether `think` hands this kind of mover to this module.
pub(super) fn runs(moves: Moves) -> bool {
    matches!(moves, Moves::Mire | Moves::Siphon | Moves::Inchworm)
}

/// What a treatment mob sees and touches of the world this frame.
pub(super) struct World<'a, 'w> {
    pub(super) terrain: &'a Terrain,
    pub(super) p: &'a mut Player,
    pub(super) anim: &'a ChameleonAnim,
    pub(super) ppos: Vec2,
    pub(super) t: f32,
    pub(super) dt: f32,
    pub(super) deeds: &'a mut Deeds,
    pub(super) bleed: &'a mut MessageWriter<'w, Bleed>,
}

/// One frame of a treatment mob (alive, not a nest): sets its state and moves it.
pub(super) fn think(m: &mut Mob, tf: &mut Transform, sees: bool, touching: bool, w: &mut World) {
    let pos = tf.translation.truncate();
    m.treatment.cool = (m.treatment.cool - w.dt).max(0.0);
    let next = match m.spec.moves {
        Moves::Mire => naiad(m, pos, sees, w),
        Moves::Siphon => siphon(m, pos, sees, touching, w),
        _ => leech(m, pos, sees, w),
    };
    tf.translation.x = next.x;
    tf.translation.y = next.y;
}

/// Running from the chameleon, caught by surprise (none of them freezes long).
fn startled(m: &Mob) -> Option<Shock> {
    match m.notice {
        Notice::Shock { how, .. } => Some(how),
        _ => None,
    }
}

// --- Naiad ---

fn naiad(m: &mut Mob, pos: Vec2, sees: bool, w: &mut World) -> Vec2 {
    let spec = m.spec;
    let dt = w.dt;
    let ease = |rate: f32| 1.0 - (-rate * dt).exp();
    let to = w.ppos - pos;
    let side = if to.x >= 0.0 { 1.0 } else { -1.0 };
    let shock = startled(m);
    let mind = &mut m.treatment;
    let mut want = 0.0;
    m.state = match m.state {
        State::Hurt(s) => {
            // Flinches, then dives back under and backs off.
            mind.jaw = (mind.jaw - JAW_REACH * 4.0 * dt).max(0.0);
            m.vel.x *= 1.0 - 6.0 * dt;
            if s < 0.3 {
                State::Hurt(s + dt)
            } else {
                mind.cool = NAIAD_COOL * 1.5;
                State::Idle
            }
        }
        State::Idle => {
            mind.rise = (mind.rise - SINK * dt).max(0.0);
            mind.jaw = 0.0;
            let level = to.y.abs() < STRIKE_Y;
            if sees && mind.cool <= 0.0 && level && to.x.abs() < STRIKE_X && mind.rise < 0.5 {
                m.facing = side;
                m.plan = w.ppos;
                mind.caught = false;
                State::Windup(0.0)
            } else {
                want = if sees && level {
                    // Creep under the crust to striking range (not too close: it wants the reach).
                    let goal = w.ppos.x - side * STRIKE_X * 0.7;
                    m.facing = side;
                    if (goal - pos.x).abs() > 8.0 { (goal - pos.x).signum() } else { 0.0 }
                } else if let Some(Shock::Flee | Shock::Panic | Shock::Scatter(_)) = shock {
                    -side * 1.6
                } else if sees {
                    0.0
                } else {
                    // Back toward home, slowly.
                    let back = m.home.x - pos.x;
                    if back.abs() > 30.0 { back.signum() * 0.5 } else { 0.0 }
                };
                State::Idle
            }
        }
        State::Windup(s) => {
            // Rears out of the floor, facing it (it can still turn).
            mind.rise = (mind.rise + RISE * dt).min(1.0);
            m.facing = side;
            if s < spec.windup { State::Windup(s + dt) } else { State::Attack(0.0) }
        }
        State::Attack(s) => {
            mind.rise = 1.0;
            mind.jaw = JAW_REACH * (s / JAW_OUT).min(1.0);
            let mouth = pos + Vec2::new(MOUTH.x * m.facing, MOUTH.y);
            let tip = mouth + Vec2::X * m.facing * mind.jaw;
            if !mind.caught && w.p.dead.is_none() && dist_to_segment(w.ppos, mouth, tip) < JAW_GRIP {
                mind.caught = true;
                w.deeds.bites.push((tip, spec.damage, m.kind));
            }
            if s < spec.attack_time { State::Attack(s + dt) } else { State::Recover(0.0) }
        }
        State::Recover(s) => {
            // Out in the open, reeling the mask back in.
            mind.jaw = JAW_REACH * (1.0 - s / (spec.recover * JAW_REEL)).max(0.0);
            if s < spec.recover {
                State::Recover(s + dt)
            } else {
                mind.cool = NAIAD_COOL;
                State::Idle
            }
        }
        dead @ State::Dead { .. } => dead,
    };
    if !matches!(m.state, State::Hurt(_)) {
        m.vel.x += (want * spec.speed - m.vel.x) * ease(6.0);
    }
    let feet = pos - Vec2::Y * spec.foot;
    let (feet, blocked) = walk(w.terrain, feet, m.vel.x * dt, m.home.x, spec);
    if blocked {
        m.vel.x = 0.0;
    }
    m.stride += dt * m.vel.x.abs() * 0.12;
    feet + Vec2::Y * spec.foot
}

fn dist_to_segment(p: Vec2, a: Vec2, b: Vec2) -> f32 {
    let ab = b - a;
    let t = if ab.length_squared() > 0.0 { ((p - a).dot(ab) / ab.length_squared()).clamp(0.0, 1.0) } else { 0.0 };
    p.distance(a + ab * t)
}

// --- Siphon ---

fn siphon(m: &mut Mob, pos: Vec2, sees: bool, touching: bool, w: &mut World) -> Vec2 {
    let spec = m.spec;
    let dt = w.dt;
    let ease = |rate: f32| 1.0 - (-rate * dt).exp();
    m.treatment.bloat = (m.treatment.bloat - dt / DIGEST).max(0.0);

    if let Some(off) = m.treatment.latch {
        // Shaken off by a dash or a spin (or the chameleon is gone).
        let shaken = w.p.dash > 0.0 || w.anim.spinning() || w.p.dead.is_some() || matches!(m.state, State::Hurt(_));
        if shaken {
            m.treatment.latch = None;
            m.vel = (pos - w.ppos).normalize_or(Vec2::Y) * 260.0 + Vec2::Y * 80.0;
            // Dazed a while: easy prey.
            m.state = State::Recover(-0.8);
            return pos;
        }
        m.treatment.drink += dt;
        m.treatment.bloat = (m.treatment.drink / FEED).min(1.0);
        m.facing = w.p.facing;
        m.stride += dt * 30.0;
        let at = w.ppos + Vec2::new(off.x * w.p.facing, off.y);
        if m.treatment.drink >= FEED {
            let full = match w.p.hurt(1, Vec2::new(w.p.vel.x * 0.5, 140.0), "Drained dry by a plant mosquito") {
                player::Hurt::Ignored => m.treatment.drink > FEED + FEED_GIVE_UP,
                _ => {
                    w.bleed.write(Bleed { pos: at, dir: Vec2::Y, spread: 0.7, speed: 260.0, amount: 14, fluid: Fluid::Red, sticky: true });
                    true
                }
            };
            if full {
                m.treatment.latch = None;
                m.treatment.cool = SIPHON_COOL * 2.0;
                m.vel = Vec2::new(-w.p.facing * 60.0, 120.0);
                m.state = State::Recover(0.0);
            }
        }
        return at;
    }

    let heavy = 1.0 - 0.55 * m.treatment.bloat;
    let away = pos.distance(m.home) > spec.leash;
    m.state = match m.state {
        State::Hurt(s) => {
            m.vel *= 1.0 - 4.0 * dt;
            if s < 0.3 { State::Hurt(s + dt) } else { State::Recover(spec.recover * 0.5) }
        }
        State::Idle => {
            let spot = w.ppos + Vec2::new(BACK.x * w.p.facing, BACK.y);
            if sees && !away && m.treatment.cool <= 0.0 && m.treatment.bloat < 0.3 {
                // Round to its back, a little wobbly.
                let wob = Vec2::new((w.t * 5.0 + m.seed).sin(), (w.t * 7.0 + m.seed).cos()) * 10.0;
                let desired = ((spot + wob - pos) * 3.0).clamp_length_max(spec.speed * 2.6 * heavy);
                m.vel = m.vel.lerp(desired, ease(4.0));
                m.facing = if (w.ppos.x - pos.x).abs() > 4.0 { (w.ppos.x - pos.x).signum() } else { m.facing };
                if pos.distance(spot) < READY {
                    m.plan = w.ppos;
                    State::Windup(0.0)
                } else {
                    State::Idle
                }
            } else {
                // Hovering about its spot, bobbing (sagging when full); startled, it zips off.
                let run = match startled(m) {
                    Some(Shock::Flee | Shock::Panic) => (pos - w.ppos).normalize_or(Vec2::Y),
                    Some(Shock::Scatter(a)) => Vec2::from_angle(a),
                    _ => Vec2::ZERO,
                };
                let bob = Vec2::new((w.t * 0.8 + m.seed).sin() * 40.0, (w.t * 2.3 + m.seed).sin() * 14.0 - 20.0 * m.treatment.bloat);
                let desired = if run != Vec2::ZERO {
                    run * spec.speed * 2.2
                } else {
                    ((m.home + bob - pos) * 2.0).clamp_length_max(spec.speed * heavy)
                };
                m.vel = m.vel.lerp(desired, ease(3.0));
                if m.vel.x.abs() > 8.0 {
                    m.facing = m.vel.x.signum();
                }
                State::Idle
            }
        }
        State::Windup(s) => {
            // Hangs still, legs spread, the proboscis on the chameleon's back.
            m.vel = m.vel.lerp(Vec2::ZERO, ease(10.0));
            m.facing = if w.ppos.x >= pos.x { 1.0 } else { -1.0 };
            if s < spec.windup {
                State::Windup(s + dt)
            } else {
                // Committed to where it was (following only a little).
                let aim = m.plan.lerp(w.ppos, 0.3) + Vec2::Y * 8.0;
                m.vel = (aim - pos).normalize_or(Vec2::new(m.facing, -0.5)) * spec.attack_speed;
                State::Attack(0.0)
            }
        }
        State::Attack(s) => {
            if touching && w.p.dead.is_none() && w.p.guard <= 0.0 {
                // Latched: somewhere on its back.
                let local = Vec2::new(((pos.x - w.ppos.x) * w.p.facing).clamp(-14.0, 8.0), 14.0);
                m.treatment.latch = Some(local);
                m.treatment.drink = 0.0;
                m.vel = Vec2::ZERO;
                State::Attack(s)
            } else if s > spec.attack_time {
                m.vel *= 0.3;
                m.treatment.cool = SIPHON_COOL;
                State::Recover(0.0)
            } else {
                State::Attack(s + dt)
            }
        }
        State::Recover(s) => {
            // Drifts off home, slowly (sagging when full, tumbling when dazed).
            let back = ((m.home - pos) * 1.2).clamp_length_max(spec.speed * 0.8 * heavy) - Vec2::Y * 30.0 * m.treatment.bloat;
            m.vel = m.vel.lerp(back, ease(if s < 0.0 { 1.0 } else { 3.0 }));
            if s < spec.recover { State::Recover(s + dt) } else { State::Idle }
        }
        dead @ State::Dead { .. } => dead,
    };
    if let Some(off) = m.treatment.latch {
        return w.ppos + Vec2::new(off.x * w.p.facing, off.y);
    }
    let (next, blocked) = fly(w.terrain, pos, m.vel * dt, spec.radius * 0.8);
    if blocked && matches!(m.state, State::Attack(_)) {
        m.vel *= -0.2;
        m.treatment.cool = SIPHON_COOL;
        m.state = State::Recover(0.0);
    }
    next
}

/// Latched siphons weigh the chameleon's walk down (after the webs set it, before it moves).
fn drag(mobs: Query<&Mob, Without<crate::terrain_props::Asleep>>, mut player: Single<&mut Player>) {
    let n = mobs.iter().filter(|m| m.treatment.latch.is_some() && !matches!(m.state, State::Dead { .. })).count();
    if n > 0 {
        player.webbed = player.webbed.max((DRAG * n as f32).min(DRAG_MAX));
    }
}

// --- Sump leech ---

/// The bottom of the ceiling right above `p` (within `CEILING_REACH` cells), if any.
fn ceiling(terrain: &Terrain, p: Vec2) -> Option<f32> {
    let mut c = terrain.to_cell(p);
    if terrain.solid(c) {
        return None;
    }
    for _ in 0..CEILING_REACH {
        c.y -= 1;
        if terrain.solid(c) {
            return Some(terrain.cell_min(c).y);
        }
    }
    None
}

/// Creep `dx` along the ceiling from `pos` (the body's center): it follows
/// the ceiling up and down small steps, stops at walls, gaps and the leash.
fn creep(terrain: &Terrain, pos: Vec2, dx: f32, home_x: f32, leash: f32) -> (Vec2, bool) {
    let n = (dx.abs() / 4.0).ceil().max(1.0) as i32;
    let step = dx / n as f32;
    let mut at = pos;
    for _ in 0..n {
        let nx = at.x + step;
        if step != 0.0 && (nx - home_x).abs() > leash && (nx - home_x).abs() > (at.x - home_x).abs() {
            return (at, true);
        }
        let Some(top) = ceiling(terrain, Vec2::new(nx, at.y)) else { return (at, true) };
        let next = Vec2::new(nx, top - HANG);
        if (next.y - at.y).abs() > CEILING_STEP || terrain.box_hits(next, Vec2::new(6.0, 3.0)) {
            return (at, true);
        }
        at = next;
    }
    (at, false)
}

fn leech(m: &mut Mob, pos: Vec2, sees: bool, w: &mut World) -> Vec2 {
    let spec = m.spec;
    let dt = w.dt;
    let ease = |rate: f32| 1.0 - (-rate * dt).exp();
    let to = w.ppos - pos;
    let mut want = 0.0;
    m.state = match m.state {
        State::Hurt(s) => {
            m.treatment.swell = (m.treatment.swell - dt * 2.0).max(0.0);
            if s < 0.4 { State::Hurt(s + dt) } else { State::Recover(spec.recover * 0.5) }
        }
        State::Idle => {
            m.treatment.swell = (m.treatment.swell - dt * 2.0).max(0.0);
            if sees && to.y < -20.0 {
                // Loops along to hang right over it.
                if to.x.abs() < OVER && m.treatment.cool <= 0.0 {
                    m.plan = w.ppos;
                    State::Windup(0.0)
                } else {
                    want = if to.x.abs() > 6.0 { to.x.signum() } else { 0.0 };
                    State::Idle
                }
            } else {
                // Grazing the ceiling slime, now and then moving on.
                m.wander.1 -= dt;
                if m.wander.1 <= 0.0 {
                    let off = pos.x - m.home.x;
                    m.wander = if m.rng.chance(0.4) {
                        (0.0, m.rng.range(1.5, 3.0))
                    } else if off.abs() > spec.leash * 0.6 {
                        (-off.signum(), m.rng.range(1.5, 3.0))
                    } else {
                        (if m.rng.chance(0.5) { 1.0 } else { -1.0 }, m.rng.range(1.5, 3.0))
                    };
                }
                want = m.wander.0 * 0.6;
                State::Idle
            }
        }
        State::Windup(s) => {
            // Swells, dripping where the drop will fall.
            m.treatment.swell = (s / spec.windup).min(1.0);
            if m.rng.chance(dt * 6.0 * m.treatment.swell) {
                let drip = Bleed { pos: pos - Vec2::Y * 6.0, dir: -Vec2::Y, spread: 0.15, speed: 60.0, amount: 1, fluid: spec.blood, sticky: true };
                w.bleed.write(drip);
            }
            if s < spec.windup {
                State::Windup(s + dt)
            } else {
                m.treatment.release = true;
                m.treatment.swell = 0.0;
                m.treatment.cool = LEECH_COOL;
                State::Recover(0.0)
            }
        }
        State::Recover(s) => {
            if s < spec.recover { State::Recover(s + dt) } else { State::Idle }
        }
        other => other,
    };
    m.vel.x += (want * spec.speed - m.vel.x) * ease(5.0);
    if m.vel.x.abs() > 4.0 {
        m.facing = m.vel.x.signum();
    }
    let (next, blocked) = creep(w.terrain, pos, m.vel.x * dt, m.home.x, spec.leash);
    if blocked {
        m.vel.x = 0.0;
        if m.state == State::Idle && !sees {
            m.wander = (-m.wander.0, m.rng.range(1.0, 2.0));
        }
    }
    m.stride += dt * m.vel.x.abs() * 0.12;
    next
}

/// A drop of caustic falling from a sump leech.
#[derive(Component)]
struct Drop {
    vel: Vec2,
}

/// A burning puddle where a drop landed.
#[derive(Component)]
struct Puddle {
    age: f32,
}

struct AcidAssets {
    drop: Handle<Mesh>,
    puddle: Handle<Mesh>,
    foam: Handle<Mesh>,
    acid: Handle<ColorMaterial>,
    froth: Handle<ColorMaterial>,
}

/// Drops let go by the leeches fall; they burn the chameleon, else leave a
/// puddle on the floor that burns whoever stands in it, then dries up.
#[allow(clippy::too_many_arguments)]
fn acid(
    mut commands: Commands,
    time: Res<Time>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    mut assets: Local<Option<AcidAssets>>,
    mut shake: ResMut<Shake>,
    mut stop: ResMut<HitStop>,
    mut bleed: MessageWriter<Bleed>,
    player: Single<(&mut Player, &Transform), (Without<Drop>, Without<Puddle>, Without<Mob>)>,
    mut mobs: Query<(&mut Mob, &Transform), (Without<Drop>, Without<Puddle>, Without<crate::terrain_props::Asleep>)>,
    mut drops: Query<(Entity, &mut Drop, &mut Transform), (Without<Puddle>, Without<Mob>)>,
    mut puddles: Query<(Entity, &mut Puddle, &mut Transform), (Without<Drop>, Without<Mob>)>,
    terrain: Res<Terrain>,
) {
    let dt = time.delta_secs().min(1.0 / 30.0);
    let (mut p, ptf) = player.into_inner();
    let ppos = ptf.translation.truncate();
    let a = assets.get_or_insert_with(|| AcidAssets {
        drop: meshes.add(fill(&smooth_closed(&[Vec2::new(0.0, 5.0), Vec2::new(2.6, -0.5), Vec2::new(0.0, -3.2), Vec2::new(-2.6, -0.5)], 4), |p| 0.8 + 0.2 * p.y / 5.0)),
        puddle: meshes.add(fill(&ellipse_pts(PUDDLE_HALF, 3.5, 24), |p| 0.75 + 0.25 * (p.y / 3.5))),
        foam: meshes.add(fill(&ellipse_pts(2.2, 2.0, 10), |_| 1.0)),
        acid: materials.add(ACID),
        froth: materials.add(ACID_FOAM),
    });

    for (mut m, tf) in &mut mobs {
        if !std::mem::take(&mut m.treatment.release) || matches!(m.state, State::Dead { .. }) {
            continue;
        }
        let at = tf.translation.truncate() - Vec2::Y * 7.0;
        commands.spawn((
            Drop { vel: Vec2::new(0.0, -60.0) },
            MobStuff,
            Mesh2d(a.drop.clone()),
            MeshMaterial2d(a.acid.clone()),
            Transform::from_translation(at.extend(13.0)).with_scale(Vec3::splat(1.6)),
            camera::color_layer(),
        ));
    }

    let alive = p.dead.is_none();
    for (e, mut d, mut tf) in &mut drops {
        d.vel.y -= DROP_GRAVITY * dt;
        let pos = tf.translation.truncate() + d.vel * dt;
        tf.translation.x = pos.x;
        tf.translation.y = pos.y;
        let splash = |bleed: &mut MessageWriter<Bleed>, amount| {
            bleed.write(Bleed { pos, dir: Vec2::Y, spread: 1.3, speed: 180.0, amount, fluid: Fluid::Pink, sticky: true });
        };
        if alive && (pos - ppos).abs().cmplt(player::HALF + Vec2::splat(3.0)).all() {
            boss::hurt_player(&mut p, ppos, pos + Vec2::Y * 10.0, 1, "Dissolved by sump leech acid", &mut bleed, (&mut shake, &mut stop));
            splash(&mut bleed, 10);
            commands.entity(e).despawn();
        } else if terrain.solid_at(pos) {
            splash(&mut bleed, 6);
            commands.entity(e).despawn();
            // A puddle on the floor it hit (a wall or a ledge lip: just the splash).
            if let Some(top) = terrain.surface(pos + Vec2::Y * STEP_UP, 3.0 * STEP_UP) {
                let at = Vec2::new(pos.x, top + 1.0);
                let puddle = commands
                    .spawn((
                        Puddle { age: 0.0 },
                        MobStuff,
                        Mesh2d(a.puddle.clone()),
                        MeshMaterial2d(a.acid.clone()),
                        Transform::from_translation(at.extend(13.0)),
                        camera::color_layer(),
                    ))
                    .id();
                for (i, x) in [-13.0f32, 2.0, 15.0].into_iter().enumerate() {
                    commands.spawn((
                        Mesh2d(a.foam.clone()),
                        MeshMaterial2d(a.froth.clone()),
                        Transform::from_xyz(x, 1.0 + i as f32 * 0.4, 0.1),
                        camera::color_layer(),
                        ChildOf(puddle),
                    ));
                }
            }
        } else if pos.y < terrain.bounds.min.y - 600.0 {
            commands.entity(e).despawn();
        }
    }

    let feet = ppos - Vec2::Y * player::HALF.y;
    for (e, mut pd, mut tf) in &mut puddles {
        pd.age += dt;
        let left = PUDDLE_LIFE - pd.age;
        if left <= 0.0 {
            commands.entity(e).despawn();
            continue;
        }
        // Spreads out at once, seethes, shrinks away at the end.
        let k = (pd.age / 0.12).min(1.0) * (left / PUDDLE_FADE).min(1.0);
        let seethe = 1.0 + (pd.age * 9.0).sin() * 0.06;
        tf.scale = Vec3::new(k * seethe, k * (2.0 - seethe), 1.0);
        let c = tf.translation.truncate();
        let burning = left > PUDDLE_FADE;
        if burning && alive && p.grounded && (feet.x - c.x).abs() < PUDDLE_HALF + player::HALF.x * 0.4 && (feet.y - c.y).abs() < 10.0 {
            boss::hurt_player(&mut p, ppos, c - Vec2::Y * 30.0, 1, "Dissolved by sump leech acid", &mut bleed, (&mut shake, &mut stop));
        }
    }
}

/// The treatment mobs' own systems: the drag of latched siphons (after the
/// webs set `Player::webbed`, before the chameleon moves) and the caustic.
pub(super) fn build(app: &mut App) {
    app.add_systems(
        Update,
        (
            drag.after(crate::webs::stick).before(player::move_player),
            acid.after(super::think).before(super::nests),
        ),
    );
}

// --- Placement ---

/// A spot near `p` where a treatment kind can live (its center), if any.
pub(super) fn site(terrain: &Terrain, spec: &MobSpec, p: Vec2, rect: Rect) -> Option<Vec2> {
    match spec.moves {
        Moves::Siphon => {
            let open = terrain.clearance(p) >= super::FLY_ROOM;
            (open && terrain.surface(p, 500.0).is_some()).then_some(p)
        }
        Moves::Inchworm => {
            // Under a ceiling with some run to it, over open air and a floor.
            let mut c = terrain.to_cell(p);
            for _ in 0..40 {
                c.y -= 1;
                if terrain.solid(c) {
                    let at = Vec2::new(p.x, terrain.cell_min(c).y - HANG);
                    let run = creep(terrain, at, 6.0 * PX, at.x, f32::MAX).0.x - creep(terrain, at, -6.0 * PX, at.x, f32::MAX).0.x;
                    let room = !terrain.box_hits(at - Vec2::Y * 60.0, Vec2::new(20.0, 40.0));
                    return (rect.contains(at) && room && run >= 8.0 * PX && terrain.surface(at, 600.0).is_some()).then_some(at);
                }
            }
            None
        }
        _ => {
            // On the floor, with room to strike along it.
            let feet = Vec2::new(p.x, terrain.surface(p, rect.height())?);
            let (left, right) = terrain.flat_span(feet, 24.0);
            (rect.contains(feet) && super::fits(terrain, feet, spec) && right - left >= 16.0 * PX).then_some(feet + Vec2::Y * spec.foot)
        }
    }
}
