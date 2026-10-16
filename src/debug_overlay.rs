//! F1: debug overlay for reporting problems. A text panel (position in world
//! units, sketch pixels from the origin marker and canvas pixels; movement and
//! ground contact; region and features; state, distance and direction of the
//! boss whose lair the chameleon is in) plus world drawings: the chameleon's
//! box, velocity, ground normal and foot samples, region/feature/lair/arena
//! outlines, moving platforms, and what each boss senses (line of sight, goal,
//! planned path). The panel is a strip along the bottom of the screen (the
//! chameleon on the left, its surroundings and the boss in the middle, the F
//! keys on the right).
//! With the overlay on, F12 also saves the panel text next to the screenshot.

use bevy::prelude::*;

use crate::{
    boss::{Boss, Defeated, Lair, Sense},
    camera,
    level::Platform,
    player::{self, Player},
    terrain::{Terrain, PX},
    terrain_art::Contrast,
};

pub struct DebugOverlayPlugin;

impl Plugin for DebugOverlayPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<DebugInfo>()
            .add_systems(Startup, setup)
            .add_systems(Update, (toggle, (describe, draw).run_if(|d: Res<DebugInfo>| d.on)).chain().after(player::move_player));
    }
}

/// The overlay's state and its latest panel text.
#[derive(Resource, Default)]
pub struct DebugInfo {
    pub on: bool,
    pub text: String,
}

/// The panel strip along the bottom of the screen, with two text columns.
#[derive(Component)]
pub(crate) struct Panel;

/// A text column of the panel: 0 = the chameleon, 1 = the world around it,
/// 2 = the F keys.
#[derive(Component)]
struct Column(usize);

const DIM: Color = Color::srgba(1.0, 1.0, 1.0, 0.25);
const REGION: Color = Color::srgb(0.3, 0.6, 1.0);
const FEATURE: Color = Color::srgb(1.0, 0.4, 0.9);
const ARENA: Color = Color::srgb(1.0, 0.6, 0.1);
const GOOD: Color = Color::srgb(0.2, 1.0, 0.3);
const BAD: Color = Color::srgb(1.0, 0.2, 0.2);
const VEL: Color = Color::srgb(1.0, 1.0, 0.2);
const PATH: Color = Color::srgb(0.2, 0.9, 1.0);

fn setup(mut commands: Commands, mut store: ResMut<GizmoConfigStore>) {
    // On the color layer: the world camera is grayscale.
    let (config, _) = store.config_mut::<DefaultGizmoConfigGroup>();
    config.render_layers = camera::color_layer();
    config.line.width = 2.0;
    config.enabled = false;
    let font = TextFont { font_size: FontSize::Px(12.0), ..default() };
    let color = TextColor(Color::srgb(0.9, 0.95, 0.9));
    commands.spawn((
        Panel,
        BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.65)),
        Node {
            position_type: PositionType::Absolute,
            bottom: px(0),
            left: px(0),
            right: px(0),
            flex_direction: FlexDirection::Row,
            column_gap: px(24),
            padding: UiRect::axes(px(10), px(6)),
            ..default()
        },
        Visibility::Hidden,
        GlobalZIndex(10),
        children![
            (Column(0), Text::new(""), font.clone(), color, Node { flex_basis: percent(45), ..default() }),
            (Column(1), Text::new(""), font.clone(), color, Node { flex_basis: percent(40), ..default() }),
            (Column(2), Text::new(""), font, color, Node { flex_basis: percent(15), flex_shrink: 0.0, ..default() }),
        ],
    ));
}

pub(crate) fn toggle(
    keys: Res<ButtonInput<KeyCode>>,
    mut info: ResMut<DebugInfo>,
    mut store: ResMut<GizmoConfigStore>,
    mut panel: Single<&mut Visibility, With<Panel>>,
) {
    if !keys.just_pressed(KeyCode::F1) {
        return;
    }
    info.on = !info.on;
    store.config_mut::<DefaultGizmoConfigGroup>().0.enabled = info.on;
    **panel = if info.on { Visibility::Inherited } else { Visibility::Hidden };
}

/// Sketch pixels from the origin marker (x right, y down), as in `cargo run -- diff`.
fn sketch_px(p: Vec2) -> IVec2 {
    IVec2::new((p.x / PX).floor() as i32, (-p.y / PX).floor() as i32)
}

fn direction(d: Vec2) -> &'static str {
    const NAMES: [&str; 8] = ["right", "up-right", "up", "up-left", "left", "down-left", "down", "down-right"];
    let a = d.y.atan2(d.x).rem_euclid(std::f32::consts::TAU);
    NAMES[((a / (std::f32::consts::TAU / 8.0)).round() as usize) % 8]
}

