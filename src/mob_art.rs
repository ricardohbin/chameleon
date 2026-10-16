//! What the mobs look like: small, simple kin of the bosses, drawn in code
//! (`shapes.rs`) and animated through a few bones. Also tears parts off
//! fresh corpses. Behavior lives in `mobs.rs`.

use std::f32::consts::PI;

mod mall;
mod catacombs;
pub(crate) mod bunker;
mod sewer;
mod sinkhole;
mod dam;

use bevy::{prelude::*, sprite_render::AlphaMode2d};

mod treatment;
mod subway;

pub(crate) use subway::{KNOT_BALL, KNOT_FOOT};
mod cave;

use crate::{
    boss,
    mobs::{self, Mob, MobKind, MobStuff, Moves, State},
    shapes::*,
};

const ROACH_BROWN: Color = Color::srgb(0.55, 0.33, 0.16);
const SEWER_BROWN: Color = Color::srgb(0.34, 0.24, 0.14);
const ROACH_DARK: Color = Color::srgb(0.24, 0.14, 0.07);
const ROACH_RIM: Color = Color::srgb(0.86, 0.70, 0.44);
const ROACH_LEG: Color = Color::srgb(0.42, 0.26, 0.12);
const WASP_YELLOW: Color = Color::srgb(0.86, 0.68, 0.14);
const WASP_BLACK: Color = Color::srgb(0.08, 0.07, 0.06);
const WING: Color = Color::srgba(0.90, 0.92, 0.95, 0.30);
const GRUB_CREAM: Color = Color::srgb(0.86, 0.82, 0.68);
const GRUB_HEAD: Color = Color::srgb(0.30, 0.22, 0.12);
const EYE: Color = Color::srgb(0.08, 0.06, 0.05);
const FLY_BODY: Color = Color::srgb(0.16, 0.20, 0.18);
const FLY_EYE: Color = Color::srgb(0.45, 0.10, 0.08);
const FLEA_BROWN: Color = Color::srgb(0.36, 0.20, 0.10);
const PAPER: Color = Color::srgb(0.62, 0.58, 0.50);
const PAPER_DARK: Color = Color::srgb(0.34, 0.31, 0.27);
const RAT_FUR: Color = Color::srgb(0.40, 0.35, 0.30);
const RAT_SKIN: Color = Color::srgb(0.66, 0.50, 0.48);
const SPIDER_BODY: Color = Color::srgb(0.10, 0.09, 0.09);
const SPIDER_MARK: Color = Color::srgb(0.62, 0.10, 0.08);
const SILK: Color = Color::srgba(0.92, 0.92, 0.90, 0.55);
/// The spider boss's blue, on its brood.
const SPIDERLING_MARK: Color = Color::srgb_u8(0x7f, 0x9f, 0xd6);
/// A nest hangs this far (in its drawing units) below the ceiling.
pub const NEST_STALK: f32 = 22.0;
/// Mobs are drawn between the player (10) and the bosses (12).
const Z: f32 = 11.0;
/// Grub segments (radii, head to tail); they rest on a belly line this far below the center.
const GRUB_RADII: [(f32, f32); 6] = [(3.2, 4.2), (3.3, 4.4), (3.3, 4.3), (3.1, 3.9), (2.8, 3.3), (2.3, 2.5)];
const GRUB_BELLY: f32 = 4.4;

#[derive(Component)]
pub struct MobBone {
    owner: Entity,
    kind: BoneKind,
}

#[derive(Clone, Copy)]
enum BoneKind {
    Rig,
    Head,
    Antenna { offset: f32 },
    /// `pair`: 0 = front, 1 = middle, 2 = hind.
    Leg { pair: usize, near: bool },
    Wing { front: bool },
    Abdomen,
    Segment(usize),
    /// A nest swinging from its stalk (pivot at the ceiling).
    Sway,
    Tail,
    /// A spider's silk, from its body up to the ceiling (a unit-long strip, scaled).
    Thread,
    /// The mall's mantises and moths (`mob_art/mall.rs`).
    Mall(mall::Bone),
    /// The treatment plant mobs' own bones (`mob_art/treatment.rs`).
    Treatment(treatment::Bone),
    /// The subway's vermin (`mob_art/subway.rs`).
    Subway(subway::Bone),
    /// The catacomb kin's own bones (`mob_art/catacombs.rs`).
    Crypt(catacombs::Bone),
    /// The bunker beetles' own parts (`mob_art/bunker.rs`).
    Bunker(bunker::Part),
    /// The old sewer's mobs (`mob_art/sewer.rs`).
    Sewer(sewer::Part),
    /// The sunken street's own bones (`sinkhole.rs`).
    Sinkhole(sinkhole::Part),
    /// The broken dam's mobs (`mob_art/dam.rs`).
    Dam(dam::Bone),
    /// The cave system's vermin (`mob_art/cave.rs`).
    Cave(cave::Bone),
}

struct Builder<'a, 'w, 's> {
    commands: &'a mut Commands<'w, 's>,
    meshes: &'a mut Assets<Mesh>,
    owner: Entity,
}

impl Builder<'_, '_, '_> {
    fn part(&mut self, mesh: Mesh, mat: &Handle<ColorMaterial>, pos: Vec3, parent: Entity) {
        let mesh = self.meshes.add(mesh);
        self.commands.spawn((Mesh2d(mesh), MeshMaterial2d(mat.clone()), Transform::from_translation(pos), ChildOf(parent)));
    }

    fn bone(&mut self, pos: Vec3, parent: Entity, kind: BoneKind) -> Entity {
        self.commands
            .spawn((MobBone { owner: self.owner, kind }, Transform::from_translation(pos), Visibility::default(), ChildOf(parent)))
            .id()
    }
}

/// How thick the black outline around every mob is, in world units: tiny gray
/// mobs read against the gray world.
const OUTLINE: f32 = 2.2;
/// The outline sits this far behind its part (local z), so it is under every part of the mob.
const OUTLINE_Z: f32 = -1.0;

/// Gives every newly spawned mob a thick black outline: each opaque part gets a
/// black copy behind it, stamped in 8 directions (`OUTLINE` world units out).
/// Translucent parts (wings, silk) are left alone.
pub fn outline(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    mut black: Local<Option<Handle<ColorMaterial>>>,
    mobs: Query<Entity, Added<Mob>>,
    children: Query<&Children>,
    parts: Query<(&Mesh2d, &MeshMaterial2d<ColorMaterial>, &GlobalTransform)>,
) {
    let black = black.get_or_insert_with(|| materials.add(Color::BLACK)).clone();
    for mob in &mobs {
        for part in children.iter_descendants(mob) {
            let Ok((mesh, mat, gt)) = parts.get(part) else { continue };
            if materials.get(&mat.0).is_none_or(|m| m.alpha_mode == AlphaMode2d::Blend || m.color.alpha() < 1.0) {
                continue;
            }
            let Some(mesh) = meshes.get(&mesh.0).and_then(|m| stamped(m, OUTLINE / gt.scale().truncate().abs().max_element().max(1e-3))) else {
                continue;
            };
            commands.spawn((Mesh2d(meshes.add(mesh)), MeshMaterial2d(black.clone()), Transform::from_xyz(0.0, 0.0, OUTLINE_Z), ChildOf(part)));
        }
    }
}

