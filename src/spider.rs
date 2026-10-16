//! The Weaver: a giant spider, about three times the chameleon. It bleeds
//! blue, and beating it gives the chameleon its color and wall walking.
//!
//! It hangs from a thread under the ceiling of its lair, the thread's anchor
//! creeping along the ceiling after the chameleon, out of easy reach (a jump
//! and a slash get there, the venom spit and the air spin too). From up
//! there it spits globs of web: one that hits snares the chameleon (stuck
//! for a moment, then slowed in the web), one that misses leaves a sticky web
//! where it lands (the same webs as the catacombs', `webs.rs`). Now and then
//! it lays egg sacs that drop to the floor and hatch spiderlings after a
//! while, unless the chameleon slashes them first. When the chameleon is
//! right under it, it tenses up and drops: a crash onto the floor that hurts
//! whatever is under it. Then it fights on the floor for a few seconds,
//! biting, and climbs back up its thread: the floor and the climb are the
//! openings. Below half health it spits two at a time, lays more often and
//! more eggs, and stays down less. Health, getting hit and the death
//! spectacle live in `boss.rs`.

use std::f32::consts::PI;

use bevy::{prelude::*, sprite_render::AlphaMode2d};

use crate::{
    boss::{self, Arena, Boss, BossHit, BossSpec, BossStuff},
    camera::{HitStop, MainCamera, Shake},
    chameleon::{self, ChameleonAnim},
    gore::{Bleed, Fluid},
    level::Platform,
    mob_art,
    mobs::{Mob, MobKind, State as MobState},
    player::{self, Player, Respawned},
    shapes::*,
    terrain::Terrain,
    webs::Web,
};

/// Its color (what the chameleon earns, its blood, the health bar).
const BLUE: Color = Color::srgb_u8(0x7f, 0x9f, 0xd6);

pub(crate) static SPEC: BossSpec = BossSpec {
    name: "THE WEAVER",
    hp: 18,
    radius: RADIUS,
    blood: Fluid::Blue,
    color: BLUE,
    aggro: AGGRO,
    // Big, and it keeps a brood: a full meal.
    food: 5.0,
};
/// Size of the boss relative to its drawing.
const SCALE: f32 = 3.2;
/// Hurtbox radius around the middle of its body.
const RADIUS: f32 = 16.0 * SCALE;
const AGGRO: f32 = 800.0;
/// Its body hangs this high over the floor (less under a low ceiling).
const HANG: f32 = 190.0;
/// Least room between the ceiling and its body.
const CEILING_ROOM: f32 = 70.0;
/// How fast its thread's anchor creeps along the ceiling.
const CRAWL: f32 = 140.0;
/// Its legs reach this far below its center (standing on the floor).
const LEGS: f32 = 13.0 * SCALE;
/// Spitting: the tell, the glob's speed and drop, the pause between spits.
const SPIT_WINDUP: f32 = 0.55;
const GLOB_SPEED: f32 = 520.0;
const GLOB_GRAVITY: f32 = 600.0;
const SPIT_EVERY: (f32, f32) = (1.8, 2.8);
/// A glob that hits holds the chameleon this long, in a web this big.
const SNARE_STUN: f32 = 0.45;
const SNARE_RADIUS: f32 = 48.0;
/// Webs left by its globs last this long; at most this many at once.
const WEB_LIFE: f32 = 7.0;
const MAX_WEBS: usize = 5;
/// Laying eggs: how long it takes, how often, how many; how long they take
/// to hatch once down; how many spiderlings may be about.
const LAY_TIME: f32 = 0.9;
const LAY_EVERY: (f32, f32) = (8.0, 11.0);
const EGGS: u32 = 2;
const HATCH: f32 = 2.6;
const MAX_BROOD: usize = 5;
/// Dropping: it drops on the chameleon this close under it (x), after the tell.
const DROP_RANGE: f32 = 120.0;
const DROP_WINDUP: f32 = 0.5;
const DROP_EVERY: f32 = 3.0;
const DROP_DAMAGE: i32 = 1;
/// The crash hurts this far either side of where it lands.
const CRASH_REACH: f32 = 110.0;
/// On the floor: for this long, biting at the chameleon this close.
const GROUND_TIME: f32 = 2.2;
const BITE_RANGE: f32 = 150.0;
const BITE_WINDUP: f32 = 0.4;
const BITE_REACH: f32 = 100.0;
const BITE_DAMAGE: i32 = 1;
const CLIMB_SPEED: f32 = 260.0;
/// The face-off the first time it sees the chameleon.
const TAUNT_TIME: f32 = 1.6;
const TAUNT_BREAK: f32 = 200.0;
/// How far inside the screen edge (px) it must be to count as in view.
const SCREEN_MARGIN: f32 = 60.0;

const BODY: Color = Color::srgb(0.12, 0.11, 0.12);
const HAIR: Color = Color::srgb(0.30, 0.28, 0.29);
const EYE: Color = Color::srgb(0.85, 0.12, 0.10);
const SILK: Color = Color::srgba(0.92, 0.92, 0.90, 0.6);
const EGG: Color = Color::srgb(0.86, 0.84, 0.76);
/// The webs it leaves: see-through, fading out at the end.
const WEB_ALPHA: f32 = 0.35;

pub struct SpiderPlugin;

impl Plugin for SpiderPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, temp_scan); // TEMP-AUTOTEST
        app.add_systems(Startup, load).add_systems(
            Update,
            (reset_on_respawn, on_hit, think, move_globs, eggs, fade_webs, dismember, animate)
                .chain()
                .after(player::move_player)
                .after(chameleon::update_anim_state),
        );
    }
}

