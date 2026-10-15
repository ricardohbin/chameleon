//! Ordinary enemies ("mobs"): the vermin living around each boss.
//!
//! Every slice of the world sketch belongs to a boss: the one whose marker is
//! inside it, else the nearest. Its mobs are that boss's ecosystem:
//! - the Gutter King's (cockroach) gutter: roachlings that patrol and charge,
//!   big sewer roaches that charge through hits, carrion-fly swarms that buzz
//!   and nip in erratic orbits, and fleas that leap at the chameleon in arcs
//!   (they bleed red: someone else's blood);
//! - the Venom Mother's (wasp) hive: drones that hover and dive, larvae that
//!   crawl and lunge, soldiers that stand guard and spit venom, and paper
//!   nests hanging from ceilings that release drones.
//!
//! The slice with the player spawn is left empty (a calm start), and so are
//! both bosses' arenas. Placement is deterministic (seeded per slice name) and
//! the count scales with the slice size. Nobody strays more than `leash` from
//! where it was placed. A few HP, killed by the claw slash. Everyone comes back
//! when the chameleon respawns, and the lot is replaced when the sketch reloads.
//!
//! Bodies, animation and gibs live in `mob_art.rs`.

use std::{collections::HashMap, f32::consts::PI};

use bevy::prelude::*;

mod rat;

use crate::{
    boss::{self, Arena, BossKind},
    camera::{self, HitStop, Shake},
    chameleon::{self, ChameleonAnim},
    spit::{self, SpitHit},
    gore::{Bleed, Fluid, Paint},
    level::{Platform, WorldReloaded},
    mob_art,
    player::{self, Player, Respawned},
    shapes::{ellipse_pts, fill, Rng},
    terrain::{Style, Terrain, PX, STEP_UP},
};

/// No mobs this close to the player spawn.
const SPAWN_CLEAR: f32 = 700.0;
/// Kept free around the Gutter King's arena (left..right), more than any leash.
const ROACH_ARENA_MARGIN: f32 = 400.0;
/// Kept free around the Venom Mother's home (more than her aggro range).
const WASP_CLEAR: f32 = 800.0;
/// Sketch pixels of slice per mob (a swarm counts as one), and the count limits per slice.
const CELLS_PER_MOB: f32 = 2600.0;
const MIN_PER_AREA: usize = 2;
const MAX_PER_AREA: usize = 16;
/// Minimum distance between two mobs (or swarms).
const MIN_GAP: f32 = 150.0;
/// Room a drone needs around its spot.
const FLY_ROOM: f32 = 6.0 * PX;
/// Veterans' shock lasts this share of a green mob's.
const VETERAN_REFLEX: f32 = 0.3;
/// A mob hurt this near (px) rallies the veterans around it at once.
const RALLY_RANGE: f32 = 420.0;
/// Share of mobs that snap out of the shock much sooner than the rest.
/// Panic from being touched in shock: extra seconds of shock, up to this much in all.
const PANIC_EXTRA: f32 = 0.7;
const PANIC_CAP: f32 = 3.0;
const QUICK_SHARE: f32 = 0.15;
/// Busy with its own affairs, a mob only spots the chameleon this close (as a share of its sense).
const UNAWARE_RANGE: f32 = 0.75;
/// An aware mob that hasn't seen the chameleon for this long is caught off guard again.
const FORGET: f32 = 4.0;
/// Flies per swarm.
const SWARM: usize = 4;
/// A spider rests this far below its ceiling, and its thread reaches this far at most.
pub const THREAD_REST: f32 = 40.0;
const THREAD_MAX: f32 = 420.0;
const GRAVITY: f32 = 1400.0;
/// A flea's leap takes about this long.
const LEAP_TIME: f32 = 0.55;
/// Venom spat by soldiers.
const GLOB_GRAVITY: f32 = 520.0;
const GLOB_COLOR: Color = Color::srgb(0.20, 0.56, 0.08);
/// Drones a nest keeps out at once, and how often it releases one.
const BROOD: usize = 2;
const BROOD_EVERY: (f32, f32) = (3.5, 5.5);

pub struct MobPlugin;

impl Plugin for MobPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, load_glob).add_systems(
            Update,
            (rebuild, slash_mobs, think, nests, move_globs, mob_art::dismember, mob_art::animate, flash_mobs)
                .chain()
                .after(player::move_player)
                .after(chameleon::update_anim_state)
                .after(spit::move_spit),
        );
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MobKind {
    /// Small cockroach: patrols, rears up and charges.
    Roachling,
    /// Big sewer cockroach: slower, tougher, charges through hits.
    SewerRoach,
    /// Carrion fly, in swarms: buzzes about, orbits the chameleon and nips.
    Fly,
    /// Flea: hops about, crouches and leaps at the chameleon.
    Flea,
    /// Small wasp: hovers, aims and dives.
    Drone,
    /// Wasp larva: crawls, rears and lunges.
    Grub,
    /// Wasp soldier: stands guard and spits venom in arcs.
    Soldier,
    /// Paper nest hanging from a ceiling: releases drones.
    Nest,
    /// Gutter rat, in packs: runs from the chameleon's face, rushes its back.
    Rat,
    /// Spider on a silk thread from a ceiling: drops on the chameleon, climbs back.
    Spider,
}

impl MobKind {
    pub fn spec(self) -> &'static MobSpec {
        match self {
            MobKind::Roachling => &ROACHLING,
            MobKind::SewerRoach => &SEWER_ROACH,
            MobKind::Fly => &FLY,
            MobKind::Flea => &FLEA,
            MobKind::Drone => &DRONE,
            MobKind::Grub => &GRUB,
            MobKind::Soldier => &SOLDIER,
            MobKind::Nest => &NEST,
            MobKind::Rat => &RAT,
            MobKind::Spider => &SPIDER,
        }
    }
}

/// How a mob gets around (and where it can be placed).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Moves {
    /// Follows the ground; charges along it.
    Walk,
    /// Flies steadily; dives.
    Fly,
    /// Flies erratically, in groups; harasses.
    Swarm,
    /// Jumps in arcs.
    Hop,
    /// Stands still on the ground; shoots.
    Perch,
    /// Hangs from a ceiling; spawns others.
    Hang,
    /// Runs along the ground in a pack: away from the chameleon's face, at its back.
    Pack,
    /// Hangs from a ceiling on a thread; drops and climbs.
    Dangle,
}

pub struct MobSpec {
    pub moves: Moves,
    pub hp: i32,
    /// Size relative to its drawing.
    pub scale: f32,
    /// Hurtbox radius around its center.
    pub radius: f32,
    /// Center height above the ground (standing; the corpse lies lower).
    pub foot: f32,
    pub blood: Fluid,
    /// Patrol / hover / idle hop speed.
    pub speed: f32,
    /// Notices the chameleon within this distance.
    pub sense: f32,
    pub windup: f32,
    /// Charge / dive / chase / max leap speed (soldiers: venom speed).
    pub attack_speed: f32,
    pub attack_time: f32,
    pub recover: f32,
    /// Never further than this from its spot.
    pub leash: f32,
    pub damage: i32,
    /// Its attack isn't interrupted by hits.
    pub stubborn: bool,
    /// How slowly it reacts to the chameleon turning up (seconds in shock,
    /// before the mob-to-mob spread): bigger is slower.
    pub initiative: f32,
    /// A seasoned fighter: keeps its head when surprised (reacts much sooner,
    /// never panics or bolts) and rallies when mobs near it are attacked.
    pub veteran: bool,
    /// Shadow health its remains give when eaten (`feeding.rs`), in health ticks.
    /// Rule of thumb: a quarter tick per hit point, more for what makes it
    /// dangerous (hard to hit, packs that hit your back, ranged venom, stubborn
    /// charges). Drones born from a nest give nothing (no farming the nest).
    pub food: f32,
}

