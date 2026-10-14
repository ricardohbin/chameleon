//! The chameleon: a procedural, fully code-drawn character rig.
//!
//! Everything is built from shaded meshes (see `shapes.rs`) arranged in a
//! bone-like hierarchy, so it can be animated procedurally:
//! walk cycle, curling tail, darting eye, claw slash and camouflage.
//!
//! Local coordinates: the chameleon faces +X, origin = player collider center.

use std::f32::consts::PI;

use bevy::{prelude::*, sprite_render::AlphaMode2d};

use crate::{
    camera,
    gore::PaintTarget,
    player::{self, Player},
    shapes::*,
};

/// Ash/bone white. Colors are earned back through the game.
const DEFAULT_SKIN: Color = Color::srgb(0.86, 0.85, 0.83);
const MOUTH: Color = Color::srgb(0.05, 0.04, 0.04);
const BONE: Color = Color::srgb(0.95, 0.93, 0.88);
const EYE_GLOW: Color = Color::srgb(1.0, 0.96, 0.85);
const WAVE: Color = Color::srgb(0.95, 0.94, 0.90);
const SLASH_TIME: f32 = 0.34;
/// Part of the slash (as a fraction of `SLASH_TIME`) where the claws cut.
const STRIKE: (f32, f32) = (0.3, 0.52);
/// Slash wave, in rig units: centered on the swiping shoulder.
const WAVE_CENTER: Vec2 = Vec2::new(24.0, -2.0);
const WAVE_RADIUS: f32 = 52.0;
/// The slash hitbox in world units (x is mirrored by facing): matches the wave.
pub const SLASH_CENTER: Vec2 = Vec2::new(WAVE_CENTER.x * player::SCALE, WAVE_CENTER.y * player::SCALE);
pub const SLASH_REACH: f32 = WAVE_RADIUS * player::SCALE * 1.15;

pub struct ChameleonPlugin;

impl Plugin for ChameleonPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<UnlockedColors>().add_message::<ColorEarned>().add_systems(
            Update,
            (update_anim_state, earn_color, pose_parts, animate_tail, camouflage, slash_wave)
                .chain()
                .after(player::move_player),
        );
    }
}

/// Colors the chameleon has earned so far. Camouflage only works with these;
/// it starts empty, so the chameleon stays white until colors are unlocked
/// (e.g. `unlocked.0.push(level::lost_colors::PINK)` when the player reaches a goal).
#[derive(Resource, Default)]
pub struct UnlockedColors(pub Vec<Color>);

/// A beaten boss gives its color: the chameleon's skin turns to it for good.
#[derive(Message)]
pub struct ColorEarned(pub Color);

/// Skin materials, re-tinted together by the camouflage system.
#[derive(Resource)]
struct SkinMaterials {
    base: Handle<ColorMaterial>,
    shadow: Handle<ColorMaterial>,
    light: Handle<ColorMaterial>,
    accent: Handle<ColorMaterial>,
}

/// Animation state, lives on the player entity.
#[derive(Component)]
pub struct ChameleonAnim {
    /// Walk-cycle phase (radians).
    phase: f32,
    /// 0 = standing, 1 = full stride.
    stride: f32,
    /// 0 = grounded pose, 1 = airborne pose.
    air: f32,
    /// Tail curl amount (1 = tight spiral).
    curl: f32,
    look: Vec2,
    look_target: Vec2,
    look_timer: f32,
    /// Seconds since the claw slash started, `None` when not slashing.
    slash: Option<f32>,
    /// Increments on every slash, so each one hits a target only once.
    pub slash_id: u32,
    skin: Oklcha,
    /// Resting skin color (white until a color is earned).
    base_skin: Color,
    /// 0 = alive, 1 = lying dead belly-up.
    death: f32,
    rng: u32,
}

/// Which bone a rig entity is; `pose_parts` drives each one.
#[derive(Component, Clone, Copy)]
enum Part {
    Rig,
    Body,
    Head,
    /// `swipe`: the near front leg, which also slashes.
    Leg { front: bool, offset: f32, joint: Joint, swipe: bool },
    EyeCone,
    Pupil,
}

