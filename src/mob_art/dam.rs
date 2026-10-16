//! Bodies of the broken dam's mobs (behavior in `mobs/dam.rs`): a giant
//! isopod of overlapping plates that curls into a ball, a leech of segments
//! that inches and swells red with stolen blood, a barnacled crab with one
//! huge crusher claw held up as a shield, and a hermit crab wearing a broken
//! lump of the dam (formwork seams, rebar and all).

use std::f32::consts::PI;

use bevy::{prelude::*, sprite_render::AlphaMode2d};

use super::{BoneKind, Builder};
use crate::{
    camera,
    gore::Fluid,
    mobs::{
        dam::{stretch, INCH_MAX, INCH_MIN},
        Mob, MobKind, State,
    },
    shapes::*,
};

const LOUSE_SHELL: Color = Color::srgb(0.44, 0.42, 0.40);
const LOUSE_DARK: Color = Color::srgb(0.20, 0.19, 0.18);
const LOUSE_BELLY: Color = Color::srgb(0.64, 0.60, 0.54);
const LEECH_SKIN: Color = Color::srgb(0.20, 0.22, 0.16);
const LEECH_STRIPE: Color = Color::srgb(0.46, 0.42, 0.26);
const LEECH_MOUTH: Color = Color::srgb(0.08, 0.06, 0.06);
/// The chameleon's blood showing through a leech's skin (on the color layer).
const GORGE: Color = Color::srgba(0.72, 0.02, 0.03, 0.85);
const CRAB_SHELL: Color = Color::srgb(0.36, 0.38, 0.44);
const CRAB_DARK: Color = Color::srgb(0.15, 0.15, 0.18);
const CRAB_TIP: Color = Color::srgb(0.06, 0.05, 0.06);
const BARNACLE: Color = Color::srgb(0.76, 0.75, 0.70);
const EYE: Color = Color::srgb(0.06, 0.05, 0.05);
/// The hermit's shell: the dam's own concrete, soot and rebar.
const CONCRETE: Color = Color::srgb(0.50, 0.48, 0.44);
const SOOT: Color = Color::srgb(0.13, 0.12, 0.11);
const RUST: Color = Color::srgb(0.42, 0.25, 0.16);
const CALCITE: Color = Color::srgb(0.84, 0.83, 0.78);

/// Louse back plates, tail to head; the length of its back and the height of its top (drawing units).
const PLATES: usize = 7;
const LOUSE_LEN: f32 = 22.0;
const LOUSE_TOP: f32 = 4.0;
/// Curled up, the ball's center sits this high.
const BALL_LIFT: f32 = 1.0;
/// Leech segments, rear sucker to mouth, and how far its belly is below its center.
const SEGS: usize = 9;
const SEG_RADII: [f32; SEGS] = [2.3, 2.6, 2.9, 3.1, 3.2, 3.1, 2.8, 2.4, 1.9];
const LEECH_BELLY: f32 = 3.0;
/// Where the hermit's soft parts come out of its shell.
const OPENING: Vec2 = Vec2::new(8.0, -3.0);

#[derive(Clone, Copy)]
pub enum Bone {
    /// Everything but the rig: flipped onto the ceiling and spun as a ball
    /// (louse), hung off the chameleon (leech).
    Body,
    /// Louse back plates, tail (0) to head (`PLATES` − 1).
    Plate(usize),
    /// Louse underside (hidden as it curls).
    Belly,
    /// Legs, from the back; near side in front.
    Leg { i: usize, near: bool },
    Antenna(usize),
    /// Leech segments, rear sucker (0) to mouth.
    Seg(usize),
    /// Blood filling a leech.
    Gorge,
    /// Crab claws (big = the crusher), and the crusher's movable finger.
    Claw { big: bool },
    Finger,
    /// Eye stalks.
    Stalk(usize),
    /// The hermit's concrete shell, and all that comes out of it.
    Shell,
    Out,
}

type Mats = Vec<(Handle<ColorMaterial>, Color)>;

/// Build a dam mob's body under `rig`.
pub(super) fn body(b: &mut Builder, materials: &mut Assets<ColorMaterial>, rig: Entity, kind: MobKind) -> Mats {
    match kind {
        MobKind::GalleryLouse => louse(b, materials, rig),
        MobKind::SeepLeech => leech(b, materials, rig),
        MobKind::SpillwayCrab => crab(b, materials, rig),
        _ => hermit(b, materials, rig),
    }
}

