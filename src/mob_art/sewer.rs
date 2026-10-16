//! The old sewer's mobs (behavior in `mobs/sewer.rs`): bile leeches, gullet
//! toads, plague bloats and drain newts. Side views facing +X, like the rest.

use std::f32::consts::PI;

use bevy::prelude::*;

use super::{BoneKind, Builder, EYE};
use crate::{
    gore::Fluid,
    mobs::{sewer::TOAD_MOUTH, Mob, MobKind, State},
    shapes::*,
};

const LEECH: Color = Color::srgb(0.13, 0.12, 0.10);
const LEECH_STRIPE: Color = Color::srgb(0.46, 0.34, 0.20);
const LEECH_MOUTH: Color = Color::srgb(0.30, 0.10, 0.09);
const TOAD: Color = Color::srgb(0.32, 0.33, 0.22);
const TOAD_DARK: Color = Color::srgb(0.16, 0.17, 0.11);
const TOAD_BELLY: Color = Color::srgb(0.70, 0.66, 0.50);
/// The frog's poison, on its kin.
const WART: Color = Color::srgb(0.40, 0.10, 0.52);
const SAC: Color = Color::srgb(0.62, 0.30, 0.78);
const TONGUE: Color = Color::srgb(0.66, 0.38, 0.44);
const IRIS: Color = Color::srgb(0.85, 0.62, 0.10);
const BLOAT: Color = Color::srgb(0.64, 0.60, 0.50);
const BLOAT_VEIN: Color = Color::srgb(0.30, 0.16, 0.34);
const NEWT: Color = Color::srgb(0.20, 0.19, 0.16);
const NEWT_BELLY: Color = Color::srgb(0.86, 0.46, 0.14);
/// Blind, milky eyes.
const MILK: Color = Color::srgb(0.82, 0.82, 0.76);

/// Leech rings (radii, head to tail), resting on a belly line this far below the center.
const RINGS: [(f32, f32); 6] = [(1.9, 1.6), (2.0, 2.0), (2.1, 2.3), (2.1, 2.4), (2.0, 2.3), (2.0, 2.2)];
const RING_GAP: f32 = 2.2;
const LEECH_BELLY: f32 = 2.6;

/// A bone of a sewer mob.
#[derive(Clone, Copy)]
pub(super) enum Part {
    /// Leech body ring (0 = head).
    Ring(usize),
    Leg { front: bool, near: bool },
    /// The toad's throat sac.
    Throat,
    /// The toad's tongue (a unit strip, scaled out to its length) and its sticky tip.
    Tongue,
    TongueTip,
    /// The bloat's swollen body.
    Sack,
    Head,
    Tail,
}

type Mats = Vec<(Handle<ColorMaterial>, Color)>;

fn ring_x(i: usize) -> f32 {
    5.5 - i as f32 * RING_GAP
}

/// A bile leech: a glossy black ringed slug with pale side stripes and a sucker at each end; belly at y ≈ -2.6.
pub(super) fn leech(b: &mut Builder, materials: &mut Assets<ColorMaterial>, rig: Entity) -> Mats {
    let body = materials.add(LEECH);
    let stripe = materials.add(LEECH_STRIPE);
    let mouth = materials.add(LEECH_MOUTH);
    let gloss = materials.add(Color::WHITE);
    for (i, (rx, ry)) in RINGS.into_iter().enumerate() {
        let seg = b.bone(Vec3::new(ring_x(i), ry - LEECH_BELLY, -0.01 * i as f32), rig, BoneKind::Sewer(Part::Ring(i)));
        b.part(fill(&ellipse_pts(rx, ry, 14), move |p| 0.55 + 0.45 * ((p.y + ry) / (2.0 * ry))), &body, Vec3::ZERO, seg);
        b.part(fill(&ellipse_pts(rx * 0.55, 0.35, 8), |_| 1.0), &stripe, Vec3::new(0.0, ry * 0.25, 0.01), seg);
        b.part(fill(&ellipse_pts(rx * 0.35, 0.3, 6), |_| 1.0), &gloss, Vec3::new(0.3, ry - 0.6, 0.02), seg);
        if i == 0 {
            // The mouth sucker, with a ring of teeth.
            b.part(fill(&ellipse_pts(0.9, 1.2, 10), |_| 0.9), &mouth, Vec3::new(1.6, -0.4, 0.03), seg);
            let teeth: Vec<[Vec2; 3]> = (0..4)
                .map(|k| {
                    let y = -1.2 + k as f32 * 0.7;
                    [Vec2::new(2.2, y), Vec2::new(2.2, y + 0.4), Vec2::new(2.9, y + 0.2)]
                })
                .collect();
            b.part(triangles(&teeth), &stripe, Vec3::new(0.0, 0.0, 0.04), seg);
        }
        if i == RINGS.len() - 1 {
            // The tail sucker: a flat disc.
            b.part(fill(&ellipse_pts(1.0, 2.3, 10), |p| 0.6 + 0.2 * p.y), &body, Vec3::new(-2.1, -0.2, -0.01), seg);
        }
    }
    vec![(body, LEECH), (stripe, LEECH_STRIPE)]
}

