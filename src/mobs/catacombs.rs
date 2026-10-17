//! The catacombs' own vermin: the Weaver's kin, and what stalks the bones.
//!
//! - **Crypt jumpers** (jumping spiders, fodder): stalk in stop-and-go darts
//!   to pouncing range, freeze, raise their front legs (the tell) and pounce
//!   in an arc committed to where the chameleon stood, trailing a dragline.
//!   A miss leaves them splayed where they land.
//! - **Grave trapdoors** (ambushers): hidden under a lid in the floor, only
//!   the lid and the silk tripwires around it show (and nothing hurts them in
//!   there). Something walking near: the lid lifts and eyes glint (the tell),
//!   then it bursts out in a lunge along the floor, bites, and has to drag
//!   itself back into its hole: the window to hit it.
//! - **Ossuary wolves** (wolf spiders, veterans): hunt in sprints and
//!   freezes, keep their distance, rear and lunge. They carry their brood on
//!   their back: every hit shakes a spiderling off, and the rest scatter
//!   when the mother bursts (`hatch`).
//! - **Bonestriders** (harvestmen): a small body slung high on eight long
//!   stilts that step over gaps and rubble. They stalk over the chameleon,
//!   rise a little (the tell) and drop their body like a hammer, then sit
//!   low for a moment before climbing back up. Struck, they shed a leg and
//!   scramble off, slower. Not spiders: the webs catch them too.
//!
//! The first three are spiders (blue blood, the webs don't hold them).
//! Bodies and animation in `mob_art/catacombs.rs`.

use std::f32::consts::PI;

use bevy::prelude::*;

use super::{fits, hop, walk, Mob, MobKind, MobSpec, Moves, Notice, Shock, State, GRAVITY};
use crate::{
    gore::Fluid,
    level::Platform,
    mob_art,
    player::Player,
    terrain::{Terrain, PX, STEP_UP},
};

/// The catacombs' menu, whoever owns the region: the Weaver's kin, plus the
/// dangling spiders, corpse flies and ossuary rats that already lived here.
pub(super) fn menu() -> Vec<(MobKind, f32)> {
    vec![
        (MobKind::CryptJumper, 0.24),
        (MobKind::Bonestrider, 0.16),
        (MobKind::GraveTrapdoor, 0.12),
        (MobKind::OssuaryWolf, 0.12),
        (MobKind::Spider, 0.16),
        (MobKind::Fly, 0.1),
        (MobKind::Rat, 0.1),
    ]
}

pub(super) static CRYPT_JUMPER: MobSpec = MobSpec {
    moves: Moves::Crypt,
    hp: 1,
    scale: 1.3,
    radius: 12.0,
    foot: 6.0 * 1.3,
    blood: Fluid::Blue,
    speed: 60.0,
    sense: 420.0,
    // Front legs up.
    windup: 0.45,
    // Most horizontal pounce speed.
    attack_speed: 520.0,
    attack_time: 1.2,
    // Splayed where it landed.
    recover: 0.7,
    leash: 340.0,
    damage: 1,
    stubborn: false,
    initiative: 0.5,
    veteran: false,
    // 1 hp, but it comes flying.
    food: 0.35,
};

pub(super) static GRAVE_TRAPDOOR: MobSpec = MobSpec {
    moves: Moves::Crypt,
    hp: 3,
    scale: 1.7,
    radius: 17.0,
    foot: 5.5 * 1.7,
    blood: Fluid::Blue,
    // Back to its hole.
    speed: 90.0,
    // Feels steps on its floor from this far (see `seen`).
    sense: 300.0,
    // The lid lifts, eyes in the gap.
    windup: 0.45,
    attack_speed: 600.0,
    attack_time: 0.35,
    // Out of its hole, gathering itself: the window.
    recover: 1.1,
    // How far it lunges from its hole.
    leash: 150.0,
    damage: 1,
    stubborn: true,
    initiative: 0.3,
    veteran: true,
    // 3 hp, untouchable until it strikes.
    food: 1.0,
};

pub(super) static OSSUARY_WOLF: MobSpec = MobSpec {
    moves: Moves::Crypt,
    hp: 3,
    scale: 1.9,
    radius: 19.0,
    foot: 6.0 * 1.9,
    blood: Fluid::Blue,
    speed: 75.0,
    sense: 480.0,
    // Rears up, fangs out.
    windup: 0.35,
    attack_speed: 460.0,
    attack_time: 0.35,
    recover: 0.9,
    leash: 420.0,
    damage: 1,
    stubborn: false,
    initiative: 0.8,
    veteran: true,
    // 3 hp, quick, sheds its brood on you (they give nothing).
    food: 1.0,
};

