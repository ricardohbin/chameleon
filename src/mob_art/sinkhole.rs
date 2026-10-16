//! Bodies of the sunken street's vermin (`mobs/sinkhole.rs`): a locust in
//! flight, a black rubble cricket on its drumstick legs, a velvety mole
//! cricket with shovel claws under its mound of dirt, and a katydid hidden
//! under a dead-leaf wing. All drawn facing +X, animated by `animate` (they
//! don't go through the shared one), and torn apart by `dismember` (the
//! shared bones there, plus `torn` for these).

use std::f32::consts::PI;

use bevy::{prelude::*, sprite_render::AlphaMode2d};

use super::{BoneKind, Builder, EYE};
use crate::{
    gore::Fluid,
    mobs::{Mob, MobKind, Moves, State},
    shapes::*,
};

type Mats = Vec<(Handle<ColorMaterial>, Color)>;

const LOCUST: Color = Color::srgb(0.50, 0.40, 0.24);
const LOCUST_DARK: Color = Color::srgb(0.22, 0.16, 0.09);
const TEGMEN: Color = Color::srgb(0.42, 0.35, 0.22);
/// The grasshopper boss's rust brown, in the locusts' hind wings.
const HINDWING: Color = Color::srgba(0.56, 0.34, 0.23, 0.45);
const CRICKET: Color = Color::srgb(0.09, 0.08, 0.07);
const CRICKET_SHEEN: Color = Color::srgb(0.45, 0.40, 0.34);
const CRICKET_WING: Color = Color::srgb(0.20, 0.15, 0.10);
const MOLE: Color = Color::srgb(0.40, 0.29, 0.18);
const MOLE_DARK: Color = Color::srgb(0.20, 0.14, 0.08);
const CLAW: Color = Color::srgb(0.13, 0.09, 0.05);
const SOIL: Color = Color::srgb(0.30, 0.26, 0.21);
const SOIL_DARK: Color = Color::srgb(0.17, 0.14, 0.11);
const LEAF: Color = Color::srgb(0.46, 0.44, 0.32);
const LEAF_VEIN: Color = Color::srgb(0.28, 0.26, 0.18);
const KATYDID: Color = Color::srgb(0.55, 0.53, 0.40);
const KATY_WING: Color = Color::srgba(0.85, 0.85, 0.75, 0.35);
/// The mound of dirt over a buried mole cricket, half-width (world units).
const MOUND: f32 = 16.0;

/// Bones only these bodies have.
#[derive(Clone, Copy)]
pub(super) enum Part {
    /// A big jumping leg (femur + tibia), pivot at the hip.
    Hind { near: bool },
    /// The mole cricket's digging claws.
    Shovel,
    /// The dirt over a buried mole cricket (a child of the root, unscaled: it doesn't flip or sink).
    Mound,
    /// A forewing over the back, pivot at its front: the cricket's chirping wing, the katydid's leaf.
    Tegmen,
}

/// The kinds drawn (and animated) here.
pub(super) fn draws(kind: MobKind) -> bool {
    matches!(kind, MobKind::Locust | MobKind::RubbleCricket | MobKind::MoleCricket | MobKind::WreckKatydid)
}

fn stripe(pts: &[Vec2], w0: f32, w1: f32) -> Mesh {
    stroke(&smooth_open(pts, 4), move |t| w0 + (w1 - w0) * t, 1.0, 0.85, |_| 1.0)
}

/// A jumping leg: a thick femur from the hip to the knee, a thin spined tibia to the foot.
fn hind_leg(b: &mut Builder, mat: &Handle<ColorMaterial>, bone: Entity, knee: Vec2, foot: Vec2, thick: f32) {
    let mid = knee * 0.5 + Vec2::new(0.0, thick * 0.3);
    b.part(stripe(&[Vec2::ZERO, mid, knee], thick, thick * 0.45), mat, Vec3::ZERO, bone);
    b.part(stripe(&[knee, knee.lerp(foot, 0.5) + Vec2::new(0.4, 0.0), foot], thick * 0.4, thick * 0.25), mat, Vec3::new(0.0, 0.0, 0.01), bone);
    // Spines along the tibia.
    let spines: Vec<[Vec2; 3]> = (1..4)
        .map(|i| {
            let p = knee.lerp(foot, i as f32 / 4.0);
            let n = (foot - knee).perp().normalize_or_zero() * 1.2;
            [p, p + (foot - knee).normalize_or_zero() * 0.8, p - n]
        })
        .collect();
    b.part(triangles(&spines), mat, Vec3::new(0.0, 0.0, 0.01), bone);
}

