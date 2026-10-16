//! The Burrower: a giant centipede that owns the cave system. It bleeds
//! purple (`#a200a2`), and beating it gives the chameleon that color.
//!
//! Its body is a chain of plates that follows its head along the exact path
//! the head took, so every curve the head draws, the whole body repeats: a
//! long sinuous ribbon.
//!
//! It is heavy, and the rock is where it lives: it swims through it like a
//! worm through sand, and the open cave is where it shows itself. Every move
//! goes through `drive`: it thrusts only against something (rock, or a surface
//! its legs reach), its speed changes slowly and it can't turn tighter than its
//! speed allows; out in the open with nothing to grip it only coasts and falls.
//! Underground, the dust shaken out of the surface over it and the rumble give
//! it away; where the body goes through a surface, the edges of the hole crumble.
//!
//! It doesn't change its mind halfway: it plans a move and carries it through.
//! Between moves it prowls the rock in a wide loop round the chameleon
//! (`Prowl`), then picks the next one from where the chameleon is and what it
//! did last (`next_move`, no dice):
//! - **Breach**: to a surface near the chameleon, out through it on a
//!   ballistic arc through where the chameleon will be, and down into the rock
//!   again wherever the arc takes it. The surface cracks open just before.
//! - **Rise**: out of a surface on the chameleon's level, head up, slowing to a
//!   stop; the head turns to the chameleon, winds up, and strikes; whatever the
//!   strike misses, the momentum carries it on into the rock.
//! - **Back wall**: dimmer, smaller, behind the chameleon, harmless and
//!   untouchable, looping through the background, until it swings forward out
//!   of the wall into a strike.
//!
//! Below half health everything gets faster and the prowls shorter, and every few moves it
//! runs for the depths of the lair (**Flee**) and waits there (**Lurk**) until the chameleon, which
//! has to chase it down the caves, gets close: then it ambushes. Health,
//! getting hit and the death spectacle live in `boss.rs`.

use std::{
    collections::VecDeque,
    f32::consts::{FRAC_PI_2, PI, TAU},
};

use bevy::{prelude::*, sprite_render::AlphaMode2d};

use crate::{
    boss::{self, Arena, Boss, BossHit, BossSpec, BossStuff},
    camera::{ArenaView, ArenaWant, HitStop, Shake},
    chameleon::ChameleonAnim,
    feeding::Morsel,
    gore::{Bleed, Fluid},
    level::Platform,
    player::{self, Player, Respawned},
    shapes::*,
    terrain::Terrain,
};

/// Its color (what the chameleon earns, its blood, the health bar).
const PURPLE: Color = Color::srgb_u8(0xa2, 0x00, 0xa2);

pub(crate) static SPEC: BossSpec = BossSpec {
    name: "THE BURROWER",
    hp: 32,
    radius: SEG_R + 6.0,
    blood: Fluid::Purple,
    color: PURPLE,
    aggro: 900.0,
    // A whole body of meat.
    food: 20.0,
};

/// How much bigger than its original design it is: the art is drawn at the old size and scaled.
const BIG: f32 = 4.5;
/// Plates in the chain (the head is the first), the distance between them.
const SEGS: usize = 26;
const SPACING: f32 = 17.0 * BIG;
/// From the head to the last plate.
const LENGTH: f32 = (SEGS - 1) as f32 * SPACING;
/// Radius of a plate (the hurtbox is a bit bigger).
const SEG_R: f32 = 13.0 * BIG;
/// How far from the surface its middle line crawls.
const SKIN: f32 = 13.0 * BIG;
/// It is huge, so it moves at this fraction of what it could (every speed and every pull is scaled
/// by it: more inertia, the same turning radius, a better sense of scale).
const HEFT: f32 = 0.8;
/// Depth is a third axis of its motion: part of the way it travels goes into depth, and this is
/// how far it must travel to go all the way between the front and the back wall. Still, it doesn't
/// change depth (no scaling, no dimming); and each plate follows the path of the head in depth too.
const DEPTH_DIST: f32 = 450.0;
/// How dark the back wall makes it.
const DIM: f32 = 0.95;
/// How much smaller it is on the back wall.
const FAR_SHRINK: f32 = 0.4;
/// Where the back wall is in the world's depth: between the solid ground (`terrain_art::Z`) and
/// the interiors layer just behind it, so the ground hides it and the cave wall doesn't.
const WALL_Z: f32 = crate::terrain_art::Z - 0.0025;
/// Its weight: how fast it can speed up and slow down, the sideways pull it can muster to turn
/// (turning radius = speed² / `LATERAL`), and the fastest it turns at a crawl (radians per second).
const ACCEL: f32 = 450.0 * HEFT;
const BRAKE: f32 = 350.0 * HEFT;
const LATERAL: f32 = 1400.0 * HEFT * HEFT;
const MAX_TURN: f32 = 1.8 * HEFT;
/// In the open with nothing to grip: it falls, and steers only this much of what it could.
const FALL: f32 = 900.0;
const AIR_STEER: f32 = 0.15;
/// Cruising through the rock.
const CRUISE: f32 = 650.0 * HEFT;
/// Prowling: the loop round the chameleon, and how long (seconds) between moves.
const PROWL_R: f32 = 1000.0;
const PROWL_TIME: (f32, f32) = (1.8, 3.5);
/// How fast it comes out of the rock for a breach and for a rise.
const BREACH_SPEED: f32 = 1250.0 * HEFT;
const RISE_SPEED: f32 = 900.0 * HEFT;
/// Risen: how fast it comes to a stop, and the windup before the strike.
const REAR_BRAKE: f32 = 800.0 * HEFT;
const WINDUP: f32 = 1.1;
/// The strike: the body behind pushes it for this long, up to this fast.
const STRIKE_TIME: f32 = 0.45;
const STRIKE_SPEED: f32 = 1350.0 * HEFT;
/// The crack opens this long before it comes out.
const TELEGRAPH: f32 = 1.3;
const BACK_SPEED: f32 = 400.0 * HEFT;
/// Swinging forward off the back wall, before the strike.
const SWING_TIME: f32 = 1.2;
const CONTACT_DAMAGE: i32 = 1;
/// What hurts: the mouth (a point this far in front of the head, in head-plate lengths, with this
/// radius) and the two tips of the tail, both very tight. Nothing else on it does damage.
const MOUTH_REACH: f32 = 22.0 * BIG * 1.1;
const MOUTH_R: f32 = 16.0;
const TAIL_TIP: Vec2 = Vec2::new(-42.0, 1.0);
const TAIL_R: f32 = 14.0;
/// What a leg knocked off is worth to eat (ticks of health).
const LEG_FOOD: f32 = 0.25;
/// How close under the surface it must pass to shake dust out of it.
const WAKE_RANGE: f32 = 450.0;
/// Writhing in the open: how far the wave down the body swings it sideways, how fast it runs
/// (radians per second) and how much it turns from one plate to the next; the tail whip (the last
/// of the tail being dragged into the rock) swings its tip this far, this fast.
const WIGGLE: f32 = 75.0;
const WAVE_RATE: f32 = 6.5;
const THRASH: f32 = 165.0;
const THRASH_RATE: f32 = 17.0;
const WAVE_K: f32 = 0.7;
/// The sideways swat of the risen column: leaning away first (seconds), then falling sideways
/// faster and faster (radians per second², at most per second).
const SWAT_LEAN: f32 = 0.45;
const SWAT_ACCEL: f32 = 5.0;
const SWAT_MAX: f32 = 3.6;
/// How far the camera pulls back for the fight (zoom multipliers: it frames the chameleon and the
/// head, wherever it is, within these; it also keeps to the lair).
const ARENA_ZOOM: (f32, f32) = (2.8, 4.2);
/// Distance between the points of the trail the body follows, and how many it keeps.
const STEP: f32 = 24.0;
const TRAIL: usize = (LENGTH / STEP) as usize + 40;
/// Height of the walkable back above the middle line of a plate (at full size), and the
/// thickness of the platform that stands for it.
const BACK_H: f32 = 12.0 * BIG;
const BACK_THICK: f32 = 10.0;
/// Steeper than this (radians) the back is a wall, not something to walk on.
const MAX_TILT: f32 = 1.0;
/// Riding it into the rock (or scraped off its back by the rock): thrown off, stunned this long.
const THROWN_STUN: f32 = 0.6;

const BODY: Color = Color::srgb(0.17, 0.03, 0.17);
const LEG: Color = Color::srgb(0.46, 0.14, 0.40);
const EYE: Color = Color::srgb(0.92, 0.88, 0.74);
const CRACK: Color = Color::srgb(0.64, 0.0, 0.64);
const CHUNK: Color = Color::srgb(0.32, 0.30, 0.30);

pub struct CentipedePlugin;

impl Plugin for CentipedePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, load).add_systems(
            Update,
            (reset_on_respawn, on_hit, think, effects, dismember, animate, sweep_plates)
                .chain()
                .before(boss::slash_bosses)
                .after(player::move_player)
                .after(crate::chameleon::update_anim_state),
        );
    }
}