pub(super) static BONESTRIDER: MobSpec = MobSpec {
    moves: Moves::Crypt,
    hp: 3,
    scale: 1.0,
    radius: 14.0,
    // The body alone (the corpse lies on the floor); the stilts are `STILT`.
    foot: 8.0,
    // Harvestmen bleed blue too.
    blood: Fluid::Blue,
    speed: 60.0,
    sense: 460.0,
    // Rises a little, shudders.
    windup: 0.55,
    // Body plunge speed.
    attack_speed: 620.0,
    attack_time: 0.6,
    // Sitting low after the plunge: the window.
    recover: 1.1,
    leash: 360.0,
    damage: 1,
    stubborn: false,
    initiative: 1.2,
    veteran: false,
    // 3 hp, hard to reach up there.
    food: 0.9,
};

/// The smallest step a crawler takes (creeping a fraction of a unit per
/// frame, the body box would catch on the edge of a pixel step).
const MIN_STEP: f32 = 4.0;

/// Jumper: pounces from this far (horizontally), stalking in to it / backing off from closer.
const POUNCE_MIN: f32 = 70.0;
const POUNCE_MAX: f32 = 230.0;
/// Jumper: darts while stalking, and the rest after a pounce.
const DART: f32 = 230.0;
const POUNCE_REST: f32 = 0.6;
/// Jumper: how much the pounce follows the chameleon after the tell (0 = committed to where it stood).
const POUNCE_FOLLOW: f32 = 0.2;
/// Jumper: horizontal pace that sets the pounce's flight time.
const POUNCE_PACE: f32 = 440.0;

/// Trapdoor: how open the lid is during the tell.
const PEEK: f32 = 0.35;
/// Trapdoor: springs at the chameleon this close along its floor, and this level with it.
const TRIP: f32 = 140.0;
const TRIP_HEIGHT: f32 = 50.0;
/// Trapdoor: overshoots where the chameleon stood by this much, and rests this long back in its hole.
const OVERSHOOT: f32 = 24.0;
const BURROW_REST: f32 = 1.2;

/// Wolf: lunges from this close, keeps at least this far otherwise.
const LUNGE: f32 = 140.0;
const KEEP: f32 = 70.0;
/// Wolf: sprint and back-off speeds (× `attack_speed`), and the rest between lunges.
const SPRINT: f32 = 0.75;
const BACK_OFF: f32 = 0.35;
const LUNGE_REST: f32 = 0.5;
/// Wolf: spiderlings carried on its back.
const BROOD: u8 = 3;

/// Strider: legs (0..4 near side, 4..8 far side).
pub(crate) const LEGS: usize = 8;
/// Strider: its body rides this high over the ground.
pub(crate) const STILT: f32 = 72.0;
/// Strider: leg halves (hip → knee → foot).
pub(crate) const THIGH: f32 = 58.0;
pub(crate) const SHIN: f32 = 80.0;
/// Strider: where each foot wants to stand (x from the body), and the hips (x on the body).
const REACH: [f32; LEGS] = [-74.0, -40.0, 30.0, 68.0, -60.0, -24.0, 16.0, 52.0];
pub(crate) const HIPS: [f32; LEGS] = [-3.5, -1.5, 1.5, 3.5, -2.5, -0.8, 0.8, 2.5];
/// Strider: a foot steps when this far from where it wants to be; a step takes this long, this high.
const STEP_TRIGGER: f32 = 30.0;
const STEP_TIME: f32 = 0.16;
const STEP_LIFT: f32 = 18.0;
/// Strider: highest rise of the ground it steps up in one go, and how far apart it feels for footing.
const MAX_RISE: f32 = 5.0 * PX;
const SPAN: f32 = 26.0;
/// Strider: drops when the chameleon is this close below (horizontally), down to this over the floor.
const OVER: f32 = 28.0;
const LOWEST: f32 = 22.0;
/// Strider: rises this much in the tell, climbs back at this speed, rests this long after.
const GATHER: f32 = 10.0;
const RISE: f32 = 90.0;
const STRIDE_REST: f32 = 0.8;
/// Strider: speed lost per leg shed.
const LIMP: f32 = 0.1;
/// Strider: which leg it sheds next when struck.
const SHED_ORDER: [usize; LEGS] = [3, 4, 0, 7, 2, 5, 1, 6];

