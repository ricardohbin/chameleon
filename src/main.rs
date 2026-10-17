// Release builds on Windows are a GUI program: no console window opening beside the game (dev builds keep it for the logs).
#![cfg_attr(all(not(debug_assertions), target_os = "windows"), windows_subsystem = "windows")]

mod atmosphere;
mod boss;
mod camera;
mod chameleon;
mod controls;
mod debug_overlay;
#[cfg(not(target_arch = "wasm32"))]
mod dev;
mod dragonfly;
mod electric;
mod webs;
#[cfg(not(target_arch = "wasm32"))]
mod map_export;
mod features;
mod fumes;
mod frog;
mod grasshopper;
mod king_rat;
mod mantis;
mod crab;
mod beetle;
mod ending;
mod feeding;
mod gore;
mod hud;
mod level;
mod menu;
mod mob_art;
mod mobs;
mod outline;
#[cfg(all(debug_assertions, not(target_arch = "wasm32")))]
mod perf;
mod pill;
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
mod storage;
mod terrain;
mod terrain_art;
mod terrain_props;
mod unstable;
mod wasp;
mod water;

use bevy::prelude::*;
use bevy::window::{CursorGrabMode, CursorOptions, PrimaryWindow, WindowMode};

/// Bevy's built-in font, preserved before the game font replaces the default.
#[derive(Resource)]
pub struct BuiltinFont(pub Handle<Font>);

/// Every `Text` without an explicit font uses Bevy's default font asset: swap it for ours.
fn install_default_font(mut commands: Commands, mut fonts: ResMut<Assets<Font>>) {
    // Keep Bevy's built-in font reachable (the debug overlay uses it).
    if let Some(builtin) = fonts.get(AssetId::default()).cloned() {
        commands.insert_resource(BuiltinFont(fonts.add(builtin)));
    }
    let font = Font::from_bytes(include_bytes!("../assets/fonts/GhoreSerif.ttf").to_vec());
    fonts.insert(AssetId::default(), font).unwrap();
}

fn main() {
    // `cargo run -- bake` / `cargo run -- diff`: world sketch tools, no game.
    #[cfg(not(target_arch = "wasm32"))]
    if sketch_tool::run_cli() {
        return;
    }
    let mut app = App::new();
    #[cfg(all(debug_assertions, not(target_arch = "wasm32")))]
    app.add_plugins(perf::PerfPlugin);
    app
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Ghore, The Chameleon".into(),
                resolution: (1280, 720).into(),
                // Native: hidden until `set_window_icon` has installed the icon (the web canvas is always shown).
                visible: cfg!(target_arch = "wasm32"),
                // Web: draw into the page's `<canvas id="bevy">`, sized by its parent, and keep the
                // browser from scrolling/zooming on the game's keys.
                canvas: Some("#bevy".into()),
                fit_canvas_to_parent: true,
                prevent_default_event_handling: true,
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
            debug_overlay::DebugOverlayPlugin,
            hud::HudPlugin,
        ))
        .add_plugins((feeding::FeedingPlugin, save::SavePlugin, spit::SpitPlugin, regrowth::RegrowthPlugin, fumes::FumesPlugin, water::WaterPlugin, electric::ElectricPlugin, webs::WebPlugin, dragonfly::DragonflyPlugin, spider::SpiderPlugin, sound::SoundPlugin, centipede::CentipedePlugin, burrow::BurrowPlugin, frog::FrogPlugin, grasshopper::GrasshopperPlugin))
        .add_plugins((king_rat::KingRatPlugin, mantis::MantisPlugin, crab::CrabPlugin, beetle::BeetlePlugin, ending::EndingPlugin, pill::PillPlugin))
        .add_systems(
            PreUpdate,
            capture_mouse.after(bevy::input::InputSystems).after(menu::input).before(level::restart),
        )
        .add_systems(PreStartup, install_default_font)
        .add_systems(Update, toggle_fullscreen);
    #[cfg(not(target_arch = "wasm32"))]
    app.add_plugins(dev::DevPlugin).add_systems(Update, set_window_icon);
    app.run();
}

