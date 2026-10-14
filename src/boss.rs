//! Shared boss machinery. Every boss fights in the level's arena; this module
//! does what they all have in common: picking which boss to fight, health,
//! getting slashed, the death spectacle, torn-off gibs, healing when the
//! chameleon dies, and the HUD status. Each boss module (`wasp`, `roach`)
//! only builds its body and runs its own AI and corpse.
//!
//! Pick the boss with `cargo run -- <id>` (or `BOSS=<id> cargo run`), ids in
//! `BossKind::id`. In-game, F2 swaps to the next boss (the arena resets).
//!
//! Adding a boss: a module with a `spawn` fn (spawns a `Boss` + its own AI
//! component) and its systems, then a `BossKind` entry.

use std::f32::consts::PI;

use bevy::prelude::*;

use crate::{
    camera::{HitStop, Shake, ZoomPunch},
    chameleon::{self, ChameleonAnim, ColorEarned},
    gore::{Bleed, Fluid, Paint},
    hud::ScreenFlash,
    level::{self, Platform},
    player::{self, Hurt, Player, Respawned},
    roach, wasp,
};

pub struct BossPlugin;

impl Plugin for BossPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(SelectedBoss(BossKind::from_env()))
            .init_resource::<BossStatus>()
            .add_message::<BossHit>()
            // PostStartup: the level's platforms exist by then.
            .add_systems(PostStartup, find_arena)
            .add_systems(
                Update,
                (
                    switch_boss,
                    spawn_selected.run_if(resource_changed::<SelectedBoss>),
                    slash_bosses,
                    reset_bosses,
                    flash_bosses,
                    update_status,
                    move_gibs,
                )
                    .chain()
                    .after(player::move_player)
                    .after(chameleon::update_anim_state),
            );
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum BossKind {
    Wasp,
    Roach,
}

impl BossKind {
    const ALL: [BossKind; 2] = [BossKind::Wasp, BossKind::Roach];

    /// Name used on the command line.
    fn id(self) -> &'static str {
        match self {
            BossKind::Wasp => "wasp",
            BossKind::Roach => "roach",
        }
    }

    /// From the first CLI argument or `BOSS`; defaults to the newest boss.
    fn from_env() -> Self {
        let wanted = std::env::args().nth(1).or_else(|| std::env::var("BOSS").ok());
        wanted
            .and_then(|w| Self::ALL.into_iter().find(|k| k.id() == w.to_lowercase()))
            .unwrap_or(BossKind::Roach)
    }

    fn next(self) -> Self {
        let i = Self::ALL.iter().position(|&k| k == self).unwrap_or(0);
        Self::ALL[(i + 1) % Self::ALL.len()]
    }

    fn spawn(
        self,
        commands: &mut Commands,
        meshes: &mut Assets<Mesh>,
        materials: &mut Assets<ColorMaterial>,
        arena: &Arena,
    ) {
        match self {
            BossKind::Wasp => wasp::spawn(commands, meshes, materials, arena),
            BossKind::Roach => roach::spawn(commands, meshes, materials, arena),
        }
    }
}

#[derive(Resource)]
pub struct SelectedBoss(pub BossKind);

/// The arena's floor height and walkable x range.
#[derive(Resource, Clone, Copy)]
pub struct Arena {
    pub floor: f32,
    pub left: f32,
    pub right: f32,
}

impl Arena {
    pub fn center(&self) -> f32 {
        (self.left + self.right) / 2.0
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
}

#[derive(Component)]
pub struct Boss {
    pub spec: &'static BossSpec,
    pub hp: i32,
    pub home: Vec2,
    pub dead: bool,
    /// Last slash that hit it (one hit per slash).
    hit_by: u32,
    /// 1 = white flash after a hit, fades to 0.
    flash: f32,
    /// Its own materials (to flash) with their base colors.
    mats: Vec<(Handle<ColorMaterial>, Color)>,
}

impl Boss {
    pub fn new(spec: &'static BossSpec, home: Vec2, mats: Vec<(Handle<ColorMaterial>, Color)>) -> Self {
        Self { spec, hp: spec.hp, home, dead: false, hit_by: 0, flash: 0.0, mats }
    }

