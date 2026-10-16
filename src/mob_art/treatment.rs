//! Bodies of the water treatment plant's mobs (`mobs/treatment.rs`): the
//! naiad (a dragonfly nymph caked in sludge, with its folding mask), the
//! siphon (a gaunt mosquito whose belly fills with the chameleon's blood) and
//! the sump leech (an inchworm-looping leech under the ceilings, pink with
//! the chemical it gorged on when it swells).

use std::f32::consts::PI;

use bevy::{prelude::*, sprite_render::AlphaMode2d};

use super::{BoneKind, Builder};
use crate::{
    camera,
    gore::Fluid,
    mobs::{treatment::HANG, Mob, MobKind, State},
    shapes::*,
};

const NAIAD_BODY: Color = Color::srgb(0.33, 0.31, 0.24);
const NAIAD_DARK: Color = Color::srgb(0.17, 0.16, 0.12);
const NAIAD_PALE: Color = Color::srgb(0.62, 0.58, 0.46);
const NAIAD_EYE: Color = Color::srgb(0.22, 0.12, 0.16);
/// The sludge crust it lies under.
const SLUDGE: Color = Color::srgb(0.19, 0.20, 0.15);
const SIPHON_BODY: Color = Color::srgb(0.23, 0.22, 0.21);
const SIPHON_LEG: Color = Color::srgb(0.14, 0.13, 0.13);
const SIPHON_PALE: Color = Color::srgb(0.70, 0.68, 0.64);
const WING: Color = Color::srgba(0.86, 0.88, 0.90, 0.26);
/// What fills a siphon's belly (on the color layer, like blood).
const BLOOD: Color = Color::srgba(0.72, 0.02, 0.03, 0.9);
const LEECH_SKIN: Color = Color::srgb(0.27, 0.24, 0.25);
const LEECH_DARK: Color = Color::srgb(0.12, 0.11, 0.11);
const LEECH_SHINE: Color = Color::srgb(0.66, 0.62, 0.64);
/// The chemical showing through a leech's skin as it swells (color layer).
const GUT: Color = Color::srgba(0.86, 0.52, 0.74, 0.85);

/// Naiad: mouth (where the mask hinges) in drawing units.
const HINGE: Vec2 = Vec2::new(8.2, -1.2);
/// Leech: segments, their radii, and the body's rest length (drawing units).
const SEGS: usize = 9;
const SEG_R: f32 = 2.5;
const LEECH_LEN: f32 = 22.0;

/// Bones of the treatment mobs.
#[derive(Clone, Copy)]
pub(super) enum Bone {
    /// Naiad: the sludge crust over it, with its eyes peeking (pivot on the floor).
    Mound,
    NaiadHead,
    /// Naiad / siphon legs (`pair`: 0 front .. 2 hind).
    Leg { pair: usize, near: bool },
    /// Naiad: segmented abdomen with its tail spikes.
    Tail,
    /// Naiad: the mask's shaft (a unit-long strip, stretched) and its hooked plate.
    Arm,
    Mask,
    SiphonWing { front: bool },
    Proboscis,
    /// Siphon: the abdomen, and the blood filling it.
    Belly,
    Blood,
    /// Leech: a body segment (0 = rear sucker .. `SEGS - 1` = head), and the gut glow.
    Seg(usize),
    Gut,
}

fn part_on_color(b: &mut Builder, mesh: Mesh, mat: &Handle<ColorMaterial>, pos: Vec3, parent: Entity) {
    let mesh = b.meshes.add(mesh);
    b.commands.spawn((Mesh2d(mesh), MeshMaterial2d(mat.clone()), Transform::from_translation(pos), camera::color_layer(), ChildOf(parent)));
}

fn blend(materials: &mut Assets<ColorMaterial>, color: Color) -> Handle<ColorMaterial> {
    materials.add(ColorMaterial { color, alpha_mode: AlphaMode2d::Blend, ..default() })
}

/// Whether this module draws (and animates the rig of) `kind`.
pub(super) fn draws(kind: MobKind) -> bool {
    matches!(kind, MobKind::Naiad | MobKind::Siphon | MobKind::SumpLeech)
}

