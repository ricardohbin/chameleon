//! Colors that come back to the world, not only to the chameleon.
//!
//! Each beaten boss gives its color back to the part of the world it ruled.
//! The Venom Mother's green is vegetation: the vines of her region (placed by
//! `terrain_props/vines.rs`) are drawn twice, black and white on the world
//! layer and in color on the color layer, and when the wasp falls the colored
//! copies fade in as a wave spreading out from where she lived. The Gutter
//! King's yellow works the same (sludge, dry grass, sulfur fumes...).

use bevy::{prelude::*, render::view::ColorGrading};

use bevy::{
    asset::RenderAssetUsages, mesh::Indices, render::render_resource::PrimitiveTopology, sprite_render::AlphaMode2d,
};

use crate::{
    atmosphere::Glare,
    boss::{BossKind, Defeated},
    camera::{self, MainCamera, Shake},
    hud::ScreenFlash,
    terrain::Terrain,
};

/// The colors the lightning flash tints the world with (`atmosphere::lightning`).
const FLASH: Color = Color::srgb(0.9, 0.9, 1.0);

/// How long after the kill the wave of plants/things starts (virtual seconds: the death is in
/// slow motion). The Venom Mother's green washes over everything first, fades out, and only
/// then do the plants turn green, like an aftershock.
fn delay(boss: BossKind) -> f32 {
    match boss {
        BossKind::Wasp => 3.6,
        BossKind::Roach | BossKind::Dragonfly | BossKind::Spider | BossKind::Centipede | BossKind::Frog | BossKind::Grasshopper | BossKind::Rat | BossKind::Mantis | BossKind::Crab | BossKind::Beetle => 0.8,
    }
}
/// How fast the wave spreads, world units per second, and how much faster it gets
/// each second (the world is huge: the far regions shouldn't wait half a minute).
const SPEED: f32 = 700.0;
const SPEEDUP: f32 = 400.0;
/// How wide the fading edge of the wave is, world units.
const BAND: f32 = 350.0;

pub struct RegrowthPlugin;

impl Plugin for RegrowthPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Regrowth>().add_systems(Startup, (spawn_tint, spawn_skylight)).add_systems(Update, (regrow, tint_and_pulse, skylight, tint_backdrop).chain());
    }
}

/// When each boss's color was earned (virtual seconds), `None` while it hasn't been.
#[derive(Resource, Default)]
pub struct Regrowth {
    since: [Option<f32>; 11],
    now: f32,
    /// How far each wave has got (world units from the boss's home).
    reach: [Option<f32>; 11],
}

/// The colors themselves (the bosses' own, lightly).
pub const GREEN: Color = Color::srgb(0.36, 0.78, 0.18);
pub const YELLOW: Color = Color::srgb(0.93, 0.80, 0.32);
pub const PINK: Color = Color::srgb_u8(0xd1, 0xb0, 0xc6);
pub const BLUE: Color = Color::srgb_u8(0x7f, 0x9f, 0xd6);
pub const PURPLE: Color = Color::srgb_u8(0xa2, 0x00, 0xa2);
pub const VIOLET: Color = Color::srgb_u8(0xb8, 0x00, 0xff);
pub const BROWN: Color = Color::srgb_u8(0x8f, 0x56, 0x3b);
pub const RED: Color = Color::srgb_u8(0xac, 0x32, 0x32);
pub const CYAN: Color = Color::srgb_u8(0x00, 0xff, 0xff);
pub const INDIGO: Color = Color::srgb_u8(0x5b, 0x6e, 0xe1);
pub const ORANGE: Color = Color::srgb_u8(0xff, 0x81, 0x00);

/// How far from its home a boss's color is felt in the air.
const FELT: f32 = 4500.0;

