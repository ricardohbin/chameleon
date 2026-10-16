//! Player settings: difficulty, volume, contrast and the control scheme. They
//! live in a small text file next to the executable (`ghore.settings`, like the
//! saves) and are edited from the menu (`menu.rs`).
//!
//! Difficulty has four steps. The bosses were tuned on the second one
//! (Normal, the default); the mob quantity of the first design is the last (Hard).
//! Deep code (mob placement, bosses, damage) asks `difficulty()` instead of
//! carrying the resource around.

use std::{
    fs,
    sync::atomic::{AtomicU8, Ordering},
};

use bevy::{
    audio::{GlobalVolume, Volume},
    prelude::*,
};

use crate::{save, terrain_art::Contrast};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Difficulty {
    Easy,
    Normal,
    Tough,
    Hard,
}

impl Difficulty {
    pub const ALL: [Difficulty; 4] = [Difficulty::Easy, Difficulty::Normal, Difficulty::Tough, Difficulty::Hard];

    pub fn name(self) -> &'static str {
        match self {
            Difficulty::Easy => "Easy",
            Difficulty::Normal => "Normal",
            Difficulty::Tough => "Tough",
            Difficulty::Hard => "Hard",
        }
    }

    pub fn index(self) -> usize {
        Self::ALL.iter().position(|d| *d == self).unwrap_or(1)
    }

    /// Share of the mobs the world design calls for (Hard is all of them).
    pub fn mobs(self) -> f32 {
        match self {
            Difficulty::Easy => 0.35,
            Difficulty::Normal => 0.6,
            Difficulty::Tough => 0.8,
            Difficulty::Hard => 1.0,
        }
    }

    /// Boss health against what it was tuned with (Normal).
    pub fn boss_hp(self) -> f32 {
        match self {
            Difficulty::Easy => 0.7,
            Difficulty::Normal => 1.0,
            Difficulty::Tough => 1.2,
            Difficulty::Hard => 1.4,
        }
    }

    /// How fast a boss's whole fight runs against how it was designed (Normal):
    /// its movement, wind-ups and recoveries, cooldowns.
    pub fn tempo(self) -> f32 {
        match self {
            Difficulty::Easy => 0.85,
            Difficulty::Normal => 1.0,
            Difficulty::Tough => 1.1,
            Difficulty::Hard => 1.2,
        }
    }

    /// The last difficulty only: a boss's region sends help during the fight.
    pub fn reinforcements(self) -> bool {
        self == Difficulty::Hard
    }

    pub fn blurb(self) -> &'static str {
        match self {
            Difficulty::Easy => "A third of the enemies, softer, slower bosses.",
            Difficulty::Normal => "The bosses as designed, with fewer enemies around.",
            Difficulty::Tough => "More enemies, faster, tougher bosses.",
            Difficulty::Hard => "Every enemy the world holds, faster bosses with more health, and help arrives mid-fight.",
        }
    }
}

static DIFFICULTY: AtomicU8 = AtomicU8::new(1);

/// The difficulty in force (changes apply to what is built or reset next:
/// mobs on a load or new game, bosses when they stand down).
pub fn difficulty() -> Difficulty {
    Difficulty::ALL[(DIFFICULTY.load(Ordering::Relaxed) as usize).min(3)]
}

/// How the chameleon is moved and aimed.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Scheme {
    /// WASD or arrows move, the mouse aims.
    MouseKeyboard,
    /// Keyboard only: the aim follows the last direction pressed.
    Directional,
    /// WASD (or the left stick) moves, the arrows (or the right stick) aim.
    Dual,
}

impl Scheme {
    pub const ALL: [Scheme; 3] = [Scheme::MouseKeyboard, Scheme::Directional, Scheme::Dual];

    pub fn name(self) -> &'static str {
        match self {
            Scheme::MouseKeyboard => "Mouse + keyboard",
            Scheme::Directional => "Directional",
            Scheme::Dual => "Dual (move + aim)",
        }
    }

    pub fn blurb(self) -> &'static str {
        match self {
            Scheme::MouseKeyboard => "WASD or arrows move, the mouse aims.",
            Scheme::Directional => "Keyboard or one stick: the aim follows the last direction pressed.",
            Scheme::Dual => "WASD / left stick move, arrows / right stick aim.",
        }
    }

    pub fn index(self) -> usize {
        Self::ALL.iter().position(|s| *s == self).unwrap_or(0)
    }
}

