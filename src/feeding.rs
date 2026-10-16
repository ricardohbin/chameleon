//! Feeding on the remains of the battle. `E` with a corpse in reach bites
//! into it: the corpse's food (`MobSpec::food`, `BossSpec::food`) becomes
//! shadow health (`Player::shadow`), which slowly fills into real health ticks.
//! Mob corpses are devoured whole; a boss carcass stays, picked clean, and
//! first grows the chameleon's max health by one tick (then fills it as shadow).
//! Until the chameleon has eaten once, a tip floats over the nearest remains.

use std::f32::consts::PI;

use bevy::prelude::*;

use crate::{
    boss::Boss,
    save::AutoSave,
    camera::{HitStop, MainCamera, Shake},
    chameleon::{self, ChameleonAnim},
    gore::{Bleed, Fluid, Paint},
    mobs::{Mob, State},
    player::{self, Player},
    shapes::Rng,
    sound::Sfx,
};

/// Remains count as in reach a bit further than a hit (they lie on the ground).
const REACH_SLACK: f32 = 10.0;
/// Seconds a devoured corpse takes to vanish.
const DEVOUR_TIME: f32 = 0.3;
/// The eating tip shows over remains this close to the chameleon (world units).
const TIP_RANGE: f32 = 260.0;
/// The tip floats this far below the remains (world units).
const TIP_DROP: f32 = 34.0;

pub struct FeedingPlugin;

impl Plugin for FeedingPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Learned>();
        app.add_systems(Startup, spawn_tip);
        app.add_systems(Update, leak_carcasses);
        app.add_systems(Update, (feed, devour, show_tip).chain().after(chameleon::update_anim_state).after(player::move_player));
    }
}

/// A mob corpse being eaten: shrinks away, then is gone.
#[derive(Component)]
struct Devoured(f32);

/// A bite of food lying around (a leg knocked off a boss).
#[derive(Component)]
pub struct Morsel {
    pub food: f32,
    pub fluid: Fluid,
}

/// How big a morsel counts as, for reaching it.
const MORSEL_R: f32 = 40.0;

/// A boss carcass that was already eaten.
#[derive(Component)]
struct PickedClean;

/// The chameleon has eaten once this session: no more tip.
#[derive(Resource, Default)]
struct Learned(bool);

/// "E: eat" over the nearest remains, until the first meal.
#[derive(Component)]
struct Tip;

/// Something lying around that can be eaten.
struct Meal {
    entity: Entity,
    pos: Vec2,
    food: f32,
    fluid: Fluid,
    boss: bool,
}

/// The remains the chameleon could eat right now (boss carcasses always,
/// mob corpses only when hungry), nearest to `from` first.
fn meals(
    p: &Player,
    from: Vec2,
    mobs: &Query<(Entity, &Mob, &Transform), Without<Devoured>>,
    bosses: &Query<(Entity, &Boss, &Transform, Has<PickedClean>)>,
    morsels: &Query<(Entity, &Morsel, &Transform), Without<Devoured>>,
) -> Vec<(Meal, f32)> {
    let mut found = Vec::new();
    for (entity, m, tf) in morsels {
        if p.hunger() > 0.0 {
            found.push((Meal { entity, pos: tf.translation.truncate(), food: m.food, fluid: m.fluid, boss: false }, MORSEL_R));
        }
    }
    for (entity, m, tf) in mobs {
        if matches!(m.state, State::Dead { .. }) && m.food() > 0.0 && p.hunger() > 0.0 {
            let pos = tf.translation.truncate();
            found.push((Meal { entity, pos, food: m.food(), fluid: m.spec.blood, boss: false }, m.spec.radius));
        }
    }
    for (entity, boss, tf, picked) in bosses {
        if boss.dead && !picked {
            let pos = tf.translation.truncate();
            found.push((Meal { entity, pos, food: boss.spec.food, fluid: boss.spec.blood, boss: true }, boss.spec.radius));
        }
    }
    found.sort_by(|a, b| a.0.pos.distance(from).total_cmp(&b.0.pos.distance(from)));
    found
}

fn feed(
    mut commands: Commands,
    mut shake: ResMut<Shake>,
    mut stop: ResMut<HitStop>,
    mut learned: ResMut<Learned>,
    mut bleed: MessageWriter<Bleed>,
    mut paint: MessageWriter<Paint>,
    mut sfx: MessageWriter<Sfx>,
    mut autosave: MessageWriter<AutoSave>,
    player: Single<(&mut Player, &Transform, &mut ChameleonAnim)>,
    mobs: Query<(Entity, &Mob, &Transform), Without<Devoured>>,
    bosses: Query<(Entity, &Boss, &Transform, Has<PickedClean>)>,
    morsels: Query<(Entity, &Morsel, &Transform), Without<Devoured>>,
) {
    let (mut p, ptf, mut anim) = player.into_inner();
    if !anim.wants_to_eat() {
        return;
    }
    let ppos = ptf.translation.truncate();
    // Within slash reach of the shoulder, on either side: the head finds it.
    let center = chameleon::slash_center(ppos, p.facing);
    let Some((meal, _)) = meals(&p, center, &mobs, &bosses, &morsels)
        .into_iter()
        .find(|(m, radius)| m.pos.distance(center) < chameleon::SLASH_REACH + radius + REACH_SLACK)
    else {
        return;
    };

    learned.0 = true;
    anim.bite();
    sfx.write(Sfx::Feed);
    if meal.boss {
        p.max_hp += 1;
        commands.entity(meal.entity).insert(PickedClean);
        autosave.write(AutoSave);
    }
    p.feed(meal.food);
    if !meal.boss {
        commands.entity(meal.entity).insert(Devoured(0.0));
    }

    // Tear in: the corpse bursts toward the chameleon's face and paints it.
    let big = (meal.food / 0.5).clamp(1.0, 4.0);
    let face = ppos + Vec2::new(p.facing * 14.0, 4.0);
    let gush = Bleed {
        pos: meal.pos,
        dir: face - meal.pos,
        spread: 0.7,
        speed: 300.0,
        amount: (24.0 * big) as u32,
        fluid: meal.fluid,
        sticky: true,
    };
    bleed.write(gush);
    bleed.write(Bleed { dir: Vec2::Y, spread: PI * 0.5, speed: 260.0, amount: (16.0 * big) as u32, ..gush });
    paint.write(Paint { amount: (2.0 * big) as u32, fluid: meal.fluid });
    shake.add(0.12 * big);
    stop.add(0.04);
}