#[derive(Component)]
struct Spider {
    state: State,
    vel: Vec2,
    facing: f32,
    seed: f32,
    rng: Rng,
    /// Where its thread is fixed (on the ceiling).
    anchor: Vec2,
    /// The floor it fights over, and its walkable span.
    floor: f32,
    span: (f32, f32),
    spit_cd: f32,
    lay_cd: f32,
    drop_cd: f32,
    /// Where it aimed its spit (or bite).
    plan: Vec2,
    introduced: bool,
    dismember: bool,
    /// A flurry of hits: answer (see `BossHit::retaliate`).
    retaliate: bool,
}

#[derive(Clone, Copy, PartialEq)]
enum State {
    Hang,
    Spit(f32),
    Lay { t: f32, laid: bool },
    /// Tensing up before the drop.
    Brace(f32),
    Drop,
    /// On the floor (seconds there), and biting.
    Ground(f32),
    Bite { t: f32, ground: f32 },
    Climb,
    Taunt(f32),
    Dead { t: f32, angle: f32, landed: bool },
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
    /// `i`: 0 = front .. 3 = hind.
    Leg { i: usize, near: bool },
    Fang { side: f32 },
}

/// Its thread, from the body up to the anchor (a unit-long strip, scaled).
#[derive(Component)]
struct Thread {
    owner: Entity,
}

/// A glob of web in flight.
#[derive(Component)]
struct Glob {
    vel: Vec2,
    life: f32,
}

/// An egg sac: falling, then waiting to hatch.
#[derive(Component)]
struct Egg {
    owner: Entity,
    vel: Vec2,
    /// Seconds since it landed (`None` while falling).
    down: Option<f32>,
}

/// A web left by a glob (`webs.rs` makes it sticky through its `Web`).
#[derive(Component)]
struct Snare {
    age: f32,
    mat: Handle<ColorMaterial>,
}

#[derive(Resource)]
struct Assets2 {
    glob: Handle<Mesh>,
    egg: Handle<Mesh>,
    web: Handle<Mesh>,
    silk: Handle<ColorMaterial>,
    egg_mat: Handle<ColorMaterial>,
}

fn load(mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>, mut materials: ResMut<Assets<ColorMaterial>>) {
    let glob = blobs(
        &[ellipse_pts(7.0, 5.5, 14), ellipse_pts(3.0, 2.5, 8).into_iter().map(|q| q + Vec2::new(-7.0, 2.0)).collect()],
        |q| 0.8 + 0.2 * (q.y / 6.0),
    );
    let egg = blobs(
        &[(-5.0, 0.0), (4.0, 1.0), (0.0, 6.0), (-2.0, -3.0), (5.0, -3.0)]
            .map(|(x, y)| ellipse_pts(6.0, 5.2, 12).into_iter().map(|q| q + Vec2::new(x, y)).collect::<Vec<_>>()),
        |q| 0.7 + 0.3 * ((q.y + 8.0) / 18.0).clamp(0.0, 1.0),
    );
    commands.insert_resource(Assets2 {
        glob: meshes.add(glob),
        egg: meshes.add(egg),
        web: meshes.add(orb_web(SNARE_RADIUS)),
        silk: materials.add(ColorMaterial { color: SILK, alpha_mode: AlphaMode2d::Blend, ..default() }),
        egg_mat: materials.add(EGG),
    });
}

/// An orb web of radius `r`: spokes and a spiral of rings.
/// `x` kept `margin` inside `span`; a span too narrow for the margin (a sketch edit
/// can shrink the arena) gives its middle instead of panicking like `f32::clamp`.
fn clamp_span(x: f32, span: (f32, f32), margin: f32) -> f32 {
    let (lo, hi) = (span.0 + margin, span.1 - margin);
    if lo <= hi { x.clamp(lo, hi) } else { (span.0 + span.1) * 0.5 }
}

fn orb_web(r: f32) -> Mesh {
    let mut tris = Vec::new();
    let spokes = 12;
    let line = |a: Vec2, b: Vec2, w: f32, tris: &mut Vec<[Vec2; 3]>| {
        let n = (b - a).normalize_or(Vec2::X).perp() * w * 0.5;
        tris.push([a - n, a + n, b + n]);
        tris.push([a - n, b + n, b - n]);
    };
    for i in 0..spokes {
        let a = i as f32 / spokes as f32 * 2.0 * PI;
        line(Vec2::ZERO, Vec2::from_angle(a) * r, 1.0, &mut tris);
    }
    for ring in 1..=5 {
        let rr = r * ring as f32 / 5.5;
        for i in 0..spokes {
            let (a, b) = (i as f32 / spokes as f32 * 2.0 * PI, (i + 1) as f32 / spokes as f32 * 2.0 * PI);
            line(Vec2::from_angle(a) * rr, Vec2::from_angle(b) * rr * 0.97, 0.7, &mut tris);
        }
    }
    triangles(&tris)
}

/// The ceiling over `x` (where the thread can be fixed), if there is one within reach.
fn ceiling(terrain: &Terrain, x: f32, floor: f32) -> Option<f32> {
    terrain.raycast(Vec2::new(x, floor + 40.0), Vec2::new(x, floor + 1400.0)).map(|hit| hit.y)
}

/// How high its body hangs at `x`: `HANG` over the floor, under the ceiling.
fn hang_y(s: &Spider, terrain: &Terrain, x: f32) -> f32 {
    let top = ceiling(terrain, x, s.floor).unwrap_or(s.anchor.y);
    (s.floor + HANG).min(top - CEILING_ROOM).max(s.floor + LEGS + 20.0)
}

