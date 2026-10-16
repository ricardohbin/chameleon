//! Shared boss machinery. Every boss fights in its arena (found around its
//! marker in the world sketch); this module
//! does what they all have in common: picking which boss to fight, health,
//! getting slashed, the death spectacle, torn-off gibs, healing when the
//! chameleon dies, and the HUD status. Each boss module (`wasp`, `roach`, `dragonfly`, `spider`, `centipede`, `frog`, `grasshopper`, `king_rat`)
//! only builds its body and runs its own AI and corpse.
//!
//! Every boss with a marker in the sketch is always there, asleep in its lair
//! (the slice it's drawn in). It wakes when the chameleon steps into its arena
//! (the walkable floor around a walker, the open chamber around a flyer) and
//! hunts it; if the chameleon leaves the lair or gets `LEASH` away, the boss is
//! reset: back home, full health, asleep. Dead bosses stay dead.
//! F2 teleports the chameleon to the next boss's lair.
//!
//! Adding a boss: a module with a `spawn` fn (spawns a `Boss` + its own AI
//! component) and its systems, then a `BossKind` entry.

use std::f32::consts::PI;

use bevy::prelude::*;

use crate::{
    camera::{HitStop, Shake, ZoomPunch},
    chameleon::{self, ChameleonAnim, ColorEarned},
    centipede, dragonfly, frog, grasshopper, king_rat, spider,
    gore::{Bleed, Fluid, Paint},
    hud::ScreenFlash,
    level::{self, Platform, Restart, WorldReloaded},
    player::{self, Hurt, Player, Respawned},
    roach,
    spit::{self, SpitHit},
    terrain::Terrain,
    wasp,
};

pub struct BossPlugin;

impl Plugin for BossPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<BossStatus>()
            .init_resource::<Defeated>()
            .add_message::<BossHit>()
            .add_message::<Beaten>()
            .add_systems(Startup, spawn_all)
            .add_systems(PreUpdate, forget_defeated.after(level::restart))
            .add_systems(
                Update,
                (
                    visit_next_boss,
                    restart_on_reload,
                    lairs,
                    slash_bosses,
                    tick_poise,
                    reset_on_respawn,
                    flash_bosses,
                    update_status,
                    move_gibs,
                )
                    .chain()
                    .after(player::move_player)
                    .after(chameleon::update_anim_state)
                    .after(spit::move_spit),
            );
    }
}

/// The bosses beaten so far, in order. Each one gives a power for good: the
/// Gutter King the dash, the Venom Mother the venom spit, the dragonfly the
/// air spin, the spider walking on walls, the centipede burrowing through rubble, the frog moving
/// through water as if on land, the grasshopper the charged big jump, the King Rat breathing the sewer air. F8 forgets them.
#[derive(Resource, Default)]
pub struct Defeated(pub Vec<BossKind>);

impl Defeated {
    pub fn add(&mut self, kind: BossKind) {
        if !self.0.contains(&kind) {
            self.0.push(kind);
        }
    }

    /// Shift / K: dash.
    pub fn dash(&self) -> bool {
        self.0.contains(&BossKind::Roach)
    }

    /// Right mouse button / G: spit venom.
    pub fn spit(&self) -> bool {
        self.0.contains(&BossKind::Wasp)
    }

    /// Attacking in the air: the spin.
    pub fn spin(&self) -> bool {
        self.0.contains(&BossKind::Dragonfly)
    }

    /// E at a rubble feature: burrow through it.
    pub fn burrow(&self) -> bool {
        self.0.contains(&BossKind::Centipede)
    }

    /// In water: no wading, no tiring, no drowning (moves as on land).
    pub fn wade(&self) -> bool {
        self.0.contains(&BossKind::Frog)
    }

    /// Holding down on the ground charges a big jump (released = launched).
    pub fn leap(&self) -> bool {
        self.0.contains(&BossKind::Grasshopper)
    }

    /// The sewer's foul air doesn't hurt.
    pub fn sewer(&self) -> bool {
        self.0.contains(&BossKind::Rat)
    }

    /// Pushing into a wall: walking on it.
    pub fn climb(&self) -> bool {
        self.0.contains(&BossKind::Spider)
    }
}

