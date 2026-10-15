//! The Venom Mother: a giant wasp boss that spits venom and dives to sting.
//! It bleeds green, and the chameleon wears that blood. Beating it gives
//! the chameleon its color.
//!
//! Behavior loop: hover at a distance → aim (rears its head) → spit venom,
//! twice, then dive at the chameleon. Below half health it gets enraged:
//! faster, spits fans of three, dives after every spit. On death its wings
//! and abdomen tear off; the rest falls, leaks, and stays where it lands.
//! Health, getting hit and the death spectacle live in `boss.rs`.

use std::f32::consts::PI;

use bevy::{prelude::*, sprite_render::AlphaMode2d};

use crate::{
    boss::{self, Arena, Boss, BossHit, BossSpec, BossStuff},
    camera::{self, HitStop, Shake},
    chameleon,
    gore::{Bleed, Fluid},
    level::Platform,
    player::{self, Player, Respawned},
    shapes::*,
    terrain::{Terrain, PX},
};

pub(crate) static SPEC: BossSpec = BossSpec {
    name: "VENOM MOTHER",
    hp: 14,
    radius: RADIUS,
    blood: Fluid::Green,
    color: Color::srgb(0.36, 0.78, 0.18),
    aggro: AGGRO,
    // 14 hp, venom fans and dives: a full meal.
    food: 5.0,
};
/// Size of the boss relative to its drawing.
const SCALE: f32 = 1.7;
/// Hurtbox radius around the thorax.
const RADIUS: f32 = 22.0 * SCALE;
const AGGRO: f32 = 650.0;
/// Where it hovers relative to the chameleon (x is mirrored to its side).
const KEEP_AWAY: Vec2 = Vec2::new(240.0, 140.0);
const SPEED: f32 = 280.0;
const DIVE_SPEED: f32 = 440.0;
const AIM_TIME: f32 = 0.6;
const SPITS_BEFORE_DIVE: u32 = 2;
const VENOM_SPEED: f32 = 480.0;
const VENOM_GRAVITY: f32 = 380.0;
const VENOM_DAMAGE: i32 = 1;
const STING_DAMAGE: i32 = 2;
/// How close to walls its body gets.
const BODY_CLEARANCE: f32 = 22.0;
/// Navigation grid cells (in sketch pixels) and the clearance needed to fly through one.
const NAV_CELL: i32 = 4;
const NAV_CLEARANCE: f32 = 30.0;
/// How far (in nav cells) a plan searches from the wasp.
const NAV_RANGE: u16 = 160;
/// How often it rethinks where to be (sooner when its reason changes).
const REPLAN: f32 = 0.4;
/// A new spot must be this much better (in score) to give up the current one.
const HYSTERESIS: f32 = 60.0;
/// Shooting distance from the chameleon: (min, ideal, max).
const PERCH_RANGE: (f32, f32, f32) = (170.0, KEEP_AWAY.x + 40.0, 430.0);
/// Dive starting distance from the chameleon: (min, ideal, max).
const DIVE_RANGE: (f32, f32, f32) = (150.0, 260.0, 380.0);
/// Givet up on finding a dive lane after this long and spits again instead.
const DIVE_PATIENCE: f32 = 3.0;

const YELLOW: Color = Color::srgb(0.86, 0.68, 0.14);
const BLACK: Color = Color::srgb(0.08, 0.07, 0.06);
const EYE: Color = Color::srgb(0.14, 0.10, 0.10);
const WING: Color = Color::srgba(0.90, 0.92, 0.95, 0.32);
/// A darker green than its blood. Drawn on the color layer.
const VENOM: Color = Color::srgb(0.20, 0.56, 0.08);

pub struct WaspPlugin;

impl Plugin for WaspPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, load_venom).add_systems(
            Update,
            (reset_on_respawn, on_hit, think, move_venom, dismember, animate)
                .chain()
                .after(player::move_player)
                .after(chameleon::update_anim_state),
        );
    }
}

#[derive(Component)]
struct Wasp {
    vel: Vec2,
    state: State,
    cooldown: f32,
    spits: u32,
    facing: f32,
    /// Time offset for its wobble and wingbeats.
    seed: f32,
    /// Just died: tear off wings and abdomen this frame.
    dismember: bool,
    rng: Rng,
    /// Where it's flying to, why, and the planned way there (waypoints).
    spot: Option<Vec2>,
    want: Want,
    path: Vec<Vec2>,
    replan: f32,
    /// How long it has been looking for a dive lane.
    dive_wait: f32,
}

/// What a spot is for.
#[derive(Clone, Copy, PartialEq)]
enum Want {
    /// A spot with a clear shot at the chameleon, at spitting range, above it.
    Perch,
    /// A spot with a straight, roomy lane down to the chameleon.
    Dive,
    /// Back home (asleep / the chameleon is dead).
    Home,
}

#[derive(Clone, Copy, PartialEq)]
enum State {
    Hover,
    Aim(f32),
    Dive(f32),
    Hurt(f32),
    Dead {
        t: f32,
        angle: f32,
        spin: f32,
        landed: bool,
    },
}

#[derive(Component)]
struct Bone {
    owner: Entity,
    kind: BoneKind,
}

#[derive(Clone, Copy)]
enum BoneKind {
    Rig,
    Abdomen,
    Head,
    Wing { front: bool },
    Mandible { side: f32 },
    Leg { offset: f32 },
}

#[derive(Component)]
struct Venom {
    vel: Vec2,
    life: f32,
}

#[derive(Resource)]
struct VenomAssets {
    mesh: Handle<Mesh>,
    mat: Handle<ColorMaterial>,
}

