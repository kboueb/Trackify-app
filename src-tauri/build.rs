fn main() {
    // Windows : tauri-build n'embarque son manifeste (Common Controls v6) que dans le binaire
    // de l'app. Les exécutables de test, liés aux mêmes API, plantent alors au démarrage
    // (STATUS_ENTRYPOINT_NOT_FOUND). On l'embarque nous-mêmes, pour tous les exécutables.
    let windows = if std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc") {
        let manifest = std::env::current_dir().unwrap().join("windows-app-manifest.xml");
        println!("cargo:rerun-if-changed={}", manifest.display());
        println!("cargo:rustc-link-arg=/MANIFEST:EMBED");
        println!("cargo:rustc-link-arg=/MANIFESTINPUT:{}", manifest.display());
        tauri_build::WindowsAttributes::new_without_app_manifest()
    } else {
        tauri_build::WindowsAttributes::new()
    };
    tauri_build::try_build(tauri_build::Attributes::new().windows_attributes(windows))
        .expect("échec de tauri-build");
}
