//! Follow camera with look-ahead and trauma-based screen shake.

use bevy::{
    prelude::*,
    render::view::{ColorGrading, ColorGradingGlobal},
};

use crate::player::{self, Player};

const LOOK_AHEAD: f32 = 80.0;
const HEIGHT_OFFSET: f32 = 40.0;
const FOLLOW_RATE: f32 = 4.0;
const ZOOM: f32 = 0.65;
/// 0.0 = grayscale world, 1.0 = full color. Colors are earned as the game
/// progresses, so this starts at 0 (all color data is kept intact).
const WORLD_SATURATION: f32 = 0.0;

pub struct CameraPlugin;

impl Plugin for CameraPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Shake>()
            .add_systems(Startup, spawn_camera)
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

/// Camera position before shake is applied.
#[derive(Component)]
struct Follow(Vec2);

fn spawn_camera(mut commands: Commands) {
    commands.spawn((
        Camera2d,
        Follow(player::SPAWN),
        Projection::Orthographic(OrthographicProjection { scale: ZOOM, ..OrthographicProjection::default_2d() }),
        Transform::from_translation(player::SPAWN.extend(0.0)),
        ColorGrading {
            global: ColorGradingGlobal { post_saturation: WORLD_SATURATION, ..default() },
            ..default()
        },
    ));
}

fn follow_player(
    time: Res<Time>,
    mut shake: ResMut<Shake>,
    camera: Single<(&mut Transform, &mut Follow), Without<Player>>,
    player: Single<(&Transform, &Player)>,
) {
    let (mut tf, mut follow) = camera.into_inner();
    let (ptf, p) = *player;
    let dt = time.delta_secs();

    let target = ptf.translation.truncate() + Vec2::new(p.facing * LOOK_AHEAD, HEIGHT_OFFSET);
    follow.0 = follow.0.lerp(target, 1.0 - (-FOLLOW_RATE * dt).exp());

    let t = time.elapsed_secs();
    let k = shake.trauma * shake.trauma;
    let offset = Vec2::new(noise(t * 23.0), noise(t * 19.0 + 7.0)) * 18.0 * k;
    tf.translation = (follow.0 + offset).extend(tf.translation.z);
    tf.rotation = Quat::from_rotation_z(noise(t * 13.0 + 3.0) * 0.03 * k);
    shake.trauma = (shake.trauma - dt * 1.1).max(0.0);
}

/// Cheap smooth pseudo-noise in about -1..1.
fn noise(x: f32) -> f32 {
    (x.sin() + (x * 2.13 + 1.7).sin() * 0.5) / 1.5
}