/// A gullet toad: squat, warty, a wide mouth and a throat sac; feet at y ≈ -5.
pub(super) fn toad(b: &mut Builder, materials: &mut Assets<ColorMaterial>, rig: Entity) -> Mats {
    let skin = materials.add(TOAD);
    let dark = materials.add(TOAD_DARK);
    let belly = materials.add(TOAD_BELLY);
    let wart = materials.add(WART);
    let sac = materials.add(SAC);
    let tongue = materials.add(TONGUE);
    let iris = materials.add(IRIS);
    let pupil = materials.add(EYE);
    // Legs: the far ones darker, behind the body.
    for near in [false, true] {
        let (mat, z, dx) = if near { (&skin, 0.05, 0.0) } else { (&dark, -0.3, 1.0) };
        let hind = b.bone(Vec3::new(-4.5 + dx, -1.5, z), rig, BoneKind::Sewer(Part::Leg { front: false, near }));
        let thigh = smooth_open(&[Vec2::ZERO, Vec2::new(2.5, -1.6), Vec2::new(0.5, -3.4), Vec2::new(4.2, -3.5)], 4);
        b.part(stroke(&thigh, |t| 2.6 - 1.6 * t, 1.0, 0.7, |_| 1.0), mat, Vec3::ZERO, hind);
        let arm = b.bone(Vec3::new(4.5 + dx, -2.0, z), rig, BoneKind::Sewer(Part::Leg { front: true, near }));
        let path = smooth_open(&[Vec2::ZERO, Vec2::new(0.9, -1.5), Vec2::new(1.9, -3.0)], 3);
        b.part(stroke(&path, |t| 1.4 - 0.6 * t, 1.0, 0.8, |_| 1.0), mat, Vec3::ZERO, arm);
    }
    let body = smooth_closed(
        &[
            Vec2::new(8.6, 0.4),
            Vec2::new(7.0, 3.4),
            Vec2::new(3.0, 5.0),
            Vec2::new(-3.0, 4.8),
            Vec2::new(-7.5, 2.4),
            Vec2::new(-8.0, -1.0),
            Vec2::new(-5.0, -3.8),
            Vec2::new(2.0, -4.3),
            Vec2::new(7.0, -2.4),
        ],
        5,
    );
    b.part(fill(&body, |p| 0.5 + 0.5 * ((p.y + 4.3) / 9.3).clamp(0.0, 1.0)), &skin, Vec3::ZERO, rig);
    let under = smooth_open(&[Vec2::new(6.0, -2.6), Vec2::new(1.0, -3.9), Vec2::new(-5.0, -3.3)], 4);
    b.part(stroke(&under, |t| 1.4 * (PI * t).sin(), 1.0, 1.0, |_| 1.0), &belly, Vec3::new(0.0, 0.0, 0.01), rig);
    // Poison warts over the back.
    for (x, y, r) in [(-5.5, 2.2, 0.8), (-3.0, 3.8, 1.0), (-0.5, 4.2, 0.7), (1.8, 3.6, 0.9), (-6.5, 0.2, 0.6), (-2.0, 1.8, 0.6)] {
        b.part(fill(&ellipse_pts(r, r * 0.8, 8), |p| 0.7 + 0.3 * p.y), &wart, Vec3::new(x, y, 0.02), rig);
    }
    // The wide mouth, the eye bump.
    let lips = smooth_open(&[Vec2::new(8.7, -0.3), Vec2::new(6.0, -0.1), Vec2::new(2.8, 0.6)], 4);
    b.part(stroke(&lips, |t| 0.7 - 0.3 * t, 1.0, 1.0, |_| 1.0), &dark, Vec3::new(0.0, 0.0, 0.03), rig);
    b.part(fill(&ellipse_pts(2.0, 1.7, 12), |p| 0.7 + 0.3 * (p.y / 1.7)), &skin, Vec3::new(4.5, 4.4, 0.03), rig);
    b.part(fill(&ellipse_pts(1.2, 1.1, 10), |_| 1.0), &iris, Vec3::new(4.9, 4.7, 0.04), rig);
    b.part(fill(&ellipse_pts(0.9, 0.3, 8), |_| 1.0), &pupil, Vec3::new(5.0, 4.7, 0.05), rig);
    let brow = [[Vec2::new(2.8, 5.6), Vec2::new(6.6, 5.0), Vec2::new(3.4, 5.0)]];
    b.part(triangles(&brow), &dark, Vec3::new(0.0, 0.0, 0.06), rig);
    let throat = b.bone(Vec3::new(5.2, -3.0, 0.02), rig, BoneKind::Sewer(Part::Throat));
    b.part(fill(&ellipse_pts(2.8, 1.9, 14), |p| 0.7 + 0.3 * (p.y / 1.9)), &sac, Vec3::ZERO, throat);
    // The tongue: a strip out of the mouth, and its sticky tip.
    let strip = b.bone(TOAD_MOUTH.extend(0.07), rig, BoneKind::Sewer(Part::Tongue));
    b.part(stroke(&[Vec2::ZERO, Vec2::Y], |_| 1.5, 1.0, 0.7, |_| 1.0), &tongue, Vec3::ZERO, strip);
    let tip = b.bone(TOAD_MOUTH.extend(0.08), rig, BoneKind::Sewer(Part::TongueTip));
    b.part(fill(&ellipse_pts(2.0, 1.6, 12), |p| 0.7 + 0.3 * (p.y / 1.6)), &tongue, Vec3::ZERO, tip);
    vec![(skin, TOAD), (dark, TOAD_DARK), (wart, WART), (sac, SAC)]
}