#[derive(Component)]
struct Centipede {
    state: State,
    head: Vec2,
    /// Unit direction the head faces (where it goes; stopped, it can look round).
    heading: Vec2,
    /// Velocity of the head: everything it does goes through it (`drive`).
    vel: Vec2,
    /// Advances with the distance traveled: drives the legs.
    phase: f32,
    /// Where the head has been, newest first, a point every `STEP` units.
    trail: VecDeque<Vec2>,
    /// How far from the front each trail point was when the head went through it: the body takes
    /// the depth of the head as it passed, so it travels between the wall and the front plate by plate.
    trail_depth: VecDeque<f32>,
    seg_depth: Vec<f32>,
    /// The point the back wall recedes toward (where the chameleon is).
    vanish: Vec2,
    pts: Vec<Vec2>,
    /// 0..1 how much of each plate is out.
    vis: Vec<f32>,
    /// The parts of the body farther than `hide_rear` from the head, and nearer than `hide_front`,
    /// are hidden (asleep: all of it; coming out the first time: what hasn't come out yet).
    hide_front: f32,
    hide_rear: f32,
    /// 0 = on the front of the world, 1 = on the back wall.
    depth: f32,
    seed: f32,
    rng: Rng,
    /// Where it may roam (its lair).
    domain: Rect,
    /// The head is in the rock.
    in_rock: bool,
    /// Prowling: the middle of its loop (it follows the chameleon slowly), and which way round it goes.
    center: Vec2,
    orbit: f32,
    /// Where a breach means to pass.
    target: Vec2,
    /// Where it last came out (it doesn't like the same spot twice).
    last_exit: Vec2,
    /// Moves since the last rise and since the last walk on the back wall.
    since_rise: u32,
    since_back: u32,
    /// Moves since the last retreat to the depths (enraged only).
    since_flee: u32,
    /// Where each plate is drawn: its place in perspective, re-spaced so the chain stays whole while
    /// its plates are at different depths.
    shown: Vec<Vec2>,
    strike_dir: Vec2,
    /// Strikes since the last swat (it mixes the two).
    since_swat: u32,
    /// 0..1 per plate: how free it is to writhe (not near the neck or a hole); the wave's phase;
    /// 0..1 how hard it is struggling into the rock.
    wig: Vec<f32>,
    wave: f32,
    effort: f32,
    /// The plates last frame (how fast each one moves).
    prev: Vec<Vec2>,
    /// Distance the head went since the last tremor mark on the surface over it.
    mark_d: f32,
    introduced: bool,
    react: React,
    dismember: bool,
    dead_vel: Vec<Vec2>,
    shade: Vec<Handle<ColorMaterial>>,
    /// 0..1 how open the jaws are, for the animation.
    jaws: f32,
    /// Where it was struck dead: the head goes there and stays as the carcass.
    dead_at: Vec2,
    /// The dead head: 0 flying to where it was struck, 1 falling, 2 at rest.
    dead_phase: u8,
}

#[derive(Clone, Copy, PartialEq)]
enum React {
    None,
    /// A flurry of hits: it hits back (strikes at once, if it has risen).
    Answer,
}

#[derive(Clone, Copy, PartialEq)]
enum Move {
    Breach,
    Rise,
}

#[derive(Clone, Copy, PartialEq)]
enum State {
    /// Asleep underground.
    Dormant,
    /// Cruising through the rock round the chameleon (seconds left).
    Prowl(f32),
    /// Through the rock to come out at `at` (surface normal `n`) going at `launch`, for `mv`.
    /// The crack opens once `locked`.
    Approach { t: f32, mv: Move, at: Vec2, n: Vec2, launch: Vec2, locked: bool, intro: bool },
    /// Out in the open after a breach, on its momentum (seconds so far).
    Air(f32),
    /// Risen out of the rock, slowing to a stop (seconds so far), then winding up (`w` seconds).
    Rear { t: f32, w: f32 },
    /// Striking (seconds so far): pushed for `STRIKE_TIME`, then flying on.
    Strike(f32),
    /// Risen: the column out of the hole swings sideways (`side`, ±1) onto the chameleon,
    /// pivoting on the hole (`pivot`; the first `n` points of the trail are out), at `w` rad/s.
    Swat { t: f32, side: f32, w: f32, pivot: Vec2, n: usize },
    /// Walking on the back wall (seconds left).
    Back(f32),
    /// Swinging forward off the back wall (seconds so far), into a strike.
    Swing(f32),
    /// Enraged: running for the depths of its lair, away from the chameleon, to `goal`
    /// (seconds so far). The tremors trace its way for the chameleon to follow.
    Flee { goal: Vec2, t: f32 },
    /// Waiting in the rock at the end of a flight (seconds so far), until the chameleon catches up.
    Lurk(f32),
    /// Nobody to hunt: through the rock, home.
    Home,
    Dead(f32),
}

#[derive(Component)]
struct Seg {
    owner: Entity,
    k: usize,
}

/// The back of the body between plate `k - 1` and `k`, as a platform the chameleon can walk on.
#[derive(Component)]
struct Plate {
    owner: Entity,
    k: usize,
}

#[derive(Component)]
struct Leg {
    owner: Entity,
    k: usize,
    side: f32,
}

#[derive(Component)]
struct Part {
    owner: Entity,
    kind: PartKind,
}

#[derive(Clone, Copy)]
enum PartKind {
    Fang(f32),
    Antenna(f32),
    TailLeg(f32),
}

/// The darkening of the plates on the back wall.
#[derive(Component)]
struct Shade;

/// A crack in the surface where it is about to come out.
#[derive(Component)]
struct Crack {
    age: f32,
    life: f32,
    size: f32,
    /// How bright it gets.
    strength: f32,
    mat: Handle<ColorMaterial>,
}

/// A piece of rock thrown up.
#[derive(Component)]
struct Chunk {
    vel: Vec2,
    spin: f32,
    life: f32,
    total: f32,
}

#[derive(Resource)]
pub(crate) struct Assets3 {
    crack: Handle<Mesh>,
    chunk: Handle<Mesh>,
    chunk_mat: Handle<ColorMaterial>,
}

fn load(mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>, mut materials: ResMut<Assets<ColorMaterial>>) {
    commands.insert_resource(Assets3 {
        crack: meshes.add(crack_mesh()),
        chunk: meshes.add(fill(&[Vec2::new(-4.0, -3.0), Vec2::new(3.0, -4.0), Vec2::new(5.0, 1.0), Vec2::new(0.0, 4.0), Vec2::new(-5.0, 2.0)], |p| 0.7 + 0.1 * p.y)),
        chunk_mat: materials.add(CHUNK),
    });
}

/// Jagged cracks fanning out along the surface (x along it, y out of it).
fn crack_mesh() -> Mesh {
    let mut rng = Rng(0xC0DE_5EED);
    let mut tris: Vec<[Vec2; 3]> = Vec::new();
    let line = |a: Vec2, b: Vec2, w: f32, tris: &mut Vec<[Vec2; 3]>| {
        let n = (b - a).normalize_or(Vec2::X).perp() * w * 0.5;
        tris.push([a - n, a + n, b + n]);
        tris.push([a - n, b + n, b - n]);
    };
    for dir in [-1.0, 1.0] {
        for branch in 0..3 {
            let mut at = Vec2::new(dir * 3.0, 0.0);
            let reach = 78.0 - branch as f32 * 18.0;
            let lean = (branch as f32 - 1.0) * 0.9;
            let mut w = 3.6 - branch as f32 * 0.8;
            while at.x.abs() < reach {
                let next = at + Vec2::new(dir * rng.range(7.0, 13.0), lean * 3.0 + rng.range(-5.0, 5.0));
                line(at, next, w, &mut tris);
                at = next;
                w = (w - 0.2).max(1.0);
            }
        }
    }
    triangles(&tris)
}

