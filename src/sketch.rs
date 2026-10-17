//! Reads the world sketch (`assets/world.aseprite`) and sorts its pixels by meaning.
//!
//! The sketch is drawn with a few flat colors (whatever the layers are called):
//! white = solid ground, dark gray = passable structures (building interiors),
//! black / transparent = open air, saturated colors = markers (magenta cross =
//! world origin, red = player spawn, yellow = Gutter King, green = Venom
//! Mother). Slices name the areas.
//!
//! Only RGBA sprites are supported; layers blend with Normal or Multiply.
//!
//! `Sketch::bake` stores the classified result in a compact file
//! (`assets/world.baked`): the state Claude last interpreted, and what release
//! builds embed. `decode` reads both formats.

use bevy::math::{IRect, IVec2, Vec2};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Cell {
    Air,
    Solid,
    /// Passable structure: drawn, but no collision.
    Back,
}

/// The sketch colors (RGB, ±2) that mark the dragonfly and spider bosses.
const DRAGONFLY_MARK: [u8; 3] = [0xd1, 0xb0, 0xc6];
const SPIDER_MARK: [u8; 3] = [0x81, 0xa6, 0xf3];
/// Purple `#a200a2`: the centipede (magenta-ish, so it must be matched before the channel test).
const CENTIPEDE_MARK: [u8; 3] = [0xa2, 0x00, 0xa2];
/// Violet `#b800ff`: the frog (would read as the origin's magenta in the channel test).
const FROG_MARK: [u8; 3] = [0xb8, 0x00, 0xff];
/// Brown `#8f563b`: the grasshopper (dull, would read as background structure).
const GRASSHOPPER_MARK: [u8; 3] = [0x8f, 0x56, 0x3b];
/// Dull red `#ac3232`: the King Rat (would read as the player's red in the channel test).
const RAT_MARK: [u8; 3] = [0xac, 0x32, 0x32];
/// Cyan `#00ffff`: the mantis (no channel test match: green and blue both high).
const MANTIS_MARK: [u8; 3] = [0x00, 0xff, 0xff];
/// Dark blue `#5b6ee1`: the crab (the channel test has no blue class).
const CRAB_MARK: [u8; 3] = [0x5b, 0x6e, 0xe1];
/// The beetle's orange (its blood): saturated, so matched before the channel test (it would read as the player's red).
const BEETLE_MARK: [u8; 3] = [0xff, 0x81, 0x00];
/// Pure blue `#0000ff`: a power pill (the tile under it is a plain wall).
const PILL_MARK: [u8; 3] = [0x00, 0x00, 0xff];
/// Pale green `#bdffc2`: a pill bug that refills the whole health (the tile under it is a plain wall).
const HEALTH_MARK: [u8; 3] = [0xbd, 0xff, 0xc2];

