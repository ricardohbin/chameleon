//! The King Rat: a big, mangy sewer rat (about 1.8× the chameleon) that fights on the ground.
//!
//! It hits hard but it is a coward, like the rats of its packs. It reads how
//! much the chameleon is paying attention to it (`attention`: the body facing
//! it, the head looking at it, coming straight at it) and keeps its distance
//! while watched, backing off; ignored, it creeps in. It strikes when the
//! chameleon looks away, when it is cornered against the edge of its ground
//! (then it fights in a fury, with shorter tells) or when hunger gets the
//! better of its fear (`PATIENCE`). Hits frighten it (`fear`): enough of them
//! and it bolts to the other side.
//!
//! It roams its whole lair, not just the floor around its marker: `Ground` maps
//! the walkable floor from its home left and right (up ledges it can hop, down
//! drops, across small gaps; not into water, not out of its region) and it
//! chases and flees along all of it, hopping the ledges, drops and gaps
//! (`Leap::Hop`). It wakes as soon as the chameleon sets foot on that ground.
//!
//! It is quick on its feet: it **vaults** over the chameleon to get out of a
//! corner, and when it runs into a wall at the edge of its ground it
//! **scurries up** it, clings there hissing, and **drops** onto the chameleon.
//!
//! Attacks: the **lunge** (crouches, teeth bared, tail lashing: the tell; then
//! a fast rush along the floor through where the chameleon stood): a bite
//! costs `BITE_DAMAGE` and throws the chameleon, and the rat bolts at once,
//! hit and run; missed, it skids and is open for a moment. Close up, the
//! **maul**: it rears up on its hind legs and slams both clawed paws down in
//! front of it (`MAUL_DAMAGE`), then needs a moment to recover. From mid range
//! the lunge is sometimes a **leaping bite** in an arc (jumping over it isn't
//! safe). Enraged (half health) a missed lunge turns straight into another one back.
//!
//! Every attack ends in a short **claw slash** thrown in front of it (`Swipe`:
//! flies `SWIPE_RANGE`, `SWIPE_DAMAGE`; bigger enraged), so a near miss still
//! catches the chameleon.
//!
//! It bleeds dull red `#ac3232` and gives the chameleon the sewer's air
//! (`foul_air`: until it falls, breathing in the sewer costs health). On death
//! it rolls onto its back, the tail tears off and the legs twitch. Health,
//! getting hit and the death spectacle live in `boss.rs`.

use std::{f32::consts::PI, sync::Arc};

use bevy::{prelude::*, sprite_render::AlphaMode2d};

use crate::{
    boss::{self, Arena, Boss, BossHit, BossSpec, BossStuff, Defeated},
    camera::{HitStop, MainCamera, Shake},
    chameleon::{self, ChameleonAnim},
    fumes::Cloud,
    gore::{Bleed, Fluid},
    player::{self, Hurt, Player, Respawned},
    shapes::*,
    terrain::{Style, Terrain, STEP_UP},
};

/// Its color: what the chameleon earns, its blood, the health bar.
pub const RED: Color = Color::srgb(0xac as f32 / 255.0, 0x32 as f32 / 255.0, 0x32 as f32 / 255.0);

pub(crate) static SPEC: BossSpec = BossSpec {
    name: "THE KING RAT",
    hp: 16,
    radius: RADIUS,
    blood: Fluid::Crimson,
    color: RED,
    aggro: AGGRO,
    // Big and fat.
    food: 5.0,
};

/// Size of the boss relative to its drawing.
const SCALE: f32 = 1.7;
/// Hurtbox radius around the body center.
const RADIUS: f32 = 18.0 * SCALE;
/// Body center height above the floor when standing.
const FOOT: f32 = 12.0 * SCALE;
/// Half its length: how close to the arena edges its center may go.
const HALF_W: f32 = 28.0 * SCALE;
const AGGRO: f32 = 1000.0;
/// How far inside the screen edge (px) it must be to count as in view.
const SCREEN_MARGIN: f32 = 60.0;

/// The face-off: it rears up, hisses and gnashes its teeth.
const TAUNT_TIME: f32 = 1.8;
/// The chameleon this close cuts the taunt short.
const TAUNT_BREAK: f32 = 200.0;

/// Attention: how much the body facing it counts, and the head looking at it.
const BODY: f32 = 0.7;
const HEAD: f32 = 0.3;
/// Attention kept while the chameleon slashes elsewhere or eats, and while it's in the air.
const BUSY: f32 = 0.35;
const AIRBORNE: f32 = 0.7;
/// Distance it keeps: ignored → watched.
const CREEP: f32 = 200.0;
const WARY: f32 = 460.0;
/// Speeds: creeping in, backing off (still facing the chameleon), bolting away.
const SNEAK: f32 = 170.0;
const BACK_OFF: f32 = 260.0;
const BOLT: f32 = 820.0;
const BOLT_TIME: f32 = 0.9;
/// Sniffing pauses while creeping in (per second), and how long they last.
const SNIFF: f32 = 0.6;
const SNIFF_TIME: f32 = 0.35;
/// Only strikes when attention is under this (unless cornered or out of patience).
const DARE: f32 = 0.45;
/// Seconds without attacking after which it strikes anyway.
const PATIENCE: f32 = 4.5;
/// Seconds between attacks (enraged: × `RAGE_PACE`).
const COOLDOWN: (f32, f32) = (0.9, 1.5);
const RAGE_PACE: f32 = 0.7;
/// Fear: each hit adds `FEAR_HIT`, the chameleon rushing it adds up too; at 1 it bolts.
const FEAR_HIT: f32 = 0.4;
const FEAR_FADE: f32 = 0.25;
/// Cornered: less room than this behind it, with the chameleon closer than `CORNER_DIST`.
const CORNER_ROOM: f32 = 90.0;
const CORNER_DIST: f32 = 280.0;
/// Fury (cornered): tells this much shorter.
const FURY_PACE: f32 = 0.65;

// The lunge (main attack).
const CROUCH: f32 = 0.55;
const LUNGE_SPEED: f32 = 1250.0;
/// It runs this far past where the chameleon stood.
const OVERSHOOT: f32 = 150.0;
const LUNGE_REACH: f32 = 950.0;
const BITE_DAMAGE: i32 = 2;
const BITE_LAUNCH: Vec2 = Vec2::new(720.0, 420.0);
/// Missed: skids, open to hits.
const SKID: f32 = 0.9;

// Leaps: the leaping bite (mid range), the vault over the chameleon, the drop from a wall.
const LEAP_SHARE: f32 = 0.4;
const LEAP_MIN: f32 = 240.0;
const LEAP_HEIGHT: f32 = 120.0;
const LEAP_TIME: f32 = 0.55;
const LEAP_OVERSHOOT: f32 = 40.0;
/// Cornered and watched: it vaults over the chameleon this often instead of fighting.
const VAULT_SHARE: f32 = 0.55;
const VAULT_HEIGHT: f32 = 170.0;
const VAULT_TIME: f32 = 0.6;
/// Lands this far past the chameleon.
const VAULT_PAST: f32 = 180.0;

// Up the walls at the edges of its ground.
/// Runs into a wall while bolting: climbs it this often.
const CLIMB_SHARE: f32 = 0.75;
const CLIMB_SPEED: f32 = 520.0;
/// How high it goes (above its floor), and the least wall worth climbing.
const CLIMB_HEIGHT: f32 = 260.0;
const MIN_WALL: f32 = 120.0;
/// How far past the edge of its ground a wall may stand.
const WALL_SEARCH: f32 = 120.0;
/// Clings there (hissing) before it drops on the chameleon.
const CLING: (f32, f32) = (0.5, 1.1);
const DROP_TIME: f32 = 0.6;

// The claw slash left by every attack: a crescent thrown forward a short way.
const SWIPE_RANGE: f32 = 110.0;
const SWIPE_TIME: f32 = 0.22;
const SWIPE_RADIUS: f32 = 26.0;
const SWIPE_DAMAGE: i32 = 1;
/// Enraged: this much bigger and farther.
const SWIPE_RAGE: f32 = 1.35;
const SWIPE_COLOR: Color = Color::srgba(0.88, 0.85, 0.80, 0.85);

// The maul (close up).
const MAUL_RANGE: f32 = 150.0;
const REAR: f32 = 0.55;
const SLAM: f32 = 0.16;
/// The paws reach this far in front of its body.
const MAUL_REACH: f32 = 60.0;
const MAUL_DAMAGE: i32 = 2;
const MAUL_LAUNCH: Vec2 = Vec2::new(620.0, 460.0);
const RECOVER: f32 = 0.75;

