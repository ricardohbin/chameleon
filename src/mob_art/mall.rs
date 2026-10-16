//! Bodies of the mall's mobs (`mobs/mall.rs`): mantis nymphs and their
//! hatchlings, ghost mantises (dead-leaf disguise), neon moths, and the
//! oothecae (mantis egg cases) dangling on cables. Side views, facing +X.

use std::f32::consts::PI;

use bevy::{prelude::*, sprite_render::AlphaMode2d};

use super::{BoneKind, Builder, NEST_STALK};
use crate::{
    gore::Fluid,
    mobs::{Mob, MobKind, Moves, State},
    shapes::*,
};

/// Sickly pale green-gray, like something grown in the dark.
const NYMPH: Color = Color::srgb(0.56, 0.60, 0.46);
const HATCHLING: Color = Color::srgb(0.74, 0.76, 0.64);
/// Dead-leaf browns of the ghost mantis.
const LEAF: Color = Color::srgb(0.44, 0.38, 0.28);
const LEAF_DARK: Color = Color::srgb(0.25, 0.21, 0.15);
const MOTH: Color = Color::srgb(0.47, 0.43, 0.37);
const MOTH_DARK: Color = Color::srgb(0.21, 0.18, 0.15);
/// The pale owl-eye spots on a moth's wings.
const MOTH_SPOT: Color = Color::srgb(0.80, 0.76, 0.66);
/// Dried egg foam.
const FOAM: Color = Color::srgb(0.66, 0.60, 0.48);
const WIRE: Color = Color::srgb(0.07, 0.07, 0.07);
const EYE: Color = Color::srgb(0.08, 0.06, 0.05);
/// Shed hatchling skins hanging from an egg case.
const HUSK: Color = Color::srgba(0.80, 0.78, 0.66, 0.6);
/// Feet of the mantises, below the center (drawing units).
const FEET: f32 = 7.0;

/// The mall's own bones (`BoneKind::Mall`).
#[derive(Clone, Copy)]
pub(super) enum Bone {
    /// A mantis above its legs: sways, crouches (ghosts in disguise), lunges.
    Body,
    /// A mantis's triangular head: turns to the chameleon.
    Head,
    /// A raptorial foreleg's femur, from the shoulder (`near`: the visible side).
    Arm { near: bool },
    /// Its spined tibia, folding against the femur (carries the hook).
    Claw { near: bool },
    /// The ghost mantis's leaf wings: flat in disguise, flared to threaten.
    Leaf { front: bool },
    /// A moth's broad wings.
    MothWing { front: bool },
}

type Mats = Vec<(Handle<ColorMaterial>, Color)>;

/// Four long walking legs (mid and hind pairs as `Leg` 0 and 2), feet at y = -`FEET`.
fn walking_legs(b: &mut Builder, rig: Entity, near_mat: &Handle<ColorMaterial>, far_mat: &Handle<ColorMaterial>, leafy: bool) {
    let paths = [
        [Vec2::ZERO, Vec2::new(3.2, 3.0), Vec2::new(6.0, -FEET)],
        [Vec2::ZERO, Vec2::new(-3.8, 3.2), Vec2::new(-8.0, -FEET)],
    ];
    for near in [false, true] {
        let (mat, z, dx) = if near { (near_mat, 0.05, 0.0) } else { (far_mat, -0.3, 1.0) };
        for (i, pair) in [0, 2].into_iter().enumerate() {
            let hip = [1.0, -1.5][i];
            let bone = b.bone(Vec3::new(hip + dx, 0.0, z), rig, BoneKind::Leg { pair, near });
            b.part(stroke(&smooth_open(&paths[i], 4), |t| 1.0 - 0.45 * t, 1.0, 0.8, |_| 1.0), mat, Vec3::ZERO, bone);
            if leafy {
                // A dead-leaf flap on the femur.
                let k = paths[i][1];
                b.part(triangles(&[[k * 0.3, k, k * 0.6 + Vec2::new(0.0, 2.2)]]), mat, Vec3::new(0.0, 0.0, 0.01), bone);
            }
        }
    }
}

