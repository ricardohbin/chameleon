mod atmosphere;
mod boss;
mod camera;
mod chameleon;
mod controls;
mod debug_overlay;
mod dev;
mod dragonfly;
mod electric;
mod webs;
mod map_export;
mod features;
mod fumes;
mod frog;
mod grasshopper;
mod king_rat;
mod feeding;
mod gore;
mod hud;
mod level;
mod menu;
mod mob_art;
mod mobs;
mod outline;
#[cfg(debug_assertions)]
mod perf;
mod player;
mod regrowth;
mod roach;
mod save;
mod shapes;
mod sound;
mod settings;
mod sketch;
mod sketch_tool;
mod burrow;
mod centipede;
mod spider;
mod spit;
mod terrain;
mod terrain_art;
mod terrain_props;
mod unstable;
mod wasp;
mod water;

use bevy::prelude::*;
use bevy::window::{CursorGrabMode, CursorOptions, PrimaryWindow, WindowMode};

/// Every `Text` without an explicit font uses Bevy's default font asset: swap it for ours.
fn install_default_font(mut fonts: ResMut<Assets<Font>>) {
    let font = Font::from_bytes(include_bytes!("../assets/fonts/GhoreSerif.ttf").to_vec());
    fonts.insert(AssetId::default(), font).unwrap();
}

fn main() {
    // `cargo run -- bake` / `cargo run -- diff`: world sketch tools, no game.
    if sketch_tool::run_cli() {
        return;
    }
    let mut app = App::new();
    #[cfg(debug_assertions)]
    app.add_plugins(perf::PerfPlugin);
    app
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Ghore, The Chameleon".into(),
                resolution: (1280, 720).into(),
                ..default()
            }),
            ..default()
        }))
        .add_plugins((settings::SettingsPlugin, menu::MenuPlugin, controls::ControlsPlugin))
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
        .add_plugins((feeding::FeedingPlugin, save::SavePlugin, spit::SpitPlugin, regrowth::RegrowthPlugin, fumes::FumesPlugin, water::WaterPlugin, electric::ElectricPlugin, webs::WebPlugin, dragonfly::DragonflyPlugin, spider::SpiderPlugin, sound::SoundPlugin, centipede::CentipedePlugin, burrow::BurrowPlugin, frog::FrogPlugin, grasshopper::GrasshopperPlugin))
        .add_plugins(king_rat::KingRatPlugin)
        .add_systems(
            PreUpdate,
            capture_mouse.after(bevy::input::InputSystems).after(menu::input).before(level::restart),
        )
        .add_systems(PreStartup, install_default_font)
        .add_systems(Update, toggle_fullscreen)
        .add_systems(Update, set_window_icon)
        .run();
}

/// Click inside the game to confine the cursor; Escape or losing focus releases it.
/// While confined the cursor is a crosshair (`hud.rs`).
fn capture_mouse(
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    menu: Res<menu::Menu>,
    settings: Res<settings::Settings>,
    mut window: Single<(&Window, &mut CursorOptions), With<PrimaryWindow>>,
) {
    let (window, cursor) = &mut *window;
    // Only the mouse scheme aims with it; the menu needs it free.
    if !window.focused || menu.open || settings.scheme != settings::Scheme::MouseKeyboard || keys.just_pressed(KeyCode::Escape) {
        cursor.grab_mode = CursorGrabMode::None;
    } else if window.cursor_position().is_some() && mouse.get_just_pressed().next().is_some() {
        cursor.grab_mode = CursorGrabMode::Confined;
    }
}

/// Alt+Enter or F11 toggles borderless fullscreen.
fn toggle_fullscreen(
    keys: Res<ButtonInput<KeyCode>>,
    window: Single<&Window, With<PrimaryWindow>>,
    mut settings: ResMut<settings::Settings>,
    contrast: Res<terrain_art::Contrast>,
) {
    let alt_enter = keys.just_pressed(KeyCode::Enter)
        && (keys.pressed(KeyCode::AltLeft) || keys.pressed(KeyCode::AltRight));
    if alt_enter || keys.just_pressed(KeyCode::F11) {
        settings.fullscreen = window.mode == WindowMode::Windowed;
        settings::store(&settings, contrast.0);
    }
}

/// The title-bar / taskbar icon: a close-up of the chameleon's eye (`assets/icon.png`, embedded).
fn set_window_icon(world: &mut World, mut done: Local<bool>) {
    // Exclusive system: `WINIT_WINDOWS` is thread-local, so this must run on the main thread.
    if *done {
        return;
    }
    let Some(window) = world.query_filtered::<Entity, With<PrimaryWindow>>().iter(world).next() else {
        return;
    };
    let Ok(img) = image::load_from_memory_with_format(include_bytes!("../assets/icon.png"), image::ImageFormat::Png) else {
        return;
    };
    let img = img.into_rgba8();
    let Ok(icon) = winit::window::Icon::from_rgba(img.to_vec(), img.width(), img.height()) else {
        return;
    };
    bevy::winit::WINIT_WINDOWS.with_borrow(|windows| {
        // The OS window is created after `Startup`: keep trying until it exists.
        if let Some(w) = windows.get_window(window) {
            w.set_window_icon(Some(icon));
            *done = true;
        }
    });
}