/// A plague bloat: a toad swollen into a blistered sack, its little head sunk in front; feet at y ≈ -9.
pub(super) fn bloat(b: &mut Builder, materials: &mut Assets<ColorMaterial>, rig: Entity) -> Mats {
    let skin = materials.add(BLOAT);
    let dark = materials.add(scale_color(BLOAT, 0.45));
    let vein = materials.add(BLOAT_VEIN);
    let wart = materials.add(WART);
    let eye = materials.add(EYE);
    for near in [false, true] {
        let (mat, z) = if near { (&skin, 0.3) } else { (&dark, -0.3) };
        for (front, x) in [(true, 5.0), (false, -6.0)] {
            let x = x + if near { 0.0 } else { 1.2 };
            let leg = b.bone(Vec3::new(x, -5.5, z), rig, BoneKind::Sewer(Part::Leg { front, near }));
            let path = smooth_open(&[Vec2::ZERO, Vec2::new(0.6, -1.8), Vec2::new(1.6, -3.5)], 3);
            b.part(stroke(&path, |t| 2.6 - 1.0 * t, 1.0, 0.7, |_| 1.0), mat, Vec3::ZERO, leg);
        }
    }
    let sack = b.bone(Vec3::new(-1.0, 1.5, 0.0), rig, BoneKind::Sewer(Part::Sack));
    let mut rng = Rng(0xB10A7);
    let outline: Vec<Vec2> = ellipse_pts(10.5, 9.0, 24).into_iter().map(|p| p * rng.range(0.96, 1.04)).collect();
    b.part(fill(&smooth_closed(&outline, 3), |p| 0.5 + 0.5 * ((p.y + 9.0) / 18.0) - 0.1 * (p.x / 10.5).abs()), &skin, Vec3::ZERO, sack);
    // Veins crawling over the skin, and blisters full of poison.
    for (from, mid, to) in [
        (Vec2::new(-8.0, -2.0), Vec2::new(-4.0, 3.0), Vec2::new(1.0, 6.5)),
        (Vec2::new(-2.0, -6.5), Vec2::new(0.5, -1.0), Vec2::new(6.0, 2.5)),
        (Vec2::new(-9.0, 3.0), Vec2::new(-6.0, 6.0), Vec2::new(-2.0, 8.0)),
    ] {
        let path = smooth_open(&[from, mid, to], 5);
        b.part(stroke(&path, |t| 0.6 * (1.0 - t) + 0.15, 1.0, 1.0, |_| 1.0), &vein, Vec3::new(0.0, 0.0, 0.01), sack);
    }
    for (x, y, r) in [(-3.0, 5.0, 1.6), (2.5, 6.0, 1.1), (-6.5, 1.0, 1.3), (4.5, 0.5, 1.0), (-1.0, -3.5, 0.9), (0.5, 2.0, 0.7), (-7.0, 5.5, 0.8)] {
        b.part(fill(&ellipse_pts(r, r * 0.9, 10), |p| 0.6 + 0.4 * (p.y / 1.6).clamp(-1.0, 1.0)), &wart, Vec3::new(x, y, 0.02), sack);
    }
    // The head, half sunk in the swelling: a slit eye and a drooling slit mouth.
    let head = smooth_closed(&[Vec2::new(5.5, -1.5), Vec2::new(9.0, -1.8), Vec2::new(11.0, -3.5), Vec2::new(9.5, -5.5), Vec2::new(6.0, -5.5)], 4);
    b.part(fill(&head, |p| 0.6 + 0.4 * ((p.y + 5.5) / 4.0)), &dark, Vec3::new(0.0, 0.0, 0.1), rig);
    b.part(fill(&ellipse_pts(0.8, 0.35, 8), |_| 1.0), &eye, Vec3::new(8.8, -2.7, 0.11), rig);
    let mouth = smooth_open(&[Vec2::new(10.6, -4.0), Vec2::new(8.5, -4.3), Vec2::new(7.0, -4.0)], 3);
    b.part(stroke(&mouth, |_| 0.4, 1.0, 1.0, |_| 1.0), &vein, Vec3::new(0.0, 0.0, 0.11), rig);
    vec![(skin, BLOAT), (dark, scale_color(BLOAT, 0.45)), (wart, WART)]
}