/// A dragonfly nymph, facing +X, feet at y ≈ -4.6: squat and broad, bulging
/// eyes on a wide head, wing pads on its back, a tapering banded abdomen with
/// tail spikes, spiky splayed legs, and the mask folded under its chin.
pub(super) fn naiad(b: &mut Builder, materials: &mut Assets<ColorMaterial>, rig: Entity, root: Entity) -> Vec<(Handle<ColorMaterial>, Color)> {
    let body = materials.add(NAIAD_BODY);
    let dark = materials.add(NAIAD_DARK);
    let pale = materials.add(NAIAD_PALE);
    let eye = materials.add(NAIAD_EYE);
    let sludge = materials.add(SLUDGE);
    let glint = materials.add(Color::WHITE);

    // The crust it hides under: a low heap with two eyes peeking out of the front.
    let foot = MobKind::Naiad.spec().foot;
    let mound = b.bone(Vec3::new(0.0, -foot, 0.4), root, BoneKind::Treatment(Bone::Mound));
    let heap = smooth_closed(&[Vec2::new(-22.0, 0.0), Vec2::new(-12.0, 4.0), Vec2::new(2.0, 6.5), Vec2::new(16.0, 4.5), Vec2::new(24.0, 0.0)], 5);
    b.part(fill(&heap, |p| 0.6 + 0.4 * (p.y / 6.5)), &sludge, Vec3::ZERO, mound);
    let clods: Vec<[Vec2; 3]> = [-14.0f32, -4.0, 7.0, 17.0]
        .into_iter()
        .map(|x| [Vec2::new(x - 2.5, 0.5), Vec2::new(x + 2.0, 0.5), Vec2::new(x, 3.5 + (x * 0.7).sin())])
        .collect();
    b.part(triangles(&clods), &dark, Vec3::new(0.0, 0.0, 0.01), mound);
    for x in [11.0f32, 16.0] {
        b.part(fill(&ellipse_pts(2.6, 2.2, 12), |p| 0.6 + 0.4 * (p.y / 2.2)), &eye, Vec3::new(x, 5.0, 0.02), mound);
        b.part(fill(&ellipse_pts(0.6, 0.5, 6), |_| 1.0), &glint, Vec3::new(x + 0.6, 6.0, 0.03), mound);
    }

    // Legs: three pairs, spiky, splayed to the ground.
    let reach = [(4.0, 9.0), (1.0, 3.0), (-2.5, -8.0)];
    for near in [false, true] {
        let (mat, z, dx) = if near { (&body, 0.05, 0.0) } else { (&dark, -0.3, 1.0) };
        for (pair, (hip, end)) in reach.into_iter().enumerate() {
            let bone = b.bone(Vec3::new(hip + dx, -1.0, z), rig, BoneKind::Treatment(Bone::Leg { pair, near }));
            let knee = Vec2::new((end - hip) * 0.55, 2.2);
            let path = smooth_open(&[Vec2::ZERO, knee, Vec2::new(end - hip, -3.6)], 4);
            b.part(stroke(&path, |t| 1.5 - 0.9 * t, 1.0, 0.7, |_| 1.0), mat, Vec3::ZERO, bone);
            // A spine on the shin.
            b.part(triangles(&[[knee + Vec2::new(0.4, -1.0), knee + Vec2::new(-0.4, -1.4), knee + Vec2::new(1.6, 0.6)]]), mat, Vec3::ZERO, bone);
        }
    }

    // Abdomen: broad, flat, banded, tapering to three tail spikes.
    let tail = b.bone(Vec3::new(-2.5, 0.0, 0.05), rig, BoneKind::Treatment(Bone::Tail));
    let outline = smooth_closed(
        &[
            Vec2::new(1.0, 3.0),
            Vec2::new(-5.0, 4.0),
            Vec2::new(-11.0, 3.0),
            Vec2::new(-15.5, 0.8),
            Vec2::new(-16.0, -1.2),
            Vec2::new(-11.0, -2.8),
            Vec2::new(-4.0, -3.4),
            Vec2::new(1.0, -2.8),
        ],
        5,
    );
    b.part(fill(&outline, |p| 0.55 + 0.45 * ((p.y + 3.4) / 7.4).clamp(0.0, 1.0)), &body, Vec3::ZERO, tail);
    for x in [-2.5f32, -6.0, -9.5, -12.8] {
        let h = 3.9 - (x / 16.0).powi(2) * 3.0;
        let band = smooth_open(&[Vec2::new(x + 0.5, h), Vec2::new(x - 0.4, 0.0), Vec2::new(x + 0.3, -h + 0.6)], 3);
        b.part(stroke(&band, |_| 0.6, 1.0, 1.0, |_| 1.0), &dark, Vec3::new(0.0, 0.0, 0.01), tail);
    }
    b.part(
        triangles(&[
            [Vec2::new(-15.0, 0.8), Vec2::new(-15.5, -0.2), Vec2::new(-20.5, 1.5)],
            [Vec2::new(-15.5, -0.2), Vec2::new(-15.5, -1.2), Vec2::new(-20.0, -1.5)],
            [Vec2::new(-15.0, -1.0), Vec2::new(-14.0, -2.0), Vec2::new(-18.5, -3.6)],
        ]),
        &dark,
        Vec3::new(0.0, 0.0, -0.02),
        tail,
    );

    // Thorax, with the wing pads lying back along it.
    let thorax = smooth_closed(&[Vec2::new(7.5, 1.5), Vec2::new(5.0, 4.2), Vec2::new(-1.0, 4.4), Vec2::new(-3.0, 1.0), Vec2::new(-1.0, -3.0), Vec2::new(6.0, -2.8)], 5);
    b.part(fill(&thorax, |p| 0.6 + 0.4 * ((p.y + 3.0) / 7.4).clamp(0.0, 1.0)), &body, Vec3::new(0.0, 0.0, 0.1), rig);
    let pads = smooth_closed(&[Vec2::new(4.0, 4.0), Vec2::new(-2.0, 5.0), Vec2::new(-8.0, 4.4), Vec2::new(-7.0, 3.2), Vec2::new(-1.0, 3.0)], 4);
    b.part(fill(&pads, |p| 0.7 + 0.3 * ((p.y - 3.0) / 2.0).clamp(0.0, 1.0)), &dark, Vec3::new(0.0, 0.0, 0.15), rig);
    let sheen = smooth_open(&[Vec2::new(2.0, 4.2), Vec2::new(-3.0, 4.6), Vec2::new(-6.5, 4.1)], 4);
    b.part(stroke(&sheen, |t| 0.7 * (PI * t).sin(), 1.0, 1.0, |_| 1.0), &pale, Vec3::new(0.0, 0.0, 0.16), rig);

    // The mask: a shaft out of the chin (stretched by the strike) and a hooked plate at its end.
    let arm = b.bone(HINGE.extend(0.17), rig, BoneKind::Treatment(Bone::Arm));
    b.part(stroke(&[Vec2::ZERO, Vec2::X], |_| 1.3, 1.0, 0.6, |_| 1.0), &pale, Vec3::ZERO, arm);
    let mask = b.bone(HINGE.extend(0.18), rig, BoneKind::Treatment(Bone::Mask));
    let plate = smooth_closed(&[Vec2::new(-1.0, 1.4), Vec2::new(1.6, 1.8), Vec2::new(2.6, 0.0), Vec2::new(1.6, -1.8), Vec2::new(-1.0, -1.2)], 4);
    b.part(fill(&plate, |p| 0.7 + 0.3 * (p.y / 1.8)), &pale, Vec3::ZERO, mask);
    for side in [1.0f32, -1.0] {
        let hook = smooth_open(&[Vec2::new(1.8, 1.2 * side), Vec2::new(4.2, 1.6 * side), Vec2::new(4.6, -0.2 * side)], 4);
        b.part(stroke(&hook, |t| 1.0 - 0.7 * t, 1.0, 1.0, |_| 1.0), &dark, Vec3::new(0.0, 0.0, 0.01), mask);
    }

    // Head: wide, the eyes bulging up off its top corners.
    let head = b.bone(Vec3::new(7.5, 0.6, 0.2), rig, BoneKind::Treatment(Bone::NaiadHead));
    let skull = smooth_closed(&[Vec2::new(-1.5, 2.6), Vec2::new(3.0, 2.2), Vec2::new(5.0, 0.0), Vec2::new(3.5, -2.4), Vec2::new(-1.5, -2.2)], 4);
    b.part(fill(&skull, |p| 0.6 + 0.4 * ((p.y + 2.4) / 5.0)), &body, Vec3::ZERO, head);
    b.part(fill(&ellipse_pts(2.4, 2.1, 14), |p| 0.6 + 0.4 * (p.y / 2.1)), &eye, Vec3::new(1.5, 2.6, 0.02), head);
    b.part(fill(&ellipse_pts(0.5, 0.45, 6), |_| 1.0), &glint, Vec3::new(2.1, 3.5, 0.03), head);
    for dy in [0.0f32, 0.8] {
        let ant = smooth_open(&[Vec2::new(3.5, 1.2 + dy), Vec2::new(5.5, 2.6 + dy), Vec2::new(7.0, 2.2 + dy)], 3);
        b.part(stroke(&ant, |t| 0.5 - 0.3 * t, 1.0, 1.0, |_| 1.0), &dark, Vec3::new(0.0, 0.0, 0.01), head);
    }

    vec![(body, NAIAD_BODY), (dark, NAIAD_DARK), (pale, NAIAD_PALE), (eye, NAIAD_EYE)]
}