/// A raptorial foreleg on `body` at `shoulder`: femur (with spines) down from
/// the shoulder, tibia folded back up along it, a hook at its tip.
fn foreleg(b: &mut Builder, body: Entity, shoulder: Vec3, mat: &Handle<ColorMaterial>, spine: &Handle<ColorMaterial>, near: bool) {
    let arm = b.bone(shoulder, body, BoneKind::Mall(Bone::Arm { near }));
    let femur = smooth_open(&[Vec2::ZERO, Vec2::new(0.7, -3.5), Vec2::new(0.9, -7.0)], 4);
    b.part(stroke(&femur, |t| 2.7 - 0.9 * t, 1.0, 0.7, |_| 1.0), mat, Vec3::ZERO, arm);
    let spines: Vec<[Vec2; 3]> = [-2.2f32, -3.8, -5.4]
        .into_iter()
        .map(|y| [Vec2::new(1.3, y + 0.4), Vec2::new(1.4, y - 0.5), Vec2::new(2.6, y - 0.3)])
        .collect();
    b.part(triangles(&spines), spine, Vec3::new(0.0, 0.0, -0.01), arm);
    let claw = b.bone(Vec3::new(0.9, -7.0, 0.02), arm, BoneKind::Mall(Bone::Claw { near }));
    let tibia = smooth_open(&[Vec2::ZERO, Vec2::new(0.6, 3.0), Vec2::new(0.9, 5.6)], 4);
    b.part(stroke(&tibia, |t| 1.7 - 0.8 * t, 1.0, 0.8, |_| 1.0), mat, Vec3::ZERO, claw);
    b.part(triangles(&[[Vec2::new(0.4, 5.4), Vec2::new(1.2, 5.8), Vec2::new(2.4, 4.4)]]), spine, Vec3::new(0.0, 0.0, 0.01), claw);
}

/// A mantis nymph (or a hatchling, smaller and paler), feet at y = -7: four
/// long legs, the segmented abdomen curled up over its back, the long
/// prothorax up to a triangular head, forelegs folded in prayer.
pub(super) fn mantis(b: &mut Builder, materials: &mut Assets<ColorMaterial>, rig: Entity, hatchling: bool) -> Mats {
    let base = if hatchling { HATCHLING } else { NYMPH };
    let skin = materials.add(base);
    let dark = materials.add(scale_color(base, 0.55));
    let pale = materials.add(scale_color(base, 1.3));
    let eye = materials.add(EYE);
    walking_legs(b, rig, &skin, &dark, false);
    let body = b.bone(Vec3::ZERO, rig, BoneKind::Mall(Bone::Body));
    foreleg(b, body, Vec3::new(4.3, 5.0, -0.25), &dark, &dark, false);

    // Abdomen: banded segments, curled up over the back.
    let abdomen = b.bone(Vec3::new(-2.5, 0.6, 0.0), body, BoneKind::Abdomen);
    let curl = smooth_open(&[Vec2::ZERO, Vec2::new(-5.0, 0.4), Vec2::new(-9.0, 3.0), Vec2::new(-10.5, 7.0), Vec2::new(-9.2, 9.6)], 6);
    let bands = |t: f32| if (t * 7.0).fract() < 0.2 { 0.72 } else { 1.0 };
    b.part(stroke(&curl, |t| 5.4 - 3.4 * t, 1.0, 0.6, bands), &skin, Vec3::ZERO, abdomen);
    // Thorax and the long neck (prothorax) up to the head.
    b.part(fill(&ellipse_pts(3.6, 2.3, 16), |p| 0.6 + 0.4 * ((p.y + 2.3) / 4.6)), &skin, Vec3::new(-0.3, 0.4, 0.05), body);
    let neck = smooth_open(&[Vec2::new(1.5, 0.8), Vec2::new(3.8, 4.0), Vec2::new(5.6, 8.0)], 5);
    b.part(stroke(&neck, |t| 2.6 - 0.8 * t, 1.0, 0.65, |_| 1.0), &skin, Vec3::new(0.0, 0.0, 0.1), body);
    if !hatchling {
        // Wing buds on the back.
        let bud = smooth_closed(&[Vec2::new(0.5, 1.6), Vec2::new(-2.5, 2.8), Vec2::new(-4.5, 1.8), Vec2::new(-1.5, 1.0)], 3);
        b.part(fill(&bud, |_| 0.85), &dark, Vec3::new(0.0, 0.0, 0.08), body);
    }
    mantis_head(b, body, Vec3::new(6.0, 8.8, 0.2), &skin, &pale, &eye, &dark, false);
    foreleg(b, body, Vec3::new(4.0, 5.2, 0.25), &skin, &dark, true);
    vec![(skin, base), (dark, scale_color(base, 0.55)), (pale, scale_color(base, 1.3))]
}

