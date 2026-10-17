//! The chameleon: a procedural, fully code-drawn character rig.
//!
//! Everything is built from shaded meshes (see `shapes.rs`) arranged in a
//! bone-like hierarchy, so it can be animated procedurally:
//! walk cycle, curling tail, darting eye, claw slash (and the air spin) and camouflage.
//!
//! Local coordinates: the chameleon faces +X, origin = `player::RIG_DROP` below the player collider center.

use std::f32::consts::{PI, TAU};

use bevy::{prelude::*, sprite_render::AlphaMode2d};

use crate::{
    boss::Defeated,
    camera,
    gore::PaintTarget,
    level::{self, Restart},
    player::{self, Player},
    shapes::*,
};

/// Ash/bone white. Colors are earned back through the game.
const DEFAULT_SKIN: Color = Color::srgb(0.86, 0.85, 0.83);
const MOUTH: Color = Color::srgb(0.05, 0.04, 0.04);
const BONE: Color = Color::srgb(0.95, 0.93, 0.88);
const EYE_GLOW: Color = Color::srgb(1.0, 0.96, 0.85);
const WAVE: Color = Color::srgb(0.95, 0.94, 0.90);
const SLASH_TIME: f32 = 0.28;
/// A bite into remains (`E`, see `feeding.rs`).
const BITE_TIME: f32 = 0.45;
/// Part of the slash (as a fraction of `SLASH_TIME`) where the claws cut.
const STRIKE: (f32, f32) = (0.3, 0.52);
/// Slash wave, in rig units: centered on the swiping shoulder.
const WAVE_CENTER: Vec2 = Vec2::new(24.0, -2.0);
const WAVE_RADIUS: f32 = 60.0;
/// The slash hitbox: a half-disc of this radius (world units) around the
/// swiping shoulder, on the side the slash is aimed at. Matches the wave.
pub const SLASH_REACH: f32 = WAVE_RADIUS * player::SCALE * 1.15;
/// The scythe claws (earned from the mantis): the slash reaches this much farther
/// (the wave grows with it) and takes `SCYTHE_DAMAGE` health instead of 1.
pub const SCYTHE_REACH: f32 = 1.35;
pub const SCYTHE_DAMAGE: i32 = 2;
/// The wave with the scythe claws: a cold cyan edge.
const SCYTHE_WAVE: Color = Color::srgb(0.75, 1.0, 1.0);
/// The power blow (earned from the crab): holding attack after a slash charges it
/// (no more repeated slashes while held); released once charged for `BLOW_CHARGE`,
/// the next slash is a blow: a wide fan `BLOW_REACH` long, `BLOW_FAN` radians each
/// side of the aim, taking `BLOW_DAMAGE` health.
pub const BLOW_CHARGE: f32 = 3.0;
pub const BLOW_REACH: f32 = SLASH_REACH * 2.4;
const BLOW_FAN: f32 = 1.1;
pub const BLOW_DAMAGE: i32 = 5;
/// The blow's wave: deep indigo-white.
const BLOW_WAVE: Color = Color::srgb(0.70, 0.76, 1.0);
/// The air spin (earned from the dragonfly): attacking in the air whirls the
/// chameleon around once, cutting a wide fan toward the aim, untouchable for
/// `SPIN_GUARD`; then it needs `SPIN_COOLDOWN` before the next one.
const SPIN_TIME: f32 = 0.4;
const SPIN_GUARD: f32 = 0.5;
const SPIN_COOLDOWN: f32 = 2.0;
/// The spin's fan: this far from the body, this wide each side of the aim (radians).
pub const SPIN_REACH: f32 = SLASH_REACH * 1.45;
pub const SPIN_FAN: f32 = 1.75;
/// Part of the spin (fraction of `SPIN_TIME`) where it cuts.
const SPIN_STRIKE: (f32, f32) = (0.1, 0.85);

/// Whether a target at `pos` (with hurtbox `radius`) is inside a fan from
/// `center` along `dir`: within `reach`, at most `half` radians off `dir`
/// (anything touching the center counts, whatever the angle).
pub fn in_fan(center: Vec2, dir: Vec2, reach: f32, half: f32, pos: Vec2, radius: f32) -> bool {
    let d = pos - center;
    let len = d.length();
    if len >= reach + radius {
        return false;
    }
    // The hurtbox widens the angle it covers.
    let slack = (radius / len.max(1.0)).min(1.0).asin();
    len < radius + 10.0 || d.angle_to(dir).abs() <= half + slack
}

/// The head's pivot, in rig units.
const NECK: Vec2 = Vec2::new(38.0, 6.0);
/// A spit jerks the head forward for this long.
const SPIT_TIME: f32 = 0.2;
/// How far the head pitches toward the mouse (its own frame, radians), and how
/// much of the mouse's angle it follows: the eyes do the rest.
const HEAD_PITCH: (f32, f32) = (-0.8, 0.9);
const HEAD_FOLLOW: f32 = 0.75;
/// A blink takes this long, lid down and up.
const BLINK_TIME: f32 = 0.16;

/// A rig point (rig units, facing +X) in the world, for a chameleon at `ppos` facing `facing`.
fn rig_to_world(ppos: Vec2, facing: f32, at: Vec2) -> Vec2 {
    ppos + Vec2::new(at.x * facing, at.y) * player::SCALE + Vec2::Y * player::RIG_DROP
}

/// The swiping shoulder: the slash arcs around it (center of the slash hitbox).
pub fn slash_center(ppos: Vec2, facing: f32) -> Vec2 {
    rig_to_world(ppos, facing, WAVE_CENTER)
}

/// The neck: aiming starts here, and spit leaves from just past it.
pub fn neck(ppos: Vec2, facing: f32) -> Vec2 {
    rig_to_world(ppos, facing, NECK)
}

/// A world direction in the rig's space (the rig is mirrored when facing left).
fn local(dir: Vec2, facing: f32) -> Vec2 {
    Vec2::new(dir.x * facing, dir.y)
}

pub struct ChameleonPlugin;

impl Plugin for ChameleonPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<UnlockedColors>()
            .add_message::<ColorEarned>()
            .add_systems(PreUpdate, forget_colors.after(level::restart))
            .add_systems(
            Update,
            (update_anim_state, earn_color, pose_parts, animate_tail, camouflage, slash_wave, spin_wave, flap_wings)
                .chain()
                .after(player::move_player),
        );
    }
}

/// Colors the chameleon has earned so far. Camouflage only works with these;
/// it starts empty, so the chameleon stays white until colors are unlocked
/// (e.g. `unlocked.0.push(level::lost_colors::PINK)` when the player reaches a goal).
#[derive(Resource, Default)]
pub struct UnlockedColors(pub Vec<Color>);

/// A beaten boss gives its color: the chameleon's skin wears it for good,
/// alongside the colors it already had.
#[derive(Message)]
pub struct ColorEarned(pub Color);

/// Where each earned color shows, in the order they're earned: the first is
/// the torso, every later one takes over one more part, big ones first.
/// Until its color is earned, a part wears the torso color's tones (tail
/// rings hide).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Slot {
    Torso,
    /// Head and eyelids.
    Head,
    Tail,
    Legs,
    /// The crest along the back and the casque spikes and ridge.
    Spikes,
    /// The slashing stripes across the flank.
    Stripes,
    /// The underside of the body, with its line.
    Belly,
    /// Under the jaw.
    Throat,
    TailRings,
    /// The sunken turret around the eye, and the brow.
    Eye,
    Scars,
}

/// Which shade of its slot's color a skin piece wears.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Tone {
    Base,
    /// Far side, sunken parts.
    Shadow,
    /// Highlights (belly line, scar edges).
    Light,
    /// Markings (crest, stripes, casque, brow).
    Accent,
}

/// One skin material: its slot and tone, and its current color, easing
/// toward the target (Oklab).
struct SkinMat {
    slot: Slot,
    tone: Tone,
    handle: Handle<ColorMaterial>,
    current: Oklaba,
}

/// Skin materials, re-tinted together by the camouflage system.
#[derive(Resource, Default)]
struct SkinMaterials(Vec<SkinMat>);

