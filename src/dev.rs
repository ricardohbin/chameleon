//! Developer helpers: F12 saves a screenshot to `screenshots/`.

use bevy::{
    prelude::*,
    render::view::screenshot::{save_to_disk, Screenshot},
};

pub struct DevPlugin;

impl Plugin for DevPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, screenshot_on_f12);
    }
}

fn screenshot_on_f12(mut commands: Commands, keys: Res<ButtonInput<KeyCode>>, mut count: Local<u32>) {
    if keys.just_pressed(KeyCode::F12) {
        let path = format!("screenshots/shot-{}-{}.png", std::process::id(), *count);
        *count += 1;
        commands.spawn(Screenshot::primary_window()).observe(save_to_disk(path));
    }
}