/// What a catacomb mob has in mind (kept on its `Mob`); each kind uses its part.
#[derive(Default)]
pub(crate) struct Mind {
    ready: bool,
    /// Movement saved up until it makes a whole step (`MIN_STEP`).
    carry: f32,
    /// Stop-and-go: moving this way (0 = frozen), for this long still.
    dir: f32,
    burst: f32,
    /// Time before it may attack again.
    rest: f32,
    /// Jumper: where its dragline is fixed (where it pounced from), while it trails one.
    pub(crate) anchor: Option<Vec2>,
    /// Trapdoor: 0 = shut in its hole, 1 = all the way out.
    pub(crate) out: f32,
    /// Wolf: spiderlings on its back, and how many just fell off (`hatch` spawns them).
    pub(crate) brood: u8,
    shed: u8,
    /// Strider: the ground under it (eased), how low it has dropped its body.
    ground: f32,
    drop: f32,
    /// Strider: where each foot stands (world), its step (0 = planted, else 0..1 through it) from → to.
    pub(crate) feet: [Vec2; LEGS],
    lift: [f32; LEGS],
    from: [Vec2; LEGS],
    to: [Vec2; LEGS],
    /// Strider: legs it has shed, and the one just shed (torn off by `mob_art::dismember`).
    pub(crate) lost: [bool; LEGS],
    pub(crate) cut: Option<usize>,
}

/// What a catacomb mob sees of the world this frame.
pub(super) struct World<'a> {
    pub terrain: &'a Terrain,
    pub ppos: Vec2,
    pub t: f32,
    pub dt: f32,
}

/// Whether it can make out the chameleon from where it is (line of sight is checked by the caller).
pub(super) fn seen(m: &Mob, to_player: Vec2, from_home: f32) -> bool {
    let spec = m.spec;
    match m.kind {
        MobKind::GraveTrapdoor => to_player.x.abs() < TRIP && to_player.y.abs() < TRIP_HEIGHT,
        MobKind::OssuaryWolf => to_player.y.abs() < 140.0 && from_home.abs() < spec.leash + 150.0,
        MobKind::Bonestrider => to_player.y.abs() < STILT + 160.0 && from_home.abs() < spec.leash + 100.0,
        _ => to_player.y.abs() < 220.0 && from_home.abs() < spec.leash + 200.0,
    }
}

/// Hiding in its hole: nothing reaches it.
pub(crate) fn hidden(m: &Mob) -> bool {
    m.kind == MobKind::GraveTrapdoor && m.state == State::Idle && m.catacombs.out < 0.15
}

/// It was just hit (`hp` already taken off): wolves shed brood, striders a leg.
pub(super) fn struck(m: &mut Mob) {
    match m.kind {
        MobKind::OssuaryWolf => {
            let n = if m.hp <= 0 { m.catacombs.brood } else { m.catacombs.brood.min(1) };
            m.catacombs.brood -= n;
            m.catacombs.shed += n;
        }
        MobKind::Bonestrider if m.hp > 0 => {
            if let Some(&i) = SHED_ORDER.iter().find(|&&i| !m.catacombs.lost[i]) {
                m.catacombs.lost[i] = true;
                m.catacombs.cut = Some(i);
                m.dismember = true;
            }
        }
        _ => {}
    }
}

/// Knocked back by a blow it survived (`side`: away from the chameleon).
pub(super) fn knock(m: &mut Mob, side: f32) -> Vec2 {
    match m.kind {
        MobKind::CryptJumper => {
            m.airborne = true;
            Vec2::new(side * 240.0, 200.0)
        }
        MobKind::OssuaryWolf => Vec2::new(side * 220.0, 0.0),
        MobKind::GraveTrapdoor => Vec2::new(side * 100.0, 0.0),
        _ => Vec2::ZERO,
    }
}

/// A spot near `p` where this kind can live (its center), if any.
pub(super) fn site(terrain: &Terrain, kind: MobKind, p: Vec2, rect: Rect) -> Option<Vec2> {
    let spec = kind.spec();
    let feet = Vec2::new(p.x, terrain.surface(p, rect.height())?);
    let (left, right) = terrain.flat_span(feet, 24.0);
    if !rect.contains(feet) || !fits(terrain, feet, spec) {
        return None;
    }
    match kind {
        MobKind::Bonestrider => {
            // Room for the stilts and the body on top, and ground to walk.
            let half = (STILT + 12.0) / 2.0;
            let clear = !terrain.box_hits(feet + Vec2::Y * (STEP_UP + half), Vec2::new(16.0, half - STEP_UP));
            (clear && right - left >= 16.0 * PX).then_some(feet + Vec2::Y * STILT)
        }
        // A burrow in the middle of a floor long enough to lunge along.
        MobKind::GraveTrapdoor => {
            let room = feet.x - left > spec.leash * 0.6 && right - feet.x > spec.leash * 0.6;
            room.then_some(feet + Vec2::Y * spec.foot)
        }
        _ => (right - left >= 12.0 * PX).then_some(feet + Vec2::Y * spec.foot),
    }
}