/// The head: a rounded triangle, a huge pale eye with a black pseudopupil,
/// whip antennae (`crest`: the ghost mantis's tall leaf crown).
#[allow(clippy::too_many_arguments)]
fn mantis_head(
    b: &mut Builder,
    body: Entity,
    at: Vec3,
    skin: &Handle<ColorMaterial>,
    pale: &Handle<ColorMaterial>,
    eye: &Handle<ColorMaterial>,
    dark: &Handle<ColorMaterial>,
    crest: bool,
) {
    let head = b.bone(at, body, BoneKind::Mall(Bone::Head));
    let shape = smooth_closed(
        &[Vec2::new(-1.6, 1.0), Vec2::new(0.2, 2.6), Vec2::new(2.4, 2.1), Vec2::new(3.1, 0.2), Vec2::new(1.6, -2.4), Vec2::new(0.4, -2.9), Vec2::new(-1.2, -1.0)],
        4,
    );
    b.part(fill(&shape, |p| 0.65 + 0.35 * ((p.y + 2.9) / 5.5)), skin, Vec3::ZERO, head);
    b.part(fill(&ellipse_pts(1.4, 1.7, 12), |p| 0.8 + 0.2 * (p.y / 1.7)), pale, Vec3::new(1.4, 1.0, 0.02), head);
    b.part(fill(&ellipse_pts(0.45, 0.55, 8), |_| 1.0), eye, Vec3::new(1.9, 1.0, 0.03), head);
    b.part(triangles(&[[Vec2::new(0.6, -2.4), Vec2::new(1.8, -2.2), Vec2::new(1.0, -3.8)]]), dark, Vec3::new(0.0, 0.0, 0.01), head);
    if crest {
        b.part(triangles(&[[Vec2::new(-1.0, 1.4), Vec2::new(0.9, 2.3), Vec2::new(-3.6, 9.0)]]), skin, Vec3::new(0.0, 0.0, -0.01), head);
        let vein = [Vec2::new(-0.2, 1.9), Vec2::new(-3.2, 8.4)];
        b.part(stroke(&vein, |_| 0.3, 1.0, 1.0, |_| 1.0), dark, Vec3::ZERO, head);
    }
    for (offset, z) in [(0.0, -0.05), (1.1, 0.04)] {
        let bone = b.bone(Vec3::new(1.0, 2.2, z), head, BoneKind::Antenna { offset });
        let path = smooth_open(&[Vec2::ZERO, Vec2::new(1.5, 4.0), Vec2::new(4.5, 8.5), Vec2::new(9.0, 11.0)], 6);
        b.part(stroke(&path, |t| 0.5 - 0.3 * t, 1.0, 1.0, |_| 1.0), dark, Vec3::ZERO, bone);
    }
}

