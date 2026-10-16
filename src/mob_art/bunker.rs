//! Bodies of the bunker's beetles (behavior in `mobs/bunker.rs`): the
//! wireworm reared out of a crack in the floor, the bombardier with its
//! turret tail, the horned ramhorn, the sexton and its ball of carrion (also
//! drawn loose, rolling).

use std::f32::consts::PI;

use bevy::prelude::*;

use super::{BoneKind, Builder, Z};
use crate::{
    gore::Fluid,
    mobs::{
        bunker::{BALL_AT, BALL_R, REACH},
        Mob, MobKind, State,
    },
    shapes::*,
};

type Mats = Vec<(Handle<ColorMaterial>, Color)>;

const WIRE_GOLD: Color = Color::srgb(0.66, 0.47, 0.20);
const WIRE_DARK: Color = Color::srgb(0.27, 0.17, 0.07);
const SHINE: Color = Color::srgb(0.92, 0.88, 0.78);
const CONCRETE: Color = Color::srgb(0.47, 0.46, 0.44);
const HOLE: Color = Color::srgb(0.05, 0.05, 0.05);
const BOMB_HEAD: Color = Color::srgb(0.62, 0.36, 0.14);
const BOMB_SHELL: Color = Color::srgb(0.11, 0.13, 0.17);
const BOMB_SPOT: Color = Color::srgb(0.72, 0.58, 0.34);
const BEETLE_BLACK: Color = Color::srgb(0.08, 0.07, 0.07);
const LEG_DARK: Color = Color::srgb(0.16, 0.12, 0.09);
const RHINO: Color = Color::srgb(0.24, 0.15, 0.09);
const RHINO_RIM: Color = Color::srgb(0.66, 0.50, 0.38);
const SEXTON_BAND: Color = Color::srgb(0.80, 0.46, 0.12);
const ROT: Color = Color::srgb(0.30, 0.22, 0.19);
const FLESH: Color = Color::srgb(0.46, 0.17, 0.15);
const BONE: Color = Color::srgb(0.84, 0.80, 0.70);

/// Wireworm body: segments under the head (drawing units), the first one's center below the head.
const SEGMENTS: usize = 9;
const SEG: f32 = 3.6;
const SEG_TOP: f32 = 3.2;

#[derive(Clone, Copy)]
pub(super) enum Part {
    /// Wireworm: its head (mandibles up) and the hard segments hanging under it.
    WormHead,
    Segment(usize),
    /// Wireworm: a mandible.
    Jaw { left: bool },
    /// Wireworm: the cracked floor over it (stays at floor level).
    Rubble,
    /// Bombardier: the turret tip of its abdomen.
    Nozzle,
    /// Ramhorn: the great horn.
    Horn,
    /// Sexton: the ball it rolls.
    Ball,
}

fn bone(b: &mut Builder, pos: Vec3, parent: Entity, part: Part) -> Entity {
    b.bone(pos, parent, BoneKind::Bunker(part))
}

/// Six legs on `Leg` bones (the common tripod gait): (hip x, path) per pair, at `hip_y`.
fn legs(b: &mut Builder, rig: Entity, near_mat: &Handle<ColorMaterial>, far_mat: &Handle<ColorMaterial>, hip_y: f32, pairs: [(f32, [Vec2; 3]); 3], width: f32) {
    for near in [false, true] {
        for (pair, (x, path)) in pairs.iter().enumerate() {
            let (mat, z, dx) = if near { (near_mat, -0.05, 0.0) } else { (far_mat, -0.3, 1.2) };
            let leg = b.bone(Vec3::new(x + dx, hip_y, z), rig, BoneKind::Leg { pair, near });
            b.part(stroke(&smooth_open(path, 4), |t| width * (1.0 - 0.5 * t), 1.0, 0.8, |_| 1.0), mat, Vec3::ZERO, leg);
        }
    }
}

/// Shading lighter toward the top of a shape `lo..hi` high.
fn dome(lo: f32, hi: f32) -> impl Fn(Vec2) -> f32 {
    move |p| 0.5 + 0.5 * ((p.y - lo) / (hi - lo)).clamp(0.0, 1.0)
}

