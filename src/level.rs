//! Level generation: a long, broken wasteland of rubble, slabs and girders.
//!
//! The layout is procedural but seeded, so it's the same every run.
//! Change `SEED` for a different level, `LEVEL_END` for a longer one.

use bevy::prelude::*;

use crate::{
    shapes::*,
    unstable::{Chain, Motion},
};

/// The world's lost colors — gone with the war, to be earned back.
#[allow(dead_code)]
pub mod lost_colors {
    use bevy::prelude::Color;

    pub const MOSS: Color = Color::srgb(0.40, 0.66, 0.30);
    pub const PINK: Color = Color::srgb(0.93, 0.48, 0.64);
    pub const TEAL: Color = Color::srgb(0.22, 0.68, 0.66);
    pub const ORANGE: Color = Color::srgb(0.96, 0.60, 0.24);
    pub const PURPLE: Color = Color::srgb(0.58, 0.44, 0.86);
    pub const YELLOW: Color = Color::srgb(0.93, 0.83, 0.32);
}

const CONCRETE: Color = Color::srgb(0.40, 0.38, 0.36);
const ASPHALT: Color = Color::srgb(0.24, 0.24, 0.25);
const RUST: Color = Color::srgb(0.44, 0.27, 0.18);
const STEEL: Color = Color::srgb(0.40, 0.42, 0.45);
const SOOT: Color = Color::srgb(0.09, 0.09, 0.10);

const SEED: u32 = 0xC0FFEE;
const LEVEL_END: f32 = 7000.0;
const GROUND_DEPTH: f32 = 1000.0;
const SLAB_H: f32 = 18.0;

pub struct LevelPlugin;

impl Plugin for LevelPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn_level);
    }
}

/// Collision box the player stands on. `color` is what the chameleon blends into.
#[derive(Component)]
pub struct Platform {
    pub half: Vec2,
    pub color: Color,
    /// One-way platforms can be jumped through from below.
    pub one_way: bool,
    /// False while crumbled away.
    pub solid: bool,
    /// Safe to respawn on.
    pub stable: bool,
    /// How far it moved this frame (so the player rides along).
    pub delta: Vec2,
}

impl Platform {
    fn new(half: Vec2, color: Color, one_way: bool, stable: bool) -> Self {
        Self { half, color, one_way, solid: true, stable, delta: Vec2::ZERO }
    }
}

struct Mats {
    concrete: Handle<ColorMaterial>,
    concrete_dark: Handle<ColorMaterial>,
    asphalt: Handle<ColorMaterial>,
    rust: Handle<ColorMaterial>,
    steel: Handle<ColorMaterial>,
    steel_dark: Handle<ColorMaterial>,
    soot: Handle<ColorMaterial>,
    paint: Handle<ColorMaterial>,
    white: Handle<ColorMaterial>,
    rim: Handle<ColorMaterial>,
}

fn spawn_level(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
) {
    let mats = Mats {
        concrete: materials.add(CONCRETE),
        concrete_dark: materials.add(scale_color(CONCRETE, 0.6)),
        asphalt: materials.add(ASPHALT),
        rust: materials.add(RUST),
        steel: materials.add(STEEL),
        steel_dark: materials.add(scale_color(STEEL, 0.55)),
        soot: materials.add(SOOT),
        paint: materials.add(Color::srgb(0.80, 0.78, 0.70)),
        white: materials.add(Color::WHITE),
        rim: materials.add(Color::srgb(0.70, 0.67, 0.62)),
    };
    let mut b = Builder { commands: &mut commands, meshes: &mut meshes, mats, rng: Rng(SEED) };

    // Starting area.
    let mut x = -950.0;
    let mut top = -100.0;
    b.ground(x, 750.0, top);
    x += 750.0;

    while x < LEVEL_END {
        // A gap, sometimes bridged by something unreliable.
        let gap = b.rng.range(90.0, 300.0);
        let next_top = (top + b.rng.range(-110.0, 70.0)).clamp(-240.0, 140.0);
        let mid = x + gap / 2.0;
        let high = top.max(next_top);
        if gap > 160.0 {
            if b.rng.chance(0.5) {
                let w = b.rng.range(110.0, 150.0);
                b.girder(Vec2::new(mid, high + 70.0), w);
            } else {
                let w = b.rng.range(90.0, 120.0);
                b.slab(Vec2::new(mid, high + 35.0), w, Some(Motion::crumble()));
            }
        }
        x += gap;
        top = next_top;

        let w = b.rng.range(280.0, 650.0);
        b.ground(x, w, top);

        // Upper tiers keep things dense.
        let mut px = x + b.rng.range(30.0, 120.0);
        while px < x + w - 90.0 {
            let pw = b.rng.range(80.0, 150.0);
            let tier = top + b.rng.range(95.0, 120.0);
            b.floating(Vec2::new(px + pw / 2.0, tier), pw);
            if b.rng.chance(0.45) {
                let center = Vec2::new(px + pw / 2.0 + b.rng.range(-70.0, 70.0), tier + b.rng.range(95.0, 115.0));
                let w2 = b.rng.range(70.0, 130.0);
                b.floating(center, w2);
            }
            px += pw + b.rng.range(90.0, 220.0);
        }
        x += w;
    }

    // A wall of ruins closes the level (for now).
    b.ground(x, 500.0, top + 700.0);
}