/// A drain newt: long, low and slimy, a fiery belly and violet spots, blind milky eyes; feet at y ≈ -4.5.
pub(super) fn newt(b: &mut Builder, materials: &mut Assets<ColorMaterial>, rig: Entity) -> Mats {
    let skin = materials.add(NEWT);
    let dark = materials.add(scale_color(NEWT, 0.6));
    let belly = materials.add(NEWT_BELLY);
    let spot = materials.add(WART);
    let milk = materials.add(MILK);
    let gloss = materials.add(Color::WHITE);
    let tail = b.bone(Vec3::new(-7.0, 0.0, -0.05), rig, BoneKind::Sewer(Part::Tail));
    let path = smooth_open(&[Vec2::ZERO, Vec2::new(-5.0, 0.8), Vec2::new(-10.0, 0.3), Vec2::new(-15.0, -1.2)], 5);
    b.part(stroke(&path, |t| 3.6 * (1.0 - t) + 0.2, 1.0, 0.7, |_| 1.0), &skin, Vec3::ZERO, tail);
    for near in [false, true] {
        let (mat, z, dx) = if near { (&skin, 0.05, 0.0) } else { (&dark, -0.3, 1.0) };
        for (front, x) in [(true, 4.0), (false, -4.5)] {
            let leg = b.bone(Vec3::new(x + dx, -1.2, z), rig, BoneKind::Sewer(Part::Leg { front, near }));
            let reach = if front { 1.0 } else { -1.0 };
            let path = smooth_open(&[Vec2::ZERO, Vec2::new(1.8 * reach, -1.0), Vec2::new(2.2 * reach, -3.3)], 3);
            b.part(stroke(&path, |t| 1.4 - 0.7 * t, 1.0, 0.8, |_| 1.0), mat, Vec3::ZERO, leg);
            let toes = [Vec2::new(2.2 * reach - 0.8, -3.3), Vec2::new(2.2 * reach + 1.0, -3.4)];
            b.part(stroke(&toes, |_| 0.5, 1.0, 1.0, |_| 1.0), mat, Vec3::ZERO, leg);
        }
    }
    let body = smooth_closed(
        &[
            Vec2::new(6.5, 1.0),
            Vec2::new(3.0, 2.8),
            Vec2::new(-3.0, 2.6),
            Vec2::new(-7.5, 1.2),
            Vec2::new(-8.0, -0.8),
            Vec2::new(-3.0, -2.2),
            Vec2::new(3.0, -2.2),
            Vec2::new(6.5, -1.0),
        ],
        5,
    );
    b.part(fill(&body, |p| 0.5 + 0.5 * ((p.y + 2.2) / 5.0).clamp(0.0, 1.0)), &skin, Vec3::ZERO, rig);
    let under = smooth_open(&[Vec2::new(5.5, -1.6), Vec2::new(0.0, -2.0), Vec2::new(-6.5, -1.2)], 4);
    b.part(stroke(&under, |t| 1.3 * (PI * t).sin() + 0.2, 1.0, 1.0, |_| 1.0), &belly, Vec3::new(0.0, 0.0, 0.01), rig);
    for (x, y, r) in [(-5.0, 1.2, 0.6), (-2.0, 1.9, 0.7), (1.2, 1.7, 0.5), (3.8, 1.3, 0.6), (-0.5, 0.4, 0.4)] {
        b.part(fill(&ellipse_pts(r, r * 0.8, 8), |_| 1.0), &spot, Vec3::new(x, y, 0.02), rig);
    }
    let shine = smooth_open(&[Vec2::new(-5.0, 2.0), Vec2::new(-1.0, 2.5), Vec2::new(3.0, 2.2)], 4);
    b.part(stroke(&shine, |t| 0.5 * (PI * t).sin(), 1.0, 1.0, |_| 1.0), &gloss, Vec3::new(0.0, 0.0, 0.03), rig);
    let head = b.bone(Vec3::new(6.0, 0.2, 0.1), rig, BoneKind::Sewer(Part::Head));
    let skull = smooth_closed(&[Vec2::new(-1.0, 2.0), Vec2::new(2.5, 1.8), Vec2::new(5.0, 0.6), Vec2::new(5.3, -0.6), Vec2::new(2.0, -1.6), Vec2::new(-1.0, -1.8)], 4);
    b.part(fill(&skull, |p| 0.55 + 0.45 * ((p.y + 1.8) / 3.8)), &skin, Vec3::ZERO, head);
    b.part(fill(&ellipse_pts(0.8, 0.7, 10), |_| 1.0), &milk, Vec3::new(2.2, 0.8, 0.02), head);
    let lips = smooth_open(&[Vec2::new(5.2, -0.4), Vec2::new(3.0, -0.8), Vec2::new(0.5, -0.6)], 3);
    b.part(stroke(&lips, |_| 0.35, 1.0, 1.0, |_| 1.0), &dark, Vec3::new(0.0, 0.0, 0.02), head);
    vec![(skin, NEWT), (belly, NEWT_BELLY), (spot, WART)]
}

