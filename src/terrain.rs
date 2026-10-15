//! The static world, built from the sketch: a grid of cells (one per sketch pixel)
//! used for collision, ground queries and navigation.
//!
//! World (0, 0) is the origin marker (a magenta cross), so the canvas can grow or
//! shrink in any direction without moving anything. One sketch pixel = `PX` world units.

use bevy::prelude::*;

use crate::sketch::{Cell, MarkerKind, Sketch};

/// World units per sketch pixel.
pub const PX: f32 = 8.0;
/// Ledges up to 2 pixels are climbed by just walking into them (1 px reads as a ramp).
pub const STEP_UP: f32 = 2.0 * PX + 0.5;
/// Empty sketch pixels kept around the drawing (things can fly and fall there).
const MARGIN: i32 = 80;

/// How an area looks; picked from the slice names in the sketch.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Style {
    Concrete,
    Trash,
    Sewer,
    Park,
    Subway,
    Treatment,
    Sinkhole,
    Catacombs,
    Mall,
    Dam,
    Bunker,
    Cave,
}

/// One storey of the buildings (the window grid in `terrain_props`), in world units.
pub const FLOOR_HEIGHT: f32 = 12.0 * PX;

impl Style {
    /// How much of a fall this ground absorbs (0 = hard, 1 = lands like a feather).
    /// Exhaustive on purpose: a new style has to say how it lands.
    pub fn softness(self) -> f32 {
        match self {
            Style::Concrete => 0.0,
            Style::Trash => 0.6,
            Style::Sewer => 0.5,
            Style::Park => 0.35,
            Style::Subway => 0.0,
            Style::Treatment => 0.1,
            Style::Sinkhole => 0.25,
            Style::Catacombs => 0.15,
            Style::Mall => 0.0,
            Style::Dam => 0.0,
            Style::Bunker => 0.0,
            Style::Cave => 0.2,
        }
    }

    fn from_name(name: &str) -> Style {
        let n = name.to_lowercase();
        let has = |words: &[&str]| words.iter().any(|w| n.contains(w));
        // Whole words only ("dam" is in "damaged").
        let word = |words: &[&str]| n.split(|c: char| !c.is_alphanumeric()).any(|x| words.contains(&x));
        if has(&["subway", "station", "metro", "train"]) {
            Style::Subway
        } else if has(&["treatment", "filtration", "pumping"]) {
            Style::Treatment
        } else if has(&["sinkhole", "sunken"]) {
            Style::Sinkhole
        } else if has(&["catacomb", "crypt", "ossuary", "tomb"]) {
            Style::Catacombs
        } else if has(&["shopping", "mall", "store"]) {
            Style::Mall
        } else if word(&["dam", "spillway", "reservoir"]) {
            Style::Dam
        } else if has(&["bunker", "military", "shelter"]) {
            Style::Bunker
        } else if has(&["cave", "cavern", "grotto"]) {
            Style::Cave
        } else if has(&["trash", "dump", "junk", "garbage", "scrap"]) {
            Style::Trash
        } else if has(&["sewer", "pipe", "drain", "canal"]) {
            Style::Sewer
        } else if has(&["park", "square", "plaza", "garden"]) {
            Style::Park
        } else {
            Style::Concrete
        }
    }
}

/// A "region: …" slice (or an unprefixed one): an area with its own look and boss.
#[derive(Clone)]
pub struct Area {
    pub name: String,
    pub rect: Rect,
    pub style: Style,
    /// "sliding" / "one way down" in the name: slopes here can only be slid down.
    pub slide: bool,
}

/// A "feature: …" slice: a special spot described in words (a gap to dash
/// across, a slope to slide down...). The game reacts to keywords in the name.
#[derive(Clone)]
pub struct Feature {
    pub name: String,
    pub rect: Rect,
    /// "slippery" / "sliding" / "one way down": slopes here can only be slid down.
    pub slide: bool,
    /// "fall apart" / "falls apart": junk breaks off as you slide down it.
    pub crumbles: bool,
    /// "track" / "rail" / "electrif": a live third rail runs along the floor (`electric.rs`).
    pub electrified: bool,
}