/// A ghost mantis, feet at y = -7: leafy and flat, a dead leaf with legs.
/// Leaf wings over a lobed abdomen, a leaf-flap collar on the neck, a crown on the head.
pub(super) fn ghost(b: &mut Builder, materials: &mut Assets<ColorMaterial>, rig: Entity) -> Mats {
    let leaf = materials.add(LEAF);
    let dark = materials.add(LEAF_DARK);
    let pale = materials.add(scale_color(LEAF, 1.35));
    let eye = materials.add(EYE);
    walking_legs(b, rig, &leaf, &dark, true);
    let body = b.bone(Vec3::ZERO, rig, BoneKind::Mall(Bone::Body));
    foreleg(b, body, Vec3::new(4.3, 5.0, -0.25), &dark, &dark, false);

    // Abdomen: flat and straight back, ragged lobes like a chewed leaf.
    let abdomen = b.bone(Vec3::new(-2.5, 0.6, 0.0), body, BoneKind::Abdomen);
    let flat = smooth_open(&[Vec2::ZERO, Vec2::new(-6.0, 0.6), Vec2::new(-11.5, 1.8)], 5);
    b.part(stroke(&flat, |t| 5.0 - 2.6 * t, 1.0, 0.6, |_| 1.0), &leaf, Vec3::ZERO, abdomen);
    let lobes: Vec<[Vec2; 3]> = [-3.0f32, -6.5, -9.5]
        .into_iter()
        .map(|x| [Vec2::new(x + 1.2, -1.5), Vec2::new(x - 1.2, -1.2), Vec2::new(x - 0.4, -4.2)])
        .collect();
    b.part(triangles(&lobes), &dark, Vec3::new(0.0, 0.0, -0.01), abdomen);
    b.part(fill(&ellipse_pts(3.6, 2.3, 16), |p| 0.6 + 0.4 * ((p.y + 2.3) / 4.6)), &leaf, Vec3::new(-0.3, 0.4, 0.05), body);
    let neck = smooth_open(&[Vec2::new(1.5, 0.8), Vec2::new(3.8, 4.0), Vec2::new(5.6, 8.0)], 5);
    b.part(stroke(&neck, |t| 2.4 - 0.6 * t, 1.0, 0.65, |_| 1.0), &leaf, Vec3::new(0.0, 0.0, 0.1), body);
    let collar = smooth_closed(&[Vec2::new(2.0, 2.0), Vec2::new(5.5, 4.0), Vec2::new(6.0, 6.8), Vec2::new(3.6, 5.6), Vec2::new(1.2, 3.2)], 3);
    b.part(fill(&collar, |p| 0.7 + 0.3 * (p.y / 7.0)), &leaf, Vec3::new(0.0, 0.0, 0.11), body);
    // Leaf wings: a back one and a front one, veined.
    for front in [false, true] {
        let (mat, z) = if front { (&leaf, 0.15) } else { (&dark, -0.1) };
        let wing = b.bone(Vec3::new(0.8, 2.4, z), body, BoneKind::Mall(Bone::Leaf { front }));
        let shape = smooth_closed(
            &[Vec2::ZERO, Vec2::new(-4.0, 2.4), Vec2::new(-10.5, 2.4), Vec2::new(-15.5, -0.2), Vec2::new(-10.0, -1.8), Vec2::new(-3.0, -1.1)],
            4,
        );
        b.part(fill(&shape, |p| 0.65 + 0.35 * ((p.y + 1.8) / 4.2)), mat, Vec3::ZERO, wing);
        let vein = smooth_open(&[Vec2::new(-0.5, 0.2), Vec2::new(-7.0, 0.5), Vec2::new(-14.5, -0.2)], 4);
        b.part(stroke(&vein, |t| 0.5 - 0.3 * t, 1.0, 1.0, |_| 1.0), &dark, Vec3::new(0.0, 0.0, 0.01), wing);
        for x in [-4.0f32, -7.5, -11.0] {
            let rib = [Vec2::new(x, 0.4), Vec2::new(x - 2.0, 2.0)];
            b.part(stroke(&rib, |_| 0.25, 1.0, 1.0, |_| 1.0), &dark, Vec3::new(0.0, 0.0, 0.01), wing);
        }
    }
    mantis_head(b, body, Vec3::new(6.0, 8.8, 0.2), &leaf, &pale, &eye, &dark, true);
    foreleg(b, body, Vec3::new(4.0, 5.2, 0.25), &leaf, &dark, true);
    vec![(leaf, LEAF), (dark, LEAF_DARK), (pale, scale_color(LEAF, 1.35))]
}