impl SkinMaterials {
    /// The material for this slot and tone (made on first use).
    fn get(&mut self, materials: &mut Assets<ColorMaterial>, slot: Slot, tone: Tone) -> Handle<ColorMaterial> {
        if let Some(m) = self.0.iter().find(|m| m.slot == slot && m.tone == tone) {
            return m.handle.clone();
        }
        let color = skin_color(&[], slot, tone, None);
        let handle = materials.add(color);
        self.0.push(SkinMat { slot, tone, handle: handle.clone(), current: color.into() });
        handle
    }
}

/// Animation state, lives on the player entity.
#[derive(Component)]
pub struct ChameleonAnim {
    /// Walk-cycle phase (radians).
    phase: f32,
    /// 0 = standing, 1 = full stride.
    stride: f32,
    /// 0 = grounded pose, 1 = airborne pose.
    air: f32,
    /// Tail curl amount (1 = tight spiral).
    curl: f32,
    /// Sideways tail swing while playing with it (0 = none).
    sway: f32,
    /// Seconds before the next idle tail game.
    idle_wait: f32,
    /// Which `TAIL_PLAYS` game is on, and how long it's been going (`None` when not playing).
    play_kind: usize,
    play: Option<f32>,
    look: Vec2,
    look_target: Vec2,
    look_timer: f32,
    /// Counts down to the next blink; below zero the lid is mid-blink.
    blink: f32,
    /// A twitchy glance off the mouse: its offset and the time left on it.
    tic: Vec2,
    tic_time: f32,
    tic_timer: f32,
    /// Time left of the head being locked on the mouse (after a ranged attack).
    focus: f32,
    /// Seconds since the claw slash started, `None` when not slashing.
    slash: Option<f32>,
    /// Where the current slash goes (unit, world space): the aim when it started.
    slash_aim: Vec2,
    /// Seconds since the air spin started, `None` when not spinning.
    spin: Option<f32>,
    /// Seconds before another air spin.
    spin_cooldown: f32,
    /// Seconds since the last spit, `None` after `SPIT_TIME`.
    spat: Option<f32>,
    /// The head looks forward (1) or back over the shoulder (-1), turning
    /// through 0; and how far it pitches up (+) in its own frame.
    head_side: f32,
    head_pitch: f32,
    /// Increments on every slash, so each one hits a target only once.
    pub slash_id: u32,
    /// Eat pressed this frame, free to bite (`feeding.rs` decides if there's anything to eat).
    eat: bool,
    /// Seconds since a bite started, `None` when not biting.
    bite: Option<f32>,
    /// Colors earned, in order: they all show on the skin (see `Slot`).
    earned: Vec<Color>,
    /// 0 = alive, 1 = lying dead belly-up.
    death: f32,
    /// 0 = upright, 1 = on a wall (turned head up, belly to the wall).
    climb: f32,
    /// The mantis's scythe claws: a wider, deadlier slash.
    scythe: bool,
    /// Seconds attack has been held with the crab's power (charging a blow), `None` when released.
    charge: Option<f32>,
    /// The current slash is a power blow.
    blow: bool,
    rng: u32,
}

/// Which bone a rig entity is; `pose_parts` drives each one.
#[derive(Component, Clone, Copy)]
enum Part {
    Rig,
    Body,
    Head,
    /// `swipe`: the near front leg, which also slashes.
    Leg { front: bool, offset: f32, joint: Joint, swipe: bool },
    EyeCone,
    Pupil,
    /// Closes over the eye (scaled on Y) to blink.
    Blink,
}

#[derive(Clone, Copy)]
enum Joint {
    Hip,
    Knee,
    Foot,
}

#[derive(Component)]
struct Tail {
    mesh: Handle<Mesh>,
    rings: Handle<Mesh>,
}

/// The tail's rings, shown once their color is earned.
#[derive(Component)]
struct TailRings;

/// The faint crescent left in the air by the claws.
#[derive(Component)]
struct SlashWave(Handle<ColorMaterial>);

/// The fan swept by the air spin.
#[derive(Component)]
struct SpinWave(Handle<ColorMaterial>);

/// A small wing, shown and flapping while flying.
#[derive(Component)]
struct Wing {
    near: bool,
}

fn flap_wings(time: Res<Time>, player: Single<&Player>, mut wings: Query<(&Wing, &mut Transform, &mut Visibility)>) {
    let on = player.flying && player.dead.is_none();
    let t = time.elapsed_secs();
    for (wing, mut tf, mut vis) in &mut wings {
        *vis = if on { Visibility::Inherited } else { Visibility::Hidden };
        if on {
            let flap = (t * 38.0 + if wing.near { 0.0 } else { 0.8 }).sin();
            let lean = if wing.near { 0.5 } else { 0.9 };
            tf.rotation = Quat::from_rotation_z(lean + 0.7 * flap);
        }
    }
}

