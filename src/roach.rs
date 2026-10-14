//! The Gutter King: a giant cockroach boss. Easier than the wasp: it can't
//! fly at first.
//!
//! Phase 1 (above half health), on the ground: stalks the chameleon at a
//! distance, rears up (the telegraph) and dashes across the floor, then
//! stops, winded: the opening to hit it.
//! Phase 2 (half health or less): its wing covers snap open and it takes to
//! the air: hovers clumsily, aims and dashes through the air at the chameleon.
//! Missing sends it crashing into the floor, stunned, before it takes off again.
//!
//! It bleeds very light yellow. On death its antennae and wings tear off and
//! the body lands belly-up, legs twitching. Health, getting hit and the death
//! spectacle live in `boss.rs`.

use std::f32::consts::PI;

use bevy::{prelude::*, sprite_render::AlphaMode2d};

use crate::{
    boss::{self, Arena, Boss, BossHit, BossSpec},
    camera::{HitStop, Shake},
    chameleon,
    gore::{Bleed, Fluid},
    player::{self, Player, Respawned},
    shapes::*,
};

static SPEC: BossSpec = BossSpec {
    name: "THE GUTTER KING",
    hp: 12,
    radius: RADIUS,
    blood: Fluid::Yellow,
    color: Color::srgb(0.93, 0.80, 0.32),
    aggro: AGGRO,
};
/// Size of the boss relative to its drawing.
const SCALE: f32 = 1.5;
/// Hurtbox radius around the body center.
const RADIUS: f32 = 24.0 * SCALE;
/// Body center height above the floor when standing.
const FOOT: f32 = 23.0 * SCALE;
const AGGRO: f32 = 700.0;
const CONTACT_DAMAGE: i32 = 1;

// Phase 1: on the ground.
/// Preferred distance to the chameleon while stalking.
const KEEP_AWAY: f32 = 260.0;
const WALK_SPEED: f32 = 150.0;
/// Rearing up before a dash.
const WINDUP: f32 = 0.7;
const DASH_SPEED: f32 = 780.0;
const DASH_TIME: f32 = 0.9;
/// Winded after a dash (the chance to hit it).
const RECOVER: f32 = 0.9;

// Phase 2: in the air.
const TAKEOFF: f32 = 1.0;
const FLY_HEIGHT: f32 = 170.0;
const FLY_SPEED: f32 = 230.0;
const AIR_WINDUP: f32 = 0.55;
const AIR_DASH_SPEED: f32 = 700.0;
const AIR_DASH_TIME: f32 = 0.8;
/// Stunned on the floor after an air dash that hit the ground.
const CRASH_STUN: f32 = 1.1;

const BROWN: Color = Color::srgb(0.58, 0.35, 0.17);
const DARK: Color = Color::srgb(0.26, 0.15, 0.07);
const RIM: Color = Color::srgb(0.88, 0.72, 0.46);
const LEG: Color = Color::srgb(0.44, 0.27, 0.13);
const EYE: Color = Color::srgb(0.06, 0.04, 0.03);
const WING: Color = Color::srgba(0.92, 0.85, 0.70, 0.35);

pub struct RoachPlugin;

