//! HUD: controls hint, the chameleon's health pips and the boss health bar.
//! UI is drawn by the color camera, so it keeps its colors.

use bevy::prelude::*;

use crate::{
    boss::BossStatus,
    player::{self, Player},
};

const PIP_FULL: Color = Color::srgb(0.78, 0.06, 0.05);
const PIP_EMPTY: Color = Color::srgba(0.2, 0.18, 0.17, 0.7);
/// The bar's "recent damage" chunk, which drains after the fill.
const BOSS_TRAIL: Color = Color::srgb(0.92, 0.90, 0.80);

pub struct HudPlugin;

impl Plugin for HudPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ScreenFlash>()
            .add_systems(Startup, spawn_hud)
            .add_systems(Update, (update_pips, update_boss_bar, update_flash));
    }
}

/// A full-screen color flash that fades out (in real time, so slow motion doesn't drag it).
#[derive(Resource, Default)]
pub struct ScreenFlash {
    color: Color,
    alpha: f32,
}

impl ScreenFlash {
    pub fn flash(&mut self, color: Color, alpha: f32) {
        self.color = color;
        self.alpha = alpha;
    }
}

#[derive(Component)]
struct FlashOverlay;

#[derive(Component)]
struct Pip(i32);

#[derive(Component)]
struct BossBar;

#[derive(Component)]
struct BossName;

#[derive(Component)]
struct BossFill;

#[derive(Component)]
struct BossTrail;

fn spawn_hud(mut commands: Commands) {
    // First, so the rest of the HUD draws over it.
    commands.spawn((
        FlashOverlay,
        Node { position_type: PositionType::Absolute, width: percent(100), height: percent(100), ..default() },
        BackgroundColor(Color::NONE),
    ));
    commands.spawn((
        Text::new("A/D or Arrows: walk    Space/W/Up: jump    F/Left click: slash"),
        TextFont { font_size: FontSize::Px(18.0), ..default() },
        TextColor(Color::srgba(0.85, 0.82, 0.78, 0.7)),
        Node { position_type: PositionType::Absolute, top: px(12), left: px(14), ..default() },
    ));

    // Health: one slanted red shard per hit point.
    let pips = commands
        .spawn(Node {
            position_type: PositionType::Absolute,
            top: px(44),
            left: px(16),
            column_gap: px(6),
            ..default()
        })
        .id();
    for i in 0..player::MAX_HP {
        commands.spawn((
            Pip(i),
            Node { width: px(16), height: px(22), ..default() },
            BackgroundColor(PIP_FULL),
            UiTransform { rotation: Rot2::radians(0.25), ..default() },
            ChildOf(pips),
        ));
    }

    // Boss bar: name over a long bar at the bottom, hidden until the fight.
    let bar = commands
        .spawn((
            BossBar,
            Node {
                position_type: PositionType::Absolute,
                bottom: px(28),
                left: percent(20),
                width: percent(60),
                flex_direction: FlexDirection::Column,
                row_gap: px(6),
                ..default()
            },
            Visibility::Hidden,
        ))
        .id();
    commands.spawn((
        BossName,
        Text::new(""),
        TextFont { font_size: FontSize::Px(20.0), ..default() },
        TextColor(Color::srgb(0.88, 0.85, 0.78)),
        ChildOf(bar),
    ));
    let track = commands
        .spawn((
            Node { width: percent(100), height: px(12), ..default() },
            BackgroundColor(Color::srgba(0.05, 0.05, 0.05, 0.85)),
            ChildOf(bar),
        ))
        .id();
    let fill = || Node { position_type: PositionType::Absolute, width: percent(100), height: percent(100), ..default() };
    commands.spawn((BossTrail, fill(), BackgroundColor(BOSS_TRAIL), ChildOf(track)));
    commands.spawn((BossFill, fill(), BackgroundColor(Color::WHITE), ChildOf(track)));
}

fn update_flash(real: Res<Time<Real>>, mut flash: ResMut<ScreenFlash>, mut overlay: Single<&mut BackgroundColor, With<FlashOverlay>>) {
    if flash.alpha <= 0.0 && overlay.0.alpha() == 0.0 {
        return;
    }
    overlay.0 = flash.color.with_alpha(flash.alpha.max(0.0));
    flash.alpha -= real.delta_secs() * 1.6;
}

fn update_pips(player: Single<&Player>, mut pips: Query<(&Pip, &mut BackgroundColor)>) {
    for (pip, mut bg) in &mut pips {
        bg.0 = if pip.0 < player.hp { PIP_FULL } else { PIP_EMPTY };
    }
}

fn update_boss_bar(
    time: Res<Time>,
    status: Res<BossStatus>,
    mut bar: Single<&mut Visibility, With<BossBar>>,
    mut name: Single<&mut Text, With<BossName>>,
    mut fill: Single<(&mut Node, &mut BackgroundColor), (With<BossFill>, Without<BossTrail>)>,
    mut trail: Single<&mut Node, (With<BossTrail>, Without<BossFill>)>,
    mut trail_width: Local<f32>,
) {
    **bar = if status.engaged { Visibility::Inherited } else { Visibility::Hidden };
    let health = status.health * 100.0;
    if name.0 != status.name {
        name.0 = status.name.into();
    }
    // The bar takes the color of the boss (the one the chameleon will earn).
    fill.0.width = percent(health);
    fill.1.0 = status.color;
    // The trail lags behind, then drains (it jumps up on a heal).
    *trail_width = if health > *trail_width { health } else { *trail_width - (*trail_width - health) * (1.0 - (-3.0 * time.delta_secs()).exp()) };
    trail.width = percent(*trail_width);
}