/// A mosquito, facing +X, centered on its hunched thorax: long dangling legs,
/// narrow wings, a needle proboscis, a slender abdomen that fills with blood.
pub(super) fn siphon(b: &mut Builder, materials: &mut Assets<ColorMaterial>, rig: Entity) -> Vec<(Handle<ColorMaterial>, Color)> {
    let body = materials.add(SIPHON_BODY);
    let leg = materials.add(SIPHON_LEG);
    let pale = materials.add(SIPHON_PALE);
    let wing = blend(materials, WING);
    let blood = blend(materials, BLOOD);
    let wing_mesh = || {
        let pts: Vec<Vec2> = ellipse_pts(7.0, 1.4, 16).into_iter().map(|p| Vec2::from_angle(0.25).rotate(p + Vec2::new(-6.5, 0.0))).collect();
        fill(&pts, |p| 0.8 + 0.2 * (p.y / 3.0).clamp(-1.0, 1.0))
    };
    let back = b.bone(Vec3::new(-0.5, 2.2, -0.4), rig, BoneKind::Treatment(Bone::SiphonWing { front: false }));
    b.part(wing_mesh(), &wing, Vec3::ZERO, back);

    // Long jointed legs, hanging.
    for (pair, x) in [(0, 1.6), (1, 0.4), (2, -0.8)] {
        let bone = b.bone(Vec3::new(x, -1.6, -0.2), rig, BoneKind::Treatment(Bone::Leg { pair, near: true }));
        let k = pair as f32;
        let path = smooth_open(&[Vec2::ZERO, Vec2::new(1.5 - k * 1.5, 3.0), Vec2::new(3.0 - k * 3.2, -3.5), Vec2::new(3.5 - k * 4.0, -11.0)], 4);
        b.part(stroke(&path, |t| 0.7 - 0.45 * t, 1.0, 1.0, |t| if (t * 6.0).fract() < 0.2 { 1.6 } else { 1.0 }), &leg, Vec3::ZERO, bone);
    }

    // Abdomen: slender, banded; the blood inside shows on the color layer as it fills.
    let belly = b.bone(Vec3::new(-2.0, -0.2, 0.0), rig, BoneKind::Treatment(Bone::Belly));
    let outline = smooth_closed(&[Vec2::new(0.5, 1.2), Vec2::new(-4.0, 1.8), Vec2::new(-9.0, 1.0), Vec2::new(-11.5, -0.6), Vec2::new(-8.0, -1.8), Vec2::new(-3.0, -1.6), Vec2::new(0.5, -1.0)], 5);
    b.part(fill(&outline, |p| 0.6 + 0.4 * ((p.y + 1.8) / 3.6).clamp(0.0, 1.0)), &body, Vec3::ZERO, belly);
    for x in [-2.5f32, -5.0, -7.5] {
        b.part(stroke(&[Vec2::new(x + 0.3, 1.6), Vec2::new(x - 0.3, -1.6)], |_| 0.5, 1.0, 1.0, |_| 1.0), &pale, Vec3::new(0.0, 0.0, 0.01), belly);
    }
    let sac = b.bone(Vec3::new(-5.0, -0.2, 0.02), belly, BoneKind::Treatment(Bone::Blood));
    part_on_color(b, fill(&ellipse_pts(5.0, 1.4, 16), |p| 0.7 + 0.3 * (p.y / 1.4)), &blood, Vec3::ZERO, sac);

    // Thorax: a hunched hump.
    let hump = smooth_closed(&[Vec2::new(2.5, 0.5), Vec2::new(1.0, 2.8), Vec2::new(-1.8, 2.4), Vec2::new(-2.5, 0.0), Vec2::new(-0.5, -1.8), Vec2::new(2.0, -1.4)], 4);
    b.part(fill(&hump, |p| 0.6 + 0.4 * ((p.y + 1.8) / 4.6)), &body, Vec3::new(0.0, 0.0, 0.1), rig);
    b.part(stroke(&smooth_open(&[Vec2::new(-1.2, 2.2), Vec2::new(0.6, 2.6), Vec2::new(1.8, 1.4)], 3), |_| 0.5, 1.0, 1.0, |_| 1.0), &pale, Vec3::new(0.0, 0.0, 0.11), rig);

    // Head and needle.
    let needle = b.bone(Vec3::new(3.6, -0.8, 0.12), rig, BoneKind::Treatment(Bone::Proboscis));
    b.part(fill(&ellipse_pts(1.5, 1.4, 12), |p| 0.6 + 0.4 * (p.y / 1.4)), &body, Vec3::ZERO, needle);
    b.part(fill(&ellipse_pts(0.9, 1.0, 8), |_| 0.4), &leg, Vec3::new(0.4, 0.3, 0.01), needle);
    b.part(stroke(&[Vec2::new(1.0, -0.4), Vec2::new(5.0, -2.0), Vec2::new(8.5, -3.6)], |t| 0.55 - 0.4 * t, 1.0, 1.0, |_| 1.0), &leg, Vec3::new(0.0, 0.0, 0.02), needle);
    for dy in [0.0f32, 0.6] {
        let ant = smooth_open(&[Vec2::new(0.8, 0.6 + dy), Vec2::new(3.0, 2.2 + dy), Vec2::new(4.6, 1.8 + dy)], 3);
        b.part(stroke(&ant, |_| 0.35, 1.0, 1.0, |t| 0.6 + 0.4 * ((t * 12.0).sin() * 0.5 + 0.5)), &leg, Vec3::new(0.0, 0.0, 0.02), needle);
    }

    let front = b.bone(Vec3::new(-0.2, 2.4, 0.4), rig, BoneKind::Treatment(Bone::SiphonWing { front: true }));
    b.part(wing_mesh(), &wing, Vec3::ZERO, front);

    vec![(body, SIPHON_BODY), (leg, SIPHON_LEG), (pale, SIPHON_PALE)]
}