impl Plugin for RoachPlugin {
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
struct Roach {
    vel: Vec2,
    state: State,
    /// Phase 2 has started (it flies from now on).
    flying: bool,
    cooldown: f32,
    facing: f32,
    dash_dir: Vec2,
    /// Walk-cycle phase.
    stride: f32,
    /// Wing covers: 0 = closed, 1 = open.
    wings: f32,
    seed: f32,
    /// Just died: tear off antennae and wings this frame.
    dismember: bool,
    arena: Arena,
    rng: Rng,
}

#[derive(Clone, Copy, PartialEq)]
enum State {
    Stalk,
    Windup(f32),
    Dash(f32),
    Recover(f32),
    Hurt(f32),
    Takeoff(f32),
    Fly,
    AirWindup(f32),
    AirDash(f32),
    Dead { t: f32, landed: bool },
}

impl Roach {
    /// Standing on the arena floor (the rest of the time it flies or falls).
    fn grounded(&self) -> bool {
        match self.state {
            State::Stalk | State::Windup(_) | State::Dash(_) | State::Recover(_) => true,
            State::Hurt(_) => !self.flying,
            _ => false,
        }
    }
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
    Antenna { offset: f32 },
    /// `pair`: 0 = front, 1 = middle, 2 = hind. Tripod gait: alternate groups.
    Leg { pair: usize, near: bool },
    Elytron { near: bool },
    HindWing { near: bool },
}

/// Spawns the roach on the arena floor, right of center, facing left.
pub fn spawn(commands: &mut Commands, meshes: &mut Assets<Mesh>, materials: &mut Assets<ColorMaterial>, arena: &Arena) {
    let brown = materials.add(BROWN);
    let dark = materials.add(DARK);
    let rim = materials.add(RIM);
    let leg = materials.add(LEG);
    let eye = materials.add(EYE);
    let glint = materials.add(Color::WHITE);
    let wing = materials.add(ColorMaterial { color: WING, alpha_mode: AlphaMode2d::Blend, ..default() });
    let mats = vec![(brown.clone(), BROWN), (dark.clone(), DARK), (rim.clone(), RIM), (leg.clone(), LEG)];

    let home = Vec2::new(arena.center() + 150.0, arena.floor + FOOT);
    let roach = commands.spawn((Transform::from_translation(home.extend(12.0)), Visibility::default())).id();
    let mut b = Builder { commands, meshes, owner: roach };
    let rig = b.bone(Vec3::ZERO, roach, BoneKind::Rig);
    b.commands.entity(rig).insert(Transform::from_scale(Vec3::new(-SCALE, SCALE, 1.0)));

    // Legs: spiny, three pairs; the far side darker and behind the body.
    let leg_paths = [
        [Vec2::ZERO, Vec2::new(7.0, -7.0), Vec2::new(14.0, -17.0)],
        [Vec2::ZERO, Vec2::new(2.0, -8.0), Vec2::new(3.0, -17.0)],
        [Vec2::ZERO, Vec2::new(-7.0, -7.0), Vec2::new(-16.0, -17.0)],
    ];
    let hips = [14.0, 0.0, -13.0];
    for near in [false, true] {
        for pair in 0..3 {
            // Both sides come out from under the body; the far side is darker and further back.
            let (mat, z, dx) = if near { (&leg, -0.05, 0.0) } else { (&dark, -0.3, 3.0) };
            let bone = b.bone(Vec3::new(hips[pair] + dx, -6.0, z), rig, BoneKind::Leg { pair, near });
            let [a, k, f] = leg_paths[pair];
            let path = smooth_open(&[a, k, f], 4);
            b.part(stroke(&path, |t| 3.0 - 1.8 * t, 1.0, 0.8, |_| 1.0), mat, Vec3::ZERO, bone);
            // Spines along the shin.
            let spines: Vec<[Vec2; 3]> = [0.3, 0.55, 0.8]
                .into_iter()
                .map(|t| {
                    let p = k.lerp(f, t);
                    let n = (f - k).normalize().perp();
                    [p - (f - k).normalize() * 1.0, p + (f - k).normalize() * 1.0, p + n * 3.0 - (f - k).normalize() * 1.5]
                })
                .collect();
            b.part(triangles(&spines), mat, Vec3::new(0.0, 0.0, 0.01), bone);
        }
    }

    // Hind wings: membranes, folded away (hidden) until it flies.
    for near in [false, true] {
        let z = if near { 0.25 } else { -0.4 };
        let bone = b.bone(Vec3::new(12.0, 8.0, z), rig, BoneKind::HindWing { near });
        let pts: Vec<Vec2> = ellipse_pts(30.0, 9.0, 28).into_iter().map(|p| p + Vec2::new(-28.0, 2.0)).collect();
        b.part(fill(&pts, |p| 0.75 + 0.25 * (p.y / 10.0).clamp(-1.0, 1.0)), &wing, Vec3::ZERO, bone);
        b.commands.entity(bone).insert(Visibility::Hidden);
    }

    // Body: long flat oval with cerci at the back.
    let body = smooth_closed(
        &[
            Vec2::new(30.0, 1.0),
            Vec2::new(27.0, 7.0),
            Vec2::new(18.0, 11.0),
            Vec2::new(4.0, 12.0),
            Vec2::new(-12.0, 11.0),
            Vec2::new(-26.0, 8.0),
            Vec2::new(-36.0, 3.0),
            Vec2::new(-40.0, -1.0),
            Vec2::new(-35.0, -5.0),
            Vec2::new(-22.0, -7.0),
            Vec2::new(-4.0, -8.0),
            Vec2::new(14.0, -7.0),
            Vec2::new(26.0, -4.0),
        ],
        6,
    );
    b.part(fill(&body, |p| 0.55 + 0.45 * ((p.y + 8.0) / 20.0).clamp(0.0, 1.0)), &brown, Vec3::ZERO, rig);
    b.part(
        triangles(&[
            [Vec2::new(-37.0, 1.0), Vec2::new(-38.0, -3.0), Vec2::new(-49.0, -3.0)],
            [Vec2::new(-36.0, -1.0), Vec2::new(-37.0, -4.5), Vec2::new(-46.0, -7.0)],
        ]),
        &dark,
        Vec3::new(0.0, 0.0, -0.05),
        rig,
    );

    // Wing covers (elytra): shiny, hinged at the front, they snap open to fly.
    let elytron = smooth_closed(
        &[
            Vec2::new(0.0, 1.5),
            Vec2::new(-12.0, 5.0),
            Vec2::new(-30.0, 5.0),
            Vec2::new(-46.0, 1.0),
            Vec2::new(-52.0, -3.0),
            Vec2::new(-44.0, -5.0),
            Vec2::new(-28.0, -3.5),
            Vec2::new(-10.0, -2.5),
            Vec2::new(0.0, -1.0),
        ],
        5,
    );
    for near in [false, true] {
        // The far cover sits just behind the near one.
        let (mat, pos) = if near { (&brown, Vec3::new(16.0, 8.0, 0.3)) } else { (&dark, Vec3::new(14.0, 9.5, 0.2)) };
        let bone = b.bone(pos, rig, BoneKind::Elytron { near });
        b.part(fill(&elytron, |p| 0.7 + 0.3 * ((p.y + 5.0) / 10.0)), mat, Vec3::ZERO, bone);
        if near {
            let shine = smooth_open(&[Vec2::new(-6.0, 3.0), Vec2::new(-24.0, 3.8), Vec2::new(-40.0, 1.5)], 5);
            b.part(stroke(&shine, |t| 1.8 * (PI * t).sin(), 1.0, 1.0, |_| 1.0), &rim, Vec3::new(0.0, 0.0, 0.01), bone);
        }
    }

    // Pronotum: a dome-shaped shield over the head, with a pale rim along its top.
    let shield = smooth_closed(
        &[
            Vec2::new(9.0, 3.0),
            Vec2::new(12.0, 10.0),
            Vec2::new(21.0, 13.0),
            Vec2::new(29.0, 10.0),
            Vec2::new(34.0, 2.0),
            Vec2::new(31.0, -2.0),
            Vec2::new(20.0, 0.0),
        ],
        6,
    );
    b.part(fill(&shield, |p| 0.6 + 0.4 * (p.y / 13.0).clamp(0.0, 1.0)), &dark, Vec3::new(0.0, 0.0, 0.35), rig);
    let rim_line = smooth_open(&[Vec2::new(11.0, 8.0), Vec2::new(16.0, 12.0), Vec2::new(22.0, 13.0), Vec2::new(29.0, 10.0), Vec2::new(33.5, 3.0)], 6);
    b.part(stroke(&rim_line, |t| 1.0 + 2.6 * (PI * t).sin(), 1.0, 0.8, |_| 1.0), &rim, Vec3::new(0.0, 0.0, 0.36), rig);

    // Head under the shield: small, with a beady eye, palps and whip antennae.
    let head = b.bone(Vec3::new(30.0, -3.0, 0.32), rig, BoneKind::Head);
    b.part(fill(&ellipse_pts(6.5, 6.0, 20), |p| 0.7 + 0.3 * (p.y / 6.0)), &dark, Vec3::ZERO, head);
    let eye_pts: Vec<Vec2> = ellipse_pts(2.2, 3.4, 14).into_iter().map(|p| Vec2::from_angle(0.3).rotate(p)).collect();
    b.part(fill(&eye_pts, |_| 1.0), &eye, Vec3::new(1.5, 1.5, 0.02), head);
    b.part(fill(&ellipse_pts(0.7, 0.9, 8), |_| 1.0), &glint, Vec3::new(2.2, 2.6, 0.03), head);
    for dy in [0.0, -1.5] {
        let palp = smooth_open(&[Vec2::new(4.0, -3.0 + dy), Vec2::new(7.5, -6.5 + dy), Vec2::new(9.0, -9.0 + dy)], 4);
        b.part(stroke(&palp, |t| 1.6 - 0.8 * t, 1.0, 1.0, |_| 1.0), &dark, Vec3::new(0.0, 0.0, 0.01), head);
    }
    for (offset, z) in [(0.0, -0.05), (1.3, 0.04)] {
        let bone = b.bone(Vec3::new(4.0, 3.0, z), head, BoneKind::Antenna { offset });
        let path = smooth_open(
            &[Vec2::ZERO, Vec2::new(12.0, 9.0), Vec2::new(26.0, 19.0), Vec2::new(42.0, 23.0), Vec2::new(58.0, 17.0)],
            8,
        );
        b.part(stroke(&path, |t| 2.2 - 1.4 * t, 1.0, 1.0, |_| 1.0), &leg, Vec3::ZERO, bone);
    }

    let mut rng = Rng(0xC0C4_0AC4);
    commands.entity(roach).insert((
        Boss::new(&SPEC, home, mats),
        Roach {
            vel: Vec2::ZERO,
            state: State::Stalk,
            flying: false,
            cooldown: rng.range(1.0, 1.6),
            facing: -1.0,
            dash_dir: Vec2::NEG_X,
            stride: 0.0,
            wings: 0.0,
            seed: rng.range(0.0, 100.0),
            dismember: false,
            arena: *arena,
            rng,
        },
    ));
}

struct Builder<'a, 'w, 's> {
    commands: &'a mut Commands<'w, 's>,
    meshes: &'a mut Assets<Mesh>,
    owner: Entity,
}

