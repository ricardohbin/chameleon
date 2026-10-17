//! The end: the last boss falls and every color is back. A few seconds after
//! the kill (the death spectacle and its wave first), the chameleon takes no
//! more orders and hops for joy, untouchable, while the eleven colors it
//! earned light up one by one and circle it (in kill order). "THE END" drops
//! in from the top of the screen, its letters cycling through those colors.
//! Then the world goes beyond its colors: every world camera is graded
//! saturated, contrasty, its color casts drifting (almost psychedelic), and
//! "THE COLORS ARE BACK IN THE WORLD AGAIN" fades in. The words and the
//! lights fade, the screen fades slowly to black: the credits ("MADE IN SAO
//! PAULO, BY BIN & ZED") and "PRESS ENTER TO CONTINUE". Enter fades back in
//! at the start of the game: the **celebration** (`boss::Celebration`). Every
//! boss and mob is back, in its own color, and they cheer for the chameleon
//! (hopping for joy, bursts of their color, `Cheer`). Nobody fights: they
//! never attack, and the chameleon can't attack either (`peace`). The world
//! stays psychedelic. It is saved (an auto save as it starts); F8 brings the
//! gray world back.
//!
//! It starts only on the kill that completes the set (`boss::Beaten`), not
//! on loading a save. `ENDING=1` plays it as soon as the game is on (every
//! boss counted as beaten).

use std::f32::consts::TAU;

use bevy::{
    prelude::*,
    render::view::{ColorGrading, ColorGradingGlobal, ColorGradingSection},
    sprite_render::AlphaMode2d,
};

use crate::{
    boss::{Boss, BossKind, Celebration, Defeated},
    camera::{self, BackdropCamera, ColorCamera, MainCamera},
    chameleon::ColorEarned,
    fumes::Cloud,
    hud::ScreenFlash,
    level::{self, Restart, WorldReloaded},
    mobs::Mob,
    player::{self, Player},
    regrowth,
    save::AutoSave,
    shapes::*,
};

/// Seconds (game time) from the kill to the hops and the lights (`DEV_WAIT` for `ENDING=1`).
const WAIT: f32 = 4.0;
const DEV_WAIT: f32 = 1.5;
/// Then, counted from the start of the hops:
const LIGHT_EVERY: f32 = 0.22;
const TITLE_AT: f32 = 1.2;
const TITLE_TIME: f32 = 2.6;
const COLORS_AT: f32 = 5.5;
const COLORS_TIME: f32 = 3.5;
const MESSAGE_AT: f32 = 7.0;
const MESSAGE_TIME: f32 = 1.5;
const FADE_AT: f32 = 18.0;
const FADE_TIME: f32 = 2.5;
/// Then slowly to black (the celebration is set up there), the credits, and after Enter back.
const BLACK_IN: f32 = 3.5;
const CREDITS_IN: f32 = 1.5;
/// "PRESS ENTER TO CONTINUE" shows this long after the black.
const PROMPT_AT: f32 = 2.5;
const CREDITS_OUT: f32 = 0.6;
const BLACK_OUT: f32 = 2.0;
/// The hops.
const HOP: f32 = 760.0;
/// The lights' circle around the chameleon.
const ORBIT: f32 = 78.0;

pub struct EndingPlugin;

impl Plugin for EndingPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Ending>()
            .add_systems(PreUpdate, (hold_input, peace).after(level::restart).after(bevy::input::InputSystems))
            .add_systems(PreUpdate, unhop)
            .add_systems(Update, (start, hop.before(player::move_player)))
            .add_systems(Update, (lights, words, blackout, grade, paint).chain().after(player::move_player).after(regrowth::tint_backdrop))
            .add_systems(PostUpdate, (cheer, confetti).before(bevy::transform::TransformSystems::Propagate));
    }
}

/// Where the ending is.
#[derive(Resource, Default)]
pub struct Ending {
    /// Seconds since the last kill (or since `ENDING=1` asked for it); `None` when not playing.
    t: Option<f32>,
    /// How far the world's extra color has come in (0..1).
    colors: f32,
    /// The grading is touched (so it's put back once the colors go).
    graded: bool,
    /// `ENDING=1` has played it already.
    dev_done: bool,
    /// Seconds into the show when Enter was pressed on the credits.
    went_on: Option<f32>,
    /// The celebration has been set up in this show (at the black).
    set_up: bool,
}

