mod atmosphere;
mod camera;
mod chameleon;
mod dev;
mod gore;
mod hud;
mod level;
mod player;
mod shapes;
mod unstable;
mod wasp;

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
            gore::GorePlugin,
            wasp::WaspPlugin,
            camera::CameraPlugin,
            dev::DevPlugin,
            hud::HudPlugin,
        ))
        .run();
}