// Its ground (`Ground`): mapped in steps of `FLOOR_STEP`, at most `RANGE` each way from home.
const FLOOR_STEP: f32 = 12.0;
const RANGE: f32 = 3000.0;
/// Ledges it hops up, drops it hops down, gaps it jumps.
const MAX_UP: f32 = 110.0;
const MAX_DROP: f32 = 280.0;
const MAX_GAP: f32 = 170.0;
/// Steps taller than this (or gaps) are hopped, not walked.
const HOP_DY: f32 = STEP_UP * 1.5;
const HOP_TIME: f32 = 0.3;

// The sewer's air (until the King Rat falls).
/// Seconds in the sewer before the first cough, then one per `AIR_EVERY`.
const AIR_GRACE: f32 = 3.0;
const AIR_EVERY: f32 = 4.0;
const AIR_DAMAGE: i32 = 1;

const FUR: Color = Color::srgb(0.34, 0.31, 0.28);
const DARK: Color = Color::srgb(0.17, 0.15, 0.14);
const SKIN: Color = Color::srgb(0.60, 0.46, 0.44);
const TEETH: Color = Color::srgb(0.80, 0.70, 0.40);
const MILKY: Color = Color::srgb(0.80, 0.82, 0.78);
const SCAR: Color = Color::srgb(0.56, 0.47, 0.44);
const EYE: Color = Color::srgb(0.04, 0.03, 0.03);

pub struct KingRatPlugin;

impl Plugin for KingRatPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (reset_on_respawn, on_hit, think, swipes, dismember, animate, foul_air)
                .chain()
                .after(player::move_player)
                .after(chameleon::update_anim_state),
        );
    }
}

#[derive(Component)]
struct KingRat {
    state: State,
    vel: Vec2,
    facing: f32,
    cooldown: f32,
    /// Seconds since its last attack (out of patience, it attacks anyway).
    waited: f32,
    fear: f32,
    /// Frozen (sniffing) for this long.
    pause: f32,
    /// Last frame's attention, to notice the chameleon turning to it.
    attention: f32,
    /// Lunges left in an enraged chain.
    chain: u32,
    /// Cornered: shorter tells, no breaking off.
    fury: bool,
    retaliate: bool,
    introduced: bool,
    seed: f32,
    dismember: bool,
    ground: Ground,
    rng: Rng,
}

/// The floor it can get around on: feet positions left to right, `FLOOR_STEP`
/// apart where it walks (farther apart over a gap it jumps).
#[derive(Clone)]
struct Ground(Arc<Vec<Vec2>>);

impl Ground {
    /// Maps the ground from `feet`, both ways, inside `region`.
    fn map(terrain: &Terrain, feet: Vec2, region: Rect) -> Ground {
        // Headroom over a spot (a narrow column: the foot of a step mustn't count), out of the water, in its lair.
        let fits = |f: Vec2| {
            region.contains(f)
                && !terrain.box_hits(f + Vec2::Y * (FOOT + 4.0), Vec2::new(6.0, FOOT * 0.85))
                && !terrain.features.iter().any(|w| w.water.is_some() && w.rect.contains(f + Vec2::Y * 4.0))
        };
        let mut sides = [Vec::new(), Vec::new()];
        for (side, dir) in sides.iter_mut().zip([-1.0f32, 1.0]) {
            let mut f = feet;
            while (f.x - feet.x).abs() < RANGE {
                let x = f.x + dir * FLOOR_STEP;
                // Walk on (a wall taller than `MAX_UP` reads as solid above: no way).
                let step = terrain.surface(Vec2::new(x, f.y + MAX_UP), MAX_UP + MAX_DROP).filter(|&y| y <= f.y + MAX_UP + 1.0).map(|y| Vec2::new(x, y));
                // Else a gap: ground at about the same height a jump away.
                let next = step.filter(|&g| fits(g)).or_else(|| {
                    (2..=(MAX_GAP / FLOOR_STEP) as i32).find_map(|k| {
                        let x = f.x + dir * k as f32 * FLOOR_STEP;
                        let y = terrain.surface(Vec2::new(x, f.y + MAX_UP * 0.5), MAX_UP)?;
                        let g = Vec2::new(x, y);
                        ((y - f.y).abs() <= MAX_UP * 0.5 && fits(g)).then_some(g)
                    })
                });
                let Some(g) = next else { break };
                side.push(g);
                f = g;
            }
        }
        let [left, right] = sides;
        Ground(Arc::new(left.into_iter().rev().chain(std::iter::once(feet)).chain(right).collect()))
    }

    fn pts(&self) -> &[Vec2] {
        &self.0
    }

    /// The segment `x` is on (its left point).
    fn index(&self, x: f32) -> usize {
        let p = self.pts();
        p.partition_point(|q| q.x <= x).saturating_sub(1).min(p.len() - 1)
    }

    /// Between points `i` and `i + 1`: a ledge, a drop or a gap (hopped, not walked).
    fn breaks(&self, i: usize) -> bool {
        let p = self.pts();
        i + 1 < p.len() && ((p[i + 1].y - p[i].y).abs() > HOP_DY || p[i + 1].x - p[i].x > FLOOR_STEP * 1.5)
    }

    /// How far its center may go each way (its body over the ground).
    fn lo(&self) -> f32 {
        self.pts()[0].x + HALF_W * 0.6
    }

    fn hi(&self) -> f32 {
        self.pts()[self.pts().len() - 1].x - HALF_W * 0.6
    }

    fn inside(&self, x: f32) -> f32 {
        let (lo, hi) = (self.lo(), self.hi());
        if lo < hi { x.clamp(lo, hi) } else { (lo + hi) * 0.5 }
    }

    /// Floor height at `x` (walked smoothly, not across a break).
    fn y_at(&self, x: f32) -> f32 {
        let p = self.pts();
        let i = self.index(x);
        if i + 1 < p.len() && !self.breaks(i) {
            let k = ((x - p[i].x) / (p[i + 1].x - p[i].x).max(1e-3)).clamp(0.0, 1.0);
            p[i].y + (p[i + 1].y - p[i].y) * k
        } else {
            p[i].y
        }
    }

    fn land(&self, x: f32) -> Vec2 {
        let x = self.inside(x);
        Vec2::new(x, self.y_at(x))
    }

    /// Moving from `x0` to `x1`: the first ledge, drop or gap on the way, as
    /// where to land past it.
    fn hop(&self, x0: f32, x1: f32) -> Option<Vec2> {
        let (i0, i1) = (self.index(x0), self.index(x1));
        let p = self.pts();
        if i1 > i0 {
            (i0..i1).find(|&i| self.breaks(i)).map(|i| p[(i + 3).min(p.len() - 1)])
        } else if i1 < i0 {
            (i1..i0).rev().find(|&i| self.breaks(i)).map(|i| p[i.saturating_sub(2)])
        } else {
            None
        }
        .map(|q| self.land(q.x))
    }

    /// Its whole ground as a rectangle (what wakes it), with room above.
    fn bounds(&self) -> Rect {
        let p = self.pts();
        let (lo, hi) = p.iter().fold((Vec2::MAX, Vec2::MIN), |(lo, hi), q| (lo.min(*q), hi.max(*q)));
        Rect::new(lo.x - HALF_W, lo.y - 100.0, hi.x + HALF_W, hi.y + 450.0)
    }
}

/// Where the King Rat wakes: anywhere on the ground it roams.
pub fn wake_area(terrain: &Terrain, arena: &Arena, region: Rect) -> Rect {
    Ground::map(terrain, Vec2::new(arena.home.x, arena.floor), region).bounds()
}

#[derive(Clone, Copy, PartialEq)]
enum State {
    Stand,
    Taunt(f32),
    /// Crouched, teeth bared (`len` = how long this one takes); `leap` = a leaping bite.
    Crouch { t: f32, len: f32, leap: bool },
    /// Rushing along the floor toward `to` (x).
    Lunge { to: f32, dir: f32 },
    Skid(f32),
    /// Up on its hind legs (`len` = the tell), then the slam (`t` past `len`).
    Rear { t: f32, len: f32 },
    Recover(f32),
    /// Running away (`dir`).
    Bolt { t: f32, dir: f32 },
    /// Through the air on a parabola (feet `from` → `to`); `bit` once it has bitten.
    Air { from: Vec2, to: Vec2, height: f32, time: f32, t: f32, kind: Leap, bit: bool },
    /// Running up the wall on `side` (its face at `face`), to `top` (body center height).
    Climb { side: f32, face: f32, top: f32 },
    /// Clinging to the wall, hissing, before the drop.
    Cling { side: f32, t: f32, len: f32 },
    Hurt(f32),
    Dead { t: f32, landed: bool },
}

/// A claw slash in flight (`BossStuff`).
#[derive(Component)]
struct Swipe {
    vel: Vec2,
    age: f32,
    /// Enraged: bigger.
    size: f32,
    hit: bool,
    material: Handle<ColorMaterial>,
}

/// What it goes back to after a hop.
#[derive(Clone, Copy, PartialEq)]
enum Resume {
    Stand,
    Bolt(f32),
    Lunge { to: f32, dir: f32 },
}

