//! Developer helpers: F12 saves a screenshot to `screenshots/`.
//!
//! `TOUR=1 cargo run` visits the places that matter in the world sketch (the
//! spawn, the bosses, every named area, the moving platforms, then the whole
//! world from afar),
//! saves `screenshots/tour-*.webp` at each one and quits. Handy to check the
//! world after editing the sketch without playing through it.

use bevy::{
    prelude::*,
    render::view::screenshot::Screenshot,
};

use crate::map_export::save_lossless_webp;
use crate::{
    camera::{self, DevZoom},
    chameleon,
    debug_overlay::DebugInfo,
    level,
    player::{self, Player},
    terrain::Terrain,
};

pub struct DevPlugin;

impl Plugin for DevPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, screenshot_on_f12);
        crate::map_export::register(app);
        if std::env::var("HOLD").is_ok() || std::env::var("TAP").is_ok() {
            app.add_systems(PreUpdate, fake_input.after(bevy::input::InputSystems).before(crate::level::restart));
        }
        if std::env::var("START").is_ok() || std::env::var("EXIT_AFTER").is_ok() {
            app.add_systems(Update, start_and_exit.after(player::move_player));
        }
        if std::env::var("AIM").is_ok() {
            app.add_systems(Update, fake_aim.after(player::move_player).before(chameleon::update_anim_state));
        }
        if std::env::var("TOUR").is_ok() {
            app.add_systems(Update, tour.after(player::move_player).before(camera::follow_player));
        }
    }
}

/// F12: screenshot; with the F1 overlay on, its text goes in a .txt beside it.
fn screenshot_on_f12(mut commands: Commands, keys: Res<ButtonInput<KeyCode>>, debug: Res<DebugInfo>, mut count: Local<u32>) {
    if keys.just_pressed(KeyCode::F12) {
        let name = format!("screenshots/shot-{}-{}", std::process::id(), *count);
        *count += 1;
        if debug.on {
            let _ = std::fs::create_dir_all("screenshots");
            let _ = std::fs::write(format!("{name}.txt"), &debug.text);
        }
        info!("screenshot {name}.webp");
        commands.spawn(Screenshot::primary_window()).observe(save_lossless_webp(format!("{name}.webp")));
    }
}

/// `HOLD=d,shift` keeps keys down; `TAP=space@1.0,k@1.1` taps keys at those
/// seconds. Keys: a d w s space k shift f j g f1-f10 f12. For testing movement with `START`.
fn fake_input(time: Res<Time<Real>>, mut keys: ResMut<ButtonInput<KeyCode>>, mut tapped: Local<Vec<KeyCode>>) {
    let code = |name: &str| match name.trim() {
        "a" => Some(KeyCode::KeyA),
        "d" => Some(KeyCode::KeyD),
        "w" => Some(KeyCode::KeyW),
        "s" => Some(KeyCode::KeyS),
        "space" => Some(KeyCode::Space),
        "k" => Some(KeyCode::KeyK),
        "shift" => Some(KeyCode::ShiftLeft),
        "f" => Some(KeyCode::KeyF),
        "j" => Some(KeyCode::KeyJ),
        "g" => Some(KeyCode::KeyG),
        "f1" => Some(KeyCode::F1),
        "f2" => Some(KeyCode::F2),
        "f3" => Some(KeyCode::F3),
        "f4" => Some(KeyCode::F4),
        "f5" => Some(KeyCode::F5),
        "f6" => Some(KeyCode::F6),
        "f7" => Some(KeyCode::F7),
        "f8" => Some(KeyCode::F8),
        "f9" => Some(KeyCode::F9),
        "p" => Some(KeyCode::KeyP),
        "pause" => Some(KeyCode::Pause),
        "f10" => Some(KeyCode::F10),
        "f12" => Some(KeyCode::F12),
        _ => None,
    };
    for key in tapped.drain(..) {
        keys.release(key);
    }
    for name in std::env::var("HOLD").unwrap_or_default().split(',') {
        if let Some(key) = code(name) {
            keys.press(key);
        }
    }
    let (t, dt) = (time.elapsed_secs(), time.delta_secs());
    for tap in std::env::var("TAP").unwrap_or_default().split(',') {
        let Some((name, at)) = tap.split_once('@') else { continue };
        let (Some(key), Ok(at)) = (code(name), at.trim().parse::<f32>()) else { continue };
        if t >= at && t - dt < at {
            keys.press(key);
            tapped.push(key);
        }
    }
}