#[derive(Clone, Copy)]
enum Joint {
    Hip,
    Knee,
    Foot,
}

#[derive(Component)]
struct Tail(Handle<Mesh>);

/// The faint crescent left in the air by the claws.
#[derive(Component)]
struct SlashWave(Handle<ColorMaterial>);

pub fn spawn_chameleon(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<ColorMaterial>,
    player: Entity,
) {
    let [base, shadow, light, accent] = palette(DEFAULT_SKIN.into()).map(|c| materials.add(c));
    let mouth = materials.add(MOUTH);
    let bone = materials.add(BONE);
    let black = materials.add(Color::srgb(0.02, 0.02, 0.03));
    let glow = materials.add(ColorMaterial {
        color: EYE_GLOW.with_alpha(0.22),
        alpha_mode: AlphaMode2d::Blend,
        ..default()
    });
    let iris_mat = materials.add(EYE_GLOW);

    let mut c = Builder { commands, meshes };

    let rig = c.pivot(Vec3::ZERO, player, Part::Rig);
    c.commands.entity(rig).insert(Transform::from_scale(Vec3::new(player::SCALE, player::SCALE, 1.0)));

    // --- Body: lean, hunched, armored back ---
    let body_outline = smooth_closed(
        &[
            Vec2::new(40.0, 5.0),
            Vec2::new(30.0, 17.0),
            Vec2::new(12.0, 25.0),
            Vec2::new(-8.0, 24.0),
            Vec2::new(-26.0, 17.0),
            Vec2::new(-38.0, 7.0),
            Vec2::new(-40.0, -3.0),
            Vec2::new(-28.0, -12.0),
            Vec2::new(-8.0, -15.0),
            Vec2::new(14.0, -14.0),
            Vec2::new(31.0, -9.0),
            Vec2::new(41.0, -2.0),
        ],
        8,
    );
    let body_mesh = fill(&body_outline, |p| 0.55 + 0.45 * ((p.y + 15.0) / 40.0).clamp(0.0, 1.0));
    let body = c.part(body_mesh, &base, Vec3::new(0.0, 2.0, 0.0), rig);
    c.tag(body, Part::Body);
    c.commands.entity(body).insert(PaintTarget { center: Vec2::new(0.0, 5.0), radii: Vec2::new(36.0, 16.0), claw: false });

    // Back-swept saw-tooth crest, longest over the middle of the back.
    let center = Vec2::new(0.0, 4.0);
    let spikes: Vec<[Vec2; 3]> = body_outline
        .iter()
        .enumerate()
        .filter(|(i, p)| i % 2 == 0 && p.y > 12.0 && p.x > -32.0 && p.x < 34.0)
        .map(|(_, &p)| {
            let n = (p - center).normalize();
            let t = n.perp();
            let u = ((p.x + 32.0) / 66.0).clamp(0.0, 1.0);
            let len = 4.0 + 7.0 * (PI * u).sin();
            [p - t * 2.6 - n * 2.0, p + t * 2.6 - n * 2.0, p + n * len + Vec2::new(-0.5 * len, 0.0)]
        })
        .collect();
    c.part(triangles(&spikes), &accent, Vec3::new(0.0, 0.0, -0.05), body);

    // Dark slashing stripes across the flank.
    let top_at = |x: f32| {
        body_outline.iter().filter(|p| (p.x - x).abs() < 3.5 && p.y > 0.0).map(|p| p.y).fold(0.0, f32::max)
    };
    let stripes: Vec<[Vec2; 3]> = [-26.0, -14.0, -2.0, 10.0, 22.0]
        .into_iter()
        .map(|x| {
            let ty = top_at(x) - 1.0;
            [Vec2::new(x - 3.5, ty), Vec2::new(x + 4.0, ty), Vec2::new(x - 7.0, ty - 19.0)]
        })
        .collect();
    c.part(triangles(&stripes), &accent, Vec3::new(0.0, 0.0, 0.1), body);

    // Belly line and war scars.
    let stripe = smooth_open(
        &[Vec2::new(-35.0, -5.0), Vec2::new(-18.0, -9.0), Vec2::new(0.0, -10.0), Vec2::new(20.0, -8.0), Vec2::new(38.0, -3.0)],
        6,
    );
    c.part(stroke(&stripe, |t| 1.0 + 2.2 * (PI * t).sin(), 1.0, 0.85, |_| 1.0), &light, Vec3::new(0.0, 0.0, 0.1), body);
    c.scar(&[Vec2::new(4.0, 18.0), Vec2::new(10.0, 11.0), Vec2::new(7.0, 6.0), Vec2::new(14.0, 0.0)], &shadow, &light, body);
    c.scar(&[Vec2::new(-20.0, 12.0), Vec2::new(-15.0, 5.0), Vec2::new(-18.0, 0.0)], &shadow, &light, body);

    // --- Tail (mesh regenerated every frame by `animate_tail`) ---
    let tail_handle = c.meshes.add(tail_mesh(1.0));
    let tail = c.part_handle(tail_handle.clone(), &base, Vec3::new(-34.0, 3.0, -0.3), body);
    c.commands.entity(tail).insert(Tail(tail_handle));

    // --- Legs: far side (behind body, in shadow) then near side ---
    c.leg(Vec3::new(28.0, -4.0, -0.5), true, PI, false, &shadow, body);
    c.leg(Vec3::new(-20.0, -4.0, -0.5), false, 0.0, false, &shadow, body);
    c.leg(Vec3::new(24.0, -4.0, 0.6), true, 0.0, true, &base, body);
    c.leg(Vec3::new(-24.0, -4.0, 0.6), false, PI, false, &base, body);

    // --- Head: tall blade-like casque, heavy brow, fanged jaw ---
    let head = c.pivot(Vec3::new(38.0, 4.0, 0.2), body, Part::Head);
    c.commands.entity(head).insert(PaintTarget { center: Vec2::new(12.0, 3.0), radii: Vec2::new(18.0, 10.0), claw: false });
    let head_outline = smooth_closed(
        &[
            Vec2::new(-8.0, -2.0),
            Vec2::new(-7.0, 14.0),
            Vec2::new(-5.0, 28.0),
            Vec2::new(5.0, 21.0),
            Vec2::new(17.0, 12.0),
            Vec2::new(27.0, 5.0),
            Vec2::new(34.0, 0.0),
            Vec2::new(32.0, -4.0),
            Vec2::new(24.0, -9.0),
            Vec2::new(10.0, -12.0),
            Vec2::new(-2.0, -11.0),
        ],
        8,
    );
    let head_mesh = fill(&head_outline, |p| 0.6 + 0.4 * ((p.y + 12.0) / 38.0).clamp(0.0, 1.0));
    c.part(head_mesh, &base, Vec3::ZERO, head);

    // Casque spikes and ridge.
    let casque_spikes: Vec<[Vec2; 3]> = head_outline
        .iter()
        .enumerate()
        .filter(|(i, p)| i % 3 == 0 && p.y > 15.0 && p.x < 6.0)
        .map(|(_, &p)| {
            let n = (p - Vec2::new(4.0, 6.0)).normalize();
            let t = n.perp();
            [p - t * 2.0 - n * 1.5, p + t * 2.0 - n * 1.5, p + n * 5.0 + Vec2::new(-3.0, 0.0)]
        })
        .collect();
    c.part(triangles(&casque_spikes), &accent, Vec3::new(0.0, 0.0, -0.05), head);
    let ridge = smooth_open(&[Vec2::new(-6.0, 15.0), Vec2::new(-5.0, 27.0), Vec2::new(5.0, 20.0), Vec2::new(14.0, 13.0)], 8);
    c.part(stroke(&ridge, |t| 3.0 - 1.8 * t, 1.0, 1.0, |_| 1.0), &accent, Vec3::new(0.0, 0.0, 0.05), head);
    c.scar(&[Vec2::new(-4.0, 21.0), Vec2::new(1.0, 15.0), Vec2::new(-1.0, 10.0)], &shadow, &light, head);

    // Jaw: dark mouth line ending in a sneer, with fangs.
    let mouth_line = smooth_open(
        &[Vec2::new(33.5, -2.5), Vec2::new(25.0, -4.8), Vec2::new(16.0, -5.8), Vec2::new(9.0, -4.8), Vec2::new(4.5, -1.5)],
        6,
    );
    let teeth: Vec<[Vec2; 3]> = [12.5, 15.5, 18.5, 21.5, 24.5, 27.5, 30.5]
        .into_iter()
        .map(|x| {
            let y = mouth_line.iter().min_by(|a, b| (a.x - x).abs().total_cmp(&(b.x - x).abs())).unwrap().y;
            let len = if x == 27.5 { 4.2 } else { 2.4 };
            [Vec2::new(x - 1.1, y), Vec2::new(x + 1.1, y), Vec2::new(x + 0.3, y - len)]
        })
        .collect();
    c.part(triangles(&teeth), &bone, Vec3::new(0.0, 0.0, 0.055), head);
    c.part(stroke(&mouth_line, |t| 1.9 - 0.7 * t, 1.0, 1.0, |_| 1.0), &mouth, Vec3::new(0.0, 0.0, 0.06), head);
    c.part(fill(&ellipse_pts(1.1, 0.8, 10), |_| 1.0), &mouth, Vec3::new(29.0, 2.0, 0.06), head);

    // Eye: sunken turret, glowing iris, vertical slit pupil, angry lid.
    let eye = c.empty(Vec3::new(11.0, 4.0, 0.1), head);
    let turret = fill(&ellipse_pts(8.0, 8.0, 40), |p| 0.9 - 0.4 * (p.length() / 8.0));
    c.part(turret, &shadow, Vec3::ZERO, eye);
    c.part(Circle::new(7.0).into(), &glow, Vec3::new(0.0, 0.0, 0.005), eye);
    let cone = c.pivot(Vec3::new(0.0, 0.0, 0.01), eye, Part::EyeCone);
    let iris = fill(&ellipse_pts(4.2, 4.2, 28), |p| 1.0 - 0.3 * (p.length() / 4.2));
    c.part(iris, &iris_mat, Vec3::ZERO, cone);
    let pupil = c.pivot(Vec3::new(0.0, 0.0, 0.02), cone, Part::Pupil);
    c.part(fill(&ellipse_pts(0.85, 3.5, 16), |_| 1.0), &black, Vec3::ZERO, pupil);
    // The lid: a half-disc of skin slanted down towards the snout.
    let lid: Vec<Vec2> = (0..=16)
        .map(|i| {
            let a = i as f32 / 16.0 * PI;
            Vec2::new(a.cos(), a.sin()) * 8.6
        })
        .collect();
    let lid_tf = Transform::from_xyz(0.0, 1.6, 0.04).with_rotation(Quat::from_rotation_z(-0.35));
    let lid_e = c.part(fill(&lid, |p| 0.65 + 0.35 * (p.y / 8.6)), &base, Vec3::ZERO, eye);
    c.commands.entity(lid_e).insert(lid_tf);
    let brow = smooth_open(&[Vec2::new(-0.5, 10.5), Vec2::new(10.0, 7.8), Vec2::new(20.5, 3.8)], 6);
    c.part(stroke(&brow, |t| 3.8 - 2.3 * t, 1.0, 1.0, |_| 1.0), &accent, Vec3::new(0.0, 0.0, 0.15), head);

    // Slash wave: a thin crescent around the swiping shoulder, hidden until a slash.
    let wave_mat = materials.add(ColorMaterial {
        color: WAVE.with_alpha(0.0),
        alpha_mode: AlphaMode2d::Blend,
        ..default()
    });
    let arc: Vec<Vec2> = (0..=32).map(|i| Vec2::from_angle(lerp(1.35, -1.45, i as f32 / 32.0)) * WAVE_RADIUS).collect();
    let wave_mesh = stroke(&arc, |t| 0.4 + 7.0 * (PI * t).sin().powf(0.7) * (0.35 + 0.65 * t), 1.0, 0.55, |_| 1.0);
    let wave = c.part(wave_mesh, &wave_mat, WAVE_CENTER.extend(0.9), rig);
    c.commands.entity(wave).insert((SlashWave(wave_mat), Visibility::Hidden));

    c.commands.insert_resource(SkinMaterials { base, shadow, light, accent });
    c.commands.entity(player).insert(ChameleonAnim {
        phase: 0.0,
        stride: 0.0,
        air: 0.0,
        curl: 1.0,
        look: Vec2::X,
        look_target: Vec2::X,
        look_timer: 1.0,
        slash: None,
        slash_id: 0,
        skin: DEFAULT_SKIN.into(),
        base_skin: DEFAULT_SKIN,
        death: 0.0,
        rng: 0x9E37_79B9,
    });
}