/// A click-beetle larva standing up out of the floor, head (mandibles up) at
/// the center, hard ringed segments hanging down; a heap of concrete crumbs
/// around the hole at the floor.
pub(super) fn wireworm(b: &mut Builder, materials: &mut Assets<ColorMaterial>, rig: Entity) -> Mats {
    let gold = materials.add(WIRE_GOLD);
    let dark = materials.add(WIRE_DARK);
    let shine = materials.add(SHINE);
    let rubble = materials.add(CONCRETE);
    let hole = materials.add(HOLE);
    let s = MobKind::Wireworm.spec().scale;

    // The floor: a black crack and crumbs of concrete, in world units on the root.
    let floor = bone(b, Vec3::new(0.0, 0.0, 0.3), b.owner, Part::Rubble);
    b.part(fill(&ellipse_pts(5.5 * s, 1.5 * s, 14), |_| 1.0), &hole, Vec3::new(0.0, 0.4 * s, 0.0), floor);
    let mut rng = Rng(0x0b0b_1e57);
    let crumbs: Vec<Vec<Vec2>> = (0..7)
        .map(|i| {
            let x = (-11.0 + i as f32 * 3.7 + rng.range(-0.8, 0.8)) * s;
            let r = rng.range(1.1, 2.3) * s * if (2..5).contains(&i) { 0.7 } else { 1.0 };
            rock_pts(&mut rng, r, 7).into_iter().map(|p| p + Vec2::new(x, r * 0.5)).collect()
        })
        .collect();
    b.part(blobs(&crumbs, |p| 0.6 + 0.4 * (p.y / (3.0 * s)).clamp(0.0, 1.0)), &rubble, Vec3::new(0.0, 0.0, 0.02), floor);

    // Body: wiry segments, tapering, with a ring and a glint each; tiny legs under the head.
    for i in 0..SEGMENTS {
        let w = 2.8 - i as f32 * 0.09;
        let h = SEG * 0.5 + 0.3;
        let seg = bone(b, Vec3::new(0.0, -SEG_TOP - i as f32 * SEG, -0.01 * i as f32), rig, Part::Segment(i));
        let outline = smooth_closed(
            &[Vec2::new(w, 0.0), Vec2::new(w * 0.9, h), Vec2::new(0.0, h + 0.3), Vec2::new(-w * 0.9, h), Vec2::new(-w, 0.0), Vec2::new(-w * 0.9, -h), Vec2::new(0.0, -h - 0.3), Vec2::new(w * 0.9, -h)],
            3,
        );
        b.part(fill(&outline, move |p| 0.45 + 0.55 * (1.0 - ((p.x + w * 0.3) / w).abs().min(1.0))), &gold, Vec3::ZERO, seg);
        b.part(stroke(&[Vec2::new(-w, h - 0.2), Vec2::new(w, h - 0.2)], |_| 0.5, 1.0, 1.0, |_| 1.0), &dark, Vec3::new(0.0, 0.0, 0.01), seg);
        b.part(fill(&ellipse_pts(0.35, h * 0.6, 6), |_| 1.0), &shine, Vec3::new(-w * 0.35, 0.0, 0.02), seg);
        if i < 3 {
            for sx in [-1.0f32, 1.0] {
                let leg = [Vec2::new(sx * w, 0.0), Vec2::new(sx * (w + 1.6), -0.8), Vec2::new(sx * (w + 2.0), -2.0)];
                b.part(stroke(&leg, |t| 0.6 - 0.3 * t, 1.0, 1.0, |_| 1.0), &dark, Vec3::new(0.0, 0.0, -0.02), seg);
            }
        }
    }
    let head = bone(b, Vec3::new(0.0, 0.0, 0.1), rig, Part::WormHead);
    let plate = smooth_closed(
        &[Vec2::new(3.2, 0.0), Vec2::new(2.6, 2.2), Vec2::new(0.0, 2.8), Vec2::new(-2.6, 2.2), Vec2::new(-3.2, 0.0), Vec2::new(-2.4, -2.0), Vec2::new(0.0, -2.4), Vec2::new(2.4, -2.0)],
        4,
    );
    b.part(fill(&plate, dome(-2.4, 2.8)), &dark, Vec3::ZERO, head);
    // Blind: two pale pits.
    for x in [-1.2f32, 1.2] {
        b.part(fill(&ellipse_pts(0.45, 0.35, 6), |_| 0.8), &shine, Vec3::new(x, 0.6, 0.01), head);
        let feeler = [Vec2::new(x * 1.6, 2.2), Vec2::new(x * 2.3, 3.6)];
        b.part(stroke(&feeler, |t| 0.5 - 0.2 * t, 1.0, 1.0, |_| 1.0), &gold, Vec3::new(0.0, 0.0, -0.01), head);
    }
    for left in [true, false] {
        let sx = if left { -1.0 } else { 1.0 };
        let jaw = bone(b, Vec3::new(sx * 1.1, 2.2, 0.02), head, Part::Jaw { left });
        let hook = smooth_open(&[Vec2::ZERO, Vec2::new(sx * 1.0, 1.8), Vec2::new(sx * 0.3, 3.4), Vec2::new(-sx * 0.9, 4.0)], 4);
        b.part(stroke(&hook, |t| 1.3 - 1.0 * t, 1.0, 0.8, |_| 1.0), &dark, Vec3::ZERO, jaw);
    }
    vec![(gold, WIRE_GOLD), (dark, WIRE_DARK)]
}