/// A neon moth, facing +X: fat furry body, feathered antennae, broad wings
/// with pale owl-eye spots.
pub(super) fn moth(b: &mut Builder, materials: &mut Assets<ColorMaterial>, rig: Entity) -> Mats {
    let fur = materials.add(MOTH);
    let dark = materials.add(MOTH_DARK);
    let spot = materials.add(MOTH_SPOT);
    let eye = materials.add(EYE);
    let wing_mesh = || {
        let shape = smooth_closed(
            &[Vec2::ZERO, Vec2::new(1.8, 6.0), Vec2::new(-1.5, 11.0), Vec2::new(-8.5, 11.0), Vec2::new(-12.5, 5.5), Vec2::new(-6.0, 1.0)],
            4,
        );
        fill(&shape, |p| 0.55 + 0.45 * (p.y / 11.0).clamp(0.0, 1.0))
    };
    for front in [false, true] {
        let (mat, z) = if front { (&fur, 0.4) } else { (&dark, -0.3) };
        let wing = b.bone(Vec3::new(-0.5, 1.8, z), rig, BoneKind::Mall(Bone::MothWing { front }));
        b.part(wing_mesh(), mat, Vec3::ZERO, wing);
        if front {
            // The owl eye, and a ragged dark margin.
            b.part(fill(&ellipse_pts(2.4, 2.1, 14), |_| 1.0), &spot, Vec3::new(-5.5, 6.5, 0.01), wing);
            b.part(fill(&ellipse_pts(1.1, 1.2, 10), |_| 1.0), &eye, Vec3::new(-5.2, 6.4, 0.02), wing);
            let edge = smooth_open(&[Vec2::new(-1.5, 10.6), Vec2::new(-8.5, 10.6), Vec2::new(-12.0, 5.6)], 4);
            b.part(stroke(&edge, |t| 0.6 + 0.6 * (t * 9.0).sin().abs(), 1.0, 1.0, |_| 1.0), &dark, Vec3::new(0.0, 0.0, 0.01), wing);
        }
    }
    for (pair, x) in [(0, 1.8), (2, -0.8)] {
        let bone = b.bone(Vec3::new(x, -2.0, -0.2), rig, BoneKind::Leg { pair, near: true });
        let path = smooth_open(&[Vec2::ZERO, Vec2::new(0.8, -2.0), Vec2::new(0.2, -3.8)], 3);
        b.part(stroke(&path, |t| 0.7 - 0.3 * t, 1.0, 1.0, |_| 1.0), &dark, Vec3::ZERO, bone);
    }
    let abdomen = b.bone(Vec3::new(-2.0, -0.3, 0.0), rig, BoneKind::Abdomen);
    let bands = |p: Vec2| (0.6 + 0.4 * ((p.y + 2.6) / 5.2)) * if ((p.x + 9.0) * 0.6).fract() < 0.25 { 0.75 } else { 1.0 };
    b.part(fill(&ellipse_pts(4.8, 2.6, 18), bands), &fur, Vec3::new(-3.0, 0.0, 0.0), abdomen);
    b.part(fill(&ellipse_pts(3.2, 2.9, 16), |p| 0.55 + 0.45 * ((p.y + 2.9) / 5.8)), &fur, Vec3::new(0.3, 0.2, 0.1), rig);
    let head = b.bone(Vec3::new(3.2, 0.6, 0.2), rig, BoneKind::Head);
    b.part(fill(&ellipse_pts(1.9, 1.8, 12), |_| 0.8), &fur, Vec3::ZERO, head);
    b.part(fill(&ellipse_pts(1.2, 1.3, 10), |_| 1.0), &eye, Vec3::new(0.7, 0.2, 0.02), head);
    for (offset, z) in [(0.0, -0.05), (1.2, 0.05)] {
        let bone = b.bone(Vec3::new(0.8, 1.4, z), head, BoneKind::Antenna { offset });
        let shaft = smooth_open(&[Vec2::ZERO, Vec2::new(2.0, 2.6), Vec2::new(4.5, 4.0)], 4);
        b.part(stroke(&shaft, |t| 0.5 - 0.2 * t, 1.0, 1.0, |_| 1.0), &dark, Vec3::ZERO, bone);
        // Feathered: a comb along it.
        let comb: Vec<[Vec2; 3]> = (1..5)
            .map(|i| {
                let a = shaft[(i * shaft.len() / 5).min(shaft.len() - 1)];
                [a, a + Vec2::new(-0.4, 0.4), a + Vec2::new(0.7, 1.3)]
            })
            .collect();
        b.part(triangles(&comb), &dark, Vec3::ZERO, bone);
    }
    vec![(fur, MOTH), (dark, MOTH_DARK), (spot, MOTH_SPOT)]
}

