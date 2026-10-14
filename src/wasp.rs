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
};

static SPEC: BossSpec = BossSpec {
    name: "VENOM MOTHER",
    hp: 14,
    radius: RADIUS,
    blood: Fluid::Green,
    color: Color::srgb(0.36, 0.78, 0.18),
    aggro: AGGRO,
};
/// Size of the boss relative to its drawing.
const SCALE: f32 = 1.7;
const HOVER_HEIGHT: f32 = 230.0;
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
pub fn spawn(commands: &mut Commands, meshes: &mut Assets<Mesh>, materials: &mut Assets<ColorMaterial>, arena: &Arena) {
    let home = Vec2::new(arena.center(), arena.floor + HOVER_HEIGHT);

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
        },
    ));
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
    mut wasps: Query<(Entity, &mut Wasp, &Boss, &mut Transform)>,
    platforms: Query<(&Platform, &Transform), (Without<Wasp>, Without<Player>)>,
) {
    let dt = time.delta_secs().min(1.0 / 30.0);
    let t = time.elapsed_secs();
    let (mut p, ptf) = player.into_inner();
    let ppos = ptf.translation.truncate();

    for (e, mut w, boss, mut tf) in &mut wasps {
        let pos = tf.translation.truncate();
        let to_player = ppos - pos;
        let alive = p.dead.is_none();
        let aggro = alive && to_player.length() < AGGRO;
        let enraged = boss.enraged();
        let side = if to_player.x >= 0.0 { 1.0 } else { -1.0 };
        let wobble = Vec2::new(
            (t * 1.3 + w.seed).sin() * 25.0,
            (t * 2.1 + w.seed).sin() * 14.0,
        );

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
                    if let Some(top) = boss::floor(platforms.iter(), pos, next, 8.0 * SCALE) {
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
                if tf.translation.y < -900.0 {
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
                    w.vel = -dir * 300.0;
                    w.cooldown = 1.2;
                    State::Hover
                } else if s > 1.3 {
                    w.cooldown = 1.0;
                    State::Hover
                } else {
                    State::Dive(s + dt)
                }
            }
            State::Hover | State::Aim(_) => {
                let target = if aggro {
                    ppos + Vec2::new(-side * KEEP_AWAY.x, KEEP_AWAY.y)
                } else {
                    boss.home
                } + wobble;
                let speed = if enraged { SPEED * 1.3 } else { SPEED };
                let mut desired = ((target - pos) * 2.5).clamp_length_max(speed);
                if matches!(w.state, State::Aim(_)) {
                    desired *= 0.2;
                }
                w.vel = w.vel.lerp(desired, 1.0 - (-4.0 * dt).exp());
                if aggro {
                    w.facing = side;
                } else if w.vel.x.abs() > 20.0 {
                    w.facing = w.vel.x.signum();
                }
                w.cooldown -= dt;

                let aim_time = if enraged { AIM_TIME * 0.7 } else { AIM_TIME };
                match w.state {
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
                    _ if aggro && w.cooldown <= 0.0 => {
                        let spits = if enraged { 1 } else { SPITS_BEFORE_DIVE };
                        if w.spits >= spits {
                            w.spits = 0;
                            State::Dive(0.0)
                        } else {
                            State::Aim(0.0)
                        }
                    }
                    _ => State::Hover,
                }
            }
        };

        // (Corpses move themselves above.)
        if !matches!(w.state, State::Dead { .. }) {
            tf.translation.x += w.vel.x * dt;
            tf.translation.y += w.vel.y * dt;
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
        let hit_wall = platforms.iter().any(|(pl, ptf)| {
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