fn index(boss: BossKind) -> usize {
    match boss {
        BossKind::Wasp => 0,
        BossKind::Roach => 1,
        BossKind::Dragonfly => 2,
        BossKind::Spider => 3,
        BossKind::Centipede => 4,
        BossKind::Frog => 5,
        BossKind::Grasshopper => 6,
        BossKind::Rat => 7,
        BossKind::Mantis => 8,
        BossKind::Crab => 9,
        BossKind::Beetle => 10,
    }
}

impl Regrowth {
    /// 0..1: the transient wash of the Venom Mother's green (in the light, the mist,
    /// the spores): it comes in with the kill and is gone in about three seconds.
    /// The Gutter King's yellow doesn't wash out, it stays (always 1 once earned).
    pub fn wash(&self, boss: BossKind) -> f32 {
        let Some(since) = self.since[index(boss)] else { return 0.0 };
        let t = self.now - since;
        let smooth = |x: f32| {
            let x = x.clamp(0.0, 1.0);
            x * x * (3.0 - 2.0 * x)
        };
        match boss {
            BossKind::Wasp => smooth(t / 0.4) * (1.0 - smooth((t - 0.8) / 1.8)),
            // The Weaver's blue and the Razorwing's pink wash over the screen while their wave
            // runs over the world (it starts `delay` after the kill) and are gone once the
            // world is painted. Only the Gutter King's yellow stays: the smog is his.
            BossKind::Spider | BossKind::Dragonfly | BossKind::Centipede | BossKind::Frog | BossKind::Grasshopper | BossKind::Rat | BossKind::Mantis | BossKind::Crab | BossKind::Beetle => smooth(t / 0.6) * (1.0 - smooth((t - 3.6) / 3.0)),
            BossKind::Roach => 1.0,
        }
    }

    /// (how much of the color has come back where the camera is, how close
    /// the camera is to the boss's home: 1 = there, 0 = far).
    pub fn felt_at(&self, boss: BossKind, terrain: &Terrain, eye: Vec2) -> (f32, f32) {
        let home = match boss {
            BossKind::Wasp => terrain.wasp,
            BossKind::Roach => terrain.roach,
            BossKind::Dragonfly => terrain.dragonfly,
            BossKind::Spider => terrain.spider,
            BossKind::Centipede => terrain.centipede,
            BossKind::Frog => terrain.frog,
            BossKind::Grasshopper => terrain.grasshopper,
            BossKind::Rat => terrain.rat,
            BossKind::Mantis => terrain.mantis,
            BossKind::Crab => terrain.crab,
            BossKind::Beetle => terrain.beetle,
        };
        let dist = home.map_or(f32::MAX, |h| h.distance(eye));
        (self.reveal(boss, dist), (1.0 - dist / FELT).clamp(0.0, 1.0))
    }

    /// 0..1: how much of `boss`'s color has come back at `dist` from its home.
    pub fn reveal(&self, boss: BossKind, dist: f32) -> f32 {
        let want = self.reach[index(boss)].map_or(0.0, |r| ((r - dist) / BAND).clamp(0.0, 1.0));
        want * want * (3.0 - 2.0 * want)
    }
}

/// The living copy of a vine (on the color layer, hidden until its turn).
#[derive(Component)]
pub struct Vine {
    /// The boss whose color this is.
    pub boss: BossKind,
    /// Distance from the boss's home: the wave reaches it later the farther it is.
    pub dist: f32,
    pub stem: Handle<ColorMaterial>,
    pub leaf: Handle<ColorMaterial>,
    pub shown: f32,
    /// The colors in daylight (the flash tints them).
    pub base: [Color; 2],
    pub lit: f32,
}