/// A bombardier beetle, facing +X, feet at y ≈ -6: a dark domed shell with pale
/// spots, a rusty head and pronotum, long antennae, and a turret tail.
pub(super) fn bombardier(b: &mut Builder, materials: &mut Assets<ColorMaterial>, rig: Entity) -> Mats {
    let shell = materials.add(BOMB_SHELL);
    let head_mat = materials.add(BOMB_HEAD);
    let spot = materials.add(BOMB_SPOT);
    let leg = materials.add(LEG_DARK);
    let black = materials.add(BEETLE_BLACK);
    let glint = materials.add(SHINE);
    legs(
        b,
        rig,
        &head_mat,
        &black,
        -1.5,
        [
            (4.0, [Vec2::ZERO, Vec2::new(2.5, -2.4), Vec2::new(5.0, -4.6)]),
            (0.5, [Vec2::ZERO, Vec2::new(0.6, -2.6), Vec2::new(1.2, -4.6)]),
            (-3.5, [Vec2::ZERO, Vec2::new(-2.6, -2.4), Vec2::new(-6.0, -4.6)]),
        ],
        1.3,
    );
    // The turret tail, under the back of the shell.
    let nozzle = bone(b, Vec3::new(-9.5, 0.0, -0.1), rig, Part::Nozzle);
    let tail = smooth_open(&[Vec2::ZERO, Vec2::new(-2.5, -0.4), Vec2::new(-4.5, 0.4)], 4);
    b.part(stroke(&tail, |t| 3.0 - 1.6 * t, 1.0, 0.7, |_| 1.0), &head_mat, Vec3::ZERO, nozzle);
    b.part(fill(&ellipse_pts(0.6, 0.8, 8), |_| 1.0), &black, Vec3::new(-4.6, 0.4, 0.01), nozzle);

    let outline = smooth_closed(
        &[
            Vec2::new(7.0, 0.5),
            Vec2::new(5.0, 4.5),
            Vec2::new(0.0, 6.2),
            Vec2::new(-6.0, 5.6),
            Vec2::new(-10.0, 3.0),
            Vec2::new(-11.0, 0.0),
            Vec2::new(-9.0, -2.2),
            Vec2::new(-3.0, -2.8),
            Vec2::new(4.0, -2.4),
            Vec2::new(7.0, -1.2),
        ],
        5,
    );
    b.part(fill(&outline, dome(-2.8, 6.2)), &shell, Vec3::ZERO, rig);
    // Pale blotches and a jagged band, the seam, a shine.
    let blotches: Vec<Vec<Vec2>> = [(Vec2::new(-1.5, 3.8), 1.6), (Vec2::new(-7.0, 2.6), 1.3), (Vec2::new(-4.0, 0.2), 1.1)]
        .into_iter()
        .map(|(c, r)| ellipse_pts(r * 1.2, r, 10).into_iter().map(|p| p + c).collect())
        .collect();
    b.part(blobs(&blotches, |_| 0.9), &spot, Vec3::new(0.0, 0.0, 0.02), rig);
    let seam = smooth_open(&[Vec2::new(4.0, 4.6), Vec2::new(-4.0, 5.2), Vec2::new(-10.0, 2.4)], 4);
    b.part(stroke(&seam, |_| 0.5, 1.0, 1.0, |_| 1.0), &black, Vec3::new(0.0, 0.0, 0.03), rig);
    let shine = smooth_open(&[Vec2::new(2.0, 5.0), Vec2::new(-3.0, 5.6), Vec2::new(-7.0, 4.4)], 4);
    b.part(stroke(&shine, |t| 0.9 * (PI * t).sin(), 1.0, 1.0, |_| 1.0), &glint, Vec3::new(0.0, 0.0, 0.04), rig);
    // Pronotum.
    let pronotum = smooth_closed(&[Vec2::new(4.0, -1.0), Vec2::new(4.5, 3.6), Vec2::new(7.5, 4.2), Vec2::new(9.5, 2.0), Vec2::new(9.0, -1.2)], 4);
    b.part(fill(&pronotum, dome(-1.2, 4.2)), &head_mat, Vec3::new(0.0, 0.0, 0.1), rig);

    let head = b.bone(Vec3::new(9.5, 0.4, 0.15), rig, BoneKind::Head);
    b.part(fill(&ellipse_pts(2.4, 2.0, 12), dome(-2.0, 2.0)), &head_mat, Vec3::ZERO, head);
    b.part(fill(&ellipse_pts(0.7, 0.8, 8), |_| 1.0), &black, Vec3::new(0.8, 0.6, 0.01), head);
    for sy in [1.0f32, -1.0] {
        let jaw = smooth_open(&[Vec2::new(1.6, -0.8), Vec2::new(3.4, -0.6 - 0.5 * sy), Vec2::new(3.8, -1.6)], 3);
        b.part(stroke(&jaw, |t| 0.9 - 0.5 * t, 1.0, 1.0, |_| 1.0), &black, Vec3::new(0.0, 0.0, 0.02 * sy), head);
    }
    for (offset, z) in [(0.0, -0.05), (1.3, 0.04)] {
        let ant = b.bone(Vec3::new(1.4, 1.2, z), head, BoneKind::Antenna { offset });
        let path = smooth_open(&[Vec2::ZERO, Vec2::new(4.0, 3.0), Vec2::new(9.0, 4.5), Vec2::new(13.0, 3.0)], 5);
        // Beaded.
        b.part(stroke(&path, |t| 0.8 - 0.3 * t, 1.0, 1.0, |t| 0.75 + 0.25 * (t * 40.0).sin()), &leg, Vec3::ZERO, ant);
    }
    vec![(shell, BOMB_SHELL), (head_mat, BOMB_HEAD), (spot, BOMB_SPOT)]
}

