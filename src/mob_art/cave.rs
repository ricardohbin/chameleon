//! Bodies of the cave system's vermin (behavior in `mobs/cave.rs`): the
//! hundredfoot's chain of links laid along its trail, the curlback's armored
//! rings that roll up into a ball, the burrowling growing out of its hole
//! under a heap of rubble, and the broodcoil wrapped around its eggs.

use std::f32::consts::{FRAC_PI_2, PI, TAU};

use bevy::prelude::*;

use super::{BoneKind, Builder};
use crate::{
    gore::Fluid,
    mobs::{
        cave::{self as mind, Gait},
        Mob, Moves, State,
    },
    shapes::*,
};

/// The centipede's kin: the boss's dark plum, the legs paler.
const KIN: Color = Color::srgb(0.20, 0.07, 0.19);
const KIN_LEG: Color = Color::srgb(0.48, 0.20, 0.42);
const KIN_RIM: Color = Color::srgb(0.62, 0.36, 0.56);
/// Pale, blind things born in the rock.
const PALE: Color = Color::srgb(0.80, 0.76, 0.70);
const PALE_DARK: Color = Color::srgb(0.50, 0.44, 0.42);
/// Millipede armor: glossy slate rings with dull rims.
const SLATE: Color = Color::srgb(0.20, 0.19, 0.18);
const SLATE_RIM: Color = Color::srgb(0.56, 0.50, 0.40);
const ROCK: Color = Color::srgb(0.36, 0.34, 0.31);
const HOLE: Color = Color::srgb(0.05, 0.04, 0.04);
const EGG: Color = Color::srgb(0.86, 0.82, 0.66);

/// Curlback rings, their spacing stretched out, and the circle they make rolled up.
const RINGS: usize = 9;
const RING_GAP: f32 = 2.8;
const CURL_R: f32 = 5.4;
/// Angle between two rings on the ball (the head takes one slot).
const DELTA: f32 = TAU / (RINGS as f32 + 1.0);
/// Burrowling body segments, and the height of its head over the ground (drawing units).
const SEGMENTS: usize = 5;
const CROWN: f32 = 27.0;
/// Broodcoil neck links.
const NECK: usize = 6;

#[derive(Clone, Copy)]
pub(super) enum Bone {
    /// Hundredfoot: the head (0) and the links behind it, laid along its trail; their legs.
    Link(usize),
    Feet(usize),
    /// Curlback: the armored rings (stretched out or rolled into a ball), the head.
    Ring(usize),
    Snout,
    /// Burrowling: everything out of the rock (grows out of the hole), its head,
    /// the rubble heaped over it while it travels, the hole, the cracks of the tell.
    Rise,
    Crown,
    Mound,
    Hole,
    Crack,
    /// Broodcoil: the neck from the coil to the head, the head, the eggs.
    Neck(usize),
    Jaws,
    Egg(usize),
}

type Mats = Vec<(Handle<ColorMaterial>, Color)>;

fn bone(b: &mut Builder, pos: Vec3, parent: Entity, part: Bone) -> Entity {
    b.bone(pos, parent, BoneKind::Cave(part))
}

/// A pair of jointed legs hanging from (0, -0.8), the far one darker and behind.
fn legs(b: &mut Builder, near: &Handle<ColorMaterial>, far: &Handle<ColorMaterial>, at: Entity, reach: f32) {
    let path = smooth_open(&[Vec2::new(0.0, -0.8), Vec2::new(-0.9, -0.5 - reach * 0.5), Vec2::new(-0.2, -0.8 - reach)], 3);
    b.part(stroke(&path, |t| 0.8 - 0.4 * t, 1.0, 0.8, |_| 1.0), far, Vec3::new(1.0, 0.0, -0.2), at);
    b.part(stroke(&path, |t| 0.8 - 0.4 * t, 1.0, 0.8, |_| 1.0), near, Vec3::new(0.0, 0.0, 0.05), at);
}