pub(crate) fn regrow(
    time: Res<Time<Virtual>>,
    defeated: Res<Defeated>,
    glare: Res<Glare>,
    mut grow: ResMut<Regrowth>,
    mut vines: Query<(&mut Vine, &mut Visibility)>,
    mut materials: ResMut<Assets<ColorMaterial>>,
) {
    let now = time.elapsed_secs();
    grow.now = now;
    for boss in BossKind::ALL {
        let i = index(boss);
        let earned = defeated.0.contains(&boss);
        match (earned, grow.since[i]) {
            (true, None) => grow.since[i] = Some(now),
            (false, Some(_)) => grow.since[i] = None,
            _ => {}
        }
        grow.reach[i] = grow.since[i].map(|s| {
            let t = (now - s - delay(boss)).max(0.0);
            t * SPEED + t * t * SPEEDUP
        });
    }
    for (mut vine, mut visibility) in &mut vines {
        let want = grow.reveal(vine.boss, vine.dist);
        let lit = glare.0 * 0.35;
        if (want - vine.shown).abs() < 0.002 && (want > 0.0) == (vine.shown > 0.0) && (want == 0.0 || lit == vine.lit) {
            continue;
        }
        (vine.shown, vine.lit) = (want, lit);
        *visibility = if want > 0.0 { Visibility::Inherited } else { Visibility::Hidden };
        for (h, base) in [(&vine.stem, vine.base[0]), (&vine.leaf, vine.base[1])] {
            if let Some(mut m) = materials.get_mut(h) {
                // The flash overlay doesn't reach the color layer: tint them the same.
                m.color = base.mix(&FLASH, lit).with_alpha(want);
            }
        }
    }
}

/// A wash of the colors over the whole screen, thickest near the home of the
/// boss that gave them: the gray world takes on a cast, you feel it come back.
/// (The Venom Mother's, the Razorwing's and the Weaver's only last as a transition: see `Regrowth::wash`.)
#[derive(Component)]
struct Tint;

fn spawn_tint(mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>, mut materials: ResMut<Assets<ColorMaterial>>) {
    let material = materials.add(ColorMaterial { color: Color::srgba(1.0, 1.0, 1.0, 0.0), alpha_mode: AlphaMode2d::Blend, ..default() });
    commands.spawn((
        Mesh2d(meshes.add(Rectangle::new(12000.0, 8000.0))),
        MeshMaterial2d(material),
        Transform::from_xyz(0.0, 0.0, -0.9),
        camera::color_layer(),
        Tint,
    ));
}

fn tint_and_pulse(
    regrowth: Res<Regrowth>,
    terrain: Res<Terrain>,
    camera: Single<&Transform, (With<MainCamera>, Without<Tint>)>,
    tint: Single<(&mut Transform, &MeshMaterial2d<ColorMaterial>), With<Tint>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    mut flash: ResMut<ScreenFlash>,
    mut shake: ResMut<Shake>,
    mut waved: Local<[bool; 11]>,
) {
    let eye = camera.translation.truncate();
    let (mut tf, mat) = tint.into_inner();
    tf.translation.x = eye.x;
    tf.translation.y = eye.y;
    let (mut rgb, mut alpha) = (Vec3::ZERO, 0.0);
    for (boss, color, strength) in [(BossKind::Wasp, GREEN, 0.03), (BossKind::Roach, YELLOW, 0.15), (BossKind::Dragonfly, PINK, 0.1), (BossKind::Spider, BLUE, 0.1), (BossKind::Centipede, PURPLE, 0.1), (BossKind::Frog, VIOLET, 0.1), (BossKind::Grasshopper, BROWN, 0.1), (BossKind::Rat, RED, 0.1), (BossKind::Mantis, CYAN, 0.1), (BossKind::Crab, INDIGO, 0.1), (BossKind::Beetle, ORANGE, 0.1)] {
        let (revealed, near) = regrowth.felt_at(boss, &terrain, eye);
        let mut a = revealed * (if boss == BossKind::Wasp { 0.0 } else { 0.04 } + strength * near * near);
        // A transition, not a lasting cast: the blue lets go once the world is painted.
        if matches!(boss, BossKind::Spider | BossKind::Dragonfly | BossKind::Centipede | BossKind::Frog | BossKind::Grasshopper | BossKind::Rat | BossKind::Mantis | BossKind::Crab | BossKind::Beetle) {
            a *= regrowth.wash(boss);
        }
        let c = color.to_srgba();
        rgb += Vec3::new(c.red, c.green, c.blue) * a;
        alpha += a;
        // The front of the wave goes over the camera: a flash and a thud.
        let i = index(boss);
        if revealed > 0.15 && !waved[i] {
            waved[i] = true;
            if near > 0.05 {
                flash.flash(color, 0.35);
                shake.add(0.35);
            }
        } else if revealed <= 0.0 {
            waved[i] = false;
        }
    }
    if let Some(mut m) = materials.get_mut(&mat.0) {
        let c = if alpha > 0.0 { rgb / alpha } else { Vec3::ONE };
        m.color = Color::srgba(c.x, c.y, c.z, alpha.min(0.22));
    }
}