/// One frame of a catacomb mob (alive; corpses are handled by `mobs::think`).
pub(super) fn think(
    m: &mut Mob,
    tf: &mut Transform,
    sees: bool,
    touching: bool,
    w: &World,
    platforms: &Query<(&Platform, &Transform), (Without<Mob>, Without<Player>)>,
    bites: &mut Vec<(Vec2, i32, MobKind)>,
) {
    let pos = tf.translation.truncate();
    if !m.catacombs.ready {
        m.catacombs.ready = true;
        m.catacombs.brood = BROOD;
        m.catacombs.ground = pos.y - STILT;
        if m.kind == MobKind::Bonestrider {
            for i in 0..LEGS {
                let foot = foothold(w.terrain, pos, REACH[i]).unwrap_or(pos - Vec2::new(-REACH[i], STILT));
                m.catacombs.feet[i] = foot;
            }
        }
    }
    m.catacombs.rest -= w.dt;
    let next = match m.kind {
        MobKind::CryptJumper => jumper(m, pos, sees, touching, w, platforms, bites),
        MobKind::GraveTrapdoor => trapdoor(m, pos, sees, touching, w, bites),
        MobKind::OssuaryWolf => wolf(m, pos, sees, touching, w, bites),
        MobKind::Bonestrider => strider(m, pos, sees, touching, w, bites),
        _ => pos,
    };
    tf.translation.x = next.x;
    tf.translation.y = next.y;
}

fn ease(rate: f32, dt: f32) -> f32 {
    1.0 - (-rate * dt).exp()
}

/// Caught by surprise: which way it runs (0 = frozen), if it is in shock.
fn startled(m: &Mob, side: f32) -> Option<f32> {
    match m.notice {
        Notice::Shock { how: Shock::Freeze, .. } => Some(0.0),
        Notice::Shock { how: Shock::Flee, .. } => Some(-side),
        Notice::Shock { how: Shock::Panic, .. } => Some(-side * 1.8),
        Notice::Shock { how: Shock::Scatter(a), .. } => Some(a.cos().signum()),
        _ => None,
    }
}

/// Minding its own business: strolls about its spot, pausing now and then.
fn wander(m: &mut Mob, pos: Vec2, dt: f32) -> f32 {
    m.wander.1 -= dt;
    if m.wander.1 <= 0.0 {
        let away = pos.x - m.home.x;
        m.wander = if m.rng.chance(0.4) {
            (0.0, m.rng.range(0.8, 2.0))
        } else if away.abs() > m.spec.leash * 0.6 {
            (-away.signum(), m.rng.range(1.0, 2.5))
        } else {
            (if m.rng.chance(0.5) { 1.0 } else { -1.0 }, m.rng.range(1.0, 2.5))
        };
    }
    m.wander.0 * m.spec.speed
}

/// Stop-and-go: darts `toward` (or away, or nowhere) for a moment, then
/// freezes for a moment. Returns the speed wanted.
fn stop_and_go(m: &mut Mob, toward: f32, dash: (f32, f32), freeze: (f32, f32), dt: f32) -> f32 {
    m.catacombs.burst -= dt;
    if m.catacombs.burst <= 0.0 {
        if m.catacombs.dir != 0.0 || toward == 0.0 {
            m.catacombs.dir = 0.0;
            m.catacombs.burst = m.rng.range(freeze.0, freeze.1);
        } else {
            m.catacombs.dir = toward;
            m.catacombs.burst = m.rng.range(dash.0, dash.1);
        }
    }
    m.catacombs.dir
}

/// Along the ground toward speed `want` (eased at `rate`), in whole steps.
/// Returns the new center and whether it bumped into something.
fn crawl(m: &mut Mob, terrain: &Terrain, pos: Vec2, want: f32, rate: f32, dt: f32) -> (Vec2, bool) {
    let spec = m.spec;
    m.vel.x += (want - m.vel.x) * ease(rate, dt);
    m.stride += dt * m.vel.x.abs() * 0.12;
    m.catacombs.carry += m.vel.x * dt;
    if m.catacombs.carry.abs() < MIN_STEP {
        return (pos, false);
    }
    let dx = std::mem::take(&mut m.catacombs.carry);
    let (feet, blocked) = walk(terrain, pos - Vec2::Y * spec.foot, dx, m.home.x, spec);
    if blocked {
        m.vel.x = 0.0;
    }
    (feet + Vec2::Y * spec.foot, blocked)
}

