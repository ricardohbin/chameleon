//! Sound effects, recorded by the user (`assets/audio/*.m4a`, AAC), embedded in
//! the binary like the world sketch. Everything is one-shot and non-spatial:
//! the claw slash, blood when an enemy is hit (two takes, alternated at random),
//! a death cry when a boss falls, each boss's calls (now and then while it fights),
//! and the air-spin whoosh (the Razorwing's, and the chameleon's once earned).

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use bevy::audio::Volume;
use bevy::prelude::*;

use crate::{
    boss::{Boss, BossHit, BossKind, Lair},
    chameleon::ChameleonAnim,
    dragonfly,
    shapes::Rng,
};

pub struct SoundPlugin;

impl Plugin for SoundPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<Sfx>()
            .add_systems(Startup, load)
            .add_systems(Update, (slash, hits, voices, whoosh, play).chain());
    }
}

/// Something to play (also sent by other modules, e.g. a mob slashed).
#[derive(Message, Clone, Copy)]
pub enum Sfx {
    Slash,
    Blood,
    BossDeath,
    Whoosh,
}

#[derive(Resource)]
struct Clips {
    slash: Handle<AudioSource>,
    blood: [Handle<AudioSource>; 2],
    death: Handle<AudioSource>,
    gutter: Handle<AudioSource>,
    wasp: Handle<AudioSource>,
    dragonfly: Handle<AudioSource>,
    spider: Handle<AudioSource>,
    airblow: Handle<AudioSource>,
}

fn load(mut commands: Commands, mut sources: ResMut<Assets<AudioSource>>) {
    let mut add = |bytes: &'static [u8]| sources.add(AudioSource { bytes: Arc::from(bytes) });
    commands.insert_resource(Clips {
        slash: add(include_bytes!("../assets/audio/slash.m4a")),
        blood: [add(include_bytes!("../assets/audio/blood1.m4a")), add(include_bytes!("../assets/audio/blood2.m4a"))],
        death: add(include_bytes!("../assets/audio/death.m4a")),
        gutter: add(include_bytes!("../assets/audio/gutter.m4a")),
        wasp: add(include_bytes!("../assets/audio/wasp.m4a")),
        dragonfly: add(include_bytes!("../assets/audio/dragonfly.m4a")),
        spider: add(include_bytes!("../assets/audio/spider.m4a")),
        airblow: add(include_bytes!("../assets/audio/airblow.m4a")),
    });
}

/// Every new slash of the claw; the air spin gets the whoosh instead.
fn slash(anim: Single<&ChameleonAnim>, mut last: Local<u32>, mut sfx: MessageWriter<Sfx>) {
    let id = anim.slash_id;
    if id != *last && id != 0 {
        sfx.write(if anim.spinning() { Sfx::Whoosh } else { Sfx::Slash });
    }
    *last = id;
}

/// Bosses hit bleed; the killing blow also gets the death cry.
fn hits(mut hits: MessageReader<BossHit>, mut sfx: MessageWriter<Sfx>) {
    for hit in hits.read() {
        sfx.write(Sfx::Blood);
        if hit.killed {
            sfx.write(Sfx::BossDeath);
        }
    }
}

/// Seconds of silence between a boss's calls (random in this range), counted
/// from the end of the last one.
const VOICE_GAP: (f32, f32) = (0.3, 1.5);

/// A boss's call that is playing (cut short if it dies or resets).
#[derive(Component)]
struct Voice(Entity);