/// A boss carcass nobody has eaten yet keeps spurting its blood now and then.
fn leak_carcasses(
    time: Res<Time>,
    mut rng: Local<Option<Rng>>,
    mut bleed: MessageWriter<Bleed>,
    bosses: Query<(&Boss, &Transform), Without<PickedClean>>,
) {
    let dt = time.delta_secs();
    let rng = rng.get_or_insert(Rng(0x9E37_79B9));
    for (boss, tf) in &bosses {
        if boss.dead && rng.chance(dt * 0.6) {
            bleed.write(Bleed {
                pos: tf.translation.truncate(),
                dir: Vec2::Y,
                spread: 1.0,
                speed: 220.0,
                amount: 6,
                fluid: boss.spec.blood,
                sticky: true,
            });
        }
    }
}

fn devour(mut commands: Commands, time: Res<Time>, mut corpses: Query<(Entity, &mut Devoured, &mut Transform)>) {
    for (e, mut d, mut tf) in &mut corpses {
        d.0 += time.delta_secs();
        let k = (1.0 - d.0 / DEVOUR_TIME).max(0.0);
        // Squashed flat while it's torn apart.
        tf.scale = Vec3::new(k.sqrt(), k * k, 1.0);
        if d.0 >= DEVOUR_TIME {
            commands.entity(e).despawn();
        }
    }
}

fn spawn_tip(mut commands: Commands) {
    commands.spawn((
        Tip,
        Text::new("E  Feed"),
        TextFont { font_size: FontSize::Px(20.0), ..default() },
        TextColor(Color::NONE),
        BackgroundColor(Color::NONE),
        TextLayout::justify(Justify::Center),
        Node { position_type: PositionType::Absolute, padding: crate::hud::chip_pad(), ..default() },
        Visibility::Hidden,
    ));
}

/// Until the first meal: the tip floats under the nearest remains in range,
/// fading in, brighter once they're in reach.
fn show_tip(
    time: Res<Time<Real>>,
    learned: Res<Learned>,
    camera: Single<(&Camera, &GlobalTransform), With<MainCamera>>,
    player: Single<(&Player, &Transform)>,
    mobs: Query<(Entity, &Mob, &Transform), Without<Devoured>>,
    bosses: Query<(Entity, &Boss, &Transform, Has<PickedClean>)>,
    morsels: Query<(Entity, &Morsel, &Transform), Without<Devoured>>,
    tip: Single<(&mut Node, &mut TextColor, &mut BackgroundColor, &mut Visibility, &ComputedNode), With<Tip>>,
) {
    let (mut node, mut color, mut bg, mut vis, computed) = tip.into_inner();
    let (p, ptf) = player.into_inner();
    let ppos = ptf.translation.truncate();
    let center = chameleon::slash_center(ppos, p.facing);
    let target = (!learned.0 && p.dead.is_none())
        .then(|| meals(p, ppos, &mobs, &bosses, &morsels).into_iter().next())
        .flatten()
        .filter(|(m, _)| m.pos.distance(ppos) < TIP_RANGE);
    let at = target.as_ref().and_then(|(m, radius)| {
        let reach = m.pos.distance(center) < chameleon::SLASH_REACH + radius + REACH_SLACK;
        let below = m.pos - Vec2::Y * (radius + TIP_DROP);
        camera.0.world_to_viewport(camera.1, below.extend(0.0)).ok().map(|v| (v, reach))
    });

    let alpha = color.0.alpha();
    let goal = match at {
        Some((_, true)) => 1.0,
        Some((_, false)) => 0.6,
        None => 0.0,
    };
    let alpha = alpha + (goal - alpha) * (1.0 - (-8.0 * time.delta_secs()).exp());
    if let Some((v, _)) = at {
        let size = computed.size() * computed.inverse_scale_factor();
        node.left = px(v.x - size.x * 0.5);
        node.top = px(v.y - size.y * 0.5);
    }
    color.0 = crate::hud::CHIP_INK.with_alpha(alpha);
    bg.0 = crate::hud::CHIP.with_alpha(crate::hud::CHIP.alpha() * alpha);
    *vis = if alpha > 0.01 { Visibility::Inherited } else { Visibility::Hidden };
}