/// A leech under the ceiling, facing +X, its back against the ceiling at
/// y = `HANG` (world) above the center: a chain of ringed segments from the
/// rear sucker to the head sucker, looping down between them as it inches.
pub(super) fn sump_leech(b: &mut Builder, materials: &mut Assets<ColorMaterial>, rig: Entity) -> Vec<(Handle<ColorMaterial>, Color)> {
    let skin = materials.add(LEECH_SKIN);
    let dark = materials.add(LEECH_DARK);
    let shine = materials.add(LEECH_SHINE);
    let gut = blend(materials, GUT);
    let eye = materials.add(Color::srgb(0.05, 0.04, 0.04));
    for i in 0..SEGS {
        let u = i as f32 / (SEGS - 1) as f32;
        // Fattest a third from the back, thin at the head.
        let r = SEG_R * (0.75 + 0.45 * (PI * (u * 0.85 + 0.1)).sin()) * if i == SEGS - 1 { 0.8 } else { 1.0 };
        let seg = b.bone(Vec3::new(0.0, 0.0, 0.01 * i as f32), rig, BoneKind::Treatment(Bone::Seg(i)));
        b.part(fill(&ellipse_pts(r * 1.1, r, 14), move |p| 0.55 + 0.45 * ((r - p.y) / (2.0 * r))), &skin, Vec3::ZERO, seg);
        // A dark ring, and a wet shine on the underside (lit from below by nothing: it's grim).
        b.part(stroke(&[Vec2::new(r * 0.5, r * 0.95), Vec2::new(r * 0.3, -r * 0.95)], |_| 0.5, 1.0, 1.0, |_| 1.0), &dark, Vec3::new(0.0, 0.0, 0.005), seg);
        b.part(fill(&ellipse_pts(r * 0.45, 0.35, 6), |_| 1.0), &shine, Vec3::new(-0.2, -r * 0.6, 0.006), seg);
        if i == 0 || i == SEGS - 1 {
            // Suckers: flat discs pressed to the ceiling.
            b.part(fill(&ellipse_pts(r * 1.2, r * 0.45, 12), |_| 0.4), &dark, Vec3::new(0.0, r * 0.75, -0.01), seg);
        }
        if i == SEGS - 1 {
            for x in [0.6f32, 1.6] {
                b.part(fill(&ellipse_pts(0.35, 0.35, 6), |_| 1.0), &eye, Vec3::new(x, 0.6, 0.01), seg);
            }
        }
        if i == SEGS / 2 {
            let glow = b.bone(Vec3::new(0.0, 0.0, 0.02), seg, BoneKind::Treatment(Bone::Gut));
            part_on_color(b, fill(&ellipse_pts(SEG_R * 3.2, SEG_R * 1.1, 18), |p| 0.75 + 0.25 * (p.y / (SEG_R * 1.1))), &gut, Vec3::ZERO, glow);
        }
    }
    vec![(skin, LEECH_SKIN), (dark, LEECH_DARK), (shine, LEECH_SHINE)]
}

