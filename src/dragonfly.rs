//! The Razorwing: a giant dragonfly, the fast boss. It bleeds pink, and
//! beating it gives the chameleon its color and the air spin.
//!
//! It never sits still: it hovers a moment near the chameleon, then darts to
//! another spot (often across, over its head) in a blink and stops dead, a
//! few times; then it attacks. The attack is the air spin it gives away:
//! it hangs back and tenses up (the fan it's about to cut shows faintly),
//! commits to where the chameleon was, shoots in and whirls, cutting a wide
//! fan in front of it. Mid-spin nothing touches it (blows glance off). After
//! the spin it's dizzy for a moment, sinking and wobbling: the opening for
//! the claws. Hit outside that, it flinches and darts away. Venom spit
//! clogs its wings: slowed for a while, no darting, no attacks, an easy
//! target. Below half health it's quicker, and dizzy for less.
//! Health, getting hit and the death spectacle live in `boss.rs`.

use std::f32::consts::PI;

use bevy::{prelude::*, sprite_render::AlphaMode2d};

use crate::{
    boss::{self, Arena, Boss, BossHit, BossSpec},
    camera::{HitStop, MainCamera, Shake},
    chameleon,
    gore::{Bleed, Fluid},
    level::Platform,
    player::{self, Player, Respawned},
    shapes::*,
    terrain::Terrain,
    wasp::{clear_line, Nav, NAV_STEP},
};

/// Its color (what the chameleon earns, and the health bar).
const PINK: Color = Color::srgb_u8(0xd1, 0xb0, 0xc6);

pub(crate) static SPEC: BossSpec = BossSpec {
    name: "RAZORWING",
    hp: 10,
    radius: RADIUS,
    blood: Fluid::Pink,
    color: PINK,
    aggro: AGGRO,
    // Light, but a boss.
    food: 3.5,
};
/// Size of the boss relative to its drawing.
const SCALE: f32 = 1.6;
/// Hurtbox radius around the thorax.
const RADIUS: f32 = 17.0 * SCALE;
const AGGRO: f32 = 700.0;
/// Where it likes to hover, relative to the chameleon (x on either side).
const HOVER_RANGE: (f32, f32) = (150.0, 300.0);
const HOVER_HEIGHT: (f32, f32) = (60.0, 200.0);
/// Drifting about a hover spot.
const DRIFT_SPEED: f32 = 240.0;
/// A dart: a straight burst to the next spot, then a dead stop.
const DART_SPEED: f32 = 1000.0;
const DART_MAX: f32 = 0.4;
/// Seconds it hangs at a spot before the next dart.
const HOLD: (f32, f32) = (0.55, 1.0);
/// Darts before an attack.
const DARTS: (u32, u32) = (2, 3);
/// It attacks from at most this far.
const ATTACK_RANGE: f32 = 520.0;
/// Tensing up before the strike (the tell).
const WINDUP: f32 = 0.75;
/// Share of the windup during which it still tracks the chameleon (then it's committed).
const TRACK: f32 = 0.4;
const STRIKE_SPEED: f32 = 760.0;
const STRIKE_MAX: f32 = 0.7;
/// Its spin: like the chameleon's, a wide fan along the strike.
const SPIN_TIME: f32 = 0.45;
const SPIN_REACH: f32 = 100.0;
const SPIN_DAMAGE: i32 = 1;
/// Dizzy after a spin: the opening.
const DIZZY: f32 = 1.6;
/// Venom clogs its wings for this long, and this slow.
const SLOWED: f32 = 2.5;
const SLOW: f32 = 0.45;
/// The face-off the first time it sees the chameleon.
const TAUNT_TIME: f32 = 1.6;
/// The chameleon this close cuts the taunt short.
const TAUNT_BREAK: f32 = 220.0;
/// How far inside the screen edge (px) it must be to count as in view.
const SCREEN_MARGIN: f32 = 60.0;
/// How close to walls its body gets.
const BODY_CLEARANCE: f32 = 20.0;
/// How often it looks for a way around walls when it has no straight line.
const REPLAN: f32 = 0.5;

const BODY: Color = Color::srgb(0.80, 0.22, 0.45);
const DARK: Color = Color::srgb(0.10, 0.06, 0.09);
const EYE: Color = Color::srgb(0.42, 0.08, 0.22);
const WING: Color = Color::srgba(0.92, 0.95, 1.0, 0.26);
const FAN: Color = Color::srgb(1.0, 0.85, 0.93);

pub struct DragonflyPlugin;

impl Plugin for DragonflyPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (reset_on_respawn, on_hit, think, dismember, animate)
                .chain()
                .after(player::move_player)
                .after(chameleon::update_anim_state),
        );
    }
}