/// Spawns the spider hanging over its arena.
pub fn spawn(commands: &mut Commands, meshes: &mut Assets<Mesh>, materials: &mut Assets<ColorMaterial>, arena: &Arena, terrain: &Terrain) -> Entity {
    let home = arena.home;
    let body = materials.add(BODY);
    let hair = materials.add(HAIR);
    let eye = materials.add(EYE);
    let mark = materials.add(BLUE);
    let silk = materials.add(ColorMaterial { color: SILK, alpha_mode: AlphaMode2d::Blend, ..default() });
    let mats = vec![(body.clone(), BODY), (hair.clone(), HAIR), (eye.clone(), EYE), (mark.clone(), BLUE)];

    let floor = terrain.rest(home).y;
    let anchor = Vec2::new(home.x, ceiling(terrain, home.x, floor).unwrap_or(home.y + 500.0));
    let root = commands.spawn((Transform::from_translation(home.extend(12.0)), Visibility::default())).id();
    let mut b = Builder { commands, meshes, owner: root };

    // The thread (not in the rig: it doesn't turn with the body).
    let thread = b.commands.spawn((Thread { owner: root }, Transform::from_xyz(0.0, 0.0, -0.6), Visibility::default(), ChildOf(root))).id();
    b.part(stroke(&[Vec2::ZERO, Vec2::Y], |_| 1.4, 1.0, 1.0, |_| 1.0), &silk, Vec3::ZERO, thread);

    let rig = b.bone(Vec3::ZERO, root, BoneKind::Rig);
    b.commands.entity(rig).insert(Transform::from_scale(Vec3::new(-SCALE, SCALE, 1.0)));

    // Legs: four a side, two segments each, the far ones darker and behind.
    for near in [false, true] {
        for i in 0..4 {
            let k = i as f32;
            let hip = Vec2::new(5.0 - k * 2.6, -1.0);
            let bone = b.bone(hip.extend(if near { 0.3 } else { -0.3 }), rig, BoneKind::Leg { i, near });
            // Front legs reach forward, hind legs back; all arch up, then down to the floor.
            let reach = [16.0, 9.0, -4.0, -14.0][i];
            let knee = Vec2::new(reach * 0.55, 9.0 - k * 0.8);
            let foot = Vec2::new(reach, -12.0);
            let path = smooth_open(&[Vec2::ZERO, knee * 0.6 + Vec2::Y * 2.0, knee, (knee + foot) * 0.5 + Vec2::X * reach.signum() * 1.5, foot], 4);
            let mat = if near { &hair } else { &body };
            b.part(stroke(&path, |t| 2.4 * (1.0 - t) + 0.6, 1.0, 0.7, |_| 1.0), mat, Vec3::ZERO, bone);
        }
    }

    // Abdomen: big, round, with a blue mark; spinnerets at the back.
    let abdomen = b.bone(Vec3::new(-6.0, 2.0, 0.1), rig, BoneKind::Abdomen);
    b.part(fill(&ellipse_pts(13.0, 11.0, 32), |p| 0.55 + 0.45 * ((p.y + 11.0) / 22.0)), &body, Vec3::new(-7.0, 1.0, 0.0), abdomen);
    let markings: Vec<Vec<Vec2>> = [(-7.0, 7.5, 2.6), (-11.5, 5.5, 2.0), (-3.0, 6.0, 1.8)]
        .map(|(x, y, r)| ellipse_pts(r, r * 0.7, 10).into_iter().map(|q| q + Vec2::new(x, y)).collect::<Vec<_>>())
        .to_vec();
    b.part(blobs(&markings, |_| 1.0), &mark, Vec3::new(0.0, 0.0, 0.02), abdomen);
    b.part(triangles(&[[Vec2::new(-19.0, 3.0), Vec2::new(-19.5, -1.0), Vec2::new(-23.0, 0.5)]]), &hair, Vec3::new(0.0, 0.0, 0.01), abdomen);

    // Front body and head: eight red eyes, two fangs.
    b.part(fill(&ellipse_pts(8.0, 6.5, 24), |p| 0.65 + 0.35 * ((p.y + 6.5) / 13.0)), &body, Vec3::new(4.0, 0.0, 0.2), rig);
    let head = b.bone(Vec3::new(10.0, 0.5, 0.25), rig, BoneKind::Head);
    b.part(fill(&ellipse_pts(4.5, 4.0, 18), |p| 0.7 + 0.3 * ((p.y + 4.0) / 8.0)), &body, Vec3::ZERO, head);
    let eyes: Vec<Vec<Vec2>> = [(2.0, 2.4, 1.0), (3.6, 1.6, 0.9), (0.6, 2.9, 0.7), (4.4, 0.3, 0.6), (1.2, 1.2, 0.5), (2.8, 0.2, 0.5)]
        .map(|(x, y, r)| ellipse_pts(r, r, 8).into_iter().map(|q| q + Vec2::new(x, y)).collect::<Vec<_>>())
        .to_vec();
    b.part(blobs(&eyes, |_| 1.0), &eye, Vec3::new(0.0, 0.0, 0.02), head);
    for side in [1.0, -1.0] {
        let fang = b.bone(Vec3::new(3.5, -2.5, 0.03 * side), head, BoneKind::Fang { side });
        let path = smooth_open(&[Vec2::ZERO, Vec2::new(1.5, -2.5), Vec2::new(0.5, -5.0)], 4);
        b.part(stroke(&path, |t| 1.8 * (1.0 - t) + 0.3, 1.0, 0.8, |_| 1.0), &hair, Vec3::ZERO, fang);
    }

    let mut rng = Rng(0x5B1D_E7A1);
    commands.entity(root).insert((
        Boss::new(&SPEC, home, mats).staged(),
        Spider {
            state: State::Hang,
            vel: Vec2::ZERO,
            facing: -1.0,
            seed: rng.range(0.0, 100.0),
            anchor,
            floor,
            span: (arena.left, arena.right),
            spit_cd: rng.range(1.0, 2.0),
            lay_cd: rng.range(LAY_EVERY.0 * 0.5, LAY_EVERY.0),
            drop_cd: 1.0,
            plan: home,
            introduced: false,
            dismember: false,
            retaliate: false,
            rng,
        },
    ));
    root
}