/// `mesh` copied 8 times around, `r` out (its own units): its silhouette, grown by `r`.
fn stamped(mesh: &Mesh, r: f32) -> Option<Mesh> {
    let pos = mesh.attribute(Mesh::ATTRIBUTE_POSITION)?.as_float3()?;
    let indices: Vec<u32> = mesh.indices()?.iter().map(|i| i as u32).collect();
    let (mut out_pos, mut out_idx) = (Vec::new(), Vec::new());
    for k in 0..8 {
        let d = Vec2::from_angle(k as f32 * PI / 4.0) * r;
        let base = out_pos.len() as u32;
        out_pos.extend(pos.iter().map(|p| [p[0] + d.x, p[1] + d.y, 0.0]));
        out_idx.extend(indices.iter().map(|i| base + i));
    }
    let n = out_pos.len();
    Some(
        Mesh::new(bevy::render::render_resource::PrimitiveTopology::TriangleList, bevy::asset::RenderAssetUsages::default())
            .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, out_pos)
            .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, vec![[0.0, 0.0, 1.0]; n])
            .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, vec![[0.0, 0.0]; n])
            .with_inserted_indices(bevy::mesh::Indices::U32(out_idx)),
    )
}

/// Spawn a mob of `kind` centered at `at` (`parent`: the nest that released it).
pub fn spawn(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<ColorMaterial>,
    kind: MobKind,
    at: Vec2,
    parent: Option<Entity>,
) -> Entity {
    let root = commands.spawn((Transform::from_translation(at.extend(Z)), Visibility::default())).id();
    let mut b = Builder { commands, meshes, owner: root };
    let rig = b.bone(Vec3::ZERO, root, BoneKind::Rig);
    let s = kind.spec().scale;
    b.commands.entity(rig).insert(Transform::from_scale(Vec3::new(s, s, 1.0)));
    let mats = match kind {
        MobKind::Roachling => roach(&mut b, materials, rig, false),
        MobKind::SewerRoach => roach(&mut b, materials, rig, true),
        MobKind::Fly => fly(&mut b, materials, rig),
        MobKind::Flea => flea(&mut b, materials, rig),
        // Soldiers are drones standing guard (wings folded).
        MobKind::Drone | MobKind::Soldier => drone(&mut b, materials, rig),
        MobKind::Grub => grub(&mut b, materials, rig),
        MobKind::Nest => nest(&mut b, materials, rig),
        MobKind::Rat => rat(&mut b, materials, rig),
        MobKind::Spider => spider(&mut b, materials, rig, root),
        MobKind::Spiderling => spiderling(&mut b, materials, rig),
        MobKind::MantisNymph => mall::mantis(&mut b, materials, rig, false),
        MobKind::MantisHatchling => mall::mantis(&mut b, materials, rig, true),
        MobKind::GhostMantis => mall::ghost(&mut b, materials, rig),
        MobKind::NeonMoth => mall::moth(&mut b, materials, rig),
        MobKind::Ootheca => mall::ootheca(&mut b, materials, rig),
        MobKind::Naiad => treatment::naiad(&mut b, materials, rig, root),
        MobKind::Siphon => treatment::siphon(&mut b, materials, rig),
        MobKind::SumpLeech => treatment::sump_leech(&mut b, materials, rig),
        MobKind::PlagueTick => subway::tick(&mut b, materials, rig),
        MobKind::BallastMole => subway::mole(&mut b, materials, rig),
        MobKind::RatKnot => subway::knot(&mut b, materials, rig),
        MobKind::CryptJumper => catacombs::jumper(&mut b, materials, rig, root),
        MobKind::GraveTrapdoor => catacombs::trapdoor(&mut b, materials, rig, root),
        MobKind::OssuaryWolf => catacombs::wolf(&mut b, materials, rig),
        MobKind::Bonestrider => catacombs::strider(&mut b, materials, rig, root),
        MobKind::Wireworm => bunker::wireworm(&mut b, materials, rig),
        MobKind::Bombardier => bunker::bombardier(&mut b, materials, rig),
        MobKind::Ramhorn => bunker::ramhorn(&mut b, materials, rig),
        MobKind::Sexton => bunker::sexton(&mut b, materials, rig),
        MobKind::BileLeech => sewer::leech(&mut b, materials, rig),
        MobKind::GulletToad => sewer::toad(&mut b, materials, rig),
        MobKind::PlagueBloat => sewer::bloat(&mut b, materials, rig),
        MobKind::DrainNewt => sewer::newt(&mut b, materials, rig),
        MobKind::Locust => sinkhole::locust(&mut b, materials, rig),
        MobKind::RubbleCricket => sinkhole::cricket(&mut b, materials, rig),
        MobKind::MoleCricket => sinkhole::mole_cricket(&mut b, materials, rig, root, kind.spec().foot),
        MobKind::WreckKatydid => sinkhole::katydid(&mut b, materials, rig),
        MobKind::GalleryLouse | MobKind::SeepLeech | MobKind::SpillwayCrab | MobKind::RubbleHermit => dam::body(&mut b, materials, rig, kind),
        MobKind::Hundredfoot => cave::hundredfoot(&mut b, materials, root),
        MobKind::Curlback => cave::curlback(&mut b, materials, rig),
        MobKind::Burrowling => cave::burrowling(&mut b, materials, rig),
        MobKind::Broodcoil => cave::broodcoil(&mut b, materials, rig),
    };
    commands.entity(root).insert(Mob::new(kind, at, mats, parent));
    root
}