/// The rig of a treatment mob: facing, rising out of the floor, poses.
pub(super) fn rig(m: &Mob, tf: &mut Transform, tt: f32, dt: f32) {
    let s = m.spec.scale;
    tf.scale.x += (m.facing * s - tf.scale.x) * (1.0 - (-18.0 * dt).exp());
    let windup = m.windup();
    let shiver = windup * (tt * 60.0).sin() * 0.06;
    let dead = matches!(m.state, State::Dead { .. });
    let flip = |t: f32, landed: bool| PI * if landed { 1.0 } else { (t * 3.0).min(1.0) };
    let mut y = 0.0;
    let mut sy = s;
    let angle = match (m.kind, m.state) {
        (MobKind::Naiad, State::Dead { t, landed }) => flip(t, landed),
        (MobKind::Naiad, st) => {
            // Grows up out of the floor, feet staying put.
            let r = m.treatment.rise;
            y = -m.spec.foot * (1.0 - r);
            sy = s * r;
            match st {
                State::Windup(_) => -0.12 * windup + shiver,
                State::Attack(_) => 0.05,
                State::Hurt(h) => 0.25 * (1.0 - h / 0.3).max(0.0),
                _ => 0.0,
            }
        }
        (MobKind::Siphon, State::Dead { t, landed }) => flip(t, landed),
        (MobKind::Siphon, st) => {
            let latched = m.treatment.latch.is_some();
            match st {
                // Head down into the chameleon's back, pumping.
                _ if latched => -0.55 + (tt * 9.0).sin() * 0.05,
                State::Windup(_) => -0.25 * windup + shiver,
                State::Attack(_) => (m.vel * m.facing).to_angle().clamp(-1.2, 1.2),
                State::Hurt(h) => 0.4 * (1.0 - h / 0.3).max(0.0),
                // Dazed (shaken off): tumbling.
                State::Recover(r) if r < 0.0 => tt * 9.0,
                _ => -(m.vel.x * m.facing) * 0.001 + (tt * 2.2).sin() * 0.08 - 0.2 * m.treatment.bloat,
            }
        }
        // A dead leech lies on the floor, slack.
        (_, State::Dead { .. }) => 0.0,
        (_, State::Hurt(h)) => (tt * 40.0).sin() * 0.05 * (1.0 - h / 0.4).max(0.0),
        _ => 0.0,
    };
    if dead && m.kind == MobKind::Naiad {
        sy = s;
    }
    tf.translation.y = y;
    tf.scale.y = sy.max(0.001);
    tf.rotation = Quat::from_rotation_z(angle);
}

