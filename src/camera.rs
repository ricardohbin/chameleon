//! Follow camera with look-ahead and trauma-based screen shake.

use bevy::{
    camera::visibility::RenderLayers,
    prelude::*,
    render::view::{ColorGrading, ColorGradingGlobal},
    window::PrimaryWindow,
};

use crate::{
    level::{self, Restart},
    player::{self, Player},
    terrain::Terrain,
};

const LOOK_AHEAD: f32 = 80.0;
const HEIGHT_OFFSET: f32 = 40.0;
const FOLLOW_RATE: f32 = 4.0;
pub const ZOOM: f32 = 0.65;
/// How much a full `ZoomPunch` zooms in.
const PUNCH_ZOOM: f32 = 0.2;
/// 0.0 = grayscale world, 1.0 = full color. Colors are earned as the game
/// progresses, so this starts at 0 (all color data is kept intact).
const WORLD_SATURATION: f32 = 0.0;
/// Render layer drawn by a second camera *without* the grayscale grading.
/// Blood lives here: in a world drained of color, gore is the first thing that has any.
pub const COLOR_LAYER: usize = 1;

pub fn color_layer() -> RenderLayers {
    RenderLayers::layer(COLOR_LAYER)
}

/// Render layer of the far backdrop (sky, skylines, smoke): drawn first by its own camera,
/// gray like the world but with its own grading, so a region can tint it (`regrowth`).
pub const BACKDROP_LAYER: usize = 2;

pub fn backdrop_layer() -> RenderLayers {
    RenderLayers::layer(BACKDROP_LAYER)
}

/// Render layer drawn in color *under* the world, by its own camera (after the backdrop):
/// interiors that have earned a color back (`terrain_art`). The gray interior over it fades
/// out as the color comes in, so the gray world (mobs, props, bosses) still draws on top.
pub const UNDERLAY_LAYER: usize = 3;

pub fn underlay_layer() -> RenderLayers {
    RenderLayers::layer(UNDERLAY_LAYER)
}

/// The grading of the main (world) camera: gray.
pub fn world_grading() -> ColorGrading {
    ColorGrading { global: ColorGradingGlobal { post_saturation: WORLD_SATURATION, ..default() }, ..default() }
}

/// The cameras that keep their colors: the underlay (under the world) and the overlay (over it).
/// They have no grading of their own, except at the very end (`ending.rs`).
#[derive(Component)]
pub struct ColorCamera;

/// The backdrop camera (a child of the main one).
#[derive(Component)]
pub struct BackdropCamera;

/// The backdrop's grading: gray, with `k` (0..1) of a pink cast pushed through it.
pub fn backdrop_grading(k: f32) -> ColorGrading {
    ColorGrading { global: ColorGradingGlobal { temperature: -0.7 * k, tint: -0.6 * k, post_saturation: WORLD_SATURATION + 0.6 * k, ..default() }, ..default() }
}

pub struct CameraPlugin;

impl Plugin for CameraPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Shake>()
            .init_resource::<HitStop>()
            .init_resource::<ZoomPunch>()
            .init_resource::<DevZoom>()
            .init_resource::<ArenaView>()
            .add_systems(Startup, spawn_camera)
            .add_systems(First, apply_hit_stop)
            .add_systems(PreUpdate, restart.after(level::restart))
            .add_systems(Update, follow_player.after(player::move_player));
    }
}

/// Screen shake. Add trauma (0..1); it decays over time.
#[derive(Resource, Default)]
pub struct Shake {
    trauma: f32,
}

impl Shake {
    pub fn add(&mut self, amount: f32) {
        self.trauma = (self.trauma + amount).min(1.0);
    }

    /// Keep the shake at least this strong (for continuous rumbles).
    pub fn at_least(&mut self, amount: f32) {
        self.trauma = self.trauma.max(amount);
    }
}

/// Freeze-frame on heavy hits (game time crawls for a few real-time
/// milliseconds), optionally followed by slow motion that eases back.
#[derive(Resource, Default)]
pub struct HitStop {
    freeze: f32,
    slow: f32,
    slow_total: f32,
}

impl HitStop {
    pub fn add(&mut self, seconds: f32) {
        self.freeze = self.freeze.max(seconds);
    }

    /// Slow motion for `seconds` (real time) after the freeze, easing back to normal speed.
    pub fn slow_motion(&mut self, seconds: f32) {
        self.slow = seconds;
        self.slow_total = seconds;
    }
}

fn apply_hit_stop(real: Res<Time<Real>>, mut stop: ResMut<HitStop>, mut time: ResMut<Time<Virtual>>) {
    if stop.freeze <= 0.0 && stop.slow <= 0.0 {
        return;
    }
    let dt = real.delta_secs();
    if stop.freeze > 0.0 {
        stop.freeze -= dt;
    } else {
        stop.slow -= dt;
    }
    let speed = if stop.freeze > 0.0 {
        0.04
    } else if stop.slow > 0.0 {
        let k = stop.slow / stop.slow_total;
        1.0 - 0.75 * k.powf(1.5)
    } else {
        1.0
    };
    time.set_relative_speed(speed);
}

