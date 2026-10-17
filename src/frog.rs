//! The Plague Frog: a giant, warty, poisonous frog that fights on the ground.
//!
//! Its main attack is **poison**: the throat sac swells (the tell), then it
//! belches a violet cloud at where the chameleon stood. The cloud drifts there,
//! stops and lingers for a while, hurting whatever stays inside (one tick per
//! `player::INVULN`). Enraged (half health) it belches three at once, fanned.
//! Its second attack is the **stomp**: it crouches low (the tell), leaps high
//! and comes down on where the chameleon stood: crushed if still under it.
//! Every big landing (stomp, or a leap back out of reach) sends a **shockwave**
//! along the floor that stuns the chameleon if it is standing on it: jump.
//! After a stomp it is winded for a moment: the opening to hit it.
//! Between attacks it keeps its distance with little hops. Face-off on first
//! sight (it croaks, sac pulsing). Hits during a leap don't stop it.
//!
//! It bleeds violet `#b800ff` and gives the chameleon the power to move through
//! water as on land (`boss::Defeated::wade`, used in `player.rs`). On death the
//! sac bursts (one last, harmless cloud), it flips belly-up, a hind leg tears
//! off and the others twitch. Health, getting hit and the death spectacle live
//! in `boss.rs`.

use std::f32::consts::PI;

use bevy::{prelude::*, sprite_render::AlphaMode2d};

use crate::{
    boss::{self, Arena, Boss, BossHit, BossSpec, BossStuff},
    camera::{self, HitStop, MainCamera, Shake},
    chameleon,
    fumes::Cloud,
    gore::{Bleed, Fluid},
    player::{self, Hurt, Player, Respawned},
    shapes::*,
    terrain::{Terrain, STEP_UP},
};

/// Its color: what the chameleon earns, its blood, the health bar.
pub const VIOLET: Color = Color::srgb(0.72, 0.0, 1.0);

pub(crate) static SPEC: BossSpec = BossSpec {
    name: "THE PLAGUE FROG",
    hp: 16,
    radius: RADIUS,
    blood: Fluid::Violet,
    color: VIOLET,
    aggro: AGGRO,
    // 16 hp, poison and stomps.
    food: 5.0,
};

/// Size of the boss relative to its drawing.
const SCALE: f32 = 4.8;
/// Hurtbox radius around the body center.
const RADIUS: f32 = 21.0 * SCALE;
/// Body center height above the floor when sitting.
const FOOT: f32 = 10.0 * SCALE;
/// Half its width: how close to the arena edges its center may go.
const HALF_W: f32 = 20.0 * SCALE;
const AGGRO: f32 = 1200.0;
/// How far inside the screen edge (px) it must be to count as in view.
const SCREEN_MARGIN: f32 = 60.0;

/// The face-off: it croaks (sac pulsing) before the fight.
const TAUNT_TIME: f32 = 2.2;
/// The chameleon this close (px) cuts the taunt short: it stomps at once.
const TAUNT_BREAK: f32 = 330.0;

/// Preferred distance to the chameleon between attacks.
const KEEP_AWAY: f32 = 520.0;
/// Closer than this it would rather stomp than belch.
const CLOSE: f32 = 280.0;
/// Little repositioning hops.
const HOP_LENGTH: f32 = 220.0;
const HOP_HEIGHT: f32 = 80.0;
const HOP_TIME: f32 = 0.5;
/// Sitting still between hops.
const HOP_REST: (f32, f32) = (0.35, 0.8);
/// Seconds between attacks (enraged: × `RAGE_PACE`).
const COOLDOWN: (f32, f32) = (1.4, 2.2);
const RAGE_PACE: f32 = 0.65;

// The poison cloud (primary attack).
/// Share of the attacks that are poison (the rest are stomps); close up it stomps more.
const POISON_SHARE: f32 = 0.65;
/// The sac swelling before the belch.
const INFLATE: f32 = 0.9;
/// Mouth open, belching.
const BELCH: f32 = 0.45;
/// How fast a cloud slows down (per second); it is thrown to stop where the chameleon stood.
const CLOUD_DRAG: f32 = 2.2;
const CLOUD_MAX_SPEED: f32 = 1100.0;
/// Radius it grows to, and how long it lingers (enraged: longer).
const CLOUD_RADIUS: f32 = 120.0;
const CLOUD_LIFE: f32 = 5.5;
const CLOUD_LIFE_RAGE: f32 = 7.5;
const MAX_CLOUDS: usize = 5;
/// Enraged: three clouds, the side ones this far (px) from the aimed spot.
const CLOUD_FAN: f32 = 210.0;
const POISON_DAMAGE: i32 = 1;

// The stomp (second attack).
/// Crouching before the leap.
const CROUCH: f32 = 0.7;
const STOMP_HEIGHT: f32 = 400.0;
const STOMP_TIME: f32 = 0.95;
/// Furthest it leaps at the chameleon.
const STOMP_REACH: f32 = 800.0;
/// Crushed if this close (horizontally) to where it lands, and low enough.
const CRUSH_HALF: f32 = 120.0;
const CRUSH_HEIGHT: f32 = 140.0;
const CRUSH_DAMAGE: i32 = 2;
/// Winded after a stomp (the opening).
const RECOVER: f32 = 1.1;
const RECOVER_RAGE: f32 = 0.7;

/// The shockwave of a big landing: standing on the floor this close = stunned.
const SHOCK_REACH: f32 = 650.0;
const SHOCK_STUN: f32 = 1.0;
/// Feet within this of the floor where it landed count as standing on it.
const SHOCK_HEIGHT: f32 = 30.0;

/// After a flurry of hits: answers if the chameleon is this close, else leaps away.
const RETALIATE_REACH: f32 = 340.0;
const EVADE_LENGTH: f32 = 520.0;
const EVADE_HEIGHT: f32 = 230.0;
const EVADE_TIME: f32 = 0.8;

