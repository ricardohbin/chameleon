//! The apocalyptic backdrop: ruined skyline, smoke plumes over fires,
//! falling ash, rising embers, fog and lightning.

use std::f32::consts::PI;

use bevy::{prelude::*, sprite_render::AlphaMode2d};

use crate::{camera::Shake, shapes::*};

const SKY_TOP: Color = Color::srgb(0.07, 0.07, 0.08);
const SKY_HORIZON: Color = Color::srgb(0.52, 0.36, 0.27);
const SMOKE: Color = Color::srgb(0.10, 0.09, 0.09);
const FIRE: Color = Color::srgb(1.0, 0.62, 0.25);
const ASH_COUNT: usize = 160;
const EMBER_COUNT: usize = 50;
/// Particles wrap around this box centered on the camera.
const WRAP: Vec2 = Vec2::new(560.0, 340.0);

pub struct AtmospherePlugin;

impl Plugin for AtmospherePlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(ClearColor(SKY_HORIZON))
            .insert_resource(Lightning { next: 3.0, since: 99.0, rng: Rng(0xF1A5) })
            .add_systems(Startup, (spawn_backdrop, spawn_particles))
            .add_systems(Update, (drift_particles, animate_smoke, animate_fire, lightning))
            .add_systems(PostUpdate, parallax.before(TransformSystems::Propagate));
    }
}

/// Position = origin + camera * factor. factor 0 = part of the world,
/// 1 = glued to the screen, in between = distant layers, negative = foreground.
#[derive(Component)]
struct Parallax {
    origin: Vec2,
    factor: Vec2,
}

#[derive(Component)]
struct Particle {
    vel: Vec2,
    spin: f32,
    wobble: f32,
}

/// One puff in a smoke plume (each has its own material to fade).
#[derive(Component)]
struct Puff {
    source: Vec2,
    age: f32,
    life: f32,
    seed: f32,
}

#[derive(Component)]
struct Fire {
    seed: f32,
    base_alpha: f32,
}

#[derive(Component)]
struct Flash;

#[derive(Component)]
struct Bolt;

#[derive(Resource)]
struct Lightning {
    next: f32,
    since: f32,
    rng: Rng,
}

fn blend(materials: &mut Assets<ColorMaterial>, color: Color) -> Handle<ColorMaterial> {
    materials.add(ColorMaterial { color, alpha_mode: AlphaMode2d::Blend, ..default() })
}