pub fn spawn_chameleon(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<ColorMaterial>,
    player: Entity,
) {
    let mut skin = SkinMaterials::default();
    let mut mat = |slot, tone| skin.get(materials, slot, tone);
    let base = mat(Slot::Torso, Tone::Base);
    let head_mat = mat(Slot::Head, Tone::Base);
    let tail_mat = mat(Slot::Tail, Tone::Base);
    let (far_legs, near_legs) = (mat(Slot::Legs, Tone::Shadow), mat(Slot::Legs, Tone::Base));
    let spikes_mat = mat(Slot::Spikes, Tone::Accent);
    let stripes_mat = mat(Slot::Stripes, Tone::Accent);
    let (belly, belly_line) = (mat(Slot::Belly, Tone::Base), mat(Slot::Belly, Tone::Light));
    let throat = mat(Slot::Throat, Tone::Base);
    let rings = mat(Slot::TailRings, Tone::Accent);
    let (turret_mat, brow_mat) = (mat(Slot::Eye, Tone::Shadow), mat(Slot::Eye, Tone::Accent));
    let (scar_dark, scar_pale) = (mat(Slot::Scars, Tone::Shadow), mat(Slot::Scars, Tone::Light));
    let mouth = materials.add(MOUTH);
    let bone = materials.add(BONE);
    let black = materials.add(Color::srgb(0.02, 0.02, 0.03));
    let glow = materials.add(ColorMaterial {
        color: EYE_GLOW.with_alpha(0.22),
        alpha_mode: AlphaMode2d::Blend,
        ..default()
    });
    let iris_mat = materials.add(EYE_GLOW);

    let mut c = Builder { commands, meshes };

    let rig = c.pivot(Vec3::ZERO, player, Part::Rig);
    c.commands.entity(rig).insert(Transform::from_scale(Vec3::new(player::SCALE, player::SCALE, 1.0)));

    // --- Body: lean, hunched, armored back ---
    let body_outline = smooth_closed(
        &[
            Vec2::new(40.0, 5.0),
            Vec2::new(30.0, 17.0),
            Vec2::new(12.0, 25.0),
            Vec2::new(-8.0, 24.0),
            Vec2::new(-26.0, 17.0),
            Vec2::new(-38.0, 7.0),
            Vec2::new(-40.0, -3.0),
            Vec2::new(-28.0, -12.0),
            Vec2::new(-8.0, -15.0),
            Vec2::new(14.0, -14.0),
            Vec2::new(31.0, -9.0),
            Vec2::new(41.0, -2.0),
        ],
        8,
    );
    let body_shade = |p: Vec2| 0.55 + 0.45 * ((p.y + 15.0) / 40.0).clamp(0.0, 1.0);
    let body_mesh = fill(&body_outline, body_shade);
    let body = c.part(body_mesh, &base, Vec3::new(0.0, 2.0, 0.0), rig);
    c.tag(body, Part::Body);
    c.commands.entity(body).insert(PaintTarget {
        center: Vec2::new(0.0, 5.0),
        radii: Vec2::new(36.0, 16.0),
        claw: false,
        clear: None,
    });

    // Back-swept saw-tooth crest, longest over the middle of the back.
    let center = Vec2::new(0.0, 4.0);
    let spikes: Vec<[Vec2; 3]> = body_outline
        .iter()
        .enumerate()
        .filter(|(i, p)| i % 2 == 0 && p.y > 12.0 && p.x > -32.0 && p.x < 34.0)
        .map(|(_, &p)| {
            let n = (p - center).normalize();
            let t = n.perp();
            let u = ((p.x + 32.0) / 66.0).clamp(0.0, 1.0);
            let len = 4.0 + 7.0 * (PI * u).sin();
            [p - t * 2.6 - n * 2.0, p + t * 2.6 - n * 2.0, p + n * len + Vec2::new(-0.5 * len, 0.0)]
        })
        .collect();
    c.part(triangles(&spikes), &spikes_mat, Vec3::new(0.0, 0.0, -0.05), body);

    // Dark slashing stripes across the flank.
    let top_at = |x: f32| {
        body_outline.iter().filter(|p| (p.x - x).abs() < 3.5 && p.y > 0.0).map(|p| p.y).fold(0.0, f32::max)
    };
    let stripes: Vec<[Vec2; 3]> = [-26.0, -14.0, -2.0, 10.0, 22.0]
        .into_iter()
        .map(|x| {
            let ty = top_at(x) - 1.0;
            [Vec2::new(x - 3.5, ty), Vec2::new(x + 4.0, ty), Vec2::new(x - 7.0, ty - 19.0)]
        })
        .collect();
    c.part(triangles(&stripes), &stripes_mat, Vec3::new(0.0, 0.0, 0.1), body);

    // Belly line and war scars.
    let stripe = smooth_open(
        &[Vec2::new(-35.0, -5.0), Vec2::new(-18.0, -9.0), Vec2::new(0.0, -10.0), Vec2::new(20.0, -8.0), Vec2::new(38.0, -3.0)],
        6,
    );
    // The underside: from the belly line down to the body's edge.
    let below = |p: Vec2, outline: &[Vec2]| {
        let edge = outline.iter().filter(|q| q.y < 0.0).min_by(|a, b| (a.x - p.x).abs().total_cmp(&(b.x - p.x).abs()));
        Vec2::new(p.x, edge.map_or(p.y, |q| q.y.min(p.y)))
    };
    let underside: Vec<Vec2> = stripe.iter().map(|&p| below(p, &body_outline)).collect();
    c.part(band(&stripe, &underside, body_shade), &belly, Vec3::new(0.0, 0.0, 0.05), body);
    c.part(stroke(&stripe, |t| 1.0 + 2.2 * (PI * t).sin(), 1.0, 0.85, |_| 1.0), &belly_line, Vec3::new(0.0, 0.0, 0.1), body);
    c.scar(&[Vec2::new(4.0, 18.0), Vec2::new(10.0, 11.0), Vec2::new(7.0, 6.0), Vec2::new(14.0, 0.0)], &scar_dark, &scar_pale, body);
    c.scar(&[Vec2::new(-20.0, 12.0), Vec2::new(-15.0, 5.0), Vec2::new(-18.0, 0.0)], &scar_dark, &scar_pale, body);

    // --- Tail (mesh regenerated every frame by `animate_tail`) ---
    let tail_handle = c.meshes.add(tail_mesh(1.0, 0.0));
    let tail = c.part_handle(tail_handle.clone(), &tail_mat, Vec3::new(-34.0, 3.0, -0.3), body);
    let rings_handle = c.meshes.add(tail_rings(1.0, 0.0));
    let ring_e = c.part_handle(rings_handle.clone(), &rings, Vec3::new(0.0, 0.0, 0.01), tail);
    c.commands.entity(ring_e).insert((TailRings, Visibility::Hidden));
    c.commands.entity(tail).insert(Tail { mesh: tail_handle, rings: rings_handle });

    // --- Legs: far side (behind body, in shadow) then near side ---
    c.leg(Vec3::new(28.0, -4.0, -0.5), true, PI, false, &far_legs, body);
    c.leg(Vec3::new(-20.0, -4.0, -0.5), false, 0.0, false, &far_legs, body);
    c.leg(Vec3::new(24.0, -4.0, 0.6), true, 0.0, true, &near_legs, body);
    c.leg(Vec3::new(-24.0, -4.0, 0.6), false, PI, false, &near_legs, body);

    // --- Head: tall blade-like casque, heavy brow, fanged jaw ---
    let head = c.pivot(Vec3::new(38.0, 4.0, 0.2), body, Part::Head);
    c.commands.entity(head).insert(PaintTarget {
        center: Vec2::new(12.0, 3.0),
        radii: Vec2::new(18.0, 10.0),
        claw: false,
        // Keep the eye readable.
        clear: Some((Vec2::new(11.0, 4.0), 10.0)),
    });
    let head_outline = smooth_closed(
        &[
            Vec2::new(-8.0, -2.0),
            Vec2::new(-7.0, 14.0),
            Vec2::new(-5.0, 28.0),
            Vec2::new(5.0, 21.0),
            Vec2::new(17.0, 12.0),
            Vec2::new(27.0, 5.0),
            Vec2::new(34.0, 0.0),
            Vec2::new(32.0, -4.0),
            Vec2::new(24.0, -9.0),
            Vec2::new(10.0, -12.0),
            Vec2::new(-2.0, -11.0),
        ],
        8,
    );
    let head_shade = |p: Vec2| 0.6 + 0.4 * ((p.y + 12.0) / 38.0).clamp(0.0, 1.0);
    c.part(fill(&head_outline, head_shade), &head_mat, Vec3::ZERO, head);

    // Casque spikes and ridge.
    let casque_spikes: Vec<[Vec2; 3]> = head_outline
        .iter()
        .enumerate()
        .filter(|(i, p)| i % 3 == 0 && p.y > 15.0 && p.x < 6.0)
        .map(|(_, &p)| {
            let n = (p - Vec2::new(4.0, 6.0)).normalize();
            let t = n.perp();
            [p - t * 2.0 - n * 1.5, p + t * 2.0 - n * 1.5, p + n * 5.0 + Vec2::new(-3.0, 0.0)]
        })
        .collect();
    c.part(triangles(&casque_spikes), &spikes_mat, Vec3::new(0.0, 0.0, -0.05), head);
    let ridge = smooth_open(&[Vec2::new(-6.0, 15.0), Vec2::new(-5.0, 27.0), Vec2::new(5.0, 20.0), Vec2::new(14.0, 13.0)], 8);
    c.part(stroke(&ridge, |t| 3.0 - 1.8 * t, 1.0, 1.0, |_| 1.0), &spikes_mat, Vec3::new(0.0, 0.0, 0.05), head);
    c.scar(&[Vec2::new(-4.0, 21.0), Vec2::new(1.0, 15.0), Vec2::new(-1.0, 10.0)], &scar_dark, &scar_pale, head);

    // Jaw: dark mouth line ending in a sneer, with fangs.
    let mouth_line = smooth_open(
        &[Vec2::new(33.5, -2.5), Vec2::new(25.0, -4.8), Vec2::new(16.0, -5.8), Vec2::new(9.0, -4.8), Vec2::new(4.5, -1.5)],
        6,
    );
    let teeth: Vec<[Vec2; 3]> = [12.5, 15.5, 18.5, 21.5, 24.5, 27.5, 30.5]
        .into_iter()
        .map(|x| {
            let y = mouth_line.iter().min_by(|a, b| (a.x - x).abs().total_cmp(&(b.x - x).abs())).unwrap().y;
            let len = if x == 27.5 { 4.2 } else { 2.4 };
            [Vec2::new(x - 1.1, y), Vec2::new(x + 1.1, y), Vec2::new(x + 0.3, y - len)]
        })
        .collect();
    // The throat: under the jaw line, down to the head's edge.
    let jaw: Vec<Vec2> = mouth_line.iter().rev().map(|&p| p - Vec2::Y * 1.2).collect();
    let chin: Vec<Vec2> = jaw.iter().map(|&p| below(p, &head_outline)).collect();
    c.part(band(&jaw, &chin, head_shade), &throat, Vec3::new(0.0, 0.0, 0.03), head);
    c.part(triangles(&teeth), &bone, Vec3::new(0.0, 0.0, 0.055), head);
    c.part(stroke(&mouth_line, |t| 1.9 - 0.7 * t, 1.0, 1.0, |_| 1.0), &mouth, Vec3::new(0.0, 0.0, 0.06), head);
    c.part(fill(&ellipse_pts(1.1, 0.8, 10), |_| 1.0), &mouth, Vec3::new(29.0, 2.0, 0.06), head);

    // Eye: sunken turret, glowing iris, vertical slit pupil, angry lid.
    let eye = c.empty(Vec3::new(11.0, 4.0, 0.1), head);
    let turret = fill(&ellipse_pts(8.0, 8.0, 40), |p| 0.9 - 0.4 * (p.length() / 8.0));
    c.part(turret, &turret_mat, Vec3::ZERO, eye);
    c.part(Circle::new(7.0).into(), &glow, Vec3::new(0.0, 0.0, 0.005), eye);
    let cone = c.pivot(Vec3::new(0.0, 0.0, 0.01), eye, Part::EyeCone);
    let iris = fill(&ellipse_pts(4.2, 4.2, 28), |p| 1.0 - 0.3 * (p.length() / 4.2));
    c.part(iris, &iris_mat, Vec3::ZERO, cone);
    let pupil = c.pivot(Vec3::new(0.0, 0.0, 0.02), cone, Part::Pupil);
    c.part(fill(&ellipse_pts(0.85, 3.5, 16), |_| 1.0), &black, Vec3::ZERO, pupil);
    // The lid: a half-disc of skin slanted down towards the snout.
    let lid: Vec<Vec2> = (0..=16)
        .map(|i| {
            let a = i as f32 / 16.0 * PI;
            Vec2::new(a.cos(), a.sin()) * 8.6
        })
        .collect();
    let lid_tf = Transform::from_xyz(0.0, 1.6, 0.04).with_rotation(Quat::from_rotation_z(-0.35));
    let lid_e = c.part(fill(&lid, |p| 0.65 + 0.35 * (p.y / 8.6)), &head_mat, Vec3::ZERO, eye);
    c.commands.entity(lid_e).insert(lid_tf);
    // The blink: a disc of skin squeezing shut over the iris, top down.
    let blink = c.pivot(Vec3::new(0.0, 0.0, 0.045), eye, Part::Blink);
    c.part(fill(&ellipse_pts(8.0, 8.0, 24), |_| 0.8), &head_mat, Vec3::ZERO, blink);
    let brow = smooth_open(&[Vec2::new(-0.5, 10.5), Vec2::new(10.0, 7.8), Vec2::new(20.5, 3.8)], 6);
    c.part(stroke(&brow, |t| 3.8 - 2.3 * t, 1.0, 1.0, |_| 1.0), &brow_mat, Vec3::new(0.0, 0.0, 0.15), head);

    // Slash wave: a thin crescent around the swiping shoulder, hidden until a slash.
    let wave_mat = materials.add(ColorMaterial {
        color: WAVE.with_alpha(0.0),
        alpha_mode: AlphaMode2d::Blend,
        ..default()
    });
    let arc: Vec<Vec2> = (0..=32).map(|i| Vec2::from_angle(lerp(1.35, -1.45, i as f32 / 32.0)) * WAVE_RADIUS).collect();
    let wave_mesh = stroke(&arc, |t| 0.4 + 7.0 * (PI * t).sin().powf(0.7) * (0.35 + 0.65 * t), 1.0, 0.55, |_| 1.0);
    let wave = c.part(wave_mesh, &wave_mat, WAVE_CENTER.extend(0.9), rig);
    c.commands.entity(wave).insert((SlashWave(wave_mat), Visibility::Hidden));

    // Spin wave: a wide fan of crescents around the body (in world units, not
    // in the rig, so it doesn't turn with the spin), hidden until a spin.
    let spin_mat = materials.add(ColorMaterial {
        color: WAVE.with_alpha(0.0),
        alpha_mode: AlphaMode2d::Blend,
        ..default()
    });
    let spin = c.commands.spawn((SpinWave(spin_mat.clone()), Transform::from_xyz(0.0, 0.0, 0.9), Visibility::Hidden, ChildOf(player))).id();
    for (r, w) in [(1.0, 9.0), (0.72, 5.0)] {
        let arc: Vec<Vec2> = (0..=40).map(|i| Vec2::from_angle(lerp(SPIN_FAN, -SPIN_FAN, i as f32 / 40.0)) * SPIN_REACH * r).collect();
        c.part(stroke(&arc, |t| 0.5 + w * (PI * t).sin().powf(0.6), 1.0, 0.5, |_| 1.0), &spin_mat, Vec3::ZERO, spin);
    }

    // Small translucent wings on the back, flapping while it flies.
    let wing_mat = materials.add(ColorMaterial {
        color: Color::srgba(0.92, 0.96, 1.0, 0.6),
        alpha_mode: AlphaMode2d::Blend,
        ..default()
    });
    let wing_pts: Vec<Vec2> = ellipse_pts(5.0, 13.0, 20).into_iter().map(|p| p + Vec2::Y * 12.0).collect();
    for (near, z) in [(false, -0.3), (true, 0.3)] {
        let root = c.empty(Vec3::new(-4.0 + if near { 3.0 } else { 0.0 }, 18.0, z), rig);
        c.commands.entity(root).insert((Wing { near }, Visibility::Hidden));
        c.part(fill(&wing_pts, |p| 0.75 + 0.25 * (p.y / 25.0).clamp(0.0, 1.0)), &wing_mat, Vec3::ZERO, root);
    }

    c.commands.insert_resource(skin);
    c.commands.entity(player).insert(ChameleonAnim {
        phase: 0.0,
        stride: 0.0,
        air: 0.0,
        curl: 1.0,
        sway: 0.0,
        idle_wait: 3.0,
        play_kind: 0,
        play: None,
        look: Vec2::X,
        look_target: Vec2::X,
        look_timer: 1.0,
        blink: 2.0,
        tic: Vec2::ZERO,
        tic_time: 0.0,
        tic_timer: 1.5,
        focus: 0.0,
        slash: None,
        slash_aim: Vec2::X,
        spin: None,
        spin_cooldown: 0.0,
        spat: None,
        head_side: 1.0,
        head_pitch: 0.0,
        slash_id: 0,
        eat: false,
        bite: None,
        earned: Vec::new(),
        death: 0.0,
        climb: 0.0,
        scythe: false,
        charge: None,
        blow: false,
        rng: 0x9E37_79B9,
    });
}