pub const VOLUME_STEPS: u8 = 10;

#[derive(Resource, Clone, PartialEq)]
pub struct Settings {
    pub difficulty: Difficulty,
    /// 0..=`VOLUME_STEPS`.
    pub volume: u8,
    pub scheme: Scheme,
    pub fullscreen: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Settings { difficulty: Difficulty::Normal, volume: 8, scheme: Scheme::MouseKeyboard, fullscreen: false }
    }
}

impl Settings {
    pub fn volume_linear(&self) -> f32 {
        self.volume as f32 / VOLUME_STEPS as f32
    }
}

/// True when the game starts on the menu; the developer shortcuts that script
/// the game (`HOLD`, `TOUR`...) skip it.
pub fn menu_at_start() -> bool {
    !["HOLD", "TAP", "PRESS", "START", "EXIT_AFTER", "AIM", "TOUR"].iter().any(|k| std::env::var(k).is_ok())
}

fn parse(text: &str) -> (Settings, f32) {
    let mut s = Settings::default();
    let mut contrast = Contrast::default().0;
    for line in text.lines() {
        let Some((key, value)) = line.split_once('=') else { continue };
        let value = value.trim();
        match key.trim() {
            "difficulty" => s.difficulty = Difficulty::ALL.into_iter().find(|d| d.name() == value).unwrap_or(s.difficulty),
            "volume" => s.volume = value.parse::<u8>().map_or(s.volume, |v| v.min(VOLUME_STEPS)),
            "fullscreen" => s.fullscreen = value == "true",
            "scheme" => s.scheme = Scheme::ALL.into_iter().find(|c| c.name() == value).unwrap_or(s.scheme),
            "contrast" => {
                if let Ok(c) = value.parse::<f32>() {
                    if c.is_finite() {
                        contrast = c.clamp(crate::terrain_art::CONTRAST_RANGE.0, crate::terrain_art::CONTRAST_RANGE.1);
                    }
                }
            }
            _ => {}
        }
    }
    (s, contrast)
}

fn load() -> (Settings, f32) {
    save::save_path("settings")
        .and_then(|path| fs::read_to_string(path).ok())
        .map_or((Settings::default(), Contrast::default().0), |t| parse(&t))
}

/// Writes the settings down (a failure is only a warning: they just won't stick).
pub fn store(settings: &Settings, contrast: f32) {
    let Some(path) = save::save_path("settings") else { return };
    let text = format!(
        "difficulty={}\nvolume={}\nscheme={}\nfullscreen={}\ncontrast={}\n",
        settings.difficulty.name(),
        settings.volume,
        settings.scheme.name(),
        settings.fullscreen,
        contrast
    );
    if let Err(e) = fs::write(&path, text) {
        warn!("settings: {}: {e}", path.display());
    }
}

pub struct SettingsPlugin;

impl Plugin for SettingsPlugin {
    fn build(&self, app: &mut App) {
        let (settings, contrast) = load();
        DIFFICULTY.store(settings.difficulty.index() as u8, Ordering::Relaxed);
        app.insert_resource(GlobalVolume::new(Volume::Linear(settings.volume_linear())))
            .insert_resource(settings)
            .insert_resource(Contrast(contrast))
            .add_systems(Update, apply);
    }
}

/// Settings changed in the menu: the difficulty and the volume take effect.
fn apply(settings: Res<Settings>, mut volume: ResMut<GlobalVolume>, mut window: Single<&mut Window, With<bevy::window::PrimaryWindow>>) {
    if !settings.is_changed() {
        return;
    }
    let mode = if settings.fullscreen {
        bevy::window::WindowMode::BorderlessFullscreen(bevy::window::MonitorSelection::Current)
    } else {
        bevy::window::WindowMode::Windowed
    };
    if window.mode != mode {
        window.mode = mode;
    }
    DIFFICULTY.store(settings.difficulty.index() as u8, Ordering::Relaxed);
    volume.volume = Volume::Linear(settings.volume_linear());
}