#[derive(Component)]
pub struct Dragonfly {
    vel: Vec2,
    state: State,
    facing: f32,
    /// Time offset for its wobble and wingbeats.
    seed: f32,
    rng: Rng,
    /// The spot it hovers at, and how long it stays there.
    spot: Vec2,
    hold: f32,
    /// Darts left before it attacks.
    darts: u32,
    /// Where it committed its strike to, and the strike's direction.
    plan: Vec2,
    dir: Vec2,
    /// Seconds left of venom-clogged wings.
    slowed: f32,
    /// A way around walls (waypoints), when there's no straight line.
    path: Vec<Vec2>,
    replan: f32,
    /// It has faced the chameleon off (until the lair resets it).
    introduced: bool,
    /// Just died: tear its wings off this frame.
    dismember: bool,
}

impl Dragonfly {
    /// Mid-spin (its whirl of blades).
    pub fn spinning(&self) -> bool {
        matches!(self.state, State::Spin { .. })
    }
}

#[derive(Clone, Copy, PartialEq)]
enum State {
    /// Hanging at its spot (seconds there).
    Hover(f32),
    Dart { to: Vec2, t: f32 },
    Windup(f32),
    Strike(f32),
    Spin { t: f32, hit: bool },
    Dizzy(f32),
    Hurt(f32),
    Taunt(f32),
    Dead { t: f32, angle: f32, spin: f32, landed: bool },
}

#[derive(Component)]
struct Bone {
    owner: Entity,
    kind: BoneKind,
}

#[derive(Clone, Copy)]
enum BoneKind {
    Rig,
    Head,
    Abdomen,
    /// `fore`: the front pair; `near`: the side facing the camera.
    Wing { fore: bool, near: bool },
    Leg { offset: f32 },
}

/// The fan its spin cuts (shown faintly while it winds up).
#[derive(Component)]
struct Fan {
    owner: Entity,
    mat: Handle<ColorMaterial>,
}

