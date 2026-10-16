//! Bodies of the buried station's vermin (`mobs/subway.rs`): the plague tick
//! (a leathery sac that swells as it feeds, flips on its back), the ballast
//! mole (a wrinkled naked mole rat with yellow incisors, squeezed down into
//! the ground under a heap of gravel while it tunnels) and the rat knot (four
//! rats radiating from a clot of tangled tails and filth, the whole ball turning
//! as it rolls).

use std::f32::consts::PI;

use bevy::prelude::*;

use super::{BoneKind, Builder, EYE, RAT_FUR, RAT_SKIN};
use crate::{
    gore::Fluid,
    mobs::{Mob, State},
    shapes::*,
};

/// Naked mole rat: wrinkled grayish pink hide, yellowed incisors.
const MOLE_SKIN: Color = Color::srgb(0.70, 0.56, 0.52);
const MOLE_FOLD: Color = Color::srgb(0.40, 0.29, 0.27);
const TOOTH: Color = Color::srgb(0.82, 0.74, 0.48);
/// The track ballast it pushes up while tunneling.
const GRAVEL: Color = Color::srgb(0.40, 0.39, 0.37);
const GRAVEL_DARK: Color = Color::srgb(0.20, 0.19, 0.18);
/// Tick: a dark shield, a leathery gray sac.
const TICK_SHELL: Color = Color::srgb(0.30, 0.22, 0.16);
const TICK_SAC: Color = Color::srgb(0.52, 0.46, 0.42);
const TICK_DARK: Color = Color::srgb(0.12, 0.09, 0.07);
/// The knot: tails matted with filth and dried blood.
const KNOT_FILTH: Color = Color::srgb(0.17, 0.13, 0.11);

/// The knot ball's radius (drawing units): it turns this much per unit rolled.
pub(crate) const KNOT_BALL: f32 = 11.0;
/// The knot's center over its feet (drawing units).
pub(crate) const KNOT_FOOT: f32 = 12.0;
/// The knotted rats' headings from the knot (radians, in the ball's frame).
const KNOTTED: [f32; 4] = [0.4, 2.0, 3.5, 5.0];
/// Mole: its center over its feet; the tick's sac at rest.
const MOLE_FOOT: f32 = 5.0;
const SAC_AT: Vec2 = Vec2::new(-1.4, 0.4);

#[derive(Clone, Copy)]
pub(super) enum Bone {
    /// Knot: the whole ball (turns as it rolls).
    Ball,
    /// Knot: one of the knotted rats, squirming about its heading.
    Knotted(usize),
    /// Mole: the body, squeezed down into the ground from its feet when dug in.
    Body,
    /// Mole: the heap of gravel over it while it tunnels (heaves before it comes up).
    Mound,
    /// Mole: the incisors.
    Teeth,
    /// Tick: the whole body (flips on its back, tilts its mouth into the hide).
    TickBody,
    /// Tick: the sac (swells as it feeds).
    Sac,
    /// Tick: a leg (`i` front to back, near side or far).
    TickLeg(usize, bool),
}

/// What happens to a part of a fresh corpse.
pub(super) enum Tear {
    Off { vel: Vec2, spin: f32, bleeds: Option<Fluid> },
    /// Gone from the body (the rats that got out of a dead knot alive).
    Gone,
    Stay,
}

type Mats = Vec<(Handle<ColorMaterial>, Color)>;