/// Click inside the game to confine the cursor; Escape, Pause or losing focus releases it.
/// The menu and pause keep it free, and the click that closes them doesn't recapture it.
/// While confined the cursor is a crosshair (`hud.rs`).
fn capture_mouse(
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    menu: Res<menu::Menu>,
    settings: Res<settings::Settings>,
    mut window: Single<(&Window, &mut CursorOptions), With<PrimaryWindow>>,
    time: Res<Time<Virtual>>,
    mut was_idle: Local<bool>,
) {
    let (window, cursor) = &mut *window;
    // The browser can't confine the pointer to the canvas (only lock it): leave it free.
    if cfg!(target_arch = "wasm32") {
        return;
    }
    // The menu and the pause screen both need the mouse free.
    let idle = menu.open || time.is_paused();
    let menu_was_open = std::mem::replace(&mut *was_idle, idle);
    // Only the mouse scheme aims with it.
    if !window.focused
        || idle
        || settings.scheme != settings::Scheme::MouseKeyboard
        || keys.any_just_pressed([KeyCode::Escape, KeyCode::KeyP, KeyCode::Pause])
    {
        cursor.grab_mode = CursorGrabMode::None;
    } else if !menu_was_open && window.cursor_position().is_some() && mouse.get_just_pressed().next().is_some() {
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

#[cfg(not(target_arch = "wasm32"))]
/// The title-bar / taskbar icon: a close-up of the chameleon's eye (`assets/icon.png`, embedded).
fn set_window_icon(world: &mut World, mut done: Local<bool>) {
    // Exclusive system: `WINIT_WINDOWS` is thread-local, so this must run on the main thread.
    if *done {
        return;
    }
    let Some(window) = world.query_filtered::<Entity, With<PrimaryWindow>>().iter(world).next() else {
        return;
    };
    // A bad icon must not keep the window hidden: it is shown either way.
    let icon = image::load_from_memory_with_format(include_bytes!("../assets/icon.png"), image::ImageFormat::Png)
        .ok()
        .map(|img| img.into_rgba8())
        .and_then(|img| winit::window::Icon::from_rgba(img.to_vec(), img.width(), img.height()).ok());
    bevy::winit::WINIT_WINDOWS.with_borrow(|windows| {
        // The OS window is created after `Startup`: keep trying until it exists.
        if let Some(w) = windows.get_window(window) {
            if let Some(icon) = icon {
                // The title-bar icon is the small one; the taskbar and Alt+Tab use the big one.
                #[cfg(target_os = "windows")]
                winit::platform::windows::WindowExtWindows::set_taskbar_icon(&**w, Some(icon.clone()));
                w.set_window_icon(Some(icon));
            }
            // The window starts hidden (see `main`): Windows builds its taskbar button from the
            // icon the window has when it first appears, so show it only once the icon is set.
            #[cfg(target_os = "windows")]
            class_icons::set(w);
            w.set_visible(true);
            *done = true;
        }
    });
}

/// Windows only: also put the exe's embedded icon (resource 1, see `build.rs`) on the window *class*.
/// The taskbar button can ignore `WM_SETICON` (winit#4170) and fall back to the class icon.
#[cfg(target_os = "windows")]
mod class_icons {
    use std::ffi::c_void;
    use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};

    #[link(name = "user32")]
    unsafe extern "system" {
        fn LoadImageW(inst: *mut c_void, name: *const u16, kind: u32, cx: i32, cy: i32, flags: u32) -> *mut c_void;
        fn GetSystemMetrics(index: i32) -> i32;
        fn SetClassLongPtrW(hwnd: *mut c_void, index: i32, value: isize) -> usize;
        fn SendMessageW(hwnd: *mut c_void, msg: u32, wparam: usize, lparam: isize) -> isize;
    }
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetModuleHandleW(name: *const u16) -> *mut c_void;
    }

    pub fn set(window: &winit::window::Window) {
        let Ok(handle) = window.window_handle() else { return };
        let RawWindowHandle::Win32(win) = handle.as_raw() else { return };
        let hwnd = win.hwnd.get() as *mut c_void;
        const IMAGE_ICON: u32 = 1;
        const GCLP_HICON: i32 = -14;
        const GCLP_HICONSM: i32 = -34;
        const WM_SETICON: u32 = 0x0080;
        const SM_CXICON: i32 = 11;
        const SM_CYICON: i32 = 12;
        const SM_CXSMICON: i32 = 49;
        const SM_CYSMICON: i32 = 50;
        // SAFETY: plain Win32 calls on this process's own window and its own embedded icon resource.
        unsafe {
            let inst = GetModuleHandleW(std::ptr::null());
            let load = |cx, cy| LoadImageW(inst, 1 as *const u16, IMAGE_ICON, GetSystemMetrics(cx), GetSystemMetrics(cy), 0);
            let (big, small) = (load(SM_CXICON, SM_CYICON), load(SM_CXSMICON, SM_CYSMICON));
            if !big.is_null() {
                SetClassLongPtrW(hwnd, GCLP_HICON, big as isize);
                SendMessageW(hwnd, WM_SETICON, 1, big as isize);
            }
            if !small.is_null() {
                SetClassLongPtrW(hwnd, GCLP_HICONSM, small as isize);
                SendMessageW(hwnd, WM_SETICON, 0, small as isize);
            }
        }
    }
}