fn bone(b: &mut Builder, pos: Vec3, parent: Entity, kind: Bone) -> Entity {
    b.bone(pos, parent, BoneKind::Dam(kind))
}

/// A giant isopod, facing +X, legs down to y ≈ -5: an arch of overlapping
/// plates, a small head with long feelers, little legs underneath, tail prongs.
fn louse(b: &mut Builder, materials: &mut Assets<ColorMaterial>, rig: Entity) -> Mats {
    let shell = materials.add(LOUSE_SHELL);
    let dark = materials.add(LOUSE_DARK);
    let belly = materials.add(LOUSE_BELLY);
    let eye = materials.add(EYE);
    let body = bone(b, Vec3::ZERO, rig, Bone::Body);
    for near in [false, true] {
        let (mat, z, dx) = if near { (&dark, 0.2, 0.0) } else { (&dark, -0.3, 1.0) };
        for i in 0..3 {
            let leg = bone(b, Vec3::new(-6.0 + i as f32 * 5.5 + dx, -0.8, z), body, Bone::Leg { i, near });
            for k in 0..2 {
                let x = k as f32 * 2.2 - 1.1;
                let path = smooth_open(&[Vec2::new(x, 0.0), Vec2::new(x + 1.2, -2.2), Vec2::new(x + 0.4, -4.2)], 3);
                b.part(stroke(&path, |t| 0.8 - 0.4 * t, 1.0, 0.8, |_| 1.0), mat, Vec3::ZERO, leg);
            }
        }
    }
    let under = bone(b, Vec3::new(0.0, 0.4, -0.1), body, Bone::Belly);
    b.part(fill(&ellipse_pts(10.5, 2.0, 18), |p| 0.6 + 0.3 * (p.y / 2.0)), &belly, Vec3::ZERO, under);
    let plate = smooth_closed(
        &[
            Vec2::new(-2.6, -2.8),
            Vec2::new(-2.9, -0.2),
            Vec2::new(-1.8, 1.8),
            Vec2::new(0.4, 2.3),
            Vec2::new(2.4, 1.4),
            Vec2::new(2.9, -0.8),
            Vec2::new(2.4, -2.8),
        ],
        4,
    );
    let rim = smooth_open(&[Vec2::new(-2.6, 0.6), Vec2::new(-1.6, 1.9), Vec2::new(0.4, 2.2), Vec2::new(2.3, 1.3)], 4);
    for i in 0..PLATES {
        let p = bone(b, Vec3::new(0.0, 0.0, 0.01 * i as f32), body, Bone::Plate(i));
        if i == PLATES - 1 {
            // The head: rounded, a dull eye, feelers.
            b.part(fill(&ellipse_pts(2.6, 2.6, 14), |q| 0.55 + 0.35 * ((q.y + 2.6) / 5.2)), &shell, Vec3::new(0.6, -0.6, 0.0), p);
            b.part(fill(&ellipse_pts(0.7, 0.8, 8), |_| 1.0), &eye, Vec3::new(1.8, -0.2, 0.02), p);
            for (k, z) in [(0, -0.05), (1, 0.05)] {
                let a = bone(b, Vec3::new(2.4, 0.4, z), p, Bone::Antenna(k));
                let path = smooth_open(&[Vec2::ZERO, Vec2::new(3.5, 1.6), Vec2::new(7.5, 0.6), Vec2::new(10.0, -2.2)], 5);
                b.part(stroke(&path, |t| 0.7 - 0.45 * t, 1.0, 1.0, |_| 1.0), &dark, Vec3::ZERO, a);
            }
        } else {
            b.part(fill(&plate, |q| 0.5 + 0.5 * ((q.y + 2.8) / 5.1).clamp(0.0, 1.0)), &shell, Vec3::ZERO, p);
            b.part(stroke(&rim, |t| 0.3 + 0.6 * (PI * t).sin(), 1.0, 1.0, |_| 1.0), &dark, Vec3::new(0.0, 0.0, 0.005), p);
            if i == 0 {
                for (end, z) in [(Vec2::new(-6.5, -3.4), -0.02), (Vec2::new(-6.0, -1.6), 0.006)] {
                    let prong = smooth_open(&[Vec2::new(-1.8, -1.2), Vec2::new(-4.0, -1.6), end], 3);
                    b.part(stroke(&prong, |t| 1.0 - 0.7 * t, 1.0, 1.0, |_| 1.0), &dark, Vec3::new(0.0, 0.0, z), p);
                }
            }
        }
    }
    vec![(shell, LOUSE_SHELL), (dark, LOUSE_DARK), (belly, LOUSE_BELLY)]
}