/// Spawns the dragonfly hovering at its home.
pub fn spawn(commands: &mut Commands, meshes: &mut Assets<Mesh>, materials: &mut Assets<ColorMaterial>, arena: &Arena) -> Entity {
    let home = arena.home;
    let body = materials.add(BODY);
    let dark = materials.add(DARK);
    let eye = materials.add(EYE);
    let wing = materials.add(ColorMaterial { color: WING, alpha_mode: AlphaMode2d::Blend, ..default() });
    let shine = materials.add(Color::WHITE);
    let fan_mat = materials.add(ColorMaterial { color: FAN.with_alpha(0.0), alpha_mode: AlphaMode2d::Blend, ..default() });
    let mats = vec![(body.clone(), BODY), (dark.clone(), DARK), (eye.clone(), EYE)];

    let root = commands.spawn((Transform::from_translation(home.extend(12.0)), Visibility::default())).id();
    let mut b = Builder { commands, meshes, owner: root };
    let rig = b.bone(Vec3::ZERO, root, BoneKind::Rig);
    b.commands.entity(rig).insert(Transform::from_scale(Vec3::new(-SCALE, SCALE, 1.0)));

    // Wings: long and narrow, swept up and back from the thorax; the far
    // pair behind everything, the near pair on top. A dark spot near each tip.
    let wing_mesh = |m: &mut Assets<Mesh>, len: f32| {
        let pts: Vec<Vec2> = ellipse_pts(len * 0.5, 3.4, 26).into_iter().map(|p| p + Vec2::X * len * 0.5).collect();
        m.add(fill(&pts, |p| 0.75 + 0.25 * (p.x / len)))
    };
    let mut wings = Vec::new();
    for near in [false, true] {
        for (fore, x, len) in [(true, 3.0, 38.0), (false, -3.0, 34.0)] {
            let z = if near { 0.5 } else { -0.4 };
            let bone = b.bone(Vec3::new(x, 6.0, z + if fore { 0.01 } else { 0.0 }), rig, BoneKind::Wing { fore, near });
            let mesh = wing_mesh(b.meshes, len);
            b.part_handle(mesh, &wing, Vec3::ZERO, bone);
            b.part(fill(&ellipse_pts(2.6, 1.4, 10), |_| 1.0), &dark, Vec3::new(len - 5.0, 0.0, 0.01), bone);
            wings.push(bone);
        }
    }

    // Legs: thin, held forward in a basket under the thorax.
    for (k, x) in [3.0, 0.0, -3.0].into_iter().enumerate() {
        let leg = b.bone(Vec3::new(x, -5.0, -0.2), rig, BoneKind::Leg { offset: k as f32 * 1.3 });
        let path = smooth_open(&[Vec2::ZERO, Vec2::new(3.0, -6.0), Vec2::new(7.0, -9.0), Vec2::new(10.0, -8.0)], 5);
        b.part(stroke(&path, |t| 1.5 - 0.7 * t, 1.0, 1.0, |_| 1.0), &dark, Vec3::ZERO, leg);
    }

    // Abdomen: long and thin, segmented, ending in claspers.
    let abdomen = b.bone(Vec3::new(-7.0, 0.0, 0.0), rig, BoneKind::Abdomen);
    let spine = smooth_open(&[Vec2::ZERO, Vec2::new(-18.0, -1.0), Vec2::new(-38.0, -2.5), Vec2::new(-58.0, -1.0)], 8);
    b.part(
        stroke(&spine, |t| 6.0 * (1.0 - t) + 2.6, 1.0, 0.7, |t| 1.0 - 0.35 * (0.5 + 0.5 * (t * 9.0 * 2.0 * PI).cos()).powi(12)),
        &body,
        Vec3::ZERO,
        abdomen,
    );
    b.part(
        triangles(&[
            [Vec2::new(-57.0, 0.5), Vec2::new(-57.0, -2.5), Vec2::new(-65.0, 1.5)],
            [Vec2::new(-57.0, -0.5), Vec2::new(-57.0, -2.5), Vec2::new(-64.0, -4.5)],
        ]),
        &dark,
        Vec3::new(0.0, 0.0, -0.02),
        abdomen,
    );

    // Thorax, with a dark stripe.
    b.part(fill(&ellipse_pts(9.5, 7.0, 28), |p| 0.7 + 0.3 * ((p.y + 7.0) / 14.0)), &body, Vec3::new(0.0, 0.0, 0.1), rig);
    b.part(
        stroke(&smooth_open(&[Vec2::new(-6.0, 3.0), Vec2::new(0.0, 5.5), Vec2::new(6.0, 3.5)], 5), |_| 1.6, 1.0, 1.0, |_| 1.0),
        &dark,
        Vec3::new(0.0, 0.0, 0.12),
        rig,
    );

    // Head: almost all eye, a little jaw.
    let head = b.bone(Vec3::new(10.0, 1.0, 0.2), rig, BoneKind::Head);
    b.part(fill(&ellipse_pts(5.0, 5.5, 20), |p| 0.7 + 0.3 * ((p.y + 5.5) / 11.0)), &body, Vec3::ZERO, head);
    b.part(fill(&ellipse_pts(6.5, 6.8, 24), |p| 0.75 + 0.25 * (p.y / 6.8)), &eye, Vec3::new(2.5, 2.0, 0.05), head);
    b.part(fill(&ellipse_pts(1.2, 1.6, 8), |_| 1.0), &shine, Vec3::new(4.5, 5.0, 0.06), head);
    b.part(triangles(&[[Vec2::new(3.0, -3.5), Vec2::new(8.0, -4.5), Vec2::new(4.0, -6.5)]]), &dark, Vec3::new(0.0, 0.0, 0.07), head);

    // The fan of its spin, in world units (it doesn't turn with the body).
    let fan = b.commands.spawn((Fan { owner: root, mat: fan_mat.clone() }, Transform::from_xyz(0.0, 0.0, 0.8), Visibility::Hidden, ChildOf(root))).id();
    for (r, w) in [(1.0, 10.0), (0.7, 6.0)] {
        let arc: Vec<Vec2> = (0..=40).map(|i| Vec2::from_angle(lerp(chameleon::SPIN_FAN, -chameleon::SPIN_FAN, i as f32 / 40.0)) * SPIN_REACH * r).collect();
        b.part(stroke(&arc, |t| 0.5 + w * (PI * t).sin().powf(0.6), 1.0, 0.5, |_| 1.0), &fan_mat, Vec3::ZERO, fan);
    }

    let mut rng = Rng(0xD7A6_0F17);
    commands.entity(root).insert((
        Boss::new(&SPEC, home, mats).staged(),
        Dragonfly {
            vel: Vec2::ZERO,
            state: State::Hover(0.0),
            facing: -1.0,
            seed: rng.range(0.0, 100.0),
            spot: home,
            hold: rng.range(HOLD.0, HOLD.1),
            darts: DARTS.0,
            plan: home,
            dir: Vec2::X,
            slowed: 0.0,
            path: Vec::new(),
            replan: 0.0,
            introduced: false,
            dismember: false,
            rng,
        },
    ));
    root
}

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

struct Builder<'a, 'w, 's> {
    commands: &'a mut Commands<'w, 's>,
    meshes: &'a mut Assets<Mesh>,
    owner: Entity,
}