impl Ending {
    /// Seconds into the show (the hops), if it has started.
    fn show(&self, wait: f32) -> Option<f32> {
        self.t.map(|t| t - wait).filter(|s| *s >= 0.0)
    }
}

fn wait() -> f32 {
    if std::env::var("ENDING").is_ok() { DEV_WAIT } else { WAIT }
}

/// The colors the bosses gave back (`regrowth`'s, the bosses' own).
fn color(kind: BossKind) -> Color {
    match kind {
        BossKind::Wasp => regrowth::GREEN,
        BossKind::Roach => regrowth::YELLOW,
        BossKind::Dragonfly => regrowth::PINK,
        BossKind::Spider => regrowth::BLUE,
        BossKind::Centipede => regrowth::PURPLE,
        BossKind::Frog => regrowth::VIOLET,
        BossKind::Grasshopper => regrowth::BROWN,
        BossKind::Rat => regrowth::RED,
        BossKind::Mantis => regrowth::CYAN,
        BossKind::Crab => regrowth::INDIGO,
        BossKind::Beetle => regrowth::ORANGE,
    }
}

/// The bosses in kill order (any not in `Defeated` go last).
fn kill_order(defeated: &Defeated) -> Vec<BossKind> {
    let mut order = defeated.0.clone();
    let rest: Vec<BossKind> = BossKind::ALL.into_iter().filter(|k| !order.contains(k)).collect();
    order.extend(rest);
    order
}

fn all_beaten(defeated: &Defeated) -> bool {
    BossKind::ALL.iter().all(|k| defeated.0.contains(k))
}

/// One of the eleven lights.
#[derive(Component)]
struct Light {
    /// Its place in the kill order.
    i: usize,
    halo: Handle<ColorMaterial>,
    core: Handle<ColorMaterial>,
}

/// The words on screen ("THE END" and the message), each on its own row.
#[derive(Component)]
struct Title;

/// The row of "THE END" (it drops in).
#[derive(Component)]
struct Drop;

#[derive(Component)]
struct Letter(usize);

#[derive(Component)]
struct Message;

#[allow(clippy::too_many_arguments)]
fn start(
    time: Res<Time>,
    mut ending: ResMut<Ending>,
    mut beaten: MessageReader<crate::boss::Beaten>,
    mut restarted: MessageReader<Restart>,
    defeated: Res<Defeated>,
    menu: Res<crate::menu::Menu>,
    party: Res<Celebration>,
    mut commands: Commands,
    stuff: Query<Entity, Or<(With<Light>, With<Title>, With<Message>, With<Blackout>, With<Credits>)>>,
) {
    if restarted.read().count() > 0 && ending.t.is_some() {
        // F8 / a load mid-show: it's over.
        ending.t = None;
        for e in &stuff {
            commands.entity(e).despawn();
        }
    }
    let completed = beaten.read().count() > 0 && all_beaten(&defeated) && !party.0;
    let dev = std::env::var("ENDING").is_ok() && !ending.dev_done && !menu.open && all_beaten(&defeated);
    if ending.t.is_none() && (completed || dev) {
        ending.dev_done = true;
        ending.t = Some(0.0);
        ending.went_on = None;
        ending.set_up = false;
        info!("the end");
    }
    if let Some(t) = ending.t.as_mut() {
        *t += time.delta_secs();
    }
}

/// While the show is on the keys (but the F keys, Esc and Enter for the credits) and the mouse are nobody's.
fn hold_input(ending: Res<Ending>, mut keys: ResMut<ButtonInput<KeyCode>>, mut mouse: ResMut<ButtonInput<MouseButton>>) {
    if ending.show(wait()).is_none() {
        return;
    }
    let held: Vec<KeyCode> = keys.get_pressed().copied().filter(|k| !matches!(k, KeyCode::F1 | KeyCode::F2 | KeyCode::F3 | KeyCode::F4 | KeyCode::F5 | KeyCode::F6 | KeyCode::F7 | KeyCode::F8 | KeyCode::F9 | KeyCode::F10 | KeyCode::F11 | KeyCode::F12 | KeyCode::Escape | KeyCode::Enter | KeyCode::NumpadEnter)).collect();
    for key in held {
        keys.reset(key);
    }
    mouse.reset_all();
}

