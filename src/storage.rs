//! Where saves and settings live. Native: small text files next to the executable
//! (`ghore.exe` → `ghore.save`, `.autosave`, `.settings`; a `target/*/deps/` test build shares
//! the files of the normal one). Web: the browser's `localStorage`, one key per slot.
//!
//! A slot is just its extension: `"save"`, `"autosave"` or `"settings"`.

#[cfg(not(target_arch = "wasm32"))]
mod imp {
    use std::{fs, path::PathBuf};

    fn path(slot: &str) -> Option<PathBuf> {
        let exe = std::env::current_exe().ok()?;
        let dir = exe.parent()?;
        if dir.file_name().is_some_and(|n| n == "deps") {
            let stem = exe.file_stem()?.to_str()?;
            let name = stem.rsplit_once('-').map_or(stem, |(name, _hash)| name);
            return Some(dir.parent()?.join(name).with_extension(slot));
        }
        Some(exe.with_extension(slot))
    }

    pub fn describe(slot: &str) -> String {
        path(slot).map_or_else(|| slot.to_string(), |p| p.display().to_string())
    }

    pub fn read(slot: &str) -> Result<String, String> {
        fs::read_to_string(path(slot).ok_or("can't tell where the executable is")?).map_err(|e| e.to_string())
    }

    pub fn write(slot: &str, text: &str) -> Result<(), String> {
        let path = path(slot).ok_or("can't tell where the executable is")?;
        // Write beside it and rename, so a crash can't leave half a save.
        let tmp = path.with_extension(format!("{slot}.tmp"));
        fs::write(&tmp, text).and_then(|_| fs::rename(&tmp, &path)).map_err(|e| format!("{}: {e}", path.display()))
    }

    /// The most recently written of these slots.
    pub fn newest(slots: &[&'static str]) -> Option<&'static str> {
        slots
            .iter()
            .filter_map(|&slot| Some((fs::metadata(path(slot)?).and_then(|m| m.modified()).ok()?, slot)))
            .max_by_key(|(time, _)| *time)
            .map(|(_, slot)| slot)
    }
}

#[cfg(target_arch = "wasm32")]
mod imp {
    const PREFIX: &str = "ghore.";
    const NEWEST: &str = "ghore.newest";

    fn store() -> Option<web_sys::Storage> {
        web_sys::window()?.local_storage().ok()?
    }

    pub fn describe(slot: &str) -> String {
        format!("localStorage[{PREFIX}{slot}]")
    }

    pub fn read(slot: &str) -> Result<String, String> {
        store().and_then(|s| s.get_item(&format!("{PREFIX}{slot}")).ok().flatten()).ok_or_else(|| "empty".to_string())
    }

    pub fn write(slot: &str, text: &str) -> Result<(), String> {
        let store = store().ok_or("localStorage is not available")?;
        store.set_item(&format!("{PREFIX}{slot}"), text).map_err(|_| "localStorage refused the write".to_string())?;
        // Saves have no file times: remember which one came last.
        if slot != "settings" {
            let _ = store.set_item(NEWEST, slot);
        }
        Ok(())
    }

    pub fn newest(slots: &[&'static str]) -> Option<&'static str> {
        let last = store()?.get_item(NEWEST).ok().flatten();
        let has = |slot: &str| read(slot).is_ok();
        last.and_then(|l| slots.iter().copied().find(|s| *s == l && has(s))).or_else(|| slots.iter().copied().find(|s| has(s)))
    }
}

pub use imp::*;