/// Small helper to keep the rig-building code readable.
struct Builder<'a, 'w, 's> {
    commands: &'a mut Commands<'w, 's>,
    meshes: &'a mut Assets<Mesh>,
}

impl Builder<'_, '_, '_> {
    fn part(&mut self, mesh: Mesh, mat: &Handle<ColorMaterial>, pos: Vec3, parent: Entity) -> Entity {
        let mesh = self.meshes.add(mesh);
        self.part_handle(mesh, mat, pos, parent)
    }

    fn part_handle(&mut self, mesh: Handle<Mesh>, mat: &Handle<ColorMaterial>, pos: Vec3, parent: Entity) -> Entity {
        self.commands
            .spawn((Mesh2d(mesh), MeshMaterial2d(mat.clone()), Transform::from_translation(pos), ChildOf(parent)))
            .id()
    }

    fn empty(&mut self, pos: Vec3, parent: Entity) -> Entity {
        self.commands
            .spawn((Transform::from_translation(pos), Visibility::default(), ChildOf(parent)))
            .id()
    }

    fn pivot(&mut self, pos: Vec3, parent: Entity, part: Part) -> Entity {
        let e = self.empty(pos, parent);
        self.tag(e, part);
        e
    }

    fn tag(&mut self, e: Entity, part: Part) {
        self.commands.entity(e).insert(part);
    }

    /// A healed wound: dark line with a pale edge.
    fn scar(&mut self, pts: &[Vec2], dark: &Handle<ColorMaterial>, pale: &Handle<ColorMaterial>, parent: Entity) {
        let path = smooth_open(pts, 4);
        self.part(stroke(&path, |t| 2.6 * (PI * t).sin() + 0.6, 1.0, 1.0, |_| 1.0), pale, Vec3::new(0.0, 0.0, 0.12), parent);
        self.part(stroke(&path, |t| 1.2 * (PI * t).sin() + 0.3, 1.0, 1.0, |_| 1.0), dark, Vec3::new(0.0, 0.0, 0.13), parent);
    }

    /// Two-segment leg with a zygodactyl (tong-like) foot.
    fn leg(&mut self, joint: Vec3, front: bool, offset: f32, swipe: bool, mat: &Handle<ColorMaterial>, parent: Entity) {
        let part = |joint| Part::Leg { front, offset, joint, swipe };
        let hip = self.pivot(joint, parent, part(Joint::Hip));
        // The swiping arm is split in depth: the shoulder socket tucks behind the
        // head (z 0.2), while forearm and claw reach in front of it, so a high
        // slash crosses the face but a turned-back head doesn't poke through the socket.
        let (upper_z, knee_z) = if swipe { (-0.5, 0.51) } else { (0.0, 0.01) };
        self.part(Capsule2d::new(3.1, 12.0).into(), mat, Vec3::new(0.0, -6.0, upper_z), hip);
        let knee = self.pivot(Vec3::new(0.0, -12.0, knee_z), hip, part(Joint::Knee));
        if swipe {
            // The killing arm gets soaked first.
            let claw = |center, radii| PaintTarget { center, radii, claw: true, clear: None };
            self.commands.entity(hip).insert(claw(Vec2::new(0.0, -6.0), Vec2::new(3.2, 8.0)));
            self.commands.entity(knee).insert(claw(Vec2::new(0.0, -6.0), Vec2::new(2.6, 7.5)));
        }
        self.part(Capsule2d::new(2.3, 11.0).into(), mat, Vec3::new(0.0, -5.5, 0.0), knee);
        let foot = self.pivot(Vec3::new(0.0, -11.0, 0.01), knee, part(Joint::Foot));
        for side in [1.0, -1.0] {
            let toe = self.part(Capsule2d::new(1.4, 6.0).into(), mat, Vec3::new(2.6 * side, -1.6, 0.0), foot);
            self.commands
                .entity(toe)
                .insert(Transform::from_xyz(2.6 * side, -1.6, 0.0).with_rotation(Quat::from_rotation_z(1.25 * side)));
        }
    }
}