/// The celebration is fully peaceful: the chameleon's attacks (slash, spit, the power blow's
/// charge, the spin) are swallowed before it sees them.
fn peace(party: Res<Celebration>, mut keys: ResMut<ButtonInput<KeyCode>>, mut mouse: ResMut<ButtonInput<MouseButton>>) {
    if !party.0 {
        return;
    }
    for key in [KeyCode::KeyF, KeyCode::KeyJ, KeyCode::KeyQ] {
        keys.reset(key);
    }
    mouse.reset(MouseButton::Left);
    mouse.reset(MouseButton::Right);
}

/// Hops for joy, turning round each time, untouchable.
fn hop(ending: Res<Ending>, mut player: Single<&mut Player>, mut was_grounded: Local<bool>) {
    if ending.show(wait()).is_none_or(|s| s > FADE_AT + FADE_TIME) || player.dead.is_some() {
        *was_grounded = false;
        return;
    }
    player.guard = player.guard.max(0.3);
    player.vel.x = 0.0;
    if player.grounded && !*was_grounded {
        player.vel.y = HOP;
        player.facing = -player.facing;
    }
    *was_grounded = player.grounded;
}

/// The eleven lights: they come one by one, circle the chameleon, pulse, and fade with the words.
#[allow(clippy::too_many_arguments)]
fn lights(
    time: Res<Time>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    mut flash: ResMut<ScreenFlash>,
    cloud: Res<Cloud>,
    ending: Res<Ending>,
    defeated: Res<Defeated>,
    mut core_mesh: Local<Option<Handle<Mesh>>>,
    player: Single<&Transform, With<Player>>,
    mut lights: Query<(Entity, &Light, &mut Transform), Without<Player>>,
) {
    let Some(s) = ending.show(wait()) else { return };
    let now = time.elapsed_secs();
    let center = player.translation.truncate();
    let core = core_mesh.get_or_insert_with(|| meshes.add(fill(&ellipse_pts(1.0, 1.0, 16), |p| 1.0 - 0.4 * p.length()))).clone();
    let order = kill_order(&defeated);
    let shown = lights.iter().count();
    for (i, &kind) in order.iter().enumerate().skip(shown) {
        if s < i as f32 * LIGHT_EVERY {
            break;
        }
        let c = color(kind);
        let halo = materials.add(ColorMaterial { color: c.with_alpha(0.0), alpha_mode: AlphaMode2d::Blend, ..default() });
        let core_mat = materials.add(ColorMaterial { color: c.with_alpha(0.0), alpha_mode: AlphaMode2d::Blend, ..default() });
        commands
            .spawn((Transform::from_translation(center.extend(11.8)), Visibility::default(), Light { i, halo: halo.clone(), core: core_mat.clone() }))
            .with_children(|l| {
                l.spawn((Mesh2d(cloud.0.clone()), MeshMaterial2d(halo), Transform::from_scale(Vec3::splat(30.0)), camera::color_layer()));
                l.spawn((Mesh2d(core.clone()), MeshMaterial2d(core_mat), Transform::from_xyz(0.0, 0.0, 0.1).with_scale(Vec3::splat(6.0)), camera::color_layer()));
            });
        flash.flash(c, 0.12);
    }
    let fade = 1.0 - ((s - FADE_AT) / FADE_TIME).clamp(0.0, 1.0);
    for (e, light, mut tf) in &mut lights {
        if fade <= 0.0 {
            commands.entity(e).despawn();
            continue;
        }
        let born = (s - light.i as f32 * LIGHT_EVERY).max(0.0);
        let rise = (born / 0.4).min(1.0);
        let a = now * 1.3 + light.i as f32 * TAU / BossKind::ALL.len() as f32;
        let r = ORBIT * (0.4 + 0.6 * rise) * (1.0 + 0.08 * (now * 3.0 + light.i as f32).sin());
        let at = center + Vec2::new(a.cos() * r, a.sin() * r * 0.8);
        tf.translation.x = at.x;
        tf.translation.y = at.y;
        let pulse = 0.75 + 0.25 * (now * 5.0 + light.i as f32 * 0.7).sin();
        tf.scale = Vec3::splat(0.6 + 0.4 * rise + 0.15 * pulse);
        let alpha = rise * fade;
        for (handle, k) in [(&light.halo, 0.55 * pulse), (&light.core, 1.0)] {
            if let Some(mut m) = materials.get_mut(handle) {
                m.color = m.color.with_alpha(alpha * k);
            }
        }
    }
}