struct Builder<'a, 'w, 's> {
    commands: &'a mut Commands<'w, 's>,
    meshes: &'a mut Assets<Mesh>,
    owner: Entity,
}

impl Builder<'_, '_, '_> {
    fn part(&mut self, mesh: Mesh, mat: &Handle<ColorMaterial>, pos: Vec3, parent: Entity) {
        let mesh = self.meshes.add(mesh);
        self.commands.spawn((Mesh2d(mesh), MeshMaterial2d(mat.clone()), Transform::from_translation(pos), ChildOf(parent)));
    }

    fn bone(&mut self, pos: Vec3, parent: Entity, kind: BoneKind) -> Entity {
        self.commands
            .spawn((Bone { owner: self.owner, kind }, Transform::from_translation(pos), Visibility::default(), ChildOf(parent)))
            .id()
    }
}

/// Where its mouth is.
fn mouth(pos: Vec2, facing: f32) -> Vec2 {
    pos + Vec2::new(facing * 15.0, -3.0) * SCALE
}

fn think(
    time: Res<Time>,
    mut commands: Commands,
    mut shake: ResMut<Shake>,
    mut stop: ResMut<HitStop>,
    mut bleed: MessageWriter<Bleed>,
    assets: Res<Assets2>,
    player: Single<(&mut Player, &Transform), Without<Spider>>,
    camera: Single<(&Camera, &GlobalTransform), With<MainCamera>>,
    mut spiders: Query<(Entity, &mut Spider, &mut Boss, &mut Transform, Option<&mut boss::Sense>)>,
    eggs: Query<&Egg>,
    brood: Query<&Mob>,
    platforms: Query<(&Platform, &Transform), (Without<Spider>, Without<Player>)>,
    terrain: Res<Terrain>,
) {
    let dt = time.delta_secs().min(1.0 / 30.0) * crate::boss::tempo();
    let t = time.elapsed_secs();
    let (mut p, ptf) = player.into_inner();
    let ppos = ptf.translation.truncate();
    let alive_brood = brood.iter().filter(|m| m.kind == MobKind::Spiderling && !matches!(m.state, MobState::Dead { .. })).count();

    for (e, mut s, mut boss, mut tf, sense) in &mut spiders {
        let pos = tf.translation.truncate();
        let to_player = ppos - pos;
        let alive = p.dead.is_none();
        let hunting = alive && boss.awake;
        let aggro = hunting && s.introduced;
        let enraged = boss.enraged();
        let side = if to_player.x >= 0.0 { 1.0 } else { -1.0 };
        let on_screen = match (camera.0.world_to_viewport(camera.1, pos.extend(0.0)), camera.0.logical_viewport_size()) {
            (Ok(v), Some(size)) => v.x > SCREEN_MARGIN && v.y > SCREEN_MARGIN && v.x < size.x - SCREEN_MARGIN && v.y < size.y - SCREEN_MARGIN,
            _ => false,
        };
        if !s.introduced {
            boss.staged = true;
            if hunting && on_screen && s.state == State::Hang {
                s.introduced = true;
                boss.staged = false;
                s.state = State::Taunt(0.0);
                shake.add(0.25);
            }
        }
        let sees = terrain.raycast(mouth(pos, s.facing), ppos).is_none();
        if let Some(mut sense) = sense {
            sense.goal = Some(Vec2::new(s.anchor.x, hang_y(&s, &terrain, s.anchor.x)));
            sense.target = Some(s.anchor);
            sense.sees_player = sees;
            sense.path.clear();
        }
        s.spit_cd -= dt;
        s.lay_cd -= dt;
        s.drop_cd -= dt;
        let hung = matches!(s.state, State::Hang | State::Spit(_) | State::Lay { .. } | State::Brace(_) | State::Taunt(_));

        // Its thread's anchor creeps along the ceiling: over the chameleon
        // while it hunts, home otherwise (never off its floor).
        if hung {
            let want = if aggro { ppos.x } else { boss.home.x };
            let want = clamp_span(want, s.span, 60.0);
            let speed = if matches!(s.state, State::Brace(_)) { 0.0 } else { CRAWL * if enraged { 1.25 } else { 1.0 } };
            let next = s.anchor.x + (want - s.anchor.x).clamp(-speed * dt, speed * dt);
            if let Some(top) = ceiling(&terrain, next, s.floor) {
                s.anchor = Vec2::new(next, top);
            }
        }

        // Answer a flurry: from the floor it flees up its thread, up there it spits at once.
        if std::mem::take(&mut s.retaliate) {
            s.state = match s.state {
                State::Ground(_) | State::Bite { .. } => State::Climb,
                State::Hang if sees => {
                    s.plan = ppos;
                    State::Spit(SPIT_WINDUP * 0.5)
                }
                other => other,
            };
        }

        s.state = match s.state {
            State::Dead { t: t0, angle, landed } => {
                let td = t0 + dt;
                let mut landed = landed;
                if !landed {
                    s.vel.y -= 1400.0 * dt;
                    let next = pos + s.vel * dt;
                    if let Some(top) = boss::floor(&terrain, platforms.iter(), pos, next, 6.0 * SCALE) {
                        tf.translation.x = next.x;
                        tf.translation.y = top + 6.0 * SCALE;
                        landed = true;
                        shake.add(0.4);
                        bleed.write(Bleed { pos, dir: Vec2::Y, spread: 1.4, speed: 300.0, amount: 50, fluid: Fluid::Blue, sticky: true });
                    } else {
                        tf.translation.x = next.x;
                        tf.translation.y = next.y;
                    }
                }
                if td < 3.5 && s.rng.chance(dt * if landed { 8.0 } else { 20.0 }) {
                    bleed.write(Bleed { pos: tf.translation.truncate(), dir: Vec2::Y, spread: 1.0, speed: 140.0, amount: 3, fluid: Fluid::Blue, sticky: true });
                }
                // Rolls over onto its back.
                let angle = if landed { angle + (PI - angle) * (1.0 - (-6.0 * dt).exp()) } else { angle + 2.0 * dt };
                State::Dead { t: td, angle, landed }
            }
            State::Taunt(s0) => {
                // Lowers itself a little, legs up, and hisses.
                s.facing = side;
                if !alive {
                    State::Hang
                } else if s0 >= TAUNT_TIME || to_player.length() < TAUNT_BREAK {
                    s.spit_cd = 0.6;
                    State::Hang
                } else {
                    if s0 < 0.6 && s0 + dt >= 0.6 {
                        shake.add(0.3);
                    }
                    State::Taunt(s0 + dt)
                }
            }
            State::Hang => {
                if aggro {
                    s.facing = side;
                }
                let under = (ppos.x - pos.x).abs() < DROP_RANGE && ppos.y < pos.y - 40.0 && p.grounded;
                match () {
                    _ if !aggro => State::Hang,
                    _ if under && s.drop_cd <= 0.0 => State::Brace(0.0),
                    _ if s.lay_cd <= 0.0 && alive_brood + eggs.iter().filter(|g| g.owner == e).count() < MAX_BROOD => State::Lay { t: 0.0, laid: false },
                    _ if s.spit_cd <= 0.0 && sees && to_player.length() < AGGRO => {
                        s.plan = ppos;
                        State::Spit(0.0)
                    }
                    _ => State::Hang,
                }
            }
            State::Spit(s0) => {
                s.facing = if s.plan.x >= pos.x { 1.0 } else { -1.0 };
                if !alive {
                    State::Hang
                } else if s0 >= SPIT_WINDUP {
                    // Aimed where the chameleon was when it started (it leads a little).
                    let at = s.plan.lerp(ppos, 0.3);
                    let fan: &[f32] = if enraged { &[-0.14, 0.14] } else { &[0.0] };
                    for &a in fan {
                        spit(&mut commands, &assets, mouth(pos, s.facing), at, a);
                    }
                    let k = if enraged { 0.7 } else { 1.0 };
                    s.spit_cd = s.rng.range(SPIT_EVERY.0, SPIT_EVERY.1) * k;
                    State::Hang
                } else {
                    State::Spit(s0 + dt)
                }
            }
            State::Lay { t: s0, laid } => {
                let mut laid = laid;
                if !laid && s0 >= LAY_TIME * 0.55 {
                    laid = true;
                    let n = EGGS + u32::from(enraged);
                    let back = pos + Vec2::new(-s.facing * 20.0, -2.0) * SCALE;
                    for i in 0..n {
                        let vx = (i as f32 - (n - 1) as f32 * 0.5) * 110.0 + s.rng.range(-30.0, 30.0);
                        commands.spawn((
                            Egg { owner: e, vel: Vec2::new(vx, 60.0), down: None },
                            Mesh2d(assets.egg.clone()),
                            MeshMaterial2d(assets.egg_mat.clone()),
                            Transform::from_translation(back.extend(11.5)),
                            BossStuff,
                        ));
                    }
                }
                if s0 >= LAY_TIME {
                    s.lay_cd = s.rng.range(LAY_EVERY.0, LAY_EVERY.1) * if enraged { 0.7 } else { 1.0 };
                    State::Hang
                } else {
                    State::Lay { t: s0 + dt, laid }
                }
            }
            State::Brace(s0) => {
                if !alive {
                    State::Hang
                } else if s0 >= DROP_WINDUP * if enraged { 0.75 } else { 1.0 } {
                    s.vel = Vec2::new(0.0, -300.0);
                    State::Drop
                } else {
                    State::Brace(s0 + dt)
                }
            }
            State::Drop => {
                s.vel.y = (s.vel.y - 2600.0 * dt).max(-1400.0);
                let next = pos + s.vel * dt;
                let ground = terrain.surface(Vec2::new(next.x, next.y), LEGS + 4.0).filter(|g| next.y - g <= LEGS);
                if let Some(g) = ground.or_else(|| (next.y - LEGS <= s.floor).then_some(s.floor)) {
                    tf.translation.y = g + LEGS;
                    s.vel = Vec2::ZERO;
                    // The crash: hurts whatever is under it, shakes everything.
                    shake.add(0.6);
                    stop.add(0.06);
                    let d = ppos - Vec2::new(pos.x, g);
                    if alive && d.x.abs() < CRASH_REACH && d.y < LEGS + 30.0 {
                        boss::hurt_player(&mut p, ppos, pos, DROP_DAMAGE, &mut bleed, (&mut shake, &mut stop));
                    }
                    s.drop_cd = DROP_EVERY;
                    State::Ground(0.0)
                } else {
                    tf.translation.y = next.y;
                    State::Drop
                }
            }
            State::Ground(g) => {
                s.facing = side;
                // Shuffles toward the chameleon a little, on its floor.
                let want = if aggro && to_player.x.abs() > BITE_REACH * 0.6 { side * 70.0 } else { 0.0 };
                s.vel.x = s.vel.x + (want - s.vel.x) * (1.0 - (-6.0 * dt).exp());
                let x = clamp_span(pos.x + s.vel.x * dt, s.span, 40.0);
                if !terrain.box_hits(Vec2::new(x, pos.y + 10.0), Vec2::new(30.0, 20.0)) {
                    tf.translation.x = x;
                }
                let stay = if enraged { GROUND_TIME * 0.7 } else { GROUND_TIME };
                if !aggro || g >= stay {
                    State::Climb
                } else if to_player.length() < BITE_RANGE && g > 0.35 {
                    s.plan = ppos;
                    State::Bite { t: 0.0, ground: g }
                } else {
                    State::Ground(g + dt)
                }
            }
            State::Bite { t: s0, ground } => {
                s.vel.x = 0.0;
                if s0 < BITE_WINDUP && s0 + dt >= BITE_WINDUP {
                    // Lunges and snaps where the chameleon was.
                    tf.translation.x += s.facing * 25.0;
                    if alive && mouth(tf.translation.truncate(), s.facing).distance(ppos) < BITE_REACH {
                        boss::hurt_player(&mut p, ppos, pos, BITE_DAMAGE, &mut bleed, (&mut shake, &mut stop));
                    }
                }
                if s0 >= BITE_WINDUP + 0.35 {
                    State::Ground(ground + s0 + 0.3)
                } else {
                    State::Bite { t: s0 + dt, ground }
                }
            }
            State::Climb => {
                // Reels itself up its thread (the anchor comes over it first).
                let top = hang_y(&s, &terrain, pos.x);
                if let Some(c) = ceiling(&terrain, pos.x, s.floor) {
                    s.anchor = Vec2::new(pos.x, c);
                }
                tf.translation.y = (pos.y + CLIMB_SPEED * dt).min(top);
                if tf.translation.y >= top - 1.0 {
                    s.spit_cd = s.spit_cd.max(0.8);
                    State::Hang
                } else {
                    State::Climb
                }
            }
        };

        // Hanging, the body swings under its anchor.
        if matches!(s.state, State::Hang | State::Spit(_) | State::Lay { .. } | State::Brace(_) | State::Taunt(_)) {
            let mut target = Vec2::new(s.anchor.x, hang_y(&s, &terrain, s.anchor.x));
            if let State::Taunt(s0) = s.state {
                target.y -= 40.0 * (s0 / 0.6).min(1.0);
            }
            let swing = Vec2::new((t * 1.1 + s.seed).sin() * 10.0, (t * 0.8 + s.seed).sin() * 6.0);
            let goal = target + swing;
            s.vel = s.vel.lerp((goal - pos) * 3.0, 1.0 - (-4.0 * dt).exp());
            tf.translation.x += s.vel.x * dt;
            tf.translation.y += s.vel.y * dt;
        }
    }
}