/// A quick zoom-in (0..1) that relaxes back.
#[derive(Resource, Default)]
pub struct ZoomPunch(f32);

impl ZoomPunch {
    pub fn add(&mut self, amount: f32) {
        self.0 = (self.0 + amount).min(1.0);
    }
}

/// A fight that wants a wider view kept inside its arena (the Burrower's caves): the boss asks
/// for it every frame it lasts. The camera pulls back (between the zoom multipliers `zoom`) to
/// frame both the chameleon and `show` (where the boss is), leaning toward it, and keeps the view
/// inside `rect`; it eases into it, and back out when nobody asks.
#[derive(Resource, Default)]
pub struct ArenaView {
    pub want: Option<ArenaWant>,
    /// 0..1 how far into the arena view it is; the zoom and the middle of the view it eases toward;
    /// the last request.
    k: f32,
    zoom: f32,
    center: Vec2,
    last: Option<ArenaWant>,
}

#[derive(Clone, Copy)]
pub struct ArenaWant {
    pub zoom: (f32, f32),
    pub rect: Rect,
    pub show: Vec2,
}

/// Zoom multiplier for developer tools (1 = normal).
#[derive(Resource)]
pub struct DevZoom(pub f32);

impl Default for DevZoom {
    fn default() -> Self {
        DevZoom(1.0)
    }
}

/// The grayscale world camera (the color overlay camera is its child).
#[derive(Component)]
pub struct MainCamera;

/// Camera position before shake is applied.
#[derive(Component)]
pub struct Follow(Vec2);

impl Follow {
    /// Cut the camera to where it would sit with the chameleon at `pos` (no easing).
    pub fn cut_to(&mut self, pos: Vec2) {
        self.0 = pos + Vec2::new(LOOK_AHEAD, HEIGHT_OFFSET);
    }
}

fn spawn_camera(mut commands: Commands, terrain: Res<Terrain>) {
    let spawn = player::spawn_point(&terrain);
    let projection = || Projection::Orthographic(OrthographicProjection { scale: ZOOM, ..OrthographicProjection::default_2d() });
    let main = commands
        .spawn((
            Camera2d,
            Camera { clear_color: ClearColorConfig::None, ..default() },
            MainCamera,
            Follow(spawn),
            projection(),
            Transform::from_translation(spawn.extend(0.0)),
            world_grading(),
        ))
        .id();
    // Backdrop: same view, drawn before the world (which doesn't clear), its own grading.
    commands.spawn((
        Camera2d,
        Camera { order: -2, ..default() },
        projection(),
        backdrop_layer(),
        BackdropCamera,
        backdrop_grading(0.0),
        Transform::default(),
        ChildOf(main),
    ));
    // Underlay: same view, between the backdrop and the world, keeps its colors.
    commands.spawn((
        Camera2d,
        Camera { order: -1, clear_color: ClearColorConfig::None, ..default() },
        projection(),
        underlay_layer(),
        ColorCamera,
        Transform::default(),
        ChildOf(main),
    ));
    // Overlay: same view, drawn after the world, keeps its colors.
    commands.spawn((
        Camera2d,
        Camera { order: 1, clear_color: ClearColorConfig::None, ..default() },
        projection(),
        color_layer(),
        ColorCamera,
        Transform::default(),
        ChildOf(main),
    ));
    // The HUD: its own camera, last, drawing no world layer, so no grading ever touches it.
    commands.spawn((
        Camera2d,
        IsDefaultUiCamera,
        Camera { order: 2, clear_color: ClearColorConfig::None, ..default() },
        RenderLayers::none(),
        Transform::default(),
        ChildOf(main),
    ));
}

/// F8: no leftover shake, freeze, slow motion or zoom; the camera cuts to the spawn.
fn restart(
    mut restarted: MessageReader<Restart>,
    terrain: Res<Terrain>,
    pending: Res<crate::save::Pending>,
    mut shake: ResMut<Shake>,
    mut stop: ResMut<HitStop>,
    mut punch: ResMut<ZoomPunch>,
    mut view: ResMut<ArenaView>,
    mut time: ResMut<Time<Virtual>>,
    mut follow: Single<&mut Follow, With<MainCamera>>,
) {
    if restarted.read().count() == 0 {
        return;
    }
    *shake = default();
    *view = default();
    *stop = default();
    *punch = default();
    time.set_relative_speed(1.0);
    follow.0 = pending.at().unwrap_or_else(|| player::spawn_point(&terrain)) + Vec2::new(LOOK_AHEAD, HEIGHT_OFFSET);
}

