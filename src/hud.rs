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
    pill::{self, prism},
    player::{self, God, Player},
    settings::{Scheme, Settings},
};

const PIP_FULL: Color = Color::srgb(0.78, 0.06, 0.05);
/// Pips in F3 invulnerability: molten gold, breathing toward white-hot, in a halo.
const PIP_GOD: Color = Color::srgb(0.95, 0.70, 0.12);
const PIP_GOD_HOT: Color = Color::srgb(1.0, 0.97, 0.7);
const PIP_GOD_HALO: Color = Color::srgb(1.0, 0.85, 0.3);
/// Pips in invulnerability + touch of death: hellfire flickering from blood to white-hot, ringed in crimson.
const PIP_SMITE: Color = Color::srgb(0.6, 0.0, 0.02);
const PIP_SMITE_HOT: Color = Color::srgb(1.0, 0.55, 0.3);
const PIP_SMITE_HALO: Color = Color::srgb(1.0, 0.1, 0.05);
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
            .add_systems(Update, (update_pips, update_swim_bar, update_pill_icons, update_boss_bar, update_flash, update_hint, announce, crosshair));
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

/// Key hints, bottom right: a row per action, its name (outlined) left of the key (white on black).
#[derive(Component)]
struct HintRow(usize);

#[derive(Component)]
struct HintLabel(usize);

#[derive(Component)]
struct HintKey(usize);

const HINT_ROWS: usize = 20;

/// The look of every piece of HUD text that sits on a background: white on black.
pub const CHIP: Color = Color::srgba(0.0, 0.0, 0.0, 0.88);
pub const CHIP_INK: Color = Color::WHITE;

pub fn chip_pad() -> UiRect {
    UiRect::axes(px(12), px(5))
}

/// "Power earned" in the middle of the screen, fading out (seconds left).
#[derive(Component)]
struct Announcement(f32);

const ANNOUNCE_TIME: f32 = 4.0;

/// The crosshair cursor image.
#[derive(Resource)]
struct Crosshair(Handle<Image>);

/// Cursor image size in pixels (the hotspot is its center).
const CROSSHAIR: u32 = 31;

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

/// A power pill carried: the icons over the health.
#[derive(Component)]
struct PillIcon(u32);

