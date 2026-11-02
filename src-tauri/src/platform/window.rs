//! Fenêtre au premier plan (app + titre), pour le suivi d'activité.
//!
//! Sur macOS, le titre des fenêtres des autres apps n'est lisible qu'avec la permission
//! « Enregistrement de l'écran » ; sans elle, on ne récupère que le nom de l'app.

/// App et titre de la fenêtre active ; `None` si rien n'est lisible (écran verrouillé…).
pub fn active_window() -> Option<(String, String)> {
    let w = active_win_pos_rs::get_active_window().ok()?;
    let app = w.app_name.trim().to_string();
    if app.is_empty() {
        return None;
    }
    Some((app, w.title.trim().to_string()))
}

/// La permission nécessaire aux titres de fenêtre est-elle accordée ?
#[cfg(target_os = "macos")]
pub fn titles_allowed() -> bool {
    #[link(name = "CoreGraphics", kind = "framework")]
    unsafe extern "C" {
        fn CGPreflightScreenCaptureAccess() -> bool;
    }
    unsafe { CGPreflightScreenCaptureAccess() }
}

/// Ailleurs, aucune permission n'est demandée.
#[cfg(not(target_os = "macos"))]
pub fn titles_allowed() -> bool {
    true
}

/// Demande la permission (macOS affiche sa fenêtre système la première fois seulement),
/// puis ouvre le bon panneau des Réglages Système pour pouvoir l'activer.
#[cfg(target_os = "macos")]
pub fn request_titles() {
    #[link(name = "CoreGraphics", kind = "framework")]
    unsafe extern "C" {
        fn CGRequestScreenCaptureAccess() -> bool;
    }
    if !unsafe { CGRequestScreenCaptureAccess() } {
        let _ = std::process::Command::new("open")
            .arg("x-apple.systempreferences:com.apple.preference.security?Privacy_ScreenCapture")
            .spawn();
    }
}

#[cfg(not(target_os = "macos"))]
pub fn request_titles() {}