/// The Venom Mother's color doesn't hang in the air like the king's fumes: in
/// the open it comes with the light. A sky glow, green at the top and thinning
/// toward the ground, and slow shafts of pale green light leaning across the
/// scene, over where the sky is open.
#[derive(Component)]
struct SkyGlow;

#[derive(Component)]
struct Shaft {
    base_x: f32,
    phase: f32,
}

fn gradient_quad(w: f32, h: f32, top: [f32; 4], bottom: [f32; 4]) -> Mesh {
    let pos = vec![[-w / 2.0, h / 2.0, 0.0], [w / 2.0, h / 2.0, 0.0], [-w / 2.0, -h / 2.0, 0.0], [w / 2.0, -h / 2.0, 0.0]];
    mesh_of(pos, vec![top, top, bottom, bottom], vec![0, 2, 1, 1, 2, 3])
}

/// A long soft beam: clear at its edges and ends, brightest in the middle.
fn shaft_mesh(w: f32, len: f32) -> Mesh {
    let (mut pos, mut colors, mut idx) = (Vec::new(), Vec::new(), Vec::new());
    for (j, (y, a_row)) in [(0.0, 0.0), (len * 0.2, 1.0), (len, 0.0)].into_iter().enumerate() {
        for (i, (x, a_col)) in [(-w / 2.0, 0.0), (0.0, 1.0), (w / 2.0, 0.0)].into_iter().enumerate() {
            pos.push([x, y, 0.0]);
            colors.push([1.0, 1.0, 1.0, a_row * a_col]);
            if i < 2 && j < 2 {
                let a = (j * 3 + i) as u32;
                idx.extend([a, a + 1, a + 3, a + 1, a + 4, a + 3]);
            }
        }
    }
    mesh_of(pos, colors, idx)
}

fn mesh_of(pos: Vec<[f32; 3]>, colors: Vec<[f32; 4]>, idx: Vec<u32>) -> Mesh {
    let n = pos.len();
    Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default())
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, pos)
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, vec![[0.0, 0.0, 1.0]; n])
        .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, vec![[0.0, 0.0]; n])
        .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, colors)
        .with_inserted_indices(Indices::U32(idx))
}

fn spawn_skylight(mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>, mut materials: ResMut<Assets<ColorMaterial>>) {
    let blend = |color| ColorMaterial { color, alpha_mode: AlphaMode2d::Blend, ..default() };
    let glow = GREEN.to_linear().to_f32_array();
    let glow_top = [glow[0] * 1.1, glow[1] * 1.2, glow[2] * 0.9, 1.0];
    let glow_bottom = [glow[0], glow[1], glow[2], 0.12];
    commands.spawn((
        Mesh2d(meshes.add(gradient_quad(12000.0, 8000.0, glow_top, glow_bottom))),
        MeshMaterial2d(materials.add(blend(Color::srgba(1.0, 1.0, 1.0, 0.0)))),
        Transform::from_xyz(0.0, 0.0, -0.88),
        camera::color_layer(),
        SkyGlow,
    ));
    let beam = meshes.add(shaft_mesh(150.0, 3400.0));
    for i in 0..6 {
        let material = materials.add(blend(Color::srgba(0.80, 0.96, 0.55, 0.0)));
        commands.spawn((
            Mesh2d(beam.clone()),
            MeshMaterial2d(material),
            Transform::from_xyz(0.0, 0.0, -0.87).with_rotation(Quat::from_rotation_z(-0.5 + 0.06 * (i % 3) as f32)).with_scale(Vec3::new(0.7 + 0.35 * (i % 4) as f32, 1.0, 1.0)),
            camera::color_layer(),
            Visibility::Hidden,
            Shaft { base_x: i as f32 * 620.0 + (i * i % 5) as f32 * 90.0, phase: i as f32 * 1.7 },
        ));
    }
}

