//! Quick save (F5) / quick load (F9), and the auto save.
//!
//! Two slots, small text files next to the executable with the same name and
//! a `.save` (quick) or `.autosave` extension (`ghore.exe` → `ghore.save`; a
//! `target/*/deps/` test build shares the files of the normal one; the web
//! build keeps them in `localStorage`, see `storage.rs`). The auto
//! save is written when a boss fight starts, when the boss dies and when its
//! carcass is eaten (`AutoSave`). When the chameleon dies, the most recent of
//! the two is loaded by itself. A save holds the progress that
//! matters: where the chameleon is and how it is (health, max health, shadow
//! health, facing, speed) and which bosses are dead (their colors and the
//! skin come from that, and the powers they gave). Everything else resets the way it does when the
//! chameleon dies: living bosses asleep at home, mobs back, no blood. Loading
//! is `Restart` (F8) + putting the saved state back a frame later.

use bevy::prelude::*;

use crate::{
    boss::{Boss, BossKind, Celebration, Defeated, Lair},
    camera::{Follow, MainCamera},
    chameleon::ColorEarned,
    hud::ScreenFlash,
    level, storage,
    player::Player,
};

const HEADER: &str = "ghore-save 1";

pub struct SavePlugin;

impl Plugin for SavePlugin {
    fn build(&self, app: &mut App) {
        // Start from the newest save: the world is built around it, and `apply_load` finishes the job.
        // With the menu up the player chooses ("Load last checkpoint"); scripted runs still start from it.
        let snapshot = newest_save().filter(|_| !crate::settings::menu_at_start()).and_then(|slot| {
            let loaded = storage::read(slot).and_then(|t| Snapshot::parse(&t));
            match loaded {
                Ok(s) => {
                    info!("startup load: {}", storage::describe(slot));
                    Some(s)
                }
                Err(e) => {
                    warn!("startup load: {}: {e}", storage::describe(slot));
                    None
                }
            }
        });
        app.insert_resource(Pending { snapshot, wait: 1 })
            .add_message::<LoadGame>()
            .add_message::<LoadLast>()
            .add_message::<AutoSave>()
            .add_message::<LoadOnDeath>()
            .add_systems(
                PreUpdate,
                (quick_save, auto_save, request_load, request_load_last, load_on_death).after(bevy::input::InputSystems).before(level::restart),
            )
            .add_systems(Update, apply_load);
    }
}

/// A save was read: `level::restart` resets the world (like F8) and `apply_load` puts the saved state back.
#[derive(Message)]
pub struct LoadGame;

/// The menu asks for the most recent save, quick or auto.
#[derive(Message)]
pub struct LoadLast;

/// Something worth saving happened (a boss fight began, a boss died, a boss was eaten).
#[derive(Message)]
pub struct AutoSave;

/// The chameleon has lain dead long enough and there is a save: load it
/// instead of respawning at the spawn point.
#[derive(Message)]
pub struct LoadOnDeath;

struct Snapshot {
    pos: Vec2,
    vel: Vec2,
    facing: f32,
    hp: i32,
    max_hp: i32,
    shadow: f32,
    digest: f32,
    /// Bosses killed, in the order their colors were earned.
    dead: Vec<BossKind>,
    /// After the ending: everyone is back and cheering (`boss::Celebration`), not dead.
    celebration: bool,
}

/// A loaded snapshot waiting for the rebuilt world (bosses and the new chameleon
/// exist from the frame after the restart).
#[derive(Resource, Default)]
pub struct Pending {
    snapshot: Option<Snapshot>,
    wait: u8,
}

impl Pending {
    /// Where the chameleon will stand once the load is done, so the rebuilt
    /// world starts it (and the camera) there rather than at the spawn point.
    pub fn at(&self) -> Option<Vec2> {
        self.snapshot.as_ref().map(|s| s.pos)
    }
}

fn kind_name(kind: BossKind) -> String {
    format!("{kind:?}")
}

