// Embeds a Windows resource with version info, file description, and product
// name into the compiled .exe. This is what Explorer shows in the file's
// Properties dialog and what SmartScreen / AV engines key off when deciding
// whether a binary looks "legit" vs "anonymous". It's not a replacement for
// code signing, but every little bit of metadata helps reduce false flags.

#[cfg(windows)]
fn main() {
    embed_resources();
    link_static_cxx_runtime();
}

#[cfg(all(not(windows), target_os = "linux"))]
fn main() {
    // Cross-compiling from Linux to Windows: use x86_64-w64-mingw32-windres.
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        embed_resources();
        link_static_cxx_runtime();
    }
}

#[cfg(all(not(windows), not(target_os = "linux")))]
fn main() {}

/// When cross-compiling for Windows with mingw-w64, openh264-sys2 emits a
/// `cargo:rustc-link-lib=dylib=stdc++` directive that explicitly pulls the
/// libstdc++-6.dll import, so we can't statically link via build-script link
/// args — the dylib= directive wins. The installer bundles the three mingw
/// runtime DLLs alongside callmor-agent.exe instead (handled by NSIS).
#[cfg(any(windows, target_os = "linux"))]
fn link_static_cxx_runtime() {
    // Intentionally no-op — kept as a stable entry point in case we migrate
    // to MSVC or fork openh264-sys2 later.
}

#[cfg(any(windows, target_os = "linux"))]
fn embed_resources() {
    use std::io::Write;

    let version = env!("CARGO_PKG_VERSION");
    let v_parts: Vec<u32> = version
        .split('.')
        .filter_map(|s| s.parse().ok())
        .collect();
    let (maj, min, pat) = (
        *v_parts.first().unwrap_or(&0),
        *v_parts.get(1).unwrap_or(&1),
        *v_parts.get(2).unwrap_or(&0),
    );

    let rc = format!(r#"
#include <winver.h>

1 VERSIONINFO
FILEVERSION    {maj},{min},{pat},0
PRODUCTVERSION {maj},{min},{pat},0
FILEOS         VOS_NT_WINDOWS32
FILETYPE       VFT_APP
{{
  BLOCK "StringFileInfo"
  {{
    BLOCK "040904b0"
    {{
      VALUE "CompanyName",      "Callmor\0"
      VALUE "FileDescription",  "Callmor Remote Desktop Agent\0"
      VALUE "FileVersion",      "{version}\0"
      VALUE "InternalName",     "callmor-agent\0"
      VALUE "LegalCopyright",   "Copyright (C) Callmor\0"
      VALUE "OriginalFilename", "callmor-agent.exe\0"
      VALUE "ProductName",      "Callmor Remote Desktop\0"
      VALUE "ProductVersion",   "{version}\0"
    }}
  }}
  BLOCK "VarFileInfo"
  {{
    VALUE "Translation", 0x409, 1200
  }}
}}
"#);

    let out_dir = std::env::var("OUT_DIR").expect("OUT_DIR");
    let rc_path = std::path::PathBuf::from(&out_dir).join("callmor-agent.rc");
    let res_path = std::path::PathBuf::from(&out_dir).join("callmor-agent.res");
    std::fs::File::create(&rc_path)
        .expect("create rc")
        .write_all(rc.as_bytes())
        .expect("write rc");

    // Pick a windres that matches the target.
    let windres = if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows")
        && std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("gnu")
        && cfg!(not(windows))
    {
        "x86_64-w64-mingw32-windres"
    } else {
        "windres"
    };

    let status = std::process::Command::new(windres)
        .args([rc_path.to_str().unwrap(), "-O", "coff", "-o"])
        .arg(&res_path)
        .status();
    match status {
        Ok(s) if s.success() => {
            println!("cargo:rustc-link-arg-bins={}", res_path.display());
        }
        _ => {
            // Non-fatal: if windres isn't available we still compile, just
            // without the version info. Don't fail the build.
            println!("cargo:warning=windres not found, skipping version resource");
        }
    }
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=Cargo.toml");
}