fn skylight(
    time: Res<Time>,
    regrowth: Res<Regrowth>,
    terrain: Res<Terrain>,
    camera: Single<&Transform, (With<MainCamera>, Without<SkyGlow>, Without<Shaft>)>,
    glow: Single<(&mut Transform, &MeshMaterial2d<ColorMaterial>), (With<SkyGlow>, Without<Shaft>)>,
    mut shafts: Query<(&mut Transform, &MeshMaterial2d<ColorMaterial>, &mut Visibility, &Shaft), Without<SkyGlow>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
) {
    let eye = camera.translation.truncate();
    let (_, near) = regrowth.felt_at(BossKind::Wasp, &terrain, eye);
    let revealed = regrowth.wash(BossKind::Wasp);
    // Only in the open: a roof over the camera box takes the sky and its light away.
    let open = [-900.0, -450.0, 0.0, 450.0, 900.0]
        .into_iter()
        .filter(|dx| (1..=150).all(|k| !terrain.solid_at(eye + Vec2::new(*dx, k as f32 * crate::terrain::PX))))
        .count() as f32
        / 5.0;
    let strength = revealed * (0.35 + 0.65 * near);
    let (mut tf, mat) = glow.into_inner();
    (tf.translation.x, tf.translation.y) = (eye.x, eye.y);
    if let Some(mut m) = materials.get_mut(&mat.0) {
        m.color = m.color.with_alpha(strength * (0.12 + 0.2 * open));
    }
    let t = time.elapsed_secs();
    for (mut tf, mat, mut vis, shaft) in &mut shafts {
        let a = strength * open * (0.55 + 0.45 * (t * 0.35 + shaft.phase).sin()) * 0.2;
        *vis = if a > 0.002 { Visibility::Inherited } else { Visibility::Hidden };
        // Drifts slowly against the world, wrapped around the camera.
        let x = (shaft.base_x - eye.x * 0.12 + t * 6.0).rem_euclid(3600.0) - 1800.0;
        tf.translation = Vec3::new(eye.x + x, eye.y - 700.0, tf.translation.z);
        if let Some(mut m) = materials.get_mut(&mat.0) {
            m.color = m.color.with_alpha(a);
        }
    }
}

/// The Razorwing's pink stays in the far background of its own region (the treatment plant):
/// the backdrop camera's grading lets a pink cast through while the camera is there.
fn tint_backdrop(
    regrowth: Res<Regrowth>,
    terrain: Res<Terrain>,
    camera: Single<&Transform, (With<MainCamera>, Without<camera::BackdropCamera>)>,
    mut grading: Single<&mut ColorGrading, With<camera::BackdropCamera>>,
    mut last: Local<f32>,
) {
    let eye = camera.translation.truncate();
    let (revealed, _) = regrowth.felt_at(BossKind::Dragonfly, &terrain, eye);
    // In its region, fading out over 1500 units around it.
    let away = terrain.dragonfly.and_then(|d| terrain.area_at(d)).map_or(f32::MAX, |a| eye.clamp(a.rect.min, a.rect.max).distance(eye));
    let inside = (1.0 - away / 1500.0).clamp(0.0, 1.0);
    let k = revealed * inside * inside * (3.0 - 2.0 * inside);
    if (k - *last).abs() > 0.002 || (k == 0.0) != (*last == 0.0) {
        *last = k;
        **grading = camera::backdrop_grading(k);
    }
}
