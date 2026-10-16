//! The Burrower: a giant centipede that owns the cave system. It bleeds
//! purple (`#a200a2`), and beating it gives the chameleon that color.
//!
//! Its body is a chain of plates that follows its head along the exact path
//! the head took, so every curve the head draws, the whole body repeats: a
//! long sinuous ribbon. The head crawls hugging whatever surface is nearest
//! (floor, wall, ceiling: it reads the terrain around itself and follows its
//! shell, round corners too), swaying from side to side, and circles the
//! chameleon at a distance, closing in and backing off in turns.
//!
//! Its trick is the rock. It dives into the wall, floor or ceiling it is
//! crawling on and the body goes in after the head, segment by segment,
//! shrinking into the hole where it crossed the surface until nothing is left
//! outside. Underground it travels to a new spot: often right at the
//! chameleon (under its feet, over its head, in front of where it is going),
//! sometimes far away, to show itself and sway there as if taunting it.
//! Where it will come out cracks open and rumbles (the tell, `TELEGRAPH`);
//! then the head bursts out of the surface and the rest of the body is pulled
//! out of the hole behind it.
//!
//! It also walks on the back wall of the cave: dimmer, smaller, behind the
//! chameleon, harmless and untouchable, circling and weaving through the
//! background, until it swings forward out of the wall in a lunge. Closer to
//! the chameleon it rears up and lunges too. Below half health everything
//! gets faster and the pauses shorter. Health, getting hit and the death
//! spectacle live in `boss.rs`.

use std::{
    collections::VecDeque,
    f32::consts::{FRAC_PI_2, PI, TAU},
};

use bevy::{prelude::*, sprite_render::AlphaMode2d};

use crate::{
    boss::{self, Arena, Boss, BossHit, BossSpec, BossStuff},
    camera::{HitStop, Shake},
    chameleon::ChameleonAnim,
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
    hp: 48,
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
/// How far the sway takes it off its line.
const SWAY: f32 = 5.0 * BIG;

const CRAWL: f32 = 380.0;
/// How fast it can change the direction it crawls in (radians per second): no sharp turns.
const TURN: f32 = 1.0;
/// Seconds of circling without getting a chance to strike before it changes plans.
const PATIENCE: f32 = 6.0;
/// Circling the chameleon: the radius far away and close in.
const FAR: f32 = 340.0 * BIG * 0.4;
const NEAR: f32 = 110.0 * BIG * 0.5;
/// Closing in: how close before it rears up.
const STRIKE_RANGE: f32 = 900.0;
const WINDUP: f32 = 0.9;
const LUNGE_SPEED: f32 = 1500.0;
const LUNGE_TIME: f32 = 0.7;
const DIVE_SPEED: f32 = 900.0;
const EMERGE_SPEED: f32 = 1600.0;
/// How far the head shoots out of the hole.
const EMERGE_LEN: f32 = 900.0;
/// The crack opens this long before it comes out.
const TELEGRAPH: f32 = 0.95;
const BACK_SPEED: f32 = 480.0;
const CONTACT_DAMAGE: i32 = 1;
/// The first plates (head and neck) are the ones that bite.
const BITING: usize = 3;
/// Moving less than this over `STUCK_TIME` means it is stuck.
const STUCK_MOVE: f32 = 120.0;
const STUCK_TIME: f32 = 2.0;
/// Digging through the rock: cruising speed and how fast it can turn (radians per second).
const TUNNEL_SPEED: f32 = 900.0;
const TUNNEL_TURN: f32 = 1.3;
/// How deep it goes in before it starts steering through the rock.
const DIG_IN: f32 = 250.0;
/// Distance between the points of the trail the body follows, and how many it keeps.
const STEP: f32 = 24.0;
const TRAIL: usize = (LENGTH / STEP) as usize + 40;
/// Height of the walkable back above the middle line of a plate (at full size), and the
/// thickness of the platform that stands for it.
const BACK_H: f32 = 12.0 * BIG;
const BACK_THICK: f32 = 10.0;
/// Steeper than this (radians) the back is a wall, not something to walk on.
const MAX_TILT: f32 = 1.0;

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
    /// Unit direction the head is going (it faces where it goes).
    heading: Vec2,
    /// Smoothed normal of the surface it hugs.
    n: Vec2,
    /// Which way around the surface it goes (±1), and the time before it may turn round.
    sign: f32,
    sign_cd: f32,
    /// Advances with the distance traveled: drives the sway and the legs.
    phase: f32,
    /// Where the head has been, newest first, a point every `STEP` units.
    trail: VecDeque<Vec2>,
    pts: Vec<Vec2>,
    /// 0..1 how much of each plate is out (it shrinks into holes).
    vis: Vec<f32>,
    /// The parts of the body farther than `hide_rear` from the head, and nearer
    /// than `hide_front`, are underground.
    hide_front: f32,
    hide_rear: f32,
    /// 0 = on the front of the world, 1 = on the back wall.
    depth: f32,
    seed: f32,
    rng: Rng,
    /// Where it may roam (its lair).
    domain: Rect,
    act_cd: f32,
    orbit_dir: f32,
    /// 0..1: closing in on the chameleon.
    engage: f32,
    /// Seconds left of showing off, head raised.
    taunt: f32,
    /// Where it was and when, to know if it's stuck.
    anchor: (Vec2, f32),
    /// Seconds spent far from the chameleon.
    far_t: f32,
    dive_dir: Vec2,
    buried: bool,
    d_in: f32,
    lunge_dir: Vec2,
    /// A combo: dive right after the lunge.
    force_dive: bool,
    introduced: bool,
    react: React,
    dismember: bool,
    dead_vel: Vec<Vec2>,
    shade: Handle<ColorMaterial>,
    /// 0..1 how open the jaws are, for the animation.
    jaws: f32,
    /// Seconds left of wandering off, ignoring the chameleon, and where to.
    wander: f32,
    wander_goal: Vec2,
    /// Where it was struck dead: the head goes there and stays as the carcass.
    dead_at: Vec2,
    /// The dead head: 0 flying to where it was struck, 1 falling, 2 at rest.
    dead_phase: u8,
    /// Direction it crawls along the surface (turns slowly).
    tdir: Vec2,
    /// Where it has been lately (a point every 200 units): it avoids going over the same ground.
    visited: VecDeque<Vec2>,
    /// Seconds spent circling without striking.
    frus: f32,
    /// Velocity of the head (it has inertia), and its speed while digging.
    vel: Vec2,
    speed: f32,
    /// Distance dug through solid rock in this tunnel.
    dug: f32,
}