/// A rat knot, centered on the clot of tails, `KNOT_FOOT` over its feet.
pub(super) fn knot(b: &mut Builder, materials: &mut Assets<ColorMaterial>, rig: Entity) -> Mats {
    let fur = materials.add(RAT_FUR);
    let mangy = materials.add(scale_color(RAT_FUR, 0.72));
    let dark = materials.add(scale_color(RAT_FUR, 0.45));
    let skin = materials.add(RAT_SKIN);
    let filth = materials.add(KNOT_FILTH);
    let eye = materials.add(EYE);
    let glint = materials.add(Color::WHITE);
    let ball = b.bone(Vec3::ZERO, rig, BoneKind::Subway(Bone::Ball));
    // Rats drawn along +X, rump at the knot, snout out.
    let body = smooth_closed(
        &[
            Vec2::new(2.0, 0.0),
            Vec2::new(4.0, 3.2),
            Vec2::new(8.5, 3.9),
            Vec2::new(11.5, 2.6),
            Vec2::new(14.2, 0.4),
            Vec2::new(14.0, -0.8),
            Vec2::new(11.0, -2.4),
            Vec2::new(7.5, -3.4),
            Vec2::new(3.5, -2.8),
        ],
        4,
    );
    for i in 0..KNOTTED.len() {
        let mat = if i % 2 == 0 { &fur } else { &mangy };
        let rat = b.bone(Vec3::new(0.0, 0.0, 0.1 * i as f32), ball, BoneKind::Subway(Bone::Knotted(i)));
        // Hind and front paws, clawing at the air.
        for (x, reach) in [(5.0, -1.0), (10.5, 1.2)] {
            let paw = smooth_open(&[Vec2::new(x, -2.0), Vec2::new(x + reach * 0.5, -4.0), Vec2::new(x + reach, -5.2)], 3);
            b.part(stroke(&paw, |t| 1.2 - 0.7 * t, 1.0, 0.8, |_| 1.0), &dark, Vec3::new(0.0, 0.0, -0.02), rat);
        }
        b.part(fill(&body, |p| 0.5 + 0.5 * ((p.y + 3.4) / 7.3).clamp(0.0, 1.0)), mat, Vec3::ZERO, rat);
        if i % 2 == 1 {
            // Mange: bald, scabbed patches.
            b.part(fill(&ellipse_pts(1.7, 1.0, 10), |_| 0.8), &skin, Vec3::new(7.0, 1.4, 0.01), rat);
            b.part(fill(&ellipse_pts(0.6, 0.4, 6), |_| 1.0), &filth, Vec3::new(7.4, 1.2, 0.02), rat);
        }
        b.part(fill(&ellipse_pts(1.3, 1.7, 10), |_| 0.85), &skin, Vec3::new(10.8, 3.0, -0.01), rat);
        b.part(fill(&ellipse_pts(0.65, 0.65, 8), |_| 1.0), &eye, Vec3::new(12.0, 1.0, 0.02), rat);
        b.part(fill(&ellipse_pts(0.22, 0.22, 6), |_| 1.0), &glint, Vec3::new(12.2, 1.3, 0.03), rat);
        // Bared teeth.
        b.part(stroke(&[Vec2::new(13.8, -0.6), Vec2::new(14.3, -2.0)], |t| 0.6 - 0.3 * t, 1.0, 1.0, |_| 1.0), &skin, Vec3::new(0.0, 0.0, 0.02), rat);
    }
    // The tails, knotted and matted into a clot of filth in the middle.
    b.part(fill(&ellipse_pts(4.6, 4.2, 16), |p| 0.6 + 0.4 * ((p.y + 4.2) / 8.4)), &filth, Vec3::new(0.0, 0.0, 0.6), ball);
    for k in 0..6 {
        let a = k as f32 * 1.05;
        let lp: Vec<Vec2> = ellipse_pts(3.6, 1.5, 10).into_iter().map(|p| Vec2::from_angle(a).rotate(p + Vec2::new(0.6, 0.0))).collect();
        let mut path = smooth_closed(&lp, 3);
        path.push(path[0]);
        b.part(stroke(&path, |_| 0.8, 1.0, 0.6, |_| 1.0), &skin, Vec3::new(0.0, 0.0, 0.62 + 0.01 * k as f32), ball);
    }
    // Loose tail ends whipping out between the rats.
    for a in [1.2f32, 2.75, 4.25, 5.7] {
        let d = Vec2::from_angle(a);
        let path = smooth_open(&[d * 3.5, d * 7.0 + d.perp() * 1.5, d * 10.5 - d.perp() * 0.5, d * 13.0 + d.perp() * 1.0], 4);
        b.part(stroke(&path, |t| 1.0 - 0.75 * t, 1.0, 0.7, |_| 1.0), &skin, Vec3::new(0.0, 0.0, -0.05), ball);
    }
    vec![(fur, RAT_FUR), (mangy, scale_color(RAT_FUR, 0.72)), (skin, RAT_SKIN), (filth, KNOT_FILTH)]
}