    /// Second phase: below half health.
    pub fn enraged(&self) -> bool {
        self.hp * 2 <= self.spec.hp
    }
}

/// Sent when a boss is slashed. `dir` points away from the chameleon.
#[derive(Message)]
pub struct BossHit {
    pub boss: Entity,
    pub dir: Vec2,
    pub killed: bool,
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

/// Landing height if moving `from` → `to` crosses the top of a solid platform
/// (`foot` = distance from the center down to the bottom).
pub fn floor<'a>(
    platforms: impl Iterator<Item = (&'a Platform, &'a Transform)>,
    from: Vec2,
    to: Vec2,
    foot: f32,
) -> Option<f32> {
    platforms
        .filter(|(pl, ptf)| {
            let c = ptf.translation.truncate();
            let top = c.y + pl.half.y;
            pl.solid && (to.x - c.x).abs() < pl.half.x && to.y - foot <= top && from.y - foot >= top - 6.0
        })
        .map(|(pl, ptf)| ptf.translation.y + pl.half.y)
        .reduce(f32::max)
}

fn find_arena(mut commands: Commands, platforms: Query<(&Platform, &Transform)>) {
    let ground = platforms
        .iter()
        .filter(|(p, tf)| !p.one_way && (tf.translation.x - level::ARENA_X).abs() < p.half.x)
        .max_by(|a, b| (a.1.translation.y + a.0.half.y).total_cmp(&(b.1.translation.y + b.0.half.y)));
    let arena = match ground {
        Some((p, tf)) => Arena {
            floor: tf.translation.y + p.half.y,
            left: tf.translation.x - p.half.x,
            right: tf.translation.x + p.half.x,
        },
        None => Arena {
            floor: 0.0,
            left: level::ARENA_X - level::ARENA_HALF_WIDTH,
            right: level::ARENA_X + level::ARENA_HALF_WIDTH,
        },
    };
    commands.insert_resource(arena);
}

fn spawn_selected(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    selected: Res<SelectedBoss>,
    arena: Res<Arena>,
) {
    selected.0.spawn(&mut commands, &mut meshes, &mut materials, &arena);
}

/// F2: clear the arena and bring in the next boss (and heal the chameleon).
fn switch_boss(
    mut commands: Commands,
    keys: Res<ButtonInput<KeyCode>>,
    mut selected: ResMut<SelectedBoss>,
    mut player: Single<&mut Player>,
    stuff: Query<Entity, Or<(With<Boss>, With<BossStuff>)>>,
) {
    if !keys.just_pressed(KeyCode::F2) {
        return;
    }
    for e in &stuff {
        commands.entity(e).despawn();
    }
    selected.0 = selected.0.next();
    if player.dead.is_none() {
        player.hp = player::MAX_HP;
    }
}

fn slash_bosses(
    mut shake: ResMut<Shake>,
    mut stop: ResMut<HitStop>,
    mut punch: ResMut<ZoomPunch>,
    mut flash: ResMut<ScreenFlash>,
    mut bleed: MessageWriter<Bleed>,
    mut paint: MessageWriter<Paint>,
    mut earned: MessageWriter<ColorEarned>,
    mut hits: MessageWriter<BossHit>,
    player: Single<(&Player, &Transform, &ChameleonAnim)>,
    mut bosses: Query<(Entity, &mut Boss, &Transform)>,
) {
    let (p, ptf, anim) = *player;
    if !anim.striking() {
        return;
    }
    let ppos = ptf.translation.truncate();
    // The hitbox is the slash wave: a half-disc in front of the chameleon.
    let center = ppos + Vec2::new(chameleon::SLASH_CENTER.x * p.facing, chameleon::SLASH_CENTER.y);
    for (e, mut boss, tf) in &mut bosses {
        let pos = tf.translation.truncate();
        let radius = boss.spec.radius;
        let in_front = (pos.x - center.x) * p.facing > -radius * 0.5;
        let in_reach = pos.distance(center) < chameleon::SLASH_REACH + radius;
        if boss.dead || boss.hit_by == anim.slash_id || !in_front || !in_reach {
            continue;
        }
        boss.hit_by = anim.slash_id;
        boss.hp -= 1;
        boss.flash = 1.0;
        let killed = boss.hp <= 0;
        boss.dead = killed;
        let fluid = boss.spec.blood;
        hits.write(BossHit { boss: e, dir: Vec2::new(p.facing, 0.3).normalize(), killed });

        // The gush: a fan of blood along the cut, and a backsplash right onto the chameleon.
        let gush = Bleed { pos, dir: Vec2::new(p.facing, 0.35), spread: 0.8, speed: 650.0, amount: 60, fluid, sticky: true };
        bleed.write(gush);
        bleed.write(Bleed { dir: ppos + Vec2::Y * 8.0 - pos, spread: 0.4, speed: 420.0, amount: 40, ..gush });
        paint.write(Paint { amount: 5, fluid });

        if killed {
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

/// The chameleon came back: living bosses heal and go home (their AI resets itself).
fn reset_bosses(mut respawned: MessageReader<Respawned>, mut bosses: Query<(&mut Boss, &mut Transform)>) {
    if respawned.read().count() == 0 {
        return;
    }
    for (mut boss, mut tf) in &mut bosses {
        if boss.dead {
            continue;
        }
        boss.hp = boss.spec.hp;
        boss.flash = 0.01; // restores its colors
        tf.translation = boss.home.extend(tf.translation.z);
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
    for (boss, tf) in &bosses {
        let near = tf.translation.truncate().distance(ptf.translation.truncate()) < boss.spec.aggro * 1.4;
        status.name = boss.spec.name;
        status.color = boss.spec.color;
        status.health = boss.hp.max(0) as f32 / boss.spec.hp as f32;
        status.engaged = p.dead.is_none() && !boss.dead && near;
    }
}

fn move_gibs(
    mut commands: Commands,
    time: Res<Time>,
    mut bleed: MessageWriter<Bleed>,
    mut gibs: Query<(Entity, &mut Gib, &mut Transform)>,
    platforms: Query<(&Platform, &Transform), Without<Gib>>,
) {
    let dt = time.delta_secs().min(1.0 / 30.0);
    for (e, mut g, mut tf) in &mut gibs {
        g.t += dt;
        let pos = tf.translation.truncate();
        if !g.landed {
            g.vel.y -= 1400.0 * dt;
            let next = pos + g.vel * dt;
            if let Some(top) = floor(platforms.iter(), pos, next, 4.0) {
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
        if tf.translation.y < -900.0 {
            commands.entity(e).despawn();
        }
    }
}
