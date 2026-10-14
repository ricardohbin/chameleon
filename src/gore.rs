//! Gore: blood drops, stains on the ground and blood painted on the chameleon.
//!
//! Blood is drawn on `camera::COLOR_LAYER`, so it keeps its color in the
//! grayscale world (venom too): the first colors the chameleon gets back are its enemies'.
//!
//! Send a `Bleed` message to spray drops. Sticky drops that touch a
//! `PaintTarget` (a part of the chameleon) become splats on it; drops that
//! land on a platform become stains. Fresh splats drip, then slowly dry.

use bevy::{camera::visibility::RenderLayers, prelude::*};

use crate::{camera, level::Platform, shapes::*};

const GRAVITY: f32 = 1500.0;
const DROP_LIFE: f32 = 4.0;
const MAX_SPLATS: usize = 90;
const MAX_STAINS: usize = 320;
/// Seconds for fresh blood to dry to its dark tone.
const DRY_TIME: f32 = 25.0;
/// Fresh splats drip for this long.
const WET_TIME: f32 = 4.0;
const VARIANTS: usize = 8;

pub struct GorePlugin;

impl Plugin for GorePlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<Bleed>()
            .add_message::<Paint>()
            .add_systems(Startup, setup)
            .add_systems(Update, (spray, paint_claws, move_drops, age_splats, cap_stains).chain());
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Fluid {
    /// Wasp blood.
    Green,
    /// The chameleon's own.
    Red,
    Venom,
}

impl Fluid {
    const ALL: [Fluid; 3] = [Fluid::Green, Fluid::Red, Fluid::Venom];

    fn fresh(self) -> Color {
        match self {
            Fluid::Green => Color::srgb(0.42, 0.95, 0.10),
            Fluid::Red => Color::srgb(0.72, 0.02, 0.03),
            Fluid::Venom => Color::srgb(0.20, 0.56, 0.08),
        }
    }

    fn dry(self) -> Color {
        match self {
            Fluid::Green => Color::srgb(0.13, 0.34, 0.05),
            Fluid::Red => Color::srgb(0.28, 0.02, 0.02),
            Fluid::Venom => Color::srgb(0.09, 0.24, 0.04),
        }
    }

    /// Every fluid keeps its color in the gray world.
    fn layers(self) -> RenderLayers {
        camera::color_layer()
    }

    fn index(self) -> usize {
        self as usize
    }
}

/// Spray `amount` drops from `pos` in a cone around `dir`.
#[derive(Message, Clone, Copy)]
pub struct Bleed {
    pub pos: Vec2,
    pub dir: Vec2,
    /// Half-angle of the cone (radians, PI = all around).
    pub spread: f32,
    pub speed: f32,
    pub amount: u32,
    pub fluid: Fluid,
    /// Drops can stick to the chameleon.
    pub sticky: bool,
}

/// Splash `amount` splats straight onto the chameleon's claws.
#[derive(Message, Clone, Copy)]
pub struct Paint {
    pub amount: u32,
    pub fluid: Fluid,
}

/// A part of the chameleon that blood sticks to: an ellipse in the part's local space.
#[derive(Component)]
pub struct PaintTarget {
    pub center: Vec2,
    pub radii: Vec2,
    /// The slashing arm (receives `Paint`).
    pub claw: bool,
}

#[derive(Component)]
struct Drop {
    vel: Vec2,
    size: f32,
    fluid: Fluid,
    sticky: bool,
    life: f32,
}

#[derive(Component)]
struct Splat {
    age: f32,
    fluid: Fluid,
    mat: Handle<ColorMaterial>,
    seq: u32,
}

#[derive(Component)]
struct Stain(u32);

#[derive(Resource)]
struct Gore {
    drop: Handle<Mesh>,
    splats: Vec<Handle<Mesh>>,
    stains: Vec<Handle<Mesh>>,
    drop_mats: Vec<Handle<ColorMaterial>>,
    /// Two shades per fluid.
    stain_mats: Vec<[Handle<ColorMaterial>; 2]>,
    rng: Rng,
    seq: u32,
}