/// A ballast mole, facing +X, feet at y = -`MOLE_FOOT`.
pub(super) fn mole(b: &mut Builder, materials: &mut Assets<ColorMaterial>, rig: Entity) -> Mats {
    let skin = materials.add(MOLE_SKIN);
    let fold = materials.add(MOLE_FOLD);
    let tooth = materials.add(TOOTH);
    let eye = materials.add(EYE);
    let gravel = materials.add(GRAVEL);
    let gravel_dark = materials.add(GRAVEL_DARK);
    // Everything on the body is drawn `o` up from its feet.
    let o = MOLE_FOOT;
    let body = b.bone(Vec3::new(0.0, -o, 0.0), rig, BoneKind::Subway(Bone::Body));
    let leg_path = smooth_open(&[Vec2::ZERO, Vec2::new(0.8, -1.6), Vec2::new(1.8, -3.0)], 3);
    for near in [false, true] {
        let (mat, z) = if near { (&skin, 0.05) } else { (&fold, -0.2) };
        for (pair, x) in [(0, 3.5), (2, -5.0)] {
            let bone = b.bone(Vec3::new(x + if near { 0.0 } else { 0.8 }, o - 1.8, z), body, BoneKind::Leg { pair, near });
            b.part(stroke(&leg_path, |t| 1.6 - 0.8 * t, 1.0, 0.8, |_| 1.0), mat, Vec3::ZERO, bone);
        }
    }
    let tail = [Vec2::new(-9.0, o - 0.5), Vec2::new(-11.0, o - 1.0), Vec2::new(-12.5, o - 2.0)];
    b.part(stroke(&smooth_open(&tail, 3), |t| 1.2 - 0.8 * t, 1.0, 0.8, |_| 1.0), &skin, Vec3::new(0.0, 0.0, -0.05), body);
    let hide = smooth_closed(
        &[
            Vec2::new(8.8, 0.4),
            Vec2::new(7.6, 3.0),
            Vec2::new(4.0, 4.3),
            Vec2::new(-2.0, 4.5),
            Vec2::new(-7.0, 3.8),
            Vec2::new(-9.6, 1.2),
            Vec2::new(-9.4, -1.6),
            Vec2::new(-6.0, -3.0),
            Vec2::new(0.0, -3.3),
            Vec2::new(5.5, -2.6),
            Vec2::new(8.2, -1.2),
        ],
        4,
    );
    b.part(fill(&hide, |p| 0.55 + 0.45 * ((p.y + 3.3) / 7.8).clamp(0.0, 1.0)), &skin, Vec3::new(0.0, o, 0.0), body);
    // Folds of loose hide, and a couple of sores.
    for x in [-6.5f32, -4.0, -1.5, 1.0, 3.5] {
        let crease = smooth_open(&[Vec2::new(x + 0.5, o + 4.0), Vec2::new(x - 0.4, o + 0.8), Vec2::new(x + 0.2, o - 2.6)], 3);
        b.part(stroke(&crease, |t| 0.45 * (PI * t).sin() + 0.1, 1.0, 1.0, |_| 1.0), &fold, Vec3::new(0.0, 0.0, 0.01), body);
    }
    for (x, y, r) in [(-4.8f32, 2.4, 0.8), (0.6, -1.4, 0.6)] {
        b.part(fill(&ellipse_pts(r, r * 0.7, 8), |_| 1.0), &fold, Vec3::new(x, o + y, 0.02), body);
    }
    // Blind slit of an eye, nostril.
    b.part(fill(&ellipse_pts(0.6, 0.25, 6), |_| 1.0), &eye, Vec3::new(5.6, o + 1.8, 0.02), body);
    b.part(fill(&ellipse_pts(0.35, 0.3, 6), |_| 1.0), &eye, Vec3::new(8.4, o + 0.4, 0.02), body);
    // The incisors: two long upper, two lower, sticking out of the lips.
    let teeth = b.bone(Vec3::new(7.8, o - 0.6, 0.05), body, BoneKind::Subway(Bone::Teeth));
    let upper = smooth_open(&[Vec2::new(0.0, 0.5), Vec2::new(2.4, 0.0), Vec2::new(3.4, -1.9)], 3);
    let lower = smooth_open(&[Vec2::new(0.0, -0.9), Vec2::new(2.0, -0.7), Vec2::new(3.0, 0.9)], 3);
    b.part(stroke(&upper, |t| 1.1 - 0.6 * t, 1.0, 0.7, |_| 1.0), &tooth, Vec3::ZERO, teeth);
    b.part(stroke(&lower, |t| 0.9 - 0.5 * t, 1.0, 0.7, |_| 1.0), &tooth, Vec3::new(0.0, 0.0, -0.01), teeth);

    // The heap of gravel over it while it tunnels.
    let mound = b.bone(Vec3::new(0.0, -o, 0.4), rig, BoneKind::Subway(Bone::Mound));
    let heap: Vec<Vec2> = (0..=12).map(|i| {
        let x = -11.0 + i as f32 * (22.0 / 12.0);
        Vec2::new(x, 4.5 * (1.0 - (x / 11.0).powi(2)).max(0.0))
    }).collect();
    b.part(fill(&heap, |p| 0.5 + 0.5 * (p.y / 4.5)), &gravel_dark, Vec3::ZERO, mound);
    let mut rng = Rng(0x6a11a5);
    for i in 0..10 {
        let x = rng.range(-9.0, 9.0);
        let top = 4.5 * (1.0 - (x / 11.0).powi(2));
        let y = rng.range(0.6, top.max(1.0));
        let r = rng.range(0.9, 1.9);
        let mat = if i % 3 == 0 { &gravel_dark } else { &gravel };
        b.part(fill(&rock_pts(&mut rng, r, 6), |p| 0.7 + 0.3 * (p.y / r)), mat, Vec3::new(x, y, 0.01 * i as f32), mound);
    }
    vec![(skin, MOLE_SKIN), (fold, MOLE_FOLD), (tooth, TOOTH)]
}

