//! The Sawleg: a giant grasshopper that fights on the ground and in great leaps.
//!
//! Its main attack is the **pounce**: it cocks its huge hind legs (the tell:
//! body low, femurs up, quivering), then springs in a fast, low arc through
//! where the chameleon stood, landing well past it; the body hurts on the way.
//! Enraged (half health) it chains `RAGE_POUNCES` pounces back and forth.
//! Close up it **kicks**: turns its back on the chameleon, wiggles (the tell)
//! and lashes out with both saw-toothed hind legs, sending it flying.
//! From afar it **spits** a brown glob in an arc (enraged: a fan of three) that
//! stains wherever it lands.
//! Between attacks it keeps its distance with twitchy little hops. Face-off on
//! first sight: it saws a hind leg against its wing (stridulating).
//! After a pounce chain it is winded for a moment: the opening to hit it.
//!
//! It bleeds brown `#8f563b` and gives the chameleon the **big jump** (hold
//! S on the ground to charge, release to launch; `player.rs`), shown by the
//! charge aura drawn here (`aura`). On death it flips over, a hind leg and
//! the antennae tear off and the other legs twitch. Health, getting hit and
//! the death spectacle live in `boss.rs`.

use std::f32::consts::{PI, TAU};

use bevy::{prelude::*, sprite_render::AlphaMode2d};

use crate::{
    boss::{self, Arena, Boss, BossHit, BossSpec, BossStuff},
    camera::{self, HitStop, MainCamera, Shake},
    chameleon,
    fumes::Cloud,
    gore::{Bleed, Fluid},
    player::{self, Player, Respawned},
    shapes::*,
    terrain::{Terrain, STEP_UP},
};

/// Its color: what the chameleon earns, its blood, the health bar.
pub const BROWN: Color = Color::srgb(0x8f as f32 / 255.0, 0x56 as f32 / 255.0, 0x3b as f32 / 255.0);

pub(crate) static SPEC: BossSpec = BossSpec {
    name: "THE SAWLEG",
    hp: 14,
    radius: RADIUS,
    blood: Fluid::Brown,
    color: BROWN,
    aggro: AGGRO,
    // 14 hp, pounces and kicks.
    food: 4.5,
};

/// Size of the boss relative to its drawing.
const SCALE: f32 = 2.6;
/// Hurtbox radius around the body center.
const RADIUS: f32 = 22.0 * SCALE;
/// Body center height above the floor when standing.
const FOOT: f32 = 15.0 * SCALE;
/// Half its length: how close to the arena edges its center may go.
const HALF_W: f32 = 30.0 * SCALE;
const AGGRO: f32 = 1000.0;
/// How far inside the screen edge (px) it must be to count as in view.
const SCREEN_MARGIN: f32 = 60.0;
const CONTACT_DAMAGE: i32 = 1;

/// The face-off: it stridulates (saws a hind leg on its wing) before the fight.
const TAUNT_TIME: f32 = 2.0;
/// The chameleon this close (px) cuts the taunt short: it pounces at once.
const TAUNT_BREAK: f32 = 260.0;

/// Preferred distance to the chameleon between attacks.
const KEEP_AWAY: f32 = 420.0;
/// Little repositioning hops.
const HOP_LENGTH: f32 = 170.0;
const HOP_HEIGHT: f32 = 70.0;
const HOP_TIME: f32 = 0.36;
const HOP_REST: (f32, f32) = (0.25, 0.6);
/// Seconds between attacks (enraged: × `RAGE_PACE`).
const COOLDOWN: (f32, f32) = (1.1, 1.8);
const RAGE_PACE: f32 = 0.7;

// The pounce (main attack).
const COCK: f32 = 0.6;
const POUNCE_HEIGHT: f32 = 130.0;
const POUNCE_TIME: f32 = 0.55;
/// It lands this far past where the chameleon stood.
const OVERSHOOT: f32 = 220.0;
const POUNCE_REACH: f32 = 900.0;
/// Enraged: pounces in a row (back and forth), with a short cock between them.
const RAGE_POUNCES: u32 = 3;
const RAGE_COCK: f32 = 0.3;
/// Winded after the pounce(s) (the opening).
const WINDED: f32 = 1.0;

// The kick (close up).
const KICK_RANGE: f32 = 230.0;
const KICK_WINDUP: f32 = 0.45;
const KICK_TIME: f32 = 0.3;
/// The hind legs reach this far behind its body.
const KICK_REACH: f32 = 120.0;
const KICK_DAMAGE: i32 = 1;
/// Sent flying.
const KICK_LAUNCH: Vec2 = Vec2::new(900.0, 520.0);

// The spit (from afar).
const AIM: f32 = 0.5;
const GLOB_TIME: f32 = 0.75;
const GLOB_GRAVITY: f32 = 1400.0;
const GLOB_RADIUS: f32 = 11.0;
const GLOB_DAMAGE: i32 = 1;
/// Enraged fan: the side globs land this far from the aimed spot.
const GLOB_FAN: f32 = 150.0;
/// Shares of the attacks from afar: pounce vs spit.
const SPIT_SHARE: f32 = 0.35;

/// After a flurry of hits: kicks if the chameleon is this close, else leaps away.
const EVADE_LENGTH: f32 = 420.0;
const EVADE_HEIGHT: f32 = 200.0;
const EVADE_TIME: f32 = 0.6;

const SHELL: Color = Color::srgb(0.47, 0.46, 0.28);
const DARK: Color = Color::srgb(0.24, 0.23, 0.13);
const PALE: Color = Color::srgb(0.70, 0.66, 0.44);
const EYE: Color = Color::srgb(0.16, 0.10, 0.06);
const SPINE: Color = Color::srgb(0.55, 0.20, 0.12);
const WING: Color = Color::srgba(0.66, 0.58, 0.40, 0.85);
const MEMBRANE: Color = Color::srgba(0.70, 0.30, 0.18, 0.45);

pub struct GrasshopperPlugin;

