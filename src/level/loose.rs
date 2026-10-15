//! Loose platforms: pieces of the world that give way under your weight.
//!
//! They are not hand-pinned: `plan` walks the terrain grid and finds
//! * **bridges**: a gap between two ledges at (nearly) the same height, small
//!   enough to jump, covered by a slab. It may cover a vertical path (the gap
//!   is a shaft) but never a horizontal or inclined one: the headroom above must
//!   be free, so it only ever *adds* a floor.
//! * **loose ledges**: a short shelf stuck out past the end of a ledge.
//!
//! What they are made of follows the area (concrete and rebar, rusty steel,
//! natural rock, rotten planks). They fall only when the chameleon *stands* on
//! them (`Player::ground`), and leave their broken contact points behind
//! (`Remnant`s: rebar, concrete, bent steel, splintered planks) on the side
//! they were fixed to.

use bevy::prelude::*;

use super::{Builder, Platform};
use crate::{
    shapes::*,
    terrain::{Style, Terrain, PX},
    terrain_art::TerrainPart,
    unstable::{Motion, Remnant},
};

/// Gaps (in sketch pixels) a bridge may cover: always within a plain jump
/// (~20 px), so a collapsed one never makes a route impossible, and never one
/// of the "requires dash" gaps.
const MIN_GAP: i32 = 4;
const MAX_GAP: i32 = 18;
/// Free air above a loose piece (pixels): the chameleon must be able to stand on it.
const HEADROOM: i32 = 6;
/// Inclined slabs: how many pixels the far ledge may differ in height, and how
/// steep the slab may be (rise per run: 0.8 is ~39°).
const MAX_RISE: i32 = 12;
const MAX_SLOPE: f32 = 0.8;
/// Free air below one: a real drop, not a notch.
const DROP: i32 = 6;
/// How far a piece reaches into the ground it is fixed to.
const OVERLAP: f32 = 4.0;
/// No loose piece closer than this (pixels) to another.
const SPACING: f32 = 30.0;
/// Nothing is placed this close (world units) to where the game starts.
const SPAWN_CLEAR: f32 = 320.0;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    Concrete,
    Metal,
    Rock,
    Planks,
}

impl Kind {
    fn thickness(self) -> f32 {
        match self {
            Kind::Concrete => 16.0,
            Kind::Metal => 12.0,
            Kind::Rock => 20.0,
            Kind::Planks => 12.0,
        }
    }

    /// What the chameleon blends into while standing on it.
    fn color(self) -> Color {
        match self {
            Kind::Concrete => super::CONCRETE,
            Kind::Metal => super::RUST,
            Kind::Rock => super::ROCK,
            Kind::Planks => super::PLANK,
        }
    }

    /// Seconds from the first step to the fall.
    fn delay(self) -> f32 {
        match self {
            Kind::Concrete => 0.6,
            Kind::Metal => 0.45,
            Kind::Rock => 0.7,
            Kind::Planks => 0.35,
        }
    }

    /// Material by area (`r` in 0..1 picks among the options).
    fn of(style: Style, r: f32) -> Kind {
        use Kind::*;
        let pick = |options: &[(Kind, f32)]| {
            let mut left = r;
            for &(k, w) in options {
                if left < w {
                    return k;
                }
                left -= w;
            }
            options[0].0
        };
        match style {
            Style::Concrete => pick(&[(Concrete, 0.7), (Metal, 0.3)]),
            Style::Trash => pick(&[(Planks, 0.5), (Metal, 0.3), (Concrete, 0.2)]),
            Style::Sewer => pick(&[(Metal, 0.6), (Concrete, 0.4)]),
            Style::Park => pick(&[(Planks, 0.6), (Concrete, 0.4)]),
            Style::Subway => pick(&[(Concrete, 0.5), (Metal, 0.5)]),
            Style::Treatment => pick(&[(Metal, 0.8), (Concrete, 0.2)]),
            Style::Sinkhole => pick(&[(Rock, 0.6), (Concrete, 0.4)]),
            Style::Catacombs => pick(&[(Rock, 0.7), (Planks, 0.3)]),
            Style::Mall => pick(&[(Concrete, 0.6), (Metal, 0.4)]),
            Style::Dam => pick(&[(Concrete, 0.8), (Metal, 0.2)]),
            Style::Bunker => Metal,
            Style::Cave => Rock,
        }
    }
}