/// A cockroach, facing +X, feet at y ≈ -6.8. The sewer kind is darker, with a banded back.
fn roach(b: &mut Builder, materials: &mut Assets<ColorMaterial>, rig: Entity, sewer: bool) -> Vec<(Handle<ColorMaterial>, Color)> {
    let base = if sewer { SEWER_BROWN } else { ROACH_BROWN };
    let brown = materials.add(base);
    let dark = materials.add(ROACH_DARK);
    let rim = materials.add(ROACH_RIM);
    let leg = materials.add(ROACH_LEG);
    let eye = materials.add(EYE);
    let glint = materials.add(Color::WHITE);

    // Legs: three pairs, the far side darker and behind the body.
    let paths = [
        [Vec2::ZERO, Vec2::new(2.5, -2.8), Vec2::new(5.5, -5.6)],
        [Vec2::ZERO, Vec2::new(0.6, -3.0), Vec2::new(1.0, -5.6)],
        [Vec2::ZERO, Vec2::new(-2.6, -2.8), Vec2::new(-6.5, -5.6)],
    ];
    let hips = [5.0, 0.5, -4.5];
    for near in [false, true] {
        for pair in 0..3 {
            let (mat, z, dx) = if near { (&leg, -0.05, 0.0) } else { (&dark, -0.3, 1.2) };
            let bone = b.bone(Vec3::new(hips[pair] + dx, -1.4, z), rig, BoneKind::Leg { pair, near });
            b.part(stroke(&smooth_open(&paths[pair], 4), |t| 1.4 - 0.7 * t, 1.0, 0.8, |_| 1.0), mat, Vec3::ZERO, bone);
        }
    }

    // Body: a flat oval with cerci at the back.
    let body = smooth_closed(
        &[
            Vec2::new(11.0, 0.5),
            Vec2::new(9.5, 3.0),
            Vec2::new(5.0, 4.5),
            Vec2::new(-3.0, 4.8),
            Vec2::new(-9.0, 4.0),
            Vec2::new(-13.0, 1.5),
            Vec2::new(-14.0, -0.5),
            Vec2::new(-11.0, -2.5),
            Vec2::new(-3.0, -3.0),
            Vec2::new(5.0, -2.6),
            Vec2::new(9.5, -1.5),
        ],
        5,
    );
    b.part(fill(&body, |p| 0.55 + 0.45 * ((p.y + 3.0) / 8.0).clamp(0.0, 1.0)), &brown, Vec3::ZERO, rig);
    b.part(
        triangles(&[
            [Vec2::new(-13.0, 0.5), Vec2::new(-13.5, -1.0), Vec2::new(-18.0, -1.0)],
            [Vec2::new(-12.5, -0.5), Vec2::new(-13.0, -2.0), Vec2::new(-17.0, -3.0)],
        ]),
        &dark,
        Vec3::new(0.0, 0.0, -0.05),
        rig,
    );
    // The seam between the wing covers, and a shine.
    let seam = smooth_open(&[Vec2::new(4.0, 4.2), Vec2::new(-5.0, 3.6), Vec2::new(-13.0, 0.8)], 4);
    b.part(stroke(&seam, |_| 0.7, 1.0, 1.0, |_| 1.0), &dark, Vec3::new(0.0, 0.0, 0.02), rig);
    let shine = smooth_open(&[Vec2::new(2.0, 3.2), Vec2::new(-4.0, 3.4), Vec2::new(-9.0, 2.4)], 4);
    b.part(stroke(&shine, |t| 1.0 * (PI * t).sin(), 1.0, 1.0, |_| 1.0), &rim, Vec3::new(0.0, 0.0, 0.03), rig);
    if sewer {
        // Grimy bands across the back.
        let bands: Vec<[Vec2; 3]> = [-1.0, -5.0, -9.0]
            .into_iter()
            .flat_map(|x: f32| {
                let h = 4.6 - (x.abs() * 0.12);
                let (a, b, c, d) = (Vec2::new(x + 0.8, h), Vec2::new(x - 0.8, h), Vec2::new(x - 0.4, -2.4), Vec2::new(x + 1.2, -2.4));
                [[a, b, c], [a, c, d]]
            })
            .collect();
        b.part(triangles(&bands), &dark, Vec3::new(0.0, 0.0, 0.025), rig);
    }

    // Pronotum shield over the head, with a pale rim.
    let shield = smooth_closed(
        &[
            Vec2::new(3.0, 1.0),
            Vec2::new(4.5, 4.5),
            Vec2::new(8.0, 5.5),
            Vec2::new(11.0, 4.0),
            Vec2::new(12.5, 0.8),
            Vec2::new(11.0, -0.8),
            Vec2::new(7.0, 0.0),
        ],
        5,
    );
    b.part(fill(&shield, |p| 0.6 + 0.4 * (p.y / 5.5).clamp(0.0, 1.0)), &dark, Vec3::new(0.0, 0.0, 0.35), rig);
    let rim_line = smooth_open(&[Vec2::new(4.2, 3.8), Vec2::new(8.0, 5.3), Vec2::new(12.2, 1.2)], 5);
    b.part(stroke(&rim_line, |t| 0.4 + 1.0 * (PI * t).sin(), 1.0, 0.8, |_| 1.0), &rim, Vec3::new(0.0, 0.0, 0.36), rig);

    // Head: small, beady eye, whip antennae.
    let head = b.bone(Vec3::new(11.5, -1.5, 0.32), rig, BoneKind::Head);
    b.part(fill(&ellipse_pts(2.6, 2.4, 14), |p| 0.7 + 0.3 * (p.y / 2.4)), &dark, Vec3::ZERO, head);
    b.part(fill(&ellipse_pts(0.9, 1.3, 10), |_| 1.0), &eye, Vec3::new(0.8, 0.6, 0.02), head);
    b.part(fill(&ellipse_pts(0.3, 0.35, 6), |_| 1.0), &glint, Vec3::new(1.0, 1.1, 0.03), head);
    for (offset, z) in [(0.0, -0.05), (1.3, 0.04)] {
        let bone = b.bone(Vec3::new(1.5, 1.2, z), head, BoneKind::Antenna { offset });
        let path = smooth_open(&[Vec2::ZERO, Vec2::new(5.0, 3.5), Vec2::new(11.0, 7.0), Vec2::new(17.0, 7.5), Vec2::new(22.0, 5.0)], 6);
        b.part(stroke(&path, |t| 0.9 - 0.5 * t, 1.0, 1.0, |_| 1.0), &leg, Vec3::ZERO, bone);
    }

    vec![(brown, base), (dark, ROACH_DARK), (rim, ROACH_RIM), (leg, ROACH_LEG)]
}

/// A carrion fly, facing +X: fat dark body, huge eyes, blurred wings.
fn fly(b: &mut Builder, materials: &mut Assets<ColorMaterial>, rig: Entity) -> Vec<(Handle<ColorMaterial>, Color)> {
    let body = materials.add(FLY_BODY);
    let eye = materials.add(FLY_EYE);
    let wing = materials.add(ColorMaterial { color: WING, alpha_mode: AlphaMode2d::Blend, ..default() });
    let shine = materials.add(Color::WHITE);
    let wing_mesh = || {
        let pts: Vec<Vec2> = ellipse_pts(4.5, 1.6, 14).into_iter().map(|p| Vec2::from_angle(0.5).rotate(p + Vec2::new(-4.0, 0.0))).collect();
        fill(&pts, |_| 1.0)
    };
    let back = b.bone(Vec3::new(-0.5, 2.4, -0.3), rig, BoneKind::Wing { front: false });
    b.part(wing_mesh(), &wing, Vec3::ZERO, back);
    for (pair, x) in [(0, 1.5), (2, -1.0)] {
        let bone = b.bone(Vec3::new(x, -2.0, -0.2), rig, BoneKind::Leg { pair, near: true });
        let path = smooth_open(&[Vec2::ZERO, Vec2::new(-0.8, -2.2), Vec2::new(0.4, -3.6)], 3);
        b.part(stroke(&path, |t| 0.8 - 0.4 * t, 1.0, 1.0, |_| 1.0), &body, Vec3::ZERO, bone);
    }
    let abdomen = b.bone(Vec3::new(-2.5, -0.4, 0.0), rig, BoneKind::Abdomen);
    b.part(fill(&ellipse_pts(3.6, 2.6, 16), |p| 0.6 + 0.4 * ((p.y + 2.6) / 5.2)), &body, Vec3::new(-1.5, 0.0, 0.0), abdomen);
    b.part(fill(&ellipse_pts(1.4, 0.5, 8), |_| 1.0), &shine, Vec3::new(-1.0, 1.6, 0.01), abdomen);
    b.part(fill(&ellipse_pts(3.0, 2.7, 16), |p| 0.6 + 0.4 * ((p.y + 2.7) / 5.4)), &body, Vec3::new(0.5, 0.3, 0.1), rig);
    let head = b.bone(Vec3::new(3.4, 0.4, 0.2), rig, BoneKind::Head);
    b.part(fill(&ellipse_pts(2.2, 2.3, 14), |_| 0.8), &body, Vec3::ZERO, head);
    b.part(fill(&ellipse_pts(1.8, 2.1, 14), |p| 0.7 + 0.3 * (p.y / 2.1)), &eye, Vec3::new(0.5, 0.3, 0.02), head);
    b.part(fill(&ellipse_pts(0.4, 0.5, 6), |_| 1.0), &shine, Vec3::new(1.0, 1.3, 0.03), head);
    let front = b.bone(Vec3::new(-0.8, 2.6, 0.4), rig, BoneKind::Wing { front: true });
    b.part(wing_mesh(), &wing, Vec3::ZERO, front);
    vec![(body, FLY_BODY), (eye, FLY_EYE)]
}