/// Hundredfoot: links are bones of the root (not the rig), placed in world units along its trail.
pub(super) fn hundredfoot(b: &mut Builder, materials: &mut Assets<ColorMaterial>, root: Entity) -> Mats {
    let body = materials.add(KIN);
    let leg = materials.add(KIN_LEG);
    let rim = materials.add(KIN_RIM);
    let dark = materials.add(scale_color(KIN, 0.6));
    for k in 0..=mind::LINKS {
        let link = bone(b, Vec3::new(0.0, 0.0, 0.3 - k as f32 * 0.02), root, Bone::Link(k));
        let feet = bone(b, Vec3::ZERO, link, Bone::Feet(k));
        legs(b, &leg, &dark, feet, 3.2);
        if k == 0 {
            // The head: flat, eyeless, with hooked poison claws and long feelers.
            b.part(fill(&ellipse_pts(3.3, 2.4, 16), |p| 0.6 + 0.4 * ((p.y + 2.4) / 4.8)), &body, Vec3::ZERO, link);
            for (dz, mat) in [(-0.05, &dark), (0.04, &rim)] {
                let claw = smooth_open(&[Vec2::new(2.2, -1.0), Vec2::new(4.6, -1.8), Vec2::new(5.4, -0.2)], 4);
                b.part(stroke(&claw, |t| 1.1 - 0.8 * t, 1.0, 0.8, |_| 1.0), mat, Vec3::new(0.0, 0.0, dz), link);
                let feeler = smooth_open(&[Vec2::new(2.6, 1.0), Vec2::new(6.5, 3.4), Vec2::new(10.5, 2.6), Vec2::new(13.0, 0.5)], 5);
                b.part(stroke(&feeler, |t| 0.6 - 0.35 * t, 1.0, 1.0, |_| 1.0), &leg, Vec3::new(dz * 8.0, 0.0, dz), link);
            }
        } else {
            let r = 3.0 - (k as f32 / mind::LINKS as f32).powi(2) * 0.8;
            b.part(fill(&ellipse_pts(r, 2.0, 14), |p| 0.55 + 0.45 * ((p.y + 2.0) / 4.0)), &body, Vec3::ZERO, link);
            let plate = smooth_open(&[Vec2::new(-r * 0.8, 1.2), Vec2::new(0.0, 1.9), Vec2::new(r * 0.8, 1.2)], 4);
            b.part(stroke(&plate, |t| 0.5 * (PI * t).sin(), 1.0, 1.0, |_| 1.0), &rim, Vec3::new(0.0, 0.0, 0.02), link);
        }
        if k == mind::LINKS {
            // The long trailing legs.
            let tail = smooth_open(&[Vec2::new(-2.0, -0.4), Vec2::new(-6.5, -1.2), Vec2::new(-10.0, 0.6)], 4);
            b.part(stroke(&tail, |t| 0.8 - 0.5 * t, 1.0, 0.8, |_| 1.0), &leg, Vec3::new(0.0, 0.0, -0.05), link);
        }
    }
    vec![(body, KIN), (leg, KIN_LEG), (rim, KIN_RIM)]
}

fn ring_x(k: usize) -> f32 {
    8.5 - k as f32 * RING_GAP
}