/// Small helper to keep the rig-building code readable.
struct Builder<'a, 'w, 's> {
    commands: &'a mut Commands<'w, 's>,
    meshes: &'a mut Assets<Mesh>,
}

impl Builder<'_, '_, '_> {
    fn part(&mut self, mesh: Mesh, mat: &Handle<ColorMaterial>, pos: Vec3, parent: Entity) -> Entity {
        let mesh = self.meshes.add(mesh);
        self.part_handle(mesh, mat, pos, parent)
    }

    fn part_handle(&mut self, mesh: Handle<Mesh>, mat: &Handle<ColorMaterial>, pos: Vec3, parent: Entity) -> Entity {
        self.commands
            .spawn((Mesh2d(mesh), MeshMaterial2d(mat.clone()), Transform::from_translation(pos), ChildOf(parent)))
            .id()
    }

    fn empty(&mut self, pos: Vec3, parent: Entity) -> Entity {
        self.commands
            .spawn((Transform::from_translation(pos), Visibility::default(), ChildOf(parent)))
            .id()
    }

    fn pivot(&mut self, pos: Vec3, parent: Entity, part: Part) -> Entity {
        let e = self.empty(pos, parent);
        self.tag(e, part);
        e
    }

    fn tag(&mut self, e: Entity, part: Part) {
        self.commands.entity(e).insert(part);
    }

    /// A healed wound: dark line with a pale edge.
    fn scar(&mut self, pts: &[Vec2], dark: &Handle<ColorMaterial>, pale: &Handle<ColorMaterial>, parent: Entity) {
        let path = smooth_open(pts, 4);
        self.part(stroke(&path, |t| 2.6 * (PI * t).sin() + 0.6, 1.0, 1.0, |_| 1.0), pale, Vec3::new(0.0, 0.0, 0.12), parent);
        self.part(stroke(&path, |t| 1.2 * (PI * t).sin() + 0.3, 1.0, 1.0, |_| 1.0), dark, Vec3::new(0.0, 0.0, 0.13), parent);
    }

    /// Two-segment leg with a zygodactyl (tong-like) foot.
    fn leg(&mut self, joint: Vec3, front: bool, offset: f32, swipe: bool, mat: &Handle<ColorMaterial>, parent: Entity) {
        let part = |joint| Part::Leg { front, offset, joint, swipe };
        let hip = self.pivot(joint, parent, part(Joint::Hip));
        self.part(Capsule2d::new(3.1, 12.0).into(), mat, Vec3::new(0.0, -6.0, 0.0), hip);
        let knee = self.pivot(Vec3::new(0.0, -12.0, 0.01), hip, part(Joint::Knee));
        if swipe {
            // The killing arm gets soaked first.
            let claw = |center, radii| PaintTarget { center, radii, claw: true };
            self.commands.entity(hip).insert(claw(Vec2::new(0.0, -6.0), Vec2::new(3.2, 8.0)));
            self.commands.entity(knee).insert(claw(Vec2::new(0.0, -6.0), Vec2::new(2.6, 7.5)));
        }
        self.part(Capsule2d::new(2.3, 11.0).into(), mat, Vec3::new(0.0, -5.5, 0.0), knee);
        let foot = self.pivot(Vec3::new(0.0, -11.0, 0.01), knee, part(Joint::Foot));
        for side in [1.0, -1.0] {
            let toe = self.part(Capsule2d::new(1.4, 6.0).into(), mat, Vec3::new(2.6 * side, -1.6, 0.0), foot);
            self.commands
                .entity(toe)
                .insert(Transform::from_xyz(2.6 * side, -1.6, 0.0).with_rotation(Quat::from_rotation_z(1.25 * side)));
        }
    }
}