/// Animate one of their bones.
pub(super) fn animate(bone: Bone, m: &Mob, tf: &mut Transform, tt: f32) {
    let windup = m.windup();
    let (dead, landed) = match m.state {
        State::Dead { t, landed } => (Some(t), landed),
        _ => (None, false),
    };
    let s = m.spec.scale;
    match bone {
        Bone::Mound => {
            // Shows while it's under; shuffles along with it, quivers before it bursts out.
            let k = if dead.is_some() { 0.0 } else { (1.0 - m.treatment.rise * 1.8).max(0.0) };
            let crawl = (m.vel.x.abs() / 60.0).min(1.0);
            let heave = 1.0 + (tt * 3.0).sin() * 0.05 + crawl * (tt * 14.0).sin() * 0.12 + windup * (tt * 50.0).sin() * 0.15;
            tf.scale = Vec3::new(m.facing * s * k.max(0.001), s * k.max(0.001) * heave, 1.0);
        }
        Bone::NaiadHead => {
            let bob = if dead.is_some() { -0.3 } else { (tt * 3.0).sin() * 0.04 + 0.12 * windup };
            tf.rotation = Quat::from_rotation_z(bob);
        }
        Bone::Leg { pair, .. } if m.kind == MobKind::Siphon => {
            let base = [0.2, 0.0, -0.2][pair];
            let angle = if let Some(td) = dead {
                base + (tt * 28.0 + pair as f32).sin() * 0.4 * (1.0 - td / 4.0).max(0.1)
            } else if m.treatment.latch.is_some() {
                // Clasping.
                base * 2.0 - 0.6 + (tt * 6.0 + pair as f32).sin() * 0.05
            } else {
                // Dangling, trailing the flight; spread wide before the lunge.
                let spread = windup * [0.7, 0.0, -0.7][pair];
                base + spread - 0.25 * (m.vel.x * m.facing / 200.0).clamp(-1.0, 1.0) + (tt * 2.5 + pair as f32 * 1.3).sin() * 0.12
            };
            tf.rotation = Quat::from_rotation_z(angle);
        }
        Bone::Leg { pair, near } => {
            // Naiad: a slow crawl; tucked in under the crust; braced out to strike.
            let base = [0.1, 0.0, -0.1][pair];
            let angle = if let Some(td) = dead {
                base + (tt * 26.0 + pair as f32 * 1.7).sin() * 0.45 * (1.0 - td / 5.0).max(0.1)
            } else {
                let group = if (pair == 1) != near { 0.0 } else { PI };
                let amp = ((m.vel.x.abs() / 80.0).min(1.0) * 0.35).max(windup * 0.15);
                let brace = if matches!(m.state, State::Attack(_)) { -0.15 * [1.0, 0.0, -1.0][pair] } else { 0.0 };
                base + (m.stride * 1.6 + group).sin() * amp + brace
            };
            tf.rotation = Quat::from_rotation_z(angle);
        }
        Bone::Tail => {
            // Pumps water through its gut (it breathes through its rear); curls when it strikes.
            let curl = if matches!(m.state, State::Attack(_)) { 0.18 } else { 0.1 * windup };
            let angle = if dead.is_some() { 0.2 } else { (tt * 2.4).sin() * 0.05 + curl };
            tf.rotation = Quat::from_rotation_z(angle);
            tf.scale.y = 1.0 + (tt * 2.4).sin() * 0.06;
        }
        Bone::Arm => {
            // The shaft: out to the plate, folded back to nothing at rest.
            let reach = m.treatment.jaw / s;
            let quiver = windup * (tt * 45.0).sin() * 0.4;
            tf.scale = Vec3::new((reach + quiver.abs()).max(0.001), 1.0, 1.0);
        }
        Bone::Mask => {
            let reach = m.treatment.jaw / s;
            let quiver = windup * (tt * 45.0).sin() * 0.4;
            tf.translation.x = HINGE.x + reach + quiver;
            // The hooks open on the way out, snap shut once out.
            let open = if matches!(m.state, State::Attack(_)) { 1.3 } else { 1.0 + windup * 0.4 };
            tf.scale = Vec3::new(1.0, open, 1.0);
        }
        Bone::SiphonWing { front } => {
            if dead.is_some() {
                tf.rotation = Quat::from_rotation_z(-0.4);
                tf.scale.y = 1.0;
            } else if m.treatment.latch.is_some() {
                // Folded flat while it drinks, a shiver now and then.
                let flick = if (tt * 1.7).sin() > 0.95 { (tt * 80.0).sin() * 0.2 } else { 0.0 };
                tf.rotation = Quat::from_rotation_z(-0.15 + flick);
                tf.scale.y = 1.0;
            } else {
                let flap = (tt * 130.0 + if front { 0.0 } else { 1.2 }).sin();
                tf.rotation = Quat::from_rotation_z(flap * 0.5);
                tf.scale.y = 0.3 + 0.7 * flap.abs();
            }
        }
        Bone::Proboscis => {
            // Lowered onto the target in the windup, buried in it while drinking.
            let down = if m.treatment.latch.is_some() { -0.5 } else { -0.35 * windup };
            let pump = if m.treatment.latch.is_some() { (tt * 9.0).sin() * 0.05 } else { 0.0 };
            tf.rotation = Quat::from_rotation_z(if dead.is_some() { -0.6 } else { down + pump });
        }
        Bone::Belly => {
            let b = m.treatment.bloat;
            let pump = if m.treatment.latch.is_some() { (tt * 9.0).sin() * 0.06 } else { 0.0 };
            tf.scale = Vec3::new(1.0 + 0.25 * b, 1.0 + 1.5 * b + pump, 1.0);
            tf.rotation = Quat::from_rotation_z(0.25 * b + if dead.is_some() { 0.0 } else { (tt * 3.0).sin() * 0.05 });
        }
        Bone::Blood => {
            let b = m.treatment.bloat;
            tf.scale = Vec3::new(b.max(0.001).sqrt(), b.max(0.001), 1.0);
        }
        Bone::Seg(i) => {
            let u = i as f32 / (SEGS - 1) as f32;
            let top = HANG / s - SEG_R;
            let (x, y) = if dead.is_some() {
                // Slack on the floor (the rig sits a little above it).
                let len = LEECH_LEN * 1.05;
                (-len / 2.0 + u * len, -1.0 + 1.6 * (u - 0.5).powi(2) * if landed { 1.0 } else { -2.0 })
            } else {
                // Inching: the ends close up and the middle loops down, then it stretches out.
                let phase = m.stride * 1.4 + tt * 0.6;
                let draw = 0.5 + 0.5 * phase.cos();
                let len = LEECH_LEN * (0.62 + 0.38 * draw);
                let sag = (LEECH_LEN - len) * 0.75 + 1.5 * m.treatment.swell + (tt * 2.0).sin() * 0.3;
                // Swelling, the middle hangs heavier.
                (-len / 2.0 + u * len, top - sag * (PI * u).sin())
            };
            tf.translation.x = x;
            tf.translation.y = y;
            let fat = 1.0 + 0.55 * m.treatment.swell * (PI * u).sin();
            let wince = if matches!(m.state, State::Hurt(_)) { 0.85 } else { 1.0 };
            tf.scale = Vec3::new(fat * wince, fat, 1.0);
        }
        Bone::Gut => {
            let k = if dead.is_some() { 0.0 } else { m.treatment.swell };
            tf.scale = Vec3::new(k.max(0.001), k.max(0.001), 1.0);
        }
    }
}