/// Curlback, facing +X, feet at y ≈ -4: a row of domed armor rings on short legs.
pub(super) fn curlback(b: &mut Builder, materials: &mut Assets<ColorMaterial>, rig: Entity) -> Mats {
    let slate = materials.add(SLATE);
    let rim = materials.add(SLATE_RIM);
    let leg = materials.add(PALE_DARK);
    let dark = materials.add(scale_color(PALE_DARK, 0.5));
    let gloss = materials.add(Color::WHITE);
    let dome = smooth_closed(
        &[
            Vec2::new(-1.7, -0.9),
            Vec2::new(-1.9, 1.2),
            Vec2::new(-1.0, 2.9),
            Vec2::new(0.6, 3.1),
            Vec2::new(1.7, 2.0),
            Vec2::new(1.9, -0.2),
            Vec2::new(1.2, -1.3),
            Vec2::new(-0.8, -1.4),
        ],
        4,
    );
    for k in 0..RINGS {
        let ring = bone(b, Vec3::new(ring_x(k), 0.0, -0.01 * k as f32), rig, Bone::Ring(k));
        legs(b, &leg, &dark, ring, 2.6);
        let shrink = if k + 2 >= RINGS { 0.8 } else { 1.0 };
        let pts: Vec<Vec2> = dome.iter().map(|p| *p * shrink).collect();
        b.part(fill(&pts, |p| 0.55 + 0.45 * ((p.y + 1.4) / 4.5).clamp(0.0, 1.0)), &slate, Vec3::ZERO, ring);
        let edge = [Vec2::new(-1.7, -0.7), Vec2::new(0.0, -1.0), Vec2::new(1.7, -0.6)];
        b.part(stroke(&edge, |_| 0.6, 1.0, 1.0, |_| 1.0), &rim, Vec3::new(0.0, 0.0, 0.02), ring);
        let shine = smooth_open(&[Vec2::new(-1.0, 2.2), Vec2::new(0.2, 2.7), Vec2::new(1.2, 1.9)], 3);
        b.part(stroke(&shine, |t| 0.5 * (PI * t).sin(), 1.0, 1.0, |_| 1.0), &gloss, Vec3::new(0.0, 0.0, 0.03), ring);
    }
    let head = bone(b, Vec3::new(11.2, -0.5, 0.1), rig, Bone::Snout);
    b.part(fill(&ellipse_pts(2.6, 2.2, 14), |p| 0.6 + 0.4 * ((p.y + 2.2) / 4.4)), &slate, Vec3::ZERO, head);
    for dz in [-0.05f32, 0.04] {
        let feeler = smooth_open(&[Vec2::new(1.4, 1.0), Vec2::new(3.4, 3.4), Vec2::new(5.6, 2.6)], 3);
        b.part(stroke(&feeler, |t| 0.6 - 0.3 * t, 1.0, 1.0, |_| 1.0), &leg, Vec3::new(dz * 10.0, 0.0, dz), head);
    }
    vec![(slate, SLATE), (rim, SLATE_RIM), (leg, PALE_DARK)]
}

/// Burrowling, rising straight up out of the ground (at y = -4): a column of
/// pale segments with legs out both sides, a blind head with hooked fangs.
pub(super) fn burrowling(b: &mut Builder, materials: &mut Assets<ColorMaterial>, rig: Entity) -> Mats {
    let pale = materials.add(PALE);
    let joint = materials.add(PALE_DARK);
    let fang = materials.add(KIN);
    let rock = materials.add(ROCK);
    let hole = materials.add(HOLE);
    let ground = Vec3::new(0.0, -4.0, 0.0);
    let hole_bone = bone(b, ground + Vec3::Z * -0.1, rig, Bone::Hole);
    b.part(fill(&ellipse_pts(7.0, 1.8, 16), |_| 1.0), &hole, Vec3::ZERO, hole_bone);
    let rise = bone(b, ground, rig, Bone::Rise);
    for i in 0..SEGMENTS {
        let y = 3.0 + i as f32 * 4.6;
        let r = 3.7 - i as f32 * 0.15;
        for side in [-1.0f32, 1.0] {
            let leg = smooth_open(&[Vec2::new(side * r * 0.7, y), Vec2::new(side * (r + 2.6), y + 1.6), Vec2::new(side * (r + 3.6), y - 1.0)], 3);
            b.part(stroke(&leg, |t| 0.8 - 0.4 * t, 1.0, 1.0, |_| 1.0), &joint, Vec3::new(0.0, 0.0, -0.1), rise);
        }
        b.part(fill(&ellipse_pts(r, 2.7, 16), |p| 0.6 + 0.4 * (1.0 - (p.x / 4.0).abs())), &pale, Vec3::new(0.0, y, 0.01 * i as f32), rise);
        let seam = [Vec2::new(-r * 0.9, y - 2.1), Vec2::new(0.0, y - 2.6), Vec2::new(r * 0.9, y - 2.1)];
        b.part(stroke(&seam, |_| 0.5, 1.0, 1.0, |_| 1.0), &joint, Vec3::new(0.0, 0.0, 0.06), rise);
    }
    let crown = bone(b, Vec3::new(0.0, CROWN, 0.1), rise, Bone::Crown);
    b.part(fill(&ellipse_pts(4.0, 3.0, 16), |p| 0.6 + 0.4 * ((p.y + 3.0) / 6.0)), &pale, Vec3::ZERO, crown);
    for side in [-1.0f32, 1.0] {
        let hook = smooth_open(&[Vec2::new(side * 1.5, 1.6), Vec2::new(side * 4.6, 4.6), Vec2::new(side * 2.4, 8.2)], 4);
        b.part(stroke(&hook, |t| 1.4 - 1.0 * t, 1.0, 0.8, |_| 1.0), &fang, Vec3::new(0.0, 0.0, 0.02), crown);
        let feeler = smooth_open(&[Vec2::new(side * 1.0, 2.6), Vec2::new(side * 5.0, 9.0), Vec2::new(side * 9.0, 12.0)], 4);
        b.part(stroke(&feeler, |t| 0.6 - 0.35 * t, 1.0, 1.0, |_| 1.0), &joint, Vec3::new(0.0, 0.0, -0.02), crown);
    }
    // The rubble it pushes up while it travels under the floor.
    let mound = bone(b, ground + Vec3::Z * 0.4, rig, Bone::Mound);
    let mut rng = Rng(0x5eed_cafe);
    let mut heap = vec![ellipse_pts(8.0, 3.2, 16)];
    for x in [-6.0f32, -2.0, 3.0, 6.5] {
        heap.push(rock_pts(&mut rng, 2.2, 7).into_iter().map(|p| p + Vec2::new(x, 2.0 + rng.range(-0.5, 1.5))).collect());
    }
    b.part(blobs(&heap, |p| 0.55 + 0.45 * (p.y / 5.0).clamp(0.0, 1.0)), &rock, Vec3::ZERO, mound);
    let crack = bone(b, ground + Vec3::Z * 0.5, rig, Bone::Crack);
    for (dx, dy) in [(9.0f32, 0.6), (6.0, 3.0), (-9.0, 0.4), (-5.5, 3.2), (1.0, 4.0)] {
        let line = [Vec2::ZERO, Vec2::new(dx * 0.5, dy * 0.4 + 0.6), Vec2::new(dx, dy)];
        b.part(stroke(&line, |t| 0.9 - 0.6 * t, 1.0, 1.0, |_| 1.0), &hole, Vec3::ZERO, crack);
    }
    vec![(pale, PALE), (joint, PALE_DARK), (fang, KIN)]
}