/// One planned piece.
pub struct Loose {
    kind: Kind,
    /// The two ends of its top surface (left, right): on the ground faces
    /// (bridge) or the face and the free tip (ledge). Level, or inclined to
    /// reach a ledge higher or lower on the other side.
    a: Vec2,
    b: Vec2,
    /// Which ends are fixed to ground: (left, right).
    hold: (bool, bool),
}

impl Loose {
    fn center(&self) -> Vec2 {
        (self.a + self.b) / 2.0
    }

    /// The top surface including what reaches into the ground.
    fn line(&self) -> (Vec2, Vec2) {
        let d = (self.b - self.a).normalize();
        (self.a - d * if self.hold.0 { OVERLAP } else { 0.0 }, self.b + d * if self.hold.1 { OVERLAP } else { 0.0 })
    }
}

/// Deterministic noise in 0..1 for a spot.
fn hash(x: i32, y: i32, salt: u32) -> f32 {
    let mut h = (x as u32).wrapping_mul(0x9E37_79B1) ^ (y as u32).wrapping_mul(0x85EB_CA6B) ^ salt.wrapping_mul(0xC2B2_AE35);
    h ^= h >> 15;
    h = h.wrapping_mul(0x2C1B_3C6D);
    h ^= h >> 12;
    h = h.wrapping_mul(0x297A_2D39);
    h ^= h >> 15;
    (h >> 8) as f32 / (1u32 << 24) as f32
}

/// Whether an inclined slab along the top line `a` → `b` has its room: free
/// headroom above, free body below, a real drop under it.
fn incline_fits(t: &Terrain, a: Vec2, b: Vec2) -> bool {
    let free = |p: Vec2| !t.solid_at(p);
    let n = ((b.x - a.x) / (PX * 0.5)).ceil() as i32;
    for s in 1..n {
        let p = a.lerp(b, s as f32 / n as f32);
        if !(1..=HEADROOM * 2).all(|k| free(p + Vec2::Y * (k as f32 * PX * 0.5))) || ![2.0, 9.0, 16.0].into_iter().all(|off| free(p - Vec2::Y * off)) {
            return false;
        }
    }
    [0.3, 0.5, 0.7].into_iter().all(|u| {
        let p = a.lerp(b, u);
        (0..=DROP * 2).all(|j| free(p - Vec2::Y * (18.0 + j as f32 * PX * 0.5)))
    })
}

struct Candidate {
    loose: Loose,
    /// Position in sketch cells (for spacing).
    at: Vec2,
    luck: f32,
    bridge: bool,
}