#[derive(Clone, Copy, PartialEq)]
enum Leap {
    /// Over a ledge, a drop or a gap in its ground, then on with what it was doing.
    Hop(Resume),
    /// The leaping bite.
    Pounce,
    /// Over the chameleon's head, away from a corner.
    Vault,
    /// Off a wall onto the chameleon.
    Drop,
}

impl Leap {
    /// The leaping bite, the drop and a hop mid-lunge bite what they hit.
    fn bites(self) -> bool {
        matches!(self, Leap::Pounce | Leap::Drop | Leap::Hop(Resume::Lunge { .. }))
    }
}

impl KingRat {
    fn airborne(&self) -> bool {
        matches!(self.state, State::Air { .. } | State::Climb { .. } | State::Cling { .. })
    }
}

#[derive(Component)]
struct Bone {
    owner: Entity,
    kind: BoneKind,
}

#[derive(Clone, Copy, PartialEq)]
enum BoneKind {
    Rig,
    Head,
    Jaw,
    Ear,
    Tail,
    TailTip,
    /// Front (0) and hind (1) legs.
    Leg { pair: usize, near: bool },
}

/// Spawns the King Rat on the arena floor, facing left.
pub fn spawn(commands: &mut Commands, meshes: &mut Assets<Mesh>, materials: &mut Assets<ColorMaterial>, arena: &Arena, terrain: &Terrain, region: Rect) -> Entity {
    let fur = materials.add(FUR);
    let dark = materials.add(DARK);
    let skin = materials.add(SKIN);
    let teeth = materials.add(TEETH);
    let milky = materials.add(MILKY);
    let scar = materials.add(SCAR);
    let eye = materials.add(EYE);
    let mats = vec![(fur.clone(), FUR), (dark.clone(), DARK), (skin.clone(), SKIN), (teeth.clone(), TEETH), (scar.clone(), SCAR)];

    let home = Vec2::new(arena.home.x, arena.floor + FOOT);
    let rat = commands.spawn((Transform::from_translation(home.extend(12.0)), Visibility::default())).id();
    let mut b = Builder { commands, meshes, owner: rat };
    let rig = b.bone(Vec3::ZERO, rat, BoneKind::Rig);
    b.commands.entity(rig).insert(Transform::from_scale(Vec3::new(-SCALE, SCALE, 1.0)));

    // The tail: long, bald and ringed, in two pieces so it can lash.
    let tail = b.bone(Vec3::new(-26.0, -2.0, -0.3), rig, BoneKind::Tail);
    let base = smooth_open(&[Vec2::ZERO, Vec2::new(-8.0, -3.0), Vec2::new(-16.0, -4.0)], 4);
    b.part(stroke(&base, |t| 3.4 - 1.2 * t, 1.0, 0.75, |_| 1.0), &skin, Vec3::ZERO, tail);
    let tip = b.bone(Vec3::new(-16.0, -4.0, 0.01), tail, BoneKind::TailTip);
    let rest = smooth_open(&[Vec2::ZERO, Vec2::new(-9.0, -1.0), Vec2::new(-17.0, 3.0), Vec2::new(-22.0, 8.0)], 5);
    b.part(stroke(&rest, |t| 2.2 - 1.7 * t, 1.0, 0.75, |_| 1.0), &skin, Vec3::ZERO, tip);
    let rings: Vec<[Vec2; 3]> = [Vec2::new(-5.0, -2.2), Vec2::new(-10.0, -3.5), Vec2::new(-14.0, -4.0)]
        .into_iter()
        .map(|c| [c + Vec2::new(-0.3, 1.6), c + Vec2::new(0.3, 1.6), c + Vec2::new(0.0, -1.6)])
        .collect();
    b.part(triangles(&rings), &scar, Vec3::new(0.0, 0.0, 0.01), tail);

    // Legs: the far pair dark, behind the body; short and clawed.
    for near in [false, true] {
        let (mat, z, dx) = if near { (&fur, 0.5, 0.0) } else { (&dark, -0.45, 3.0) };
        let hind = b.bone(Vec3::new(-16.0 + dx, -6.0, z), rig, BoneKind::Leg { pair: 1, near });
        let path = smooth_open(&[Vec2::ZERO, Vec2::new(3.0, -4.0), Vec2::new(0.0, -6.0), Vec2::new(7.0, -6.3)], 3);
        b.part(stroke(&path, |t| 3.0 - 1.6 * t, 1.0, 0.8, |_| 1.0), mat, Vec3::ZERO, hind);
        b.part(claws(Vec2::new(7.0, -6.3)), &skin, Vec3::new(0.0, 0.0, 0.01), hind);
        let front = b.bone(Vec3::new(13.0 + dx, -5.0, z), rig, BoneKind::Leg { pair: 0, near });
        let path = smooth_open(&[Vec2::ZERO, Vec2::new(1.5, -4.0), Vec2::new(0.5, -7.0), Vec2::new(4.0, -7.0)], 3);
        b.part(stroke(&path, |t| 2.6 - 1.2 * t, 1.0, 0.8, |_| 1.0), mat, Vec3::ZERO, front);
        b.part(claws(Vec2::new(4.0, -7.0)), &skin, Vec3::new(0.0, 0.0, 0.01), front);
    }

    // The body: fat, hunched, mangy.
    let body = smooth_closed(
        &[
            Vec2::new(20.0, 4.0),
            Vec2::new(14.0, 10.0),
            Vec2::new(2.0, 14.0),
            Vec2::new(-8.0, 15.0),
            Vec2::new(-18.0, 11.0),
            Vec2::new(-26.0, 4.0),
            Vec2::new(-28.0, -2.0),
            Vec2::new(-22.0, -8.0),
            Vec2::new(-8.0, -9.5),
            Vec2::new(6.0, -8.5),
            Vec2::new(16.0, -5.0),
            Vec2::new(21.0, -1.0),
        ],
        6,
    );
    b.part(fill(&body, |p| 0.5 + 0.5 * ((p.y + 9.5) / 24.5).clamp(0.0, 1.0)), &fur, Vec3::ZERO, rig);
    // Bristling fur along the spine.
    let tufts: Vec<[Vec2; 3]> = [(-24.0, 6.0), (-20.0, 10.0), (-15.0, 13.0), (-10.0, 14.5), (-5.0, 14.8), (0.0, 13.8), (5.0, 12.2), (10.0, 10.5)]
        .into_iter()
        .map(|(x, y)| {
            let c = Vec2::new(x, y);
            [c + Vec2::new(1.8, -1.0), c + Vec2::new(-1.8, -0.8), c + Vec2::new(-1.6, 2.6)]
        })
        .collect();
    b.part(triangles(&tufts), &fur, Vec3::new(0.0, 0.0, 0.01), rig);
    // Mange: bald patches of skin, sores and old scars.
    for (c, r) in [(Vec2::new(-12.0, 6.0), Vec2::new(4.5, 3.0)), (Vec2::new(3.0, -3.0), Vec2::new(3.0, 2.0)), (Vec2::new(-21.0, 0.0), Vec2::new(2.6, 2.2))] {
        b.part(fill(&ellipse_pts(r.x, r.y, 14), |p| 0.8 + 0.2 * (p.y / r.y)), &skin, c.extend(0.02), rig);
    }
    for c in [Vec2::new(-11.0, 5.5), Vec2::new(-13.5, 7.0), Vec2::new(3.5, -3.5), Vec2::new(-6.0, -5.0)] {
        b.part(fill(&ellipse_pts(0.8, 0.7, 8), |_| 1.0), &dark, c.extend(0.03), rig);
    }
    for path in [[Vec2::new(-4.0, 12.0), Vec2::new(-1.0, 8.0), Vec2::new(0.0, 5.0)], [Vec2::new(-17.0, 9.0), Vec2::new(-15.0, 6.0), Vec2::new(-14.0, 3.0)]] {
        b.part(stroke(&path, |_| 0.7, 1.0, 1.0, |_| 1.0), &scar, Vec3::new(0.0, 0.0, 0.03), rig);
    }
    // The near haunch.
    b.part(fill(&ellipse_pts(8.0, 7.0, 18), |p| 0.6 + 0.4 * ((p.y + 7.0) / 14.0)), &fur, Vec3::new(-17.0, -3.0, 0.4), rig);

    // The head: a long wedge of a skull, a torn ear, one milky eye, yellow buck teeth.
    let head = b.bone(Vec3::new(19.0, 3.0, 0.6), rig, BoneKind::Head);
    let jaw = b.bone(Vec3::new(6.0, -4.0, -0.05), head, BoneKind::Jaw);
    b.part(fill(&ellipse_pts(6.0, 2.2, 12), |_| 1.0), &dark, Vec3::new(4.0, -0.5, -0.05), jaw);
    let chin = smooth_closed(&[Vec2::new(0.0, 0.0), Vec2::new(12.0, 0.5), Vec2::new(14.0, -1.0), Vec2::new(12.0, -3.0), Vec2::new(2.0, -3.0)], 3);
    b.part(fill(&chin, |p| 0.6 + 0.4 * ((p.y + 3.0) / 3.5)), &fur, Vec3::ZERO, jaw);
    b.part(triangles(&[[Vec2::new(11.6, 0.2), Vec2::new(13.2, 0.2), Vec2::new(12.9, 2.6)]]), &teeth, Vec3::new(0.0, 0.0, 0.01), jaw);
    let skull = smooth_closed(
        &[
            Vec2::new(-3.0, 5.0),
            Vec2::new(4.0, 7.0),
            Vec2::new(10.0, 5.0),
            Vec2::new(17.0, 1.0),
            Vec2::new(21.0, -1.5),
            Vec2::new(20.0, -3.5),
            Vec2::new(12.0, -4.5),
            Vec2::new(2.0, -5.0),
            Vec2::new(-3.0, -2.0),
        ],
        5,
    );
    b.part(fill(&skull, |p| 0.6 + 0.4 * ((p.y + 5.0) / 12.0)), &fur, Vec3::ZERO, head);
    b.part(
        triangles(&[
            [Vec2::new(16.4, -3.4), Vec2::new(19.0, -3.4), Vec2::new(18.4, -9.5)],
            [Vec2::new(15.0, -3.6), Vec2::new(17.2, -3.6), Vec2::new(16.4, -8.4)],
        ]),
        &teeth,
        Vec3::new(0.0, 0.0, 0.02),
        head,
    );
    b.part(fill(&ellipse_pts(1.4, 1.1, 10), |_| 1.0), &dark, Vec3::new(20.4, -1.8, 0.02), head);
    b.part(fill(&ellipse_pts(1.7, 1.5, 12), |_| 1.0), &milky, Vec3::new(9.0, 2.2, 0.02), head);
    b.part(fill(&ellipse_pts(0.5, 0.5, 8), |_| 1.0), &eye, Vec3::new(9.5, 1.9, 0.03), head);
    b.part(stroke(&[Vec2::new(7.0, 5.5), Vec2::new(9.5, 2.0), Vec2::new(11.5, -1.5)], |_| 0.6, 1.0, 1.0, |_| 1.0), &scar, Vec3::new(0.0, 0.0, 0.04), head);
    for end in [Vec2::new(28.0, 1.0), Vec2::new(28.0, -2.5), Vec2::new(26.5, -5.5)] {
        b.part(stroke(&[Vec2::new(19.0, -1.2), end], |t| 0.35 - 0.25 * t, 1.0, 1.0, |_| 1.0), &dark, Vec3::new(0.0, 0.0, 0.03), head);
    }
    let ear = b.bone(Vec3::new(1.0, 5.5, -0.1), head, BoneKind::Ear);
    // Torn: a bite out of its rim.
    let lobe = [
        Vec2::new(0.0, 0.0),
        Vec2::new(-3.0, 4.0),
        Vec2::new(-1.5, 8.0),
        Vec2::new(2.5, 8.6),
        Vec2::new(5.0, 5.5),
        Vec2::new(3.2, 4.4),
        Vec2::new(4.3, 2.6),
        Vec2::new(3.0, 0.0),
    ];
    b.part(fill(&lobe, |p| 0.7 + 0.3 * (p.y / 8.6)), &fur, Vec3::ZERO, ear);
    b.part(fill(&ellipse_pts(1.5, 2.4, 10), |_| 0.9), &skin, Vec3::new(0.8, 4.2, 0.01), ear);

    let mut rng = Rng(0x5EA7_0A71);
    commands.entity(rat).insert((
        Boss::new(&SPEC, home, mats).staged(),
        KingRat {
            state: State::Stand,
            vel: Vec2::ZERO,
            facing: -1.0,
            cooldown: rng.range(COOLDOWN.0, COOLDOWN.1),
            waited: 0.0,
            fear: 0.0,
            pause: 0.0,
            attention: 0.0,
            chain: 0,
            fury: false,
            retaliate: false,
            introduced: false,
            seed: rng.range(0.0, 100.0),
            dismember: false,
            ground: Ground::map(terrain, Vec2::new(arena.home.x, arena.floor), region),
            rng,
        },
    ));
    rat
}

