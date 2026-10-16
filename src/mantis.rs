//! The Reaper: a giant praying mantis whose forelegs are scythes. It haunts the
//! abandoned shopping center and it is relentless: it stalks straight at the
//! chameleon and barely rests between attacks.
//!
//! Close up it **reaps**: raises both scythes high (the tell: blades up, open,
//! quivering), then sweeps them down in front of it, lurching forward; it chains
//! `COMBO` sweeps (enraged `RAGE_COMBO`, the last one throwing a cyan scythe
//! wave along the floor). Mid range it **lunges**: coils back (the tell: body
//! low, prothorax pulled in, blades cocked) and shoots forward along the floor,
//! scythes snapping shut on whatever is there (`LUNGE_DAMAGE`). When the
//! chameleon is out of reach (far away or up on a ledge) it **pounces** on it
//! from the air, wings spread, scythes slamming down where it lands.
//! After a combo, a lunge or a pounce it needs a moment (`Recover`): the opening.
//! Hit too many times in a row it **parries**: scythes crossed in front (blows
//! glance off, `Boss.guard`), then counters with a sweep at once.
//! Face-off on first sight: the threat display, rearing up with its scythes
//! spread and the eyespot wings fanned out.
//!
//! It bleeds cyan `#00ffff` and gives the chameleon the **scythe claws** (a
//! wider slash that cuts deeper, `chameleon::SCYTHE_*`). On death it is thrown
//! onto its back, its head and the near scythe torn off, the legs twitching.
//! Health, getting hit and the death spectacle live in `boss.rs`.

use std::f32::consts::PI;

use bevy::{prelude::*, sprite_render::AlphaMode2d};

use crate::{
    boss::{self, Arena, Boss, BossHit, BossSpec, BossStuff},
    camera::{self, HitStop, MainCamera, Shake},
    chameleon,
    gore::{Bleed, Fluid},
    player::{self, Player, Respawned},
    shapes::*,
    terrain::{Terrain, STEP_UP},
};

/// Its color: what the chameleon earns, its blood, the health bar.
pub const CYAN: Color = Color::srgb(0.0, 1.0, 1.0);

pub(crate) static SPEC: BossSpec = BossSpec {
    name: "THE REAPER",
    hp: 16,
    radius: RADIUS,
    blood: Fluid::Cyan,
    color: CYAN,
    aggro: AGGRO,
    // 16 hp, and it never lets up.
    food: 5.0,
};

/// Size of the boss relative to its drawing.
const SCALE: f32 = 2.2;
/// Hurtbox radius around the body center (the hips).
const RADIUS: f32 = 22.0 * SCALE;
/// Body center height above the floor when standing.
const FOOT: f32 = 16.0 * SCALE;
/// Half its length: how close to the arena edges its center may go.
const HALF_W: f32 = 26.0 * SCALE;
const AGGRO: f32 = 1100.0;
/// How far inside the screen edge (px) it must be to count as in view.
const SCREEN_MARGIN: f32 = 60.0;

/// The threat display before the fight.
const TAUNT_TIME: f32 = 1.6;
/// The chameleon this close (px) cuts the display short: it reaps at once.
const TAUNT_BREAK: f32 = 220.0;

/// Stalking speed (enraged: `RAGE_WALK`).
const WALK: f32 = 210.0;
const RAGE_WALK: f32 = 290.0;
/// Seconds between attacks (enraged: × `RAGE_PACE`): short, it is relentless.
const COOLDOWN: (f32, f32) = (0.35, 0.7);
const RAGE_PACE: f32 = 0.7;

// The reap (close up): scythes raised, then swept down in front.
/// It starts reaping when the chameleon is this close (px, horizontally).
const STRIKE_RANGE: f32 = 150.0;
/// How far the sweep reaches from its shoulders.
const STRIKE_REACH: f32 = 30.0 * SCALE;
const STRIKE_DAMAGE: i32 = 1;
/// Raising the scythes: the first time (the tell) and between the sweeps of a combo.
const RAISE: f32 = 0.4;
const COMBO_RAISE: f32 = 0.18;
const STRIKE_TIME: f32 = 0.22;
/// Part of the sweep (fraction of `STRIKE_TIME`) where the blades cut.
const STRIKE_HIT: f32 = 0.45;
/// It lurches this far forward with each sweep.
const STRIKE_STEP: f32 = 40.0;
/// Sweeps in a row (enraged: `RAGE_COMBO`, the last one throws a scythe wave).
const COMBO: u32 = 2;
const RAGE_COMBO: u32 = 3;

// The lunge (mid range): coiled back, then a dash along the floor.
const COIL: f32 = 0.45;
const LUNGE_MIN: f32 = 160.0;
const LUNGE_RANGE: f32 = 420.0;
const LUNGE_SPEED: f32 = 950.0;
const LUNGE_TIME: f32 = 0.32;
const LUNGE_DAMAGE: i32 = 2;
/// Share of the mid range attacks that are lunges (else it walks in to reap).
const LUNGE_SHARE: f32 = 0.6;

// The pounce (out of reach): a leap onto the chameleon, scythes slamming down.
/// The chameleon's feet this high above its floor count as out of reach.
const LEDGE: f32 = 90.0;
const POUNCE_HEIGHT: f32 = 170.0;
const POUNCE_TIME: f32 = 0.6;
/// The slam on landing hurts within this distance.
const SLAM_REACH: f32 = 34.0 * SCALE;
const SLAM_DAMAGE: i32 = 1;

/// The opening after a combo, a lunge or a pounce (enraged × 0.7).
const RECOVER: f32 = 0.7;
/// Scythes crossed (blows glance off), then the counter.
const PARRY: f32 = 0.45;

// The scythe wave (enraged): a cyan crescent skimming the floor.
const WAVE_RANGE: f32 = 320.0;
const WAVE_TIME: f32 = 0.4;
const WAVE_RADIUS: f32 = 22.0;
const WAVE_DAMAGE: i32 = 1;
const WAVE_COLOR: Color = Color::srgba(0.75, 1.0, 1.0, 0.9);

const SHELL: Color = Color::srgb(0.50, 0.56, 0.40);
const DARK: Color = Color::srgb(0.26, 0.30, 0.20);
const PALE: Color = Color::srgb(0.70, 0.74, 0.56);
/// The scythe blades: bone white.
const BLADE: Color = Color::srgb(0.88, 0.86, 0.78);
const EYE: Color = Color::srgb(0.12, 0.14, 0.08);
const WING: Color = Color::srgba(0.55, 0.62, 0.45, 0.85);
/// The hind wings of the threat display, with their eyespots.
const DISPLAY: Color = Color::srgba(0.30, 0.75, 0.75, 0.6);

