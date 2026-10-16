//! Bodies of the catacombs' kin (`mobs/catacombs.rs`): the crypt jumper
//! (boxy, huge front eyes, raises its front legs), the grave trapdoor (glossy,
//! big fangs, under a lid in the floor ringed with tripwires), the ossuary
//! wolf (banded legs, brood clinging to its abdomen) and the bonestrider (a
//! small bone-pale body on eight long stilts, legs solved hip → knee → foot
//! every frame from where its feet stand).

use std::f32::consts::PI;

use bevy::{prelude::*, sprite_render::AlphaMode2d};

use super::{BoneKind, Builder, EYE, SILK};
use crate::{
    gore::Fluid,
    mobs::{
        catacombs::{hip, LEGS, SHIN, STILT, THIGH},
        Mob, State,
    },
    shapes::*,
};

const JUMPER_BODY: Color = Color::srgb(0.14, 0.12, 0.11);
const JUMPER_FUR: Color = Color::srgb(0.46, 0.43, 0.38);
const TRAPDOOR_BODY: Color = Color::srgb(0.09, 0.08, 0.08);
const TRAPDOOR_SHINE: Color = Color::srgb(0.42, 0.40, 0.38);
const LID: Color = Color::srgb(0.36, 0.33, 0.28);
const HOLE: Color = Color::srgb(0.03, 0.03, 0.03);
const CHIP: Color = Color::srgb(0.62, 0.58, 0.50);
const WOLF_BODY: Color = Color::srgb(0.28, 0.24, 0.20);
const WOLF_DARK: Color = Color::srgb(0.12, 0.10, 0.09);
const WOLF_PALE: Color = Color::srgb(0.58, 0.54, 0.46);
const BROODLING: Color = Color::srgb(0.50, 0.47, 0.42);
const STRIDER_BODY: Color = Color::srgb(0.60, 0.56, 0.48);
const STRIDER_SADDLE: Color = Color::srgb(0.30, 0.27, 0.23);
const STRIDER_NEAR: Color = Color::srgb(0.44, 0.41, 0.36);
const STRIDER_FAR: Color = Color::srgb(0.26, 0.24, 0.21);
/// Feet below the center, in drawing units: jumper, trapdoor, wolf.
const JUMPER_FOOT: f32 = 6.0;
const TRAPDOOR_FOOT: f32 = 5.5;
/// The trapdoor's hole and lid (world units: they hang off the root).
const HOLE_HALF: f32 = 20.0;

#[derive(Clone, Copy)]
pub(super) enum Bone {
    /// Jumper: a front leg, raised to strike in the windup.
    Raise { near: bool },
    /// Jumper: its safety line back to where it pounced from (unit strip, scaled).
    Dragline,
    /// Trapdoor: the hole in the floor (stays over its home) and its hinged lid.
    Hole,
    Lid,
    /// Trapdoor: the whole spider, squashed into the hole while it hides (pivot at its feet).
    Emerge,
    /// Wolf: its abdomen, and a broodling clinging to it.
    Belly,
    Brood(usize),
    /// Strider: a leg's upper and lower half (unit strips stretched hip → knee → foot).
    Thigh(usize),
    Shin(usize),
}

type Mats = Vec<(Handle<ColorMaterial>, Color)>;

fn bone(b: &mut Builder, pos: Vec3, parent: Entity, kind: Bone) -> Entity {
    b.bone(pos, parent, BoneKind::Crypt(kind))
}

/// A plain carrier entity (no animation) under `parent`.
fn carrier(b: &mut Builder, pos: Vec3, parent: Entity) -> Entity {
    b.commands.spawn((Transform::from_translation(pos), Visibility::default(), ChildOf(parent))).id()
}

/// A unit-long strip along +Y, `w0` wide at its base to `w1` at its tip.
fn strip(w0: f32, w1: f32) -> Mesh {
    stroke(&[Vec2::ZERO, Vec2::Y], move |t| w0 + (w1 - w0) * t, 1.0, 0.75, |_| 1.0)
}