/// Broodcoil, facing +X, lying at y ≈ -5: its body coiled round a clutch of
/// eggs (the far half of the coil behind them), the neck rising from the front.
pub(super) fn broodcoil(b: &mut Builder, materials: &mut Assets<ColorMaterial>, rig: Entity) -> Mats {
    let body = materials.add(KIN);
    let leg = materials.add(KIN_LEG);
    let rim = materials.add(KIN_RIM);
    let dark = materials.add(scale_color(KIN, 0.6));
    let egg = materials.add(EGG);
    let gloss = materials.add(Color::WHITE);
    // The coil: plates round an ellipse on the floor, smaller toward the tail.
    const COIL: usize = 16;
    for i in 0..COIL {
        let a = i as f32 / COIL as f32 * TAU * 0.95;
        let at = Vec2::new(-2.0, -1.5) + Vec2::new(a.cos() * 12.0, a.sin() * 3.6);
        let k = 1.0 - i as f32 / COIL as f32 * 0.45;
        let front = a.sin() < 0.0;
        let z = if front { 0.2 } else { -0.2 } + i as f32 * 0.001;
        if front {
            let foot = smooth_open(&[at + Vec2::new(0.0, -0.8), at + Vec2::new(-0.8, -2.4), at + Vec2::new(-0.3, -3.6)], 3);
            b.part(stroke(&foot, |t| 0.8 - 0.4 * t, 1.0, 1.0, |_| 1.0), &leg, Vec3::new(0.0, 0.0, z - 0.01), rig);
        }
        let mat = if front { &body } else { &dark };
        b.part(fill(&ellipse_pts(3.2 * k, 2.4 * k, 14), |p| 0.55 + 0.45 * ((p.y + 2.4) / 4.8)), mat, (at).extend(z), rig);
        let plate = smooth_open(&[Vec2::new(-2.4 * k, 1.3 * k), Vec2::new(0.0, 2.1 * k), Vec2::new(2.4 * k, 1.3 * k)], 4);
        b.part(stroke(&plate, |t| 0.5 * (PI * t).sin(), 1.0, 1.0, |_| 1.0), &rim, (at).extend(z + 0.005), rig);
    }
    for (i, (x, y)) in [(-7.0f32, 0.0), (-3.5, 1.2), (0.0, 0.2), (3.5, 1.0), (-1.8, 3.2)].into_iter().enumerate() {
        let e = bone(b, Vec3::new(x, y, 0.0 + i as f32 * 0.002), rig, Bone::Egg(i));
        b.part(fill(&ellipse_pts(1.8, 2.3, 14), |p| 0.6 + 0.4 * ((p.y + 2.3) / 4.6)), &egg, Vec3::ZERO, e);
        b.part(fill(&ellipse_pts(0.5, 0.7, 6), |_| 1.0), &gloss, Vec3::new(-0.6, 1.0, 0.01), e);
    }
    for k in 0..NECK {
        let n = bone(b, mind::NECK_BASE.extend(0.3 + k as f32 * 0.01), rig, Bone::Neck(k));
        legs(b, &leg, &dark, n, 2.6);
        b.part(fill(&ellipse_pts(2.9, 2.2, 14), |p| 0.55 + 0.45 * ((p.y + 2.2) / 4.4)), &body, Vec3::ZERO, n);
    }
    let jaws = bone(b, Vec3::new(13.0, 8.0, 0.4), rig, Bone::Jaws);
    b.part(fill(&ellipse_pts(3.8, 2.8, 16), |p| 0.6 + 0.4 * ((p.y + 2.8) / 5.6)), &body, Vec3::ZERO, jaws);
    for (dz, mat) in [(-0.05, &dark), (0.04, &rim)] {
        let claw = smooth_open(&[Vec2::new(2.4, -1.2), Vec2::new(5.6, -2.2), Vec2::new(6.6, 0.0)], 4);
        b.part(stroke(&claw, |t| 1.5 - 1.1 * t, 1.0, 0.8, |_| 1.0), mat, Vec3::new(0.0, 0.0, dz), jaws);
        let feeler = smooth_open(&[Vec2::new(2.8, 1.2), Vec2::new(7.0, 4.6), Vec2::new(12.0, 4.0), Vec2::new(15.0, 1.0)], 5);
        b.part(stroke(&feeler, |t| 0.7 - 0.4 * t, 1.0, 1.0, |_| 1.0), &leg, Vec3::new(dz * 8.0, 0.0, dz), jaws);
    }
    vec![(body, KIN), (leg, KIN_LEG), (rim, KIN_RIM), (egg, EGG)]
}