impl MobSpec {
    pub fn flies(&self) -> bool {
        matches!(self.moves, Moves::Fly | Moves::Swarm)
    }
}

static ROACHLING: MobSpec = MobSpec {
    moves: Moves::Walk,
    hp: 2,
    scale: 1.5,
    radius: 17.0,
    foot: 6.8 * 1.5,
    blood: Fluid::Yellow,
    speed: 70.0,
    sense: 300.0,
    windup: 0.35,
    attack_speed: 430.0,
    attack_time: 0.55,
    recover: 1.2,
    leash: 260.0,
    damage: 1,
    stubborn: false,
    initiative: 1.0,
    veteran: false,
    // 2 hp, a telegraphed charge.
    food: 0.5,
};
static SEWER_ROACH: MobSpec = MobSpec {
    hp: 4,
    scale: 2.2,
    radius: 24.0,
    foot: 6.8 * 2.2,
    speed: 55.0,
    sense: 340.0,
    windup: 0.6,
    attack_speed: 520.0,
    attack_time: 0.7,
    recover: 1.5,
    leash: 300.0,
    stubborn: true,
    initiative: 1.4,
    veteran: true,
    // 4 hp, charges through hits.
    food: 1.25,
    ..ROACHLING
};
static FLY: MobSpec = MobSpec {
    moves: Moves::Swarm,
    hp: 1,
    scale: 1.6,
    radius: 11.0,
    foot: 4.0,
    blood: Fluid::Yellow,
    speed: 120.0,
    sense: 320.0,
    windup: 0.0,
    attack_speed: 280.0,
    attack_time: 5.0,
    recover: 1.0,
    leash: 380.0,
    damage: 1,
    stubborn: false,
    initiative: 1.5,
    veteran: false,
    // 1 hp, a swarm of 4 makes a tick.
    food: 0.25,
};
static FLEA: MobSpec = MobSpec {
    moves: Moves::Hop,
    hp: 2,
    scale: 1.7,
    radius: 14.0,
    foot: 6.5 * 1.7,
    // Fleas burst with what they sucked.
    blood: Fluid::Red,
    speed: 110.0,
    sense: 380.0,
    windup: 0.4,
    attack_speed: 460.0,
    attack_time: 2.0,
    recover: 0.8,
    leash: 320.0,
    damage: 1,
    stubborn: false,
    initiative: 0.7,
    veteran: false,
    // 2 hp, erratic leaps.
    food: 0.5,
};
static DRONE: MobSpec = MobSpec {
    moves: Moves::Fly,
    hp: 2,
    scale: 1.4,
    radius: 17.0,
    foot: 6.0,
    blood: Fluid::Green,
    speed: 160.0,
    sense: 420.0,
    windup: 0.5,
    attack_speed: 500.0,
    attack_time: 0.7,
    recover: 1.4,
    leash: 300.0,
    damage: 1,
    stubborn: false,
    initiative: 0.8,
    veteran: false,
    // 2 hp, dives.
    food: 0.5,
};
static GRUB: MobSpec = MobSpec {
    moves: Moves::Walk,
    hp: 3,
    scale: 1.6,
    radius: 17.0,
    foot: 4.4 * 1.6,
    blood: Fluid::Green,
    speed: 28.0,
    sense: 200.0,
    windup: 0.5,
    attack_speed: 320.0,
    attack_time: 0.3,
    recover: 1.0,
    leash: 160.0,
    damage: 1,
    stubborn: false,
    initiative: 1.8,
    veteran: false,
    // 3 hp but slow and easy.
    food: 0.5,
};
static SOLDIER: MobSpec = MobSpec {
    moves: Moves::Perch,
    hp: 3,
    scale: 1.8,
    radius: 19.0,
    foot: 12.5 * 1.8,
    blood: Fluid::Green,
    speed: 0.0,
    sense: 560.0,
    windup: 0.7,
    attack_speed: 430.0,
    attack_time: 0.0,
    recover: 1.8,
    leash: 0.0,
    damage: 1,
    stubborn: true,
    initiative: 1.1,
    veteran: true,
    // 3 hp, venom from afar, stubborn.
    food: 1.0,
};
static NEST: MobSpec = MobSpec {
    moves: Moves::Hang,
    hp: 4,
    scale: 1.8,
    radius: 20.0,
    foot: 16.0,
    blood: Fluid::Green,
    speed: 0.0,
    sense: 650.0,
    windup: 0.0,
    attack_speed: 0.0,
    attack_time: 0.4,
    recover: 0.0,
    leash: 0.0,
    damage: 0,
    stubborn: true,
    initiative: 0.0,
    veteran: false,
    // 4 hp, harmless itself, full of larvae.
    food: 0.75,
};

static RAT: MobSpec = MobSpec {
    moves: Moves::Pack,
    hp: 2,
    scale: 1.6,
    radius: 15.0,
    foot: 5.5 * 1.6,
    blood: Fluid::Red,
    speed: 90.0,
    sense: 450.0,
    // Cornered and stared at: rears up this long before it fights.
    windup: 0.4,
    attack_speed: 400.0,
    attack_time: 1.2,
    recover: 0.9,
    leash: 380.0,
    damage: 1,
    stubborn: false,
    initiative: 0.6,
    veteran: false,
    // 2 hp, packs of 3 ambushing your back.
    food: 0.5,
};
static SPIDER: MobSpec = MobSpec {
    moves: Moves::Dangle,
    hp: 2,
    scale: 1.6,
    radius: 15.0,
    foot: 8.0,
    blood: Fluid::Green,
    speed: 0.0,
    sense: 360.0,
    windup: 0.25,
    attack_speed: 700.0,
    attack_time: 1.5,
    recover: 0.6,
    leash: 0.0,
    damage: 1,
    stubborn: false,
    initiative: 0.4,
    veteran: false,
    // 2 hp, ambush from above.
    food: 0.5,
};

#[derive(Clone, Copy, PartialEq)]
pub enum State {
    Idle,
    Windup(f32),
    /// Charging, diving, chasing, leaping, (nests) releasing a drone.
    Attack(f32),
    Recover(f32),
    Hurt(f32),
    Dead { t: f32, landed: bool },
}

/// What a mob makes of the chameleon: caught by surprise, it freezes up,
/// flees or scatters for a moment before it starts fighting.
#[derive(Clone, Copy, PartialEq)]
enum Notice {
    /// Hasn't seen it (or lost it long ago).
    Unaware,
    /// Just saw it: no attacks yet, `left` seconds of the given reaction.
    Shock { left: f32, how: Shock },
    Aware,
}

#[derive(Clone, Copy, PartialEq)]
enum Shock {
    Freeze,
    Flee,
    /// Touched while in shock: bolts from the chameleon, harder than a flee.
    Panic,
    /// Bolts off at this angle.
    Scatter(f32),
}

#[derive(Component)]
pub struct Mob {
    pub kind: MobKind,
    pub spec: &'static MobSpec,
    pub hp: i32,
    /// Where it was placed (its center).
    pub home: Vec2,
    pub vel: Vec2,
    pub facing: f32,
    pub state: State,
    /// Time offset for wobbles.
    pub seed: f32,
    /// Walk-cycle phase.
    pub stride: f32,
    /// Hoppers: off the ground.
    pub airborne: bool,
    /// Just died: tear off some parts this frame.
    pub dismember: bool,
    /// The nest that released it.
    parent: Option<Entity>,
    /// Patrol: walking direction (0 = pausing) and time left on it.
    wander: (f32, f32),
    /// Nests: time to the next drone.
    cooldown: f32,
    /// Swarms: angle around the chameleon.
    orbit: f32,
    /// 0..1: how stuck in cobwebs (`webs.rs`), set every frame; slows its time.
    pub webbed: f32,
    /// Last slash that hit it (one hit per slash).
    hit_by: u32,
    /// 1 = white flash after a hit, fades to 0.
    pub flash: f32,
    /// Its own materials (to flash) with their base colors.
    mats: Vec<(Handle<ColorMaterial>, Color)>,
    rng: Rng,
    notice: Notice,
    /// Where it saw the chameleon when it began preparing its attack.
    plan: Vec2,
    /// Seconds without seeing the chameleon, once aware.
    lost: f32,
    /// Rats: their state of mind (`rat.rs`).
    rat: rat::Mind,
}