/// A locust in flight, centered on its thorax: long speckled forewings, rust
/// hind wings fanned behind, the jumping legs trailing.
pub(super) fn locust(b: &mut Builder, materials: &mut Assets<ColorMaterial>, rig: Entity) -> Mats {
    let body = materials.add(LOCUST);
    let dark = materials.add(LOCUST_DARK);
    let teg = materials.add(TEGMEN);
    let eye = materials.add(EYE);
    let glint = materials.add(Color::WHITE);
    let fan = materials.add(ColorMaterial { color: HINDWING, alpha_mode: AlphaMode2d::Blend, ..default() });

    // The hind wing: a fan back and up from its root.
    let back = b.bone(Vec3::new(-1.0, 2.5, -0.4), rig, BoneKind::Wing { front: false });
    let mut pts = vec![Vec2::ZERO];
    pts.extend((0..=10).map(|i| {
        let a = PI * (0.58 + 0.42 * i as f32 / 10.0);
        Vec2::from_angle(a) * (12.0 - 2.0 * (i as f32 / 10.0 - 0.6).abs() * 3.0)
    }));
    b.part(fill(&pts, |p| 0.7 + 0.3 * (p.length() / 12.0)), &fan, Vec3::ZERO, back);
    // Ribs across the fan.
    for i in 0..4 {
        let a = PI * (0.62 + 0.11 * i as f32);
        b.part(stripe(&[Vec2::ZERO, Vec2::from_angle(a) * 10.5], 0.35, 0.15), &dark, Vec3::new(0.0, 0.0, 0.01), back);
    }

    // Far legs, then the far jumping leg.
    for (pair, x) in [(0, 3.0), (1, 0.5)] {
        let bone = b.bone(Vec3::new(x + 0.8, -2.2, -0.3), rig, BoneKind::Leg { pair, near: false });
        b.part(stripe(&[Vec2::ZERO, Vec2::new(0.8, -2.4), Vec2::new(0.2, -4.4)], 0.8, 0.4), &dark, Vec3::ZERO, bone);
    }
    let far = b.bone(Vec3::new(-1.0, -0.5, -0.35), rig, BoneKind::Sinkhole(Part::Hind { near: false }));
    hind_leg(b, &dark, far, Vec2::new(-9.0, -1.5), Vec2::new(-4.5, -6.5), 2.2);

    // Abdomen: long, ringed.
    let abdomen = b.bone(Vec3::new(-3.0, -0.3, 0.0), rig, BoneKind::Abdomen);
    let outline = smooth_closed(
        &[Vec2::new(1.0, 2.2), Vec2::new(-6.0, 2.6), Vec2::new(-13.0, 1.6), Vec2::new(-16.0, 0.0), Vec2::new(-13.0, -1.8), Vec2::new(-6.0, -2.6), Vec2::new(1.0, -2.4)],
        4,
    );
    b.part(fill(&outline, |p| 0.55 + 0.45 * ((p.y + 2.6) / 5.2).clamp(0.0, 1.0)), &body, Vec3::ZERO, abdomen);
    for x in [-3.0f32, -6.0, -9.0, -12.0] {
        let h = 2.4 - (x.abs() / 16.0) * 1.2;
        b.part(stripe(&[Vec2::new(x + 0.4, h), Vec2::new(x - 0.3, 0.0), Vec2::new(x + 0.3, -h)], 0.35, 0.35), &dark, Vec3::new(0.0, 0.0, 0.01), abdomen);
    }

    // Thorax and the saddle of the pronotum, with its crest.
    b.part(fill(&ellipse_pts(4.5, 3.6, 18), |p| 0.6 + 0.4 * ((p.y + 3.6) / 7.2)), &body, Vec3::new(0.5, 0.0, 0.1), rig);
    let saddle = smooth_closed(&[Vec2::new(-3.0, 1.0), Vec2::new(-2.6, 3.8), Vec2::new(1.5, 4.6), Vec2::new(4.5, 3.6), Vec2::new(4.2, 0.0), Vec2::new(0.0, -0.8)], 4);
    b.part(fill(&saddle, |p| 0.65 + 0.35 * (p.y / 4.6).clamp(0.0, 1.0)), &teg, Vec3::new(0.0, 0.0, 0.15), rig);
    b.part(stripe(&[Vec2::new(-2.6, 3.9), Vec2::new(1.0, 4.8), Vec2::new(4.4, 3.7)], 0.5, 0.3), &dark, Vec3::new(0.0, 0.0, 0.16), rig);

    // Near legs and the near jumping leg.
    for (pair, x) in [(0, 3.0), (1, 0.5)] {
        let bone = b.bone(Vec3::new(x, -2.5, 0.2), rig, BoneKind::Leg { pair, near: true });
        b.part(stripe(&[Vec2::ZERO, Vec2::new(0.8, -2.4), Vec2::new(0.2, -4.6)], 0.9, 0.45), &body, Vec3::ZERO, bone);
    }
    let near = b.bone(Vec3::new(-1.5, -0.5, 0.3), rig, BoneKind::Sinkhole(Part::Hind { near: true }));
    hind_leg(b, &body, near, Vec2::new(-10.0, -1.5), Vec2::new(-5.0, -7.0), 2.6);

    // Head: tall and blunt, a big eye, chewing jaws, short antennae.
    let head = b.bone(Vec3::new(4.8, 0.6, 0.25), rig, BoneKind::Head);
    let face = smooth_closed(&[Vec2::new(-1.5, 3.4), Vec2::new(2.0, 3.8), Vec2::new(4.0, 1.4), Vec2::new(4.2, -2.6), Vec2::new(2.4, -4.4), Vec2::new(-1.2, -3.4)], 4);
    b.part(fill(&face, |p| 0.6 + 0.4 * ((p.y + 4.4) / 8.2)), &body, Vec3::ZERO, head);
    b.part(fill(&ellipse_pts(1.5, 1.9, 12), |p| 0.7 + 0.3 * (p.y / 1.9)), &eye, Vec3::new(1.6, 1.3, 0.02), head);
    b.part(fill(&ellipse_pts(0.35, 0.45, 6), |_| 1.0), &glint, Vec3::new(2.0, 2.0, 0.03), head);
    b.part(fill(&[Vec2::new(2.0, -3.6), Vec2::new(4.4, -3.0), Vec2::new(3.4, -5.2)], |_| 1.0), &dark, Vec3::new(0.0, 0.0, 0.02), head);
    for (offset, z) in [(0.0, -0.05), (1.1, 0.04)] {
        let bone = b.bone(Vec3::new(2.4, 3.2, z), head, BoneKind::Antenna { offset });
        b.part(stripe(&[Vec2::ZERO, Vec2::new(3.0, 3.0), Vec2::new(7.5, 4.5), Vec2::new(10.0, 3.6)], 0.7, 0.35), &dark, Vec3::ZERO, bone);
    }

    // The forewing: long, narrow, leathery, speckled.
    let front = b.bone(Vec3::new(-1.0, 3.2, 0.5), rig, BoneKind::Wing { front: true });
    let wing: Vec<Vec2> = ellipse_pts(9.5, 1.8, 16).into_iter().map(|p| Vec2::from_angle(0.18).rotate(p + Vec2::new(-8.5, 0.0))).collect();
    b.part(fill(&wing, |p| 0.75 + 0.25 * (p.y / 3.0).clamp(-1.0, 1.0)), &teg, Vec3::ZERO, front);
    let mut rng = Rng(0x10C0_57A1);
    for _ in 0..6 {
        let x = rng.range(-15.0, -3.0);
        let at = Vec2::from_angle(0.18).rotate(Vec2::new(x, rng.range(-0.8, 0.8)));
        b.part(fill(&ellipse_pts(rng.range(0.5, 0.9), 0.5, 6), |_| 1.0), &dark, at.extend(0.01), front);
    }

    vec![(body, LOCUST), (dark, LOCUST_DARK), (teg, TEGMEN)]
}