fn load_venom(mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>, mut materials: ResMut<Assets<ColorMaterial>>) {
    let glob = fill(&ellipse_pts(8.0, 6.0, 20), |p| 1.0 - 0.35 * (p - Vec2::new(2.5, 2.0)).length() / 9.0);
    commands.insert_resource(VenomAssets { mesh: meshes.add(glob), mat: materials.add(VENOM) });
}

/// Spawns the wasp hovering over the arena.
pub fn spawn(commands: &mut Commands, meshes: &mut Assets<Mesh>, materials: &mut Assets<ColorMaterial>, arena: &Arena) -> Entity {
    let home = arena.home;

    let yellow = materials.add(YELLOW);
    let black = materials.add(BLACK);
    let eye = materials.add(EYE);
    let wing = materials.add(ColorMaterial {
        color: WING,
        alpha_mode: AlphaMode2d::Blend,
        ..default()
    });
    let shine = materials.add(Color::WHITE);
    let mats = vec![
        (yellow.clone(), YELLOW),
        (black.clone(), BLACK),
        (eye.clone(), EYE),
    ];

    let wasp = commands
        .spawn((
            Transform::from_translation(home.extend(12.0)),
            Visibility::default(),
        ))
        .id();
    let mut b = WaspBuilder {
        commands,
        meshes,
        owner: wasp,
    };

    let rig = b.bone(Vec3::ZERO, wasp, BoneKind::Rig);
    b.commands
        .entity(rig)
        .insert(Transform::from_scale(Vec3::new(-SCALE, SCALE, 1.0)));

    // Wings: the far one behind everything, the near one on top.
    let wing_mesh = |m: &mut Assets<Mesh>| {
        let pts: Vec<Vec2> = ellipse_pts(17.0, 5.5, 24)
            .into_iter()
            .map(|p| Vec2::from_angle(0.45).rotate(p + Vec2::new(-15.0, 0.0)))
            .collect();
        m.add(fill(&pts, |p| 0.8 + 0.2 * (p.y / 10.0).clamp(-1.0, 1.0)))
    };
    let back_wing = b.bone(
        Vec3::new(-1.0, 7.0, -0.4),
        rig,
        BoneKind::Wing { front: false },
    );
    let mesh = wing_mesh(b.meshes);
    b.part_handle(mesh, &wing, Vec3::ZERO, back_wing);

    // Dangling legs, hooked at the end.
    for (k, x) in [-5.0, 0.0, 5.0].into_iter().enumerate() {
        let leg = b.bone(
            Vec3::new(x, -5.0, -0.2),
            rig,
            BoneKind::Leg {
                offset: k as f32 * 1.7,
            },
        );
        let path = smooth_open(
            &[
                Vec2::ZERO,
                Vec2::new(-3.0, -8.0),
                Vec2::new(-1.0, -15.0),
                Vec2::new(2.0, -18.0),
            ],
            5,
        );
        b.part(
            stroke(&path, |t| 2.0 - 1.0 * t, 1.0, 1.0, |_| 1.0),
            &black,
            Vec3::ZERO,
            leg,
        );
    }

    // Abdomen: striped, ending in a long stinger.
    let abdomen = b.bone(Vec3::new(-8.0, -2.0, 0.0), rig, BoneKind::Abdomen);
    let outline = smooth_closed(
        &[
            Vec2::new(1.0, 3.0),
            Vec2::new(-6.0, 7.0),
            Vec2::new(-16.0, 7.0),
            Vec2::new(-25.0, 3.0),
            Vec2::new(-31.0, -3.0),
            Vec2::new(-24.0, -7.0),
            Vec2::new(-13.0, -8.0),
            Vec2::new(-4.0, -6.0),
            Vec2::new(1.0, -3.0),
        ],
        6,
    );
    b.part(
        fill(&outline, |p| {
            0.65 + 0.35 * ((p.y + 8.0) / 15.0).clamp(0.0, 1.0)
        }),
        &yellow,
        Vec3::ZERO,
        abdomen,
    );
    let span = |x: f32| {
        let near = outline.iter().filter(|p| (p.x - x).abs() < 2.0);
        near.fold((0.0f32, 0.0f32), |(lo, hi), p| (lo.min(p.y), hi.max(p.y)))
    };
    let bands: Vec<[Vec2; 3]> = [-7.0, -14.5, -22.0]
        .into_iter()
        .flat_map(|x| {
            let (lo, hi) = span(x);
            let (a, b, c, d) = (
                Vec2::new(x + 1.5, hi + 0.5),
                Vec2::new(x - 2.5, hi + 0.5),
                Vec2::new(x - 3.5, lo - 0.5),
                Vec2::new(x + 0.5, lo - 0.5),
            );
            [[a, b, c], [a, c, d]]
        })
        .collect();
    b.part(
        triangles(&bands),
        &black,
        Vec3::new(0.0, 0.0, 0.05),
        abdomen,
    );
    b.part(
        triangles(&[[
            Vec2::new(-29.0, -1.5),
            Vec2::new(-28.0, -5.0),
            Vec2::new(-40.0, -6.5),
        ]]),
        &black,
        Vec3::new(0.0, 0.0, -0.05),
        abdomen,
    );
    b.part(
        fill(&ellipse_pts(3.0, 2.4, 12), |_| 1.0),
        &black,
        Vec3::new(-7.0, -1.0, 0.08),
        rig,
    );

    // Thorax with a dark saddle.
    b.part(
        fill(&ellipse_pts(10.0, 8.0, 28), |p| {
            0.7 + 0.3 * ((p.y + 8.0) / 16.0)
        }),
        &yellow,
        Vec3::new(0.0, 0.0, 0.1),
        rig,
    );
    let saddle = smooth_closed(
        &[
            Vec2::new(-7.0, 4.0),
            Vec2::new(0.0, 8.0),
            Vec2::new(7.0, 5.0),
            Vec2::new(0.0, 2.0),
        ],
        5,
    );
    b.part(
        fill(&saddle, |_| 1.0),
        &black,
        Vec3::new(0.0, 0.0, 0.12),
        rig,
    );

    // Head: huge compound eye, angry brow, mandibles, antennae.
    let head = b.bone(Vec3::new(10.0, 3.0, 0.2), rig, BoneKind::Head);
    b.part(
        fill(&ellipse_pts(7.0, 7.5, 24), |p| {
            0.7 + 0.3 * ((p.y + 7.5) / 15.0)
        }),
        &yellow,
        Vec3::ZERO,
        head,
    );
    let eye_pts: Vec<Vec2> = ellipse_pts(3.6, 5.8, 20)
        .into_iter()
        .map(|p| Vec2::from_angle(-0.3).rotate(p))
        .collect();
    b.part(
        fill(&eye_pts, |p| 0.8 + 0.2 * p.y / 6.0),
        &eye,
        Vec3::new(1.0, 1.5, 0.05),
        head,
    );
    b.part(
        fill(&ellipse_pts(0.9, 1.3, 8), |_| 1.0),
        &shine,
        Vec3::new(2.2, 4.0, 0.06),
        head,
    );
    b.part(
        triangles(&[[
            Vec2::new(-3.5, 8.5),
            Vec2::new(6.5, 4.0),
            Vec2::new(-2.0, 5.5),
        ]]),
        &black,
        Vec3::new(0.0, 0.0, 0.07),
        head,
    );
    for side in [1.0, -1.0] {
        let m = b.bone(
            Vec3::new(5.0, -4.0, 0.04 * side),
            head,
            BoneKind::Mandible { side },
        );
        let pts = smooth_open(
            &[
                Vec2::ZERO,
                Vec2::new(4.0, -1.5 * side),
                Vec2::new(6.5, 0.5 * side),
            ],
            5,
        );
        b.part(
            stroke(&pts, |t| 2.6 * (1.0 - t) + 0.4, 1.0, 1.0, |_| 1.0),
            &black,
            Vec3::ZERO,
            m,
        );
    }
    for (dx, z) in [(0.0, -0.05), (2.5, 0.08)] {
        let ant = smooth_open(
            &[
                Vec2::new(dx, 6.0),
                Vec2::new(dx + 4.0, 14.0),
                Vec2::new(dx + 11.0, 17.0),
                Vec2::new(dx + 15.0, 14.0),
            ],
            5,
        );
        b.part(
            stroke(&ant, |t| 1.4 - 0.6 * t, 1.0, 1.0, |_| 1.0),
            &black,
            Vec3::new(0.0, 0.0, z),
            head,
        );
    }

    let front_wing = b.bone(
        Vec3::new(-3.0, 8.0, 0.5),
        rig,
        BoneKind::Wing { front: true },
    );
    let mesh = wing_mesh(b.meshes);
    b.part_handle(mesh, &wing, Vec3::ZERO, front_wing);

    let mut rng = Rng(0x5EED_1234);
    commands.entity(wasp).insert((
        Boss::new(&SPEC, home, mats),
        Wasp {
            vel: Vec2::ZERO,
            state: State::Hover,
            cooldown: rng.range(0.5, 1.5),
            spits: 0,
            facing: -1.0,
            seed: rng.range(0.0, 100.0),
            dismember: false,
            rng,
            spot: None,
            want: Want::Home,
            path: Vec::new(),
            replan: 0.0,
            dive_wait: 0.0,
        },
    ));
    wasp
}