/// "THE END" drops in from the top; the message fades in under it; both fade away.
fn words(
    time: Res<Time>,
    mut commands: Commands,
    ending: Res<Ending>,
    defeated: Res<Defeated>,
    rows: Query<(Entity, Has<Drop>, &mut Node), With<Title>>,
    mut letters: Query<(&Letter, &mut TextColor), Without<Message>>,
    mut message: Query<(&mut TextColor, &mut TextShadow), (With<Message>, Without<Letter>)>,
    mut shadows: Query<&mut TextShadow, (Without<Message>, Without<Letter>)>,
) {
    let Some(s) = ending.show(wait()) else { return };
    let now = time.elapsed_secs();
    let fade = 1.0 - ((s - FADE_AT) / FADE_TIME).clamp(0.0, 1.0);
    if s >= TITLE_AT && rows.is_empty() && fade > 0.0 {
        let font = TextFont { font_size: FontSize::Px(130.0), ..default() };
        commands
            .spawn((
                Title,
                Drop,
                Node { position_type: PositionType::Absolute, width: percent(100), top: percent(-25), justify_content: JustifyContent::Center, ..default() },
            ))
            .with_children(|t| {
                t.spawn((Text::default(), font.clone(), TextShadow { offset: Vec2::splat(5.0), color: Color::BLACK.with_alpha(0.85) }))
                    .with_children(|text| {
                        for (i, ch) in "THE END".chars().enumerate() {
                            text.spawn((TextSpan::new(ch.to_string()), font.clone(), TextColor(Color::WHITE), Letter(i)));
                        }
                    });
            });
        commands
            .spawn((
                Title,
                Node { position_type: PositionType::Absolute, width: percent(100), top: percent(56), justify_content: JustifyContent::Center, ..default() },
            ))
            .with_child((
                Text::new("THE COLORS ARE BACK IN THE WORLD AGAIN"),
                TextFont { font_size: FontSize::Px(38.0), ..default() },
                TextColor(Color::WHITE.with_alpha(0.0)),
                TextShadow { offset: Vec2::splat(3.0), color: Color::BLACK.with_alpha(0.85) },
                Message,
            ));
    }
    // The title drops in (an ease out with a little bounce).
    let k = ((s - TITLE_AT) / TITLE_TIME).clamp(0.0, 1.0);
    let back = 1.0 + 2.2 * (k - 1.0).powi(3) + 1.2 * (k - 1.0).powi(2);
    let top = -25.0 + (30.0 + 25.0) * back;
    let mut rows = rows;
    for (e, drops, mut node) in &mut rows {
        if fade <= 0.0 {
            commands.entity(e).despawn();
        } else if drops {
            node.top = percent(top);
        }
    }
    // Its letters cycle through the colors, in kill order.
    let order = kill_order(&defeated);
    for (letter, mut color) in &mut letters {
        let i = ((now * 4.0) as usize + letter.0) % order.len();
        let next = color_of(&order, i + 1);
        let blend = (now * 4.0).fract();
        let c = Oklaba::from(color_of(&order, i)).mix(&next.into(), blend);
        color.0 = Color::from(c).with_alpha(fade);
    }
    let shown = ((s - MESSAGE_AT) / MESSAGE_TIME).clamp(0.0, 1.0) * fade;
    for (mut color, mut shadow) in &mut message {
        color.0 = Color::WHITE.with_alpha(shown);
        shadow.color = Color::BLACK.with_alpha(0.85 * shown);
    }
    // The title's shadow fades with it.
    for mut shadow in &mut shadows {
        shadow.color = Color::BLACK.with_alpha(0.85 * fade);
    }
}

fn color_of(order: &[BossKind], i: usize) -> Color {
    color(order[i % order.len()])
}