/// A rubble cricket standing, feet at y ≈ -6.5: glossy black, a big round
/// head with whip antennae, the chirping wings over its back, drumstick legs.
pub(super) fn cricket(b: &mut Builder, materials: &mut Assets<ColorMaterial>, rig: Entity) -> Mats {
    let black = materials.add(CRICKET);
    let sheen = materials.add(CRICKET_SHEEN);
    let wing = materials.add(CRICKET_WING);
    let eye = materials.add(Color::srgb(0.30, 0.26, 0.20));

    let far = b.bone(Vec3::new(-1.5, 0.5, -0.3), rig, BoneKind::Sinkhole(Part::Hind { near: false }));
    hind_leg(b, &black, far, Vec2::new(-8.5, 3.5), Vec2::new(-5.0, -7.0), 3.0);
    for (pair, x) in [(0, 4.5), (1, 0.5)] {
        let bone = b.bone(Vec3::new(x + 0.8, -2.0, -0.25), rig, BoneKind::Leg { pair, near: false });
        b.part(stripe(&[Vec2::ZERO, Vec2::new(1.2, -2.4), Vec2::new(1.6, -4.5)], 0.9, 0.5), &black, Vec3::ZERO, bone);
    }
    // Cerci.
    for (dy, z) in [(1.5f32, -0.05), (-1.0, 0.05)] {
        b.part(stripe(&[Vec2::new(-9.5, 0.5), Vec2::new(-13.0, 0.8 + dy * 0.5), Vec2::new(-16.0, 1.0 + dy)], 0.6, 0.2), &black, Vec3::new(0.0, 0.0, z), rig);
    }
    let body = smooth_closed(
        &[Vec2::new(6.0, 1.5), Vec2::new(4.5, 4.5), Vec2::new(0.0, 5.2), Vec2::new(-6.0, 4.6), Vec2::new(-10.0, 2.0), Vec2::new(-10.5, -1.5), Vec2::new(-7.0, -3.5), Vec2::new(0.0, -3.6), Vec2::new(5.0, -2.5)],
        5,
    );
    b.part(fill(&body, |p| 0.5 + 0.5 * ((p.y + 3.6) / 8.8).clamp(0.0, 1.0)), &black, Vec3::ZERO, rig);
    b.part(stripe(&[Vec2::new(-6.0, 3.6), Vec2::new(-2.0, 4.6), Vec2::new(2.0, 4.4)], 0.6, 0.2), &sheen, Vec3::new(0.0, 0.0, 0.02), rig);

    // The wings that sing: flat over the back, raised to chirp.
    let teg = b.bone(Vec3::new(3.5, 3.4, 0.3), rig, BoneKind::Sinkhole(Part::Tegmen));
    let outline = smooth_closed(&[Vec2::new(0.5, 0.5), Vec2::new(-4.0, 1.8), Vec2::new(-11.0, 1.2), Vec2::new(-13.0, -0.6), Vec2::new(-9.0, -1.6), Vec2::new(-2.0, -1.2)], 4);
    b.part(fill(&outline, |p| 0.7 + 0.3 * (p.y / 1.8).clamp(-1.0, 1.0)), &wing, Vec3::ZERO, teg);
    for (a, c) in [(Vec2::new(-1.0, 0.0), Vec2::new(-11.0, 0.2)), (Vec2::new(-3.0, 1.2), Vec2::new(-7.0, -1.0)), (Vec2::new(-6.0, 1.2), Vec2::new(-10.0, -1.0))] {
        b.part(stripe(&[a, c], 0.3, 0.2), &black, Vec3::new(0.0, 0.0, 0.01), teg);
    }

    for (pair, x) in [(0, 4.5), (1, 0.5)] {
        let bone = b.bone(Vec3::new(x, -2.2, 0.2), rig, BoneKind::Leg { pair, near: true });
        b.part(stripe(&[Vec2::ZERO, Vec2::new(1.2, -2.4), Vec2::new(1.6, -4.3)], 1.0, 0.5), &black, Vec3::ZERO, bone);
    }
    let near = b.bone(Vec3::new(-2.0, 0.5, 0.35), rig, BoneKind::Sinkhole(Part::Hind { near: true }));
    hind_leg(b, &black, near, Vec2::new(-9.0, 3.0), Vec2::new(-4.5, -7.0), 3.4);
    b.part(stripe(&[Vec2::new(-1.0, 0.8), Vec2::new(-5.0, 2.4), Vec2::new(-8.0, 2.8)], 0.7, 0.2), &sheen, Vec3::new(0.0, 0.0, 0.02), near);

    let head = b.bone(Vec3::new(6.0, 1.0, 0.4), rig, BoneKind::Head);
    b.part(fill(&ellipse_pts(3.3, 3.5, 16), |p| 0.6 + 0.4 * ((p.y + 3.5) / 7.0)), &black, Vec3::ZERO, head);
    b.part(fill(&ellipse_pts(1.0, 1.2, 10), |_| 1.0), &eye, Vec3::new(1.2, 1.2, 0.02), head);
    b.part(stripe(&[Vec2::new(2.2, -2.5), Vec2::new(3.5, -4.0), Vec2::new(3.0, -5.2)], 0.6, 0.3), &black, Vec3::new(0.0, 0.0, 0.01), head);
    for (offset, z) in [(0.0, -0.05), (1.4, 0.04)] {
        let bone = b.bone(Vec3::new(2.0, 2.5, z), head, BoneKind::Antenna { offset });
        let path = [Vec2::ZERO, Vec2::new(6.0, 5.0), Vec2::new(15.0, 8.0), Vec2::new(25.0, 6.0), Vec2::new(33.0, 1.0)];
        b.part(stroke(&smooth_open(&path, 6), |t| 0.6 - 0.4 * t, 1.0, 1.0, |_| 1.0), &black, Vec3::ZERO, bone);
    }

    vec![(black, CRICKET), (sheen, CRICKET_SHEEN), (wing, CRICKET_WING)]
}