/// Cloned to paint the art and place props on other threads.
#[derive(Resource, Clone)]
pub struct Terrain {
    size: IVec2,
    /// Grid cell at world (0, 0).
    origin: IVec2,
    cells: Vec<Cell>,
    /// Distance (in cells, capped) from each cell to the nearest solid one.
    clearance: Vec<u8>,
    /// Where the player stands at the start (feet position), below the red marker.
    pub spawn: Vec2,
    /// The origin marker in canvas pixels of the sketch (for debug readouts).
    pub canvas_origin: IVec2,
    /// Boss markers resting on the ground below them (feet position).
    pub roach: Option<Vec2>,
    /// The flying bosses: where they were drawn.
    pub wasp: Option<Vec2>,
    pub dragonfly: Option<Vec2>,
    pub areas: Vec<Area>,
    pub features: Vec<Feature>,
    /// Everything drawn, in world space.
    pub bounds: Rect,
}

impl Terrain {
    pub fn new(sketch: Sketch) -> Terrain {
        let full = sketch.size;
        // Crop to the drawing plus a margin.
        let (mut lo, mut hi) = (full, IVec2::ZERO);
        for (i, c) in sketch.cells.iter().enumerate() {
            if *c != Cell::Air {
                let p = IVec2::new(i as i32 % full.x, i as i32 / full.x);
                lo = lo.min(p);
                hi = hi.max(p);
            }
        }
        if lo.x > hi.x {
            (lo, hi) = (IVec2::ZERO, full - 1);
        }
        lo -= MARGIN;
        hi += MARGIN;
        let size = hi - lo + 1;
        let mut cells = vec![Cell::Air; (size.x * size.y) as usize];
        for y in 0..size.y {
            for x in 0..size.x {
                let s = IVec2::new(x, y) + lo;
                if s.x >= 0 && s.y >= 0 && s.x < full.x && s.y < full.y {
                    cells[(y * size.x + x) as usize] = sketch.cells[(s.y * full.x + s.x) as usize];
                }
            }
        }

        let marker = |kind| sketch.markers.iter().find(|m| m.kind == kind).map(|m| m.at - lo.as_vec2());
        let origin = match marker(MarkerKind::Origin).or_else(|| marker(MarkerKind::Player)) {
            Some(p) => p.floor().as_ivec2(),
            None => {
                warn!("world sketch: no origin marker (magenta cross), using the canvas center");
                size / 2
            }
        };
        let mut t = Terrain {
            size,
            origin,
            cells,
            clearance: Vec::new(),
            spawn: Vec2::ZERO,
            canvas_origin: origin + lo,
            roach: None,
            wasp: None,
            dragonfly: None,
            areas: Vec::new(),
            features: Vec::new(),
            bounds: Rect::default(),
        };
        t.clearance = t.distance_field(|c| c == Cell::Solid);
        let to_world = |p: Vec2| Vec2::new(p.x - origin.x as f32, origin.y as f32 - p.y) * PX;
        t.bounds = Rect::from_corners(to_world(Vec2::splat(MARGIN as f32)), to_world((size - MARGIN).as_vec2()));
        t.spawn = t.rest(marker(MarkerKind::Player).map_or(Vec2::ZERO, to_world));
        t.roach = marker(MarkerKind::Roach).map(|p| t.rest(to_world(p)));
        t.wasp = marker(MarkerKind::Wasp).map(to_world);
        t.dragonfly = marker(MarkerKind::Dragonfly).map(to_world);
        for s in sketch.slices {
            let rect = Rect::from_corners(to_world((s.rect.min - lo).as_vec2()), to_world((s.rect.max - lo).as_vec2()));
            let n = s.name.to_lowercase();
            let slide = n.contains("slid") || n.contains("slipp") || n.contains("one way down");
            if n.trim_start().starts_with("feature:") {
                let crumbles = n.contains("fall apart") || n.contains("falls apart") || n.contains("crumbl");
                let electrified = n.contains("track") || n.contains("rail") || n.contains("electrif");
                t.features.push(Feature { name: s.name, rect, slide, crumbles, electrified });
            } else {
                t.areas.push(Area { style: Style::from_name(&s.name), name: s.name, rect, slide });
            }
        }
        t
    }

    // --- Grid <-> world ---