/// Idle tail games: (duration in seconds, name for reference).
const TAIL_PLAYS: [(f32, &str); 5] = [
    (2.6, "unroll and roll back"),
    (1.5, "double flick"),
    (2.8, "wag"),
    (2.2, "tighten pulses"),
    (3.2, "slow lasso"),
];

/// The (curl, sway) of tail game `kind` at progress `u` in 0..1.
fn tail_play(kind: usize, u: f32) -> (f32, f32) {
    let env = (u * PI).sin().max(0.0);
    match kind {
        // Slowly unrolls, holds it stretched, rolls back up.
        0 => (1.0 - 1.05 * env.powf(0.5).min(1.0), 0.0),
        // Two quick snaps open.
        1 => (1.0 - 1.15 * (u * PI * 4.0).sin().max(0.0), 0.0),
        // Half unrolled, swinging side to side.
        2 => (1.0 - 0.7 * env.powf(0.4), env.powf(0.4) * (u * TAU * 3.0).sin()),
        // Winds tighter and looser a few times.
        3 => (1.0 + 0.5 * env * (0.5 - 0.5 * (u * TAU * 3.0).cos()), 0.0),
        // Stretches out and swirls it around, then reels it back.
        _ => (1.0 - 0.8 * env.powf(0.4), env.powf(0.4) * 1.3 * (u * TAU * 2.0).sin()),
    }
}

/// The spiral tail's centerline, base to tip. `curl` = 1 → tight coil, 0 → nearly straight;
/// `sway` bends it into a wave.
fn tail_path(curl: f32, sway: f32) -> Vec<Vec2> {
    const N: usize = 56;
    const LEN: f32 = 100.0;
    let ds = LEN / N as f32;
    let mut pts = Vec::with_capacity(N + 1);
    let (mut p, mut angle) = (Vec2::ZERO, PI - 0.08);
    for i in 0..=N {
        pts.push(p);
        let s = i as f32 / N as f32;
        // Curvature grows towards the tip → logarithmic-looking spiral.
        angle += curl * (0.008 + 0.26 * s.powf(2.5)) * ds + sway * 0.09 * (s * TAU * 0.75).cos() * ds;
        p += Vec2::from_angle(angle) * ds;
    }
    pts
}

/// The tail's width at `t` along it (0 = base).
fn tail_width(t: f32) -> f32 {
    1.2 + 10.5 * (1.0 - t).powf(0.9)
}

/// The spiral tail. `curl` = 1 → tight coil, 0 → nearly straight.
fn tail_mesh(curl: f32, sway: f32) -> Mesh {
    // Left edge (perp) is the inner/belly side; add soft bands along the tail.
    stroke(&tail_path(curl, sway), tail_width, 0.55, 1.0, |t| 1.0 - 0.55 * (0.5 + 0.5 * (t * 30.0).sin()).powi(8))
}

/// Rings around the tail, thinning toward the tip, following its curl.
fn tail_rings(curl: f32, sway: f32) -> Mesh {
    let pts = tail_path(curl, sway);
    let last = pts.len() - 1;
    let edge = |i: usize| {
        let t = i as f32 / last as f32;
        let tangent = (pts[(i + 1).min(last)] - pts[i.saturating_sub(1)]).normalize_or(Vec2::X);
        let offset = tangent.perp() * tail_width(t) * 0.5;
        (pts[i] + offset, pts[i] - offset)
    };
    let mut tris = Vec::new();
    for start in [0.1, 0.24, 0.38, 0.52, 0.66, 0.8] {
        let a = (start * last as f32) as usize;
        let b = (a + (4.0 - 2.5 * start) as usize).min(last);
        for i in a..b {
            let ((l0, r0), (l1, r1)) = (edge(i), edge(i + 1));
            tris.push([l0, r0, l1]);
            tris.push([r0, r1, l1]);
        }
    }
    triangles(&tris)
}