/// A rhinoceros beetle, facing +X, feet at y ≈ -7: a heavy glossy dome, a
/// thick pronotum with a short spur, a great curved horn, chipped and gouged.
pub(super) fn ramhorn(b: &mut Builder, materials: &mut Assets<ColorMaterial>, rig: Entity) -> Mats {
    let body = materials.add(RHINO);
    let dark = materials.add(BEETLE_BLACK);
    let rim = materials.add(RHINO_RIM);
    let leg = materials.add(LEG_DARK);
    legs(
        b,
        rig,
        &leg,
        &dark,
        -1.0,
        [
            (6.0, [Vec2::ZERO, Vec2::new(3.0, -3.0), Vec2::new(6.0, -6.0)]),
            (0.5, [Vec2::ZERO, Vec2::new(0.8, -3.4), Vec2::new(1.4, -6.0)]),
            (-5.0, [Vec2::ZERO, Vec2::new(-3.0, -3.0), Vec2::new(-7.0, -6.0)]),
        ],
        2.2,
    );
    let shell = smooth_closed(
        &[
            Vec2::new(8.0, 0.0),
            Vec2::new(6.0, 5.0),
            Vec2::new(0.0, 7.5),
            Vec2::new(-7.0, 7.0),
            Vec2::new(-12.0, 4.0),
            Vec2::new(-13.5, 0.0),
            Vec2::new(-11.0, -3.0),
            Vec2::new(-3.0, -3.6),
            Vec2::new(5.0, -3.0),
            Vec2::new(8.0, -1.6),
        ],
        5,
    );
    b.part(fill(&shell, dome(-3.6, 7.5)), &body, Vec3::ZERO, rig);
    // A chip bitten out of the back edge, gouges, the seam, a shine.
    b.part(triangles(&[[Vec2::new(-9.5, 5.8), Vec2::new(-11.5, 4.6), Vec2::new(-9.8, 3.8)]]), &dark, Vec3::new(0.0, 0.0, 0.02), rig);
    for (a, c) in [(Vec2::new(-2.0, 6.0), Vec2::new(-6.0, 3.0)), (Vec2::new(-5.0, 6.4), Vec2::new(-8.0, 4.4)), (Vec2::new(2.0, 5.0), Vec2::new(0.5, 2.0))] {
        b.part(stroke(&[a, c], |t| 0.5 * (PI * t).sin() + 0.1, 1.0, 1.0, |_| 1.0), &rim, Vec3::new(0.0, 0.0, 0.03), rig);
    }
    let seam = smooth_open(&[Vec2::new(5.0, 5.4), Vec2::new(-4.0, 6.8), Vec2::new(-13.0, 1.0)], 4);
    b.part(stroke(&seam, |_| 0.6, 1.0, 1.0, |_| 1.0), &dark, Vec3::new(0.0, 0.0, 0.025), rig);
    let shine = smooth_open(&[Vec2::new(3.0, 6.0), Vec2::new(-3.0, 7.0), Vec2::new(-9.0, 5.6)], 4);
    b.part(stroke(&shine, |t| 1.2 * (PI * t).sin(), 1.0, 1.0, |_| 1.0), &rim, Vec3::new(0.0, 0.0, 0.04), rig);
    // Pronotum with a forward spur, pale rim.
    let pronotum = smooth_closed(
        &[Vec2::new(5.0, -1.5), Vec2::new(5.5, 4.5), Vec2::new(9.0, 7.0), Vec2::new(13.5, 8.8), Vec2::new(12.0, 5.0), Vec2::new(13.0, 1.0), Vec2::new(11.0, -1.8)],
        4,
    );
    b.part(fill(&pronotum, dome(-1.8, 8.8)), &dark, Vec3::new(0.0, 0.0, 0.1), rig);
    let rim_line = smooth_open(&[Vec2::new(6.0, 4.6), Vec2::new(9.0, 6.6), Vec2::new(13.0, 8.4)], 4);
    b.part(stroke(&rim_line, |t| 0.3 + 0.6 * (PI * t).sin(), 1.0, 0.8, |_| 1.0), &rim, Vec3::new(0.0, 0.0, 0.11), rig);

    let head = b.bone(Vec3::new(12.5, -0.5, 0.15), rig, BoneKind::Head);
    b.part(fill(&ellipse_pts(2.8, 2.3, 12), dome(-2.3, 2.3)), &dark, Vec3::ZERO, head);
    b.part(fill(&ellipse_pts(0.6, 0.6, 6), |_| 1.0), &rim, Vec3::new(0.6, 0.5, 0.01), head);
    for (offset, z) in [(0.0, -0.05), (1.3, 0.04)] {
        let ant = b.bone(Vec3::new(1.5, -0.8, z), head, BoneKind::Antenna { offset });
        let path = smooth_open(&[Vec2::ZERO, Vec2::new(2.0, 0.8), Vec2::new(3.5, 0.6)], 3);
        b.part(stroke(&path, |t| 0.6 + 0.6 * t, 1.0, 1.0, |_| 1.0), &leg, Vec3::ZERO, ant);
    }
    let horn = bone(b, Vec3::new(1.5, 0.5, 0.02), head, Part::Horn);
    let curve = smooth_open(&[Vec2::ZERO, Vec2::new(4.0, 1.5), Vec2::new(7.5, 4.5), Vec2::new(8.5, 8.5), Vec2::new(7.4, 10.5)], 5);
    b.part(stroke(&curve, |t| 3.4 * (1.0 - t) + 0.4, 1.0, 0.6, |_| 1.0), &dark, Vec3::ZERO, horn);
    let edge = smooth_open(&[Vec2::new(3.0, 2.2), Vec2::new(6.4, 4.8), Vec2::new(7.4, 8.4)], 4);
    b.part(stroke(&edge, |t| 0.6 * (PI * t).sin(), 1.0, 1.0, |_| 1.0), &rim, Vec3::new(0.0, 0.0, 0.01), horn);
    vec![(body, RHINO), (dark, BEETLE_BLACK), (rim, RHINO_RIM)]
}