impl Builder<'_, '_, '_> {
    fn part(&mut self, mesh: Mesh, mat: &Handle<ColorMaterial>, pos: Vec3, parent: Entity) -> Entity {
        let mesh = self.meshes.add(mesh);
        self.part_handle(mesh, mat, pos, parent)
    }

    fn part_handle(&mut self, mesh: Handle<Mesh>, mat: &Handle<ColorMaterial>, pos: Vec3, parent: Entity) -> Entity {
        self.commands.spawn((Mesh2d(mesh), MeshMaterial2d(mat.clone()), Transform::from_translation(pos), ChildOf(parent))).id()
    }

    fn bone(&mut self, pos: Vec3, parent: Entity, kind: BoneKind) -> Entity {
        self.commands
            .spawn((Bone { owner: self.owner, kind }, Transform::from_translation(pos), Visibility::default(), ChildOf(parent)))
            .id()
    }
}

/// A spot to dart to near the chameleon at `ppos`: in the open, reachable in
/// a straight line, with a view of the chameleon. Often across, over its head.
fn dart_spot(terrain: &Terrain, rng: &mut Rng, pos: Vec2, ppos: Vec2) -> Option<Vec2> {
    let here = if pos.x >= ppos.x { 1.0 } else { -1.0 };
    (0..16).find_map(|_| {
        let side = if rng.chance(0.55) { -here } else { here };
        let q = ppos + Vec2::new(side * rng.range(HOVER_RANGE.0, HOVER_RANGE.1), rng.range(HOVER_HEIGHT.0, HOVER_HEIGHT.1));
        let ok = (q - pos).length() > 90.0
            && terrain.clearance(q) >= BODY_CLEARANCE + 14.0
            && clear_line(terrain, pos, q, BODY_CLEARANCE)
            && terrain.raycast(q, ppos).is_none();
        ok.then_some(q)
    })
}

/// A straight way in from `from` to the chameleon at `ppos` (it stops short:
/// the spin reaches the rest).
fn strike_lane(terrain: &Terrain, from: Vec2, ppos: Vec2) -> bool {
    let back = (from - ppos).normalize_or(Vec2::Y);
    clear_line(terrain, from, ppos + back * SPIN_REACH * 0.5, BODY_CLEARANCE)
}

/// Gets out of reach: a dart to a spot, else just hover where it is.
fn dart_away(d: &mut Dragonfly, terrain: &Terrain, pos: Vec2, ppos: Vec2) -> State {
    match dart_spot(terrain, &mut d.rng, pos, ppos) {
        Some(to) => State::Dart { to, t: 0.0 },
        None => {
            d.spot = pos;
            d.replan = 0.0;
            State::Hover(0.0)
        }
    }
}