/// A leech, facing +X, belly at y = -`LEECH_BELLY`: tapered segments with a
/// pale stripe, a rear sucker, a toothed mouth; a red sac inside the middle
/// segments shows (on the color layer) as it fills.
fn leech(b: &mut Builder, materials: &mut Assets<ColorMaterial>, rig: Entity) -> Mats {
    let skin = materials.add(LEECH_SKIN);
    let stripe = materials.add(LEECH_STRIPE);
    let mouth = materials.add(LEECH_MOUTH);
    let teeth = materials.add(CALCITE);
    let blood = materials.add(ColorMaterial { color: GORGE, alpha_mode: AlphaMode2d::Blend, ..default() });
    let body = bone(b, Vec3::ZERO, rig, Bone::Body);
    for (i, r) in SEG_RADII.into_iter().enumerate() {
        let seg = bone(b, Vec3::new(0.0, 0.0, 0.01 * i as f32), body, Bone::Seg(i));
        b.part(fill(&ellipse_pts(r * 1.2, r, 16), move |p| 0.5 + 0.5 * ((p.y + r) / (2.0 * r))), &skin, Vec3::ZERO, seg);
        b.part(fill(&ellipse_pts(r * 0.9, 0.35, 8), |_| 1.0), &stripe, Vec3::new(0.0, r * 0.55, 0.003), seg);
        if (3..=6).contains(&i) {
            let gorge = bone(b, Vec3::new(0.0, -r * 0.1, 0.004), seg, Bone::Gorge);
            let mesh = b.meshes.add(fill(&ellipse_pts(r * 1.0, r * 0.7, 14), |_| 1.0));
            b.commands.spawn((Mesh2d(mesh), MeshMaterial2d(blood.clone()), Transform::default(), ChildOf(gorge), camera::color_layer()));
        }
        if i == 0 {
            b.part(fill(&ellipse_pts(1.0, r * 1.05, 10), |_| 0.7), &mouth, Vec3::new(-r * 1.0, -0.3, 0.002), seg);
        }
        if i == SEGS - 1 {
            b.part(fill(&ellipse_pts(0.9, r * 0.95, 10), |_| 1.0), &mouth, Vec3::new(r * 1.05, -0.2, 0.002), seg);
            let ring: Vec<[Vec2; 3]> = (0..4)
                .map(|k| {
                    let y = -1.3 + k as f32 * 0.85;
                    [Vec2::new(r * 1.0, y), Vec2::new(r * 1.0, y + 0.5), Vec2::new(r * 1.0 + 0.9, y + 0.25)]
                })
                .collect();
            b.part(triangles(&ring), &teeth, Vec3::new(0.0, 0.0, 0.003), seg);
        }
    }
    vec![(skin, LEECH_SKIN), (stripe, LEECH_STRIPE)]
}

/// Arched crab legs on `parent`: hips from the back, tips spread out down to y = `foot`.
fn crab_legs(b: &mut Builder, parent: Entity, near_mat: &Handle<ColorMaterial>, far_mat: &Handle<ColorMaterial>, hips: &[f32], reach: &[f32], hip_y: f32, foot: f32) {
    for near in [false, true] {
        let (mat, z, dx) = if near { (near_mat, 0.15, 0.0) } else { (far_mat, -0.3, 1.3) };
        for (i, (&x, &r)) in hips.iter().zip(reach).enumerate() {
            let leg = bone(b, Vec3::new(x + dx, hip_y, z), parent, Bone::Leg { i, near });
            let drop = foot - hip_y;
            let path = smooth_open(&[Vec2::ZERO, Vec2::new(r * 0.45, 5.0), Vec2::new(r * 0.85, 1.0), Vec2::new(r, drop)], 4);
            b.part(stroke(&path, |t| 1.7 - 1.3 * t, 1.0, 0.75, |_| 1.0), mat, Vec3::ZERO, leg);
        }
    }
}