fn spit(commands: &mut Commands, assets: &Assets2, from: Vec2, at: Vec2, angle: f32) {
    let flight = (at - from).length() / GLOB_SPEED;
    let dir = Vec2::from_angle(angle).rotate((at - from).normalize_or(Vec2::NEG_Y));
    let vel = dir * GLOB_SPEED + Vec2::Y * GLOB_GRAVITY * flight * 0.5;
    commands.spawn((
        Glob { vel, life: 3.0 },
        Mesh2d(assets.glob.clone()),
        MeshMaterial2d(assets.silk.clone()),
        Transform::from_translation(from.extend(15.0)),
        BossStuff,
    ));
}

/// Leaves a sticky web at `at`.
fn snare(commands: &mut Commands, materials: &mut Assets<ColorMaterial>, assets: &Assets2, at: Vec2, scale: f32) {
    let mat = materials.add(ColorMaterial { color: Color::srgba(0.92, 0.92, 0.90, WEB_ALPHA), alpha_mode: AlphaMode2d::Blend, ..default() });
    commands.spawn((
        Snare { age: 0.0, mat: mat.clone() },
        Web { center: Vec2::ZERO, radius: SNARE_RADIUS },
        Mesh2d(assets.web.clone()),
        MeshMaterial2d(mat),
        Transform::from_translation(at.extend(14.0)).with_scale(Vec3::splat(scale)),
        BossStuff,
    ));
}