/// Light purple `#bc75be`: asks `level::loose` for a falling platform, over a back wall (`#976099` over open air).
const FALLING_MARK: [u8; 3] = [0xbc, 0x75, 0xbe];
/// The same hint, over open air instead of a back wall.
const FALLING_OPEN_MARK: [u8; 3] = [0x97, 0x60, 0x99];
/// Green `#34744e` (`#29573c` over open air): the same slab, but fixed in place.
const FIXED_MARK: [u8; 3] = [0x34, 0x74, 0x4e];
const FIXED_OPEN_MARK: [u8; 3] = [0x29, 0x57, 0x3c];
/// Clear water: `#97e6e6` over a back wall, `#629292` over open air. The filled box is the pool.
const CLEAR_WATER_MARK: [u8; 3] = [0x97, 0xe6, 0xe6];
const CLEAR_WATER_OPEN_MARK: [u8; 3] = [0x62, 0x92, 0x92];
/// Sludge: `#8f9ed5` over a back wall, `#7682ac` over open air.
const SLUDGE_MARK: [u8; 3] = [0x8f, 0x9e, 0xd5];
const SLUDGE_OPEN_MARK: [u8; 3] = [0x76, 0x82, 0xac];

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MarkerKind {
    /// The reference point: world (0, 0).
    Origin,
    Player,
    Roach,
    Wasp,
    /// Dusty pink `#d1b0c6` (`DRAGONFLY_MARK`, the dragonfly's own color): the dragonfly boss.
    Dragonfly,
    /// Dusty blue `#7f9fd6` (`SPIDER_MARK`, the spider's own color): the spider boss.
    Spider,
    /// Purple `#a200a2` (`CENTIPEDE_MARK`, the centipede's own color): the centipede boss.
    Centipede,
    /// Violet `#b800ff` (`FROG_MARK`, the frog's own color): the frog boss (rests on the ground).
    Frog,
    /// Brown `#8f563b` (`GRASSHOPPER_MARK`, its blood): the grasshopper boss (rests on the ground).
    Grasshopper,
    /// Dull red `#ac3232` (`RAT_MARK`, its blood): the King Rat boss (rests on the ground).
    Rat,
    /// Purple `#bc75be` (`FALLING_MARK`, over a wall) or `#976099` (`FALLING_OPEN_MARK`, over open air):
    /// a hint to place a falling platform here.
    Falling,
    /// Cyan `#00ffff` (`MANTIS_MARK`, its blood): the mantis boss (rests on the ground).
    /// After `Falling`: the baked format stores the variant index.
    Mantis,
    /// Green `#34744e` (`FIXED_MARK`, over a wall) or `#29573c` (`FIXED_OPEN_MARK`, over open air):
    /// the same slab, fixed in place.
    Fixed,
    /// Light cyan `#97e6e6` (`CLEAR_WATER_MARK`, over a wall) or `#629292` (`CLEAR_WATER_OPEN_MARK`, over open air):
    /// the blob's bounding box is a pool of clear water.
    ClearWater,
    /// Blue-gray `#8f9ed5` (`SLUDGE_MARK`, over a wall) or `#7682ac` (`SLUDGE_OPEN_MARK`, over open air):
    /// the blob's bounding box is a pool of sludge.
    Sludge,
    /// Dark blue `#5b6ee1` (`CRAB_MARK`, its blood): the crab boss (rests on the ground).
    Crab,
    /// Orange `#ff8100` (`BEETLE_MARK`, its blood): the beetle boss (rests on the ground).
    Beetle,
    /// Pure blue `#0000ff`: a power pill creature crawls here; the tile under it is a normal wall.
    PowerPill,
    /// Pale green `#bdffc2`: a pill bug that refills the whole health; the tile under it is a normal wall.
    HealthPill,
}

pub struct Marker {
    pub kind: MarkerKind,
    /// Centroid in sketch pixels.
    pub at: Vec2,
    /// Size of its bounding box in sketch pixels.
    pub extent: Vec2,
    /// Fitted slope of the blob (rise per run, sketch y down); 0 when it has no clear direction.
    pub slope: f32,
}

pub struct Slice {
    pub name: String,
    pub rect: IRect,
}

pub struct Sketch {
    pub size: IVec2,
    /// Row-major, y down (sketch pixel coordinates).
    pub cells: Vec<Cell>,
    pub markers: Vec<Marker>,
    pub slices: Vec<Slice>,
}

/// Starts a baked sketch; the rest is zlib-compressed.
const BAKED_MAGIC: &[u8; 8] = b"CAMSKT03";
/// Markers with an extent but no slope.
const BAKED_MAGIC_V2: &[u8; 8] = b"CAMSKT02";
/// The previous format: markers without an extent.
const BAKED_MAGIC_V1: &[u8; 8] = b"CAMSKT01";