#[derive(Clone, Copy, PartialEq)]
enum React {
    None,
    /// Hit while closing in: get away into the rock.
    Evade,
    /// A flurry of hits: hit back.
    Answer,
}

#[derive(Clone, Copy, PartialEq)]
enum State {
    /// Asleep underground.
    Dormant,
    /// Circling the chameleon along the walls.
    Crawl,
    /// Closing in for a lunge (seconds left).
    Stalk(f32),
    /// Rearing up to lunge (from the back wall: swinging forward).
    Windup { t: f32, back: bool },
    Lunge(f32),
    /// Going into the rock (seconds so far).
    Dive(f32),
    /// Digging through the rock toward `exit` (or back home), for real: the head is somewhere.
    /// The crack opens once `locked`.
    Tunnel { t: f32, exit: Vec2, normal: Vec2, far: bool, intro: bool, locked: bool, home: bool },
    /// Out of the hole, `d` units so far.
    Emerge { d: f32, normal: Vec2, far: bool, intro: bool },
    /// Walking on the back wall (seconds left).
    Back(f32),
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
    let shade = materials.add(ColorMaterial { color: Color::srgba(0.0, 0.0, 0.0, 0.0), alpha_mode: AlphaMode2d::Blend, ..default() });
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
        b.commands.spawn((Shade, Mesh2d(sh), MeshMaterial2d(shade.clone()), Transform::from_xyz(0.0, 0.0, 0.05), ChildOf(seg)));
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
            n: Vec2::Y,
            sign: 1.0,
            sign_cd: 0.0,
            phase: 0.0,
            trail: (0..TRAIL).map(|i| head - Vec2::X * STEP * i as f32).collect(),
            pts: vec![head; SEGS],
            vis: vec![0.0; SEGS],
            hide_front: f32::INFINITY,
            hide_rear: f32::INFINITY,
            depth: 0.0,
            seed: rng.range(0.0, 100.0),
            domain,
            act_cd: 2.0,
            orbit_dir: 1.0,
            engage: 0.0,
            taunt: 0.0,
            anchor: (head, 0.0),
            far_t: 0.0,
            dive_dir: Vec2::NEG_Y,
            buried: false,
            d_in: 0.0,
            lunge_dir: Vec2::X,
            force_dive: false,
            introduced: false,
            react: React::None,
            dismember: false,
            dead_vel: vec![Vec2::ZERO; SEGS],
            shade,
            jaws: 0.0,
            wander: 0.0,
            wander_goal: head,
            dead_at: head,
            dead_phase: 0,
            tdir: Vec2::X,
            visited: VecDeque::new(),
            frus: 0.0,
            vel: Vec2::ZERO,
            speed: 0.0,
            dug: 0.0,
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

/// The way out of the rock from `p`.
fn escape(terrain: &Terrain, p: Vec2) -> Vec2 {
    for step in 1..60 {
        for i in 0..16 {
            let d = Vec2::from_angle(i as f32 / 16.0 * TAU);
            if !terrain.solid_at(p + d * step as f32 * 12.0) {
                return d;
            }
        }
    }
    Vec2::Y
}

/// A surface point to come out of: (point just inside the rock, normal), picked around the
/// chameleon (`far`: far from it instead). Near, it likes the spots that are inconvenient: the
/// floor under its feet, the ceiling over its head, the wall where it is heading.
fn pick_exit(terrain: &Terrain, rng: &mut Rng, domain: Rect, ppos: Vec2, pvel: Vec2, grounded: bool, far: bool, avoid: Vec2, from: Vec2) -> Option<(Vec2, Vec2)> {
    let mut best: Option<(f32, Vec2, Vec2)> = None;
    for _ in 0..48 {
        let dir = Vec2::from_angle(rng.range(0.0, TAU));
        let reach = if far { rng.range(900.0, 1500.0) } else { rng.range(150.0, 600.0) };
        let Some(hit) = terrain.raycast(ppos, ppos + dir * reach) else { continue };
        if !domain.contains(hit) || hit.distance(ppos) < 60.0 {
            continue;
        }
        let Some((_, n)) = probe(terrain, hit - dir * 5.0, 48.0) else { continue };
        // Room to come out into.
        if [100.0, 250.0, 450.0].iter().any(|&k| terrain.solid_at(hit + n * k)) {
            continue;
        }
        let mut score = rng.range(0.0, 1.0);
        if !far {
            if grounded && n.y > 0.7 && hit.y < ppos.y && (hit.x - (ppos.x + pvel.x * 0.5)).abs() < 300.0 {
                score += 2.0;
            }
            if n.y < -0.7 && (hit.x - ppos.x).abs() < 300.0 {
                score += 1.4;
            }
            if pvel.length() > 40.0 && dir.dot(pvel.normalize()) > 0.7 {
                score += 1.0;
            }
        }
        if hit.distance(avoid) < 500.0 {
            score -= 1.0;
        }
        if !far {
            // Sneaks round: likes coming out on the other side of the chameleon from where it is,
            // and not too far to dig.
            let (a, b) = ((hit - ppos).normalize_or_zero(), (from - ppos).normalize_or_zero());
            score += 1.6 * 0.5 * (1.0 - a.dot(b)) - hit.distance(from) / 4000.0;
        }
        if best.is_none_or(|b| score > b.0) {
            best = Some((score, hit, n));
        }
    }
    best.map(|b| (b.1, b.2))
}

/// One step of crawling along the nearest surface: along it toward `goal` at `speed`, `offset`
/// above it (plus the sway). Round corners and into crevices the shell of the rock leads it.
fn crawl(s: &mut Centipede, terrain: &Terrain, goal: Vec2, speed: f32, offset: f32, dt: f32) {
    let pos = s.head;
    let vel;
    if terrain.solid_at(pos) {
        // The sway pushed it into the rock: out the nearest way.
        vel = escape(terrain, pos) * 900.0;
    } else if let Some((d, n)) = probe(terrain, pos, SKIN + 400.0) {
        s.n = s.n.lerp(n, 1.0 - (-6.0 * dt).exp()).normalize_or(n);
        let n = s.n;
        let perp = n.perp();
        let to_goal = (goal - pos).normalize_or_zero();
        // Leans away from the ground it has already covered.
        let score = |sign: f32| (perp * sign).dot(to_goal) + 0.3 * (perp * sign).dot(s.heading) - 0.45 * crowd(&s.visited, pos + perp * sign * 350.0);
        s.sign_cd -= dt;
        if score(-s.sign) > score(s.sign) + 0.8 && s.sign_cd <= 0.0 {
            s.sign = -s.sign;
            s.sign_cd = 2.5;
        }
        // The way along the surface turns slowly (a U-turn is a wide arc out from the surface),
        // and it slows down to do it.
        let want = perp * s.sign;
        let diff = s.tdir.angle_to(want);
        let turn = if diff.abs() > 2.6 { s.tdir.perp_dot(n).signum() * TURN * dt } else { diff.clamp(-TURN * dt, TURN * dt) };
        s.tdir = Vec2::from_angle(s.tdir.to_angle() + turn);
        let slow = 1.0 - 0.55 * (diff.abs() / PI);
        let along = speed * slow * (1.0 + 0.08 * (s.phase * 2.0).sin());
        let sway = s.phase.sin() * SWAY;
        let radial = ((offset + sway - d) * 3.0).clamp(-900.0, 900.0);
        vel = s.tdir * along + n * radial;
        s.phase += along * dt * 0.0045;
    } else if let Some((_, n)) = probe(terrain, pos, 1400.0) {
        // In the open: reaches for the nearest surface.
        vel = -n * speed.max(500.0) * 1.15 + s.heading * speed * 0.25;
    } else {
        vel = s.heading * speed - Vec2::Y * 500.0;
    }
    // Inertia: it steers by changing its velocity, not its position (out of the rock it is quick).
    let vel = if terrain.solid_at(pos) { vel } else { s.vel.lerp(vel, 1.0 - (-3.0 * dt).exp()) };
    s.vel = vel;
    s.head += vel * dt;
    if speed > 1.0 && vel.length() > 20.0 {
        let to = vel.normalize();
        s.heading = s.heading.lerp(to, 1.0 - (-4.0 * dt).exp()).normalize_or(to);
    }
    for _ in 0..10 {
        if !terrain.solid_at(s.head) {
            break;
        }
        s.head += s.n * 20.0;
    }
}

/// Moves the head toward `dir` at `speed` the way a heavy body does: the direction of its velocity
/// turns at most `turn` radians per second, and the speed changes by at most `accel` per second.
/// A new intention never changes where it is going at once.
fn steer(s: &mut Centipede, dir: Vec2, speed: f32, turn: f32, accel: f32, dt: f32) {
    let v = s.vel;
    let cur = if v.length() > 80.0 { v.normalize() } else { dir };
    let now = turn_toward(cur, dir, turn * dt);
    let sp = v.length() + (speed - v.length()).clamp(-accel * dt, accel * dt);
    s.vel = now * sp;
    s.heading = now;
    s.head += s.vel * dt;
}

/// Speed-up for getting back to the chameleon when far from it.
fn catch_up(dist: f32) -> f32 {
    1.0 + ((dist - 1200.0) / 1200.0).clamp(0.0, 2.0)
}

/// How much of the ground around `q` it has already covered.
fn crowd(visited: &VecDeque<Vec2>, q: Vec2) -> f32 {
    visited.iter().map(|v| (1.0 - v.distance(q) / 350.0).max(0.0)).sum()
}

/// Turn the unit vector `from` toward `to` by at most `max` radians.
fn turn_toward(from: Vec2, to: Vec2, max: f32) -> Vec2 {
    Vec2::from_angle(from.to_angle() + from.angle_to(to).clamp(-max, max))
}

fn push_trail(trail: &mut VecDeque<Vec2>, head: Vec2) {
    let Some(&last) = trail.front() else {
        trail.push_front(head);
        return;
    };
    let d = head - last;
    let l = d.length();
    if l < STEP {
        return;
    }
    for i in 1..=(l / STEP) as usize {
        trail.push_front(last + d * (i as f32 * STEP / l));
    }
    while trail.len() > TRAIL + 30 {
        trail.pop_back();
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
    terrain: Res<Terrain>,
) {
    let dt = time.delta_secs().min(1.0 / 30.0);
    let t = time.elapsed_secs();
    let (mut p, mut ptf_player, anim) = player.into_inner();
    let ppos = ptf_player.translation.truncate();

    for (me, mut s, mut boss, mut tf, sense) in &mut bosses {
        let s = &mut *s;
        let alive = p.dead.is_none();
        let hunting = alive && boss.awake;
        let enraged = boss.enraged();
        let rage = if enraged { 1.3 } else { 1.0 };
        let mut before = s.head;
        let sees = |from: Vec2| terrain.raycast(from, ppos).is_none();

        // Hit: get away into the rock, or answer.
        match std::mem::replace(&mut s.react, React::None) {
            React::Evade if matches!(s.state, State::Crawl | State::Stalk(_)) => begin_dive(s),
            React::Answer if matches!(s.state, State::Crawl | State::Stalk(_)) => {
                if s.head.distance(ppos) < 240.0 && sees(s.head) {
                    s.state = State::Windup { t: WINDUP * 0.6, back: false };
                } else {
                    begin_dive(s);
                }
            }
            _ => {}
        }

        let mut depth_goal = 0.0;
        s.jaws += (0.0 - s.jaws) * (1.0 - (-8.0 * dt).exp());
        s.state = match s.state {
            State::Dormant => {
                if hunting {
                    // It wakes under your feet, as it means to go on: cracks open near you.
                    let exit = pick_exit(&terrain, &mut s.rng, s.domain, ppos, p.vel, p.grounded, false, s.head, s.head);
                    match exit {
                        Some((exit, normal)) => {
                            s.head = boss.home;
                            s.heading = (exit - s.head).normalize_or(Vec2::Y);
                            s.trail = (0..TRAIL).map(|_| s.head).collect();
                            s.buried = true;
                            s.d_in = 0.0;
                            s.dug = 0.0;
                            s.speed = 0.0;
                            State::Tunnel { t: 0.0, exit, normal, far: false, intro: true, locked: false, home: false }
                        }
                        None => State::Dormant,
                    }
                } else {
                    State::Dormant
                }
            }

            // Nobody to hunt: back to the burrow, along the walls, and in.
            State::Crawl if !boss.awake => {
                s.wander = 0.0;
                s.taunt = 0.0;
                crawl(s, &terrain, boss.home, CRAWL * 0.8, SKIN, dt);
                let mut dive = s.head.distance(boss.home) < 500.0;
                if t - s.anchor.1 > STUCK_TIME {
                    dive |= s.head.distance(s.anchor.0) < STUCK_MOVE;
                    s.anchor = (s.head, t);
                }
                if dive {
                    begin_dive(s);
                    s.state
                } else {
                    State::Crawl
                }
            }

            State::Crawl => {
                s.act_cd -= dt;
                s.engage += (0.0 - s.engage) * (1.0 - (-2.0 * dt).exp());
                let r = FAR + (NEAR - FAR) * s.engage + 100.0 * (t * 0.6 + s.seed).sin();
                let a = (s.head - ppos).to_angle() + s.orbit_dir * 0.6;
                let mut goal = ppos + Vec2::from_angle(a) * r;
                let showing = s.taunt > 0.0;
                if hunting && !showing && s.wander <= 0.0 {
                    s.frus += dt;
                }
                let mut lift = 0.0;
                if showing {
                    // Head raised, swaying: look at me.
                    s.taunt -= dt;
                    lift = 300.0 * (1.0 - ((s.taunt - 1.2).max(0.0) / 0.4)).min(1.0) + 40.0 * (t * 1.5).sin();
                    s.jaws = s.jaws.max(0.5 + 0.4 * (t * 2.5).sin());
                }
                // Wandering off: the chameleon is not worth its attention for now.
                let wandering = s.wander > 0.0;
                if wandering {
                    s.wander -= dt;
                    goal = s.wander_goal;
                    if s.head.distance(goal) < 400.0 {
                        s.wander = 0.0;
                    }
                }
                let hurry = if wandering || showing { 1.0 } else { catch_up(s.head.distance(ppos)) };
                crawl(s, &terrain, goal, CRAWL * rage * hurry * if showing { 0.2 } else if wandering { 0.7 } else { 1.0 }, SKIN + lift, dt);

                // Stuck, or off chasing nothing: it digs through.
                let far = s.head.distance(ppos) > 2400.0 && !wandering;
                s.far_t = if far { s.far_t + dt } else { 0.0 };
                if t - s.anchor.1 > STUCK_TIME {
                    let moved = s.head.distance(s.anchor.0);
                    s.anchor = (s.head, t);
                    if hunting && !showing && moved < STUCK_MOVE {
                        if wandering {
                            s.wander = 0.0;
                        } else {
                            s.act_cd = -1.0;
                            s.force_dive = true;
                        }
                    }
                }
                if hunting && !showing && s.far_t > 5.0 {
                    s.far_t = 0.0;
                    s.force_dive = true;
                }
                if wandering && s.wander <= 0.0 {
                    // Done with its stroll: comes back along the walls, to attack again.
                    s.engage = 1.0;
                    s.act_cd = s.rng.range(3.0, 5.0);
                    State::Stalk(8.0)
                } else if wandering {
                    State::Crawl
                } else if hunting && !showing && s.frus > PATIENCE {
                    // All this circling and no opening: the long way round, or under.
                    if begin_detour(s, &terrain, ppos) {
                        State::Crawl
                    } else {
                        begin_dive(s);
                        s.state
                    }
                } else if hunting && !showing && (s.force_dive || s.act_cd <= 0.0) {
                    let roll = s.rng.next();
                    s.act_cd = s.rng.range(4.0, 7.0) / rage;
                    if s.force_dive {
                        s.force_dive = false;
                        begin_dive(s);
                        s.state
                    } else if roll < 0.30 {
                        State::Stalk(4.0)
                    } else if roll < 0.48 && begin_wander(s, &terrain, ppos) {
                        State::Crawl
                    } else if roll < 0.74 + if enraged { 0.1 } else { 0.0 } {
                        begin_dive(s);
                        s.state
                    } else if roll < 0.92 {
                        State::Back(s.rng.range(5.0, 8.0))
                    } else {
                        s.orbit_dir = -s.orbit_dir;
                        s.act_cd = 1.2;
                        State::Crawl
                    }
                } else {
                    State::Crawl
                }
            }

            State::Stalk(left) => {
                s.engage += (1.0 - s.engage) * (1.0 - (-3.0 * dt).exp());
                let a = (s.head - ppos).to_angle() + s.orbit_dir * 0.5;
                let goal = ppos + Vec2::from_angle(a) * NEAR;
                s.frus += dt;
                crawl(s, &terrain, goal, CRAWL * 1.2 * rage * catch_up(s.head.distance(ppos)), SKIN, dt);
                if hunting && s.head.distance(ppos) < STRIKE_RANGE && sees(s.head) {
                    State::Windup { t: 0.0, back: false }
                } else if left - dt <= 0.0 || !hunting {
                    s.act_cd = s.rng.range(2.0, 3.5);
                    State::Crawl
                } else {
                    State::Stalk(left - dt)
                }
            }

            State::Windup { t: w, back } => {
                let to = (ppos - s.head).normalize_or(s.heading);
                s.heading = turn_toward(s.heading, to, 4.0 * dt);
                let dur = if back { 1.2 } else { WINDUP / rage };
                let k = (w / dur).min(1.0);
                s.jaws = s.jaws.max(k);
                s.frus = 0.0;
                if back {
                    // Swinging forward out of the wall.
                    depth_goal = 0.0;
                } else {
                    crawl(s, &terrain, ppos, 0.0, SKIN + 400.0 * k, dt);
                }
                if w + dt >= dur {
                    s.lunge_dir = (ppos + p.vel * 0.12 - s.head).normalize_or(s.heading);
                    shake.add(0.3);
                    State::Lunge(LUNGE_TIME)
                } else {
                    State::Windup { t: w + dt, back }
                }
            }

            State::Lunge(left) => {
                let dir = s.lunge_dir;
                s.jaws = 1.0;
                let hit_rock = terrain.solid_at(s.head + dir * 60.0);
                if !hit_rock {
                    // Accelerates into it.
                    steer(s, dir, LUNGE_SPEED * rage, 4.0, 3500.0, dt);
                }
                if hit_rock || left - dt <= 0.0 {
                    // Pulls back: disengages, sometimes straight into the rock.
                    s.engage = 0.0;
                    s.act_cd = s.rng.range(2.0, 3.5);
                    if s.rng.chance(if enraged { 0.6 } else { 0.35 }) {
                        s.force_dive = true;
                    }
                    if s.rng.chance(0.5) {
                        s.orbit_dir = -s.orbit_dir;
                    }
                    State::Crawl
                } else {
                    State::Lunge(left - dt)
                }
            }

            State::Dive(tm) => {
                if !s.buried {
                    // Keeps aiming at the nearest rock; it turns toward it, it doesn't snap.
                    if let Some((_, n)) = probe(&terrain, s.head, 700.0) {
                        s.dive_dir = (-n * 0.85 + s.heading * 0.45).normalize_or(-n);
                    }
                }
                let dir = s.dive_dir;
                steer(s, dir, DIVE_SPEED * rage, 1.8, 900.0, dt);
                let step = s.head.distance(before);
                if !s.buried && terrain.solid_at(s.head) {
                    // Through the surface: the body goes in after it.
                    s.buried = true;
                    s.d_in = 0.0;
                    burst(&mut commands, &assets, &mut s.rng, s.head, s.n, 18, 800.0);
                    shake.add(0.25);
                }
                if s.buried {
                    s.d_in += step;
                    s.hide_front = s.d_in;
                }
                if s.buried && s.d_in >= DIG_IN {
                    // Deep enough: from here it steers through the rock. Near the chameleon (round the
                    // flank), or far away to taunt it; or home, when there is nobody to hunt.
                    s.dug = s.d_in;
                    s.speed = DIVE_SPEED * rage;
                    if !boss.awake {
                        State::Tunnel { t: 0.0, exit: boss.home, normal: Vec2::Y, far: false, intro: false, locked: false, home: true }
                    } else {
                        let far = s.rng.chance(0.35);
                        let exit = pick_exit(&terrain, &mut s.rng, s.domain, ppos, p.vel, p.grounded, far, s.head, s.head)
                            .or_else(|| pick_exit(&terrain, &mut s.rng, s.domain, ppos, p.vel, p.grounded, !far, s.head, s.head));
                        match exit {
                            Some((exit, normal)) => State::Tunnel { t: 0.0, exit, normal, far, intro: false, locked: false, home: false },
                            None => {
                                // Nowhere to come out: back where it went in.
                                s.hide_front = f32::NEG_INFINITY;
                                s.buried = false;
                                State::Crawl
                            }
                        }
                    }
                } else if !s.buried && tm > 2.5 {
                    State::Crawl
                } else {
                    State::Dive(tm + dt)
                }
            }

            State::Tunnel { t: tt, mut exit, mut normal, far, intro, mut locked, home } => {
                let tt = tt + dt;
                let cruise = TUNNEL_SPEED * rage;
                s.speed += (cruise - s.speed) * (1.0 - (-1.5 * dt).exp());
                // Until the crack opens it follows the chameleon underground.
                if !locked && !home && hunting && !intro && (tt * 2.2).fract() < (dt * 2.2) {
                    if let Some((e, n)) = pick_exit(&terrain, &mut s.rng, s.domain, ppos, p.vel, p.grounded, far, exit, s.head) {
                        (exit, normal) = (e, n);
                    }
                }
                let dist = s.head.distance(exit);
                if !locked && !home && dist < cruise * TELEGRAPH {
                    locked = true;
                    let mat = materials.add(ColorMaterial { color: CRACK.with_alpha(0.0), alpha_mode: AlphaMode2d::Blend, ..default() });
                    commands.spawn((
                        Crack { age: 0.0, life: TELEGRAPH + 0.25, mat: mat.clone() },
                        Mesh2d(assets.crack.clone()),
                        MeshMaterial2d(mat),
                        Transform::from_translation((exit + normal * 2.0).extend(11.0)).with_rotation(Quat::from_rotation_z(normal.to_angle() - FRAC_PI_2)),
                        BossStuff,
                    ));
                }
                if locked {
                    let k = (1.0 - dist / (cruise * TELEGRAPH)).clamp(0.0, 1.0);
                    shake.add(0.5 * dt * (0.4 + k));
                    if s.rng.chance(dt * (8.0 + 30.0 * k)) {
                        burst(&mut commands, &assets, &mut s.rng, exit, normal, 1, 300.0 + 250.0 * k);
                    }
                }
                // Steers for the point below the exit, then up through it, with some inertia.
                let aim = if !home && dist > 600.0 { exit - normal * 350.0 } else { exit };
                let want = (aim - s.head).normalize_or(s.heading);
                s.heading = turn_toward(s.heading, want, TUNNEL_TURN * dt);
                s.head += s.heading * s.speed * dt;
                let step = s.head.distance(before);
                s.d_in += step;
                if terrain.solid_at(s.head) {
                    s.dug += step;
                }
                // The rock itself hides what is in it (see the visibility below).
                s.hide_front = f32::NEG_INFINITY;
                s.hide_rear = if intro && s.d_in < LENGTH + 3.0 * SPACING { s.d_in } else { f32::INFINITY };

                if home && dist < 120.0 {
                    // Home: asleep in the rock again.
                    s.head = boss.home;
                    s.hide_front = f32::INFINITY;
                    s.hide_rear = f32::INFINITY;
                    s.buried = false;
                    State::Dormant
                } else {
                    // Out: where the crack was, or through the far side when the rock is too thin.
                    let thin = !home && !locked && s.dug > 250.0 && dist > 400.0 && !terrain.solid_at(s.head + s.heading * 60.0);
                    let out = if !home && dist < 100.0 { Some((exit, normal)) } else if thin { Some((s.head, s.heading)) } else { None };
                    match out {
                        Some((at, normal)) => {
                            s.head = at - normal * 24.0;
                            before = s.head;
                            s.heading = normal;
                            s.hide_front = f32::NEG_INFINITY;
                            s.hide_rear = if intro { s.d_in } else { f32::INFINITY };
                            s.buried = false;
                            burst(&mut commands, &assets, &mut s.rng, at, normal, 40, 1300.0);
                            shake.add(0.7);
                            stop.add(0.05);
                            // Whatever is right over the hole gets thrown.
                            let d = ppos - at;
                            if alive && d.length() < 260.0 && d.dot(normal) > -60.0 {
                                boss::hurt_player(&mut p, ppos, at, CONTACT_DAMAGE, &mut bleed, (&mut shake, &mut stop));
                            }
                            s.anchor = (s.head, t);
                            State::Emerge { d: 0.0, normal, far, intro }
                        }
                        None => State::Tunnel { t: tt, exit, normal, far, intro, locked, home },
                    }
                }
            }

            State::Emerge { d, normal, far, intro } => {
                let prog = (d / EMERGE_LEN).min(1.0);
                let speed = (EMERGE_SPEED + (300.0 - EMERGE_SPEED) * prog * prog) * rage;
                let to = (ppos - s.head).normalize_or(normal);
                let dir = if to.dot(normal) > 0.0 { normal.lerp(to, 0.35 * prog).normalize_or(normal) } else { normal };
                steer(s, dir, speed, 2.5, 3000.0, dt);
                s.jaws = s.jaws.max(0.8);
                let d = d + speed * dt;
                if d >= EMERGE_LEN {
                    if intro {
                        s.introduced = true;
                        shake.add(0.4);
                    }
                    if far {
                        s.taunt = 1.8;
                    } else {
                        s.engage = if enraged { 1.0 } else { 0.3 };
                    }
                    s.act_cd = s.rng.range(0.5, 1.4);
                    State::Crawl
                } else {
                    State::Emerge { d, normal, far, intro }
                }
            }

            State::Back(left) => {
                depth_goal = 1.0;
                let r = 900.0 + 250.0 * (t * 0.4 + s.seed).sin();
                let a = (s.head - ppos).to_angle() + s.orbit_dir * 0.6;
                let mut goal = ppos + Vec2::from_angle(a) * r;
                if !s.domain.inflate(-400.0).contains(s.head) {
                    goal = s.domain.center();
                }
                let want = (goal - s.head).normalize_or(s.heading);
                // Weaving through the background: steering plus a slow S.
                let heading = turn_toward(s.heading, want, 0.9 * dt);
                s.heading = Vec2::from_angle(heading.to_angle() + (t * 1.0 + s.seed).sin() * 0.4 * dt);
                s.head += s.heading * BACK_SPEED * rage * dt;
                s.phase += BACK_SPEED * dt * 0.0045;
                let near = s.head.distance(ppos) < 1800.0;
                if !boss.awake {
                    State::Crawl
                } else if !hunting {
                    State::Back(left)
                } else if left - dt <= 0.0 && near {
                    State::Windup { t: 0.0, back: true }
                } else {
                    State::Back((left - dt).max(0.0))
                }
            }

            State::Dead(td) => State::Dead(td + dt),
        };

        // What isn't steered by `crawl` hands its momentum on.
        if !matches!(s.state, State::Crawl | State::Stalk(_) | State::Windup { back: false, .. } | State::Dormant | State::Dead(_)) {
            s.vel = ((s.head - before) / dt).clamp_length_max(2500.0);
            if s.vel.length() > 100.0 {
                s.tdir = s.vel.normalize();
            }
        }
        if !matches!(s.state, State::Dormant | State::Dead(_)) && s.visited.back().is_none_or(|v| v.distance(s.head) > 200.0) {
            let head = s.head;
            s.visited.push_back(head);
            if s.visited.len() > 60 {
                s.visited.pop_front();
            }
        }

        // Which side of the world it is on.
        let back_now = matches!(s.state, State::Back(_) | State::Windup { back: true, .. });
        if back_now {
            depth_goal = if matches!(s.state, State::Windup { .. }) { 0.0 } else { 1.0 };
        }
        s.depth += (depth_goal - s.depth) * (1.0 - (-5.0 * dt).exp());
        if s.depth < 0.01 {
            s.depth = 0.0;
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
            push_trail(&mut s.trail, head);
            let Centipede { pts, trail, .. } = &mut *s;
            body(head, trail, pts);
        }
        let (fr, rr, dead) = (s.hide_front, s.hide_rear, matches!(s.state, State::Dead(_)));
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

        // Its back is ground: the chameleon can walk along the plates that are out in front.
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
            let out = a.distance(b) < SPACING * 1.5 && s.vis[k - 1] > 0.9 && s.vis[k] > 0.9;
            let up = Vec2::new(-dir.y, dir.x);
            let height = 0.5 * (back_height(k - 1) + back_height(k));
            let at = (a + b) * 0.5 + up * (height - BACK_THICK);
            let solid = walkable && out && tilt.abs() < MAX_TILT;
            // Whoever stands on it travels with it, until the rock gets in the way.
            if p.ground == Some(pe) && pl.solid && solid {
                let old = ptf.translation.truncate();
                let local = Vec2::from_angle(-pl.tilt).rotate(ppos - old);
                let carried = at + Vec2::from_angle(tilt).rotate(local);
                if !terrain.box_hits(carried, player::HALF) {
                    ptf_player.translation = carried.extend(ptf_player.translation.z);
                }
            }
            pl.delta = Vec2::ZERO;
            pl.solid = solid;
            pl.tilt = tilt;
            ptf.translation = at.extend(0.0);
            ptf.rotation = Quat::from_rotation_z(tilt);
        }

        // The root is the part of it the chameleon can reach: the plate it is slashing, else the nearest.
        let exposed = |k: usize| s.vis[k] > 0.5 && s.depth < 0.5;
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
        boss.staged = !s.introduced;

        // The head and neck bite whatever they touch.
        let biters = if matches!(s.state, State::Lunge(_) | State::Emerge { .. }) { BITING + 2 } else { BITING };
        if alive && !matches!(s.state, State::Dead(_) | State::Dormant) {
            for k in 0..biters {
                if !exposed(k) {
                    continue;
                }
                let c = s.pts[k].clamp(ppos - player::HALF, ppos + player::HALF);
                if c.distance(s.pts[k]) < SEG_R + 2.0 {
                    boss::hurt_player(&mut p, ppos, s.pts[k], CONTACT_DAMAGE, &mut bleed, (&mut shake, &mut stop));
                    break;
                }
            }
        }

        if let Some(mut sense) = sense {
            sense.goal = match s.state {
                State::Tunnel { exit, .. } => Some(exit),
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

/// Picks a far spot along the rock to stroll off to, ignoring the chameleon for a while.
fn begin_wander(s: &mut Centipede, terrain: &Terrain, ppos: Vec2) -> bool {
    let mut best: Option<(f32, Vec2)> = None;
    for _ in 0..24 {
        let dir = Vec2::from_angle(s.rng.range(0.0, TAU));
        let Some(hit) = terrain.raycast(s.head, s.head + dir * s.rng.range(1500.0, 3000.0)) else { continue };
        if !s.domain.contains(hit) || hit.distance(ppos) < 1200.0 || hit.distance(s.head) < 1000.0 {
            continue;
        }
        let score = s.rng.range(0.0, 1.0) + hit.distance(ppos) / 4000.0 - 0.5 * crowd(&s.visited, hit);
        if best.is_none_or(|b| score > b.0) {
            best = Some((score, hit));
        }
    }
    let Some((_, goal)) = best else { return false };
    s.wander_goal = goal;
    s.wander = s.rng.range(6.0, 10.0);
    s.frus = 0.0;
    true
}

/// Circling gets nowhere: takes the long way round to the other side of the chameleon.
fn begin_detour(s: &mut Centipede, terrain: &Terrain, ppos: Vec2) -> bool {
    let away = (s.head - ppos).normalize_or(Vec2::X);
    let mut best: Option<(f32, Vec2)> = None;
    for _ in 0..32 {
        let dir = Vec2::from_angle(s.rng.range(0.0, TAU));
        if dir.dot(away) > -0.2 {
            continue;
        }
        let Some(hit) = terrain.raycast(ppos, ppos + dir * s.rng.range(700.0, 1600.0)) else { continue };
        if !s.domain.contains(hit) || hit.distance(s.head) < 600.0 {
            continue;
        }
        let score = s.rng.range(0.0, 1.0) - 0.6 * crowd(&s.visited, hit);
        if best.is_none_or(|b| score > b.0) {
            best = Some((score, hit));
        }
    }
    let Some((_, goal)) = best else { return false };
    s.wander_goal = goal;
    s.wander = s.rng.range(7.0, 11.0);
    s.frus = 0.0;
    true
}

/// Removes the walkable plates of a centipede that is gone.
fn sweep_plates(mut commands: Commands, plates: Query<(Entity, &Plate)>, bosses: Query<(), With<Centipede>>) {
    for (e, plate) in &plates {
        if !bosses.contains(plate.owner) {
            commands.entity(e).despawn();
        }
    }
}

/// Starts burrowing into the surface under it.
fn begin_dive(s: &mut Centipede) {
    s.frus = 0.0;
    s.wander = 0.0;
    s.dive_dir = (-s.n * 0.85 + s.heading * 0.45).normalize_or(-s.n);
    s.buried = false;
    s.d_in = 0.0;
    s.state = State::Dive(0.0);
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
        let alpha = (c.age / 0.35).min(1.0) * (1.0 - ((c.age - (c.life - 0.25)) / 0.25).clamp(0.0, 1.0)) * (0.65 + 0.35 * (c.age * 22.0).sin());
        tf.scale = Vec3::splat(3.0 * (0.55 + 0.45 * (c.age / 0.9).min(1.0)));
        if let Some(mut m) = materials.get_mut(&c.mat) {
            m.color = CRACK.with_alpha(alpha.clamp(0.0, 1.0));
        }
        if c.age >= c.life {
            commands.entity(e).despawn();
        }
    }
}

/// React to a hit (health and blood are handled in `boss.rs`).
fn on_hit(mut hits: MessageReader<BossHit>, mut bosses: Query<(&mut Centipede, &Transform)>) {
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
        } else if hit.retaliate {
            s.react = React::Answer;
        } else if hit.stagger && s.rng.chance(0.55) {
            s.react = React::Evade;
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
        if let Some(mut m) = materials.get_mut(&c.shade) {
            m.color = Color::srgba(0.0, 0.0, 0.0, 0.5 * c.depth);
        }
    }
    let seg_dir = |c: &Centipede, k: usize| -> Vec2 {
        if k == 0 {
            c.heading
        } else {
            (c.pts[k - 1] - c.pts[(k + 1).min(SEGS - 1)]).normalize_or(c.heading)
        }
    };
    let seg_scale = |c: &Centipede, k: usize| -> f32 { (BIG * taper(k) * c.vis[k] * (1.0 - 0.22 * c.depth)).max(0.001) };
    for (seg, mut tf, mut vis) in &mut segs {
        let Ok((c, root)) = bosses.get(seg.owner) else { continue };
        let k = seg.k;
        *vis = if c.vis[k] > 0.02 { Visibility::Inherited } else { Visibility::Hidden };
        let dir = seg_dir(c, k);
        let z = -7.0 * c.depth - k as f32 * 0.003;
        tf.translation = (c.pts[k] - root.translation.truncate()).extend(z);
        tf.rotation = Quat::from_rotation_z(dir.to_angle());
        tf.scale = Vec3::new(seg_scale(c, k), seg_scale(c, k), 1.0);
    }
    for (leg, mut tf) in &mut legs {
        let Ok((c, _)) = bosses.get(leg.owner) else { continue };
        let k = leg.k;
        let dir = seg_dir(c, k);
        let swing = (c.phase * 2.0 - k as f32 * 0.55).sin() * 0.38 * leg.side;
        let local = leg.side * FRAC_PI_2 + swing;
        let world = Vec2::from_angle(dir.to_angle() + local);
        let scale = seg_scale(c, k);
        let len = if c.depth > 0.5 || matches!(c.state, State::Dead(_)) {
            14.0 * BIG
        } else {
            // Planted on the surface when there is one within reach.
            let from = c.pts[k];
            terrain.raycast(from, from + world * 30.0 * BIG).map_or((15.0 + 3.0 * swing.abs()) * BIG, |hit| (hit.distance(from) - 3.0 * BIG).clamp(6.0 * BIG, 30.0 * BIG))
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