/// Spawns it asleep underground, somewhere in its lair.
pub fn spawn(commands: &mut Commands, meshes: &mut Assets<Mesh>, materials: &mut Assets<ColorMaterial>, arena: &Arena, _terrain: &Terrain, domain: Rect) -> Entity {
    let home = arena.home;
    let body = materials.add(BODY);
    let purple = materials.add(PURPLE);
    let leg = materials.add(LEG);
    let eye = materials.add(EYE);
    let shade: Vec<Handle<ColorMaterial>> = (0..SEGS).map(|_| materials.add(ColorMaterial { color: Color::srgba(0.0, 0.0, 0.0, 0.0), alpha_mode: AlphaMode2d::Blend, ..default() })).collect();
    let mats = vec![(body.clone(), BODY), (purple.clone(), PURPLE), (leg.clone(), LEG)];

    let plate = meshes.add(fill(&ellipse_pts(11.5, 12.5, 22), |p| 0.55 + 0.45 * ((p.y + 12.5) / 25.0)));
    let dorsal = meshes.add(fill(&ellipse_pts(6.5, 8.5, 16), |p| 0.7 + 0.3 * ((p.y + 8.5) / 17.0)));
    let rim = meshes.add(fill(&ellipse_pts(13.5, 14.0, 22), |_| 0.8));
    let head_plate = meshes.add(fill(&ellipse_pts(16.0, 13.0, 24), |p| 0.6 + 0.4 * ((p.y + 13.0) / 26.0)));
    let head_cap = meshes.add(fill(&ellipse_pts(9.0, 7.5, 16), |p| 0.75 + 0.25 * ((p.y + 7.5) / 15.0)));
    let eyes = meshes.add(blobs(
        &[(7.0, 6.2), (7.0, -6.2)].map(|(x, y)| ellipse_pts(1.7, 1.7, 8).into_iter().map(|q| q + Vec2::new(x, y)).collect::<Vec<_>>()),
        |_| 1.0,
    ));
    let leg_mesh = meshes.add(stroke(&smooth_open(&[Vec2::ZERO, Vec2::new(7.0, 4.5), Vec2::new(14.0, 2.5), Vec2::new(20.0, -1.5)], 4), |t| 2.4 * (1.0 - t) + 0.7, 1.0, 0.7, |_| 1.0));
    let tail_leg = meshes.add(stroke(&smooth_open(&[Vec2::ZERO, Vec2::new(-12.0, 3.0), Vec2::new(-26.0, 4.0), Vec2::new(-42.0, 1.0)], 4), |t| 2.8 * (1.0 - t) + 0.6, 1.0, 0.7, |_| 1.0));
    let fang = |side: f32| stroke(&smooth_open(&[Vec2::ZERO, Vec2::new(7.0, side * -1.0), Vec2::new(12.0, side * -5.0)], 4), |t| 3.6 * (1.0 - t) + 0.5, 1.0, 0.7, |_| 1.0);
    let antenna = |side: f32| stroke(&smooth_open(&[Vec2::ZERO, Vec2::new(12.0, side * 5.0), Vec2::new(24.0, side * 9.0), Vec2::new(36.0, side * 6.0)], 5), |t| 1.8 * (1.0 - t) + 0.4, 1.0, 0.8, |_| 1.0);

    let root = commands.spawn((Transform::from_translation(home.extend(12.0)), Visibility::default())).id();
    let mut b = Builder { commands, meshes, owner: root };
    for k in 0..SEGS {
        let seg = b.commands.spawn((Seg { owner: root, k }, Transform::default(), Visibility::Hidden, ChildOf(root))).id();
        if k == 0 {
            b.mesh(head_plate.clone(), &body, Vec3::ZERO, seg);
            b.mesh(head_cap.clone(), &purple, Vec3::new(-2.0, 0.0, 0.01), seg);
            b.mesh(eyes.clone(), &eye, Vec3::new(0.0, 0.0, 0.02), seg);
            for side in [1.0, -1.0] {
                let f = b.commands.spawn((Part { owner: root, kind: PartKind::Fang(side) }, Transform::from_xyz(14.0, side * 5.0, 0.03), Visibility::default(), ChildOf(seg))).id();
                let m = b.meshes.add(fang(side));
                b.commands.spawn((Mesh2d(m), MeshMaterial2d(leg.clone()), Transform::default(), ChildOf(f)));
                let a = b.commands.spawn((Part { owner: root, kind: PartKind::Antenna(side) }, Transform::from_xyz(11.0, side * 8.0, 0.03), Visibility::default(), ChildOf(seg))).id();
                let m = b.meshes.add(antenna(side));
                b.commands.spawn((Mesh2d(m), MeshMaterial2d(leg.clone()), Transform::default(), ChildOf(a)));
            }
        } else {
            b.mesh(rim.clone(), &body, Vec3::new(0.0, 0.0, -0.01), seg);
            b.mesh(plate.clone(), &body, Vec3::ZERO, seg);
            b.mesh(dorsal.clone(), &purple, Vec3::new(0.0, 0.0, 0.01), seg);
            for side in [1.0, -1.0] {
                let l = b.commands.spawn((Leg { owner: root, k, side }, Transform::from_xyz(0.0, side * 9.0, -0.02), Visibility::default(), ChildOf(seg))).id();
                b.mesh(leg_mesh.clone(), &leg, Vec3::ZERO, l);
            }
            if k == SEGS - 1 {
                for side in [1.0, -1.0] {
                    let t = b.commands.spawn((Part { owner: root, kind: PartKind::TailLeg(side) }, Transform::from_xyz(-6.0, side * 4.0, -0.02), Visibility::default(), ChildOf(seg))).id();
                    b.mesh(tail_leg.clone(), &leg, Vec3::ZERO, t);
                }
            }
        }
        // Dims the plate on the back wall.
        let sh = b.meshes.add(fill(&ellipse_pts(if k == 0 { 16.5 } else { 14.0 }, if k == 0 { 13.5 } else { 14.5 }, 20), |_| 1.0));
        b.commands.spawn((Shade, Mesh2d(sh), MeshMaterial2d(shade[k].clone()), Transform::from_xyz(0.0, 0.0, 0.05), ChildOf(seg)));
    }

    for k in 1..SEGS {
        commands.spawn((
            Plate { owner: root, k },
            Platform { half: Vec2::new(SPACING * 0.5 + 14.0, BACK_THICK), color: BODY, one_way: true, solid: false, stable: false, delta: Vec2::ZERO, tilt: 0.0 },
            Transform::from_translation(home.extend(0.0)),
        ));
    }

    let mut rng = Rng(0xB0AD_5E17);
    let head = home;
    commands.entity(root).insert((
        Boss::new(&SPEC, home, mats).staged(),
        Centipede {
            state: State::Dormant,
            head,
            heading: Vec2::X,
            vel: Vec2::ZERO,
            phase: 0.0,
            trail: (0..TRAIL).map(|i| head - Vec2::X * STEP * i as f32).collect(),
            trail_depth: (0..TRAIL).map(|_| 0.0).collect(),
            seg_depth: vec![0.0; SEGS],
            vanish: head,
            pts: vec![head; SEGS],
            vis: vec![0.0; SEGS],
            hide_front: f32::INFINITY,
            hide_rear: f32::INFINITY,
            depth: 0.0,
            seed: rng.range(0.0, 100.0),
            domain,
            in_rock: true,
            center: head,
            orbit: 1.0,
            target: head,
            last_exit: head,
            since_rise: 0,
            since_back: 0,
            since_flee: 99,
            shown: vec![head; SEGS],
            strike_dir: Vec2::X,
            since_swat: 1,
            wig: vec![0.0; SEGS],
            wave: 0.0,
            effort: 0.0,
            prev: vec![head; SEGS],
            mark_d: 0.0,
            introduced: false,
            react: React::None,
            dismember: false,
            dead_vel: vec![Vec2::ZERO; SEGS],
            shade,
            jaws: 0.0,
            dead_at: head,
            dead_phase: 0,
            rng,
        },
    ));
    root
}

struct Builder<'a, 'w, 's> {
    commands: &'a mut Commands<'w, 's>,
    meshes: &'a mut Assets<Mesh>,
    #[allow(dead_code)]
    owner: Entity,
}

impl Builder<'_, '_, '_> {
    fn mesh(&mut self, mesh: Handle<Mesh>, mat: &Handle<ColorMaterial>, pos: Vec3, parent: Entity) {
        self.commands.spawn((Mesh2d(mesh), MeshMaterial2d(mat.clone()), Transform::from_translation(pos), ChildOf(parent)));
    }
}

// --- Reading the terrain ---

/// The nearest solid within `range` of `p`: its distance and the normal of the surface there
/// (pointing out of the rock, toward `p`). Found with a fan of rays.
fn probe(terrain: &Terrain, p: Vec2, range: f32) -> Option<(f32, Vec2)> {
    const RAYS: usize = 24;
    let mut hits = [f32::MAX; RAYS];
    let mut best = f32::MAX;
    for (i, hit) in hits.iter_mut().enumerate() {
        let d = Vec2::from_angle(i as f32 / RAYS as f32 * TAU);
        let mut r = 3.0;
        while r <= range {
            if terrain.solid_at(p + d * r) {
                *hit = r;
                best = best.min(r);
                break;
            }
            r += 12.0;
        }
    }
    if best == f32::MAX {
        return None;
    }
    let mut n = Vec2::ZERO;
    for (i, &h) in hits.iter().enumerate() {
        if h < best * 1.3 + 30.0 {
            n -= Vec2::from_angle(i as f32 / RAYS as f32 * TAU) / (1.0 + (h - best) * 0.03);
        }
    }
    Some((best, n.normalize_or(Vec2::Y)))
}

/// What the head is in.
#[derive(Clone, Copy)]
enum Medium {
    Rock,
    /// In the open, with a surface in reach of its legs (that surface's normal).
    Grip(Vec2),
    /// In the open, nothing to hold on to.
    Air,
}

/// Outside its lair it is in the depths: as good as rock (it never falls out of the world).
fn medium(terrain: &Terrain, domain: Rect, p: Vec2) -> Medium {
    if terrain.solid_at(p) || !domain.contains(p) {
        Medium::Rock
    } else if let Some((_, n)) = probe(terrain, p, SKIN + 90.0) {
        Medium::Grip(n)
    } else {
        Medium::Air
    }
}

/// The one way it moves, a heavy body's: it thrusts toward `want` at `speed` (gaining at most
/// `accel` per second) when it has something to push against, the rock or a surface its legs reach
/// (`free`: anyway, on the back wall or with the body behind pushing a strike), and turns no tighter
/// than its speed allows. In the open with nothing to grip it only coasts, steers a little, and falls.
fn drive(s: &mut Centipede, terrain: &Terrain, want: Vec2, speed: f32, accel: f32, dt: f32, free: bool) {
    let grip = free || !matches!(medium(terrain, s.domain, s.head), Medium::Air);
    let sp = s.vel.length();
    let cur = if sp > 1.0 { s.vel / sp } else { s.heading };
    let turn = (LATERAL / sp.max(1.0)).min(MAX_TURN) * if grip { 1.0 } else { AIR_STEER };
    let dir = turn_toward(cur, want, turn * dt);
    let sp = if grip { sp + (speed - sp).clamp(-BRAKE * dt, accel * dt) } else { sp };
    let mut vel = dir * sp;
    if !grip {
        vel.y -= FALL * dt;
    }
    s.vel = vel.clamp_length_max(2600.0);
    s.head += s.vel * dt;
    if let Some(d) = s.vel.try_normalize() {
        s.heading = turn_toward(s.heading, d, 8.0 * dt);
    }
    s.phase += s.vel.length() * dt * 0.0045;
}

/// Flying on its momentum: steering what little it can toward `aim`, and (`dig`) into the rock as
/// soon as there is a surface under its legs.
fn coast(s: &mut Centipede, terrain: &Terrain, aim: Option<Vec2>, dig: bool, dt: f32) {
    let want = match (medium(terrain, s.domain, s.head), aim) {
        (Medium::Grip(n), _) if dig => (-n * 0.8 + s.heading * 0.5).normalize_or(-n),
        (_, Some(a)) => (a - s.head).normalize_or(s.heading),
        _ => s.vel.normalize_or(s.heading),
    };
    let sp = s.vel.length().max(500.0);
    drive(s, terrain, want, sp, ACCEL, dt, false);
}

/// Toward `want`, the way that keeps it in the rock: it looks ahead along a fan of headings round it.
fn through_rock(terrain: &Terrain, s: &Centipede, want: Vec2) -> Vec2 {
    let cur = s.vel.normalize_or(s.heading);
    let mut best = (f32::MIN, want);
    for off in [0.0, 0.3, -0.3, 0.6, -0.6, 1.0, -1.0, 1.5, -1.5, 2.2, -2.2] {
        let d = Vec2::from_angle(want.to_angle() + off);
        // Open space ahead counts against a heading; so does leaving the lair, a little less.
        let open: f32 = [150.0, 320.0, 500.0]
            .iter()
            .map(|&k| {
                let q = s.head + d * k;
                if !s.domain.contains(q) {
                    0.6
                } else if terrain.solid_at(q) {
                    0.0
                } else {
                    1.0
                }
            })
            .sum::<f32>()
            / 3.0;
        let score = off.cos() - 1.6 * open - 0.25 * d.angle_to(cur).abs() / PI;
        if score > best.0 {
            best = (score, d);
        }
    }
    best.1
}

/// From inside the rock, the nearest open space within `range`: its distance and direction.
fn nearest_open(terrain: &Terrain, p: Vec2, range: f32) -> Option<(f32, Vec2)> {
    let mut r = 16.0;
    while r <= range {
        for i in 0..16 {
            let d = Vec2::from_angle(i as f32 / 16.0 * TAU);
            if !terrain.solid_at(p + d * r) {
                return Some((r, d));
            }
        }
        r += 24.0;
    }
    None
}