/// A burying beetle, facing +X, feet at y ≈ -6: black, two jagged pale bands
/// across short wing covers, the tip of the abdomen showing, clubbed
/// antennae, and its ball in front.
pub(super) fn sexton(b: &mut Builder, materials: &mut Assets<ColorMaterial>, rig: Entity) -> Mats {
    let black = materials.add(BEETLE_BLACK);
    let band = materials.add(SEXTON_BAND);
    let leg = materials.add(LEG_DARK);
    let glint = materials.add(SHINE);
    legs(
        b,
        rig,
        &leg,
        &black,
        -1.5,
        [
            (4.0, [Vec2::ZERO, Vec2::new(2.5, -2.6), Vec2::new(5.5, -4.6)]),
            (0.0, [Vec2::ZERO, Vec2::new(0.6, -2.8), Vec2::new(1.0, -4.6)]),
            (-4.0, [Vec2::ZERO, Vec2::new(-2.6, -2.6), Vec2::new(-6.5, -4.6)]),
        ],
        1.4,
    );
    // The bare abdomen behind the short wing covers.
    let rings: Vec<Vec<Vec2>> = (0..3)
        .map(|i| {
            let c = Vec2::new(-7.5 - i as f32 * 1.8, 0.5 - i as f32 * 0.4);
            ellipse_pts(1.6 - i as f32 * 0.25, 2.4 - i as f32 * 0.4, 10).into_iter().map(|p| p + c).collect()
        })
        .collect();
    b.part(blobs(&rings, |p| 0.5 + 0.4 * ((p.y + 2.0) / 4.0).clamp(0.0, 1.0)), &black, Vec3::new(0.0, 0.0, -0.05), rig);
    let shell = smooth_closed(
        &[Vec2::new(6.0, 0.5), Vec2::new(4.5, 4.5), Vec2::new(-1.0, 5.5), Vec2::new(-6.0, 4.5), Vec2::new(-7.5, 1.5), Vec2::new(-7.0, -2.0), Vec2::new(-2.0, -2.8), Vec2::new(4.0, -2.2)],
        5,
    );
    b.part(fill(&shell, dome(-2.8, 5.5)), &black, Vec3::ZERO, rig);
    let jag = |x: f32, top: f32| -> Vec<[Vec2; 3]> {
        let (a, b2, c, d) = (Vec2::new(x + 1.2, top), Vec2::new(x - 1.0, top - 0.2), Vec2::new(x - 0.6, -1.8), Vec2::new(x + 1.6, -1.6));
        let notch = Vec2::new(x + 0.8, (top - 1.8) * 0.5);
        vec![[a, b2, notch], [b2, c, notch], [notch, c, d], [a, notch, d]]
    };
    let mut bands = jag(1.0, 5.2);
    bands.extend(jag(-4.2, 4.6));
    b.part(triangles(&bands), &band, Vec3::new(0.0, 0.0, 0.02), rig);
    let shine = smooth_open(&[Vec2::new(2.5, 4.6), Vec2::new(-2.0, 5.1), Vec2::new(-5.5, 4.0)], 4);
    b.part(stroke(&shine, |t| 0.8 * (PI * t).sin(), 1.0, 1.0, |_| 1.0), &glint, Vec3::new(0.0, 0.0, 0.03), rig);
    let pronotum = smooth_closed(&[Vec2::new(4.0, -1.2), Vec2::new(4.2, 3.4), Vec2::new(6.8, 4.4), Vec2::new(8.8, 2.4), Vec2::new(8.6, -1.4)], 4);
    b.part(fill(&pronotum, dome(-1.4, 4.4)), &black, Vec3::new(0.0, 0.0, 0.1), rig);
    let head = b.bone(Vec3::new(8.8, -0.2, 0.15), rig, BoneKind::Head);
    b.part(fill(&ellipse_pts(2.2, 1.9, 12), dome(-1.9, 1.9)), &black, Vec3::ZERO, head);
    b.part(fill(&ellipse_pts(0.5, 0.6, 6), |_| 1.0), &glint, Vec3::new(0.8, 0.5, 0.01), head);
    for (offset, z) in [(0.0, -0.05), (1.3, 0.04)] {
        let ant = b.bone(Vec3::new(1.2, 1.0, z), head, BoneKind::Antenna { offset });
        let path = [Vec2::ZERO, Vec2::new(2.5, 2.6), Vec2::new(4.0, 3.2)];
        b.part(stroke(&path, |_| 0.5, 1.0, 1.0, |_| 1.0), &leg, Vec3::ZERO, ant);
        // The club, in the band color.
        b.part(fill(&ellipse_pts(1.3, 0.9, 10), |_| 0.9), &band, Vec3::new(4.6, 3.3, 0.01), ant);
    }
    let ball = bone(b, BALL_AT.extend(0.2), rig, Part::Ball);
    carrion_ball(b, materials, ball);
    vec![(black, BEETLE_BLACK), (band, SEXTON_BAND)]
}