/// Finds where loose pieces fit in the world.
pub fn plan(t: &Terrain) -> Vec<Loose> {
    let size = t.size();
    let solid = |x: i32, y: i32| t.solid(IVec2::new(x, y));
    let air = |x: i32, y: i32| !solid(x, y);
    // Solid ground with free air right above: somewhere to stand.
    let ledge = |x: i32, y: i32| solid(x, y) && air(x, y - 1);
    // World coordinates: the left face of a column, the top of a row.
    let face = |x: i32| t.cell_min(IVec2::new(x, 0)).x;
    let top_of = |y: i32| t.cell_min(IVec2::new(0, y)).y + PX;
    let dash_zone = |p: Vec2| t.features.iter().any(|f| f.name.to_lowercase().contains("dash") && f.rect.inflate(PX * 8.0).contains(p));
    // Air in the columns `xs` over the rows `ys`.
    let clear = |xs: std::ops::RangeInclusive<i32>, ys: std::ops::RangeInclusive<i32>| xs.into_iter().all(|x| ys.clone().all(|y| air(x, y)));

    let mut found: Vec<Candidate> = Vec::new();
    for y in HEADROOM + 3..size.y - DROP - 4 {
        for x in 4..size.x - 4 {
            if !ledge(x, y) {
                continue;
            }
            for dir in [1, -1] {
                // The ground must end here, with a few flat cells under foot.
                if solid(x + dir, y) || solid(x + dir, y - 1) || !ledge(x - dir, y) || !ledge(x - 2 * dir, y) {
                    continue;
                }
                // A bank facing this one across a jumpable gap (found from the left bank only).
                let mut bank = None;
                if dir == 1 {
                    for g in 1..=MAX_GAP + 1 {
                        let xx = x + g;
                        if solid(xx, y) {
                            bank = [y, y - 1].into_iter().find(|&r| ledge(xx, r)).map(|r| (xx, r));
                            break;
                        }
                        if ledge(xx, y + 1) {
                            bank = Some((xx, y + 1));
                            break;
                        }
                    }
                }
                if let Some((xx, r)) = bank {
                    // The slab sits at the lower of the two tops; the higher bank is a step.
                    let ys = y.max(r);
                    let gap = xx - x - 1;
                    let mid = x + 1 + gap / 2;
                    let fits = (MIN_GAP..=MAX_GAP).contains(&gap)
                        // Solid ground at both ends, flat enough to hold it.
                        && solid(x, ys)
                        && solid(xx, ys)
                        && ledge(xx + 1, r)
                        && ledge(xx + 2, r)
                        // The slab's own room, free headroom (never blocking a path), a real drop under it.
                        && clear(x + 1..=xx - 1, ys..=ys + 1)
                        && clear(x + 1..=xx - 1, ys - HEADROOM..=ys - 1)
                        && clear(mid..=mid, ys + 2..=ys + 1 + DROP);
                    if fits {
                        let (a, b) = (face(x + 1), face(xx));
                        let center = Vec2::new((a + b) / 2.0, top_of(ys));
                        if !dash_zone(center) {
                            let kind = Kind::of(t.style_at(center), hash(x, y, 1));
                            found.push(Candidate {
                                loose: Loose { kind, a: Vec2::new(a, top_of(ys)), b: Vec2::new(b, top_of(ys)), hold: (true, true) },
                                at: Vec2::new((x + xx) as f32 / 2.0, ys as f32),
                                luck: hash(x, y, 2),
                                bridge: true,
                            });
                        }
                        continue;
                    }
                }
                // Across the gap to a ledge higher or lower: an inclined slab.
                if dir == 1 {
                    let found_incline = (MIN_GAP + 1..=MAX_GAP + 1).find_map(|g| {
                        let xx = x + g;
                        (2..=MAX_RISE).find_map(|d| {
                            [y - d, y + d].into_iter().find_map(|r| {
                                let bank = ledge(xx, r) && ledge(xx + 1, r) && ledge(xx + 2, r) && air(xx - 1, r) && air(xx - 1, r - 1);
                                let (a, b) = (Vec2::new(face(x + 1), top_of(y)), Vec2::new(face(xx), top_of(r)));
                                (bank && (0.2..=MAX_SLOPE).contains(&((b.y - a.y) / (b.x - a.x)).abs()) && incline_fits(t, a, b)).then_some((a, b))
                            })
                        })
                    });
                    if let Some((a, b)) = found_incline {
                        let center = (a + b) / 2.0;
                        if !dash_zone(center) {
                            found.push(Candidate {
                                loose: Loose { kind: Kind::of(t.style_at(center), hash(x, y, 1)), a, b, hold: (true, true) },
                                at: Vec2::new((x + (b.x - a.x) as i32 / 16) as f32, y as f32),
                                luck: hash(x, y, 2),
                                bridge: true,
                            });
                        }
                        continue;
                    }
                }
                // A short loose ledge sticking out into open air. Never where another
                // ledge is within reach (that would shorten a jump or a dash).
                let len = 4 + (hash(x, y, 3) * 4.0) as i32;
                let (near, far) = (x + dir, x + dir * len);
                let beyond = x + dir * (len + 3);
                let (elo, ehi) = (near.min(far), near.max(far));
                let (lo, hi) = (near.min(beyond), near.max(beyond));
                // Some are angled, rising or sinking away from the ground they hang from.
                let pitch = [0.0, 0.0, 0.3, -0.3, 0.55, -0.55][(hash(x, y, 5) * 6.0) as usize % 6];
                let (base, tip_x) = if dir == 1 { (face(x + 1), face(x + len + 1)) } else { (face(x), face(x - len)) };
                let tip = Vec2::new(tip_x, top_of(y) + pitch * (tip_x - base).abs());
                let base = Vec2::new(base, top_of(y));
                let (l, r) = if dir == 1 { (base, tip) } else { (tip, base) };
                let open = if pitch == 0.0 {
                    clear(lo..=hi, y - HEADROOM..=y + 1) && clear(elo..=ehi, y + 2..=y + DROP)
                } else {
                    // Room along the slant, and a little free air past its tip.
                    incline_fits(t, l, r) && (1..=3).all(|k| (0..=HEADROOM * 2).all(|j| !t.solid_at(tip + Vec2::new(dir as f32 * k as f32 * PX, j as f32 * PX * 0.5))))
                };
                let neighbor = (1..=70).any(|k| (y - 3..=y + 3).any(|r| ledge(x + dir * k, r)));
                if open && !neighbor {
                    let center = (l + r) / 2.0;
                    if !dash_zone(center) {
                        found.push(Candidate {
                            loose: Loose { kind: Kind::of(t.style_at(center), hash(x, y, 1)), a: l, b: r, hold: (dir == 1, dir == -1) },
                            at: Vec2::new((x + dir * len / 2) as f32, y as f32),
                            luck: hash(x, y, 2),
                            bridge: false,
                        });
                    }
                }
            }
        }
    }

    // Most gaps get one, only some ledges do. Keep clear of the spawn and of each other.
    found.sort_by(|a, b| b.bridge.cmp(&a.bridge).then(a.luck.total_cmp(&b.luck)));
    let mut kept: Vec<Candidate> = Vec::new();
    for c in found {
        let wanted = if c.bridge { 0.9 } else { 0.4 };
        if hash(c.at.x as i32, c.at.y as i32, 4) > wanted || c.loose.center().distance(t.spawn) < SPAWN_CLEAR {
            continue;
        }
        let spacing = if c.bridge { SPACING } else { SPACING * 1.5 };
        if kept.iter().any(|k| k.at.distance(c.at) < spacing) {
            continue;
        }
        kept.push(c);
    }
    kept.into_iter().map(|c| c.loose).collect()
}