/// Bands along a leg.
fn banded(n: f32) -> impl Fn(f32) -> f32 {
    move |t| if (t * n).fract() < 0.5 { 1.0 } else { 0.6 }
}

fn silk(materials: &mut Assets<ColorMaterial>) -> Handle<ColorMaterial> {
    materials.add(ColorMaterial { color: SILK, alpha_mode: AlphaMode2d::Blend, ..default() })
}

/// Spider legs grouped on the three walking bones a side (`Leg { pair }`):
/// (pair, hip x, reach); `foot` = how far below the center the feet are.
fn walking_legs(b: &mut Builder, rig: Entity, legs: &[(usize, f32, f32)], foot: f32, w: f32, near_mat: &Handle<ColorMaterial>, far_mat: &Handle<ColorMaterial>, bands: f32) {
    for near in [false, true] {
        let z = if near { 0.05 } else { -0.2 };
        let mat = if near { near_mat } else { far_mat };
        let bones: Vec<Entity> = (0..3).map(|pair| b.bone(Vec3::new(0.0, 0.0, z), rig, BoneKind::Leg { pair, near })).collect();
        for &(pair, x, reach) in legs {
            let x = x + if near { 0.0 } else { 0.8 };
            let path = smooth_open(&[Vec2::new(x, 0.5), Vec2::new(x + reach * 0.45, foot * 0.7), Vec2::new(x + reach, -foot)], 4);
            b.part(stroke(&path, |t| w - 0.45 * w * t, 1.0, 0.8, banded(bands)), mat, Vec3::ZERO, bones[pair]);
        }
    }
}

/// A jumping spider, side view, facing +X, feet at y ≈ -6: boxy front body,
/// one huge glossy front eye, pale palps, stout raised front legs.
pub(super) fn jumper(b: &mut Builder, materials: &mut Assets<ColorMaterial>, rig: Entity, root: Entity) -> Mats {
    let body = materials.add(JUMPER_BODY);
    let fur = materials.add(JUMPER_FUR);
    let eye = materials.add(EYE);
    let glint = materials.add(Color::WHITE);
    let line = silk(materials);
    let drag = bone(b, Vec3::new(0.0, 0.0, -0.5), root, Bone::Dragline);
    b.part(strip(0.8, 0.8), &line, Vec3::ZERO, drag);

    walking_legs(b, rig, &[(1, 0.8, 3.0), (1, -0.2, 0.0), (2, -1.2, -4.5)], JUMPER_FOOT, 1.1, &body, &body, 3.0);
    // The front legs: thick, fringed, raised for the strike.
    for near in [false, true] {
        let z = if near { 0.2 } else { -0.25 };
        let raise = bone(b, Vec3::new(3.0 + if near { 0.0 } else { 0.7 }, 0.5, z), rig, Bone::Raise { near });
        let path = smooth_open(&[Vec2::ZERO, Vec2::new(3.5, 3.0), Vec2::new(6.0, -1.0), Vec2::new(7.0, -6.5)], 4);
        b.part(stroke(&path, |t| 1.8 - 0.9 * t, 1.0, 0.8, banded(4.0)), if near { &fur } else { &body }, Vec3::ZERO, raise);
    }
    // Abdomen: fuzzy, two pale chevrons.
    b.part(fill(&ellipse_pts(4.6, 3.8, 18), |p| 0.6 + 0.4 * ((p.y + 3.8) / 7.6)), &body, Vec3::new(-5.0, 1.8, 0.1), rig);
    let chevrons: Vec<[Vec2; 3]> = [-3.5f32, -6.0]
        .into_iter()
        .map(|x| [Vec2::new(x + 1.2, 4.6), Vec2::new(x - 1.2, 4.4), Vec2::new(x, 3.0)])
        .collect();
    b.part(triangles(&chevrons), &fur, Vec3::new(0.0, 0.0, 0.12), rig);
    // Front body: tall and square, the big eye on its face.
    let head = smooth_closed(
        &[Vec2::new(-1.5, -0.5), Vec2::new(-1.0, 3.0), Vec2::new(2.5, 4.6), Vec2::new(5.6, 4.0), Vec2::new(6.4, 1.0), Vec2::new(5.5, -1.2), Vec2::new(1.0, -1.6)],
        4,
    );
    b.part(fill(&head, |p| 0.6 + 0.4 * ((p.y + 1.6) / 6.2)), &body, Vec3::new(0.0, 0.0, 0.15), rig);
    let tufts: Vec<[Vec2; 3]> = (0..3)
        .map(|i| {
            let x = 4.0 - i as f32 * 2.0;
            [Vec2::new(x + 0.8, 4.2), Vec2::new(x - 0.8, 4.2), Vec2::new(x - 0.4, 5.4)]
        })
        .collect();
    b.part(triangles(&tufts), &body, Vec3::new(0.0, 0.0, 0.14), rig);
    b.part(fill(&ellipse_pts(1.7, 1.8, 14), |p| 0.7 + 0.3 * (p.y / 1.8)), &eye, Vec3::new(5.3, 1.6, 0.17), rig);
    b.part(fill(&ellipse_pts(0.5, 0.55, 6), |_| 1.0), &glint, Vec3::new(5.9, 2.4, 0.18), rig);
    b.part(fill(&ellipse_pts(0.6, 0.6, 8), |_| 1.0), &eye, Vec3::new(2.4, 4.2, 0.17), rig);
    let palp = smooth_open(&[Vec2::new(5.4, -1.0), Vec2::new(6.8, -1.6), Vec2::new(7.2, -0.6)], 3);
    b.part(stroke(&palp, |t| 1.2 - 0.4 * t, 1.0, 1.0, |_| 1.0), &fur, Vec3::new(0.0, 0.0, 0.19), rig);
    vec![(body, JUMPER_BODY), (fur, JUMPER_FUR)]
}