/// The launch velocities of speed `v` (low arc first) that carry something falling at `FALL`
/// from `from` through `to`.
fn aim(from: Vec2, to: Vec2, v: f32) -> Vec<Vec2> {
    let d = to - from;
    let (x, y, g, v2) = (d.x.abs(), d.y, FALL, v * v);
    if x < 40.0 {
        return if y > 0.0 && v2 > 2.0 * g * y {
            vec![Vec2::Y * v]
        } else if y < 0.0 {
            vec![Vec2::NEG_Y * v]
        } else {
            vec![]
        };
    }
    let disc = v2 * v2 - g * (g * x * x + 2.0 * y * v2);
    if disc < 0.0 {
        return vec![];
    }
    [-1.0, 1.0]
        .iter()
        .map(|k| {
            let a = ((v2 + k * disc.sqrt()) / (g * x)).atan();
            Vec2::new(d.x.signum() * a.cos(), a.sin()) * v
        })
        .collect()
}

/// The arc out of the surface at `at` with `vel` gets to `to` through the open.
fn clear_flight(terrain: &Terrain, at: Vec2, vel: Vec2, to: Vec2) -> bool {
    let (mut p, mut v) = (at, vel);
    let mut closest = f32::MAX;
    for _ in 0..120 {
        p += v * 0.025;
        v.y -= FALL * 0.025;
        let d = p.distance(to);
        if d > closest + 40.0 {
            break;
        }
        closest = closest.min(d);
        if terrain.solid_at(p) && p.distance(at) > 140.0 {
            return false;
        }
    }
    closest < 120.0
}

/// A breach: a surface near the chameleon to come out of on an arc through `target`.
/// (point just inside the rock, its normal, launch velocity)
fn plan_breach(s: &mut Centipede, terrain: &Terrain, target: Vec2, speed: f32) -> Option<(Vec2, Vec2, Vec2)> {
    let mut best: Option<(f32, Vec2, Vec2, Vec2)> = None;
    for _ in 0..48 {
        let dir = Vec2::from_angle(s.rng.range(0.0, TAU));
        let Some(hit) = terrain.raycast(target, target + dir * s.rng.range(350.0, 1300.0)) else { continue };
        if !s.domain.contains(hit) {
            continue;
        }
        let Some((_, n)) = probe(terrain, hit - dir * 5.0, 48.0) else { continue };
        for launch in aim(hit, target, speed * s.rng.range(0.9, 1.1)) {
            let l = launch.normalize();
            // Out of the surface, with rock behind it for the run-up, and nothing in the way.
            if l.dot(n) < 0.3 || ![120.0, 300.0, 600.0, 900.0].iter().all(|&k| terrain.solid_at(hit - l * k)) || !clear_flight(terrain, hit, launch, target) {
                continue;
            }
            let score = s.rng.range(0.0, 0.6) + 0.5 * l.dot(n) - (hit.distance(target) - 800.0).abs() / 1500.0 - (hit - l * 600.0).distance(s.head) / 5000.0
                - if hit.distance(s.last_exit) < 500.0 { 1.0 } else { 0.0 };
            if best.is_none_or(|b| score > b.0) {
                best = Some((score, hit, n, launch));
            }
        }
    }
    best.map(|b| (b.1, b.2, b.3))
}

/// A rise: a floor or wall on the chameleon's level, in view of it, to come straight out of.
fn plan_rise(s: &mut Centipede, terrain: &Terrain, ppos: Vec2) -> Option<(Vec2, Vec2, Vec2)> {
    let mut best: Option<(f32, Vec2, Vec2)> = None;
    for _ in 0..48 {
        let dir = Vec2::from_angle(s.rng.range(0.0, TAU));
        let Some(hit) = terrain.raycast(ppos, ppos + dir * s.rng.range(450.0, 1000.0)) else { continue };
        if !s.domain.contains(hit) || hit.distance(ppos) < 380.0 {
            continue;
        }
        let Some((_, n)) = probe(terrain, hit - dir * 5.0, 48.0) else { continue };
        // Not out of the ceiling; room to rise into, rock behind, and the chameleon in sight.
        if n.y < -0.3
            || [150.0, 350.0, 550.0].iter().any(|&k| terrain.solid_at(hit + n * k))
            || ![120.0, 300.0, 600.0, 900.0].iter().all(|&k| terrain.solid_at(hit - n * k))
            || terrain.raycast(hit + n * 450.0, ppos).is_some()
        {
            continue;
        }
        let score = s.rng.range(0.0, 0.5) + n.y.max(0.0) - (hit - n * 600.0).distance(s.head) / 5000.0 - if hit.distance(s.last_exit) < 500.0 { 1.0 } else { 0.0 };
        if best.is_none_or(|b| score > b.0) {
            best = Some((score, hit, n));
        }
    }
    best.map(|b| (b.1, b.2, b.2 * RISE_SPEED))
}

/// Risen: a swing of the whole column sideways onto the chameleon, pivoting on the hole, when the
/// chameleon is in its reach to one side. (side, pivot, trail points out of the hole)
fn swat_plan(s: &Centipede, terrain: &Terrain, ppos: Vec2) -> Option<(f32, Vec2, usize)> {
    let n = s.trail.iter().position(|&q| terrain.solid_at(q))?;
    if n < 6 {
        return None;
    }
    let pivot = s.trail[n];
    let up = s.head - pivot;
    let len = up.length();
    let rel = ppos - pivot;
    let reach = rel.length();
    (reach > 150.0 && reach < len + 250.0 && up.angle_to(rel).abs() > 0.5).then_some((up.perp_dot(rel).signum(), pivot, n))
}

/// A retreat: a spot deep in the rock, far from the chameleon and well below it, preferring the
/// lowest and the one not too far from where it is.
fn plan_flee(s: &mut Centipede, terrain: &Terrain, ppos: Vec2) -> Option<Vec2> {
    let d = s.domain.inflate(-250.0);
    let mut best: Option<(f32, Vec2)> = None;
    for _ in 0..64 {
        let p = Vec2::new(s.rng.range(d.min.x, d.max.x), s.rng.range(d.min.y, d.max.y));
        if p.y > ppos.y - 500.0 || p.distance(ppos) < 1500.0 || ![0.0, 1.0, 2.0, 3.0].iter().all(|&i| terrain.solid_at(p + Vec2::from_angle(i * FRAC_PI_2) * 200.0)) {
            continue;
        }
        let score = (ppos.y - p.y) / 1000.0 + s.rng.range(0.0, 0.8) - p.distance(s.head) / 6000.0;
        if best.is_none_or(|b| score > b.0) {
            best = Some((score, p));
        }
    }
    best.map(|b| b.1)
}

/// A prowl through the rock before the next move.
fn prowl(s: &mut Centipede, rage: f32) -> State {
    State::Prowl(s.rng.range(PROWL_TIME.0, PROWL_TIME.1) / rage)
}

/// Done prowling: the next move, from where the chameleon is and what it did last. Mostly
/// breaches; a rise once in a while when the chameleon is on the ground; now and then the back wall.
fn next_move(s: &mut Centipede, terrain: &Terrain, ppos: Vec2, pvel: Vec2, grounded: bool, rage: f32) -> State {
    if s.rng.chance(0.35) {
        s.orbit = -s.orbit;
    }
    if s.since_back >= 5 && s.rng.chance(0.5) {
        s.since_back = 0;
        return State::Back(s.rng.range(5.0, 7.0));
    }
    // Enraged, every few moves it runs for the depths instead.
    if rage > 1.0 && s.since_flee >= 3 {
        if let Some(goal) = plan_flee(s, terrain, ppos) {
            s.since_flee = 0;
            return State::Flee { goal, t: 0.0 };
        }
    }
    s.since_flee += 1;
    // A breach goes through where the chameleon will be.
    let target = ppos + Vec2::new(pvel.x * 0.5, 30.0);
    let speed = BREACH_SPEED * rage.sqrt();
    let plan = if grounded && s.since_rise >= 2 {
        plan_rise(s, terrain, ppos).map(|p| (Move::Rise, p)).or_else(|| plan_breach(s, terrain, target, speed).map(|p| (Move::Breach, p)))
    } else {
        plan_breach(s, terrain, target, speed).map(|p| (Move::Breach, p)).or_else(|| plan_rise(s, terrain, ppos).map(|p| (Move::Rise, p)))
    };
    let Some((mv, (at, n, launch))) = plan else { return State::Prowl(1.0) };
    s.since_back += 1;
    s.since_rise = if mv == Move::Rise { 0 } else { s.since_rise + 1 };
    s.target = target;
    State::Approach { t: 0.0, mv, at, n, launch, locked: false, intro: false }
}

/// Turn the unit vector `from` toward `to` by at most `max` radians.
fn turn_toward(from: Vec2, to: Vec2, max: f32) -> Vec2 {
    Vec2::from_angle(from.to_angle() + from.angle_to(to).clamp(-max, max))
}

fn push_trail(trail: &mut VecDeque<Vec2>, depths: &mut VecDeque<f32>, head: Vec2, depth: f32) {
    let Some(&last) = trail.front() else {
        trail.push_front(head);
        depths.push_front(depth);
        return;
    };
    let d = head - last;
    let l = d.length();
    if l >= STEP {
        for i in 1..=(l / STEP) as usize {
            trail.push_front(last + d * (i as f32 * STEP / l));
            depths.push_front(depth);
        }
        while trail.len() > TRAIL + 30 {
            trail.pop_back();
            depths.pop_back();
        }
    }
    // The newest point is where the head is now.
    if let Some(f) = depths.front_mut() {
        *f = depth;
    }
}

/// The depth of each plate: that of the head when it went through where the plate is now.
fn body_depth(depth: f32, depths: &VecDeque<f32>, out: &mut [f32]) {
    for (k, o) in out.iter_mut().enumerate() {
        let at = k as f32 * SPACING / STEP;
        let (i, f) = (at as usize, at.fract());
        let get = |i: usize| depths.get(i).or(depths.back()).copied().unwrap_or(depth);
        *o = if k == 0 { depth } else { get(i) * (1.0 - f) + get(i + 1) * f };
    }
}

/// The plates: one every `SPACING` along the head's path.
fn body(head: Vec2, trail: &VecDeque<Vec2>, out: &mut [Vec2]) {
    out[0] = head;
    let (mut cur, mut next, mut left, mut n) = (head, 0, SPACING, 1);
    while n < out.len() {
        let Some(&to) = trail.get(next) else {
            out[n] = cur;
            n += 1;
            continue;
        };
        let d = to - cur;
        let l = d.length();
        if l >= left {
            cur += d / l * left;
            out[n] = cur;
            n += 1;
            left = SPACING;
        } else {
            left -= l;
            cur = to;
            next += 1;
        }
    }
}