// --- Crypt jumper ---

fn jumper(
    m: &mut Mob,
    pos: Vec2,
    sees: bool,
    touching: bool,
    w: &World,
    platforms: &Query<(&Platform, &Transform), (Without<Mob>, Without<Player>)>,
    bites: &mut Vec<(Vec2, i32, MobKind)>,
) -> Vec2 {
    let spec = m.spec;
    let dt = w.dt;
    let to = w.ppos - pos;
    let side = if to.x >= 0.0 { 1.0 } else { -1.0 };
    let d = to.x.abs();
    let mut want = 0.0;
    m.state = match m.state {
        State::Hurt(s) if s < 0.3 => State::Hurt(s + dt),
        State::Hurt(_) => State::Recover(spec.recover * 0.5),
        // Front legs up, eyes on the spot.
        State::Windup(s) if s < spec.windup => State::Windup(s + dt),
        State::Windup(_) => {
            pounce(m, pos, w.ppos);
            State::Attack(0.0)
        }
        State::Attack(_) if touching && m.airborne => {
            bites.push((pos, spec.damage, m.kind));
            m.vel = Vec2::new(-m.vel.x * 0.3, 160.0);
            State::Recover(0.0)
        }
        // Landed (a miss): splayed there for a moment.
        State::Attack(s) if (!m.airborne && s > 0.05) || s > spec.attack_time => State::Recover(0.0),
        State::Attack(s) => State::Attack(s + dt),
        State::Recover(s) if s < spec.recover => State::Recover(s + dt),
        State::Recover(_) => {
            m.catacombs.anchor = None;
            m.catacombs.rest = POUNCE_REST;
            State::Idle
        }
        State::Idle if sees && !m.airborne && m.catacombs.rest <= 0.0 && m.catacombs.dir == 0.0 && (POUNCE_MIN..POUNCE_MAX).contains(&d) => {
            m.facing = side;
            m.plan = w.ppos;
            State::Windup(0.0)
        }
        State::Idle => {
            if let Some(dir) = startled(m, side) {
                want = dir * spec.speed * 2.0;
            } else if sees {
                // Stalking: dart in to pouncing range (or back out to it), freeze, look.
                m.facing = side;
                let toward = if d > POUNCE_MAX - 20.0 {
                    side
                } else if d < POUNCE_MIN + 15.0 {
                    -side
                } else {
                    0.0
                };
                want = stop_and_go(m, toward, (0.12, 0.25), (0.25, 0.6), dt) * DART;
            } else {
                want = wander(m, pos, dt);
            }
            if !sees && want.abs() > 1.0 {
                m.facing = want.signum();
            }
            State::Idle
        }
        dead @ State::Dead { .. } => dead,
    };
    if m.airborne {
        return hop(m, w.terrain, platforms, pos, dt);
    }
    if m.state != State::Idle {
        want = 0.0;
    }
    let (next, blocked) = crawl(m, w.terrain, pos, want, 14.0, dt);
    if blocked && m.state == State::Idle {
        m.wander = (-m.wander.0, m.rng.range(1.0, 2.0));
        m.catacombs.burst = 0.0;
    }
    next
}

/// The tell is over: an arc that comes down where the chameleon stood.
fn pounce(m: &mut Mob, pos: Vec2, ppos: Vec2) {
    let spec = m.spec;
    let to = m.plan.lerp(ppos, POUNCE_FOLLOW) - pos;
    let time = (to.x.abs() / POUNCE_PACE).clamp(0.32, 0.55);
    let vx = (to.x / time).clamp(-spec.attack_speed, spec.attack_speed);
    let vy = ((to.y + 0.5 * GRAVITY * time * time) / time).clamp(220.0, 720.0);
    m.vel = Vec2::new(vx, vy);
    m.airborne = true;
    m.catacombs.anchor = Some(pos);
}

// --- Grave trapdoor ---