/// A crusher claw on `parent`, from the shoulder: arm, swollen hand, fixed finger, movable `Finger`.
fn crusher(b: &mut Builder, parent: Entity, at: Vec3, shell: &Handle<ColorMaterial>, dark: &Handle<ColorMaterial>, tip: &Handle<ColorMaterial>) {
    let claw = bone(b, at, parent, Bone::Claw { big: true });
    let arm = smooth_open(&[Vec2::ZERO, Vec2::new(3.0, 1.6), Vec2::new(6.0, 2.2)], 4);
    b.part(stroke(&arm, |t| 2.6 - 0.8 * t, 1.0, 0.7, |_| 1.0), shell, Vec3::ZERO, claw);
    let hand = smooth_closed(
        &[Vec2::new(5.0, 2.0), Vec2::new(6.5, 5.6), Vec2::new(10.5, 6.4), Vec2::new(13.5, 4.5), Vec2::new(13.0, 0.6), Vec2::new(9.0, -0.6)],
        4,
    );
    b.part(fill(&hand, |p| 0.5 + 0.5 * ((p.y + 0.6) / 7.0)), shell, Vec3::new(0.0, 0.0, 0.02), claw);
    let lower = smooth_open(&[Vec2::new(12.0, 1.5), Vec2::new(15.0, 1.6), Vec2::new(17.5, 2.8)], 4);
    b.part(stroke(&lower, |t| 2.2 - 1.9 * t, 1.0, 0.8, |_| 1.0), tip, Vec3::new(0.0, 0.0, 0.01), claw);
    let knobs: Vec<[Vec2; 3]> = [13.0f32, 14.6]
        .into_iter()
        .map(|x| [Vec2::new(x, 2.2), Vec2::new(x + 1.0, 2.3), Vec2::new(x + 0.5, 3.2)])
        .collect();
    b.part(triangles(&knobs), dark, Vec3::new(0.0, 0.0, 0.015), claw);
    let finger = bone(b, Vec3::new(12.6, 4.6, 0.03), claw, Bone::Finger);
    let path = smooth_open(&[Vec2::ZERO, Vec2::new(2.6, 0.5), Vec2::new(4.8, -1.4)], 4);
    b.part(stroke(&path, |t| 2.0 - 1.7 * t, 1.0, 0.8, |_| 1.0), tip, Vec3::ZERO, finger);
}

fn stalks(b: &mut Builder, parent: Entity, at: [Vec2; 2], mat: &Handle<ColorMaterial>, eye: &Handle<ColorMaterial>, glint: &Handle<ColorMaterial>) {
    for (k, p) in at.into_iter().enumerate() {
        let s = bone(b, p.extend(0.05 - 0.2 * k as f32), parent, Bone::Stalk(k));
        b.part(stroke(&[Vec2::ZERO, Vec2::new(0.5, 3.2)], |t| 0.9 - 0.3 * t, 1.0, 1.0, |_| 1.0), mat, Vec3::ZERO, s);
        b.part(fill(&ellipse_pts(1.0, 1.1, 10), |_| 1.0), eye, Vec3::new(0.6, 3.6, 0.01), s);
        b.part(fill(&ellipse_pts(0.3, 0.3, 6), |_| 1.0), glint, Vec3::new(0.9, 4.0, 0.02), s);
    }
}