/// A boss was just killed (its power is earned now).
#[derive(Message)]
pub struct Beaten(pub BossKind);

impl BossKind {
    /// The power it gives, as the HUD names it.
    pub fn power(self) -> &'static str {
        match self {
            BossKind::Roach => "DASH  (Shift / K)",
            BossKind::Wasp => "VENOM SPIT  (Right click / G)",
            BossKind::Dragonfly => "AIR SPIN  (attack in the air)",
            BossKind::Spider => "WALL WALK  (push into a wall, W / S)",
            BossKind::Centipede => "BURROW  (aim into rubble, E)",
            BossKind::Frog => "WATER WALK  (water no longer slows or tires you)",
            BossKind::Grasshopper => "BIG JUMP  (hold S to charge, release)",
            BossKind::Rat => "SEWER RAT  (the sewer air no longer hurts you)",
        }
    }
}

fn forget_defeated(mut restarted: MessageReader<Restart>, mut defeated: ResMut<Defeated>) {
    if restarted.read().count() > 0 {
        defeated.0.clear();
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum BossKind {
    Wasp,
    Roach,
    Dragonfly,
    Spider,
    Centipede,
    Frog,
    Grasshopper,
    Rat,
}

impl BossKind {
    pub const ALL: [BossKind; 8] = [BossKind::Wasp, BossKind::Roach, BossKind::Dragonfly, BossKind::Spider, BossKind::Centipede, BossKind::Frog, BossKind::Grasshopper, BossKind::Rat];

    /// Where its marker is in the sketch (no marker = no boss).
    fn marker(self, terrain: &Terrain) -> Option<Vec2> {
        match self {
            BossKind::Roach => terrain.roach,
            BossKind::Wasp => terrain.wasp,
            BossKind::Dragonfly => terrain.dragonfly,
            BossKind::Spider => terrain.spider,
            BossKind::Centipede => terrain.centipede,
            BossKind::Frog => terrain.frog,
            BossKind::Grasshopper => terrain.grasshopper,
            BossKind::Rat => terrain.rat,
        }
    }

    /// Spawns it asleep in its lair, if the sketch has its marker.
    fn spawn(self, commands: &mut Commands, meshes: &mut Assets<Mesh>, materials: &mut Assets<ColorMaterial>, terrain: &Terrain) {
        let Some(marker) = self.marker(terrain) else { return };
        let arena = Arena::find(self, terrain);
        // Where it wakes: the floor it fights on, or the open chamber it flies in.
        let mut arena_rect = match self {
            BossKind::Roach | BossKind::Frog | BossKind::Grasshopper | BossKind::Rat => Rect::new(arena.left, arena.floor - 100.0, arena.right, arena.floor + 500.0),
            BossKind::Wasp | BossKind::Dragonfly | BossKind::Spider | BossKind::Centipede => chamber(terrain, marker, 70),
        };
        // The lair is the slice it's drawn in (always covering its arena).
        let region = terrain
            .areas
            .iter()
            .filter(|a| a.rect.contains(marker))
            .min_by(|a, b| a.rect.size().element_product().total_cmp(&b.rect.size().element_product()))
            .map_or(Rect::from_center_half_size(marker, Vec2::new(1000.0, 700.0)), |a| a.rect)
            .union(arena_rect);
        // The centipede's domain is the whole cave system: it wakes as soon as you set foot in it.
        if self == BossKind::Centipede {
            arena_rect = region;
        }
        // The King Rat roams all the ground it can get around on: it wakes when you step on any of it.
        if self == BossKind::Rat {
            arena_rect = king_rat::wake_area(terrain, &arena, region);
        }
        let e = match self {
            BossKind::Wasp => wasp::spawn(commands, meshes, materials, &arena),
            BossKind::Roach => roach::spawn(commands, meshes, materials, &arena),
            BossKind::Dragonfly => dragonfly::spawn(commands, meshes, materials, &arena),
            BossKind::Spider => spider::spawn(commands, meshes, materials, &arena, terrain),
            BossKind::Centipede => centipede::spawn(commands, meshes, materials, &arena, terrain, region),
            BossKind::Frog => frog::spawn(commands, meshes, materials, &arena),
            BossKind::Grasshopper => grasshopper::spawn(commands, meshes, materials, &arena),
            BossKind::Rat => king_rat::spawn(commands, meshes, materials, &arena, terrain, region),
        };
        debug!("{self:?} lair {:.0?}..{:.0?}, wakes in {:.0?}..{:.0?}", region.min, region.max, arena_rect.min, arena_rect.max);
        commands.entity(e).insert((Lair { kind: self, region, arena: arena_rect }, Sense::default()));
    }
}

/// The open space around `at` (flood fill through non-solid cells, at most
/// `radius` sketch pixels away), as a rectangle.
pub(crate) fn chamber(terrain: &Terrain, at: Vec2, radius: i32) -> Rect {
    let start = terrain.to_cell(at);
    let (mut lo, mut hi) = (start, start);
    let mut seen = std::collections::HashSet::from([start]);
    let mut queue = std::collections::VecDeque::from([start]);
    while let Some(c) = queue.pop_front() {
        lo = lo.min(c);
        hi = hi.max(c);
        for d in [IVec2::X, IVec2::NEG_X, IVec2::Y, IVec2::NEG_Y] {
            let n = c + d;
            if (n - start).abs().max_element() <= radius && !terrain.solid(n) && seen.insert(n) {
                queue.push_back(n);
            }
        }
    }
    Rect::from_corners(terrain.cell_min(lo), terrain.cell_min(hi) + Vec2::splat(crate::terrain::PX))
}

/// What a boss perceives and wants this frame (shown by the F1 debug overlay).
#[derive(Component, Default)]
pub struct Sense {
    /// Where it wants to be.
    pub goal: Option<Vec2>,
    /// Where it's actually heading (a path step when walls are in the way).
    pub target: Option<Vec2>,
    /// Clear line of sight to the chameleon.
    pub sees_player: bool,
    /// Its planned route (flyers: waypoints to where they want to be).
    pub path: Vec<Vec2>,
}

/// How far the chameleon can lead an awake boss before it gives up and resets.
const LEASH: f32 = 1800.0;

/// Where a boss lives.
#[derive(Component)]
pub struct Lair {
    pub kind: BossKind,
    /// Leaving this resets it.
    pub region: Rect,
    /// Entering this wakes it.
    pub arena: Rect,
}

/// Where a boss fights: the floor height and walkable x range around its home.
#[derive(Clone, Copy)]
pub struct Arena {
    pub floor: f32,
    pub left: f32,
    pub right: f32,
    /// Where the boss starts (on the floor for walkers, in the air for flyers).
    pub home: Vec2,
}

impl Arena {
    /// The ground around the boss's marker in the sketch (or a stand-in near the spawn).
    pub fn find(kind: BossKind, terrain: &Terrain) -> Arena {
        let fallback = terrain.spawn + Vec2::new(600.0, 0.0);
        let home = match kind {
            BossKind::Roach => terrain.roach.unwrap_or(fallback),
            BossKind::Wasp => terrain.wasp.unwrap_or(fallback + Vec2::Y * 230.0),
            BossKind::Dragonfly => terrain.dragonfly.unwrap_or(fallback + Vec2::Y * 230.0),
            BossKind::Spider => terrain.spider.unwrap_or(fallback + Vec2::Y * 230.0),
            BossKind::Centipede => terrain.centipede.unwrap_or(fallback + Vec2::Y * 230.0),
            BossKind::Frog => terrain.frog.unwrap_or(fallback),
            BossKind::Grasshopper => terrain.grasshopper.unwrap_or(fallback),
            BossKind::Rat => terrain.rat.unwrap_or(fallback),
        };
        let feet = terrain.rest(home);
        // "Relatively flat": the ground may wander this much up or down.
        let (left, right) = terrain.flat_span(feet, 60.0);
        Arena { floor: feet.y, left, right, home }
    }
}

/// A boss's fixed stats: each boss module defines one as a `const`.
pub struct BossSpec {
    pub name: &'static str,
    pub hp: i32,
    /// Hurtbox radius around its center.
    pub radius: f32,
    pub blood: Fluid,
    /// The color the chameleon earns by killing it (also its health bar).
    pub color: Color,
    /// It fights the chameleon within this distance.
    pub aggro: f32,
    /// Shadow health its carcass gives when eaten (`feeding.rs`), in health ticks.
    pub food: f32,
}

#[derive(Component)]
pub struct Boss {
    pub spec: &'static BossSpec,
    pub hp: i32,
    pub home: Vec2,
    pub dead: bool,
    /// Woken by the chameleon entering its lair (asleep, it stays home).
    pub awake: bool,
    /// Waiting for its introduction (it must be on screen first): no health bar yet.
    pub staged: bool,
    /// Untouchable right now (set by its AI, e.g. mid-spin): blows glance off.
    pub guard: bool,
    /// Hits taken lately (it fades): enough of them and it stops flinching and hits back.
    pressure: f32,
    /// Time left in which hits don't stagger it.
    armor: f32,
    /// Last slash that hit it (one hit per slash).
    hit_by: u32,
    /// 1 = white flash after a hit, fades to 0.
    flash: f32,
    /// Its own materials (to flash) with their base colors.
    mats: Vec<(Handle<ColorMaterial>, Color)>,
}

impl Boss {
    /// A boss that introduces itself: it hides its health bar until it does.
    pub fn staged(mut self) -> Self {
        self.staged = true;
        self
    }

    pub fn new(spec: &'static BossSpec, home: Vec2, mats: Vec<(Handle<ColorMaterial>, Color)>) -> Self {
        Self { spec, hp: spec.hp, home, dead: false, awake: false, staged: false, guard: false, pressure: 0.0, armor: 0.0, hit_by: 0, flash: 0.0, mats }
    }

    /// Stops hunting and heals: it goes back home on its own, it is not replaced.
    pub fn stand_down(&mut self) {
        self.awake = false;
        self.hp = self.spec.hp;
        self.pressure = 0.0;
        self.armor = 0.0;
    }

    /// Second phase: below half health.
    pub fn enraged(&self) -> bool {
        self.hp * 2 <= self.spec.hp
    }
}

/// Hits in a row (they fade) that make a boss stop flinching and answer.
const POISE_HITS: f32 = 3.0;
/// Hits fade out at this rate (per second).
const POISE_FADE: f32 = 0.8;
/// After answering, it shrugs off hits (no stagger) for this long.
const ARMOR_TIME: f32 = 2.0;

/// Sent when a boss is slashed. `dir` points away from the chameleon.
#[derive(Message)]
pub struct BossHit {
    pub boss: Entity,
    pub dir: Vec2,
    pub killed: bool,
    /// The blow staggers it (else it keeps doing what it was doing).
    pub stagger: bool,
    /// Too many hits in a row: it answers with an attack or gets out of reach.
    pub retaliate: bool,
    /// Venom spit rather than claws.
    pub venom: bool,
}

/// Hits fade and the armor wears off.
fn tick_poise(time: Res<Time>, mut bosses: Query<&mut Boss>) {
    let dt = time.delta_secs();
    for mut b in &mut bosses {
        b.pressure = (b.pressure - POISE_FADE * dt).max(0.0);
        b.armor = (b.armor - dt).max(0.0);
    }
}

/// Anything a boss spawns outside its body (projectiles, gibs): cleared when swapping bosses.
#[derive(Component)]
pub struct BossStuff;

/// What the HUD needs to know about the fight.
#[derive(Resource)]
pub struct BossStatus {
    pub name: &'static str,
    pub color: Color,
    /// Health, 0..1.
    pub health: f32,
    /// The fight is on (show the health bar).
    pub engaged: bool,
}

impl Default for BossStatus {
    fn default() -> Self {
        Self { name: "", color: Color::WHITE, health: 1.0, engaged: false }
    }
}

/// A torn-off body part.
#[derive(Component)]
pub struct Gib {
    vel: Vec2,
    spin: f32,
    bleeds: Option<Fluid>,
    landed: bool,
    t: f32,
}

/// Detach a body part from its boss and send it flying. `bleeds`: it pumps out that fluid.
pub fn tear_off(commands: &mut Commands, part: Entity, gt: &GlobalTransform, vel: Vec2, spin: f32, bleeds: Option<Fluid>) {
    commands.entity(part).remove::<ChildOf>().insert((
        gt.compute_transform(),
        Gib { vel, spin, bleeds, landed: false, t: 0.0 },
        BossStuff,
    ));
}

/// Damage the chameleon, knock it back and make it bleed (a lot, if it dies).
pub fn hurt_player(
    p: &mut Player,
    ppos: Vec2,
    from: Vec2,
    damage: i32,
    bleed: &mut MessageWriter<Bleed>,
    feel: (&mut Shake, &mut HitStop),
) {
    let side = if ppos.x >= from.x { 1.0 } else { -1.0 };
    let (shake, stop) = feel;
    let red = |amount, spread, speed| Bleed {
        pos: ppos,
        dir: Vec2::new(side, 0.6),
        spread,
        speed,
        amount,
        fluid: Fluid::Red,
        sticky: false,
    };
    match p.hurt(damage, Vec2::new(side * 380.0, 320.0)) {
        Hurt::Ignored => {}
        Hurt::Hurt => {
            shake.add(0.35);
            stop.add(0.05);
            bleed.write(red(18 * damage as u32, 0.8, 380.0));
        }
        Hurt::Killed => {
            shake.add(0.7);
            stop.add(0.25);
            bleed.write(red(120, PI, 520.0));
        }
    }
}

/// Landing height if moving `from` → `to` crosses the top of the terrain or a
/// solid platform (`foot` = distance from the center down to the bottom).
pub fn floor<'a>(
    terrain: &Terrain,
    platforms: impl Iterator<Item = (&'a Platform, &'a Transform)>,
    from: Vec2,
    to: Vec2,
    foot: f32,
) -> Option<f32> {
    let ground = terrain.landing(from, to, foot);
    platforms
        .filter(|(pl, ptf)| {
            let c = ptf.translation.truncate();
            let top = pl.top_at(c, to.x);
            pl.solid && (to.x - c.x).abs() < pl.run() && to.y - foot <= top && from.y - foot >= top - 6.0 - pl.tilt.tan().abs() * 10.0
        })
        .map(|(pl, ptf)| pl.top_at(ptf.translation.truncate(), to.x))
        .chain(ground)
        .reduce(f32::max)
}

fn spawn_all(mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>, mut materials: ResMut<Assets<ColorMaterial>>, terrain: Res<Terrain>) {
    for kind in BossKind::ALL {
        kind.spawn(&mut commands, &mut meshes, &mut materials, &terrain);
    }
}

/// Bosses wake when the chameleon enters their lair, and reset when it leaves
/// the lair or runs too far.
fn lairs(
    player: Single<(&Player, &Transform)>,
    mut bosses: Query<(&mut Boss, &Lair, &Transform)>,
) {
    let (p, ptf) = *player;
    let ppos = ptf.translation.truncate();
    for (mut boss, lair, tf) in &mut bosses {
        if boss.dead || p.dead.is_some() {
            continue;
        }
        let inside = lair.region.contains(ppos);
        if !boss.awake && lair.arena.contains(ppos) {
            boss.awake = true;
            info!("{} wakes", boss.spec.name);
        } else if boss.awake && (!inside || (lair.kind != BossKind::Centipede && tf.translation.truncate().distance(ppos) > LEASH)) {
            info!("{} gives up and goes home", boss.spec.name);
            boss.stand_down();
        }
    }
}

/// F2: jump to the next boss's lair (it wakes) and heal up.
fn visit_next_boss(
    keys: Res<ButtonInput<KeyCode>>,
    terrain: Res<Terrain>,
    mut next: Local<usize>,
    player: Single<(&mut Player, &mut Transform)>,
    bosses: Query<(&Boss, &Lair)>,
) {
    if !keys.just_pressed(KeyCode::F2) {
        return;
    }
    let (mut p, mut tf) = player.into_inner();
    for _ in 0..BossKind::ALL.len() {
        let kind = BossKind::ALL[*next % BossKind::ALL.len()];
        *next += 1;
        let Some((boss, _)) = bosses.iter().find(|(_, l)| l.kind == kind) else { continue };
        // A little away from it, on the ground.
        let at = terrain.rest(boss.home - Vec2::X * 300.0) + Vec2::Y * player::HALF.y;
        tf.translation.x = at.x;
        tf.translation.y = at.y;
        p.vel = Vec2::ZERO;
        p.forget_fall(at);
        if p.dead.is_none() {
            p.hp = p.max_hp;
        }
        return;
    }
}

pub(crate) fn slash_bosses(
    mut shake: ResMut<Shake>,
    mut stop: ResMut<HitStop>,
    mut punch: ResMut<ZoomPunch>,
    mut flash: ResMut<ScreenFlash>,
    mut bleed: MessageWriter<Bleed>,
    mut paint: MessageWriter<Paint>,
    mut earned: MessageWriter<ColorEarned>,
    mut hits: MessageWriter<BossHit>,
    mut spat: MessageReader<SpitHit>,
    mut defeated: ResMut<Defeated>,
    mut beaten: MessageWriter<Beaten>,
    player: Single<(&Player, &Transform, &ChameleonAnim)>,
    mut bosses: Query<(Entity, &mut Boss, &Transform, &Lair)>,
) {
    let (p, ptf, anim) = *player;
    let ppos = ptf.translation.truncate();
    // (boss, direction of the blow, slashed rather than spat on)
    let mut struck: Vec<(Entity, Vec2, bool)> = spat.read().map(|h| (h.target, h.dir, false)).collect();
    if anim.striking() {
        // The slash wave's half-disc toward the mouse, or the spin's fan.
        for (e, boss, tf, _) in &bosses {
            let pos = tf.translation.truncate();
            if !boss.dead && boss.hit_by != anim.slash_id && anim.reaches(ppos, p.facing, pos, boss.spec.radius) {
                struck.push((e, anim.blow_dir(ppos, pos), true));
            }
        }
    }
    for (e, dir, slashed) in struck {
        let Ok((_, mut boss, tf, lair)) = bosses.get_mut(e) else { continue };
        if boss.dead {
            continue;
        }
        let pos = tf.translation.truncate();
        if slashed {
            boss.hit_by = anim.slash_id;
        }
        if boss.guard && !p.god {
            // Glances off: a flash, no harm.
            boss.flash = 0.6;
            shake.add(0.12);
            stop.add(0.03);
            continue;
        }
        // F3 god mode: any blow kills.
        boss.hp = if p.god { 0 } else { boss.hp - 1 };
        boss.flash = 1.0;
        let killed = boss.hp <= 0;
        boss.dead = killed;
        let fluid = boss.spec.blood;
        // A flurry doesn't keep it flinching: after a few hits it shrugs off
        // the next ones for a while and answers.
        let (stagger, retaliate) = if killed {
            (true, false)
        } else if boss.armor > 0.0 {
            (false, false)
        } else {
            boss.pressure += 1.0;
            if boss.pressure >= POISE_HITS {
                boss.pressure = 0.0;
                boss.armor = ARMOR_TIME;
                (false, true)
            } else {
                (true, false)
            }
        };
        hits.write(BossHit { boss: e, dir: (dir + Vec2::Y * 0.3).normalize(), killed, stagger, retaliate, venom: !slashed });

        // The gush: a fan of blood along the cut, and (slashed) a backsplash right onto the chameleon.
        let gush = Bleed { pos, dir: dir + Vec2::Y * 0.35, spread: 0.8, speed: 650.0, amount: if slashed { 60 } else { 30 }, fluid, sticky: true };
        bleed.write(gush);
        if slashed {
            bleed.write(Bleed { dir: ppos + Vec2::Y * 8.0 - pos, spread: 0.4, speed: 420.0, amount: 40, ..gush });
            paint.write(Paint { amount: 5, fluid });
        }

        if killed {
            defeated.add(lair.kind);
            beaten.write(Beaten(lair.kind));
            // Burst open and give up its color. Make it land: freeze, slow motion,
            // zoom punch and a flash of its color.
            bleed.write(Bleed { dir: Vec2::Y, spread: PI, speed: 800.0, amount: 240, ..gush });
            paint.write(Paint { amount: 10, fluid });
            earned.write(ColorEarned(boss.spec.color));
            shake.add(1.0);
            stop.add(0.4);
            stop.slow_motion(1.8);
            punch.add(1.0);
            flash.flash(boss.spec.color, 0.55);
        } else {
            shake.add(0.2);
            stop.add(0.07);
        }
    }
}

/// The chameleon came back: living bosses heal and walk home, asleep (they are not replaced).
fn reset_on_respawn(mut respawned: MessageReader<Respawned>, mut bosses: Query<&mut Boss>) {
    if respawned.read().count() == 0 {
        return;
    }
    for mut boss in &mut bosses {
        if !boss.dead {
            boss.stand_down();
        }
    }
}

fn flash_bosses(time: Res<Time>, mut materials: ResMut<Assets<ColorMaterial>>, mut bosses: Query<&mut Boss>) {
    for mut boss in &mut bosses {
        if boss.flash <= 0.0 {
            continue;
        }
        boss.flash = (boss.flash - time.delta_secs() * 5.0).max(0.0);
        for (handle, base) in &boss.mats {
            if let Some(mut mat) = materials.get_mut(handle) {
                mat.color = Oklaba::from(*base).mix(&Color::WHITE.into(), boss.flash).into();
            }
        }
    }
}

fn update_status(
    mut status: ResMut<BossStatus>,
    player: Single<(&Player, &Transform)>,
    bosses: Query<(&Boss, &Transform)>,
) {
    let (p, ptf) = *player;
    status.engaged = false;
    // The nearest awake boss (or one that just died, so the bar empties).
    let fighting = bosses
        .iter()
        .filter(|(b, tf)| b.awake && (!b.dead || tf.translation.truncate().distance(ptf.translation.truncate()) < b.spec.aggro))
        .min_by(|a, b| {
            let d = |tf: &Transform| tf.translation.truncate().distance(ptf.translation.truncate());
            d(a.1).total_cmp(&d(b.1))
        });
    if let Some((boss, _)) = fighting {
        status.name = boss.spec.name;
        status.color = boss.spec.color;
        status.health = boss.hp.max(0) as f32 / boss.spec.hp as f32;
        status.engaged = p.dead.is_none() && !boss.dead && !boss.staged;
    }
}

/// The sketch moved a boss: clear the fight and start it again at the new spot.
fn restart_on_reload(
    mut commands: Commands,
    mut reloaded: MessageReader<WorldReloaded>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    terrain: Res<Terrain>,
    stuff: Query<Entity, Or<(With<Boss>, With<BossStuff>)>>,
) {
    if !reloaded.read().any(|r| r.bosses_moved) {
        return;
    }
    for e in &stuff {
        commands.entity(e).despawn();
    }
    for kind in BossKind::ALL {
        kind.spawn(&mut commands, &mut meshes, &mut materials, &terrain);
    }
}

fn move_gibs(
    mut commands: Commands,
    time: Res<Time>,
    mut bleed: MessageWriter<Bleed>,
    mut gibs: Query<(Entity, &mut Gib, &mut Transform)>,
    platforms: Query<(&Platform, &Transform), Without<Gib>>,
    terrain: Res<Terrain>,
) {
    let dt = time.delta_secs().min(1.0 / 30.0);
    for (e, mut g, mut tf) in &mut gibs {
        g.t += dt;
        let pos = tf.translation.truncate();
        if !g.landed {
            g.vel.y -= 1400.0 * dt;
            let next = pos + g.vel * dt;
            if let Some(top) = floor(&terrain, platforms.iter(), pos, next, 4.0) {
                tf.translation.y = top + 4.0;
                g.landed = true;
            } else {
                tf.translation.x = next.x;
                tf.translation.y = next.y;
                tf.rotate_z(g.spin * dt);
            }
        }
        // A torn end pumps out blood for a while.
        if let Some(fluid) = g.bleeds {
            if g.t < 3.0 && (g.t * 60.0) as u32 % 3 == 0 {
                bleed.write(Bleed {
                    pos: tf.translation.truncate(),
                    dir: Vec2::Y,
                    spread: 1.2,
                    speed: if g.landed { 160.0 } else { 90.0 },
                    amount: 1,
                    fluid,
                    sticky: true,
                });
            }
        }
        if tf.translation.y < terrain.bounds.min.y - 600.0 {
            commands.entity(e).despawn();
        }
    }
}