impl Plugin for GrasshopperPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (reset_on_respawn, on_hit, think, globs, dismember, animate, aura)
                .chain()
                .after(player::move_player)
                .after(chameleon::update_anim_state),
        );
    }
}

#[derive(Component)]
struct Hopper {
    state: State,
    /// Velocity (from the leap path while airborne: for the pose).
    vel: Vec2,
    facing: f32,
    cooldown: f32,
    rest: f32,
    /// Pounces left in an enraged chain.
    chain: u32,
    retaliate: bool,
    introduced: bool,
    /// 1 right after landing, fading: squash for the pose.
    squash: f32,
    seed: f32,
    dismember: bool,
    arena: Arena,
    rng: Rng,
}

#[derive(Clone, Copy, PartialEq)]
enum Leap {
    Hop,
    /// Through the chameleon: contact damage on the way.
    Pounce,
    Evade,
}

#[derive(Clone, Copy, PartialEq)]
enum State {
    Stand,
    Taunt(f32),
    /// Hind legs cocked (`len` = how long this one takes).
    Cock { t: f32, len: f32 },
    Air { from: Vec2, to: Vec2, height: f32, time: f32, t: f32, kind: Leap },
    Winded(f32),
    /// Back turned, wiggling, then the kick (`t` past `KICK_WINDUP`).
    Kick(f32),
    Aim(f32),
    Hurt(f32),
    Dead { t: f32, landed: bool },
}