const SKIN: Color = Color::srgb(0.30, 0.34, 0.22);
const DARK: Color = Color::srgb(0.15, 0.17, 0.11);
const BELLY: Color = Color::srgb(0.74, 0.70, 0.52);
const WART: Color = Color::srgb(0.40, 0.10, 0.52);
const SAC: Color = Color::srgb(0.62, 0.30, 0.78);
const IRIS: Color = Color::srgb(0.85, 0.62, 0.10);
const PUPIL: Color = Color::srgb(0.03, 0.02, 0.02);

pub struct FrogPlugin;

impl Plugin for FrogPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Daze>().add_systems(
            Update,
            (reset_on_respawn, on_hit, think, poison, dust, dismember, animate, stars)
                .chain()
                .after(player::move_player)
                .after(chameleon::update_anim_state),
        );
    }
}

#[derive(Component)]
struct Frog {
    state: State,
    /// Velocity (from the leap path while airborne: for the pose).
    vel: Vec2,
    facing: f32,
    cooldown: f32,
    /// Time left sitting before the next little hop.
    rest: f32,
    /// A flurry of hits: answer with an attack or get away (see `BossHit::retaliate`).
    retaliate: bool,
    /// It has faced the chameleon off (until the lair resets it).
    introduced: bool,
    /// 0..1: the sac swelling (eased toward its target for the pose).
    sac: f32,
    /// 1 right after landing, fading: squash for the pose.
    squash: f32,
    seed: f32,
    /// Just died: burst the sac and tear a leg off this frame.
    dismember: bool,
    arena: Arena,
    rng: Rng,
}

#[derive(Clone, Copy, PartialEq)]
enum Leap {
    /// A little hop: no shockwave.
    Hop,
    /// At the chameleon: crush + shockwave, then winded.
    Stomp,
    /// Out of reach after a flurry: shockwave.
    Evade,
}

#[derive(Clone, Copy, PartialEq)]
enum State {
    Sit,
    Taunt(f32),
    Inflate(f32),
    Belch(f32),
    Crouch(f32),
    /// Along a parabola from `from` to `to` (feet positions), `height` over the straight line.
    Air { from: Vec2, to: Vec2, height: f32, time: f32, t: f32, kind: Leap },
    Recover(f32),
    Hurt(f32),
    Dead { t: f32, landed: bool },
}

