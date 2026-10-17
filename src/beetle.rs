//! The BattleTank Beetle: the last boss, a giant armored rhinoceros beetle
//! (about eight times the chameleon) holding the military bunker.
//!
//! Its armor is too strong for the claws: slashes, the air spin and spit all
//! **glance off** (sparks, `Boss::shell`). Only the crab's **power blow**
//! struck at its **back** (the pale abdomen under the rear of the elytra)
//! breaks through. It stands high on its legs, so the chameleon can run under its
//! belly to get behind it; heavy and slow, it takes a while to notice and to
//! **turn around** (`TURN_DELAY`, `TURN_TIME`): that's the window.
//!
//! Attacks: the **horn toss** close in front (`Lower`: the head dips, the tell
//! → `Toss`: the horn sweeps up and throws the chameleon), the **charge**
//! (`Rev`: legs pawing, horn low → `Charge` along the floor: the horn hurts
//! below `HORN_HIGH`, so a well-timed jump clears it and lands behind it;
//! rammed into the arena's end it is dazed, `Daze`), **mortar shells** from
//! afar (`Aim`: head up, a glow in the mouth → shells lobbed where the
//! chameleon stands, `SHELLS` / enraged `RAGE_SHELLS`), and the **quake**
//! (`Rear`: the front rises → `Slam`: a shockwave along the floor, jump to
//! dodge) when the chameleon lingers under or behind it, or after a flurry
//! of hits. Face-off: rears up, horn high, legs drumming.
//!
//! It bleeds orange `#ff8100` and gives the chameleon the **shield** (G: two
//! seconds untouchable, `SHIELD_COOLDOWN`; drawn here, `shield`). On death it
//! is thrown on its back, the horn and a leg torn off, the legs twitching.
//! Health, getting hit and the death spectacle live in `boss.rs`.

use std::f32::consts::PI;

use bevy::{prelude::*, sprite_render::AlphaMode2d};

use crate::{
    boss::{self, Arena, Boss, BossHit, BossSpec, BossStuff, Defeated, Glanced},
    camera::{self, HitStop, MainCamera, Shake},
    chameleon,
    fumes::Cloud,
    gore::{Bleed, Fluid},
    level::Restart,
    player::{self, Player, Respawned},
    shapes::*,
    terrain::{Terrain, STEP_UP},
};

/// Its color: what the chameleon earns, its blood, the health bar.
pub const ORANGE: Color = Color::srgb(1.0, 0x81 as f32 / 255.0, 0.0);

pub(crate) static SPEC: BossSpec = BossSpec {
    name: "THE BATTLETANK BEETLE",
    hp: 20,
    radius: RADIUS,
    blood: Fluid::Orange,
    color: ORANGE,
    aggro: AGGRO,
    // A tank's worth of meat under the armor.
    food: 8.0,
};

/// Size of the boss relative to its drawing (about eight times the chameleon).
const SCALE: f32 = 6.5;
/// Hurtbox radius around the body center.
const RADIUS: f32 = 25.0 * SCALE;
/// Body center height above the floor when standing.
const FOOT: f32 = 16.0 * SCALE;
/// Half its length: how close to the arena edges its center may go.
const HALF_W: f32 = 28.0 * SCALE;
const AGGRO: f32 = 2200.0;
/// How far inside the screen edge (px) it must be to count as in view.
const SCREEN_MARGIN: f32 = 60.0;

/// The face-off: rears up, horn high.
const TAUNT_TIME: f32 = 2.0;
const TAUNT_BREAK: f32 = 300.0;

/// Its plod.
const WALK: f32 = 70.0;
/// Seconds between attacks (enraged: × `RAGE_PACE`).
const COOLDOWN: (f32, f32) = (1.0, 1.8);
const RAGE_PACE: f32 = 0.7;

// Turning around: it notices the chameleon behind it after `TURN_DELAY`, then turns for `TURN_TIME`.
const TURN_DELAY: f32 = 0.7;
const RAGE_TURN_DELAY: f32 = 0.4;
const TURN_TIME: f32 = 1.4;
const RAGE_TURN_TIME: f32 = 1.0;

// The horn toss (close, in front).
const TOSS_RANGE: f32 = 38.0 * SCALE;
const LOWER: f32 = 0.6;
const TOSS_TIME: f32 = 0.35;
const TOSS_DAMAGE: i32 = 2;
/// Up to this high above the floor the sweeping horn catches the chameleon.
const TOSS_HIGH: f32 = 32.0 * SCALE;

// The charge.
const REV: f32 = 1.0;
const RAGE_REV: f32 = 0.75;
const CHARGE_SPEED: f32 = 640.0;
const CHARGE_MAX: f32 = 2.6;
/// It stops this far past the chameleon (skidding).
const CHARGE_PAST: f32 = 520.0;
const CHARGE_DAMAGE: i32 = 2;
const CHARGE_MIN: f32 = 420.0;
const CHARGE_SHARE: f32 = 0.5;
/// The lowered horn: from this far ahead of its center to its tip, and up to this high above the floor.
const HORN_FROM: f32 = 16.0 * SCALE;
const HORN_TIP: f32 = 37.0 * SCALE;
const HORN_HIGH: f32 = 15.0 * SCALE;
/// Rammed into the end of its floor: dazed (the big opening).
const DAZE: f32 = 2.4;
const SKID: f32 = 0.7;

// The mortar (far).
const AIM: f32 = 0.9;
const RAGE_AIM: f32 = 0.7;
const SHELL_TIME: f32 = 1.2;
const SHELLS: usize = 1;
const RAGE_SHELLS: usize = 3;
const SHELL_SPREAD: f32 = 170.0;
const BLAST: f32 = 85.0;
const SHELL_DAMAGE: i32 = 1;
const MORTAR_MIN: f32 = 380.0;
const GRAVITY: f32 = 1400.0;

// The quake.
const REAR: f32 = 0.75;
const QUAKE_REACH: f32 = 720.0;
const QUAKE_DAMAGE: i32 = 1;
const QUAKE_STUN: f32 = 0.5;
/// The chameleon lingering under its belly this long sets it off.
const UNDER_PATIENCE: f32 = 1.4;
const SLAM: f32 = 0.5;

const ARMOR: Color = Color::srgb(0.34, 0.33, 0.32);
const PLATE: Color = Color::srgb(0.46, 0.44, 0.42);
const DARK: Color = Color::srgb(0.17, 0.16, 0.16);
const SOFT: Color = Color::srgb(0.70, 0.66, 0.60);
const EYE: Color = Color::srgb(0.05, 0.05, 0.05);
/// The mouth's glow before a shell (color layer).
const GLOW: Color = Color::srgba(1.0, 0.55, 0.1, 0.0);

/// The shield it gives: this long untouchable, then this long before it can be raised again.
pub const SHIELD_TIME: f32 = 2.0;
pub const SHIELD_COOLDOWN: f32 = 20.0;