/// `AIM=dx,dy` (world units from the chameleon) stands in for the mouse, to
/// test aiming (head, eyes, slash, spit) without one.
fn fake_aim(player: Single<(&mut Player, &Transform)>) {
    let Some(offset) = std::env::var("AIM").ok().and_then(|s| {
        let (x, y) = s.split_once(',')?;
        Some(Vec2::new(x.trim().parse().ok()?, y.trim().parse().ok()?))
    }) else {
        return;
    };
    let (mut p, tf) = player.into_inner();
    let pos = tf.translation.truncate();
    p.aim_at = Some(pos + offset);
    p.aim = (pos + offset - chameleon::neck(pos, p.facing)).normalize_or(Vec2::X);
}

/// `START=x,y` (world units) drops the chameleon there; `EXIT_AFTER=seconds`
/// logs where it ended up, saves `screenshots/exit.webp` and quits. For testing movement.
fn start_and_exit(
    mut commands: Commands,
    time: Res<Time>,
    mut frame: Local<u32>,
    mut logged: Local<bool>,
    terrain: Res<Terrain>,
    player: Single<(&mut Player, &mut Transform)>,
    mut exit: MessageWriter<AppExit>,
) {
    let (mut p, mut tf) = player.into_inner();
    *frame += 1;
    if *frame == 2 {
        let start = std::env::var("START").ok().and_then(|s| {
            let (x, y) = s.split_once(',')?;
            Some(Vec2::new(x.trim().parse().ok()?, y.trim().parse().ok()?))
        });
        if let Some(at) = start {
            tf.translation.x = at.x;
            tf.translation.y = at.y;
            p.vel = Vec2::ZERO;
            p.forget_fall(at);
        }
    }
    let Some(limit) = std::env::var("EXIT_AFTER").ok().and_then(|s| s.parse::<f32>().ok()) else { return };
    let t = time.elapsed_secs();
    if t > limit && !*logged {
        *logged = true;
        let feet = tf.translation.truncate() - Vec2::Y * player::HALF.y;
        info!(
            "player at {:.0?}, velocity {:.0?}, grounded {}, slope {:.2?}, slide area {}",
            tf.translation.truncate(),
            p.vel,
            p.grounded,
            terrain.slope_at(feet),
            terrain.slides_at(feet)
        );
        commands.spawn(Screenshot::primary_window()).observe(save_lossless_webp("screenshots/exit.webp"));
    }
    if t > limit + 0.5 {
        exit.write(AppExit::Success);
    }
}

/// Frames spent at each stop (the camera glides there, then a screenshot).
const STOP_FRAMES: u32 = 70;

fn tour(
    mut commands: Commands,
    terrain: Res<Terrain>,
    mut frame: Local<u32>,
    mut zoom: ResMut<DevZoom>,
    player: Single<(&mut Player, &mut Transform)>,
    mut exit: MessageWriter<AppExit>,
) {
    let mut stops: Vec<(String, Vec2, f32)> = vec![("spawn".into(), player::spawn_point(&terrain), 1.0)];
    if let Some(p) = terrain.roach {
        stops.push(("gutter-king".into(), p + Vec2::Y * player::HALF.y, 1.0));
    }
    if let Some(p) = terrain.wasp {
        stops.push(("venom-mother".into(), p, 1.0));
    }
    let slug = |name: &str| name.split(|c: char| !c.is_alphanumeric()).filter(|w| !w.is_empty()).take(6).collect::<Vec<_>>().join("-");
    for a in &terrain.areas {
        stops.push((slug(&a.name), a.rect.center(), 1.6));
    }
    for f in &terrain.features {
        stops.push((slug(&f.name), f.rect.center(), 1.0));
    }
    for (kind, at) in level::pin_spots() {
        stops.push((format!("pin-{kind}"), at + Vec2::Y * 60.0, 1.0));
    }
    let b = terrain.bounds;
    let fit = (b.width() / 1280.0).max(b.height() / 720.0) * 1.1 / camera::ZOOM;
    stops.push(("overview".into(), b.center(), fit));

    let stop = (*frame / STOP_FRAMES) as usize;
    let t = *frame % STOP_FRAMES;
    *frame += 1;
    let Some((name, at, z)) = stops.get(stop) else {
        exit.write(AppExit::Success);
        return;
    };
    // Hold the chameleon there (the camera follows it).
    let (mut p, mut tf) = player.into_inner();
    p.vel = Vec2::ZERO;
    p.forget_fall(*at);
    tf.translation.x = at.x;
    tf.translation.y = at.y;
    zoom.0 = *z;
    if t == STOP_FRAMES - 5 {
        let path = format!("screenshots/tour-{stop:02}-{name}.webp");
        commands.spawn(Screenshot::primary_window()).observe(save_lossless_webp(path));
    }
}