fn move_globs(
    mut commands: Commands,
    time: Res<Time>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    assets: Res<Assets2>,
    mut shake: ResMut<Shake>,
    player: Single<(&mut Player, &Transform), Without<Glob>>,
    mut globs: Query<(Entity, &mut Glob, &mut Transform)>,
    terrain: Res<Terrain>,
) {
    let dt = time.delta_secs().min(1.0 / 30.0);
    let (mut p, ptf) = player.into_inner();
    let ppos = ptf.translation.truncate();
    for (e, mut g, mut tf) in &mut globs {
        g.vel.y -= GLOB_GRAVITY * dt;
        g.life -= dt;
        let pos = tf.translation.truncate() + g.vel * dt;
        tf.translation.x = pos.x;
        tf.translation.y = pos.y;
        tf.rotation = Quat::from_rotation_z(g.vel.to_angle());
        let d = (pos - ppos).abs();
        if p.dead.is_none() && d.x < player::HALF.x + 6.0 && d.y < player::HALF.y + 6.0 {
            // Snared: held for a moment, then slowed in the web around it.
            if p.guard <= 0.0 && !p.god.on() {
                p.stun = p.stun.max(SNARE_STUN);
                p.vel *= 0.2;
                p.climbing = None;
            }
            snare(&mut commands, &mut materials, &assets, ppos, 1.0);
            shake.add(0.1);
            commands.entity(e).despawn();
        } else if terrain.solid_at(pos) || g.life <= 0.0 {
            // A web where it lands, a little back out of the wall.
            let back = pos - g.vel.normalize_or(Vec2::NEG_Y) * 10.0;
            snare(&mut commands, &mut materials, &assets, back, 0.85);
            commands.entity(e).despawn();
        }
    }
}