/// Three hooked claws at a paw.
fn claws(paw: Vec2) -> Mesh {
    let tris: Vec<[Vec2; 3]> = (0..3)
        .map(|i| {
            let c = paw + Vec2::new(0.6 + i as f32 * 0.9, 0.2 - i as f32 * 0.3);
            [c + Vec2::new(-0.4, 0.5), c + Vec2::new(0.4, 0.5), c + Vec2::new(1.6, -0.6)]
        })
        .collect();
    triangles(&tris)
}

struct Builder<'a, 'w, 's> {
    commands: &'a mut Commands<'w, 's>,
    meshes: &'a mut Assets<Mesh>,
    owner: Entity,
}

impl Builder<'_, '_, '_> {
    fn part(&mut self, mesh: Mesh, mat: &Handle<ColorMaterial>, pos: Vec3, parent: Entity) -> Entity {
        let mesh = self.meshes.add(mesh);
        self.commands.spawn((Mesh2d(mesh), MeshMaterial2d(mat.clone()), Transform::from_translation(pos), ChildOf(parent))).id()
    }

    fn bone(&mut self, pos: Vec3, parent: Entity, kind: BoneKind) -> Entity {
        self.commands
            .spawn((Bone { owner: self.owner, kind }, Transform::from_translation(pos), Visibility::default(), ChildOf(parent)))
            .id()
    }
}

/// React to a hit (health and blood are handled in `boss.rs`): every hit frightens it.
fn on_hit(mut hits: MessageReader<BossHit>, mut rats: Query<&mut KingRat>) {
    for hit in hits.read() {
        let Ok(mut r) = rats.get_mut(hit.boss) else { continue };
        r.fear += FEAR_HIT;
        if hit.killed {
            r.vel = Vec2::new(hit.dir.x * 300.0, 480.0);
            r.state = State::Dead { t: 0.0, landed: false };
            r.dismember = true;
        } else if hit.retaliate {
            r.retaliate = true;
        } else if hit.stagger && matches!(r.state, State::Stand | State::Taunt(_) | State::Skid(_) | State::Recover(_) | State::Crouch { .. }) {
            r.vel = Vec2::new(hit.dir.x * 280.0, 0.0);
            r.state = State::Hurt(0.0);
        }
    }
}

/// How high it can jump over `at` (feet) without hitting a ceiling, up to `want`.
fn headroom(terrain: &Terrain, at: Vec2, want: f32) -> f32 {
    let body = FOOT * 2.0 + 10.0;
    let mut h = 0.0;
    while h < want {
        if terrain.solid_at(at + Vec2::Y * (h + body)) {
            return (h - 10.0).max(20.0);
        }
        h += 8.0;
    }
    want
}

fn leap(ground: &Ground, terrain: &Terrain, feet: Vec2, x: f32, height: f32, time: f32, kind: Leap) -> State {
    let to = ground.land(x);
    let height = headroom(terrain, (feet + to) * 0.5, height);
    State::Air { from: feet, to, height, time, t: 0.0, kind, bit: false }
}

/// A wall to climb at the edge of its ground on `side`: (its face x, how high to climb to).
fn wall(ground: &Ground, terrain: &Terrain, side: f32) -> Option<(f32, f32)> {
    let p = ground.pts();
    let end = if side > 0.0 { p[p.len() - 1] } else { p[0] };
    let (edge, floor) = (end.x, end.y);
    let low = floor + FOOT;
    let face = (0..=(WALL_SEARCH / 4.0) as i32).map(|i| edge + side * i as f32 * 4.0).find(|&x| terrain.solid_at(Vec2::new(x, low)))?;
    // How far up it goes (solid all the way, open air in front of it).
    let mut h = 0.0;
    while h < CLIMB_HEIGHT && terrain.solid_at(Vec2::new(face + side * 4.0, low + h)) && !terrain.solid_at(Vec2::new(face - side * FOOT * 1.5, low + h)) {
        h += 8.0;
    }
    (h >= MIN_WALL).then_some((face, floor + h.min(CLIMB_HEIGHT) - 30.0))
}

/// How much the chameleon is paying attention to the rat at `pos` (0..1), as its packs read it.
fn attention(p: &Player, anim: &ChameleonAnim, ppos: Vec2, pos: Vec2) -> f32 {
    if p.dead.is_some() || p.stun > 0.0 {
        return 0.0;
    }
    let to_rat = (pos - chameleon::neck(ppos, p.facing)).normalize_or_zero();
    let body = if p.facing * (pos.x - ppos.x) > 0.0 { 1.0 } else { 0.0 };
    let head = p.aim.dot(to_rat).max(0.0).powi(2);
    let mut a = BODY * body + HEAD * head;
    if anim.busy() && !(anim.slashing() && anim.slash_dir().dot(to_rat) > 0.5) {
        a *= BUSY;
    }
    if !p.grounded {
        a *= AIRBORNE;
    }
    // Coming straight at it: hard to ignore.
    let charge = (p.vel.x * (pos.x - ppos.x).signum() / 240.0).clamp(0.0, 1.0);
    a.max(charge * 0.8)
}

