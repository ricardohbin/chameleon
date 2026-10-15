//! Feeding on the remains of the battle. Attacking with no living enemy in
//! the slash's reach but a corpse there turns the slash into a bite: the
//! corpse's food (`MobSpec::food`, `BossSpec::food`) becomes shadow health
//! (`Player::shadow`), which slowly fills into real health ticks.
//! Mob corpses are devoured whole; a boss carcass stays, picked clean, and
//! first grows the chameleon's max health by one tick (then fills it as shadow).

use std::f32::consts::PI;

use bevy::prelude::*;

use crate::{
    boss::Boss,
    camera::{HitStop, Shake},
    chameleon::{self, ChameleonAnim},
    gore::{Bleed, Fluid, Paint},
    mobs::{Mob, State},
    player::{self, Player},
};

/// Remains count as in reach a bit further than a hit (they lie on the ground).
const REACH_SLACK: f32 = 10.0;
/// Seconds a devoured corpse takes to vanish.
const DEVOUR_TIME: f32 = 0.3;

pub struct FeedingPlugin;

impl Plugin for FeedingPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, (feed, devour).chain().after(chameleon::update_anim_state).after(player::move_player));
    }
}

/// A mob corpse being eaten: shrinks away, then is gone.
#[derive(Component)]
struct Devoured(f32);

/// A boss carcass that was already eaten.
#[derive(Component)]
struct PickedClean;

/// Something lying in reach that can be eaten.
struct Meal {
    entity: Entity,
    pos: Vec2,
    food: f32,
    fluid: Fluid,
    boss: bool,
}

fn feed(
    mut commands: Commands,
    mut shake: ResMut<Shake>,
    mut stop: ResMut<HitStop>,
    mut bleed: MessageWriter<Bleed>,
    mut paint: MessageWriter<Paint>,
    player: Single<(&mut Player, &Transform, &mut ChameleonAnim)>,
    mobs: Query<(Entity, &Mob, &Transform), Without<Devoured>>,
    bosses: Query<(Entity, &Boss, &Transform, Has<PickedClean>)>,
) {
    let (mut p, ptf, mut anim) = player.into_inner();
    if !anim.attacked() {
        return;
    }
    let ppos = ptf.translation.truncate();
    // The same half-disc as the slash hitbox.
    let dir = anim.slash_dir();
    let center = chameleon::slash_center(ppos, p.facing);
    let in_reach = |pos: Vec2, radius: f32, slack: f32| {
        (pos - center).dot(dir) > -radius * 0.5 && pos.distance(center) < chameleon::SLASH_REACH + radius + slack
    };

    let mut meal: Option<Meal> = None;
    let mut consider = |m: Meal| {
        if meal.as_ref().is_none_or(|best| m.pos.distance(center) < best.pos.distance(center)) {
            meal = Some(m);
        }
    };
    for (entity, m, tf) in &mobs {
        let pos = tf.translation.truncate();
        let dead = matches!(m.state, State::Dead { .. });
        if !dead && in_reach(pos, m.spec.radius, 0.0) {
            return; // Something alive to slash first.
        }
        if dead && m.food() > 0.0 && in_reach(pos, m.spec.radius, REACH_SLACK) {
            consider(Meal { entity, pos, food: m.food(), fluid: m.spec.blood, boss: false });
        }
    }
    for (entity, boss, tf, picked) in &bosses {
        let pos = tf.translation.truncate();
        if !boss.dead && in_reach(pos, boss.spec.radius, 0.0) {
            return;
        }
        if boss.dead && !picked && in_reach(pos, boss.spec.radius, REACH_SLACK) {
            consider(Meal { entity, pos, food: boss.spec.food, fluid: boss.spec.blood, boss: true });
        }
    }
    // Nothing to gain from a mob when full: slash instead (the corpse keeps).
    let Some(meal) = meal.filter(|m| m.boss || p.hunger() > 0.0) else { return };

    anim.bite();
    if meal.boss {
        p.max_hp += 1;
        commands.entity(meal.entity).insert(PickedClean);
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
