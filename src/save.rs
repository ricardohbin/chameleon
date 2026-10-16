//! Quick save (F5) / quick load (F9), and the auto save.
//!
//! Two slots, small text files next to the executable with the same name and
//! a `.save` (quick) or `.autosave` extension (`ghore.exe` → `ghore.save`; a
//! `target/*/deps/` test build shares the files of the normal one). The auto
//! save is written when a boss fight starts, when the boss dies and when its
//! carcass is eaten (`AutoSave`). When the chameleon dies, the most recent of
//! the two is loaded by itself. A save holds the progress that
//! matters: where the chameleon is and how it is (health, max health, shadow
//! health, facing, speed) and which bosses are dead (their colors and the
//! skin come from that, and the powers they gave). Everything else resets the way it does when the
//! chameleon dies: living bosses asleep at home, mobs back, no blood. Loading
//! is `Restart` (F8) + putting the saved state back a frame later.

use std::{fs, path::PathBuf};

use bevy::prelude::*;

use crate::{
    boss::{Boss, BossKind, Defeated, Lair},
    camera::{Follow, MainCamera},
    chameleon::ColorEarned,
    hud::ScreenFlash,
    level,
    player::Player,
};

const HEADER: &str = "ghore-save 1";

pub struct SavePlugin;

impl Plugin for SavePlugin {
    fn build(&self, app: &mut App) {
        // Start from the newest save: the world is built around it, and `apply_load` finishes the job.
        // With the menu up the player chooses ("Load last checkpoint"); scripted runs still start from it.
        let snapshot = newest_save().filter(|_| !crate::settings::menu_at_start()).and_then(|path| {
            let loaded = fs::read_to_string(&path).map_err(|e| e.to_string()).and_then(|t| Snapshot::parse(&t));
            match loaded {
                Ok(s) => {
                    info!("startup load: {}", path.display());
                    Some(s)
                }
                Err(e) => {
                    warn!("startup load: {}: {e}", path.display());
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

/// Next to the executable, same name, `.save` or `.autosave`. Test builds in `deps/` (named
/// `ghore-<hash>.exe`) use the file of the build in the folder above.
pub(crate) fn save_path(ext: &str) -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let dir = exe.parent()?;
    if dir.file_name().is_some_and(|n| n == "deps") {
        let stem = exe.file_stem()?.to_str()?;
        let name = stem.rsplit_once('-').map_or(stem, |(name, _hash)| name);
        return Some(dir.parent()?.join(name).with_extension(ext));
    }
    Some(exe.with_extension(ext))
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
    fn capture(p: &Player, tf: &Transform, defeated: &Defeated) -> Snapshot {
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
        }
    }
}

/// Writes the current game down in the slot with this extension.
fn write_save(ext: &str, what: &str, player: (&Player, &Transform), defeated: &Defeated) -> bool {
    let (p, tf) = player;
    if p.dead.is_some() {
        warn!("{what}: not while dead");
        return false;
    }
    let Some(path) = save_path(ext) else {
        warn!("{what}: can't tell where the executable is");
        return false;
    };
    let snapshot = Snapshot::capture(p, tf, defeated);
    // Write beside it and rename, so a crash can't leave half a save.
    let tmp = path.with_extension(format!("{ext}.tmp"));
    match fs::write(&tmp, snapshot.to_text()).and_then(|_| fs::rename(&tmp, &path)) {
        Ok(()) => {
            info!("{what}: {}", path.display());
            true
        }
        Err(e) => {
            warn!("{what}: {}: {e}", path.display());
            false
        }
    }
}

/// F5: write the current game down.
fn quick_save(keys: Res<ButtonInput<KeyCode>>, mut flash: ResMut<ScreenFlash>, pending: Res<Pending>, defeated: Res<Defeated>, player: Single<(&Player, &Transform)>) {
    if keys.just_pressed(KeyCode::F5) && pending.snapshot.is_none() && write_save("save", "quick save", *player, &defeated) {
        flash.flash(Color::WHITE, 0.18);
    }
}

/// A boss fight began, or a boss died or was eaten: the one auto save.
fn auto_save(mut events: MessageReader<AutoSave>, pending: Res<Pending>, defeated: Res<Defeated>, player: Single<(&Player, &Transform)>) {
    // A load is rebuilding the world: `Defeated` and the player are blank until `apply_load`
    // puts the save back, and a boss waking meanwhile (the save is at its lair) would overwrite
    // the save with that blank state.
    if events.read().count() > 0 && pending.snapshot.is_none() {
        write_save("autosave", "auto save", *player, &defeated);
    }
}

/// Reads a save and starts the rebuild (`apply_load` finishes it).
fn start_load(path: &PathBuf, what: &str, pending: &mut Pending, load: &mut MessageWriter<LoadGame>) {
    let snapshot = match fs::read_to_string(path) {
        Ok(text) => Snapshot::parse(&text).map_err(|e| format!("{}: {e}", path.display())),
        Err(e) => Err(format!("{}: {e}", path.display())),
    };
    match snapshot {
        Ok(snapshot) => {
            info!("{what}: {}", path.display());
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
    let Some(path) = save_path("save") else {
        warn!("quick load: can't tell where the executable is");
        return;
    };
    start_load(&path, "quick load", &mut pending, &mut load);
}

/// Menu: "Load last checkpoint".
fn request_load_last(mut asked: MessageReader<LoadLast>, mut pending: ResMut<Pending>, mut load: MessageWriter<LoadGame>) {
    if asked.read().count() == 0 {
        return;
    }
    match newest_save() {
        Some(path) => start_load(&path, "load last checkpoint", &mut pending, &mut load),
        None => warn!("load last checkpoint: no save"),
    }
}

/// The most recent save, quick or auto (by file time).
fn newest_save() -> Option<PathBuf> {
    ["save", "autosave"]
        .into_iter()
        .filter_map(save_path)
        .filter_map(|path| Some((fs::metadata(&path).and_then(|m| m.modified()).ok()?, path)))
        .max_by_key(|(time, _)| *time)
        .map(|(_, path)| path)
}

pub fn has_save() -> bool {
    newest_save().is_some()
}

/// Dead for long enough: go back to the most recent save.
fn load_on_death(mut dying: MessageReader<LoadOnDeath>, mut pending: ResMut<Pending>, mut load: MessageWriter<LoadGame>) {
    if dying.read().count() == 0 || pending.snapshot.is_some() {
        return;
    }
    if let Some(path) = newest_save() {
        start_load(&path, "load on death", &mut pending, &mut load);
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
    for kind in &s.dead {
        defeated.add(*kind);
        for (e, boss, _) in bosses.iter().filter(|(_, _, l)| l.kind == *kind) {
            earned.write(ColorEarned(boss.spec.color));
            commands.entity(e).despawn();
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