/// The spiral tail. `curl` = 1 → tight coil, 0 → nearly straight.
fn tail_mesh(curl: f32) -> Mesh {
    const N: usize = 56;
    const LEN: f32 = 100.0;
    let ds = LEN / N as f32;
    let mut pts = Vec::with_capacity(N + 1);
    let (mut p, mut angle) = (Vec2::ZERO, PI - 0.08);
    for i in 0..=N {
        pts.push(p);
        let s = i as f32 / N as f32;
        // Curvature grows towards the tip → logarithmic-looking spiral.
        angle += curl * (0.008 + 0.26 * s.powf(2.5)) * ds;
        p += Vec2::from_angle(angle) * ds;
    }
    // Left edge (perp) is the inner/belly side; add soft bands along the tail.
    stroke(&pts, |t| 1.2 + 10.5 * (1.0 - t).powf(0.9), 0.55, 1.0, |t| {
        1.0 - 0.55 * (0.5 + 0.5 * (t * 30.0).sin()).powi(8)
    })
}

pub fn update_anim_state(
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    player: Single<(&Player, &mut ChameleonAnim)>,
) {
    let dt = time.delta_secs();
    let (p, mut a) = player.into_inner();
    let speed = p.vel.x.abs();

    a.stride = smooth(a.stride, (speed / 260.0).min(1.0), 8.0, dt);
    a.air = smooth(a.air, if p.grounded { 0.0 } else { 1.0 }, 10.0, dt);
    a.phase += dt * speed * 0.075;

    let dead = p.dead.is_some();
    a.death = if dead { smooth(a.death, 1.0, 6.0, dt) } else { 0.0 };
    let curl_target = if dead { 1.0 } else if !p.grounded { 0.25 } else { 1.0 - 0.55 * a.stride };
    a.curl = smooth(a.curl, curl_target, 4.0, dt);

    // Claw slash.
    let attack = keys.any_just_pressed([KeyCode::KeyF, KeyCode::KeyJ]) || mouse.just_pressed(MouseButton::Left);
    if attack && a.slash.is_none() && p.stun <= 0.0 && !dead {
        a.slash = Some(0.0);
        a.slash_id += 1;
    }
    a.slash = a.slash.map(|t| t + dt).filter(|&t| t < SLASH_TIME);

    // Eye: look ahead while moving, dart around randomly while idle.
    a.look_timer -= dt;
    if a.slash.is_some() || speed > 20.0 {
        a.look_target = Vec2::new(1.0, (p.vel.y * 0.0006).clamp(-0.6, 0.6)).normalize();
        a.look_timer = 0.8;
    } else if a.look_timer <= 0.0 {
        let angle = a.rand() * PI * 2.0;
        a.look_target = Vec2::from_angle(angle) * (0.4 + 0.6 * a.rand());
        a.look_timer = 0.5 + 2.0 * a.rand();
    }
    // Chameleon eyes snap quickly between targets.
    let target = a.look_target;
    a.look = a.look.lerp(target, 1.0 - (-dt * 18.0).exp());
}