/// A mole cricket, feet at y ≈ -5: velvety cylinder, a domed shield over the
/// front, shovel claws; plus the mound of dirt it hides under (on `root`).
pub(super) fn mole_cricket(b: &mut Builder, materials: &mut Assets<ColorMaterial>, rig: Entity, root: Entity, foot: f32) -> Mats {
    let velvet = materials.add(MOLE);
    let dark = materials.add(MOLE_DARK);
    let claw = materials.add(CLAW);
    let eye = materials.add(EYE);
    let soil = materials.add(SOIL);
    let soil_dark = materials.add(SOIL_DARK);

    for (pair, x) in [(1, 0.0), (2, -5.0)] {
        let bone = b.bone(Vec3::new(x + 0.8, -2.2, -0.25), rig, BoneKind::Leg { pair, near: false });
        b.part(stripe(&[Vec2::ZERO, Vec2::new(-1.2, -1.4), Vec2::new(-2.4, -2.8)], 1.0, 0.5), &dark, Vec3::ZERO, bone);
    }
    // Cerci and the soft abdomen with short wings over it.
    for dy in [1.2f32, -0.8] {
        b.part(stripe(&[Vec2::new(-13.5, 0.5), Vec2::new(-17.0, 0.6 + dy), Vec2::new(-19.5, 1.0 + dy * 1.5)], 0.6, 0.2), &dark, Vec3::new(0.0, 0.0, -0.05), rig);
    }
    b.part(fill(&ellipse_pts(8.5, 3.6, 20), |p| 0.55 + 0.45 * ((p.y + 3.6) / 7.2)), &velvet, Vec3::new(-6.0, 0.3, 0.0), rig);
    for x in [-3.0f32, -6.0, -9.0, -12.0] {
        b.part(stripe(&[Vec2::new(x + 0.3, 3.2), Vec2::new(x - 0.3, 0.3), Vec2::new(x + 0.3, -2.8)], 0.3, 0.3), &dark, Vec3::new(0.0, 0.0, 0.01), rig);
    }
    let wings = smooth_closed(&[Vec2::new(-1.0, 3.0), Vec2::new(-6.0, 3.9), Vec2::new(-10.5, 3.0), Vec2::new(-9.0, 1.6), Vec2::new(-2.0, 1.8)], 4);
    b.part(fill(&wings, |_| 0.85), &dark, Vec3::new(0.0, 0.0, 0.02), rig);

    for (pair, x) in [(1, 0.0), (2, -5.0)] {
        let bone = b.bone(Vec3::new(x, -2.5, 0.2), rig, BoneKind::Leg { pair, near: true });
        b.part(stripe(&[Vec2::ZERO, Vec2::new(-1.2, -1.2), Vec2::new(-2.6, -2.5)], 1.1, 0.5), &velvet, Vec3::ZERO, bone);
    }
    // The shield: a hard dome over the front.
    let shield = smooth_closed(&[Vec2::new(-1.0, -2.6), Vec2::new(-1.5, 2.6), Vec2::new(2.0, 4.8), Vec2::new(6.5, 4.4), Vec2::new(8.6, 1.0), Vec2::new(7.6, -2.8), Vec2::new(3.0, -3.6)], 5);
    b.part(fill(&shield, |p| 0.55 + 0.45 * ((p.y + 3.6) / 8.4).clamp(0.0, 1.0)), &velvet, Vec3::new(0.0, 0.0, 0.1), rig);
    b.part(stripe(&[Vec2::new(-0.8, 2.4), Vec2::new(2.5, 4.3), Vec2::new(6.4, 4.0)], 0.5, 0.3), &dark, Vec3::new(0.0, 0.0, 0.11), rig);

    let head = b.bone(Vec3::new(8.6, 0.0, 0.15), rig, BoneKind::Head);
    b.part(fill(&ellipse_pts(2.4, 2.1, 12), |p| 0.6 + 0.4 * ((p.y + 2.1) / 4.2)), &dark, Vec3::ZERO, head);
    b.part(fill(&ellipse_pts(0.6, 0.6, 6), |_| 1.0), &eye, Vec3::new(0.8, 0.7, 0.02), head);
    for (offset, z) in [(0.0, -0.05), (0.9, 0.04)] {
        let bone = b.bone(Vec3::new(1.5, 1.0, z), head, BoneKind::Antenna { offset });
        b.part(stripe(&[Vec2::ZERO, Vec2::new(3.0, 1.5), Vec2::new(6.0, 1.2)], 0.6, 0.3), &dark, Vec3::ZERO, bone);
    }

    // The shovels: a thick forearm and a broad toothed claw.
    let shovel = b.bone(Vec3::new(6.5, -2.0, 0.3), rig, BoneKind::Sinkhole(Part::Shovel));
    b.part(stripe(&[Vec2::ZERO, Vec2::new(1.8, -1.2), Vec2::new(3.0, -2.2)], 2.4, 1.8), &dark, Vec3::ZERO, shovel);
    let palm = smooth_closed(&[Vec2::new(2.0, -1.0), Vec2::new(4.6, -1.4), Vec2::new(5.2, -3.4), Vec2::new(2.4, -3.6)], 3);
    b.part(fill(&palm, |_| 0.9), &claw, Vec3::new(0.0, 0.0, 0.01), shovel);
    let teeth: Vec<[Vec2; 3]> = (0..4)
        .map(|i| {
            let x = 2.4 + i as f32 * 0.8;
            [Vec2::new(x, -3.3), Vec2::new(x + 0.7, -3.3), Vec2::new(x + 0.9, -5.2 + i as f32 * 0.25)]
        })
        .collect();
    b.part(triangles(&teeth), &claw, Vec3::new(0.0, 0.0, 0.02), shovel);

    // The mound: a hump of broken soil with pebbles, sitting on the ground.
    let mound = b.bone(Vec3::new(0.0, -foot, 0.6), root, BoneKind::Sinkhole(Part::Mound));
    let mut hump: Vec<Vec2> = (0..=16).map(|i| {
        let a = PI * i as f32 / 16.0;
        Vec2::new(-a.cos() * MOUND, a.sin() * 9.0 + (a * 7.0).sin() * 0.8)
    }).collect();
    hump.push(Vec2::new(0.0, -1.0));
    b.part(fill(&hump, |p| 0.55 + 0.45 * (p.y / 9.0).clamp(0.0, 1.0)), &soil, Vec3::ZERO, mound);
    let mut rng = Rng(0x0D16_6E25);
    for _ in 0..7 {
        let x = rng.range(-MOUND * 0.85, MOUND * 0.85);
        let top = (1.0 - (x / MOUND).powi(2)).sqrt() * 8.0;
        let at = Vec2::new(x, rng.range(0.5, top.max(1.0)));
        b.part(fill(&rock_pts(&mut rng, rng_r(x), 6), |_| 0.9), &soil_dark, at.extend(0.01), mound);
    }

    vec![(velvet, MOLE), (dark, MOLE_DARK), (claw, CLAW)]
}