// The pose at rest, in its drawing (rig) frame: the prothorax up, the scythes
// folded in prayer.
const NECK_REST: f32 = 0.95;
const FEMUR_REST: f32 = -1.25;
/// The blade folded back against the femur.
const TIBIA_FOLD: f32 = -2.95;
/// The scythes raised high, blades open: ready to reap.
const FEMUR_RAISED: f32 = 1.35;
const TIBIA_OPEN: f32 = -1.7;
/// Where the scythes hang from the prothorax, and where the prothorax starts (rig units).
const SHOULDER: Vec2 = Vec2::new(15.0, -1.5);
const NECK_BASE: Vec2 = Vec2::new(3.0, 2.0);

pub struct MantisPlugin;

impl Plugin for MantisPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (reset_on_respawn, on_hit, think, waves, dismember, animate)
                .chain()
                .after(player::move_player)
                .after(chameleon::update_anim_state),
        );
    }
}

#[derive(Component)]
struct Mantis {
    state: State,
    /// Velocity (from the leap path while airborne: for the pose).
    vel: Vec2,
    facing: f32,
    cooldown: f32,
    retaliate: bool,
    introduced: bool,
    /// Walk cycle phase (radians).
    stride: f32,
    /// 1 right after landing, fading: squash for the pose.
    squash: f32,
    seed: f32,
    dismember: bool,
    arena: Arena,
    rng: Rng,
}

#[derive(Clone, Copy, PartialEq)]
enum State {
    /// Stalking (or going home, asleep).
    Stand,
    Taunt(f32),
    /// Scythes going up (`len`: how long this one takes); `combo` sweeps left.
    Raise { t: f32, len: f32, combo: u32 },
    Strike { t: f32, combo: u32, hit: bool },
    /// Coiled back before a lunge (or crouched before a pounce, `leap`).
    Coil { t: f32, leap: bool },
    Lunge { t: f32, dir: f32, hit: bool },
    /// Pouncing onto the chameleon: the scythes slam down on landing.
    Air { from: Vec2, to: Vec2, height: f32, time: f32, t: f32 },
    /// Scythes crossed: blows glance off; then the counter.
    Parry(f32),
    /// The opening.
    Recover(f32),
    Hurt(f32),
    Dead { t: f32, landed: bool },
}

impl Mantis {
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
    /// The long prothorax, carrying the head and the scythes.
    Neck,
    Head,
    Antenna { offset: f32 },
    /// The scythes: the spiny femur and the blade folding against it.
    Femur { near: bool },
    Tibia { near: bool },
    /// Middle (0) and hind (1) walking legs.
    Leg { pair: usize, near: bool },
    Wing,
    /// The hind wings fanned out in the threat display (hidden otherwise).
    Display,
}

/// A scythe wave skimming the floor (`BossStuff`).
#[derive(Component)]
struct Wave {
    vel: Vec2,
    age: f32,
    hit: bool,
    material: Handle<ColorMaterial>,
}