/// The world beyond its colors: every world camera saturated, contrasty, its color casts
/// drifting. In with the show, and for as long as every boss is beaten.
#[allow(clippy::too_many_arguments)]
fn grade(
    time: Res<Time>,
    mut ending: ResMut<Ending>,
    party: Res<Celebration>,
    mut flash: ResMut<ScreenFlash>,
    mut main: Query<&mut ColorGrading, (With<MainCamera>, Without<BackdropCamera>, Without<ColorCamera>)>,
    mut backdrop: Query<&mut ColorGrading, (With<BackdropCamera>, Without<MainCamera>, Without<ColorCamera>)>,
    mut colored: Query<(Entity, Option<&mut ColorGrading>), (With<ColorCamera>, Without<MainCamera>, Without<BackdropCamera>)>,
    mut commands: Commands,
) {
    let dt = time.delta_secs();
    let now = time.elapsed_secs();
    // In with the show, and kept through the celebration.
    let want = match ending.show(wait()) {
        _ if party.0 => 1.0,
        Some(s) => ((s - COLORS_AT) / COLORS_TIME).clamp(0.0, 1.0),
        None => 0.0,
    };
    let before = ending.colors;
    // In with the show's pace; a loaded game with every boss dead comes in quickly; out at once.
    ending.colors = if want < ending.colors { want } else { (ending.colors + dt / if ending.t.is_some() { COLORS_TIME } else { 1.0 }).min(want) };
    if before < 0.05 && ending.colors >= 0.05 && ending.t.is_some() {
        flash.flash(Color::WHITE, 0.6);
    }
    let k = ending.colors;
    if k <= 0.0 {
        if ending.graded {
            // Back to the gray world.
            ending.graded = false;
            for mut g in &mut main {
                *g = camera::world_grading();
            }
            for mut g in &mut backdrop {
                *g = camera::backdrop_grading(0.0);
            }
            for (e, _) in &colored {
                commands.entity(e).remove::<ColorGrading>();
            }
        }
        return;
    }
    ending.graded = true;
    let wild = |base_saturation: f32, extra: f32| {
        let section = ColorGradingSection { saturation: 1.0 + 0.3 * k, contrast: 1.0 + 0.4 * k, ..default() };
        ColorGrading {
            global: ColorGradingGlobal {
                exposure: 0.15 * k,
                temperature: 0.55 * k * (now * 0.5).sin(),
                tint: 0.5 * k * (now * 0.37 + 1.0).cos(),
                hue: 0.6 * k * (now * 0.23).sin(),
                post_saturation: base_saturation + extra * k,
                ..default()
            },
            shadows: section,
            midtones: section,
            highlights: section,
        }
    };
    for mut g in &mut main {
        *g = wild(0.0, 2.0);
    }
    for mut g in &mut backdrop {
        *g = wild(0.0, 2.0);
    }
    for (e, g) in &mut colored {
        match g {
            Some(mut g) => *g = wild(1.0, 0.6),
            None => {
                commands.entity(e).insert(wild(1.0, 0.6));
            }
        }
    }
}

/// The black between the show and the celebration (a full-screen UI node).
#[derive(Component)]
struct Blackout;

/// The credits on the black (their own full-screen column, over it).
#[derive(Component)]
struct Credits;

#[derive(Component)]
struct CreditsLine;

#[derive(Component)]
struct PressEnter;