/// A flea, facing +X: a hunched glossy body on huge folded jumping legs, feet at y ≈ -6.5.
fn flea(b: &mut Builder, materials: &mut Assets<ColorMaterial>, rig: Entity) -> Vec<(Handle<ColorMaterial>, Color)> {
    let brown = materials.add(FLEA_BROWN);
    let dark = materials.add(ROACH_DARK);
    let shine = materials.add(ROACH_RIM);
    let eye = materials.add(EYE);
    // Small front legs.
    for (pair, x, z) in [(0, 3.0, -0.2), (1, 0.5, -0.25)] {
        let bone = b.bone(Vec3::new(x, -2.0, z), rig, BoneKind::Leg { pair, near: false });
        let path = smooth_open(&[Vec2::ZERO, Vec2::new(1.5, -2.5), Vec2::new(1.0, -4.5)], 3);
        b.part(stroke(&path, |t| 1.0 - 0.5 * t, 1.0, 1.0, |_| 1.0), &dark, Vec3::ZERO, bone);
    }
    // Body: tall, flattened sideways, armored bands.
    let body = smooth_closed(
        &[
            Vec2::new(5.5, 0.5),
            Vec2::new(4.0, 4.0),
            Vec2::new(0.0, 6.0),
            Vec2::new(-4.0, 5.5),
            Vec2::new(-6.5, 2.5),
            Vec2::new(-6.5, -1.5),
            Vec2::new(-4.0, -3.5),
            Vec2::new(0.0, -3.5),
            Vec2::new(4.0, -2.0),
        ],
        5,
    );
    b.part(fill(&body, |p| 0.55 + 0.45 * ((p.y + 3.5) / 9.5).clamp(0.0, 1.0)), &brown, Vec3::ZERO, rig);
    for x in [-3.5f32, -0.5, 2.5] {
        let band = smooth_open(&[Vec2::new(x + 0.6, 5.5 - x.abs() * 0.2), Vec2::new(x - 0.4, 1.0), Vec2::new(x + 0.3, -3.0)], 3);
        b.part(stroke(&band, |_| 0.5, 1.0, 1.0, |_| 1.0), &dark, Vec3::new(0.0, 0.0, 0.02), rig);
    }
    let gloss = smooth_open(&[Vec2::new(-3.0, 4.6), Vec2::new(0.0, 5.3), Vec2::new(3.0, 3.8)], 4);
    b.part(stroke(&gloss, |t| 0.9 * (PI * t).sin(), 1.0, 1.0, |_| 1.0), &shine, Vec3::new(0.0, 0.0, 0.03), rig);
    let head = b.bone(Vec3::new(5.5, 0.5, 0.1), rig, BoneKind::Head);
    let snout = [Vec2::new(-1.5, 2.0), Vec2::new(2.0, 0.5), Vec2::new(3.2, -1.5), Vec2::new(-1.0, -2.0)];
    b.part(fill(&snout, |_| 0.8), &brown, Vec3::ZERO, head);
    b.part(fill(&ellipse_pts(0.6, 0.6, 6), |_| 1.0), &eye, Vec3::new(0.2, 0.4, 0.02), head);
    let beak = [Vec2::new(2.6, -1.0), Vec2::new(3.6, -3.5)];
    b.part(stroke(&beak, |t| 0.6 - 0.3 * t, 1.0, 1.0, |_| 1.0), &dark, Vec3::new(0.0, 0.0, 0.01), head);
    // The jumping leg: a thick thigh folded up along the body, a long shin to the ground.
    let hind = b.bone(Vec3::new(-3.0, 0.0, 0.1), rig, BoneKind::Leg { pair: 2, near: true });
    let thigh = smooth_open(&[Vec2::ZERO, Vec2::new(3.0, -1.5), Vec2::new(4.5, -3.5)], 4);
    b.part(stroke(&thigh, |t| 3.0 - 1.2 * t, 1.0, 0.7, |_| 1.0), &brown, Vec3::ZERO, hind);
    let shin = smooth_open(&[Vec2::new(4.5, -3.5), Vec2::new(-1.0, -5.0), Vec2::new(-5.0, -6.5)], 4);
    b.part(stroke(&shin, |t| 1.4 - 0.6 * t, 1.0, 0.8, |_| 1.0), &dark, Vec3::new(0.0, 0.0, 0.01), hind);
    vec![(brown, FLEA_BROWN), (dark, ROACH_DARK)]
}

/// A paper nest hanging on a stalk; the stalk's top (the pivot) is `NEST_STALK` above the center.
fn nest(b: &mut Builder, materials: &mut Assets<ColorMaterial>, rig: Entity) -> Vec<(Handle<ColorMaterial>, Color)> {
    let paper = materials.add(PAPER);
    let dark = materials.add(PAPER_DARK);
    let hole = materials.add(EYE);
    let sway = b.bone(Vec3::new(0.0, NEST_STALK, 0.0), rig, BoneKind::Sway);
    let o = -NEST_STALK;
    let stalk = [Vec2::ZERO, Vec2::new(0.3, -5.0), Vec2::new(0.0, -10.0)];
    b.part(stroke(&stalk, |t| 2.6 - 0.8 * t, 1.0, 0.7, |_| 1.0), &dark, Vec3::new(0.0, 0.0, -0.1), sway);
    let outline = smooth_closed(
        &[
            Vec2::new(0.0, 13.0),
            Vec2::new(7.0, 10.0),
            Vec2::new(10.5, 2.0),
            Vec2::new(9.0, -7.0),
            Vec2::new(5.0, -12.0),
            Vec2::new(0.0, -13.5),
            Vec2::new(-5.0, -12.0),
            Vec2::new(-9.0, -7.0),
            Vec2::new(-10.5, 2.0),
            Vec2::new(-7.0, 10.0),
        ],
        5,
    );
    let shade = |p: Vec2| 0.6 + 0.4 * ((p.y + 13.5) / 26.5) - 0.15 * (p.x / 10.5).abs();
    b.part(fill(&outline, shade), &paper, Vec3::new(0.0, o, 0.0), sway);
    // Layers of chewed paper.
    for (y, w) in [(8.0, 7.5), (3.0, 10.0), (-2.5, 10.2), (-7.5, 8.6)] {
        let band = smooth_open(&[Vec2::new(-w, y + 1.0), Vec2::new(0.0, y - 1.2), Vec2::new(w, y + 1.0)], 5);
        b.part(stroke(&band, |t| 0.4 + 0.8 * (PI * t).sin(), 1.0, 1.0, |_| 1.0), &dark, Vec3::new(0.0, o, 0.02), sway);
    }
    b.part(fill(&ellipse_pts(2.6, 1.8, 12), |_| 1.0), &hole, Vec3::new(0.0, o - 11.0, 0.03), sway);
    vec![(paper, PAPER), (dark, PAPER_DARK)]
}

