// Windows (GNU toolchain): embed the eye icon and product name into the executable with `windres`.
// Best effort: without windres the game still builds, it just keeps the default exe icon.
fn main() {
    println!("cargo:rerun-if-changed=assets/icon.ico");
    println!("cargo:rerun-if-changed=build.rs");
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