fn think(
    time: Res<Time>,
    mut shake: ResMut<Shake>,
    mut stop: ResMut<HitStop>,
    mut bleed: MessageWriter<Bleed>,
    player: Single<(&mut Player, &Transform), Without<Dragonfly>>,
    camera: Single<(&Camera, &GlobalTransform), With<MainCamera>>,
    mut flies: Query<(Entity, &mut Dragonfly, &mut Boss, &mut Transform, Option<&mut boss::Sense>)>,
    platforms: Query<(&Platform, &Transform), (Without<Dragonfly>, Without<Player>)>,
    terrain: Res<Terrain>,
    mut commands: Commands,
    mut nav: Local<Nav>,
) {
    let dt = time.delta_secs().min(1.0 / 30.0) * crate::boss::tempo();
    let t = time.elapsed_secs();
    let (mut p, ptf) = player.into_inner();
    let ppos = ptf.translation.truncate();

    for (e, mut d, mut boss, mut tf, sense) in &mut flies {
        let pos = tf.translation.truncate();
        let to_player = ppos - pos;
        let alive = p.dead.is_none();
        let hunting = alive && boss.awake;
        let aggro = hunting && d.introduced;
        let enraged = boss.enraged();
        let side = if to_player.x >= 0.0 { 1.0 } else { -1.0 };
        let on_screen = match (camera.0.world_to_viewport(camera.1, pos.extend(0.0)), camera.0.logical_viewport_size()) {
            (Ok(v), Some(size)) => v.x > SCREEN_MARGIN && v.y > SCREEN_MARGIN && v.x < size.x - SCREEN_MARGIN && v.y < size.y - SCREEN_MARGIN,
            _ => false,
        };
        if !d.introduced {
            boss.staged = true;
            if hunting && on_screen && matches!(d.state, State::Hover(_) | State::Hurt(_)) {
                // First sight: the face-off.
                d.introduced = true;
                boss.staged = false;
                d.state = State::Taunt(0.0);
                shake.add(0.15);
            }
        }
        d.slowed = (d.slowed - dt).max(0.0);
        let slow = if d.slowed > 0.0 { SLOW } else { 1.0 };
        let quick = if enraged { 1.15 } else { 1.0 };
        let sees = terrain.raycast(pos, ppos).is_none();
        if let Some(mut s) = sense {
            s.goal = Some(d.spot);
            s.target = Some(d.path.first().copied().unwrap_or(d.spot));
            s.sees_player = sees;
            s.path.clone_from(&d.path);
        }
        // Mid-spin, blows glance off.
        boss.guard = matches!(d.state, State::Spin { .. });

        d.state = match d.state {
            State::Dead { t: t0, angle, spin, landed } => {
                let td = t0 + dt;
                // Death throes: bursts while it falls.
                for at in [0.25, 0.6] {
                    if t0 < at && td >= at {
                        bleed.write(Bleed { pos, dir: Vec2::Y, spread: PI, speed: 560.0, amount: 60, fluid: Fluid::Pink, sticky: true });
                        shake.add(0.3);
                    }
                }
                let mut landed = landed;
                if !landed {
                    d.vel.y -= 1400.0 * dt;
                    let next = pos + d.vel * dt;
                    if let Some(top) = boss::floor(&terrain, platforms.iter(), pos, next, 6.0 * SCALE) {
                        tf.translation.x = next.x;
                        tf.translation.y = top + 6.0 * SCALE;
                        d.vel = Vec2::ZERO;
                        landed = true;
                        shake.add(0.1);
                        bleed.write(Bleed { pos, dir: Vec2::Y, spread: 1.3, speed: 240.0, amount: 30, fluid: Fluid::Pink, sticky: true });
                    } else {
                        tf.translation.x = next.x;
                        tf.translation.y = next.y;
                    }
                }
                if td < 3.0 && d.rng.chance(dt * if landed { 8.0 } else { 25.0 }) {
                    bleed.write(Bleed {
                        pos: tf.translation.truncate(),
                        dir: Vec2::new(0.0, if landed { 1.0 } else { -1.0 }),
                        spread: 1.0,
                        speed: 140.0,
                        amount: 2,
                        fluid: Fluid::Pink,
                        sticky: true,
                    });
                }
                if tf.translation.y < terrain.bounds.min.y - 600.0 {
                    commands.entity(e).despawn();
                }
                let angle = if landed { angle + (PI - angle) * (1.0 - (-8.0 * dt).exp()) } else { angle + spin * dt };
                State::Dead { t: td, angle, spin, landed }
            }
            State::Taunt(s) => {
                // Hangs facing the chameleon, rearing up, wings roaring.
                d.facing = side;
                d.vel = d.vel.lerp(Vec2::new(0.0, (t * 3.0 + d.seed).sin() * 20.0), 1.0 - (-6.0 * dt).exp());
                if !alive {
                    State::Hover(0.0)
                } else if to_player.length() < TAUNT_BREAK && strike_lane(&terrain, pos, ppos) {
                    d.plan = ppos;
                    State::Windup(0.0)
                } else if s >= TAUNT_TIME {
                    d.darts = 1;
                    dart_away(&mut d, &terrain, pos, ppos)
                } else {
                    State::Taunt(s + dt)
                }
            }
            State::Hurt(s) => {
                d.vel *= 1.0 - 5.0 * dt;
                if s > 0.25 {
                    // Skittish: off it goes.
                    if aggro && d.slowed <= 0.0 { dart_away(&mut d, &terrain, pos, ppos) } else { State::Hover(0.0) }
                } else {
                    State::Hurt(s + dt)
                }
            }
            State::Dart { to, t: s } => {
                let off = to - pos;
                let step = DART_SPEED * quick * slow * dt;
                if off.length() <= step * 1.2 || s > DART_MAX || !alive {
                    // A dead stop.
                    d.vel *= 0.12;
                    d.spot = to;
                    d.path.clear();
                    d.hold = d.rng.range(HOLD.0, HOLD.1) / quick;
                    State::Hover(0.0)
                } else {
                    d.vel = off.normalize() * DART_SPEED * quick * slow;
                    if d.vel.x.abs() > 20.0 {
                        d.facing = d.vel.x.signum();
                    }
                    State::Dart { to, t: s + dt }
                }
            }
            State::Windup(s) => {
                // Hangs back, tense, still tracking at first; then committed.
                let windup = if enraged { WINDUP * 0.75 } else { WINDUP };
                if s < windup * TRACK {
                    d.plan = ppos;
                }
                let to = d.plan - pos;
                d.facing = if to.x >= 0.0 { 1.0 } else { -1.0 };
                d.dir = to.normalize_or(Vec2::X * d.facing);
                d.vel = d.vel.lerp(-d.dir * 45.0, 1.0 - (-6.0 * dt).exp());
                if !alive {
                    State::Hover(0.0)
                } else if s >= windup {
                    d.vel = d.dir * STRIKE_SPEED * 0.4;
                    State::Strike(0.0)
                } else {
                    State::Windup(s + dt)
                }
            }
            State::Strike(s) => {
                d.vel = d.vel.lerp(d.dir * STRIKE_SPEED * quick, 1.0 - (-10.0 * dt).exp());
                let ahead = pos + d.dir * (BODY_CLEARANCE + 10.0);
                let arrived = (d.plan - pos).length() < SPIN_REACH * 0.75 || to_player.length() < SPIN_REACH * 0.75;
                if arrived || s > STRIKE_MAX || terrain.clearance(ahead) < BODY_CLEARANCE * 0.75 {
                    d.vel *= 0.3;
                    State::Spin { t: 0.0, hit: false }
                } else {
                    State::Strike(s + dt)
                }
            }
            State::Spin { t: s, hit } => {
                d.vel *= (-9.0 * dt).exp();
                let cutting = (0.05..SPIN_TIME * 0.85).contains(&s);
                let mut hit = hit;
                if !hit && alive && cutting && chameleon::in_fan(pos, d.dir, SPIN_REACH, chameleon::SPIN_FAN, ppos, player::HALF.x) {
                    hit = true;
                    boss::hurt_player(&mut p, ppos, pos, SPIN_DAMAGE, &mut bleed, (&mut shake, &mut stop));
                }
                if s >= SPIN_TIME {
                    // Reels back out of its own spin.
                    d.vel = -d.dir * 90.0 + Vec2::Y * 50.0;
                    State::Dizzy(0.0)
                } else {
                    State::Spin { t: s + dt, hit }
                }
            }
            State::Dizzy(s) => {
                // Sinks and wobbles, slow: the opening (not into the ground, though).
                let mut want = Vec2::new((t * 3.0 + d.seed).sin() * 35.0, -40.0);
                if terrain.clearance(pos + Vec2::NEG_Y * 30.0) < BODY_CLEARANCE {
                    want.y = 0.0;
                }
                d.vel = d.vel.lerp(want, 1.0 - (-2.5 * dt).exp());
                let dizzy = if enraged { DIZZY * 0.7 } else { DIZZY };
                if s >= dizzy {
                    d.darts = d.rng.range(DARTS.0 as f32, DARTS.1 as f32 + 0.99) as u32 - u32::from(enraged);
                    if aggro { dart_away(&mut d, &terrain, pos, ppos) } else { State::Hover(0.0) }
                } else {
                    State::Dizzy(s + dt)
                }
            }
            State::Hover(h) => {
                if !aggro {
                    d.spot = boss.home;
                }
                // No straight line to the spot (walls): plan a way around.
                d.replan -= dt;
                if d.replan <= 0.0 {
                    d.replan = REPLAN;
                    if clear_line(&terrain, pos, d.spot, BODY_CLEARANCE) {
                        d.path.clear();
                    } else {
                        let goal = d.spot;
                        let found = nav.plan(&terrain, pos, None, |q, steps| Some((q - goal).length() + steps as f32 * NAV_STEP * 0.3));
                        d.path = found.map(|(_, path)| path).unwrap_or_default();
                    }
                }
                while let Some(&next) = d.path.first() {
                    if (next - pos).length() < NAV_STEP * 0.75 || (d.path.len() >= 2 && clear_line(&terrain, pos, d.path[1], BODY_CLEARANCE + 2.0)) {
                        d.path.remove(0);
                    } else {
                        break;
                    }
                }
                // Clogged wings: it sags.
                let sag = if d.slowed > 0.0 { Vec2::NEG_Y * 50.0 } else { Vec2::ZERO };
                let wobble = Vec2::new((t * 2.3 + d.seed).sin() * 14.0, (t * 3.1 + d.seed).sin() * 9.0);
                let target = d.path.first().copied().unwrap_or(d.spot + wobble + sag);
                let desired = ((target - pos) * 3.0).clamp_length_max(DRIFT_SPEED * slow);
                d.vel = d.vel.lerp(desired, 1.0 - (-5.0 * dt).exp());
                if aggro {
                    d.facing = side;
                } else if d.vel.x.abs() > 20.0 {
                    d.facing = d.vel.x.signum();
                }
                if !aggro || d.slowed > 0.0 || h < d.hold {
                    State::Hover(h + dt)
                } else if d.darts == 0 {
                    // Its turn to attack, with a straight way in; else find a better spot.
                    if to_player.length() < ATTACK_RANGE && strike_lane(&terrain, pos, ppos) {
                        d.plan = ppos;
                        State::Windup(0.0)
                    } else {
                        d.darts = 1;
                        dart_away(&mut d, &terrain, pos, ppos)
                    }
                } else {
                    d.darts -= 1;
                    match dart_spot(&terrain, &mut d.rng, pos, ppos) {
                        Some(to) => State::Dart { to, t: 0.0 },
                        None => {
                            // Nowhere to dart in a straight line: work its way closer.
                            // The line of sight is the costly test and the search visits thousands of
                            // cells: only cast it for a cell that could still beat the best so far.
                            let best = std::cell::Cell::new(f32::INFINITY);
                            let found = nav.plan(&terrain, pos, None, |q, steps| {
                                let off = q - ppos;
                                if off.y <= 0.0 {
                                    return None;
                                }
                                let score = (off.length() - 240.0).abs() + steps as f32 * NAV_STEP * 0.4;
                                if score >= best.get() || terrain.raycast(q, ppos).is_some() {
                                    return None;
                                }
                                best.set(score);
                                Some(score)
                            });
                            if let Some((spot, path)) = found {
                                d.spot = spot;
                                d.path = path;
                            }
                            State::Hover(0.0)
                        }
                    }
                }
            }
        };

        // (Corpses move themselves above.) Walls stop it, one axis at a time.
        if !matches!(d.state, State::Dead { .. }) {
            let mut at = tf.translation.truncate();
            let mut bumped = false;
            for axis in [Vec2::X, Vec2::Y] {
                let next = at + axis * d.vel.dot(axis) * dt;
                let room = terrain.clearance(next);
                if room >= BODY_CLEARANCE || room >= terrain.clearance(at) {
                    at = next;
                } else {
                    let along = d.vel.dot(axis);
                    d.vel -= axis * along;
                    bumped = true;
                }
            }
            if bumped {
                match d.state {
                    State::Dart { .. } => {
                        d.spot = at;
                        d.state = State::Hover(0.0);
                    }
                    State::Strike(_) => d.state = State::Spin { t: 0.0, hit: false },
                    _ => {}
                }
            }
            tf.translation.x = at.x;
            tf.translation.y = at.y;
        }
    }
}