impl Builder<'_, '_, '_> {
    fn part(&mut self, mesh: Mesh, mat: &Handle<ColorMaterial>, pos: Vec3, parent: Entity) -> Entity {
        let mesh = self.meshes.add(mesh);
        self.commands
            .spawn((Mesh2d(mesh), MeshMaterial2d(mat.clone()), Transform::from_translation(pos), ChildOf(parent)))
            .id()
    }

    fn bone(&mut self, pos: Vec3, parent: Entity, kind: BoneKind) -> Entity {
        self.commands
            .spawn((
                Bone { owner: self.owner, kind },
                Transform::from_translation(pos),
                Visibility::default(),
                ChildOf(parent),
            ))
            .id()
    }
}

/// React to the slash (health and blood are handled in `boss.rs`).
fn on_hit(mut hits: MessageReader<BossHit>, mut roaches: Query<&mut Roach>) {
    for hit in hits.read() {
        let Ok(mut r) = roaches.get_mut(hit.boss) else { continue };
        if hit.killed {
            // Flipped up into the air.
            r.vel = Vec2::new(hit.dir.x * 300.0, 420.0);
            r.state = State::Dead { t: 0.0, landed: false };
            r.dismember = true;
        } else if !matches!(r.state, State::Dash(_) | State::AirDash(_)) {
            // Dashes power through hits.
            r.vel = if r.flying { hit.dir * 320.0 } else { Vec2::new(hit.dir.x * 320.0, 0.0) };
            r.state = State::Hurt(0.0);
        }
    }
}

