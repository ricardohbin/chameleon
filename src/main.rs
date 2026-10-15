mod atmosphere;
mod boss;
mod camera;
mod chameleon;
mod debug_overlay;
mod dev;
mod dragonfly;
mod electric;
mod webs;
mod map_export;
mod features;
mod fumes;
mod feeding;
mod gore;
mod hud;
mod level;
mod mob_art;
mod mobs;
mod outline;
mod player;
mod regrowth;
mod roach;
mod save;
mod shapes;
mod sketch;
mod sketch_tool;
mod spit;
mod terrain;
mod terrain_art;
mod terrain_props;
mod unstable;
mod wasp;
mod water;

use bevy::prelude::*;
use bevy::window::{CursorGrabMode, CursorOptions, MonitorSelection, PrimaryWindow, WindowMode};

fn main() {
    // `cargo run -- bake` / `cargo run -- diff`: world sketch tools, no game.
    if sketch_tool::run_cli() {
        return;
    }
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
            features::FeaturesPlugin,
            unstable::UnstablePlugin,
            player::PlayerPlugin,
            chameleon::ChameleonPlugin,
            gore::GorePlugin,
            boss::BossPlugin,
            wasp::WaspPlugin,
            roach::RoachPlugin,
            mobs::MobPlugin,
            camera::CameraPlugin,
            dev::DevPlugin,
            debug_overlay::DebugOverlayPlugin,
            hud::HudPlugin,
        ))
        .add_plugins((feeding::FeedingPlugin, save::SavePlugin, spit::SpitPlugin, regrowth::RegrowthPlugin, fumes::FumesPlugin, water::WaterPlugin, electric::ElectricPlugin, webs::WebPlugin, dragonfly::DragonflyPlugin))
        .add_systems(
            PreUpdate,
            capture_mouse.after(bevy::input::InputSystems).before(level::restart),
        )
        .add_systems(Update, toggle_fullscreen)
        .run();
}

/// Click inside the game to confine the cursor; Escape or losing focus releases it.
/// While confined the cursor is a crosshair (`hud.rs`).
fn capture_mouse(
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    mut window: Single<(&Window, &mut CursorOptions), With<PrimaryWindow>>,
) {
    let (window, cursor) = &mut *window;
    if !window.focused || keys.just_pressed(KeyCode::Escape) {
        cursor.grab_mode = CursorGrabMode::None;
    } else if window.cursor_position().is_some() && mouse.get_just_pressed().next().is_some() {
        cursor.grab_mode = CursorGrabMode::Confined;
    }
}

/// Alt+Enter or F11 toggles borderless fullscreen.
fn toggle_fullscreen(
    keys: Res<ButtonInput<KeyCode>>,
    mut window: Single<&mut Window, With<PrimaryWindow>>,
) {
    let alt_enter = keys.just_pressed(KeyCode::Enter)
        && (keys.pressed(KeyCode::AltLeft) || keys.pressed(KeyCode::AltRight));
    if alt_enter || keys.just_pressed(KeyCode::F11) {
        window.mode = match window.mode {
            WindowMode::Windowed => WindowMode::BorderlessFullscreen(MonitorSelection::Current),
            _ => WindowMode::Windowed,
        };
    }
}