impl Snapshot {
    fn to_text(&self) -> String {
        let mut out = format!("{HEADER}\n");
        let mut line = |k: &str, v: String| out.push_str(&format!("{k}={v}\n"));
        line("pos", format!("{} {}", self.pos.x, self.pos.y));
        line("vel", format!("{} {}", self.vel.x, self.vel.y));
        line("facing", self.facing.to_string());
        line("hp", self.hp.to_string());
        line("max_hp", self.max_hp.to_string());
        line("shadow", self.shadow.to_string());
        line("digest", self.digest.to_string());
        for kind in &self.dead {
            line("dead", kind_name(*kind));
        }
        if self.celebration {
            line("celebration", "1".into());
        }
        out
    }

    fn parse(text: &str) -> Result<Snapshot, String> {
        let mut lines = text.lines();
        if lines.next() != Some(HEADER) {
            return Err("not a save of this version".into());
        }
        let mut s = Snapshot {
            pos: Vec2::ZERO,
            vel: Vec2::ZERO,
            facing: 1.0,
            hp: 0,
            max_hp: 0,
            shadow: 0.0,
            digest: 0.0,
            dead: Vec::new(),
            celebration: false,
        };
        let mut has_pos = false;
        for line in lines {
            let Some((key, value)) = line.split_once('=') else { continue };
            let num = |v: &str| v.trim().parse::<f32>().map_err(|e| format!("{key}: {e}"));
            let vec = |v: &str| -> Result<Vec2, String> {
                let (x, y) = v.split_once(' ').ok_or_else(|| format!("{key}: expected two numbers"))?;
                Ok(Vec2::new(num(x)?, num(y)?))
            };
            match key {
                "pos" => {
                    s.pos = vec(value)?;
                    has_pos = true;
                }
                "vel" => s.vel = vec(value)?,
                "facing" => s.facing = if num(value)? < 0.0 { -1.0 } else { 1.0 },
                "hp" => s.hp = num(value)? as i32,
                "max_hp" => s.max_hp = num(value)? as i32,
                "shadow" => s.shadow = num(value)?,
                "digest" => s.digest = num(value)?,
                "celebration" => s.celebration = value.trim() == "1",
                "dead" => s.dead.extend(BossKind::ALL.into_iter().find(|k| kind_name(*k) == value.trim())),
                _ => {}
            }
        }
        if !has_pos || s.max_hp < 1 || s.hp < 1 || !s.pos.is_finite() || !s.vel.is_finite() {
            return Err("incomplete save".into());
        }
        s.hp = s.hp.min(s.max_hp);
        Ok(s)
    }
}

impl Snapshot {
    fn capture(p: &Player, tf: &Transform, defeated: &Defeated, party: &Celebration) -> Snapshot {
        // In kill order: colors are earned in it, and the last one is the skin.
        Snapshot {
            pos: tf.translation.truncate(),
            vel: p.vel,
            facing: p.facing,
            hp: p.hp,
            max_hp: p.max_hp,
            shadow: p.shadow,
            digest: p.digest,
            dead: defeated.0.clone(),
            celebration: party.0,
        }
    }
}

/// Writes the current game down in the slot with this extension.
fn write_save(ext: &str, what: &str, player: (&Player, &Transform), defeated: &Defeated, party: &Celebration) -> bool {
    let (p, tf) = player;
    if p.dead.is_some() {
        warn!("{what}: not while dead");
        return false;
    }
    let snapshot = Snapshot::capture(p, tf, defeated, party);
    match storage::write(ext, &snapshot.to_text()) {
        Ok(()) => {
            info!("{what}: {}", storage::describe(ext));
            true
        }
        Err(e) => {
            warn!("{what}: {e}");
            false
        }
    }
}

/// F5: write the current game down.
fn quick_save(keys: Res<ButtonInput<KeyCode>>, mut flash: ResMut<ScreenFlash>, pending: Res<Pending>, defeated: Res<Defeated>, party: Res<Celebration>, player: Single<(&Player, &Transform)>) {
    if keys.just_pressed(KeyCode::F5) && pending.snapshot.is_none() && write_save("save", "quick save", *player, &defeated, &party) {
        flash.flash(Color::WHITE, 0.18);
    }
}

