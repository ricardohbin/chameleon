//! Cobwebs (the catacombs' props, `terrain_props/catacombs.rs`) are sticky:
//! the chameleon and every creature but the spiders move slower while they
//! are in one.
//!
//! The curtains spun across corridors (`terrain_props/curtains.rs`) wear out:
//! slashing at them, pushing through them and thrashing in them tear strands
//! off until the whole thing breaks apart and fades, and they vibrate like
//! taut silk while something is caught.

use bevy::prelude::*;

use crate::{
    chameleon::{self, ChameleonAnim},
    mobs::{Mob, MobKind, State},
    player::{self, Player},
};

/// How much of its pull a web keeps at its edge (it is strongest in the middle).
const EDGE: f32 = 0.5;
/// A curtain pulls this much harder than a fan web (1 = a fan's full grip): the slowdown
/// formulas clamp at a crawl, so a new curtain nearly pins whatever is in it.
const STRENGTH: f32 = 1.6;
/// A curtain's wear (1 = new, 0 = gone) per slash that cuts it, per full pass
/// through it, per key tap while caught in it, and per second of pushing.
const SLASH: f32 = 0.34;
const PASS: f32 = 0.25;
const TAP: f32 = 0.07;
const PUSH: f32 = 0.04;
/// Faster than this inside a curtain (a dash) rips it apart.
const RIP_SPEED: f32 = 700.0;
/// Most the curtain vibrates and how fast it settles (per second).
const MAX_KICK: f32 = 0.3;
const SETTLE: f32 = 5.0;
/// How fast the taut silk vibrates (rad/s).
const BUZZ: f32 = 55.0;
/// Seconds a torn curtain takes to break apart and fade out.
const FALL_TIME: f32 = 0.7;
/// Opacity of a new curtain's silk.
const SILK_ALPHA: f32 = 0.25;

pub struct WebPlugin;

impl Plugin for WebPlugin {
    fn build(&self, app: &mut App) {
        // Before the walk, so the slowdown is read by the very frame it applies to.
        app.add_systems(Update, (stick.before(player::move_player), wear.after(stick), sway.after(wear)));
    }
}

/// A web on a prop's root entity: where it hangs, in the root's local units.
#[derive(Component)]
pub struct Web {
    pub center: Vec2,
    pub radius: f32,
}

/// An orb web across a whole corridor (on its root, which hangs from the ceiling).
#[derive(Component)]
pub struct Curtain {
    /// The sticky zone, in world units.
    center: Vec2,
    half: Vec2,
    /// Strand groups, torn off one by one as `hp` drops.
    groups: Vec<Entity>,
    /// Its own silk material, faded out when it breaks.
    silk: Handle<ColorMaterial>,
    /// 1 = new, 0 = torn down.
    hp: f32,
    /// Vibration amplitude and its phase.
    kick: f32,
    phase: f32,
    /// Which side (-1/1) the chameleon came in from, while it is inside.
    entered: Option<f32>,
    /// The last slash that cut it.
    slash: u32,
    /// Seconds since it was torn down (0 = still up).
    fall: f32,
}

impl Curtain {
    pub fn new(center: Vec2, half: Vec2, groups: Vec<Entity>, silk: Handle<ColorMaterial>) -> Self {
        Self { center, half, groups, silk, hp: 1.0, kick: 0.0, phase: 0.0, entered: None, slash: 0, fall: 0.0 }
    }

    /// Wears it out by `by`, vibrating it by `kick`.
    fn hurt(&mut self, by: f32, kick: f32) {
        self.hp -= by;
        self.kick = (self.kick + kick).min(MAX_KICK);
    }

    /// 0 outside, up to 1 deep inside, for a body of half size `body` at `at`.
    fn grip(&self, at: Vec2, body: Vec2) -> f32 {
        if self.fall > 0.0 {
            return 0.0;
        }
        let d = (at.x - self.center.x).abs() - body.x;
        if d >= self.half.x || (at.y - self.center.y).abs() >= self.half.y + body.y {
            return 0.0;
        }
        STRENGTH * (1.0 - (d.max(0.0) / self.half.x) * EDGE) * (0.4 + 0.6 * self.hp.clamp(0.0, 1.0))
    }
}

/// 0 outside the web, up to 1 deep inside it.
fn grip(web: Vec2, radius: f32, at: Vec2, body: f32) -> f32 {
    let d = web.distance(at) - body;
    if d >= radius {
        0.0
    } else {
        (1.0 - (d.max(0.0) / radius) * EDGE).clamp(0.0, 1.0)
    }
}

/// The chameleon's collision box inside a curtain (a little narrower than its body).
fn player_body() -> Vec2 {
    Vec2::new(player::HALF.x * 0.6, player::HALF.y)
}