/// A trapdoor spider, facing +X, feet at y ≈ -5.5: glossy domed body, short
/// thick legs, big down-curved fangs; its hole and lid stay in the floor.
pub(super) fn trapdoor(b: &mut Builder, materials: &mut Assets<ColorMaterial>, rig: Entity, root: Entity) -> Mats {
    let body = materials.add(TRAPDOOR_BODY);
    let shine = materials.add(TRAPDOOR_SHINE);
    let lid = materials.add(LID);
    let hole_mat = materials.add(HOLE);
    let chip = materials.add(CHIP);
    let eye = materials.add(Color::WHITE);
    let line = silk(materials);

    // The hole, its tripwires spread over the floor, the lid on a hinge.
    let hole = bone(b, Vec3::new(0.0, 0.0, -0.6), root, Bone::Hole);
    b.part(fill(&ellipse_pts(HOLE_HALF, 3.5, 20), |_| 1.0), &hole_mat, Vec3::ZERO, hole);
    let wires: Vec<[Vec2; 3]> = [(-1.0, 52.0, 0.8), (-1.0, 38.0, -0.6), (-1.0, 61.0, 0.0), (1.0, 47.0, 0.6), (1.0, 58.0, -0.4), (1.0, 34.0, 0.0)]
        .into_iter()
        .flat_map(|(s, len, dy): (f32, f32, f32)| {
            let (a, c) = (Vec2::new(s * (HOLE_HALF - 2.0), 1.0), Vec2::new(s * len, 0.5 + dy));
            let n = Vec2::new(0.0, 0.35);
            [[a - n, a + n, c + n], [a - n, c + n, c - n]]
        })
        .collect();
    b.part(triangles(&wires), &line, Vec3::new(0.0, 0.0, 0.01), hole);
    let hinge = bone(b, Vec3::new(-HOLE_HALF, 0.5, 0.05), hole, Bone::Lid);
    let w = 2.0 * HOLE_HALF + 2.0;
    let disc = smooth_closed(
        &[Vec2::new(0.0, -1.4), Vec2::new(w * 0.5, -2.0), Vec2::new(w, -1.4), Vec2::new(w - 1.5, 1.6), Vec2::new(w * 0.5, 3.2), Vec2::new(1.5, 1.6)],
        4,
    );
    b.part(fill(&disc, |p| 0.6 + 0.4 * ((p.y + 2.0) / 5.2)), &lid, Vec3::ZERO, hinge);
    // Grit and bone chips on the lid (it is made of the floor).
    for (x, y, rx, ry) in [(8.0, 2.6, 2.4, 0.9), (17.0, 3.3, 1.6, 0.8), (27.0, 2.8, 3.0, 0.8), (35.0, 2.0, 1.3, 0.7)] {
        b.part(fill(&ellipse_pts(rx, ry, 8), |_| 0.9), &chip, Vec3::new(x, y, 0.01), hinge);
    }

    // The spider itself, rising out of the floor.
    let emerge = bone(b, Vec3::new(0.0, -TRAPDOOR_FOOT, 0.0), rig, Bone::Emerge);
    let s = carrier(b, Vec3::new(0.0, TRAPDOOR_FOOT, 0.0), emerge);
    walking_legs(b, s, &[(0, 3.0, 5.0), (0, 1.8, 3.5), (1, 0.2, -1.5), (2, -1.2, -5.0)], TRAPDOOR_FOOT, 1.9, &body, &body, 2.0);
    b.part(fill(&ellipse_pts(5.2, 4.4, 18), |p| 0.55 + 0.45 * ((p.y + 4.4) / 8.8)), &body, Vec3::new(-5.0, 1.2, 0.1), s);
    let dome = smooth_closed(&[Vec2::new(-1.5, -1.0), Vec2::new(-0.5, 2.6), Vec2::new(3.0, 3.8), Vec2::new(6.0, 2.4), Vec2::new(6.5, -0.5), Vec2::new(3.0, -1.8)], 4);
    b.part(fill(&dome, |p| 0.55 + 0.45 * ((p.y + 1.8) / 5.6)), &body, Vec3::new(0.0, 0.0, 0.15), s);
    let gloss = smooth_open(&[Vec2::new(0.0, 2.6), Vec2::new(2.8, 3.4), Vec2::new(5.0, 2.4)], 4);
    b.part(stroke(&gloss, |t| 0.8 * (PI * t).sin(), 1.0, 1.0, |_| 1.0), &shine, Vec3::new(0.0, 0.0, 0.16), s);
    b.part(fill(&ellipse_pts(0.45, 0.45, 6), |_| 1.0), &eye, Vec3::new(3.6, 3.6, 0.17), s);
    b.part(fill(&ellipse_pts(0.35, 0.35, 6), |_| 1.0), &eye, Vec3::new(4.5, 3.3, 0.17), s);
    // Chelicerae and fangs, pointing down and forward.
    b.part(fill(&ellipse_pts(2.2, 1.8, 10), |_| 0.8), &body, Vec3::new(6.0, -0.4, 0.18), s);
    let fang = smooth_open(&[Vec2::new(7.0, -1.2), Vec2::new(8.4, -2.6), Vec2::new(7.6, -4.4)], 4);
    b.part(stroke(&fang, |t| 1.1 - 0.9 * t, 1.0, 1.0, |_| 1.0), &shine, Vec3::new(0.0, 0.0, 0.19), s);
    vec![(body, TRAPDOOR_BODY), (shine, TRAPDOOR_SHINE)]
}