/// The rig of a cave mob: facing, the ball's spin and lift, the burrowling's sinking, corpses.
pub(super) fn rig(m: &Mob, tf: &mut Transform, windup: f32, tt: f32, dt: f32) {
    let Moves::Cave(gait) = m.spec.moves else { return };
    let s = m.spec.scale;
    tf.scale.x += (m.facing * s - tf.scale.x) * (1.0 - (-18.0 * dt).exp());
    tf.scale.y = s;
    let shiver = windup * (tt * 60.0).sin() * 0.06;
    let (lift, angle) = match (gait, m.state) {
        (Gait::Crawl, _) => (0.0, 0.0),
        (Gait::Roll, State::Dead { t, landed }) => (0.0, PI * if landed { 1.0 } else { (t * 3.0).min(1.0) }),
        (Gait::Roll, _) => ((mind::BALL - 4.0) * s * smooth(m.cave.curl), m.cave.roll + shiver),
        // Falls over.
        (Gait::Burrow, State::Dead { t, .. }) => (0.0, -FRAC_PI_2 * m.facing * (t * 4.0).min(1.0)),
        // Its center stands higher out of the rock: the drawing stays on the ground.
        (Gait::Burrow, _) => (-m.cave.rise * mind::RAISE, 0.0),
        (Gait::Coil, State::Hurt(h)) => (0.0, 0.08 * (1.0 - h / 0.3).max(0.0)),
        (Gait::Coil, _) => (0.0, 0.0),
    };
    tf.translation.y = lift;
    tf.rotation = Quat::from_rotation_z(angle);
}

fn smooth(x: f32) -> f32 {
    let x = x.clamp(0.0, 1.0);
    x * x * (3.0 - 2.0 * x)
}

