//! The game menu: up when the game starts and on Escape (or a gamepad's Start).
//!
//! Main page: Resume (once a game is on), Load last checkpoint (F9), Start new (F8),
//! Settings, Exit. Settings page: difficulty, volume, contrast
//! (F6 / F7) and the control scheme. The menu pauses the game (`Time<Virtual>`)
//! and swallows the keyboard and the mouse while it is up.
//!
//! Keys: up / down (W S, d-pad) pick a row, left / right (A D) change a value,
//! Enter, Space or F (south button) confirm, Escape (east button) goes back.
//! The mouse works too: hover to pick, click to confirm (the left half of a
//! value lowers it, the right half raises it).

use bevy::{
    prelude::*,
    text::LineBreak,
    ui::RelativeCursorPosition,
    window::{PrimaryWindow, WindowMode},
};

use crate::{
    level::NewGame,
    save::{self, LoadLast},
    settings::{self, Difficulty, Scheme, Settings, VOLUME_STEPS},
    terrain_art::{Contrast, CONTRAST_RANGE},
};

const ROWS: usize = 6;
const ROW_HEIGHT: f32 = 52.0;
/// The arrows around a value (GhoreSerif has them).
const LEFT: &str = "◀";
const RIGHT: &str = "▶";
const ROW_WIDTH: f32 = 760.0;
const LABEL_WIDTH: f32 = 220.0;
const VALUE_WIDTH: f32 = 260.0;
const ARROW_WIDTH: f32 = 64.0;
const CONTRAST_STEP: f32 = 0.5;

const INK: Color = Color::srgb(0.88, 0.85, 0.78);
const INK_DIM: Color = Color::srgba(0.88, 0.85, 0.78, 0.35);
const INK_HOT: Color = Color::srgb(1.0, 0.93, 0.62);
/// The picked row: a solid amber bar with dark ink on it.
const BAR: Color = Color::srgb(0.95, 0.74, 0.22);
const BAR_HOT: Color = Color::srgb(1.0, 0.92, 0.55);
const BAR_HALO: Color = Color::srgb(1.0, 0.85, 0.4);
const INK_ON_BAR: Color = Color::srgb(0.10, 0.07, 0.02);
const INK_ON_BAR_DIM: Color = Color::srgba(0.10, 0.07, 0.02, 0.3);

pub struct MenuPlugin;