/// A crab, facing +X, feet at y ≈ -7: a wide barnacled carapace, arched
/// legs, eyes on stalks, a small picking claw and one huge crusher.
fn crab(b: &mut Builder, materials: &mut Assets<ColorMaterial>, rig: Entity) -> Mats {
    let shell = materials.add(CRAB_SHELL);
    let dark = materials.add(CRAB_DARK);
    let tip = materials.add(CRAB_TIP);
    let barnacle = materials.add(BARNACLE);
    let eye = materials.add(EYE);
    let glint = materials.add(Color::WHITE);
    crab_legs(b, rig, &shell, &dark, &[-6.0, -3.0, 0.0, 3.0], &[-9.0, -6.0, -2.0, 3.0], -1.0, -7.0);
    let small = bone(b, Vec3::new(7.5, -1.0, 0.2), rig, Bone::Claw { big: false });
    let pick = smooth_open(&[Vec2::ZERO, Vec2::new(2.5, -1.0), Vec2::new(4.5, 0.2), Vec2::new(5.5, -0.8)], 4);
    b.part(stroke(&pick, |t| 1.4 - 0.9 * t, 1.0, 0.8, |_| 1.0), &shell, Vec3::ZERO, small);
    let carapace = smooth_closed(
        &[
            Vec2::new(-11.0, 0.0),
            Vec2::new(-10.0, 4.0),
            Vec2::new(-6.0, 7.5),
            Vec2::new(0.0, 8.5),
            Vec2::new(6.0, 7.5),
            Vec2::new(10.0, 4.5),
            Vec2::new(11.5, 1.0),
            Vec2::new(9.0, -2.0),
            Vec2::new(0.0, -3.0),
            Vec2::new(-9.0, -2.0),
        ],
        5,
    );
    b.part(fill(&carapace, |p| 0.45 + 0.55 * ((p.y + 3.0) / 11.5).clamp(0.0, 1.0)), &shell, Vec3::ZERO, rig);
    // Spines along the front edge, a crack, barnacles crusted on the back.
    let spines: Vec<[Vec2; 3]> = [(7.0, 7.0), (9.5, 5.0), (11.0, 2.6)]
        .into_iter()
        .map(|(x, y)| [Vec2::new(x - 0.8, y), Vec2::new(x + 0.6, y - 0.6), Vec2::new(x + 1.4, y + 0.9)])
        .collect();
    b.part(triangles(&spines), &shell, Vec3::new(0.0, 0.0, -0.01), rig);
    let crack = [Vec2::new(2.0, 8.3), Vec2::new(0.6, 6.0), Vec2::new(-1.2, 4.2), Vec2::new(0.4, 1.5)];
    b.part(stroke(&crack, |t| 0.6 - 0.4 * t, 1.0, 1.0, |_| 1.0), &tip, Vec3::new(0.0, 0.0, 0.01), rig);
    for (x, y, r) in [(-6.5, 5.8, 1.3), (-3.0, 7.4, 1.0), (-8.6, 3.0, 0.9), (-4.6, 3.8, 0.7)] {
        b.part(fill(&ellipse_pts(r, r * 0.8, 10), |p| 0.7 + 0.3 * (p.y / r)), &barnacle, Vec3::new(x, y, 0.02), rig);
        b.part(fill(&ellipse_pts(r * 0.35, r * 0.3, 6), |_| 1.0), &tip, Vec3::new(x, y + r * 0.2, 0.03), rig);
    }
    let mouth: Vec<[Vec2; 3]> = vec![[Vec2::new(9.5, 2.5), Vec2::new(11.8, 1.6), Vec2::new(9.8, 0.4)]];
    b.part(triangles(&mouth), &dark, Vec3::new(0.0, 0.0, 0.01), rig);
    stalks(b, rig, [Vec2::new(6.0, 6.8), Vec2::new(7.6, 6.4)], &shell, &eye, &glint);
    crusher(b, rig, Vec3::new(7.0, 0.5, 0.4), &shell, &dark, &tip);
    vec![(shell, CRAB_SHELL), (dark, CRAB_DARK), (barnacle, BARNACLE)]
}