/// Reads a baked sketch, or parses an .aseprite file and classifies the
/// composited first frame.
pub fn decode(bytes: &[u8]) -> Result<Sketch, String> {
    if let Some(packed) = bytes.strip_prefix(BAKED_MAGIC) {
        return unbake(packed, 3);
    }
    if let Some(packed) = bytes.strip_prefix(BAKED_MAGIC_V2) {
        return unbake(packed, 2);
    }
    if let Some(packed) = bytes.strip_prefix(BAKED_MAGIC_V1) {
        return unbake(packed, 1);
    }
    let ase = parse(bytes)?;
    let (w, h) = (ase.size.x as usize, ase.size.y as usize);

    let mut cells = vec![Cell::Air; w * h];
    let mut marks: Vec<Option<MarkerKind>> = vec![None; w * h];
    for (i, px) in ase.pixels.iter().enumerate() {
        let [r, g, b, a] = px.map(|c| c as i32);
        if a < 128 {
            continue;
        }
        let (max, min) = (r.max(g).max(b), r.min(g).min(b));
        // Their colors are nearly gray, so they're matched by color, not by saturation.
        let is = |mark: [u8; 3]| mark.iter().zip([r, g, b]).all(|(m, c)| (*m as i32 - c).abs() <= 2);
        if is(FALLING_MARK) || is(FALLING_OPEN_MARK) {
            marks[i] = Some(MarkerKind::Falling);
            cells[i] = if is(FALLING_MARK) { Cell::Back } else { Cell::Air };
            continue;
        }
        if is(FIXED_MARK) || is(FIXED_OPEN_MARK) {
            marks[i] = Some(MarkerKind::Fixed);
            cells[i] = if is(FIXED_MARK) { Cell::Back } else { Cell::Air };
            continue;
        }
        if is(CLEAR_WATER_MARK) || is(CLEAR_WATER_OPEN_MARK) {
            marks[i] = Some(MarkerKind::ClearWater);
            cells[i] = if is(CLEAR_WATER_MARK) { Cell::Back } else { Cell::Air };
            continue;
        }
        if is(SLUDGE_MARK) || is(SLUDGE_OPEN_MARK) {
            marks[i] = Some(MarkerKind::Sludge);
            cells[i] = if is(SLUDGE_MARK) { Cell::Back } else { Cell::Air };
            continue;
        }
        if is(DRAGONFLY_MARK) {
            marks[i] = Some(MarkerKind::Dragonfly);
            continue;
        }
        if is(CENTIPEDE_MARK) {
            marks[i] = Some(MarkerKind::Centipede);
            continue;
        }
        if is(GRASSHOPPER_MARK) {
            marks[i] = Some(MarkerKind::Grasshopper);
            continue;
        }
        if is(RAT_MARK) {
            marks[i] = Some(MarkerKind::Rat);
            continue;
        }
        if is(MANTIS_MARK) {
            marks[i] = Some(MarkerKind::Mantis);
            continue;
        }
        if is(CRAB_MARK) {
            marks[i] = Some(MarkerKind::Crab);
            continue;
        }
        if is(BEETLE_MARK) {
            marks[i] = Some(MarkerKind::Beetle);
            continue;
        }
        if is(FROG_MARK) {
            marks[i] = Some(MarkerKind::Frog);
            continue;
        }
        if is(SPIDER_MARK) {
            marks[i] = Some(MarkerKind::Spider);
            continue;
        }
        if is(PILL_MARK) {
            marks[i] = Some(MarkerKind::PowerPill);
            cells[i] = Cell::Back;
            continue;
        }
        if is(HEALTH_MARK) {
            marks[i] = Some(MarkerKind::HealthPill);
            cells[i] = Cell::Back;
            continue;
        }
        if max - min > 100 {
            marks[i] = match (r > 160, g > 160, b > 160) {
                (true, false, false) => Some(MarkerKind::Player),
                (true, false, true) => Some(MarkerKind::Origin),
                (true, true, false) => Some(MarkerKind::Roach),
                (false, true, false) => Some(MarkerKind::Wasp),
                _ => None,
            };
            continue;
        }
        let lum = (r + g + b) / 3;
        cells[i] = if lum > 200 {
            Cell::Solid
        } else if lum >= 40 {
            Cell::Back
        } else {
            Cell::Air
        };
    }
    // Markers may be drawn over the ground: under them, continue whatever surrounds them.
    for y in 0..h {
        for x in 0..w {
            if matches!(marks[y * w + x], None | Some(MarkerKind::Falling | MarkerKind::Fixed | MarkerKind::ClearWater | MarkerKind::Sludge | MarkerKind::PowerPill | MarkerKind::HealthPill)) {
                continue;
            }
            let side = |dir: isize| {
                let mut i = x as isize + dir;
                while i >= 0 && (i as usize) < w && marks[y * w + i as usize].is_some() {
                    i += dir;
                }
                if i >= 0 && (i as usize) < w { cells[y * w + i as usize] } else { Cell::Air }
            };
            cells[y * w + x] = match (side(-1), side(1)) {
                (Cell::Solid, Cell::Solid) => Cell::Solid,
                (Cell::Back, _) | (_, Cell::Back) => Cell::Back,
                _ => Cell::Air,
            };
        }
    }

    Ok(Sketch { size: ase.size, cells, markers: find_markers(&marks, w, h), slices: ase.slices })
}