struct WaspBuilder<'a, 'w, 's> {
    commands: &'a mut Commands<'w, 's>,
    meshes: &'a mut Assets<Mesh>,
    owner: Entity,
}

impl WaspBuilder<'_, '_, '_> {
    fn part(
        &mut self,
        mesh: Mesh,
        mat: &Handle<ColorMaterial>,
        pos: Vec3,
        parent: Entity,
    ) -> Entity {
        let mesh = self.meshes.add(mesh);
        self.part_handle(mesh, mat, pos, parent)
    }

    fn part_handle(
        &mut self,
        mesh: Handle<Mesh>,
        mat: &Handle<ColorMaterial>,
        pos: Vec3,
        parent: Entity,
    ) -> Entity {
        self.commands
            .spawn((
                Mesh2d(mesh),
                MeshMaterial2d(mat.clone()),
                Transform::from_translation(pos),
                ChildOf(parent),
            ))
            .id()
    }

    fn bone(&mut self, pos: Vec3, parent: Entity, kind: BoneKind) -> Entity {
        self.commands
            .spawn((
                Bone {
                    owner: self.owner,
                    kind,
                },
                Transform::from_translation(pos),
                Visibility::default(),
                ChildOf(parent),
            ))
            .id()
    }
}

fn think(
    mut commands: Commands,
    time: Res<Time>,
    mut shake: ResMut<Shake>,
    mut stop: ResMut<HitStop>,
    venom: Res<VenomAssets>,
    mut bleed: MessageWriter<Bleed>,
    player: Single<(&mut Player, &Transform), Without<Wasp>>,
    mut wasps: Query<(Entity, &mut Wasp, &Boss, &mut Transform, Option<&mut boss::Sense>)>,
    platforms: Query<(&Platform, &Transform), (Without<Wasp>, Without<Player>)>,
    terrain: Res<Terrain>,
    mut nav: Local<Nav>,
) {
    let dt = time.delta_secs().min(1.0 / 30.0);
    let t = time.elapsed_secs();
    let (mut p, ptf) = player.into_inner();
    let ppos = ptf.translation.truncate();

    for (e, mut w, boss, mut tf, mut sense) in &mut wasps {
        let pos = tf.translation.truncate();
        let to_player = ppos - pos;
        let alive = p.dead.is_none();
        // Awake (the chameleon entered its lair): it hunts until the lair resets it.
        let aggro = alive && boss.awake;
        let enraged = boss.enraged();
        let side = if to_player.x >= 0.0 { 1.0 } else { -1.0 };
        let wobble = Vec2::new(
            (t * 1.3 + w.seed).sin() * 25.0,
            (t * 2.1 + w.seed).sin() * 14.0,
        );
        let spits_per_dive = if enraged { 1 } else { SPITS_BEFORE_DIVE };

        // Plan ahead where to be (a perch to spit from, the top of a dive
        // lane, or home) and the way there; never mid-dive.
        if !matches!(w.state, State::Dead { .. } | State::Dive(_)) {
            let want = match () {
                _ if !aggro => Want::Home,
                _ if w.spits >= spits_per_dive => Want::Dive,
                _ => Want::Perch,
            };
            if want != w.want {
                w.want = want;
                w.replan = 0.0;
            }
            w.replan -= dt;
            if w.replan <= 0.0 {
                w.replan = REPLAN;
                let keep = w.spot;
                let plan = if want == Want::Home && clear_line(&terrain, pos, boss.home, BODY_CLEARANCE) {
                    Some((boss.home, Vec::new()))
                } else {
                    // Can't dive from anywhere near: at least find a perch; no
                    // perch either (a tunnel, a pit): get as close as it can.
                    let wants: &[Want] = match want {
                        Want::Dive => &[Want::Dive, Want::Perch],
                        Want::Perch => &[Want::Perch],
                        Want::Home => &[Want::Home],
                    };
                    let mut plan = None;
                    for &want in wants {
                        plan = nav.plan(&terrain, pos, keep, |q, steps| spot_score(&terrain, want, q, steps, ppos, boss.home));
                        if plan.is_some() {
                            break;
                        }
                    }
                    plan.or_else(|| {
                        let near = ppos + Vec2::Y * KEEP_AWAY.y;
                        nav.plan(&terrain, pos, None, |q, steps| Some((q - near).length() + steps as f32 * NAV_STEP * 0.3))
                    })
                };
                match plan {
                    Some((spot, path)) => {
                        w.spot = Some(spot);
                        w.path = path;
                    }
                    None => {
                        w.spot = None;
                        w.path.clear();
                    }
                }
            }
        }

        w.state = match w.state {
            State::Dead {
                t: dt0,
                angle,
                spin,
                landed,
            } => {
                let td = dt0 + dt;
                // Death throes: aftershock bursts while it falls.
                for at in [0.3, 0.6, 1.0] {
                    if dt0 < at && td >= at {
                        bleed.write(Bleed {
                            pos: tf.translation.truncate(),
                            dir: Vec2::Y,
                            spread: PI,
                            speed: 600.0,
                            amount: 70,
                            fluid: Fluid::Green,
                            sticky: true,
                        });
                        shake.add(0.35);
                    }
                }
                let mut landed = landed;
                if !landed {
                    w.vel.y -= 1400.0 * dt;
                    let next = pos + w.vel * dt;
                    // Land belly-up on the first solid top below.
                    if let Some(top) = boss::floor(&terrain, platforms.iter(), pos, next, 8.0 * SCALE) {
                        tf.translation.y = top + 8.0 * SCALE;
                        tf.translation.x = next.x;
                        w.vel = Vec2::ZERO;
                        landed = true;
                        shake.add(0.1);
                        bleed.write(Bleed {
                            pos: tf.translation.truncate(),
                            dir: Vec2::Y,
                            spread: 1.3,
                            speed: 260.0,
                            amount: 40,
                            fluid: Fluid::Green,
                            sticky: true,
                        });
                    } else {
                        tf.translation.x = next.x;
                        tf.translation.y = next.y;
                    }
                }
                // Leaks for a while after death.
                if td < 3.5 && w.rng.chance(dt * if landed { 10.0 } else { 30.0 }) {
                    bleed.write(Bleed {
                        pos: tf.translation.truncate(),
                        dir: Vec2::new(0.0, if landed { 1.0 } else { -1.0 }),
                        spread: 1.0,
                        speed: 150.0,
                        amount: 2,
                        fluid: Fluid::Green,
                        sticky: true,
                    });
                }
                if tf.translation.y < terrain.bounds.min.y - 600.0 {
                    commands.entity(e).despawn();
                }
                let angle = if landed {
                    angle + (PI - angle) * (1.0 - (-8.0 * dt).exp())
                } else {
                    angle + spin * dt
                };
                State::Dead {
                    t: td,
                    angle,
                    spin,
                    landed,
                }
            }
            State::Hurt(s) => {
                w.vel *= 1.0 - 4.0 * dt;
                if s > 0.35 {
                    // Knocked off its route: think again.
                    w.replan = 0.0;
                    State::Hover
                } else {
                    State::Hurt(s + dt)
                }
            }
            State::Dive(s) => {
                let dir = to_player.normalize_or(Vec2::NEG_Y);
                let max = if enraged {
                    DIVE_SPEED * 1.25
                } else {
                    DIVE_SPEED
                };
                w.vel = (w.vel + dir * 1400.0 * dt).clamp_length_max(max);
                w.facing = side;
                // Look ahead: a wall coming up (the chameleon ducked away)
                // means pulling out, not stinging the wall.
                let ahead = pos + w.vel.normalize_or_zero() * (BODY_CLEARANCE + w.vel.length() * 0.08);
                // (The lane was only checked down to a body length short of the chameleon.)
                let near = (ahead - ppos).length() < RADIUS + 20.0;
                if !alive {
                    State::Hover
                } else if to_player.length() < RADIUS + 15.0 {
                    boss::hurt_player(
                        &mut p,
                        ppos,
                        pos,
                        STING_DAMAGE,
                        &mut bleed,
                        (&mut shake, &mut stop),
                    );
                    // Bounce back up the lane it came down.
                    w.vel = -w.vel.normalize_or(dir) * 300.0;
                    w.cooldown = 1.2;
                    w.replan = 0.0;
                    State::Hover
                } else if t > 1.3 || (!near && terrain.clearance(ahead) < BODY_CLEARANCE * 0.75) {
                    w.vel *= 0.4;
                    w.cooldown = 1.0;
                    w.replan = 0.0;
                    State::Hover
                } else {
                    State::Dive(s + dt)
                }
            }
            State::Hover | State::Aim(_) => {
                let goal = w.spot.unwrap_or(if aggro { pos } else { boss.home });
                // Follow the planned route: drop reached waypoints, cut corners
                // whenever a later one is in plain (roomy) sight.
                while let Some(&next) = w.path.first() {
                    let reached = (next - pos).length() < NAV_STEP * 0.75;
                    if reached || (w.path.len() >= 2 && clear_line(&terrain, pos, w.path[1], BODY_CLEARANCE + 2.0)) {
                        w.path.remove(0);
                    } else {
                        break;
                    }
                }
                let routed = !w.path.is_empty();
                if routed && !clear_line(&terrain, pos, w.path[0], BODY_CLEARANCE * 0.5) {
                    // Pushed off the route (hit, bumped): plan from here.
                    w.replan = w.replan.min(0.0);
                }
                // Only wobble where there's room for it.
                let wobble = if terrain.clearance(goal) >= BODY_CLEARANCE + 32.0 { wobble } else { Vec2::ZERO };
                let target = w.path.first().copied().unwrap_or(goal + wobble);
                let sees = terrain.raycast(pos, ppos).is_none();
                if let Some(t) = sense.as_mut() {
                    t.goal = Some(goal);
                    t.target = Some(target);
                    t.sees_player = sees;
                    t.path.clone_from(&w.path);
                }
                let speed = if enraged { SPEED * 1.3 } else { SPEED };
                // Full speed along the route, easing in at the end of it.
                let mut desired = if routed {
                    (target - pos).normalize_or_zero() * speed
                } else {
                    ((target - pos) * 2.5).clamp_length_max(speed)
                };
                if matches!(w.state, State::Aim(_)) {
                    desired *= 0.2;
                }
                let steer = if routed { 6.0 } else { 4.0 };
                w.vel = w.vel.lerp(desired, 1.0 - (-steer * dt).exp());
                if aggro {
                    w.facing = side;
                } else if w.vel.x.abs() > 20.0 {
                    w.facing = w.vel.x.signum();
                }
                w.cooldown -= dt;

                let aim_time = if enraged { AIM_TIME * 0.7 } else { AIM_TIME };
                match w.state {
                    State::Aim(t) if t >= aim_time && !sees => {
                        // Lost the shot: find a new perch.
                        w.cooldown = 0.3;
                        w.replan = 0.0;
                        State::Hover
                    }
                    State::Aim(s) if s >= aim_time => {
                        let mouth = pos + Vec2::new(w.facing * 18.0, -2.0) * SCALE;
                        // Enraged: a fan of three.
                        let fan: &[f32] = if enraged { &[-0.2, 0.0, 0.2] } else { &[0.0] };
                        for &a in fan {
                            spit(&mut commands, &venom, mouth, ppos, a);
                        }
                        w.spits += 1;
                        w.cooldown = if enraged {
                            w.rng.range(0.8, 1.3)
                        } else {
                            w.rng.range(1.3, 2.1)
                        };
                        State::Hover
                    }
                    State::Aim(s) => State::Aim(s + dt),
                    _ if aggro && w.cooldown <= 0.0 && w.spits >= spits_per_dive => {
                        // Dive only down a lane it can fly all the way.
                        let lane = dive_lane(&terrain, pos, ppos);
                        if lane && to_player.length() < DIVE_RANGE.2 + 60.0 {
                            w.spits = 0;
                            w.dive_wait = 0.0;
                            w.path.clear();
                            // Lined up with the lane, so it flies the lane it checked.
                            w.vel = to_player.normalize_or(Vec2::NEG_Y) * w.vel.length().min(150.0);
                            State::Dive(0.0)
                        } else {
                            w.dive_wait += dt;
                            if w.dive_wait > DIVE_PATIENCE {
                                // No way down to it: spit some more instead.
                                w.dive_wait = 0.0;
                                w.spits = 0;
                            }
                            State::Hover
                        }
                    }
                    // Only start aiming with a clear shot (the perch has one).
                    _ if aggro && w.cooldown <= 0.0 && sees => State::Aim(0.0),
                    _ => State::Hover,
                }
            }
        };

        // (Corpses move themselves above.) Walls stop it, one axis at a time.
        if !matches!(w.state, State::Dead { .. }) {
            let mut pos = tf.translation.truncate();
            let mut bumped = false;
            for axis in [Vec2::X, Vec2::Y] {
                let next = pos + axis * w.vel.dot(axis) * dt;
                let room = terrain.clearance(next);
                if room >= BODY_CLEARANCE || room >= terrain.clearance(pos) {
                    pos = next;
                } else {
                    let along = w.vel.dot(axis);
                    w.vel -= axis * along;
                    bumped = true;
                }
            }
            if bumped && matches!(w.state, State::Dive(_)) {
                // Stung the wall.
                shake.add(0.15);
                w.cooldown = 0.8;
                w.replan = 0.0;
                w.state = State::Hover;
            }
            tf.translation.x = pos.x;
            tf.translation.y = pos.y;
        }
    }
}

