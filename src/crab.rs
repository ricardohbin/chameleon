//! The Dambreaker: a giant crab (about five times the chameleon) that lives on
//! the shore of the broken dam's flood, and in it.
//!
//! On land it scuttles sideways after the chameleon and **pinches** with its
//! small claw when close (`Pinch`: the claw opens, the tell, then snaps). It
//! **rams** sideways across its shore (`Rear`: legs drumming, the tell →
//! `Scuttle`). Its great claw is the **crusher**: it raises it high and charges
//! it (`Wind`: the claw shakes and glows indigo, a glow grows on the floor where
//! it will land) for `WIND` seconds, then slams it down: whatever is under it
//! dies at once (`CRUSH_*`, one-hit kill). The claw sticks in the ground for a
//! while after (`Stuck`): the big opening.
//! Every so often (`DIVE_EVERY`), when hit too many times in a row, or when the
//! chameleon is in the water, it **dives** into the flood (`Under`: only its eye
//! stalks show above the surface, blows glance off it) and glides along under
//! the surface, then **bursts out** onto the chameleon (`Rise`: the surface
//! boils, the tell → a leap out, slamming down where it lands). If the chameleon
//! is in the water with it, it goes for it there.
//! Face-off: it rears up and clacks both claws.
//!
//! It bleeds dark blue `#5b6ee1` and gives the chameleon the **power blow**
//! (hold attack `chameleon::BLOW_CHARGE` seconds, release; the charge aura is
//! drawn here, `charge_aura`). On death it is thrown on its back, the great
//! claw and an eye stalk torn off, the legs twitching. Health, getting hit and
//! the death spectacle live in `boss.rs`.

use std::f32::consts::{PI, TAU};

use bevy::{prelude::*, sprite_render::AlphaMode2d};

use crate::{
    boss::{self, Arena, Boss, BossHit, BossSpec, BossStuff},
    camera::{self, HitStop, MainCamera, Shake},
    chameleon::{self, ChameleonAnim},
    fumes::Cloud,
    gore::{Bleed, Fluid},
    player::{self, Player, Respawned},
    shapes::*,
    terrain::{Terrain, STEP_UP},
    water::{Splash, Waters},
};

/// Its color: what the chameleon earns, its blood, the health bar.
pub const INDIGO: Color = Color::srgb(0x5b as f32 / 255.0, 0x6e as f32 / 255.0, 0xe1 as f32 / 255.0);

pub(crate) static SPEC: BossSpec = BossSpec {
    name: "THE DAMBREAKER",
    hp: 20,
    radius: RADIUS,
    blood: Fluid::Indigo,
    color: INDIGO,
    aggro: AGGRO,
    // A mountain of crab meat.
    food: 6.0,
};

/// Size of the boss relative to its drawing (about five times the chameleon).
const SCALE: f32 = 4.5;
/// Hurtbox radius around the body center.
const RADIUS: f32 = 24.0 * SCALE;
/// Body center height above the floor when standing.
const FOOT: f32 = 20.0 * SCALE;
/// Half its width: how close to the arena edges its center may go.
const HALF_W: f32 = 24.0 * SCALE;
const AGGRO: f32 = 1400.0;
/// How far inside the screen edge (px) it must be to count as in view.
const SCREEN_MARGIN: f32 = 60.0;

/// The face-off: claws up, clacking.
const TAUNT_TIME: f32 = 1.8;
const TAUNT_BREAK: f32 = 260.0;

/// Sideways walk.
const WALK: f32 = 150.0;
/// Seconds between attacks (enraged: × `RAGE_PACE`).
const COOLDOWN: (f32, f32) = (0.7, 1.3);
const RAGE_PACE: f32 = 0.7;

// The pinch (close): the small claw snaps.
const PINCH_RANGE: f32 = 55.0 * SCALE;
const PINCH_TELL: f32 = 0.35;
const PINCH_TIME: f32 = 0.25;
const PINCH_DAMAGE: i32 = 1;

// The ram: sideways across the shore.
const REAR: f32 = 0.45;
const SCUTTLE_SPEED: f32 = 720.0;
const SCUTTLE_TIME: f32 = 0.9;
const SCUTTLE_DAMAGE: i32 = 1;
const SCUTTLE_SHARE: f32 = 0.5;

// The crusher: charged, then slammed down. Kills at once.
const CRUSH_RANGE: f32 = 70.0 * SCALE;
const WIND: f32 = 1.8;
/// Enraged it charges quicker.
const RAGE_WIND: f32 = 1.4;
const CRUSH_SHARE: f32 = 0.4;
/// At least this long between two crushers.
const CRUSH_GAP: f32 = 6.0;
/// Where the claw lands (in front of its center) and the half width of what it crushes.
const CRUSH_AT: f32 = 40.0 * SCALE;
const CRUSH_HALF: f32 = 26.0 * SCALE;
/// Anything higher than this above the floor there is out of its reach.
const CRUSH_HEIGHT: f32 = 130.0;
const CRUSH_TIME: f32 = 0.18;
/// The claw stuck in the ground: the opening.
const STUCK: f32 = 1.8;

// The water.
/// It dives this often (seconds of fighting on land; enraged × `RAGE_PACE`).
const DIVE_EVERY: f32 = 11.0;
/// A pool counts as its own if its surface is at most this far below its floor and this far beside it.
const DIVE_DROP: f32 = 900.0;
const DIVE_GAP: f32 = 700.0;
/// Under water its body is this far below the surface (the eye stalks stick out).
const SUBMERGE: f32 = 9.0 * SCALE;
const SWIM: f32 = 280.0;
/// Seconds under before it may burst out, and at most.
const UNDER_MIN: f32 = 1.2;
const UNDER_MAX: f32 = 3.5;
/// The surface boils this long before it bursts out (the tell).
const RISE: f32 = 0.55;
const BURST_TIME: f32 = 0.7;
const BURST_HEIGHT: f32 = 160.0;
const SLAM_REACH: f32 = 30.0 * SCALE;
const SLAM_DAMAGE: i32 = 1;
/// In the water with the chameleon: its touch hurts.
const WATER_DAMAGE: i32 = 1;

const SHELL: Color = Color::srgb(0.42, 0.40, 0.44);
const DARK: Color = Color::srgb(0.22, 0.21, 0.25);
const PALE: Color = Color::srgb(0.68, 0.64, 0.62);
const TIP: Color = Color::srgb(0.10, 0.09, 0.10);
const EYE: Color = Color::srgb(0.06, 0.06, 0.08);
/// The crusher's charge glow (color layer).
const GLOW: Color = Color::srgba(0.36, 0.43, 0.88, 0.0);

pub struct CrabPlugin;

impl Plugin for CrabPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (reset_on_respawn, on_hit, think, glows, dismember, animate, charge_aura)
                .chain()
                .after(player::move_player)
                .after(chameleon::update_anim_state),
        );
    }
}