fn pose_parts(
    time: Res<Time>,
    player: Single<(&Player, &ChameleonAnim)>,
    mut parts: Query<(&Part, &mut Transform, &mut Visibility)>,
) {
    let (p, a) = *player;
    let t = time.elapsed_secs();
    let dt = time.delta_secs();
    let bob = -(a.phase.sin().abs()) * 1.8 * a.stride;
    let slash = a.slash.map(|t| slash_pose(t / SLASH_TIME));
    let lunge = slash.map_or(0.0, |s| s.3);

    for (part, mut tf, mut vis) in &mut parts {
        match *part {
            Part::Rig => {
                // Flip by scaling X through zero: reads as a quick turn.
                tf.scale.x = smooth(tf.scale.x, p.facing * player::SCALE, 16.0, dt);
                let tilt = (p.vel.y * 0.00035).clamp(-0.25, 0.25) * a.air;
                // Dead: rolls over belly-up.
                tf.rotation = Quat::from_rotation_z(lerp(tilt, PI, a.death) * p.facing);
                tf.translation.y = a.death * 6.0;
                // Blink while invulnerable.
                let blink = p.invuln > 0.0 && p.dead.is_none() && (t * 30.0).sin() > 0.0;
                *vis = if blink { Visibility::Hidden } else { Visibility::Inherited };
            }
            Part::Body => {
                tf.translation.x = lunge * 4.0;
                tf.translation.y = 2.0 + bob;
                tf.rotation = Quat::from_rotation_z((a.phase * 2.0).sin() * 0.025 * a.stride - lunge * 0.07);
                tf.scale.y = 1.0 + (t * 2.2).sin() * 0.012; // breathing
            }
            Part::Head => {
                let nod = (a.phase * 2.0).sin() * 0.05 * a.stride + (t * 1.3).sin() * 0.02;
                tf.rotation = Quat::from_rotation_z(nod + 0.12 * a.air - lunge * 0.08);
            }
            Part::Leg { front, offset, joint, swipe } => {
                let (mut hip, mut knee) = leg_angles(front, offset, a);
                let mut foot = -(hip + knee); // keep the foot flat
                if let (true, Some((w, s_hip, s_knee, _))) = (swipe, slash) {
                    hip = lerp(hip, s_hip, w);
                    knee = lerp(knee, s_knee, w);
                    foot = lerp(foot, 0.5, w); // claws hooked forward
                }
                if a.death > 0.0 {
                    // Legs up, twitching less and less.
                    let s = if front { 1.0 } else { -1.0 };
                    let twitch = (t * 25.0 + offset).sin() * 0.3 * (1.0 - a.death).max(0.1);
                    hip = lerp(hip, s * 0.5 + twitch, a.death);
                    knee = lerp(knee, -s * 1.3, a.death);
                }
                tf.rotation = Quat::from_rotation_z(match joint {
                    Joint::Hip => hip,
                    Joint::Knee => knee,
                    Joint::Foot => foot,
                });
            }
            Part::EyeCone => tf.translation = (a.look * 2.6).extend(tf.translation.z),
            Part::Pupil => tf.translation = (a.look * 1.0).extend(tf.translation.z),
        }
    }
}