/// Webs fade away after a while, the oldest first when there are too many.
fn fade_webs(mut commands: Commands, time: Res<Time>, mut materials: ResMut<Assets<ColorMaterial>>, mut webs: Query<(Entity, &mut Snare, &mut Web)>) {
    let dt = time.delta_secs();
    let mut all: Vec<(Entity, f32)> = webs.iter().map(|(e, s, _)| (e, s.age)).collect();
    all.sort_by(|a, b| b.1.total_cmp(&a.1));
    let extra = all.len().saturating_sub(MAX_WEBS);
    for (e, _) in all.iter().take(extra) {
        if let Ok((_, mut s, _)) = webs.get_mut(*e) {
            s.age = s.age.max(WEB_LIFE - 0.5);
        }
    }
    for (e, mut s, mut web) in &mut webs {
        s.age += dt;
        let fade = ((WEB_LIFE - s.age) / 0.8).clamp(0.0, 1.0);
        web.radius = SNARE_RADIUS * fade;
        if let Some(mut m) = materials.get_mut(&s.mat) {
            m.color.set_alpha(WEB_ALPHA * fade);
        }
        if s.age >= WEB_LIFE {
            commands.entity(e).despawn();
        }
    }
}

/// Eggs fall, sit and wobble, then hatch spiderlings, unless slashed first.
fn eggs(
    mut commands: Commands,
    time: Res<Time>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    mut bleed: MessageWriter<Bleed>,
    mut shake: ResMut<Shake>,
    player: Single<(&Player, &Transform, &ChameleonAnim)>,
    mut eggs: Query<(Entity, &mut Egg, &mut Transform), Without<Player>>,
    platforms: Query<(&Platform, &Transform), (Without<Egg>, Without<Player>)>,
    terrain: Res<Terrain>,
) {
    let dt = time.delta_secs().min(1.0 / 30.0);
    let (p, ptf, anim) = *player;
    let ppos = ptf.translation.truncate();
    for (e, mut g, mut tf) in &mut eggs {
        let pos = tf.translation.truncate();
        // Claws (or the spin) burst it.
        if anim.striking() && anim.reaches(ppos, p.facing, pos, 12.0) {
            bleed.write(Bleed { pos, dir: Vec2::Y, spread: 1.2, speed: 260.0, amount: 14, fluid: Fluid::Blue, sticky: true });
            shake.add(0.08);
            commands.entity(e).despawn();
            continue;
        }
        match g.down {
            None => {
                g.vel.y -= 1400.0 * dt;
                g.vel.x *= 1.0 - 1.5 * dt;
                let next = pos + g.vel * dt;
                if let Some(top) = boss::floor(&terrain, platforms.iter(), pos, next, 8.0) {
                    tf.translation.x = next.x;
                    tf.translation.y = top + 8.0;
                    g.down = Some(0.0);
                } else if terrain.solid_at(next) {
                    g.vel.x = 0.0;
                } else {
                    tf.translation.x = next.x;
                    tf.translation.y = next.y;
                }
                if tf.translation.y < terrain.bounds.min.y - 600.0 {
                    commands.entity(e).despawn();
                }
            }
            Some(s) => {
                let s = s + dt;
                g.down = Some(s);
                // Wobbles more and more as it's about to hatch.
                let k = (s / HATCH).min(1.0);
                tf.rotation = Quat::from_rotation_z((s * (6.0 + 20.0 * k)).sin() * 0.25 * k * k);
                tf.scale = Vec3::new(1.0 + 0.08 * k * (s * 9.0).sin(), 1.0 - 0.06 * k * (s * 9.0).sin(), 1.0);
                if s >= HATCH {
                    let at = Vec2::new(pos.x, pos.y - 8.0 + MobKind::Spiderling.spec().foot);
                    mob_art::spawn(&mut commands, &mut meshes, &mut materials, MobKind::Spiderling, at, Some(g.owner));
                    bleed.write(Bleed { pos, dir: Vec2::Y, spread: 1.0, speed: 160.0, amount: 6, fluid: Fluid::Blue, sticky: true });
                    commands.entity(e).despawn();
                }
            }
        }
    }
}

/// React to a hit (health and blood are handled in `boss.rs`).
fn on_hit(mut hits: MessageReader<BossHit>, mut spiders: Query<&mut Spider>) {
    for hit in hits.read() {
        let Ok(mut s) = spiders.get_mut(hit.boss) else { continue };
        if hit.killed {
            // The thread snaps: it drops, curling up.
            s.vel = Vec2::new(hit.dir.x * 160.0, 200.0);
            s.state = State::Dead { t: 0.0, angle: 0.0, landed: matches!(s.state, State::Ground(_) | State::Bite { .. }) };
            s.dismember = true;
        } else if hit.retaliate {
            s.retaliate = true;
        } else if hit.stagger {
            // Too heavy to knock about: it sways on its thread.
            s.vel += hit.dir * 120.0;
        }
    }
}

/// The chameleon came back: start over (`boss.rs` heals it).
fn reset_on_respawn(mut commands: Commands, mut respawned: MessageReader<Respawned>, stuff: Query<Entity, Or<(With<Egg>, With<Glob>, With<Snare>)>>) {
    if respawned.read().count() == 0 {
        return;
    }
    // (The boss itself is respawned at home by `boss.rs`.)
    for e in &stuff {
        commands.entity(e).despawn();
    }
}