impl Sketch {
    /// Compact form: size, run-length encoded cells, markers, slices (zlib).
    pub fn bake(&self) -> Vec<u8> {
        let mut raw = Vec::new();
        raw.extend((self.size.x as u32).to_le_bytes());
        raw.extend((self.size.y as u32).to_le_bytes());
        let mut runs = Vec::new();
        for &c in &self.cells {
            match runs.last_mut() {
                Some((last, n)) if *last == c => *n += 1,
                _ => runs.push((c, 1u32)),
            }
        }
        raw.extend((runs.len() as u32).to_le_bytes());
        for (c, n) in runs {
            raw.push(c as u8);
            raw.extend(n.to_le_bytes());
        }
        raw.extend((self.markers.len() as u32).to_le_bytes());
        for m in &self.markers {
            raw.push(m.kind as u8);
            raw.extend(m.at.x.to_bits().to_le_bytes());
            raw.extend(m.at.y.to_bits().to_le_bytes());
            raw.extend(m.extent.x.to_bits().to_le_bytes());
            raw.extend(m.extent.y.to_bits().to_le_bytes());
            raw.extend(m.slope.to_bits().to_le_bytes());
        }
        raw.extend((self.slices.len() as u32).to_le_bytes());
        for s in &self.slices {
            raw.extend((s.name.len() as u16).to_le_bytes());
            raw.extend(s.name.as_bytes());
            for v in [s.rect.min.x, s.rect.min.y, s.rect.max.x, s.rect.max.y] {
                raw.extend(v.to_le_bytes());
            }
        }
        let mut out = BAKED_MAGIC.to_vec();
        out.extend(miniz_oxide::deflate::compress_to_vec_zlib(&raw, 9));
        out
    }
}

fn unbake(packed: &[u8], version: u8) -> Result<Sketch, String> {
    let raw = miniz_oxide::inflate::decompress_to_vec_zlib(packed).map_err(|e| format!("bad baked sketch: {e:?}"))?;
    let mut r = Reader { data: &raw, pos: 0 };
    let size = IVec2::new(r.u32()? as i32, r.u32()? as i32);
    let total = size.x as usize * size.y as usize;
    let mut cells = Vec::with_capacity(total);
    for _ in 0..r.u32()? {
        let cell = match r.u8()? {
            0 => Cell::Air,
            1 => Cell::Solid,
            2 => Cell::Back,
            c => return Err(format!("bad baked cell {c}")),
        };
        let n = r.u32()? as usize;
        if cells.len() + n > total {
            return Err("baked cells overflow the canvas".into());
        }
        cells.extend(std::iter::repeat_n(cell, n));
    }
    if cells.len() != total {
        return Err("baked cells don't fill the canvas".into());
    }
    let mut markers = Vec::new();
    for _ in 0..r.u32()? {
        let kind = match r.u8()? {
            0 => MarkerKind::Origin,
            1 => MarkerKind::Player,
            2 => MarkerKind::Roach,
            3 => MarkerKind::Wasp,
            4 => MarkerKind::Dragonfly,
            5 => MarkerKind::Spider,
            6 => MarkerKind::Centipede,
            7 => MarkerKind::Frog,
            8 => MarkerKind::Grasshopper,
            9 => MarkerKind::Rat,
            10 => MarkerKind::Falling,
            11 => MarkerKind::Mantis,
            12 => MarkerKind::Fixed,
            13 => MarkerKind::ClearWater,
            14 => MarkerKind::Sludge,
            15 => MarkerKind::Crab,
            16 => MarkerKind::Beetle,
            17 => MarkerKind::PowerPill,
            18 => MarkerKind::HealthPill,
            k => return Err(format!("bad baked marker {k}")),
        };
        let at = Vec2::new(f32::from_bits(r.u32()?), f32::from_bits(r.u32()?));
        let extent = if version >= 2 { Vec2::new(f32::from_bits(r.u32()?), f32::from_bits(r.u32()?)) } else { Vec2::ZERO };
        let slope = if version >= 3 { f32::from_bits(r.u32()?) } else { 0.0 };
        markers.push(Marker { kind, at, extent, slope });
    }
    let mut slices = Vec::new();
    for _ in 0..r.u32()? {
        let name = r.string()?;
        let rect = IRect::new(r.i32()?, r.i32()?, r.i32()?, r.i32()?);
        slices.push(Slice { name, rect });
    }
    Ok(Sketch { size, cells, markers, slices })
}

