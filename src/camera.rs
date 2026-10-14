//! Follow camera with look-ahead and trauma-based screen shake.

use bevy::{
    camera::visibility::RenderLayers,
    prelude::*,
    render::view::{ColorGrading, ColorGradingGlobal},
};

use crate::player::{self, Player};

const LOOK_AHEAD: f32 = 80.0;
const HEIGHT_OFFSET: f32 = 40.0;
const FOLLOW_RATE: f32 = 4.0;
const ZOOM: f32 = 0.65;
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

pub struct CameraPlugin;

impl Plugin for CameraPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Shake>()
            .init_resource::<HitStop>()
            .init_resource::<ZoomPunch>()
            .add_systems(Startup, spawn_camera)
            .add_systems(First, apply_hit_stop)
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

/// The grayscale world camera (the color overlay camera is its child).
#[derive(Component)]
pub struct MainCamera;

/// Camera position before shake is applied.
#[derive(Component)]
struct Follow(Vec2);

fn spawn_camera(mut commands: Commands) {
    let projection = || Projection::Orthographic(OrthographicProjection { scale: ZOOM, ..OrthographicProjection::default_2d() });
    let main = commands
        .spawn((
            Camera2d,
            MainCamera,
            Follow(player::SPAWN),
            projection(),
            Transform::from_translation(player::SPAWN.extend(0.0)),
            ColorGrading {
                global: ColorGradingGlobal { post_saturation: WORLD_SATURATION, ..default() },
                ..default()
            },
        ))
        .id();
    // Overlay: same view, drawn after the world, keeps its colors (the HUD too).
    commands.spawn((
        Camera2d,
        IsDefaultUiCamera,
        Camera { order: 1, clear_color: ClearColorConfig::None, ..default() },
        projection(),
        color_layer(),
        Transform::default(),
        ChildOf(main),
    ));
}

fn follow_player(
    time: Res<Time>,
    mut shake: ResMut<Shake>,
    mut punch: ResMut<ZoomPunch>,
    camera: Single<(&mut Transform, &mut Follow), (With<MainCamera>, Without<Player>)>,
    mut projections: Query<&mut Projection>,
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

    let t = time.elapsed_secs();
    let k = shake.trauma * shake.trauma;
    let offset = Vec2::new(noise(t * 23.0), noise(t * 19.0 + 7.0)) * 18.0 * k;
    tf.translation = (follow.0 + offset).extend(tf.translation.z);
    tf.rotation = Quat::from_rotation_z(noise(t * 13.0 + 3.0) * 0.03 * k);
    shake.trauma = (shake.trauma - dt * 1.1).max(0.0);

    // Both cameras share the zoom.
    let scale = ZOOM * (1.0 - PUNCH_ZOOM * punch.0 * punch.0 * (3.0 - 2.0 * punch.0));
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