/// Spawns the mantis on the arena floor, facing left.
pub fn spawn(commands: &mut Commands, meshes: &mut Assets<Mesh>, materials: &mut Assets<ColorMaterial>, arena: &Arena) -> Entity {
    let shell = materials.add(SHELL);
    let dark = materials.add(DARK);
    let pale = materials.add(PALE);
    let blade = materials.add(BLADE);
    let eye = materials.add(EYE);
    let glint = materials.add(Color::WHITE);
    let wing = materials.add(ColorMaterial { color: WING, alpha_mode: AlphaMode2d::Blend, ..default() });
    let display = materials.add(ColorMaterial { color: DISPLAY, alpha_mode: AlphaMode2d::Blend, ..default() });
    let mats = vec![(shell.clone(), SHELL), (dark.clone(), DARK), (pale.clone(), PALE), (blade.clone(), BLADE)];

    let home = Vec2::new(arena.home.x, arena.floor + FOOT);
    let mantis = commands.spawn((Transform::from_translation(home.extend(12.0)), Visibility::default())).id();
    let mut b = Builder { commands, meshes, owner: mantis };
    let rig = b.bone(Vec3::ZERO, mantis, BoneKind::Rig);
    b.commands.entity(rig).insert(Transform::from_scale(Vec3::new(-SCALE, SCALE, 1.0)));

    // Walking legs: long and thin, knees up, feet on the floor.
    for near in [false, true] {
        let (mat, z, dx) = if near { (&shell, 0.15, 0.0) } else { (&dark, -0.4, 2.5) };
        for pair in 0..2 {
            let hip = [Vec2::new(-1.0, -2.0), Vec2::new(-6.0, -2.0)][pair];
            let bone = b.bone((hip + Vec2::X * dx).extend(z), rig, BoneKind::Leg { pair, near });
            let path = if pair == 0 {
                smooth_open(&[Vec2::ZERO, Vec2::new(5.0, 5.0), Vec2::new(8.0, -4.0), Vec2::new(10.0, -14.0)], 3)
            } else {
                smooth_open(&[Vec2::ZERO, Vec2::new(-8.0, 6.0), Vec2::new(-11.0, -4.0), Vec2::new(-15.0, -14.0)], 3)
            };
            b.part(stroke(&path, |t| 1.5 - 0.7 * t, 1.0, 0.8, |_| 1.0), mat, Vec3::ZERO, bone);
        }
    }

    // The threat display: hind wings fanned up behind, with a staring eyespot.
    let fan = b.bone(Vec3::new(-2.0, 5.0, -0.25), rig, BoneKind::Display);
    let fan_pts: Vec<Vec2> = std::iter::once(Vec2::ZERO).chain((0..=14).map(|i| Vec2::from_angle(1.15 + i as f32 / 14.0 * 1.8) * Vec2::new(36.0, 30.0))).collect();
    b.part(fill(&fan_pts, |p| 0.6 + 0.4 * (p.length() / 36.0)), &display, Vec3::ZERO, fan);
    b.part(fill(&ellipse_pts(5.0, 4.4, 16), |_| 0.9), &pale, Vec3::new(-12.0, 18.0, 0.01), fan);
    b.part(fill(&ellipse_pts(3.2, 2.8, 14), |_| 1.0), &eye, Vec3::new(-12.0, 18.0, 0.02), fan);
    b.commands.entity(fan).insert(Visibility::Hidden);

    // Abdomen: a long leaf, segmented, held a little off the floor.
    let belly = smooth_closed(
        &[
            Vec2::new(3.0, 3.0),
            Vec2::new(-6.0, 7.0),
            Vec2::new(-20.0, 8.5),
            Vec2::new(-32.0, 6.0),
            Vec2::new(-40.0, 2.5),
            Vec2::new(-41.0, 0.0),
            Vec2::new(-34.0, -2.5),
            Vec2::new(-20.0, -4.5),
            Vec2::new(-6.0, -4.5),
            Vec2::new(3.0, -3.0),
        ],
        6,
    );
    b.part(fill(&belly, |p| 0.55 + 0.45 * ((p.y + 4.5) / 13.0).clamp(0.0, 1.0)), &shell, Vec3::ZERO, rig);
    let rings: Vec<[Vec2; 3]> = (0..6)
        .map(|i| {
            let x = -8.0 - i as f32 * 5.2;
            let top = 7.5 - i as f32 * 0.9;
            [Vec2::new(x, top), Vec2::new(x - 1.1, top), Vec2::new(x - 0.5, -3.5 + i as f32 * 0.3)]
        })
        .collect();
    b.part(triangles(&rings), &dark, Vec3::new(0.0, 0.0, 0.01), rig);
    // The thorax joint under the prothorax.
    b.part(fill(&ellipse_pts(5.0, 3.4, 14), |p| 0.6 + 0.4 * ((p.y + 3.4) / 6.8)), &shell, Vec3::new(2.0, 0.5, 0.05), rig);

    // Wing covers folded along the back (they lift in the display and in the air).
    let wing_bone = b.bone(Vec3::new(1.0, 4.5, 0.3), rig, BoneKind::Wing);
    let tegmen = smooth_closed(&[Vec2::new(0.0, 1.2), Vec2::new(-16.0, 3.0), Vec2::new(-34.0, 2.0), Vec2::new(-40.0, -0.5), Vec2::new(-32.0, -2.0), Vec2::new(-14.0, -1.5), Vec2::new(0.0, -1.0)], 5);
    b.part(fill(&tegmen, |p| 0.75 + 0.25 * (p.y / 3.0)), &wing, Vec3::ZERO, wing_bone);
    let vein = smooth_open(&[Vec2::new(-2.0, 0.4), Vec2::new(-20.0, 1.2), Vec2::new(-37.0, -0.3)], 4);
    b.part(stroke(&vein, |_| 0.6, 1.0, 1.0, |_| 1.0), &dark, Vec3::new(0.0, 0.0, 0.01), wing_bone);

    // The prothorax: a long stalk rising from the thorax, a bulge where the scythes hang.
    let neck = b.bone(NECK_BASE.extend(0.2), rig, BoneKind::Neck);
    let stalk = smooth_closed(&[Vec2::new(-2.0, 0.0), Vec2::new(0.0, 2.6), Vec2::new(8.0, 2.2), Vec2::new(22.0, 1.8), Vec2::new(23.5, 0.0), Vec2::new(22.0, -1.8), Vec2::new(14.0, -2.6), Vec2::new(6.0, -2.8), Vec2::new(0.0, -2.8)], 5);
    b.part(fill(&stalk, |p| 0.6 + 0.4 * ((p.y + 2.8) / 5.4)), &shell, Vec3::ZERO, neck);
    b.part(fill(&ellipse_pts(3.4, 3.0, 12), |p| 0.6 + 0.4 * ((p.y + 3.0) / 6.0)), &dark, (SHOULDER + Vec2::new(0.0, 1.0)).extend(0.01), neck);

    // The scythes: a big spiny femur and the bone-white blade folding against it,
    // hooked at the tip.
    for near in [false, true] {
        let (mat, z, dx) = if near { (&shell, 0.5, 0.0) } else { (&dark, -0.45, 2.0) };
        let femur = b.bone((SHOULDER + Vec2::X * dx).extend(z), neck, BoneKind::Femur { near });
        let thigh = smooth_closed(&[Vec2::new(-1.5, 0.0), Vec2::new(0.0, 2.2), Vec2::new(7.0, 3.6), Vec2::new(15.0, 3.2), Vec2::new(22.0, 1.6), Vec2::new(23.0, 0.0), Vec2::new(22.0, -1.6), Vec2::new(15.0, -2.6), Vec2::new(7.0, -2.8), Vec2::new(0.0, -2.2)], 5);
        b.part(fill(&thigh, |p| 0.65 + 0.35 * ((p.y + 2.8) / 6.4)), mat, Vec3::ZERO, femur);
        let spines: Vec<[Vec2; 3]> = (0..7)
            .map(|i| {
                let x = 4.0 + i as f32 * 2.6;
                let h = if i % 2 == 0 { 2.4 } else { 1.5 };
                [Vec2::new(x - 0.7, -2.3), Vec2::new(x + 0.7, -2.3), Vec2::new(x + 0.3, -2.3 - h)]
            })
            .collect();
        b.part(triangles(&spines), &blade, Vec3::new(0.0, 0.0, 0.01), femur);
        let tibia = b.bone(Vec3::new(22.0, 0.0, 0.02), femur, BoneKind::Tibia { near });
        let edge = smooth_closed(&[Vec2::new(0.0, 1.5), Vec2::new(8.0, 1.4), Vec2::new(13.0, 0.9), Vec2::new(16.0, -0.6), Vec2::new(16.4, -3.4), Vec2::new(14.4, -1.4), Vec2::new(8.0, -1.2), Vec2::new(0.0, -1.5)], 4);
        b.part(fill(&edge, |p| 0.75 + 0.25 * ((p.y + 1.5) / 3.0)), if near { &blade } else { &pale }, Vec3::ZERO, tibia);
        let teeth: Vec<[Vec2; 3]> = (0..6)
            .map(|i| {
                let x = 2.0 + i as f32 * 2.0;
                [Vec2::new(x - 0.5, -1.2), Vec2::new(x + 0.5, -1.2), Vec2::new(x + 0.6, -2.8)]
            })
            .collect();
        b.part(triangles(&teeth), &blade, Vec3::new(0.0, 0.0, 0.01), tibia);
    }

    // The head: a wide triangle, two huge eyes at its corners, jaws at the point.
    let head = b.bone(Vec3::new(23.0, 0.0, 0.2), neck, BoneKind::Head);
    let face = smooth_closed(
        &[Vec2::new(-3.0, 1.0), Vec2::new(-2.0, 4.0), Vec2::new(2.0, 6.0), Vec2::new(6.0, 5.5), Vec2::new(8.5, 3.0), Vec2::new(7.0, -1.0), Vec2::new(4.0, -5.0), Vec2::new(2.0, -6.5), Vec2::new(0.0, -5.0), Vec2::new(-2.0, -2.0)],
        5,
    );
    b.part(fill(&face, |p| 0.6 + 0.4 * ((p.y + 6.5) / 12.5)), &shell, Vec3::ZERO, head);
    b.part(fill(&ellipse_pts(2.0, 2.6, 12), |_| 0.7), &eye, Vec3::new(0.0, 4.4, -0.01), head);
    let eye_pts: Vec<Vec2> = ellipse_pts(2.9, 3.5, 16).into_iter().map(|p| Vec2::from_angle(-0.4).rotate(p)).collect();
    b.part(fill(&eye_pts, |p| 0.8 + 0.2 * (p.y / 3.5)), &eye, Vec3::new(5.4, 3.0, 0.01), head);
    b.part(fill(&ellipse_pts(0.8, 1.0, 8), |_| 1.0), &glint, Vec3::new(6.2, 4.6, 0.02), head);
    b.part(triangles(&[[Vec2::new(1.5, -6.0), Vec2::new(4.0, -5.4), Vec2::new(3.2, -8.6)], [Vec2::new(0.0, -5.4), Vec2::new(2.0, -5.8), Vec2::new(0.6, -8.0)]]), &dark, Vec3::new(0.0, 0.0, 0.01), head);
    for (offset, z) in [(0.0, -0.05), (1.3, 0.03)] {
        let bone = b.bone(Vec3::new(3.5, 5.5, z), head, BoneKind::Antenna { offset });
        let path = smooth_open(&[Vec2::ZERO, Vec2::new(-3.0, 8.0), Vec2::new(-9.0, 15.0), Vec2::new(-18.0, 18.0)], 6);
        b.part(stroke(&path, |t| 1.1 - 0.8 * t, 1.0, 1.0, |_| 1.0), &dark, Vec3::ZERO, bone);
    }

    let mut rng = Rng(0x5C17_7E25);
    commands.entity(mantis).insert((
        Boss::new(&SPEC, home, mats).staged(),
        Mantis {
            state: State::Stand,
            vel: Vec2::ZERO,
            facing: -1.0,
            cooldown: rng.range(COOLDOWN.0, COOLDOWN.1),
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
    mantis
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

/// React to a hit (health and blood are handled in `boss.rs`). It doesn't
/// flinch out of an attack.
fn on_hit(mut hits: MessageReader<BossHit>, mut mantises: Query<&mut Mantis>) {
    for hit in hits.read() {
        let Ok(mut m) = mantises.get_mut(hit.boss) else { continue };
        if hit.killed {
            m.vel = Vec2::new(hit.dir.x * 340.0, 560.0);
            m.state = State::Dead { t: 0.0, landed: false };
            m.dismember = true;
        } else if hit.retaliate {
            m.retaliate = true;
        } else if hit.stagger && matches!(m.state, State::Stand | State::Taunt(_) | State::Recover(_)) {
            m.vel = Vec2::new(hit.dir.x * 240.0, 0.0);
            m.state = State::Hurt(0.0);
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
    let body = FOOT * 2.6 + 10.0;
    let mut h = 0.0;
    while h < want {
        if terrain.solid_at(at + Vec2::Y * (h + body)) {
            return (h - 10.0).max(20.0);
        }
        h += 8.0;
    }
    want
}

fn pounce(arena: &Arena, terrain: &Terrain, feet: Vec2, x: f32, height: f32) -> State {
    let to = landing(arena, terrain, feet, x);
    let height = headroom(terrain, (feet + to) * 0.5, height);
    State::Air { from: feet, to, height, time: POUNCE_TIME, t: 0.0 }
}

/// Its shoulders (where the scythes hang) in world space, for a body at `pos`.
fn shoulders(pos: Vec2, facing: f32) -> Vec2 {
    let at = NECK_BASE + Vec2::from_angle(NECK_REST).rotate(SHOULDER);
    pos + Vec2::new(at.x * facing, at.y) * SCALE
}

#[allow(clippy::too_many_arguments)]
fn think(
    time: Res<Time>,
    mut commands: Commands,
    mut shake: ResMut<Shake>,
    mut stop: ResMut<HitStop>,
    mut bleed: MessageWriter<Bleed>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    mut wave_mesh: Local<Option<Handle<Mesh>>>,
    player: Single<(&mut Player, &Transform), Without<Mantis>>,
    camera: Single<(&Camera, &GlobalTransform), With<MainCamera>>,
    mut mantises: Query<(&mut Mantis, &mut Boss, &mut Transform, Option<&mut boss::Sense>)>,
    terrain: Res<Terrain>,
) {
    let dt = time.delta_secs().min(1.0 / 30.0) * crate::boss::tempo();
    let (mut p, ptf) = player.into_inner();
    let ppos = ptf.translation.truncate();
    // Scythe waves to throw this frame: (from, direction).
    let mut throws: Vec<(Vec2, f32)> = Vec::new();

    for (mut m, mut boss, mut tf, sense) in &mut mantises {
        let pos = tf.translation.truncate();
        let to_player = ppos - pos;
        let alive = p.dead.is_none();
        let aggro = alive && boss.awake;
        let arena = m.arena;
        let ground_at = |x: f32, y: f32| terrain.surface(Vec2::new(x, y - FOOT + 2.0 * STEP_UP), 3000.0);
        let floor = ground_at(pos.x, pos.y).unwrap_or(arena.floor);
        let feet = Vec2::new(pos.x, floor);
        if let Some(mut s) = sense {
            s.goal = aggro.then_some(ppos);
            s.target = match m.state {
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
        if !m.introduced {
            boss.staged = true;
        }
        let enraged = boss.enraged();
        let pace = if enraged { RAGE_PACE } else { 1.0 };
        let dx = to_player.x.abs();
        // How high the chameleon's feet are above its floor.
        let above = ppos.y - player::HALF.y - floor;

        // Hit too many times in a row: it crosses its scythes and counters.
        if std::mem::take(&mut m.retaliate) && matches!(m.state, State::Stand | State::Hurt(_) | State::Recover(_) | State::Taunt(_) | State::Raise { .. }) {
            m.facing = side;
            m.vel = Vec2::ZERO;
            m.state = State::Parry(0.0);
        }

        // A sweep of the scythes in front of it: hurts the chameleon if it is there.
        let reap = |p: &mut Player, shake: &mut Shake, stop: &mut HitStop, bleed: &mut MessageWriter<Bleed>, facing: f32| {
            let shoulder = shoulders(pos, facing);
            let ahead = (ppos.x - pos.x) * facing;
            if alive && ahead > -RADIUS * 0.3 && ppos.distance(shoulder) < STRIKE_REACH + player::HALF.x + 12.0 && ppos.y < shoulder.y + 50.0 {
                boss::hurt_player(p, ppos, pos, STRIKE_DAMAGE, bleed, (shake, stop));
            }
        };

        m.state = match m.state {
            State::Dead { t: t0, landed } => {
                let td = t0 + dt;
                for at in [0.3, 0.7] {
                    if t0 < at && td >= at {
                        bleed.write(Bleed { pos, dir: Vec2::Y, spread: PI, speed: 520.0, amount: 60, fluid: Fluid::Cyan, sticky: true });
                    }
                }
                let mut landed = landed;
                let mut next = pos;
                if !landed {
                    m.vel.y -= 1400.0 * dt;
                    next += m.vel * dt;
                    let rest = floor + 6.0 * SCALE;
                    if ground_at(next.x, pos.y).is_some() && next.y <= rest && m.vel.y < 0.0 {
                        next.y = rest;
                        m.vel = Vec2::new(m.vel.x * 0.3, 0.0);
                        landed = true;
                        shake.add(0.25);
                        bleed.write(Bleed { pos, dir: Vec2::Y, spread: 1.3, speed: 240.0, amount: 40, fluid: Fluid::Cyan, sticky: true });
                    }
                } else {
                    m.vel.x *= 1.0 - 5.0 * dt;
                    next.x += m.vel.x * dt;
                }
                if td < 3.5 && m.rng.chance(dt * 12.0) {
                    bleed.write(Bleed { pos, dir: Vec2::Y, spread: 1.0, speed: 140.0, amount: 2, fluid: Fluid::Cyan, sticky: true });
                }
                tf.translation.x = next.x;
                tf.translation.y = next.y;
                State::Dead { t: td, landed }
            }
            State::Hurt(s) => {
                m.vel.x *= 1.0 - 6.0 * dt;
                if s < 0.22 { State::Hurt(s + dt) } else { State::Stand }
            }
            State::Stand => {
                m.cooldown -= dt;
                if !aggro {
                    // Asleep or given up: back home, slowly.
                    m.introduced = false;
                    let home = boss.home.x - pos.x;
                    if home.abs() > 30.0 {
                        m.facing = home.signum();
                        m.vel.x = m.facing * WALK * 0.5;
                    } else {
                        m.vel.x = 0.0;
                    }
                    State::Stand
                } else if !m.introduced {
                    m.vel.x = 0.0;
                    m.facing = side;
                    if on_screen {
                        m.introduced = true;
                        boss.staged = false;
                        shake.add(0.15);
                        State::Taunt(0.0)
                    } else {
                        State::Stand
                    }
                } else {
                    m.facing = side;
                    let walk = if enraged { RAGE_WALK } else { WALK };
                    // Stalks right up to it.
                    m.vel.x = if dx > STRIKE_RANGE * 0.7 { side * walk } else { 0.0 };
                    if m.cooldown > 0.0 {
                        State::Stand
                    } else if dx < STRIKE_RANGE && above < LEDGE {
                        m.cooldown = m.rng.range(COOLDOWN.0, COOLDOWN.1) * pace;
                        m.vel.x = 0.0;
                        State::Raise { t: 0.0, len: RAISE * pace.max(0.8), combo: if enraged { RAGE_COMBO } else { COMBO } }
                    } else if above >= LEDGE || dx > LUNGE_RANGE * 1.25 {
                        m.cooldown = m.rng.range(COOLDOWN.0, COOLDOWN.1) * pace;
                        m.vel.x = 0.0;
                        State::Coil { t: 0.0, leap: true }
                    } else if dx > LUNGE_MIN && dx < LUNGE_RANGE && m.rng.chance(LUNGE_SHARE) {
                        m.cooldown = m.rng.range(COOLDOWN.0, COOLDOWN.1) * pace;
                        m.vel.x = 0.0;
                        State::Coil { t: 0.0, leap: false }
                    } else {
                        // Walks in to reap; asks again in a moment.
                        m.cooldown = 0.25;
                        State::Stand
                    }
                }
            }
            State::Taunt(s) => {
                m.facing = side;
                m.vel.x = 0.0;
                if to_player.length() < TAUNT_BREAK {
                    State::Raise { t: RAISE * 0.5, len: RAISE, combo: 1 }
                } else if s >= TAUNT_TIME {
                    m.cooldown = m.rng.range(0.1, 0.3);
                    State::Stand
                } else {
                    State::Taunt(s + dt)
                }
            }
            State::Raise { t: s, len, combo } => {
                m.facing = side;
                m.vel.x = 0.0;
                if s >= len {
                    State::Strike { t: 0.0, combo, hit: false }
                } else {
                    State::Raise { t: s + dt, len, combo }
                }
            }
            State::Strike { t: s, combo, hit } => {
                let s1 = s + dt;
                let k = (s1 / STRIKE_TIME).min(1.0);
                // Lurches forward behind the blades.
                m.vel.x = m.facing * STRIKE_STEP / STRIKE_TIME * 2.0 * (1.0 - k);
                let mut hit = hit;
                if !hit && k >= STRIKE_HIT {
                    hit = true;
                    shake.add(0.12);
                    reap(&mut p, &mut shake, &mut stop, &mut bleed, m.facing);
                    if enraged && combo <= 1 {
                        throws.push((Vec2::new(pos.x + m.facing * 24.0 * SCALE, floor + 14.0), m.facing));
                    }
                }
                if k < 1.0 {
                    State::Strike { t: s1, combo, hit }
                } else if combo > 1 && alive {
                    State::Raise { t: 0.0, len: COMBO_RAISE * pace.max(0.8), combo: combo - 1 }
                } else {
                    m.vel.x = 0.0;
                    State::Recover(0.0)
                }
            }
            State::Coil { t: s, leap: high } => {
                m.facing = side;
                m.vel.x = 0.0;
                let len = COIL * pace.max(0.8);
                if s < len {
                    State::Coil { t: s + dt, leap: high }
                } else if high {
                    // Commits to where the chameleon is now.
                    shake.add(0.1);
                    let height = POUNCE_HEIGHT + above.max(0.0);
                    pounce(&arena, &terrain, feet, ppos.x, height)
                } else {
                    shake.add(0.08);
                    State::Lunge { t: 0.0, dir: side, hit: false }
                }
            }
            State::Lunge { t: s, dir, hit } => {
                let s1 = s + dt;
                let k = (s1 / LUNGE_TIME).min(1.0);
                m.facing = dir;
                m.vel.x = dir * LUNGE_SPEED * (1.0 - 0.6 * k * k);
                let mut hit = hit;
                // The scythes shoot out ahead of it and snap shut.
                let tip = pos + Vec2::new(dir * 38.0 * SCALE, 6.0 * SCALE);
                if !hit && alive && ppos.distance(tip) < 18.0 * SCALE + player::HALF.y {
                    hit = true;
                    stop.add(0.05);
                    boss::hurt_player(&mut p, ppos, pos, LUNGE_DAMAGE, &mut bleed, (&mut shake, &mut stop));
                }
                let blocked = (inside(&arena, pos.x + dir * 4.0) - (pos.x + dir * 4.0)).abs() > 0.5;
                if k < 1.0 && !blocked {
                    State::Lunge { t: s1, dir, hit }
                } else {
                    if enraged {
                        throws.push((Vec2::new(pos.x + dir * 30.0 * SCALE, floor + 14.0), dir));
                    }
                    m.vel.x *= 0.3;
                    State::Recover(0.0)
                }
            }
            State::Air { from, to, height, time, t: s } => {
                let s = s + dt;
                let k = (s / time).min(1.0);
                let next = Vec2::new(from.x + (to.x - from.x) * k, from.y + (to.y - from.y) * k + FOOT + height * 4.0 * k * (1.0 - k));
                m.vel = (next - pos) / dt.max(1e-4);
                if (to.x - from.x).abs() > 1.0 {
                    m.facing = (to.x - from.x).signum();
                }
                tf.translation.x = next.x;
                tf.translation.y = next.y;
                if k < 1.0 {
                    State::Air { from, to, height, time, t: s }
                } else {
                    m.squash = 1.0;
                    m.vel = Vec2::ZERO;
                    // The scythes slam down where it lands.
                    shake.add(0.3);
                    let landed_at = Vec2::new(to.x, to.y + FOOT);
                    if alive && ppos.distance(landed_at) < SLAM_REACH + player::HALF.x {
                        boss::hurt_player(&mut p, ppos, landed_at, SLAM_DAMAGE, &mut bleed, (&mut shake, &mut stop));
                    }
                    if enraged {
                        throws.push((Vec2::new(to.x + 20.0 * SCALE, to.y + 14.0), 1.0));
                        throws.push((Vec2::new(to.x - 20.0 * SCALE, to.y + 14.0), -1.0));
                    }
                    State::Recover(0.0)
                }
            }
            State::Parry(s) => {
                m.facing = side;
                m.vel.x = 0.0;
                if s >= PARRY {
                    // The counter: a sweep straight away.
                    State::Strike { t: 0.0, combo: 1, hit: false }
                } else {
                    State::Parry(s + dt)
                }
            }
            State::Recover(s) => {
                m.vel.x *= 1.0 - 8.0 * dt;
                if s >= RECOVER * if enraged { 0.7 } else { 1.0 } {
                    m.cooldown = m.cooldown.min(0.3 * pace);
                    State::Stand
                } else {
                    State::Recover(s + dt)
                }
            }
        };
        // Blows glance off the crossed scythes.
        boss.guard = matches!(m.state, State::Parry(_));

        // On the ground: follow the floor under it.
        if !m.airborne() && !matches!(m.state, State::Dead { .. }) {
            let x = inside(&arena, tf.translation.x + m.vel.x * dt);
            let floor = ground_at(x, tf.translation.y).unwrap_or(floor);
            tf.translation.x = x;
            tf.translation.y = floor + FOOT;
            m.stride += (m.vel.x.abs() * dt * 0.07).min(1.0);
        }
        m.squash = (m.squash - dt * 4.0).max(0.0);
    }

    for (at, dir) in throws {
        let mesh = wave_mesh.get_or_insert_with(|| meshes.add(crescent())).clone();
        let material = materials.add(ColorMaterial { color: WAVE_COLOR, alpha_mode: AlphaMode2d::Blend, ..default() });
        commands.spawn((
            Mesh2d(mesh),
            MeshMaterial2d(material.clone()),
            Transform::from_translation(at.extend(13.0)).with_scale(Vec3::new(dir, 1.0, 1.0)),
            camera::color_layer(),
            Wave { vel: Vec2::new(dir * WAVE_RANGE / WAVE_TIME, 0.0), age: 0.0, hit: false, material },
            BossStuff,
        ));
    }
}

/// A scythe-shaped crescent curving forward (+x), thick in the middle, sharp at the ends.
fn crescent() -> Mesh {
    let n = 16;
    let pts: Vec<Vec2> = (0..=n).map(|i| Vec2::from_angle(-1.2 + 2.4 * i as f32 / n as f32) * WAVE_RADIUS * 1.3).collect();
    let mut tris: Vec<[Vec2; 3]> = Vec::new();
    for i in 0..n {
        let (a, b) = (pts[i], pts[i + 1]);
        let w = |j: usize| 7.0 * (PI * j as f32 / n as f32).sin();
        let (na, nb) = (a.normalize() * w(i), b.normalize() * w(i + 1));
        tris.push([a - na, b - nb, b]);
        tris.push([a - na, b, a]);
    }
    triangles(&tris)
}

/// Scythe waves skim the floor, slowing and fading; they cut the chameleon once, and walls stop them.
fn waves(
    time: Res<Time>,
    mut commands: Commands,
    mut shake: ResMut<Shake>,
    mut stop: ResMut<HitStop>,
    mut bleed: MessageWriter<Bleed>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    terrain: Res<Terrain>,
    player: Single<(&mut Player, &Transform), Without<Wave>>,
    mut waves: Query<(Entity, &mut Wave, &mut Transform), Without<Player>>,
) {
    let dt = time.delta_secs().min(1.0 / 30.0);
    let (mut p, ptf) = player.into_inner();
    let ppos = ptf.translation.truncate();
    for (e, mut w, mut tf) in &mut waves {
        w.age += dt;
        let k = w.age / WAVE_TIME;
        let pos = tf.translation.truncate();
        if k >= 1.0 || terrain.solid_at(pos + w.vel.normalize_or_zero() * WAVE_RADIUS) {
            commands.entity(e).despawn();
            continue;
        }
        tf.translation += (w.vel * dt * 1.6 * (1.0 - 0.6 * k)).extend(0.0);
        let grow = 1.0 + 0.3 * k;
        tf.scale = Vec3::new(w.vel.x.signum() * grow, grow, 1.0);
        if !w.hit && p.dead.is_none() && pos.distance(ppos) < WAVE_RADIUS + player::HALF.y {
            w.hit = true;
            boss::hurt_player(&mut p, ppos, pos, WAVE_DAMAGE, &mut bleed, (&mut shake, &mut stop));
        }
        if let Some(mut m) = materials.get_mut(&w.material) {
            m.color = WAVE_COLOR.with_alpha(0.9 * (1.0 - k * k));
        }
    }
}

/// The chameleon came back: start the fight over (`boss.rs` heals it).
fn reset_on_respawn(mut commands: Commands, mut respawned: MessageReader<Respawned>, mut mantises: Query<(&mut Mantis, &mut Boss)>, waves: Query<Entity, With<Wave>>) {
    if respawned.read().count() == 0 {
        return;
    }
    for e in &waves {
        commands.entity(e).despawn();
    }
    for (mut m, mut boss) in &mut mantises {
        if matches!(m.state, State::Dead { .. }) {
            continue;
        }
        m.state = State::Stand;
        m.introduced = false;
        m.vel = Vec2::ZERO;
        m.cooldown = 1.0;
        boss.guard = false;
    }
}

/// A freshly killed mantis: the head and the near scythe tear off.
fn dismember(mut commands: Commands, mut mantises: Query<&mut Mantis>, bones: Query<(Entity, &Bone, &GlobalTransform)>) {
    let mut done = Vec::new();
    for (e, bone, gt) in &bones {
        let Ok(m) = mantises.get(bone.owner) else { continue };
        if !m.dismember {
            continue;
        }
        done.push(bone.owner);
        let f = m.facing;
        let (vel, spin, bleeds) = match bone.kind {
            BoneKind::Femur { near: true } => (Vec2::new(-f * 260.0, 620.0), 9.0, Some(Fluid::Cyan)),
            BoneKind::Head => (Vec2::new(f * 220.0, 700.0), -12.0, Some(Fluid::Cyan)),
            _ => continue,
        };
        commands.entity(e).remove::<Bone>();
        boss::tear_off(&mut commands, e, gt, vel, spin, bleeds);
    }
    done.sort();
    done.dedup();
    for owner in done {
        if let Ok(mut m) = mantises.get_mut(owner) {
            m.dismember = false;
        }
    }
}

/// Where the body is held this frame (angles in the rig frame, the scythes'
/// femurs as angles from the rig's +x, the blades relative to their femur).
struct Pose {
    neck: f32,
    head: f32,
    femur: [f32; 2],
    tibia: [f32; 2],
    /// Body tilt (+ = nose up) and how low it crouches (rig units).
    lean: f32,
    low: f32,
    /// Wing covers lifted (0..1) and the display fanned out.
    wings: f32,
    display: bool,
}

fn ease(k: f32) -> f32 {
    let k = k.clamp(0.0, 1.0);
    k * k * (3.0 - 2.0 * k)
}

fn pose(m: &Mantis, tt: f32) -> Pose {
    let rest = Pose {
        neck: NECK_REST + (tt * 1.6).sin() * 0.03,
        head: -0.15 + (tt * 2.3).sin() * 0.05,
        femur: [FEMUR_REST + (tt * 2.0).sin() * 0.04; 2],
        tibia: [TIBIA_FOLD; 2],
        lean: (tt * 1.3).sin() * 0.01,
        low: 0.0,
        wings: 0.0,
        display: false,
    };
    match m.state {
        State::Stand => {
            // Stalking: the prothorax sways side to side, the head locked on.
            let walking = (m.vel.x.abs() / WALK).min(1.0);
            Pose { neck: rest.neck + walking * (m.stride * 2.0).sin() * 0.06, low: walking * 1.0, ..rest }
        }
        State::Taunt(s) => {
            // The threat display: reared up, scythes spread wide, wings fanned.
            let k = ease(s / 0.35);
            let rock = (tt * 5.0).sin() * 0.05 * k;
            Pose {
                neck: NECK_REST + 0.35 * k + rock,
                head: -0.15 + (tt * 9.0).sin() * 0.12 * k,
                femur: [FEMUR_REST + (0.9 - FEMUR_REST) * k, FEMUR_REST + (1.45 - FEMUR_REST) * k],
                tibia: [TIBIA_FOLD + (-1.2 - TIBIA_FOLD) * k; 2],
                lean: -0.1 * k + rock * 0.5,
                low: 0.0,
                wings: k,
                display: k > 0.3,
            }
        }
        State::Raise { t, len, .. } => {
            let k = ease(t / len);
            let quiver = (tt * 60.0).sin() * 0.04 * k;
            Pose {
                neck: NECK_REST + 0.25 * k,
                head: -0.15 - 0.1 * k,
                femur: [FEMUR_REST + (FEMUR_RAISED - FEMUR_REST) * k + quiver, FEMUR_REST + (FEMUR_RAISED + 0.15 - FEMUR_REST) * k - quiver],
                tibia: [TIBIA_FOLD + (TIBIA_OPEN - TIBIA_FOLD) * k; 2],
                lean: -0.12 * k,
                ..rest
            }
        }
        State::Strike { t, .. } => {
            // The near blade a beat ahead of the far one: a scissor sweep.
            let sweep = |lag: f32| {
                let k = ((t / STRIKE_TIME) - lag).clamp(0.0, 1.0);
                let femur = FEMUR_RAISED + (-0.9 - FEMUR_RAISED) * (k * k * (2.0 - k));
                // Opens wide on the way down, snaps shut at the end.
                let tibia = if k < 0.6 { TIBIA_OPEN + (-0.5 - TIBIA_OPEN) * (k / 0.6) } else { -0.5 + (TIBIA_FOLD + 0.4 + 0.5) * ((k - 0.6) / 0.4) };
                (femur, tibia)
            };
            let (fn_, tn) = sweep(0.0);
            let (ff, tf) = sweep(0.2);
            let k = (t / STRIKE_TIME).min(1.0);
            Pose { neck: NECK_REST + 0.25 - 0.55 * k, head: -0.25, femur: [fn_, ff], tibia: [tn, tf], lean: 0.18 * k, low: 1.5 * k, ..rest }
        }
        State::Coil { t, leap } => {
            let k = ease(t / COIL);
            let quiver = (tt * 50.0).sin() * 0.03 * k;
            if leap {
                Pose { low: 5.0 * k, lean: 0.12 * k + quiver, wings: 0.3 * k, ..rest }
            } else {
                // Prothorax pulled back and low, blades cocked by the head.
                Pose {
                    neck: NECK_REST - 0.35 * k + quiver,
                    head: -0.1,
                    femur: [FEMUR_REST + (0.9 - FEMUR_REST) * k; 2],
                    tibia: [TIBIA_FOLD; 2],
                    lean: -0.1 * k,
                    low: 4.0 * k,
                    ..rest
                }
            }
        }
        State::Lunge { t, .. } => {
            let k = (t / LUNGE_TIME).min(1.0);
            // Straight out, blades open, then they snap shut.
            let tibia = if k < 0.7 { -0.35 } else { -0.35 + (TIBIA_FOLD + 0.35) * ((k - 0.7) / 0.3) };
            Pose { neck: 0.35, head: -0.05, femur: [0.05, 0.15], tibia: [tibia; 2], lean: 0.18, low: 2.0, ..rest }
        }
        State::Air { .. } => {
            // Wings spread, scythes up, then brought down on the way down.
            let falling = m.vel.y < 0.0;
            Pose {
                neck: NECK_REST + 0.2,
                head: -0.3,
                femur: if falling { [-0.6, -0.4] } else { [FEMUR_RAISED; 2] },
                tibia: [-0.7; 2],
                lean: (m.vel.y * 0.0005).clamp(-0.35, 0.35),
                low: 0.0,
                wings: 1.0,
                display: true,
            }
        }
        State::Parry(s) => {
            // Scythes crossed in front of the face.
            let k = ease(s / 0.12);
            Pose {
                neck: NECK_REST + 0.15 * k,
                head: -0.2,
                femur: [FEMUR_REST + (0.25 - FEMUR_REST) * k, FEMUR_REST + (0.65 - FEMUR_REST) * k],
                tibia: [TIBIA_FOLD + 0.3 * k; 2],
                lean: -0.05 * k,
                ..rest
            }
        }
        State::Recover(s) => {
            // Spent: slumped forward, blades hanging half open.
            let k = ease(s / 0.2) * (1.0 - ease((s - RECOVER + 0.2) / 0.2));
            let sway = (tt * 4.0).sin() * 0.04;
            Pose {
                neck: NECK_REST - 0.35 * k + sway,
                head: 0.1 * k,
                femur: [FEMUR_REST - 0.2 * k; 2],
                tibia: [TIBIA_FOLD + 0.6 * k; 2],
                lean: 0.12 * k,
                low: 2.0 * k,
                ..rest
            }
        }
        State::Hurt(s) => {
            let k = 1.0 - (s / 0.22).min(1.0);
            Pose { neck: NECK_REST - 0.2 * k, lean: -0.18 * k, ..rest }
        }
        State::Dead { t, .. } => {
            let fade = (1.0 - t / 6.0).max(0.1);
            Pose {
                neck: 0.4,
                head: 0.0,
                femur: [-0.3 + (tt * 21.0).sin() * 0.4 * fade, -0.2 + (tt * 17.0).sin() * 0.4 * fade],
                tibia: [-1.5 + (tt * 25.0).sin() * 0.5 * fade; 2],
                lean: 0.0,
                low: 0.0,
                wings: 0.2,
                display: false,
            }
        }
    }
}

fn animate(time: Res<Time>, mantises: Query<&Mantis>, mut bones: Query<(&Bone, &mut Transform, &mut Visibility)>) {
    let t = time.elapsed_secs();
    let dt = time.delta_secs();
    for (bone, mut tf, mut vis) in &mut bones {
        let Ok(m) = mantises.get(bone.owner) else { continue };
        let tt = t + m.seed;
        let pose = pose(m, tt);
        let (dead_t, landed) = match m.state {
            State::Dead { t, landed } => (Some(t), landed),
            _ => (None, false),
        };
        let walking = (m.vel.x.abs() / WALK).min(1.0) * if m.airborne() { 0.0 } else { 1.0 };
        match bone.kind {
            BoneKind::Rig => {
                tf.scale.x += (m.facing * SCALE - tf.scale.x) * (1.0 - (-18.0 * dt).exp());
                tf.scale.y = SCALE * (1.0 - 0.08 * m.squash);
                tf.translation.y = -(pose.low + 3.0 * m.squash) * SCALE;
                let angle = match dead_t {
                    Some(td) => {
                        if landed { PI } else { PI * (td * 2.4).min(1.0) }
                    }
                    None => pose.lean * m.facing,
                };
                tf.rotation = Quat::from_rotation_z(angle);
            }
            BoneKind::Neck => tf.rotation = Quat::from_rotation_z(pose.neck),
            BoneKind::Head => tf.rotation = Quat::from_rotation_z(pose.head - pose.neck),
            BoneKind::Antenna { offset } => {
                let angle = match m.state {
                    _ if dead_t.is_some() => -0.5 + (tt * 10.0 + offset).sin() * 0.05,
                    State::Air { .. } | State::Lunge { .. } => -0.5,
                    _ => (tt * 2.4 + offset).sin() * 0.12,
                };
                tf.rotation = Quat::from_rotation_z(angle);
            }
            BoneKind::Femur { near } => tf.rotation = Quat::from_rotation_z(pose.femur[(!near) as usize] - pose.neck),
            BoneKind::Tibia { near } => tf.rotation = Quat::from_rotation_z(pose.tibia[(!near) as usize]),
            BoneKind::Leg { pair, near } => {
                let group = if near { 0.0 } else { PI } + pair as f32 * PI * 0.5;
                let angle = if let Some(td) = dead_t {
                    let fade = (1.0 - td / 6.0).max(0.15);
                    (tt * 26.0 + pair as f32 * 1.7 + group).sin() * 0.5 * fade
                } else if m.airborne() {
                    // Reaching out for the landing.
                    if pair == 0 { 0.5 } else { -0.4 }
                } else {
                    walking * 0.35 * (m.stride * 2.0 + group).sin()
                };
                tf.rotation = Quat::from_rotation_z(angle);
            }
            BoneKind::Wing => {
                let flutter = if m.airborne() { (tt * 50.0).sin() * 0.06 } else { 0.0 };
                tf.rotation = Quat::from_rotation_z(0.55 * pose.wings + flutter);
            }
            BoneKind::Display => {
                *vis = if pose.display { Visibility::Inherited } else { Visibility::Hidden };
                let open = (pose.wings * 1.2).min(1.0);
                tf.rotation = Quat::from_rotation_z(-0.4 * (1.0 - open) + if m.airborne() { (tt * 60.0).sin() * 0.15 } else { (tt * 6.0).sin() * 0.03 });
                tf.scale = Vec3::new(1.0, 0.4 + 0.6 * open, 1.0);
            }
        }
    }
}