/// The whole body's pose (instead of the shared one).
pub(super) fn rig(m: &Mob, tf: &mut Transform, tt: f32, dt: f32) {
    let s = m.spec.scale;
    tf.scale.x += (m.facing * s - tf.scale.x) * (1.0 - (-18.0 * dt).exp());
    let windup = m.windup();
    let shiver = windup * (tt * 60.0).sin() * 0.06;
    let (sy, angle) = match (m.kind, m.state) {
        (_, State::Dead { t, landed }) => (1.0, PI * if landed { 1.0 } else { (t * 3.0).min(1.0) }),
        (MobKind::BileLeech, _) if m.airborne => (1.0, (m.vel * m.facing).to_angle().clamp(-0.9, 0.9) * 0.7),
        (MobKind::BileLeech, _) if m.sewer.latched.is_some() => (1.0, (tt * 8.0).sin() * 0.12),
        (MobKind::BileLeech, _) => (1.0, 0.0),
        (MobKind::GulletToad, _) if m.airborne => (1.0, (m.vel * m.facing).to_angle().clamp(-0.6, 0.6) * 0.5),
        // Crouches to lash; level while the tongue is out.
        (MobKind::GulletToad, State::Windup(_)) => (1.0 - 0.12 * windup, shiver * 0.5),
        (MobKind::GulletToad, _) => (1.0, 0.0),
        (MobKind::PlagueBloat, State::Windup(_)) => (1.0, shiver),
        (MobKind::PlagueBloat, _) => (1.0, (m.stride * 2.0).sin() * 0.06 * (m.vel.x.abs() / 20.0).min(1.0)),
        // Upside down on the ceiling, turning over as it falls.
        (MobKind::DrainNewt, State::Windup(_)) => (2.0 * m.sewer.turned - 1.0, shiver),
        (MobKind::DrainNewt, _) => (2.0 * m.sewer.turned - 1.0, 0.0),
        _ => (1.0, 0.0),
    };
    tf.scale.y = s * sy;
    tf.rotation = Quat::from_rotation_z(angle);
}