fn think(
    time: Res<Time>,
    mut shake: ResMut<Shake>,
    mut stop: ResMut<HitStop>,
    mut bleed: MessageWriter<Bleed>,
    player: Single<(&mut Player, &Transform), Without<Roach>>,
    mut roaches: Query<(&mut Roach, &Boss, &mut Transform)>,
) {
    let dt = time.delta_secs().min(1.0 / 30.0);
    let t = time.elapsed_secs();
    let (mut p, ptf) = player.into_inner();
    let ppos = ptf.translation.truncate();

    for (mut r, boss, mut tf) in &mut roaches {
        let pos = tf.translation.truncate();
        let to_player = ppos - pos;
        let alive = p.dead.is_none();
        let aggro = alive && to_player.length() < AGGRO;
        let side = if to_player.x >= 0.0 { 1.0 } else { -1.0 };
        let floor = r.arena.floor;
        let touching = alive && to_player.length() < RADIUS * 0.8 + 20.0;

        r.state = match r.state {
            State::Dead { t: dt0, landed } => {
                let td = dt0 + dt;
                // Death throes: a few more bursts as it flails.
                for at in [0.3, 0.6, 1.0] {
                    if dt0 < at && td >= at {
                        bleed.write(Bleed {
                            pos,
                            dir: Vec2::Y,
                            spread: PI,
                            speed: 550.0,
                            amount: 60,
                            fluid: Fluid::Yellow,
                            sticky: true,
                        });
                    }
                }
                let mut landed = landed;
                if !landed {
                    r.vel.y -= 1400.0 * dt;
                    // Belly-up, its back is on the floor: lower than standing.
                    let rest = floor + 12.0 * SCALE;
                    let over_floor = (r.arena.left..r.arena.right).contains(&pos.x);
                    if over_floor && pos.y + r.vel.y * dt <= rest && r.vel.y < 0.0 {
                        tf.translation.y = rest;
                        r.vel = Vec2::new(r.vel.x * 0.4, 0.0);
                        landed = true;
                        bleed.write(Bleed {
                            pos,
                            dir: Vec2::Y,
                            spread: 1.3,
                            speed: 240.0,
                            amount: 40,
                            fluid: Fluid::Yellow,
                            sticky: true,
                        });
                    }
                } else {
                    r.vel.x *= 1.0 - 5.0 * dt;
                }
                if td < 3.5 && r.rng.chance(dt * 12.0) {
                    bleed.write(Bleed {
                        pos,
                        dir: Vec2::Y,
                        spread: 1.0,
                        speed: 140.0,
                        amount: 2,
                        fluid: Fluid::Yellow,
                        sticky: true,
                    });
                }
                State::Dead { t: td, landed }
            }
            State::Hurt(s) => {
                r.vel *= 1.0 - 5.0 * dt;
                match () {
                    _ if s < 0.3 => State::Hurt(s + dt),
                    _ if r.flying => State::Fly,
                    _ => State::Stalk,
                }
            }

            // --- Phase 1: the ground ---
            State::Stalk if boss.enraged() => {
                // Second phase: wing covers snap open, it rises.
                shake.add(0.3);
                r.flying = true;
                State::Takeoff(0.0)
            }
            State::Stalk => {
                let dx = to_player.x.abs();
                let want = if !aggro {
                    // Wander back home.
                    (boss.home.x - pos.x).clamp(-1.0, 1.0) * 0.5
                } else if dx > KEEP_AWAY + 50.0 {
                    side
                } else if dx < KEEP_AWAY - 70.0 {
                    -side * 0.8 // skitter back
                } else {
                    (t * 2.7 + r.seed).sin() * 0.4 // restless twitching
                };
                r.vel.x += (want * WALK_SPEED - r.vel.x) * (1.0 - (-8.0 * dt).exp());
                r.facing = if aggro { side } else if r.vel.x.abs() > 10.0 { r.vel.x.signum() } else { r.facing };
                r.cooldown -= dt;
                if aggro && r.cooldown <= 0.0 { State::Windup(0.0) } else { State::Stalk }
            }
            State::Windup(s) => {
                // Rears up and scrabbles in place, creeping back a little.
                r.facing = side;
                r.vel.x = -r.facing * 30.0;
                if s >= WINDUP {
                    r.dash_dir = Vec2::new(r.facing, 0.0);
                    shake.add(0.15);
                    State::Dash(0.0)
                } else {
                    State::Windup(s + dt)
                }
            }
            State::Dash(s) => {
                r.vel.x = r.dash_dir.x * DASH_SPEED;
                if touching {
                    boss::hurt_player(&mut p, ppos, pos, CONTACT_DAMAGE, &mut bleed, (&mut shake, &mut stop));
                }
                let at_edge = (r.dash_dir.x > 0.0 && pos.x >= r.arena.right - 50.0)
                    || (r.dash_dir.x < 0.0 && pos.x <= r.arena.left + 50.0);
                if at_edge {
                    // Skids to a stop at the very edge.
                    shake.add(0.25);
                    State::Recover(0.0)
                } else if s > DASH_TIME {
                    State::Recover(0.0)
                } else {
                    State::Dash(s + dt)
                }
            }
            State::Recover(s) => {
                r.vel.x *= 1.0 - 6.0 * dt;
                let stun = if r.flying { CRASH_STUN } else { RECOVER };
                if s < stun {
                    State::Recover(s + dt)
                } else if r.flying {
                    State::Takeoff(TAKEOFF * 0.4)
                } else {
                    r.cooldown = r.rng.range(1.0, 1.6);
                    State::Stalk
                }
            }

            // --- Phase 2: the air ---
            State::Takeoff(s) => {
                r.vel.x *= 1.0 - 4.0 * dt;
                let target = floor + FOOT + FLY_HEIGHT;
                r.vel.y = (target - pos.y) * 2.5 * (s / TAKEOFF).min(1.0);
                if s >= TAKEOFF {
                    r.cooldown = 0.8;
                    State::Fly
                } else {
                    State::Takeoff(s + dt)
                }
            }
            State::Fly => {
                // Clumsy, jittery flight that keeps its distance above the chameleon.
                let jitter = Vec2::new((t * 5.3 + r.seed).sin() * 40.0, (t * 7.1 + r.seed).sin() * 25.0);
                let target = if aggro {
                    ppos + Vec2::new(-side * 230.0, 150.0)
                } else {
                    Vec2::new(boss.home.x, floor + FOOT + FLY_HEIGHT)
                } + jitter;
                let target = Vec2::new(target.x, target.y.max(floor + FOOT + 70.0));
                let desired = ((target - pos) * 2.5).clamp_length_max(FLY_SPEED);
                r.vel = r.vel.lerp(desired, 1.0 - (-3.5 * dt).exp());
                r.facing = if aggro { side } else { r.facing };
                r.cooldown -= dt;
                if aggro && r.cooldown <= 0.0 { State::AirWindup(0.0) } else { State::Fly }
            }
            State::AirWindup(s) => {
                r.vel *= 1.0 - 6.0 * dt;
                r.facing = side;
                if s >= AIR_WINDUP {
                    r.dash_dir = to_player.normalize_or(Vec2::new(side, -0.5));
                    shake.add(0.12);
                    State::AirDash(0.0)
                } else {
                    State::AirWindup(s + dt)
                }
            }
            State::AirDash(s) => {
                r.vel = r.dash_dir * AIR_DASH_SPEED;
                if touching {
                    boss::hurt_player(&mut p, ppos, pos, CONTACT_DAMAGE, &mut bleed, (&mut shake, &mut stop));
                    // Bounces off and climbs back up.
                    r.vel = Vec2::new(-r.dash_dir.x * 200.0, 250.0);
                    r.cooldown = 1.2;
                    State::Fly
                } else if pos.y - FOOT <= floor + 2.0 && r.dash_dir.y < 0.0 {
                    // Missed: slams into the floor.
                    shake.add(0.4);
                    r.vel = Vec2::new(r.dash_dir.x * 150.0, 0.0);
                    State::Recover(0.0)
                } else if s > AIR_DASH_TIME {
                    r.cooldown = r.rng.range(1.0, 1.5);
                    State::Fly
                } else {
                    State::AirDash(s + dt)
                }
            }
        };

        // Move, staying over the arena.
        let dead = matches!(r.state, State::Dead { .. });
        let mut next = tf.translation.truncate() + r.vel * dt;
        if !dead {
            next.x = next.x.clamp(r.arena.left + 40.0, r.arena.right - 40.0);
            if r.grounded() {
                next.y = floor + FOOT;
                r.vel.y = 0.0;
            } else {
                next.y = next.y.max(floor + FOOT);
            }
        }
        tf.translation.x = next.x;
        tf.translation.y = next.y;

        // Legs cycle with speed; wing covers open in the air.
        let scrabble = if matches!(r.state, State::Windup(_)) { 28.0 } else { 0.0 };
        r.stride += dt * (r.vel.x.abs() * 0.1 + scrabble);
        let wings = if r.grounded() || dead { 0.0 } else { 1.0 };
        r.wings += (wings - r.wings) * (1.0 - (-10.0 * dt).exp());
    }
}