/// A ball of rotten meat and rubble, radius `BALL_R`, centered on `parent`:
/// grit, chunks of concrete, scraps of flesh, a rib sticking out, a twist of wire.
fn carrion_ball(b: &mut Builder, materials: &mut Assets<ColorMaterial>, parent: Entity) {
    let rot = materials.add(ROT);
    let grit = materials.add(CONCRETE);
    let flesh = materials.add(FLESH);
    let bone_mat = materials.add(BONE);
    let wire = materials.add(BEETLE_BLACK);
    let mut rng = Rng(0x5e47_0b11);
    let lump: Vec<Vec2> = (0..18).map(|i| Vec2::from_angle(i as f32 / 18.0 * 2.0 * PI) * BALL_R * rng.range(0.86, 1.0)).collect();
    b.part(fill(&lump, |p| 0.45 + 0.55 * ((p.y + BALL_R) / (2.0 * BALL_R))), &rot, Vec3::ZERO, parent);
    let chunks: Vec<Vec<Vec2>> = [0.4f32, 1.9, 3.3, 5.0]
        .into_iter()
        .map(|a| {
            let c = Vec2::from_angle(a) * BALL_R * 0.72;
            let r = rng.range(1.8, 2.8);
            rock_pts(&mut rng, r, 6).into_iter().map(|p| p + c).collect()
        })
        .collect();
    b.part(blobs(&chunks, |p| 0.6 + 0.4 * ((p.y + BALL_R) / (2.0 * BALL_R))), &grit, Vec3::new(0.0, 0.0, 0.02), parent);
    let scraps: Vec<Vec<Vec2>> = [1.1f32, 2.6, 4.2, 5.7]
        .into_iter()
        .map(|a| {
            let c = Vec2::from_angle(a) * BALL_R * rng.range(0.3, 0.8);
            ellipse_pts(rng.range(1.2, 2.0), rng.range(0.8, 1.3), 8).into_iter().map(|p| p + c).collect()
        })
        .collect();
    b.part(blobs(&scraps, |_| 0.85), &flesh, Vec3::new(0.0, 0.0, 0.03), parent);
    let rib = smooth_open(&[Vec2::new(1.5, 2.5), Vec2::new(5.5, 5.8), Vec2::new(9.5, 6.4), Vec2::new(11.5, 4.8)], 4);
    b.part(stroke(&rib, |t| 1.3 - 0.8 * t, 1.0, 0.75, |_| 1.0), &bone_mat, Vec3::new(0.0, 0.0, 0.04), parent);
    let twist = smooth_open(&[Vec2::new(-5.0, -2.5), Vec2::new(-9.5, -0.5), Vec2::new(-8.5, 3.0), Vec2::new(-5.5, 2.0), Vec2::new(-7.5, 0.0)], 4);
    b.part(stroke(&twist, |_| 0.45, 1.0, 1.0, |_| 1.0), &wire, Vec3::new(0.0, 0.0, 0.05), parent);
}

