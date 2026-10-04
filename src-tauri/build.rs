fn main() {
    let mut attributes = tauri_build::Attributes::new();

    // `cfg(debug_assertions)` describes the build script itself, not the app being built.
    if std::env::var("PROFILE").as_deref() == Ok("release") {
        attributes = attributes.windows_attributes(
            tauri_build::WindowsAttributes::new().app_manifest(include_str!("app.manifest")),
        );
    }

    tauri_build::try_build(attributes).expect("failed to run tauri-build");
}