#[derive(Component)]
struct Crab {
    state: State,
    vel: Vec2,
    facing: f32,
    cooldown: f32,
    /// Seconds since it last dived (on land, fighting).
    dive_timer: f32,
    /// Seconds before it may crush again.
    crush_cd: f32,
    retaliate: bool,
    introduced: bool,
    stride: f32,
    squash: f32,
    seed: f32,
    dismember: bool,
    arena: Arena,
    rng: Rng,
}

#[derive(Clone, Copy, PartialEq)]
enum Leap {
    /// Off the shore into the water.
    Dive,
    /// Out of the water onto the chameleon: slams down on landing.
    Burst,
}

#[derive(Clone, Copy, PartialEq)]
enum State {
    Stand,
    Taunt(f32),
    Pinch { t: f32, hit: bool },
    Rear(f32),
    Scuttle { t: f32, dir: f32, hit: bool },
    /// The crusher raised and charging.
    Wind(f32),
    Crush(f32),
    Stuck(f32),
    Air { from: Vec2, to: Vec2, height: f32, time: f32, t: f32, kind: Leap },
    /// Under the surface (`t`: how long).
    Under(f32),
    /// About to burst out.
    Rise(f32),
    Hurt(f32),
    Dead { t: f32, landed: bool },
}

impl Crab {
    fn airborne(&self) -> bool {
        matches!(self.state, State::Air { .. } | State::Dead { landed: false, .. })
    }