/// The attack for this distance: the maul close up, else the lunge.
fn attack(r: &mut KingRat, dx: f32, side: f32, pace: f32) -> State {
    r.waited = 0.0;
    r.facing = side;
    if dx < MAUL_RANGE {
        State::Rear { t: 0.0, len: REAR * pace }
    } else if dx > LEAP_MIN && r.rng.chance(LEAP_SHARE) {
        State::Crouch { t: 0.0, len: CROUCH * pace * 0.8, leap: true }
    } else {
        State::Crouch { t: 0.0, len: CROUCH * pace, leap: false }
    }
}

#[allow(clippy::too_many_arguments)]
fn think(
    time: Res<Time>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    mut swipe_mesh: Local<Option<Handle<Mesh>>>,
    mut shake: ResMut<Shake>,
    mut stop: ResMut<HitStop>,
    mut bleed: MessageWriter<Bleed>,
    player: Single<(&mut Player, &Transform, &ChameleonAnim), Without<KingRat>>,
    camera: Single<(&Camera, &GlobalTransform), With<MainCamera>>,
    mut rats: Query<(&mut KingRat, &mut Boss, &mut Transform, Option<&mut boss::Sense>)>,
    terrain: Res<Terrain>,
) {
    let dt = time.delta_secs().min(1.0 / 30.0);
    let (mut p, ptf, anim) = player.into_inner();
    let ppos = ptf.translation.truncate();
    // Claw slashes thrown this frame: (where, which way, enraged).
    let mut slashes: Vec<(Vec2, f32, bool)> = Vec::new();

    for (mut r, mut boss, mut tf, sense) in &mut rats {
        let pos = tf.translation.truncate();
        let to_player = ppos - pos;
        let alive = p.dead.is_none();
        let aggro = alive && boss.awake;
        let ground = r.ground.clone();
        let ground_at = |x: f32, y: f32| terrain.surface(Vec2::new(x, y - FOOT + 2.0 * STEP_UP), 3000.0);
        let floor = ground_at(pos.x, pos.y).unwrap_or(ground.y_at(pos.x));
        let feet = Vec2::new(pos.x, floor);
        if let Some(mut s) = sense {
            s.goal = aggro.then_some(ppos);
            s.target = match r.state {
                State::Lunge { to, .. } => Some(Vec2::new(to, floor)),
                State::Air { to, .. } => Some(to),
                _ => aggro.then_some(ground.land(ppos.x)),
            };
            s.sees_player = terrain.raycast(pos, ppos).is_none();
            s.path.clear();
        }
        let side = if to_player.x >= 0.0 { 1.0 } else { -1.0 };
        let on_screen = match (camera.0.world_to_viewport(camera.1, pos.extend(0.0)), camera.0.logical_viewport_size()) {
            (Ok(v), Some(size)) => v.x > SCREEN_MARGIN && v.y > SCREEN_MARGIN && v.x < size.x - SCREEN_MARGIN && v.y < size.y - SCREEN_MARGIN,
            _ => false,
        };
        if !r.introduced {
            boss.staged = true;
        }
        let enraged = boss.enraged();
        let dx = to_player.x.abs();
        let touching = alive && to_player.length() < RADIUS + 26.0;
        let a = if aggro { attention(&p, anim, ppos, pos) } else { 0.0 };
        // The chameleon suddenly turning to it, close: a fright.
        if a - r.attention > 0.35 && dx < WARY {
            r.fear += 0.25;
            r.pause = r.pause.max(0.25);
        }
        r.attention = a;
        r.fear = (r.fear - FEAR_FADE * dt).max(0.0);
        // Backed against the edge of its ground, the chameleon close: nowhere to run.
        let room = if side > 0.0 { pos.x - ground.lo() } else { ground.hi() - pos.x };
        let cornered = aggro && dx < CORNER_DIST && room < CORNER_ROOM;
        let pace = (if enraged { RAGE_PACE } else { 1.0 }) * if cornered { FURY_PACE } else { 1.0 };

        // Over the chameleon's head, away from the corner (lands past it).
        let vault = |r: &mut KingRat| {
            r.facing = side;
            r.fear = 0.0;
            leap(&ground, &terrain, feet, ppos.x + side * VAULT_PAST, VAULT_HEIGHT, VAULT_TIME, Leap::Vault)
        };

        // Hit too many times in a row: cornered, it mauls back at once or vaults out; else it runs.
        if std::mem::take(&mut r.retaliate) && matches!(r.state, State::Stand | State::Hurt(_) | State::Skid(_) | State::Recover(_) | State::Taunt(_)) {
            r.state = if cornered && r.rng.chance(VAULT_SHARE) {
                vault(&mut r)
            } else if cornered {
                r.fury = true;
                attack(&mut r, 0.0, side, FURY_PACE * 0.6)
            } else {
                r.fear = 0.0;
                State::Bolt { t: 0.0, dir: -side }
            };
        }

        r.state = match r.state {
            State::Dead { t: t0, landed } => {
                let td = t0 + dt;
                for at in [0.25, 0.6] {
                    if t0 < at && td >= at {
                        bleed.write(Bleed { pos, dir: Vec2::Y, spread: PI, speed: 520.0, amount: 60, fluid: Fluid::Crimson, sticky: true });
                    }
                }
                let mut landed = landed;
                let mut next = pos;
                if !landed {
                    r.vel.y -= 1400.0 * dt;
                    next += r.vel * dt;
                    let rest = floor + 9.0 * SCALE;
                    if ground_at(next.x, pos.y).is_some() && next.y <= rest && r.vel.y < 0.0 {
                        next.y = rest;
                        r.vel = Vec2::new(r.vel.x * 0.3, 0.0);
                        landed = true;
                        shake.add(0.3);
                        bleed.write(Bleed { pos, dir: Vec2::Y, spread: 1.3, speed: 240.0, amount: 40, fluid: Fluid::Crimson, sticky: true });
                    }
                } else {
                    r.vel.x *= 1.0 - 5.0 * dt;
                    next.x += r.vel.x * dt;
                }
                if td < 3.5 && r.rng.chance(dt * 12.0) {
                    bleed.write(Bleed { pos, dir: Vec2::Y, spread: 1.0, speed: 140.0, amount: 2, fluid: Fluid::Crimson, sticky: true });
                }
                tf.translation.x = next.x;
                tf.translation.y = next.y;
                State::Dead { t: td, landed }
            }
            State::Hurt(s) => {
                r.vel.x *= 1.0 - 6.0 * dt;
                if s < 0.25 {
                    State::Hurt(s + dt)
                } else if r.fear >= 1.0 && !cornered {
                    r.fear = 0.3;
                    State::Bolt { t: 0.0, dir: -side }
                } else {
                    State::Stand
                }
            }
            State::Stand => {
                r.cooldown -= dt;
                r.pause = (r.pause - dt).max(0.0);
                if !aggro {
                    r.introduced = false;
                    r.fury = false;
                    // Back home, at a walk.
                    let home = boss.home.x - pos.x;
                    r.vel.x = if home.abs() > 40.0 { home.signum() * SNEAK } else { 0.0 };
                    if r.vel.x != 0.0 {
                        r.facing = r.vel.x.signum();
                    }
                    State::Stand
                } else if !r.introduced {
                    r.vel.x = 0.0;
                    r.facing = side;
                    if on_screen {
                        r.introduced = true;
                        boss.staged = false;
                        shake.add(0.15);
                        State::Taunt(0.0)
                    } else {
                        State::Stand
                    }
                } else {
                    r.waited += dt;
                    r.fury = cornered;
                    let bold = a < DARE || r.waited > PATIENCE || cornered || (enraged && r.rng.chance(dt * 0.8));
                    if r.cooldown <= 0.0 && bold && dx < LUNGE_REACH {
                        r.vel.x = 0.0;
                        r.cooldown = r.rng.range(COOLDOWN.0, COOLDOWN.1) * pace;
                        r.chain = if enraged { 2 } else { 1 };
                        // Cornered and watched: as often as not it gets out over the chameleon's head.
                        if cornered && a >= DARE && r.rng.chance(VAULT_SHARE) {
                            vault(&mut r)
                        } else {
                            attack(&mut r, dx, side, pace)
                        }
                    } else if r.fear >= 1.0 && !cornered {
                        r.fear = 0.3;
                        State::Bolt { t: 0.0, dir: -side }
                    } else {
                        // Keeps its distance: farther while watched, creeping in while ignored.
                        let want = CREEP + (WARY - CREEP) * a;
                        r.facing = side;
                        r.vel.x = if r.pause > 0.0 {
                            0.0
                        } else if dx > want + 40.0 {
                            if r.rng.chance(dt * SNIFF) {
                                r.pause = SNIFF_TIME;
                            }
                            side * SNEAK
                        } else if dx < want - 40.0 && room > 10.0 {
                            -side * BACK_OFF
                        } else {
                            0.0
                        };
                        State::Stand
                    }
                }
            }
            State::Taunt(s) => {
                r.vel.x = 0.0;
                r.facing = side;
                if to_player.length() < TAUNT_BREAK {
                    r.waited = 0.0;
                    State::Rear { t: REAR * 0.4, len: REAR }
                } else if s >= TAUNT_TIME {
                    r.cooldown = r.rng.range(0.4, 0.8);
                    State::Stand
                } else {
                    State::Taunt(s + dt)
                }
            }
            State::Crouch { t: s, len, leap: leaping } => {
                r.vel.x = 0.0;
                r.facing = side;
                if s >= len {
                    // Commits to where the chameleon is now.
                    shake.add(0.08);
                    let dir = side;
                    if leaping {
                        leap(&ground, &terrain, feet, ppos.x + dir * LEAP_OVERSHOOT, LEAP_HEIGHT, LEAP_TIME, Leap::Pounce)
                    } else {
                        let reach = dx.min(LUNGE_REACH);
                        State::Lunge { to: ground.inside(pos.x + dir * (reach + OVERSHOOT)), dir }
                    }
                } else {
                    State::Crouch { t: s + dt, len, leap: leaping }
                }
            }
            State::Lunge { to, dir } => {
                r.facing = dir;
                r.vel.x = dir * LUNGE_SPEED * if enraged { 1.1 } else { 1.0 };
                if touching {
                    let before = p.wounds;
                    boss::hurt_player(&mut p, ppos, pos, BITE_DAMAGE, &mut bleed, (&mut shake, &mut stop));
                    if p.wounds != before {
                        // Thrown, then the rat runs off with the taste of it.
                        p.vel = Vec2::new(dir * BITE_LAUNCH.x, BITE_LAUNCH.y);
                        stop.add(0.06);
                    }
                    r.chain = 0;
                    r.vel.x *= 0.3;
                    slashes.push((pos + Vec2::new(dir * HALF_W * 0.7, 0.0), dir, enraged));
                    State::Bolt { t: 0.0, dir: -dir }
                } else if (to - pos.x) * dir <= r.vel.x.abs() * dt || (ground.inside(pos.x + r.vel.x * dt) - pos.x).abs() < 1.0 {
                    shake.add(0.06);
                    r.chain = r.chain.saturating_sub(1);
                    slashes.push((pos + Vec2::new(dir * HALF_W * 0.7, 0.0), dir, enraged));
                    State::Skid(0.0)
                } else {
                    State::Lunge { to, dir }
                }
            }
            State::Skid(s) => {
                r.vel.x *= 1.0 - 7.0 * dt;
                if r.chain > 0 && alive && s >= 0.25 {
                    // Enraged: wheels round and goes again.
                    r.facing = side;
                    State::Crouch { t: CROUCH * 0.5, len: CROUCH * pace, leap: false }
                } else if s >= SKID * if enraged { 0.7 } else { 1.0 } {
                    State::Stand
                } else {
                    State::Skid(s + dt)
                }
            }
            State::Rear { t: s, len } => {
                r.vel.x = 0.0;
                let s1 = s + dt;
                if s < len && s1 >= len {
                    // The slam: both paws come down in front of it.
                    let reach = (ppos.x - pos.x) * r.facing;
                    let hit = alive && reach > -RADIUS * 0.3 && reach < RADIUS + MAUL_REACH && (ppos.y - pos.y).abs() < FOOT + 50.0;
                    shake.add(0.3);
                    slashes.push((pos + Vec2::new(r.facing * HALF_W * 0.8, -FOOT * 0.4), r.facing, enraged));
                    if hit {
                        let before = p.wounds;
                        boss::hurt_player(&mut p, ppos, pos, MAUL_DAMAGE, &mut bleed, (&mut shake, &mut stop));
                        if p.wounds != before {
                            p.vel = Vec2::new(r.facing * MAUL_LAUNCH.x, MAUL_LAUNCH.y);
                            stop.add(0.06);
                        }
                    }
                }
                if s1 >= len + SLAM {
                    State::Recover(0.0)
                } else {
                    State::Rear { t: s1, len }
                }
            }
            State::Recover(s) => {
                r.vel.x = 0.0;
                if s >= RECOVER * if enraged { 0.7 } else { 1.0 } {
                    // Still a rat: often it runs after striking.
                    if !cornered && dx < MAUL_RANGE && r.rng.chance(0.5) {
                        State::Bolt { t: 0.0, dir: -side }
                    } else {
                        State::Stand
                    }
                } else {
                    State::Recover(s + dt)
                }
            }
            State::Bolt { t: s, dir } => {
                r.facing = dir;
                let k = 1.0 - (s / BOLT_TIME).min(1.0);
                r.vel.x = dir * BOLT * (0.35 + 0.65 * k);
                // Ran into the edge of its ground: up the wall if there is one, else it turns to face the chameleon.
                let stuck = (ground.inside(pos.x + r.vel.x * dt) - pos.x).abs() < 1.0;
                let up = if stuck && aggro && r.rng.chance(CLIMB_SHARE) { wall(&ground, &terrain, dir) } else { None };
                if let Some((face, top)) = up {
                    State::Climb { side: dir, face, top }
                } else if s >= BOLT_TIME || stuck {
                    r.vel.x = 0.0;
                    r.facing = side;
                    r.cooldown = r.cooldown.max(if stuck { 0.3 } else { 0.7 });
                    State::Stand
                } else {
                    State::Bolt { t: s + dt, dir }
                }
            }
            State::Air { from, to, height, time, t: s, kind, mut bit } => {
                let s = s + dt;
                let k = (s / time).min(1.0);
                let next = Vec2::new(from.x + (to.x - from.x) * k, from.y + (to.y - from.y) * k + FOOT + height * 4.0 * k * (1.0 - k));
                r.vel = (next - pos) / dt.max(1e-4);
                if (to.x - from.x).abs() > 1.0 {
                    r.facing = (to.x - from.x).signum();
                }
                tf.translation.x = next.x;
                tf.translation.y = next.y;
                // The leaping bite and the drop bite whatever they land on (once).
                if kind.bites() && !bit && touching {
                    bit = true;
                    let before = p.wounds;
                    boss::hurt_player(&mut p, ppos, pos, BITE_DAMAGE, &mut bleed, (&mut shake, &mut stop));
                    if p.wounds != before {
                        p.vel = Vec2::new(r.facing * BITE_LAUNCH.x, BITE_LAUNCH.y);
                        stop.add(0.06);
                    }
                }
                if k < 1.0 {
                    State::Air { from, to, height, time, t: s, kind, bit }
                } else {
                    r.vel = Vec2::ZERO;
                    shake.add(match kind {
                        Leap::Vault => 0.1,
                        Leap::Hop(_) => 0.03,
                        _ => 0.2,
                    });
                    if matches!(kind, Leap::Pounce | Leap::Drop) {
                        slashes.push((tf.translation.truncate() + Vec2::new(r.facing * HALF_W * 0.7, 0.0), r.facing, enraged));
                    }
                    match kind {
                        Leap::Hop(Resume::Stand) => State::Stand,
                        Leap::Hop(Resume::Bolt(dir)) => State::Bolt { t: BOLT_TIME * 0.3, dir },
                        Leap::Hop(Resume::Lunge { to, dir }) if !bit => State::Lunge { to, dir },
                        Leap::Vault => {
                            r.facing = side;
                            r.cooldown = r.cooldown.min(0.5);
                            State::Stand
                        }
                        // Bit: off it runs. Missed: it skids (the opening).
                        _ if bit => State::Bolt { t: 0.0, dir: -side },
                        _ => {
                            r.chain = r.chain.saturating_sub(1);
                            State::Skid(0.0)
                        }
                    }
                }
            }
            State::Climb { side: wall_side, face, top } => {
                r.facing = wall_side;
                r.vel = Vec2::new(0.0, CLIMB_SPEED);
                // Onto the wall, belly to it, then up.
                let x = face - wall_side * FOOT;
                tf.translation.x = pos.x + (x - pos.x) * (1.0 - (-20.0 * dt).exp());
                tf.translation.y = (pos.y + CLIMB_SPEED * dt).min(top);
                if !aggro || tf.translation.y >= top {
                    r.vel = Vec2::ZERO;
                    State::Cling { side: wall_side, t: 0.0, len: r.rng.range(CLING.0, CLING.1) * pace }
                } else {
                    State::Climb { side: wall_side, face, top }
                }
            }
            State::Cling { side: wall_side, t: s, len } => {
                r.facing = wall_side;
                r.vel = Vec2::ZERO;
                if s >= len || !aggro {
                    // Drops onto the chameleon (or just back down).
                    r.waited = 0.0;
                    r.chain = 1;
                    let target = if aggro { ppos.x } else { pos.x - wall_side * 150.0 };
                    shake.add(0.06);
                    leap(&ground, &terrain, pos - Vec2::Y * FOOT, target, 60.0, DROP_TIME, Leap::Drop)
                } else {
                    State::Cling { side: wall_side, t: s + dt, len }
                }
            }
        };

        // On the ground: follow the floor under it.
        if !r.airborne() && !matches!(r.state, State::Dead { .. }) {
            let x0 = tf.translation.x;
            let x = ground.inside(x0 + r.vel.x * dt);
            if let Some(to) = ground.hop(x0, x) {
                // A ledge, a drop or a gap on the way: it hops it and carries on.
                let resume = match r.state {
                    State::Bolt { dir, .. } => Resume::Bolt(dir),
                    State::Lunge { to, dir } => Resume::Lunge { to, dir },
                    _ => Resume::Stand,
                };
                let from = Vec2::new(x0, ground.y_at(x0));
                let height = headroom(&terrain, (from + to) * 0.5, 40.0 + (to.y - from.y).max(0.0) * 0.6);
                let time = HOP_TIME * (1.0 + (to - from).length() / 400.0).min(1.6);
                r.state = State::Air { from, to, height, time, t: 0.0, kind: Leap::Hop(resume), bit: false };
            } else {
                tf.translation.x = x;
                tf.translation.y = ground.y_at(x) + FOOT;
            }
        }
    }

    for (at, dir, enraged) in slashes {
        let mesh = swipe_mesh.get_or_insert_with(|| meshes.add(crescent())).clone();
        let material = materials.add(ColorMaterial { color: SWIPE_COLOR, alpha_mode: AlphaMode2d::Blend, ..default() });
        let size = if enraged { SWIPE_RAGE } else { 1.0 };
        commands.spawn((
            Mesh2d(mesh),
            MeshMaterial2d(material.clone()),
            Transform::from_translation(at.extend(13.0)).with_scale(Vec3::new(dir * size, size, 1.0)),
            Swipe { vel: Vec2::new(dir * SWIPE_RANGE * size / SWIPE_TIME, 0.0), age: 0.0, size, hit: false, material },
            BossStuff,
        ));
    }
}