/// A wolf spider, facing +X, feet at y ≈ -6: long banded legs, a pale stripe
/// down its back, big night eyes, broodlings clinging to its abdomen.
pub(super) fn wolf(b: &mut Builder, materials: &mut Assets<ColorMaterial>, rig: Entity) -> Mats {
    let body = materials.add(WOLF_BODY);
    let dark = materials.add(WOLF_DARK);
    let pale = materials.add(WOLF_PALE);
    let brood = materials.add(BROODLING);
    let eye = materials.add(EYE);
    let glint = materials.add(Color::WHITE);

    walking_legs(b, rig, &[(0, 2.8, 7.0), (0, 1.8, 5.0), (1, 0.4, -2.5), (2, -0.8, -6.5)], JUMPER_FOOT, 1.3, &body, &dark, 4.0);
    let belly = bone(b, Vec3::new(-3.0, 1.0, 0.1), rig, Bone::Belly);
    b.part(fill(&ellipse_pts(6.5, 5.0, 20), |p| 0.55 + 0.45 * ((p.y + 5.0) / 10.0)), &body, Vec3::new(-3.5, 0.5, 0.0), belly);
    let heart = smooth_open(&[Vec2::new(-0.5, 4.6), Vec2::new(-3.5, 5.2), Vec2::new(-7.0, 3.8)], 4);
    b.part(stroke(&heart, |t| 1.4 * (PI * t).sin() + 0.3, 1.0, 1.0, |_| 1.0), &dark, Vec3::new(0.0, 0.0, 0.01), belly);
    // The brood on its back: pale bumps with legs and a glint of an eye.
    for (i, (x, y)) in [(-6.5, 5.0), (-3.5, 5.9), (-0.8, 5.0)].into_iter().enumerate() {
        let ling = bone(b, Vec3::new(x, y, 0.03), belly, Bone::Brood(i));
        for dx in [-1.0f32, 1.0] {
            let leg = [Vec2::new(dx * 0.6, 0.0), Vec2::new(dx * 2.0, 1.0), Vec2::new(dx * 2.6, -0.8)];
            b.part(stroke(&leg, |_| 0.4, 1.0, 1.0, |_| 1.0), &dark, Vec3::new(0.0, 0.0, -0.01), ling);
        }
        b.part(fill(&ellipse_pts(1.5, 1.2, 10), |p| 0.6 + 0.4 * ((p.y + 1.2) / 2.4)), &brood, Vec3::ZERO, ling);
        b.part(fill(&ellipse_pts(0.3, 0.3, 6), |_| 1.0), &glint, Vec3::new(0.8, 0.3, 0.01), ling);
    }
    let head = smooth_closed(&[Vec2::new(-1.0, -1.0), Vec2::new(-0.5, 3.5), Vec2::new(3.5, 4.8), Vec2::new(6.5, 3.2), Vec2::new(7.0, 0.5), Vec2::new(5.0, -1.5)], 4);
    b.part(fill(&head, |p| 0.55 + 0.45 * ((p.y + 1.5) / 6.3)), &body, Vec3::new(0.0, 0.0, 0.15), rig);
    let stripe = smooth_open(&[Vec2::new(-0.5, 3.4), Vec2::new(2.5, 4.5), Vec2::new(5.5, 3.6)], 4);
    b.part(stroke(&stripe, |t| 1.3 * (PI * t).sin() + 0.2, 1.0, 1.0, |_| 1.0), &pale, Vec3::new(0.0, 0.0, 0.16), rig);
    b.part(fill(&ellipse_pts(1.0, 1.0, 10), |_| 1.0), &eye, Vec3::new(5.2, 3.6, 0.17), rig);
    b.part(fill(&ellipse_pts(0.35, 0.35, 6), |_| 1.0), &glint, Vec3::new(5.5, 4.0, 0.18), rig);
    b.part(fill(&ellipse_pts(0.5, 0.5, 6), |_| 1.0), &eye, Vec3::new(6.6, 1.8, 0.17), rig);
    let fang = smooth_open(&[Vec2::new(6.4, -0.6), Vec2::new(7.4, -1.8), Vec2::new(7.0, -3.0)], 4);
    b.part(stroke(&fang, |t| 1.2 - 0.9 * t, 1.0, 1.0, |_| 1.0), &dark, Vec3::new(0.0, 0.0, 0.18), rig);
    vec![(body, WOLF_BODY), (dark, WOLF_DARK), (pale, WOLF_PALE)]
}