pub fn update_anim_state(
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    defeated: Res<Defeated>,
    player: Single<(&mut Player, &mut ChameleonAnim)>,
) {
    let dt = time.delta_secs();
    let (mut p, mut a) = player.into_inner();
    let on_wall = p.climbing.is_some();
    // On a wall it walks up and down it.
    let speed = if on_wall { p.vel.length() } else { p.vel.x.abs() };
    a.climb = smooth(a.climb, if on_wall { 1.0 } else { 0.0 }, 14.0, dt);

    // Swimming: the legs paddle (the walk cycle at its own pace), more when tired.
    let paddle = if p.swimming && p.dead.is_none() { 0.35 + 0.65 * (p.vel.length() / 170.0).min(1.0) } else { 0.0 };
    a.stride = smooth(a.stride, if paddle > 0.0 { paddle } else { (speed / 260.0).min(1.0) }, 8.0, dt);
    a.air = smooth(a.air, if p.grounded || paddle > 0.0 || on_wall { if paddle > 0.0 { 0.35 } else { 0.0 } } else { 1.0 }, 10.0, dt);
    // Walking backwards (facing the mouse) plays the walk cycle backwards.
    a.phase += if paddle > 0.0 {
        dt * 9.0 * (0.6 + 0.4 * paddle)
    } else if on_wall {
        dt * if p.vel.y.abs() > 1.0 { p.vel.y } else { p.vel.x * p.facing } * 0.075
    } else {
        dt * p.vel.x * p.facing * 0.075
    };

    let dead = p.dead.is_some();
    a.death = if dead { smooth(a.death, 1.0, 6.0, dt) } else { 0.0 };
    let curl_target = if dead { 1.0 } else if !p.grounded { 0.25 } else { 1.0 - 0.55 * a.stride };

    // Idle: after a while standing around, it plays with its tail.
    let resting = p.grounded
        && !dead
        && !on_wall
        && !p.swimming
        && a.stride < 0.05
        && a.slash.is_none()
        && a.spin.is_none()
        && a.bite.is_none()
        && p.stun <= 0.0;
    if resting {
        a.idle_wait -= dt;
        if a.play.is_none() && a.idle_wait <= 0.0 {
            a.play_kind = (a.rand() * TAIL_PLAYS.len() as f32) as usize % TAIL_PLAYS.len();
            a.play = Some(0.0);
        }
    } else {
        a.play = None;
        a.idle_wait = 2.5 + 3.0 * a.rand();
    }
    if let Some(t) = a.play {
        let duration = TAIL_PLAYS[a.play_kind].0;
        if t >= duration {
            a.play = None;
            a.idle_wait = 2.0 + 5.0 * a.rand();
        } else {
            a.play = Some(t + dt);
        }
    }
    let (play_curl, play_sway) = match a.play {
        Some(t) => tail_play(a.play_kind, t / TAIL_PLAYS[a.play_kind].0),
        None => (curl_target, 0.0),
    };
    let rate = if a.play.is_some() { 12.0 } else { 4.0 };
    a.curl = smooth(a.curl, play_curl, rate, dt);
    a.sway = smooth(a.sway, play_sway, 12.0, dt);
    if !(a.curl.is_finite() && a.sway.is_finite()) {
        eprintln!("tail: bad state curl={} sway={} play={:?} kind={} (reset)", a.curl, a.sway, a.play, a.play_kind);
        (a.curl, a.sway, a.play) = (1.0, 0.0, None);
    }

    // Claw slash: holding the button keeps slashing, one right after another.
    // In the air, with the dragonfly's power and when it's ready: the spin.
    // With the crab's power, holding it charges a power blow instead (a press still slashes once).
    let attack = keys.any_pressed([KeyCode::KeyF, KeyCode::KeyJ]) || mouse.pressed(MouseButton::Left);
    let free = a.slash.is_none() && a.spin.is_none() && a.bite.is_none() && p.stun <= 0.0 && !dead;
    a.eat = keys.just_pressed(KeyCode::KeyE) && free && p.burrow.is_none();
    a.spin_cooldown = (a.spin_cooldown - dt).max(0.0);
    a.scythe = defeated.scythe();
    let power = defeated.blow();
    let mut released = false;
    if power && attack && !dead && p.stun <= 0.0 {
        a.charge = Some(a.charge.map_or(0.0, |c| c + dt));
    } else if let Some(c) = a.charge.take() {
        released = c >= BLOW_CHARGE && !dead;
    }
    if released {
        // Let loose: the blow, whatever the claws were doing.
        a.slash_aim = p.aim;
        a.slash_id += 1;
        a.spin = None;
        a.slash = Some(0.0);
        a.blow = true;
    } else if attack && free && !a.eat && a.charge.is_none_or(|c| c < 0.15) {
        a.slash_aim = p.aim;
        a.slash_id += 1;
        if !p.grounded && !p.swimming && defeated.spin() && a.spin_cooldown <= 0.0 {
            a.spin = Some(0.0);
            a.spin_cooldown = SPIN_COOLDOWN;
            p.guard = p.guard.max(SPIN_GUARD);
        } else {
            a.slash = Some(0.0);
        }
    }
    // Power pills speed the claws up (the crash slows them).
    a.slash = a.slash.map(|t| t + dt * p.pill.speed()).filter(|&t| t < SLASH_TIME);
    if a.slash.is_none() {
        a.blow = false;
    }
    a.spin = a.spin.map(|t| t + dt).filter(|&t| t < SPIN_TIME && !dead);
    a.bite = a.bite.map(|t| t + dt).filter(|&t| t < BITE_TIME && !dead);
    a.spat = a.spat.map(|t| t + dt).filter(|&t| t < SPIT_TIME);

    // Head: turned toward the mouse (back over the shoulder when it's behind,
    // with a little hysteresis), pitched toward it, roughly.
    let aim = local(p.aim, p.facing);
    let aiming = p.aim_at.is_some() && !dead && a.bite.is_none();
    let side = match () {
        _ if !aiming => 1.0,
        _ if aim.x < -0.2 => -1.0,
        _ if aim.x > 0.2 => 1.0,
        _ => a.head_side.signum(),
    };
    // Only a ranged attack pins the head on the mouse (snapping to it); the
    // rest of the time it follows loosely and wobbles, the eye leads.
    a.focus = (a.focus - dt).max(0.0);
    let locked = a.focus > 0.0;
    let mouse_pitch = Vec2::new(aim.x * side, aim.y).to_angle().clamp(HEAD_PITCH.0, HEAD_PITCH.1);
    let t = time.elapsed_secs();
    let wobble = (t * 1.7).sin() * 0.07 + (t * 2.9 + 1.0).sin() * 0.05;
    let pitch = match () {
        _ if !aiming => 0.0,
        _ if locked => mouse_pitch,
        _ => mouse_pitch * HEAD_FOLLOW * 0.45 + wobble,
    };
    a.head_side = smooth(a.head_side, side, if locked { 20.0 } else { 5.0 }, dt);
    a.head_pitch = smooth(a.head_pitch, pitch, if locked { 30.0 } else { 2.5 }, dt);

    // Blink every few seconds (sometimes twice in a row).
    a.blink -= dt;
    if a.blink < -BLINK_TIME {
        a.blink = if a.rand() < 0.2 { 0.0 } else { 1.5 + 3.5 * a.rand() };
    }
    // Wander: now and then the eye drifts off the mouse on its own.
    a.tic_timer -= dt;
    if a.tic_timer <= 0.0 {
        if a.tic_time > 0.0 {
            a.tic_time = 0.0;
            a.tic_timer = 0.5 + 2.0 * a.rand();
        } else {
            a.tic = Vec2::from_angle(a.rand() * PI * 2.0) * (0.8 + 0.8 * a.rand());
            a.tic_time = 0.15 + 0.9 * a.rand();
            a.tic_timer = a.tic_time;
        }
    }

    // Eye: right on the mouse, from where the head points. Before the mouse
    // ever shows up: look ahead while moving, dart around randomly while idle.
    a.look_timer -= dt;
    if aiming {
        let on_mouse = Vec2::from_angle(-a.head_pitch).rotate(Vec2::new(aim.x * side, aim.y));
        a.look_target = if a.tic_time > 0.0 && !locked { (on_mouse + a.tic).clamp_length_max(1.0) } else { on_mouse };
    } else if a.slash.is_some() || speed > 20.0 {
        a.look_target = Vec2::new(1.0, (p.vel.y * 0.0006).clamp(-0.6, 0.6)).normalize();
        a.look_timer = 0.8;
    } else if a.look_timer <= 0.0 {
        let angle = a.rand() * PI * 2.0;
        a.look_target = Vec2::from_angle(angle) * (0.4 + 0.6 * a.rand());
        a.look_timer = 0.5 + 2.0 * a.rand();
    }
    // Chameleon eyes snap quickly between targets.
    let target = a.look_target;
    a.look = a.look.lerp(target, 1.0 - (-dt * 30.0).exp());
}