/// A loose ball of carrion (shoved by a sexton) at `at`, at the sexton's `scale`.
pub(crate) fn ball(commands: &mut Commands, meshes: &mut Assets<Mesh>, materials: &mut Assets<ColorMaterial>, at: Vec2, scale: f32) -> Entity {
    let root = commands.spawn((Transform::from_translation(at.extend(Z)), Visibility::default())).id();
    let body = commands.spawn((Transform::from_scale(Vec3::new(scale, scale, 1.0)), Visibility::default(), ChildOf(root))).id();
    let mut b = Builder { commands, meshes, owner: root };
    carrion_ball(&mut b, materials, body);
    root
}

/// What a dying bunker mob loses: (velocity, spin, what it bleeds), or nothing.
pub(super) fn torn(part: Part, m: &Mob) -> Option<(Vec2, f32, Option<Fluid>)> {
    let f = m.facing;
    match part {
        Part::WormHead => Some((Vec2::new(f * 150.0, 380.0), 8.0, Some(m.spec.blood))),
        Part::Nozzle => Some((Vec2::new(-f * 160.0, 300.0), -7.0, Some(m.spec.blood))),
        Part::Horn => Some((Vec2::new(f * 190.0, 420.0), 9.0, None)),
        Part::Ball => Some((Vec2::new(f * 140.0, 220.0), -6.0, Some(Fluid::Red))),
        Part::Segment(_) | Part::Jaw { .. } | Part::Rubble => None,
    }
}