fn spawn_backdrop(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
) {
    let white = materials.add(Color::WHITE);
    let white_blend = blend(&mut materials, Color::WHITE);
    let mut rng = Rng(0xA5A5);

    // Sky: dark smoke-choked top, burning horizon.
    let sky = gradient_rect(Vec2::new(4000.0, 720.0), SKY_TOP, SKY_HORIZON);
    layer(&mut commands, &mut meshes, sky, &white, Vec2::new(0.0, 60.0), -100.0, Vec2::ONE);

    // A pale, sick sun behind the haze.
    for (r, a) in [(120.0, 0.06), (80.0, 0.12), (46.0, 0.85)] {
        let mat = blend(&mut materials, Color::srgba(0.95, 0.88, 0.80, a));
        layer(&mut commands, &mut meshes, Circle::new(r).into(), &mat, Vec2::new(-220.0, 150.0), -99.0, Vec2::splat(0.97));
    }

    // Ruined skylines, far to near.
    let skylines = [
        (Color::srgb(0.36, 0.28, 0.24), Vec2::splat(0.9), -95.0, -130.0, (60.0, 140.0), (120.0, 300.0), false),
        (Color::srgb(0.22, 0.18, 0.17), Vec2::new(0.75, 0.8), -90.0, -210.0, (70.0, 160.0), (160.0, 380.0), true),
        (Color::srgb(0.12, 0.11, 0.11), Vec2::new(0.5, 0.6), -70.0, -330.0, (90.0, 200.0), (220.0, 480.0), true),
    ];
    for (color, factor, z, base_y, (wmin, wmax), (hmin, hmax), windows) in skylines {
        let mat = materials.add(color);
        let hole = materials.add(scale_color(SKY_HORIZON, 0.75));
        let mut x = -2200.0;
        let extent = 7500.0 * (1.0 - factor.x) + 1500.0;
        while x < extent {
            let w = rng.range(wmin, wmax);
            let h = rng.range(hmin, hmax);
            let origin = Vec2::new(x, base_y);
            let tilt = if rng.chance(0.2) { rng.range(-0.12, 0.12) } else { 0.0 };
            let id = layer(&mut commands, &mut meshes, fill(&ruin_outline(&mut rng, w, h), |p| 0.8 + 0.2 * (p.y / h)), &mat, origin, z, factor);
            commands.entity(id).insert(Transform::from_translation(origin.extend(z)).with_rotation(Quat::from_rotation_z(tilt)));
            if windows {
                // Blown-out windows showing the burning sky behind.
                let cols = (w / 22.0) as i32;
                let rows = (h * 0.55 / 26.0) as i32;
                for c in 0..cols {
                    for r in 0..rows {
                        if rng.chance(0.45) {
                            let p = Vec2::new(-w / 2.0 + 14.0 + c as f32 * 22.0, 20.0 + r as f32 * 26.0);
                            let win = Rectangle::new(rng.range(6.0, 10.0), rng.range(8.0, 14.0));
                            let mesh = meshes.add(win);
                            commands.spawn((Mesh2d(mesh), MeshMaterial2d(hole.clone()), Transform::from_translation(p.extend(0.01)), ChildOf(id)));
                        }
                    }
                }
            }
            // Twisted beams poking out of the broken tops.
            for _ in 0..rng.below(3) {
                let bx = rng.range(-w / 2.0 + 5.0, w / 2.0 - 5.0);
                let beam = smooth_open(&[Vec2::new(bx, h * 0.6), Vec2::new(bx + rng.range(-15.0, 15.0), h * 0.85), Vec2::new(bx + rng.range(-35.0, 35.0), h + rng.range(0.0, 30.0))], 5);
                let mesh = meshes.add(stroke(&beam, |_| 3.0, 1.0, 1.0, |_| 1.0));
                commands.spawn((Mesh2d(mesh), MeshMaterial2d(mat.clone()), Transform::from_xyz(0.0, 0.0, -0.01), ChildOf(id)));
            }
            x += w + rng.range(-20.0, 80.0);
        }
    }

    // Toppled power pylons with sagging cables, and broken cranes.
    let mid = materials.add(Color::srgb(0.17, 0.15, 0.14));
    let factor = Vec2::new(0.62, 0.7);
    let mut x = -1500.0;
    let mut prev_top: Option<Vec2> = None;
    while x < 4500.0 {
        let h = rng.range(170.0, 240.0);
        let lean = rng.range(-0.15, 0.15);
        let base = Vec2::new(x, -260.0);
        let top = base + Vec2::new(lean * h, h);
        let pylon = [base + Vec2::new(-14.0, 0.0), base + Vec2::new(14.0, 0.0), top + Vec2::new(2.0, 0.0), top + Vec2::new(-2.0, 0.0)];
        layer(&mut commands, &mut meshes, fill(&pylon, |_| 1.0), &mid, Vec2::ZERO, -80.0, factor);
        let arm = [top + Vec2::new(-26.0, -4.0), top + Vec2::new(26.0, -4.0 + lean * 30.0), top + Vec2::new(26.0, -1.0 + lean * 30.0), top + Vec2::new(-26.0, -1.0)];
        layer(&mut commands, &mut meshes, fill(&arm, |_| 1.0), &mid, Vec2::ZERO, -80.0, factor);
        if let Some(prev) = prev_top {
            for dy in [0.0, -4.0] {
                let a = prev + Vec2::new(20.0, dy - 3.0);
                let b = top + Vec2::new(-20.0, dy - 3.0);
                let sag = smooth_open(&[a, (a + b) / 2.0 - Vec2::Y * rng.range(30.0, 70.0), b], 10);
                layer(&mut commands, &mut meshes, stroke(&sag, |_| 1.2, 1.0, 1.0, |_| 1.0), &mid, Vec2::ZERO, -80.0, factor);
            }
        }
        // Some cables snapped and dangle.
        prev_top = if rng.chance(0.7) { Some(top) } else { None };
        if rng.chance(0.25) {
            let mast = Vec2::new(x + 120.0, -260.0);
            let mh = rng.range(260.0, 340.0);
            let tip = mast + Vec2::Y * mh;
            layer(&mut commands, &mut meshes, Rectangle::new(8.0, mh).into(), &mid, mast + Vec2::Y * mh / 2.0, -81.0, factor);
            let jib_end = tip + Vec2::new(rng.range(120.0, 180.0), rng.range(-60.0, -10.0));
            layer(&mut commands, &mut meshes, stroke(&[tip, jib_end], |_| 6.0, 1.0, 1.0, |_| 1.0), &mid, Vec2::ZERO, -81.0, factor);
            let hook = smooth_open(&[jib_end, jib_end + Vec2::new(4.0, -60.0), jib_end + Vec2::new(-2.0, -110.0)], 6);
            layer(&mut commands, &mut meshes, stroke(&hook, |_| 1.5, 1.0, 1.0, |_| 1.0), &mid, Vec2::ZERO, -81.0, factor);
        }
        x += rng.range(260.0, 380.0);
    }

    // Fog banks between the layers and at the bottom of the screen.
    for (y, h, alpha, z, factor) in [(-200.0, 260.0, 0.55, -85.0, Vec2::splat(0.75)), (-330.0, 300.0, 0.6, -60.0, Vec2::new(0.5, 0.6)), (-190.0, 260.0, 0.75, 35.0, Vec2::ONE)] {
        let fog = gradient_rect(Vec2::new(4000.0, h), Color::srgba(0.30, 0.25, 0.22, 0.0), Color::srgba(0.16, 0.14, 0.13, alpha));
        layer(&mut commands, &mut meshes, fog, &white_blend, Vec2::new(0.0, y), z, factor);
    }

    // Foreground wreckage silhouettes, moving faster than the world.
    let fg = materials.add(Color::srgb(0.03, 0.03, 0.035));
    let mut x = -1200.0;
    while x < 10000.0 {
        let w = rng.range(120.0, 260.0);
        let h = rng.range(40.0, 90.0);
        let mound = ruin_outline(&mut rng, w, h);
        layer(&mut commands, &mut meshes, fill(&mound, |_| 1.0), &fg, Vec2::new(x, -300.0), 45.0, Vec2::new(-0.3, 1.0));
        if rng.chance(0.6) {
            let bx = x + rng.range(-w / 3.0, w / 3.0);
            let bar = smooth_open(&[Vec2::new(bx, -240.0), Vec2::new(bx + rng.range(-20.0, 20.0), -170.0), Vec2::new(bx + rng.range(-60.0, 60.0), -140.0)], 6);
            layer(&mut commands, &mut meshes, stroke(&bar, |t| 5.0 - 3.0 * t, 1.0, 1.0, |_| 1.0), &fg, Vec2::ZERO, 45.0, Vec2::new(-0.3, 1.0));
        }
        x += rng.range(500.0, 900.0);
    }

    // Smoke plumes rising from distant fires.
    let factor = Vec2::new(0.4, 0.5);
    let mut x = -900.0;
    while x < 5000.0 {
        let source = Vec2::new(x, -240.0 + rng.range(-30.0, 40.0));
        for (r, a) in [(26.0, 0.25), (12.0, 0.7)] {
            let mat = blend(&mut materials, FIRE.with_alpha(a));
            let id = layer(&mut commands, &mut meshes, fill(&ellipse_pts(r * 1.6, r, 20), |_| 1.0), &mat, source, -55.0, factor);
            commands.entity(id).insert(Fire { seed: rng.range(0.0, 100.0), base_alpha: a });
        }
        let life = rng.range(6.0, 9.0);
        for i in 0..14 {
            let mat = blend(&mut materials, SMOKE.with_alpha(0.0));
            let puff = fill(&smooth_closed(&rock_pts(&mut rng, 1.0, 7), 5), |p| 0.8 + 0.4 * p.y);
            let id = layer(&mut commands, &mut meshes, puff, &mat, source, -54.0 + i as f32 * 0.01, factor);
            commands.entity(id).insert(Puff { source, age: life * i as f32 / 14.0, life, seed: rng.range(0.0, 100.0) });
        }
        x += rng.range(380.0, 700.0);
    }

    // Lightning: a full-screen flash and a bolt in the sky.
    let flash_mat = blend(&mut materials, Color::srgba(0.9, 0.9, 1.0, 0.0));
    let id = layer(&mut commands, &mut meshes, Rectangle::new(4000.0, 3000.0).into(), &flash_mat, Vec2::ZERO, 80.0, Vec2::ONE);
    commands.entity(id).insert(Flash);
    let bolt_mat = materials.add(Color::srgb(0.95, 0.95, 1.0));
    let id = layer(&mut commands, &mut meshes, Rectangle::new(1.0, 1.0).into(), &bolt_mat, Vec2::ZERO, -96.0, Vec2::ONE);
    commands.entity(id).insert((Bolt, Visibility::Hidden));
}