/// Pebble size by position (a little variety without another random draw).
fn rng_r(x: f32) -> f32 {
    1.0 + (x * 1.7).sin().abs() * 1.2
}

/// A wreck katydid standing, feet at y ≈ -7: thin long legs and antennae, a
/// sabre at the back, and over it all a dead-leaf wing (veined, bitten, moldy)
/// that opens on clear hind wings to glide.
pub(super) fn katydid(b: &mut Builder, materials: &mut Assets<ColorMaterial>, rig: Entity) -> Mats {
    let pale = materials.add(KATYDID);
    let leaf = materials.add(LEAF);
    let vein = materials.add(LEAF_VEIN);
    let eye = materials.add(EYE);
    let clear = materials.add(ColorMaterial { color: KATY_WING, alpha_mode: AlphaMode2d::Blend, ..default() });
    let dark = materials.add(scale_color(KATYDID, 0.55));

    let fan = |front: bool| {
        let n = if front { 9.0 } else { 12.0 };
        let mut pts = vec![Vec2::ZERO];
        pts.extend((0..=10).map(|i| Vec2::from_angle(PI * (0.55 + 0.4 * i as f32 / 10.0)) * n));
        fill(&pts, |p| 0.6 + 0.4 * (p.length() / 12.0))
    };
    let back = b.bone(Vec3::new(0.0, 2.0, -0.4), rig, BoneKind::Wing { front: false });
    b.part(fan(false), &clear, Vec3::ZERO, back);

    let far = b.bone(Vec3::new(-0.5, -0.5, -0.3), rig, BoneKind::Sinkhole(Part::Hind { near: false }));
    hind_leg(b, &dark, far, Vec2::new(-12.0, 4.5), Vec2::new(-8.0, -7.0), 1.5);
    for (pair, x) in [(0, 4.0), (1, 1.0)] {
        let bone = b.bone(Vec3::new(x + 0.8, -1.5, -0.25), rig, BoneKind::Leg { pair, near: false });
        b.part(stripe(&[Vec2::ZERO, Vec2::new(2.0, -2.6), Vec2::new(3.0, -5.5)], 0.6, 0.35), &dark, Vec3::ZERO, bone);
    }

    // Body under the leaf, with the sabre.
    let abdomen = b.bone(Vec3::new(-2.0, -1.0, 0.0), rig, BoneKind::Abdomen);
    b.part(fill(&ellipse_pts(6.0, 2.4, 16), |p| 0.6 + 0.4 * ((p.y + 2.4) / 4.8)), &pale, Vec3::new(-4.0, 0.0, 0.0), abdomen);
    b.part(stripe(&[Vec2::new(-9.0, 0.0), Vec2::new(-12.5, 1.0), Vec2::new(-15.5, 3.0)], 1.3, 0.3), &dark, Vec3::new(0.0, 0.0, 0.01), abdomen);
    b.part(fill(&ellipse_pts(3.5, 2.6, 14), |p| 0.6 + 0.4 * ((p.y + 2.6) / 5.2)), &pale, Vec3::new(2.5, 0.0, 0.05), rig);

    for (pair, x) in [(0, 4.0), (1, 1.0)] {
        let bone = b.bone(Vec3::new(x, -1.5, 0.2), rig, BoneKind::Leg { pair, near: true });
        b.part(stripe(&[Vec2::ZERO, Vec2::new(2.0, -2.6), Vec2::new(3.0, -5.7)], 0.7, 0.35), &pale, Vec3::ZERO, bone);
    }
    let near = b.bone(Vec3::new(-1.0, -0.5, 0.25), rig, BoneKind::Sinkhole(Part::Hind { near: true }));
    hind_leg(b, &pale, near, Vec2::new(-13.0, 4.0), Vec2::new(-9.0, -6.6), 1.7);

    let head = b.bone(Vec3::new(5.8, 0.6, 0.3), rig, BoneKind::Head);
    b.part(fill(&ellipse_pts(2.2, 2.9, 14), |p| 0.6 + 0.4 * ((p.y + 2.9) / 5.8)), &pale, Vec3::ZERO, head);
    b.part(fill(&ellipse_pts(0.8, 0.9, 8), |_| 1.0), &eye, Vec3::new(0.6, 1.0, 0.02), head);
    for (offset, z) in [(0.0, -0.05), (1.6, 0.04)] {
        let bone = b.bone(Vec3::new(0.8, 2.6, z), head, BoneKind::Antenna { offset });
        let path = [Vec2::ZERO, Vec2::new(5.0, 6.0), Vec2::new(14.0, 9.0), Vec2::new(26.0, 8.0), Vec2::new(38.0, 3.0)];
        b.part(stroke(&smooth_open(&path, 6), |t| 0.45 - 0.3 * t, 1.0, 1.0, |_| 1.0), &dark, Vec3::ZERO, bone);
    }

    let front = b.bone(Vec3::new(0.0, 2.2, 0.35), rig, BoneKind::Wing { front: true });
    b.part(fan(true), &clear, Vec3::ZERO, front);

    // The leaf: a forewing over everything, its midrib, veins, a bite out of
    // its edge and blotches of mold.
    let teg = b.bone(Vec3::new(5.0, 1.8, 0.45), rig, BoneKind::Sinkhole(Part::Tegmen));
    let outline = smooth_closed(
        &[
            Vec2::new(1.0, 0.4),
            Vec2::new(-3.0, 3.2),
            Vec2::new(-9.0, 4.3),
            Vec2::new(-11.5, 4.1),
            Vec2::new(-12.5, 2.8),
            Vec2::new(-14.0, 3.9),
            Vec2::new(-17.5, 2.8),
            Vec2::new(-21.0, 0.2),
            Vec2::new(-17.0, -1.7),
            Vec2::new(-10.0, -2.5),
            Vec2::new(-3.0, -1.7),
        ],
        4,
    );
    b.part(fill(&outline, |p| 0.65 + 0.35 * ((p.y + 2.5) / 6.8).clamp(0.0, 1.0)), &leaf, Vec3::ZERO, teg);
    b.part(stripe(&[Vec2::new(0.5, 0.4), Vec2::new(-10.0, 0.8), Vec2::new(-20.5, 0.2)], 0.5, 0.2), &vein, Vec3::new(0.0, 0.0, 0.01), teg);
    for x in [-4.0f32, -8.0, -12.0, -16.0] {
        b.part(stripe(&[Vec2::new(x, 0.7), Vec2::new(x - 2.2, 3.0)], 0.25, 0.15), &vein, Vec3::new(0.0, 0.0, 0.01), teg);
        b.part(stripe(&[Vec2::new(x - 1.0, 0.6), Vec2::new(x - 3.0, -1.6)], 0.25, 0.15), &vein, Vec3::new(0.0, 0.0, 0.01), teg);
    }
    let mut rng = Rng(0x7EAF_0D1D);
    for _ in 0..3 {
        let at = Vec2::new(rng.range(-17.0, -4.0), rng.range(-1.0, 2.5));
        b.part(fill(&rock_pts(&mut rng, 1.0, 7), |_| 1.0), &vein, at.extend(0.02), teg);
    }

    vec![(pale, KATYDID), (leaf, LEAF), (vein, LEAF_VEIN), (dark, scale_color(KATYDID, 0.55))]
}

