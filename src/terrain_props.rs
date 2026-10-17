//! Props along the terrain, styled per area, so the shapes of the sketch read
//! as real places: rebar, cables and broken windows on the crashed buildings,
//! tires, fridges and bags on the trash cliff, dripping pipes and grates in the
//! sewer, broken benches, bent lamp posts and barbed wire in the park.
//!
//! Decoration only (no collision), rebuilt with the terrain (`TerrainPart`).
//! Props sit just *behind* the terrain tiles, so their bases and embedded ends
//! sink into the ground behind its lit rim; details lying on the terrain's face
//! (window holes, grates, slime) are drawn over it. Both stay behind the
//! platforms and the player.
//!
//! Places come from the grid (top surfaces with headroom, ledge corners, wall
//! faces, ceilings); each spot seeds its own RNG from its world position, so
//! editing one corner of the sketch doesn't reshuffle the rest.

use std::{
    sync::Arc,
    time::Duration,
};

use bevy::{
    platform::time::Instant,
    prelude::*,
    sprite_render::AlphaMode2d,
    tasks::{futures::check_ready, AsyncComputeTaskPool, Task},
};

use crate::{
    shapes::*,
    sketch::Cell,
    terrain::{Style, Terrain, PX},
    terrain_art::{self, TerrainPart},
};

// One module per newer region, each placing its own props through `Hooks`.
mod subway;
mod treatment;
mod sinkhole;
mod catacombs;
mod mall;
mod dam;
mod bunker;
mod cave;
mod vines;
mod moss;
mod curtains;
mod chemicals;

/// How a region places its props (the older styles are matched inline below).
struct Hooks {
    spacing: fn(Kind) -> (f32, f32),
    ground: fn(&mut Props, IVec2, f32) -> bool,
    edge: fn(&mut Props, Vec2, f32) -> bool,
    wall: fn(&mut Props, Vec2, f32) -> bool,
    ceiling: fn(&mut Props, Vec2) -> bool,
}

fn hooks(style: Style) -> Option<&'static Hooks> {
    Some(match style {
        Style::Subway => &subway::HOOKS,
        Style::Treatment => &treatment::HOOKS,
        Style::Sinkhole => &sinkhole::HOOKS,
        Style::Catacombs => &catacombs::HOOKS,
        Style::Mall => &mall::HOOKS,
        Style::Dam => &dam::HOOKS,
        Style::Bunker => &bunker::HOOKS,
        Style::Cave => &cave::HOOKS,
        Style::Concrete | Style::Trash | Style::Sewer | Style::Park => return None,
    })
}

/// Depth of every prop: just behind the terrain tiles.
const BACK: f32 = terrain_art::Z - 0.25;
/// Depth of props standing indoors, in front of the dark interiors (which
/// would hide them), still behind the platforms even with their `OVER` parts.
const INDOOR: f32 = terrain_art::Z + 0.2;
/// Local depth of the parts drawn over the terrain (world z −0.5).
const OVER: f32 = 0.75;
/// Props are drawn in their own units, then scaled up to the world.
const S: f32 = 1.7;

/// Materials (the vertex shades multiply these).
#[derive(Clone, Copy)]
enum M {
    Concrete,
    Rim,
    Soot,
    Rust,
    Steel,
    Iron,
    Rubber,
    Enamel,
    Slime,
    Wood,
    Bronze,
    Plastic,
    /// Cobweb silk: see-through (`WEB_ALPHA`).
    Web,
    /// Silk of the curtains across corridors (`curtains.rs`): denser than `Web`.
    Silk,
}

/// Opacity of the cobweb silk.
const WEB_ALPHA: f32 = 0.15;
/// Opacity of the silk of the corridor curtains.
const SILK_ALPHA: f32 = 0.25;

const COLORS: [[f32; 3]; 14] = [
    [0.40, 0.38, 0.36],
    [0.70, 0.67, 0.62],
    [0.09, 0.09, 0.10],
    [0.44, 0.27, 0.18],
    [0.42, 0.44, 0.46],
    [0.17, 0.17, 0.18],
    [0.15, 0.15, 0.15],
    [0.68, 0.66, 0.60],
    [0.44, 0.52, 0.28],
    [0.36, 0.28, 0.20],
    [0.36, 0.32, 0.24],
    [0.40, 0.40, 0.41],
    [0.78, 0.76, 0.72],
    [0.86, 0.85, 0.82],
];

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Kind {
    /// Top surface with headroom.
    Ground,
    /// Where the ground ends at a drop (`dir` = the open side).
    Edge,
    /// A straight vertical face (`dir` = the open side).
    Wall,
    /// Underside of a thick overhang.
    Ceiling,
}

struct Site {
    cell: IVec2,
    kind: Kind,
    dir: i32,
    /// Stable per spot: the visiting order and the RNG seed.
    key: u32,
}

/// How long the props may take each frame, so a (re)load never stalls the game.
const BUDGET: Duration = Duration::from_millis(4);

/// Props being placed (`TerrainPart`, so a reload drops the job): the spots
/// are found on the task pool, then visited a few milliseconds per frame
/// (`place_props`), always in the same order, so the result doesn't depend
/// on the frame rate.
#[derive(Component)]
pub struct PropJob {
    t: Arc<Terrain>,
    finding: Option<Task<(Vec<Site>, Vec<(IVec2, bool)>)>>,
    sites: Vec<Site>,
    facades: Vec<(IVec2, bool)>,
    next: usize,
    taken: Vec<(Vec2, Kind)>,
    statues: Vec<Vec2>,
    vined: Vec<Vec2>,
    hints: Vec<Vec2>,
    yellow: Vec<Vec2>,
    vents: Vec<Vec2>,
    curtains: Vec<Vec2>,
    pink: Vec<Vec2>,
    drums: Vec<Vec2>,
    stains: Vec<Vec2>,
    blue: Blue,
    mats: Vec<Handle<ColorMaterial>>,
}

/// What the Weaver's blue needs to remember while the props are placed.
#[derive(Default)]
struct Blue {
    /// Where the blue hints are (`moss`).
    spots: Vec<Vec2>,
    /// The blue materials of the metal and plastic parts, one per kind and distance band
    /// (so the parts of a band fade in together and share one material).
    mats: std::collections::HashMap<(i32, u8), Handle<ColorMaterial>>,
}

/// How far apart the bands of blue materials are (world units).
const BLUE_BAND: f32 = 250.0;

/// The blue a metal or plastic part turns once the Weaver's color is back: bluish steel,
/// darker iron, bright plastic. Rust stays rust.
fn blue_of(m: M) -> Option<Color> {
    match m {
        M::Steel => Some(Color::srgb(0.50, 0.63, 0.88)),
        M::Iron => Some(Color::srgb(0.25, 0.33, 0.54)),
        M::Plastic => Some(Color::srgb(0.20, 0.42, 0.88)),
        _ => None,
    }
}

pub fn spawn(commands: &mut Commands, materials: &mut Assets<ColorMaterial>, terrain: &Terrain) {
    let mats = COLORS
        .iter()
        .enumerate()
        .map(|(i, &[r, g, b])| {
            let alpha = match i {
                i if i == M::Web as usize => Some(WEB_ALPHA),
                i if i == M::Silk as usize => Some(SILK_ALPHA),
                _ => None,
            };
            match alpha {
                Some(a) => materials.add(ColorMaterial { color: Color::srgba(r, g, b, a), alpha_mode: AlphaMode2d::Blend, ..default() }),
                None => materials.add(Color::srgb(r, g, b)),
            }
        })
        .collect();
    let t = Arc::new(terrain.clone());
    let found = t.clone();
    let finding = AsyncComputeTaskPool::get().spawn(async move {
        let mut sites = find_sites(&found);
        sites.sort_by_key(|s| s.key);
        (sites, facade_spots(&found))
    });
    let job = PropJob { t, finding: Some(finding), sites: Vec::new(), facades: Vec::new(), next: 0, taken: crate::level::loose_marks(terrain).into_iter().map(|p| (p, Kind::Ground)).collect(), statues: Vec::new(), vined: Vec::new(), hints: Vec::new(), yellow: Vec::new(), vents: Vec::new(), curtains: Vec::new(), pink: Vec::new(), drums: Vec::new(), stains: Vec::new(), blue: Blue::default(), mats };
    commands.spawn((job, TerrainPart, terrain_art::WorldArt));
}

pub fn place_props(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut colors: ResMut<Assets<ColorMaterial>>,
    mut jobs: Query<(Entity, &mut PropJob)>,
) {
    for (e, mut job) in &mut jobs {
        let job = &mut *job;
        if let Some(task) = &mut job.finding {
            let Some((sites, facades)) = check_ready(task) else { continue };
            (job.sites, job.facades, job.finding) = (sites, facades, None);
        }
        let start = Instant::now();
        let t = job.t.clone();
        let mut p = Props {
            commands: &mut commands,
            meshes: &mut meshes,
            colors: &mut colors,
            mats: std::mem::take(&mut job.mats),
            t: &t,
            rng: Rng(1),
            taken: std::mem::take(&mut job.taken),
            statues: std::mem::take(&mut job.statues),
            vined: std::mem::take(&mut job.vined),
            hints: std::mem::take(&mut job.hints),
            yellow: std::mem::take(&mut job.yellow),
            vents: std::mem::take(&mut job.vents),
            curtains: std::mem::take(&mut job.curtains),
            pink: std::mem::take(&mut job.pink),
            drums: std::mem::take(&mut job.drums),
            stains: std::mem::take(&mut job.stains),
            blue: std::mem::take(&mut job.blue),
            root_at: Vec2::ZERO,
            tilt: 0.0,
            depth: BACK,
        };
        let total = job.sites.len() + job.facades.len();
        while job.next < total && start.elapsed() < BUDGET {
            match job.sites.get(job.next) {
                Some(s) => p.visit(s),
                None => {
                    let (c, roof) = job.facades[job.next - job.sites.len()];
                    p.facade(c, roof)
                }
            }
            job.next += 1;
        }
        (job.mats, job.taken, job.statues, job.vined, job.hints, job.yellow, job.vents, job.curtains) = (p.mats, p.taken, p.statues, p.vined, p.hints, p.yellow, p.vents, p.curtains);
        (job.pink, job.drums, job.stains, job.blue) = (p.pink, p.drums, p.stains, p.blue);
        if job.next >= total {
            commands.entity(e).try_despawn();
        }
    }
}

