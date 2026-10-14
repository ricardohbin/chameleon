mod atmosphere;
mod camera;
mod chameleon;
mod dev;
mod level;
mod player;
mod shapes;
mod unstable;

use bevy::prelude::*;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Camaleon".into(),
                resolution: (1280, 720).into(),
                ..default()
            }),
            ..default()
        }))
        .add_plugins((
            atmosphere::AtmospherePlugin,
            level::LevelPlugin,
            unstable::UnstablePlugin,
            player::PlayerPlugin,
            chameleon::ChameleonPlugin,
            camera::CameraPlugin,
            dev::DevPlugin,
        ))
        .add_systems(Startup, spawn_hud)
        .run();
}

fn spawn_hud(mut commands: Commands) {
    commands.spawn((
        Text::new("A/D or Arrows: walk    Space/W/Up: jump    F: tongue    F12: screenshot"),
        TextFont { font_size: FontSize::Px(18.0), ..default() },
        TextColor(Color::srgba(0.85, 0.82, 0.78, 0.7)),
        Node { position_type: PositionType::Absolute, top: px(12), left: px(14), ..default() },
    ));
}