/// Slowly to black; there the celebration is set up (everyone back, in color, the chameleon fully
/// painted and healed at the start of the game, an auto save) and the credits show; Enter fades
/// back in and the chameleon is free.
#[allow(clippy::too_many_arguments)]
fn blackout(
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    terrain: Res<crate::terrain::Terrain>,
    mut commands: Commands,
    mut ending: ResMut<Ending>,
    mut party: ResMut<Celebration>,
    mut reloaded: MessageWriter<WorldReloaded>,
    mut earned: MessageWriter<ColorEarned>,
    mut autosave: MessageWriter<AutoSave>,
    player: Single<(&mut Player, &mut Transform)>,
    mut follow: Single<&mut camera::Follow, With<MainCamera>>,
    mut black: Query<(Entity, &mut BackgroundColor), With<Blackout>>,
    credits: Query<Entity, With<Credits>>,
    mut lines: Query<&mut TextColor, (With<CreditsLine>, Without<PressEnter>)>,
    mut prompt: Query<&mut TextColor, (With<PressEnter>, Without<CreditsLine>)>,
) {
    let Some(s) = ending.show(wait()) else { return };
    let at = FADE_AT + FADE_TIME;
    if s < at {
        return;
    }
    let (mut p, mut tf) = player.into_inner();
    if black.is_empty() {
        commands.spawn((
            Blackout,
            Node { position_type: PositionType::Absolute, width: percent(100), height: percent(100), ..default() },
            BackgroundColor(Color::BLACK.with_alpha(0.0)),
            GlobalZIndex(100),
        ));
    }
    let dark = at + BLACK_IN;
    if s >= dark && !ending.set_up {
        ending.set_up = true;
        party.0 = true;
        info!("the celebration");
        // Everyone back (`boss::restart_on_reload`; the mobs a frame later), painted by `paint`.
        reloaded.write(WorldReloaded { bosses_moved: true });
        for kind in BossKind::ALL {
            earned.write(ColorEarned(kind.spec().color));
        }
        // At the start of the game, healed.
        let start = player::spawn_point(&terrain);
        tf.translation.x = start.x;
        tf.translation.y = start.y;
        p.settle_at(start);
        p.vel = Vec2::ZERO;
        if p.dead.is_none() {
            p.hp = p.max_hp;
        }
        follow.cut_to(start);
        autosave.write(AutoSave);
        let line = |size: f32| TextFont { font_size: FontSize::Px(size), ..default() };
        commands
            .spawn((
                Credits,
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
                GlobalZIndex(101),
            ))
            .with_children(|c| {
                c.spawn((Text::new("MADE IN SAO PAULO, OCT 2026, BY BIN & ZED"), line(44.0), TextColor(Color::WHITE.with_alpha(0.0)), CreditsLine));
                c.spawn((Text::new("PRESS ENTER TO CONTINUE"), line(24.0), TextColor(Color::WHITE.with_alpha(0.0)), PressEnter));
            });
    }
    if s >= dark && ending.went_on.is_none() && keys.any_just_pressed([KeyCode::Enter, KeyCode::NumpadEnter]) {
        ending.went_on = Some(s);
    }
    let (credits_k, prompt_k, black_k) = match ending.went_on {
        // Fading to black, then the credits and (blinking) the prompt.
        None => {
            let shown = ((s - dark) / CREDITS_IN).clamp(0.0, 1.0);
            let blink = 0.65 + 0.35 * (time.elapsed_secs() * 3.0).sin();
            let prompt = if s >= dark + PROMPT_AT { ((s - dark - PROMPT_AT) / 0.6).min(1.0) * blink } else { 0.0 };
            (shown, prompt, ((s - at) / BLACK_IN).min(1.0))
        }
        // Enter: the credits go, then back from black.
        Some(go) => {
            let gone = 1.0 - ((s - go) / CREDITS_OUT).clamp(0.0, 1.0);
            (gone, 0.0, 1.0 - ((s - go - CREDITS_OUT) / BLACK_OUT).clamp(0.0, 1.0))
        }
    };
    for mut c in &mut lines {
        c.0 = Color::WHITE.with_alpha(credits_k);
    }
    for mut c in &mut prompt {
        c.0 = Color::WHITE.with_alpha(prompt_k.min(credits_k));
    }
    for (_, mut bg) in &mut black {
        bg.0 = Color::BLACK.with_alpha(black_k.clamp(0.0, 1.0));
    }
    if ending.went_on.is_some_and(|go| s > go + CREDITS_OUT + BLACK_OUT) {
        // The show is over: the chameleon is the player's again.
        for (e, _) in &black {
            commands.entity(e).despawn();
        }
        for e in &credits {
            commands.entity(e).despawn();
        }
        ending.t = None;
    }
}

/// A boss or a mob cheering for the chameleon: it hops for joy (`offset` is the hop added on top
/// of where its own AI puts it, taken off again before the AI runs) and throws bursts of its color.
#[derive(Component)]
struct Cheer {
    offset: f32,
    phase: f32,
    rate: f32,
    height: f32,
    color: Color,
    burst: f32,
}