fn spit(commands: &mut Commands, venom: &VenomAssets, from: Vec2, at: Vec2, angle: f32) {
    // Aim a little above the target to make up for the venom's drop.
    let flight = (at - from).length() / VENOM_SPEED;
    let dir = Vec2::from_angle(angle).rotate((at - from).normalize_or(Vec2::X));
    let vel = dir * VENOM_SPEED + Vec2::Y * VENOM_GRAVITY * flight * 0.5;
    commands.spawn((
        Venom { vel, life: 3.0 },
        Mesh2d(venom.mesh.clone()),
        MeshMaterial2d(venom.mat.clone()),
        Transform::from_translation(from.extend(15.0)),
        camera::color_layer(),
        BossStuff,
    ));
}

/// React to the slash (health and blood are handled in `boss.rs`).
fn on_hit(mut hits: MessageReader<BossHit>, mut wasps: Query<&mut Wasp>) {
    for hit in hits.read() {
        let Ok(mut w) = wasps.get_mut(hit.boss) else { continue };
        if hit.killed {
            // Launched high, spinning hard.
            w.vel = hit.dir * 420.0 + Vec2::Y * 560.0;
            let spin = -hit.dir.x.signum() * w.rng.range(10.0, 14.0);
            w.state = State::Dead { t: 0.0, angle: 0.0, spin, landed: false };
            w.dismember = true;
        } else {
            w.vel = hit.dir * 450.0;
            w.state = State::Hurt(0.0);
            w.cooldown = w.cooldown.max(0.6);
        }
    }
}