impl Mob {
    pub fn new(kind: MobKind, home: Vec2, mats: Vec<(Handle<ColorMaterial>, Color)>, parent: Option<Entity>) -> Self {
        let mut rng = Rng(((home.x as i32).wrapping_mul(73_856_093) ^ (home.y as i32).wrapping_mul(19_349_663)) as u32 | 1);
        let facing = if rng.chance(0.5) { 1.0 } else { -1.0 };
        Self {
            kind,
            spec: kind.spec(),
            hp: kind.spec().hp,
            home,
            vel: Vec2::ZERO,
            facing,
            state: State::Idle,
            seed: rng.range(0.0, 100.0),
            stride: 0.0,
            airborne: false,
            dismember: false,
            parent,
            wander: (0.0, rng.range(0.2, 1.5)),
            cooldown: rng.range(0.5, 2.0),
            orbit: rng.range(0.0, 2.0 * PI),
            webbed: 0.0,
            hit_by: 0,
            flash: 0.0,
            mats,
            rng,
            // Drones from a nest are born angry.
            plan: home,
            notice: if parent.is_some() { Notice::Aware } else { Notice::Unaware },
            lost: 0.0,
            rat: rat::Mind::default(),
        }
    }

    /// Shadow health its remains give.
    pub fn food(&self) -> f32 {
        if self.parent.is_some() { 0.0 } else { self.spec.food }
    }

    /// 0..1 through the attack telegraph.
    pub fn windup(&self) -> f32 {
        match self.state {
            State::Windup(s) => (s / self.spec.windup).min(1.0),
            _ => 0.0,
        }
    }
}

/// Body parts torn off mobs and venom they spat: cleared when they come back.
#[derive(Component)]
pub struct MobStuff;

/// A venom glob spat by a soldier.
#[derive(Component)]
struct Glob {
    vel: Vec2,
    life: f32,
}

#[derive(Resource)]
struct GlobAssets {
    mesh: Handle<Mesh>,
    mat: Handle<ColorMaterial>,
}

fn load_glob(mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>, mut materials: ResMut<Assets<ColorMaterial>>) {
    let glob = fill(&ellipse_pts(5.0, 4.0, 16), |p| 1.0 - 0.35 * (p - Vec2::new(1.5, 1.2)).length() / 6.0);
    commands.insert_resource(GlobAssets { mesh: meshes.add(glob), mat: materials.add(GLOB_COLOR) });
}

// --- Placement ---

/// A region nobody is placed in.
enum Zone {
    Rect(Rect),
    Circle(Vec2, f32),
}

impl Zone {
    fn contains(&self, p: Vec2) -> bool {
        match *self {
            Zone::Rect(r) => r.contains(p),
            Zone::Circle(c, r) => p.distance(c) < r,
        }
    }
}

/// Where the mobs go: (kind, center) for every slice, themed after its boss.
fn plan(terrain: &Terrain) -> Vec<(MobKind, Vec2)> {
    let bosses: Vec<(BossKind, Vec2)> = [(BossKind::Roach, terrain.roach), (BossKind::Wasp, terrain.wasp)]
        .into_iter()
        .filter_map(|(k, at)| Some((k, at?)))
        .collect();
    let zones: Vec<Zone> = bosses
        .iter()
        .flat_map(|&(kind, _)| {
            let a = Arena::find(kind, terrain);
            match kind {
                BossKind::Roach => vec![Zone::Rect(Rect::new(
                    a.left - ROACH_ARENA_MARGIN,
                    a.floor - 250.0,
                    a.right + ROACH_ARENA_MARGIN,
                    a.floor + 600.0,
                ))],
                // Her home and the whole chamber she fights in.
                BossKind::Wasp => vec![
                    Zone::Circle(a.home, WASP_CLEAR),
                    Zone::Rect(Rect::new(a.left - 400.0, a.floor - 250.0, a.right + 400.0, a.home.y.max(a.floor) + 500.0)),
                ],
            }
        })
        .collect();
    // Nothing lives (or flies) in the water.
    let flooded = |p: Vec2| terrain.features.iter().any(|f| crate::water::Kind::is_water(&f.name) && f.rect.contains(p));
    let banned = |p: Vec2| p.distance(terrain.spawn) < SPAWN_CLEAR || zones.iter().any(|z| z.contains(p)) || flooded(p);

    let mut placed: Vec<(MobKind, Vec2)> = Vec::new();
    for area in &terrain.areas {
        if area.rect.contains(terrain.spawn) {
            continue;
        }
        let off = |p: Vec2| (p - p.clamp(area.rect.min, area.rect.max)).length();
        let owner = bosses
            .iter()
            .find(|(_, at)| area.rect.contains(*at))
            .or_else(|| bosses.iter().min_by(|a, b| off(a.1).total_cmp(&off(b.1))));
        let Some(&(boss, _)) = owner else { continue };
        let mut rng = Rng(seed_from(&area.name));
        let cells = area.rect.size().element_product() / (PX * PX);
        let want = ((cells / CELLS_PER_MOB).round() as usize).clamp(MIN_PER_AREA, MAX_PER_AREA);
        // Each kind gets its share of the slots (in a shuffled order), so the
        // easy spots (open air) don't crowd out the rare ones (ceilings).
        let mut slots: Vec<MobKind> = Vec::new();
        for (kind, share) in menu(boss, area.style) {
            slots.extend(std::iter::repeat_n(kind, ((want as f32 * share).round() as usize).max(1)));
        }
        for i in (1..slots.len()).rev() {
            slots.swap(i, rng.below(i + 1));
        }
        let mut got: Vec<MobKind> = Vec::new();
        for kind in slots.into_iter().take(want) {
            let found = (0..120).find_map(|_| {
                let p = Vec2::new(rng.range(area.rect.min.x, area.rect.max.x), rng.range(area.rect.min.y, area.rect.max.y));
                let at = site(terrain, kind, p, area.rect)?;
                (!banned(at) && placed.iter().all(|(_, q)| q.distance(at) >= MIN_GAP)).then_some(at)
            });
            let Some(at) = found else { continue };
            if kind == MobKind::Fly {
                // A swarm around the spot.
                for i in 0..SWARM {
                    let a = i as f32 / SWARM as f32 * 2.0 * PI + rng.range(-0.4, 0.4);
                    let q = at + Vec2::from_angle(a) * rng.range(14.0, 34.0);
                    placed.push((kind, if terrain.clearance(q) >= 2.0 * PX { q } else { at }));
                }
            } else if kind == MobKind::Rat {
                // A pack, side by side on the ground.
                for dx in [0.0, -36.0, 40.0] {
                    let q = Vec2::new(at.x + dx, at.y + STEP_UP);
                    let rest = terrain.surface(q, 3.0 * STEP_UP).map(|y| Vec2::new(q.x, y));
                    let ok = rest.filter(|&f| fits(terrain, f, kind.spec()));
                    placed.push((kind, ok.map_or(at, |f| f + Vec2::Y * kind.spec().foot)));
                }
            } else {
                placed.push((kind, at));
            }
            got.push(kind);
        }
        let mut counts: Vec<(MobKind, usize)> = Vec::new();
        for k in got.iter() {
            match counts.iter_mut().find(|(c, _)| c == k) {
                Some((_, n)) => *n += 1,
                None => counts.push((*k, 1)),
            }
        }
        info!("mobs: {}/{want} for {boss:?} in \"{}\": {counts:?}", got.len(), area.name);
    }
    placed
}