/// React to a hit (health and blood are handled in `boss.rs`).
fn on_hit(mut hits: MessageReader<BossHit>, mut flies: Query<&mut Dragonfly>) {
    for hit in hits.read() {
        let Ok(mut d) = flies.get_mut(hit.boss) else { continue };
        if hit.venom {
            d.slowed = SLOWED;
        }
        if hit.killed {
            d.vel = hit.dir * 380.0 + Vec2::Y * 480.0;
            let spin = -hit.dir.x.signum() * d.rng.range(12.0, 16.0);
            d.state = State::Dead { t: 0.0, angle: 0.0, spin, landed: false };
            d.dismember = true;
        } else if matches!(d.state, State::Dizzy(_)) {
            // Too dazed to get away: just knocked about.
            d.vel += hit.dir * 120.0;
        } else if hit.stagger || hit.retaliate {
            d.vel = hit.dir * 420.0;
            d.state = State::Hurt(0.0);
        }
    }
}

/// The chameleon came back: start over (`boss.rs` heals it).
fn reset_on_respawn(mut respawned: MessageReader<Respawned>, mut flies: Query<(&mut Dragonfly, &Boss)>) {
    if respawned.read().count() == 0 {
        return;
    }
    for (mut d, boss) in &mut flies {
        if matches!(d.state, State::Dead { .. }) {
            continue;
        }
        d.state = State::Hover(0.0);
        d.introduced = false;
        d.vel = Vec2::ZERO;
        d.spot = boss.home;
        d.darts = DARTS.0;
        d.slowed = 0.0;
        d.path.clear();
    }
}