/// Whole grid cells covering `world` units (sizes are in world units, so the
/// props keep their size whatever the sketch's pixel scale).
fn cells(world: f32) -> i32 {
    (world / PX).ceil().max(1.0) as i32
}

fn find_sites(t: &Terrain) -> Vec<Site> {
    // Headroom over ground, the drop beside an edge, the face of a wall, the space under a ceiling.
    let (head, drop, face_h, reach, under) = (cells(30.0), cells(25.0), cells(15.0), cells(30.0), cells(50.0));
    // Room to move: open sky or the inside of a structure (halls, tunnels,
    // corridors), so indoor regions get edges, walls and ceilings too.
    let air = |c: IVec2| !t.solid(c);
    let solid = |c: IVec2| t.solid(c);
    let (x, y) = (IVec2::X, IVec2::Y);
    let size = t.size();
    let mut out = Vec::new();
    let mut add = |c: IVec2, kind: Kind, dir: i32| {
        // World-relative cell, so the key survives the canvas growing.
        let w = (t.cell_min(c) / PX).round().as_ivec2();
        let key = seed(w.x * 4 + kind as i32, w.y * 3 + dir);
        out.push(Site { cell: c, kind, dir, key });
    };
    for cy in 1..size.y - 1 {
        for cx in 1..size.x - 1 {
            let c = IVec2::new(cx, cy);
            if !solid(c) {
                continue;
            }
            // Grid rows grow downward: `c - y` is above.
            if (1..=head).all(|k| !solid(c - y * k)) {
                let mut edge = false;
                for dir in [-1, 1] {
                    let side = c + x * dir;
                    if (-head..=drop).all(|k| air(side + y * k)) && (1..=head).all(|k| air(c - y * k)) {
                        add(c, Kind::Edge, dir);
                        edge = true;
                    }
                }
                if !edge {
                    add(c, Kind::Ground, 0);
                }
            }
            if (1..=cells(10.0)).all(|k| solid(c - y * k)) && solid(c - x) && solid(c + x) && (1..=under).all(|k| air(c + y * k)) {
                add(c, Kind::Ceiling, 0);
            }
            for dir in [-1, 1] {
                let face = (-face_h..=face_h).all(|k| solid(c + y * k) && air(c + x * dir + y * k));
                let out_there = (2..=reach).all(|k| [-face_h, 0, face_h].into_iter().all(|j| air(c + x * dir * k + y * j)));
                if face && out_there {
                    add(c, Kind::Wall, dir);
                }
            }
        }
    }
    out
}

/// Concrete blocks are buildings: windows on a grid of floors across their
/// faces. The candidate cells (`Props::facade` checks there's room).
fn facade_spots(t: &Terrain) -> Vec<(IVec2, bool)> {
    // Window spacing in world units, as whole cells.
    let step = IVec2::new(cells(75.0), cells(crate::terrain::FLOOR_HEIGHT));
    let size = t.size();
    let mut out = Vec::new();
    for cy in 0..size.y {
        for cx in 0..size.x {
            let c = IVec2::new(cx, cy);
            let w = (t.cell_min(c) / PX).round().as_ivec2();
            if w.x.rem_euclid(step.x) != 0 || w.y.rem_euclid(step.y) != 0 || !t.solid(c) {
                continue;
            }
            if t.style_at(t.cell_min(c) + Vec2::splat(PX / 2.0)) == Style::Concrete {
                out.push((c, false));
            }
        }
    }
    // Interiors of the Venom Mother's region: ivy hanging from the top of the
    // dark walls, and patches in the middle of them.
    if let Some(area) = t.wasp.and_then(|w| t.area_at(w)).map(|a| a.rect) {
        let (gap, mid) = (cells(26.0), IVec2::new(cells(70.0), cells(90.0)));
        for cy in 1..size.y {
            for cx in 0..size.x {
                let c = IVec2::new(cx, cy);
                if t.cell(c) != Cell::Back || !area.contains(t.cell_min(c) + Vec2::splat(PX / 2.0)) {
                    continue;
                }
                let w = (t.cell_min(c) / PX).round().as_ivec2();
                let top = t.cell(c - IVec2::Y) != Cell::Back && w.x.rem_euclid(gap) == 0;
                if top || (w.x.rem_euclid(mid.x) == 0 && w.y.rem_euclid(mid.y) == 0) {
                    out.push((c, false));
                }
            }
        }
        // The tops of the big blocks, to drape their faces.
        let every = cells(45.0);
        for cy in 1..size.y {
            for cx in 0..size.x {
                let c = IVec2::new(cx, cy);
                let w = (t.cell_min(c) / PX).round().as_ivec2();
                if w.x.rem_euclid(every) == 0 && t.solid(c) && !t.solid(c - IVec2::Y) && area.contains(t.cell_min(c) + Vec2::splat(PX / 2.0)) {
                    out.push((c, true));
                }
            }
        }
    }
    out
}

fn seed(a: i32, b: i32) -> u32 {
    let mut h = (a as u32).wrapping_mul(0x9e37_79b1) ^ (b as u32).wrapping_mul(0x85eb_ca77);
    h ^= h >> 15;
    h = h.wrapping_mul(0x2c1b_3c6d);
    h ^= h >> 12;
    h | 1
}

struct Props<'a, 'w, 's> {
    commands: &'a mut Commands<'w, 's>,
    meshes: &'a mut Assets<Mesh>,
    colors: &'a mut Assets<ColorMaterial>,
    mats: Vec<Handle<ColorMaterial>>,
    t: &'a Terrain,
    rng: Rng,
    /// Spots already used (or deliberately left empty).
    taken: Vec<(Vec2, Kind)>,
    /// Park areas that already have their statue (by area center).
    statues: Vec<Vec2>,
    /// Where the vines of the Venom Mother's region are (so none of it is left bare).
    vined: Vec<Vec2>,
    /// Where the hints of green of the other regions are (`moss`).
    hints: Vec<Vec2>,
    /// The same for the yellow ones (the Gutter King's).
    yellow: Vec<Vec2>,
    /// Sulfur vents (`moss::vent`).
    vents: Vec<Vec2>,
    /// Web curtains across corridors (`curtains`).
    curtains: Vec<Vec2>,
    /// Where the pink flowers are (`moss`), and the Razorwing's chemical drums and stains (`chemicals`).
    pink: Vec<Vec2>,
    drums: Vec<Vec2>,
    stains: Vec<Vec2>,
    /// The Weaver's blue (`Blue`).
    blue: Blue,
    /// Where the last root was placed (the blue copies of its parts fade in by distance from it).
    root_at: Vec2,
    /// Ground slope under the prop being drawn (added to its root's angle).
    tilt: f32,
    /// Depth of the prop being drawn.
    depth: f32,
}

/// Ground props: (what, half width, height) in world units.
#[derive(Clone, Copy)]
enum Ground {
    Rubble,
    Window,
    Slab,
    Tire,
    Tires,
    Fridge,
    Bags,
    Bars,
    Puddle,
    PipeSegment,
    Valve,
    Bench,
    Lamp,
    Statue,
    Wire,
    Coil,
    Shrub,
}

impl Ground {
    fn size(self) -> (f32, f32) {
        match self {
            Ground::Rubble => (16.0, 14.0),
            Ground::Window => (24.0, 52.0),
            Ground::Slab => (28.0, 34.0),
            Ground::Tire => (14.0, 24.0),
            Ground::Tires => (16.0, 28.0),
            Ground::Fridge => (18.0, 50.0),
            Ground::Bags => (16.0, 18.0),
            Ground::Bars => (6.0, 40.0),
            Ground::Puddle => (16.0, 6.0),
            Ground::PipeSegment => (30.0, 16.0),
            Ground::Valve => (5.0, 30.0),
            Ground::Bench => (30.0, 34.0),
            Ground::Lamp => (5.0, 120.0),
            Ground::Statue => (66.0, 44.0),
            Ground::Wire => (40.0, 26.0),
            Ground::Coil => (34.0, 16.0),
            Ground::Shrub => (5.0, 60.0),
        }
    }
}