/// Tear two legs off a freshly killed spider.
fn dismember(mut commands: Commands, mut spiders: Query<&mut Spider>, bones: Query<(Entity, &Bone, &GlobalTransform)>) {
    let mut done = Vec::new();
    for (e, bone, gt) in &bones {
        let Ok(s) = spiders.get(bone.owner) else { continue };
        if !s.dismember {
            continue;
        }
        done.push(bone.owner);
        let f = s.facing;
        let (vel, spin) = match bone.kind {
            BoneKind::Leg { i: 0, near: true } => (Vec2::new(f * 260.0, 620.0), 9.0),
            BoneKind::Leg { i: 3, near: true } => (Vec2::new(-f * 220.0, 560.0), -8.0),
            _ => continue,
        };
        commands.entity(e).remove::<Bone>();
        boss::tear_off(&mut commands, e, gt, vel, spin, Some(Fluid::Blue));
    }
    for owner in done {
        if let Ok(mut s) = spiders.get_mut(owner) {
            s.dismember = false;
        }
    }
}

fn animate(
    time: Res<Time>,
    spiders: Query<(&Spider, &Transform)>,
    mut bones: Query<(&Bone, &mut Transform), (Without<Spider>, Without<Thread>)>,
    mut threads: Query<(&Thread, &mut Transform, &mut Visibility), (Without<Spider>, Without<Bone>)>,
) {
    let t = time.elapsed_secs();
    let dt = time.delta_secs();
    for (bone, mut tf) in &mut bones {
        let Ok((s, _)) = spiders.get(bone.owner) else { continue };
        let tt = t + s.seed;
        let dead = matches!(s.state, State::Dead { .. });
        let brace = match s.state {
            State::Brace(b) => (b / DROP_WINDUP).min(1.0),
            _ => 0.0,
        };
        let spit = match s.state {
            State::Spit(b) => (b / SPIT_WINDUP).min(1.0),
            _ => 0.0,
        };
        let taunt = match s.state {
            State::Taunt(b) => (b / 0.6).min(1.0) * (1.0 - ((b - TAUNT_TIME + 0.4) / 0.4).clamp(0.0, 1.0)),
            _ => 0.0,
        };
        let bite = match s.state {
            State::Bite { t: b, .. } => (b / BITE_WINDUP).min(1.0),
            _ => 0.0,
        };
        let ground = matches!(s.state, State::Ground(_) | State::Bite { .. });
        let jitter = brace.max(taunt) * (tt * 60.0).sin() * 0.05;
        match bone.kind {
            BoneKind::Rig => {
                let target = s.facing * SCALE;
                tf.scale.x += (target - tf.scale.x) * (1.0 - (-10.0 * dt).exp());
                let angle = match s.state {
                    State::Dead { angle, .. } => angle * s.facing,
                    // Hanging, the front tips down a little; it rears up for the bite and the hiss.
                    _ if ground => 0.25 * bite * s.facing,
                    _ => (-0.2 + 0.35 * taunt + 0.15 * spit) * s.facing + jitter,
                };
                tf.rotation = Quat::from_rotation_z(angle);
            }
            BoneKind::Leg { i, near } => {
                let k = i as f32;
                let angle = if dead {
                    // Curled up under it, twitching for a while.
                    let td = match s.state {
                        State::Dead { t, .. } => t,
                        _ => 0.0,
                    };
                    -0.9 + (tt * 25.0 + k).sin() * 0.15 * (1.0 - td / 3.0).max(0.0)
                } else if ground {
                    // Walking: alternating legs.
                    let group = if (i % 2 == 0) == near { 0.0 } else { PI };
                    let step = (tt * 9.0 + group).sin() * (s.vel.x.abs() / 70.0).min(1.0) * 0.25;
                    step + 0.2 * bite * if i < 2 { 1.0 } else { -0.3 }
                } else {
                    // Hanging: legs flex slowly; spread wide when it braces or hisses.
                    let flex = (tt * 1.6 + k * 0.9 + if near { 0.0 } else { 0.5 }).sin() * 0.08;
                    let spread = (brace + taunt) * if i < 2 { 0.45 } else { -0.3 };
                    flex + spread - 0.15
                };
                tf.rotation = Quat::from_rotation_z(angle);
            }
            BoneKind::Abdomen => {
                let lay = match s.state {
                    State::Lay { t: b, .. } => (b * 14.0).sin() * 0.08 - 0.25,
                    _ => 0.0,
                };
                tf.rotation = Quat::from_rotation_z((tt * 1.3).sin() * 0.04 + lay + 0.2 * spit - if dead { 0.2 } else { 0.0 });
            }
            BoneKind::Head => {
                tf.rotation = Quat::from_rotation_z(0.25 * spit + 0.2 * taunt - 0.1 * bite);
            }
            BoneKind::Fang { side } => {
                let open = 0.2 + 0.6 * bite.max(taunt) + 0.3 * spit + (tt * 6.0).sin().max(0.0) * 0.08;
                tf.rotation = Quat::from_rotation_z(-side * open * 0.6);
            }
        }
    }
    // The thread: from the body up to its anchor (gone once dead or loose on the floor).
    for (th, mut tf, mut vis) in &mut threads {
        let Ok((s, stf)) = spiders.get(th.owner) else { continue };
        let shown = !matches!(s.state, State::Dead { .. } | State::Ground(_) | State::Bite { .. } | State::Drop);
        *vis = if shown { Visibility::Inherited } else { Visibility::Hidden };
        let to = s.anchor - stf.translation.truncate();
        tf.translation = Vec3::new(0.0, 4.0 * SCALE, tf.translation.z);
        tf.rotation = Quat::from_rotation_z(to.to_angle() - PI / 2.0);
        tf.scale = Vec3::new(1.0, (to.length() - 4.0 * SCALE).max(0.001), 1.0);
    }
}

pub fn temp_scan(terrain: Res<Terrain>) { // TEMP-AUTOTEST
    eprintln!("SPOT origin canvas {:?}", terrain.canvas_origin);
}