/// One marker per connected blob of marker pixels (8-connected).
fn find_markers(marks: &[Option<MarkerKind>], w: usize, h: usize) -> Vec<Marker> {
    let mut seen = vec![false; marks.len()];
    let mut out = Vec::new();
    for start in 0..marks.len() {
        let Some(kind) = marks[start] else { continue };
        if seen[start] {
            continue;
        }
        seen[start] = true;
        let (mut sum, mut n, mut stack) = (Vec2::ZERO, 0.0, vec![start]);
        let (mut lo, mut hi) = (Vec2::splat(f32::MAX), Vec2::splat(f32::MIN));
        let (mut sxx, mut sxy) = (0.0, 0.0);
        while let Some(i) = stack.pop() {
            let (x, y) = ((i % w) as i32, (i / w) as i32);
            let c = Vec2::new(x as f32 + 0.5, y as f32 + 0.5);
            sum += c;
            sxx += c.x * c.x;
            sxy += c.x * c.y;
            lo = lo.min(c - 0.5);
            hi = hi.max(c + 0.5);
            n += 1.0;
            for dy in -1..=1 {
                for dx in -1..=1 {
                    let (nx, ny) = (x + dx, y + dy);
                    if nx < 0 || ny < 0 || nx >= w as i32 || ny >= h as i32 {
                        continue;
                    }
                    let j = ny as usize * w + nx as usize;
                    if !seen[j] && marks[j] == Some(kind) {
                        seen[j] = true;
                        stack.push(j);
                    }
                }
            }
        }
        // Least-squares line through the blob (y down), when it is spread out horizontally.
        let var_x = sxx - sum.x * sum.x / n;
        let slope = if var_x > 1.0 { (sxy - sum.x * sum.y / n) / var_x } else { 0.0 };
        // Water fills the blob's bounding box, so its position is that box's center (not the centroid of a slanted shape).
        let at = if matches!(kind, MarkerKind::ClearWater | MarkerKind::Sludge) { (lo + hi) / 2.0 } else { sum / n };
        out.push(Marker { kind, at, extent: hi - lo, slope });
    }
    out
}

struct Ase {
    size: IVec2,
    pixels: Vec<[u8; 4]>,
    slices: Vec<Slice>,
}

struct Layer {
    visible: bool,
    multiply: bool,
    opacity: u8,
}

struct Reader<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    fn bytes(&mut self, n: usize) -> Result<&'a [u8], String> {
        let end = self.pos + n;
        let out = self.data.get(self.pos..end).ok_or("unexpected end of file")?;
        self.pos = end;
        Ok(out)
    }
    fn u8(&mut self) -> Result<u8, String> {
        Ok(self.bytes(1)?[0])
    }
    fn u16(&mut self) -> Result<u16, String> {
        Ok(u16::from_le_bytes(self.bytes(2)?.try_into().unwrap()))
    }
    fn i16(&mut self) -> Result<i16, String> {
        Ok(self.u16()? as i16)
    }
    fn u32(&mut self) -> Result<u32, String> {
        Ok(u32::from_le_bytes(self.bytes(4)?.try_into().unwrap()))
    }
    fn i32(&mut self) -> Result<i32, String> {
        Ok(self.u32()? as i32)
    }
    fn string(&mut self) -> Result<String, String> {
        let n = self.u16()? as usize;
        Ok(String::from_utf8_lossy(self.bytes(n)?).into_owned())
    }
}