fn setup(mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>, mut materials: ResMut<Assets<ColorMaterial>>) {
    let mut rng = Rng(0xB100D);
    let splats = (0..VARIANTS).map(|_| meshes.add(splat_mesh(&mut rng))).collect();
    let stains = (0..VARIANTS).map(|_| meshes.add(stain_mesh(&mut rng))).collect();
    let drop_mats = Fluid::ALL.map(|f| materials.add(f.fresh())).to_vec();
    let stain_mats = Fluid::ALL
        .map(|f| {
            let mid: Color = Oklaba::from(f.fresh()).mix(&f.dry().into(), 0.35).into();
            [materials.add(mid), materials.add(f.dry())]
        })
        .to_vec();
    commands.insert_resource(Gore {
        drop: meshes.add(Circle::new(1.0)),
        splats,
        stains,
        drop_mats,
        stain_mats,
        rng,
        seq: 0,
    });
}

/// A splat of radius ~1: a lumpy blob, satellite droplets and a streak or two.
fn splat_mesh(rng: &mut Rng) -> Mesh {
    let n = 16;
    let main: Vec<Vec2> = (0..n)
        .map(|i| {
            let a = i as f32 / n as f32 * std::f32::consts::TAU;
            Vec2::from_angle(a) * rng.range(0.6, 1.0)
        })
        .collect();
    let mut parts = vec![smooth_closed(&main, 3)];
    for _ in 0..rng.below(3) + 2 {
        let dir = Vec2::from_angle(rng.range(0.0, std::f32::consts::TAU));
        let r = rng.range(0.12, 0.28);
        let at = dir * rng.range(1.15, 1.8);
        parts.push(ellipse_pts(r, r, 8).into_iter().map(|p| p + at).collect());
    }
    for _ in 0..rng.below(2) + 1 {
        let a = rng.range(0.0, std::f32::consts::TAU);
        let len = rng.range(0.5, 0.9);
        let streak: Vec<Vec2> = ellipse_pts(len, 0.12, 10)
            .into_iter()
            .map(|p| Vec2::from_angle(a).rotate(p + Vec2::X * (0.7 + len * 0.5)))
            .collect();
        parts.push(streak);
    }
    blobs(&parts, |p| 0.7 + 0.3 * (1.0 - p.length() / 1.8).max(0.0))
}

/// A stain of width ~2 sitting on y = 0: a flat puddle with drips running down.
fn stain_mesh(rng: &mut Rng) -> Mesh {
    let mut parts = vec![ellipse_pts(1.0, rng.range(0.16, 0.26), 16)];
    for _ in 0..rng.below(3) {
        let x = rng.range(-0.7, 0.7);
        let len = rng.range(0.3, 1.1);
        parts.push(ellipse_pts(0.09, len / 2.0, 8).into_iter().map(|p| p + Vec2::new(x, -len / 2.0)).collect());
        parts.push(ellipse_pts(0.13, 0.15, 8).into_iter().map(|p| p + Vec2::new(x, -len)).collect());
    }
    blobs(&parts, |p| 0.85 + 0.15 * p.y.clamp(-1.0, 1.0))
}

impl Gore {
    fn spawn_drop(&mut self, commands: &mut Commands, pos: Vec2, vel: Vec2, size: f32, fluid: Fluid, sticky: bool) {
        commands.spawn((
            Drop { vel, size, fluid, sticky, life: DROP_LIFE },
            Mesh2d(self.drop.clone()),
            MeshMaterial2d(self.drop_mats[fluid.index()].clone()),
            Transform::from_translation(pos.extend(20.0)).with_scale(Vec3::splat(size)),
            fluid.layers(),
        ));
    }