fn trapdoor(m: &mut Mob, pos: Vec2, sees: bool, touching: bool, w: &World, bites: &mut Vec<(Vec2, i32, MobKind)>) -> Vec2 {
    let spec = m.spec;
    let dt = w.dt;
    let to = w.ppos - pos;
    let side = if to.x >= 0.0 { 1.0 } else { -1.0 };
    let back = m.home.x - pos.x;
    let mut want = 0.0;
    m.state = match m.state {
        State::Hurt(s) if s < 0.3 => State::Hurt(s + dt),
        // Hurt: back to the hole, fast.
        State::Hurt(_) => {
            m.catacombs.burst = 1.5;
            State::Idle
        }
        State::Idle if sees && m.catacombs.out < 0.2 && m.catacombs.rest <= 0.0 => {
            m.facing = side;
            m.plan = w.ppos;
            State::Windup(0.0)
        }
        State::Idle => {
            if back.abs() > 3.0 {
                // Out: drags itself back in.
                let hurry = if m.catacombs.burst > 0.0 { 1.8 } else { 1.0 };
                m.catacombs.burst -= dt;
                want = back.signum() * spec.speed * hurry;
                m.facing = back.signum();
            }
            State::Idle
        }
        State::Windup(s) if s < spec.windup => State::Windup(s + dt),
        State::Windup(_) => State::Attack(0.0),
        State::Attack(_) if touching => {
            bites.push((pos, spec.damage, m.kind));
            State::Recover(0.0)
        }
        State::Attack(s) if s > spec.attack_time || (m.plan.x + m.facing * OVERSHOOT - pos.x) * m.facing <= 0.0 => State::Recover(0.0),
        State::Attack(s) => {
            want = m.facing * spec.attack_speed;
            State::Attack(s + dt)
        }
        State::Recover(s) if s < spec.recover => State::Recover(s + dt),
        State::Recover(_) => {
            m.catacombs.rest = BURROW_REST;
            State::Idle
        }
        dead @ State::Dead { .. } => dead,
    };
    // The lid: shut at home, ajar in the tell, thrown open otherwise.
    let home = back.abs() <= 3.0;
    let open = match m.state {
        State::Windup(_) => PEEK,
        State::Idle if home => 0.0,
        _ => 1.0,
    };
    let rate = if open > m.catacombs.out { 14.0 } else { 5.0 };
    m.catacombs.out += (open - m.catacombs.out) * ease(rate, dt);
    if open == 0.0 && m.catacombs.out < 0.02 {
        m.catacombs.out = 0.0;
    }
    let rate = if matches!(m.state, State::Attack(_)) { 40.0 } else { 10.0 };
    let (next, blocked) = crawl(m, w.terrain, pos, want, rate, dt);
    if blocked && matches!(m.state, State::Attack(_)) {
        m.state = State::Recover(0.0);
    }
    if m.state == State::Idle && home {
        m.vel.x = 0.0;
        // Settled in, exactly over its hole.
        return Vec2::new(m.home.x, next.y);
    }
    next
}

// --- Ossuary wolf ---

fn wolf(m: &mut Mob, pos: Vec2, sees: bool, touching: bool, w: &World, bites: &mut Vec<(Vec2, i32, MobKind)>) -> Vec2 {
    let spec = m.spec;
    let dt = w.dt;
    let to = w.ppos - pos;
    let side = if to.x >= 0.0 { 1.0 } else { -1.0 };
    let d = to.x.abs();
    let mut want = 0.0;
    let mut rate = 12.0;
    m.state = match m.state {
        State::Hurt(s) if s < 0.3 => State::Hurt(s + dt),
        State::Hurt(_) => State::Recover(spec.recover * 0.5),
        State::Windup(s) if s < spec.windup => State::Windup(s + dt),
        State::Windup(_) => {
            m.vel.x = m.facing * spec.attack_speed;
            State::Attack(0.0)
        }
        State::Attack(_) if touching => {
            bites.push((pos, spec.damage, m.kind));
            m.vel.x *= -0.3;
            State::Recover(0.0)
        }
        State::Attack(s) if s > spec.attack_time => State::Recover(0.0),
        State::Attack(s) => {
            want = m.facing * spec.attack_speed;
            rate = 30.0;
            State::Attack(s + dt)
        }
        State::Recover(s) if s < spec.recover => {
            rate = 6.0;
            State::Recover(s + dt)
        }
        State::Recover(_) => {
            m.catacombs.rest = LUNGE_REST;
            State::Idle
        }
        State::Idle if sees && m.catacombs.dir == 0.0 && m.catacombs.rest <= 0.0 && d < LUNGE && to.y.abs() < 50.0 => {
            m.facing = side;
            m.plan = w.ppos;
            State::Windup(0.0)
        }
        State::Idle => {
            if let Some(dir) = startled(m, side) {
                want = dir * spec.speed * 2.0;
            } else if sees {
                // Hunting: sprint, freeze and listen, sprint; never too close.
                let toward = if d > LUNGE * 0.8 {
                    side
                } else if d < KEEP {
                    -side
                } else {
                    0.0
                };
                let dir = stop_and_go(m, toward, (0.25, 0.45), (0.35, 0.7), dt);
                want = dir * spec.attack_speed * if dir == side { SPRINT } else { BACK_OFF };
                m.facing = side;
            } else {
                want = wander(m, pos, dt);
                if want.abs() > 1.0 {
                    m.facing = want.signum();
                }
            }
            State::Idle
        }
        dead @ State::Dead { .. } => dead,
    };
    let (next, blocked) = crawl(m, w.terrain, pos, want, rate, dt);
    if blocked {
        match m.state {
            State::Attack(_) => m.state = State::Recover(0.0),
            State::Idle => {
                m.wander = (-m.wander.0, m.rng.range(1.0, 2.0));
                m.catacombs.burst = 0.0;
            }
            _ => {}
        }
    }
    next
}