/// An ootheca: a ridged foam egg case glued to a cable hanging from the
/// ceiling (the cable's top, the pivot, is `NEST_STALK` above the center),
/// with shed hatchling skins dangling from its emergence seam.
pub(super) fn ootheca(b: &mut Builder, materials: &mut Assets<ColorMaterial>, rig: Entity) -> Mats {
    let foam = materials.add(FOAM);
    let dark = materials.add(scale_color(FOAM, 0.55));
    let wire = materials.add(WIRE);
    let husk = materials.add(ColorMaterial { color: HUSK, alpha_mode: AlphaMode2d::Blend, ..default() });
    let sway = b.bone(Vec3::new(0.0, NEST_STALK, 0.0), rig, BoneKind::Sway);
    let o = -NEST_STALK;
    let cable = [Vec2::ZERO, Vec2::new(0.4, -6.0), Vec2::new(-0.3, -14.0), Vec2::new(0.0, -NEST_STALK + 2.0)];
    b.part(stroke(&smooth_open(&cable, 4), |_| 1.1, 1.0, 0.6, |_| 1.0), &wire, Vec3::new(0.0, 0.0, -0.1), sway);
    let outline = smooth_closed(
        &[
            Vec2::new(0.0, 10.0),
            Vec2::new(5.0, 8.4),
            Vec2::new(7.6, 2.0),
            Vec2::new(6.4, -5.0),
            Vec2::new(3.6, -10.0),
            Vec2::new(0.0, -11.5),
            Vec2::new(-3.4, -10.2),
            Vec2::new(-6.8, -5.0),
            Vec2::new(-7.4, 2.2),
            Vec2::new(-4.8, 8.6),
        ],
        5,
    );
    let shade = |p: Vec2| 0.6 + 0.4 * ((p.y + 11.5) / 21.5) - 0.18 * (p.x / 7.6).abs();
    b.part(fill(&outline, shade), &foam, Vec3::new(0.0, o, 0.0), sway);
    // Chevron ridges, and the emergence seam down the middle.
    for y in [6.0f32, 2.0, -2.0, -6.0] {
        let w = 6.4 - (y.abs() * 0.25);
        let ridge = smooth_open(&[Vec2::new(-w, y + 1.6), Vec2::new(0.0, y - 0.6), Vec2::new(w, y + 1.6)], 4);
        b.part(stroke(&ridge, |t| 0.3 + 0.7 * (PI * t).sin(), 1.0, 1.0, |_| 1.0), &dark, Vec3::new(0.0, o, 0.02), sway);
    }
    let seam = [Vec2::new(0.0, 9.0), Vec2::new(0.0, -10.5)];
    b.part(stroke(&seam, |t| 1.4 * (PI * t).sin() + 0.2, 1.0, 1.0, |_| 1.0), &foam, Vec3::new(0.0, o, 0.03), sway);
    // The cable wrapped around its top.
    for y in [8.0f32, 6.2] {
        let wrap = [Vec2::new(-4.0, y - 1.0), Vec2::new(4.0, y + 0.6)];
        b.part(stroke(&wrap, |_| 0.8, 1.0, 0.6, |_| 1.0), &wire, Vec3::new(0.0, o, 0.04), sway);
    }
    for (x, len) in [(-2.2f32, 6.0f32), (2.6, 4.5)] {
        let thread = [Vec2::new(x, -10.0), Vec2::new(x - 0.3, -10.0 - len)];
        b.part(stroke(&thread, |_| 0.3, 1.0, 1.0, |_| 1.0), &husk, Vec3::new(0.0, o, 0.01), sway);
        b.part(fill(&ellipse_pts(0.9, 2.0, 8), |_| 1.0), &husk, Vec3::new(x - 0.3, o - 11.5 - len, 0.01), sway);
    }
    vec![(foam, FOAM), (dark, scale_color(FOAM, 0.55))]
}

/// The pose a foreleg reaches for: (femur, tibia) angles.
fn foreleg_pose(m: &Mob, tt: f32) -> (f32, f32) {
    let shiver = m.windup() * (tt * 60.0).sin() * 0.08;
    let rise = m.mall.rise;
    match m.state {
        State::Dead { t, .. } => (0.2 + (tt * 25.0).sin() * 0.3 * (1.0 - t / 4.0).max(0.0), -0.6),
        State::Hurt(_) => (0.6, -0.8),
        // Raised high and opened: the tell.
        State::Windup(_) => (1.9 + shiver, -1.3),
        State::Attack(_) => (1.35, -2.4),
        // Hanging out after the strike, slowly folding back.
        State::Recover(s) => {
            let k = (s / m.spec.recover).clamp(0.0, 1.0);
            (1.2 - 0.85 * k, -2.2 * (1.0 - k))
        }
        _ if m.mall.parried > 0.0 => (1.15, -0.2),
        _ if m.mall.guard => (0.95 + (tt * 2.0).sin() * 0.05, -0.35),
        _ if m.spec.moves == Moves::Hop && m.airborne => (1.3, -1.8),
        // Folded in prayer (tighter while disguised).
        _ => (0.35 * rise + (tt * 1.7).sin() * 0.04, 0.0),
    }
}