fn move_venom(
    mut commands: Commands,
    time: Res<Time>,
    mut shake: ResMut<Shake>,
    mut stop: ResMut<HitStop>,
    mut bleed: MessageWriter<Bleed>,
    player: Single<(&mut Player, &Transform), Without<Venom>>,
    mut venom: Query<(Entity, &mut Venom, &mut Transform)>,
    platforms: Query<(&Platform, &Transform), (Without<Venom>, Without<Player>)>,
    terrain: Res<Terrain>,
) {
    let dt = time.delta_secs().min(1.0 / 30.0);
    let t = time.elapsed_secs();
    let (mut p, ptf) = player.into_inner();
    let ppos = ptf.translation.truncate();

    for (e, mut v, mut tf) in &mut venom {
        v.vel.y -= VENOM_GRAVITY * dt;
        v.life -= dt;
        let pos = tf.translation.truncate() + v.vel * dt;
        tf.translation = pos.extend(tf.translation.z);
        tf.rotation = Quat::from_rotation_z(v.vel.to_angle());
        tf.scale = Vec3::new(
            1.15 + (t * 30.0).sin() * 0.12,
            0.9 - (t * 30.0).sin() * 0.1,
            1.0,
        );

        let splash = |dir: Vec2, amount| Bleed {
            pos,
            dir,
            spread: 1.2,
            speed: 220.0,
            amount,
            fluid: Fluid::Venom,
            sticky: false,
        };
        let d = (pos - ppos).abs();
        if p.dead.is_none() && d.x < player::HALF.x + 5.0 && d.y < player::HALF.y + 5.0 {
            let from = pos - v.vel.normalize_or_zero() * 50.0;
            boss::hurt_player(
                &mut p,
                ppos,
                from,
                VENOM_DAMAGE,
                &mut bleed,
                (&mut shake, &mut stop),
            );
            bleed.write(splash(-v.vel, 14));
            commands.entity(e).despawn();
            continue;
        }
        let hit_wall = terrain.solid_at(pos)
            || platforms.iter().any(|(pl, ptf)| {
                let d = (pos - ptf.translation.truncate()).abs();
                pl.solid && d.x < pl.half.x && d.y < pl.half.y
            });
        if hit_wall || v.life <= 0.0 {
            bleed.write(splash(Vec2::Y, 12));
            commands.entity(e).despawn();
        } else if (t * 40.0).fract() < 0.3 {
            // A thin trail of drips.
            bleed.write(Bleed {
                dir: -v.vel,
                spread: 0.6,
                speed: 60.0,
                ..splash(Vec2::Y, 1)
            });
        }
    }
}