/// Tear the wings off a freshly killed dragonfly.
fn dismember(mut commands: Commands, mut flies: Query<&mut Dragonfly>, bones: Query<(Entity, &Bone, &GlobalTransform)>) {
    let mut done = Vec::new();
    for (e, bone, gt) in &bones {
        let Ok(d) = flies.get(bone.owner) else { continue };
        if !d.dismember {
            continue;
        }
        done.push(bone.owner);
        let f = d.facing;
        let BoneKind::Wing { fore, near } = bone.kind else { continue };
        let k = if near { 1.0 } else { -0.8 };
        let vel = Vec2::new(f * k * if fore { 240.0 } else { 160.0 }, if fore { 680.0 } else { 560.0 });
        commands.entity(e).remove::<Bone>();
        boss::tear_off(&mut commands, e, gt, vel, k * 11.0, None);
    }
    for owner in done {
        if let Ok(mut d) = flies.get_mut(owner) {
            d.dismember = false;
        }
    }
}

fn animate(
    time: Res<Time>,
    flies: Query<(&Dragonfly, &Boss)>,
    mut bones: Query<(&Bone, &mut Transform), Without<Fan>>,
    mut fans: Query<(&Fan, &mut Transform, &mut Visibility)>,
    mut materials: ResMut<Assets<ColorMaterial>>,
) {
    let t = time.elapsed_secs();
    let dt = time.delta_secs();
    for (bone, mut tf) in &mut bones {
        let Ok((d, boss)) = flies.get(bone.owner) else { continue };
        let tt = t + d.seed;
        let dead = matches!(d.state, State::Dead { .. });
        let windup = match d.state {
            State::Windup(s) => (s / if boss.enraged() { WINDUP * 0.75 } else { WINDUP }).min(1.0),
            _ => 0.0,
        };
        let taunt = match d.state {
            State::Taunt(s) => (s / 0.5).min(1.0) * (1.0 - ((s - TAUNT_TIME + 0.4) / 0.4).clamp(0.0, 1.0)),
            _ => 0.0,
        };
        let dizzy = matches!(d.state, State::Dizzy(_));
        let jitter = windup.max(taunt) * (tt * 70.0).sin() * 0.06;
        match bone.kind {
            BoneKind::Rig => {
                let target = d.facing * SCALE;
                tf.scale.x += (target - tf.scale.x) * (1.0 - (-16.0 * dt).exp());
                let angle = match d.state {
                    State::Dead { angle, .. } => angle,
                    // Whirls twice, forward.
                    State::Spin { t: s, .. } => {
                        let k = (s / SPIN_TIME).min(1.0);
                        -4.0 * PI * k * (2.0 - k) * d.facing
                    }
                    // Nose along the strike.
                    State::Strike(_) => (d.dir.y * 0.8).clamp(-0.7, 0.7) * d.facing,
                    // Rears up, tense.
                    State::Windup(_) => 0.35 * windup + jitter,
                    State::Taunt(_) => 0.45 * taunt + jitter,
                    State::Dizzy(_) => (tt * 7.0).sin() * 0.4,
                    _ => (-d.vel.x * d.facing * 0.0006).clamp(-0.35, 0.35) + (d.vel.y * 0.0004).clamp(-0.3, 0.3) * d.facing,
                };
                tf.rotation = Quat::from_rotation_z(angle);
            }
            BoneKind::Wing { fore, near } => {
                if dead {
                    tf.rotation = Quat::from_rotation_z(2.6);
                    tf.scale.y = 1.0;
                    continue;
                }
                // Swept back while darting/striking; a blur when tense; lazy when clogged or dizzy.
                let rate = match d.state {
                    State::Windup(_) | State::Taunt(_) => 140.0,
                    _ if d.slowed > 0.0 || dizzy => 40.0,
                    _ => 95.0,
                };
                let phase = if fore { 0.0 } else { 1.6 } + if near { 0.0 } else { 0.4 };
                let flap = (tt * rate + phase).sin();
                let (base, swing) = match d.state {
                    State::Dart { .. } | State::Strike(_) => (2.7, 0.15),
                    _ => (if fore { 1.75 } else { 2.05 }, 0.55),
                };
                tf.rotation = Quat::from_rotation_z(base + flap * swing);
                tf.scale.y = 0.55 + 0.45 * flap.abs();
            }
            BoneKind::Abdomen => {
                let curl = match d.state {
                    State::Windup(_) => 0.35 * windup,
                    State::Taunt(_) => 0.3 * taunt,
                    State::Dizzy(_) => -0.25,
                    State::Dead { .. } => 0.2,
                    _ => 0.0,
                };
                tf.rotation = Quat::from_rotation_z((tt * 2.5).sin() * 0.05 + curl + jitter);
            }
            BoneKind::Head => {
                tf.rotation = Quat::from_rotation_z(if dead { -0.4 } else { (tt * 1.7).sin() * 0.05 - 0.2 * windup });
            }
            BoneKind::Leg { offset } => {
                let sway = if dead { 1.0 } else { (tt * 3.0 + offset).sin() * 0.12 - d.vel.x * d.facing * 0.0004 };
                tf.rotation = Quat::from_rotation_z(sway);
            }
        }
    }

    // The fan: faint while it winds up (the tell), bright as it spins.
    for (fan, mut tf, mut vis) in &mut fans {
        let Ok((d, boss)) = flies.get(fan.owner) else { continue };
        let (alpha, scale, dir) = match d.state {
            State::Windup(s) => {
                let k = (s / if boss.enraged() { WINDUP * 0.75 } else { WINDUP }).min(1.0);
                (0.18 * k * k, 0.85, d.dir)
            }
            State::Spin { t: s, .. } => {
                let k = (s / SPIN_TIME).min(1.0);
                (0.6 * (1.0 - k).powf(1.2), 0.85 + 0.25 * k, d.dir)
            }
            _ => (0.0, 1.0, d.dir),
        };
        *vis = if alpha > 0.005 { Visibility::Inherited } else { Visibility::Hidden };
        // The fan is a child of the body: in its frame, unscaled.
        tf.rotation = Quat::from_rotation_z(dir.to_angle());
        tf.scale = Vec3::splat(scale);
        if let Some(mut mat) = materials.get_mut(&fan.mat) {
            mat.color.set_alpha(alpha);
        }
    }
}