/// Dust and rock thrown up out of a surface.
pub(crate) fn burst(commands: &mut Commands, assets: &Assets3, rng: &mut Rng, at: Vec2, normal: Vec2, count: usize, speed: f32) {
    for _ in 0..count {
        let dir = (normal * rng.range(0.3, 1.0) + normal.perp() * rng.range(-0.9, 0.9)).normalize_or(normal);
        let life = rng.range(0.5, 1.1);
        commands.spawn((
            Chunk { vel: dir * speed * rng.range(0.4, 1.0), spin: rng.range(-9.0, 9.0), life, total: life },
            Mesh2d(assets.chunk.clone()),
            MeshMaterial2d(assets.chunk_mat.clone()),
            Transform::from_translation((at + normal.perp() * rng.range(-60.0, 60.0)).extend(13.0)).with_scale(Vec3::splat(rng.range(2.0, 4.8))),
            BossStuff,
        ));
    }
}

fn think(
    time: Res<Time>,
    mut commands: Commands,
    mut materials: ResMut<Assets<ColorMaterial>>,
    mut shake: ResMut<Shake>,
    mut stop: ResMut<HitStop>,
    mut bleed: MessageWriter<Bleed>,
    assets: Res<Assets3>,
    player: Single<(&mut Player, &mut Transform, &ChameleonAnim), Without<Centipede>>,
    mut bosses: Query<(Entity, &mut Centipede, &mut Boss, &mut Transform, Option<&mut boss::Sense>)>,
    mut plates: Query<(Entity, &Plate, &mut Platform, &mut Transform), (Without<Centipede>, Without<Player>)>,
    parts: Query<(&Part, &GlobalTransform)>,
    terrain: Res<Terrain>,
    mut view: ResMut<ArenaView>,
) {
    let dt = time.delta_secs().min(1.0 / 30.0) * crate::boss::tempo();
    let t = time.elapsed_secs();
    let (mut p, mut ptf_player, anim) = player.into_inner();
    let ppos = ptf_player.translation.truncate();

    for (me, mut s, mut boss, mut tf, sense) in &mut bosses {
        let s = &mut *s;
        let alive = p.dead.is_none();
        let hunting = alive && boss.awake;
        let enraged = boss.enraged();
        let rage = if enraged { 1.3 } else { 1.0 };
        let before = s.head;
        s.vanish = ppos;
        let sees = |from: Vec2| terrain.raycast(from, ppos).is_none();

        // A flurry of hits while it is up and winding: it strikes at once.
        if std::mem::replace(&mut s.react, React::None) == React::Answer {
            if let State::Rear { t: tr, .. } = s.state {
                s.state = State::Rear { t: tr, w: WINDUP };
            }
        }

        let mut depth_goal = 0.0;
        s.jaws += (0.0 - s.jaws) * (1.0 - (-8.0 * dt).exp());
        s.state = match s.state {
            State::Dormant => {
                if hunting {
                    // It wakes and comes for you, out of the rock near you (from inside the rock
                    // next to its home, when that is out in the open).
                    s.head = boss.home;
                    if !terrain.solid_at(s.head) {
                        if let Some((d, n)) = probe(&terrain, s.head, 1500.0) {
                            s.head -= n * (d + 250.0);
                        }
                    }
                    s.vel = Vec2::ZERO;
                    s.heading = (ppos - s.head).normalize_or(Vec2::Y);
                    s.trail = (0..TRAIL).map(|_| s.head).collect();
                    s.trail_depth = (0..TRAIL).map(|_| s.depth).collect();
                    s.center = ppos;
                    match next_move(s, &terrain, ppos, p.vel, p.grounded, rage) {
                        State::Approach { t: ta, mv, at, n, launch, .. } => {
                            s.hide_front = f32::NEG_INFINITY;
                            s.hide_rear = 0.0;
                            State::Approach { t: ta, mv, at, n, launch, locked: false, intro: true }
                        }
                        _ => State::Dormant,
                    }
                } else {
                    State::Dormant
                }
            }

            State::Prowl(left) => {
                // A wide loop through the rock round the chameleon, one way round; the loop follows it slowly.
                s.center = s.center.lerp(ppos, 1.0 - (-0.6 * dt).exp());
                let a = (s.head - s.center).to_angle() + s.orbit * 0.7;
                let inner = s.domain.inflate(-150.0);
                let goal = (s.center + Vec2::from_angle(a) * PROWL_R / rage.sqrt()).clamp(inner.min, inner.max);
                let want = through_rock(&terrain, s, (goal - s.head).normalize_or(s.heading));
                drive(s, &terrain, want, CRUISE * rage, ACCEL, dt, false);
                if !hunting {
                    State::Home
                } else if left <= 0.0 {
                    next_move(s, &terrain, ppos, p.vel, p.grounded, rage)
                } else {
                    State::Prowl(left - dt)
                }
            }

            State::Approach { t: ta, mv, mut at, mut n, launch, mut locked, intro } => {
                // The run-up: onto the line it will come out along, from well back on it, gathering speed.
                let l = launch.normalize_or(n);
                let rel = s.head - at;
                let along = rel.dot(l);
                let off = (rel - l * along).length();
                // Lost (beyond the spot, or near it well off the line): round through the rock to the
                // start of the run-up.
                let lost = along > 0.0 || (along > -400.0 && off > 250.0);
                let mut want = if lost { (at - l * 1000.0 - s.head).normalize_or(l) } else { (at + l * (along + 350.0) - s.head).normalize_or(l) };
                if lost || along < -600.0 {
                    want = through_rock(&terrain, s, want);
                }
                let dist = s.head.distance(at);
                let close = dist < 900.0 && off < 200.0 && along < 0.0;
                // Far off and deep in the rock, where nobody sees it, it hurries.
                let hurry = if dist > 1500.0 && s.in_rock { 1.4 } else { 1.0 };
                drive(s, &terrain, want, if close { launch.length() } else { CRUISE * rage * hurry }, if close { 1600.0 } else { ACCEL }, dt, false);
                let tell = s.vel.length().max(500.0) * TELEGRAPH;
                // The crack opens where it is really going to come out: where its way leaves the rock.
                let mut exit = None;
                if !locked && s.in_rock && dist < tell + 500.0 {
                    let dir = s.vel.normalize_or(s.heading);
                    let mut k = 24.0;
                    while k < tell {
                        let q = s.head + dir * k;
                        if !terrain.solid_at(q) {
                            exit = Some(q - dir * 12.0);
                            break;
                        }
                        k += 24.0;
                    }
                }
                if let Some(q) = exit.filter(|q| q.distance(at) < 600.0) {
                    at = q;
                    n = probe(&terrain, q + s.vel.normalize_or(s.heading) * 30.0, 60.0).map_or(n, |(_, n)| n);
                    locked = true;
                    crack(&mut commands, &mut materials, &assets, at, n, 3.6, TELEGRAPH + 0.25, 1.0);
                }
                if locked && s.in_rock {
                    let k = (1.0 - dist / tell).clamp(0.0, 1.0);
                    shake.add(0.6 * dt * (0.4 + k));
                    if s.rng.chance(dt * (14.0 + 50.0 * k)) {
                        burst(&mut commands, &assets, &mut s.rng, at, n, 2, 300.0 + 500.0 * k);
                    }
                }
                if s.in_rock && !terrain.solid_at(s.head) && dist < 500.0 && s.vel.normalize_or_zero().dot(l) > 0.4 {
                    // Out.
                    s.last_exit = at;
                    if intro {
                        s.introduced = true;
                        shake.add(0.4);
                    }
                    match mv {
                        Move::Breach => State::Air(0.0),
                        Move::Rise => State::Rear { t: 0.0, w: 0.0 },
                    }
                } else if !hunting && !intro {
                    State::Home
                } else if ta > 14.0 {
                    prowl(s, rage)
                } else {
                    State::Approach { t: ta + dt, mv, at, n, launch, locked, intro }
                }
            }

            State::Air(ta) => {
                // On its momentum: a little steering toward where it means to pass, then down into the rock.
                let passed = (s.target - s.head).dot(s.vel) < 0.0;
                coast(s, &terrain, (!passed).then_some(s.target), passed || ta > 0.4, dt);
                s.jaws = s.jaws.max(0.7);
                if terrain.solid_at(s.head) || ta > 6.0 { prowl(s, rage) } else { State::Air(ta + dt) }
            }

            State::Rear { t: tr, w } => {
                // Out of the hole, head up, slowing to a stop.
                let sp = (s.vel.length() - REAR_BRAKE * dt).max(0.0);
                s.vel = s.vel.normalize_or(s.heading) * sp;
                s.head += s.vel * dt;
                s.phase += sp * dt * 0.0045;
                let mut w = w;
                if sp < 60.0 {
                    // Then eyes the chameleon, swaying, jaws opening: the windup.
                    let to = (ppos - s.head).normalize_or(s.heading);
                    s.heading = turn_toward(s.heading, to, 1.4 * dt);
                    s.head += s.heading.perp() * (t * 2.2 + s.seed).sin() * 14.0 * dt;
                    w += dt;
                    s.jaws = s.jaws.max((w / (WINDUP / rage)).min(1.0));
                }
                if w >= WINDUP / rage {
                    // At it head first, or (in reach to one side, and it struck last time) the whole column sideways.
                    match swat_plan(s, &terrain, ppos).filter(|_| s.since_swat > 0) {
                        Some((side, pivot, n)) => {
                            s.since_swat = 0;
                            State::Swat { t: 0.0, side, w: 0.0, pivot, n }
                        }
                        None => {
                            s.since_swat += 1;
                            s.strike_dir = (ppos + p.vel * 0.15 - s.head).normalize_or(s.heading);
                            shake.add(0.3);
                            State::Strike(0.0)
                        }
                    }
                } else if tr > 6.0 {
                    prowl(s, rage)
                } else {
                    State::Rear { t: tr + dt, w }
                }
            }

            State::Strike(ts) => {
                s.jaws = 1.0;
                if ts < STRIKE_TIME {
                    // The body behind throws it forward...
                    let dir = s.strike_dir;
                    drive(s, &terrain, dir, STRIKE_SPEED * rage, 6000.0, dt, true);
                } else {
                    // ...and whatever it misses, it can't stop: on into the rock.
                    coast(s, &terrain, None, true, dt);
                }
                if (ts > 0.1 && terrain.solid_at(s.head)) || ts > 5.0 { prowl(s, rage) } else { State::Strike(ts + dt) }
            }

            State::Swat { t: tw, side, w, pivot, n } => {
                // The column leans away, then falls sideways like a felled tree, onto whatever is there.
                let (turn, w) = if tw < SWAT_LEAN {
                    (-side * 0.35 * dt, w)
                } else {
                    let w = (w + SWAT_ACCEL * rage * dt).min(SWAT_MAX * rage);
                    (side * w * dt, w)
                };
                let rot = Vec2::from_angle(turn);
                let swing = |q: Vec2| pivot + rot.rotate(q - pivot);
                s.head = swing(s.head);
                for q in s.trail.iter_mut().take(n) {
                    *q = swing(*q);
                }
                s.heading = rot.rotate(s.heading);
                s.vel = (s.head - before) / dt.max(1e-4);
                s.jaws = s.jaws.max(0.8);
                if tw > SWAT_LEAN && terrain.solid_at(s.head) {
                    // Down: the ground takes the blow.
                    shake.add(1.0);
                    stop.add(0.08);
                    burst(&mut commands, &assets, &mut s.rng, before, -s.vel.normalize_or(Vec2::NEG_Y), 40, 1100.0);
                    prowl(s, rage)
                } else if tw > 4.0 {
                    State::Strike(STRIKE_TIME)
                } else {
                    State::Swat { t: tw + dt, side, w, pivot, n }
                }
            }

            State::Back(left) => {
                depth_goal = 1.0;
                let r = 900.0 + 250.0 * (t * 0.4 + s.seed).sin();
                let a = (s.head - ppos).to_angle() + s.orbit * 0.6;
                let mut goal = ppos + Vec2::from_angle(a) * r;
                if !s.domain.inflate(-400.0).contains(s.head) {
                    goal = s.domain.center();
                }
                let want = (goal - s.head).normalize_or(s.heading);
                let far = 1.0 - FAR_SHRINK * s.depth;
                drive(s, &terrain, want, BACK_SPEED * far * rage, ACCEL * far, dt, true);
                if !boss.awake {
                    State::Home
                } else if !hunting {
                    State::Back(left)
                } else if left <= 0.0 && s.head.distance(ppos) < 1800.0 && !terrain.solid_at(s.head) && sees(s.head) {
                    State::Swing(0.0)
                } else {
                    State::Back((left - dt).max(0.0))
                }
            }

            State::Swing(tw) => {
                // Out of the wall toward the chameleon, slowing, jaws opening.
                let to = (ppos - s.head).normalize_or(s.heading);
                let far = 1.0 - FAR_SHRINK * s.depth;
                drive(s, &terrain, to, BACK_SPEED * 0.8 * far, ACCEL * far, dt, true);
                s.jaws = s.jaws.max(tw / SWING_TIME);
                if terrain.solid_at(s.head) {
                    // Swung into the rock: back onto the wall for another go.
                    State::Back(1.0)
                } else if tw >= SWING_TIME / rage && (s.depth < 0.25 || tw >= SWING_TIME * 2.0) {
                    s.strike_dir = (ppos + p.vel * 0.15 - s.head).normalize_or(s.heading);
                    shake.add(0.3);
                    State::Strike(0.0)
                } else {
                    State::Swing(tw + dt)
                }
            }

            State::Flee { goal, t: tfl } => {
                let want = through_rock(&terrain, s, (goal - s.head).normalize_or(s.heading));
                drive(s, &terrain, want, CRUISE * rage, ACCEL, dt, false);
                if !hunting {
                    State::Home
                } else if s.head.distance(goal) < 250.0 || tfl > 30.0 {
                    State::Lurk(0.0)
                } else {
                    State::Flee { goal, t: tfl + dt }
                }
            }

            State::Lurk(tl) => {
                // Nearly still in the rock, until the chameleon is on top of it (or tired of waiting).
                let want = through_rock(&terrain, s, s.heading);
                drive(s, &terrain, want, 120.0, ACCEL, dt, false);
                if !hunting {
                    State::Home
                } else if s.head.distance(ppos) < 1000.0 || tl > 12.0 {
                    next_move(s, &terrain, ppos, p.vel, p.grounded, rage)
                } else {
                    State::Lurk(tl + dt)
                }
            }

            State::Home => {
                let dist = s.head.distance(boss.home);
                let want = through_rock(&terrain, s, (boss.home - s.head).normalize_or(s.heading));
                drive(s, &terrain, want, CRUISE * 0.8 * (dist / 800.0).clamp(0.3, 1.0), ACCEL, dt, false);
                if hunting {
                    prowl(s, rage)
                } else if dist < 200.0 {
                    // Home: asleep in the rock again.
                    s.hide_front = f32::INFINITY;
                    s.hide_rear = f32::INFINITY;
                    State::Dormant
                } else {
                    State::Home
                }
            }

            State::Dead(td) => State::Dead(td + dt),
        };

        // Which side of the world it is on.
        // Depth is moved by moving: the way it travelled goes into the third axis, toward its goal.
        let step = s.head.distance(before) / DEPTH_DIST;
        s.depth += (depth_goal - s.depth).clamp(-step, step);

        // Through the surface: rock flies, and whatever is over the hole gets thrown.
        let now_rock = terrain.solid_at(s.head);
        // No tremors, dust or crumbling unless it is all the way in front: on its way back it is far away.
        let front = s.depth < 0.02 && !matches!(s.state, State::Dormant | State::Dead(_));
        if front && now_rock != s.in_rock {
            let sp = s.vel.length();
            let dir = s.vel.normalize_or(s.heading);
            if !now_rock {
                burst(&mut commands, &assets, &mut s.rng, s.head, dir, 40, 1300.0 * (sp / 1500.0).clamp(0.4, 1.0));
                shake.add(0.7 * (sp / 1500.0).clamp(0.3, 1.0));
                if sp > 900.0 {
                    stop.add(0.05);
                }
            } else {
                burst(&mut commands, &assets, &mut s.rng, before, -dir, 18, 800.0);
                shake.add(0.25 * (sp / 1000.0).clamp(0.3, 1.0));
            }
        }
        s.in_rock = now_rock;

        // Under the surface it gives itself away, all along the body: dust shaken out of the floors
        // and walls over it, pebbles raining from the ceilings under it, tremor marks tracing the
        // way the head goes, and the rumble.
        if front {
            let fast = (s.vel.length() / CRUISE).min(1.5);
            s.mark_d += s.head.distance(before);
            for k in [0, 6, 12, 18] {
                // Only plates really in front of the world, and (for the body) close to the surface:
                // a far plate must not shake dust out of a floor it is nowhere near.
                if s.seg_depth[k] > 0.05 {
                    continue;
                }
                let q = if k == 0 { s.head } else { s.pts[k] };
                if !terrain.solid_at(q) {
                    continue;
                }
                let range = if k == 0 { WAKE_RANGE } else { WAKE_RANGE * 0.45 };
                let Some((d, dir)) = nearest_open(&terrain, q, range) else { continue };
                let near = 1.0 - d / range;
                let at = q + dir * d;
                if s.rng.chance(dt * if k == 0 { 30.0 } else { 9.0 } * near * fast) {
                    if dir.y < -0.5 {
                        burst(&mut commands, &assets, &mut s.rng, at, dir, 2, 60.0);
                    } else {
                        burst(&mut commands, &assets, &mut s.rng, at, dir, 1, 220.0 + 260.0 * near);
                    }
                }
                if k == 0 && s.mark_d > 160.0 && near > 0.15 {
                    s.mark_d = 0.0;
                    crack(&mut commands, &mut materials, &assets, at, dir, 0.9 + 1.2 * near, 1.8, 0.65 * near);
                }
            }
            if now_rock {
                let near = (1.0 - s.head.distance(ppos) / 1600.0).max(0.0);
                shake.add(0.25 * dt * near * near * fast.min(1.0));
            }
        }

        // The body.
        let moved = s.head.distance(before);
        if matches!(s.state, State::Dead(_)) {
            let td = if let State::Dead(td) = s.state { td } else { 0.0 };
            s.hide_front = f32::NEG_INFINITY;
            s.hide_rear = f32::INFINITY;
            // Only the head is left: it flies to where the last blow landed, falls, and stays there.
            let Centipede { pts, dead_vel, dead_at, dead_phase, .. } = &mut *s;
            match *dead_phase {
                0 => {
                    let to = *dead_at - pts[0];
                    if to.length() > 12.0 {
                        pts[0] += to.normalize() * (3000.0 * dt).min(to.length());
                    } else {
                        *dead_phase = 1;
                        dead_vel[0] = Vec2::ZERO;
                    }
                }
                1 => {
                    dead_vel[0].y -= 1800.0 * dt;
                    pts[0] += dead_vel[0] * dt;
                    if terrain.solid_at(pts[0] - Vec2::Y * SEG_R * 0.6) {
                        for _ in 0..40 {
                            if !terrain.solid_at(pts[0] - Vec2::Y * SEG_R * 0.6) {
                                break;
                            }
                            pts[0].y += 4.0;
                        }
                        *dead_phase = 2;
                    }
                }
                _ => {}
            }
            let _ = td;
            s.head = s.pts[0];
        } else {
            if s.hide_rear.is_finite() {
                s.hide_rear += moved;
                if s.hide_rear > LENGTH + 3.0 * SPACING {
                    s.hide_rear = f32::INFINITY;
                }
            }
            let head = s.head;
            let depth = s.depth;
            let Centipede { pts, trail, trail_depth, seg_depth, .. } = &mut *s;
            push_trail(trail, trail_depth, head, depth);
            body(head, trail, pts);
            body_depth(depth, trail_depth, seg_depth);
        }
        let (fr, rr, dead) = (s.hide_front, s.hide_rear, matches!(s.state, State::Dead(_)));

        // In the open it writhes, a wave running down the body. What lies in the rock or down on a
        // surface (its legs hold it there) anchors it and doesn't sway, and the sway dies away toward
        // the anchors and the neck. Going into the rock, the last of the tail still out whips about:
        // the harder the closer it is to being dragged in, the tip widest, the part at the hole still.
        if !dead {
            let calm = match s.state {
                State::Swat { .. } => 0.0,
                State::Rear { w, .. } if w > 0.0 => 0.35,
                _ => 1.0,
            } * (1.0 - s.depth);
            let base = s.pts.clone();
            let anchored: Vec<bool> = (0..SEGS)
                .map(|k| {
                    let q = base[k];
                    terrain.solid_at(q) || (0..8).any(|i| terrain.solid_at(q + Vec2::from_angle(i as f32 * TAU / 8.0) * SEG_R * taper(k) * 1.5))
                })
                .collect();
            // The tail going in: the plates still out behind the last one in the rock.
            let hole = (0..SEGS).rev().find(|&k| terrain.solid_at(base[k]));
            let entering = s.in_rock && matches!(s.state, State::Prowl(_) | State::Approach { .. } | State::Home);
            let (hole, tail_out) = match hole {
                Some(h) if entering => (h, SEGS - 1 - h),
                _ => (SEGS, 0),
            };
            let pressure = if tail_out > 0 { (1.0 - (tail_out as f32 - 2.0) / 8.0).clamp(0.0, 1.0) } else { 0.0 };
            s.effort += (pressure - s.effort) * (1.0 - (-6.0 * dt).exp());
            s.wave += dt * (WAVE_RATE + (THRASH_RATE - WAVE_RATE) * s.effort);
            for k in 1..SEGS {
                // Plates to the nearest anchor (or the neck).
                let m = (0..SEGS).filter(|&j| anchored[j]).map(|j| (k as f32 - j as f32).abs()).fold(k as f32, f32::min);
                let free = ((m - 0.5) / 5.0).clamp(0.0, 1.0);
                let goal = if anchored[k] { 0.0 } else { free * free * (3.0 - 2.0 * free) * calm };
                s.wig[k] += (goal - s.wig[k]) * (1.0 - (-5.0 * dt).exp());
                let mut amp = WIGGLE * (1.0 + 0.5 * k as f32 / SEGS as f32) * s.wig[k];
                if k > hole {
                    let x = (k - hole) as f32 / tail_out as f32;
                    amp += THRASH * x * x * s.effort;
                }
                let dir = (base[k - 1] - base[(k + 1).min(SEGS - 1)]).normalize_or(s.heading);
                s.pts[k] = base[k] + dir.perp() * amp * (s.wave - k as f32 * WAVE_K).sin();
            }
        }
        // How fast each plate goes (a jump is not speed).
        let mut seg_speed = [0.0f32; SEGS];
        for k in 0..SEGS {
            let d = s.pts[k].distance(s.prev[k]);
            if d < 250.0 {
                seg_speed[k] = d / dt.max(1e-4);
            }
        }
        s.prev.copy_from_slice(&s.pts);
        for k in 0..SEGS {
            let sk = k as f32 * SPACING;
            let a = ((sk - (fr - 80.0)) / 180.0).clamp(0.0, 1.0);
            let b = ((rr - sk) / 180.0).clamp(0.0, 1.0);
            let mut v = a.min(b);
            // On the back wall (or dead) the rock hides it.
            if terrain.solid_at(s.pts[k]) {
                v = 0.0;
            }
            if dead && k > 0 {
                v = 0.0;
            }
            s.vis[k] = v;
        }

        // Where each plate is drawn: in perspective toward the chameleon by its own depth, but the
        // chain is re-spaced (each link keeps its length, scaled by the plates' size) so plates
        // at different depths don't pull apart or fold over each other.
        {
            let far = |k: usize| 1.0 - FAR_SHRINK * s.seg_depth[k];
            let place = |k: usize| s.vanish + (s.pts[k] - s.vanish) * far(k);
            let mut shown = std::mem::take(&mut s.shown);
            shown[0] = place(0);
            for k in 1..SEGS {
                let link = s.pts[k].distance(s.pts[k - 1]) * 0.5 * (far(k) + far(k - 1));
                let dir = (place(k) - place(k - 1)).try_normalize().unwrap_or_else(|| (s.pts[k] - s.pts[k - 1]).normalize_or(s.heading));
                shown[k] = shown[k - 1] + dir * link;
            }
            s.shown = shown;
        }

        // Where the body goes through a surface, the edges of the hole crumble.
        if front {
            for k in 1..SEGS {
                let (a, b) = (s.pts[k - 1], s.pts[k]);
                let (ra, rb) = (terrain.solid_at(a), terrain.solid_at(b));
                if ra != rb && a.distance(b) < SPACING * 1.5 && s.seg_depth[k] < 0.05 && s.seg_depth[k - 1] < 0.05 && s.rng.chance(dt * 5.0) {
                    let n = if ra { b - a } else { a - b }.normalize_or(Vec2::Y);
                    burst(&mut commands, &assets, &mut s.rng, (a + b) * 0.5, n, 1, 220.0);
                }
            }
        }

        // Its back is ground: the chameleon can walk along the plates that are out in front. They are
        // one-way, like a ledge: jumped through from below or the side, stood on from above, dropped
        // through with down. Ridden into the rock (or scraped off by it), the chameleon is thrown off,
        // stunned, and falls (fall damage counts from up there).
        let walkable = s.depth < 0.5 && !matches!(s.state, State::Dead(_) | State::Dormant);
        for (pe, plate, mut pl, mut ptf) in &mut plates {
            if plate.owner != me {
                continue;
            }
            let k = plate.k;
            let (a, b) = (s.pts[k - 1], s.pts[k]);
            // Along the body, rising to the right.
            let along = if a.x >= b.x { a - b } else { b - a };
            let dir = along.normalize_or(Vec2::X);
            let tilt = dir.to_angle();
            let out = a.distance(b) < SPACING * 1.5 && s.vis[k - 1] > 0.9 && s.vis[k] > 0.9 && s.seg_depth[k - 1] < 0.5 && s.seg_depth[k] < 0.5;
            let up = Vec2::new(-dir.y, dir.x);
            let height = 0.5 * (back_height(k - 1) + back_height(k));
            let at = (a + b) * 0.5 + up * (height - BACK_THICK);
            let solid = walkable && out && tilt.abs() < MAX_TILT;
            // Whoever stands on it travels with it, until the rock gets in the way.
            if p.ground == Some(pe) && pl.solid {
                let old = ptf.translation.truncate();
                let local = Vec2::from_angle(-pl.tilt).rotate(ppos - old);
                let carried = at + Vec2::from_angle(tilt).rotate(local);
                let into_rock = !out && walkable;
                if solid && !terrain.box_hits(carried, player::HALF) {
                    ptf_player.translation = carried.extend(ptf_player.translation.z);
                } else if into_rock || solid {
                    // Going under, or the rock scrapes it off: thrown, with the plate's momentum.
                    p.ground = None;
                    p.grounded = false;
                    p.stun = p.stun.max(THROWN_STUN);
                    p.vel = ((at - old) / dt.max(1e-4)).clamp_length_max(900.0) * 0.6 + Vec2::Y * 180.0;
                    shake.add(0.3);
                }
            }
            pl.delta = Vec2::ZERO;
            pl.solid = solid;
            pl.tilt = tilt;
            ptf.translation = at.extend(0.0);
            ptf.rotation = Quat::from_rotation_z(tilt);
        }

        // The root is the part of it the chameleon can reach: the plate it is slashing, else the nearest.
        let exposed = |k: usize| s.vis[k] > 0.5 && s.seg_depth[k] < 0.5;
        let striking = anim.striking();
        let mut pick: Option<(usize, f32)> = None;
        for k in 0..SEGS {
            if !exposed(k) {
                continue;
            }
            let d = s.pts[k].distance(ppos);
            let reach = striking && anim.reaches(ppos, p.facing, s.pts[k], SEG_R + 4.0);
            let key = if reach { d - 10_000.0 } else { d };
            if pick.is_none_or(|b| key < b.1) {
                pick = Some((k, key));
            }
        }
        let root = pick.map_or(s.head, |(k, _)| s.pts[k]);
        tf.translation.x = root.x;
        tf.translation.y = root.y;
        boss.guard = pick.is_none() || matches!(s.state, State::Dormant);
        // The bar shows as soon as the fight is on, even before it first shows itself.
        boss.staged = !(s.introduced || boss.awake);

        // Only the mouth and the tips of the tail hurt, each a tight little circle.
        if alive && !matches!(s.state, State::Dead(_) | State::Dormant) {
            let touches = |c: Vec2, r: f32| c.distance(c.clamp(ppos - player::HALF, ppos + player::HALF)) < r;
            let mouth = s.pts[0] + s.heading * MOUTH_REACH;
            let mut hurt_from = (exposed(0) && touches(mouth, MOUTH_R)).then_some(mouth);
            if hurt_from.is_none() && exposed(SEGS - 1) {
                hurt_from = parts
                    .iter()
                    .filter(|(part, _)| part.owner == me && matches!(part.kind, PartKind::TailLeg(_)))
                    .map(|(_, gt)| gt.transform_point(TAIL_TIP.extend(0.0)).truncate())
                    .find(|&tip| touches(tip, TAIL_R));
            }
            if let Some(from) = hurt_from {
                boss::hurt_player(&mut p, ppos, from, CONTACT_DAMAGE, &mut bleed, (&mut shake, &mut stop));
            }
        }

        // The camera pulls back for the fight and keeps to the lair.
        if boss.awake && !dead && s.domain.contains(ppos) {
            view.want = Some(ArenaWant { zoom: ARENA_ZOOM, rect: s.domain, show: s.head });
        }

        if let Some(mut sense) = sense {
            sense.goal = match s.state {
                State::Approach { at, .. } => Some(at),
                State::Prowl(_) => Some(s.center),
                _ => Some(ppos),
            };
            sense.target = Some(s.head);
            sense.sees_player = sees(s.head);
            sense.path.clear();
        }
    }
}