fn layer(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    mesh: Mesh,
    mat: &Handle<ColorMaterial>,
    origin: Vec2,
    z: f32,
    factor: Vec2,
) -> Entity {
    commands
        .spawn((
            Mesh2d(meshes.add(mesh)),
            MeshMaterial2d(mat.clone()),
            Transform::from_translation(origin.extend(z)),
            Parallax { origin, factor },
        ))
        .id()
}

/// A building silhouette with a broken, jagged top. Base at y = 0.
fn ruin_outline(rng: &mut Rng, w: f32, h: f32) -> Vec<Vec2> {
    let mut pts = vec![Vec2::new(-w / 2.0, 0.0), Vec2::new(-w / 2.0, h * rng.range(0.6, 1.0))];
    let mut x = -w / 2.0 + rng.range(4.0, 12.0);
    while x < w / 2.0 - 4.0 {
        let collapse = if rng.chance(0.25) { rng.range(0.1, 0.35) * h } else { 0.0 };
        pts.push(Vec2::new(x, h - rng.range(0.0, 18.0) - collapse));
        x += rng.range(6.0, 18.0);
    }
    pts.push(Vec2::new(w / 2.0, h * rng.range(0.5, 1.0)));
    pts.push(Vec2::new(w / 2.0, 0.0));
    pts
}