/// Three parallel claw marks curving forward (+x), thick in the middle, sharp at the ends.
fn crescent() -> Mesh {
    let mut tris: Vec<[Vec2; 3]> = Vec::new();
    for (k, r) in [(0.0, 26.0), (1.0, 19.0), (2.0, 12.0)] {
        let pts: Vec<Vec2> = (0..=12).map(|i| Vec2::from_angle(-1.1 + 2.2 * i as f32 / 12.0) * r - Vec2::X * k * 2.0).collect();
        let n = pts.len() - 1;
        for i in 0..n {
            let (a, b) = (pts[i], pts[i + 1]);
            let w = |j: usize| 3.2 * (PI * j as f32 / n as f32).sin() * (1.0 - k * 0.2);
            let (na, nb) = (a.normalize() * w(i), b.normalize() * w(i + 1));
            tris.push([a - na * 0.5, b - nb * 0.5, b + nb * 0.5]);
            tris.push([a - na * 0.5, b + nb * 0.5, a + na * 0.5]);
        }
    }
    triangles(&tris)
}

/// Claw slashes fly forward a short way, slowing and fading; they cut the chameleon once, and walls stop them.
fn swipes(
    time: Res<Time>,
    mut commands: Commands,
    mut shake: ResMut<Shake>,
    mut stop: ResMut<HitStop>,
    mut bleed: MessageWriter<Bleed>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    terrain: Res<Terrain>,
    player: Single<(&mut Player, &Transform), Without<Swipe>>,
    mut swipes: Query<(Entity, &mut Swipe, &mut Transform), Without<Player>>,
) {
    let dt = time.delta_secs().min(1.0 / 30.0);
    let (mut p, ptf) = player.into_inner();
    let ppos = ptf.translation.truncate();
    for (e, mut s, mut tf) in &mut swipes {
        s.age += dt;
        let k = s.age / SWIPE_TIME;
        let pos = tf.translation.truncate();
        if k >= 1.0 || terrain.solid_at(pos + s.vel.normalize_or_zero() * 20.0 * s.size) {
            commands.entity(e).despawn();
            continue;
        }
        // Fast out of the claws, slowing down.
        let step = s.vel * dt * 2.0 * (1.0 - k);
        tf.translation += step.extend(0.0);
        let grow = 1.0 + 0.4 * k;
        tf.scale = Vec3::new(s.vel.x.signum() * s.size * grow, s.size * grow, 1.0);
        if !s.hit && p.dead.is_none() && pos.distance(ppos) < (SWIPE_RADIUS * s.size + player::HALF.y) {
            s.hit = true;
            boss::hurt_player(&mut p, ppos, pos, SWIPE_DAMAGE, &mut bleed, (&mut shake, &mut stop));
        }
        if let Some(mut m) = materials.get_mut(&s.material) {
            m.color = SWIPE_COLOR.with_alpha(0.85 * (1.0 - k * k));
        }
    }
}