pub(super) fn animate(part: Bone, m: &Mob, mtf: &Transform, tf: &mut Transform, windup: f32, tt: f32) {
    let dead = match m.state {
        State::Dead { t, .. } => Some(t),
        _ => None,
    };
    let shiver = windup * (tt * 60.0).sin();
    match part {
        Bone::Link(k) => {
            let s = m.spec.scale;
            if let Some(td) = dead {
                // Curls up belly-up, the links bending one after the other.
                let bend = (td * 2.0).min(1.0) * 0.55;
                let mut h = if m.facing > 0.0 { PI } else { 0.0 };
                let mut p = Vec2::ZERO;
                for _ in 0..k {
                    p += Vec2::from_angle(h) * mind::LINK;
                    h += bend;
                }
                tf.translation.x = p.x;
                tf.translation.y = p.y;
                tf.rotation = Quat::from_rotation_z(h + PI);
                tf.scale = Vec3::new(s, -s, 1.0);
                return;
            }
            let (at, n) = m.cave.links[k];
            let ahead = if k == 0 { at + (at - m.cave.links[1].0) } else { m.cave.links[k - 1].0 };
            let d = (ahead - at).normalize_or(Vec2::X * m.facing);
            // Legs to the rock.
            let up = if d.perp().dot(n) >= 0.0 { 1.0 } else { -1.0 };
            let mut off = at - mtf.translation.truncate();
            if k == 0 {
                // Rears off the rock in the tell.
                off += n * (windup * 3.0 + shiver * 0.6);
            }
            tf.translation.x = off.x;
            tf.translation.y = off.y;
            tf.rotation = Quat::from_rotation_z(d.to_angle());
            tf.scale = Vec3::new(s, s * up, 1.0);
        }
        Bone::Feet(k) => {
            // A ripple running down the legs.
            let angle = match dead {
                Some(td) => (tt * 30.0 + k as f32 * 1.3).sin() * 0.5 * (1.0 - td / 5.0).max(0.1),
                None => (m.stride * 3.0 - k as f32 * 0.9).sin() * 0.4,
            };
            tf.rotation = Quat::from_rotation_z(angle);
        }
        Bone::Ring(k) => {
            let c = smooth(m.cave.curl);
            let wave = (m.stride - k as f32 * 0.8).sin() * 0.25 * (1.0 - c);
            let line = Vec2::new(ring_x(k), wave);
            let ball = Vec2::from_angle(FRAC_PI_2 + (k as f32 + 1.0) * DELTA) * CURL_R;
            let p = line.lerp(ball, c);
            tf.translation.x = p.x;
            tf.translation.y = p.y;
            tf.rotation = Quat::from_rotation_z((k as f32 + 1.0) * DELTA * c + if dead.is_some() { (tt * 20.0 + k as f32).sin() * 0.05 } else { 0.0 });
        }
        Bone::Snout => {
            // Tucked into the middle of the ball.
            let c = smooth(m.cave.curl);
            let p = Vec2::new(11.2, -0.5).lerp(Vec2::Y * CURL_R * 0.5, c);
            tf.translation.x = p.x;
            tf.translation.y = p.y;
            let bob = if dead.is_some() { -0.3 } else { (tt * 6.0).sin() * 0.06 };
            tf.rotation = Quat::from_rotation_z(bob - c * 0.8);
            tf.scale = Vec3::splat(1.0 - 0.3 * c);
        }
        Bone::Rise => {
            let r = if dead.is_some() { 1.0 } else { m.cave.rise };
            tf.scale = Vec3::new(0.5 + 0.5 * r, r.max(0.001), 1.0);
            let sway = match m.state {
                State::Attack(_) => -0.15,
                State::Dead { .. } => 0.0,
                _ => (tt * 2.2).sin() * 0.12 * r,
            };
            tf.rotation = Quat::from_rotation_z(sway);
        }
        Bone::Crown => {
            let snap = match m.state {
                State::Attack(_) => 0.0,
                State::Dead { .. } => 0.4,
                _ => (tt * 7.0).sin() * 0.15,
            };
            tf.rotation = Quat::from_rotation_z(snap);
        }
        Bone::Mound => {
            // Heaped over it while it is in the rock; shoved about as it travels, swelling in the tell.
            let k = if dead.is_some() { 0.0 } else { (1.0 - m.cave.rise * 2.0).max(0.0) };
            let push = (m.vel.x.abs() / 150.0).min(1.0);
            let wobble = (tt * 18.0).sin() * 0.12 * push;
            tf.scale = Vec3::new(k * (1.0 + wobble + 0.2 * windup), k * (0.8 + 0.4 * push + 0.7 * windup + shiver * 0.1), 1.0);
        }
        Bone::Hole => {
            let k = if dead.is_some() { 0.0 } else { (m.cave.rise * 3.0).min(1.0) };
            tf.scale = Vec3::new(k.max(0.001), 1.0, 1.0);
        }
        Bone::Crack => {
            let k = if dead.is_some() { 0.0 } else { windup.max(m.cave.rise) };
            tf.scale = Vec3::new(k.max(0.001), k.max(0.001), 1.0);
        }
        Bone::Neck(k) => {
            let (p, d) = neck(m, (k as f32 + 1.0) / (NECK as f32 + 1.0), tt);
            tf.translation.x = p.x;
            tf.translation.y = p.y;
            tf.rotation = Quat::from_rotation_z(d.to_angle());
            // Stretched along a long lash.
            let reach = (mind::coil_head(m, tt) - mind::NECK_BASE).length() / (NECK as f32 + 1.0);
            tf.scale = Vec3::new((reach / 4.5).max(1.0), 1.0, 1.0);
        }
        Bone::Jaws => {
            let (p, d) = neck(m, 1.0, tt);
            tf.translation.x = p.x;
            tf.translation.y = p.y;
            // Gapes in the tell.
            tf.rotation = Quat::from_rotation_z(d.to_angle() + 0.3 * windup + shiver * 0.05);
        }
        Bone::Egg(i) => {
            if i < m.cave.hatched as usize {
                // An empty shell.
                tf.scale = Vec3::new(1.0, 0.35, 1.0);
            } else {
                let throb = if dead.is_some() { 0.0 } else { (tt * 2.0 + i as f32 * 1.7).sin() * 0.04 };
                tf.scale = Vec3::new(1.0 - throb, 1.0 + throb, 1.0);
            }
        }
    }
}