/// Swipe pose at slash progress `u` (0..1): (blend weight, hip, knee, body lunge).
fn slash_pose(u: f32) -> (f32, f32, f32, f32) {
    let ease = |x: f32| x * x * (3.0 - 2.0 * x);
    let w = ease((u / 0.12).min(1.0)) * (1.0 - ease(((u - 0.6) / 0.4).clamp(0.0, 1.0)));
    let (hip, knee) = if u < STRIKE.0 {
        // Wind up: claw raised high above the head.
        let k = ease(u / STRIKE.0);
        (lerp(0.4, 2.6, k), lerp(-0.6, -1.5, k))
    } else {
        // Rip down through the target, then hold the follow-through.
        let k = ((u - STRIKE.0) / (STRIKE.1 - STRIKE.0)).min(1.0);
        let k = 1.0 - (1.0 - k).powi(3);
        (lerp(2.6, 0.05, k), lerp(-1.5, -0.1, k))
    };
    let lunge = (PI * ((u - 0.15) / 0.6).clamp(0.0, 1.0)).sin();
    (w, hip, knee, lunge)
}

fn leg_angles(front: bool, offset: f32, a: &ChameleonAnim) -> (f32, f32) {
    let s = if front { 1.0 } else { -1.0 };
    let ph = a.phase + offset;
    let swing = ph.sin() * 0.7 * a.stride;
    let lift = ph.cos().max(0.0) * 0.8 * a.stride;
    let hip = lerp(s * 0.25 + swing, s * 0.8, a.air);
    let knee = lerp(-s * (0.55 + lift), -s * 1.1, a.air);
    (hip, knee)
}