    pub fn size(&self) -> IVec2 {
        self.size
    }

    pub fn to_cell(&self, p: Vec2) -> IVec2 {
        IVec2::new((p.x / PX).floor() as i32 + self.origin.x, (-p.y / PX).floor() as i32 + self.origin.y)
    }

    /// Bottom-left corner of a cell in world space.
    pub fn cell_min(&self, c: IVec2) -> Vec2 {
        Vec2::new((c.x - self.origin.x) as f32, (self.origin.y - c.y - 1) as f32) * PX
    }

    fn top_of(&self, row: i32) -> f32 {
        (self.origin.y - row) as f32 * PX
    }

    pub fn cell(&self, c: IVec2) -> Cell {
        if c.x < 0 || c.y < 0 || c.x >= self.size.x || c.y >= self.size.y {
            Cell::Air
        } else {
            self.cells[(c.y * self.size.x + c.x) as usize]
        }
    }

    pub fn solid(&self, c: IVec2) -> bool {
        self.cell(c) == Cell::Solid
    }

    pub fn solid_at(&self, p: Vec2) -> bool {
        self.solid(self.to_cell(p))
    }

    /// Distance from `p` to the nearest solid cell (approximate, capped at ~250 cells).
    pub fn clearance(&self, p: Vec2) -> f32 {
        let c = self.to_cell(p);
        if c.x < 0 || c.y < 0 || c.x >= self.size.x || c.y >= self.size.y {
            return 255.0 * PX;
        }
        self.clearance[(c.y * self.size.x + c.x) as usize] as f32 * PX
    }

    // --- Box queries (the player and other boxes) ---

    /// Cells overlapped by the open box `center ± half`.
    fn box_cells(&self, center: Vec2, half: Vec2) -> impl Iterator<Item = IVec2> {
        const EPS: f32 = 0.01;
        let a = self.to_cell(Vec2::new(center.x - half.x + EPS, center.y + half.y - EPS));
        let b = self.to_cell(Vec2::new(center.x + half.x - EPS, center.y - half.y + EPS));
        (a.y..=b.y).flat_map(move |y| (a.x..=b.x).map(move |x| IVec2::new(x, y)))
    }

    pub fn box_hits(&self, center: Vec2, half: Vec2) -> bool {
        self.box_cells(center, half).any(|c| self.solid(c))
    }

    /// Highest top of the solid cells inside the box.
    pub fn box_top(&self, center: Vec2, half: Vec2) -> Option<f32> {
        self.box_cells(center, half).filter(|&c| self.solid(c)).map(|c| self.top_of(c.y)).reduce(f32::max)
    }

    /// Lowest bottom of the solid cells inside the box.
    pub fn box_bottom(&self, center: Vec2, half: Vec2) -> Option<f32> {
        self.box_cells(center, half).filter(|&c| self.solid(c)).map(|c| self.top_of(c.y) - PX).reduce(f32::min)
    }

    // --- Ground queries ---

    /// Height of the ground surface at `p.x` closest to `p`: up through solid
    /// ground if `p` is buried, else down to the first solid top (within `reach`).
    pub fn surface(&self, p: Vec2, reach: f32) -> Option<f32> {
        let mut c = self.to_cell(p);
        let steps = (reach / PX).ceil() as i32;
        if self.solid(c) {
            for _ in 0..steps {
                c.y -= 1;
                if !self.solid(c) {
                    return Some(self.top_of(c.y + 1));
                }
            }
            None
        } else {
            for _ in 0..steps {
                c.y += 1;
                if self.solid(c) {
                    return Some(self.top_of(c.y));
                }
            }
            None
        }
    }

    /// `p` dropped onto the ground below it (or left where it is if there is none).
    pub fn rest(&self, p: Vec2) -> Vec2 {
        self.surface(p, 2000.0).map_or(p, |y| Vec2::new(p.x, y))
    }

    /// Landing height if a point `foot` below the center moving `from` → `to`
    /// came down onto solid ground from above.
    pub fn landing(&self, from: Vec2, to: Vec2, foot: f32) -> Option<f32> {
        let f = to - Vec2::Y * foot;
        if !self.solid_at(f) {
            return None;
        }
        let top = self.surface(f, 200.0)?;
        (from.y - foot >= top - 6.0).then_some(top)
    }