/// Composites the first frame (see the Aseprite file format spec).
fn parse(data: &[u8]) -> Result<Ase, String> {
    let mut r = Reader { data, pos: 0 };
    r.u32()?;
    if r.u16()? != 0xA5E0 {
        return Err("not an .aseprite file".into());
    }
    r.u16()?; // frames: only the first one is used
    let (w, h) = (r.u16()? as usize, r.u16()? as usize);
    if r.u16()? != 32 {
        return Err("only RGBA sprites are supported (Sprite > Color Mode > RGB)".into());
    }
    let opacity_valid = r.u32()? & 1 != 0;
    r.pos = 128;

    let frame_start = r.pos;
    let frame_len = r.u32()? as usize;
    r.u16()?;
    let old_chunks = r.u16()? as usize;
    r.bytes(4)?;
    let new_chunks = r.u32()? as usize;
    let chunks = if new_chunks == 0 { old_chunks } else { new_chunks };

    let mut layers: Vec<Layer> = Vec::new();
    // Visibility of the group at each nesting level, so hidden groups hide their children.
    let mut group_visible: Vec<bool> = Vec::new();
    let mut pixels = vec![[0u8; 4]; w * h];
    let mut slices = Vec::new();

    for _ in 0..chunks {
        let start = r.pos;
        let len = r.u32()? as usize;
        let kind = r.u16()?;
        match kind {
            0x2004 => {
                let flags = r.u16()?;
                let _layer_type = r.u16()?;
                let level = r.u16()? as usize;
                r.bytes(4)?;
                let blend = r.u16()?;
                let opacity = r.u8()?;
                group_visible.truncate(level);
                let visible = flags & 1 != 0 && group_visible.iter().all(|&v| v);
                group_visible.push(visible);
                layers.push(Layer { visible, multiply: blend == 1, opacity: if opacity_valid { opacity } else { 255 } });
            }
            0x2005 => {
                let layer = r.u16()? as usize;
                let (x, y) = (r.i16()? as i32, r.i16()? as i32);
                let opacity = r.u8()?;
                let cel_type = r.u16()?;
                r.bytes(7)?;
                let Some(l) = layers.get(layer) else { return Err("cel before its layer".into()) };
                if l.visible && (cel_type == 0 || cel_type == 2) {
                    let (cw, ch) = (r.u16()? as usize, r.u16()? as usize);
                    let rest = &data[r.pos..start + len];
                    let raw = if cel_type == 2 {
                        miniz_oxide::inflate::decompress_to_vec_zlib(rest).map_err(|e| format!("bad cel data: {e:?}"))?
                    } else {
                        rest.to_vec()
                    };
                    let alpha = l.opacity as u32 * opacity as u32 / 255;
                    for cy in 0..ch {
                        for cx in 0..cw {
                            let (px, py) = (x + cx as i32, y + cy as i32);
                            if px < 0 || py < 0 || px >= w as i32 || py >= h as i32 {
                                continue;
                            }
                            let k = (cy * cw + cx) * 4;
                            let Some(src) = raw.get(k..k + 4) else { continue };
                            let dst = &mut pixels[py as usize * w + px as usize];
                            *dst = blend_px(*dst, [src[0], src[1], src[2], src[3]], alpha, l.multiply);
                        }
                    }
                }
            }
            0x2022 => {
                let keys = r.u32()?;
                let flags = r.u32()?;
                r.u32()?;
                let name = r.string()?;
                for _ in 0..keys {
                    let frame = r.u32()?;
                    let (x, y, sw, sh) = (r.i32()?, r.i32()?, r.u32()? as i32, r.u32()? as i32);
                    if flags & 1 != 0 {
                        r.bytes(16)?;
                    }
                    if flags & 2 != 0 {
                        r.bytes(8)?;
                    }
                    if frame == 0 {
                        slices.push(Slice { name: name.clone(), rect: IRect::new(x, y, x + sw, y + sh) });
                    }
                }
            }
            _ => {}
        }
        r.pos = start + len;
    }
    if r.pos > frame_start + frame_len {
        return Err("corrupt frame".into());
    }
    Ok(Ase { size: IVec2::new(w as i32, h as i32), pixels, slices })
}

fn blend_px(dst: [u8; 4], src: [u8; 4], opacity: u32, multiply: bool) -> [u8; 4] {
    let a = src[3] as u32 * opacity / 255;
    if a == 0 {
        return dst;
    }
    let mut out = [0u8; 4];
    for c in 0..3 {
        let s = if multiply && dst[3] > 0 { src[c] as u32 * dst[c] as u32 / 255 } else { src[c] as u32 };
        out[c] = ((s * a + dst[c] as u32 * (255 - a)) / 255) as u8;
    }
    out[3] = (a + dst[3] as u32 * (255 - a) / 255) as u8;
    out
}
