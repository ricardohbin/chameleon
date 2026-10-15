//! HUD: controls hint (powers show once earned, with an announcement), the
//! chameleon's health pips (with shadow health filling the empty ones from the
//! bottom), the boss health bar, pause (P / Pause) and the crosshair mouse
//! cursor while the mouse is captured.
//! UI is drawn by the color camera, so it keeps its colors.

use bevy::{
    asset::RenderAssetUsages,
    prelude::*,
    render::render_resource::{Extent3d, TextureDimension, TextureFormat},
    window::{CursorGrabMode, CursorIcon, CursorOptions, CustomCursor, CustomCursorImage, PrimaryWindow},
};

use crate::{
    boss::{Beaten, BossKind, BossStatus, Defeated},
    level::{self, Restart},
    player::{self, Player},
};

const PIP_FULL: Color = Color::srgb(0.78, 0.06, 0.05);
/// Pips in F3 invulnerability.
const PIP_GOD: Color = Color::srgb(0.95, 0.78, 0.2);
const PIP_EMPTY: Color = Color::srgba(0.2, 0.18, 0.17, 0.7);
/// Shadow health: promised by remains eaten, not health yet.
const PIP_SHADOW: Color = Color::srgba(0.42, 0.10, 0.09, 0.75);
/// The part of the next shadow tick already digested.
const PIP_FILLING: Color = Color::srgb(0.62, 0.12, 0.10);
/// The bar's "recent damage" chunk, which drains after the fill.
const BOSS_TRAIL: Color = Color::srgb(0.92, 0.90, 0.80);

pub struct HudPlugin;

impl Plugin for HudPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ScreenFlash>()
            .add_systems(Startup, spawn_hud)
            .add_systems(PreUpdate, (|mut r: MessageReader<Restart>, mut flash: ResMut<ScreenFlash>| {
                if r.read().count() > 0 {
                    flash.alpha = 0.0;
                }
            })
            .after(level::restart))
            .add_systems(PreUpdate, pause.after(level::restart))
            .add_systems(Update, (update_pips, update_swim_bar, update_boss_bar, update_flash, update_hint, announce, crosshair));
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

/// The controls line along the top.
#[derive(Component)]
struct Hint;

/// "Power earned" in the middle of the screen, fading out (seconds left).
#[derive(Component)]
struct Announcement(f32);

const ANNOUNCE_TIME: f32 = 4.0;

/// The crosshair cursor image.
#[derive(Resource)]
struct Crosshair(Handle<Image>);

/// Cursor image size in pixels (the hotspot is its center).
const CROSSHAIR: u32 = 25;

#[derive(Component)]
struct Pip(i32);

/// Inside an empty pip: how much shadow health is promised there.
#[derive(Component)]
struct PipShadow(i32);

/// Inside an empty pip: how much of it has filled.
#[derive(Component)]
struct PipFill(i32);

/// Swimming strength: a thin bar under the pips, shown in the water and while it refills.
#[derive(Component)]
struct SwimBar;

#[derive(Component)]
struct SwimFill;

#[derive(Component)]
struct BossBar;

#[derive(Component)]
struct BossName;

#[derive(Component)]
struct BossFill;

#[derive(Component)]
struct BossTrail;

#[derive(Component)]
struct PausedLabel;

/// P / Pause: freezes game time (`Time<Virtual>`; real-time things like the
/// hot reload keep going). While paused the game ignores everything but the
/// F keys, so nothing is buffered for the moment it resumes. F8 unpauses.
fn pause(
    mut restarted: MessageReader<Restart>,
    mut keys: ResMut<ButtonInput<KeyCode>>,
    mut mouse: ResMut<ButtonInput<MouseButton>>,
    mut time: ResMut<Time<Virtual>>,
    mut label: Single<&mut Visibility, With<PausedLabel>>,
) {
    if restarted.read().count() > 0 {
        time.unpause();
    } else if keys.any_just_pressed([KeyCode::KeyP, KeyCode::Pause]) {
        if time.is_paused() { time.unpause() } else { time.pause() }
    }
    **label = if time.is_paused() { Visibility::Inherited } else { Visibility::Hidden };
    if time.is_paused() {
        let held: Vec<KeyCode> = keys.get_pressed().copied().filter(|k| !is_function_key(*k)).collect();
        for key in held {
            keys.reset(key);
        }
        mouse.reset_all();
    }
}

fn is_function_key(key: KeyCode) -> bool {
    use KeyCode::*;
    matches!(key, F1 | F2 | F3 | F4 | F5 | F6 | F7 | F8 | F9 | F10 | F11 | F12 | Pause | KeyP)
}