/// The offscreen camera of the full-world export (native dev tool): not the player's view.
#[derive(Component)]
pub(crate) struct ExportCamera;

pub fn follow_player(
    time: Res<Time>,
    mut shake: ResMut<Shake>,
    mut punch: ResMut<ZoomPunch>,
    mut view: ResMut<ArenaView>,
    dev_zoom: Res<DevZoom>,
    window: Option<Single<&Window, With<PrimaryWindow>>>,
    camera: Single<(&mut Transform, &mut Follow), (With<MainCamera>, Without<Player>)>,
    mut projections: Query<&mut Projection, Without<ExportCamera>>,
    player: Single<(&Transform, &Player)>,
) {
    let (mut tf, mut follow) = camera.into_inner();
    let (ptf, p) = *player;
    let dt = time.delta_secs();

    let target = ptf.translation.truncate() + Vec2::new(p.facing * LOOK_AHEAD, HEIGHT_OFFSET);
    if follow.0.distance(target) > 900.0 {
        // Teleported (respawn): cut instead of gliding across the level.
        follow.0 = target;
    }
    follow.0 = follow.0.lerp(target, 1.0 - (-FOLLOW_RATE * dt).exp());

    // An arena view: pulled back, and the view kept inside the arena (centered on it along a side
    // it doesn't fill).
    let wanted = view.want.take();
    let goal = if wanted.is_some() { 1.0 } else { 0.0 };
    if wanted.is_some() {
        if view.last.is_none() {
            view.center = follow.0;
            view.zoom = 1.0;
        }
        view.last = wanted;
    }
    view.k += (goal - view.k) * (1.0 - (-1.8 * dt).exp());
    let ease = view.k * view.k * (3.0 - 2.0 * view.k);
    let window_half = window.as_ref().map_or(Vec2::new(640.0, 360.0), |w| Vec2::new(w.width(), w.height()) * 0.5);
    let mut at = follow.0;
    if let Some(w) = view.last.filter(|_| view.k > 0.001) {
        // Framing the chameleon and the boss: the middle leans toward the boss, the zoom takes in both
        // with room around them.
        let center = follow.0.lerp(w.show, 0.35);
        view.center = view.center.lerp(center, 1.0 - (-2.0 * dt).exp());
        let need = ((w.show - view.center).abs().max((follow.0 - view.center).abs()) + Vec2::splat(450.0)) / window_half;
        let zoom = (need.max_element() / (ZOOM * dev_zoom.0)).clamp(w.zoom.0, w.zoom.1);
        view.zoom += (zoom - view.zoom) * (1.0 - (-1.2 * dt).exp());
        at = follow.0.lerp(view.center, ease);
    } else {
        view.last = None;
    }
    let pull = 1.0 + (view.zoom.max(1.0) - 1.0) * ease;
    let scale = ZOOM * dev_zoom.0 * pull * (1.0 - PUNCH_ZOOM * punch.0 * punch.0 * (3.0 - 2.0 * punch.0));
    if let Some(w) = view.last {
        // The view stays inside the arena (centered on it along a side it doesn't fill).
        let half = window_half * scale;
        let r = w.rect;
        let keep = |v: f32, lo: f32, hi: f32, h: f32| if hi - lo < 2.0 * h { (lo + hi) * 0.5 } else { v.clamp(lo + h, hi - h) };
        let inside = Vec2::new(keep(at.x, r.min.x, r.max.x, half.x), keep(at.y, r.min.y, r.max.y, half.y));
        at = at.lerp(inside, ease);
        // Whatever the boss is doing, the chameleon stays in frame, 5% of it from the edge
        // (even when the arena clamp or the zoom limit would leave it out).
        let k = shake.trauma * shake.trauma;
        let reach = (half * 0.9 - Vec2::splat(18.0 * k)).max(Vec2::ZERO);
        let ppos = ptf.translation.truncate();
        at = at.clamp(ppos - reach, ppos + reach);
    }

    let t = time.elapsed_secs();
    let k = shake.trauma * shake.trauma;
    let offset = Vec2::new(noise(t * 23.0), noise(t * 19.0 + 7.0)) * 18.0 * k;
    tf.translation = (at + offset).extend(tf.translation.z);
    tf.rotation = Quat::from_rotation_z(noise(t * 13.0 + 3.0) * 0.03 * k);
    shake.trauma = (shake.trauma - dt * 1.1).max(0.0);

    // All the cameras share the zoom.
    for mut projection in &mut projections {
        if let Projection::Orthographic(ortho) = &mut *projection {
            ortho.scale = scale;
        }
    }
    punch.0 = (punch.0 - dt * 0.9).max(0.0);
}

/// Cheap smooth pseudo-noise in about -1..1.
fn noise(x: f32) -> f32 {
    (x.sin() + (x * 2.13 + 1.7).sin() * 0.5) / 1.5
}