/// The ecosystem of a region of `boss`: kinds and their share of the mobs.
pub(crate) fn menu(boss: BossKind, style: Style) -> Vec<(MobKind, f32)> {
    match boss {
        BossKind::Roach => {
            // Sewers breed the big ones.
            let big = if style == Style::Sewer { 0.2 } else { 0.08 };
            vec![
                (MobKind::SewerRoach, big),
                (MobKind::Roachling, 0.3 - big),
                (MobKind::Fly, 0.25),
                (MobKind::Flea, 0.25),
                (MobKind::Rat, 0.2),
            ]
        }
        BossKind::Wasp => vec![
            (MobKind::Drone, 0.25),
            (MobKind::Grub, 0.2),
            (MobKind::Soldier, 0.2),
            (MobKind::Nest, 0.15),
            (MobKind::Spider, 0.2),
        ],
    }
}

/// A spot near `p` where this kind can live (its center), if any.
fn site(terrain: &Terrain, kind: MobKind, p: Vec2, rect: Rect) -> Option<Vec2> {
    let spec = kind.spec();
    if terrain.solid_at(p) {
        return None;
    }
    match spec.moves {
        Moves::Fly | Moves::Swarm => {
            let open = terrain.clearance(p) >= FLY_ROOM;
            // Over some ground, not out in the void.
            (open && terrain.surface(p, 500.0).is_some()).then_some(p)
        }
        Moves::Hang | Moves::Dangle => {
            // Up to the ceiling above `p`; it must hang over open air.
            let mut c = terrain.to_cell(p);
            for _ in 0..40 {
                c.y -= 1;
                if terrain.solid(c) {
                    let ceiling = terrain.cell_min(c).y;
                    let drop = if spec.moves == Moves::Hang { mob_art::NEST_STALK * spec.scale } else { THREAD_REST };
                    let at = Vec2::new(p.x, ceiling - drop);
                    let room = terrain.clearance(at) >= 2.5 * PX && !terrain.box_hits(at - Vec2::Y * 70.0, Vec2::new(24.0, 40.0));
                    return (rect.contains(at) && room && terrain.surface(at, 900.0).is_some()).then_some(at);
                }
            }
            None
        }
        Moves::Walk | Moves::Hop | Moves::Perch | Moves::Pack => {
            let feet = Vec2::new(p.x, terrain.surface(p, rect.height())?);
            let (left, right) = terrain.flat_span(feet, 24.0);
            (rect.contains(feet) && fits(terrain, feet, spec) && right - left >= 12.0 * PX).then_some(feet + Vec2::Y * spec.foot)
        }
    }
}

/// A walker standing at `feet` has room for its body. The box starts a step
/// up, so the stair-stepped ground of a slope doesn't count.
fn fits(terrain: &Terrain, feet: Vec2, spec: &MobSpec) -> bool {
    let half = Vec2::new(spec.radius * 0.6, spec.foot * 0.6);
    !terrain.box_hits(feet + Vec2::Y * (STEP_UP + half.y), half)
}

/// FNV-1a: a stable seed per slice name.
fn seed_from(name: &str) -> u32 {
    name.bytes().fold(0x811C_9DC5u32, |h, b| (h ^ b as u32).wrapping_mul(0x0100_0193)) | 1
}

/// Spawn the mobs at the start, bring everyone back when the chameleon
/// respawns, and replace them when the sketch reloads (a frame later, once
/// the new `Terrain` is in).
fn rebuild(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    terrain: Res<Terrain>,
    mut respawned: MessageReader<Respawned>,
    mut reloaded: MessageReader<WorldReloaded>,
    old: Query<Entity, Or<(With<Mob>, With<MobStuff>)>>,
    mut started: Local<bool>,
    mut pending: Local<bool>,
) {
    let again = respawned.read().count() > 0;
    if reloaded.read().count() > 0 {
        *pending = true;
        return;
    }
    if *started && !again && !*pending {
        return;
    }
    *started = true;
    *pending = false;
    for e in &old {
        commands.entity(e).despawn();
    }
    for (kind, at) in plan(&terrain) {
        mob_art::spawn(&mut commands, &mut meshes, &mut materials, kind, at, None);
    }
}

// --- Fighting ---

fn slash_mobs(
    mut shake: ResMut<Shake>,
    mut stop: ResMut<HitStop>,
    mut bleed: MessageWriter<Bleed>,
    mut paint: MessageWriter<Paint>,
    mut spat: MessageReader<SpitHit>,
    player: Single<(&Player, &Transform, &ChameleonAnim)>,
    mut mobs: Query<(Entity, &mut Mob, &Transform)>,
) {
    let (p, ptf, anim) = *player;
    let ppos = ptf.translation.truncate();
    // (mob, direction of the blow, slashed rather than spat on)
    let mut struck: Vec<(Entity, Vec2, bool)> = spat.read().map(|h| (h.target, h.dir, false)).collect();
    if anim.striking() {
        let aim = anim.slash_dir();
        let center = chameleon::slash_center(ppos, p.facing);
        for (e, m, tf) in &mobs {
            let pos = tf.translation.truncate();
            let radius = m.spec.radius;
            let in_front = (pos - center).dot(aim) > -radius * 0.5;
            let in_reach = pos.distance(center) < chameleon::SLASH_REACH + radius;
            if !matches!(m.state, State::Dead { .. }) && m.hit_by != anim.slash_id && in_front && in_reach {
                struck.push((e, aim, true));
            }
        }
    }
    // Where mobs were hurt: the veterans around them are on guard at once.
    let mut rally: Vec<Vec2> = Vec::new();
    for (e, aim, slashed) in struck {
        let Ok((_, mut m, tf)) = mobs.get_mut(e) else { continue };
        if matches!(m.state, State::Dead { .. }) {
            continue;
        }
        let pos = tf.translation.truncate();
        let radius = m.spec.radius;
        if slashed {
            m.hit_by = anim.slash_id;
        }
        m.hp -= 1;
        m.flash = 1.0;
        m.notice = Notice::Aware;
        rally.push(tf.translation.truncate());
        let fluid = m.spec.blood;
        let dir = (aim + Vec2::Y * 0.3).normalize();
        // Knocked away sideways from the chameleon, whichever way the blow came.
        let side = if pos.x < ppos.x { -1.0 } else { 1.0 };
        // Small things spill less.
        let k = (radius / 17.0).min(1.3);
        let amount = |n: f32| (n * k) as u32;
        let gush = Bleed { pos, dir: aim + Vec2::Y * 0.35, spread: 0.8, speed: 520.0, amount: amount(26.0), fluid, sticky: true };
        bleed.write(gush);
        if slashed {
            bleed.write(Bleed { dir: ppos + Vec2::Y * 8.0 - pos, spread: 0.4, speed: 360.0, amount: amount(14.0), ..gush });
            paint.write(Paint { amount: 2, fluid });
        }

        if m.hp <= 0 {
            // Burst open and get flipped over (no boss-style spectacle).
            bleed.write(Bleed { dir: Vec2::Y, spread: PI, speed: 600.0, amount: amount(70.0), ..gush });
            paint.write(Paint { amount: 3, fluid });
            m.state = State::Dead { t: 0.0, landed: false };
            m.vel = if m.spec.moves == Moves::Hang { Vec2::ZERO } else { Vec2::new(side * 220.0, 320.0) };
            m.dismember = true;
            shake.add(0.3);
            stop.add(0.08);
        } else {
            let busy = matches!(m.state, State::Attack(_) | State::Windup(_));
            if !(busy && m.spec.stubborn) {
                m.vel = match m.spec.moves {
                    Moves::Fly | Moves::Swarm => dir * 300.0,
                    Moves::Walk | Moves::Pack => Vec2::new(side * 280.0, 0.0),
                    Moves::Hop => {
                        m.airborne = true;
                        Vec2::new(side * 260.0, 220.0)
                    }
                    Moves::Perch | Moves::Hang | Moves::Dangle => Vec2::ZERO,
                };
                m.state = State::Hurt(0.0);
            }
            shake.add(0.15);
            stop.add(0.05);
        }
    }
    if !rally.is_empty() {
        for (_, mut o, tf) in &mut mobs {
            let q = tf.translation.truncate();
            if o.spec.veteran && !matches!(o.state, State::Dead { .. }) && o.notice != Notice::Aware && rally.iter().any(|r| r.distance(q) < RALLY_RANGE) {
                o.notice = Notice::Aware;
            }
        }
    }
}