pub(crate) fn stick(
    webs: Query<(&Web, &GlobalTransform)>,
    curtains: Query<&Curtain>,
    mut player: Single<(&mut Player, &Transform)>,
    mut mobs: Query<(&mut Mob, &Transform)>,
) {
    let world: Vec<(Vec2, f32)> = webs
        .iter()
        .map(|(w, gt)| (gt.transform_point(w.center.extend(0.0)).truncate(), w.radius * gt.scale().y.abs()))
        .collect();
    // Fans are round (a body of `radius`), curtains are boxes.
    let most = |at: Vec2, body: Vec2| {
        let fans = world.iter().map(|&(c, r)| grip(c, r, at, body.x)).fold(0.0, f32::max);
        curtains.iter().map(|c| c.grip(at, body)).fold(fans, f32::max)
    };
    let (p, tf) = &mut *player;
    p.webbed = most(tf.translation.truncate(), player_body());
    for (mut m, tf) in &mut mobs {
        let spider = matches!(m.kind, MobKind::Spider | MobKind::Spiderling | MobKind::CryptJumper | MobKind::GraveTrapdoor | MobKind::OssuaryWolf) || matches!(m.state, State::Dead { .. });
        m.webbed = if spider { 0.0 } else { most(tf.translation.truncate(), Vec2::splat(m.spec.radius * 0.5)) };
    }
}

/// The chameleon wears curtains out: cutting, pushing through, thrashing about.
fn wear(
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    player: Single<(&Player, &Transform, &ChameleonAnim)>,
    mut curtains: Query<&mut Curtain>,
) {
    let (p, tf, anim) = *player;
    if p.dead.is_some() {
        return;
    }
    let dt = time.delta_secs();
    let at = tf.translation.truncate();
    let moving = [KeyCode::KeyA, KeyCode::KeyD, KeyCode::ArrowLeft, KeyCode::ArrowRight];
    let tapping = [KeyCode::KeyA, KeyCode::KeyD, KeyCode::ArrowLeft, KeyCode::ArrowRight, KeyCode::Space, KeyCode::KeyW, KeyCode::ArrowUp];
    let center = chameleon::slash_center(at, p.facing);
    for mut c in &mut curtains {
        if c.fall > 0.0 {
            continue;
        }
        // The claws cut whatever strands they sweep through (once per slash).
        if anim.striking() && c.slash != anim.slash_id {
            let near = center.clamp(c.center - c.half, c.center + c.half);
            if anim.reaches(at, p.facing, near, 0.0) {
                c.slash = anim.slash_id;
                // The touch of death (F3) cuts it down in one blow.
                c.hurt(if p.god.smites() { 1.0 } else { SLASH }, 0.18);
            }
        }
        let side = at.x - c.center.x;
        if c.grip(at, player_body()) > 0.0 {
            c.entered.get_or_insert(side.signum());
            if p.vel.length() > RIP_SPEED {
                c.hurt(1.0, MAX_KICK);
                continue;
            }
            // Thrashing: every tap of a move or jump key, and pushing on.
            if keys.any_just_pressed(tapping) {
                c.hurt(TAP, 0.12);
            }
            if keys.any_pressed(moving) {
                c.hurt(PUSH * dt, 0.0);
            }
            // Whatever is caught makes it hum.
            let hum = 0.04 + p.vel.x.abs() / 2500.0;
            c.kick = c.kick.max(hum.min(MAX_KICK));
        } else if let Some(from) = c.entered.take() {
            // Out the far side: it was pushed through.
            if from != side.signum() {
                c.hurt(PASS, 0.25);
            }
        }
    }
}

/// Vibrates the curtains (taut silk, anchored at both ends), thins them as
/// they wear out, and breaks the torn ones apart, fading them out.
fn sway(
    time: Res<Time>,
    mut commands: Commands,
    mut materials: ResMut<Assets<ColorMaterial>>,
    mut curtains: Query<(Entity, &mut Curtain, &mut Transform)>,
    mut parts: Query<(&mut Visibility, &mut Transform), Without<Curtain>>,
) {
    let dt = time.delta_secs();
    for (e, mut c, mut tf) in &mut curtains {
        if c.fall == 0.0 && c.hp <= 0.0 {
            c.fall = f32::EPSILON;
        }
        if c.fall > 0.0 {
            // The strands let go: each group drifts down and away, turning, while it fades.
            c.fall += dt;
            let t = c.fall;
            let k = (t / FALL_TIME).min(1.0);
            tf.scale.x = 1.0;
            for (i, &g) in c.groups.iter().enumerate() {
                if let Ok((_, mut gt)) = parts.get_mut(g) {
                    let s = if i % 2 == 0 { 1.0 } else { -1.0 };
                    gt.translation = Vec3::new(s * (6.0 + 9.0 * i as f32) * t, -(30.0 + 25.0 * i as f32) * t - 260.0 * t * t, 0.0);
                    gt.rotation = Quat::from_rotation_z(s * (0.2 + 0.15 * i as f32) * t);
                }
            }
            if let Some(mut m) = materials.get_mut(&c.silk) {
                m.color.set_alpha(SILK_ALPHA * (1.0 - k));
            }
            if k >= 1.0 {
                commands.entity(e).try_despawn();
            }
            continue;
        }
        c.phase += dt * BUZZ;
        c.kick *= (-SETTLE * dt).exp();
        // Pulses across the web; the ceiling and floor anchors sit on the axis, so they stay put.
        tf.scale.x = 1.0 + c.kick * 0.4 * c.phase.sin();
        let n = c.groups.len();
        for (i, &g) in c.groups.iter().enumerate() {
            // Group i goes when only (n - 1 - i) / n of the curtain is left.
            let gone = c.hp <= 1.0 - (i + 1) as f32 / n as f32;
            if let Ok((mut v, _)) = parts.get_mut(g) {
                v.set_if_neq(if gone { Visibility::Hidden } else { Visibility::Inherited });
            }
        }
    }
}