/// A harvestman: a small bone-pale body (world units) with an eye turret,
/// and eight stilts hanging off the root (solved each frame in `animate`).
pub(super) fn strider(b: &mut Builder, materials: &mut Assets<ColorMaterial>, rig: Entity, root: Entity) -> Mats {
    let body = materials.add(STRIDER_BODY);
    let saddle = materials.add(STRIDER_SADDLE);
    let near_mat = materials.add(STRIDER_NEAR);
    let far_mat = materials.add(STRIDER_FAR);
    let eye = materials.add(EYE);
    for i in 0..LEGS {
        let near = i < 4;
        let (mat, z) = if near { (&near_mat, 0.3) } else { (&far_mat, -0.3) };
        let thigh = bone(b, Vec3::new(0.0, 0.0, z), root, Bone::Thigh(i));
        b.part(strip(1.8, 1.2), mat, Vec3::ZERO, thigh);
        let shin = bone(b, Vec3::new(0.0, 0.0, z + 0.01), root, Bone::Shin(i));
        b.part(strip(1.2, 0.6), mat, Vec3::ZERO, shin);
    }
    b.part(fill(&ellipse_pts(10.0, 7.0, 22), |p| 0.55 + 0.45 * ((p.y + 7.0) / 14.0)), &body, Vec3::ZERO, rig);
    b.part(fill(&ellipse_pts(6.0, 3.0, 16), |_| 1.0), &saddle, Vec3::new(-1.5, 2.2, 0.01), rig);
    for (x, y) in [(-6.0, 3.5), (-3.0, -2.5), (2.0, -3.5), (5.5, 2.0), (-7.5, -1.0)] {
        b.part(fill(&ellipse_pts(0.8, 0.8, 6), |_| 0.7), &saddle, Vec3::new(x, y, 0.02), rig);
    }
    // The eye turret, an eye each side.
    b.part(fill(&ellipse_pts(2.4, 2.6, 12), |p| 0.6 + 0.4 * ((p.y + 2.6) / 5.2)), &body, Vec3::new(2.0, 7.0, -0.01), rig);
    b.part(fill(&ellipse_pts(0.7, 0.8, 8), |_| 1.0), &eye, Vec3::new(3.0, 7.6, 0.02), rig);
    let jaw = smooth_open(&[Vec2::new(8.5, -2.0), Vec2::new(11.0, -3.5), Vec2::new(10.5, -6.0)], 4);
    b.part(stroke(&jaw, |t| 1.4 - 0.9 * t, 1.0, 1.0, |_| 1.0), &saddle, Vec3::new(0.0, 0.0, 0.03), rig);
    vec![(body, STRIDER_BODY), (near_mat, STRIDER_NEAR), (far_mat, STRIDER_FAR)]
}