/// Spiderlings shaken off the wolves' backs scatter and fight (born from
/// the mother: nothing to eat on them).
pub(super) fn hatch(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    terrain: Res<Terrain>,
    mut mobs: Query<(Entity, &mut Mob, &Transform)>,
) {
    for (e, mut m, tf) in &mut mobs {
        let n = std::mem::take(&mut m.catacombs.shed);
        let feet = tf.translation.truncate() - Vec2::Y * m.spec.foot;
        for i in 0..n {
            let x = feet.x + (i as f32 - (n as f32 - 1.0) / 2.0) * 18.0 + m.rng.range(-5.0, 5.0);
            let y = terrain.surface(Vec2::new(x, feet.y + 3.0 * STEP_UP), 80.0).unwrap_or(feet.y);
            let at = Vec2::new(x, y + MobKind::Spiderling.spec().foot);
            mob_art::spawn(&mut commands, &mut meshes, &mut materials, MobKind::Spiderling, at, Some(e));
        }
    }
}

// --- Bonestrider ---

fn strider(m: &mut Mob, pos: Vec2, sees: bool, touching: bool, w: &World, bites: &mut Vec<(Vec2, i32, MobKind)>) -> Vec2 {
    let spec = m.spec;
    let dt = w.dt;
    let to = w.ppos - pos;
    let side = if to.x >= 0.0 { 1.0 } else { -1.0 };
    let lost = m.catacombs.lost.iter().filter(|&&l| l).count() as f32;
    let pace = (1.0 - LIMP * lost).max(0.4);
    let lowest = STILT - LOWEST;
    let mut want = 0.0;
    m.state = match m.state {
        // Struck: scrambles off on its stilts.
        State::Hurt(s) if s < 0.35 => {
            want = -side * spec.speed * 1.8 * pace;
            State::Hurt(s + dt)
        }
        State::Hurt(_) => State::Recover(spec.recover),
        State::Windup(s) if s < spec.windup => {
            m.catacombs.drop += (-GATHER - m.catacombs.drop) * ease(6.0, dt);
            State::Windup(s + dt)
        }
        State::Windup(_) => State::Attack(0.0),
        State::Attack(_) if touching => {
            bites.push((pos, spec.damage, m.kind));
            State::Recover(0.0)
        }
        State::Attack(s) if m.catacombs.drop >= lowest || s > spec.attack_time => {
            m.catacombs.drop = m.catacombs.drop.min(lowest);
            State::Recover(0.0)
        }
        State::Attack(s) => {
            m.catacombs.drop = (m.catacombs.drop + spec.attack_speed * dt).min(lowest);
            State::Attack(s + dt)
        }
        // Down low a moment, then climbs back up.
        State::Recover(s) if s < spec.recover => State::Recover(s + dt),
        State::Recover(s) => {
            m.catacombs.drop -= RISE * dt;
            if m.catacombs.drop <= 0.0 {
                m.catacombs.drop = 0.0;
                m.catacombs.rest = STRIDE_REST;
                State::Idle
            } else {
                State::Recover(s)
            }
        }
        State::Idle if sees && to.x.abs() < OVER && to.y < 0.0 && m.catacombs.rest <= 0.0 && m.catacombs.drop <= 0.0 => {
            m.plan = w.ppos;
            State::Windup(0.0)
        }
        State::Idle => {
            m.catacombs.drop = (m.catacombs.drop - RISE * dt).max(0.0);
            if let Some(dir) = startled(m, side) {
                want = dir * spec.speed * 1.5 * pace;
            } else if sees {
                // Over the chameleon: right above it, lagging behind.
                want = (to.x * 2.5).clamp(-1.0, 1.0) * spec.speed * 1.7 * pace;
                m.facing = side;
            } else {
                want = wander(m, pos, dt) * pace;
            }
            if !sees && want.abs() > 1.0 {
                m.facing = want.signum();
            }
            State::Idle
        }
        dead @ State::Dead { .. } => dead,
    };

    // Along over the ground: stepping over gaps and rubble the legs can bridge.
    m.vel.x += (want - m.vel.x) * ease(5.0, dt);
    let walking = matches!(m.state, State::Idle | State::Hurt(_));
    let mut x = pos.x;
    let mut ground = ground_under(w.terrain, x, m.catacombs.ground + STILT).unwrap_or(m.catacombs.ground);
    if walking && m.vel.x.abs() > 0.5 {
        let nx = x + m.vel.x * dt;
        match ground_under(w.terrain, nx, m.catacombs.ground + STILT) {
            Some(g) if g - m.catacombs.ground < MAX_RISE
                && (nx - m.home.x).abs() <= spec.leash
                && !w.terrain.box_hits(Vec2::new(nx, g + STILT), Vec2::new(12.0, 8.0)) =>
            {
                x = nx;
                ground = g;
            }
            _ => {
                m.vel.x = 0.0;
                if m.state == State::Idle {
                    m.wander = (-m.wander.0, m.rng.range(1.0, 2.0));
                }
            }
        }
    } else if !walking {
        m.vel.x = 0.0;
    }
    m.catacombs.ground += (ground - m.catacombs.ground) * ease(10.0, dt);
    let bob = if m.state == State::Idle { (w.t * 2.1 + m.seed).sin() * 3.0 } else { 0.0 };
    let body = Vec2::new(x, m.catacombs.ground + STILT - m.catacombs.drop + bob);
    step_feet(m, w.terrain, body, dt);
    m.stride += dt * m.vel.x.abs() * 0.12;
    body
}