/// The chameleon came back: start the fight over, on the ground (`boss.rs` heals it).
fn reset_on_respawn(mut respawned: MessageReader<Respawned>, mut roaches: Query<&mut Roach>) {
    if respawned.read().count() == 0 {
        return;
    }
    for mut r in &mut roaches {
        if matches!(r.state, State::Dead { .. }) {
            continue;
        }
        r.state = State::Stalk;
        r.flying = false;
        r.vel = Vec2::ZERO;
        r.cooldown = 1.5;
    }
}

/// Tear the antennae and wings off a freshly killed roach.
fn dismember(mut commands: Commands, mut roaches: Query<&mut Roach>, bones: Query<(Entity, &Bone, &GlobalTransform)>) {
    let mut done = Vec::new();
    for (e, bone, gt) in &bones {
        let Ok(r) = roaches.get(bone.owner) else { continue };
        if !r.dismember {
            continue;
        }
        done.push(bone.owner);
        let f = r.facing;
        let (vel, spin) = match bone.kind {
            BoneKind::Antenna { offset } => (Vec2::new(f * (180.0 + offset * 120.0), 520.0), 11.0),
            BoneKind::Elytron { near } => (Vec2::new(-f * if near { 240.0 } else { 160.0 }, 600.0), -8.0),
            BoneKind::HindWing { near } => (Vec2::new(-f * if near { 300.0 } else { 120.0 }, 680.0), 6.0),
            _ => continue,
        };
        commands.entity(e).remove::<Bone>().insert(Visibility::Inherited);
        boss::tear_off(&mut commands, e, gt, vel, spin, None);
    }
    for owner in done {
        if let Ok(mut r) = roaches.get_mut(owner) {
            r.dismember = false;
        }
    }
}