/// A boss fight began, or a boss died or was eaten: the one auto save.
fn auto_save(mut events: MessageReader<AutoSave>, pending: Res<Pending>, defeated: Res<Defeated>, party: Res<Celebration>, player: Single<(&Player, &Transform)>) {
    // A load is rebuilding the world: `Defeated` and the player are blank until `apply_load`
    // puts the save back, and a boss waking meanwhile (the save is at its lair) would overwrite
    // the save with that blank state.
    if events.read().count() > 0 && pending.snapshot.is_none() {
        write_save("autosave", "auto save", *player, &defeated, &party);
    }
}

/// Reads a save and starts the rebuild (`apply_load` finishes it).
fn start_load(slot: &str, what: &str, pending: &mut Pending, load: &mut MessageWriter<LoadGame>) {
    let snapshot = storage::read(slot).and_then(|text| Snapshot::parse(&text)).map_err(|e| format!("{}: {e}", storage::describe(slot)));
    match snapshot {
        Ok(snapshot) => {
            info!("{what}: {}", storage::describe(slot));
            load.write(LoadGame);
            *pending = Pending { snapshot: Some(snapshot), wait: 1 };
        }
        Err(e) => warn!("{what}: {e}"),
    }
}

/// F9: load the quick save.
fn request_load(keys: Res<ButtonInput<KeyCode>>, mut pending: ResMut<Pending>, mut load: MessageWriter<LoadGame>) {
    if !keys.just_pressed(KeyCode::F9) {
        return;
    }
    start_load("save", "quick load", &mut pending, &mut load);
}

/// Menu: "Load last checkpoint".
fn request_load_last(mut asked: MessageReader<LoadLast>, mut pending: ResMut<Pending>, mut load: MessageWriter<LoadGame>) {
    if asked.read().count() == 0 {
        return;
    }
    match newest_save() {
        Some(slot) => start_load(slot, "load last checkpoint", &mut pending, &mut load),
        None => warn!("load last checkpoint: no save"),
    }
}

/// The most recent save, quick or auto.
fn newest_save() -> Option<&'static str> {
    storage::newest(&["save", "autosave"])
}

pub fn has_save() -> bool {
    newest_save().is_some()
}

/// Dead for long enough: go back to the most recent save.
fn load_on_death(mut dying: MessageReader<LoadOnDeath>, mut pending: ResMut<Pending>, mut load: MessageWriter<LoadGame>) {
    if dying.read().count() == 0 || pending.snapshot.is_some() {
        return;
    }
    if let Some(slot) = newest_save() {
        start_load(slot, "load on death", &mut pending, &mut load);
    }
}

/// The frame after the rebuild: dead bosses stay dead (and give their colors),
/// the chameleon is put where and how it was.
fn apply_load(
    mut pending: ResMut<Pending>,
    mut commands: Commands,
    mut flash: ResMut<ScreenFlash>,
    mut earned: MessageWriter<ColorEarned>,
    mut defeated: ResMut<Defeated>,
    mut party: ResMut<Celebration>,
    player: Single<(&mut Player, &mut Transform)>,
    mut follow: Single<&mut Follow, With<MainCamera>>,
    bosses: Query<(Entity, &Boss, &Lair)>,
) {
    if pending.snapshot.is_none() {
        return;
    }
    if pending.wait > 0 {
        pending.wait -= 1;
        return;
    }
    let Some(s) = pending.snapshot.take() else { return };
    // The celebration: every boss is back (and cheering), none stays dead.
    party.0 = s.celebration;
    for kind in &s.dead {
        defeated.add(*kind);
        earned.write(ColorEarned(kind.spec().color));
        if !s.celebration {
            for (e, _, _) in bosses.iter().filter(|(_, _, l)| l.kind == *kind) {
                commands.entity(e).despawn();
            }
        }
    }
    let (mut p, mut tf) = player.into_inner();
    tf.translation.x = s.pos.x;
    tf.translation.y = s.pos.y;
    p.settle_at(s.pos);
    p.vel = s.vel;
    p.facing = s.facing;
    p.max_hp = s.max_hp;
    p.hp = s.hp;
    p.shadow = s.shadow.clamp(0.0, (p.max_hp - p.hp) as f32);
    p.digest = s.digest.clamp(0.0, 1.0).min(p.shadow);
    follow.cut_to(s.pos);
    flash.flash(Color::WHITE, 0.3);
}