fn spawn_hud(mut commands: Commands, mut images: ResMut<Assets<Image>>) {
    commands.insert_resource(Crosshair(images.add(crosshair_image())));
    // First, so the rest of the HUD draws over it.
    commands.spawn((
        FlashOverlay,
        Node { position_type: PositionType::Absolute, width: percent(100), height: percent(100), ..default() },
        BackgroundColor(Color::NONE),
    ));
    commands.spawn((
        Hint,
        Text::new(hint(&Defeated::default())),
        TextFont { font_size: FontSize::Px(18.0), ..default() },
        TextColor(Color::srgba(0.85, 0.82, 0.78, 0.7)),
        Node { position_type: PositionType::Absolute, top: px(12), left: px(14), ..default() },
    ));
    commands.spawn((
        Announcement(0.0),
        Text::new(""),
        TextFont { font_size: FontSize::Px(30.0), ..default() },
        TextColor(Color::NONE),
        TextLayout::justify(Justify::Center),
        Node { position_type: PositionType::Absolute, top: percent(22), width: percent(100), ..default() },
    ));


    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                width: percent(100),
                height: percent(100),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.35)),
            PausedLabel,
            Visibility::Hidden,
        ))
        .with_child((
            Text::new("PAUSED"),
            TextFont { font_size: FontSize::Px(48.0), ..default() },
            TextColor(Color::srgb(0.88, 0.85, 0.78)),
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
    let layer = |height| Node { position_type: PositionType::Absolute, bottom: px(0), width: percent(100), height: percent(height), ..default() };
    // Room for the ticks boss carcasses add (hidden until earned).
    for i in 0..player::MAX_HP + BossKind::ALL.len() as i32 {
        let pip = commands
            .spawn((
                Pip(i),
                Node { width: px(16), height: px(22), ..default() },
                BackgroundColor(PIP_FULL),
                UiTransform { rotation: Rot2::radians(0.25), ..default() },
                ChildOf(pips),
            ))
            .id();
        commands.spawn((PipShadow(i), layer(0), BackgroundColor(PIP_SHADOW), ChildOf(pip)));
        commands.spawn((PipFill(i), layer(0), BackgroundColor(PIP_FILLING), ChildOf(pip)));
    }

    // Swimming strength.
    let swim = commands
        .spawn((
            SwimBar,
            Node { position_type: PositionType::Absolute, top: px(72), left: px(16), width: px(104), height: px(6), ..default() },
            BackgroundColor(Color::srgba(0.05, 0.05, 0.05, 0.7)),
            Visibility::Hidden,
        ))
        .id();
    commands.spawn((
        SwimFill,
        Node { width: percent(100), height: percent(100), ..default() },
        BackgroundColor(Color::srgb(0.55, 0.82, 0.95)),
        ChildOf(swim),
    ));

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

fn update_swim_bar(
    player: Single<&Player>,
    mut bar: Single<&mut Visibility, With<SwimBar>>,
    mut fill: Single<(&mut Node, &mut BackgroundColor), With<SwimFill>>,
) {
    **bar = if player.wet > 0.0 || player.swim < 0.999 { Visibility::Inherited } else { Visibility::Hidden };
    let (node, color) = &mut *fill;
    node.width = percent(player.swim.clamp(0.0, 1.0) * 100.0);
    // Pale blue while fresh; red once the head will go under.
    let tired = Color::srgb(0.85, 0.15, 0.1);
    color.0 = Color::srgb(0.55, 0.82, 0.95).mix(&tired, ((1.0 - player.swim - 0.35) / 0.65).clamp(0.0, 1.0));
}

/// The controls: powers only once earned.
fn hint(defeated: &Defeated) -> String {
    let mut s = String::from("A/D: walk   Space/W: jump   Mouse: aim   F/Left click: slash   E: eat remains");
    if defeated.dash() {
        s.push_str("   Shift/K: dash");
    }
    if defeated.spit() {
        s.push_str("   G/Right click: spit");
    }
    if defeated.spin() {
        s.push_str("   Attack in the air: spin");
    }
    if defeated.climb() {
        s.push_str("   Walls: climb (W/S), Space: leap off");
    }
    s
}

fn update_hint(defeated: Res<Defeated>, mut text: Single<&mut Text, With<Hint>>) {
    if defeated.is_changed() {
        text.0 = hint(&defeated);
    }
}

/// A boss fell: name the power it gave, for a few seconds (real time: the
/// death's slow motion doesn't drag it).
fn announce(
    real: Res<Time<Real>>,
    mut beaten: MessageReader<Beaten>,
    label: Single<(&mut Announcement, &mut Text, &mut TextColor)>,
) {
    let (mut a, mut text, mut color) = label.into_inner();
    if let Some(Beaten(kind)) = beaten.read().last() {
        text.0 = format!("EARNED: {}", kind.power());
        a.0 = ANNOUNCE_TIME;
    }
    if a.0 <= 0.0 {
        return;
    }
    a.0 = (a.0 - real.delta_secs()).max(0.0);
    color.0 = Color::srgb(0.95, 0.90, 0.75).with_alpha((a.0 / 1.0).min(1.0));
}

/// A crosshair: four ticks around a gap and a dot in bone white, outlined in
/// black so it reads on any gray.
fn crosshair_image() -> Image {
    let n = CROSSHAIR as i32;
    let c = n / 2;
    let (gap, tick) = (3, 7);
    let ink = |x: i32, y: i32| {
        let (dx, dy) = ((x - c).abs(), (y - c).abs());
        let on_axis = |a: i32, b: i32| a == 0 && (gap..gap + tick).contains(&b);
        on_axis(dx, dy) || on_axis(dy, dx) || (dx == 0 && dy == 0)
    };
    let mut data = vec![0u8; (n * n * 4) as usize];
    for y in 0..n {
        for x in 0..n {
            let rgba = if ink(x, y) {
                [235, 230, 214, 255]
            } else if (-1..=1).any(|j| (-1..=1).any(|i| ink(x + i, y + j))) {
                [0, 0, 0, 190]
            } else {
                continue;
            };
            let k = ((y * n + x) * 4) as usize;
            data[k..k + 4].copy_from_slice(&rgba);
        }
    }
    Image::new(
        Extent3d { width: CROSSHAIR, height: CROSSHAIR, depth_or_array_layers: 1 },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8UnormSrgb,
        // The window reads the pixels on the main world side.
        RenderAssetUsages::all(),
    )
}

/// The mouse cursor is the crosshair while the mouse is captured, the usual
/// arrow otherwise.
fn crosshair(
    mut commands: Commands,
    crosshair: Res<Crosshair>,
    window: Single<(Entity, &CursorOptions, Option<&CursorIcon>), With<PrimaryWindow>>,
) {
    let (e, cursor, icon) = *window;
    let captured = cursor.grab_mode != CursorGrabMode::None;
    let showing = matches!(icon, Some(CursorIcon::Custom(_)));
    if captured && !showing {
        let c = (CROSSHAIR / 2) as u16;
        let image = CustomCursorImage { handle: crosshair.0.clone(), hotspot: (c, c), ..default() };
        commands.entity(e).insert(CursorIcon::Custom(CustomCursor::Image(image)));
    } else if !captured && showing {
        commands.entity(e).insert(CursorIcon::default());
    }
}

fn update_flash(real: Res<Time<Real>>, mut flash: ResMut<ScreenFlash>, mut overlay: Single<&mut BackgroundColor, With<FlashOverlay>>) {
    if flash.alpha <= 0.0 && overlay.0.alpha() == 0.0 {
        return;
    }
    overlay.0 = flash.color.with_alpha(flash.alpha.max(0.0));
    flash.alpha -= real.delta_secs() * 1.6;
}

fn update_pips(
    player: Single<&Player>,
    mut pips: Query<(&Pip, &mut BackgroundColor, &mut Node), (Without<PipShadow>, Without<PipFill>)>,
    mut shadows: Query<(&PipShadow, &mut Node), (Without<PipFill>, Without<Pip>)>,
    mut fills: Query<(&PipFill, &mut Node), (Without<PipShadow>, Without<Pip>)>,
) {
    for (pip, mut bg, mut node) in &mut pips {
        node.display = if pip.0 < player.max_hp { Display::Flex } else { Display::None };
        bg.0 = match () {
            _ if pip.0 >= player.hp => PIP_EMPTY,
            _ if player.god => PIP_GOD,
            _ => PIP_FULL,
        };
    }
    // Empty pips, from the first one: shadow promised there, and the one filling.
    let past_hp = |i: i32| (i - player.hp) as f32;
    for (s, mut node) in &mut shadows {
        let k = past_hp(s.0);
        let promised = if k < 0.0 { 0.0 } else { (player.shadow - k).clamp(0.0, 1.0) };
        node.height = percent(promised * 100.0);
    }
    for (f, mut node) in &mut fills {
        let filled = if past_hp(f.0) == 0.0 { player.digest } else { 0.0 };
        node.height = percent(filled * 100.0);
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