/// Height of the top of plate `k` above its middle line.
fn back_height(k: usize) -> f32 {
    BACK_H * taper(k)
}

/// How big plate `k` is next to the others (it narrows toward the tail).
fn taper(k: usize) -> f32 {
    if k == 0 {
        1.1
    } else {
        1.0 - 0.45 * (k as f32 / SEGS as f32).powf(1.6)
    }
}

/// Removes the walkable plates of a centipede that is gone.
fn sweep_plates(mut commands: Commands, plates: Query<(Entity, &Plate)>, bosses: Query<(), With<Centipede>>) {
    for (e, plate) in &plates {
        if !bosses.contains(plate.owner) {
            commands.entity(e).despawn();
        }
    }
}

/// Cracks in the surface at `at` (normal `n`): `size` (3 or so is the big one it comes out of),
/// gone after `life` seconds, at most `strength` opaque.
fn crack(commands: &mut Commands, materials: &mut Assets<ColorMaterial>, assets: &Assets3, at: Vec2, n: Vec2, size: f32, life: f32, strength: f32) {
    let mat = materials.add(ColorMaterial { color: CRACK.with_alpha(0.0), alpha_mode: AlphaMode2d::Blend, ..default() });
    commands.spawn((
        Crack { age: 0.0, life, size, strength, mat: mat.clone() },
        Mesh2d(assets.crack.clone()),
        MeshMaterial2d(mat),
        Transform::from_translation((at + n * 2.0).extend(11.0)).with_rotation(Quat::from_rotation_z(n.to_angle() - FRAC_PI_2)),
        BossStuff,
    ));
}