/// Bosses call out now and then while they fight: first when they introduce
/// themselves (awake, health bar shown), then again at random moments, never
/// over their own last call. Dead or gone (reset, swapped) = silenced.
fn voices(
    mut commands: Commands,
    time: Res<Time>,
    clips: Res<Clips>,
    bosses: Query<(Entity, &Boss, &Lair)>,
    playing: Query<(Entity, &Voice)>,
    mut wait: Local<HashMap<Entity, f32>>,
    mut rng: Local<Option<Rng>>,
) {
    let rng = rng.get_or_insert_with(|| Rng(0x0b05_5ca1));
    for (v, Voice(owner)) in &playing {
        if bosses.get(*owner).ok().is_none_or(|(_, b, _)| b.dead) {
            commands.entity(v).despawn();
        }
    }
    wait.retain(|&e, _| bosses.get(e).is_ok_and(|(_, b, _)| b.awake && !b.staged && !b.dead));
    for (e, boss, lair) in &bosses {
        if !boss.awake || boss.staged || boss.dead || playing.iter().any(|(_, v)| v.0 == e) {
            continue;
        }
        // Just introduced: at once. Otherwise after a pause.
        let left = wait.entry(e).or_insert(0.0);
        *left -= time.delta_secs();
        if *left > 0.0 {
            continue;
        }
        *left = rng.range(VOICE_GAP.0, VOICE_GAP.1);
        let clip = match lair.kind {
            BossKind::Roach => &clips.gutter,
            BossKind::Wasp => &clips.wasp,
            BossKind::Dragonfly => &clips.dragonfly,
            BossKind::Spider => &clips.spider,
        };
        commands.spawn((
            Voice(e),
            AudioPlayer::new(clip.clone()),
            PlaybackSettings::DESPAWN.with_volume(Volume::Linear(0.9)).with_speed(1.0 + rng.range(-0.05, 0.05)),
        ));
    }
}

/// The Razorwing starting a spin.
fn whoosh(flies: Query<(Entity, &dragonfly::Dragonfly)>, mut spinning: Local<HashSet<Entity>>, mut sfx: MessageWriter<Sfx>) {
    for (e, d) in &flies {
        if d.spinning() {
            if spinning.insert(e) {
                sfx.write(Sfx::Whoosh);
            }
        } else {
            spinning.remove(&e);
        }
    }
}

fn play(mut commands: Commands, clips: Res<Clips>, mut sfx: MessageReader<Sfx>, mut rng: Local<Option<Rng>>) {
    let rng = rng.get_or_insert_with(|| Rng(0x5eed_a0d1));
    for &s in sfx.read() {
        // (clip, volume, pitch jitter: repeated sounds shouldn't all be the same)
        let (clip, volume, jitter) = match s {
            Sfx::Slash => (&clips.slash, 0.7, 0.08),
            Sfx::Blood => (&clips.blood[rng.below(2)], 0.8, 0.1),
            Sfx::BossDeath => (&clips.death, 1.0, 0.0),
            Sfx::Whoosh => (&clips.airblow, 0.8, 0.06),
        };
        commands.spawn((
            AudioPlayer::new(clip.clone()),
            PlaybackSettings::DESPAWN.with_volume(Volume::Linear(volume)).with_speed(1.0 + rng.range(-jitter, jitter)),
        ));
    }
}

#[cfg(test)]
mod tests {
    use bevy::audio::Decodable;

    use super::*;

    /// Every embedded clip decodes (an unsupported format would panic in game).
    #[test]
    fn clips_decode() {
        let clips: [&'static [u8]; 9] = [
            include_bytes!("../assets/audio/slash.m4a"),
            include_bytes!("../assets/audio/blood1.m4a"),
            include_bytes!("../assets/audio/blood2.m4a"),
            include_bytes!("../assets/audio/death.m4a"),
            include_bytes!("../assets/audio/gutter.m4a"),
            include_bytes!("../assets/audio/wasp.m4a"),
            include_bytes!("../assets/audio/dragonfly.m4a"),
            include_bytes!("../assets/audio/spider.m4a"),
            include_bytes!("../assets/audio/airblow.m4a"),
        ];
        for bytes in clips {
            let samples = AudioSource { bytes: Arc::from(bytes) }.decoder().count();
            assert!(samples > 1000);
        }
    }
}