/// The F keys, for the panel's last column.
fn keys(contrast: f32) -> String {
    [
        "F1   this overlay".to_string(),
        "F2   next boss's lair".into(),
        "F3   god mode: invulnerable / + touch of death".into(),
        "F4   flight".into(),
        "F5 / F9   save / load".into(),
        format!("F6 / F7   contrast {contrast:+.2}"),
        "F8   restart".into(),
        "P / Pause     pause".into(),
        "F10   export full map (F1: annotations)".into(),
        "F11   fullscreen".into(),
        "F12   screenshot".into(),
    ]
    .join("
")
}

fn describe(
    time: Res<Time>,
    contrast: Res<Contrast>,
    defeated: Res<Defeated>,
    terrain: Res<Terrain>,
    mut info: ResMut<DebugInfo>,
    mut columns: Query<(&Column, &mut Text)>,
    player: Single<(&Player, &Transform)>,
    platforms: Query<&Platform>,
    bosses: Query<(&Boss, &Lair, &Transform, Option<&Sense>)>,
) {
    let (p, tf) = *player;
    let pos = tf.translation.truncate();
    let feet = pos - Vec2::Y * player::HALF.y;
    let sk = sketch_px(feet);
    let canvas = terrain.canvas_origin + sk;
    let mut t = String::new();
    let mut line = |s: String| {
        t.push_str(&s);
        t.push('\n');
    };
    line(format!("time {:.1}s   fps {:.0}", time.elapsed_secs(), 1.0 / time.delta_secs().max(1e-4)));
    line(format!("feet  world ({:.0}, {:.0})", feet.x, feet.y));
    line(format!("      sketch px from origin ({}, {})   canvas px ({}, {})", sk.x, sk.y, canvas.x, canvas.y));
    line(format!("vel ({:.0}, {:.0})  speed {:.0}   facing {}", p.vel.x, p.vel.y, p.vel.length(), if p.facing > 0.0 { "right" } else { "left" }));

    let ground = if !p.grounded {
        "in the air".to_string()
    } else if let Some(pl) = p.ground.and_then(|e| platforms.get(e).ok()) {
        format!("on a platform ({}{})", if pl.one_way { "one-way" } else { "solid" }, if pl.stable { "" } else { ", moving/unstable" })
    } else {
        "on terrain".to_string()
    };
    line(format!(
        "ground: {ground}   hp {}   {}{}{}",
        p.hp,
        if p.dead.is_some() { "DEAD" } else { "" },
        match p.god {
            player::God::Off => "",
            player::God::Invulnerable => "   INVULNERABILITY (F3)",
            player::God::Smite => "   INVULNERABILITY + TOUCH OF DEATH (F3)",
        },
        if p.flying { "   FLYING (F4)" } else { "" }
    ));
    match terrain.slope_at(feet) {
        Some(s) => line(format!(
            "slope {:+.2} ({:.0} deg, {})   slippery {}",
            s,
            s.abs().atan().to_degrees(),
            if s.abs() < 0.05 {
                "flat"
            } else if s > 0.0 {
                "uphill to the right"
            } else {
                "uphill to the left"
            },
            if terrain.slides_at(feet) { "YES" } else { "no" }
        )),
        None => line("slope: none measured (no ground near the feet)".into()),
    }
    line(format!(
        "sliding {}   dashing {}   stun {:.2}   invuln {:.2}   step {:+.1}",
        if p.slipping > 0.0 { format!("yes ({:.2}s)", p.slipping) } else { "no".into() },
        if p.dash > 0.0 { "yes" } else { "no" },
        p.stun,
        p.invuln,
        p.step
    ));
    line(format!("clearance (to nearest solid) {:.0}", terrain.clearance(pos)));
    let powers: Vec<&str> = [(defeated.dash(), "dash"), (defeated.spit(), "venom spit")].into_iter().filter(|p| p.0).map(|p| p.1).collect();
    line(format!(
        "powers: {}   aim {:.0} deg",
        if powers.is_empty() { "none".to_string() } else { powers.join(", ") },
        p.aim.to_angle().to_degrees()
    ));

    let split = t.len();
    let mut line = |s: String| {
        t.push_str(&s);
        t.push('\n');
    };

    let region = terrain.areas.iter().find(|a| a.rect.contains(pos)).map(|a| a.name.clone());
    match region {
        Some(r) => line(format!("{r}   (style {:?})", terrain.style_at(pos))),
        None => line(format!("outside any region (nearest: {})", terrain.area_at(pos).map_or("none", |a| a.name.as_str()))),
    }
    for f in terrain.features_at(pos) {
        line(f.name.clone());
    }

    // Only the boss whose lair the chameleon is in.
    for (boss, lair, btf, sense) in bosses.iter().filter(|(_, lair, ..)| lair.region.contains(pos)) {
        let bpos = btf.translation.truncate();
        let d = bpos - pos;
        let state = if boss.dead {
            "dead"
        } else if boss.awake {
            "AWAKE"
        } else {
            "asleep"
        };
        let mut s = format!(
            "{} [{state}] hp {}/{}  {:.0} u to the {}  at sketch ({}, {})",
            boss.spec.name,
            boss.hp.max(0),
            boss.spec.hp,
            d.length(),
            direction(d),
            sketch_px(bpos).x,
            sketch_px(bpos).y
        );
        s.push_str(&format!("\n   you: in its lair, {} its arena", if lair.arena.contains(pos) { "in" } else { "outside" }));
        if let Some(sense) = sense {
            s.push_str(&format!(
                "   sees you: {}{}",
                if sense.sees_player { "yes" } else { "no" },
                if sense.path.is_empty() { String::new() } else { format!("   path {} steps", sense.path.len()) }
            ));
        }
        line(s);
    }
    for (column, mut text) in &mut columns {
        text.0 = match column.0 {
            0 => t[..split].trim_end().to_string(),
            1 => t[split..].trim_end().to_string(),
            _ => keys(contrast.0),
        };
    }
    info.text = t;
}

fn draw(
    mut gizmos: Gizmos,
    terrain: Res<Terrain>,
    player: Single<(&Player, &Transform)>,
    platforms: Query<(&Platform, &Transform)>,
    bosses: Query<(&Boss, &Lair, &Transform, Option<&Sense>)>,
) {
    let (p, tf) = *player;
    let pos = tf.translation.truncate();
    let feet = pos - Vec2::Y * player::HALF.y;

    // Regions and features.
    for a in &terrain.areas {
        let color = if a.rect.contains(pos) { REGION } else { REGION.with_alpha(0.3) };
        gizmos.rect_2d(a.rect.center(), a.rect.size(), color);
    }
    for f in &terrain.features {
        gizmos.rect_2d(f.rect.center(), f.rect.size(), FEATURE);
    }
    gizmos.cross_2d(Vec2::ZERO, 24.0, FEATURE);

    // Moving platforms' collision boxes.
    for (pl, ptf) in &platforms {
        let color = if pl.solid { DIM } else { BAD.with_alpha(0.3) };
        gizmos.rect_2d(ptf.translation.truncate(), pl.half * 2.0, color);
    }

    // The chameleon: box, velocity, ground normal and foot samples.
    gizmos.rect_2d(pos, player::HALF * 2.0, if p.grounded { GOOD } else { VEL });
    if p.vel.length() > 1.0 {
        gizmos.arrow_2d(pos, pos + p.vel * 0.25, VEL);
    }
    if let Some(s) = terrain.slope_at(feet) {
        let tangent = Vec2::new(1.0, s).normalize();
        let normal = Vec2::new(-s, 1.0).normalize();
        let steep = s.abs() > 1.0;
        gizmos.line_2d(feet - tangent * 40.0, feet + tangent * 40.0, if steep { BAD } else { GOOD });
        gizmos.arrow_2d(feet, feet + normal * 50.0, if steep { BAD } else { GOOD });
    }
    for i in 0..9 {
        let x = -player::HALF.x + 2.0 * player::HALF.x * i as f32 / 8.0;
        let at = feet + Vec2::new(x, -2.0);
        gizmos.circle_2d(at, 1.5, if terrain.solid_at(at) { GOOD } else { BAD });
    }

    // Bosses: lair, arena, line to it, and what it senses.
    for (boss, lair, btf, sense) in &bosses {
        let bpos = btf.translation.truncate();
        gizmos.rect_2d(lair.region.center(), lair.region.size(), DIM);
        gizmos.rect_2d(lair.arena.center(), lair.arena.size(), ARENA);
        let color = if boss.dead {
            DIM
        } else if boss.awake {
            BAD
        } else {
            ARENA
        };
        gizmos.line_2d(pos, bpos, color.with_alpha(0.5));
        gizmos.circle_2d(bpos, boss.spec.radius, color);
        gizmos.circle_2d(boss.home, 6.0, DIM);
        let Some(sense) = sense else { continue };
        if boss.awake && !boss.dead {
            gizmos.line_2d(bpos, pos, if sense.sees_player { GOOD } else { BAD });
            if let Some(goal) = sense.goal {
                gizmos.cross_2d(goal, 14.0, ARENA);
            }
            if let Some(target) = sense.target {
                gizmos.arrow_2d(bpos, target, PATH);
            }
            let mut prev = bpos;
            for &step in &sense.path {
                gizmos.line_2d(prev, step, PATH);
                prev = step;
            }
        }
    }
}