/// The knee of a two-part leg from `hip` to `foot`, bent upward.
fn knee(hip: Vec2, foot: Vec2) -> Vec2 {
    let to = foot - hip;
    let d = to.length().clamp(SHIN - THIGH + 1.0, THIGH + SHIN - 0.5);
    let dir = to.normalize_or(Vec2::NEG_Y);
    let a = (THIGH * THIGH - SHIN * SHIN + d * d) / (2.0 * d);
    let h = (THIGH * THIGH - a * a).max(0.0).sqrt();
    let perp = dir.perp();
    let perp = if perp.y >= 0.0 { perp } else { -perp };
    hip + dir * a + perp * h
}

/// Lays a unit strip from `a` to `b` (`origin` = the root's position).
fn span(tf: &mut Transform, origin: Vec2, a: Vec2, b: Vec2) {
    let d = b - a;
    tf.translation = (a - origin).extend(tf.translation.z);
    tf.rotation = Quat::from_rotation_z(d.to_angle() - PI / 2.0);
    tf.scale = Vec3::new(1.0, d.length().max(0.01), 1.0);
}

pub(super) fn animate(bone: Bone, m: &Mob, root: &Transform, tf: &mut Transform, tt: f32, _dt: f32) {
    let pos = root.translation.truncate();
    let dead = match m.state {
        State::Dead { t, .. } => Some(t),
        _ => None,
    };
    let windup = m.windup();
    let shiver = windup * (tt * 60.0).sin() * 0.06;
    let c = &m.catacombs;
    match bone {
        Bone::Raise { near } => {
            let angle = match (dead, m.state) {
                (Some(td), _) => 0.3 + (tt * 30.0).sin() * 0.5 * (1.0 - td / 5.0).max(0.1),
                (_, State::Windup(_)) => 0.15 + 0.85 * windup + shiver,
                (_, State::Attack(_)) => 0.6,
                // Splayed after landing.
                (_, State::Recover(_)) => -0.2,
                _ => {
                    let amp = (m.vel.x.abs() / 150.0).min(1.0) * 0.5;
                    0.15 + (m.stride + if near { 0.0 } else { PI }).sin() * amp
                }
            };
            tf.rotation = Quat::from_rotation_z(angle);
        }
        Bone::Dragline => match c.anchor {
            Some(anchor) if dead.is_none() => span(tf, pos, pos, anchor),
            _ => tf.scale = Vec3::new(1.0, 0.001, 1.0),
        },
        Bone::Hole => {
            // Stays in the floor over its home, wherever the spider is.
            let floor = m.home - Vec2::Y * m.spec.foot;
            tf.translation = (floor - pos).extend(tf.translation.z);
        }
        Bone::Lid => {
            let tremble = windup * (tt * 45.0).sin() * 0.08;
            tf.rotation = Quat::from_rotation_z(c.out * 1.9 + tremble);
        }
        Bone::Emerge => {
            let out = if dead.is_some() { 1.0 } else { c.out };
            tf.scale = Vec3::new(0.6 + 0.4 * out, out.max(0.001), 1.0);
        }
        Bone::Belly => {
            let lift = if dead.is_some() { 0.0 } else { (tt * 3.0).sin() * 0.04 + 0.12 * windup };
            tf.rotation = Quat::from_rotation_z(lift);
        }
        Bone::Brood(i) => {
            let on = dead.is_none() && (i as u8) < c.brood;
            tf.scale = Vec3::splat(if on { 1.0 } else { 0.001 });
            tf.rotation = Quat::from_rotation_z((tt * 7.0 + i as f32 * 2.1).sin() * 0.15);
        }
        Bone::Thigh(i) | Bone::Shin(i) => {
            let hip = hip(pos, i);
            let foot = match dead {
                // Sprawled around the corpse, twitching.
                Some(td) => {
                    let twitch = (tt * 25.0 + i as f32 * 1.3).sin() * 4.0 * (1.0 - td / 4.0).max(0.0);
                    pos + Vec2::new(REACH_DEAD[i], -4.0 + twitch)
                }
                None if c.feet[i] == Vec2::ZERO => pos + Vec2::new(REACH_DEAD[i], -STILT),
                None => c.feet[i],
            };
            let knee = knee(hip, foot);
            match bone {
                Bone::Thigh(_) => span(tf, pos, hip, knee),
                _ => span(tf, pos, knee, foot),
            }
        }
    }
}

/// Where a strider's feet lie around its corpse (x from the body).
const REACH_DEAD: [f32; LEGS] = [-70.0, -42.0, 34.0, 66.0, -58.0, -28.0, 22.0, 52.0];

/// Parts torn off as it dies (or, for a strider, the leg it just shed):
/// (velocity, spin, what it bleeds).
pub(super) fn tear(bone: Bone, m: &Mob) -> Option<(Vec2, f32, Option<Fluid>)> {
    let f = m.facing;
    let dead = matches!(m.state, State::Dead { .. });
    let blood = Some(m.spec.blood);
    match bone {
        Bone::Raise { near: true } if dead => Some((Vec2::new(f * 160.0, 420.0), -9.0, None)),
        Bone::Belly if dead => Some((Vec2::new(-f * 120.0, 300.0), -5.0, blood)),
        Bone::Thigh(i) | Bone::Shin(i) if (!dead && m.catacombs.cut == Some(i)) || (dead && i % 3 == 0) => {
            let out = if i % 4 < 2 { -1.0 } else { 1.0 };
            let bleeds = if matches!(bone, Bone::Thigh(_)) { blood } else { None };
            Some((Vec2::new(out * 150.0, 380.0), out * 6.0, bleeds))
        }
        _ => None,
    }
}