fn spawn_particles(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
) {
    let mut rng = Rng(0xA54);
    let ash_mats = [
        blend(&mut materials, Color::srgba(0.12, 0.12, 0.12, 0.8)),
        blend(&mut materials, Color::srgba(0.55, 0.53, 0.50, 0.6)),
        blend(&mut materials, Color::srgba(0.30, 0.29, 0.28, 0.7)),
    ];
    let ember_mat = blend(&mut materials, Color::srgba(1.0, 0.75, 0.40, 0.9));
    let flake = meshes.add(triangles(&[[Vec2::new(-1.0, -0.6), Vec2::new(1.2, -0.3), Vec2::new(-0.2, 0.9)]]));
    let ember = meshes.add(Circle::new(1.0));

    for i in 0..ASH_COUNT + EMBER_COUNT {
        let is_ember = i >= ASH_COUNT;
        let pos = Vec2::new(rng.range(-WRAP.x, WRAP.x), rng.range(-WRAP.y, WRAP.y));
        let z = if rng.chance(0.3) { 30.0 } else { -10.0 };
        let (mesh, mat, vel, size) = if is_ember {
            (ember.clone(), ember_mat.clone(), Vec2::new(rng.range(10.0, 50.0), rng.range(25.0, 70.0)), rng.range(0.8, 1.8))
        } else {
            (flake.clone(), ash_mats[rng.below(3)].clone(), Vec2::new(rng.range(20.0, 70.0), -rng.range(20.0, 60.0)), rng.range(1.5, 3.5))
        };
        commands.spawn((
            Mesh2d(mesh),
            MeshMaterial2d(mat),
            Transform::from_translation(pos.extend(z)).with_scale(Vec3::splat(size)),
            Particle { vel, spin: rng.range(-4.0, 4.0), wobble: rng.range(0.0, 10.0) },
        ));
    }
}

fn drift_particles(
    time: Res<Time>,
    camera: Single<&Transform, (With<Camera2d>, Without<Particle>)>,
    mut particles: Query<(&Particle, &mut Transform)>,
) {
    let dt = time.delta_secs();
    let t = time.elapsed_secs();
    let cam = camera.translation.truncate();
    for (p, mut tf) in &mut particles {
        let sway = Vec2::new((t * 1.3 + p.wobble).sin() * 15.0, (t * 2.1 + p.wobble).cos() * 8.0);
        let mut pos = tf.translation.truncate() + (p.vel + sway) * dt;
        // Wrap around the camera so there's always ash on screen.
        let rel = pos - cam;
        pos = cam + Vec2::new(wrap(rel.x, WRAP.x), wrap(rel.y, WRAP.y));
        tf.translation = pos.extend(tf.translation.z);
        tf.rotate_z(p.spin * dt);
    }
}

fn wrap(v: f32, half: f32) -> f32 {
    (v + half).rem_euclid(half * 2.0) - half
}