/// Poses the bunker kinds' rigs, their own parts, and a dazed ramhorn's legs.
/// False: the bone is left to the common code.
pub(super) fn animate(kind: BoneKind, m: &Mob, tf: &mut Transform, tt: f32, dt: f32) -> bool {
    if !crate::mobs::bunker::owns(m.kind) {
        return false;
    }
    let (dead, landed) = match m.state {
        State::Dead { t, landed } => (Some(t), landed),
        _ => (None, false),
    };
    let windup = m.windup();
    let shiver = windup * (tt * 60.0).sin() * 0.06;
    let s = m.spec.scale;
    // Wireworm: how much of its body (drawing units, down from the head) is above the floor.
    let shown = m.bunker.height(m.spec) / s;
    let exposed = shown > 3.0;
    match kind {
        BoneKind::Rig => {
            tf.scale.x += (m.facing * s - tf.scale.x) * (1.0 - (-18.0 * dt).exp());
            tf.scale.y = s;
            let angle = match (m.kind, m.state) {
                // Ripped out of the floor: lies on its side.
                (MobKind::Wireworm, State::Dead { t, .. }) => -m.facing * PI / 2.0 * if landed { 1.0 } else { (t * 3.0).min(1.0) },
                (_, State::Dead { t, .. }) => PI * if landed { 1.0 } else { (t * 3.0).min(1.0) },
                // Thrashing, reared up; swaying in its hole.
                (MobKind::Wireworm, State::Recover(_)) => (tt * 15.0).sin() * 0.25 * (shown / REACH).min(1.0),
                (MobKind::Wireworm, State::Attack(_)) => 0.0,
                (MobKind::Wireworm, _) => (tt * 2.0).sin() * 0.06 + shiver,
                // Tail up, nose down; shuddering with every pop.
                (MobKind::Bombardier, State::Windup(_)) => -0.14 * windup + shiver,
                (MobKind::Bombardier, State::Attack(_)) => -0.14 + (tt * 50.0).sin() * 0.03,
                (MobKind::Ramhorn, State::Windup(_)) => -0.08 * windup + shiver,
                (MobKind::Ramhorn, State::Attack(_)) => -0.05 + (tt * 30.0).sin() * 0.02,
                (MobKind::Ramhorn, _) if m.bunker.dazed() => (tt * 7.0).sin() * 0.07,
                // Rears back to shove.
                (MobKind::Sexton, State::Windup(_)) => 0.15 * windup + shiver,
                (MobKind::Sexton, State::Attack(_)) => -0.1,
                (_, State::Hurt(h)) => -0.15 * (1.0 - h / 0.3).max(0.0),
                _ => (tt * 9.0).sin() * 0.01,
            };
            // A slash glancing off jerks it up.
            tf.rotation = Quat::from_rotation_z(angle + m.bunker.clank() * 0.25);
        }
        BoneKind::Leg { pair, .. } if m.kind == MobKind::Ramhorn && m.bunker.dazed() && dead.is_none() => {
            tf.rotation = Quat::from_rotation_z((tt * 22.0 + pair as f32 * 2.1).sin() * 0.5);
        }
        BoneKind::Bunker(part) => match part {
            Part::Segment(i) => {
                let bottom = SEG_TOP + i as f32 * SEG + SEG * 0.5;
                let show = dead.is_some() || bottom <= shown + 1.0;
                tf.scale = Vec3::splat(if show { 1.0 } else { 0.0 });
                let wave = if dead.is_none() && exposed { (tt * 6.0 - i as f32 * 0.8).sin() * 0.4 * (i as f32 / SEGMENTS as f32) } else { 0.0 };
                tf.translation.x = wave;
            }
            Part::WormHead => tf.scale = Vec3::splat(if dead.is_some() || exposed { 1.0 } else { 0.0 }),
            Part::Jaw { left } => {
                let open = match m.state {
                    State::Dead { .. } => 0.4,
                    State::Windup(_) => 0.5 * windup,
                    State::Attack(_) => -0.2,
                    State::Recover(_) => 0.2 + (tt * 20.0).sin() * 0.3,
                    _ => 0.1,
                };
                tf.rotation = Quat::from_rotation_z(if left { open } else { -open });
            }
            Part::Rubble => {
                // Stays on the floor (the root is at the head); jiggles as the worm tunnels.
                let dig = (m.vel.x.abs() / 120.0).min(1.0);
                tf.translation.x = (tt * 40.0).sin() * 1.2 * dig + (tt * 70.0).sin() * 1.5 * windup;
                tf.translation.y = -m.bunker.height(m.spec);
                tf.scale = Vec3::splat(if dead.is_some() { 0.0 } else { 1.0 });
            }
            Part::Nozzle => {
                let curl = match m.state {
                    State::Dead { .. } => -0.3,
                    State::Windup(_) => 0.7 * windup,
                    State::Attack(_) => 0.7 + (tt * 45.0).sin() * 0.12,
                    State::Recover(r) => 0.7 * (1.0 - r / m.spec.recover).max(0.0),
                    _ => (tt * 3.0).sin() * 0.05,
                };
                // Tips up and back, toward what's behind it.
                tf.rotation = Quat::from_rotation_z(-curl);
            }
            Part::Horn => tf.rotation = Quat::from_rotation_z(m.bunker.clank() * 0.2),
            Part::Ball => {
                // Grows back from the ground up; pulled in for the shove; rolls as it walks.
                let k = m.bunker.ball();
                tf.scale = Vec3::new(k.max(0.001), k.max(0.001), 1.0);
                tf.translation.x = BALL_AT.x - BALL_R * (1.0 - k) - 3.0 * windup;
                tf.translation.y = BALL_AT.y - BALL_R * (1.0 - k);
                tf.rotation = Quat::from_rotation_z(-m.stride * 1.6);
            }
        },
        _ => return false,
    }
    true
}