/// A plague tick, facing +X, feet at y ≈ -3.6.
pub(super) fn tick(b: &mut Builder, materials: &mut Assets<ColorMaterial>, rig: Entity) -> Mats {
    let shell = materials.add(TICK_SHELL);
    let sac_mat = materials.add(TICK_SAC);
    let dark = materials.add(TICK_DARK);
    let shine = materials.add(Color::WHITE);
    let body = b.bone(Vec3::ZERO, rig, BoneKind::Subway(Bone::TickBody));
    // Legs: four a side, jointed, knees up.
    for near in [false, true] {
        let z = if near { 0.3 } else { -0.2 };
        for i in 0..4 {
            let x = 2.4 - i as f32 * 1.3 + if near { 0.0 } else { 0.5 };
            let bone = b.bone(Vec3::new(x, -0.6, z), body, BoneKind::Subway(Bone::TickLeg(i, near)));
            let lean = 1.2 - i as f32 * 0.8;
            let path = smooth_open(&[Vec2::ZERO, Vec2::new(lean, 1.0), Vec2::new(lean * 1.8, -1.2), Vec2::new(lean * 2.0, -3.0)], 3);
            b.part(stroke(&path, |t| 0.75 - 0.4 * t, 1.0, 0.8, |_| 1.0), &dark, Vec3::ZERO, bone);
        }
    }
    // The sac: leathery, folded, swelling as it feeds.
    let sac = b.bone(SAC_AT.extend(0.1), body, BoneKind::Subway(Bone::Sac));
    b.part(fill(&ellipse_pts(4.6, 3.3, 18), |p| 0.55 + 0.45 * ((p.y + 3.3) / 6.6)), &sac_mat, Vec3::ZERO, sac);
    for x in [-2.0f32, 0.6] {
        let crease = smooth_open(&[Vec2::new(x, 2.6), Vec2::new(x - 0.6, 0.0), Vec2::new(x, -2.6)], 3);
        b.part(stroke(&crease, |t| 0.35 * (PI * t).sin() + 0.05, 1.0, 1.0, |_| 1.0), &dark, Vec3::new(0.0, 0.0, 0.01), sac);
    }
    b.part(fill(&ellipse_pts(1.6, 0.45, 8), |_| 1.0), &shine, Vec3::new(-1.0, 2.3, 0.02), sac);
    // The shield over its front, and the head: palps and barbed mouthparts.
    let scutum = smooth_closed(
        &[Vec2::new(-0.8, 2.7), Vec2::new(1.8, 3.0), Vec2::new(3.6, 1.6), Vec2::new(3.4, -0.4), Vec2::new(0.6, -0.4), Vec2::new(-1.0, 0.8)],
        4,
    );
    b.part(fill(&scutum, |p| 0.6 + 0.4 * ((p.y + 0.4) / 3.4)), &shell, Vec3::new(0.0, 0.0, 0.2), body);
    b.part(fill(&ellipse_pts(1.3, 0.9, 10), |_| 0.8), &shell, Vec3::new(4.2, 0.3, 0.21), body);
    let mouth = [Vec2::new(4.6, -0.1), Vec2::new(6.8, -0.8)];
    b.part(stroke(&mouth, |t| 0.8 - 0.55 * t, 1.0, 1.0, |_| 1.0), &dark, Vec3::new(0.0, 0.0, 0.22), body);
    let barbs: Vec<[Vec2; 3]> = [5.2f32, 5.9]
        .into_iter()
        .map(|x| [Vec2::new(x, -0.3), Vec2::new(x + 0.3, -0.4), Vec2::new(x - 0.3, -1.0)])
        .collect();
    b.part(triangles(&barbs), &dark, Vec3::new(0.0, 0.0, 0.23), body);
    vec![(shell, TICK_SHELL), (sac_mat, TICK_SAC), (dark, TICK_DARK)]
}