    fn in_water(&self) -> bool {
        matches!(self.state, State::Under(_) | State::Rise(_))
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
    /// The claws: `near` is the great crusher, the far one the small pincher.
    Arm { near: bool },
    Claw { near: bool },
    /// The movable finger of a claw.
    Finger { near: bool },
    EyeStalk { near: bool },
    /// Walking legs, 0 (front) to 3 (back).
    Leg { pair: usize, near: bool },
}

/// The crusher's glow on the floor where it will land (`BossStuff`, color layer).
#[derive(Component)]
struct FloorGlow {
    owner: Entity,
    material: Handle<ColorMaterial>,
}

/// The glow around the crusher itself while it charges (color layer).
#[derive(Component)]
struct ClawGlow {
    owner: Entity,
    material: Handle<ColorMaterial>,
}

/// The water next to its shore: (pool index, its box, its surface height).
fn pool_near(waters: &Waters, arena: &Arena) -> Option<(usize, Rect, f32)> {
    let gap = |r: Rect| (r.min.x - arena.right).max(arena.left - r.max.x).max(0.0);
    waters
        .pools
        .iter()
        .enumerate()
        .filter(|(_, p)| p.level <= arena.floor + 40.0 && arena.floor - p.level < DIVE_DROP && gap(p.bounds) < DIVE_GAP && p.bounds.width() > HALF_W * 3.0)
        .min_by(|a, b| gap(a.1.bounds).total_cmp(&gap(b.1.bounds)))
        .map(|(i, p)| (i, p.bounds, p.level))
}

/// Where it wakes: its shore, and the water below and beside it.
pub fn wake_area(_terrain: &Terrain, arena: &Arena) -> Rect {
    Rect::new(arena.left - 200.0, arena.floor - DIVE_DROP, arena.right + DIVE_GAP, arena.floor + 500.0)
}

/// Spawns the crab on its shore, facing left.
pub fn spawn(commands: &mut Commands, meshes: &mut Assets<Mesh>, materials: &mut Assets<ColorMaterial>, arena: &Arena, _terrain: &Terrain) -> Entity {
    let shell = materials.add(SHELL);
    let dark = materials.add(DARK);
    let pale = materials.add(PALE);
    let tip = materials.add(TIP);
    let eye = materials.add(EYE);
    let glint = materials.add(Color::WHITE);
    let mats = vec![(shell.clone(), SHELL), (dark.clone(), DARK), (pale.clone(), PALE)];

    let home = Vec2::new(arena.home.x, arena.floor + FOOT);
    let crab = commands.spawn((Transform::from_translation(home.extend(12.0)), Visibility::default())).id();
    let glow_material = materials.add(ColorMaterial { color: GLOW, alpha_mode: AlphaMode2d::Blend, ..default() });
    let mut b = Builder { commands, meshes, owner: crab, glow_material };
    let rig = b.bone(Vec3::ZERO, crab, BoneKind::Rig);
    b.commands.entity(rig).insert(Transform::from_scale(Vec3::new(-SCALE, SCALE, 1.0)));

    // Legs: four a side, jointed, pointed tips on the floor (y = -20).
    let hips = [Vec2::new(10.0, -6.0), Vec2::new(3.0, -8.0), Vec2::new(-4.0, -8.0), Vec2::new(-11.0, -6.0)];
    let feet = [Vec2::new(22.0, -20.0), Vec2::new(10.0, -20.0), Vec2::new(-10.0, -20.0), Vec2::new(-22.0, -20.0)];
    for near in [false, true] {
        let (mat, z, dx) = if near { (&shell, 0.2, 0.0) } else { (&dark, -0.4, 3.0) };
        for pair in 0..4 {
            let bone = b.bone((hips[pair] + Vec2::X * dx).extend(z + pair as f32 * 0.01), rig, BoneKind::Leg { pair, near });
            let foot = feet[pair] - hips[pair];
            let knee = Vec2::new(foot.x * 0.55, 6.0);
            let path = smooth_open(&[Vec2::ZERO, knee, foot], 4);
            b.part(stroke(&path, |t| 3.2 - 2.4 * t, 1.0, 0.75, |_| 1.0), mat, Vec3::ZERO, bone);
            b.part(stroke(&[knee * 0.97 + (foot - knee) * 0.75, foot], |t| 1.2 - 1.0 * t, 1.0, 1.0, |_| 1.0), &tip, Vec3::new(0.0, 0.0, 0.01), bone);
        }
    }

    // The far claw (small pincher), behind the body.
    claw(&mut b, rig, false, &dark, &tip);

    // Carapace: wide, domed, its front edge toothed; knobs and pale underside.
    let shell_pts = smooth_closed(
        &[
            Vec2::new(22.0, 2.0),
            Vec2::new(19.0, 9.0),
            Vec2::new(10.0, 13.0),
            Vec2::new(-4.0, 14.0),
            Vec2::new(-16.0, 11.0),
            Vec2::new(-22.0, 4.0),
            Vec2::new(-21.0, -3.0),
            Vec2::new(-14.0, -8.0),
            Vec2::new(0.0, -9.5),
            Vec2::new(14.0, -8.0),
            Vec2::new(21.0, -3.0),
        ],
        6,
    );
    b.part(fill(&shell_pts, |p| 0.5 + 0.5 * ((p.y + 9.5) / 23.5).clamp(0.0, 1.0)), &shell, Vec3::ZERO, rig);
    let belly = smooth_closed(&[Vec2::new(16.0, -4.0), Vec2::new(0.0, -6.0), Vec2::new(-16.0, -4.0), Vec2::new(-12.0, -8.5), Vec2::new(0.0, -9.6), Vec2::new(12.0, -8.5)], 4);
    b.part(fill(&belly, |_| 0.9), &pale, Vec3::new(0.0, 0.0, 0.01), rig);
    let teeth: Vec<[Vec2; 3]> = (0..5)
        .map(|i| {
            let a = 0.15 + i as f32 * 0.24;
            let c = Vec2::new(20.5 * a.cos(), 2.0 + 11.0 * a.sin());
            let n = Vec2::from_angle(a);
            [c - n.perp() * 1.4, c + n.perp() * 1.4, c + n * 3.0]
        })
        .collect();
    b.part(triangles(&teeth), &shell, Vec3::new(0.0, 0.0, -0.01), rig);
    let knobs: Vec<Vec<Vec2>> = [Vec2::new(4.0, 8.0), Vec2::new(-6.0, 9.0), Vec2::new(-14.0, 5.0), Vec2::new(10.0, 4.0), Vec2::new(-3.0, 3.0)]
        .iter()
        .map(|&c| ellipse_pts(1.6, 1.2, 8).into_iter().map(|p| p + c).collect())
        .collect();
    b.part(blobs(&knobs, |_| 0.55), &dark, Vec3::new(0.0, 0.0, 0.02), rig);
    // The mouth parts under the front edge.
    b.part(triangles(&[[Vec2::new(17.0, -3.0), Vec2::new(21.0, -4.0), Vec2::new(18.0, -7.0)], [Vec2::new(15.0, -5.0), Vec2::new(19.0, -6.0), Vec2::new(16.0, -8.5)]]), &dark, Vec3::new(0.0, 0.0, 0.02), rig);

    // Eye stalks: up from the front edge, black eyes on top.
    for (near, x, z) in [(false, 13.0, -0.05), (true, 16.0, 0.05)] {
        let stalk = b.bone(Vec3::new(x, 11.0, z), rig, BoneKind::EyeStalk { near });
        b.part(stroke(&[Vec2::ZERO, Vec2::new(0.5, 4.0), Vec2::new(0.6, 8.0)], |_| 1.6, 1.0, 0.8, |_| 1.0), if near { &shell } else { &dark }, Vec3::ZERO, stalk);
        b.part(fill(&ellipse_pts(1.9, 2.2, 12), |_| 1.0), &eye, Vec3::new(0.6, 9.0, 0.01), stalk);
        b.part(fill(&ellipse_pts(0.5, 0.6, 6), |_| 1.0), &glint, Vec3::new(1.1, 9.8, 0.02), stalk);
    }

    // The great claw (the crusher), in front of everything.
    claw(&mut b, rig, true, &shell, &tip);

    let mut rng = Rng(0xC2AB_D00D);
    commands.entity(crab).insert((
        Boss::new(&SPEC, home, mats).staged(),
        Crab {
            state: State::Stand,
            vel: Vec2::ZERO,
            facing: -1.0,
            cooldown: rng.range(COOLDOWN.0, COOLDOWN.1),
            dive_timer: 0.0,
            crush_cd: 2.0,
            retaliate: false,
            introduced: false,
            stride: 0.0,
            squash: 0.0,
            seed: rng.range(0.0, 100.0),
            dismember: false,
            arena: *arena,
            rng,
        },
    ));
    crab
}

/// A claw: the arm (merus) from the body's front, the claw (palm and fixed
/// finger) at its end, the movable finger hinged on top. The near one is the crusher.
fn claw(b: &mut Builder, rig: Entity, near: bool, mat: &Handle<ColorMaterial>, tip: &Handle<ColorMaterial>) {
    let (at, z, k): (Vec2, f32, f32) = if near { (Vec2::new(14.0, -2.0), 0.6, 1.0) } else { (Vec2::new(16.0, 0.0), -0.5, 0.6) };
    let arm = b.bone(at.extend(z), rig, BoneKind::Arm { near });
    let merus = smooth_closed(&[Vec2::new(0.0, 3.0), Vec2::new(8.0, 4.0) * k.max(0.8), Vec2::new(13.0 * k, 3.2 * k), Vec2::new(14.0 * k, 0.0), Vec2::new(13.0 * k, -3.0 * k), Vec2::new(6.0, -3.5) * k.max(0.8), Vec2::new(0.0, -2.8)], 4);
    b.part(fill(&merus, |p| 0.6 + 0.4 * ((p.y + 3.5) / 7.5)), mat, Vec3::ZERO, arm);
    let claw = b.bone(Vec3::new(13.0 * k, 0.0, 0.02), arm, BoneKind::Claw { near });
    // Palm and the fixed finger along +x.
    let palm = smooth_closed(
        &[
            Vec2::new(-2.0, 0.0),
            Vec2::new(0.0, 5.5),
            Vec2::new(8.0, 7.0),
            Vec2::new(13.0, 4.0),
            Vec2::new(14.0, 1.0),
            Vec2::new(22.0, -0.5),
            Vec2::new(25.0, -2.5),
            Vec2::new(19.0, -4.5),
            Vec2::new(10.0, -6.0),
            Vec2::new(2.0, -5.0),
        ]
        .map(|p| p * k),
        5,
    );
    b.part(fill(&palm, |p| 0.55 + 0.45 * ((p.y + 6.0 * k) / (13.0 * k))), mat, Vec3::ZERO, claw);
    b.part(triangles(&[[Vec2::new(20.0, -1.0) * k, Vec2::new(25.0, -2.5) * k, Vec2::new(19.5, -4.0) * k]]), tip, Vec3::new(0.0, 0.0, 0.01), claw);
    let nubs: Vec<[Vec2; 3]> = (0..4)
        .map(|i| {
            let x = (14.5 + i as f32 * 2.2) * k;
            [Vec2::new(x - 0.6 * k, -0.4 * k), Vec2::new(x + 0.6 * k, -0.6 * k), Vec2::new(x, 1.2 * k)]
        })
        .collect();
    b.part(triangles(&nubs), tip, Vec3::new(0.0, 0.0, 0.01), claw);
    // The movable finger: hinged at the top of the palm, curving down to meet the fixed one.
    let finger = b.bone(Vec3::new(12.0 * k, 3.5 * k, 0.03), claw, BoneKind::Finger { near });
    let dactyl = smooth_closed(&[Vec2::new(0.0, 2.0), Vec2::new(7.0, 2.6), Vec2::new(12.0, 0.8), Vec2::new(14.0, -2.5), Vec2::new(11.0, -1.4), Vec2::new(6.0, -0.4), Vec2::new(0.0, -1.5)].map(|p| p * k), 4);
    b.part(fill(&dactyl, |p| 0.65 + 0.35 * ((p.y + 2.5 * k) / (5.1 * k))), mat, Vec3::ZERO, finger);
    b.part(triangles(&[[Vec2::new(11.0, 0.0) * k, Vec2::new(14.0, -2.5) * k, Vec2::new(10.5, -1.4) * k]]), tip, Vec3::new(0.0, 0.0, 0.01), finger);
    if near {
        // The charge glow around the crusher (shown while it winds up).
        let ring: Vec<Vec2> = ellipse_pts(20.0, 12.0, 24).into_iter().map(|p| p + Vec2::new(10.0, 0.0)).collect();
        let material = b.glow_material.clone();
        let glow = b.part(fill(&ring, |p| 1.0 - 0.6 * ((p - Vec2::new(10.0, 0.0)) / Vec2::new(20.0, 12.0)).length()), &material, Vec3::new(0.0, 0.0, -0.05), claw);
        b.commands.entity(glow).insert((camera::color_layer(), ClawGlow { owner: b.owner, material }));
    }
}

struct Builder<'a, 'w, 's> {
    commands: &'a mut Commands<'w, 's>,
    meshes: &'a mut Assets<Mesh>,
    owner: Entity,
    /// The crusher's charge glow (its own, faded in by `glows`).
    glow_material: Handle<ColorMaterial>,
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

/// React to a hit (health and blood are handled in `boss.rs`). Big and heavy:
/// it only flinches when it isn't doing anything.
fn on_hit(mut hits: MessageReader<BossHit>, mut crabs: Query<&mut Crab>) {
    for hit in hits.read() {
        let Ok(mut c) = crabs.get_mut(hit.boss) else { continue };
        if hit.killed {
            c.vel = Vec2::new(hit.dir.x * 220.0, 480.0);
            c.state = State::Dead { t: 0.0, landed: false };
            c.dismember = true;
        } else if hit.retaliate {
            c.retaliate = true;
        } else if hit.stagger && matches!(c.state, State::Stand | State::Taunt(_)) {
            c.vel = Vec2::new(hit.dir.x * 120.0, 0.0);
            c.state = State::Hurt(0.0);
        }
    }
}

/// `x` kept inside the arena, the whole body over its floor (the middle if it's too narrow).
fn inside(arena: &Arena, x: f32) -> f32 {
    let (lo, hi) = (arena.left + HALF_W, arena.right - HALF_W);
    if lo < hi { x.clamp(lo, hi) } else { (arena.left + arena.right) * 0.5 }
}

/// `x` kept inside the pool, the whole body in it.
fn in_pool(pool: Rect, x: f32) -> f32 {
    let (lo, hi) = (pool.min.x + HALF_W, pool.max.x - HALF_W);
    if lo < hi { x.clamp(lo, hi) } else { pool.center().x }
}

/// How high it can leap over `at` (feet) without hitting a ceiling, up to `want`.
fn headroom(terrain: &Terrain, at: Vec2, want: f32) -> f32 {
    let body = FOOT * 2.2 + 10.0;
    let mut h = 0.0;
    while h < want {
        if terrain.solid_at(at + Vec2::Y * (h + body)) {
            return (h - 10.0).max(20.0);
        }
        h += 8.0;
    }
    want
}

/// A leap from its center `from` to the center `to` (the arc's top `height` above the higher end).
fn leap(terrain: &Terrain, from: Vec2, to: Vec2, height: f32, time: f32, kind: Leap) -> State {
    let height = headroom(terrain, (from + to) * 0.5 - Vec2::Y * FOOT, height + (to.y - from.y).max(0.0));
    State::Air { from, to, height, time, t: 0.0, kind }
}

#[allow(clippy::too_many_arguments)]
fn think(
    time: Res<Time>,
    mut commands: Commands,
    mut shake: ResMut<Shake>,
    mut stop: ResMut<HitStop>,
    mut bleed: MessageWriter<Bleed>,
    mut splash: MessageWriter<Splash>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    player: Single<(&mut Player, &Transform), Without<Crab>>,
    camera: Single<(&Camera, &GlobalTransform), With<MainCamera>>,
    mut crabs: Query<(Entity, &mut Crab, &mut Boss, &mut Transform, Option<&mut boss::Sense>)>,
    floor_glows: Query<&FloorGlow>,
    terrain: Res<Terrain>,
    waters: Res<Waters>,
) {
    let dt = time.delta_secs().min(1.0 / 30.0) * crate::boss::tempo();
    let (mut p, ptf) = player.into_inner();
    let ppos = ptf.translation.truncate();
    let swimming = waters.sample(&terrain, ppos, player::HALF).is_some();

    for (e, mut c, mut boss, mut tf, sense) in &mut crabs {
        let pos = tf.translation.truncate();
        let to_player = ppos - pos;
        let alive = p.dead.is_none();
        let aggro = alive && boss.awake;
        let arena = c.arena;
        let pool = pool_near(&waters, &arena);
        let ground_at = |x: f32, y: f32| terrain.surface(Vec2::new(x, y - FOOT + 2.0 * STEP_UP), 3000.0);
        let floor = ground_at(pos.x, pos.y).unwrap_or(arena.floor);
        if let Some(mut s) = sense {
            s.goal = aggro.then_some(ppos);
            s.target = match c.state {
                State::Air { to, .. } => Some(to),
                _ => aggro.then_some(ppos),
            };
            s.sees_player = terrain.raycast(pos, ppos).is_none();
            s.path.clear();
        }
        let side = if to_player.x >= 0.0 { 1.0 } else { -1.0 };
        let on_screen = match (camera.0.world_to_viewport(camera.1, pos.extend(0.0)), camera.0.logical_viewport_size()) {
            (Ok(v), Some(size)) => v.x > SCREEN_MARGIN && v.y > SCREEN_MARGIN && v.x < size.x - SCREEN_MARGIN && v.y < size.y - SCREEN_MARGIN,
            _ => false,
        };
        if !c.introduced {
            boss.staged = true;
        }
        let enraged = boss.enraged();
        let pace = if enraged { RAGE_PACE } else { 1.0 };
        let dx = to_player.x.abs();
        c.crush_cd = (c.crush_cd - dt).max(0.0);
        // Where it goes under, and where it surfaces to burst out at the chameleon.
        let dive = |pool: (usize, Rect, f32)| {
            let (_, rect, level) = pool;
            let x = in_pool(rect, pos.x + (rect.center().x - pos.x).signum() * HALF_W);
            leap(&terrain, pos, Vec2::new(x, level - SUBMERGE), 120.0, 0.75, Leap::Dive)
        };

        // Hit too many times in a row: into the water if it can, else a pinch.
        if std::mem::take(&mut c.retaliate) && matches!(c.state, State::Stand | State::Hurt(_) | State::Taunt(_) | State::Stuck(_)) {
            c.facing = side;
            c.state = match pool {
                Some(pl) => dive(pl),
                None => State::Pinch { t: PINCH_TELL * 0.5, hit: false },
            };
        }

        c.state = match c.state {
            State::Dead { t: t0, landed } => {
                let td = t0 + dt;
                for at in [0.3, 0.7] {
                    if t0 < at && td >= at {
                        bleed.write(Bleed { pos, dir: Vec2::Y, spread: PI, speed: 560.0, amount: 70, fluid: Fluid::Indigo, sticky: true });
                    }
                }
                let mut landed = landed;
                let mut next = pos;
                if !landed {
                    c.vel.y -= 1400.0 * dt;
                    next += c.vel * dt;
                    let rest = floor + 10.0 * SCALE;
                    if ground_at(next.x, pos.y).is_some() && next.y <= rest && c.vel.y < 0.0 {
                        next.y = rest;
                        c.vel = Vec2::new(c.vel.x * 0.3, 0.0);
                        landed = true;
                        shake.add(0.4);
                        bleed.write(Bleed { pos, dir: Vec2::Y, spread: 1.3, speed: 260.0, amount: 50, fluid: Fluid::Indigo, sticky: true });
                    }
                } else {
                    c.vel.x *= 1.0 - 5.0 * dt;
                    next.x += c.vel.x * dt;
                }
                if td < 3.5 && c.rng.chance(dt * 12.0) {
                    bleed.write(Bleed { pos, dir: Vec2::Y, spread: 1.0, speed: 140.0, amount: 3, fluid: Fluid::Indigo, sticky: true });
                }
                tf.translation.x = next.x;
                tf.translation.y = next.y;
                State::Dead { t: td, landed }
            }
            State::Hurt(s) => {
                c.vel.x *= 1.0 - 6.0 * dt;
                if s < 0.2 { State::Hurt(s + dt) } else { State::Stand }
            }
            State::Stand => {
                c.cooldown -= dt;
                if !aggro {
                    c.introduced = false;
                    let home = boss.home.x - pos.x;
                    c.vel.x = if home.abs() > 30.0 { home.signum() * WALK * 0.6 } else { 0.0 };
                    if home.abs() > 30.0 {
                        c.facing = home.signum();
                    }
                    State::Stand
                } else if !c.introduced {
                    c.vel.x = 0.0;
                    c.facing = side;
                    if on_screen {
                        c.introduced = true;
                        boss.staged = false;
                        shake.add(0.2);
                        State::Taunt(0.0)
                    } else {
                        State::Stand
                    }
                } else {
                    c.facing = side;
                    c.dive_timer += dt;
                    c.vel.x = if dx > PINCH_RANGE * 0.8 { side * WALK } else { 0.0 };
                    if c.cooldown > 0.0 {
                        State::Stand
                    } else {
                        c.cooldown = c.rng.range(COOLDOWN.0, COOLDOWN.1) * pace;
                        c.vel.x = 0.0;
                        match pool {
                            // The chameleon in its water, or time for a swim.
                            Some(pl) if swimming || c.dive_timer > DIVE_EVERY * pace => {
                                c.dive_timer = 0.0;
                                dive(pl)
                            }
                            _ if dx < CRUSH_RANGE && c.crush_cd <= 0.0 && c.rng.chance(if enraged { CRUSH_SHARE * 1.3 } else { CRUSH_SHARE }) => {
                                c.crush_cd = CRUSH_GAP;
                                State::Wind(0.0)
                            }
                            _ if dx < PINCH_RANGE => State::Pinch { t: 0.0, hit: false },
                            _ if c.rng.chance(SCUTTLE_SHARE) => State::Rear(0.0),
                            _ => {
                                c.cooldown = 0.3;
                                State::Stand
                            }
                        }
                    }
                }
            }
            State::Taunt(s) => {
                c.facing = side;
                c.vel.x = 0.0;
                if to_player.length() < TAUNT_BREAK {
                    State::Pinch { t: 0.0, hit: false }
                } else if s >= TAUNT_TIME {
                    c.cooldown = c.rng.range(0.2, 0.5);
                    State::Stand
                } else {
                    State::Taunt(s + dt)
                }
            }
            State::Pinch { t: s, hit } => {
                let s1 = s + dt;
                let mut hit = hit;
                if !hit && s1 >= PINCH_TELL {
                    hit = true;
                    shake.add(0.08);
                    let tip = pos + Vec2::new(c.facing * 38.0 * SCALE, -2.0 * SCALE);
                    if alive && ppos.distance(tip) < 18.0 * SCALE + player::HALF.y {
                        boss::hurt_player(&mut p, ppos, pos, PINCH_DAMAGE, &mut bleed, (&mut shake, &mut stop));
                    }
                }
                if s1 >= PINCH_TELL + PINCH_TIME { State::Stand } else { State::Pinch { t: s1, hit } }
            }
            State::Rear(s) => {
                c.facing = side;
                c.vel.x = 0.0;
                if s >= REAR * pace.max(0.8) {
                    shake.add(0.1);
                    State::Scuttle { t: 0.0, dir: side, hit: false }
                } else {
                    State::Rear(s + dt)
                }
            }
            State::Scuttle { t: s, dir, hit } => {
                let s1 = s + dt;
                c.vel.x = dir * SCUTTLE_SPEED;
                let mut hit = hit;
                if !hit && alive && ppos.distance(pos) < RADIUS * 0.85 + player::HALF.y {
                    hit = true;
                    boss::hurt_player(&mut p, ppos, pos, SCUTTLE_DAMAGE, &mut bleed, (&mut shake, &mut stop));
                }
                let blocked = (inside(&arena, pos.x + dir * 6.0) - (pos.x + dir * 6.0)).abs() > 0.5;
                if s1 < SCUTTLE_TIME && !blocked {
                    State::Scuttle { t: s1, dir, hit }
                } else {
                    shake.add(if blocked { 0.15 } else { 0.05 });
                    c.vel.x = 0.0;
                    State::Stand
                }
            }
            State::Wind(s) => {
                c.facing = side;
                c.vel.x = 0.0;
                if floor_glows.iter().all(|g| g.owner != e) {
                    let material = materials.add(ColorMaterial { color: GLOW, alpha_mode: AlphaMode2d::Blend, ..default() });
                    let at = Vec2::new(pos.x + c.facing * CRUSH_AT, floor + 2.0);
                    commands.spawn((
                        Mesh2d(meshes.add(fill(&ellipse_pts(CRUSH_HALF, CRUSH_HALF * 0.18, 28), |p| 1.0 - 0.7 * (p.x.abs() / CRUSH_HALF)))),
                        MeshMaterial2d(material.clone()),
                        Transform::from_translation(at.extend(9.0)),
                        camera::color_layer(),
                        FloorGlow { owner: e, material },
                        BossStuff,
                    ));
                }
                if s >= if enraged { RAGE_WIND } else { WIND } {
                    shake.add(0.2);
                    State::Crush(0.0)
                } else {
                    State::Wind(s + dt)
                }
            }
            State::Crush(s) => {
                let s1 = s + dt;
                if s1 >= CRUSH_TIME {
                    // The slam: whatever is under the claw dies.
                    shake.add(0.9);
                    stop.add(0.12);
                    let at = Vec2::new(pos.x + c.facing * CRUSH_AT, floor);
                    let under = (ppos.x - at.x).abs() < CRUSH_HALF + player::HALF.x && ppos.y - player::HALF.y - at.y < CRUSH_HEIGHT && ppos.y > at.y - 60.0;
                    if alive && under && p.guard <= 0.0 {
                        p.invuln = 0.0;
                        let all = p.hp.max(1) + 99;
                        boss::hurt_player(&mut p, ppos, at, all, &mut bleed, (&mut shake, &mut stop));
                    }
                    bleed.write(Bleed { pos: at + Vec2::Y * 6.0, dir: Vec2::Y, spread: 1.4, speed: 300.0, amount: 0, fluid: Fluid::Indigo, sticky: false });
                    State::Stuck(0.0)
                } else {
                    State::Crush(s1)
                }
            }
            State::Stuck(s) => {
                if s >= STUCK * if enraged { 0.75 } else { 1.0 } {
                    c.cooldown = c.cooldown.min(0.4);
                    State::Stand
                } else {
                    State::Stuck(s + dt)
                }
            }
            State::Air { from, to, height, time, t: s, kind } => {
                let s = s + dt;
                let k = (s / time).min(1.0);
                let top = from.y.max(to.y) + height;
                // Up to `top` and down: a parabola through from, top and to.
                let y = if k < 0.5 { from.y + (top - from.y) * (1.0 - (1.0 - 2.0 * k).powi(2)) } else { top + (to.y - top) * (2.0 * k - 1.0).powi(2) };
                let next = Vec2::new(from.x + (to.x - from.x) * k, y);
                c.vel = (next - pos) / dt.max(1e-4);
                if (to.x - from.x).abs() > 1.0 {
                    c.facing = (to.x - from.x).signum();
                }
                tf.translation.x = next.x;
                tf.translation.y = next.y;
                if k < 1.0 {
                    State::Air { from, to, height, time, t: s, kind }
                } else {
                    c.vel = Vec2::ZERO;
                    match kind {
                        Leap::Dive => {
                            if let Some((i, _, level)) = pool {
                                splash.write(Splash { at: Vec2::new(to.x, level), pool: i, speed: 900.0 });
                            }
                            shake.add(0.2);
                            State::Under(0.0)
                        }
                        Leap::Burst => {
                            c.squash = 1.0;
                            shake.add(0.45);
                            let feet = to - Vec2::Y * FOOT;
                            if alive && ppos.distance(feet + Vec2::Y * 20.0) < SLAM_REACH + player::HALF.x {
                                boss::hurt_player(&mut p, ppos, feet, SLAM_DAMAGE, &mut bleed, (&mut shake, &mut stop));
                            }
                            c.cooldown = 0.6;
                            State::Stand
                        }
                    }
                }
            }
            State::Under(s) => {
                let s1 = s + dt;
                match pool {
                    None => State::Stand,
                    Some((i, rect, level)) => {
                        // Glides under the surface toward the chameleon (in the water: right at it).
                        let want = if swimming { ppos } else { Vec2::new(ppos.x, level - SUBMERGE) };
                        let goal = Vec2::new(in_pool(rect, want.x), want.y.clamp(rect.min.y + FOOT, level - SUBMERGE));
                        let step = (goal - pos).clamp_length_max(SWIM * dt);
                        tf.translation.x += step.x;
                        tf.translation.y += step.y;
                        c.vel = step / dt.max(1e-4);
                        if step.x.abs() > 0.1 {
                            c.facing = step.x.signum();
                        }
                        if swimming && alive && ppos.distance(pos) < RADIUS * 0.8 + player::HALF.y {
                            boss::hurt_player(&mut p, ppos, pos, WATER_DAMAGE, &mut bleed, (&mut shake, &mut stop));
                        }
                        let ready = !swimming && s1 > UNDER_MIN && (goal - pos).length() < 40.0;
                        if alive && (ready || (s1 > UNDER_MAX && !swimming)) {
                            splash.write(Splash { at: Vec2::new(pos.x, level), pool: i, speed: 300.0 });
                            State::Rise(0.0)
                        } else if !alive || !aggro {
                            // Nothing to hunt: back to its shore.
                            leap(&terrain, pos, Vec2::new(inside(&arena, boss.home.x), arena.floor + FOOT), BURST_HEIGHT, BURST_TIME, Leap::Burst)
                        } else {
                            State::Under(s1)
                        }
                    }
                }
            }
            State::Rise(s) => {
                // The surface boils over it: the tell.
                if let Some((i, _, level)) = pool {
                    if c.rng.chance(dt * 14.0) {
                        splash.write(Splash { at: Vec2::new(pos.x + c.rng.range(-1.0, 1.0) * HALF_W, level), pool: i, speed: 250.0 });
                    }
                }
                if s >= RISE * pace.max(0.8) {
                    // Out onto the shore where the chameleon is.
                    let x = inside(&arena, ppos.x);
                    let to = Vec2::new(x, ground_at(x, arena.floor + FOOT + 40.0).unwrap_or(arena.floor) + FOOT);
                    if let Some((i, _, level)) = pool {
                        splash.write(Splash { at: Vec2::new(pos.x, level), pool: i, speed: 1100.0 });
                    }
                    shake.add(0.2);
                    leap(&terrain, pos, to, BURST_HEIGHT, BURST_TIME, Leap::Burst)
                } else {
                    State::Rise(s + dt)
                }
            }
        };
        // Under the water (and while bursting out) blows glance off it.
        boss.guard = c.in_water();

        // On the ground: follow the floor under it.
        if !c.airborne() && !c.in_water() && !matches!(c.state, State::Dead { .. }) {
            let x = inside(&arena, tf.translation.x + c.vel.x * dt);
            let floor = ground_at(x, tf.translation.y).unwrap_or(floor);
            tf.translation.x = x;
            tf.translation.y = floor + FOOT;
            c.stride += (c.vel.x.abs() * dt * 0.05).min(1.0);
        }
        c.squash = (c.squash - dt * 3.0).max(0.0);
    }
}

/// The crusher's glows: around the claw and on the floor, swelling while it charges, gone after the slam.
fn glows(
    time: Res<Time>,
    mut commands: Commands,
    mut materials: ResMut<Assets<ColorMaterial>>,
    crabs: Query<&Crab>,
    claws: Query<&ClawGlow>,
    mut floors: Query<(Entity, &FloorGlow, &mut Transform)>,
) {
    let t = time.elapsed_secs();
    let charge = |owner: Entity| match crabs.get(owner).map(|c| c.state) {
        Ok(State::Wind(s)) => Some((s / WIND).min(1.0)),
        Ok(State::Crush(_)) => Some(1.0),
        _ => None,
    };
    for g in &claws {
        let k = charge(g.owner).unwrap_or(0.0);
        if let Some(mut m) = materials.get_mut(&g.material) {
            let throb = 0.75 + 0.25 * (t * (8.0 + 14.0 * k)).sin();
            m.color = GLOW.with_alpha(0.65 * k * throb);
        }
    }
    for (e, g, mut tf) in &mut floors {
        let Some(k) = charge(g.owner) else {
            commands.entity(e).despawn();
            continue;
        };
        tf.scale = Vec3::new(0.4 + 0.6 * k, 1.0, 1.0);
        if let Some(mut m) = materials.get_mut(&g.material) {
            m.color = GLOW.with_alpha((0.15 + 0.55 * k) * (0.8 + 0.2 * (t * 16.0).sin()));
        }
    }
}

/// The chameleon came back: start the fight over (`boss.rs` heals it).
fn reset_on_respawn(mut commands: Commands, mut respawned: MessageReader<Respawned>, mut crabs: Query<(&mut Crab, &mut Boss, &mut Transform)>, glows: Query<Entity, With<FloorGlow>>) {
    if respawned.read().count() == 0 {
        return;
    }
    for e in &glows {
        commands.entity(e).despawn();
    }
    for (mut c, mut boss, mut tf) in &mut crabs {
        if matches!(c.state, State::Dead { .. }) {
            continue;
        }
        // Back on its shore (it may have been in the water).
        if c.airborne() || c.in_water() {
            tf.translation.x = boss.home.x;
            tf.translation.y = boss.home.y;
        }
        c.state = State::Stand;
        c.introduced = false;
        c.vel = Vec2::ZERO;
        c.cooldown = 1.5;
        c.dive_timer = 0.0;
        c.crush_cd = 2.0;
        boss.guard = false;
    }
}

/// A freshly killed crab: the great claw and an eye stalk tear off.
fn dismember(mut commands: Commands, mut crabs: Query<&mut Crab>, bones: Query<(Entity, &Bone, &GlobalTransform)>) {
    let mut done = Vec::new();
    for (e, bone, gt) in &bones {
        let Ok(c) = crabs.get(bone.owner) else { continue };
        if !c.dismember {
            continue;
        }
        done.push(bone.owner);
        let f = c.facing;
        let (vel, spin, bleeds) = match bone.kind {
            BoneKind::Arm { near: true } => (Vec2::new(f * 260.0, 640.0), 6.0, Some(Fluid::Indigo)),
            BoneKind::EyeStalk { near: true } => (Vec2::new(-f * 200.0, 560.0), 12.0, Some(Fluid::Indigo)),
            _ => continue,
        };
        commands.entity(e).remove::<Bone>();
        boss::tear_off(&mut commands, e, gt, vel, spin, bleeds);
    }
    done.sort();
    done.dedup();
    for owner in done {
        if let Ok(mut c) = crabs.get_mut(owner) {
            c.dismember = false;
        }
    }
}

fn ease(k: f32) -> f32 {
    let k = k.clamp(0.0, 1.0);
    k * k * (3.0 - 2.0 * k)
}

fn animate(time: Res<Time>, crabs: Query<&Crab>, mut bones: Query<(&Bone, &mut Transform)>) {
    let t = time.elapsed_secs();
    let dt = time.delta_secs();
    for (bone, mut tf) in &mut bones {
        let Ok(c) = crabs.get(bone.owner) else { continue };
        let tt = t + c.seed;
        let (dead_t, landed) = match c.state {
            State::Dead { t, landed } => (Some(t), landed),
            _ => (None, false),
        };
        let walking = (c.vel.x.abs() / WALK).min(1.5) * if c.airborne() || c.in_water() { 0.0 } else { 1.0 };
        // The crusher: raised and charging (0..1), slamming down, stuck in the ground.
        let wind = match c.state {
            State::Wind(s) => ease(s / 0.5),
            State::Crush(s) => 1.0 - ease(s / CRUSH_TIME),
            _ => 0.0,
        };
        let charge = match c.state {
            State::Wind(s) => (s / WIND).min(1.0),
            _ => 0.0,
        };
        let stuck = matches!(c.state, State::Stuck(_));
        let taunt = matches!(c.state, State::Taunt(s) if s > 0.2);
        match bone.kind {
            BoneKind::Rig => {
                tf.scale.x += (c.facing * SCALE - tf.scale.x) * (1.0 - (-14.0 * dt).exp());
                tf.scale.y = SCALE * (1.0 - 0.1 * c.squash);
                let low = match c.state {
                    State::Rear(s) => 2.0 * ease(s / REAR),
                    State::Stuck(_) => 3.0,
                    _ => 0.0,
                } + 3.0 * c.squash;
                let bob = if walking > 0.0 { (c.stride * 4.0).sin().abs() * 0.8 * walking.min(1.0) } else { 0.0 };
                tf.translation.y = (bob - low) * SCALE;
                let angle = match dead_t {
                    Some(td) => {
                        if landed { PI } else { PI * (td * 2.0).min(1.0) }
                    }
                    None => (match c.state {
                        // Leans back under the raised claw, pitched forward when it is stuck.
                        State::Wind(_) => -0.08 * wind + (tt * 40.0).sin() * 0.012 * charge,
                        State::Stuck(_) => 0.1,
                        State::Air { .. } => (c.vel.y * 0.0004).clamp(-0.3, 0.3),
                        State::Scuttle { .. } => (tt * 30.0).sin() * 0.02,
                        _ => (tt * 1.4).sin() * 0.01,
                    }) * c.facing,
                };
                tf.rotation = Quat::from_rotation_z(angle);
            }
            BoneKind::Arm { near } => {
                let angle = if let Some(td) = dead_t {
                    -0.6 + (tt * 15.0).sin() * 0.3 * (1.0 - td / 6.0).max(0.1)
                } else if near {
                    match c.state {
                        State::Wind(_) | State::Crush(_) => -0.35 + 2.0 * wind + (tt * 45.0).sin() * 0.05 * charge,
                        State::Stuck(_) => -0.75,
                        State::Air { .. } | State::Under(_) | State::Rise(_) => 0.5,
                        _ if taunt => 1.1 + (tt * 6.0).sin() * 0.1,
                        _ => -0.35 + (tt * 1.8).sin() * 0.04,
                    }
                } else {
                    match c.state {
                        State::Pinch { t, .. } if t < PINCH_TELL => -0.1 + 0.5 * ease(t / PINCH_TELL),
                        State::Pinch { .. } => -0.2,
                        State::Air { .. } | State::Under(_) | State::Rise(_) => 0.6,
                        _ if taunt => 1.3 + (tt * 6.0 + 1.5).sin() * 0.1,
                        _ => -0.2 + (tt * 2.1).sin() * 0.05,
                    }
                };
                tf.rotation = Quat::from_rotation_z(angle);
            }
            BoneKind::Claw { near } => {
                let angle = if near {
                    if stuck { -0.6 } else { 0.25 * wind - 0.15 }
                } else {
                    -0.1
                };
                tf.rotation = Quat::from_rotation_z(angle);
            }
            BoneKind::Finger { near } => {
                // Open (+) / shut (0).
                let open = if let Some(td) = dead_t {
                    0.3 + (tt * 9.0).sin() * 0.2 * (1.0 - td / 6.0).max(0.1)
                } else if near {
                    match c.state {
                        State::Wind(_) => 0.5 * wind + (tt * 30.0).sin() * 0.1 * charge,
                        _ if taunt => 0.4 * (tt * 9.0).sin().max(0.0),
                        _ => 0.05,
                    }
                } else {
                    match c.state {
                        State::Pinch { t, .. } if t < PINCH_TELL => 0.7 * ease(t / PINCH_TELL),
                        _ if taunt => 0.5 * (tt * 9.0 + 1.5).sin().max(0.0),
                        _ => 0.05,
                    }
                };
                tf.rotation = Quat::from_rotation_z(open);
            }
            BoneKind::EyeStalk { near } => {
                let o = if near { 0.0 } else { 1.3 };
                let angle = if dead_t.is_some() {
                    -0.9
                } else {
                    match c.state {
                        // Folded down for the dive, up periscope-style under the water.
                        State::Air { kind: Leap::Dive, .. } => -1.0,
                        State::Wind(_) => -0.25,
                        _ => (tt * 1.7 + o).sin() * 0.12,
                    }
                };
                tf.rotation = Quat::from_rotation_z(angle);
            }
            BoneKind::Leg { pair, near } => {
                let group = pair as f32 * 1.6 + if near { 0.0 } else { PI };
                let front = if pair < 2 { 1.0 } else { -1.0 };
                let angle = if let Some(td) = dead_t {
                    (tt * 24.0 + group).sin() * 0.45 * (1.0 - td / 6.0).max(0.12)
                } else if c.airborne() || c.in_water() {
                    // Tucked in, paddling under the water.
                    -0.35 * front + if c.in_water() { (tt * 10.0 + group).sin() * 0.25 } else { 0.0 }
                } else {
                    let drum = match c.state {
                        State::Rear(_) => (tt * 32.0 + group).sin() * 0.12,
                        _ => 0.0,
                    };
                    walking * 0.25 * (c.stride * 4.0 + group).sin() + drum
                };
                tf.rotation = Quat::from_rotation_z(angle);
            }
        }
    }
}

/// The power blow charging (`ChameleonAnim::charge`): an indigo glow gathers around the
/// chameleon, motes swirling in; fully charged it throbs. Letting it loose shakes the world.
#[derive(Component)]
struct Aura;

#[derive(Component)]
struct Mote {
    vel: Vec2,
    age: f32,
    life: f32,
    material: Handle<ColorMaterial>,
}

const AURA_COLOR: Color = Color::srgb(0.36, 0.43, 0.88);
const AURA_FULL: Color = Color::srgb(0.75, 0.80, 1.0);

#[allow(clippy::too_many_arguments)]
fn charge_aura(
    time: Res<Time>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    cloud: Res<Cloud>,
    mut shake: ResMut<Shake>,
    mut stop: ResMut<HitStop>,
    mut was: Local<(bool, bool)>,
    mut mote_mesh: Local<Option<Handle<Mesh>>>,
    player: Single<(&Player, &ChameleonAnim, &Transform)>,
    mut glow: Query<(&mut Transform, &MeshMaterial2d<ColorMaterial>), (With<Aura>, Without<Player>, Without<Mote>)>,
    mut motes: Query<(Entity, &mut Mote, &mut Transform), (Without<Aura>, Without<Player>)>,
) {
    let dt = time.delta_secs();
    let now = time.elapsed_secs();
    let (p, a, ptf) = *player;
    let center = ptf.translation.truncate();
    let charging = a.charge();
    let k = charging.map_or(0.0, |c| (c / chameleon::BLOW_CHARGE).min(1.0));
    let full = k >= 1.0;
    let blowing = a.blowing();
    if full && !was.0 {
        shake.add(0.15);
    }
    if blowing && !was.1 {
        // Let loose.
        shake.add(0.6);
        stop.add(0.08);
    }
    *was = (full, blowing);

    if glow.is_empty() {
        let mat = materials.add(ColorMaterial { color: AURA_COLOR.with_alpha(0.0), alpha_mode: AlphaMode2d::Blend, ..default() });
        commands.spawn((Mesh2d(cloud.0.clone()), MeshMaterial2d(mat), Transform::from_xyz(center.x, center.y, 9.5), camera::color_layer(), Aura));
    }
    // Barely there at first: only a held attack that's clearly charging shows.
    let shown = ((k - 0.08) / 0.2).clamp(0.0, 1.0);
    for (mut tf, mat) in &mut glow {
        tf.translation.x = center.x;
        tf.translation.y = center.y;
        let throb = if full { 1.0 + 0.15 * (now * 20.0).sin() } else { 1.0 + 0.05 * (now * 10.0).sin() };
        tf.scale = Vec3::splat((28.0 + 46.0 * k) * throb);
        if let Some(mut m) = materials.get_mut(&mat.0) {
            let c = if full { AURA_FULL } else { AURA_COLOR };
            m.color = c.with_alpha(shown * (0.12 + 0.45 * k));
        }
    }

    // Motes swirl in toward the claws while charging.
    if shown > 0.0 && p.dead.is_none() {
        let mesh = mote_mesh.get_or_insert_with(|| meshes.add(fill(&ellipse_pts(1.0, 1.0, 8), |_| 1.0))).clone();
        let n = (dt * (10.0 + 40.0 * k)).ceil() as u32;
        let mut rng = Rng(((now * 1000.0) as u32).wrapping_mul(2654435761) | 1);
        rng.next();
        for _ in 0..n {
            if !rng.chance(0.6) {
                continue;
            }
            let dir = Vec2::from_angle(rng.range(0.0, TAU));
            let at = center + dir * rng.range(40.0, 70.0);
            let c = if full { AURA_FULL } else { AURA_COLOR };
            let material = materials.add(ColorMaterial { color: c, alpha_mode: AlphaMode2d::Blend, ..default() });
            commands.spawn((
                Mesh2d(mesh.clone()),
                MeshMaterial2d(material.clone()),
                Transform::from_translation(at.extend(11.5)).with_scale(Vec3::splat(rng.range(1.5, 2.8))),
                camera::color_layer(),
                Mote { vel: -dir * rng.range(90.0, 150.0) * (1.0 + k), age: 0.0, life: rng.range(0.3, 0.5), material },
            ));
        }
    }
    for (e, mut m, mut tf) in &mut motes {
        m.age += dt;
        let a = m.age / m.life;
        if a >= 1.0 {
            commands.entity(e).despawn();
            continue;
        }
        tf.translation += (m.vel * dt).extend(0.0);
        if let Some(mut mat) = materials.get_mut(&m.material) {
            mat.color = mat.color.with_alpha(0.9 * (1.0 - a));
        }
    }
}