/// The ground under a strider at `x` (`from`: its body's height): the
/// highest footing a little either side, so it bridges narrow gaps.
fn ground_under(terrain: &Terrain, x: f32, from: f32) -> Option<f32> {
    [-SPAN, 0.0, SPAN]
        .into_iter()
        .filter_map(|dx| terrain.surface(Vec2::new(x + dx, from), STILT + 140.0))
        .reduce(f32::max)
}

/// Where a foot reaching `reach` from the body could stand (pulled in when
/// there is nothing out there).
fn foothold(terrain: &Terrain, body: Vec2, reach: f32) -> Option<Vec2> {
    [1.0, 0.8, 0.6, 0.4].into_iter().find_map(|k| {
        let x = body.x + reach * k;
        let y = terrain.surface(Vec2::new(x, body.y), STILT + 140.0)?;
        (y < body.y - 10.0).then_some(Vec2::new(x, y))
    })
}

/// A strider's hip for leg `i`, its body at `body`.
pub(crate) fn hip(body: Vec2, i: usize) -> Vec2 {
    body + Vec2::new(HIPS[i], -1.0)
}

/// Feet stay planted until they lag too far behind, then step (a few at a
/// time, never two neighbors at once) to fresh footing ahead.
fn step_feet(m: &mut Mob, terrain: &Terrain, body: Vec2, dt: f32) {
    let lead = m.vel.x * 0.25;
    let c = &mut m.catacombs;
    let mut lifted = c.lift.iter().filter(|&&k| k > 0.0).count();
    for i in 0..LEGS {
        if c.lost[i] {
            continue;
        }
        if c.lift[i] > 0.0 {
            let k = (c.lift[i] + dt / STEP_TIME).min(1.0);
            c.feet[i] = c.from[i].lerp(c.to[i], k) + Vec2::Y * (PI * k).sin() * STEP_LIFT;
            c.lift[i] = if k >= 1.0 { 0.0 } else { k };
            continue;
        }
        let Some(target) = foothold(terrain, body, REACH[i] + lead) else { continue };
        let lagging = (c.feet[i].x - target.x).abs() > STEP_TRIGGER || c.feet[i].distance(hip(body, i)) > THIGH + SHIN - 4.0;
        let side = i / 4 * 4;
        let busy = (i > side && c.lift[i - 1] > 0.0) || (i + 1 < side + 4 && c.lift[i + 1] > 0.0);
        if lagging && lifted < 3 && !busy {
            c.lift[i] = f32::EPSILON;
            c.from[i] = c.feet[i];
            c.to[i] = target;
            lifted += 1;
        }
    }
}