    /// First solid point on the segment `from` → `to`.
    pub fn raycast(&self, from: Vec2, to: Vec2) -> Option<Vec2> {
        let n = ((to - from).length() / (PX * 0.5)).ceil().max(1.0) as i32;
        (0..=n).map(|i| from.lerp(to, i as f32 / n as f32)).find(|&p| self.solid_at(p))
    }

    /// The flat stretch of ground around `feet`: (left, right). Stops at walls,
    /// drops and wherever the ground strays more than `tolerance` from `feet.y`.
    pub fn flat_span(&self, feet: Vec2, tolerance: f32) -> (f32, f32) {
        let walk = |dir: f32| {
            let (mut x, mut y) = (feet.x, feet.y);
            loop {
                let next = x + dir * PX;
                match self.surface(Vec2::new(next, y + STEP_UP), STEP_UP * 2.0) {
                    Some(ny) if (ny - feet.y).abs() <= tolerance && (ny - y).abs() <= STEP_UP => {
                        x = next;
                        y = ny;
                    }
                    _ => return x,
                }
                if (x - feet.x).abs() > 4000.0 {
                    return x;
                }
            }
        };
        (walk(-1.0), walk(1.0))
    }

    // --- Areas ---

    /// The area containing `p` (the smallest one), else the nearest.
    pub fn area_at(&self, p: Vec2) -> Option<&Area> {
        let inside = self
            .areas
            .iter()
            .filter(|a| a.rect.contains(p))
            .min_by(|a, b| a.rect.size().element_product().total_cmp(&b.rect.size().element_product()));
        inside.or_else(|| {
            self.areas.iter().min_by(|a, b| {
                let d = |r: Rect| (p - p.clamp(r.min, r.max)).length();
                d(a.rect).total_cmp(&d(b.rect))
            })
        })
    }

    /// Inside an area or feature whose slopes only go one way: down.
    pub fn slides_at(&self, p: Vec2) -> bool {
        self.areas.iter().any(|a| a.slide && a.rect.contains(p)) || self.features.iter().any(|f| f.slide && f.rect.contains(p))
    }

    /// The features covering `p`.
    pub fn features_at(&self, p: Vec2) -> impl Iterator<Item = &Feature> {
        self.features.iter().filter(move |f| f.rect.contains(p))
    }

    /// Slope of the ground under `feet` (rise per unit of x; positive = uphill to the right).
    /// Measured over ±6 pixels so single-pixel jaggies don't read as steep; at a
    /// ledge, from the side that has ground; never across a wall or a drop.
    pub fn slope_at(&self, feet: Vec2) -> Option<f32> {
        // The ground heights walking out from the feet one way (`dir` = ±1),
        // stopping at a wall or a drop: a jump between neighbors steeper than
        // any ramp (~72°) is a cliff beside a narrow shelf, not the shelf's
        // slope (that read a ledge between a drop and a wall as a 67° ramp).
        let walk = |dir: i32| -> Vec<(f32, f32)> {
            let mut pts: Vec<(f32, f32)> = Vec::new();
            for k in 0..=3 {
                let dx = (dir * k) as f32 * 2.0 * PX;
                // Start just above the feet: from higher up, a ceiling or
                // overhanging wall over a corner reads as ground.
                let Some(y) = self.surface(feet + Vec2::new(dx, 3.0 * PX), 14.0 * STEP_UP) else { break };
                if pts.last().is_some_and(|&(_, prev)| (y - prev).abs() > 3.0 * 2.0 * PX) {
                    break;
                }
                pts.push((dx, y));
            }
            pts
        };
        // Least-squares line through the ground heights (pixel staircases
        // average out instead of reading 2 px per px).
        let fit = |pts: &[(f32, f32)]| -> Option<f32> {
            if pts.len() < 3 {
                return None;
            }
            let n = pts.len() as f32;
            let (mx, my) = (pts.iter().map(|p| p.0).sum::<f32>() / n, pts.iter().map(|p| p.1).sum::<f32>() / n);
            let sxx: f32 = pts.iter().map(|p| (p.0 - mx) * (p.0 - mx)).sum();
            let sxy: f32 = pts.iter().map(|p| (p.0 - mx) * (p.1 - my)).sum();
            Some(sxy / sxx)
        };
        let (left, right) = (walk(-1), walk(1));
        // Both sides together, for a short shelf neither side covers alone.
        let both = || -> Vec<(f32, f32)> { left.iter().rev().chain(right.iter().skip(1)).copied().collect() };
        match (fit(&left), fit(&right)) {
            // Only as steep as its gentler half: standing on a ledge at the top
            // (or foot) of a steep ramp is flat ground, not part of the ramp.
            (Some(a), Some(b)) => Some(if a.abs() < b.abs() { a } else { b }),
            // Ground on one side only: you're at an edge. A gentle slope there
            // still grips; a steep one is just the face you're perched above
            // (the corner rules decide), not ground you stand on.
            (a, b) => match a.or(b) {
                Some(s) if s.abs() > 1.0 => None,
                s => s.or_else(|| fit(&both())),
            },
        }
    }