/// Parts of these bodies torn off a fresh corpse: (velocity, spin, what it bleeds).
pub(super) fn torn(part: Part, f: f32, blood: Option<Fluid>) -> Option<(Vec2, f32, Option<Fluid>)> {
    match part {
        Part::Hind { near: true } => Some((Vec2::new(-f * 190.0, 430.0), 9.0, blood)),
        Part::Shovel => Some((Vec2::new(f * 170.0, 400.0), -8.0, blood)),
        Part::Tegmen => Some((Vec2::new(-f * 140.0, 380.0), 6.0, None)),
        Part::Hind { near: false } | Part::Mound => None,
    }
}

/// Pose one bone of a sunken street mob.
pub(super) fn animate(kind: BoneKind, m: &Mob, tf: &mut Transform, tt: f32, dt: f32) {
    let mind = &m.sinkhole;
    let (dead, landed) = match m.state {
        State::Dead { t, landed } => (Some(t), landed),
        _ => (None, false),
    };
    let windup = m.windup();
    let shiver = windup * (tt * 60.0).sin() * 0.06;
    let moves = m.spec.moves;
    let still = mind.disguised() && dead.is_none();
    let flying = moves == Moves::BandSweep || mind.gliding();
    let aloft = m.airborne || flying;
    let wall = mind.clinging();
    let twitch = |rate: f32, amp: f32, phase: f32| dead.map_or(0.0, |td| (tt * rate + phase).sin() * amp * (1.0 - td / 4.0).max(0.1));
    match kind {
        BoneKind::Rig => {
            let s = m.spec.scale;
            tf.scale.x += (m.facing * s - tf.scale.x) * (1.0 - (-18.0 * dt).exp());
            if moves == Moves::Tunnel {
                // Rises out of the soil (and sinks back into it).
                let e = mind.emerge();
                tf.scale.y = s * e.max(0.001);
                tf.translation.y = -m.spec.foot * (1.0 - e);
            } else {
                let crouch = if matches!(moves, Moves::WallKick | Moves::LeafGlide) { 0.2 * windup } else { 0.0 };
                tf.scale.y = s * (1.0 - crouch);
            }
            let angle = match m.state {
                State::Dead { t, .. } => PI * if landed { 1.0 } else { (t * 3.0).min(1.0) },
                State::Hurt(h) => 0.3 * (1.0 - h / 0.3).max(0.0) * if flying { 1.0 } else { -0.5 },
                // Feet against the wall.
                _ if wall != 0.0 => wall * PI / 2.0,
                _ if still => -0.04,
                _ if moves == Moves::BandSweep => (m.vel * m.facing).to_angle().clamp(-0.5, 0.5) * 0.5 + (tt * 3.0).sin() * 0.05 + shiver,
                _ if aloft => (m.vel * m.facing).to_angle().clamp(-0.7, 0.7) * 0.6,
                State::Windup(_) => -0.12 * windup + shiver,
                _ if mind.chirping() => (tt * 70.0).sin() * 0.015,
                _ => (tt * 9.0).sin() * 0.01,
            };
            tf.rotation = Quat::from_rotation_z(angle);
        }
        BoneKind::Head => {
            let bob = if dead.is_some() {
                -0.3
            } else if still {
                0.0
            } else {
                (tt * 6.0).sin() * 0.05 - 0.1 * windup
            };
            tf.rotation = Quat::from_rotation_z(bob);
        }
        BoneKind::Antenna { offset } => {
            let angle = if dead.is_some() {
                -0.5 + twitch(12.0, 0.08, offset)
            } else if still {
                // Laid back along the leaf.
                -0.35 + offset * 0.04
            } else if moves == Moves::BandSweep {
                -0.3 + (tt * 20.0 + offset).sin() * 0.08
            } else {
                (tt * 2.6 + offset).sin() * 0.16 + windup * (tt * 25.0 + offset).sin() * 0.2
            };
            tf.rotation = Quat::from_rotation_z(angle);
        }
        BoneKind::Leg { pair, near } => {
            let base = [0.15, 0.0, -0.15][pair.min(2)];
            let angle = if dead.is_some() {
                base + twitch(30.0, 0.5, pair as f32 * 1.7)
            } else if still {
                base * 0.5 - 0.1
            } else if aloft || wall != 0.0 {
                // Dangling, swinging with the flight.
                0.3 * (m.vel.x * m.facing / 300.0).clamp(-1.0, 1.0) + (tt * 4.0 + pair as f32).sin() * 0.1
            } else {
                // Tripod gait (shovels: the mole cricket just paddles).
                let group = if (pair == 1) != near { 0.0 } else { PI };
                let amp = ((m.vel.x.abs() / 150.0).min(1.0) * 0.5).max(windup * 0.3);
                base + (m.stride + group).sin() * amp
            };
            tf.rotation = Quat::from_rotation_z(angle);
        }
        BoneKind::Wing { front } => {
            let lag = if front { 0.0 } else { 1.4 };
            if dead.is_some() {
                tf.rotation = Quat::from_rotation_z(-0.3);
                tf.scale.y = 1.0;
            } else if moves == Moves::BandSweep {
                // A locust's heavy beat; harder holding the line.
                let flap = (tt * (55.0 + 25.0 * windup) + lag).sin();
                tf.rotation = Quat::from_rotation_z(flap * 0.7);
                tf.scale.y = 0.35 + 0.65 * flap.abs();
            } else if mind.gliding() {
                // Spread wide, fluttering a little.
                tf.rotation = Quat::from_rotation_z(0.15 + (tt * 9.0 + lag).sin() * 0.08);
                tf.scale.y = 1.0;
            } else {
                // Folded under the leaf (unfurling in the windup).
                tf.rotation = Quat::from_rotation_z(-0.2);
                tf.scale.y = (0.6 * windup).max(0.01);
            }
        }
        BoneKind::Abdomen => {
            let pump = if dead.is_some() { 0.2 } else if still { 0.0 } else { (tt * 4.0).sin() * 0.06 };
            tf.rotation = Quat::from_rotation_z(pump);
        }
        BoneKind::Sinkhole(Part::Hind { near }) => {
            let angle = match dead {
                Some(_) => 0.5 + twitch(25.0, 0.4, near as u8 as f32),
                // Folded tight against the wall, ready to kick.
                None if wall != 0.0 => -0.35,
                None if still => -0.1,
                // Trailing behind in flight.
                None if moves == Moves::BandSweep => 0.25 + (tt * 3.0 + m.seed).sin() * 0.05,
                // Kicked out.
                None if aloft => 0.8,
                None => -0.3 * windup + shiver,
            };
            tf.rotation = Quat::from_rotation_z(angle);
        }
        BoneKind::Sinkhole(Part::Shovel) => {
            let angle = match m.state {
                State::Dead { .. } => 0.4 + twitch(20.0, 0.5, 0.0),
                // Snapping.
                State::Attack(_) => (tt * 28.0).sin() * 0.6,
                // Scraping at the dirt, dazed.
                State::Recover(_) | State::Hurt(_) => (tt * 3.0).sin() * 0.25 - 0.2,
                _ => (tt * 9.0).sin() * 0.35,
            };
            tf.rotation = Quat::from_rotation_z(angle);
        }
        BoneKind::Sinkhole(Part::Tegmen) => {
            let angle = match m.state {
                State::Dead { .. } => -0.3,
                // The cricket's song: wings up, buzzing.
                _ if mind.chirping() => -0.4 + (tt * 90.0).sin() * 0.07,
                // The leaf thrown up off the back.
                _ if mind.gliding() => -0.9 + (tt * 9.0).sin() * 0.05,
                State::Windup(_) if moves == Moves::LeafGlide => -0.5 * windup + shiver,
                _ => 0.0,
            };
            tf.rotation = Quat::from_rotation_z(angle);
        }
        BoneKind::Sinkhole(Part::Mound) => {
            // Shows while the mole cricket is under it: heaves before it bursts, ripples as it tunnels.
            let k = if dead.is_some() { 0.0 } else { 1.0 - mind.emerge() };
            let swell = 1.0 + 0.35 * windup;
            let crawl = if m.vel.x.abs() > 5.0 { (tt * 18.0).sin() * 0.12 } else { (tt * 1.5).sin() * 0.03 };
            tf.translation.x = windup * (tt * 50.0).sin() * 1.5;
            tf.scale = Vec3::new(k * swell * (1.0 - crawl * 0.5), k * swell * (1.0 + crawl), 1.0);
        }
        _ => {}
    }
}