impl Frog {
    fn airborne(&self) -> bool {
        matches!(self.state, State::Air { .. }) || matches!(self.state, State::Dead { landed: false, .. })
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
    Jaw,
    Sac,
    Eye,
    Thigh { near: bool },
    Shin { near: bool },
    Arm { near: bool },
}

/// A poison cloud (its puffs are children). Hurts while the chameleon is inside.
#[derive(Component)]
struct Poison {
    vel: Vec2,
    age: f32,
    life: f32,
    harmful: bool,
    material: Handle<ColorMaterial>,
}

#[derive(Component)]
struct Puff {
    /// Where it sits in the cloud (unit radius) and how it swirls.
    offset: Vec2,
    phase: f32,
    size: f32,
}

/// Seconds left of the chameleon seeing stars (stunned by a shockwave).
#[derive(Resource, Default)]
struct Daze(f32);

/// One of the stars circling the dazed chameleon's head.
#[derive(Component)]
struct Star(usize);

const STARS: usize = 5;
const STAR_COLOR: Color = Color::srgb(1.0, 0.88, 0.3);

/// Dust thrown along the floor by a big landing.
#[derive(Component)]
struct Dust {
    vel: Vec2,
    age: f32,
    life: f32,
    size: f32,
    material: Handle<ColorMaterial>,
}

/// Spawns the frog on the arena floor, facing left.
pub fn spawn(commands: &mut Commands, meshes: &mut Assets<Mesh>, materials: &mut Assets<ColorMaterial>, arena: &Arena) -> Entity {
    let skin = materials.add(SKIN);
    let dark = materials.add(DARK);
    let belly = materials.add(BELLY);
    let wart = materials.add(WART);
    let sac = materials.add(SAC);
    let iris = materials.add(IRIS);
    let pupil = materials.add(PUPIL);
    let glint = materials.add(Color::WHITE);
    let mats = vec![(skin.clone(), SKIN), (dark.clone(), DARK), (belly.clone(), BELLY), (wart.clone(), WART), (sac.clone(), SAC)];

    let home = Vec2::new(arena.home.x, arena.floor + FOOT);
    let frog = commands.spawn((Transform::from_translation(home.extend(12.0)), Visibility::default())).id();
    let mut b = Builder { commands, meshes, owner: frog };
    let rig = b.bone(Vec3::ZERO, frog, BoneKind::Rig);
    b.commands.entity(rig).insert(Transform::from_scale(Vec3::new(-SCALE, SCALE, 1.0)));

    // Hind legs: a thigh from the hip to the knee (folded forward), a shin back
    // to the ankle and a long webbed foot forward along the floor.
    for near in [false, true] {
        let (mat, z, dx) = if near { (&skin, 0.2, 0.0) } else { (&dark, -0.4, 3.0) };
        let thigh = b.bone(Vec3::new(-13.0 + dx, -1.0, z), rig, BoneKind::Thigh { near });
        let thigh_pts = smooth_closed(&[Vec2::new(-3.0, 3.0), Vec2::new(5.0, 1.0), Vec2::new(9.0, -6.0), Vec2::new(6.0, -9.0), Vec2::new(-2.0, -4.0)], 4);
        b.part(fill(&thigh_pts, |p| 0.7 + 0.3 * ((p.y + 9.0) / 12.0).clamp(0.0, 1.0)), mat, Vec3::ZERO, thigh);
        let shin = b.bone(Vec3::new(7.0, -7.5, -0.05), thigh, BoneKind::Shin { near });
        let leg = smooth_open(&[Vec2::ZERO, Vec2::new(-6.0, -1.0), Vec2::new(-12.0, -1.5)], 4);
        b.part(stroke(&leg, |t| 4.5 - 1.5 * t, 1.0, 0.75, |_| 1.0), mat, Vec3::ZERO, shin);
        // The foot: toes fanned forward from the ankle.
        let toes: Vec<[Vec2; 3]> = [(-1.0, 6.0), (-1.8, 10.0), (-2.4, 13.0)]
            .into_iter()
            .map(|(dy, len)| [Vec2::new(-12.0, -0.5), Vec2::new(-12.0, -2.5), Vec2::new(-12.0 + len, dy - 0.8)])
            .collect();
        b.part(triangles(&toes), mat, Vec3::new(0.0, 0.0, 0.01), shin);
    }

    // Front arms: short, propping the chest up.
    for near in [false, true] {
        let (mat, z, dx) = if near { (&skin, 0.25, 0.0) } else { (&dark, -0.35, -3.0) };
        let arm = b.bone(Vec3::new(10.0 + dx, -4.0, z), rig, BoneKind::Arm { near });
        let path = smooth_open(&[Vec2::ZERO, Vec2::new(1.5, -3.5), Vec2::new(0.5, -6.5)], 4);
        b.part(stroke(&path, |t| 3.2 - 1.0 * t, 1.0, 0.8, |_| 1.0), mat, Vec3::ZERO, arm);
        b.part(triangles(&[[Vec2::new(-1.5, -6.0), Vec2::new(2.0, -6.0), Vec2::new(4.5, -7.2)], [Vec2::new(-1.0, -6.0), Vec2::new(1.0, -6.0), Vec2::new(-2.5, -7.2)]]), mat, Vec3::new(0.0, 0.0, 0.01), arm);
    }

    // The throat sac under the jaw (swells before a belch).
    let sac_bone = b.bone(Vec3::new(13.0, -6.0, 0.1), rig, BoneKind::Sac);
    b.part(fill(&ellipse_pts(5.5, 4.0, 20), |p| 0.75 + 0.25 * (p.y / 4.0)), &sac, Vec3::ZERO, sac_bone);

    // Body: a squat hump, high at the back, the wide flat head in front.
    let body = smooth_closed(
        &[
            Vec2::new(23.0, 2.0),
            Vec2::new(21.0, 7.0),
            Vec2::new(14.0, 11.0),
            Vec2::new(4.0, 13.0),
            Vec2::new(-7.0, 15.0),
            Vec2::new(-16.0, 12.0),
            Vec2::new(-22.0, 5.0),
            Vec2::new(-21.0, -3.0),
            Vec2::new(-14.0, -8.0),
            Vec2::new(-2.0, -10.0),
            Vec2::new(10.0, -9.0),
            Vec2::new(18.0, -5.0),
        ],
        6,
    );
    b.part(fill(&body, |p| 0.55 + 0.45 * ((p.y + 10.0) / 25.0).clamp(0.0, 1.0)), &skin, Vec3::ZERO, rig);
    // Pale belly along the bottom.
    let belly_pts = smooth_closed(&[Vec2::new(16.0, -4.0), Vec2::new(4.0, -5.0), Vec2::new(-10.0, -4.5), Vec2::new(-15.0, -7.5), Vec2::new(-2.0, -9.6), Vec2::new(10.0, -8.6)], 4);
    b.part(fill(&belly_pts, |_| 0.9), &belly, Vec3::new(0.0, 0.0, 0.01), rig);
    // Poison warts over the back and a dark ridge behind the eye.
    let mut rng = Rng(0xF20C_7A0D);
    let warts: Vec<Vec<Vec2>> = (0..11)
        .map(|_| {
            let a = rng.range(0.25, 2.5);
            let c = Vec2::new(-3.0, -1.0) + Vec2::from_angle(a) * Vec2::new(rng.range(13.0, 17.0), rng.range(9.0, 12.5));
            ellipse_pts(rng.range(1.0, 2.0), rng.range(0.8, 1.5), 8).into_iter().map(|p| p + c).collect()
        })
        .collect();
    b.part(blobs(&warts, |_| 1.0), &wart, Vec3::new(0.0, 0.0, 0.02), rig);
    let ridge = smooth_open(&[Vec2::new(10.0, 10.0), Vec2::new(2.0, 11.5), Vec2::new(-8.0, 10.0), Vec2::new(-15.0, 5.0)], 5);
    b.part(stroke(&ridge, |t| 2.4 * (PI * t).sin(), 1.0, 0.7, |_| 1.0), &dark, Vec3::new(0.0, 0.0, 0.02), rig);

    // The lower jaw: drops open to belch.
    let jaw = b.bone(Vec3::new(5.0, -1.5, 0.05), rig, BoneKind::Jaw);
    let jaw_pts = smooth_closed(&[Vec2::new(18.0, 0.5), Vec2::new(10.0, 0.0), Vec2::new(0.0, 0.0), Vec2::new(2.0, -3.0), Vec2::new(12.0, -3.0)], 4);
    b.part(fill(&jaw_pts, |_| 0.7), &skin, Vec3::ZERO, jaw);
    let lip = smooth_open(&[Vec2::new(0.0, 0.3), Vec2::new(9.0, 0.6), Vec2::new(18.0, 0.8)], 4);
    b.part(stroke(&lip, |_| 1.0, 1.0, 1.0, |_| 1.0), &dark, Vec3::new(0.0, 0.0, 0.01), jaw);

    // A bulging eye on top of the head, golden with a slit pupil.
    let eye = b.bone(Vec3::new(13.0, 11.0, 0.05), rig, BoneKind::Eye);
    b.part(fill(&ellipse_pts(5.0, 4.6, 20), |p| 0.6 + 0.4 * (p.y / 4.6).max(0.0)), &skin, Vec3::ZERO, eye);
    b.part(fill(&ellipse_pts(3.6, 3.3, 18), |_| 1.0), &iris, Vec3::new(0.6, 0.4, 0.01), eye);
    b.part(fill(&ellipse_pts(2.6, 0.9, 12), |_| 1.0), &pupil, Vec3::new(0.8, 0.4, 0.02), eye);
    b.part(fill(&ellipse_pts(0.8, 0.8, 8), |_| 1.0), &glint, Vec3::new(1.9, 1.8, 0.03), eye);

    let mut rng = Rng(0xF20C_5EED);
    commands.entity(frog).insert((
        Boss::new(&SPEC, home, mats).staged(),
        Frog {
            state: State::Sit,
            vel: Vec2::ZERO,
            facing: -1.0,
            cooldown: rng.range(COOLDOWN.0, COOLDOWN.1),
            rest: rng.range(HOP_REST.0, HOP_REST.1),
            retaliate: false,
            introduced: false,
            sac: 0.0,
            squash: 0.0,
            seed: rng.range(0.0, 100.0),
            dismember: false,
            arena: *arena,
            rng,
        },
    ));
    frog
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
fn on_hit(mut hits: MessageReader<BossHit>, mut frogs: Query<&mut Frog>) {
    for hit in hits.read() {
        let Ok(mut f) = frogs.get_mut(hit.boss) else { continue };
        if hit.killed {
            // Thrown up and over.
            f.vel = Vec2::new(hit.dir.x * 260.0, 480.0);
            f.state = State::Dead { t: 0.0, landed: false };
            f.dismember = true;
        } else if hit.retaliate {
            f.retaliate = true;
        } else if hit.stagger && matches!(f.state, State::Sit | State::Taunt(_) | State::Recover(_)) {
            // Mid-attack or mid-leap it doesn't flinch.
            f.vel = Vec2::new(hit.dir.x * 120.0, 0.0);
            f.state = State::Hurt(0.0);
        }
    }
}

/// `x` kept inside the arena, the whole body on its floor (the middle if it's too narrow).
fn inside(arena: &Arena, x: f32) -> f32 {
    let (lo, hi) = (arena.left + HALF_W, arena.right - HALF_W);
    if lo < hi { x.clamp(lo, hi) } else { (arena.left + arena.right) * 0.5 }
}

/// Where it would land leaping toward `x` from `feet`: on the arena floor.
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

fn think(
    time: Res<Time>,
    mut commands: Commands,
    mut shake: ResMut<Shake>,
    mut stop: ResMut<HitStop>,
    mut bleed: MessageWriter<Bleed>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    mut daze: ResMut<Daze>,
    cloud: Res<Cloud>,
    clouds: Query<&Poison>,
    player: Single<(&mut Player, &Transform), Without<Frog>>,
    camera: Single<(&Camera, &GlobalTransform), With<MainCamera>>,
    mut frogs: Query<(&mut Frog, &mut Boss, &mut Transform, Option<&mut boss::Sense>)>,
    terrain: Res<Terrain>,
) {
    let dt = time.delta_secs().min(1.0 / 30.0) * crate::boss::tempo();
    let t = time.elapsed_secs();
    let (mut p, ptf) = player.into_inner();
    let ppos = ptf.translation.truncate();
    let pfeet = ppos - Vec2::Y * player::HALF.y;

    for (mut f, mut boss, mut tf, sense) in &mut frogs {
        let pos = tf.translation.truncate();
        let to_player = ppos - pos;
        let alive = p.dead.is_none();
        let aggro = alive && boss.awake;
        let arena = f.arena;
        let ground_at = |x: f32, y: f32| terrain.surface(Vec2::new(x, y - FOOT + 2.0 * STEP_UP), 3000.0);
        let floor = ground_at(pos.x, pos.y).unwrap_or(arena.floor);
        let feet = Vec2::new(pos.x, floor);
        if let Some(mut s) = sense {
            s.goal = aggro.then_some(ppos);
            s.target = match f.state {
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
        if !f.introduced {
            boss.staged = true;
        }
        let enraged = boss.enraged();
        let pace = if enraged { RAGE_PACE } else { 1.0 };
        let mouth = pos + Vec2::new(22.0 * SCALE * f.facing, -2.0 * SCALE);

        // Hit too many times in a row: answers at once, or leaps out of reach.
        if std::mem::take(&mut f.retaliate) && matches!(f.state, State::Sit | State::Hurt(_) | State::Recover(_) | State::Taunt(_)) {
            f.facing = side;
            f.state = if to_player.length() < RETALIATE_REACH && f.rng.chance(0.5) {
                State::Inflate(INFLATE * 0.5)
            } else {
                let away = pos.x - side * EVADE_LENGTH;
                leap(&arena, &terrain, feet, away, EVADE_HEIGHT, EVADE_TIME, Leap::Evade)
            };
        }

        f.state = match f.state {
            State::Dead { t: t0, landed } => {
                let td = t0 + dt;
                for at in [0.3, 0.7, 1.2] {
                    if t0 < at && td >= at {
                        bleed.write(Bleed { pos, dir: Vec2::Y, spread: PI, speed: 520.0, amount: 60, fluid: Fluid::Violet, sticky: true });
                    }
                }
                let mut landed = landed;
                let mut next = pos;
                if !landed {
                    f.vel.y -= 1400.0 * dt;
                    next += f.vel * dt;
                    // Belly-up: its back on the floor.
                    let rest = floor + 9.0 * SCALE;
                    if ground_at(next.x, pos.y).is_some() && next.y <= rest && f.vel.y < 0.0 {
                        next.y = rest;
                        f.vel = Vec2::new(f.vel.x * 0.3, 0.0);
                        landed = true;
                        shake.add(0.3);
                        bleed.write(Bleed { pos, dir: Vec2::Y, spread: 1.3, speed: 260.0, amount: 40, fluid: Fluid::Violet, sticky: true });
                    }
                } else {
                    f.vel.x *= 1.0 - 5.0 * dt;
                    next.x += f.vel.x * dt;
                }
                if td < 3.5 && f.rng.chance(dt * 12.0) {
                    bleed.write(Bleed { pos, dir: Vec2::Y, spread: 1.0, speed: 140.0, amount: 2, fluid: Fluid::Violet, sticky: true });
                }
                tf.translation.x = next.x;
                tf.translation.y = next.y;
                State::Dead { t: td, landed }
            }
            State::Hurt(s) => {
                f.vel.x *= 1.0 - 6.0 * dt;
                if s < 0.3 { State::Hurt(s + dt) } else { State::Sit }
            }
            State::Sit => {
                f.vel.x = 0.0;
                f.facing = if aggro { side } else { f.facing };
                f.cooldown -= dt;
                f.rest -= dt;
                let dx = to_player.x.abs();
                if !aggro {
                    f.introduced = false;
                    // Hops back home when it lost the chameleon.
                    if f.rest <= 0.0 && (boss.home.x - pos.x).abs() > 60.0 {
                        f.rest = f.rng.range(HOP_REST.0, HOP_REST.1);
                        f.facing = (boss.home.x - pos.x).signum();
                        let x = pos.x + (boss.home.x - pos.x).clamp(-HOP_LENGTH, HOP_LENGTH);
                        leap(&arena, &terrain, feet, x, HOP_HEIGHT, HOP_TIME, Leap::Hop)
                    } else {
                        State::Sit
                    }
                } else if !f.introduced {
                    if on_screen {
                        f.introduced = true;
                        boss.staged = false;
                        shake.add(0.2);
                        State::Taunt(0.0)
                    } else {
                        State::Sit
                    }
                } else if f.cooldown <= 0.0 {
                    f.cooldown = f.rng.range(COOLDOWN.0, COOLDOWN.1) * pace;
                    // Close up it would rather stomp; poison is its main weapon.
                    let poison = if dx < CLOSE { 0.3 } else { POISON_SHARE };
                    let crowded = clouds.iter().filter(|c| c.harmful).count() >= MAX_CLOUDS;
                    if f.rng.chance(poison) && !crowded { State::Inflate(0.0) } else { State::Crouch(0.0) }
                } else if f.rest <= 0.0 && (dx < KEEP_AWAY - 90.0 || dx > KEEP_AWAY + 120.0) {
                    // Little hops to keep its distance.
                    f.rest = f.rng.range(HOP_REST.0, HOP_REST.1) * pace;
                    let dir = if dx < KEEP_AWAY { -side } else { side };
                    let x = pos.x + dir * HOP_LENGTH * f.rng.range(0.7, 1.1);
                    // Backed against the arena edge: hop over the chameleon's head instead.
                    let x = if (x - landing(&arena, &terrain, feet, x).x).abs() > 30.0 { pos.x + side * (dx + HALF_W + 100.0) } else { x };
                    leap(&arena, &terrain, feet, x, HOP_HEIGHT, HOP_TIME, Leap::Hop)
                } else {
                    State::Sit
                }
            }
            State::Taunt(s) => {
                f.facing = side;
                if s < 0.6 && s + dt >= 0.6 || s < 1.4 && s + dt >= 1.4 {
                    // The croaks.
                    shake.add(0.25);
                }
                if to_player.length() < TAUNT_BREAK {
                    State::Crouch(CROUCH * 0.4)
                } else if s >= TAUNT_TIME {
                    f.cooldown = f.rng.range(0.3, 0.7);
                    State::Sit
                } else {
                    State::Taunt(s + dt)
                }
            }
            State::Inflate(s) => {
                f.facing = side;
                if s >= INFLATE {
                    // Belch: thrown to stop right where the chameleon stands.
                    let life = if enraged { CLOUD_LIFE_RAGE } else { CLOUD_LIFE };
                    let spots: &[f32] = if enraged { &[0.0, -CLOUD_FAN, CLOUD_FAN] } else { &[0.0] };
                    for &dx in spots {
                        let aim = ppos + Vec2::new(dx, 0.0);
                        let vel = ((aim - mouth) * CLOUD_DRAG).clamp_length_max(CLOUD_MAX_SPEED);
                        spawn_cloud(&mut commands, &mut materials, &cloud, &mut f.rng, mouth, vel, life, true);
                    }
                    shake.add(0.15);
                    State::Belch(0.0)
                } else {
                    State::Inflate(s + dt)
                }
            }
            State::Belch(s) => {
                if s >= BELCH { State::Sit } else { State::Belch(s + dt) }
            }
            State::Crouch(s) => {
                f.facing = side;
                if s >= CROUCH * pace.max(0.8) {
                    // Commits to where the chameleon is now.
                    let x = pos.x + (ppos.x - pos.x).clamp(-STOMP_REACH, STOMP_REACH);
                    let time = if enraged { STOMP_TIME * 0.85 } else { STOMP_TIME };
                    leap(&arena, &terrain, feet, x, STOMP_HEIGHT, time, Leap::Stomp)
                } else {
                    State::Crouch(s + dt)
                }
            }
            State::Air { from, to, height, time, t: s, kind } => {
                let s = s + dt;
                let k = (s / time).min(1.0);
                let next = Vec2::new(from.x.lerp(to.x, k), from.y.lerp(to.y, k) + FOOT + height * 4.0 * k * (1.0 - k));
                f.vel = (next - pos) / dt.max(1e-4);
                f.facing = if (to.x - from.x).abs() > 1.0 && kind != Leap::Evade { (to.x - from.x).signum() } else { f.facing };
                tf.translation.x = next.x;
                tf.translation.y = next.y;
                if k < 1.0 {
                    State::Air { from, to, height, time, t: s, kind }
                } else {
                    f.squash = 1.0;
                    f.vel = Vec2::ZERO;
                    if kind == Leap::Hop {
                        shake.add(0.05);
                        State::Sit
                    } else {
                        // A big landing: crush what is under it, and the shockwave.
                        shake.add(if kind == Leap::Stomp { 0.6 } else { 0.4 });
                        stop.add(0.04);
                        throw_dust(&mut commands, &mut materials, &cloud, &mut f.rng, to);
                        if kind == Leap::Stomp && alive && (ppos.x - to.x).abs() < CRUSH_HALF + player::HALF.x && pfeet.y < to.y + CRUSH_HEIGHT {
                            boss::hurt_player(&mut p, ppos, to + Vec2::Y * FOOT, CRUSH_DAMAGE, "Crushed under the Plague Frog", &mut bleed, (&mut shake, &mut stop));
                        }
                        let standing = p.grounded && p.climbing.is_none() && (pfeet.y - to.y).abs() < SHOCK_HEIGHT;
                        if alive && !p.god.on() && standing && (ppos.x - to.x).abs() < SHOCK_REACH {
                            p.stun = p.stun.max(SHOCK_STUN);
                            daze.0 = SHOCK_STUN;
                            p.vel = Vec2::new(0.0, 160.0);
                            shake.add(0.2);
                        }
                        if kind == Leap::Stomp {
                            State::Recover(0.0)
                        } else {
                            f.cooldown = f.cooldown.min(0.5);
                            State::Sit
                        }
                    }
                }
            }
            State::Recover(s) => {
                let winded = if enraged { RECOVER_RAGE } else { RECOVER };
                if s >= winded {
                    f.rest = 0.2;
                    State::Sit
                } else {
                    State::Recover(s + dt)
                }
            }
        };

        // On the ground: follow the floor under it (and the knock of a hit).
        if !f.airborne() && !matches!(f.state, State::Dead { .. }) {
            let x = inside(&arena, tf.translation.x + f.vel.x * dt);
            let floor = ground_at(x, tf.translation.y).unwrap_or(floor);
            tf.translation.x = x;
            tf.translation.y = floor + FOOT;
        }

        let sac = match f.state {
            State::Inflate(s) => (s / INFLATE).min(1.0),
            State::Belch(s) => (1.0 - s / BELCH * 2.0).max(0.0),
            // The croaks: two big pulses.
            State::Taunt(s) => ((s - 0.2) * PI / 0.8).sin().max(0.0) * if s < 1.8 { 0.85 } else { 0.0 },
            State::Dead { .. } => 0.0,
            // Breathing.
            _ => 0.12 + 0.08 * (t * 2.2 + f.seed).sin(),
        };
        f.sac += (sac - f.sac) * (1.0 - (-14.0 * dt).exp());
        f.squash = (f.squash - dt * 4.0).max(0.0);
    }
}

#[allow(clippy::too_many_arguments)]
fn spawn_cloud(commands: &mut Commands, materials: &mut Assets<ColorMaterial>, cloud: &Cloud, rng: &mut Rng, at: Vec2, vel: Vec2, life: f32, harmful: bool) {
    let tint = Color::srgba(rng.range(0.55, 0.72), rng.range(0.0, 0.12), rng.range(0.85, 1.0), 0.0);
    let material = materials.add(ColorMaterial { color: tint, alpha_mode: AlphaMode2d::Blend, ..default() });
    let parent = commands
        .spawn((Transform::from_translation(at.extend(13.0)), Visibility::default(), Poison { vel, age: 0.0, life, harmful, material: material.clone() }, BossStuff))
        .id();
    for i in 0..7 {
        let offset = if i == 0 { Vec2::ZERO } else { Vec2::from_angle(i as f32 * PI / 3.0 + rng.range(-0.3, 0.3)) * rng.range(0.35, 0.6) };
        commands.spawn((
            Mesh2d(cloud.0.clone()),
            MeshMaterial2d(material.clone()),
            Transform::from_xyz(0.0, 0.0, i as f32 * 0.01),
            camera::color_layer(),
            Puff { offset, phase: rng.range(0.0, PI * 2.0), size: rng.range(0.75, 1.1) },
            ChildOf(parent),
        ));
    }
}

/// The clouds drift to a stop, swell, swirl and fade; inside one the chameleon is poisoned.
fn poison(
    time: Res<Time>,
    mut commands: Commands,
    mut materials: ResMut<Assets<ColorMaterial>>,
    mut bleed: MessageWriter<Bleed>,
    mut shake: ResMut<Shake>,
    terrain: Res<Terrain>,
    player: Single<(&mut Player, &Transform)>,
    frogs: Query<&Frog>,
    mut clouds: Query<(Entity, &mut Poison, &mut Transform, &Children), Without<Player>>,
    mut puffs: Query<(&Puff, &mut Transform), (Without<Poison>, Without<Player>)>,
) {
    let dt = time.delta_secs();
    let now = time.elapsed_secs();
    let (mut p, ptf) = player.into_inner();
    let ppos = ptf.translation.truncate();
    // A dead frog's poison thins out fast and stops hurting.
    let frog_alive = frogs.iter().any(|f| !matches!(f.state, State::Dead { .. }));
    for (e, mut c, mut tf, children) in &mut clouds {
        if !frog_alive && c.harmful {
            c.harmful = false;
            c.life = c.life.min(c.age + 1.5);
        }
        c.age += dt;
        let k = c.age / c.life;
        if k >= 1.0 {
            commands.entity(e).despawn();
            continue;
        }
        let pos = tf.translation.truncate();
        let step = c.vel * dt;
        if terrain.solid_at(pos + step) {
            c.vel = Vec2::ZERO;
        } else {
            tf.translation += step.extend(0.0);
        }
        c.vel *= (-CLOUD_DRAG * dt).exp();
        // Swells to full size quickly, then breathes.
        let radius = CLOUD_RADIUS * (0.35 + 0.65 * (c.age / 0.6).min(1.0)) * (1.0 + 0.06 * (now * 1.7 + (e.to_bits() % 97) as f32).sin());
        let fade = (c.age / 0.25).min(1.0) * (1.0 - ((k - 0.75) / 0.25).clamp(0.0, 1.0));
        if let Some(mut m) = materials.get_mut(&c.material) {
            m.color = m.color.with_alpha(0.32 * fade);
        }
        for child in children.iter() {
            let Ok((puff, mut ptf)) = puffs.get_mut(child) else { continue };
            let swirl = Vec2::from_angle(now * 0.6 + puff.phase).rotate(puff.offset);
            ptf.translation.x = swirl.x * radius;
            ptf.translation.y = swirl.y * radius * 0.75;
            ptf.scale = Vec3::splat(radius * puff.size * (1.0 + 0.1 * (now * 2.3 + puff.phase).sin()));
        }
        // Inside it: poisoned (one tick per invulnerability window).
        if c.harmful && fade > 0.5 && p.dead.is_none() && ppos.distance(tf.translation.truncate()) < radius * 0.8 {
            let stun = p.stun;
            let keep = p.vel;
            match p.hurt(POISON_DAMAGE, keep, "Poisoned by the Plague Frog") {
                Hurt::Ignored => {}
                hurt => {
                    // Poison doesn't knock you about.
                    p.stun = stun;
                    shake.add(0.15);
                    let amount = if matches!(hurt, Hurt::Killed) { 90 } else { 14 };
                    bleed.write(Bleed { pos: ppos, dir: Vec2::Y, spread: 1.2, speed: 260.0, amount, fluid: Fluid::Red, sticky: false });
                }
            }
        }
    }
}

/// A big landing kicks dust out along the floor both ways (the shockwave).
fn throw_dust(commands: &mut Commands, materials: &mut Assets<ColorMaterial>, cloud: &Cloud, rng: &mut Rng, at: Vec2) {
    for side in [-1.0, 1.0] {
        for _ in 0..8 {
            let material = materials.add(ColorMaterial { color: Color::srgba(0.62, 0.6, 0.55, 0.0), alpha_mode: AlphaMode2d::Blend, ..default() });
            commands.spawn((
                Mesh2d(cloud.0.clone()),
                MeshMaterial2d(material.clone()),
                Transform::from_translation((at + Vec2::new(side * rng.range(0.2, 0.9) * HALF_W, rng.range(6.0, 30.0))).extend(13.5)),
                Dust { vel: Vec2::new(side * rng.range(450.0, 1000.0), rng.range(10.0, 70.0)), age: 0.0, life: rng.range(0.5, 0.75), size: rng.range(35.0, 65.0), material },
                BossStuff,
            ));
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

/// The chameleon came back: start the fight over (`boss.rs` heals it), the poison cleared.
fn reset_on_respawn(mut commands: Commands, mut respawned: MessageReader<Respawned>, mut frogs: Query<&mut Frog>, clouds: Query<Entity, With<Poison>>) {
    if respawned.read().count() == 0 {
        return;
    }
    for e in &clouds {
        commands.entity(e).despawn();
    }
    for mut f in &mut frogs {
        if matches!(f.state, State::Dead { .. }) {
            continue;
        }
        f.state = State::Sit;
        f.introduced = false;
        f.vel = Vec2::ZERO;
        f.cooldown = 1.5;
    }
}

/// A freshly killed frog: the sac bursts (a last, harmless cloud) and the near hind leg tears off.
fn dismember(
    mut commands: Commands,
    mut materials: ResMut<Assets<ColorMaterial>>,
    cloud: Res<Cloud>,
    mut frogs: Query<(&mut Frog, &Transform)>,
    bones: Query<(Entity, &Bone, &GlobalTransform)>,
) {
    let mut done = Vec::new();
    for (e, bone, gt) in &bones {
        let Ok((f, _)) = frogs.get(bone.owner) else { continue };
        if !f.dismember {
            continue;
        }
        done.push(bone.owner);
        match bone.kind {
            BoneKind::Thigh { near: true } => {
                commands.entity(e).remove::<Bone>();
                boss::tear_off(&mut commands, e, gt, Vec2::new(-f.facing * 260.0, 560.0), 9.0, Some(Fluid::Violet));
            }
            BoneKind::Sac => {
                commands.entity(e).insert(Visibility::Hidden);
            }
            _ => {}
        }
    }
    done.sort();
    done.dedup();
    for owner in done {
        if let Ok((mut f, tf)) = frogs.get_mut(owner) {
            f.dismember = false;
            let at = tf.translation.truncate() + Vec2::new(13.0 * SCALE * f.facing, -6.0 * SCALE);
            spawn_cloud(&mut commands, &mut materials, &cloud, &mut f.rng, at, Vec2::Y * 60.0, 3.0, false);
        }
    }
}

fn animate(time: Res<Time>, frogs: Query<&Frog>, mut bones: Query<(&Bone, &mut Transform)>) {
    let t = time.elapsed_secs();
    let dt = time.delta_secs();
    for (bone, mut tf) in &mut bones {
        let Ok(f) = frogs.get(bone.owner) else { continue };
        let tt = t + f.seed;
        let (dead_t, landed) = match f.state {
            State::Dead { t, landed } => (Some(t), landed),
            _ => (None, false),
        };
        // 0..1: crouched down (before a stomp, and a little before every hop).
        let crouch = match f.state {
            State::Crouch(s) => (s / CROUCH).min(1.0),
            State::Recover(s) => (1.0 - s / 0.5).clamp(0.0, 1.0) * 0.6,
            _ => f.squash * 0.7,
        };
        // 0..1: legs stretched out (rising), folding back as it comes down.
        let stretch = match f.state {
            State::Air { t: s, time, .. } => (1.0 - (s / time) * 1.4).clamp(0.0, 1.0),
            State::Dead { landed: false, .. } => 0.8,
            _ => 0.0,
        };
        let shiver = if matches!(f.state, State::Crouch(_)) { (tt * 55.0).sin() * 0.03 * crouch } else { 0.0 };
        let open = match f.state {
            State::Belch(s) => (PI * (s / BELCH).min(1.0)).sin(),
            State::Taunt(_) => f.sac * 0.3,
            State::Dead { .. } => 0.5,
            _ => 0.0,
        };
        match bone.kind {
            BoneKind::Rig => {
                let sx = f.facing * SCALE * (1.0 + 0.12 * crouch);
                tf.scale.x += (sx - tf.scale.x) * (1.0 - (-18.0 * dt).exp());
                tf.scale.y = SCALE * (1.0 - 0.18 * crouch + 0.12 * stretch);
                tf.translation.y = -crouch * 3.0 * SCALE;
                let angle = if let Some(td) = dead_t {
                    if landed { PI } else { PI * (td * 2.2).min(1.0) }
                } else {
                    match f.state {
                        // Nose up rising, down falling.
                        State::Air { .. } => (f.vel.y * 0.0007).clamp(-0.35, 0.4),
                        State::Inflate(_) | State::Taunt(_) => 0.12 * f.sac,
                        _ => (tt * 1.3).sin() * 0.01,
                    }
                };
                tf.rotation = Quat::from_rotation_z(angle + shiver);
            }
            BoneKind::Jaw => {
                tf.rotation = Quat::from_rotation_z(-0.55 * open);
            }
            BoneKind::Sac => {
                let s = 1.0 + 1.6 * f.sac;
                tf.scale = Vec3::new(s, s * 1.1, 1.0);
                tf.translation.y = -6.0 - 3.0 * f.sac;
            }
            BoneKind::Eye => {
                // Blinks now and then (and squints crouched).
                let blink = if (tt * 0.7).fract() < 0.04 { 0.15 } else { 1.0 };
                tf.scale.y = if dead_t.is_some() { 0.2 } else { blink * (1.0 - 0.3 * crouch) };
            }
            BoneKind::Thigh { near } => {
                let angle = if let Some(td) = dead_t {
                    let fade = (1.0 - td / 6.0).max(0.1);
                    -0.6 + (tt * 24.0 + near as u8 as f32).sin() * 0.5 * fade
                } else {
                    0.25 * crouch - 1.3 * stretch
                };
                tf.rotation = Quat::from_rotation_z(angle);
            }
            BoneKind::Shin { near } => {
                let angle = if let Some(td) = dead_t {
                    let fade = (1.0 - td / 6.0).max(0.1);
                    0.8 + (tt * 21.0 + near as u8 as f32 * 2.0).sin() * 0.4 * fade
                } else {
                    -0.2 * crouch + 1.7 * stretch
                };
                tf.rotation = Quat::from_rotation_z(angle);
            }
            BoneKind::Arm { near } => {
                let angle = if dead_t.is_some() {
                    0.9 + (tt * 18.0 + near as u8 as f32).sin() * 0.3
                } else if f.airborne() {
                    // Reaching forward for the landing.
                    0.9 * (1.0 - stretch) + 0.3
                } else {
                    -0.3 * crouch
                };
                tf.rotation = Quat::from_rotation_z(angle);
            }
        }
    }
}

/// Stunned by a shockwave: little stars circle over the chameleon's head (a
/// tilted ring: the ones behind smaller), spinning and twinkling, until it
/// comes to (or dies, or the stun is cut short).
fn stars(
    time: Res<Time>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    mut daze: ResMut<Daze>,
    mut mesh: Local<Option<(Handle<Mesh>, Handle<ColorMaterial>)>>,
    player: Single<(&Player, &Transform)>,
    mut stars: Query<(Entity, &Star, &mut Transform), Without<Player>>,
) {
    let (p, ptf) = *player;
    daze.0 = (daze.0 - time.delta_secs()).max(0.0);
    if p.stun <= 0.0 || p.dead.is_some() {
        daze.0 = 0.0;
    }
    if daze.0 <= 0.0 {
        for (e, ..) in &stars {
            commands.entity(e).despawn();
        }
        return;
    }
    let (mesh, mat) = mesh
        .get_or_insert_with(|| {
            let outline: Vec<Vec2> = (0..10).map(|i| Vec2::from_angle(PI / 2.0 + i as f32 * PI / 5.0) * if i % 2 == 0 { 1.0 } else { 0.45 }).collect();
            (meshes.add(fill(&outline, |p| 0.8 + 0.2 * (1.0 - p.length()))), materials.add(STAR_COLOR))
        })
        .clone();
    if stars.is_empty() {
        for i in 0..STARS {
            commands.spawn((Mesh2d(mesh.clone()), MeshMaterial2d(mat.clone()), Transform::from_xyz(0.0, 0.0, 12.0), camera::color_layer(), Star(i)));
        }
    }
    let now = time.elapsed_secs();
    let head = chameleon::neck(ptf.translation.truncate(), p.facing) + Vec2::Y * 16.0;
    // The ring shrinks away in the last moments as it comes to.
    let fade = (daze.0 / 0.25).min(1.0);
    for (_, star, mut tf) in &mut stars {
        let a = now * 5.0 + star.0 as f32 * std::f32::consts::TAU / STARS as f32;
        let depth = 0.75 + 0.25 * a.sin();
        tf.translation.x = head.x + a.cos() * 20.0;
        tf.translation.y = head.y + a.sin() * 6.0;
        tf.translation.z = 12.0 + a.sin() * 0.1;
        let twinkle = 1.0 + 0.2 * (now * 17.0 + star.0 as f32 * 2.1).sin();
        tf.scale = Vec3::splat(5.0 * depth * twinkle * fade);
        tf.rotation = Quat::from_rotation_z(now * 6.0 + star.0 as f32);
    }
}