struct Builder<'a, 'w, 's> {
    commands: &'a mut Commands<'w, 's>,
    meshes: &'a mut Assets<Mesh>,
    mats: Mats,
    rng: Rng,
}

impl Builder<'_, '_, '_> {
    fn root(&mut self, center: Vec2, platform: Platform, motion: Option<Motion>) -> Entity {
        let mut e = self.commands.spawn((platform, Transform::from_translation(center.extend(0.0)), Visibility::default()));
        if let Some(motion) = motion {
            e.insert(motion);
        }
        e.id()
    }

    fn child(&mut self, mesh: Mesh, mat: &Handle<ColorMaterial>, tf: Transform, parent: Entity) {
        let mesh = self.meshes.add(mesh);
        self.commands.spawn((Mesh2d(mesh), MeshMaterial2d(mat.clone()), tf, ChildOf(parent)));
    }

    /// A deep chunk of broken ground, top edge at `top`.
    fn ground(&mut self, left: f32, w: f32, top: f32) {
        let road = self.rng.chance(0.5);
        let color = if road { ASPHALT } else { CONCRETE };
        let center = Vec2::new(left + w / 2.0, top - GROUND_DEPTH / 2.0);
        let e = self.root(center, Platform::new(Vec2::new(w / 2.0, GROUND_DEPTH / 2.0), color, false, true), None);
        let (hw, t) = (w / 2.0, GROUND_DEPTH / 2.0);

        // Jagged top edge with chipped corners.
        let mut top_pts = Vec::new();
        let mut x = -hw;
        while x < hw {
            top_pts.push(Vec2::new(x, t + self.rng.range(-3.0, 3.0)));
            x += self.rng.range(12.0, 26.0);
        }
        top_pts.push(Vec2::new(hw, t));
        let last = top_pts.len() - 1;
        top_pts[0].y -= self.rng.range(5.0, 16.0);
        top_pts[last].y -= self.rng.range(5.0, 16.0);

        let mut outline = vec![Vec2::new(-hw, -t), Vec2::new(-hw + self.rng.range(-4.0, 2.0), t - 70.0)];
        outline.push(Vec2::new(-hw + self.rng.range(0.0, 7.0), t - 25.0));
        outline.extend(&top_pts);
        outline.push(Vec2::new(hw - self.rng.range(0.0, 7.0), t - 25.0));
        outline.push(Vec2::new(hw + self.rng.range(-2.0, 4.0), t - 70.0));
        outline.push(Vec2::new(hw, -t));
        // Lit at the top, swallowed by darkness below.
        let body = fill(&outline, |p| 0.12 + 0.88 * ((p.y - (t - 170.0)) / 170.0).clamp(0.0, 1.0).powf(1.6));
        let mat = if road { self.mats.asphalt.clone() } else { self.mats.concrete.clone() };
        self.child(body, &mat, Transform::default(), e);

        self.rim(e, &top_pts);

        if road {
            // Broken road: a concrete layer under the asphalt, faded lane marks.
            let under: Vec<Vec2> = top_pts.iter().map(|p| *p - Vec2::Y * 16.0).collect();
            let concrete = self.mats.concrete.clone();
            self.child(stroke(&under, |_| 8.0, 0.55, 0.3, |_| 1.0), &concrete, Transform::from_xyz(0.0, 0.0, 0.05), e);
            let mut x = -hw + self.rng.range(20.0, 50.0);
            while x < hw - 30.0 {
                if self.rng.chance(0.7) {
                    let paint = self.mats.paint.clone();
                    let mark = Rectangle::new(self.rng.range(10.0, 18.0), 2.0);
                    self.child(mark.into(), &paint, Transform::from_xyz(x, t - 5.0, 0.1), e);
                }
                x += 44.0;
            }
        }

        // Strata: buried layers of old floors and debris.
        for depth in [self.rng.range(35.0, 55.0), self.rng.range(80.0, 120.0)] {
            let layer: Vec<Vec2> = top_pts
                .iter()
                .map(|p| Vec2::new(p.x.clamp(-hw + 6.0, hw - 6.0), t - depth + self.rng.range(-2.5, 2.5)))
                .collect();
            let soot = self.mats.soot.clone();
            let width = self.rng.range(1.5, 3.5);
            let mesh = stroke(&layer, |_| width, 1.0, 1.0, |s| 0.6 + 0.4 * (s * 50.0).sin().abs());
            self.child(mesh, &soot, Transform::from_xyz(0.0, 0.0, 0.05), e);
        }

        // Cracks.
        for _ in 0..self.rng.below(3) + 1 {
            let start = top_pts[self.rng.below(top_pts.len())];
            let len = self.rng.range(30.0, 110.0);
            self.crack(e, start, len);
        }
        // Rubble embedded in the ground and lying on top.
        for _ in 0..(w / 60.0) as usize {
            let p = Vec2::new(self.rng.range(-hw + 10.0, hw - 10.0), t - self.rng.range(25.0, 220.0));
            let r = self.rng.range(4.0, 12.0);
            let rock = fill(&rock_pts(&mut self.rng, r, 7), |_| 0.8);
            let mat = self.mats.concrete_dark.clone();
            self.child(rock, &mat, Transform::from_translation(p.extend(0.1)), e);
        }
        for _ in 0..self.rng.below(4) + 1 {
            let r = self.rng.range(3.0, 7.0);
            let p = Vec2::new(self.rng.range(-hw + 10.0, hw - 10.0), t + r * 0.2);
            let rock = fill(&rock_pts(&mut self.rng, r, 6), |p| 0.7 + 0.3 * (p.y / r + 0.5));
            let mat = if self.rng.chance(0.5) { self.mats.concrete.clone() } else { self.mats.asphalt.clone() };
            self.child(rock, &mat, Transform::from_translation(p.extend(0.3)), e);
        }
        // Rebar bursting out of the broken edges.
        for side in [-1.0, 1.0] {
            for _ in 0..self.rng.below(3) {
                let base = Vec2::new(side * (hw - 4.0), t - self.rng.range(8.0, 40.0));
                let dir = Vec2::new(side, self.rng.range(-0.6, 0.5)).normalize();
                self.rebar(e, base, dir);
            }
        }
        // Props: bent lamp posts and barbed wire.
        if self.rng.chance(0.4) {
            let x = self.rng.range(-hw + 30.0, hw - 30.0);
            let lean = self.rng.range(-0.5, 0.5);
            let post = smooth_open(
                &[Vec2::new(x, t), Vec2::new(x + lean * 10.0, t + 45.0), Vec2::new(x + lean * 40.0, t + 75.0), Vec2::new(x + lean * 40.0 + 18.0, t + 80.0)],
                6,
            );
            let mat = self.mats.steel_dark.clone();
            self.child(stroke(&post, |s| 3.5 - 1.5 * s, 1.0, 0.7, |_| 1.0), &mat, Transform::from_xyz(0.0, 0.0, -0.5), e);
        }
        if self.rng.chance(0.4) {
            let x0 = self.rng.range(-hw, hw - 120.0);
            let len = self.rng.range(60.0, 140.0);
            let phase = self.rng.range(0.0, 6.0);
            let wire: Vec<Vec2> = (0..=40)
                .map(|i| {
                    let s = i as f32 / 40.0;
                    Vec2::new(x0 + s * len, t + 9.0 + (s * 34.0 + phase).sin() * 3.5 + (s * 7.0).sin() * 2.0)
                })
                .collect();
            let mat = self.mats.steel_dark.clone();
            self.child(stroke(&wire, |_| 1.1, 1.0, 1.0, |s| if (s * 40.0) as i32 % 5 == 0 { 0.4 } else { 1.0 }), &mat, Transform::from_xyz(0.0, 0.0, -0.4), e);
        }
    }

    /// Some kind of floating debris to jump on.
    fn floating(&mut self, center: Vec2, w: f32) {
        match self.rng.below(10) {
            0..=3 => self.slab(center, w, None),
            4..=6 => self.slab(center, w, Some(Motion::crumble())),
            7..=8 => self.plate(center, w),
            _ => self.girder(center + Vec2::Y * 20.0, w),
        }
    }

    /// Broken concrete slab. With `Motion::Crumble` it falls when stepped on.
    fn slab(&mut self, center: Vec2, w: f32, motion: Option<Motion>) {
        let crumbles = motion.is_some();
        let e = self.root(center, Platform::new(Vec2::new(w / 2.0, SLAB_H / 2.0), CONCRETE, true, !crumbles), motion);
        let (hw, hh) = (w / 2.0, SLAB_H / 2.0);

        let mut outline = Vec::new();
        let mut x = -hw + self.rng.range(2.0, 8.0);
        while x < hw - 8.0 {
            outline.push(Vec2::new(x, hh + self.rng.range(-1.5, 1.0)));
            x += self.rng.range(10.0, 20.0);
        }
        outline.push(Vec2::new(hw - self.rng.range(2.0, 8.0), hh));
        let top_edge = outline.clone();
        outline.push(Vec2::new(hw + self.rng.range(-2.0, 3.0), self.rng.range(-3.0, 3.0)));
        let mut x = hw - self.rng.range(4.0, 12.0);
        while x > -hw + 8.0 {
            outline.push(Vec2::new(x, -hh - self.rng.range(-1.0, 5.0)));
            x -= self.rng.range(8.0, 18.0);
        }
        outline.push(Vec2::new(-hw + self.rng.range(-3.0, 2.0), self.rng.range(-3.0, 3.0)));
        // Reverse so the outline is counter-clockwise (only matters for consistency).
        outline.reverse();
        let mat = if crumbles { self.mats.concrete_dark.clone() } else { self.mats.concrete.clone() };
        self.child(fill(&outline, |p| 0.55 + 0.45 * ((p.y + hh) / SLAB_H).clamp(0.0, 1.0)), &mat, Transform::default(), e);

        self.rim(e, &top_edge);

        for _ in 0..if crumbles { 4 } else { 1 } {
            let start = Vec2::new(self.rng.range(-hw + 8.0, hw - 8.0), hh);
            self.crack(e, start, SLAB_H + 6.0);
        }
        for side in [-1.0, 1.0] {
            if self.rng.chance(0.7) {
                let base = Vec2::new(side * (hw - 3.0), self.rng.range(-4.0, 4.0));
                let dir = Vec2::new(side, self.rng.range(-0.8, 0.4)).normalize();
                self.rebar(e, base, dir);
            }
        }
    }

    /// Rusty scrap plate that sinks under the player's weight.
    fn plate(&mut self, center: Vec2, w: f32) {
        let motion = Motion::Sink { origin: center, offset: 0.0 };
        let h = 14.0;
        let e = self.root(center, Platform::new(Vec2::new(w / 2.0, h / 2.0), RUST, true, false), Some(motion));
        let outline = [
            Vec2::new(-w / 2.0, -h / 2.0 + 2.0),
            Vec2::new(w / 2.0 - 3.0, -h / 2.0),
            Vec2::new(w / 2.0 + 2.0, h / 2.0 - 1.0),
            Vec2::new(-w / 2.0 + 4.0, h / 2.0),
        ];
        let mat = self.mats.rust.clone();
        self.child(fill(&outline, |p| 0.6 + 0.4 * ((p.y + h / 2.0) / h)), &mat, Transform::default(), e);
        self.rim(e, &[outline[3], outline[2]]);
        let soot = self.mats.soot.clone();
        let mut x = -w / 2.0 + 6.0;
        while x < w / 2.0 - 4.0 {
            self.child(Circle::new(1.2).into(), &soot, Transform::from_xyz(x, h / 2.0 - 3.0, 0.1), e);
            x += 12.0;
        }
        for _ in 0..3 {
            let p = Vec2::new(self.rng.range(-w / 2.0 + 8.0, w / 2.0 - 8.0), self.rng.range(-3.0, 2.0));
            let dent = fill(&ellipse_pts(self.rng.range(3.0, 7.0), 2.0, 12), |_| 0.7);
            self.child(dent, &soot, Transform::from_translation(p.extend(0.05)), e);
        }
    }

    /// Steel I-beam hanging from two chains, swinging.
    fn girder(&mut self, center: Vec2, w: f32) {
        let len = 330.0;
        let anchor = center + Vec2::Y * len;
        let motion = Motion::Swing {
            anchor,
            len,
            amp: self.rng.range(0.12, 0.22),
            speed: self.rng.range(1.0, 1.5),
            phase: self.rng.range(0.0, 6.28),
        };
        let e = self.root(center, Platform::new(Vec2::new(w / 2.0, 8.0), STEEL, true, false), Some(motion));
        let (steel, dark, soot) = (self.mats.steel.clone(), self.mats.steel_dark.clone(), self.mats.soot.clone());
        // Gradient meshes carry full colors in their vertices, so they use a white material.
        let white = self.mats.white.clone();
        self.child(gradient_rect(Vec2::new(w, 10.0), STEEL, scale_color(STEEL, 0.6)), &white, Transform::default(), e);
        self.child(Rectangle::new(w, 3.0).into(), &steel, Transform::from_xyz(0.0, 6.5, 0.1), e);
        self.child(Rectangle::new(w, 3.0).into(), &dark, Transform::from_xyz(0.0, -6.5, 0.1), e);
        let mut x = -w / 2.0 + 12.0;
        while x < w / 2.0 - 8.0 {
            self.child(Circle::new(2.4).into(), &soot, Transform::from_xyz(x, 0.0, 0.05), e);
            x += 22.0;
        }
        let chain_mesh = self.meshes.add(chain_mesh());
        for side in [-1.0, 1.0] {
            let attach = Vec2::new(side * (w / 2.0 - 10.0), 8.0);
            self.commands.spawn((
                Mesh2d(chain_mesh.clone()),
                MeshMaterial2d(dark.clone()),
                Transform::from_xyz(0.0, 0.0, -1.0),
                Chain { platform: e, anchor: anchor + attach, attach },
            ));
        }
    }

    /// Light catching the top edge, so walkable surfaces read against the dark.
    fn rim(&mut self, parent: Entity, top: &[Vec2]) {
        let path: Vec<Vec2> = top.iter().map(|p| *p - Vec2::Y * 1.0).collect();
        let rim = self.mats.rim.clone();
        self.child(stroke(&path, |_| 2.4, 1.0, 0.55, |_| 1.0), &rim, Transform::from_xyz(0.0, 0.0, 0.25), parent);
    }

    fn crack(&mut self, parent: Entity, start: Vec2, len: f32) {
        let n = 5;
        let mut pts = vec![start + Vec2::Y * 2.0];
        for i in 1..=n {
            let prev = pts[pts.len() - 1];
            pts.push(Vec2::new(prev.x + self.rng.range(-7.0, 7.0), start.y - len * i as f32 / n as f32));
        }
        let soot = self.mats.soot.clone();
        self.child(stroke(&pts, |t| 2.4 * (1.0 - t) + 0.4, 1.0, 1.0, |_| 1.0), &soot, Transform::from_xyz(0.0, 0.0, 0.2), parent);
    }

    fn rebar(&mut self, parent: Entity, base: Vec2, dir: Vec2) {
        let a = self.rng.range(8.0, 14.0);
        let b = self.rng.range(14.0, 24.0);
        let bend = dir.perp() * self.rng.range(-9.0, 9.0);
        let pts = smooth_open(&[base - dir * 6.0, base + dir * a, base + dir * b + bend], 5);
        let mat = self.mats.steel_dark.clone();
        self.child(stroke(&pts, |_| 1.8, 1.0, 0.7, |_| 1.0), &mat, Transform::from_xyz(0.0, 0.0, -0.1), parent);
    }
}

/// Unit-length vertical chain (scaled to length at runtime), with link bands.
fn chain_mesh() -> Mesh {
    let pts: Vec<Vec2> = (0..=120).map(|i| Vec2::new(0.0, i as f32 / 120.0 - 0.5)).collect();
    stroke(&pts, |_| 3.0, 1.0, 0.8, |t| if (t * 60.0) as i32 % 2 == 0 { 1.0 } else { 0.45 })
}

