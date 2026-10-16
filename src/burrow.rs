//! Burrowing (earned from the Burrower): through rubble, like the centipede.
//!
//! Areas drawn as "feature: rubble" in the sketch are loose rock the chameleon can
//! carve through. Standing at the rubble and aiming (the mouse) at it, `E` digs in
//! and the chameleon comes out on the other side. The straight line along the aim must
//! go through nothing but rubble before it reaches open air (any other solid rock in
//! the way and it can't be done), and there must be room to stand where it comes out.
//! While it's possible a hint says so, and diamonds mark where it goes in and out.
//! While burrowing it is hidden and untouchable; `player::move_player` is paused.

use bevy::{prelude::*, sprite_render::AlphaMode2d};

use crate::{
    boss::Defeated,
    camera::{self, Shake},
    centipede::{self, Assets3},
    chameleon,
    player::{self, HALF, Player},
    shapes::*,
    terrain::Terrain,
};

/// How far from the body the rubble may start, and the most rubble it can cross.
const REACH: f32 = 30.0;
const MAX_LEN: f32 = 420.0;
/// Time going in, and coming out; speed underground.
const DIG: f32 = 0.28;
const SPEED: f32 = 650.0;
const PURPLE: Color = Color::srgb_u8(0xa2, 0x00, 0xa2);

/// A way through the rubble.
#[derive(Clone, Copy)]
pub struct Path {
    /// Where the rubble starts, where it ends (first open air), and where the chameleon ends up.
    pub entry: Vec2,
    pub exit: Vec2,
    pub land: Vec2,
    pub dir: Vec2,
}

/// A burrow in progress.
#[derive(Clone, Copy)]
pub struct Burrow {
    pub path: Path,
    pub from: Vec2,
    pub t: f32,
    pub dug_in: bool,
    pub dug_out: bool,
}

impl Burrow {
    fn total(&self) -> f32 {
        DIG + self.path.entry.distance(self.path.exit) / SPEED + DIG
    }
}

pub struct BurrowPlugin;

impl Plugin for BurrowPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn_ui).add_systems(
            Update,
            (
                scan.after(player::move_player).before(chameleon::update_anim_state),
                travel.after(player::move_player).before(camera::follow_player),
            ),
        );
    }
}

/// The straight way through rubble along `aim` from the chameleon at `ppos`, if there is one.
pub fn find(terrain: &Terrain, ppos: Vec2, aim: Vec2) -> Option<Path> {
    let dir = aim.normalize_or_zero();
    if dir == Vec2::ZERO {
        return None;
    }
    let rubble = |p: Vec2| terrain.features.iter().any(|f| f.rubble && f.rect.contains(p));
    // How far the body reaches from its center in that direction.
    let ext = (HALF.x / dir.x.abs().max(1e-3)).min(HALF.y / dir.y.abs().max(1e-3));
    let mut d = 0.0;
    while !terrain.solid_at(ppos + dir * d) {
        d += 3.0;
        if d > ext + REACH {
            return None;
        }
    }
    let entry = ppos + dir * d;
    if !rubble(entry) {
        return None;
    }
    // Through it, only rubble all the way.
    let start = d;
    let exit = loop {
        d += 3.0;
        let p = ppos + dir * d;
        if !terrain.solid_at(p) {
            break p;
        }
        if !rubble(p) || d - start > MAX_LEN {
            return None;
        }
    };
    // Room to stand where it comes out.
    let mut land = exit + dir * ext;
    for _ in 0..25 {
        if !terrain.box_hits(land, HALF) {
            return Some(Path { entry, exit, land, dir });
        }
        land += dir * 4.0;
    }
    None
}

#[derive(Component)]
struct Prompt;

#[derive(Component)]
struct Mark(bool);

fn spawn_ui(mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>, mut materials: ResMut<Assets<ColorMaterial>>) {
    commands.spawn((
        Prompt,
        Text::new(""),
        TextFont { font_size: FontSize::Px(24.0), ..default() },
        TextColor(Color::NONE),
        TextLayout::justify(Justify::Center),
        Node { position_type: PositionType::Absolute, bottom: percent(16), width: percent(100), ..default() },
    ));
    let mesh = meshes.add(fill(&ellipse_pts(9.0, 14.0, 4), |_| 1.0));
    let mat = materials.add(ColorMaterial { color: PURPLE.with_alpha(0.8), alpha_mode: AlphaMode2d::Blend, ..default() });
    for exit in [false, true] {
        commands.spawn((Mark(exit), Mesh2d(mesh.clone()), MeshMaterial2d(mat.clone()), Transform::from_xyz(0.0, 0.0, 13.0), Visibility::Hidden));
    }
}