/// The chameleon came back: start the fight over (`boss.rs` heals it).
fn reset_on_respawn(mut commands: Commands, mut respawned: MessageReader<Respawned>, mut rats: Query<&mut KingRat>, swipes: Query<Entity, With<Swipe>>) {
    if respawned.read().count() == 0 {
        return;
    }
    for e in &swipes {
        commands.entity(e).despawn();
    }
    for mut r in &mut rats {
        if matches!(r.state, State::Dead { .. }) {
            continue;
        }
        r.state = State::Stand;
        r.introduced = false;
        r.vel = Vec2::ZERO;
        r.cooldown = 1.5;
        r.chain = 0;
        r.fear = 0.0;
        r.waited = 0.0;
        r.fury = false;
    }
}

/// A freshly killed King Rat: the tail tears off at the root, and the torn ear goes too.
fn dismember(mut commands: Commands, mut rats: Query<&mut KingRat>, bones: Query<(Entity, &Bone, &GlobalTransform)>) {
    let mut done = Vec::new();
    for (e, bone, gt) in &bones {
        let Ok(r) = rats.get(bone.owner) else { continue };
        if !r.dismember {
            continue;
        }
        done.push(bone.owner);
        let f = r.facing;
        let (vel, spin, bleeds) = match bone.kind {
            BoneKind::Tail => (Vec2::new(-f * 260.0, 560.0), 7.0, Some(Fluid::Crimson)),
            BoneKind::Ear => (Vec2::new(f * 180.0, 500.0), 10.0, None),
            _ => continue,
        };
        commands.entity(e).remove::<Bone>();
        boss::tear_off(&mut commands, e, gt, vel, spin, bleeds);
    }
    done.sort();
    done.dedup();
    for owner in done {
        if let Ok(mut r) = rats.get_mut(owner) {
            r.dismember = false;
        }
    }
}