/// What a mob did this frame that touches the outside world.
#[derive(Default)]
struct Deeds {
    /// It bit/stung the chameleon from here.
    bites: Vec<(Vec2, i32)>,
    /// Venom spat: (from, velocity).
    spits: Vec<(Vec2, Vec2)>,
}

fn think(
    mut commands: Commands,
    time: Res<Time>,
    mut shake: ResMut<Shake>,
    mut stop: ResMut<HitStop>,
    mut bleed: MessageWriter<Bleed>,
    glob: Res<GlobAssets>,
    player: Single<(&mut Player, &Transform), Without<Mob>>,
    anim: Single<&ChameleonAnim>,
    mut mobs: Query<(&mut Mob, &mut Transform)>,
    platforms: Query<(&Platform, &Transform), (Without<Mob>, Without<Player>)>,
    terrain: Res<Terrain>,
) {
    let dt = time.delta_secs().min(1.0 / 30.0);
    let t = time.elapsed_secs();
    let (mut p, ptf) = player.into_inner();
    let ppos = ptf.translation.truncate();
    let alive = p.dead.is_none();
    let mut deeds = Deeds::default();

    for (mut m, mut tf) in &mut mobs {
        let m = &mut *m;
        let spec = m.spec;
        let pos = tf.translation.truncate();
        // Stuck in a web: everything about it goes slower.
        let dt = dt * (1.0 - 0.8 * m.webbed).max(0.1);

        if let State::Dead { t: td, landed } = m.state {
            // The corpse: falls, lands belly-up, leaks for a while.
            let mut landed = landed;
            if !landed && pos.y > terrain.bounds.min.y - 600.0 {
                m.vel.y -= GRAVITY * dt;
                let mut next = pos + m.vel * dt;
                let rest = spec.foot * 0.6;
                if let Some(top) = boss::floor(&terrain, platforms.iter(), pos, next, rest) {
                    next.y = top + rest;
                    m.vel = Vec2::ZERO;
                    landed = true;
                    bleed.write(Bleed { pos, dir: Vec2::Y, spread: 1.2, speed: 200.0, amount: 10, fluid: spec.blood, sticky: true });
                } else if terrain.solid_at(next) {
                    // Hit a wall: slide down it.
                    next.x = pos.x;
                    m.vel.x = 0.0;
                }
                tf.translation.x = next.x;
                tf.translation.y = next.y;
            }
            // Leaks hard at first, then spurts now and then until it is eaten.
            let rate = if td < 2.5 { 8.0 } else { 0.5 };
            if m.rng.chance(dt * rate) {
                let (amount, speed) = if td < 2.5 { (1, 120.0) } else { (3, 160.0) };
                bleed.write(Bleed { pos, dir: Vec2::Y, spread: 1.0, speed, amount, fluid: spec.blood, sticky: true });
            }
            m.state = State::Dead { t: td + dt, landed };
            continue;
        }
        if spec.moves == Moves::Hang {
            // Nests only flinch here; `nests` runs the rest.
            if let State::Hurt(s) = m.state {
                m.state = if s < 0.3 { State::Hurt(s + dt) } else { State::Idle };
            }
            continue;
        }

        let to_player = ppos - pos;
        let dist = to_player.length();
        let side = if to_player.x >= 0.0 { 1.0 } else { -1.0 };
        let in_view = || terrain.raycast(pos, ppos).is_none();
        let seen = alive
            && dist < spec.sense
            && match spec.moves {
                Moves::Walk => to_player.y.abs() < 90.0 && (ppos.x - m.home.x).abs() < spec.leash + 120.0,
                Moves::Hop => to_player.y.abs() < 200.0 && (ppos.x - m.home.x).abs() < spec.leash + 150.0,
                // Any height they can see across (slopes put the chameleon well above or below).
                Moves::Pack => to_player.y.abs() < 220.0 && (ppos.x - m.home.x).abs() < spec.leash + 200.0 && in_view(),
                // Right below it.
                Moves::Dangle => to_player.x.abs() < 70.0 && to_player.y < 0.0 && in_view(),
                _ => in_view(),
            };
        let touching = alive && dist < spec.radius + 16.0;
        let sees = notice(m, seen, seen && dist < spec.sense * if spec.veteran { 1.0 } else { UNAWARE_RANGE }, touching, p.vel.length(), dt);
        let ease = |rate: f32| 1.0 - (-rate * dt).exp();

        if spec.moves == Moves::Pack {
            let world = rat::World { terrain: &terrain, p: &p, anim: *anim, ppos, t, dt };
            let step = rat::think(m, pos, sees, touching, &world);
            if step.bite {
                deeds.bites.push((pos, spec.damage));
            }
            let (want, face) = (step.want, step.facing);
            if !matches!(m.state, State::Hurt(_)) {
                m.vel.x += (want * spec.attack_speed - m.vel.x) * ease(10.0);
            } else {
                m.vel.x *= 1.0 - 6.0 * dt;
            }
            let feet = pos - Vec2::Y * spec.foot;
            let dx = m.rat.stride(m.vel.x * dt);
            let (feet, blocked) = if dx != 0.0 { walk(&terrain, feet, dx, m.home.x, spec) } else { (feet, false) };
            if blocked {
                m.vel.x = 0.0;
                m.rat.blocked();
            }
            tf.translation.x = feet.x;
            tf.translation.y = feet.y + spec.foot;
            if let Some(face) = face {
                m.facing = face;
            } else if m.vel.x.abs() > 10.0 {
                m.facing = m.vel.x.signum();
            }
            m.stride += dt * m.vel.x.abs() * 0.12;
            continue;
        }

        if spec.moves == Moves::Dangle {
            // Spiders: bob on the thread, drop on the chameleon passing below,
            // dangle there a moment (the chance to hit it), climb back up.
            let anchor = m.home + Vec2::Y * THREAD_REST;
            let rest = m.home.y + (t * 1.3 + m.seed).sin() * 8.0;
            m.state = match m.state {
                State::Hurt(s) if s < 0.3 => State::Hurt(s + dt),
                State::Hurt(_) => State::Recover(0.0),
                State::Idle if sees => State::Windup(0.0),
                State::Idle => {
                    m.vel.y = (rest - pos.y) * 3.0;
                    State::Idle
                }
                State::Windup(s) if s < spec.windup => {
                    m.vel.y = 40.0; // gathers itself up
                    State::Windup(s + dt)
                }
                State::Windup(_) => State::Attack(0.0),
                State::Attack(_) if touching => {
                    deeds.bites.push((pos, spec.damage));
                    State::Recover(0.0)
                }
                State::Attack(s) if s > spec.attack_time => State::Recover(0.0),
                State::Attack(s) => {
                    m.vel.y = -spec.attack_speed;
                    State::Attack(s + dt)
                }
                State::Recover(s) => {
                    m.vel.y = if s < spec.recover { 0.0 } else { 150.0 };
                    if s > spec.recover && pos.y >= m.home.y { State::Idle } else { State::Recover(s + dt) }
                }
                other => other,
            };
            let mut next = pos + Vec2::Y * m.vel.y * dt;
            // At the end of its thread, or on the ground: stop.
            let bottom = next.y < anchor.y - THREAD_MAX || terrain.solid_at(next - Vec2::Y * spec.foot);
            if bottom {
                next.y = pos.y;
                m.vel.y = 0.0;
                if let State::Attack(_) = m.state {
                    m.state = State::Recover(0.0);
                }
            }
            // Swinging a little on the thread.
            let len = anchor.y - next.y;
            next.x = anchor.x + (t * 1.7 + m.seed).sin() * len * 0.05;
            tf.translation.x = next.x;
            tf.translation.y = next.y.min(anchor.y - 12.0);
            continue;
        }

        m.state = match m.state {
            State::Hurt(s) => {
                if !m.airborne {
                    m.vel *= 1.0 - 6.0 * dt;
                }
                if s < 0.3 { State::Hurt(s + dt) } else { State::Recover(spec.recover * 0.5) }
            }
            // Hoppers only crouch on the ground.
            State::Idle if sees && !m.airborne => {
                m.facing = side;
                if spec.moves == Moves::Swarm {
                    State::Attack(0.0)
                } else {
                    m.plan = ppos;
                    State::Windup(0.0)
                }
            }
            State::Idle => {
                let shock = match m.notice {
                    Notice::Shock { how, .. } => Some(how),
                    _ => None,
                };
                match shock {
                    Some(how) => flinch(m, how, to_player, dt),
                    None => idle(m, pos, t, dt),
                }
                // Watches the chameleon coming (running off, it looks where it goes).
                let bolting = matches!(shock, Some(Shock::Flee | Shock::Panic | Shock::Scatter(_))) && m.vel.x.abs() > 8.0;
                m.facing = if alive && dist < spec.sense * 1.4 && spec.moves != Moves::Swarm && !bolting {
                    side
                } else if m.vel.x.abs() > 8.0 {
                    m.vel.x.signum()
                } else {
                    m.facing
                };
                State::Idle
            }
            State::Windup(s) => {
                m.facing = side;
                m.vel = match spec.moves {
                    Moves::Fly => -to_player.normalize_or_zero() * 40.0,
                    Moves::Walk => Vec2::new(-side * 20.0, 0.0),
                    _ => Vec2::ZERO,
                };
                if s < spec.windup {
                    State::Windup(s + dt)
                } else {
                    attack(m, pos, ppos, &mut deeds)
                }
            }
            State::Attack(s) => {
                if spec.moves == Moves::Swarm {
                    // Circle the chameleon, closing in, then nip.
                    m.orbit += dt * 4.0 * m.facing;
                    let buzz = Vec2::new((t * 13.0 + m.seed).sin(), (t * 17.0 + m.seed).cos()) * 14.0;
                    let ring = 70.0 - 40.0 * (s / 1.5).min(1.0);
                    let target = ppos + Vec2::from_angle(m.orbit) * ring + buzz;
                    let desired = ((target - pos) * 6.0).clamp_length_max(spec.attack_speed);
                    m.vel = m.vel.lerp(desired, ease(6.0));
                    m.facing = if m.vel.x.abs() > 10.0 { m.vel.x.signum() } else { m.facing };
                }
                if touching && (spec.moves != Moves::Hop || m.airborne) {
                    deeds.bites.push((pos, spec.damage));
                    m.vel = match spec.moves {
                        // Scatter.
                        Moves::Swarm => (pos - ppos).normalize_or(Vec2::Y) * 260.0,
                        Moves::Fly => -m.vel.normalize_or_zero() * 220.0 + Vec2::Y * 80.0,
                        Moves::Hop => Vec2::new(-m.vel.x * 0.4, 200.0),
                        _ => m.vel * -0.3,
                    };
                    State::Recover(0.0)
                } else if spec.moves == Moves::Hop && !m.airborne && s > 0.05 {
                    State::Recover(0.0)
                } else if s > spec.attack_time || (spec.moves == Moves::Swarm && !sees) {
                    State::Recover(0.0)
                } else {
                    State::Attack(s + dt)
                }
            }
            State::Recover(s) => {
                match spec.moves {
                    Moves::Fly => {
                        let back = ((m.home - pos) * 1.5).clamp_length_max(spec.speed * 0.6);
                        m.vel = m.vel.lerp(back, ease(3.0));
                    }
                    Moves::Swarm => m.vel *= 1.0 - 2.0 * dt,
                    Moves::Walk => m.vel.x *= 1.0 - 6.0 * dt,
                    Moves::Hop if !m.airborne => m.vel = Vec2::ZERO,
                    _ => {}
                }
                if s < spec.recover { State::Recover(s + dt) } else { State::Idle }
            }
            dead @ State::Dead { .. } => dead,
        };

        // Move against the terrain grid, on a leash.
        let next = match spec.moves {
            Moves::Fly | Moves::Swarm => {
                let (mut next, blocked) = fly(&terrain, pos, m.vel * dt, spec.radius * 0.8);
                let off = next - m.home;
                let leashed = off.length() > spec.leash;
                if leashed {
                    next = m.home + off.clamp_length_max(spec.leash);
                }
                if (blocked || leashed) && matches!(m.state, State::Attack(_)) && spec.moves == Moves::Fly {
                    // A dive into a wall ends it.
                    m.vel *= -0.2;
                    m.state = State::Recover(0.0);
                }
                next
            }
            Moves::Walk => {
                let feet = pos - Vec2::Y * spec.foot;
                let (feet, blocked) = walk(&terrain, feet, m.vel.x * dt, m.home.x, spec);
                if blocked {
                    match m.state {
                        State::Attack(_) => {
                            m.vel *= -0.2;
                            m.state = State::Recover(0.0);
                        }
                        State::Idle => {
                            m.vel.x = 0.0;
                            m.wander = (-m.wander.0, m.rng.range(1.0, 2.0));
                        }
                        _ => m.vel.x = 0.0,
                    }
                }
                feet + Vec2::Y * spec.foot
            }
            Moves::Hop => hop(m, &terrain, &platforms, pos, dt),
            Moves::Perch | Moves::Hang | Moves::Pack | Moves::Dangle => pos,
        };
        tf.translation.x = next.x;
        tf.translation.y = next.y;

        let scrabble = if matches!(m.state, State::Windup(_)) { 25.0 } else { 0.0 };
        m.stride += dt * (m.vel.x.abs() * 0.12 + scrabble);
    }

    for (from, damage) in deeds.bites {
        boss::hurt_player(&mut p, ppos, from, damage, &mut bleed, (&mut shake, &mut stop));
    }
    for (from, vel) in deeds.spits {
        commands.spawn((
            Glob { vel, life: 3.0 },
            MobStuff,
            Mesh2d(glob.mesh.clone()),
            MeshMaterial2d(glob.mat.clone()),
            Transform::from_translation(from.extend(13.0)),
            camera::color_layer(),
        ));
    }
}