impl Props<'_, '_, '_> {
    fn visit(&mut self, s: &Site) {
        let t = self.t;
        let min = t.cell_min(s.cell);
        let at = match s.kind {
            Kind::Ground => min + Vec2::new(PX / 2.0, PX),
            Kind::Edge => min + Vec2::new(if s.dir > 0 { PX } else { 0.0 }, PX),
            Kind::Wall => min + Vec2::new(if s.dir > 0 { PX } else { 0.0 }, PX / 2.0),
            Kind::Ceiling => min + Vec2::new(PX / 2.0, 0.0),
        };
        let style = t.style_at(at);
        // The Razorwing's plant is full of its chemical drums.
        if chemicals::in_lair(t, at) && chemicals::place(self, s, at) {
            self.depth = BACK;
            self.taken.push((at, s.kind));
            return;
        }
        let green_zone = t.wasp.and_then(|w| t.area_at(w)).is_some_and(|a| a.rect.contains(at));
        if !green_zone {
            moss::hint(self, s, at, style, moss::Tone::Green);
        }
        if s.kind == Kind::Ground {
            curtains::corridor(self, s, at, style);
        }
        moss::vent(self, s, at, style);
        moss::hint(self, s, at, style, moss::Tone::Yellow);
        moss::hint(self, s, at, style, moss::Tone::Pink);
        moss::hint(self, s, at, style, moss::Tone::Blue);
        let hooks = hooks(style);
        let (gap, density) = match (s.kind, style) {
            _ if let Some(h) = hooks => (h.spacing)(s.kind),
            (Kind::Ground, Style::Concrete) => (75.0, 0.6),
            (Kind::Ground, Style::Trash) => (45.0, 0.9),
            (Kind::Ground, Style::Sewer) => (60.0, 0.7),
            (Kind::Ground, Style::Park) => (55.0, 0.85),
            (Kind::Edge, _) => (35.0, 0.85),
            (Kind::Wall, Style::Park) => (80.0, 0.5),
            (Kind::Wall, _) => (50.0, 0.75),
            (Kind::Ceiling, _) => (45.0, 0.6),
            (Kind::Ground, _) => unreachable!("regional styles space props through their hooks"),
        };
        // Spacing in prop units (scaled with the props).
        let crowded = self.taken.iter().any(|&(p, k)| {
            let d = p.distance(at);
            d < 35.0 * S || (k == s.kind && d < gap * S)
        });
        if crowded {
            return;
        }
        self.rng = Rng(s.key);
        self.rng.next();
        if !self.rng.chance(density) {
            // Leave this stretch bare (claiming it keeps the density honest).
            self.taken.push((at, s.kind));
            return;
        }
        let flip = s.dir as f32;
        // Props over an interior go in front of it (it would hide them).
        let open_side = match s.kind {
            Kind::Ground => None,
            Kind::Edge => Some(s.cell + IVec2::new(s.dir, 1)),
            Kind::Wall => Some(s.cell + IVec2::X * s.dir),
            Kind::Ceiling => Some(s.cell + IVec2::Y),
        };
        if open_side.is_some_and(|c| t.cell(c) == Cell::Back) {
            self.depth = INDOOR;
        }
        // The Venom Mother's region is overgrown with the vines she'll give back.
        let vined = s.kind != Kind::Ground
            && green_zone
            && { let k = vines::share(self, at, s.kind, flip).max(vines::starved(self, at)); self.rng.chance(k) };
        if vined {
            vines::plant(self, at, flip, s.kind, None, false);
            self.vined.push(at);
            self.depth = BACK;
            self.taken.push((at, s.kind));
            return;
        }
        let placed = match (s.kind, hooks) {
            (Kind::Ground, Some(h)) => (h.ground)(self, s.cell, at.x),
            (Kind::Edge, Some(h)) => (h.edge)(self, at, flip),
            (Kind::Wall, Some(h)) => (h.wall)(self, at, flip),
            (Kind::Ceiling, Some(h)) => (h.ceiling)(self, at),
            (Kind::Ground, None) => self.ground(s.cell, at.x, style),
            (Kind::Edge, None) => self.edge(at, flip, style),
            (Kind::Wall, None) => self.wall(at, flip, style),
            (Kind::Ceiling, None) => self.ceiling(at, style),
        };
        self.depth = BACK;
        if placed {
            self.taken.push((at, s.kind));
        }
    }

    // --- Placing ---

    /// Ground under a prop `half` wide and `head` tall (prop units) centered
    /// on `cell`, as (height at its middle, slope angle). Most props sit on the
    /// lowest top of a level stretch (so nothing floats); `tilt` ones may also
    /// lie along a straight slope. Also says if it stands inside a structure.
    fn fit(&self, cell: IVec2, half: f32, head: f32, tilt: bool) -> Option<(f32, f32, bool)> {
        let t = self.t;
        let (hc, vc) = ((half * S / PX).ceil() as i32, (head * S / PX).ceil() as i32 + 1);
        let mut tops = vec![0.0; (2 * hc + 1) as usize];
        let mut indoor = false;
        for side in [-1, 1] {
            let mut row = cell.y;
            for k in 0..=hc {
                let x = cell.x + side * k;
                row = (-2..=2)
                    .map(|d| row + d)
                    .filter(|&r| t.solid(IVec2::new(x, r)) && !t.solid(IVec2::new(x, r - 1)))
                    .min_by_key(|&r| (r - row).abs())?;
                if (1..=vc).any(|k| t.solid(IVec2::new(x, row - k))) {
                    return None;
                }
                indoor |= (1..=vc).any(|k| t.cell(IVec2::new(x, row - k)) == Cell::Back);
                tops[(hc + side * k) as usize] = t.cell_min(IVec2::new(x, row)).y + PX;
            }
        }
        let (lo, hi) = tops.iter().fold((f32::MAX, f32::MIN), |(a, b), &h| (a.min(h), b.max(h)));
        if hi - lo <= PX * if hc <= 2 { 2.01 } else { 1.01 } {
            return Some((lo, 0.0, indoor));
        }
        if !tilt {
            return None;
        }
        // Least-squares line through the tops.
        let x = |i: usize| (i as f32 - hc as f32) * PX;
        let mean = tops.iter().sum::<f32>() / tops.len() as f32;
        let slope = tops.iter().enumerate().map(|(i, h)| x(i) * (h - mean)).sum::<f32>() / (0..tops.len()).map(|i| x(i) * x(i)).sum::<f32>();
        let dev = tops.iter().enumerate().map(|(i, h)| (h - mean - slope * x(i)).abs()).fold(0.0, f32::max);
        let angle = slope.atan();
        (dev <= PX * 1.6 && angle.abs() <= 0.6).then_some((mean - PX * 0.3, angle, indoor))
    }

    /// Every cell of the box is solid ground (for things drawn on its face).
    fn inside(&self, center: Vec2, half: Vec2) -> bool {
        let m = half + Vec2::splat(PX * 1.5);
        let (a, b) = (self.t.to_cell(center + Vec2::new(-m.x, m.y)), self.t.to_cell(center + Vec2::new(m.x, -m.y)));
        (a.y..=b.y).all(|y| (a.x..=b.x).all(|x| self.t.solid(IVec2::new(x, y))))
    }

    fn ground(&mut self, cell: IVec2, x: f32, style: Style) -> bool {
        use Ground::*;
        let area = self.t.area_at(Vec2::new(x, self.t.cell_min(cell).y)).map(|a| a.rect.center());
        let mut picks: Vec<Ground> = Vec::new();
        if style == Style::Park && area.is_some_and(|c| !self.statues.contains(&c)) {
            picks.push(Statue);
        }
        let table: &[(Ground, f32)] = match style {
            Style::Concrete => &[(Rubble, 3.0), (Window, 2.0), (Slab, 2.0), (Bars, 1.0)],
            Style::Trash => &[(Tire, 2.0), (Tires, 1.5), (Fridge, 1.0), (Bags, 2.5), (Bars, 1.5)],
            Style::Sewer => &[(Puddle, 2.0), (PipeSegment, 1.5), (Valve, 1.5), (Rubble, 1.0)],
            Style::Park => &[(Bench, 2.0), (Lamp, 2.0), (Wire, 1.5), (Coil, 1.0), (Shrub, 1.5), (Rubble, 0.5)],
            _ => unreachable!("regional styles place props through their hooks"),
        };
        let total: f32 = table.iter().map(|e| e.1).sum();
        let mut r = self.rng.range(0.0, total);
        let pick = table.iter().find(|e| {
            r -= e.1;
            r <= 0.0
        });
        picks.push(pick.unwrap_or(&table[0]).0);
        // Something small if the pick doesn't fit.
        picks.push(match style {
            Style::Trash => Bags,
            Style::Sewer => Puddle,
            _ => Rubble,
        });
        for g in picks {
            let (half, head) = g.size();
            let tilt = matches!(g, Rubble | Window | Slab | Tire | Tires | Bags | PipeSegment | Coil | Wire);
            let Some((y, angle, indoor)) = self.fit(cell, half, head, tilt) else { continue };
            let at = Vec2::new(x, y);
            self.tilt = angle;
            self.depth = if indoor { INDOOR } else { BACK };
            match g {
                Rubble => self.rubble(at),
                Window => self.fallen_window(at),
                Slab => self.slab(at),
                Tire => self.tire_upright(at),
                Tires => self.tire_stack(at),
                Fridge => self.fridge(at),
                Bags => self.bags(at),
                Bars => self.bars(at),
                Puddle => self.puddle(at),
                PipeSegment => self.pipe_segment(at),
                Valve => self.valve(at),
                Bench => self.bench(at),
                Lamp => self.lamp(at),
                Statue => {
                    self.statues.extend(area);
                    self.statue(at);
                }
                Wire => self.barbed_wire(at),
                Coil => self.coil(at),
                Shrub => self.shrub(at),
            }
            self.tilt = 0.0;
            self.depth = BACK;
            return true;
        }
        false
    }

    fn edge(&mut self, at: Vec2, flip: f32, style: Style) -> bool {
        let e = self.root(at, flip, 0.0);
        match style {
            Style::Concrete => {
                self.rebar_burst(e);
                if self.rng.chance(0.4) {
                    let n = 1 + self.rng.below(2);
                    self.cables(e, Vec2::new(-3.0, -4.0), n, 0.0);
                }
            }
            Style::Trash => {
                if self.rng.chance(0.5) {
                    self.bent_bars_out(e);
                } else {
                    let r = self.rng.range(10.0, 14.0);
                    self.tire(e, Vec2::new(r * 0.15, -r * 0.9), r, 0.0);
                }
                if self.rng.chance(0.35) {
                    let (c, r) = (Vec2::new(self.rng.range(4.0, 10.0), -self.rng.range(14.0, 24.0)), self.rng.range(6.0, 8.0));
                    self.bag(e, c, r, 0.2);
                }
            }
            Style::Sewer => {
                let r = self.rng.range(5.0, 8.0);
                let y = -r - self.rng.range(4.0, 12.0);
                self.pipe(e, at, flip, Vec2::new(0.0, y), r);
            }
            Style::Park => {
                if self.rng.chance(0.5) {
                    self.wire_strand(e);
                } else {
                    self.roots(e, Vec2::new(-2.0, -3.0), 0.0);
                }
            }
            _ => unreachable!("regional styles place props through their hooks"),
        }
        true
    }

    fn wall(&mut self, at: Vec2, flip: f32, style: Style) -> bool {
        // Things on the wall's face, a little way in from the edge.
        let inset = |w: f32| at + Vec2::new(-flip * (w * S + PX * 2.0), 0.0);
        match style {
            Style::Concrete => {
                // Floors snapped off the face: rebar (the windows are on the facades).
                let e = self.root(at, flip, 0.0);
                for _ in 0..2 + self.rng.below(2) {
                    let base = Vec2::new(-2.0, self.rng.range(-10.0, 10.0));
                    let dir = Vec2::new(1.0, self.rng.range(-0.5, 0.4)).normalize();
                    self.rebar(e, base, dir, 0.0);
                }
            }
            Style::Trash => {
                let e = self.root(at, flip, 0.0);
                if self.rng.chance(0.5) {
                    let r = self.rng.range(11.0, 15.0);
                    self.tire(e, Vec2::new(r * 0.35, 0.0), r, 0.0);
                } else {
                    self.bent_bars_out(e);
                }
            }
            Style::Sewer => {
                let half = Vec2::new(12.0, 14.0);
                let c = inset(half.x);
                if self.rng.chance(0.45) && self.inside(c, half * S) {
                    let e = self.root(c, flip, 0.0);
                    self.grate(e, half);
                } else {
                    let e = self.root(at, flip, 0.0);
                    let r = self.rng.range(5.0, 9.0);
                    let y = self.rng.range(-6.0, 6.0);
                    self.pipe(e, at, flip, Vec2::new(0.0, y), r);
                }
            }
            Style::Park => {
                let e = self.root(at, flip, 0.0);
                let y = self.rng.range(-8.0, 8.0);
                self.roots(e, Vec2::new(-2.0, y), 0.0);
            }
            _ => unreachable!("regional styles place props through their hooks"),
        }
        true
    }

    fn ceiling(&mut self, at: Vec2, style: Style) -> bool {
        let e = self.root(at, 1.0, 0.0);
        match style {
            Style::Concrete => {
                let (x, n) = (self.rng.range(-6.0, 6.0), 1 + self.rng.below(3));
                self.cables(e, Vec2::new(x, 2.0), n, -1.0);
                if self.rng.chance(0.5) {
                    let base = Vec2::new(self.rng.range(-10.0, 10.0), 2.0);
                    let dir = Vec2::new(self.rng.range(-0.4, 0.4), -1.0).normalize();
                    self.rebar(e, base, dir, 0.0);
                }
            }
            Style::Trash => {
                self.cables(e, Vec2::new(0.0, 2.0), 1, -1.0);
                if self.rng.chance(0.6) {
                    let len = self.rng.range(14.0, 30.0);
                    let (c, r) = (Vec2::new(self.rng.range(-3.0, 3.0), -len - 6.0), self.rng.range(6.0, 8.0));
                    self.bag(e, c, r, OVER - 0.1);
                }
            }
            Style::Sewer => self.drips(e),
            Style::Park => self.roots(e, Vec2::new(0.0, 2.0), -1.0),
            _ => unreachable!("regional styles place props through their hooks"),
        }
        true
    }

    /// A blown-out window on a building's face (a spot from `facade_spots`),
    /// if the solid is deep enough to hold one.
    fn facade(&mut self, c: IVec2, roof: bool) {
        if roof {
            vines::facade_vine(self, c);
            self.depth = BACK;
            return;
        }
        let t = self.t;
        let w = (t.cell_min(c) / PX).round().as_ivec2();
        let at = t.cell_min(c) + Vec2::splat(PX / 2.0);
        self.rng = Rng(seed(w.x, w.y * 7 + 5));
        self.rng.next();
        if t.cell(c) == Cell::Back {
            vines::backdrop(self, c);
            return;
        }
        let half = Vec2::new(self.rng.range(10.0, 13.0), self.rng.range(13.0, 16.0));
        if !self.rng.chance(0.6) || !self.inside(at, half * S) {
            return;
        }
        let flip = if self.rng.chance(0.5) { 1.0 } else { -1.0 };
        let e = self.root(at, flip, 0.0);
        self.window_hole(e, half);
    }

    // --- Drawing primitives ---

    fn root(&mut self, at: Vec2, flip: f32, angle: f32) -> Entity {
        self.root_at = at;
        let tf = Transform::from_translation(at.extend(self.depth))
            .with_rotation(Quat::from_rotation_z(angle + self.tilt))
            .with_scale(Vec3::new(flip * S, S, 1.0));
        self.commands.spawn((tf, Visibility::default(), TerrainPart, terrain_art::WorldArt)).id()
    }

    fn part(&mut self, e: Entity, mesh: Mesh, m: M, z: f32) {
        self.part_blue(e, mesh, m, z, blue_of(m));
    }

    /// A part that turns `blue` (if it does) when the Weaver's color is back: the gray part
    /// in the world, and its blue copy on the color layer, hidden until then.
    fn part_blue(&mut self, e: Entity, mesh: Mesh, m: M, z: f32, blue: Option<Color>) {
        let mesh = self.meshes.add(mesh);
        if let Some(color) = blue {
            let dist = (self.root_at.distance(self.t.spider.unwrap_or(self.root_at)) / BLUE_BAND).round() * BLUE_BAND;
            let colors = &mut *self.colors;
            let mat = self
                .blue
                .mats
                .entry(((dist / BLUE_BAND) as i32, m as u8))
                .or_insert_with(|| colors.add(ColorMaterial { color: color.with_alpha(0.0), alpha_mode: AlphaMode2d::Blend, ..default() }))
                .clone();
            self.commands.spawn((
                Mesh2d(mesh.clone()),
                MeshMaterial2d(mat.clone()),
                Transform::from_xyz(0.0, 0.0, z),
                crate::camera::color_layer(),
                Visibility::Hidden,
                crate::regrowth::Vine { boss: crate::boss::BossKind::Spider, dist, stem: mat.clone(), leaf: mat, shown: 0.0, base: [color; 2], lit: 0.0 },
                ChildOf(e),
            ));
        }
        self.commands.spawn((Mesh2d(mesh), MeshMaterial2d(self.mats[m as usize].clone()), Transform::from_xyz(0.0, 0.0, z), ChildOf(e)));
    }

    /// A smooth bar through `pts`, lit on its left side.
    fn bar(&mut self, e: Entity, pts: &[Vec2], w: f32, m: M, z: f32) {
        self.part(e, stroke(&smooth_open(pts, 6), |_| w, 1.0, 0.55, |_| 1.0), m, z);
    }

    /// A shape lit from above (`low` = the shade at its bottom).
    fn shape(&mut self, e: Entity, outline: &[Vec2], low: f32, m: M, z: f32) {
        let (y0, y1) = outline.iter().fold((f32::MAX, f32::MIN), |(a, b), p| (a.min(p.y), b.max(p.y)));
        let h = (y1 - y0).max(0.01);
        self.part(e, fill(outline, |p| low + (1.0 - low) * ((p.y - y0) / h)), m, z);
    }

    fn ring(&mut self, e: Entity, center: Vec2, r: f32, w: f32, m: M, z: f32, pattern: impl Fn(f32) -> f32) {
        let pts: Vec<Vec2> = (0..=40).map(|i| center + Vec2::from_angle(i as f32 / 40.0 * std::f32::consts::TAU) * r).collect();
        self.part(e, stroke(&pts, |_| w, 0.55, 1.0, pattern), m, z);
    }

    fn rebar(&mut self, e: Entity, base: Vec2, dir: Vec2, z: f32) {
        let a = self.rng.range(7.0, 12.0);
        let b = self.rng.range(14.0, 26.0);
        // Sagging and bent.
        let bend = dir.perp() * self.rng.range(-7.0, 7.0) - Vec2::Y * self.rng.range(0.0, 5.0);
        self.bar(e, &[base - dir * 8.0, base + dir * a, base + dir * b + bend], 1.8, M::Rust, z);
    }

    // --- Concrete ---

    fn rubble(&mut self, at: Vec2) {
        let e = self.root(at - Vec2::Y * S * 2.0, 1.0, 0.0);
        let n = 3 + self.rng.below(3);
        for i in 0..n {
            let r = self.rng.range(3.5, 7.5);
            let c = Vec2::new(self.rng.range(-12.0, 12.0), r * 0.35);
            let pts: Vec<Vec2> = rock_pts(&mut self.rng, r, 7).into_iter().map(|p| p + c).collect();
            self.shape(e, &pts, 0.45, M::Concrete, 0.01 * i as f32);
        }
        if self.rng.chance(0.5) {
            let base = Vec2::new(self.rng.range(-6.0, 6.0), 3.0);
            let dir = Vec2::new(self.rng.range(-0.6, 0.6), 1.0).normalize();
            self.rebar(e, base, dir, -0.01);
        }
    }

    /// A slab of floor, fallen and stuck in the ground at an angle, rebar out of its broken end.
    fn slab(&mut self, at: Vec2) {
        let angle = self.rng.range(0.25, 0.55) * if self.rng.chance(0.5) { 1.0 } else { -1.0 };
        let (w, h) = (self.rng.range(34.0, 46.0), self.rng.range(11.0, 15.0));
        let e = self.root(at - Vec2::Y * S * 4.0, if angle > 0.0 { 1.0 } else { -1.0 }, angle.abs());
        let mut top = Vec::new();
        let mut x = -w / 2.0;
        while x < w / 2.0 {
            top.push(Vec2::new(x, h + self.rng.range(-1.0, 1.0)));
            x += self.rng.range(7.0, 12.0);
        }
        top.push(Vec2::new(w / 2.0 + 2.0, h - 2.0));
        let mut outline = vec![Vec2::new(-w / 2.0 + 3.0, -8.0)];
        outline.extend(top.iter().copied());
        // Broken end: jagged.
        outline.push(Vec2::new(w / 2.0 - 3.0, h * 0.5));
        outline.push(Vec2::new(w / 2.0 + 1.0, h * 0.2));
        outline.push(Vec2::new(w / 2.0 - 2.0, -8.0));
        for _ in 0..2 {
            let base = Vec2::new(w / 2.0 - 2.0, self.rng.range(2.0, h - 2.0));
            let dir = Vec2::new(1.0, self.rng.range(-0.3, 0.5)).normalize();
            self.rebar(e, base, dir, -0.01);
        }
        self.shape(e, &outline, 0.5, M::Concrete, 0.0);
        let rim: Vec<Vec2> = top.iter().map(|p| *p - Vec2::Y).collect();
        self.part(e, stroke(&rim, |_| 2.2, 1.0, 0.6, |_| 1.0), M::Rim, 0.01);
        let x0 = self.rng.range(-w / 3.0, w / 4.0);
        let crack: Vec<Vec2> = (0..4).map(|i| Vec2::new(x0 + self.rng.range(-3.0, 3.0), h - i as f32 * h / 3.0)).collect();
        self.part(e, stroke(&crack, |t| 1.8 * (1.0 - t) + 0.4, 1.0, 1.0, |_| 1.0), M::Soot, 0.02);
    }

    /// A window frame torn out of a facade, leaning in the dirt.
    fn fallen_window(&mut self, at: Vec2) {
        let (w, h) = (self.rng.range(26.0, 34.0), self.rng.range(36.0, 44.0));
        let angle = self.rng.range(-0.18, 0.18);
        let e = self.root(at - Vec2::Y * S * (3.0 + w / 2.0 * angle.abs().sin()), 1.0, angle);
        self.frame(e, Vec2::new(0.0, h / 2.0), Vec2::new(w, h) / 2.0, 0.0);
    }

    /// Window frame with a cross bar, a broken member and shards of glass.
    fn frame(&mut self, e: Entity, c: Vec2, half: Vec2, z: f32) {
        let (l, r, b, t) = (c.x - half.x, c.x + half.x, c.y - half.y, c.y + half.y);
        let broken = self.rng.below(4);
        let sides = [
            [Vec2::new(l, b), Vec2::new(l, t)],
            [Vec2::new(l, t), Vec2::new(r, t)],
            [Vec2::new(r, t), Vec2::new(r, b)],
            [Vec2::new(r, b), Vec2::new(l, b)],
        ];
        for (i, [p, q]) in sides.into_iter().enumerate() {
            let q = if i == broken { p.lerp(q, self.rng.range(0.3, 0.6)) + Vec2::new(self.rng.range(-3.0, 3.0), self.rng.range(-3.0, 3.0)) } else { q };
            self.part(e, stroke(&[p, q], |_| 2.6, 1.0, 0.6, |_| 1.0), M::Iron, z + 0.02);
        }
        let mid = c.y + half.y * 0.2;
        self.part(e, stroke(&[Vec2::new(l, mid), Vec2::new(r, mid)], |_| 1.6, 1.0, 0.6, |_| 1.0), M::Iron, z + 0.02);
        self.part(e, stroke(&[Vec2::new(c.x, b), Vec2::new(c.x, t)], |_| 1.6, 1.0, 0.6, |_| 1.0), M::Iron, z + 0.02);
        // Shards stuck in the corners of the panes.
        let mut tris = Vec::new();
        for _ in 0..3 {
            let corner = Vec2::new(if self.rng.chance(0.5) { l + 1.0 } else { r - 1.0 }, if self.rng.chance(0.5) { b + 1.0 } else { t - 1.0 });
            let inward = (c - corner).signum();
            let a = corner + Vec2::new(inward.x * self.rng.range(4.0, 9.0), 0.0);
            let d = corner + Vec2::new(0.0, inward.y * self.rng.range(4.0, 10.0));
            tris.push([corner, a, d]);
        }
        self.part(e, triangles(&tris), M::Rim, z + 0.01);
    }

    /// A blown-out window in a facade (drawn over the terrain's face).
    fn window_hole(&mut self, e: Entity, half: Vec2) {
        let mut hole = vec![Vec2::new(-half.x, -half.y), Vec2::new(half.x, -half.y)];
        // Ragged top where the lintel broke.
        let n = 4;
        for i in 0..=n {
            let x = half.x - 2.0 * half.x * i as f32 / n as f32;
            hole.push(Vec2::new(x, half.y + self.rng.range(-2.0, 3.0)));
        }
        self.part(e, fill(&hole, |p| 0.6 + 0.4 * (p.y / half.y).abs()), M::Soot, OVER);
        if self.rng.chance(0.7) {
            self.frame(e, Vec2::ZERO, half - Vec2::splat(1.5), OVER);
        } else if self.rng.chance(0.5) {
            // Burnt out: soot licking up the wall above.
            let flame = [Vec2::new(-half.x * 0.8, half.y), Vec2::new(-half.x * 0.3, half.y + 14.0), Vec2::new(half.x * 0.2, half.y + 8.0), Vec2::new(half.x * 0.6, half.y + 16.0), Vec2::new(half.x * 0.9, half.y)];
            self.part(e, fill(&flame, |p| 0.7 + 0.3 * ((p.y - half.y) / 16.0)), M::Soot, OVER - 0.01);
        }
        // Lit sill.
        self.part(e, stroke(&[Vec2::new(-half.x - 3.0, -half.y - 1.5), Vec2::new(half.x + 3.0, -half.y - 1.5)], |_| 3.0, 1.0, 0.5, |_| 1.0), M::Rim, OVER + 0.03);
    }

    /// Rebar bursting out of a broken floor edge.
    fn rebar_burst(&mut self, e: Entity) {
        for _ in 0..3 + self.rng.below(3) {
            let base = Vec2::new(-2.0, -self.rng.range(1.0, 14.0));
            let dir = Vec2::new(1.0, self.rng.range(-0.5, 0.6)).normalize();
            self.rebar(e, base, dir, 0.0);
        }
        if self.rng.chance(0.4) {
            // A chunk of concrete still hanging on the bars.
            let r = self.rng.range(4.0, 6.0);
            let c = Vec2::new(self.rng.range(4.0, 9.0), -self.rng.range(4.0, 10.0));
            let pts: Vec<Vec2> = rock_pts(&mut self.rng, r, 7).into_iter().map(|p| p + c).collect();
            self.shape(e, &pts, 0.4, M::Concrete, 0.03);
        }
    }

    /// Cables hanging from `from` (`dir` 0 = over an edge, −1 = straight down).
    fn cables(&mut self, e: Entity, from: Vec2, n: usize, dir: f32) {
        for _ in 0..n {
            let len = self.rng.range(18.0, 55.0);
            let sway = self.rng.range(-6.0, 6.0);
            let start = from + Vec2::new(self.rng.range(-3.0, 3.0), 0.0);
            let pts = if dir == 0.0 {
                vec![start, start + Vec2::new(5.0, 1.0), start + Vec2::new(9.0, -6.0), start + Vec2::new(10.0 + sway, -len)]
            } else {
                vec![start, start + Vec2::new(sway * 0.3, -len * 0.4), start + Vec2::new(sway, -len)]
            };
            let path = smooth_open(&pts, 6);
            let end = path[path.len() - 1];
            self.part(e, stroke(&path, |_| 1.5, 0.8, 0.5, |_| 1.0), M::Soot, 0.04);
            // Frayed copper.
            for s in [-1.0, 1.0] {
                let tip = [end, end + Vec2::new(s * self.rng.range(1.0, 3.0), -self.rng.range(2.0, 4.0))];
                self.part(e, stroke(&tip, |_| 0.6, 1.0, 1.0, |_| 1.0), M::Steel, 0.05);
            }
        }
    }

    // --- Trash ---

    fn tire(&mut self, e: Entity, c: Vec2, r: f32, z: f32) {
        let treads = self.rng.range(22.0, 30.0);
        self.ring(e, c, r * 0.75, r * 0.5, M::Rubber, z, move |t| if (t * treads) as i32 % 2 == 0 { 1.0 } else { 0.65 });
        if self.rng.chance(0.5) {
            // A rusty wheel rim still inside.
            self.ring(e, c, r * 0.47, 2.0, M::Rust, z + 0.01, |_| 1.0);
        }
    }

    fn tire_upright(&mut self, at: Vec2) {
        let r = self.rng.range(10.0, 13.0);
        let e = self.root(at, 1.0, 0.0);
        self.tire(e, Vec2::new(0.0, r * 0.7), r, 0.0);
    }

    fn tire_stack(&mut self, at: Vec2) {
        let e = self.root(at - Vec2::Y * S * 3.0, 1.0, 0.0);
        let n = 2 + self.rng.below(2);
        let (r, h) = (self.rng.range(12.0, 15.0), 7.5);
        let mut x = 0.0;
        for i in 0..n {
            let y = i as f32 * (h - 0.5);
            x += self.rng.range(-3.0, 3.0);
            let tilt = if i == n - 1 { self.rng.range(-1.5, 1.5) } else { 0.0 };
            let side = smooth_closed(
                &[
                    Vec2::new(x - r, y + 1.0 - tilt),
                    Vec2::new(x - r + 1.0, y + h - 1.0 - tilt),
                    Vec2::new(x + r - 1.0, y + h - 1.0 + tilt),
                    Vec2::new(x + r, y + 1.0 + tilt),
                ],
                5,
            );
            self.shape(e, &side, 0.5, M::Rubber, i as f32 * 0.01);
            // Tread grooves.
            let groove = [Vec2::new(x - r + 2.0, y + h / 2.0 - tilt), Vec2::new(x + r - 2.0, y + h / 2.0 + tilt)];
            self.part(e, stroke(&groove, |_| 0.8, 0.4, 0.4, |_| 1.0), M::Soot, i as f32 * 0.01 + 0.005);
            if i == n - 1 {
                // Looking into the top one.
                let hole: Vec<Vec2> = ellipse_pts(r * 0.55, 1.8, 14).into_iter().map(|p| p + Vec2::new(x, y + h - 0.5)).collect();
                self.part(e, fill(&hole, |_| 1.0), M::Soot, 0.1);
            }
        }
    }

    fn fridge(&mut self, at: Vec2) {
        let (w, h) = (self.rng.range(22.0, 26.0), self.rng.range(40.0, 46.0));
        let angle = self.rng.range(-0.22, 0.22);
        let e = self.root(at - Vec2::Y * S * (4.0 + w / 2.0 * angle.abs().sin()), 1.0, angle);
        let body = smooth_closed(&[Vec2::new(-w / 2.0, 0.0), Vec2::new(-w / 2.0, h), Vec2::new(w / 2.0, h), Vec2::new(w / 2.0, 0.0)], 1);
        self.shape(e, &body, 0.5, M::Enamel, 0.0);
        // Rounded top edge catching the light.
        self.part(e, stroke(&[Vec2::new(-w / 2.0 + 1.0, h - 1.0), Vec2::new(w / 2.0 - 1.0, h - 1.0)], |_| 2.0, 1.0, 1.0, |_| 1.0), M::Rim, 0.01);
        let split = h * 0.68;
        self.part(e, stroke(&[Vec2::new(-w / 2.0, split), Vec2::new(w / 2.0, split)], |_| 1.0, 1.0, 1.0, |_| 1.0), M::Soot, 0.02);
        for (y0, y1) in [(split + 3.0, split + 9.0), (split - 10.0, split - 3.0)] {
            self.part(e, stroke(&[Vec2::new(w / 2.0 - 3.5, y0), Vec2::new(w / 2.0 - 3.5, y1)], |_| 1.6, 1.0, 0.5, |_| 1.0), M::Iron, 0.02);
        }
        for _ in 0..3 {
            let c = Vec2::new(self.rng.range(-w / 2.0 + 4.0, w / 2.0 - 4.0), self.rng.range(3.0, h - 6.0));
            let r = self.rng.range(2.0, 4.5);
            let pts: Vec<Vec2> = rock_pts(&mut self.rng, r, 8).into_iter().map(|p| p + c).collect();
            self.part(e, fill(&pts, |_| 0.8), M::Rust, 0.015);
        }
        // Rust streaks down from the seams.
        let x = self.rng.range(-w / 3.0, w / 3.0);
        self.part(e, stroke(&[Vec2::new(x, split), Vec2::new(x + 0.5, split - 12.0)], |t| 2.0 * (1.0 - t) + 0.3, 1.0, 1.0, |_| 1.0), M::Rust, 0.016);
    }

    fn bag(&mut self, e: Entity, c: Vec2, r: f32, z: f32) {
        let mut pts = Vec::new();
        for i in 0..9 {
            let a = i as f32 / 9.0 * std::f32::consts::TAU - std::f32::consts::FRAC_PI_2;
            let squash = if a.sin() < 0.0 { 0.8 } else { 1.0 };
            pts.push(c + Vec2::new(a.cos() * r * self.rng.range(0.9, 1.15), a.sin() * r * 0.85 * squash));
        }
        // Knotted top.
        let top = c + Vec2::Y * r * 0.85;
        self.part(e, fill(&smooth_closed(&pts, 4), |p| 0.55 + 0.45 * ((p.y - c.y) / r + 1.0) / 2.0), M::Plastic, z);
        let ears = [top + Vec2::new(-3.0, 4.0), top, top + Vec2::new(3.0, 4.5)];
        self.part(e, stroke(&smooth_open(&ears, 4), |_| 1.8, 1.0, 0.7, |_| 1.0), M::Plastic, z + 0.01);
        // Glossy highlight.
        let hl = [c + Vec2::new(-r * 0.5, r * 0.1), c + Vec2::new(-r * 0.2, r * 0.5)];
        self.part(e, stroke(&hl, |t| 1.4 * (1.0 - (t - 0.5).abs()), 1.0, 1.0, |_| 1.0), M::Rim, z + 0.02);
    }

    fn bags(&mut self, at: Vec2) {
        let e = self.root(at - Vec2::Y * S * 2.0, 1.0, 0.0);
        let n = 2 + self.rng.below(2);
        for i in 0..n {
            let r = self.rng.range(6.0, 9.0);
            let c = Vec2::new(self.rng.range(-10.0, 10.0), r * 0.7 + if i == n - 1 && n > 2 { 6.0 } else { 0.0 });
            self.bag(e, c, r, i as f32 * 0.03);
        }
    }

    /// Bent rusty bars sticking up out of the ground.
    fn bars(&mut self, at: Vec2) {
        let e = self.root(at, 1.0, 0.0);
        for _ in 0..2 + self.rng.below(3) {
            let x = self.rng.range(-10.0, 10.0);
            let h = self.rng.range(18.0, 36.0);
            let lean = self.rng.range(-8.0, 8.0);
            let hook = self.rng.range(-10.0, 10.0);
            let pts = [Vec2::new(x, -6.0), Vec2::new(x + lean * 0.3, h * 0.5), Vec2::new(x + lean, h), Vec2::new(x + lean + hook, h - self.rng.range(0.0, 8.0))];
            self.bar(e, &pts, 2.2, M::Rust, 0.0);
        }
    }

    /// Bars poking out of a cliff of junk.
    fn bent_bars_out(&mut self, e: Entity) {
        for _ in 0..2 + self.rng.below(2) {
            let base = Vec2::new(-4.0, self.rng.range(-16.0, -2.0));
            let len = self.rng.range(14.0, 28.0);
            let droop = self.rng.range(-10.0, 4.0);
            let pts = [base, base + Vec2::new(len * 0.5, droop * 0.2 + 2.0), base + Vec2::new(len, droop)];
            self.bar(e, &pts, 2.0, M::Rust, 0.0);
        }
    }

    // --- Sewer ---

    /// A pipe out of a wall (local +X), dripping slime down to the ground below.
    fn pipe(&mut self, e: Entity, root: Vec2, flip: f32, at: Vec2, r: f32) {
        let len = self.rng.range(10.0, 24.0);
        let droop = self.rng.range(-4.0, 1.0);
        let mouth = at + Vec2::new(len, droop);
        let m = if self.rng.chance(0.5) { M::Steel } else { M::Rust };
        self.part(e, stroke(&[at - Vec2::X * 10.0, mouth], |_| 2.0 * r, 1.0, 0.4, |_| 1.0), m, 0.0);
        // Flange.
        let f = at + Vec2::new(len * 0.45, droop * 0.45);
        self.part(e, stroke(&[f - Vec2::Y * (r + 1.5), f + Vec2::Y * (r + 1.5)], |_| 3.0, 0.9, 0.6, |_| 1.0), M::Iron, 0.01);
        let lip: Vec<Vec2> = ellipse_pts(r * 0.4, r, 14).into_iter().map(|p| p + mouth).collect();
        self.part(e, fill(&lip, |_| 1.0), M::Soot, 0.02);
        self.ring_ellipse(e, mouth, r * 0.4, r, m);

        // Slime running out of the mouth.
        let lip_low = mouth + Vec2::new(r * 0.1, -r * 0.8);
        let world = root + lip_low * Vec2::new(flip, 1.0) * S;
        match self.t.surface(world - Vec2::Y * 2.0, 500.0) {
            Some(gy) if gy < world.y - 4.0 => {
                let bottom = (gy - root.y) / S;
                let wob = self.rng.range(-1.5, 1.5);
                let pts = [lip_low, lip_low + Vec2::new(2.5, -3.0), Vec2::new(lip_low.x + 3.0 + wob, (lip_low.y + bottom) / 2.0), Vec2::new(lip_low.x + 3.0, bottom)];
                self.part(e, stroke(&smooth_open(&pts, 6), |t| 2.8 - 1.2 * t, 1.0, 0.6, |_| 1.0), M::Slime, OVER);
                let puddle: Vec<Vec2> = ellipse_pts(self.rng.range(8.0, 13.0), 2.2, 16).into_iter().map(|p| p + Vec2::new(lip_low.x + 3.0, bottom + 0.3)).collect();
                self.part(e, fill(&puddle, |p| 0.7 + 0.3 * (p.y - bottom).max(0.0) / 2.2), M::Slime, OVER + 0.01);
            }
            _ => {
                let len = self.rng.range(5.0, 9.0);
                self.drip(e, lip_low + Vec2::X * 1.0, 2.2, len, OVER);
            }
        }
        // Grime on the wall below it.
        let streak = [at + Vec2::new(-3.0, -r), at + Vec2::new(-3.5, -r - self.rng.range(12.0, 30.0))];
        self.part(e, stroke(&streak, |t| 4.0 * (1.0 - t) + 0.5, 1.0, 1.0, |_| 1.0), M::Slime, OVER - 0.01);
    }

    fn ring_ellipse(&mut self, e: Entity, c: Vec2, rx: f32, ry: f32, m: M) {
        let pts: Vec<Vec2> = (0..=24).map(|i| {
            let a = i as f32 / 24.0 * std::f32::consts::TAU;
            c + Vec2::new(a.cos() * rx, a.sin() * ry)
        }).collect();
        self.part(e, stroke(&pts, |_| 1.4, 1.0, 0.6, |_| 1.0), m, 0.03);
    }

    /// A slime drop hanging from `at`.
    fn drip(&mut self, e: Entity, at: Vec2, w: f32, len: f32, z: f32) {
        let pts = [
            at + Vec2::new(-w, 1.0),
            at + Vec2::new(-w * 0.45, -len * 0.55),
            at + Vec2::new(-w * 0.9, -len * 0.85),
            at + Vec2::new(0.0, -len - w * 0.4),
            at + Vec2::new(w * 0.9, -len * 0.85),
            at + Vec2::new(w * 0.45, -len * 0.55),
            at + Vec2::new(w, 1.0),
        ];
        self.part(e, fill(&smooth_closed(&pts, 4), |p| 0.7 + 0.3 * ((p.y - at.y) / len + 1.0).clamp(0.0, 1.0)), M::Slime, z);
        if self.rng.chance(0.5) {
            let d: Vec<Vec2> = ellipse_pts(w * 0.45, w * 0.6, 10).into_iter().map(|p| p + at - Vec2::Y * (len + self.rng.range(6.0, 14.0))).collect();
            self.part(e, fill(&d, |_| 0.9), M::Slime, z);
        }
    }

    fn drips(&mut self, e: Entity) {
        for _ in 0..2 + self.rng.below(3) {
            let at = Vec2::new(self.rng.range(-12.0, 12.0), 1.0);
            let (w, len) = (self.rng.range(1.5, 2.8), self.rng.range(5.0, 16.0));
            self.drip(e, at, w, len, OVER);
        }
    }

    /// A barred culvert in a sewer wall, slime trickling out (over the terrain's face).
    fn grate(&mut self, e: Entity, half: Vec2) {
        let arch: Vec<Vec2> = {
            let mut p = vec![Vec2::new(half.x, -half.y), Vec2::new(half.x, half.y - half.x)];
            for i in 1..8 {
                let a = i as f32 / 8.0 * std::f32::consts::PI;
                p.push(Vec2::new(a.cos() * half.x, half.y - half.x + a.sin() * half.x));
            }
            p.push(Vec2::new(-half.x, half.y - half.x));
            p.push(Vec2::new(-half.x, -half.y));
            p
        };
        self.part(e, fill(&arch, |p| 0.5 + 0.5 * (1.0 - (p.y + half.y) / (2.0 * half.y))), M::Soot, OVER);
        let mut rim = arch.clone();
        rim.push(arch[0]);
        self.part(e, stroke(&rim, |_| 2.8, 1.0, 0.5, |_| 1.0), M::Concrete, OVER + 0.02);
        let n = 4;
        for i in 0..n {
            let x = -half.x + 2.0 * half.x * (i as f32 + 0.5) / n as f32;
            let top = (half.x * half.x - x * x).max(0.0).sqrt() + half.y - half.x;
            let bent = i == 1 && self.rng.chance(0.5);
            let pts = if bent {
                vec![Vec2::new(x, -half.y), Vec2::new(x + 3.0, -half.y * 0.2), Vec2::new(x + 6.0, -half.y * 0.1)]
            } else {
                vec![Vec2::new(x, -half.y), Vec2::new(x, top)]
            };
            self.bar(e, &pts, 1.8, M::Iron, OVER + 0.01);
        }
        self.part(e, stroke(&[Vec2::new(-half.x, -half.y * 0.1), Vec2::new(half.x, -half.y * 0.1)], |_| 1.6, 1.0, 0.6, |_| 1.0), M::Iron, OVER + 0.01);
        // Slime spilling over the sill and down the wall.
        let x = self.rng.range(-half.x * 0.5, half.x * 0.5);
        let len = self.rng.range(14.0, 30.0);
        let pts = [Vec2::new(x, -half.y + 1.0), Vec2::new(x + 0.5, -half.y - len * 0.5), Vec2::new(x - 0.5, -half.y - len)];
        self.part(e, stroke(&smooth_open(&pts, 5), |t| 5.0 * (1.0 - t) + 1.0, 1.0, 0.7, |_| 1.0), M::Slime, OVER + 0.03);
        let sill = [Vec2::new(-half.x + 1.0, -half.y + 1.0), Vec2::new(half.x - 1.0, -half.y + 1.0)];
        self.part(e, stroke(&sill, |_| 2.0, 1.0, 0.6, |_| 1.0), M::Slime, OVER + 0.03);
    }

    fn puddle(&mut self, at: Vec2) {
        let e = self.root(at, 1.0, 0.0);
        let rx = self.rng.range(10.0, 15.0);
        let pts: Vec<Vec2> = ellipse_pts(rx, 2.4, 18).into_iter().map(|p| p + Vec2::Y * 0.3).collect();
        self.part(e, fill(&pts, |p| 0.65 + 0.35 * (p.y.max(0.0) / 2.4)), M::Slime, OVER);
        let shine = [Vec2::new(-rx * 0.4, 1.2), Vec2::new(rx * 0.1, 1.4)];
        self.part(e, stroke(&shine, |_| 0.8, 1.0, 1.0, |_| 1.0), M::Rim, OVER + 0.01);
        if self.rng.chance(0.6) {
            // A pipe stub ending in it.
            let x = self.rng.range(-rx * 0.6, rx * 0.6);
            let top = self.rng.range(8.0, 14.0);
            self.part(e, stroke(&[Vec2::new(x, -6.0), Vec2::new(x, top)], |_| 7.0, 1.0, 0.45, |_| 1.0), M::Rust, 0.0);
        }
    }

    fn pipe_segment(&mut self, at: Vec2) {
        let (flip, angle) = (if self.rng.chance(0.5) { 1.0 } else { -1.0 }, self.rng.range(-0.06, 0.06));
        let e = self.root(at - Vec2::Y * S * 4.0, flip, angle);
        let (len, r) = (self.rng.range(40.0, 54.0), self.rng.range(5.5, 7.0));
        let (a, b) = (Vec2::new(-len / 2.0, r), Vec2::new(len / 2.0, r));
        self.part(e, stroke(&[a, b], |_| 2.0 * r, 1.0, 0.45, |_| 1.0), M::Rust, 0.0);
        for x in [-len / 2.0 + 3.0, len / 6.0] {
            self.part(e, stroke(&[Vec2::new(x, -1.0), Vec2::new(x, 2.0 * r + 1.5)], |_| 3.0, 0.9, 0.6, |_| 1.0), M::Iron, 0.01);
        }
        let lip: Vec<Vec2> = ellipse_pts(r * 0.4, r, 14).into_iter().map(|p| p + b).collect();
        self.part(e, fill(&lip, |_| 1.0), M::Soot, 0.02);
        self.ring_ellipse(e, b, r * 0.4, r, M::Rust);
        self.drip(e, b + Vec2::new(0.0, -r * 0.8), 1.8, 3.0, OVER);
    }

    fn valve(&mut self, at: Vec2) {
        let e = self.root(at, 1.0, 0.0);
        let h = self.rng.range(14.0, 20.0);
        self.part(e, stroke(&[Vec2::new(0.0, -6.0), Vec2::new(0.0, h)], |_| 7.0, 1.0, 0.45, |_| 1.0), M::Steel, 0.0);
        self.part(e, stroke(&[Vec2::new(-5.5, h - 2.0), Vec2::new(5.5, h - 2.0)], |_| 3.0, 1.0, 0.6, |_| 1.0), M::Iron, 0.01);
        let wheel = Vec2::new(0.0, h + 6.0);
        self.part(e, stroke(&[Vec2::new(0.0, h), wheel], |_| 2.0, 1.0, 0.6, |_| 1.0), M::Iron, 0.01);
        self.ring(e, wheel, 6.5, 1.8, M::Rust, 0.02, |_| 1.0);
        for i in 0..3 {
            let d = Vec2::from_angle(i as f32 * 1.047 + 0.3) * 6.5;
            self.part(e, stroke(&[wheel - d, wheel + d], |_| 1.0, 1.0, 1.0, |_| 1.0), M::Rust, 0.02);
        }
        if self.rng.chance(0.6) {
            self.drip(e, Vec2::new(3.5, h - 3.0), 1.6, 5.0, OVER);
        }
    }

    // --- Park ---

    fn bench(&mut self, at: Vec2) {
        let flip = if self.rng.chance(0.5) { 1.0 } else { -1.0 };
        let e = self.root(at - Vec2::Y * S * 2.0, flip, 0.0);
        let w = 26.0;
        let seat = 13.0;
        // The right end gave way and lies in the dirt.
        let drop = if self.rng.chance(0.7) { self.rng.range(8.0, 12.0) } else { 0.0 };
        let y_at = |x: f32| seat - drop * ((x + w) / (2.0 * w)).clamp(0.0, 1.0).powf(1.5);
        // Cast-iron legs: a front foot curl and a back leg up to the backrest.
        for x in [-w + 5.0, w - 5.0] {
            let broken = x > 0.0 && drop > 0.0;
            let top = y_at(x);
            if broken {
                self.bar(e, &[Vec2::new(x - 3.0, 0.0), Vec2::new(x, 4.0), Vec2::new(x + 2.0, 6.0)], 2.4, M::Iron, 0.0);
                continue;
            }
            self.bar(e, &[Vec2::new(x - 4.0, 0.0), Vec2::new(x - 1.0, 2.0), Vec2::new(x, top)], 2.6, M::Iron, 0.0);
            self.bar(e, &[Vec2::new(x + 4.0, 0.0), Vec2::new(x + 2.0, top * 0.6), Vec2::new(x + 3.5, top + 13.0)], 2.4, M::Iron, 0.0);
        }
        let slats = |dy: f32, from: f32, to: f32| -> Vec<Vec2> { (0..=6).map(|i| { let x = from + (to - from) * i as f32 / 6.0; Vec2::new(x, y_at(x) + dy) }).collect() };
        for (k, dy) in [0.0, 2.6].into_iter().enumerate() {
            let pts = slats(dy, -w, w);
            self.part(e, stroke(&pts, |_| 2.1, 1.0, 0.5, |_| 1.0), M::Wood, 0.01 + k as f32 * 0.01);
        }
        // Backrest: one slat snapped off halfway.
        let snap = self.rng.range(-4.0, 14.0);
        for (k, dy) in [9.0, 13.0].into_iter().enumerate() {
            let to = if k == 1 { snap } else { w };
            let pts = slats(dy, -w + 1.0, to);
            self.part(e, stroke(&pts, |_| 2.1, 1.0, 0.5, |_| 1.0), M::Wood, 0.02 + k as f32 * 0.01);
        }
    }

    fn lamp(&mut self, at: Vec2) {
        let flip = if self.rng.chance(0.5) { 1.0 } else { -1.0 };
        let e = self.root(at - Vec2::Y * S * 3.0, flip, 0.0);
        let h = self.rng.range(80.0, 110.0);
        let bend = self.rng.range(0.0, 1.0);
        let base = [Vec2::new(-6.0, -2.0), Vec2::new(-4.5, 9.0), Vec2::new(4.5, 9.0), Vec2::new(6.0, -2.0)];
        self.shape(e, &base, 0.4, M::Iron, 0.01);
        // Bent over by a blast: straight, then kinked toward +X.
        let kink = h * self.rng.range(0.55, 0.75);
        let top = Vec2::new(h * 0.35 * bend + 4.0, kink + (h - kink) * (1.0 - bend * 0.6));
        let pole = [Vec2::new(0.0, 6.0), Vec2::new(0.0, kink * 0.6), Vec2::new(bend * 2.0, kink), top];
        self.part(e, stroke(&smooth_open(&pole, 8), |t| 3.6 - 1.2 * t, 1.0, 0.5, |_| 1.0), M::Iron, 0.0);
        // The arm and the broken lantern hanging from it.
        let arm_end = top + Vec2::new(10.0, -2.0 - bend * 4.0);
        self.bar(e, &[top, top + Vec2::new(5.0, 2.0), arm_end], 1.8, M::Iron, 0.0);
        let lantern = [arm_end + Vec2::new(-4.0, -2.0), arm_end + Vec2::new(-2.5, -10.0), arm_end + Vec2::new(2.5, -10.0), arm_end + Vec2::new(4.0, -2.0)];
        self.shape(e, &lantern, 0.5, M::Soot, 0.01);
        self.part(e, triangles(&[[arm_end + Vec2::new(-2.0, -4.0), arm_end + Vec2::new(1.5, -4.5), arm_end + Vec2::new(-1.0, -8.5)]]), M::Rim, 0.02);
        let hood = [arm_end + Vec2::new(-5.5, -2.0), arm_end + Vec2::new(0.0, 2.5), arm_end + Vec2::new(5.5, -2.0)];
        self.shape(e, &hood, 0.6, M::Iron, 0.02);
    }

    /// A statue pulled off its plinth: the plinth with the feet still on it,
    /// the figure face-down next to it.
    fn statue(&mut self, at: Vec2) {
        let flip = if self.rng.chance(0.5) { 1.0 } else { -1.0 };
        let e = self.root(at - Vec2::Y * S * 3.0, flip, 0.0);
        let (px, pw, ph) = (-40.0, 34.0, 30.0);
        let plinth = [Vec2::new(px - pw / 2.0, -3.0), Vec2::new(px - pw / 2.0 + 1.0, ph), Vec2::new(px + pw / 2.0 - 1.0, ph), Vec2::new(px + pw / 2.0, -3.0)];
        self.shape(e, &plinth, 0.45, M::Concrete, 0.0);
        self.part(e, stroke(&[Vec2::new(px - pw / 2.0 - 2.0, ph), Vec2::new(px + pw / 2.0 + 2.0, ph)], |_| 3.2, 1.0, 0.6, |_| 1.0), M::Rim, 0.01);
        let plate = [Vec2::new(px - 8.0, 10.0), Vec2::new(px - 8.0, 18.0), Vec2::new(px + 8.0, 18.0), Vec2::new(px + 8.0, 10.0)];
        self.shape(e, &plate, 0.7, M::Bronze, 0.01);
        let crack: Vec<Vec2> = (0..4).map(|i| Vec2::new(px + 9.0 + self.rng.range(-3.0, 3.0), ph - i as f32 * 9.0)).collect();
        self.part(e, stroke(&crack, |t| 1.8 * (1.0 - t) + 0.4, 1.0, 1.0, |_| 1.0), M::Soot, 0.02);
        // Ankles snapped off on top.
        for dx in [-5.0, 4.0] {
            let foot = [Vec2::new(px + dx - 3.0, ph + 1.0), Vec2::new(px + dx - 2.0, ph + 9.0), Vec2::new(px + dx + 2.0, ph + 8.0), Vec2::new(px + dx + 4.0, ph + 1.0)];
            self.shape(e, &foot, 0.5, M::Bronze, 0.015);
        }
        // The figure, lying on its back, an arm still raised.
        let body = smooth_closed(
            &[Vec2::new(-14.0, 1.0), Vec2::new(-12.0, 9.0), Vec2::new(10.0, 13.0), Vec2::new(28.0, 12.0), Vec2::new(34.0, 7.0), Vec2::new(30.0, 0.0), Vec2::new(8.0, -1.0)],
            5,
        );
        self.shape(e, &body, 0.45, M::Bronze, 0.02);
        let head: Vec<Vec2> = ellipse_pts(6.5, 6.0, 16).into_iter().map(|p| p + Vec2::new(42.0, 7.0)).collect();
        self.shape(e, &head, 0.45, M::Bronze, 0.025);
        for dy in [3.0, 8.0] {
            self.bar(e, &[Vec2::new(-12.0, dy), Vec2::new(-26.0, dy - 1.0), Vec2::new(-34.0, dy + 1.0)], 6.0, M::Bronze, 0.018);
        }
        let reach = self.rng.range(-6.0, 6.0);
        self.bar(e, &[Vec2::new(26.0, 11.0), Vec2::new(24.0 + reach * 0.3, 24.0), Vec2::new(22.0 + reach, 36.0)], 4.5, M::Bronze, 0.03);
        let hand: Vec<Vec2> = ellipse_pts(2.8, 3.4, 10).into_iter().map(|p| p + Vec2::new(22.0 + reach, 38.0)).collect();
        self.shape(e, &hand, 0.6, M::Bronze, 0.03);
        // Verdigris streaks (gray in a gray world, for now).
        self.part(e, stroke(&[Vec2::new(14.0, 11.0), Vec2::new(16.0, 3.0)], |t| 2.0 * (1.0 - t) + 0.4, 1.0, 1.0, |_| 1.0), M::Rim, 0.035);
    }

    fn barbed_wire(&mut self, at: Vec2) {
        let e = self.root(at - Vec2::Y * S * 2.0, 1.0, 0.0);
        let n = 2 + self.rng.below(2);
        let span = 70.0 / (n - 1) as f32;
        let posts: Vec<Vec2> = (0..n)
            .map(|i| {
                let x = -35.0 + span * i as f32 + self.rng.range(-4.0, 4.0);
                let h = self.rng.range(17.0, 24.0);
                let lean = self.rng.range(-4.0, 4.0);
                self.bar(e, &[Vec2::new(x, -4.0), Vec2::new(x + lean, h)], 2.4, M::Wood, 0.0);
                Vec2::new(x + lean, h)
            })
            .collect();
        let mut barbs = Vec::new();
        for strand in 0..2 {
            for w in posts.windows(2) {
                let drop = Vec2::Y * (2.0 + strand as f32 * 8.0);
                let (a, b) = (w[0] - drop, w[1] - drop);
                let snapped = strand == 1 && self.rng.chance(0.3);
                let mid = (a + b) / 2.0 - Vec2::Y * self.rng.range(2.0, 5.0);
                let path = if snapped {
                    smooth_open(&[a, a.lerp(mid, 0.6), a.lerp(mid, 0.8) + Vec2::new(0.0, -8.0)], 6)
                } else {
                    smooth_open(&[a, mid, b], 8)
                };
                for p in path.iter().step_by(3) {
                    barbs.push([*p + Vec2::new(-1.5, -1.5), *p + Vec2::new(1.5, 1.5), *p + Vec2::new(0.4, -0.4)]);
                    barbs.push([*p + Vec2::new(-1.5, 1.5), *p + Vec2::new(1.5, -1.5), *p + Vec2::new(-0.4, -0.4)]);
                }
                self.part(e, stroke(&path, |_| 0.9, 1.0, 1.0, |_| 1.0), M::Steel, 0.01);
            }
        }
        self.part(e, triangles(&barbs), M::Steel, 0.01);
    }

    /// Concertina wire coiled on the ground.
    fn coil(&mut self, at: Vec2) {
        let e = self.root(at - Vec2::Y * S * 3.0, 1.0, 0.0);
        let (len, r) = (self.rng.range(50.0, 66.0), self.rng.range(7.0, 9.0));
        let loops = (len / 7.0) as usize;
        let pts: Vec<Vec2> = (0..=loops * 12)
            .map(|i| {
                let t = i as f32 / 12.0;
                let a = t * std::f32::consts::TAU;
                Vec2::new(-len / 2.0 + t * 7.0 + a.cos() * 3.0, r + a.sin() * r)
            })
            .collect();
        self.part(e, stroke(&pts, |_| 0.9, 1.0, 0.6, |_| 1.0), M::Steel, 0.0);
        let barbs: Vec<[Vec2; 3]> = pts.iter().step_by(4).map(|p| [*p + Vec2::new(-1.3, -1.3), *p + Vec2::new(1.3, 1.3), *p + Vec2::new(0.4, -0.4)]).collect();
        self.part(e, triangles(&barbs), M::Steel, 0.01);
    }

    /// A dead tree, burnt down to a few branches.
    fn shrub(&mut self, at: Vec2) {
        let e = self.root(at - Vec2::Y * S * 3.0, 1.0, 0.0);
        let h = self.rng.range(30.0, 52.0);
        let lean = self.rng.range(-8.0, 8.0);
        let trunk = [Vec2::new(0.0, -2.0), Vec2::new(lean * 0.3, h * 0.5), Vec2::new(lean, h)];
        self.part(e, stroke(&smooth_open(&trunk, 6), |t| 6.0 * (1.0 - t) + 1.2, 1.0, 0.45, |_| 1.0), M::Wood, 0.0);
        let path = smooth_open(&trunk, 6);
        for i in 0..3 + self.rng.below(3) {
            let from = path[(path.len() / 3 + i * 2).min(path.len() - 1)];
            let side = if i % 2 == 0 { 1.0 } else { -1.0 };
            let len = self.rng.range(8.0, 18.0);
            let mid = from + Vec2::new(side * len * 0.5, len * 0.4);
            let tip = from + Vec2::new(side * len, len * self.rng.range(0.3, 1.0));
            self.part(e, stroke(&smooth_open(&[from, mid, tip], 4), |t| 2.4 * (1.0 - t) + 0.5, 1.0, 0.5, |_| 1.0), M::Wood, 0.01);
            if self.rng.chance(0.6) {
                let twig = [mid, mid + Vec2::new(side * 3.0, self.rng.range(4.0, 8.0))];
                self.part(e, stroke(&twig, |t| 1.2 * (1.0 - t) + 0.3, 1.0, 0.5, |_| 1.0), M::Wood, 0.01);
            }
        }
        // Roots gripping the soil.
        for s in [-1.0, 1.0] {
            self.bar(e, &[Vec2::new(0.0, 1.0), Vec2::new(s * 5.0, 0.5), Vec2::new(s * 9.0, -2.0)], 2.0, M::Wood, 0.0);
        }
    }

    /// Roots out of an eroded edge or wall (`dir` 0 = sideways, −1 = down from a ceiling).
    fn roots(&mut self, e: Entity, from: Vec2, dir: f32) {
        for _ in 0..2 + self.rng.below(3) {
            let start = from + Vec2::new(0.0, self.rng.range(-4.0, 2.0));
            let len = self.rng.range(12.0, 30.0);
            let sway = self.rng.range(-5.0, 5.0);
            let pts = if dir == 0.0 {
                [start - Vec2::X * 4.0, start + Vec2::new(4.0, -len * 0.2), start + Vec2::new(6.0 + sway, -len)]
            } else {
                [start, start + Vec2::new(sway * 0.4, -len * 0.5), start + Vec2::new(sway, -len)]
            };
            self.part(e, stroke(&smooth_open(&pts, 6), |t| 2.4 * (1.0 - t) + 0.4, 1.0, 0.5, |_| 1.0), M::Wood, 0.0);
        }
    }

    /// A strand of barbed wire hanging off a ledge.
    fn wire_strand(&mut self, e: Entity) {
        let len = self.rng.range(20.0, 36.0);
        let pts = smooth_open(&[Vec2::new(-6.0, 1.0), Vec2::new(3.0, 2.0), Vec2::new(7.0, -len * 0.5), Vec2::new(5.0 + self.rng.range(-4.0, 4.0), -len)], 6);
        let barbs: Vec<[Vec2; 3]> = pts.iter().step_by(3).map(|p| [*p + Vec2::new(-1.5, -1.0), *p + Vec2::new(1.5, 1.0), *p + Vec2::new(0.4, -0.4)]).collect();
        self.part(e, stroke(&pts, |_| 0.9, 1.0, 1.0, |_| 1.0), M::Steel, 0.0);
        self.part(e, triangles(&barbs), M::Steel, 0.0);
    }
}