/// A hermit crab in a broken lump of the dam, facing +X, bottom at y ≈ -7:
/// the shell is concrete like the rubble around it (formwork seam, a tie
/// hole, calcite run, bent rebar); legs, eyes and a crusher come out of the
/// dark opening at the front (`Out`).
fn hermit(b: &mut Builder, materials: &mut Assets<ColorMaterial>, rig: Entity) -> Mats {
    let concrete = materials.add(CONCRETE);
    let soot = materials.add(SOOT);
    let rust = materials.add(RUST);
    let calcite = materials.add(CALCITE);
    let flesh = materials.add(CRAB_SHELL);
    let dark = materials.add(CRAB_DARK);
    let tip = materials.add(CRAB_TIP);
    let eye = materials.add(EYE);
    let glint = materials.add(Color::WHITE);

    let out = bone(b, OPENING.extend(-0.2), rig, Bone::Out);
    crab_legs(b, out, &flesh, &dark, &[-3.0, -1.0, 1.0], &[-4.0, 1.0, 4.5], -1.0, -4.0);
    stalks(b, out, [Vec2::new(2.0, 2.0), Vec2::new(3.4, 1.6)], &flesh, &eye, &glint);
    crusher(b, out, Vec3::new(2.5, 0.0, 0.5), &flesh, &dark, &tip);

    let shell = bone(b, Vec3::ZERO, rig, Bone::Shell);
    let lump = [
        Vec2::new(-14.0, -7.0),
        Vec2::new(-15.5, -1.0),
        Vec2::new(-13.0, 5.5),
        Vec2::new(-6.5, 10.0),
        Vec2::new(1.0, 11.0),
        Vec2::new(6.5, 8.5),
        Vec2::new(10.5, 3.5),
        Vec2::new(11.5, -2.5),
        Vec2::new(9.0, -7.0),
    ];
    b.part(fill(&lump, |p| 0.5 + 0.5 * ((p.y + 7.0) / 18.0) - 0.1 * (p.x / 15.0).abs()), &concrete, Vec3::ZERO, shell);
    // The dark mouth of the shell, low at the front.
    b.part(fill(&ellipse_pts(4.2, 3.6, 14), |p| 0.6 + 0.4 * (p.y / 3.6)), &soot, Vec3::new(8.0, -3.6, 0.01), shell);
    // Formwork seam, tie hole, a broken edge, calcite run down the side.
    let seam = [Vec2::new(-14.8, 1.5), Vec2::new(9.5, 2.5)];
    b.part(stroke(&seam, |_| 0.5, 1.0, 1.0, |_| 1.0), &soot, Vec3::new(0.0, 0.0, 0.02), shell);
    b.part(fill(&ellipse_pts(0.8, 0.8, 8), |_| 1.0), &soot, Vec3::new(-6.0, 5.5, 0.02), shell);
    let chip = [[Vec2::new(-6.5, 10.0), Vec2::new(-2.0, 10.6), Vec2::new(-4.5, 7.8)]];
    b.part(triangles(&chip), &soot, Vec3::new(0.0, 0.0, 0.015), shell);
    let run = smooth_open(&[Vec2::new(-10.0, 7.0), Vec2::new(-11.2, 2.0), Vec2::new(-10.6, -3.5)], 4);
    b.part(stroke(&run, |t| 1.6 - 1.2 * t, 1.0, 1.0, |_| 1.0), &calcite, Vec3::new(0.0, 0.0, 0.018), shell);
    for (path, w) in [
        (vec![Vec2::new(-3.0, 9.5), Vec2::new(-2.5, 15.0), Vec2::new(0.5, 19.0), Vec2::new(4.5, 20.0)], 1.0),
        (vec![Vec2::new(3.0, 9.0), Vec2::new(4.5, 12.5)], 0.9),
    ] {
        b.part(stroke(&smooth_open(&path, 4), move |_| w, 1.0, 0.7, |_| 1.0), &rust, Vec3::new(0.0, 0.0, -0.05), shell);
    }
    vec![(concrete, CONCRETE), (flesh, CRAB_SHELL), (dark, CRAB_DARK), (rust, RUST)]
}

/// Parts torn off a fresh corpse: (velocity, spin, bleeds), or `None` to keep it.
pub(super) fn torn(bone: Bone, m: &Mob) -> Option<(Vec2, f32, Option<Fluid>)> {
    let f = m.facing;
    let blood = Some(m.spec.blood);
    Some(match bone {
        Bone::Antenna(0) => (Vec2::new(f * 150.0, 380.0), 10.0, None),
        Bone::Plate(i) if i == PLATES - 1 && m.seed.fract() < 0.5 => (Vec2::new(f * 170.0, 360.0), 7.0, blood),
        Bone::Leg { i: 0, near: true } => (Vec2::new(-f * 140.0, 360.0), 8.0, None),
        Bone::Leg { i: 2, near: true } if m.kind != MobKind::GalleryLouse => (Vec2::new(f * 160.0, 420.0), -9.0, None),
        // Cut in two.
        Bone::Seg(i) if i >= SEGS - 3 => (Vec2::new(f * (150.0 + 25.0 * i as f32), 380.0), 6.0, blood),
        Bone::Claw { big: true } => (Vec2::new(f * 180.0, 440.0), 8.0, blood),
        Bone::Stalk(0) => (Vec2::new(f * 120.0, 400.0), 12.0, None),
        _ => return None,
    })
}

