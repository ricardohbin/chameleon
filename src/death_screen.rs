//! The death screen: after dying, the view fades to black and tells what dealt
//! the killing blow (`Player::cause`). The respawn (or the reload of the last
//! save) happens behind it; once it is done the game holds still, the screen
//! stays up for a second, and any key brings the chameleon back.

use bevy::{input::InputSystems, prelude::*};

use crate::player::Player;

/// Seconds dead before the fade to black begins (the body is left to lie there a moment first).
const FADE_FROM: f32 = 1.4;
/// Seconds of fade (it ends right when the respawn or reload kicks in).
const FADE_TIME: f32 = 1.4;
/// How long the message holds before a key is taken.
const HOLD: f32 = 1.0;
/// Fade out when the key comes.
const LEAVE_TIME: f32 = 0.5;

pub struct DeathScreenPlugin;

impl Plugin for DeathScreenPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<DeathScreen>()
            .add_systems(Startup, spawn)
            .add_systems(PreUpdate, read_input.after(InputSystems))
            .add_systems(Update, update);
    }
}

#[derive(Default, PartialEq, Eq, Clone, Copy)]
enum Phase {
    #[default]
    Idle,
    /// Black (fading in while dead, then waiting for the key).
    Dark,
    /// The key came: fading back to the game.
    Leaving,
}

#[derive(Resource, Default)]
struct DeathScreen {
    phase: Phase,
    /// How black it is, 0..1.
    alpha: f32,
    /// Seconds spent fully black.
    shown: f32,
    /// A key was pressed this frame (`read_input`).
    key: bool,
    /// Game time is paused by this screen.
    paused: bool,
    /// What killed the chameleon (kept: a reload may rebuild the player).
    cause: &'static str,
}

#[derive(Component)]
struct Backdrop;

/// A line of text: its color and how much of the screen's text opacity it takes.
#[derive(Component)]
struct Line(Color, Kind);

#[derive(Clone, Copy, PartialEq)]
enum Kind {
    Title,
    Cause,
    Hint,
}

fn spawn(mut commands: Commands) {
    let line = |text: &str, size: f32, color: Color, kind: Kind| {
        (Text::new(text), TextFont { font_size: FontSize::Px(size), ..default() }, TextColor(color.with_alpha(0.0)), TextLayout::justify(Justify::Center), Line(color, kind))
    };
    commands
        .spawn((
            Backdrop,
            Node {
                position_type: PositionType::Absolute,
                width: percent(100),
                height: percent(100),
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                row_gap: px(26),
                ..default()
            },
            BackgroundColor(Color::BLACK.with_alpha(0.0)),
            GlobalZIndex(90),
            Visibility::Hidden,
        ))
        .with_children(|root| {
            root.spawn(line("YOU DIED", 72.0, Color::srgb(0.72, 0.1, 0.1), Kind::Title));
            root.spawn(line("", 34.0, Color::srgb(0.88, 0.86, 0.82), Kind::Cause));
            root.spawn((line("press any key", 22.0, Color::srgb(0.55, 0.55, 0.55), Kind::Hint), Node { margin: UiRect::top(px(40)), ..default() }));
        });
}

/// Any key or mouse button (but not the pause and menu ones, which do their own thing).
fn read_input(mut s: ResMut<DeathScreen>, keys: Res<ButtonInput<KeyCode>>, mouse: Res<ButtonInput<MouseButton>>) {
    let key = keys.get_just_pressed().any(|k| !matches!(k, KeyCode::KeyP | KeyCode::Pause)) || mouse.get_just_pressed().next().is_some();
    s.key = key;
}

fn update(
    real: Res<Time<Real>>,
    mut game: ResMut<Time<Virtual>>,
    mut s: ResMut<DeathScreen>,
    player: Single<&Player>,
    mut backdrop: Single<(&mut BackgroundColor, &mut Visibility), With<Backdrop>>,
    mut lines: Query<(&Line, &mut TextColor, &mut Text)>,
) {
    let dt = real.delta_secs();
    let dead = player.dead;
    let key = std::mem::take(&mut s.key);
    match s.phase {
        Phase::Idle => {
            if dead.is_some_and(|t| t > FADE_FROM) {
                s.phase = Phase::Dark;
                s.alpha = 0.0;
                s.shown = 0.0;
            }
        }
        Phase::Dark => {
            if let Some(t) = dead {
                s.cause = player.cause;
                s.alpha = ((t - FADE_FROM) / FADE_TIME).clamp(0.0, 1.0);
            } else if s.alpha < 0.5 {
                // Back on its feet some other way (a restart): nothing to wait for.
                s.phase = Phase::Leaving;
            } else {
                // The respawn lands in the last frames of the fade: finish it as black.
                s.alpha = 1.0;
                // Respawned (or reloaded) behind the black: hold the game still for the key.
                if !s.paused && !game.is_paused() {
                    game.pause();
                    s.paused = true;
                } else if s.paused && !game.is_paused() {
                    // Someone else resumed it (a restart).
                    s.paused = false;
                    s.phase = Phase::Leaving;
                }
                if s.shown >= HOLD && key {
                    s.phase = Phase::Leaving;
                }
            }
            if s.alpha >= 1.0 {
                s.shown += dt;
            }
        }
        Phase::Leaving => {
            if s.paused {
                game.unpause();
                s.paused = false;
            }
            s.alpha = (s.alpha - dt / LEAVE_TIME).max(0.0);
            if s.alpha <= 0.0 {
                s.phase = Phase::Idle;
            }
        }
    }

    let (bg, visibility) = &mut *backdrop;
    bg.0 = Color::BLACK.with_alpha(s.alpha);
    **visibility = if s.phase == Phase::Idle { Visibility::Hidden } else { Visibility::Inherited };
    let text = if s.phase == Phase::Leaving { s.alpha } else { (s.shown / 0.6).min(1.0) };
    let blink = 0.55 + 0.45 * (real.elapsed_secs() * 3.0).sin();
    for (line, mut color, mut content) in &mut lines {
        let a = match line.1 {
            Kind::Title | Kind::Cause => text,
            Kind::Hint if s.shown >= HOLD || s.phase == Phase::Leaving => text * blink,
            Kind::Hint => 0.0,
        };
        color.0 = line.0.with_alpha(a);
        if line.1 == Kind::Cause && s.phase == Phase::Dark {
            let said = if s.cause.is_empty() { "Killed" } else { s.cause };
            if content.0 != said {
                content.0 = said.into();
            }
        }
    }
}