fn pose_parts(
    time: Res<Time>,
    player: Single<(&Player, &ChameleonAnim)>,
    mut parts: Query<(&Part, &mut Transform, &mut Visibility)>,
) {
    let (p, a) = *player;
    let t = time.elapsed_secs();
    let dt = time.delta_secs();
    let bob = -(a.phase.sin().abs()) * 1.8 * a.stride;
    // The slash's direction in the rig: the arm swings toward it, top to
    // bottom on whichever side it is (`sweep` -1 = behind the chameleon).
    let slash_dir = local(a.slash_aim, p.facing);
    let sweep = if slash_dir.x < 0.0 { -1.0 } else { 1.0 };
    let slash = a.slash.map(|t| slash_pose(t / SLASH_TIME, slash_dir.to_angle(), sweep));
    let lunge = slash.map_or(0.0, |s| s.3) * sweep;
    let spit = a.spat.map_or(0.0, |t| (PI * t / SPIT_TIME).sin());
    // The head's own motions are in its frame: mirrored when it looks back.
    let head_sign = if a.head_side < 0.0 { -1.0 } else { 1.0 };
    // Bite: dip the head into the remains and tear at them.
    let bite = a.bite.map_or(0.0, |t| (PI * t / BITE_TIME).sin());
    let chew = a.bite.map_or(0.0, |t| (t * 40.0).sin() * 0.08) * bite;

    for (part, mut tf, mut vis) in &mut parts {
        match *part {
            Part::Rig => {
                // Flip by scaling X through zero: reads as a quick turn.
                tf.scale.x = smooth(tf.scale.x, p.facing * player::SCALE, 16.0, dt);
                let tilt = (p.vel.y * 0.00035).clamp(-0.25, 0.25) * a.air;
                // Dead: rolls over belly-up. Spinning: one whole turn, forward.
                let spin = a.spin.map_or(0.0, |t| {
                    let k = t / SPIN_TIME;
                    -2.0 * PI * k * k * (3.0 - 2.0 * k)
                });
                // Gripping a surface: turned to lie along it (a quarter on a wall), belly against it.
                let wall = p.grip_angle * a.climb;
                tf.rotation = Quat::from_rotation_z((lerp(tilt, PI, a.death) + spin + wall) * p.facing);
                tf.translation.y = a.death * 6.0 + p.step.clamp(-12.0, 12.0) + player::RIG_DROP;
                // Blink while invulnerable.
                let blink = p.invuln > 0.0 && p.dead.is_none() && (t * 30.0).sin() > 0.0;
                *vis = if blink { Visibility::Hidden } else { Visibility::Inherited };
            }
            Part::Body => {
                tf.translation.x = lunge * 4.0 + bite * 5.0 + spit * 3.0 * head_sign;
                tf.translation.y = 2.0 + bob - bite * 3.0;
                tf.rotation = Quat::from_rotation_z((a.phase * 2.0).sin() * 0.025 * a.stride - lunge * 0.07 - bite * 0.18);
                tf.scale.y = 1.0 + (t * 2.2).sin() * 0.012; // breathing
            }
            Part::Head => {
                let nod = (a.phase * 2.0).sin() * 0.05 * a.stride + (t * 1.3).sin() * 0.02;
                let own = a.head_pitch + nod + 0.12 * a.air - lunge * 0.08 - bite * 0.45 + chew - spit * 0.15;
                tf.rotation = Quat::from_rotation_z(own * head_sign);
                tf.scale.x = a.head_side;
            }
            Part::Leg { front, offset, joint, swipe } => {
                let (mut hip, mut knee) = leg_angles(front, offset, a);
                let mut foot = -(hip + knee); // keep the foot flat
                if let (true, Some((w, s_hip, s_knee, _))) = (swipe, slash) {
                    hip = lerp(hip, s_hip, w);
                    knee = lerp(knee, s_knee, w);
                    foot = lerp(foot, 0.5, w); // claws hooked forward
                }
                if a.death > 0.0 {
                    // Legs up, twitching less and less.
                    let s = if front { 1.0 } else { -1.0 };
                    let twitch = (t * 25.0 + offset).sin() * 0.3 * (1.0 - a.death).max(0.1);
                    hip = lerp(hip, s * 0.5 + twitch, a.death);
                    knee = lerp(knee, -s * 1.3, a.death);
                }
                tf.rotation = Quat::from_rotation_z(match joint {
                    Joint::Hip => hip,
                    Joint::Knee => knee,
                    Joint::Foot => foot,
                });
            }
            Part::EyeCone => tf.translation = (a.look * 2.6).extend(tf.translation.z),
            Part::Pupil => tf.translation = (a.look * 1.0).extend(tf.translation.z),
            Part::Blink => {
                let shut = if a.blink < 0.0 { (PI * -a.blink / BLINK_TIME).sin() } else { 0.0 };
                let shut = shut.max(a.death);
                // Squeeze from the top: the disc shrinks toward its bottom edge.
                tf.scale.y = shut.max(0.001);
                tf.translation.y = -8.0 * (1.0 - shut);
            }
        }
    }
}

/// Swipe pose at slash progress `u` (0..1), aimed at `angle` (rig space, the
/// arm pivoting on the shoulder) and sweeping top to bottom on that side
/// (`sweep` 1 = in front, -1 = behind): (blend weight, hip, knee, body lunge).
fn slash_pose(u: f32, angle: f32, sweep: f32) -> (f32, f32, f32, f32) {
    let ease = |x: f32| x * x * (3.0 - 2.0 * x);
    let w = ease((u / 0.12).min(1.0)) * (1.0 - ease(((u - 0.6) / 0.4).clamp(0.0, 1.0)));
    // Hip 0 = the arm hangs straight down, so pointing it at `a` is `a + PI/2`.
    // Straight ahead this is: raised to 2.6, ripped down to 0.05.
    let raised = angle + sweep * 1.03 + PI / 2.0;
    let ripped = angle - sweep * 1.52 + PI / 2.0;
    let (hip, knee) = if u < STRIKE.0 {
        // Wind up: claw raised high over the aim.
        let k = ease(u / STRIKE.0);
        (lerp(0.4, raised, k), lerp(-0.6, -1.5, k) * sweep)
    } else {
        // Rip down through it, then hold the follow-through.
        let k = ((u - STRIKE.0) / (STRIKE.1 - STRIKE.0)).min(1.0);
        let k = 1.0 - (1.0 - k).powi(3);
        (lerp(raised, ripped, k), lerp(-1.5, -0.1, k) * sweep)
    };
    let lunge = (PI * ((u - 0.15) / 0.6).clamp(0.0, 1.0)).sin();
    (w, hip, knee, lunge)
}

fn leg_angles(front: bool, offset: f32, a: &ChameleonAnim) -> (f32, f32) {
    let s = if front { 1.0 } else { -1.0 };
    let ph = a.phase + offset;
    let swing = ph.sin() * 0.7 * a.stride;
    let lift = ph.cos().max(0.0) * 0.8 * a.stride;
    let hip = lerp(s * 0.25 + swing, s * 0.8, a.air);
    let knee = lerp(-s * (0.55 + lift), -s * 1.1, a.air);
    (hip, knee)
}

fn animate_tail(
    player: Single<&ChameleonAnim>,
    tail: Single<&Tail>,
    mut rings: Single<&mut Visibility, With<TailRings>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut last: Local<Option<(f32, f32, bool)>>,
) {
    let shown = player.earned.len() > Slot::TailRings as usize;
    **rings = if shown { Visibility::Inherited } else { Visibility::Hidden };
    // Rebuilding a mesh costs the engine a re-upload and a rescan of every mesh entity: only when the pose changed.
    let pose = (player.curl, player.sway, shown);
    if *last == Some(pose) {
        return;
    }
    *last = Some(pose);
    if let Some(mut mesh) = meshes.get_mut(&tail.mesh) {
        *mesh = tail_mesh(player.curl, player.sway);
    }
    if shown && let Some(mut mesh) = meshes.get_mut(&tail.rings) {
        *mesh = tail_rings(player.curl, player.sway);
    }
}

fn spin_wave(
    player: Single<(&Player, &ChameleonAnim)>,
    wave: Single<(&SpinWave, &mut Transform, &mut Visibility)>,
    mut materials: ResMut<Assets<ColorMaterial>>,
) {
    let (p, a) = *player;
    let (wave, mut tf, mut vis) = wave.into_inner();
    let Some(t) = a.spin else {
        *vis = Visibility::Hidden;
        return;
    };
    // Sweeps a little the way the body turns as it widens and fades.
    let k = t / SPIN_TIME;
    *vis = Visibility::Inherited;
    tf.translation.y = player::RIG_DROP;
    tf.rotation = Quat::from_rotation_z(a.slash_aim.to_angle() - p.facing * (k - 0.5) * 0.7);
    tf.scale = Vec3::splat(0.8 + 0.25 * k);
    if let Some(mut mat) = materials.get_mut(&wave.0) {
        mat.color.set_alpha(0.6 * (1.0 - k).powf(1.2) * (k / 0.15).min(1.0));
    }
}

fn slash_wave(
    player: Single<(&Player, &ChameleonAnim)>,
    wave: Single<(&SlashWave, &mut Transform, &mut Visibility)>,
    mut materials: ResMut<Assets<ColorMaterial>>,
) {
    let (p, a) = *player;
    let (wave, mut tf, mut vis) = wave.into_inner();
    // Around the shoulder, turned to the aim; mirrored behind the chameleon so
    // it still sweeps top to bottom like the arm.
    let dir = local(a.slash_aim, p.facing);
    let sweep = if dir.x < 0.0 { -1.0 } else { 1.0 };
    tf.rotation = Quat::from_rotation_z(dir.to_angle());
    let k = a.slash.map_or(-1.0, |t| (t / SLASH_TIME - STRIKE.0) / 0.45);
    if !(0.0..1.0).contains(&k) {
        *vis = Visibility::Hidden;
        return;
    }
    // Barely there: a quick ripple that widens and fades.
    *vis = Visibility::Inherited;
    let grow = if a.blow { BLOW_REACH / SLASH_REACH } else if a.scythe { SCYTHE_REACH } else { 1.0 };
    tf.scale = Vec3::new(1.0, sweep, 1.0) * (0.9 + 0.25 * k) * grow;
    if let Some(mut mat) = materials.get_mut(&wave.0) {
        let (color, alpha) = if a.blow { (BLOW_WAVE, 0.9) } else if a.scythe { (SCYTHE_WAVE, 0.7) } else { (WAVE, 0.55) };
        mat.color = color.with_alpha(alpha * (1.0 - k).powf(1.3));
    }
}