/// A sewer bone's pose.
pub(super) fn animate(part: Part, m: &Mob, tf: &mut Transform, tt: f32) {
    let dead = match m.state {
        State::Dead { t, .. } => Some(t),
        _ => None,
    };
    let windup = m.windup();
    let fade = dead.map_or(1.0, |td| (1.0 - td / 4.0).max(0.0));
    match part {
        Part::Ring(i) => {
            let n = RINGS.len() as f32 - 1.0;
            let k = i as f32 / n;
            // Inchworm: contracting arches the middle and draws the tail in.
            let c = if dead.is_some() || m.airborne {
                0.0
            } else if m.sewer.latched.is_some() {
                0.3 + 0.2 * (tt * 6.0).sin()
            } else {
                0.5 + 0.5 * m.sewer.inch.sin()
            };
            let arch = 3.0 * c * (PI * k).sin();
            let rear = windup * (1.0 - k).powi(2) * 7.0 + windup * (tt * 20.0 + k * 3.0).sin() * 0.4;
            let writhe = dead.map_or(0.0, |_| (tt * 14.0 + i as f32 * 1.3).sin() * 0.8 * fade);
            tf.translation.x = ring_x(i) * (1.0 - 0.22 * c);
            tf.translation.y = RINGS[i].1 - LEECH_BELLY + arch + rear + writhe;
            // Swollen with what it drank.
            let fed = 1.0 + 0.18 * m.sewer.fed.min(3.0);
            tf.scale = Vec3::new(fed, fed * (1.0 + 0.1 * c), 1.0);
        }
        Part::Throat => {
            let pump = match m.state {
                State::Dead { .. } => 0.6,
                State::Windup(_) => 1.0 + 0.6 * windup * (0.6 + 0.4 * (tt * 25.0).sin()),
                // Gulping after the lash.
                State::Recover(s) if s < 0.6 => 1.0 + 0.4 * (s * 18.0).sin().abs(),
                _ => 1.0 + 0.1 * (tt * 3.0).sin(),
            };
            tf.scale = Vec3::new(pump, pump, 1.0);
        }
        Part::Tongue | Part::TongueTip => {
            // From the mouth along the aim, in the rig's (flipped, scaled) frame.
            let len = if dead.is_some() { 0.0 } else { m.sewer.tongue };
            let s = m.spec.scale;
            let l = Vec2::new(m.sewer.aim.x * len / (m.facing * s), m.sewer.aim.y * len / s);
            let out = l.length() > 0.5;
            if let Part::Tongue = part {
                tf.rotation = Quat::from_rotation_z(l.to_angle() - PI / 2.0);
                tf.scale = Vec3::new(1.0, if out { l.length() } else { 0.001 }, 1.0);
            } else {
                tf.translation.x = TOAD_MOUTH.x + l.x;
                tf.translation.y = TOAD_MOUTH.y + l.y;
                tf.scale = Vec3::splat(if out { 1.0 } else { 0.001 });
            }
        }
        Part::Sack => {
            let swell = m.sewer.swell;
            let breathe = 0.03 * (tt * 2.0).sin();
            let (sx, sy) = match dead {
                // Burst and empty.
                Some(_) => (0.85, 0.6),
                None => (1.0 + 0.3 * swell + breathe, 1.0 + 0.4 * swell + breathe),
            };
            tf.scale = Vec3::new(sx, sy, 1.0);
            tf.translation.y = 1.5 + 4.0 * swell.max(0.0) - if dead.is_some() { 3.0 } else { 0.0 };
            tf.translation.x = -1.0 + windup * (tt * 50.0).sin() * 0.4;
        }
        Part::Head => {
            // The newt's head: sniffing, dipping before it drops or lunges.
            let angle = if dead.is_some() { -0.3 } else { (tt * 5.0).sin() * 0.06 - 0.35 * windup };
            tf.rotation = Quat::from_rotation_z(angle);
        }
        Part::Tail => {
            let run = (m.vel.x.abs() / 120.0).min(1.0);
            let angle = (tt * (2.0 + 7.0 * run)).sin() * (0.15 + 0.2 * run);
            tf.rotation = Quat::from_rotation_z(angle);
        }
        Part::Leg { front, near } => {
            let angle = if dead.is_some() {
                (tt * 28.0 + if front { 0.0 } else { 1.7 }).sin() * 0.5 * fade
            } else if m.airborne {
                // Kicked out on a hop, splayed in a fall.
                match m.kind {
                    MobKind::GulletToad if !front => 0.9,
                    _ => if front { 0.5 } else { -0.5 },
                }
            } else {
                let group = if front == near { 0.0 } else { PI };
                let rate = if m.kind == MobKind::DrainNewt { 1.2 } else { 2.0 };
                let amp = ((m.vel.x.abs() / 80.0).min(1.0) * 0.45).max(windup * 0.25);
                let crouch = if m.kind == MobKind::GulletToad && !front { -0.2 * windup } else { 0.0 };
                (m.stride * rate + group).sin() * amp + crouch
            };
            tf.rotation = Quat::from_rotation_z(angle);
        }
    }
}

/// What a fresh corpse loses: (velocity, spin, bleeds) for `boss::tear_off`.
pub(super) fn torn(part: Part, m: &Mob) -> Option<(Vec2, f32, Option<Fluid>)> {
    let f = m.facing;
    let blood = Some(m.spec.blood);
    match (m.kind, part) {
        (MobKind::BileLeech, Part::Ring(5)) => Some((Vec2::new(-f * 120.0, 280.0), 9.0, blood)),
        (MobKind::GulletToad, Part::Leg { front: false, near: true }) => Some((Vec2::new(-f * 140.0, 360.0), 8.0, blood)),
        (MobKind::PlagueBloat, Part::Leg { front: true, near: true }) => Some((Vec2::new(f * 160.0, 380.0), -8.0, blood)),
        // Newts shed their tail.
        (MobKind::DrainNewt, Part::Tail) => Some((Vec2::new(-f * 190.0, 320.0), 12.0, blood)),
        (MobKind::DrainNewt, Part::Leg { front: true, near: true }) => Some((Vec2::new(f * 160.0, 400.0), -9.0, None)),
        _ => None,
    }
}