fn animate(time: Res<Time>, roaches: Query<&Roach>, mut bones: Query<(&Bone, &mut Transform, &mut Visibility)>) {
    let t = time.elapsed_secs();
    let dt = time.delta_secs();
    for (bone, mut tf, mut vis) in &mut bones {
        let Ok(r) = roaches.get(bone.owner) else { continue };
        let tt = t + r.seed;
        let (dead_t, landed) = match r.state {
            State::Dead { t, landed } => (Some(t), landed),
            _ => (None, false),
        };
        let windup = match r.state {
            State::Windup(s) => (s / WINDUP).min(1.0),
            State::AirWindup(s) => (s / AIR_WINDUP).min(1.0),
            _ => 0.0,
        };
        let shiver = windup * (tt * 60.0).sin() * 0.05;
        let airborne = !r.grounded() && dead_t.is_none();
        match bone.kind {
            BoneKind::Rig => {
                tf.scale.x += (r.facing * SCALE - tf.scale.x) * (1.0 - (-18.0 * dt).exp());
                let angle = match r.state {
                    // Belly-up once dead (spins over while flipping).
                    State::Dead { t, .. } => {
                        let k = if landed { 1.0 } else { (t * 2.5).min(1.0) };
                        PI * k
                    }
                    State::Windup(_) => 0.14 * windup + shiver, // rears up
                    State::Dash(_) => -0.05,
                    State::Recover(s) if r.flying => -0.1 + (s * 40.0).sin() * 0.02 * (1.0 - s),
                    // Nose along the dash.
                    State::AirDash(_) => (r.dash_dir * r.facing).to_angle(),
                    _ if airborne => 0.25 + (r.vel.y * 0.0006).clamp(-0.3, 0.3) + shiver,
                    _ => (tt * 9.0).sin() * 0.01,
                };
                tf.rotation = Quat::from_rotation_z(angle);
            }
            BoneKind::Head => {
                let bob = if dead_t.is_some() { -0.3 } else { -0.08 * windup + (tt * 6.0).sin() * 0.03 };
                tf.rotation = Quat::from_rotation_z(bob);
            }
            BoneKind::Antenna { offset } => {
                let angle = match r.state {
                    _ if dead_t.is_some() => -0.5 + (tt * 12.0 + offset).sin() * 0.05,
                    // Swept back by the wind of a dash.
                    State::Dash(_) | State::AirDash(_) => 0.55 + (tt * 30.0 + offset).sin() * 0.05,
                    _ => (tt * 2.3 + offset).sin() * 0.14 + windup * (tt * 25.0 + offset).sin() * 0.2,
                };
                tf.rotation = Quat::from_rotation_z(angle);
            }
            BoneKind::Leg { pair, near } => {
                let base = [0.15, 0.0, -0.15][pair];
                // Tripod gait: front-near, middle-far and hind-near move together.
                let group = if (pair == 1) != near { 0.0 } else { PI };
                let angle = if let Some(dt0) = dead_t {
                    // Twitching, slower and slower.
                    let fade = (1.0 - dt0 / 6.0).max(0.15);
                    base + (tt * 30.0 + pair as f32 * 1.7 + group).sin() * 0.5 * fade
                } else if airborne {
                    // Tucked back while flying.
                    base * 0.5 + 0.7 + (tt * 8.0 + pair as f32).sin() * 0.08
                } else {
                    let amp = ((r.vel.x.abs() / 200.0).min(1.0) * 0.45).max(windup * 0.3);
                    base + (r.stride + group).sin() * amp
                };
                tf.rotation = Quat::from_rotation_z(angle);
            }
            BoneKind::Elytron { near } => {
                let open = if near { -1.0 } else { -0.75 };
                tf.rotation = Quat::from_rotation_z(open * r.wings + (tt * 40.0).sin() * 0.04 * r.wings);
            }
            BoneKind::HindWing { near } => {
                *vis = if r.wings > 0.3 { Visibility::Inherited } else { Visibility::Hidden };
                let flap = (tt * 85.0 + if near { 0.0 } else { 1.4 }).sin();
                tf.rotation = Quat::from_rotation_z(-0.7 * r.wings + flap * 0.5);
                tf.scale.y = 0.5 + 0.5 * flap.abs();
            }
        }
    }
}