/// A gutter rat, facing +X: hunched furry body, pointed snout, long bare tail, feet at y ≈ -5.5.
fn rat(b: &mut Builder, materials: &mut Assets<ColorMaterial>, rig: Entity) -> Vec<(Handle<ColorMaterial>, Color)> {
    let fur = materials.add(RAT_FUR);
    let dark = materials.add(scale_color(RAT_FUR, 0.55));
    let skin = materials.add(RAT_SKIN);
    let eye = materials.add(EYE);
    let glint = materials.add(Color::WHITE);
    let tail = b.bone(Vec3::new(-9.0, -1.0, -0.1), rig, BoneKind::Tail);
    let path = smooth_open(&[Vec2::ZERO, Vec2::new(-6.0, -1.5), Vec2::new(-12.0, 0.0), Vec2::new(-17.0, 2.5)], 5);
    b.part(stroke(&path, |t| 1.6 - 1.2 * t, 1.0, 0.7, |_| 1.0), &skin, Vec3::ZERO, tail);
    let leg_path = smooth_open(&[Vec2::ZERO, Vec2::new(0.6, -2.0), Vec2::new(1.4, -3.4)], 3);
    for near in [false, true] {
        let (mat, z) = if near { (&fur, 0.05) } else { (&dark, -0.2) };
        for (pair, x) in [(0, 4.5), (2, -5.0)] {
            let bone = b.bone(Vec3::new(x, -2.2, z), rig, BoneKind::Leg { pair, near });
            b.part(stroke(&leg_path, |t| 1.6 - 0.8 * t, 1.0, 0.8, |_| 1.0), mat, Vec3::ZERO, bone);
        }
    }
    let body = smooth_closed(
        &[
            Vec2::new(7.0, 1.0),
            Vec2::new(5.0, 4.0),
            Vec2::new(0.0, 5.2),
            Vec2::new(-6.0, 4.6),
            Vec2::new(-10.0, 1.5),
            Vec2::new(-10.0, -1.5),
            Vec2::new(-6.0, -3.0),
            Vec2::new(0.0, -3.2),
            Vec2::new(5.0, -2.5),
        ],
        5,
    );
    b.part(fill(&body, |p| 0.5 + 0.5 * ((p.y + 3.2) / 8.4).clamp(0.0, 1.0)), &fur, Vec3::ZERO, rig);
    // Matted fur along the back.
    let tufts: Vec<[Vec2; 3]> = (0..5)
        .map(|i| {
            let x = 4.0 - i as f32 * 3.0;
            let y = 5.0 - (x / 10.0).powi(2) * 3.0;
            [Vec2::new(x + 1.2, y - 0.8), Vec2::new(x - 1.2, y - 0.6), Vec2::new(x - 1.0, y + 1.2)]
        })
        .collect();
    b.part(triangles(&tufts), &fur, Vec3::new(0.0, 0.0, 0.01), rig);
    let head = b.bone(Vec3::new(6.0, 0.8, 0.1), rig, BoneKind::Head);
    let snout = smooth_closed(
        &[Vec2::new(-2.0, 3.0), Vec2::new(2.0, 2.4), Vec2::new(6.5, 0.0), Vec2::new(6.8, -1.0), Vec2::new(2.0, -2.2), Vec2::new(-2.0, -2.6)],
        4,
    );
    b.part(fill(&snout, |p| 0.6 + 0.4 * ((p.y + 2.6) / 5.6)), &fur, Vec3::ZERO, head);
    b.part(fill(&ellipse_pts(1.5, 2.0, 12), |_| 0.9), &skin, Vec3::new(-0.5, 3.0, -0.02), head);
    b.part(fill(&ellipse_pts(0.7, 0.7, 8), |_| 1.0), &eye, Vec3::new(2.4, 0.9, 0.02), head);
    b.part(fill(&ellipse_pts(0.25, 0.25, 6), |_| 1.0), &glint, Vec3::new(2.6, 1.2, 0.03), head);
    b.part(fill(&ellipse_pts(0.6, 0.5, 6), |_| 1.0), &skin, Vec3::new(6.6, -0.4, 0.02), head);
    for dy in [0.6f32, -0.6] {
        let whisker = [Vec2::new(5.5, -0.3), Vec2::new(9.0, -0.3 + dy * 2.5)];
        b.part(stroke(&whisker, |_| 0.25, 1.0, 1.0, |_| 1.0), &dark, Vec3::new(0.0, 0.0, 0.03), head);
    }
    vec![(fur, RAT_FUR), (skin, RAT_SKIN)]
}

/// A spider hanging head-down on its thread: abdomen up (with an hourglass mark), eight legs.
fn spider(b: &mut Builder, materials: &mut Assets<ColorMaterial>, rig: Entity, root: Entity) -> Vec<(Handle<ColorMaterial>, Color)> {
    let body = materials.add(SPIDER_BODY);
    let mark = materials.add(SPIDER_MARK);
    let eye = materials.add(Color::WHITE);
    let silk = materials.add(ColorMaterial { color: SILK, alpha_mode: AlphaMode2d::Blend, ..default() });
    let thread = b.bone(Vec3::new(0.0, 0.0, -0.5), root, BoneKind::Thread);
    let strip = [Vec2::new(0.0, 0.0), Vec2::new(0.0, 1.0)];
    b.part(stroke(&strip, |_| 0.9, 1.0, 1.0, |_| 1.0), &silk, Vec3::ZERO, thread);
    for near in [false, true] {
        let side = if near { 1.0 } else { -1.0 };
        let bone = b.bone(Vec3::new(0.0, -1.0, if near { 0.05 } else { -0.2 }), rig, BoneKind::Leg { pair: 0, near });
        for i in 0..4 {
            let k = i as f32;
            let path = smooth_open(
                &[Vec2::new(side * 1.2, 0.5 - k * 0.8), Vec2::new(side * 7.0, 4.0 - k * 1.8), Vec2::new(side * (9.5 - k * 0.4), -3.0 - k * 1.8)],
                4,
            );
            b.part(stroke(&path, |t| 1.0 - 0.5 * t, 1.0, 0.8, |_| 1.0), &body, Vec3::ZERO, bone);
        }
    }
    b.part(fill(&ellipse_pts(5.5, 6.5, 20), |p| 0.6 + 0.4 * ((p.y + 6.5) / 13.0)), &body, Vec3::new(0.0, 4.5, 0.1), rig);
    b.part(
        triangles(&[
            [Vec2::new(-1.4, 6.5), Vec2::new(1.4, 6.5), Vec2::new(0.0, 4.3)],
            [Vec2::new(0.0, 4.0), Vec2::new(-1.4, 1.8), Vec2::new(1.4, 1.8)],
        ]),
        &mark,
        Vec3::new(0.0, 0.0, 0.12),
        rig,
    );
    let head = b.bone(Vec3::new(0.0, -3.0, 0.15), rig, BoneKind::Head);
    b.part(fill(&ellipse_pts(3.2, 3.0, 14), |p| 0.7 + 0.3 * ((p.y + 3.0) / 6.0)), &body, Vec3::ZERO, head);
    for x in [-1.0f32, 1.0] {
        b.part(fill(&ellipse_pts(0.45, 0.45, 6), |_| 1.0), &eye, Vec3::new(x, -1.6, 0.02), head);
        let fang = [Vec2::new(x * 0.8, -2.6), Vec2::new(x * 1.2, -4.8)];
        b.part(stroke(&fang, |t| 0.9 - 0.5 * t, 1.0, 1.0, |_| 1.0), &body, Vec3::new(0.0, 0.0, 0.01), head);
    }
    vec![(body, SPIDER_BODY), (mark, SPIDER_MARK)]
}