    pub fn style_at(&self, p: Vec2) -> Style {
        self.area_at(p).map_or(Style::Concrete, |a| a.style)
    }

    /// Two-pass chamfer distance transform: distance (in cells, capped at 255)
    /// from every cell to the nearest `seed` cell. Row-major, like the grid.
    pub fn distance_field(&self, seed: impl Fn(Cell) -> bool) -> Vec<u8> {
        let (w, h) = (self.size.x as usize, self.size.y as usize);
        let mut d: Vec<u8> = self.cells.iter().map(|&c| if seed(c) { 0 } else { 255 }).collect();
        let relax = |d: &mut Vec<u8>, i: usize, j: usize| {
            let v = d[j].saturating_add(1);
            if v < d[i] {
                d[i] = v;
            }
        };
        for y in 0..h {
            for x in 0..w {
                let i = y * w + x;
                if x > 0 {
                    relax(&mut d, i, i - 1);
                }
                if y > 0 {
                    relax(&mut d, i, i - w);
                    if x > 0 {
                        relax(&mut d, i, i - w - 1);
                    }
                    if x + 1 < w {
                        relax(&mut d, i, i - w + 1);
                    }
                }
            }
        }
        for y in (0..h).rev() {
            for x in (0..w).rev() {
                let i = y * w + x;
                if x + 1 < w {
                    relax(&mut d, i, i + 1);
                }
                if y + 1 < h {
                    relax(&mut d, i, i + w);
                    if x + 1 < w {
                        relax(&mut d, i, i + w + 1);
                    }
                    if x > 0 {
                        relax(&mut d, i, i + w - 1);
                    }
                }
            }
        }
        d
    }
}

/// Exact keyword that selected the material (same precedence as Style::from_name).
pub(crate) fn style_tag(name: &str) -> String {
    let n = name.to_lowercase();
    for tag in ["subway", "station", "metro", "train"] {
        if n.contains(tag) { return tag.into(); }
    }
    for tag in ["treatment", "filtration", "pumping"] {
        if n.contains(tag) { return tag.into(); }
    }
    for tag in ["sinkhole", "sunken"] {
        if n.contains(tag) { return tag.into(); }
    }
    for tag in ["catacomb", "crypt", "ossuary", "tomb"] {
        if n.contains(tag) { return tag.into(); }
    }
    for tag in ["shopping", "mall", "store"] {
        if n.contains(tag) { return tag.into(); }
    }
    for tag in ["dam", "spillway", "reservoir"] {
        if n.split(|c: char| !c.is_alphanumeric()).any(|w| w == tag) { return tag.into(); }
    }
    for tag in ["bunker", "military", "shelter"] {
        if n.contains(tag) { return tag.into(); }
    }
    for tag in ["cave", "cavern", "grotto"] {
        if n.contains(tag) { return tag.into(); }
    }
    for tag in ["trash", "dump", "junk", "garbage", "scrap"] {
        if n.contains(tag) { return tag.into(); }
    }
    for tag in ["sewer", "pipe", "drain", "canal"] {
        if n.contains(tag) { return tag.into(); }
    }
    for tag in ["park", "square", "plaza", "garden"] {
        if n.contains(tag) { return tag.into(); }
    }
    "default (no material keyword)".into()
}