/// Slowly blend the skin towards the color of the surface we stand on.
fn camouflage(
    time: Res<Time>,
    player: Single<(&Player, &ChameleonAnim)>,
    mut skin: ResMut<SkinMaterials>,
    unlocked: Res<UnlockedColors>,
    mut materials: ResMut<Assets<ColorMaterial>>,
) {
    let (p, a) = *player;
    let ground = p.ground_color.filter(|c| p.grounded && unlocked.0.contains(c));
    let k = 1.0 - (-time.delta_secs() * 1.8).exp();
    for m in &mut skin.0 {
        // Blend in Oklab: hue interpolation from white (no hue) looks odd in Oklch.
        m.current = m.current.mix(&skin_color(&a.earned, m.slot, m.tone, ground).into(), k);
        crate::shapes::set_color(&mut materials, &m.handle, m.current.into());
    }
}

/// A skin piece's color: its slot's earned color (the torso's until then; white
/// before any), in its tone, pulled toward the `ground` color when camouflaged.
/// A marking with a color of its own shows it bright, not darkened.
fn skin_color(earned: &[Color], slot: Slot, tone: Tone, ground: Option<Color>) -> Color {
    let own = (slot != Slot::Torso).then(|| earned.get(slot as usize).copied()).flatten();
    let c = own.or(earned.first().copied()).unwrap_or(DEFAULT_SKIN);
    let c: Oklcha = match ground {
        Some(g) => Oklaba::from(c).mix(&g.into(), 0.8).into(),
        None => c.into(),
    };
    let [base, shadow, light, accent] = palette(c);
    match (tone, own.is_some()) {
        (Tone::Base, _) => base,
        (Tone::Shadow, _) => shadow,
        (Tone::Light, false) => light,
        (Tone::Light, true) => Oklcha { lightness: (c.lightness + 0.08).min(0.95), ..c }.into(),
        (Tone::Accent, false) => accent,
        (Tone::Accent, true) => Oklcha { lightness: c.lightness * 0.85, chroma: c.chroma * 1.1, ..c }.into(),
    }
}

/// Wears an earned color, with the others. The world camera is grayscale, so the whole
/// chameleon moves to the color layer to actually show it.
fn earn_color(
    mut commands: Commands,
    mut earned: MessageReader<ColorEarned>,
    mut unlocked: ResMut<UnlockedColors>,
    player: Single<(Entity, &mut ChameleonAnim)>,
    children: Query<&Children>,
    meshes: Query<(), With<Mesh2d>>,
) {
    let (entity, mut a) = player.into_inner();
    for ColorEarned(color) in earned.read() {
        if !a.earned.contains(color) {
            a.earned.push(*color);
        }
        if !unlocked.0.contains(color) {
            unlocked.0.push(*color);
        }
        for e in children.iter_descendants(entity) {
            if meshes.contains(e) {
                commands.entity(e).insert(camera::color_layer());
            }
        }
    }
}

/// F8: every earned color is lost again (the new chameleon is white).
fn forget_colors(mut restarted: MessageReader<Restart>, mut unlocked: ResMut<UnlockedColors>) {
    if restarted.read().count() > 0 {
        unlocked.0.clear();
    }
}

/// Derives the 4 skin tones from a single base color.
fn palette(c: Oklcha) -> [Color; 4] {
    let shadow = Oklcha { lightness: c.lightness * 0.55, ..c };
    let light = Oklcha { lightness: (c.lightness + 0.1).min(0.98), chroma: c.chroma * 0.5, ..c };
    let accent = Oklcha { lightness: c.lightness * 0.3, chroma: c.chroma * 1.2, hue: (c.hue + 15.0) % 360.0, ..c };
    [c.into(), shadow.into(), light.into(), accent.into()]
}

impl ChameleonAnim {
    /// Slashing or eating: its mind is on something.
    pub fn busy(&self) -> bool {
        self.slashing() || self.bite.is_some()
    }

    /// Slashing or spinning.
    pub fn slashing(&self) -> bool {
        self.slash.is_some() || self.spin.is_some()
    }

    /// Mid air spin (the Razorwing's power) rather than a plain slash.
    pub fn spinning(&self) -> bool {
        self.spin.is_some()
    }

    /// True while the claws are cutting through the air.
    pub fn striking(&self) -> bool {
        self.slash.is_some_and(|t| (STRIKE.0..=STRIKE.1).contains(&(t / SLASH_TIME)))
            || self.spin.is_some_and(|t| (SPIN_STRIKE.0..=SPIN_STRIKE.1).contains(&(t / SPIN_TIME)))
    }

    /// Whether the current attack reaches a target at `pos` with hurtbox
    /// `radius`, for a chameleon at `ppos`: the slash's half-disc around the
    /// shoulder, or the spin's wide fan around the body.
    pub fn reaches(&self, ppos: Vec2, facing: f32, pos: Vec2, radius: f32) -> bool {
        if self.spin.is_some() {
            return in_fan(ppos, self.slash_aim, SPIN_REACH, SPIN_FAN, pos, radius);
        }
        if self.blow {
            return in_fan(ppos, self.slash_aim, BLOW_REACH, BLOW_FAN, pos, radius);
        }
        let center = slash_center(ppos, facing);
        (pos - center).dot(self.slash_aim) > -radius * 0.5 && pos.distance(center) < self.slash_reach() + radius
    }

    /// How far the plain slash reaches (farther with the scythe claws).
    pub fn slash_reach(&self) -> f32 {
        if self.scythe { SLASH_REACH * SCYTHE_REACH } else { SLASH_REACH }
    }

    /// Health the current attack takes from what it cuts: the scythe claws cut
    /// deeper on a plain slash (the spin stays as it is).
    pub fn damage(&self) -> i32 {
        if self.blow {
            BLOW_DAMAGE
        } else if self.scythe && self.spin.is_none() {
            SCYTHE_DAMAGE
        } else {
            1
        }
    }

    /// How long attack has been held charging a power blow (`None`: not charging).
    pub fn charge(&self) -> Option<f32> {
        self.charge
    }

    /// The current slash is a power blow.
    pub fn blowing(&self) -> bool {
        self.blow
    }

    /// Which way the current attack throws a target at `pos`: along the slash,
    /// or straight out from the spinning body.
    pub fn blow_dir(&self, ppos: Vec2, pos: Vec2) -> Vec2 {
        if self.spin.is_some() {
            (pos - ppos).normalize_or(self.slash_aim)
        } else {
            self.slash_aim
        }
    }

    /// The eat key was pressed this frame, with the mouth free.
    pub fn wants_to_eat(&self) -> bool {
        self.eat
    }

    /// Where the current (or last) slash goes: unit, world space.
    pub fn slash_dir(&self) -> Vec2 {
        self.slash_aim
    }

    /// Jerk the head forward: venom was just spat.
    pub fn spit(&mut self) {
        self.spat = Some(0.0);
        self.focus = 0.7;
    }

    /// Bite into remains.
    pub fn bite(&mut self) {
        self.bite = Some(0.0);
    }

    /// Tiny xorshift RNG in 0..1, good enough for eye twitches.
    fn rand(&mut self) -> f32 {
        self.rng ^= self.rng << 13;
        self.rng ^= self.rng >> 17;
        self.rng ^= self.rng << 5;
        (self.rng >> 8) as f32 / (1u32 << 24) as f32
    }
}

/// Frame-rate independent exponential smoothing.
fn smooth(current: f32, target: f32, rate: f32, dt: f32) -> f32 {
    lerp(current, target, 1.0 - (-rate * dt).exp())
}

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