/// A spiderling walking, side view, facing +X, feet at y ≈ -6: a round
/// abdomen with a blue spot, the small front body, eight legs (four a side,
/// grouped on three bones for the walk).
fn spiderling(b: &mut Builder, materials: &mut Assets<ColorMaterial>, rig: Entity) -> Vec<(Handle<ColorMaterial>, Color)> {
    let body = materials.add(SPIDER_BODY);
    let mark = materials.add(SPIDERLING_MARK);
    let eye = materials.add(Color::WHITE);
    // (pair, hip x, reach): the front pair carries two legs.
    let legs = [(0, 2.5, 6.5), (0, 1.5, 5.0), (1, 0.5, 2.0), (2, -0.5, -4.5)];
    for near in [false, true] {
        let z = if near { 0.05 } else { -0.2 };
        let bones: Vec<Entity> = (0..3).map(|pair| b.bone(Vec3::new(0.0, 0.0, z), rig, BoneKind::Leg { pair, near })).collect();
        for (pair, x, reach) in legs {
            let x = x + if near { 0.0 } else { 0.8 };
            let path = smooth_open(&[Vec2::new(x, 0.5), Vec2::new(x + reach * 0.45, 4.5), Vec2::new(x + reach, -6.0)], 4);
            b.part(stroke(&path, |t| 0.9 - 0.4 * t, 1.0, 0.8, |_| 1.0), &body, Vec3::ZERO, bones[pair]);
        }
    }
    b.part(fill(&ellipse_pts(5.0, 4.2, 18), |p| 0.6 + 0.4 * ((p.y + 4.2) / 8.4)), &body, Vec3::new(-4.5, 1.5, 0.1), rig);
    b.part(fill(&ellipse_pts(1.6, 1.3, 10), |_| 1.0), &mark, Vec3::new(-4.8, 3.0, 0.12), rig);
    let head = b.bone(Vec3::new(2.2, 0.5, 0.15), rig, BoneKind::Head);
    b.part(fill(&ellipse_pts(3.0, 2.4, 14), |p| 0.7 + 0.3 * ((p.y + 2.4) / 4.8)), &body, Vec3::ZERO, head);
    b.part(fill(&ellipse_pts(0.5, 0.5, 6), |_| 1.0), &eye, Vec3::new(2.0, 0.8, 0.02), head);
    let fang = [Vec2::new(2.6, -1.4), Vec2::new(3.2, -3.0)];
    b.part(stroke(&fang, |t| 0.8 - 0.4 * t, 1.0, 1.0, |_| 1.0), &body, Vec3::new(0.0, 0.0, 0.01), head);
    vec![(body, SPIDER_BODY), (mark, SPIDERLING_MARK)]
}

/// A small wasp, facing +X, centered on its thorax.
fn drone(b: &mut Builder, materials: &mut Assets<ColorMaterial>, rig: Entity) -> Vec<(Handle<ColorMaterial>, Color)> {
    let yellow = materials.add(WASP_YELLOW);
    let black = materials.add(WASP_BLACK);
    let eye = materials.add(EYE);
    let wing = materials.add(ColorMaterial { color: WING, alpha_mode: AlphaMode2d::Blend, ..default() });
    let shine = materials.add(Color::WHITE);
    let wing_mesh = || {
        let pts: Vec<Vec2> = ellipse_pts(9.0, 2.8, 18).into_iter().map(|p| Vec2::from_angle(0.45).rotate(p + Vec2::new(-8.0, 0.0))).collect();
        fill(&pts, |p| 0.8 + 0.2 * (p.y / 5.0).clamp(-1.0, 1.0))
    };

    let back_wing = b.bone(Vec3::new(-0.5, 4.0, -0.4), rig, BoneKind::Wing { front: false });
    b.part(wing_mesh(), &wing, Vec3::ZERO, back_wing);

    // Dangling legs.
    for (pair, x) in [(0, 2.0), (2, -2.0)] {
        let bone = b.bone(Vec3::new(x, -3.0, -0.2), rig, BoneKind::Leg { pair, near: true });
        let path = smooth_open(&[Vec2::ZERO, Vec2::new(-1.5, -4.5), Vec2::new(-0.5, -8.0), Vec2::new(1.0, -9.5)], 4);
        b.part(stroke(&path, |t| 1.2 - 0.6 * t, 1.0, 1.0, |_| 1.0), &black, Vec3::ZERO, bone);
    }

    // Abdomen: striped, with a stinger.
    let abdomen = b.bone(Vec3::new(-4.0, -1.0, 0.0), rig, BoneKind::Abdomen);
    let outline = smooth_closed(
        &[
            Vec2::new(0.5, 1.8),
            Vec2::new(-3.0, 3.8),
            Vec2::new(-8.5, 3.8),
            Vec2::new(-13.0, 1.8),
            Vec2::new(-16.0, -1.5),
            Vec2::new(-12.5, -3.8),
            Vec2::new(-6.5, -4.2),
            Vec2::new(-2.0, -3.2),
            Vec2::new(0.5, -1.6),
        ],
        5,
    );
    b.part(fill(&outline, |p| 0.65 + 0.35 * ((p.y + 4.0) / 8.0).clamp(0.0, 1.0)), &yellow, Vec3::ZERO, abdomen);
    let span = |x: f32| {
        let near = outline.iter().filter(|p| (p.x - x).abs() < 1.2);
        near.fold((0.0f32, 0.0f32), |(lo, hi), p| (lo.min(p.y), hi.max(p.y)))
    };
    let bands: Vec<[Vec2; 3]> = [-4.5, -10.0]
        .into_iter()
        .flat_map(|x| {
            let (lo, hi) = span(x);
            let (a, b, c, d) = (Vec2::new(x + 1.0, hi + 0.3), Vec2::new(x - 1.5, hi + 0.3), Vec2::new(x - 2.0, lo - 0.3), Vec2::new(x + 0.4, lo - 0.3));
            [[a, b, c], [a, c, d]]
        })
        .collect();
    b.part(triangles(&bands), &black, Vec3::new(0.0, 0.0, 0.05), abdomen);
    b.part(triangles(&[[Vec2::new(-15.0, -0.5), Vec2::new(-14.5, -2.5), Vec2::new(-21.0, -3.5)]]), &black, Vec3::new(0.0, 0.0, -0.05), abdomen);

    // Thorax with a dark saddle.
    b.part(fill(&ellipse_pts(5.5, 4.5, 20), |p| 0.7 + 0.3 * ((p.y + 4.5) / 9.0)), &yellow, Vec3::new(0.0, 0.0, 0.1), rig);
    let saddle = smooth_closed(&[Vec2::new(-4.0, 2.2), Vec2::new(0.0, 4.5), Vec2::new(4.0, 2.8), Vec2::new(0.0, 1.2)], 4);
    b.part(fill(&saddle, |_| 1.0), &black, Vec3::new(0.0, 0.0, 0.12), rig);

    // Head: big eye, angry brow, mandibles.
    let head = b.bone(Vec3::new(5.5, 1.5, 0.2), rig, BoneKind::Head);
    b.part(fill(&ellipse_pts(4.0, 4.2, 18), |p| 0.7 + 0.3 * ((p.y + 4.2) / 8.4)), &yellow, Vec3::ZERO, head);
    let eye_pts: Vec<Vec2> = ellipse_pts(2.0, 3.2, 14).into_iter().map(|p| Vec2::from_angle(-0.3).rotate(p)).collect();
    b.part(fill(&eye_pts, |p| 0.8 + 0.2 * p.y / 3.2), &eye, Vec3::new(0.6, 0.8, 0.05), head);
    b.part(fill(&ellipse_pts(0.5, 0.7, 6), |_| 1.0), &shine, Vec3::new(1.3, 2.2, 0.06), head);
    b.part(triangles(&[[Vec2::new(-2.0, 4.8), Vec2::new(3.6, 2.3), Vec2::new(-1.0, 3.1)]]), &black, Vec3::new(0.0, 0.0, 0.07), head);
    for side in [1.0f32, -1.0] {
        let pts = smooth_open(&[Vec2::ZERO, Vec2::new(2.2, -0.8 * side), Vec2::new(3.6, 0.3 * side)], 4);
        b.part(stroke(&pts, |t| 1.4 * (1.0 - t) + 0.3, 1.0, 1.0, |_| 1.0), &black, Vec3::new(2.8, -2.2, 0.04 * side), head);
    }
    for (dx, z) in [(0.0, -0.05), (1.4, 0.08)] {
        let ant = smooth_open(&[Vec2::new(dx, 3.4), Vec2::new(dx + 2.2, 7.8), Vec2::new(dx + 6.0, 9.5), Vec2::new(dx + 8.5, 7.8)], 4);
        b.part(stroke(&ant, |t| 0.8 - 0.35 * t, 1.0, 1.0, |_| 1.0), &black, Vec3::new(0.0, 0.0, z), head);
    }

    let front_wing = b.bone(Vec3::new(-1.5, 4.5, 0.5), rig, BoneKind::Wing { front: true });
    b.part(wing_mesh(), &wing, Vec3::ZERO, front_wing);

    vec![(yellow, WASP_YELLOW), (black, WASP_BLACK)]
}