    fn spawn_splat(
        &mut self,
        commands: &mut Commands,
        materials: &mut Assets<ColorMaterial>,
        parent: Entity,
        target: &PaintTarget,
        local: Vec2,
        size: f32,
        fluid: Fluid,
    ) {
        // Keep it on the part: pull the center inside its ellipse.
        let d = ((local - target.center) / target.radii).length();
        let local = if d > 0.85 { target.center + (local - target.center) * (0.85 / d) } else { local };
        let size = size.min(target.radii.min_element() * 1.1);
        let mat = materials.add(fluid.fresh());
        self.seq += 1;
        let mesh = self.splats[self.rng.below(VARIANTS)].clone();
        let tf = Transform::from_translation(local.extend(0.35 + (self.seq % 200) as f32 * 0.0005))
            .with_rotation(Quat::from_rotation_z(self.rng.range(0.0, std::f32::consts::TAU)))
            .with_scale(Vec3::new(size, size * self.rng.range(0.75, 1.1), 1.0));
        commands.spawn((
            Splat { age: 0.0, fluid, mat: mat.clone(), seq: self.seq },
            Mesh2d(mesh),
            MeshMaterial2d(mat),
            tf,
            fluid.layers(),
            ChildOf(parent),
        ));
    }
}

fn spray(mut commands: Commands, mut gore: ResMut<Gore>, mut bleeds: MessageReader<Bleed>) {
    for b in bleeds.read() {
        let dir = b.dir.normalize_or(Vec2::Y);
        for _ in 0..b.amount {
            let r = &mut gore.rng;
            let vel = Vec2::from_angle(r.range(-b.spread, b.spread)).rotate(dir) * b.speed * r.range(0.25, 1.0);
            // Mostly fine mist, with the occasional fat glob.
            let size = r.range(0.9, 2.4) * if r.chance(0.1) { 2.0 } else { 1.0 };
            let pos = b.pos + Vec2::new(r.range(-4.0, 4.0), r.range(-4.0, 4.0));
            gore.spawn_drop(&mut commands, pos, vel, size, b.fluid, b.sticky);
        }
    }
}

fn paint_claws(
    mut commands: Commands,
    mut gore: ResMut<Gore>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    mut paints: MessageReader<Paint>,
    targets: Query<(Entity, &PaintTarget)>,
) {
    let claws: Vec<_> = targets.iter().filter(|(_, t)| t.claw).collect();
    if claws.is_empty() {
        return;
    }
    for paint in paints.read() {
        for _ in 0..paint.amount {
            let (e, t) = claws[gore.rng.below(claws.len())];
            let r = &mut gore.rng;
            let local = t.center + Vec2::new(r.range(-0.7, 0.7), r.range(-0.7, 0.7)) * t.radii;
            let size = r.range(2.5, 5.0);
            gore.spawn_splat(&mut commands, &mut materials, e, t, local, size, paint.fluid);
        }
    }
}