/// Looks for a way through (and says so), and starts digging when `E` is pressed.
fn scan(
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    defeated: Res<Defeated>,
    terrain: Res<Terrain>,
    player: Single<(&mut Player, &Transform)>,
    mut prompt: Single<(&mut Text, &mut TextColor), With<Prompt>>,
    mut marks: Query<(&Mark, &mut Transform, &mut Visibility), Without<Player>>,
) {
    let (mut p, ptf) = player.into_inner();
    let ppos = ptf.translation.truncate();
    let able = defeated.burrow() && p.dead.is_none() && p.burrow.is_none() && !p.flying && !p.swimming && p.stun <= 0.0;
    p.burrow_target = if able { find(&terrain, ppos, p.aim) } else { None };

    // Near rubble: say what to do.
    let near = able && terrain.features.iter().any(|f| f.rubble && f.rect.inflate(120.0).contains(ppos));
    let (text, alpha) = match (p.burrow_target, near) {
        (Some(_), _) => ("[E]  BURROW THROUGH THE RUBBLE", 0.95),
        (None, true) => ("RUBBLE: aim through it, to the other side, and press E", 0.55),
        _ => ("", 0.0),
    };
    if prompt.0.0 != text {
        prompt.0.0 = text.into();
    }
    prompt.1.0 = Color::srgb(0.85, 0.45, 0.85).with_alpha(alpha);

    let pulse = 1.0 + 0.2 * (time.elapsed_secs() * 7.0).sin();
    for (mark, mut tf, mut vis) in &mut marks {
        match p.burrow_target {
            Some(path) => {
                let at = if mark.0 { path.exit } else { path.entry };
                tf.translation = at.extend(13.0);
                tf.rotation = Quat::from_rotation_z(path.dir.to_angle());
                tf.scale = Vec3::splat(pulse * if mark.0 { 0.7 } else { 1.0 });
                *vis = Visibility::Inherited;
            }
            None => *vis = Visibility::Hidden,
        }
    }

    if keys.just_pressed(KeyCode::KeyE) {
        if let Some(path) = p.burrow_target {
            p.burrow = Some(Burrow { path, from: ppos, t: 0.0, dug_in: false, dug_out: false });
            p.vel = Vec2::ZERO;
            p.climbing = None;
            p.dash = 0.0;
        }
    }
}

/// The dig itself: in (rubble flies), through unseen, out (rubble flies).
fn travel(
    time: Res<Time>,
    mut commands: Commands,
    mut shake: ResMut<Shake>,
    assets: Option<Res<Assets3>>,
    player: Single<(&mut Player, &mut Transform, &mut Visibility)>,
) {
    let (mut p, mut tf, mut vis) = player.into_inner();
    let Some(mut b) = p.burrow else { return };
    if p.dead.is_some() {
        p.burrow = None;
        *vis = Visibility::Inherited;
        return;
    }
    let dt = time.delta_secs().min(1.0 / 30.0);
    b.t += dt;
    let total = b.total();
    let path = b.path;
    let ease = |x: f32| {
        let x = x.clamp(0.0, 1.0);
        x * x * (3.0 - 2.0 * x)
    };
    let mut rng = Rng(time.elapsed().subsec_nanos() | 1);
    let pos = if b.t < DIG {
        b.from.lerp(path.entry + path.dir * 8.0, ease(b.t / DIG))
    } else if b.t < total - DIG {
        if !b.dug_in {
            b.dug_in = true;
            shake.add(0.3);
            if let Some(a) = &assets {
                centipede::burst(&mut commands, a, &mut rng, path.entry, -path.dir, 14, 330.0);
            }
        }
        path.entry.lerp(path.exit, ((b.t - DIG) / (total - 2.0 * DIG).max(0.01)).clamp(0.0, 1.0))
    } else {
        if !b.dug_out {
            b.dug_out = true;
            shake.add(0.35);
            if let Some(a) = &assets {
                centipede::burst(&mut commands, a, &mut rng, path.exit, path.dir, 16, 380.0);
            }
        }
        path.exit.lerp(path.land, ease((b.t - (total - DIG)) / DIG))
    };
    tf.translation.x = pos.x;
    tf.translation.y = pos.y;
    *vis = if b.dug_in && !b.dug_out { Visibility::Hidden } else { Visibility::Inherited };
    p.vel = Vec2::ZERO;
    p.guard = p.guard.max(0.15);
    if b.t >= total {
        p.burrow = None;
        p.vel = path.dir * 120.0;
        p.grounded = false;
        p.forget_fall(path.land);
        *vis = Visibility::Inherited;
    } else {
        p.burrow = Some(b);
    }
}