/// The celebration: every boss and mob (new ones too) gets its color, moves to the color layer
/// (the world camera is gray) and starts cheering.
#[allow(clippy::too_many_arguments)]
fn paint(
    mut commands: Commands,
    party: Res<Celebration>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    mut bosses: Query<(Entity, &mut Boss, &crate::boss::Lair, Has<Cheer>)>,
    mut mobs: Query<(Entity, &mut Mob, Has<Cheer>)>,
    children: Query<&Children>,
    meshes: Query<(), With<Mesh2d>>,
) {
    if !party.0 {
        return;
    }
    let mut seed = 0.37_f32;
    let mut cheer = |commands: &mut Commands, e: Entity, height: f32, rate: f32, color: Color| {
        seed = (seed * 7.13 + 0.61).fract();
        for d in children.iter_descendants(e) {
            if meshes.contains(d) {
                commands.entity(d).insert(camera::color_layer());
            }
        }
        commands.entity(e).insert(Cheer { offset: 0.0, phase: seed * TAU, rate: rate * (0.85 + 0.3 * seed), height, color, burst: seed * 3.0 });
    };
    for (e, mut boss, lair, cheering) in &mut bosses {
        if cheering || boss.dead {
            continue;
        }
        // The animal's own color, nothing to do with its blood or the color it gave.
        let color = lair.kind.natural();
        boss.tint(color, &mut materials);
        cheer(&mut commands, e, boss.spec.radius * 0.35, 1.6, color);
    }
    for (e, mut mob, cheering) in &mut mobs {
        if cheering {
            continue;
        }
        mob.tint(&mut materials);
        let color = crate::mobs::natural(mob.kind);
        cheer(&mut commands, e, mob.spec.radius * 0.9 + 6.0, 2.4, color);
    }
}

/// Before anything moves them: the hop comes off, so their AIs see where they really are.
fn unhop(mut cheering: Query<(&mut Cheer, &mut Transform)>) {
    for (mut c, mut tf) in &mut cheering {
        tf.translation.y -= c.offset;
        c.offset = 0.0;
    }
}

/// A bit of confetti in a cheering creature's color (color layer).
#[derive(Component)]
struct Confetti {
    vel: Vec2,
    age: f32,
    life: f32,
    spin: f32,
    material: Handle<ColorMaterial>,
}

/// After everything has moved them: the hop goes on (it shows), and now and then a burst.
fn cheer(
    time: Res<Time>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    mut bit: Local<Option<Handle<Mesh>>>,
    mut rng: Local<Option<Rng>>,
    mut cheering: Query<(&mut Cheer, &mut Transform)>,
) {
    let dt = time.delta_secs();
    let now = time.elapsed_secs();
    let rng = rng.get_or_insert(Rng(0xC4EE_2001));
    let bit = bit.get_or_insert_with(|| meshes.add(Rectangle::new(1.0, 1.0))).clone();
    for (mut c, mut tf) in &mut cheering {
        let hop = (now * c.rate * std::f32::consts::PI + c.phase).sin().abs().powf(0.8) * c.height;
        c.offset = hop;
        tf.translation.y += hop;
        c.burst -= dt;
        if c.burst <= 0.0 {
            c.burst = rng.range(1.5, 4.0);
            let at = tf.translation.truncate() + Vec2::Y * c.height * 1.5;
            for _ in 0..10 {
                let material = materials.add(ColorMaterial { color: c.color, alpha_mode: AlphaMode2d::Blend, ..default() });
                let vel = Vec2::from_angle(rng.range(0.4, 2.7)) * rng.range(180.0, 380.0);
                commands.spawn((
                    Mesh2d(bit.clone()),
                    MeshMaterial2d(material.clone()),
                    Transform::from_translation(at.extend(16.0)).with_scale(Vec3::new(rng.range(4.0, 7.0), rng.range(2.0, 4.0), 1.0)),
                    camera::color_layer(),
                    Confetti { vel, age: 0.0, life: rng.range(0.8, 1.4), spin: rng.range(-12.0, 12.0), material },
                ));
            }
        }
    }
}

fn confetti(time: Res<Time>, mut commands: Commands, mut materials: ResMut<Assets<ColorMaterial>>, mut bits: Query<(Entity, &mut Confetti, &mut Transform)>) {
    let dt = time.delta_secs();
    for (e, mut c, mut tf) in &mut bits {
        c.age += dt;
        let k = c.age / c.life;
        if k >= 1.0 {
            commands.entity(e).despawn();
            continue;
        }
        c.vel.y -= 700.0 * dt;
        c.vel *= (-1.5 * dt).exp();
        tf.translation += (c.vel * dt).extend(0.0);
        tf.rotate_z(c.spin * dt);
        if let Some(mut m) = materials.get_mut(&c.material) {
            m.color = m.color.with_alpha(1.0 - k * k);
        }
    }
}