/// Parts torn off a fresh corpse: (velocity, spin, what it bleeds), or `None` to keep it.
pub(super) fn torn(bone: Bone, m: &Mob) -> Option<(Vec2, f32, Option<Fluid>)> {
    let f = m.facing;
    let blood = Some(m.spec.blood);
    Some(match bone {
        Bone::Mask => (Vec2::new(f * 200.0, 380.0), 11.0, blood),
        Bone::Leg { pair: 0, near: true } => (Vec2::new(f * 160.0, 420.0), -9.0, None),
        Bone::Leg { pair: 2, near: true } if m.kind == MobKind::Naiad => (Vec2::new(-f * 140.0, 360.0), 8.0, None),
        Bone::Leg { pair: 1, near: true } if m.kind == MobKind::Siphon => (Vec2::new(-f * 90.0, 300.0), 14.0, None),
        Bone::SiphonWing { front } => (Vec2::new(-f * if front { 220.0 } else { 130.0 }, 460.0), 7.0, None),
        Bone::Proboscis => (Vec2::new(f * 180.0, 360.0), -12.0, blood),
        // The head end of a leech comes off, spilling the chemical.
        Bone::Seg(i) if i == SEGS - 1 => (Vec2::new(f * 170.0, 300.0), 6.0, blood),
        _ => return None,
    })
}