fn animate(time: Res<Time>, rats: Query<&KingRat>, mut bones: Query<(&Bone, &mut Transform)>) {
    let t = time.elapsed_secs();
    let dt = time.delta_secs();
    for (bone, mut tf) in &mut bones {
        let Ok(r) = rats.get(bone.owner) else { continue };
        let tt = t + r.seed;
        let (dead_t, landed) = match r.state {
            State::Dead { t, landed } => (Some(t), landed),
            _ => (None, false),
        };
        // 0..1: crouched low (the lunge's tell, and skidding).
        let crouch = match r.state {
            State::Crouch { t, len, .. } => (t / len.max(0.01)).min(1.0),
            State::Skid(s) => (1.0 - s / SKID).clamp(0.0, 1.0) * 0.5,
            _ => 0.0,
        };
        // 0..1: up on its hind legs (the maul's tell and the taunt), back down for the slam.
        let rear = match r.state {
            State::Rear { t, len } if t < len => (t / len.max(0.01)).min(1.0),
            State::Rear { t, len } => 1.0 - ((t - len) / SLAM).min(1.0),
            State::Taunt(s) => (s / 0.3).min(1.0) * 0.7,
            _ => 0.0,
        };
        // Jaws open: bared in the tell, wide in the lunge, gnashing in the taunt.
        let jaw = match r.state {
            State::Crouch { .. } => 0.25 + 0.1 * (tt * 30.0).sin(),
            State::Lunge { .. } => 0.55,
            State::Air { kind, .. } if kind.bites() => 0.6,
            State::Cling { .. } => 0.3 + 0.2 * (tt * 16.0).sin().abs(),
            State::Rear { .. } => 0.4,
            State::Taunt(_) => 0.25 + 0.25 * (tt * 18.0).sin().abs(),
            State::Hurt(_) => 0.45,
            _ if dead_t.is_some() => 0.5,
            _ => 0.03 + 0.03 * (tt * 9.0).sin(),
        };
        let speed = if matches!(r.state, State::Climb { .. }) { r.vel.length() } else { r.vel.x.abs() };
        let climbing = matches!(r.state, State::Climb { .. } | State::Cling { .. });
        let flying = matches!(r.state, State::Air { .. });
        // Running gait: the legs cycle with its speed.
        let gait = (speed / 900.0).min(1.0);
        let phase = tt * (6.0 + speed * 0.022);
        let shiver = if matches!(r.state, State::Crouch { .. }) { (tt * 55.0).sin() * 0.02 * crouch } else { 0.0 };
        match bone.kind {
            BoneKind::Rig => {
                tf.scale.x += (r.facing * SCALE - tf.scale.x) * (1.0 - (-18.0 * dt).exp());
                let stretch = if matches!(r.state, State::Lunge { .. } | State::Bolt { .. }) { 0.08 } else { 0.0 };
                tf.scale.y = SCALE * (1.0 - 0.12 * crouch - stretch * 0.5);
                let flip = tf.scale.x.signum();
                let angle = if let Some(td) = dead_t {
                    if landed { PI } else { PI * (td * 2.2).min(1.0) }
                } else if climbing {
                    // Up the wall nose first, belly to it.
                    flip * PI * 0.5 + (phase * 2.0).sin() * 0.03 * gait
                } else if flying {
                    flip * (r.vel.y * 0.0007).clamp(-0.5, 0.5)
                } else {
                    // Rears up nose first (pivoting on the hind feet), dips nose-down to crouch.
                    flip * (0.75 * rear - 0.06 * crouch + shiver) + (phase * 2.0).sin() * 0.025 * gait
                };
                tf.rotation = Quat::from_rotation_z(angle);
                // Keep the hind feet planted while rearing.
                let pivot = Vec2::new(tf.scale.x * -14.0, tf.scale.y * -12.0);
                let held = if dead_t.is_none() && !climbing && !flying { pivot - Vec2::from_angle(angle).rotate(pivot) } else { Vec2::ZERO };
                let bob = -(phase * 2.0).sin().abs() * 1.2 * SCALE * gait;
                tf.translation = Vec3::new(held.x, held.y - crouch * 3.0 * SCALE + bob, tf.translation.z);
            }
            BoneKind::Head => {
                let angle = match r.state {
                    _ if dead_t.is_some() => -0.4,
                    // Head up and back to hiss.
                    State::Taunt(s) => 0.2 + (tt * 14.0).sin() * 0.08 * (s / 0.3).min(1.0),
                    State::Crouch { .. } => -0.15 * crouch,
                    State::Lunge { .. } => -0.1,
                    // Clinging: the head turned out from the wall, toward the chameleon below.
                    State::Cling { .. } => -0.9 + (tt * 15.0).sin() * 0.05,
                    // Sniffing about.
                    _ => (tt * 2.4).sin() * 0.06 + (tt * 17.0).sin() * 0.015,
                };
                tf.rotation = Quat::from_rotation_z(angle);
            }
            BoneKind::Jaw => tf.rotation = Quat::from_rotation_z(-jaw),
            BoneKind::Ear => {
                // Flattened back when it means harm or runs.
                let flat = matches!(r.state, State::Crouch { .. } | State::Lunge { .. } | State::Bolt { .. } | State::Rear { .. } | State::Air { .. } | State::Climb { .. });
                let angle = if flat { 0.7 } else { (tt * 3.1).sin() * 0.1 };
                tf.rotation = Quat::from_rotation_z(angle);
            }
            BoneKind::Tail | BoneKind::TailTip => {
                let tip = bone.kind == BoneKind::TailTip;
                let lash = if crouch > 0.0 || matches!(r.state, State::Taunt(_)) { 1.0 } else { 0.0 };
                let angle = if let Some(td) = dead_t {
                    (tt * 12.0).sin() * 0.3 * (1.0 - td / 4.0).max(0.0)
                } else if matches!(r.state, State::Lunge { .. } | State::Bolt { .. } | State::Air { .. } | State::Climb { .. }) {
                    // Streaming out behind.
                    if tip { -0.1 } else { 0.15 }
                } else {
                    let k = if tip { 1.6 } else { 1.0 };
                    (tt * (2.0 + 9.0 * lash) + if tip { 1.1 } else { 0.0 }).sin() * (0.12 + 0.25 * lash) * k
                };
                tf.rotation = Quat::from_rotation_z(angle);
            }
            BoneKind::Leg { pair, near } => {
                let group = if near { 0.0 } else { PI };
                let angle = if let Some(td) = dead_t {
                    let fade = (1.0 - td / 6.0).max(0.12);
                    (tt * 26.0 + pair as f32 * 1.9 + group).sin() * 0.55 * fade
                } else if pair == 0 && rear > 0.0 {
                    // Forepaws up and clawing.
                    1.2 * rear + (tt * 16.0 + group).sin() * 0.15 * rear
                } else if matches!(r.state, State::Lunge { .. } | State::Air { .. }) {
                    // Stretched out: front reaching, hind kicking back.
                    if pair == 0 { 0.7 } else { -0.6 }
                } else {
                    (phase + group + pair as f32 * PI * 0.5).sin() * 0.55 * gait
                };
                tf.rotation = Quat::from_rotation_z(angle);
            }
        }
    }
}

/// The sewer's foul air: until the King Rat falls, every breath in the sewer
/// (`Style::Sewer`) is poison. After `AIR_GRACE` seconds the chameleon starts
/// coughing up health, one tick every `AIR_EVERY`; dark wisps of the stench
/// rise around it while it is in there.
#[derive(Component)]
struct Wisp {
    vel: Vec2,
    age: f32,
    life: f32,
    material: Handle<ColorMaterial>,
}

#[allow(clippy::too_many_arguments)]
fn foul_air(
    time: Res<Time>,
    mut commands: Commands,
    mut materials: ResMut<Assets<ColorMaterial>>,
    cloud: Res<Cloud>,
    defeated: Res<Defeated>,
    terrain: Res<Terrain>,
    mut shake: ResMut<Shake>,
    mut exposed: Local<f32>,
    mut puff: Local<f32>,
    mut rng: Local<Option<Rng>>,
    player: Single<(&mut Player, &Transform), Without<Wisp>>,
    mut wisps: Query<(Entity, &mut Wisp, &mut Transform), Without<Player>>,
) {
    let dt = time.delta_secs();
    let (mut p, ptf) = player.into_inner();
    let ppos = ptf.translation.truncate();
    let rng = rng.get_or_insert(Rng(0xF0_0A1A));
    let foul = !defeated.sewer() && p.dead.is_none() && terrain.style_at(ppos) == Style::Sewer;
    if foul {
        *exposed += dt;
        if *exposed >= AIR_GRACE {
            *exposed -= AIR_EVERY;
            if !matches!(p.choke(AIR_DAMAGE), Hurt::Ignored) {
                shake.add(0.15);
            }
        }
        // The stench, thicker the longer it breathes it.
        *puff -= dt;
        while *puff <= 0.0 {
            *puff += 0.12;
            let at = ppos + Vec2::new(rng.range(-30.0, 30.0), rng.range(-player::HALF.y, player::HALF.y));
            let material = materials.add(ColorMaterial { color: Color::srgba(0.16, 0.15, 0.12, 0.0), alpha_mode: AlphaMode2d::Blend, ..default() });
            commands.spawn((
                Mesh2d(cloud.0.clone()),
                MeshMaterial2d(material.clone()),
                Transform::from_translation(at.extend(10.5)).with_scale(Vec3::splat(rng.range(14.0, 26.0))),
                Wisp { vel: Vec2::new(rng.range(-18.0, 18.0), rng.range(20.0, 45.0)), age: 0.0, life: rng.range(1.0, 1.8), material },
            ));
        }
    } else {
        // Out in clean air the lungs clear (back in, the grace starts over).
        *exposed = (*exposed - dt * 2.0).max(0.0).min(AIR_GRACE * 0.5);
    }
    for (e, mut w, mut tf) in &mut wisps {
        w.age += dt;
        let k = w.age / w.life;
        if k >= 1.0 {
            commands.entity(e).despawn();
            continue;
        }
        tf.translation += (w.vel * dt).extend(0.0);
        tf.scale *= 1.0 + 0.5 * dt;
        if let Some(mut m) = materials.get_mut(&w.material) {
            m.color = m.color.with_alpha(0.35 * (k * PI).sin());
        }
    }
}