fn animate_smoke(
    time: Res<Time>,
    mut puffs: Query<(&mut Puff, &mut Parallax, &mut Transform, &MeshMaterial2d<ColorMaterial>)>,
    mut materials: ResMut<Assets<ColorMaterial>>,
) {
    let dt = time.delta_secs();
    for (mut puff, mut layer, mut tf, mat) in &mut puffs {
        puff.age = (puff.age + dt) % puff.life;
        let k = puff.age / puff.life;
        let rise = Vec2::new(k * 160.0 + (k * 6.0 + puff.seed).sin() * 25.0, k * 380.0);
        layer.origin = puff.source + rise;
        tf.scale = Vec3::splat(18.0 + k * 110.0);
        tf.rotation = Quat::from_rotation_z(puff.seed + k * 1.5);
        if let Some(mut m) = materials.get_mut(&mat.0) {
            m.color = SMOKE.with_alpha((PI * k).sin().powf(0.7) * 0.55);
        }
    }
}

fn animate_fire(
    time: Res<Time>,
    fires: Query<(&Fire, &MeshMaterial2d<ColorMaterial>, &mut Transform)>,
    mut materials: ResMut<Assets<ColorMaterial>>,
) {
    let t = time.elapsed_secs();
    for (fire, mat, mut tf) in fires {
        let flicker = 0.75 + 0.25 * ((t * 9.0 + fire.seed).sin() * (t * 13.7 + fire.seed * 2.0).sin()).abs();
        tf.scale = Vec3::new(1.0, flicker, 1.0);
        if let Some(mut m) = materials.get_mut(&mat.0) {
            m.color = FIRE.with_alpha(fire.base_alpha * flicker);
        }
    }
}

fn lightning(
    time: Res<Time>,
    mut state: ResMut<Lightning>,
    mut shake: ResMut<Shake>,
    flash: Single<&MeshMaterial2d<ColorMaterial>, With<Flash>>,
    bolt: Single<(&Mesh2d, &mut Parallax, &mut Visibility), With<Bolt>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    let dt = time.delta_secs();
    let (bolt_mesh, mut bolt_pos, mut bolt_vis) = bolt.into_inner();
    state.next -= dt;
    state.since += dt;
    if state.next <= 0.0 {
        state.next = state.rng.range(4.0, 11.0);
        state.since = 0.0;
        bolt_pos.origin = Vec2::new(state.rng.range(-380.0, 380.0), 0.0);
        let mesh = bolt_mesh_gen(&mut state.rng);
        if let Some(mut m) = meshes.get_mut(&bolt_mesh.0) {
            *m = mesh;
        }
        shake.add(0.2);
    }
    // Double flicker: a strike, then a weaker echo.
    let s = state.since;
    let intensity = (-s * 14.0).exp().max(if s > 0.12 { 0.7 * (-(s - 0.12) * 12.0).exp() } else { 0.0 });
    if let Some(mut m) = materials.get_mut(&flash.0) {
        m.color = Color::srgba(0.9, 0.9, 1.0, intensity * 0.35);
    }
    *bolt_vis = if intensity > 0.3 { Visibility::Inherited } else { Visibility::Hidden };
}

/// A jagged bolt with a couple of forks, from high in the sky to the horizon.
fn bolt_mesh_gen(rng: &mut Rng) -> Mesh {
    let mut tris = Vec::new();
    let add_branch = |rng: &mut Rng, start: Vec2, steps: usize, width: f32, tris: &mut Vec<[Vec2; 3]>| {
        let mut p = start;
        for i in 0..steps {
            let next = p + Vec2::new(rng.range(-22.0, 22.0), -rng.range(18.0, 30.0));
            let w = width * (1.0 - i as f32 / steps as f32) + 0.6;
            let n = (next - p).normalize().perp() * w;
            tris.push([p + n, p - n, next + n]);
            tris.push([p - n, next - n, next + n]);
            p = next;
        }
        p
    };
    let mut p = Vec2::new(0.0, 330.0);
    for _ in 0..3 {
        p = add_branch(rng, p, 4, 2.2, &mut tris);
        let fork = p;
        add_branch(rng, fork, 3, 1.0, &mut tris);
    }
    triangles(&tris)
}

fn parallax(
    camera: Single<&Transform, With<Camera2d>>,
    mut layers: Query<(&Parallax, &mut Transform), Without<Camera2d>>,
) {
    let cam = camera.translation.truncate();
    for (layer, mut tf) in &mut layers {
        let p = layer.origin + cam * layer.factor;
        tf.translation.x = p.x;
        tf.translation.y = p.y;
    }
}