/// A wasp larva: pale fat segments and a dark head, facing +X, belly at y ≈ -4.4.
fn grub(b: &mut Builder, materials: &mut Assets<ColorMaterial>, rig: Entity) -> Vec<(Handle<ColorMaterial>, Color)> {
    let cream = materials.add(GRUB_CREAM);
    let head_mat = materials.add(GRUB_HEAD);
    let gloss = materials.add(Color::WHITE);

    for (i, (rx, ry)) in GRUB_RADII.into_iter().enumerate() {
        // Rest on a common belly line; the back ones further behind.
        let seg = b.bone(Vec3::new(4.0 - i as f32 * 4.2, ry - GRUB_BELLY, -0.01 * i as f32), rig, BoneKind::Segment(i));
        b.part(fill(&ellipse_pts(rx, ry, 16), move |p| 0.55 + 0.45 * ((p.y + ry) / (2.0 * ry))), &cream, Vec3::ZERO, seg);
        b.part(fill(&ellipse_pts(rx * 0.45, 0.5, 8), |_| 1.0), &gloss, Vec3::new(0.3, ry - 1.1, 0.01), seg);
    }
    let head = b.bone(Vec3::new(8.0, -1.2, 0.2), rig, BoneKind::Head);
    b.part(fill(&ellipse_pts(3.0, 2.8, 14), |p| 0.6 + 0.4 * ((p.y + 2.8) / 5.6)), &head_mat, Vec3::ZERO, head);
    for side in [1.0f32, -1.0] {
        let pts = smooth_open(&[Vec2::ZERO, Vec2::new(2.2, -0.6 * side), Vec2::new(3.4, 0.6 * side)], 4);
        b.part(stroke(&pts, |t| 1.3 * (1.0 - t) + 0.3, 1.0, 1.0, |_| 1.0), &head_mat, Vec3::new(2.0, -1.0, 0.03 * side), head);
    }

    vec![(cream, GRUB_CREAM), (head_mat, GRUB_HEAD)]
}

/// Tear a few parts off a freshly killed mob.
pub fn dismember(mut commands: Commands, mut mobs: Query<&mut Mob>, bones: Query<(Entity, &MobBone, &GlobalTransform)>) {
    for (e, bone, gt) in &bones {
        let Ok(m) = mobs.get(bone.owner) else { continue };
        if !m.dismember {
            continue;
        }
        let f = m.facing;
        let blood = Some(m.spec.blood);
        let (vel, spin, bleeds) = match bone.kind {
            BoneKind::Antenna { offset } => (Vec2::new(f * (150.0 + offset * 90.0), 380.0), 10.0, None),
            BoneKind::Leg { pair: 0, near: true } => (Vec2::new(f * 160.0, 420.0), -9.0, None),
            BoneKind::Leg { pair: 2, near: true } if m.kind != MobKind::Drone => (Vec2::new(-f * 140.0, 360.0), 8.0, None),
            BoneKind::Wing { front } => (Vec2::new(-f * if front { 220.0 } else { 130.0 }, 460.0), 7.0, None),
            BoneKind::Head if m.kind == MobKind::Grub => (Vec2::new(f * 170.0, 380.0), 6.0, blood),
            BoneKind::Abdomen => (Vec2::new(-f * 120.0, 300.0), -5.0, blood),
            BoneKind::Tail => (Vec2::new(-f * 200.0, 340.0), 12.0, blood),
            BoneKind::Mall(k) => match mall::tear(k, m) {
                Some(torn) => torn,
                None => continue,
            },
            BoneKind::Treatment(tb) => match treatment::torn(tb, m) {
                Some(torn) => torn,
                None => continue,
            },
            BoneKind::Subway(k) => match subway::tear(k, &m) {
                subway::Tear::Off { vel, spin, bleeds } => (vel, spin, bleeds),
                subway::Tear::Gone => {
                    commands.entity(e).despawn();
                    continue;
                }
                subway::Tear::Stay => continue,
            },
            BoneKind::Crypt(c) => match catacombs::tear(c, m) {
                Some(torn) => torn,
                None => continue,
            },
            BoneKind::Bunker(part) => match bunker::torn(part, &m) {
                Some(fling) => fling,
                None => continue,
            },
            BoneKind::Sewer(part) => match sewer::torn(part, m) {
                Some(torn) => torn,
                None => continue,
            },
            BoneKind::Sinkhole(part) => match sinkhole::torn(part, f, blood) {
                Some(torn) => torn,
                None => continue,
            },
            BoneKind::Dam(b) => match dam::torn(b, m) {
                Some(gib) => gib,
                None => continue,
            },
            BoneKind::Cave(part) => match cave::tear(part, m) {
                Some(torn) => torn,
                None => continue,
            },
            _ => continue,
        };
        commands.entity(e).remove::<MobBone>().insert(MobStuff);
        boss::tear_off(&mut commands, e, gt, vel, spin, bleeds);
    }
    for mut m in &mut mobs {
        m.dismember = false;
    }
}