/// The shine on a pill icon.
#[derive(Component)]
struct PillShine;

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
    menu: Res<crate::menu::Menu>,
    mut label: Single<&mut Visibility, With<PausedLabel>>,
) {
    if restarted.read().count() > 0 {
        time.unpause();
    } else if keys.any_just_pressed([KeyCode::KeyP, KeyCode::Pause]) {
        if time.is_paused() { time.unpause() } else { time.pause() }
    }
    **label = if time.is_paused() && !menu.open { Visibility::Inherited } else { Visibility::Hidden };
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
    let panel = commands
        .spawn(Node {
            position_type: PositionType::Absolute,
            right: px(24),
            bottom: px(24),
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::FlexEnd,
            row_gap: px(6),
            ..default()
        })
        .id();
    for i in 0..HINT_ROWS {
        let row = commands
            .spawn((HintRow(i), Node { align_items: AlignItems::Center, column_gap: px(10), ..default() }, ChildOf(panel)))
            .id();
        // The action: white, with a black outline made of copies around it.
        let label = commands.spawn((Node::default(), ChildOf(row))).id();
        let font = TextFont { font_size: FontSize::Px(20.0), ..default() };
        for (dx, dy) in [(-2.0, 0.0), (2.0, 0.0), (0.0, -2.0), (0.0, 2.0), (-1.5, -1.5), (1.5, -1.5), (-1.5, 1.5), (1.5, 1.5)] {
            commands.spawn((
                HintLabel(i),
                Text::new(""),
                font.clone(),
                TextColor(Color::BLACK),
                Node { position_type: PositionType::Absolute, left: px(dx), top: px(dy), ..default() },
                ChildOf(label),
            ));
        }
        commands.spawn((HintLabel(i), Text::new(""), font, TextColor(CHIP_INK), ChildOf(label)));
        commands.spawn((
            HintKey(i),
            Text::new(""),
            TextFont { font_size: FontSize::Px(18.0), ..default() },
            TextColor(CHIP_INK),
            BackgroundColor(CHIP),
            Node { padding: chip_pad(), ..default() },
            ChildOf(row),
        ));
    }
    commands.spawn((
        Announcement(0.0),
        Text::new(""),
        TextFont { font_size: FontSize::Px(30.0), ..default() },
        TextColor(Color::NONE),
        BackgroundColor(Color::NONE),
        TextLayout::justify(Justify::Center),
        Node { position_type: PositionType::Absolute, top: percent(22), left: percent(50), padding: UiRect::axes(px(20), px(8)), ..default() },
        UiTransform { translation: Val2::percent(-50.0, 0.0), ..default() },
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
            PausedLabel,
            Visibility::Hidden,
        ))
        .with_child((
            Text::new("PAUSED"),
            TextFont { font_size: FontSize::Px(48.0), ..default() },
            TextColor(CHIP_INK),
            BackgroundColor(CHIP),
            Node { padding: UiRect::axes(px(28), px(10)), ..default() },
        ));

    // Health: one slanted red shard per hit point.
    let pips = commands
        .spawn(Node {
            position_type: PositionType::Absolute,
            bottom: px(28),
            left: px(28),
            column_gap: px(9),
            ..default()
        })
        .id();
    let layer = |height| Node { position_type: PositionType::Absolute, bottom: px(0), width: percent(100), height: percent(height), ..default() };
    // Room for the ticks boss carcasses add (hidden until earned).
    for i in 0..player::MAX_HP + BossKind::ALL.len() as i32 {
        let pip = commands
            .spawn((
                Pip(i),
                Node { width: px(26), height: px(36), ..default() },
                BackgroundColor(PIP_FULL),
                Outline { width: px(2), offset: px(1), color: Color::NONE },
                UiTransform { rotation: Rot2::radians(0.25), ..default() },
                ChildOf(pips),
            ))
            .id();
        commands.spawn((PipShadow(i), layer(0), BackgroundColor(PIP_SHADOW), ChildOf(pip)));
        commands.spawn((PipFill(i), layer(0), BackgroundColor(PIP_FILLING), ChildOf(pip)));
    }

    // Power pills carried: little prismatic capsules over the health.
    let icons = commands
        .spawn(Node { position_type: PositionType::Absolute, bottom: px(94), left: px(28), column_gap: px(10), ..default() })
        .id();
    for i in 0..pill::MAX_STOCK {
        let icon = commands
            .spawn((
                PillIcon(i),
                Node { width: px(34), height: px(18), border_radius: BorderRadius::MAX, ..default() },
                BackgroundColor(Color::WHITE),
                Outline { width: px(2), offset: px(1), color: Color::NONE },
                UiTransform { rotation: Rot2::radians(-0.5), ..default() },
                ChildOf(icons),
            ))
            .id();
        commands.spawn((
            PillShine,
            Node { position_type: PositionType::Absolute, left: px(5), top: px(3), width: px(14), height: px(4), border_radius: BorderRadius::MAX, ..default() },
            BackgroundColor(Color::srgba(1.0, 1.0, 1.0, 0.6)),
            ChildOf(icon),
        ));
    }

    // Swimming strength.
    let swim = commands
        .spawn((
            SwimBar,
            Node { position_type: PositionType::Absolute, bottom: px(76), left: px(28), width: px(150), height: px(8), ..default() },
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
                top: px(22),
                left: percent(25),
                width: percent(50),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
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
        TextColor(CHIP_INK),
        BackgroundColor(CHIP),
        Node { padding: chip_pad(), ..default() },
        ChildOf(bar),
    ));
    let track = commands
        .spawn((
            Node { width: percent(100), height: px(14), ..default() },
            BackgroundColor(CHIP),
            Outline { width: px(3), offset: px(0), color: CHIP },
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

/// The controls, as (action, key): powers only once earned.
fn hints(defeated: &Defeated, scheme: Scheme, pills: bool) -> Vec<(&'static str, &'static str)> {
    let mouse = scheme == Scheme::MouseKeyboard;
    let mut v = vec![("Move", "A D"), ("Jump", "Space")];
    v.push(match scheme {
        Scheme::MouseKeyboard => ("Aim", "Mouse"),
        Scheme::Directional => ("Aim", "Last move"),
        Scheme::Dual => ("Aim", "Arrows"),
    });
    v.push(("Slash", if mouse { "F / LMB" } else { "F" }));
    v.push(("Feed", "E"));
    if defeated.dash() {
        v.push(("Dash", "Shift / K"));
    }
    if defeated.spit() {
        v.push(("Spit", if mouse { "Q / RMB" } else { "Q" }));
    }
    if defeated.shield() {
        v.push(("Shield", "G"));
    }
    if defeated.spin() {
        v.push(("Spin", "Slash midair"));
    }
    if defeated.climb() {
        v.push(("Climb", "Push at surface, A D W S"));
    }
    if defeated.burrow() {
        v.push(("Burrow", "C on rubble"));
    }
    if defeated.leap() {
        v.push(("Big jump", "Hold S, release"));
    }
    if defeated.blow() {
        v.push(("Power blow", if mouse { "Hold F / LMB" } else { "Hold F" }));
    }
    if defeated.scythe() {
        v.push(("Scythe claws", "Slash"));
    }
    if defeated.wade() {
        v.push(("Water walk", "In water"));
    }
    if defeated.sewer() {
        v.push(("Sewer air", "Passive"));
    }
    if pills {
        v.push(("Power pill", "Z"));
    }
    v.push(("Pause", "P"));
    v.push(("Menu", if cfg!(target_arch = "wasm32") { "Tab" } else { "Esc" }));
    v
}

fn update_hint(
    defeated: Res<Defeated>,
    settings: Res<Settings>,
    party: Res<crate::boss::Celebration>,
    ending: Res<crate::ending::Ending>,
    player: Single<&Player>,
    mut had_pills: Local<bool>,
    mut was_playing: Local<bool>,
    mut rows: Query<(&HintRow, &mut Node)>,
    mut labels: Query<(&HintLabel, &mut Text), Without<HintKey>>,
    mut keys: Query<(&HintKey, &mut Text), Without<HintLabel>>,
) {
    let playing = ending.playing();
    let pills = player.pill.stock > 0;
    let toggled = playing != *was_playing || pills != *had_pills;
    *was_playing = playing;
    *had_pills = pills;
    if !toggled && !defeated.is_changed() && !settings.is_changed() && !party.is_changed() {
        return;
    }
    let mut list = hints(&defeated, settings.scheme, pills);
    if playing {
        // The ending's show: no hints on the side of the screen.
        list.clear();
    }
    if party.0 {
        // The celebration is peaceful: no attacks (`ending::peace`).
        list.retain(|(action, _)| !matches!(*action, "Slash" | "Spit" | "Spin" | "Power blow" | "Scythe claws"));
    }
    for (row, mut node) in &mut rows {
        node.display = if row.0 < list.len() { Display::Flex } else { Display::None };
    }
    for (label, mut text) in &mut labels {
        text.0 = list.get(label.0).map_or("", |h| h.0).to_string();
    }
    for (key, mut text) in &mut keys {
        text.0 = list.get(key.0).map_or("", |h| h.1).to_string();
    }
}

/// A boss fell: name the power it gave, for a few seconds (real time: the
/// death's slow motion doesn't drag it).
fn announce(
    real: Res<Time<Real>>,
    mut beaten: MessageReader<Beaten>,
    label: Single<(&mut Announcement, &mut Text, &mut TextColor, &mut BackgroundColor)>,
) {
    let (mut a, mut text, mut color, mut bg) = label.into_inner();
    if let Some(Beaten(kind)) = beaten.read().last() {
        text.0 = format!("EARNED: {}", kind.power());
        a.0 = ANNOUNCE_TIME;
    }
    if a.0 <= 0.0 {
        return;
    }
    a.0 = (a.0 - real.delta_secs()).max(0.0);
    let fade = (a.0 / 1.0).min(1.0);
    color.0 = CHIP_INK.with_alpha(fade);
    bg.0 = CHIP.with_alpha(CHIP.alpha() * fade);
}

/// A crosshair: four ticks around a gap and a dot in thick white, outlined in
/// thick black so it reads on any gray.
fn crosshair_image() -> Image {
    let n = CROSSHAIR as i32;
    let c = n / 2;
    let (gap, tick, half, outline) = (4, 8, 1, 2);
    let ink = |x: i32, y: i32| {
        let (dx, dy) = ((x - c).abs(), (y - c).abs());
        let on_axis = |a: i32, b: i32| a <= half && (gap..gap + tick).contains(&b);
        on_axis(dx, dy) || on_axis(dy, dx) || (dx <= half && dy <= half)
    };
    let mut data = vec![0u8; (n * n * 4) as usize];
    for y in 0..n {
        for x in 0..n {
            let rgba = if ink(x, y) {
                [255, 255, 255, 255]
            } else if (-outline..=outline).any(|j| (-outline..=outline).any(|i| ink(x + i, y + j))) {
                [0, 0, 0, 255]
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
    time: Res<Time<Real>>,
    player: Single<&Player>,
    mut pips: Query<
        (&Pip, &mut BackgroundColor, &mut Node, &mut Outline, &mut UiTransform),
        (Without<PipShadow>, Without<PipFill>),
    >,
    mut shadows: Query<(&PipShadow, &mut Node), (Without<PipFill>, Without<Pip>)>,
    mut fills: Query<(&PipFill, &mut Node), (Without<PipShadow>, Without<Pip>)>,
) {
    let t = time.elapsed_secs();
    for (pip, mut bg, mut node, mut outline, mut tf) in &mut pips {
        node.display = if pip.0 < player.max_hp { Display::Flex } else { Display::None };
        let full = pip.0 < player.hp;
        // Each pip beats a little out of step with its neighbor, so the row ripples.
        let phase = pip.0 as f32 * 0.55;
        // Power pill: the first `lit` full pips are prismatic (fewer as it times out).
        let drained = (player.pill.crash_bar() * player.hp as f32).ceil() as i32;
        let (mut sag, mut droop) = (0.0, 0.0);
        let lit = (player.pill.bar() * player.hp as f32).ceil() as i32;
        let (color, ring, scale) = match player.god {
            _ if !full => (PIP_EMPTY, Color::NONE, 1.0),
            _ if pip.0 < lit => {
                let level = player.pill.level().max(1.0);
                let fast = 1.0 + 0.5 * level;
                let flicker = 0.5 + 0.5 * (t * 29.0 * fast + phase * 3.1).sin() * (t * 8.3 - phase).cos();
                let throb = (0.5 + 0.5 * (t * 11.0 * fast - phase).sin()).powi(3);
                let hue = t * 220.0 * fast + pip.0 as f32 * 47.0;
                (
                    prism(hue, 0.5 + 0.2 * flicker).mix(&Color::WHITE, 0.35 * flicker),
                    prism(hue + 180.0, 0.6).with_alpha(0.7 + 0.3 * throb),
                    1.14 + 0.1 * level * throb + 0.12 * throb,
                )
            }
            _ if pip.0 < drained => {
                // The comedown, the opposite of the high: ash-gray and dark, shrunken, sagging, with a slow
                // labored beat instead of the frenzy. Fewer pips stay drained as it eases off.
                let sick = (player.pill.crash_level() / pill::MAX_STACKS).sqrt().clamp(0.25, 1.0);
                let beat = (0.5 + 0.5 * (t * 1.6 - phase * 0.3).sin()).powi(2);
                let ash = Color::srgb(0.30 + 0.08 * beat, 0.31 + 0.08 * beat, 0.33 + 0.08 * beat);
                sag = 5.0 * sick;
                droop = 0.2 * sick;
                (PIP_FULL.mix(&ash, 0.9 * sick), Color::srgba(0.05, 0.06, 0.1, 0.8 * sick), 1.0 - 0.2 * sick + 0.04 * beat)
            }
            God::Off => (PIP_FULL, Color::NONE, 1.0),
            // Molten gold, breathing; a bright halo.
            God::Invulnerable => {
                let beat = 0.5 + 0.5 * (t * 3.0 - phase).sin();
                (PIP_GOD.mix(&PIP_GOD_HOT, beat), PIP_GOD_HALO.with_alpha(0.35 + 0.55 * beat), 1.0 + 0.12 * beat)
            }
            // Hellfire: flickering white-hot to blood, a violent pulse, a black-and-crimson ring.
            God::Smite => {
                let flicker = 0.5 + 0.5 * (t * 23.0 + phase * 3.1).sin() * (t * 7.3 - phase).cos();
                let throb = (0.5 + 0.5 * (t * 9.0 - phase).sin()).powi(3);
                (
                    PIP_SMITE.mix(&PIP_SMITE_HOT, flicker * 0.8),
                    PIP_SMITE_HALO.mix(&Color::BLACK, 1.0 - throb).with_alpha(0.65 + 0.35 * throb),
                    1.12 + 0.28 * throb,
                )
            }
        };
        bg.0 = color;
        outline.color = ring;
        tf.scale = Vec2::splat(scale);
        tf.translation = Val2::px(0.0, sag);
        tf.rotation = Rot2::radians(0.25 + droop);
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

/// The pills carried, each sliding around the color wheel; one icon per pill.
fn update_pill_icons(
    time: Res<Time<Real>>,
    player: Single<&Player>,
    mut icons: Query<(&PillIcon, &mut Node, &mut BackgroundColor, &mut Outline), Without<PillShine>>,
) {
    let t = time.elapsed_secs();
    for (icon, mut node, mut bg, mut ring) in &mut icons {
        node.display = if icon.0 < player.pill.stock { Display::Flex } else { Display::None };
        let hue = t * 120.0 + icon.0 as f32 * 52.0;
        bg.0 = prism(hue, 0.55);
        ring.color = prism(hue + 150.0, 0.7).with_alpha(0.8);
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