impl Hopper {
    fn airborne(&self) -> bool {
        matches!(self.state, State::Air { .. } | State::Dead { landed: false, .. })
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
    Antenna { offset: f32 },
    Femur { near: bool },
    Tibia { near: bool },
    /// Front (0) and middle (1) legs.
    Leg { pair: usize, near: bool },
    Wing,
    Membrane,
}

/// A brown glob in flight.
#[derive(Component)]
struct Glob {
    vel: Vec2,
}

/// Spawns the grasshopper on the arena floor, facing left.
pub fn spawn(commands: &mut Commands, meshes: &mut Assets<Mesh>, materials: &mut Assets<ColorMaterial>, arena: &Arena) -> Entity {
    let shell = materials.add(SHELL);
    let dark = materials.add(DARK);
    let pale = materials.add(PALE);
    let eye = materials.add(EYE);
    let spine = materials.add(SPINE);
    let glint = materials.add(Color::WHITE);
    let wing = materials.add(ColorMaterial { color: WING, alpha_mode: AlphaMode2d::Blend, ..default() });
    let membrane = materials.add(ColorMaterial { color: MEMBRANE, alpha_mode: AlphaMode2d::Blend, ..default() });
    let mats = vec![(shell.clone(), SHELL), (dark.clone(), DARK), (pale.clone(), PALE), (spine.clone(), SPINE)];

    let home = Vec2::new(arena.home.x, arena.floor + FOOT);
    let hopper = commands.spawn((Transform::from_translation(home.extend(12.0)), Visibility::default())).id();
    let mut b = Builder { commands, meshes, owner: hopper };
    let rig = b.bone(Vec3::ZERO, hopper, BoneKind::Rig);
    b.commands.entity(rig).insert(Transform::from_scale(Vec3::new(-SCALE, SCALE, 1.0)));

    // Front and middle legs: thin, from under the thorax.
    for near in [false, true] {
        for pair in 0..2 {
            let (mat, z, dx) = if near { (&shell, 0.15, 0.0) } else { (&dark, -0.4, 2.5) };
            let hip = [Vec2::new(14.0, -5.0), Vec2::new(6.0, -6.0)][pair];
            let bone = b.bone((hip + Vec2::X * dx).extend(z), rig, BoneKind::Leg { pair, near });
            let path = if pair == 0 {
                smooth_open(&[Vec2::ZERO, Vec2::new(5.0, -4.0), Vec2::new(4.0, -9.0), Vec2::new(7.0, -10.0)], 3)
            } else {
                smooth_open(&[Vec2::ZERO, Vec2::new(-2.0, -5.0), Vec2::new(-1.0, -9.0), Vec2::new(-4.0, -10.0)], 3)
            };
            b.part(stroke(&path, |t| 2.2 - 1.0 * t, 1.0, 0.8, |_| 1.0), mat, Vec3::ZERO, bone);
        }
    }

    // Hind wing membrane: hidden under the wing cover until it leaps.
    let fan = b.bone(Vec3::new(2.0, 6.0, -0.2), rig, BoneKind::Membrane);
    let fan_pts: Vec<Vec2> = std::iter::once(Vec2::ZERO).chain((0..=12).map(|i| Vec2::from_angle(PI * 0.55 + i as f32 / 12.0 * PI * 0.5) * Vec2::new(40.0, 26.0))).collect();
    b.part(fill(&fan_pts, |p| 0.7 + 0.3 * (p.length() / 40.0)), &membrane, Vec3::ZERO, fan);
    b.commands.entity(fan).insert(Visibility::Hidden);

    // Abdomen and thorax: long, segmented, tapering to the back.
    let body = smooth_closed(
        &[
            Vec2::new(22.0, 3.0),
            Vec2::new(18.0, 9.0),
            Vec2::new(6.0, 10.0),
            Vec2::new(-12.0, 8.0),
            Vec2::new(-30.0, 5.0),
            Vec2::new(-40.0, 1.5),
            Vec2::new(-41.0, -1.5),
            Vec2::new(-30.0, -3.5),
            Vec2::new(-12.0, -6.0),
            Vec2::new(6.0, -8.0),
            Vec2::new(18.0, -6.0),
        ],
        6,
    );
    b.part(fill(&body, |p| 0.55 + 0.45 * ((p.y + 8.0) / 18.0).clamp(0.0, 1.0)), &shell, Vec3::ZERO, rig);
    // Segment lines across the abdomen and a pale belly stripe.
    let rings: Vec<[Vec2; 3]> = (0..6)
        .map(|i| {
            let x = -6.0 - i as f32 * 5.6;
            let top = 7.5 - i as f32 * 0.9;
            [Vec2::new(x, top), Vec2::new(x - 1.2, top), Vec2::new(x - 0.6, -4.0 + i as f32 * 0.4)]
        })
        .collect();
    b.part(triangles(&rings), &dark, Vec3::new(0.0, 0.0, 0.01), rig);
    let belly = smooth_open(&[Vec2::new(16.0, -5.0), Vec2::new(0.0, -6.5), Vec2::new(-20.0, -4.0), Vec2::new(-38.0, -0.5)], 5);
    b.part(stroke(&belly, |t| 2.6 * (1.0 - t * 0.7), 1.0, 1.0, |_| 1.0), &pale, Vec3::new(0.0, 0.0, 0.01), rig);

    // Wing cover (tegmen): long and narrow along the back, lifts open to leap.
    let wing_bone = b.bone(Vec3::new(10.0, 7.5, 0.3), rig, BoneKind::Wing);
    let tegmen = smooth_closed(&[Vec2::new(0.0, 1.5), Vec2::new(-20.0, 3.0), Vec2::new(-44.0, 1.0), Vec2::new(-50.0, -1.5), Vec2::new(-42.0, -2.5), Vec2::new(-18.0, -1.5), Vec2::new(0.0, -1.0)], 5);
    b.part(fill(&tegmen, |p| 0.75 + 0.25 * (p.y / 3.0)), &wing, Vec3::ZERO, wing_bone);
    let vein = smooth_open(&[Vec2::new(-2.0, 0.5), Vec2::new(-24.0, 1.0), Vec2::new(-46.0, -0.5)], 4);
    b.part(stroke(&vein, |_| 0.7, 1.0, 1.0, |_| 1.0), &dark, Vec3::new(0.0, 0.0, 0.01), wing_bone);

    // Pronotum: a saddle over the thorax.
    let saddle = smooth_closed(&[Vec2::new(6.0, 6.0), Vec2::new(8.0, 11.0), Vec2::new(18.0, 12.0), Vec2::new(21.0, 6.0), Vec2::new(19.0, 1.0), Vec2::new(9.0, 1.0)], 5);
    b.part(fill(&saddle, |p| 0.6 + 0.4 * (p.y / 12.0)), &dark, Vec3::new(0.0, 0.0, 0.35), rig);

    // Hind legs: a huge drumstick femur up and back from the hip, a sawtoothed
    // tibia folded down under it, and the foot on the floor.
    for near in [false, true] {
        let (mat, z, dx) = if near { (&shell, 0.5, 0.0) } else { (&dark, -0.45, 3.0) };
        let femur = b.bone(Vec3::new(-2.0 + dx, -2.0, z), rig, BoneKind::Femur { near });
        let thigh = smooth_closed(&[Vec2::new(2.0, 2.0), Vec2::new(-10.0, 10.0), Vec2::new(-24.0, 15.0), Vec2::new(-30.0, 15.0), Vec2::new(-29.0, 12.0), Vec2::new(-14.0, 4.0), Vec2::new(0.0, -3.0)], 5);
        b.part(fill(&thigh, |p| 0.65 + 0.35 * ((p.y + 3.0) / 18.0)), mat, Vec3::ZERO, femur);
        if near {
            // The herringbone pattern on the femur.
            let marks: Vec<[Vec2; 3]> = (0..4)
                .map(|i| {
                    let c = Vec2::new(-6.0 - i as f32 * 5.5, 4.0 + i as f32 * 2.5);
                    [c + Vec2::new(-1.5, 1.5), c + Vec2::new(0.5, 1.5), c + Vec2::new(2.0, -1.5)]
                })
                .collect();
            b.part(triangles(&marks), &dark, Vec3::new(0.0, 0.0, 0.01), femur);
        }
        let tibia = b.bone(Vec3::new(-29.0, 13.5, -0.02), femur, BoneKind::Tibia { near });
        let shin = smooth_open(&[Vec2::ZERO, Vec2::new(8.0, -12.0), Vec2::new(17.0, -26.0)], 4);
        b.part(stroke(&shin, |t| 2.6 - 0.8 * t, 1.0, 0.8, |_| 1.0), mat, Vec3::ZERO, tibia);
        // The saw: spines along the back of the tibia.
        let teeth: Vec<[Vec2; 3]> = (0..6)
            .map(|i| {
                let t = 0.15 + i as f32 * 0.14;
                let p = Vec2::ZERO.lerp(Vec2::new(17.0, -26.0), t);
                let d = Vec2::new(17.0, -26.0).normalize();
                let n = -d.perp();
                [p - d * 0.8, p + d * 0.8, p + n * 2.6 - d * 1.2]
            })
            .collect();
        b.part(triangles(&teeth), &spine, Vec3::new(0.0, 0.0, 0.01), tibia);
        // Foot: a short tarsus along the floor.
        let foot = smooth_open(&[Vec2::new(17.0, -26.0), Vec2::new(23.0, -27.0), Vec2::new(28.0, -26.0)], 3);
        b.part(stroke(&foot, |_| 1.8, 1.0, 1.0, |_| 1.0), mat, Vec3::new(0.0, 0.0, 0.01), tibia);
    }

    // Head: tall and blunt, a big compound eye, jaws and antennae.
    let head = b.bone(Vec3::new(21.0, 3.0, 0.4), rig, BoneKind::Head);
    let face = smooth_closed(&[Vec2::new(0.0, 7.0), Vec2::new(6.0, 8.0), Vec2::new(10.0, 3.0), Vec2::new(10.0, -5.0), Vec2::new(7.0, -9.0), Vec2::new(1.0, -8.0), Vec2::new(-2.0, -2.0)], 5);
    b.part(fill(&face, |p| 0.65 + 0.35 * ((p.y + 9.0) / 17.0)), &shell, Vec3::ZERO, head);
    let eye_pts: Vec<Vec2> = ellipse_pts(3.4, 4.6, 16).into_iter().map(|p| Vec2::from_angle(0.25).rotate(p)).collect();
    b.part(fill(&eye_pts, |p| 0.8 + 0.2 * (p.y / 4.6)), &eye, Vec3::new(5.0, 2.5, 0.01), head);
    b.part(fill(&ellipse_pts(0.9, 1.2, 8), |_| 1.0), &glint, Vec3::new(6.0, 4.5, 0.02), head);
    b.part(triangles(&[[Vec2::new(6.0, -8.0), Vec2::new(10.0, -8.0), Vec2::new(9.0, -11.5)], [Vec2::new(4.0, -8.5), Vec2::new(7.0, -8.5), Vec2::new(5.0, -11.0)]]), &dark, Vec3::new(0.0, 0.0, 0.01), head);
    for (offset, z) in [(0.0, -0.05), (1.1, 0.03)] {
        let bone = b.bone(Vec3::new(6.0, 7.5, z), head, BoneKind::Antenna { offset });
        let path = smooth_open(&[Vec2::ZERO, Vec2::new(8.0, 9.0), Vec2::new(14.0, 20.0), Vec2::new(12.0, 30.0)], 6);
        b.part(stroke(&path, |t| 1.5 - 0.9 * t, 1.0, 1.0, |_| 1.0), &dark, Vec3::ZERO, bone);
    }

    let mut rng = Rng(0x6EA5_5409);
    commands.entity(hopper).insert((
        Boss::new(&SPEC, home, mats).staged(),
        Hopper {
            state: State::Stand,
            vel: Vec2::ZERO,
            facing: -1.0,
            cooldown: rng.range(COOLDOWN.0, COOLDOWN.1),
            rest: rng.range(HOP_REST.0, HOP_REST.1),
            chain: 0,
            retaliate: false,
            introduced: false,
            squash: 0.0,
            seed: rng.range(0.0, 100.0),
            dismember: false,
            arena: *arena,
            rng,
        },
    ));
    hopper
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

/// React to a hit (health and blood are handled in `boss.rs`).
fn on_hit(mut hits: MessageReader<BossHit>, mut hoppers: Query<&mut Hopper>) {
    for hit in hits.read() {
        let Ok(mut h) = hoppers.get_mut(hit.boss) else { continue };
        if hit.killed {
            h.vel = Vec2::new(hit.dir.x * 320.0, 520.0);
            h.state = State::Dead { t: 0.0, landed: false };
            h.dismember = true;
        } else if hit.retaliate {
            h.retaliate = true;
        } else if hit.stagger && matches!(h.state, State::Stand | State::Taunt(_) | State::Winded(_) | State::Aim(_)) {
            h.vel = Vec2::new(hit.dir.x * 260.0, 0.0);
            h.state = State::Hurt(0.0);
        }
    }
}

/// `x` kept inside the arena, the whole body over its floor (the middle if it's too narrow).
fn inside(arena: &Arena, x: f32) -> f32 {
    let (lo, hi) = (arena.left + HALF_W, arena.right - HALF_W);
    if lo < hi { x.clamp(lo, hi) } else { (arena.left + arena.right) * 0.5 }
}

fn landing(arena: &Arena, terrain: &Terrain, feet: Vec2, x: f32) -> Vec2 {
    let x = inside(arena, x);
    let y = terrain.surface(Vec2::new(x, feet.y + 2.0 * STEP_UP + 40.0), 3000.0).unwrap_or(arena.floor);
    Vec2::new(x, y)
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

fn leap(arena: &Arena, terrain: &Terrain, feet: Vec2, x: f32, height: f32, time: f32, kind: Leap) -> State {
    let to = landing(arena, terrain, feet, x);
    let height = headroom(terrain, (feet + to) * 0.5, height);
    State::Air { from: feet, to, height, time, t: 0.0, kind }
}

/// A pounce through the chameleon at `ppos`, landing past it.
fn pounce(arena: &Arena, terrain: &Terrain, feet: Vec2, ppos: Vec2, enraged: bool) -> State {
    let dx = (ppos.x - feet.x).clamp(-POUNCE_REACH, POUNCE_REACH);
    let past = if dx >= 0.0 { OVERSHOOT } else { -OVERSHOOT };
    let time = if enraged { POUNCE_TIME * 0.85 } else { POUNCE_TIME };
    leap(arena, terrain, feet, feet.x + dx + past, POUNCE_HEIGHT, time, Leap::Pounce)
}

fn think(
    time: Res<Time>,
    mut commands: Commands,
    mut shake: ResMut<Shake>,
    mut stop: ResMut<HitStop>,
    mut bleed: MessageWriter<Bleed>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    player: Single<(&mut Player, &Transform), Without<Hopper>>,
    camera: Single<(&Camera, &GlobalTransform), With<MainCamera>>,
    mut hoppers: Query<(&mut Hopper, &mut Boss, &mut Transform, Option<&mut boss::Sense>)>,
    terrain: Res<Terrain>,
) {
    let dt = time.delta_secs().min(1.0 / 30.0);
    let (mut p, ptf) = player.into_inner();
    let ppos = ptf.translation.truncate();

    for (mut h, mut boss, mut tf, sense) in &mut hoppers {
        let pos = tf.translation.truncate();
        let to_player = ppos - pos;
        let alive = p.dead.is_none();
        let aggro = alive && boss.awake;
        let arena = h.arena;
        let ground_at = |x: f32, y: f32| terrain.surface(Vec2::new(x, y - FOOT + 2.0 * STEP_UP), 3000.0);
        let floor = ground_at(pos.x, pos.y).unwrap_or(arena.floor);
        let feet = Vec2::new(pos.x, floor);
        if let Some(mut s) = sense {
            s.goal = aggro.then_some(ppos);
            s.target = match h.state {
                State::Air { to, .. } => Some(to),
                _ => aggro.then_some(Vec2::new(ppos.x.clamp(arena.left, arena.right), floor)),
            };
            s.sees_player = terrain.raycast(pos, ppos).is_none();
            s.path.clear();
        }
        let side = if to_player.x >= 0.0 { 1.0 } else { -1.0 };
        let on_screen = match (camera.0.world_to_viewport(camera.1, pos.extend(0.0)), camera.0.logical_viewport_size()) {
            (Ok(v), Some(size)) => v.x > SCREEN_MARGIN && v.y > SCREEN_MARGIN && v.x < size.x - SCREEN_MARGIN && v.y < size.y - SCREEN_MARGIN,
            _ => false,
        };
        if !h.introduced {
            boss.staged = true;
        }
        let enraged = boss.enraged();
        let pace = if enraged { RAGE_PACE } else { 1.0 };
        let dx = to_player.x.abs();
        let touching = alive && to_player.length() < RADIUS * 0.8 + 20.0;

        // Hit too many times in a row: kicks back at once, or leaps out of reach.
        if std::mem::take(&mut h.retaliate) && matches!(h.state, State::Stand | State::Hurt(_) | State::Winded(_) | State::Taunt(_) | State::Aim(_)) {
            h.state = if dx < KICK_RANGE {
                h.facing = -side;
                State::Kick(KICK_WINDUP * 0.5)
            } else {
                h.facing = side;
                leap(&arena, &terrain, feet, pos.x - side * EVADE_LENGTH, EVADE_HEIGHT, EVADE_TIME, Leap::Evade)
            };
        }

        h.state = match h.state {
            State::Dead { t: t0, landed } => {
                let td = t0 + dt;
                for at in [0.3, 0.7] {
                    if t0 < at && td >= at {
                        bleed.write(Bleed { pos, dir: Vec2::Y, spread: PI, speed: 520.0, amount: 60, fluid: Fluid::Brown, sticky: true });
                    }
                }
                let mut landed = landed;
                let mut next = pos;
                if !landed {
                    h.vel.y -= 1400.0 * dt;
                    next += h.vel * dt;
                    let rest = floor + 8.0 * SCALE;
                    if ground_at(next.x, pos.y).is_some() && next.y <= rest && h.vel.y < 0.0 {
                        next.y = rest;
                        h.vel = Vec2::new(h.vel.x * 0.3, 0.0);
                        landed = true;
                        shake.add(0.25);
                        bleed.write(Bleed { pos, dir: Vec2::Y, spread: 1.3, speed: 240.0, amount: 40, fluid: Fluid::Brown, sticky: true });
                    }
                } else {
                    h.vel.x *= 1.0 - 5.0 * dt;
                    next.x += h.vel.x * dt;
                }
                if td < 3.5 && h.rng.chance(dt * 12.0) {
                    bleed.write(Bleed { pos, dir: Vec2::Y, spread: 1.0, speed: 140.0, amount: 2, fluid: Fluid::Brown, sticky: true });
                }
                tf.translation.x = next.x;
                tf.translation.y = next.y;
                State::Dead { t: td, landed }
            }
            State::Hurt(s) => {
                h.vel.x *= 1.0 - 6.0 * dt;
                if s < 0.25 { State::Hurt(s + dt) } else { State::Stand }
            }
            State::Stand => {
                h.vel.x = 0.0;
                h.facing = if aggro { side } else { h.facing };
                h.cooldown -= dt;
                h.rest -= dt;
                if !aggro {
                    h.introduced = false;
                    if h.rest <= 0.0 && (boss.home.x - pos.x).abs() > 60.0 {
                        h.rest = h.rng.range(HOP_REST.0, HOP_REST.1);
                        h.facing = (boss.home.x - pos.x).signum();
                        let x = pos.x + (boss.home.x - pos.x).clamp(-HOP_LENGTH, HOP_LENGTH);
                        leap(&arena, &terrain, feet, x, HOP_HEIGHT, HOP_TIME, Leap::Hop)
                    } else {
                        State::Stand
                    }
                } else if !h.introduced {
                    if on_screen {
                        h.introduced = true;
                        boss.staged = false;
                        shake.add(0.15);
                        State::Taunt(0.0)
                    } else {
                        State::Stand
                    }
                } else if h.cooldown <= 0.0 {
                    h.cooldown = h.rng.range(COOLDOWN.0, COOLDOWN.1) * pace;
                    if dx < KICK_RANGE {
                        h.facing = -side;
                        State::Kick(0.0)
                    } else if dx > KEEP_AWAY * 0.8 && h.rng.chance(SPIT_SHARE) {
                        State::Aim(0.0)
                    } else {
                        h.chain = if enraged { RAGE_POUNCES } else { 1 };
                        State::Cock { t: 0.0, len: COCK }
                    }
                } else if h.rest <= 0.0 && (dx < KEEP_AWAY - 120.0 || dx > KEEP_AWAY + 160.0) {
                    h.rest = h.rng.range(HOP_REST.0, HOP_REST.1) * pace;
                    let dir = if dx < KEEP_AWAY { -side } else { side };
                    let x = pos.x + dir * HOP_LENGTH * h.rng.range(0.7, 1.2);
                    // Backed against the arena edge: hop over the chameleon instead.
                    let x = if (x - landing(&arena, &terrain, feet, x).x).abs() > 30.0 { pos.x + side * (dx + HALF_W + 100.0) } else { x };
                    leap(&arena, &terrain, feet, x, HOP_HEIGHT, HOP_TIME, Leap::Hop)
                } else {
                    State::Stand
                }
            }
            State::Taunt(s) => {
                h.facing = side;
                if to_player.length() < TAUNT_BREAK {
                    h.chain = 1;
                    State::Cock { t: COCK * 0.5, len: COCK }
                } else if s >= TAUNT_TIME {
                    h.cooldown = h.rng.range(0.3, 0.6);
                    State::Stand
                } else {
                    State::Taunt(s + dt)
                }
            }
            State::Cock { t: s, len } => {
                h.facing = side;
                if s >= len * pace.max(0.8) {
                    // Commits to where the chameleon is now.
                    shake.add(0.1);
                    pounce(&arena, &terrain, feet, ppos, enraged)
                } else {
                    State::Cock { t: s + dt, len }
                }
            }
            State::Air { from, to, height, time, t: s, kind } => {
                let s = s + dt;
                let k = (s / time).min(1.0);
                let next = Vec2::new(from.x + (to.x - from.x) * k, from.y + (to.y - from.y) * k + FOOT + height * 4.0 * k * (1.0 - k));
                h.vel = (next - pos) / dt.max(1e-4);
                if (to.x - from.x).abs() > 1.0 && kind != Leap::Evade {
                    h.facing = (to.x - from.x).signum();
                }
                tf.translation.x = next.x;
                tf.translation.y = next.y;
                if kind == Leap::Pounce && touching {
                    boss::hurt_player(&mut p, ppos, pos, CONTACT_DAMAGE, &mut bleed, (&mut shake, &mut stop));
                }
                if k < 1.0 {
                    State::Air { from, to, height, time, t: s, kind }
                } else {
                    h.squash = 1.0;
                    h.vel = Vec2::ZERO;
                    shake.add(if kind == Leap::Hop { 0.04 } else { 0.2 });
                    match kind {
                        Leap::Pounce => {
                            h.chain = h.chain.saturating_sub(1);
                            if h.chain > 0 && alive {
                                State::Cock { t: 0.0, len: RAGE_COCK }
                            } else {
                                State::Winded(0.0)
                            }
                        }
                        Leap::Evade => {
                            h.cooldown = h.cooldown.min(0.4);
                            State::Stand
                        }
                        Leap::Hop => State::Stand,
                    }
                }
            }
            State::Winded(s) => {
                if s >= WINDED * if enraged { 0.7 } else { 1.0 } {
                    h.rest = 0.2;
                    State::Stand
                } else {
                    State::Winded(s + dt)
                }
            }
            State::Kick(s) => {
                // Back turned (facing away), the hind legs toward the chameleon.
                let s1 = s + dt;
                if s < KICK_WINDUP && s1 >= KICK_WINDUP {
                    // Lashes out behind it.
                    let back = -h.facing;
                    let reach = (ppos.x - pos.x) * back;
                    let hit = alive && reach > -RADIUS * 0.3 && reach < RADIUS + KICK_REACH && (ppos.y - pos.y).abs() < FOOT + 60.0;
                    shake.add(0.2);
                    if hit {
                        let before = p.wounds;
                        boss::hurt_player(&mut p, ppos, pos, KICK_DAMAGE, &mut bleed, (&mut shake, &mut stop));
                        if p.wounds != before {
                            // Sent flying.
                            p.vel = Vec2::new(back * KICK_LAUNCH.x, KICK_LAUNCH.y);
                            stop.add(0.05);
                        }
                    }
                }
                if s1 >= KICK_WINDUP + KICK_TIME {
                    h.facing = side;
                    State::Stand
                } else {
                    State::Kick(s1)
                }
            }
            State::Aim(s) => {
                h.facing = side;
                if s >= AIM {
                    let mouth = pos + Vec2::new(30.0 * SCALE * h.facing, -1.0 * SCALE);
                    let spots: &[f32] = if enraged { &[0.0, -GLOB_FAN, GLOB_FAN] } else { &[0.0] };
                    for &off in spots {
                        let aim = ppos + Vec2::new(off, 0.0);
                        // Ballistic: lands on the spot after `GLOB_TIME`.
                        let vel = (aim - mouth) / GLOB_TIME + Vec2::Y * 0.5 * GLOB_GRAVITY * GLOB_TIME;
                        let mesh = meshes.add(fill(&ellipse_pts(GLOB_RADIUS, GLOB_RADIUS * 0.8, 14), |p| 0.75 + 0.25 * (p.y / GLOB_RADIUS)));
                        let mat = materials.add(BROWN);
                        commands.spawn((Mesh2d(mesh), MeshMaterial2d(mat), Transform::from_translation(mouth.extend(13.0)), camera::color_layer(), Glob { vel }, BossStuff));
                    }
                    shake.add(0.06);
                    h.cooldown = h.cooldown.max(0.6);
                    State::Stand
                } else {
                    State::Aim(s + dt)
                }
            }
        };

        // On the ground: follow the floor under it.
        if !h.airborne() && !matches!(h.state, State::Dead { .. }) {
            let x = inside(&arena, tf.translation.x + h.vel.x * dt);
            let floor = ground_at(x, tf.translation.y).unwrap_or(floor);
            tf.translation.x = x;
            tf.translation.y = floor + FOOT;
        }
        h.squash = (h.squash - dt * 4.0).max(0.0);
    }
}

/// Brown globs fly in an arc; they hurt the chameleon and splat on whatever they hit.
fn globs(
    time: Res<Time>,
    mut commands: Commands,
    mut shake: ResMut<Shake>,
    mut stop: ResMut<HitStop>,
    mut bleed: MessageWriter<Bleed>,
    terrain: Res<Terrain>,
    player: Single<(&mut Player, &Transform)>,
    mut globs: Query<(Entity, &mut Glob, &mut Transform), Without<Player>>,
) {
    let dt = time.delta_secs().min(1.0 / 30.0);
    let (mut p, ptf) = player.into_inner();
    let ppos = ptf.translation.truncate();
    for (e, mut g, mut tf) in &mut globs {
        g.vel.y -= GLOB_GRAVITY * dt;
        let pos = tf.translation.truncate();
        let next = pos + g.vel * dt;
        let splat = |bleed: &mut MessageWriter<Bleed>, at: Vec2, dir: Vec2| {
            bleed.write(Bleed { pos: at, dir, spread: 1.1, speed: 260.0, amount: 22, fluid: Fluid::Brown, sticky: true });
        };
        if p.dead.is_none() && next.distance(ppos) < GLOB_RADIUS + player::HALF.y {
            boss::hurt_player(&mut p, ppos, pos, GLOB_DAMAGE, &mut bleed, (&mut shake, &mut stop));
            splat(&mut bleed, next, Vec2::Y);
            commands.entity(e).despawn();
        } else if terrain.solid_at(next) || next.y < terrain.bounds.min.y {
            splat(&mut bleed, pos, -g.vel.normalize_or(Vec2::Y) + Vec2::Y * 0.5);
            commands.entity(e).despawn();
        } else {
            tf.translation.x = next.x;
            tf.translation.y = next.y;
        }
    }
}

/// The chameleon came back: start the fight over (`boss.rs` heals it).
fn reset_on_respawn(mut commands: Commands, mut respawned: MessageReader<Respawned>, mut hoppers: Query<&mut Hopper>, globs: Query<Entity, With<Glob>>) {
    if respawned.read().count() == 0 {
        return;
    }
    for e in &globs {
        commands.entity(e).despawn();
    }
    for mut h in &mut hoppers {
        if matches!(h.state, State::Dead { .. }) {
            continue;
        }
        h.state = State::Stand;
        h.introduced = false;
        h.vel = Vec2::ZERO;
        h.cooldown = 1.5;
        h.chain = 0;
    }
}

/// A freshly killed grasshopper: the near hind leg and the antennae tear off.
fn dismember(mut commands: Commands, mut hoppers: Query<&mut Hopper>, bones: Query<(Entity, &Bone, &GlobalTransform)>) {
    let mut done = Vec::new();
    for (e, bone, gt) in &bones {
        let Ok(h) = hoppers.get(bone.owner) else { continue };
        if !h.dismember {
            continue;
        }
        done.push(bone.owner);
        let f = h.facing;
        let (vel, spin, bleeds) = match bone.kind {
            BoneKind::Femur { near: true } => (Vec2::new(-f * 280.0, 600.0), 8.0, Some(Fluid::Brown)),
            BoneKind::Antenna { offset } => (Vec2::new(f * (160.0 + offset * 120.0), 520.0), 11.0, None),
            _ => continue,
        };
        commands.entity(e).remove::<Bone>();
        boss::tear_off(&mut commands, e, gt, vel, spin, bleeds);
    }
    done.sort();
    done.dedup();
    for owner in done {
        if let Ok(mut h) = hoppers.get_mut(owner) {
            h.dismember = false;
        }
    }
}

fn animate(time: Res<Time>, hoppers: Query<&Hopper>, mut bones: Query<(&Bone, &mut Transform, &mut Visibility)>) {
    let t = time.elapsed_secs();
    let dt = time.delta_secs();
    for (bone, mut tf, mut vis) in &mut bones {
        let Ok(h) = hoppers.get(bone.owner) else { continue };
        let tt = t + h.seed;
        let (dead_t, landed) = match h.state {
            State::Dead { t, landed } => (Some(t), landed),
            _ => (None, false),
        };
        // 0..1: hind legs cocked, body low (before a pounce, a little after landing).
        let cock = match h.state {
            State::Cock { t, len } => (t / len).min(1.0),
            State::Winded(s) => (1.0 - s / 0.4).clamp(0.0, 1.0) * 0.5,
            _ => h.squash * 0.6,
        };
        // 0..1: legs kicked out straight (taking off), folding back on the way down.
        let stretch = match h.state {
            State::Air { t: s, time, .. } => (1.0 - (s / time) * 1.6).clamp(0.0, 1.0),
            State::Dead { landed: false, .. } => 0.7,
            _ => 0.0,
        };
        // The kick: wiggle (windup), then both legs lash out and come back.
        let kick = match h.state {
            State::Kick(s) if s >= KICK_WINDUP => (PI * ((s - KICK_WINDUP) / KICK_TIME).min(1.0)).sin(),
            _ => 0.0,
        };
        let wiggle = match h.state {
            State::Kick(s) if s < KICK_WINDUP => (tt * 38.0).sin() * 0.06,
            _ => 0.0,
        };
        let shiver = if matches!(h.state, State::Cock { .. }) { (tt * 60.0).sin() * 0.025 * cock } else { 0.0 };
        // The taunt: a hind leg saws up and down against the wing.
        let saw = match h.state {
            State::Taunt(s) if s > 0.3 => (tt * 22.0).sin(),
            _ => 0.0,
        };
        let airborne = h.airborne() && dead_t.is_none();
        match bone.kind {
            BoneKind::Rig => {
                tf.scale.x += (h.facing * SCALE - tf.scale.x) * (1.0 - (-18.0 * dt).exp());
                tf.scale.y = SCALE * (1.0 - 0.1 * cock + 0.06 * stretch);
                tf.translation.y = -cock * 5.0 * SCALE;
                let angle = if let Some(td) = dead_t {
                    if landed { PI } else { PI * (td * 2.4).min(1.0) }
                } else {
                    match h.state {
                        State::Air { .. } => (h.vel.y * 0.0006).clamp(-0.4, 0.4),
                        // Rears its tail end up to kick, nose down.
                        State::Kick(_) => -0.12 + wiggle,
                        State::Aim(s) => 0.15 * (s / AIM).min(1.0),
                        State::Cock { .. } => -0.08 * cock + shiver,
                        _ => (tt * 1.7).sin() * 0.012,
                    }
                };
                tf.rotation = Quat::from_rotation_z(angle);
            }
            BoneKind::Head => {
                let angle = match h.state {
                    _ if dead_t.is_some() => -0.3,
                    State::Aim(s) => -0.25 * (s / AIM).min(1.0),
                    _ => (tt * 3.0).sin() * 0.04,
                };
                tf.rotation = Quat::from_rotation_z(angle);
            }
            BoneKind::Antenna { offset } => {
                let angle = match h.state {
                    _ if dead_t.is_some() => -0.6 + (tt * 10.0 + offset).sin() * 0.05,
                    // Swept back in the air.
                    State::Air { .. } => 0.7,
                    _ => (tt * 2.6 + offset).sin() * 0.15 + cock * (tt * 30.0 + offset).sin() * 0.12,
                };
                tf.rotation = Quat::from_rotation_z(angle);
            }
            BoneKind::Femur { near } => {
                let angle = if let Some(td) = dead_t {
                    let fade = (1.0 - td / 6.0).max(0.1);
                    -0.4 + (tt * 22.0 + near as u8 as f32).sin() * 0.4 * fade
                } else {
                    // Cocked = raised; the kick and the take-off swing it down and back.
                    0.18 * cock - 0.9 * stretch - 0.8 * kick + if near { 0.0 } else { 0.1 * saw }
                };
                tf.rotation = Quat::from_rotation_z(angle);
            }
            BoneKind::Tibia { near } => {
                let angle = if let Some(td) = dead_t {
                    let fade = (1.0 - td / 6.0).max(0.1);
                    0.6 + (tt * 19.0 + near as u8 as f32 * 2.0).sin() * 0.4 * fade
                } else {
                    // Folded tight cocked, kicked straight out (unfolds backwards).
                    0.15 * cock - 2.2 * stretch - 2.0 * kick + if near { 0.0 } else { 0.35 * saw }
                };
                tf.rotation = Quat::from_rotation_z(angle);
            }
            BoneKind::Leg { pair, near } => {
                let group = if near { 0.0 } else { PI };
                let angle = if let Some(td) = dead_t {
                    let fade = (1.0 - td / 6.0).max(0.15);
                    (tt * 28.0 + pair as f32 * 1.7 + group).sin() * 0.5 * fade
                } else if airborne {
                    // Reaching forward (front) and back (middle) for the landing.
                    if pair == 0 { 0.6 } else { -0.5 }
                } else {
                    (h.vel.x.abs() * 0.01).min(0.3) * (tt * 14.0 + group).sin()
                };
                tf.rotation = Quat::from_rotation_z(angle);
            }
            BoneKind::Wing => {
                let open = if airborne { 1.0 } else { 0.0 };
                tf.rotation = Quat::from_rotation_z(0.55 * open + (tt * 50.0).sin() * 0.05 * open + 0.03 * saw);
            }
            BoneKind::Membrane => {
                *vis = if airborne { Visibility::Inherited } else { Visibility::Hidden };
                let flap = (tt * 70.0).sin();
                tf.rotation = Quat::from_rotation_z(0.2 * flap);
                tf.scale = Vec3::new(1.0, 0.7 + 0.3 * flap.abs(), 1.0);
            }
        }
    }
}

/// The big jump charging (`Player::charging`): a brown-gold aura gathers around
/// the chameleon, motes rising from its feet; fully charged it throbs bright.
#[derive(Component)]
struct Aura;

#[derive(Component)]
struct Mote {
    vel: Vec2,
    age: f32,
    life: f32,
    material: Handle<ColorMaterial>,
}

const AURA_COLOR: Color = Color::srgb(0.85, 0.55, 0.25);
const AURA_FULL: Color = Color::srgb(1.0, 0.86, 0.5);

#[allow(clippy::too_many_arguments)]
fn aura(
    time: Res<Time>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    cloud: Res<Cloud>,
    mut shake: ResMut<Shake>,
    mut was_full: Local<bool>,
    mut mote_mesh: Local<Option<Handle<Mesh>>>,
    player: Single<(&Player, &Transform)>,
    mut glow: Query<(&mut Transform, &MeshMaterial2d<ColorMaterial>), (With<Aura>, Without<Player>, Without<Mote>)>,
    mut motes: Query<(Entity, &mut Mote, &mut Transform), (Without<Aura>, Without<Player>)>,
) {
    let dt = time.delta_secs();
    let now = time.elapsed_secs();
    let (p, ptf) = *player;
    let center = ptf.translation.truncate();
    let charging = p.charging();
    let k = charging.map_or(0.0, |c| (c / player::CHARGE_TIME).min(1.0));
    let full = k >= 1.0;
    if full && !*was_full {
        // Ready: a little jolt.
        shake.add(0.12);
    }
    *was_full = full;

    if glow.is_empty() {
        let mat = materials.add(ColorMaterial { color: AURA_COLOR.with_alpha(0.0), alpha_mode: AlphaMode2d::Blend, ..default() });
        commands.spawn((Mesh2d(cloud.0.clone()), MeshMaterial2d(mat), Transform::from_xyz(center.x, center.y, 9.5), camera::color_layer(), Aura));
    }
    for (mut tf, mat) in &mut glow {
        tf.translation.x = center.x;
        tf.translation.y = center.y;
        let throb = if full { 1.0 + 0.12 * (now * 18.0).sin() } else { 1.0 + 0.05 * (now * 9.0).sin() };
        tf.scale = Vec3::splat((30.0 + 40.0 * k) * throb);
        if let Some(mut m) = materials.get_mut(&mat.0) {
            let c = if full { AURA_FULL } else { AURA_COLOR };
            m.color = c.with_alpha(if charging.is_some() { 0.15 + 0.4 * k } else { 0.0 });
        }
    }

    // Motes rise from the feet while charging, more and faster as it fills.
    if charging.is_some() && p.dead.is_none() {
        let mesh = mote_mesh.get_or_insert_with(|| meshes.add(fill(&ellipse_pts(1.0, 1.0, 8), |_| 1.0))).clone();
        let n = (dt * (12.0 + 40.0 * k)).ceil() as u32;
        let mut rng = Rng(((now * 1000.0) as u32).wrapping_mul(2654435761) | 1);
        rng.next();
        for _ in 0..n {
            if !rng.chance(0.6) {
                continue;
            }
            let at = center + Vec2::new(rng.range(-1.0, 1.0) * player::HALF.x * 1.3, -player::HALF.y + rng.range(0.0, 6.0));
            let c = if full { AURA_FULL } else { AURA_COLOR };
            let material = materials.add(ColorMaterial { color: c, alpha_mode: AlphaMode2d::Blend, ..default() });
            commands.spawn((
                Mesh2d(mesh.clone()),
                MeshMaterial2d(material.clone()),
                Transform::from_translation(at.extend(11.5)).with_scale(Vec3::splat(rng.range(1.5, 3.0))),
                camera::color_layer(),
                Mote { vel: Vec2::new(rng.range(-15.0, 15.0), rng.range(60.0, 120.0) * (1.0 + k)), age: 0.0, life: rng.range(0.4, 0.8), material },
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
        tf.rotation = Quat::from_rotation_z(a * TAU);
        if let Some(mut mat) = materials.get_mut(&m.material) {
            mat.color = mat.color.with_alpha(0.9 * (1.0 - a));
        }
    }
}