/// Cracks, and rock flying about.
fn effects(
    time: Res<Time>,
    mut commands: Commands,
    mut materials: ResMut<Assets<ColorMaterial>>,
    mut chunks: Query<(Entity, &mut Chunk, &mut Transform), Without<Crack>>,
    mut cracks: Query<(Entity, &mut Crack, &mut Transform), Without<Chunk>>,
) {
    let dt = time.delta_secs().min(1.0 / 30.0);
    for (e, mut c, mut tf) in &mut chunks {
        c.vel.y -= 1800.0 * dt;
        c.life -= dt;
        tf.translation.x += c.vel.x * dt;
        tf.translation.y += c.vel.y * dt;
        tf.rotate_z(c.spin * dt);
        tf.scale = Vec3::splat(tf.scale.x.min(4.8) * (c.life / c.total).min(1.0).max(0.01).powf(0.1));
        if c.life <= 0.0 {
            commands.entity(e).despawn();
        }
    }
    for (e, mut c, mut tf) in &mut cracks {
        c.age += dt;
        let alpha = c.strength * (c.age / 0.35).min(1.0) * (1.0 - ((c.age - (c.life - 0.25)) / 0.25).clamp(0.0, 1.0)) * (0.65 + 0.35 * (c.age * 22.0).sin());
        tf.scale = Vec3::splat(c.size * (0.55 + 0.45 * (c.age / 0.9).min(1.0)));
        if let Some(mut m) = materials.get_mut(&c.mat) {
            m.color = CRACK.with_alpha(alpha.clamp(0.0, 1.0));
        }
        if c.age >= c.life {
            commands.entity(e).despawn();
        }
    }
}