pub fn animate(time: Res<Time>, mobs: Query<(&Mob, &Transform)>, mut bones: Query<(&MobBone, &mut Transform), Without<Mob>>) {
    let t = time.elapsed_secs();
    let dt = time.delta_secs();
    for (bone, mut tf) in &mut bones {
        let Ok((m, mtf)) = mobs.get(bone.owner) else { continue };
        let tt = t + m.seed;
        if bunker::animate(bone.kind, m, &mut tf, tt, dt) {
            continue;
        }
        if sinkhole::draws(m.kind) {
            sinkhole::animate(bone.kind, m, &mut tf, tt, dt);
            continue;
        }
        let (dead, landed) = match m.state {
            State::Dead { t, landed } => (Some(t), landed),
            _ => (None, false),
        };
        let windup = m.windup();
        let shiver = windup * (tt * 60.0).sin() * 0.06;
        let attacking = matches!(m.state, State::Attack(_));
        let flies = m.spec.flies();
        let moves = m.spec.moves;
        match bone.kind {
            BoneKind::Rig if treatment::draws(m.kind) => treatment::rig(m, &mut tf, tt, dt),
            BoneKind::Treatment(tb) => treatment::animate(tb, m, &mut tf, tt),
            BoneKind::Rig if mobs::sewer::owns(moves) => sewer::rig(m, &mut tf, tt, dt),
            BoneKind::Sewer(part) => sewer::animate(part, m, &mut tf, tt),
            BoneKind::Rig if matches!(moves, Moves::Cave(_)) => cave::rig(m, &mut tf, windup, tt, dt),
            BoneKind::Cave(part) => cave::animate(part, m, mtf, &mut tf, windup, tt),
            BoneKind::Rig => {
                let s = m.spec.scale;
                tf.scale.x += (m.facing * s - tf.scale.x) * (1.0 - (-18.0 * dt).exp());
                // Fleas crouch before leaping.
                tf.scale.y = s * if moves == Moves::Hop { 1.0 - 0.2 * windup } else { 1.0 };
                let angle = match m.state {
                    // A nest just drops; the rest end belly-up (flipping over while they fall).
                    State::Dead { .. } if moves == Moves::Hang => 0.0,
                    State::Dead { t, .. } => PI * if landed { 1.0 } else { (t * 3.0).min(1.0) },
                    _ if moves == Moves::Hang => 0.0,
                    // Nose along the leap.
                    _ if moves == Moves::Hop && m.airborne => (m.vel * m.facing).to_angle().clamp(-0.7, 0.7) * 0.6,
                    State::Windup(_) if moves == Moves::Hop => -0.12 * windup + shiver,
                    State::Windup(_) if !flies => 0.2 * windup + shiver, // rears up
                    State::Attack(_) if flies => (m.vel * m.facing).to_angle().clamp(-1.2, 1.2),
                    State::Attack(_) => -0.06,
                    State::Hurt(s) => 0.3 * (1.0 - s / 0.3).max(0.0) * if flies { 1.0 } else { -0.5 },
                    _ if flies => -(m.vel.x * m.facing) * 0.0012 + (tt * 3.0).sin() * 0.05 + shiver,
                    _ => (tt * 9.0).sin() * 0.01,
                };
                tf.rotation = Quat::from_rotation_z(angle);
            }
            BoneKind::Head => {
                let bob = if dead.is_some() { -0.3 } else { (tt * 6.0).sin() * 0.05 - 0.1 * windup };
                tf.rotation = Quat::from_rotation_z(bob);
            }
            BoneKind::Antenna { offset } => {
                let angle = match m.state {
                    _ if dead.is_some() => -0.5 + (tt * 12.0 + offset).sin() * 0.05,
                    State::Attack(_) => 0.5 + (tt * 30.0 + offset).sin() * 0.05,
                    _ => (tt * 2.6 + offset).sin() * 0.16 + windup * (tt * 25.0 + offset).sin() * 0.2,
                };
                tf.rotation = Quat::from_rotation_z(angle);
            }
            BoneKind::Leg { pair: 2, near: true } if moves == Moves::Hop => {
                // The jumping leg: folded on the ground (tighter in the crouch), kicked out in the air.
                let angle = match dead {
                    Some(td) => 0.6 + (tt * 25.0).sin() * 0.4 * (1.0 - td / 4.0).max(0.1),
                    None if m.airborne => 0.9,
                    None => -0.25 * windup + shiver,
                };
                tf.rotation = Quat::from_rotation_z(angle);
            }
            BoneKind::Leg { pair, near } => {
                let base = [0.15, 0.0, -0.15][pair];
                let angle = if let Some(td) = dead {
                    let fade = (1.0 - td / 5.0).max(0.1);
                    base + (tt * 30.0 + pair as f32 * 1.7).sin() * 0.5 * fade
                } else if flies || moves == Moves::Perch || moves == Moves::Dangle || (moves == Moves::Hop && m.airborne) {
                    // Dangling, swinging with the flight.
                    0.3 * (m.vel.x * m.facing / 300.0).clamp(-1.0, 1.0) + (tt * 4.0 + pair as f32).sin() * 0.1
                } else {
                    // Tripod gait.
                    let group = if (pair == 1) != near { 0.0 } else { PI };
                    let amp = ((m.vel.x.abs() / 150.0).min(1.0) * 0.5).max(windup * 0.3);
                    base + (m.stride + group).sin() * amp
                };
                tf.rotation = Quat::from_rotation_z(angle);
            }
            BoneKind::Wing { front } => {
                if dead.is_some() {
                    tf.rotation = Quat::from_rotation_z(-0.3);
                    tf.scale.y = 1.0;
                } else if moves == Moves::Perch && windup == 0.0 {
                    // A soldier on guard keeps its wings folded back, with a flick now and then.
                    let flick = if (tt * 1.3).sin() > 0.97 { (tt * 70.0).sin() * 0.3 } else { 0.0 };
                    tf.rotation = Quat::from_rotation_z(-0.35 + flick);
                    tf.scale.y = 1.0;
                } else {
                    let rate = if moves == Moves::Swarm { 110.0 } else { 75.0 };
                    let flap = (tt * rate + if front { 0.0 } else { 1.4 }).sin();
                    tf.rotation = Quat::from_rotation_z(flap * 0.6);
                    tf.scale.y = 0.4 + 0.6 * flap.abs();
                }
            }
            BoneKind::Abdomen => {
                // Pumps; curls the stinger forward to aim and strike.
                let curl = if attacking { 0.6 } else { 0.55 * windup };
                tf.rotation = Quat::from_rotation_z((tt * 4.0).sin() * 0.06 - curl);
            }
            BoneKind::Segment(i) => {
                let k = i as f32;
                let crawl = dead.map_or(1.0, |td| (1.0 - td / 4.0).max(0.0) * 3.0);
                // A wave of squeezes runs from tail to head; rearing lifts the front.
                let wave = (m.stride * 1.4 + tt * 2.0 * crawl - k * 0.9).sin();
                let rear = windup * (1.0 - k / 3.0).max(0.0) * 4.0;
                tf.translation.y = GRUB_RADII[i].1 - GRUB_BELLY + wave * 0.5 + rear;
                tf.scale = Vec3::new(1.0 - wave * 0.06, 1.0 + wave * 0.08, 1.0);
            }
            BoneKind::Tail => {
                // Lashes while it runs, curls lazily otherwise.
                let run = (m.vel.x.abs() / 300.0).min(1.0);
                let angle = if dead.is_some() { -0.4 } else { (tt * (3.0 + 9.0 * run)).sin() * (0.15 + 0.2 * run) + 0.15 * run };
                tf.rotation = Quat::from_rotation_z(angle);
            }
            BoneKind::Thread => {
                // From the body up to its anchor on the ceiling (gone once it's dead).
                let pos = mtf.translation.truncate();
                let to = m.home + Vec2::Y * mobs::THREAD_REST - pos;
                let alive = dead.is_none();
                tf.rotation = Quat::from_rotation_z(to.to_angle() - PI / 2.0);
                tf.scale = Vec3::new(1.0, if alive { to.length() } else { 0.001 }, 1.0);
            }
            BoneKind::Mall(k) => mall::animate(k, m, &mut tf, t, dt),
            BoneKind::Subway(k) => subway::animate(k, m, &mut tf, tt),
            BoneKind::Crypt(c) => catacombs::animate(c, m, mtf, &mut tf, tt, dt),
            BoneKind::Dam(b) => dam::pose(b, m, &mut tf, tt, dt),
            BoneKind::Sway => {
                // Swings gently; shudders when a drone crawls out or it's hit.
                let shudder = match m.state {
                    State::Attack(_) | State::Hurt(_) => (tt * 40.0).sin() * 0.08,
                    _ => 0.0,
                };
                let hang = if dead.is_some() { 0.0 } else { (tt * 0.9).sin() * 0.05 + shudder };
                tf.rotation = Quat::from_rotation_z(hang);
            }
            // Posed by `bunker::animate` above.
            BoneKind::Bunker(_) => {}
            BoneKind::Sinkhole(_) => {}
        }
    }
}