/// How far (px) the chameleon can stray from where an attack was planned
/// before the plan is worthless.
const PLAN_SLACK: f32 = 120.0;
/// Chance an attack falls apart when the chameleon is `PLAN_SLACK` or more off.
const MISFIRE: f32 = 0.9;

/// Surprise: whether the mob has made up its mind to fight. The first sight
/// of the chameleon puts it in shock for a while (each mob its own time, from
/// its initiative; a few are quick, and a fast arrival stuns them longer),
/// then it turns aware. Only a hit snaps it out of it; bumping into it makes
/// it panic and run from you, for longer. Until then they go about their business
/// and only notice the chameleon from fairly close (`spotted`).
fn notice(m: &mut Mob, seen: bool, spotted: bool, touched: bool, speed: f32, dt: f32) -> bool {
    match m.notice {
        Notice::Unaware if spotted => {
            let mut left = m.spec.initiative * m.rng.range(0.7, 2.0);
            if m.rng.chance(QUICK_SHARE) {
                left *= 0.25;
            }
            // Something crashing in from nowhere takes longer to make sense of.
            left *= 1.0 + (speed / 500.0).min(2.0);
            if m.spec.veteran {
                left *= VETERAN_REFLEX;
            }
            let how = match m.rng.below(10) {
                _ if m.spec.veteran => Shock::Freeze,
                0..=3 => Shock::Freeze,
                4..=6 => Shock::Flee,
                _ => Shock::Scatter(m.rng.range(0.0, 2.0 * PI)),
            };
            m.notice = if left > 0.0 { Notice::Shock { left, how } } else { Notice::Aware };
            m.lost = 0.0;
            false
        }
        Notice::Unaware => false,
        Notice::Shock { left, how } => {
            if touched && m.spec.veteran {
                // Bumped: it does not panic, it fights.
                m.notice = Notice::Aware;
                return seen;
            }
            if touched {
                // Bumped while stunned: panic, and a longer one.
                m.notice = Notice::Shock { left: (left + PANIC_EXTRA).min(PANIC_CAP), how: Shock::Panic };
                return false;
            }
            m.notice = if left <= dt { Notice::Aware } else { Notice::Shock { left: left - dt, how } };
            m.notice == Notice::Aware && seen
        }
        Notice::Aware => {
            m.lost = if seen { 0.0 } else { m.lost + dt };
            if m.lost > FORGET {
                m.notice = Notice::Unaware;
            }
            seen
        }
    }
}