fn animate_tail(
    player: Single<&ChameleonAnim>,
    tail: Single<&Tail>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    if let Some(mut mesh) = meshes.get_mut(&tail.0) {
        *mesh = tail_mesh(player.curl);
    }
}

fn slash_wave(
    player: Single<&ChameleonAnim>,
    wave: Single<(&SlashWave, &mut Transform, &mut Visibility)>,
    mut materials: ResMut<Assets<ColorMaterial>>,
) {
    let (wave, mut tf, mut vis) = wave.into_inner();
    let k = player.slash.map_or(-1.0, |t| (t / SLASH_TIME - STRIKE.0) / 0.45);
    if !(0.0..1.0).contains(&k) {
        *vis = Visibility::Hidden;
        return;
    }
    // Barely there: a quick ripple that widens and fades.
    *vis = Visibility::Inherited;
    tf.scale = Vec3::splat(0.9 + 0.25 * k);
    if let Some(mut mat) = materials.get_mut(&wave.0) {
        mat.color.set_alpha(0.55 * (1.0 - k).powf(1.3));
    }
}

/// Slowly blend the skin towards the color of the surface we stand on.
fn camouflage(
    time: Res<Time>,
    player: Single<(&Player, &mut ChameleonAnim)>,
    skin: Res<SkinMaterials>,
    unlocked: Res<UnlockedColors>,
    mut materials: ResMut<Assets<ColorMaterial>>,
) {
    let (p, mut a) = player.into_inner();
    // Blend in Oklab: hue interpolation from white (no hue) looks odd in Oklch.
    let base: Oklaba = a.base_skin.into();
    let target = match p.ground_color {
        Some(c) if p.grounded && unlocked.0.contains(&c) => base.mix(&c.into(), 0.8),
        _ => base,
    };
    let current: Oklaba = a.skin.into();
    a.skin = current.mix(&target, 1.0 - (-time.delta_secs() * 1.8).exp()).into();
    let colors = palette(a.skin);
    for (handle, color) in [&skin.base, &skin.shadow, &skin.light, &skin.accent].into_iter().zip(colors) {
        if let Some(mut mat) = materials.get_mut(handle) {
            mat.color = color;
        }
    }
}