/// The chameleon came back: start the fight over (`boss.rs` heals it).
fn reset_on_respawn(
    mut commands: Commands,
    mut respawned: MessageReader<Respawned>,
    mut wasps: Query<&mut Wasp>,
    venom: Query<Entity, With<Venom>>,
) {
    if respawned.read().count() == 0 {
        return;
    }
    for mut w in &mut wasps {
        if matches!(w.state, State::Dead { .. }) {
            continue;
        }
        w.state = State::Hover;
        w.vel = Vec2::ZERO;
        w.spits = 0;
        w.cooldown = 1.5;
        w.spot = None;
        w.path.clear();
        w.replan = 0.0;
        w.dive_wait = 0.0;
    }
    for e in &venom {
        commands.entity(e).despawn();
    }
}

/// Tear the wings and abdomen off a freshly killed wasp.
fn dismember(mut commands: Commands, mut wasps: Query<&mut Wasp>, bones: Query<(Entity, &Bone, &GlobalTransform)>) {
    let mut done = Vec::new();
    for (e, bone, gt) in &bones {
        let Ok(w) = wasps.get(bone.owner) else { continue };
        if !w.dismember {
            continue;
        }
        done.push(bone.owner);
        let f = w.facing;
        let (vel, spin, bleeds) = match bone.kind {
            BoneKind::Wing { front: true } => (Vec2::new(f * 260.0, 700.0), 13.0, None),
            BoneKind::Wing { front: false } => (Vec2::new(-f * 230.0, 620.0), -10.0, None),
            BoneKind::Abdomen => (Vec2::new(-f * 380.0, 460.0), f * 8.0, Some(Fluid::Green)),
            _ => continue,
        };
        commands.entity(e).remove::<Bone>();
        boss::tear_off(&mut commands, e, gt, vel, spin, bleeds);
    }
    for owner in done {
        if let Ok(mut w) = wasps.get_mut(owner) {
            w.dismember = false;
        }
    }
}