/// Eases `tf`'s rotation to `angle` at `rate`.
fn turn_to(tf: &mut Transform, angle: f32, rate: f32, dt: f32) {
    let now = tf.rotation.to_euler(EulerRot::XYZ).2;
    let k = 1.0 - (-rate * dt).exp();
    tf.rotation = Quat::from_rotation_z(now + (angle - now) * k);
}

pub(super) fn animate(bone: Bone, m: &Mob, tf: &mut Transform, t: f32, dt: f32) {
    let tt = t + m.seed;
    let dead = matches!(m.state, State::Dead { .. });
    let windup = m.windup();
    let attacking = matches!(m.state, State::Attack(_));
    // Snaps are fast; the rest eases.
    let rate = if attacking || m.mall.parried > 0.0 { 45.0 } else { 14.0 };
    let crouch = 1.0 - m.mall.rise;
    match bone {
        Bone::Body => {
            // Sways like a leaf in a draft; leans into the strike; a ghost lies low in disguise.
            let sway = if dead || attacking { 0.0 } else { (tt * 1.7).sin() * 0.07 * m.mall.rise };
            let lean = if attacking { -0.18 } else { 0.0 };
            tf.translation.y = -3.0 * crouch;
            turn_to(tf, sway + lean - 0.3 * crouch, rate, dt);
        }
        Bone::Head => {
            let angle = if dead { -0.5 } else { m.mall.look - 0.6 * crouch + (tt * 0.9).sin() * 0.05 };
            turn_to(tf, angle, 10.0, dt);
        }
        Bone::Arm { near } => {
            let (femur, _) = foreleg_pose(m, tt);
            turn_to(tf, femur + if near { 0.0 } else { 0.08 }, rate, dt);
        }
        Bone::Claw { near } => {
            // The far one a touch behind.
            let (_, tibia) = foreleg_pose(m, tt);
            turn_to(tf, tibia + if near { 0.0 } else { 0.1 }, rate, dt);
        }
        Bone::Leaf { front } => {
            // Flat on the back, a breath of wind now and then; flared wide in the display.
            let flare = match m.state {
                _ if dead => 0.5,
                State::Windup(_) => windup * if front { 1.5 } else { 1.0 },
                State::Attack(_) => 0.7,
                State::Recover(_) => 0.3,
                _ => 0.12 * m.mall.rise + (tt * 0.8).sin().max(0.0) * 0.04,
            };
            tf.scale.y = 1.0 + 0.4 * windup;
            turn_to(tf, flare, 10.0, dt);
        }
        Bone::MothWing { front } => {
            let phase = if front { 0.0 } else { 0.9 };
            let (angle, open) = match m.state {
                _ if dead => (0.4, 1.0),
                // Swept back for the dart.
                State::Attack(_) => (0.9, 0.5),
                _ => {
                    let rate = if windup > 0.0 { 38.0 } else { 15.0 };
                    let flap = (tt * rate + phase).sin();
                    (0.45 * flap, 0.35 + 0.65 * flap.abs())
                }
            };
            tf.rotation = Quat::from_rotation_z(angle);
            tf.scale.y = open;
        }
    }
}

/// Parts torn off a fresh corpse: (velocity, spin, what it bleeds), or None to keep it.
pub(super) fn tear(bone: Bone, m: &Mob) -> Option<(Vec2, f32, Option<Fluid>)> {
    let f = m.facing;
    let blood = Some(m.spec.blood);
    match bone {
        // Mantises lose their heads (hatchlings are too small to bother).
        Bone::Head if m.kind != MobKind::MantisHatchling => Some((Vec2::new(f * 160.0, 420.0), 9.0, blood)),
        Bone::Arm { near: true } => Some((Vec2::new(f * 210.0, 380.0), -11.0, blood)),
        Bone::Leaf { front: true } => Some((Vec2::new(-f * 170.0, 430.0), 6.0, None)),
        Bone::MothWing { front } => Some((Vec2::new(-f * if front { 200.0 } else { 120.0 }, 440.0), 7.0, None)),
        _ => None,
    }
}