/// React to a hit (health and blood are handled in `boss.rs`).
fn on_hit(
    mut commands: Commands,
    mut hits: MessageReader<BossHit>,
    mut bosses: Query<(&mut Centipede, &Transform)>,
    legs: Query<(Entity, &Leg, &GlobalTransform)>,
) {
    for hit in hits.read() {
        let Ok((mut s, tf)) = bosses.get_mut(hit.boss) else { continue };
        if hit.killed {
            s.dead_at = tf.translation.truncate();
            // Goes limp where it is.
            let d = hit.dir;
            for k in 0..SEGS {
                let v = Vec2::new(d.x * 400.0 + s.rng.range(-150.0, 150.0), 500.0 + s.rng.range(0.0, 450.0));
                s.dead_vel[k] = v;
            }
            s.state = State::Dead(0.0);
            continue;
        }
        if hit.retaliate {
            s.react = React::Answer;
        }
        // Every blow knocks a leg off, the one nearest where it landed: the legs left show how hurt
        // it is, and the ones on the ground are a bite to eat.
        let at = tf.translation.truncate();
        let nearest = legs
            .iter()
            .filter(|(_, leg, _)| leg.owner == hit.boss && s.vis[leg.k] > 0.5)
            .min_by(|a, b| a.2.translation().truncate().distance(at).total_cmp(&b.2.translation().truncate().distance(at)));
        if let Some((e, _, gt)) = nearest {
            let vel = Vec2::new(hit.dir.x * 350.0 + s.rng.range(-120.0, 120.0), 420.0 + s.rng.range(0.0, 200.0));
            commands.entity(e).remove::<Leg>().insert(Morsel { food: LEG_FOOD, fluid: Fluid::Purple });
            boss::tear_off(&mut commands, e, gt, vel, s.rng.range(-9.0, 9.0), Some(Fluid::Purple));
        }
    }
}

/// The chameleon came back: start over (`boss.rs` respawns it asleep).
fn reset_on_respawn(mut commands: Commands, mut respawned: MessageReader<Respawned>, stuff: Query<Entity, Or<(With<Chunk>, With<Crack>)>>) {
    if respawned.read().count() == 0 {
        return;
    }
    for e in &stuff {
        commands.entity(e).despawn();
    }
}

/// Tear a few legs off a freshly killed centipede.
fn dismember(mut commands: Commands, mut bosses: Query<&mut Centipede>, legs: Query<(Entity, &Leg, &GlobalTransform)>) {
    let mut done = Vec::new();
    for (e, leg, gt) in &legs {
        let Ok(s) = bosses.get(leg.owner) else { continue };
        if !s.dismember {
            continue;
        }
        done.push(leg.owner);
        let (vel, spin) = match (leg.k, leg.side > 0.0) {
            (2, true) => (Vec2::new(200.0, 560.0), 9.0),
            (7, false) => (Vec2::new(-220.0, 600.0), -8.0),
            (12, true) => (Vec2::new(160.0, 520.0), 7.0),
            (17, false) => (Vec2::new(-170.0, 540.0), -9.0),
            _ => continue,
        };
        commands.entity(e).remove::<Leg>();
        boss::tear_off(&mut commands, e, gt, vel, spin, Some(Fluid::Purple));
    }
    for owner in done {
        if let Ok(mut s) = bosses.get_mut(owner) {
            s.dismember = false;
        }
    }
}

fn animate(
    time: Res<Time>,
    terrain: Res<Terrain>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    bosses: Query<(&Centipede, &Transform)>,
    mut segs: Query<(&Seg, &mut Transform, &mut Visibility), (Without<Centipede>, Without<Leg>, Without<Part>)>,
    mut legs: Query<(&Leg, &mut Transform), (Without<Centipede>, Without<Seg>, Without<Part>)>,
    mut parts: Query<(&Part, &mut Transform), (Without<Centipede>, Without<Seg>, Without<Leg>)>,
) {
    let t = time.elapsed_secs();
    for (c, _) in &bosses {
        for (k, h) in c.shade.iter().enumerate() {
            if let Some(mut m) = materials.get_mut(h) {
                m.color = Color::srgba(0.0, 0.0, 0.0, DIM * c.seg_depth[k]);
            }
        }
    }
    // How small plate `k` is, the farther back it is.
    let far = |c: &Centipede, k: usize| 1.0 - FAR_SHRINK * c.seg_depth[k];
    let seg_dir = |c: &Centipede, k: usize| -> Vec2 {
        if k == 0 {
            c.heading
        } else {
            (c.shown[k - 1] - c.shown[(k + 1).min(SEGS - 1)]).normalize_or(c.heading)
        }
    };
    let seg_scale = |c: &Centipede, k: usize| -> f32 { (BIG * taper(k) * c.vis[k] * far(c, k)).max(0.001) };
    // Perspective: each plate recedes toward the chameleon by its own depth, so what is still in
    // front stays on its real path (`shown` keeps the chain whole).
    let place = |c: &Centipede, k: usize| c.shown[k];
    for (seg, mut tf, mut vis) in &mut segs {
        let Ok((c, root)) = bosses.get(seg.owner) else { continue };
        let k = seg.k;
        *vis = if c.vis[k] > 0.02 { Visibility::Inherited } else { Visibility::Hidden };
        let dir = seg_dir(c, k);
        // From in front of everything to behind the ground, plate by plate; on the wall the plates
        // are squeezed flat (their parts' own offsets with them) so they stay between the two layers.
        let d = c.seg_depth[k].clamp(0.0, 1.0);
        let front_z = root.translation.z - k as f32 * 0.003;
        let z = front_z + (WALL_Z - k as f32 * 0.00008 - front_z) * d - root.translation.z;
        tf.translation = (place(c, k) - root.translation.truncate()).extend(z);
        tf.rotation = Quat::from_rotation_z(dir.to_angle());
        tf.scale = Vec3::new(seg_scale(c, k), seg_scale(c, k), 1.0 - 0.999 * d);
    }
    for (leg, mut tf) in &mut legs {
        let Ok((c, _)) = bosses.get(leg.owner) else { continue };
        let k = leg.k;
        let dir = seg_dir(c, k);
        let swing = (c.phase * 2.0 - k as f32 * 0.55).sin() * 0.38 * leg.side;
        let local = leg.side * FRAC_PI_2 + swing;
        let world = Vec2::from_angle(dir.to_angle() + local);
        let scale = seg_scale(c, k);
        let flat = 14.0 * BIG;
        let len = if matches!(c.state, State::Dead(_)) {
            flat
        } else {
            // Planted on the surface when there is one within reach (not on the back wall).
            let from = c.pts[k];
            let planted = terrain.raycast(from, from + world * 30.0 * BIG).map_or((15.0 + 3.0 * swing.abs()) * BIG, |hit| (hit.distance(from) - 3.0 * BIG).clamp(6.0 * BIG, 30.0 * BIG));
            planted + (flat - planted) * c.seg_depth[k].clamp(0.0, 1.0)
        };
        tf.rotation = Quat::from_rotation_z(local);
        tf.scale = Vec3::new(len / 20.0 / scale, 1.0, 1.0);
    }
    for (part, mut tf) in &mut parts {
        let Ok((c, _)) = bosses.get(part.owner) else { continue };
        let tt = t + c.seed;
        tf.rotation = Quat::from_rotation_z(match part.kind {
            // Jaws gape for the lunge and the show-off.
            PartKind::Fang(side) => -side * (0.12 + 0.6 * c.jaws),
            PartKind::Antenna(side) => side * (0.15 + 0.25 * (tt * 3.0 + side).sin()) + if matches!(c.state, State::Dead(_)) { side * 0.5 } else { 0.0 },
            PartKind::TailLeg(side) => PI + side * (0.28 + 0.12 * (c.phase * 2.0).sin()),
        });
    }
}