fn animate(time: Res<Time>, wasps: Query<&Wasp>, mut bones: Query<(&Bone, &mut Transform)>) {
    let t = time.elapsed_secs();
    let dt = time.delta_secs();
    for (bone, mut tf) in &mut bones {
        let Ok(w) = wasps.get(bone.owner) else {
            continue;
        };
        let tt = t + w.seed;
        let dead = matches!(w.state, State::Dead { .. });
        let aim = match w.state {
            State::Aim(s) => (s / AIM_TIME).min(1.0),
            _ => 0.0,
        };
        let jitter = aim * (tt * 70.0).sin() * 0.06;
        match bone.kind {
            BoneKind::Rig => {
                let target = w.facing * SCALE;
                tf.scale.x += (target - tf.scale.x) * (1.0 - (-14.0 * dt).exp());
                tf.rotation = match w.state {
                    State::Dead { angle, .. } => Quat::from_rotation_z(angle),
                    // Nose down when diving.
                    State::Dive(_) => {
                        Quat::from_rotation_z((w.vel.y * 0.0015).clamp(-0.6, 0.6) * w.facing)
                    }
                    _ => Quat::from_rotation_z(
                        (-w.vel.x * w.facing * 0.0008).clamp(-0.3, 0.3) + jitter,
                    ),
                };
            }
            BoneKind::Wing { front } => {
                if dead {
                    tf.rotation = Quat::from_rotation_z(-0.6);
                    tf.scale.y = 1.0;
                } else {
                    let flap = (tt * 95.0 + if front { 0.0 } else { 1.3 }).sin();
                    tf.rotation = Quat::from_rotation_z(0.15 + flap * 0.45);
                    tf.scale.y = 0.55 + 0.45 * flap.abs();
                }
            }
            BoneKind::Abdomen => {
                let pump = (tt * 6.0).sin() * 0.07;
                let curl = match w.state {
                    State::Dive(_) => 0.9, // stinger forward
                    State::Dead { .. } => 0.3,
                    _ => 0.5 * aim,
                };
                tf.rotation = Quat::from_rotation_z(pump + curl + jitter);
            }
            BoneKind::Head => {
                // Rears back while aiming, snaps forward to spit.
                let rear = if aim > 0.0 {
                    0.35 * aim
                } else {
                    (tt * 2.0).sin() * 0.05
                };
                tf.rotation = Quat::from_rotation_z(rear - if dead { 0.4 } else { 0.0 });
            }
            BoneKind::Mandible { side } => {
                let open = 0.15
                    + 0.55 * aim
                    + if matches!(w.state, State::Dive(_)) {
                        0.5
                    } else {
                        0.0
                    };
                let chew = (tt * 18.0).sin().max(0.0) * 0.15;
                tf.rotation = Quat::from_rotation_z(-side * (open + chew));
            }
            BoneKind::Leg { offset } => {
                let sway = if dead {
                    0.9
                } else {
                    (tt * 3.0 + offset).sin() * 0.15 - w.vel.x * w.facing * 0.0006
                };
                tf.rotation = Quat::from_rotation_z(sway);
            }
        }
    }
}

/// World size of a nav cell.
const NAV_STEP: f32 = NAV_CELL as f32 * PX;

/// Every point between `a` and `b` (not counting `a`) has `room` to the nearest wall.
fn clear_line(terrain: &Terrain, a: Vec2, b: Vec2, room: f32) -> bool {
    let n = ((b - a).length() / (PX * 0.5)).ceil().max(1.0) as i32;
    (1..=n).all(|i| terrain.clearance(a.lerp(b, i as f32 / n as f32)) >= room)
}

/// A straight dive from `from` reaches the chameleon at `ppos` without
/// clipping a wall (it stops a body length short: that's the sting).
fn dive_lane(terrain: &Terrain, from: Vec2, ppos: Vec2) -> bool {
    let back = (from - ppos).normalize_or(Vec2::Y);
    clear_line(terrain, from, ppos + back * (RADIUS + 10.0), BODY_CLEARANCE)
}