/// A point along a broodcoil's neck (0 = at the coil, 1 = the head) and the way it runs there:
/// a curve arched up over the floor, straight when it lashes.
fn neck(m: &Mob, u: f32, tt: f32) -> (Vec2, Vec2) {
    let (a, h) = (mind::NECK_BASE, mind::coil_head(m, tt));
    let straight = matches!(m.state, State::Attack(_)) as i32 as f32;
    let c = (a + h) / 2.0 + Vec2::new(-2.0, 7.0 * (1.0 - straight));
    let p = a * (1.0 - u) * (1.0 - u) + c * 2.0 * u * (1.0 - u) + h * u * u;
    let d = (c - a) * 2.0 * (1.0 - u) + (h - c) * 2.0 * u;
    (p, d.normalize_or(Vec2::X))
}

/// What a fresh corpse loses: (velocity, spin, bleeds) for the parts that fly off.
pub(super) fn tear(part: Bone, m: &Mob) -> Option<(Vec2, f32, Option<Fluid>)> {
    let f = m.facing;
    let blood = Some(m.spec.blood);
    Some(match part {
        // The tail end comes off.
        Bone::Link(k) if k + 1 >= mind::LINKS => (Vec2::new(-f * (120.0 + 50.0 * (k + 1 - mind::LINKS) as f32), 380.0), 9.0, blood),
        Bone::Ring(k) if k + 2 >= RINGS => (Vec2::new(-f * (110.0 + 60.0 * (k + 2 - RINGS) as f32), 340.0), -7.0, blood),
        Bone::Snout => (Vec2::new(f * 160.0, 380.0), 6.0, blood),
        Bone::Crown => (Vec2::new(f * 150.0, 440.0), 7.0, blood),
        Bone::Jaws => (Vec2::new(f * 170.0, 400.0), -6.0, blood),
        // The clutch spills.
        Bone::Egg(i) if i >= m.cave.hatched as usize => (Vec2::new((i as f32 - 2.0) * 90.0, 240.0 + 30.0 * i as f32), 4.0, blood),
        _ => return None,
    })
}