fn move_drops(
    mut commands: Commands,
    time: Res<Time>,
    mut gore: ResMut<Gore>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    mut drops: Query<(Entity, &mut Drop, &mut Transform)>,
    targets: Query<(Entity, &PaintTarget, &GlobalTransform)>,
    platforms: Query<(Entity, &Platform, &Transform), Without<Drop>>,
) {
    let dt = time.delta_secs().min(1.0 / 30.0);
    let gore = &mut *gore;
    'drops: for (e, mut d, mut tf) in &mut drops {
        let prev = tf.translation.truncate();
        d.vel.y -= GRAVITY * dt;
        d.vel *= 1.0 - 0.8 * dt;
        let pos = prev + d.vel * dt;
        d.life -= dt;
        tf.translation = pos.extend(tf.translation.z);
        // Stretch along the motion.
        let stretch = (1.0 + d.vel.length() * 0.004).min(3.0);
        tf.rotation = Quat::from_rotation_z(d.vel.to_angle());
        tf.scale = Vec3::new(d.size * stretch, d.size, 1.0);

        if d.life <= 0.0 || pos.y < -900.0 {
            commands.entity(e).despawn();
            continue;
        }

        if d.sticky {
            let hit = targets
                .iter()
                .map(|(te, t, gt)| {
                    let local = gt.affine().inverse().transform_point3(pos.extend(0.0)).truncate();
                    (te, t, gt, local, ((local - t.center) / t.radii).length())
                })
                .min_by(|a, b| a.4.total_cmp(&b.4));
            if let Some((te, t, gt, local, dist)) = hit {
                if dist < 1.15 {
                    // Splats grow as they hit, measured in the part's (scaled) space.
                    let size = d.size * 1.8 / gt.scale().y.abs().max(0.01);
                    gore.spawn_splat(&mut commands, &mut materials, te, t, local, size, d.fluid);
                    commands.entity(e).despawn();
                    continue;
                }
            }
        }

        for (pe, plat, ptf) in &platforms {
            if !plat.solid {
                continue;
            }
            let c = ptf.translation.truncate();
            let inv = ptf.rotation.inverse();
            let local = (inv * (pos - c).extend(0.0)).truncate();
            if local.x.abs() > plat.half.x || local.y.abs() > plat.half.y {
                continue;
            }
            let prev_local = (inv * (prev - c).extend(0.0)).truncate();
            if prev_local.y >= plat.half.y - 3.0 {
                // Landed on top: leave a stain that rides with the platform.
                let r = &mut gore.rng;
                let w = d.size * r.range(2.0, 3.5);
                let flip = if r.chance(0.5) { -1.0 } else { 1.0 };
                let mesh = gore.stains[r.below(VARIANTS)].clone();
                let mat = gore.stain_mats[d.fluid.index()][r.below(2)].clone();
                gore.seq += 1;
                let z = 0.5 + (gore.seq % 300) as f32 * 0.0005;
                commands.spawn((
                    Stain(gore.seq),
                    Mesh2d(mesh),
                    MeshMaterial2d(mat),
                    Transform::from_xyz(local.x, plat.half.y, z).with_scale(Vec3::new(w * flip, w, 1.0)),
                    d.fluid.layers(),
                    ChildOf(pe),
                ));
            }
            commands.entity(e).despawn();
            continue 'drops;
        }
    }
}

/// Fresh splats drip, then dry to a dark crust. Oldest ones flake off.
fn age_splats(
    mut commands: Commands,
    time: Res<Time>,
    mut gore: ResMut<Gore>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    mut splats: Query<(Entity, &mut Splat, &GlobalTransform)>,
) {
    let dt = time.delta_secs();
    for (_, mut s, gt) in &mut splats {
        s.age += dt;
        if s.age < WET_TIME && gore.rng.chance(dt * 0.7) {
            let r = &mut gore.rng;
            let vel = Vec2::new(r.range(-8.0, 8.0), -r.range(10.0, 40.0));
            gore.spawn_drop(&mut commands, gt.translation().truncate(), vel, 1.0, s.fluid, false);
        }
        if s.age < DRY_TIME + 1.0 {
            let k = (s.age / DRY_TIME).min(1.0);
            if let Some(mut mat) = materials.get_mut(&s.mat) {
                mat.color = Oklaba::from(s.fluid.fresh()).mix(&s.fluid.dry().into(), k).into();
            }
        }
    }
    let count = splats.iter().count();
    if count > MAX_SPLATS {
        let mut all: Vec<_> = splats.iter().map(|(e, s, _)| (s.seq, e)).collect();
        all.sort_unstable_by_key(|&(seq, _)| seq);
        for &(_, e) in &all[..count - MAX_SPLATS] {
            commands.entity(e).despawn();
        }
    }
}

fn cap_stains(mut commands: Commands, stains: Query<(Entity, &Stain)>) {
    let count = stains.iter().count();
    if count > MAX_STAINS {
        let mut all: Vec<_> = stains.iter().map(|(e, s)| (s.0, e)).collect();
        all.sort_unstable_by_key(|&(seq, _)| seq);
        for &(_, e) in &all[..count - MAX_STAINS] {
            commands.entity(e).despawn();
        }
    }
}
