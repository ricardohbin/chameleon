// Windows (GNU toolchain): embed the eye icon and product name into the executable with `windres`.
// Best effort: without windres the game still builds, it just keeps the default exe icon.
fn main() {
    println!("cargo:rerun-if-changed=assets/icon.ico");
    println!("cargo:rerun-if-changed=build.rs");
    check_bake();
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    let out = std::path::PathBuf::from(std::env::var("OUT_DIR").unwrap());
    let manifest = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    let icon = format!("{manifest}/assets/icon.ico").replace('\\', "/");
    let rc = out.join("app.rc");
    let obj = out.join("app_res.o");
    let script = format!(
        "1 ICON \"{icon}\"
         1 VERSIONINFO
FILEVERSION 0,1,0,0
PRODUCTVERSION 0,1,0,0
BEGIN
 BLOCK \"StringFileInfo\"
 BEGIN
  BLOCK \"040904b0\"
  BEGIN
            VALUE \"ProductName\", \"Ghore, The Chameleon\"
   VALUE \"FileDescription\", \"Ghore, The Chameleon\"
  END
 END
          BLOCK \"VarFileInfo\"
 BEGIN
  VALUE \"Translation\", 0x409, 1200
 END
END
"
    );
    std::fs::write(&rc, script).unwrap();
    let ok = std::process::Command::new("windres")
        .args(["-O", "coff", "-i"])
        .arg(&rc)
        .arg("-o")
        .arg(&obj)
        .status()
        .is_ok_and(|s| s.success());
    if ok {
        println!("cargo:rustc-link-arg-bins={}", obj.display());
    } else {
        println!("cargo:warning=windres failed: the exe keeps the default icon");
    }
}

/// Release builds embed `assets/world.baked`, not the sketch itself: refuse to build one that is older than
/// `assets/world.aseprite` (the sketch was edited and nobody ran the bake since).
fn check_bake() {
    println!("cargo:rerun-if-changed=assets/world.aseprite");
    println!("cargo:rerun-if-changed=assets/world.baked");
    println!("cargo:rerun-if-env-changed=GHORE_SKIP_BAKE_CHECK");
    if std::env::var("PROFILE").as_deref() != Ok("release") || std::env::var_os("GHORE_SKIP_BAKE_CHECK").is_some() {
        return;
    }
    let modified = |path: &str| std::fs::metadata(path).and_then(|m| m.modified()).ok();
    let (Some(sketch), Some(baked)) = (modified("assets/world.aseprite"), modified("assets/world.baked")) else { return };
    if sketch > baked {
        panic!(
            "

assets/world.aseprite is newer than assets/world.baked, and release builds embed the baked one,
             so this build would ship an out-of-date world.

             To fix it:
               1. cargo run -- diff    (review what changed in the sketch since the last bake, and interpret it)
               2. cargo run -- bake    (write assets/world.baked)
               3. build the release again

             To build anyway (e.g. after a fresh checkout reset the file times), set GHORE_SKIP_BAKE_CHECK=1.
"
        );
    }
}