/// Caught by surprise: stands stock still, runs from the chameleon, or bolts
/// somewhere at random.
fn flinch(m: &mut Mob, how: Shock, to_player: Vec2, dt: f32) {
    let spec = m.spec;
    let ease = |rate: f32| 1.0 - (-rate * dt).exp();
    let run = match how {
        Shock::Freeze => Vec2::ZERO,
        Shock::Flee | Shock::Panic => -to_player.normalize_or_zero(),
        Shock::Scatter(a) => Vec2::from_angle(a),
    };
    let rush = if how == Shock::Panic { 1.8 } else { 1.0 };
    match spec.moves {
        Moves::Fly => m.vel = m.vel.lerp(run * spec.speed * 1.6 * rush, ease(6.0)),
        // Flies never hold still.
        Moves::Swarm if run == Vec2::ZERO => {}
        Moves::Swarm => m.vel = m.vel.lerp(run * spec.speed * 2.0 * rush, ease(8.0)),
        Moves::Walk => {
            let dir = if run.x == 0.0 { 0.0 } else { run.x.signum() };
            m.vel.x += (dir * spec.speed * 2.0 * rush - m.vel.x) * ease(10.0);
        }
        Moves::Hop if !m.airborne => m.vel = Vec2::ZERO,
        _ => {}
    }
}

/// Minding its own business.
fn idle(m: &mut Mob, pos: Vec2, t: f32, dt: f32) {
    let spec = m.spec;
    let ease = |rate: f32| 1.0 - (-rate * dt).exp();
    match spec.moves {
        Moves::Fly => {
            let drift = Vec2::new((t * 0.7 + m.seed).sin() * 60.0, (t * 1.1 + m.seed).sin() * 25.0);
            let desired = ((m.home + drift - pos) * 2.0).clamp_length_max(spec.speed);
            m.vel = m.vel.lerp(desired, ease(3.0));
        }
        Moves::Swarm => {
            // Buzzing loops around the spot, never still.
            let s = m.seed;
            let loops = Vec2::new(
                (t * 3.1 + s).sin() * 45.0 + (t * 7.3 + s * 2.0).sin() * 18.0,
                (t * 2.3 + s).cos() * 28.0 + (t * 9.1 + s).sin() * 12.0,
            );
            let desired = ((m.home + loops - pos) * 5.0).clamp_length_max(spec.speed * 2.0);
            m.vel = m.vel.lerp(desired, ease(7.0));
        }
        Moves::Walk | Moves::Pack => {
            m.wander.1 -= dt;
            if m.wander.1 <= 0.0 {
                let away = pos.x - m.home.x;
                m.wander = if m.rng.chance(0.35) {
                    (0.0, m.rng.range(0.6, 1.6))
                } else if away.abs() > spec.leash * 0.6 {
                    (-away.signum(), m.rng.range(1.0, 2.5))
                } else {
                    (if m.rng.chance(0.5) { 1.0 } else { -1.0 }, m.rng.range(1.0, 2.5))
                };
            }
            m.vel.x += (m.wander.0 * spec.speed - m.vel.x) * ease(8.0);
        }
        Moves::Hop => {
            // Little hops about, now and then.
            if m.airborne {
                return;
            }
            m.wander.1 -= dt;
            if m.wander.1 <= 0.0 {
                let away = pos.x - m.home.x;
                let dir = if away.abs() > spec.leash * 0.5 { -away.signum() } else if m.rng.chance(0.5) { 1.0 } else { -1.0 };
                m.vel = Vec2::new(dir * spec.speed * m.rng.range(0.6, 1.2), m.rng.range(240.0, 340.0));
                m.airborne = true;
                m.wander.1 = m.rng.range(0.8, 2.2);
            }
        }
        Moves::Perch | Moves::Hang | Moves::Dangle => {}
    }
}

/// The telegraph is over: charge, dive, leap or spit.
fn attack(m: &mut Mob, pos: Vec2, ppos: Vec2, deeds: &mut Deeds) -> State {
    let spec = m.spec;
    // The attack was prepared against where the chameleon was: it only follows
    // a little. The further it has moved since, the worse the attack goes, up
    // to tripping over itself (and being open for a moment).
    let off = ((ppos - m.plan).length() / PLAN_SLACK).min(1.0);
    if m.rng.chance(off * MISFIRE) {
        m.vel = match spec.moves {
            Moves::Walk => Vec2::new(m.facing * spec.attack_speed * 0.25, 0.0),
            _ => Vec2::ZERO,
        };
        return State::Recover(-spec.recover * 0.5);
    }
    let ppos = m.plan.lerp(ppos, 0.2);
    let to = ppos - pos;
    if spec.moves == Moves::Walk {
        m.facing = if to.x >= 0.0 { 1.0 } else { -1.0 };
    }
    match spec.moves {
        Moves::Walk => m.vel = Vec2::new(m.facing * spec.attack_speed, 0.0),
        Moves::Fly | Moves::Swarm => m.vel = to.normalize_or(Vec2::new(m.facing, 0.0)) * spec.attack_speed,
        Moves::Hop => {
            // A ballistic leap that comes down on the chameleon.
            let vx = (to.x / LEAP_TIME).clamp(-spec.attack_speed, spec.attack_speed);
            let vy = ((to.y + 0.5 * GRAVITY * LEAP_TIME * LEAP_TIME) / LEAP_TIME).clamp(300.0, 720.0);
            m.vel = Vec2::new(vx, vy);
            m.airborne = true;
        }
        Moves::Perch => {
            // Spit in an arc that lands on the chameleon.
            let mouth = pos + Vec2::new(9.0 * m.facing, 2.0) * spec.scale;
            let to = ppos - mouth;
            let time = (to.length() / spec.attack_speed).clamp(0.35, 1.3);
            let vel = Vec2::new(to.x / time, (to.y + 0.5 * GLOB_GRAVITY * time * time) / time);
            deeds.spits.push((mouth, vel));
            return State::Recover(0.0);
        }
        Moves::Hang | Moves::Pack | Moves::Dangle => {}
    }
    State::Attack(0.0)
}