/// The first piece of each material, for the dev tour.
pub fn tour_spots(t: &Terrain) -> Vec<(String, Vec2)> {
    let mut seen = Vec::new();
    let mut out = Vec::new();
    for l in plan(t) {
        if !seen.contains(&l.kind) {
            seen.push(l.kind);
            out.push((format!("loose-{:?}", l.kind).to_lowercase(), l.center()));
        }
    }
    out
}

pub fn spawn(b: &mut Builder, t: &Terrain) {
    let pieces = plan(t);
    info!("loose platforms: {} placed", pieces.len());
    for l in &pieces {
        b.loose(l);
    }
}

fn platform_tilted(tilt: f32) -> bool {
    tilt.abs() >= 0.01
}

impl Builder<'_, '_, '_> {
    fn loose(&mut self, l: &Loose) {
        let (a, b) = l.line();
        let (w, h) = ((b - a).length(), l.kind.thickness());
        let dir = (b - a) / w;
        let tilt = dir.y.atan2(dir.x);
        // The body hangs below the top line, perpendicular to it.
        let center = (a + b) / 2.0 - Vec2::new(-dir.y, dir.x) * (h / 2.0);
        let mut platform = Platform::new(Vec2::new(w / 2.0, h / 2.0), l.kind.color(), true, false);
        platform.tilt = if tilt.abs() < 0.01 { 0.0 } else { tilt };
        let e = self.root(center, platform, Some(Motion::crumble_in(l.kind.delay())));
        if platform_tilted(tilt) {
            self.commands.entity(e).insert(Transform::from_translation(center.extend(0.0)).with_rotation(Quat::from_rotation_z(tilt)));
        }
        match l.kind {
            Kind::Concrete => self.concrete_body(e, w, h),
            Kind::Metal => self.metal_body(e, w, h),
            Kind::Rock => self.rock_body(e, w, h),
            Kind::Planks => self.planks_body(e, w, h),
        }
        // What stays behind where it was fixed.
        if l.hold.0 {
            self.remnant(e, l.kind, l.a, 1.0);
        }
        if l.hold.1 {
            self.remnant(e, l.kind, l.b, -1.0);
        }
    }

    fn concrete_body(&mut self, e: Entity, w: f32, h: f32) {
        let (hw, hh) = (w / 2.0, h / 2.0);
        let mut outline = Vec::new();
        let mut x = -hw + self.rng.range(2.0, 8.0);
        while x < hw - 8.0 {
            outline.push(Vec2::new(x, hh + self.rng.range(-1.2, 0.0)));
            x += self.rng.range(10.0, 20.0);
        }
        outline.push(Vec2::new(hw - self.rng.range(2.0, 6.0), hh));
        let top_edge = outline.clone();
        outline.push(Vec2::new(hw + self.rng.range(-2.0, 2.0), self.rng.range(-3.0, 3.0)));
        let mut x = hw - self.rng.range(4.0, 12.0);
        while x > -hw + 8.0 {
            outline.push(Vec2::new(x, -hh - self.rng.range(-1.0, 4.0)));
            x -= self.rng.range(8.0, 18.0);
        }
        outline.push(Vec2::new(-hw + self.rng.range(-2.0, 2.0), self.rng.range(-3.0, 3.0)));
        outline.reverse();
        let mat = self.mats.concrete_dark.clone();
        self.child(fill(&outline, |p| 0.55 + 0.45 * ((p.y + hh) / h).clamp(0.0, 1.0)), &mat, Transform::default(), e);
        self.rim(e, &top_edge);
        for _ in 0..2 + (w / 60.0) as i32 {
            let start = Vec2::new(self.rng.range(-hw + 8.0, hw - 8.0), hh);
            self.crack(e, start, h + 6.0);
        }
    }

    fn metal_body(&mut self, e: Entity, w: f32, h: f32) {
        let (hw, hh) = (w / 2.0, h / 2.0);
        let outline = [
            Vec2::new(-hw, -hh + self.rng.range(0.0, 2.0)),
            Vec2::new(0.0, -hh - self.rng.range(0.0, 1.0)),
            Vec2::new(hw, -hh + self.rng.range(0.0, 2.0)),
            Vec2::new(hw, hh),
            Vec2::new(0.0, hh),
            Vec2::new(-hw, hh),
        ];
        let rust = self.mats.rust.clone();
        self.child(fill(&outline, |p| 0.6 + 0.4 * ((p.y + hh) / h)), &rust, Transform::default(), e);
        // The frame under the grating, and its ribs.
        let (dark, soot) = (self.mats.steel_dark.clone(), self.mats.soot.clone());
        self.child(Rectangle::new(w, 3.0).into(), &dark, Transform::from_xyz(0.0, -hh + 1.5, 0.1), e);
        let mut x = -hw + 9.0;
        while x < hw - 5.0 {
            self.child(Rectangle::new(2.0, h - 4.0).into(), &soot, Transform::from_xyz(x, -1.0, 0.08), e);
            x += self.rng.range(14.0, 20.0);
        }
        let mut x = -hw + 5.0;
        while x < hw - 3.0 {
            self.child(Circle::new(1.3).into(), &soot, Transform::from_xyz(x, hh - 2.6, 0.12), e);
            x += 10.0;
        }
        // Rusted-through holes.
        for _ in 0..(w / 50.0) as i32 {
            let p = Vec2::new(self.rng.range(-hw + 8.0, hw - 8.0), self.rng.range(-2.0, 1.0));
            let hole = fill(&ellipse_pts(self.rng.range(2.5, 5.0), 1.6, 10), |_| 0.55);
            self.child(hole, &soot, Transform::from_translation(p.extend(0.15)), e);
        }
        self.rim(e, &[Vec2::new(-hw, hh), Vec2::new(0.0, hh), Vec2::new(hw, hh)]);
    }

    fn rock_body(&mut self, e: Entity, w: f32, h: f32) {
        let (hw, hh) = (w / 2.0, h / 2.0);
        let mut outline = vec![Vec2::new(-hw + 2.0, hh), Vec2::new(hw - 2.0, hh), Vec2::new(hw + self.rng.range(-1.0, 2.0), 0.0)];
        let top_edge = vec![Vec2::new(-hw + 2.0, hh), Vec2::new(0.0, hh - self.rng.range(0.0, 1.0)), Vec2::new(hw - 2.0, hh)];
        // Jagged underside, like a shelf of the cave's own rock that cracked away.
        let mut x = hw - 5.0;
        let mut down = true;
        while x > -hw + 6.0 {
            let y = if down { -hh - self.rng.range(0.0, 5.0) } else { -hh + self.rng.range(2.0, 6.0) };
            outline.push(Vec2::new(x, y));
            down = !down;
            x -= self.rng.range(5.0, 11.0);
        }
        outline.push(Vec2::new(-hw + self.rng.range(-2.0, 1.0), 0.0));
        outline.reverse();
        let mat = self.mats.rock.clone();
        self.child(fill(&outline, |p| 0.5 + 0.5 * ((p.y + hh) / h).clamp(0.0, 1.0)), &mat, Transform::default(), e);
        self.rim(e, &top_edge);
        let soot = self.mats.soot.clone();
        for _ in 0..(1.0 + w / 60.0) as i32 {
            let mut pts = vec![Vec2::new(self.rng.range(-hw + 6.0, hw - 6.0), hh - 2.0)];
            for _ in 0..3 {
                let p = pts[pts.len() - 1];
                pts.push(Vec2::new(p.x + self.rng.range(-4.0, 4.0), p.y - h * 0.3));
            }
            self.child(stroke(&pts, |t| 1.8 * (1.0 - t) + 0.3, 1.0, 1.0, |_| 1.0), &soot, Transform::from_xyz(0.0, 0.0, 0.2), e);
        }
    }

    fn planks_body(&mut self, e: Entity, w: f32, h: f32) {
        let (hw, hh) = (w / 2.0, h / 2.0);
        let (light, dark, soot) = (self.mats.plank.clone(), self.mats.plank_dark.clone(), self.mats.soot.clone());
        // Two stringers underneath.
        for x in [-hw * 0.6, hw * 0.6] {
            self.child(Rectangle::new(4.0, h * 0.6).into(), &dark, Transform::from_xyz(x, -hh * 0.4, 0.0), e);
        }
        let n = (w / 13.0).round().max(2.0) as i32;
        let pw = w / n as f32;
        for i in 0..n {
            let x = -hw + pw * (i as f32 + 0.5);
            let ph = self.rng.range(h * 0.6, h);
            let mat = if i % 2 == 0 { &light } else { &dark };
            // Planks at the ends are a little crooked.
            let tilt = if i == 0 || i == n - 1 { self.rng.range(-0.08, 0.08) } else { 0.0 };
            let tf = Transform::from_xyz(x, hh - ph / 2.0, 0.05 + 0.01 * i as f32).with_rotation(Quat::from_rotation_z(tilt));
            self.child(Rectangle::new(pw - 1.4, ph).into(), mat, tf, e);
            self.child(Circle::new(0.9).into(), &soot, Transform::from_xyz(x, hh - 2.2, 0.3), e);
        }
        self.rim(e, &[Vec2::new(-hw, hh), Vec2::new(hw, hh)]);
    }

    /// The broken contact point on the ground's face (`at`: face x, top y;
    /// `side` +1 = pointing right, into the gap). Hidden until the piece falls.
    fn remnant(&mut self, owner: Entity, kind: Kind, at: Vec2, side: f32) {
        let h = kind.thickness();
        let root = self
            .commands
            .spawn((
                Remnant { owner },
                Transform::from_translation(at.extend(0.1)).with_scale(Vec3::new(side, 1.0, 1.0)),
                Visibility::Hidden,
                TerrainPart,
            ))
            .id();
        match kind {
            Kind::Concrete => {
                // A jagged wedge of concrete still stuck to the wall, with rebar sticking out.
                let r = &mut self.rng;
                let outline = [
                    Vec2::new(-1.0, -0.5),
                    Vec2::new(r.range(5.0, 8.0), -r.range(0.5, 2.0)),
                    Vec2::new(r.range(8.0, 12.0), -r.range(4.0, 7.0)),
                    Vec2::new(r.range(4.0, 7.0), -h * 0.6),
                    Vec2::new(r.range(7.0, 11.0), -h + 3.0),
                    Vec2::new(-1.0, -h),
                ];
                let mat = self.mats.concrete_dark.clone();
                self.child(fill(&outline, |p| 0.5 + 0.5 * (1.0 + p.y / h)), &mat, Transform::default(), root);
                for _ in 0..2 + usize::from(self.rng.chance(0.5)) {
                    let base = Vec2::new(3.0, -self.rng.range(3.0, h - 3.0));
                    let dir = Vec2::new(1.0, self.rng.range(-0.55, 0.1)).normalize();
                    self.rebar(root, base, dir);
                }
            }
            Kind::Metal => {
                // The bolted bracket and a strip of plate torn off it, hanging bent.
                let (dark, rust, soot) = (self.mats.steel_dark.clone(), self.mats.rust.clone(), self.mats.soot.clone());
                self.child(Rectangle::new(4.0, h).into(), &dark, Transform::from_xyz(1.0, -h / 2.0, 0.0), root);
                let torn = [Vec2::new(2.0, -0.5), Vec2::new(11.0, -1.5), Vec2::new(7.0, -5.0), Vec2::new(13.0, -h * 0.75), Vec2::new(2.0, -h + 0.5)];
                self.child(fill(&torn, |p| 0.6 + 0.4 * (1.0 + p.y / h)), &rust, Transform::from_xyz(0.0, 0.0, 0.02), root);
                for y in [-2.5, -h + 2.5] {
                    self.child(Circle::new(1.1).into(), &soot, Transform::from_xyz(2.0, y, 0.1), root);
                }
                let strip = smooth_open(&[Vec2::new(10.0, -3.0), Vec2::new(16.0, -10.0), Vec2::new(15.0, -20.0 - self.rng.range(0.0, 6.0))], 6);
                self.child(stroke(&strip, |t| 4.0 * (1.0 - t * 0.6), 1.0, 0.6, |_| 1.0), &rust, Transform::from_xyz(0.0, 0.0, -0.05), root);
            }
            Kind::Rock => {
                // A stump of the shelf, snapped off in spikes, and a fang under it.
                let r = &mut self.rng;
                let outline = [
                    Vec2::new(-1.0, -0.5),
                    Vec2::new(r.range(6.0, 9.0), -r.range(1.0, 3.0)),
                    Vec2::new(r.range(4.0, 6.0), -h * 0.35),
                    Vec2::new(r.range(10.0, 14.0), -h * 0.7),
                    Vec2::new(r.range(3.0, 6.0), -h - 3.0),
                    Vec2::new(-1.0, -h - 4.0),
                ];
                let mat = self.mats.rock.clone();
                self.child(fill(&outline, |p| 0.5 + 0.5 * (1.0 + p.y / (h + 4.0)).clamp(0.0, 1.0)), &mat, Transform::default(), root);
                let fang = [Vec2::new(-1.0, -h - 3.0), Vec2::new(5.0, -h - 4.0), Vec2::new(2.0, -h - self.rng.range(8.0, 14.0))];
                self.child(fill(&fang, |_| 0.6), &mat, Transform::default(), root);
            }
            Kind::Planks => {
                // Splintered plank ends, a nail and a frayed rope.
                let (light, dark, soot) = (self.mats.plank.clone(), self.mats.plank_dark.clone(), self.mats.soot.clone());
                for (i, y) in [-h * 0.15, -h * 0.5, -h * 0.82].into_iter().enumerate() {
                    let len = self.rng.range(5.0, 15.0);
                    let tip = [Vec2::new(len - 4.0, 2.0), Vec2::new(len + 3.0, self.rng.range(-1.0, 1.0)), Vec2::new(len - 4.0, -2.0)];
                    let mat = if i % 2 == 0 { &light } else { &dark };
                    self.child(Rectangle::new(len, 4.0).into(), mat, Transform::from_xyz(len / 2.0, y, 0.0), root);
                    self.child(fill(&tip, |_| 0.8), mat, Transform::from_xyz(0.0, y, 0.0), root);
                }
                self.child(Circle::new(1.0).into(), &soot, Transform::from_xyz(2.5, -h * 0.5, 0.2), root);
                let rope = smooth_open(&[Vec2::new(3.0, -h), Vec2::new(5.0, -h - 6.0), Vec2::new(3.0, -h - 14.0)], 6);
                self.child(stroke(&rope, |_| 1.6, 1.0, 0.6, |_| 1.0), &dark, Transform::from_xyz(0.0, 0.0, -0.05), root);
            }
        }
    }
}