/// How good `q` (reached in `steps` nav cells) is for `want`; lower is
/// better, `None` = unsuitable.
fn spot_score(terrain: &Terrain, want: Want, q: Vec2, steps: u16, ppos: Vec2, home: Vec2) -> Option<f32> {
    let travel = steps as f32 * NAV_STEP;
    if want == Want::Home {
        return Some((q - home).length() + travel * 0.2);
    }
    let ((lo, ideal, hi), height, min_y) = match want {
        Want::Dive => (DIVE_RANGE, 170.0, 30.0),
        _ => (PERCH_RANGE, KEEP_AWAY.y, -20.0),
    };
    let d = q - ppos;
    let len = d.length();
    if len < lo || len > hi || d.y < min_y {
        return None;
    }
    // Cheap tests first; the line of sight / lane last.
    let room = terrain.clearance(q).min(96.0);
    let score = (len - ideal).abs() + (d.y - height).abs() * 0.7 + travel * 0.5 - room * 0.6;
    let clear = match want {
        Want::Dive => dive_lane(terrain, q, ppos),
        _ => terrain.raycast(q, ppos).is_none(),
    };
    clear.then_some(score)
}

/// Route planning over a coarse grid of the open air: a breadth-first search
/// out from the wasp, so it finds its way around walls and through buildings
/// to wherever it wants to be. Buffers are reused between plans.
#[derive(Default)]
struct Nav {
    size: IVec2,
    /// `seen[i] == mark`: visited by the current plan.
    seen: Vec<u32>,
    mark: u32,
    from: Vec<u32>,
    steps: Vec<u16>,
    order: Vec<u32>,
}

impl Nav {
    fn coarse(terrain: &Terrain, p: Vec2) -> IVec2 {
        terrain.to_cell(p).div_euclid(IVec2::splat(NAV_CELL))
    }

    fn center(terrain: &Terrain, c: IVec2) -> Vec2 {
        terrain.cell_min(c * NAV_CELL + NAV_CELL / 2) + Vec2::splat(PX / 2.0)
    }

    fn open(terrain: &Terrain, c: IVec2) -> bool {
        terrain.clearance(Self::center(terrain, c)) >= NAV_CLEARANCE
    }

    /// The best-scoring reachable spot (sticking with `keep` unless another is
    /// clearly better) and the waypoints to it.
    fn plan(
        &mut self,
        terrain: &Terrain,
        from: Vec2,
        keep: Option<Vec2>,
        score: impl Fn(Vec2, u16) -> Option<f32>,
    ) -> Option<(Vec2, Vec<Vec2>)> {
        let size = (terrain.size() + NAV_CELL - 1) / NAV_CELL;
        let n = (size.x * size.y) as usize;
        if self.size != size || self.seen.len() != n {
            *self = Nav { size, seen: vec![0; n], from: vec![0; n], steps: vec![0; n], ..default() };
        }
        self.mark = self.mark.wrapping_add(1);
        if self.mark == 0 {
            self.seen.fill(0);
            self.mark = 1;
        }
        let inside = |c: IVec2| c.cmpge(IVec2::ZERO).all() && c.cmplt(size).all();
        let index = |c: IVec2| (c.y * size.x + c.x) as usize;
        let cell = |i: usize| IVec2::new(i as i32 % size.x, i as i32 / size.x);

        let start = Self::coarse(terrain, from);
        if !inside(start) {
            return None;
        }
        self.order.clear();
        let mut queue = std::collections::VecDeque::new();
        let t = index(start);
        self.seen[t] = self.mark;
        self.from[t] = t as u32;
        self.steps[t] = 0;
        queue.push_back(start);
        // Hugging a wall (its own cell too tight): start from the open cells around it.
        for dy in -2..=2 {
            for dx in -2..=2 {
                let c = start + IVec2::new(dx, dy);
                if inside(c) && self.seen[index(c)] != self.mark && Self::open(terrain, c) {
                    let i = index(c);
                    self.seen[i] = self.mark;
                    self.from[i] = t as u32;
                    self.steps[i] = 1;
                    queue.push_back(c);
                }
            }
        }
        while let Some(c) = queue.pop_front() {
            let i = index(c);
            self.order.push(i as u32);
            if self.steps[i] >= NAV_RANGE {
                continue;
            }
            for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1), (1, 1), (1, -1), (-1, 1), (-1, -1)] {
                let nc = c + IVec2::new(dx, dy);
                if !inside(nc) || self.seen[index(nc)] == self.mark || !Self::open(terrain, nc) {
                    continue;
                }
                // No cutting corners diagonally.
                if dx != 0 && dy != 0 && !(Self::open(terrain, c + IVec2::new(dx, 0)) && Self::open(terrain, c + IVec2::new(0, dy))) {
                    continue;
                }
                let j = index(nc);
                self.seen[j] = self.mark;
                self.from[j] = i as u32;
                self.steps[j] = self.steps[i] + 1;
                queue.push_back(nc);
            }
        }

        let keep = keep.map(|k| Self::coarse(terrain, k)).filter(|&k| inside(k)).map(index);
        let (mut best, mut kept) = (None::<(usize, f32)>, None::<(usize, f32)>);
        for &i in &self.order {
            let i = i as usize;
            let c = cell(i);
            if !Self::open(terrain, c) {
                continue;
            }
            let Some(t) = score(Self::center(terrain, c), self.steps[i]) else { continue };
            if Some(i) == keep {
                kept = Some((i, t));
            }
            if best.is_none_or(|(_, b)| t < b) {
                best = Some((i, t));
            }
        }
        let (goal, _) = match (best, kept) {
            (Some((_, b)), Some(k)) if k.1 < b + HYSTERESIS => k,
            (best, _) => best?,
        };

        let mut path = Vec::new();
        let mut i = goal;
        while self.steps[i] > 0 {
            path.push(Self::center(terrain, cell(i)));
            i = self.from[i] as usize;
        }
        path.reverse();
        Some((Self::center(terrain, cell(goal)), path))
    }
}
