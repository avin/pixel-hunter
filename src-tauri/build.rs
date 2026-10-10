fn main() {
    copy_native_runtime();
    let mut attributes = tauri_build::Attributes::new();

    // `cfg(debug_assertions)` describes the build script itself, not the app being built.
    if std::env::var("PROFILE").as_deref() == Ok("release") {
        attributes = attributes.windows_attributes(
            tauri_build::WindowsAttributes::new().app_manifest(include_str!("app.manifest")),
        );
    }

    tauri_build::try_build(attributes).expect("failed to run tauri-build");
}

// Cargo executables and tests load native DLLs beside the executable, without an OpenCV PATH.
fn copy_native_runtime() {
    let runtime = std::path::Path::new("native/runtime");
    println!("cargo:rerun-if-changed={}", runtime.display());
    let out = std::path::PathBuf::from(std::env::var_os("OUT_DIR").expect("Cargo OUT_DIR"));
    let profile = out.ancestors().nth(3).expect("Cargo profile directory");
    for entry in std::fs::read_dir(runtime).expect("Run scripts/prepare-opencv.ps1 first") {
        let path = entry.expect("native runtime entry").path();
        if path.extension().is_some_and(|extension| extension == "dll") {
            let name = path.file_name().expect("runtime DLL filename");
            for directory in [
                profile.to_path_buf(),
                profile.join("deps"),
                profile.join("examples"),
            ] {
                std::fs::create_dir_all(&directory).expect("create DLL destination");
                let destination = directory.join(name);
                // Windows locks loaded DLLs. Leave an identical runtime in place
                // so rebuilding while the development app runs remains possible.
                let unchanged = std::fs::read(&destination).ok().is_some_and(|bytes| {
                    bytes == std::fs::read(&path).expect("read native runtime DLL")
                });
                if !unchanged {
                    std::fs::copy(&path, destination).expect("copy native runtime DLL");
                }
            }
        }
    }
}