pub(super) fn pose(bone: Bone, m: &Mob, tf: &mut Transform, tt: f32, dt: f32) {
    let d = &m.dam;
    let dead = match m.state {
        State::Dead { t, .. } => Some(t),
        _ => None,
    };
    let windup = m.windup();
    let ease = |rate: f32| 1.0 - (-rate * dt).exp();
    // Legs kick on their backs for a while.
    let twitch = |k: f32| dead.map(|td| (tt * 28.0 + k * 1.7).sin() * 0.5 * (1.0 - td / 5.0).max(0.1));
    let walk = |k: f32, amp: f32| (m.stride * 2.0 + k).sin() * amp * (m.vel.x.abs() / 60.0).min(1.0);
    match (m.kind, bone) {
        // --- Louse ---
        (MobKind::GalleryLouse, Bone::Body) => {
            let up = if d.clinging && dead.is_none() { -1.0 } else { 1.0 };
            tf.scale.y += (up - tf.scale.y) * ease(6.0);
            let curl = d.curl * d.curl * (3.0 - 2.0 * d.curl);
            tf.rotation = Quat::from_rotation_z(d.roll * curl);
        }
        (MobKind::GalleryLouse, Bone::Plate(i)) => {
            let c = d.curl;
            let a = 0.5 + (5.6 - 0.5) * c;
            let r = LOUSE_LEN / a;
            let cy = (LOUSE_TOP - r) + (BALL_LIFT - (LOUSE_TOP - r)) * c;
            let s = -LOUSE_LEN / 2.0 + LOUSE_LEN * i as f32 / (PLATES - 1) as f32;
            // A breathing ripple down the back, a shiver in the windup.
            let ripple = (tt * 3.0 - i as f32 * 0.8).sin() * 0.04 * (1.0 - c) + windup * (tt * 50.0 + i as f32).sin() * 0.03;
            let phi = s / r + ripple;
            tf.translation.x = r * phi.sin();
            tf.translation.y = cy + r * phi.cos() - 1.0;
            tf.rotation = Quat::from_rotation_z(-phi);
        }
        (MobKind::GalleryLouse, Bone::Belly) => tf.scale = Vec3::splat((1.0 - d.curl).max(0.001)),
        (MobKind::GalleryLouse, Bone::Leg { i, near }) => {
            let k = i as f32 * 1.3 + if near { PI } else { 0.0 };
            let busy = if d.clinging || windup > 0.0 { (tt * 18.0 + k).sin() * 0.25 * windup.max(0.3) } else { 0.0 };
            tf.rotation = Quat::from_rotation_z(twitch(k).unwrap_or(walk(k, 0.4) + busy));
            tf.scale = Vec3::splat((1.0 - d.curl).max(0.001));
        }
        (MobKind::GalleryLouse, Bone::Antenna(k)) => {
            let k = k as f32 * 1.1;
            let angle = if dead.is_some() { -0.5 } else { (tt * 2.4 + k).sin() * 0.2 + windup * (tt * 30.0 + k).sin() * 0.25 };
            tf.rotation = Quat::from_rotation_z(angle);
        }

        // --- Leech ---
        (MobKind::SeepLeech, Bone::Body) => {
            let hang = if d.latch.is_some() { 0.55 + (tt * 5.0).sin() * 0.08 } else { 0.0 };
            tf.rotation = Quat::from_rotation_z(hang);
        }
        (MobKind::SeepLeech, Bone::Seg(i)) => {
            let u = i as f32 / (SEGS - 1) as f32;
            let flying = m.airborne && matches!(m.state, State::Attack(_));
            let (len, arch) = if dead.is_some() {
                (INCH_MAX, 0.0)
            } else if d.latch.is_some() {
                (INCH_MIN + 5.0 + (tt * 6.0).sin() * 1.5, 3.0)
            } else if flying || matches!(m.state, State::Recover(_)) {
                (INCH_MAX, 0.5)
            } else {
                let len = INCH_MIN + (INCH_MAX - INCH_MIN) * stretch(d.inch);
                (len, 0.3 * (INCH_MAX * INCH_MAX - len * len).max(0.0).sqrt())
            };
            // Rearing: the front end lifts and sways, feeling for the chameleon.
            let rear = windup * u * u * 9.0 + windup * u * (tt * 7.0).sin() * 2.0;
            let writhe = dead.map_or(0.0, |td| (tt * 14.0 - u * 6.0).sin() * 1.2 * (1.0 - td / 4.0).max(0.0));
            let r = SEG_RADII[i];
            tf.translation.x = -len / 2.0 + u * len;
            tf.translation.y = -LEECH_BELLY + r * 0.85 + arch * (PI * u).sin() + rear + writhe;
            let slope = arch * PI * (PI * u).cos() / len + windup * 2.0 * u * 9.0 / len;
            tf.rotation = Quat::from_rotation_z(slope.atan());
            let swell = if d.gorged { 1.0 } else { d.swell };
            let mid = (PI * u).sin();
            tf.scale = Vec3::new(1.0 + 0.25 * swell * mid, 1.0 + 0.5 * swell * mid, 1.0);
        }
        (MobKind::SeepLeech, Bone::Gorge) => {
            let swell = if d.gorged { 1.0 } else { d.swell };
            let pulse = if d.latch.is_some() { 1.0 + (tt * 9.0).sin() * 0.08 } else { 1.0 };
            tf.scale = Vec3::splat((swell * pulse).max(0.001));
        }

        // --- Crab and hermit ---
        (_, Bone::Leg { i, near }) => {
            let k = i as f32 * 1.4 + if near { PI } else { 0.0 };
            let scrabble = windup * (tt * 30.0 + k).sin() * 0.12;
            tf.rotation = Quat::from_rotation_z(twitch(k).unwrap_or(walk(k, 0.35) + scrabble));
        }
        (_, Bone::Stalk(k)) => {
            let k = k as f32 * 2.3;
            // Twitching now and then; folded down when dead.
            let flick = if (tt * 0.9 + k).sin() > 0.95 { (tt * 40.0).sin() * 0.3 } else { 0.0 };
            let angle = if dead.is_some() { -1.2 } else { (tt * 1.7 + k).sin() * 0.12 + flick - 0.6 * windup };
            tf.rotation = Quat::from_rotation_z(angle);
        }
        (MobKind::SpillwayCrab, Bone::Claw { big: true }) => {
            // Up in front of the face (the guard), raised in the windup, thrust
            // forward in the snap, jammed into the ground after it.
            let target = match m.state {
                _ if dead.is_some() => -0.6,
                State::Windup(_) => 1.4 * windup + 0.9 * (1.0 - windup),
                State::Attack(_) => -0.15,
                State::Recover(_) | State::Hurt(_) => -0.55,
                _ if m.guarding() => 0.9 + (tt * 3.0).sin() * 0.05,
                _ => 0.1 + (tt * 1.3).sin() * 0.05,
            };
            let rate = if matches!(m.state, State::Attack(_)) { 40.0 } else { 10.0 };
            let now = tf.rotation.to_euler(EulerRot::ZYX).0;
            tf.rotation = Quat::from_rotation_z(now + (target - now) * ease(rate));
        }
        (MobKind::SpillwayCrab, Bone::Claw { big: false }) => {
            // Picking at the silt, to its mouth and back.
            let angle = if dead.is_some() { -0.4 } else { (tt * 2.2).sin().max(0.0) * 0.6 - 0.2 };
            tf.rotation = Quat::from_rotation_z(angle);
        }
        (MobKind::RubbleHermit, Bone::Claw { .. }) => {
            let target = match m.state {
                _ if dead.is_some() => -0.5,
                State::Windup(_) => 1.6 * windup,
                State::Attack(s) => if s < 0.1 { 1.6 - 20.0 * s } else { -0.45 },
                State::Recover(s) => -0.45 * (1.0 - (s / m.spec.recover).powi(3)),
                _ => (tt * 1.5).sin() * 0.06,
            };
            let rate = if matches!(m.state, State::Attack(_)) { 50.0 } else { 12.0 };
            let now = tf.rotation.to_euler(EulerRot::ZYX).0;
            tf.rotation = Quat::from_rotation_z(now + (target - now) * ease(rate));
        }
        (_, Bone::Finger) => {
            let open = match m.state {
                State::Windup(_) => 0.7 * windup,
                State::Attack(_) => -0.1,
                State::Recover(_) => 0.25,
                _ if dead.is_some() => 0.4,
                _ => 0.08 + (tt * 2.0).sin().max(0.0) * 0.15,
            };
            tf.rotation = Quat::from_rotation_z(open);
        }
        (_, Bone::Shell) => {
            // Jolts as it bursts out; settles otherwise.
            let jolt = windup * (tt * 45.0).sin() * 0.05;
            tf.rotation = Quat::from_rotation_z(jolt);
        }
        (_, Bone::Out) => {
            let o = if dead.is_some() { 0.7 } else { d.out };
            tf.translation.x = OPENING.x - (1.0 - o) * 5.0;
            tf.translation.y = OPENING.y - (1.0 - o) * 1.0;
            tf.scale = Vec3::splat(0.15 + 0.85 * o);
        }
        _ => {}
    }
}