/// Takes on an earned color. The world camera is grayscale, so the whole
/// chameleon moves to the color layer to actually show it.
fn earn_color(
    mut commands: Commands,
    mut earned: MessageReader<ColorEarned>,
    mut unlocked: ResMut<UnlockedColors>,
    player: Single<(Entity, &mut ChameleonAnim)>,
    children: Query<&Children>,
    meshes: Query<(), With<Mesh2d>>,
) {
    let (entity, mut a) = player.into_inner();
    for ColorEarned(color) in earned.read() {
        a.base_skin = *color;
        if !unlocked.0.contains(color) {
            unlocked.0.push(*color);
        }
        for e in children.iter_descendants(entity) {
            if meshes.contains(e) {
                commands.entity(e).insert(camera::color_layer());
            }
        }
    }
}

/// Derives the 4 skin tones from a single base color.
fn palette(c: Oklcha) -> [Color; 4] {
    let shadow = Oklcha { lightness: c.lightness * 0.55, ..c };
    let light = Oklcha { lightness: (c.lightness + 0.1).min(0.98), chroma: c.chroma * 0.5, ..c };
    let accent = Oklcha { lightness: c.lightness * 0.3, chroma: c.chroma * 1.2, hue: (c.hue + 15.0) % 360.0, ..c };
    [c.into(), shadow.into(), light.into(), accent.into()]
}

impl ChameleonAnim {
    /// True while the claws are cutting through the air.
    pub fn striking(&self) -> bool {
        self.slash.is_some_and(|t| (STRIKE.0..=STRIKE.1).contains(&(t / SLASH_TIME)))
    }

    /// Tiny xorshift RNG in 0..1, good enough for eye twitches.
    fn rand(&mut self) -> f32 {
        self.rng ^= self.rng << 13;
        self.rng ^= self.rng >> 17;
        self.rng ^= self.rng << 5;
        (self.rng >> 8) as f32 / (1u32 << 24) as f32
    }
}

/// Frame-rate independent exponential smoothing.
fn smooth(current: f32, target: f32, rate: f32, dt: f32) -> f32 {
    lerp(current, target, 1.0 - (-rate * dt).exp())
}

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}
