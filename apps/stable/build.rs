use std::{env, path::PathBuf, process::Command};
fn main() {
    println!("cargo:rerun-if-changed=assets/app.rc");
    println!("cargo:rerun-if-changed=assets/app-icon.ico");
    if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    let mut compilers = vec![PathBuf::from("rc.exe")];
    if let Some(program_files) = env::var_os("ProgramFiles(x86)") {
        let sdk = PathBuf::from(program_files).join("Windows Kits/10/bin");
        if let Ok(entries) = std::fs::read_dir(sdk) {
            let mut versions: Vec<_> = entries.flatten().map(|entry| entry.path()).collect();
            versions.sort();
            versions.reverse();
            compilers.extend(
                versions
                    .into_iter()
                    .map(|version| version.join("x64/rc.exe")),
            );
        }
    }
    let root = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap());
    let assets = root.join("assets");
    let output = PathBuf::from(env::var_os("OUT_DIR").unwrap()).join("app-icon.res");
    for compiler in compilers {
        let result = Command::new(compiler)
            .current_dir(&assets)
            .arg("/nologo")
            .arg("/fo")
            .arg(&output)
            .arg("app.rc")
            .status();
        if result.is_ok_and(|status| status.success()) {
            println!("cargo:rustc-link-arg-bins={}", output.display());
            return;
        }
    }
    panic!("Windows SDK resource compiler rc.exe was not found or failed; required to embed the application icon");
}