pub struct BeetlePlugin;

impl Plugin for BeetlePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Shield>().add_systems(
            Update,
            (reset_on_respawn, on_hit, think, shells, glows, sparks, dust, dismember, animate, shield)
                .chain()
                .after(player::move_player)
                .after(chameleon::update_anim_state)
                .after(boss::slash_bosses),
        );
    }
}

#[derive(Component)]
struct Beetle {
    state: State,
    vel: Vec2,
    facing: f32,
    cooldown: f32,
    /// Seconds the chameleon has been behind it (it turns once it's had enough).
    behind: f32,
    /// Seconds the chameleon has been under its belly.
    under: f32,
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
enum State {
    Stand,
    Taunt(f32),
    /// Turning around (it faces the other way once done).
    Turn(f32),
    Lower(f32),
    Toss { t: f32, hit: bool },
    Rev(f32),
    Charge { t: f32, dir: f32, hit: bool },
    Skid(f32),
    Daze(f32),
    Aim(f32),
    Rear(f32),
    Slam(f32),
    Hurt(f32),
    Dead { t: f32, landed: bool },
}

#[derive(Component)]
struct Bone {
    owner: Entity,
    kind: BoneKind,
}

#[derive(Clone, Copy, PartialEq)]
enum BoneKind {
    Rig,
    /// Head and its great horn.
    Head,
    Elytra,
    /// Legs, 0 (front) to 2 (back).
    Leg { pair: usize, near: bool },
}

/// The glow in its mouth before it fires (color layer).
#[derive(Component)]
struct MouthGlow {
    owner: Entity,
    material: Handle<ColorMaterial>,
}

/// A mortar shell, lobbed in an arc (`BossStuff`, color layer).
#[derive(Component)]
struct Shell {
    vel: Vec2,
    life: f32,
}

/// A shell's blast: a ring that grows and fades (color layer).
#[derive(Component)]
struct Blast {
    age: f32,
    material: Handle<ColorMaterial>,
}

/// Sparks off the armor when a blow glances (color layer).
#[derive(Component)]
struct Spark {
    vel: Vec2,
    age: f32,
    life: f32,
    material: Handle<ColorMaterial>,
}

/// Dust kicked along the floor by the quake and the charge.
#[derive(Component)]
struct Dust {
    vel: Vec2,
    age: f32,
    life: f32,
    size: f32,
    material: Handle<ColorMaterial>,
}

/// Spawns the beetle on its floor, facing left.
pub fn spawn(commands: &mut Commands, meshes: &mut Assets<Mesh>, materials: &mut Assets<ColorMaterial>, arena: &Arena) -> Entity {
    let armor = materials.add(ARMOR);
    let plate = materials.add(PLATE);
    let dark = materials.add(DARK);
    let soft = materials.add(SOFT);
    let eye = materials.add(EYE);
    let glint = materials.add(Color::WHITE);
    let mats = vec![(armor.clone(), ARMOR), (plate.clone(), PLATE), (dark.clone(), DARK), (soft.clone(), SOFT)];

    let home = Vec2::new(arena.home.x, arena.floor + FOOT);
    let beetle = commands.spawn((Transform::from_translation(home.extend(12.0)), Visibility::default())).id();
    let mut b = Builder { commands, meshes, owner: beetle };
    let rig = b.bone(Vec3::ZERO, beetle, BoneKind::Rig);
    b.commands.entity(rig).insert(Transform::from_scale(Vec3::new(-SCALE, SCALE, 1.0)));

    // Legs: three a side, long and spiny, the feet on the floor (y = -16).
    let hips = [Vec2::new(13.0, -3.0), Vec2::new(2.0, -4.0), Vec2::new(-11.0, -3.5)];
    let feet = [Vec2::new(27.0, -16.0), Vec2::new(4.0, -16.0), Vec2::new(-26.0, -16.0)];
    for near in [false, true] {
        let (mat, z, dx) = if near { (&armor, 0.3, 0.0) } else { (&dark, -0.4, 3.0) };
        for pair in 0..3 {
            let bone = b.bone((hips[pair] + Vec2::X * dx).extend(z + pair as f32 * 0.01), rig, BoneKind::Leg { pair, near });
            let foot = feet[pair] - hips[pair];
            let knee = Vec2::new(foot.x * 0.6, 5.0);
            let path = smooth_open(&[Vec2::ZERO, knee, foot], 4);
            b.part(stroke(&path, |t| 4.2 - 2.8 * t, 1.0, 0.7, |_| 1.0), mat, Vec3::ZERO, bone);
            // Spurs along the tibia.
            let spurs: Vec<[Vec2; 3]> = (1..4)
                .map(|i| {
                    let p = knee.lerp(foot, i as f32 * 0.22);
                    let d = (foot - knee).normalize();
                    let out = d.perp() * if foot.x > 0.0 { 1.0 } else { -1.0 };
                    [p - d * 0.7, p + d * 0.7, p + out * 2.2 - d * 0.6]
                })
                .collect();
            b.part(triangles(&spurs), &dark, Vec3::new(0.0, 0.0, 0.01), bone);
        }
    }

    // The soft rear (the weak spot): pale, segmented, under the end of the elytra.
    let belly = smooth_closed(&[Vec2::new(-28.5, 1.0), Vec2::new(-27.0, -3.5), Vec2::new(-20.0, -6.0), Vec2::new(-12.0, -5.0), Vec2::new(-14.0, 0.0), Vec2::new(-22.0, 2.5)], 4);
    b.part(fill(&belly, |p| 0.75 + 0.25 * ((p.y + 6.0) / 8.5)), &soft, Vec3::new(0.0, 0.0, 0.0), rig);
    for x in [-25.0, -21.5, -18.0, -14.5] {
        b.part(stroke(&[Vec2::new(x, 1.5), Vec2::new(x + 0.6, -5.2)], |_| 0.45, 1.0, 1.0, |_| 1.0), &dark, Vec3::new(0.0, 0.0, 0.01), rig);
    }

    // The elytra: a riveted armored dome over the back.
    let elytra = b.bone(Vec3::new(0.0, 0.0, 0.1), rig, BoneKind::Elytra);
    let dome = smooth_closed(
        &[
            Vec2::new(-28.0, 3.0),
            Vec2::new(-26.0, 10.0),
            Vec2::new(-17.0, 16.0),
            Vec2::new(-3.0, 18.0),
            Vec2::new(8.0, 15.5),
            Vec2::new(12.0, 9.0),
            Vec2::new(11.5, 1.0),
            Vec2::new(4.0, -3.0),
            Vec2::new(-10.0, -3.5),
            Vec2::new(-22.0, -0.5),
        ],
        6,
    );
    b.part(fill(&dome, |p| 0.45 + 0.55 * ((p.y + 3.5) / 21.5).clamp(0.0, 1.0)), &armor, Vec3::ZERO, elytra);
    // Plate seams, a heavy rim and rivets.
    for (top, bottom) in [(Vec2::new(-18.0, 15.5), Vec2::new(-20.0, -1.5)), (Vec2::new(-8.0, 17.6), Vec2::new(-9.0, -3.3)), (Vec2::new(2.0, 17.0), Vec2::new(2.5, -2.4))] {
        let mid = (top + bottom) * 0.5 + Vec2::new(-1.2, 0.0);
        b.part(stroke(&smooth_open(&[top, mid, bottom], 3), |_| 0.7, 1.0, 1.0, |_| 1.0), &dark, Vec3::new(0.0, 0.0, 0.02), elytra);
    }
    let rim = smooth_open(&[Vec2::new(-27.5, 2.0), Vec2::new(-20.0, -1.0), Vec2::new(-8.0, -3.0), Vec2::new(4.0, -2.5), Vec2::new(11.5, 1.0)], 4);
    b.part(stroke(&rim, |_| 1.6, 0.8, 1.0, |_| 1.0), &plate, Vec3::new(0.0, 0.0, 0.02), elytra);
    let rivets: Vec<Vec<Vec2>> = [
        Vec2::new(-21.0, 11.0),
        Vec2::new(-22.0, 4.0),
        Vec2::new(-11.0, 14.0),
        Vec2::new(-12.0, 6.0),
        Vec2::new(-1.0, 15.0),
        Vec2::new(-1.5, 7.0),
        Vec2::new(6.0, 11.0),
        Vec2::new(6.5, 4.0),
        Vec2::new(-16.0, -0.5),
        Vec2::new(-4.0, -1.5),
    ]
    .iter()
    .map(|&c| ellipse_pts(0.75, 0.75, 8).into_iter().map(|p| p + c).collect())
    .collect();
    b.part(blobs(&rivets, |_| 0.9), &plate, Vec3::new(0.0, 0.0, 0.03), elytra);
    // A sheen along the top of the dome.
    let sheen = smooth_open(&[Vec2::new(-20.0, 13.0), Vec2::new(-9.0, 16.4), Vec2::new(2.0, 16.0)], 4);
    b.part(stroke(&sheen, |t| 1.2 * (1.0 - (2.0 * t - 1.0).powi(2)) + 0.1, 1.0, 1.0, |_| 1.0), &plate, Vec3::new(0.0, 0.0, 0.03), elytra);

    // The pronotum: the turret-like shield over the thorax, with its own forward horn.
    let pronotum = smooth_closed(&[Vec2::new(10.0, -2.0), Vec2::new(9.0, 8.0), Vec2::new(13.0, 13.5), Vec2::new(20.0, 12.5), Vec2::new(24.0, 7.0), Vec2::new(24.0, 0.0), Vec2::new(18.0, -3.0)], 5);
    b.part(fill(&pronotum, |p| 0.5 + 0.5 * ((p.y + 3.0) / 16.5).clamp(0.0, 1.0)), &armor, Vec3::new(0.0, 0.0, 0.2), rig);
    let crest = smooth_open(&[Vec2::new(16.0, 12.5), Vec2::new(23.0, 15.5), Vec2::new(29.0, 16.0), Vec2::new(32.0, 14.0)], 4);
    b.part(stroke(&crest, |t| 3.6 - 3.0 * t, 1.0, 0.7, |_| 1.0), &armor, Vec3::new(0.0, 0.0, 0.21), rig);
    let plate_rivets: Vec<Vec<Vec2>> = [Vec2::new(13.0, 9.0), Vec2::new(18.0, 9.5), Vec2::new(21.5, 5.0), Vec2::new(14.0, 2.0)]
        .iter()
        .map(|&c| ellipse_pts(0.7, 0.7, 8).into_iter().map(|p| p + c).collect())
        .collect();
    b.part(blobs(&plate_rivets, |_| 0.9), &plate, Vec3::new(0.0, 0.0, 0.22), rig);

    // The head with the great horn, pivoting at the neck.
    let head = b.bone(Vec3::new(22.0, 1.0, 0.25), rig, BoneKind::Head);
    b.part(fill(&ellipse_pts(4.6, 4.0, 16).into_iter().map(|p| p + Vec2::new(3.0, 0.0)).collect::<Vec<_>>(), |p| 0.6 + 0.4 * ((p.y + 4.0) / 8.0)), &armor, Vec3::ZERO, head);
    let horn = smooth_open(&[Vec2::new(4.0, 1.5), Vec2::new(9.0, 3.5), Vec2::new(13.0, 8.0), Vec2::new(15.0, 14.0), Vec2::new(13.5, 17.0)], 5);
    b.part(stroke(&horn, |t| 5.0 - 4.4 * t, 1.0, 0.65, |_| 1.0), &armor, Vec3::new(0.0, 0.0, 0.01), head);
    b.part(stroke(&horn[horn.len() - 4..], |t| 1.6 - 1.2 * t, 1.0, 1.0, |_| 1.0), &dark, Vec3::new(0.0, 0.0, 0.02), head);
    // Mandibles and a small eye.
    b.part(triangles(&[[Vec2::new(5.5, -1.5), Vec2::new(9.0, -2.5), Vec2::new(6.0, -3.8)], [Vec2::new(4.0, -2.5), Vec2::new(7.5, -4.0), Vec2::new(4.5, -4.5)]]), &dark, Vec3::new(0.0, 0.0, 0.02), head);
    b.part(fill(&ellipse_pts(1.1, 1.0, 10).into_iter().map(|p| p + Vec2::new(4.6, 1.3)).collect::<Vec<_>>(), |_| 1.0), &eye, Vec3::new(0.0, 0.0, 0.03), head);
    b.part(fill(&ellipse_pts(0.35, 0.35, 6).into_iter().map(|p| p + Vec2::new(4.9, 1.7)).collect::<Vec<_>>(), |_| 1.0), &glint, Vec3::new(0.0, 0.0, 0.04), head);
    // The mouth's glow (shown while it aims a shell).
    let material = materials.add(ColorMaterial { color: GLOW, alpha_mode: AlphaMode2d::Blend, ..default() });
    let ring: Vec<Vec2> = ellipse_pts(5.0, 4.0, 20).into_iter().map(|p| p + Vec2::new(7.0, -2.5)).collect();
    let glow = b.part(fill(&ring, |p| 1.0 - 0.8 * ((p - Vec2::new(7.0, -2.5)) / Vec2::new(5.0, 4.0)).length()), &material, Vec3::new(0.0, 0.0, 0.05), head);
    b.commands.entity(glow).insert((camera::color_layer(), MouthGlow { owner: beetle, material }));

    let mut rng = Rng(0xBEE7_7A2C);
    commands.entity(beetle).insert((
        Boss::new(&SPEC, home, mats).staged(),
        Beetle {
            state: State::Stand,
            vel: Vec2::ZERO,
            facing: -1.0,
            cooldown: rng.range(COOLDOWN.0, COOLDOWN.1),
            behind: 0.0,
            under: 0.0,
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
    beetle
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

/// React to a hit (health and blood are handled in `boss.rs`). A tank: it only
/// flinches when it isn't doing anything.
fn on_hit(mut hits: MessageReader<BossHit>, mut beetles: Query<&mut Beetle>) {
    for hit in hits.read() {
        let Ok(mut c) = beetles.get_mut(hit.boss) else { continue };
        if hit.killed {
            c.vel = Vec2::new(hit.dir.x * 160.0, 420.0);
            c.state = State::Dead { t: 0.0, landed: false };
            c.dismember = true;
        } else if hit.retaliate {
            c.retaliate = true;
        } else if hit.stagger && matches!(c.state, State::Stand | State::Taunt(_)) {
            c.vel = Vec2::new(hit.dir.x * 60.0, 0.0);
            c.state = State::Hurt(0.0);
        }
    }
}

/// `x` kept inside the arena, the whole body over its floor (the middle if it's too narrow).
fn inside(arena: &Arena, x: f32) -> f32 {
    let (lo, hi) = (arena.left + HALF_W, arena.right - HALF_W);
    if lo < hi { x.clamp(lo, hi) } else { (arena.left + arena.right) * 0.5 }
}

/// Where the mouth is (shells come from there).
fn mouth(pos: Vec2, facing: f32) -> Vec2 {
    pos + Vec2::new(facing * 31.0 * SCALE, 2.0 * SCALE)
}

/// The launch velocity of a shell from `from` landing on `to` after `time`.
fn lob(from: Vec2, to: Vec2, time: f32) -> Vec2 {
    Vec2::new((to.x - from.x) / time, (to.y - from.y) / time + 0.5 * GRAVITY * time)
}

/// Dust thrown along the floor both ways from `at` (`reach`: how far it flies).
fn throw_dust(commands: &mut Commands, materials: &mut Assets<ColorMaterial>, cloud: &Cloud, rng: &mut Rng, at: Vec2, n: usize, reach: f32) {
    for side in [-1.0, 1.0] {
        for _ in 0..n {
            let material = materials.add(ColorMaterial { color: Color::srgba(0.6, 0.58, 0.54, 0.0), alpha_mode: AlphaMode2d::Blend, ..default() });
            commands.spawn((
                Mesh2d(cloud.0.clone()),
                MeshMaterial2d(material.clone()),
                Transform::from_translation((at + Vec2::new(side * rng.range(0.1, 0.9) * HALF_W, rng.range(6.0, 40.0))).extend(13.5)),
                Dust { vel: Vec2::new(side * rng.range(0.5, 1.0) * reach, rng.range(10.0, 90.0)), age: 0.0, life: rng.range(0.5, 0.8), size: rng.range(45.0, 85.0), material },
                BossStuff,
            ));
        }
    }
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
    cloud: Res<Cloud>,
    player: Single<(&mut Player, &Transform), Without<Beetle>>,
    camera: Single<(&Camera, &GlobalTransform), With<MainCamera>>,
    mut beetles: Query<(&mut Beetle, &mut Boss, &mut Transform, Option<&mut boss::Sense>)>,
    terrain: Res<Terrain>,
) {
    let dt = time.delta_secs().min(1.0 / 30.0) * crate::boss::tempo();
    let (mut p, ptf) = player.into_inner();
    let ppos = ptf.translation.truncate();

    for (mut c, mut boss, mut tf, sense) in &mut beetles {
        let pos = tf.translation.truncate();
        let to_player = ppos - pos;
        let alive = p.dead.is_none();
        let aggro = alive && boss.awake;
        let arena = c.arena;
        let ground_at = |x: f32, y: f32| terrain.surface(Vec2::new(x, y - FOOT + 2.0 * STEP_UP), 3000.0);
        let floor = ground_at(pos.x, pos.y).unwrap_or(arena.floor);
        if let Some(mut s) = sense {
            s.goal = aggro.then_some(ppos);
            s.target = aggro.then_some(ppos);
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
        // The chameleon's feet above the floor here.
        let lift = ppos.y - player::HALF.y - floor;
        let grounded_near = p.grounded && lift.abs() < 40.0;
        let under = dx < HALF_W * 0.85 && lift < FOOT;

        // Hit too many times in a row: the quake shakes the chameleon off.
        if std::mem::take(&mut c.retaliate) && matches!(c.state, State::Stand | State::Hurt(_) | State::Taunt(_) | State::Turn(_)) {
            c.state = State::Rear(REAR * 0.4);
        }

        c.state = match c.state {
            State::Dead { t: t0, landed } => {
                let td = t0 + dt;
                for at in [0.3, 0.8] {
                    if t0 < at && td >= at {
                        bleed.write(Bleed { pos, dir: Vec2::Y, spread: PI, speed: 600.0, amount: 80, fluid: Fluid::Orange, sticky: true });
                    }
                }
                let mut landed = landed;
                let mut next = pos;
                if !landed {
                    c.vel.y -= 1400.0 * dt;
                    next += c.vel * dt;
                    let rest = floor + 12.0 * SCALE;
                    if ground_at(next.x, pos.y).is_some() && next.y <= rest && c.vel.y < 0.0 {
                        next.y = rest;
                        c.vel = Vec2::new(c.vel.x * 0.3, 0.0);
                        landed = true;
                        shake.add(0.6);
                        throw_dust(&mut commands, &mut materials, &cloud, &mut c.rng, Vec2::new(pos.x, floor), 6, 700.0);
                        bleed.write(Bleed { pos, dir: Vec2::Y, spread: 1.3, speed: 280.0, amount: 60, fluid: Fluid::Orange, sticky: true });
                    }
                } else {
                    c.vel.x *= 1.0 - 5.0 * dt;
                    next.x += c.vel.x * dt;
                }
                if td < 4.0 && c.rng.chance(dt * 12.0) {
                    bleed.write(Bleed { pos, dir: Vec2::Y, spread: 1.0, speed: 150.0, amount: 3, fluid: Fluid::Orange, sticky: true });
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
                    c.behind = 0.0;
                    c.under = 0.0;
                    let home = boss.home.x - pos.x;
                    c.vel.x = if home.abs() > 30.0 { home.signum() * WALK } else { 0.0 };
                    if home.abs() > 30.0 && home.signum() != c.facing {
                        State::Turn(0.0)
                    } else {
                        State::Stand
                    }
                } else if !c.introduced {
                    c.vel.x = 0.0;
                    if on_screen {
                        c.introduced = true;
                        boss.staged = false;
                        shake.add(0.3);
                        State::Taunt(0.0)
                    } else {
                        State::Stand
                    }
                } else {
                    // Behind it: it notices after a while and turns, slowly.
                    c.behind = if side != c.facing { c.behind + dt } else { 0.0 };
                    c.under = if under { c.under + dt } else { (c.under - dt).max(0.0) };
                    let notice = if enraged { RAGE_TURN_DELAY } else { TURN_DELAY };
                    c.vel.x = if side == c.facing && dx > TOSS_RANGE * 0.7 { side * WALK } else { 0.0 };
                    if c.under > UNDER_PATIENCE || (c.behind > notice * 2.5 && dx < QUAKE_REACH * 0.6) {
                        c.under = 0.0;
                        c.behind = 0.0;
                        c.vel.x = 0.0;
                        State::Rear(0.0)
                    } else if c.behind > notice && !under {
                        c.behind = 0.0;
                        c.vel.x = 0.0;
                        State::Turn(0.0)
                    } else if c.cooldown > 0.0 || side != c.facing {
                        State::Stand
                    } else {
                        c.cooldown = c.rng.range(COOLDOWN.0, COOLDOWN.1) * pace;
                        c.vel.x = 0.0;
                        if dx < TOSS_RANGE {
                            State::Lower(0.0)
                        } else if dx > CHARGE_MIN && c.rng.chance(CHARGE_SHARE) {
                            State::Rev(0.0)
                        } else if dx > MORTAR_MIN {
                            State::Aim(0.0)
                        } else {
                            c.cooldown = 0.3;
                            State::Stand
                        }
                    }
                }
            }
            State::Taunt(s) => {
                c.vel.x = 0.0;
                if s > 0.3 && s < 1.6 && c.rng.chance(dt * 6.0) {
                    shake.add(0.06);
                }
                if to_player.length() < TAUNT_BREAK {
                    State::Rear(0.0)
                } else if s >= TAUNT_TIME {
                    c.cooldown = c.rng.range(0.3, 0.6);
                    State::Stand
                } else {
                    State::Taunt(s + dt)
                }
            }
            State::Turn(s) => {
                c.vel.x = 0.0;
                let time = if enraged { RAGE_TURN_TIME } else { TURN_TIME };
                if s >= time {
                    c.facing = -c.facing;
                    c.cooldown = c.cooldown.min(0.5);
                    State::Stand
                } else {
                    State::Turn(s + dt)
                }
            }
            State::Lower(s) => {
                if s >= LOWER * pace.max(0.8) {
                    State::Toss { t: 0.0, hit: false }
                } else {
                    State::Lower(s + dt)
                }
            }
            State::Toss { t: s, hit } => {
                let s1 = s + dt;
                let mut hit = hit;
                if !hit && s1 >= TOSS_TIME * 0.4 {
                    hit = true;
                    shake.add(0.15);
                    let ahead = (ppos.x - pos.x) * c.facing;
                    if alive && ahead > 0.0 && ahead < TOSS_RANGE + player::HALF.x && lift < TOSS_HIGH {
                        let before = p.wounds;
                        boss::hurt_player(&mut p, ppos, pos, TOSS_DAMAGE, "Flung skyward by the Battletank Beetle", &mut bleed, (&mut shake, &mut stop));
                        if p.wounds > before {
                            // Thrown high over its head.
                            p.vel = Vec2::new(-c.facing * 260.0, 780.0);
                        }
                    }
                }
                if s1 >= TOSS_TIME { State::Stand } else { State::Toss { t: s1, hit } }
            }
            State::Rev(s) => {
                c.vel.x = 0.0;
                if c.rng.chance(dt * 10.0) {
                    let heel = Vec2::new(pos.x - c.facing * HALF_W * 0.6, floor);
                    throw_dust(&mut commands, &mut materials, &cloud, &mut c.rng, heel, 1, 160.0);
                }
                if s >= if enraged { RAGE_REV } else { REV } {
                    shake.add(0.2);
                    State::Charge { t: 0.0, dir: c.facing, hit: false }
                } else {
                    State::Rev(s + dt)
                }
            }
            State::Charge { t: s, dir, hit } => {
                let s1 = s + dt;
                c.vel.x = dir * CHARGE_SPEED;
                let mut hit = hit;
                let ahead = (ppos.x - pos.x) * dir;
                if !hit && alive && ahead > HORN_FROM - player::HALF.x && ahead < HORN_TIP + player::HALF.x && lift < HORN_HIGH {
                    hit = true;
                    let before = p.wounds;
                    boss::hurt_player(&mut p, ppos, pos, CHARGE_DAMAGE, "Run down by the Battletank Beetle's charge", &mut bleed, (&mut shake, &mut stop));
                    if p.wounds > before {
                        p.vel = Vec2::new(dir * 700.0, 520.0);
                    }
                }
                if c.rng.chance(dt * 18.0) {
                    throw_dust(&mut commands, &mut materials, &cloud, &mut c.rng, Vec2::new(pos.x - dir * HALF_W * 0.5, floor), 1, 120.0);
                }
                let blocked = (inside(&arena, pos.x + dir * 6.0) - (pos.x + dir * 6.0)).abs() > 0.5;
                if blocked {
                    // Rammed into the end of its floor.
                    shake.add(0.7);
                    stop.add(0.08);
                    c.vel.x = 0.0;
                    throw_dust(&mut commands, &mut materials, &cloud, &mut c.rng, Vec2::new(pos.x + dir * HALF_W, floor + FOOT), 5, 400.0);
                    State::Daze(0.0)
                } else if s1 >= CHARGE_MAX || (ahead < -CHARGE_PAST) {
                    State::Skid(0.0)
                } else {
                    State::Charge { t: s1, dir, hit }
                }
            }
            State::Skid(s) => {
                c.vel.x *= (-4.0 * dt).exp();
                if s >= SKID {
                    c.vel.x = 0.0;
                    State::Stand
                } else {
                    State::Skid(s + dt)
                }
            }
            State::Daze(s) => {
                c.vel.x = 0.0;
                if s >= DAZE * if enraged { 0.75 } else { 1.0 } {
                    c.cooldown = c.cooldown.min(0.4);
                    State::Stand
                } else {
                    State::Daze(s + dt)
                }
            }
            State::Aim(s) => {
                c.vel.x = 0.0;
                if s >= if enraged { RAGE_AIM } else { AIM } {
                    // Fire: shells lobbed onto where the chameleon stands.
                    shake.add(0.25);
                    let from = mouth(pos, c.facing);
                    let n = if enraged { RAGE_SHELLS } else { SHELLS };
                    let mesh = meshes.add(fill(&ellipse_pts(13.0, 13.0, 16), |p| 1.0 - 0.5 * p.length() / 13.0));
                    for i in 0..n {
                        let off = if n == 1 { 0.0 } else { (i as f32 - (n - 1) as f32 * 0.5) * SHELL_SPREAD };
                        let x = ppos.x + off;
                        let to = Vec2::new(x, terrain.surface(Vec2::new(x, ppos.y), 600.0).unwrap_or(ppos.y - player::HALF.y));
                        let material = materials.add(ColorMaterial { color: ORANGE, ..default() });
                        commands.spawn((
                            Mesh2d(mesh.clone()),
                            MeshMaterial2d(material),
                            Transform::from_translation(from.extend(14.0)),
                            camera::color_layer(),
                            Shell { vel: lob(from, to, SHELL_TIME * c.rng.range(0.9, 1.1)), life: SHELL_TIME * 2.5 },
                            BossStuff,
                        ));
                    }
                    State::Stand
                } else {
                    State::Aim(s + dt)
                }
            }
            State::Rear(s) => {
                c.vel.x = 0.0;
                if s >= REAR * pace.max(0.8) {
                    // The slam: a shockwave along the floor.
                    c.squash = 1.0;
                    shake.add(0.8);
                    stop.add(0.06);
                    throw_dust(&mut commands, &mut materials, &cloud, &mut c.rng, Vec2::new(pos.x, floor), 9, 1100.0);
                    if alive && grounded_near && dx < QUAKE_REACH {
                        let stun = p.stun;
                        let before = p.wounds;
                        boss::hurt_player(&mut p, ppos, pos, QUAKE_DAMAGE, "Shaken apart by the Battletank Beetle's quake", &mut bleed, (&mut shake, &mut stop));
                        if p.wounds > before {
                            p.stun = stun.max(QUAKE_STUN);
                            p.vel = Vec2::new(side * 420.0, 260.0);
                        }
                    }
                    State::Slam(0.0)
                } else {
                    State::Rear(s + dt)
                }
            }
            State::Slam(s) => {
                if s >= SLAM {
                    c.cooldown = c.cooldown.min(0.6);
                    State::Stand
                } else {
                    State::Slam(s + dt)
                }
            }
        };

        // Only its back is soft: the way it faces (mid-turn, the way it's turned to).
        boss.shell = match c.state {
            State::Dead { .. } => None,
            State::Turn(s) if s > 0.5 * if enraged { RAGE_TURN_TIME } else { TURN_TIME } => Some(-c.facing),
            _ => Some(c.facing),
        };

        // On the ground: follow the floor under it.
        if !matches!(c.state, State::Dead { .. }) {
            let x = inside(&arena, tf.translation.x + c.vel.x * dt);
            let floor = ground_at(x, tf.translation.y).unwrap_or(floor);
            tf.translation.x = x;
            tf.translation.y = floor + FOOT;
            c.stride += (c.vel.x.abs() * dt * 0.03).min(1.0);
        }
        c.squash = (c.squash - dt * 2.5).max(0.0);
    }
}

/// Shells fly, and burst where they land (or on the chameleon).
#[allow(clippy::too_many_arguments)]
fn shells(
    time: Res<Time>,
    mut commands: Commands,
    mut shake: ResMut<Shake>,
    mut stop: ResMut<HitStop>,
    mut bleed: MessageWriter<Bleed>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    cloud: Res<Cloud>,
    mut rng: Local<Option<Rng>>,
    mut ring: Local<Option<Handle<Mesh>>>,
    player: Single<(&mut Player, &Transform), (Without<Shell>, Without<Blast>)>,
    mut flying: Query<(Entity, &mut Shell, &mut Transform), Without<Blast>>,
    mut blasts: Query<(Entity, &mut Blast, &mut Transform), Without<Shell>>,
    terrain: Res<Terrain>,
) {
    let dt = time.delta_secs().min(1.0 / 30.0);
    let rng = rng.get_or_insert(Rng(0x5e11_b1a5));
    let ring = ring.get_or_insert_with(|| meshes.add(fill(&ellipse_pts(1.0, 1.0, 28), |p| 0.2 + 0.8 * p.length().powi(3)))).clone();
    let (mut p, ptf) = player.into_inner();
    let ppos = ptf.translation.truncate();
    for (e, mut s, mut tf) in &mut flying {
        s.vel.y -= GRAVITY * dt;
        s.life -= dt;
        let pos = tf.translation.truncate() + s.vel * dt;
        tf.translation.x = pos.x;
        tf.translation.y = pos.y;
        tf.rotate_z(6.0 * dt);
        let on_player = p.dead.is_none() && pos.distance(ppos) < 14.0 + player::HALF.y;
        if !(terrain.solid_at(pos) || on_player || s.life <= 0.0) {
            continue;
        }
        commands.entity(e).despawn();
        shake.add(0.3);
        if p.dead.is_none() && pos.distance(ppos) < BLAST + player::HALF.y {
            boss::hurt_player(&mut p, ppos, pos, SHELL_DAMAGE, "Blown up by a Battletank Beetle shell", &mut bleed, (&mut shake, &mut stop));
        }
        let material = materials.add(ColorMaterial { color: ORANGE.with_alpha(0.9), alpha_mode: AlphaMode2d::Blend, ..default() });
        commands.spawn((Mesh2d(ring.clone()), MeshMaterial2d(material.clone()), Transform::from_translation(pos.extend(14.5)), camera::color_layer(), Blast { age: 0.0, material }, BossStuff));
        throw_dust(&mut commands, &mut materials, &cloud, rng, pos, 2, 300.0);
    }
    for (e, mut b, mut tf) in &mut blasts {
        b.age += dt;
        let k = b.age / 0.35;
        if k >= 1.0 {
            commands.entity(e).despawn();
            continue;
        }
        tf.scale = Vec3::splat(BLAST * (0.3 + 0.9 * k));
        if let Some(mut m) = materials.get_mut(&b.material) {
            m.color = m.color.with_alpha(0.85 * (1.0 - k));
        }
    }
}

/// The mouth's glow while it aims a shell.
fn glows(time: Res<Time>, mut materials: ResMut<Assets<ColorMaterial>>, beetles: Query<&Beetle>, glows: Query<&MouthGlow>) {
    let t = time.elapsed_secs();
    for g in &glows {
        let k = match beetles.get(g.owner).map(|c| c.state) {
            Ok(State::Aim(s)) => (s / AIM).min(1.0),
            _ => 0.0,
        };
        if let Some(mut m) = materials.get_mut(&g.material) {
            m.color = GLOW.with_alpha(0.85 * k * (0.75 + 0.25 * (t * (10.0 + 20.0 * k)).sin()));
        }
    }
}

/// Blows that glance off its armor throw sparks.
fn sparks(
    time: Res<Time>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    mut glanced: MessageReader<Glanced>,
    mut mesh: Local<Option<Handle<Mesh>>>,
    mut rng: Local<Option<Rng>>,
    beetles: Query<(), With<Beetle>>,
    mut live: Query<(Entity, &mut Spark, &mut Transform)>,
) {
    let dt = time.delta_secs();
    let rng = rng.get_or_insert(Rng(0x5a2c_0ff1));
    let mesh = mesh.get_or_insert_with(|| meshes.add(stroke(&[Vec2::new(-1.0, 0.0), Vec2::new(1.0, 0.0)], |t| 1.0 - 0.7 * t, 1.0, 1.0, |_| 1.0))).clone();
    for g in glanced.read() {
        if !beetles.contains(g.boss) {
            continue;
        }
        for _ in 0..14 {
            let dir = Vec2::from_angle(g.out.to_angle() + rng.range(-1.1, 1.1));
            let material = materials.add(ColorMaterial { color: Color::srgb(1.0, rng.range(0.75, 0.95), rng.range(0.4, 0.7)), alpha_mode: AlphaMode2d::Blend, ..default() });
            let vel = dir * rng.range(300.0, 700.0);
            commands.spawn((
                Mesh2d(mesh.clone()),
                MeshMaterial2d(material.clone()),
                Transform::from_translation(g.at.extend(15.0)).with_rotation(Quat::from_rotation_z(vel.to_angle())).with_scale(Vec3::new(rng.range(6.0, 12.0), 2.0, 1.0)),
                camera::color_layer(),
                Spark { vel, age: 0.0, life: rng.range(0.15, 0.35), material },
                BossStuff,
            ));
        }
    }
    for (e, mut s, mut tf) in &mut live {
        s.age += dt;
        let k = s.age / s.life;
        if k >= 1.0 {
            commands.entity(e).despawn();
            continue;
        }
        s.vel.y -= 900.0 * dt;
        tf.translation += (s.vel * dt).extend(0.0);
        tf.rotation = Quat::from_rotation_z(s.vel.to_angle());
        if let Some(mut m) = materials.get_mut(&s.material) {
            m.color = m.color.with_alpha(1.0 - k);
        }
    }
}

fn dust(time: Res<Time>, mut commands: Commands, mut materials: ResMut<Assets<ColorMaterial>>, mut puffs: Query<(Entity, &mut Dust, &mut Transform)>) {
    let dt = time.delta_secs();
    for (e, mut d, mut tf) in &mut puffs {
        d.age += dt;
        let k = d.age / d.life;
        if k >= 1.0 {
            commands.entity(e).despawn();
            continue;
        }
        tf.translation += (d.vel * dt).extend(0.0);
        d.vel *= (-3.5 * dt).exp();
        tf.scale = Vec3::new(d.size * (1.0 + 1.5 * k), d.size * (0.6 + 0.8 * k), 1.0);
        if let Some(mut m) = materials.get_mut(&d.material) {
            m.color = m.color.with_alpha(0.55 * (1.0 - k));
        }
    }
}

/// The chameleon came back: start the fight over (`boss.rs` heals it), the shells gone.
fn reset_on_respawn(mut commands: Commands, mut respawned: MessageReader<Respawned>, mut beetles: Query<&mut Beetle>, shells: Query<Entity, With<Shell>>) {
    if respawned.read().count() == 0 {
        return;
    }
    for e in &shells {
        commands.entity(e).despawn();
    }
    for mut c in &mut beetles {
        if matches!(c.state, State::Dead { .. }) {
            continue;
        }
        c.state = State::Stand;
        c.introduced = false;
        c.vel = Vec2::ZERO;
        c.cooldown = 1.5;
        c.behind = 0.0;
        c.under = 0.0;
    }
}

/// A freshly killed beetle: the horn and the near front leg tear off.
fn dismember(mut commands: Commands, mut beetles: Query<&mut Beetle>, bones: Query<(Entity, &Bone, &GlobalTransform)>) {
    let mut done = Vec::new();
    for (e, bone, gt) in &bones {
        let Ok(c) = beetles.get(bone.owner) else { continue };
        if !c.dismember {
            continue;
        }
        done.push(bone.owner);
        let f = c.facing;
        let (vel, spin, bleeds) = match bone.kind {
            BoneKind::Head => (Vec2::new(f * 300.0, 700.0), 5.0, Some(Fluid::Orange)),
            BoneKind::Leg { pair: 0, near: true } => (Vec2::new(f * 160.0, 560.0), 9.0, Some(Fluid::Orange)),
            _ => continue,
        };
        commands.entity(e).remove::<Bone>();
        boss::tear_off(&mut commands, e, gt, vel, spin, bleeds);
    }
    done.sort();
    done.dedup();
    for owner in done {
        if let Ok(mut c) = beetles.get_mut(owner) {
            c.dismember = false;
        }
    }
}

fn ease(k: f32) -> f32 {
    let k = k.clamp(0.0, 1.0);
    k * k * (3.0 - 2.0 * k)
}

fn animate(time: Res<Time>, beetles: Query<(&Beetle, &Boss)>, mut bones: Query<(&Bone, &mut Transform)>) {
    let t = time.elapsed_secs();
    let dt = time.delta_secs();
    for (bone, mut tf) in &mut bones {
        let Ok((c, boss)) = beetles.get(bone.owner) else { continue };
        let tt = t + c.seed;
        let (dead_t, landed) = match c.state {
            State::Dead { t, landed } => (Some(t), landed),
            _ => (None, false),
        };
        let walking = (c.vel.x.abs() / WALK).min(3.0);
        let charging = matches!(c.state, State::Charge { .. });
        // The front rising for the quake (0..1) and slammed down.
        let rear = match c.state {
            State::Rear(s) => ease(s / REAR),
            State::Taunt(s) => ease(s / 0.4) * (1.0 - ease((s - TAUNT_TIME + 0.4) / 0.4)),
            _ => 0.0,
        };
        match bone.kind {
            BoneKind::Rig => {
                match c.state {
                    // Turning: the body swings round through edge-on.
                    State::Turn(s) => {
                        let time = if boss.enraged() { RAGE_TURN_TIME } else { TURN_TIME };
                        tf.scale.x = c.facing * SCALE * (PI * ease(s / time)).cos();
                    }
                    _ => tf.scale.x += (c.facing * SCALE - tf.scale.x) * (1.0 - (-10.0 * dt).exp()),
                }
                tf.scale.y = SCALE * (1.0 - 0.08 * c.squash);
                let low = match c.state {
                    State::Lower(s) => 1.5 * ease(s / LOWER),
                    State::Rev(_) => 1.2,
                    State::Daze(_) => 2.0,
                    _ => 0.0,
                } + 2.5 * c.squash;
                let bob = if walking > 0.0 { (c.stride * 3.0).sin().abs() * 0.5 * walking.min(1.0) } else { 0.0 };
                let shiver = match c.state {
                    State::Rev(_) => (tt * 50.0).sin() * 0.25,
                    State::Daze(_) => (tt * 3.0).sin() * 0.4,
                    _ => 0.0,
                };
                tf.translation.y = (bob - low + shiver + rear * 3.0) * SCALE;
                let angle = match dead_t {
                    Some(td) => {
                        if landed { PI } else { PI * (td * 1.6).min(1.0) }
                    }
                    None => (match c.state {
                        State::Charge { .. } => -0.05 + (tt * 25.0).sin() * 0.015,
                        State::Daze(s) => (tt * 4.0).sin() * 0.04 * (1.0 - s / DAZE),
                        State::Slam(_) => -0.04 * (1.0 - c.squash),
                        _ => rear * 0.2 + (tt * 1.1).sin() * 0.006,
                    }) * c.facing,
                };
                tf.rotation = Quat::from_rotation_z(angle);
            }
            BoneKind::Head => {
                let angle = if let Some(td) = dead_t {
                    -0.4 + (tt * 12.0).sin() * 0.2 * (1.0 - td / 6.0).max(0.1)
                } else {
                    match c.state {
                        State::Lower(s) => -0.45 * ease(s / LOWER),
                        State::Toss { t, .. } => -0.45 + 1.25 * ease(t / (TOSS_TIME * 0.5)),
                        State::Rev(_) | State::Charge { .. } => -0.35,
                        State::Aim(s) => 0.35 * ease(s / 0.3) + (tt * 30.0).sin() * 0.02,
                        State::Daze(_) => -0.2 + (tt * 2.5).sin() * 0.15,
                        _ if rear > 0.0 => 0.5 * rear,
                        _ => (tt * 1.3).sin() * 0.04,
                    }
                };
                tf.rotation = Quat::from_rotation_z(angle);
            }
            BoneKind::Elytra => {
                // Cracks open a little to fire, and while it rears.
                let open = match c.state {
                    State::Aim(s) => 0.06 * ease(s / 0.4),
                    _ => 0.05 * rear,
                };
                tf.rotation = Quat::from_rotation_z(open);
            }
            BoneKind::Leg { pair, near } => {
                // Alternating tripods: front and back of one side with the middle of the other.
                let group = if (pair == 1) == near { 0.0 } else { PI };
                let swing = if pair == 0 { 1.0 } else if pair == 2 { -1.0 } else { 0.5 };
                let angle = if let Some(td) = dead_t {
                    (tt * 22.0 + pair as f32 * 1.3 + group).sin() * 0.4 * (1.0 - td / 6.0).max(0.12)
                } else {
                    let paw = match c.state {
                        State::Rev(_) if pair == 0 => (tt * 18.0 + group).sin() * 0.25,
                        State::Taunt(_) => (tt * 14.0 + group + pair as f32).sin() * 0.12,
                        _ => 0.0,
                    };
                    let step = if charging { 0.3 } else { 0.18 * walking.min(1.0) };
                    step * (c.stride * 3.0 + group).sin() + paw - rear * 0.25 * swing
                };
                tf.rotation = Quat::from_rotation_z(angle);
            }
        }
    }
}

/// The shield earned from the beetle: G makes the chameleon untouchable for
/// `SHIELD_TIME` (`Player::guard`), then it needs `SHIELD_COOLDOWN` to come back.
#[derive(Resource, Default)]
pub struct Shield {
    /// Seconds before it can be raised again.
    pub cooldown: f32,
    /// Seconds it has been up (`None`: down).
    up: Option<f32>,
}

/// The bubble around the chameleon while the shield is up, and the ping when it is ready again.
#[derive(Component)]
struct Bubble;

#[derive(Component)]
struct Ping {
    age: f32,
    material: Handle<ColorMaterial>,
}

#[allow(clippy::too_many_arguments)]
fn shield(
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    defeated: Res<Defeated>,
    mut state: ResMut<Shield>,
    mut restarted: MessageReader<Restart>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    mut shake: ResMut<Shake>,
    mut ring: Local<Option<Handle<Mesh>>>,
    player: Single<(&mut Player, &Transform)>,
    mut bubble: Query<(&mut Transform, &MeshMaterial2d<ColorMaterial>), (With<Bubble>, Without<Player>, Without<Ping>)>,
    mut pings: Query<(Entity, &mut Ping, &mut Transform), (Without<Bubble>, Without<Player>)>,
) {
    let dt = time.delta_secs();
    let now = time.elapsed_secs();
    let (mut p, ptf) = player.into_inner();
    let center = ptf.translation.truncate();
    if restarted.read().count() > 0 || p.dead.is_some() {
        *state = Shield::default();
    }
    let ring = ring.get_or_insert_with(|| meshes.add(fill(&ellipse_pts(1.0, 1.0, 32), |p| 0.15 + 0.85 * p.length().powi(4)))).clone();
    let was_cooling = state.cooldown > 0.0;
    state.cooldown = (state.cooldown - dt).max(0.0);
    if was_cooling && state.cooldown <= 0.0 && defeated.shield() {
        // Ready again: a ping.
        let material = materials.add(ColorMaterial { color: ORANGE.with_alpha(0.8), alpha_mode: AlphaMode2d::Blend, ..default() });
        commands.spawn((Mesh2d(ring.clone()), MeshMaterial2d(material.clone()), Transform::from_translation(center.extend(11.6)), camera::color_layer(), Ping { age: 0.0, material }));
    }
    if keys.just_pressed(KeyCode::KeyG) && defeated.shield() && state.cooldown <= 0.0 && p.dead.is_none() {
        state.cooldown = SHIELD_COOLDOWN;
        state.up = Some(0.0);
        p.guard = p.guard.max(SHIELD_TIME);
        shake.add(0.2);
    }
    if let Some(up) = state.up.as_mut() {
        *up += dt;
        // Kept up for its whole time (other powers may set a shorter guard).
        p.guard = p.guard.max(SHIELD_TIME - *up);
        if *up >= SHIELD_TIME {
            state.up = None;
        }
    }

    if bubble.is_empty() {
        let mat = materials.add(ColorMaterial { color: ORANGE.with_alpha(0.0), alpha_mode: AlphaMode2d::Blend, ..default() });
        commands.spawn((Mesh2d(ring.clone()), MeshMaterial2d(mat), Transform::from_xyz(center.x, center.y, 11.6), camera::color_layer(), Bubble));
    }
    for (mut tf, mat) in &mut bubble {
        tf.translation.x = center.x;
        tf.translation.y = center.y;
        let (alpha, size) = match state.up {
            Some(up) => {
                let rise = ease(up / 0.15);
                // Flickers in its last half second.
                let left = SHIELD_TIME - up;
                let flicker = if left < 0.5 { 0.5 + 0.5 * (now * 40.0).sin().signum() } else { 1.0 };
                (0.55 * rise * flicker, 30.0 + 8.0 * rise + 2.0 * (now * 9.0).sin())
            }
            None => (0.0, 30.0),
        };
        tf.scale = Vec3::splat(size);
        if let Some(mut m) = materials.get_mut(&mat.0) {
            m.color = ORANGE.with_alpha(alpha);
        }
    }
    for (e, mut ping, mut tf) in &mut pings {
        ping.age += dt;
        let k = ping.age / 0.45;
        if k >= 1.0 {
            commands.entity(e).despawn();
            continue;
        }
        tf.translation.x = center.x;
        tf.translation.y = center.y;
        tf.scale = Vec3::splat(20.0 + 40.0 * k);
        if let Some(mut m) = materials.get_mut(&ping.material) {
            m.color = ORANGE.with_alpha(0.7 * (1.0 - k));
        }
    }
}