pub(super) fn animate(bone: Bone, m: &Mob, tf: &mut Transform, tt: f32) {
    let dead = match m.state {
        State::Dead { t, .. } => Some(t),
        _ => None,
    };
    let windup = m.windup();
    let twitch = |k: f32| dead.map_or(0.0, |td| (tt * 22.0 + k * 2.1).sin() * 0.3 * (1.0 - td / 4.0).max(0.0));
    match bone {
        Bone::Ball => tf.rotation = Quat::from_rotation_z(m.ball_spin() + windup * (tt * 45.0).sin() * 0.06),
        Bone::Knotted(i) => {
            let k = i as f32;
            let squirm = match m.state {
                State::Dead { .. } => twitch(k),
                // Heads thrashing up, squealing.
                State::Windup(_) => 0.3 * windup * (tt * 32.0 + k * 1.3).sin(),
                // Snapping at each other.
                State::Recover(_) => (tt * 12.0 + k * 1.9).sin() * 0.3,
                State::Attack(_) => (tt * 8.0 + k).sin() * 0.05,
                _ => (tt * 4.0 + k * 1.7).sin() * 0.15 + (tt * 9.3 + k).sin() * 0.05,
            };
            tf.rotation = Quat::from_rotation_z(KNOTTED[i] + squirm);
            // Straining away from the knot when it squeals.
            tf.scale.x = 1.0 + 0.1 * windup + if matches!(m.state, State::Recover(_)) { (tt * 12.0 + k).sin().max(0.0) * 0.06 } else { 0.0 };
        }
        Bone::Body => {
            let out = m.mole_out();
            tf.scale = Vec3::new(1.0 + (1.0 - out) * 0.2, out.max(0.02), 1.0);
            // Shaking the dirt off, dazed.
            let shake = if matches!(m.state, State::Recover(_)) { (tt * 30.0).sin() * 0.04 } else { 0.0 };
            tf.rotation = Quat::from_rotation_z(shake);
        }
        Bone::Mound => {
            let base = (1.0 - m.mole_out() * 1.2).clamp(0.0, 1.0);
            // Heaving and shaking before it comes up; crawling along otherwise.
            let heave = 1.0 + 0.5 * windup;
            let crawl = (m.stride * 2.0).sin() * 0.06 * (m.vel.x.abs() / 60.0).min(1.0);
            tf.scale = Vec3::new(base * (1.0 + crawl), (base * heave * (1.0 - crawl)).max(0.001), 1.0);
            tf.rotation = Quat::from_rotation_z(windup * (tt * 50.0).sin() * 0.1);
        }
        Bone::Teeth => {
            let gnash = match m.state {
                State::Attack(_) => 0.35,
                State::Dead { .. } => 0.25,
                State::Recover(_) => (tt * 18.0).sin() * 0.12,
                _ => (tt * 3.0).sin() * 0.05,
            };
            tf.rotation = Quat::from_rotation_z(-gnash);
        }
        Bone::TickBody => {
            let angle = if m.tick_flipped() {
                PI + (tt * 9.0).sin() * 0.12
            } else if m.tick_latched() {
                // Mouthparts dug in, pumping.
                m.tick_tilt() + (tt * 5.0).sin() * 0.05
            } else {
                0.0
            };
            tf.rotation = Quat::from_rotation_z(angle);
        }
        Bone::Sac => {
            let g = m.tick_gorge();
            let pump = if m.tick_latched() { (tt * 6.0).sin() * 0.04 } else { 0.0 };
            tf.scale = Vec3::new(1.0 + 0.7 * g + pump, 1.0 + 0.9 * g + pump, 1.0);
            // Swelling up off the ground, not into it.
            tf.translation.y = SAC_AT.y + g * 2.6;
            tf.translation.x = SAC_AT.x - g * 1.5;
        }
        Bone::TickLeg(i, near) => {
            let k = i as f32 + if near { 0.0 } else { 0.5 };
            let angle = if dead.is_some() || m.tick_flipped() {
                // Kicking on its back.
                (tt * 26.0 + k * 1.7).sin() * 0.5 * dead.map_or(1.0, |td| (1.0 - td / 4.0).max(0.1))
            } else if m.tick_latched() {
                // Clutching the hide.
                -0.5 + (tt * 3.0 + k).sin() * 0.05
            } else if m.airborne {
                0.5
            } else if i == 0 {
                // Questing: the front legs raised, waving (frantically before it leaps).
                0.9 + (tt * (2.5 + 20.0 * windup) + k).sin() * (0.25 + 0.15 * windup)
            } else {
                let group = if (i % 2 == 0) == near { 0.0 } else { PI };
                (m.stride * 3.0 + group).sin() * (m.vel.x.abs() / 30.0).min(1.0) * 0.4
            };
            tf.rotation = Quat::from_rotation_z(angle);
        }
    }
}

/// What a fresh corpse loses.
pub(super) fn tear(bone: Bone, m: &Mob) -> Tear {
    let f = m.facing;
    let blood = Some(m.spec.blood);
    match bone {
        // These two got out alive (`mobs::subway::split`); one is torn off dead.
        Bone::Knotted(0) | Bone::Knotted(2) => Tear::Gone,
        Bone::Knotted(1) => Tear::Off { vel: Vec2::new(-f * 180.0, 420.0), spin: 9.0, bleeds: blood },
        Bone::Teeth => Tear::Off { vel: Vec2::new(f * 200.0, 380.0), spin: 14.0, bleeds: None },
        // The sac bursts off the shield.
        Bone::Sac => Tear::Off { vel: Vec2::new(-f * 120.0, 300.0), spin: -6.0, bleeds: blood },
        Bone::TickLeg(0, true) => Tear::Off { vel: Vec2::new(f * 150.0, 380.0), spin: -10.0, bleeds: None },
        _ => Tear::Stay,
    }
}