/// Hoppers: ballistic in the air, stuck to the ground otherwise.
fn hop(m: &mut Mob, terrain: &Terrain, platforms: &Query<(&Platform, &Transform), (Without<Mob>, Without<Player>)>, pos: Vec2, dt: f32) -> Vec2 {
    let spec = m.spec;
    if !m.airborne {
        // Still ground under its feet? Else it falls.
        let feet = pos - Vec2::Y * spec.foot;
        match terrain.surface(feet + Vec2::Y * STEP_UP, 2.0 * STEP_UP) {
            Some(y) => return Vec2::new(pos.x, y + spec.foot),
            None => m.airborne = true,
        }
    }
    m.vel.y -= GRAVITY * dt;
    let mut next = pos + m.vel * dt;
    // Walls stop it sideways, ceilings from above; the leash too.
    // (The body box sits above the stair-stepped ground of slopes.)
    let body = Vec2::new(spec.radius * 0.4, spec.foot * 0.4);
    if terrain.box_hits(Vec2::new(next.x, pos.y + 4.0), body) || (next.x - m.home.x).abs() > spec.leash {
        next.x = pos.x;
        m.vel.x = 0.0;
    }
    if m.vel.y > 0.0 && terrain.box_hits(Vec2::new(next.x, next.y + body.y), Vec2::new(body.x, 2.0)) {
        next.y = pos.y;
        m.vel.y = 0.0;
    }
    if let Some(top) = boss::floor(terrain, platforms.iter(), pos, next, spec.foot) {
        next.y = top + spec.foot;
        m.vel = Vec2::ZERO;
        m.airborne = false;
    }
    if next.y < terrain.bounds.min.y - 600.0 {
        // Fell out of the world: back to its spot.
        next = m.home;
        m.vel = Vec2::ZERO;
    }
    next
}

/// Walk `dx` along the ground from `feet`: up and down steps up to `STEP_UP`,
/// stopping at walls, drops and the leash. Returns the new feet and whether it was blocked.
fn walk(terrain: &Terrain, feet: Vec2, dx: f32, home_x: f32, spec: &MobSpec) -> (Vec2, bool) {
    let n = (dx.abs() / 4.0).ceil().max(1.0) as i32;
    let step = dx / n as f32;
    let mut f = feet;
    for _ in 0..n {
        let nx = f.x + step;
        if step != 0.0 && (nx - home_x).abs() > spec.leash && (nx - home_x).abs() > (f.x - home_x).abs() {
            return (f, true);
        }
        let Some(ny) = terrain.surface(Vec2::new(nx, f.y + STEP_UP), 2.0 * STEP_UP) else { return (f, true) };
        if (ny - f.y).abs() > STEP_UP + 0.1 || !fits(terrain, Vec2::new(nx, ny), spec) {
            return (f, true);
        }
        f = Vec2::new(nx, ny);
    }
    (f, false)
}

/// Fly by `d`, keeping `r` from solid cells (sliding along walls).
/// Returns the new position and whether it was blocked.
fn fly(terrain: &Terrain, pos: Vec2, d: Vec2, r: f32) -> (Vec2, bool) {
    // Squeezed somewhere tight (knocked in): any move that doesn't get tighter is fine.
    let need = r.min(terrain.clearance(pos));
    let ok = |p: Vec2| !terrain.solid_at(p) && terrain.clearance(p) >= need;
    if ok(pos + d) {
        (pos + d, false)
    } else if ok(pos + Vec2::new(d.x, 0.0)) {
        (pos + Vec2::new(d.x, 0.0), true)
    } else if ok(pos + Vec2::new(0.0, d.y)) {
        (pos + Vec2::new(0.0, d.y), true)
    } else {
        (pos, true)
    }
}

/// Nests release a drone now and then while the chameleon is around
/// (at most `BROOD` out at once); the drone guards the spot under the nest.
fn nests(
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
        if let (Some(nest), false) = (m.parent, matches!(m.state, State::Dead { .. })) {
            *brood.entry(nest).or_default() += 1;
        }
    }
    for (e, mut m, tf) in &mut mobs {
        if m.spec.moves != Moves::Hang {
            continue;
        }
        let pos = tf.translation.truncate();
        match m.state {
            State::Idle => {
                m.cooldown -= dt;
                let near = p.dead.is_none() && pos.distance(ppos) < m.spec.sense;
                if near && m.cooldown <= 0.0 && brood.get(&e).copied().unwrap_or(0) < BROOD {
                    m.cooldown = m.rng.range(BROOD_EVERY.0, BROOD_EVERY.1);
                    m.state = State::Attack(0.0);
                    // Out of the hole at the bottom.
                    let at = pos + Vec2::new(m.rng.range(-12.0, 12.0), -16.0 * m.spec.scale);
                    mob_art::spawn(&mut commands, &mut meshes, &mut materials, MobKind::Drone, at, Some(e));
                }
            }
            State::Attack(s) => m.state = if s < m.spec.attack_time { State::Attack(s + dt) } else { State::Idle },
            _ => {}
        }
    }
}

fn move_globs(
    mut commands: Commands,
    time: Res<Time>,
    mut shake: ResMut<Shake>,
    mut stop: ResMut<HitStop>,
    mut bleed: MessageWriter<Bleed>,
    player: Single<(&mut Player, &Transform), Without<Glob>>,
    mut globs: Query<(Entity, &mut Glob, &mut Transform)>,
    terrain: Res<Terrain>,
) {
    let dt = time.delta_secs().min(1.0 / 30.0);
    let (mut p, ptf) = player.into_inner();
    let ppos = ptf.translation.truncate();
    for (e, mut g, mut tf) in &mut globs {
        g.vel.y -= GLOB_GRAVITY * dt;
        g.life -= dt;
        let pos = tf.translation.truncate() + g.vel * dt;
        tf.translation.x = pos.x;
        tf.translation.y = pos.y;
        tf.rotation = Quat::from_rotation_z(g.vel.to_angle());
        let splash = |bleed: &mut MessageWriter<Bleed>, amount| {
            bleed.write(Bleed { pos, dir: -g.vel.normalize_or(Vec2::Y), spread: 1.2, speed: 200.0, amount, fluid: Fluid::Venom, sticky: true });
        };
        if p.dead.is_none() && pos.distance(ppos) < player::HALF.y + 6.0 {
            boss::hurt_player(&mut p, ppos, pos - g.vel * 0.05, 1, &mut bleed, (&mut shake, &mut stop));
            splash(&mut bleed, 12);
            commands.entity(e).despawn();
        } else if terrain.solid_at(pos) || g.life <= 0.0 {
            splash(&mut bleed, 6);
            commands.entity(e).despawn();
        }
    }
}

fn flash_mobs(time: Res<Time>, mut materials: ResMut<Assets<ColorMaterial>>, mut mobs: Query<&mut Mob>) {
    for mut m in &mut mobs {
        if m.flash <= 0.0 {
            continue;
        }
        m.flash = (m.flash - time.delta_secs() * 6.0).max(0.0);
        for (handle, base) in &m.mats {
            if let Some(mut mat) = materials.get_mut(handle) {
                mat.color = Oklaba::from(*base).mix(&Color::WHITE.into(), m.flash).into();
            }
        }
    }
}