impl Plugin for MenuPlugin {
    fn build(&self, app: &mut App) {
        let up = settings::menu_at_start();
        app.insert_resource(Menu { open: up, page: Page::Main, sel: 0, peek: false, arrow: None, started: !up })
            .add_systems(Startup, spawn_menu)
            .add_systems(PreUpdate, input.after(bevy::input::InputSystems).before(crate::level::restart))
            .add_systems(Update, draw);
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Page {
    Main,
    Settings,
}

#[derive(Resource)]
pub struct Menu {
    pub open: bool,
    page: Page,
    sel: usize,
    /// The contrast was just changed: only that row shows, so the map can be judged,
    /// until the focus moves to another row.
    peek: bool,
    /// The arrow the mouse is over (row, direction).
    arrow: Option<(usize, i32)>,
    /// A game is on (the menu has been left once): "Return to game" shows.
    started: bool,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Act {
    LoadLast,
    NewGame,
    Settings,
    Resume,
    Exit,
    Difficulty,
    Volume,
    Fullscreen,
    Contrast,
    Scheme,
    Back,
}

struct Row {
    act: Act,
    label: String,
    /// Settings: the value between the arrows.
    value: Option<String>,
    enabled: bool,
}

impl Act {
    /// A row that holds a value (left / right change it).
    fn adjustable(self) -> bool {
        matches!(self, Act::Difficulty | Act::Volume | Act::Fullscreen | Act::Contrast | Act::Scheme)
    }
}

fn rows(menu: &Menu, settings: &Settings, contrast: f32, full: bool) -> Vec<Row> {
    let row = |act, label: &str, enabled| Row { act, label: label.to_string(), value: None, enabled };
    let val = |act, label: &str, value: String| Row { act, label: label.to_string(), value: Some(value), enabled: true };
    match menu.page {
        Page::Main => {
            let mut rows = Vec::new();
            // Once a game is on: the same as Escape.
            if menu.started {
                rows.push(row(Act::Resume, "Resume", true));
            }
            rows.extend([
                row(Act::LoadLast, "Load last checkpoint", save::has_save()),
                row(Act::NewGame, "Start new", true),
                row(Act::Settings, "Settings", true),
                row(Act::Exit, "Exit", true),
            ]);
            rows
        }
        Page::Settings => vec![
            val(Act::Difficulty, "Difficulty", settings.difficulty.name().to_string()),
            val(Act::Volume, "Volume", format!("{}%", settings.volume as u32 * 100 / VOLUME_STEPS as u32)),
            val(Act::Fullscreen, "Fullscreen", if full { "On" } else { "Off" }.to_string()),
            val(Act::Contrast, "Contrast", format!("{:+.1}", contrast)),
            val(Act::Scheme, "Controls", settings.scheme.name().to_string()),
            row(Act::Back, "Back", true),
        ],
    }
}

fn help(act: Act, settings: &Settings) -> &'static str {
    match act {
        Act::Difficulty => settings.difficulty.blurb(),
        Act::Scheme => settings.scheme.blurb(),
        Act::Contrast => "Ground against interiors (F6 / F7).",
        Act::Volume => "Sound effects.",
        Act::Fullscreen => "Borderless fullscreen (F11 / Alt+Enter).",
        Act::LoadLast => "Back to the latest quick or automatic save (F5 saves / F9 loads).",
        Act::NewGame => "Start over from the beginning (F8).",
        _ => "",
    }
}

#[derive(Component)]
struct MenuRoot;

#[derive(Component)]
pub struct MenuRow(usize);

/// The parts of a row: its name, the arrows around its value (a click on one
/// steps the value that way), and the value.
#[derive(Component)]
struct MenuLabel(usize);

#[derive(Component)]
struct MenuValue(usize);

/// The fixed-width slot between the arrows; the value is centered in it.
#[derive(Component)]
struct MenuValueBox(usize);

#[derive(Component)]
pub struct MenuArrow(usize, i32);

#[derive(Component)]
struct MenuHelp;

#[derive(Component)]
struct MenuTitle;

fn spawn_menu(mut commands: Commands) {
    let text_font = |size: f32| TextFont { font_size: FontSize::Px(size), ..default() };
    commands
        .spawn((
            MenuRoot,
            Node {
                position_type: PositionType::Absolute,
                width: percent(100),
                height: percent(100),
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                row_gap: px(8),
                ..default()
            },
            BackgroundColor(Color::srgba(0.02, 0.02, 0.02, 0.86)),
            GlobalZIndex(100),
            Visibility::Hidden,
        ))
        .with_children(|root| {
            root.spawn((
                MenuTitle,
                Text::new("GHORE, THE CHAMELEON"),
                text_font(44.0),
                TextColor(INK),
                Node { margin: UiRect::bottom(px(28)), ..default() },
            ));
            for i in 0..ROWS {
                let layout = TextLayout { justify: Justify::Center, linebreak: LineBreak::NoWrap };
                root.spawn((
                    MenuRow(i),
                    RelativeCursorPosition::default(),
                    Node { width: px(ROW_WIDTH), height: px(ROW_HEIGHT), justify_content: JustifyContent::Center, align_items: AlignItems::Center, ..default() },
                    BackgroundColor(Color::NONE),
                    Outline::new(px(3), px(2), Color::NONE),
                ))
                .with_children(|row| {
                    let pad = UiRect::axes(px(0), px(10));
                    row.spawn((MenuLabel(i), Text::new(""), text_font(26.0), TextColor(INK), layout, Node { padding: pad, ..default() }));
                    row.spawn((MenuArrow(i, -1), RelativeCursorPosition::default(), Text::new(LEFT), text_font(26.0), TextColor(INK), layout, Node { width: px(ARROW_WIDTH), padding: pad, ..default() }));
                    row.spawn((
                        MenuValueBox(i),
                        Node { width: px(VALUE_WIDTH), justify_content: JustifyContent::Center, align_items: AlignItems::Center, ..default() },
                    ))
                    .with_children(|slot| {
                        slot.spawn((MenuValue(i), Text::new(""), text_font(26.0), TextColor(INK), layout, Node { padding: pad, ..default() }));
                    });
                    row.spawn((MenuArrow(i, 1), RelativeCursorPosition::default(), Text::new(RIGHT), text_font(26.0), TextColor(INK), layout, Node { width: px(ARROW_WIDTH), padding: pad, ..default() }));
                });
            }
            root.spawn((
                MenuHelp,
                Text::new(""),
                text_font(18.0),
                TextColor(INK_DIM),
                TextLayout { justify: Justify::Center, linebreak: LineBreak::NoWrap },
                Node { margin: UiRect::top(px(24)), height: px(28), ..default() },
            ));
        });
}

/// Moves the selection by `step` over the rows that can be picked.
fn step(sel: usize, rows: &[Row], step: i32) -> usize {
    let n = rows.len() as i32;
    let mut i = sel as i32;
    for _ in 0..n {
        i = (i + step).rem_euclid(n);
        if rows[i as usize].enabled {
            return i as usize;
        }
    }
    sel
}

/// Changes the value of an adjustable row by `dir` (-1, +1); `wrap` loops around the ends.
fn adjust(act: Act, dir: i32, wrap: bool, settings: &mut Settings, contrast: &mut Contrast) {
    let go = |i: i32, n: i32| if wrap { i.rem_euclid(n) } else { i.clamp(0, n - 1) };
    match act {
        Act::Difficulty => settings.difficulty = Difficulty::ALL[go(settings.difficulty.index() as i32 + dir, 4) as usize],
        Act::Volume => settings.volume = go(settings.volume as i32 + dir, VOLUME_STEPS as i32 + 1) as u8,
        Act::Fullscreen => settings.fullscreen = !settings.fullscreen,
        Act::Scheme => settings.scheme = Scheme::ALL[go(settings.scheme.index() as i32 + dir, 3) as usize],
        Act::Contrast => {
            let next = contrast.0 + dir as f32 * CONTRAST_STEP;
            contrast.0 = if wrap && next > CONTRAST_RANGE.1 {
                CONTRAST_RANGE.0
            } else if wrap && next < CONTRAST_RANGE.0 {
                CONTRAST_RANGE.1
            } else {
                next.clamp(CONTRAST_RANGE.0, CONTRAST_RANGE.1)
            };
        }
        _ => {}
    }
}

/// Would a step of `dir` change the value?
fn can(act: Act, dir: i32, settings: &Settings, contrast: f32) -> bool {
    let inside = |i: i32, n: i32| (0..n).contains(&i);
    match act {
        Act::Difficulty => inside(settings.difficulty.index() as i32 + dir, 4),
        Act::Volume => inside(settings.volume as i32 + dir, VOLUME_STEPS as i32 + 1),
        Act::Scheme => inside(settings.scheme.index() as i32 + dir, 3),
        Act::Fullscreen => true,
        Act::Contrast => (CONTRAST_RANGE.0..=CONTRAST_RANGE.1).contains(&(contrast + dir as f32 * CONTRAST_STEP)),
        _ => false,
    }
}

/// Reads the keys (before the game does), runs the menu, and keeps the game
/// paused and deaf while it is up.
pub fn input(
    mut menu: ResMut<Menu>,
    mut keys: ResMut<ButtonInput<KeyCode>>,
    mut mouse: ResMut<ButtonInput<MouseButton>>,
    pads: Query<&Gamepad>,
    mut settings: ResMut<Settings>,
    mut contrast: ResMut<Contrast>,
    mut time: ResMut<Time<Virtual>>,
    window: Single<&Window, With<PrimaryWindow>>,
    rows_ui: Query<(&MenuRow, &RelativeCursorPosition)>,
    arrows_ui: Query<(&MenuArrow, &RelativeCursorPosition)>,
    mut hover: Local<Option<usize>>,
    mut seen_sel: Local<usize>,
    mut new_game: MessageWriter<NewGame>,
    mut load_last: MessageWriter<LoadLast>,
    mut exit: MessageWriter<AppExit>,
) {
    let was_open = menu.open;
    let pad = |b: GamepadButton| pads.iter().any(|p| p.just_pressed(b));
    let key = |ks: &[KeyCode]| keys.any_just_pressed(ks.iter().copied());

    let back = key(&[KeyCode::Escape, KeyCode::Tab, KeyCode::KeyM]) || pad(GamepadButton::East) || pad(GamepadButton::Start);
    if !menu.open {
        if back || pad(GamepadButton::Start) {
            menu.open = true;
            menu.page = Page::Main;
            // On "Return to game".
            let r = rows(&menu, &settings, contrast.0, window.mode != WindowMode::Windowed);
            menu.sel = r.iter().position(|r| r.act == Act::Resume).unwrap_or(0);
        }
    } else {
        let r = rows(&menu, &settings, contrast.0, window.mode != WindowMode::Windowed);
        menu.sel = menu.sel.min(r.len() - 1);
        if !r[menu.sel].enabled {
            menu.sel = step(menu.sel, &r, 1);
        }
        let mut activate: Option<(usize, i32)> = None; // (row, direction: 0 = confirm)
        let mut moved = 0;
        if key(&[KeyCode::ArrowUp, KeyCode::KeyW]) || pad(GamepadButton::DPadUp) {
            moved = -1;
        }
        if key(&[KeyCode::ArrowDown, KeyCode::KeyS]) || pad(GamepadButton::DPadDown) {
            moved = 1;
        }
        let side = if key(&[KeyCode::ArrowLeft, KeyCode::KeyA]) || pad(GamepadButton::DPadLeft) {
            -1
        } else if key(&[KeyCode::ArrowRight, KeyCode::KeyD]) || pad(GamepadButton::DPadRight) {
            1
        } else {
            0
        };
        if moved != 0 {
            menu.sel = step(menu.sel, &r, moved);
        }
        if side != 0 && r[menu.sel].act.adjustable() {
            activate = Some((menu.sel, side));
        }
        let alt = keys.pressed(KeyCode::AltLeft) || keys.pressed(KeyCode::AltRight);
        let enter = key(&[KeyCode::Enter, KeyCode::NumpadEnter]) && !alt; // Alt+Enter toggles fullscreen
        if enter || key(&[KeyCode::Space, KeyCode::KeyF]) || pad(GamepadButton::South) {
            activate = Some((menu.sel, 0));
        }
        // The mouse: hovering picks a row; a click on an arrow steps that value,
        // a click on any other row confirms it (a value row only gets picked).
        let over_arrow = arrows_ui
            .iter()
            .find(|(a, at)| at.cursor_over && a.0 < r.len() && r[a.0].value.is_some() && can(r[a.0].act, a.1, &settings, contrast.0))
            .map(|(a, _)| (a.0, a.1));
        let over_row = rows_ui.iter().find(|(row, at)| at.cursor_over && row.0 < r.len() && r[row.0].enabled).map(|(row, _)| row.0);
        menu.arrow = over_arrow;
        let hovered = over_arrow.map(|(i, _)| i).or(over_row);
        if hovered != *hover {
            if let Some(i) = hovered {
                menu.sel = i;
            }
            *hover = hovered;
        }
        if mouse.just_pressed(MouseButton::Left) {
            if let Some((i, dir)) = over_arrow {
                activate = Some((i, dir));
            } else if let Some(i) = over_row.filter(|&i| !r[i].act.adjustable()) {
                activate = Some((i, 0));
            }
        }

        if menu.sel != *seen_sel {
            *seen_sel = menu.sel;
            menu.peek = false;
        }
        if let Some((i, dir)) = activate {
            let act = r[i].act;
            if act.adjustable() {
                let before = (settings.clone(), contrast.0);
                adjust(act, if dir == 0 { 1 } else { dir }, dir == 0, &mut settings, &mut contrast);
                if before != (settings.clone(), contrast.0) {
                    settings::store(&settings, contrast.0);
                    menu.peek = act == Act::Contrast;
                }
            } else if dir == 0 {
                match act {
                    Act::LoadLast => {
                        load_last.write(LoadLast);
                        menu.open = false;
                    }
                    Act::NewGame => {
                        new_game.write(NewGame);
                        menu.open = false;
                    }
                    Act::Settings => {
                        menu.page = Page::Settings;
                        menu.sel = 0;
                    }
                    Act::Resume => menu.open = false,
                    Act::Exit => {
                        exit.write(AppExit::Success);
                    }
                    Act::Back => {
                        menu.page = Page::Main;
                        menu.sel = if menu.started { 3 } else { 2 };
                    }
                    _ => {}
                }
            }
        } else if back {
            match menu.page {
                Page::Settings => {
                    menu.page = Page::Main;
                    menu.sel = if menu.started { 3 } else { 2 };
                }
                Page::Main if menu.started => menu.open = false,
                Page::Main => {}
            }
        }
        if !menu.open {
            menu.started = true;
        }
        if !menu.open || menu.page != Page::Settings {
            menu.peek = false;
        }
    }

    if menu.open {
        time.pause();
    } else if was_open {
        time.unpause();
    }
    // Nothing reaches the game while it is up, nor on the frame it goes away
    // (the Enter that left it must not become a jump).
    if menu.open || was_open {
        // Alt stays visible (it would otherwise read as released on the next Enter), and so does
        // an Alt+Enter, which `toggle_fullscreen` still has to see.
        let alt = keys.pressed(KeyCode::AltLeft) || keys.pressed(KeyCode::AltRight);
        let held: Vec<KeyCode> = keys
            .get_pressed()
            .copied()
            .chain(keys.get_just_pressed().copied())
            .filter(|k| !is_function_key(*k) && !matches!(k, KeyCode::AltLeft | KeyCode::AltRight) && !(alt && *k == KeyCode::Enter))
            .collect();
        for k in held {
            keys.reset(k);
        }
        mouse.reset_all();
    }
}

fn is_function_key(key: KeyCode) -> bool {
    use KeyCode::*;
    matches!(key, F1 | F2 | F3 | F4 | F5 | F6 | F7 | F8 | F9 | F10 | F11 | F12)
}

fn draw(
    menu: Res<Menu>,
    settings: Res<Settings>,
    contrast: Res<Contrast>,
    window: Single<&Window, With<PrimaryWindow>>,
    real: Res<Time<Real>>,
    mut root: Single<(&mut Visibility, &mut BackgroundColor), (With<MenuRoot>, Without<MenuRow>)>,
    mut title: Single<&mut TextColor, (With<MenuTitle>, Without<MenuLabel>, Without<MenuValue>, Without<MenuArrow>, Without<MenuHelp>)>,
    mut boxes: Query<(&MenuRow, &mut Node, &mut BackgroundColor, &mut Outline), (Without<MenuLabel>, Without<MenuValue>, Without<MenuValueBox>, Without<MenuArrow>, Without<MenuRoot>)>,
    mut labels: Query<(&MenuLabel, &mut Text, &mut TextColor, &mut Node), (Without<MenuRow>, Without<MenuValue>, Without<MenuValueBox>, Without<MenuArrow>, Without<MenuHelp>, Without<MenuTitle>)>,
    mut values: Query<(&MenuValue, &mut Text, &mut TextColor, &mut Node), (Without<MenuRow>, Without<MenuLabel>, Without<MenuValueBox>, Without<MenuArrow>, Without<MenuHelp>, Without<MenuTitle>)>,
    mut slots: Query<(&MenuValueBox, &mut Node), (Without<MenuRow>, Without<MenuLabel>, Without<MenuValue>, Without<MenuArrow>)>,
    mut arrows: Query<(&MenuArrow, &mut TextColor, &mut Node), (Without<MenuRow>, Without<MenuLabel>, Without<MenuValue>, Without<MenuValueBox>, Without<MenuHelp>, Without<MenuTitle>)>,
    mut help_text: Single<&mut Text, (With<MenuHelp>, Without<MenuRow>, Without<MenuLabel>, Without<MenuValue>)>,
) {
    *root.0 = if menu.open { Visibility::Inherited } else { Visibility::Hidden };
    if !menu.open {
        return;
    }
    let r = rows(&menu, &settings, contrast.0, window.mode != WindowMode::Windowed);
    // Adjusting the contrast: everything but that row steps aside, to see the map change.
    let peek = menu.peek && r.get(menu.sel).is_some_and(|i| i.act == Act::Contrast);
    root.1.0 = Color::srgba(0.02, 0.02, 0.02, if peek { 0.0 } else { 0.86 });
    title.0 = if peek { Color::NONE } else { INK };
    // How a part of row `i` is inked.
    let ink = |i: usize, enabled: bool| {
        if peek && i != menu.sel {
            Color::NONE
        } else if !enabled {
            INK_DIM
        } else if i == menu.sel && !peek {
            INK_ON_BAR
        } else if i == menu.sel {
            INK_HOT
        } else {
            INK
        }
    };

    // The picked row breathes like the god-mode pips: amber to white-hot, with a haloed ring.
    let beat = 0.5 + 0.5 * (real.elapsed_secs() * 4.0).sin();
    for (row, mut node, mut bg, mut outline) in &mut boxes {
        node.display = if row.0 < r.len() { Display::Flex } else { Display::None };
        bg.0 = match r.get(row.0) {
            Some(item) if row.0 == menu.sel && item.enabled => {
                if peek {
                    Color::srgba(0.0, 0.0, 0.0, 0.55)
                } else {
                    BAR.mix(&BAR_HOT, beat)
                }
            }
            _ => Color::NONE,
        };
        outline.color = match r.get(row.0) {
            Some(item) if row.0 == menu.sel && item.enabled && !peek => BAR_HALO.with_alpha(0.35 + 0.65 * beat),
            _ => Color::NONE,
        };
    }
    for (label, mut text, mut color, mut node) in &mut labels {
        let Some(item) = r.get(label.0) else { continue };
        if text.0 != item.label {
            text.0 = item.label.clone();
        }
        color.0 = ink(label.0, item.enabled);
        // Settings names line up in a column; a menu entry is centered on its own.
        node.width = if item.value.is_some() { px(LABEL_WIDTH) } else { Val::Auto };
    }
    for (slot, mut node) in &mut slots {
        let Some(item) = r.get(slot.0) else { continue };
        node.display = if item.value.is_some() { Display::Flex } else { Display::None };
    }
    for (value, mut text, mut color, _) in &mut values {
        let Some(item) = r.get(value.0) else { continue };
        let shown = item.value.clone().unwrap_or_default();
        if text.0 != shown {
            text.0 = shown;
        }
        color.0 = ink(value.0, item.enabled);
    }
    for (arrow, mut color, mut node) in &mut arrows {
        let Some(item) = r.get(arrow.0) else { continue };
        node.display = if item.value.is_some() { Display::Flex } else { Display::None };
        color.0 = if peek && arrow.0 != menu.sel {
            Color::NONE
        } else if !can(item.act, arrow.1, &settings, contrast.0) {
            if arrow.0 == menu.sel && !peek { INK_ON_BAR_DIM } else { INK_DIM }
        } else if menu.arrow == Some((arrow.0, arrow.1)) {
            if arrow.0 == menu.sel && !peek { Color::BLACK } else { Color::WHITE }
        } else {
            ink(arrow.0, true)
        };
    }
    help_text.0 = r.get(menu.sel).map_or("", |i| help(i.act, &settings)).to_string();
}
