//! Propagation des changements : événements vers toutes les fenêtres + mise à jour du tray.

use tauri::AppHandle;
use tauri_specta::Event;

use crate::{
    events::{EntriesChanged, ProjectsChanged, SettingsChanged, TimerChanged},
    tray,
};

/// Timer démarré, arrêté ou entrée supprimée. `last_used_at` a pu changer aussi.
pub fn timer_changed(app: &AppHandle) {
    let _ = TimerChanged.emit(app);
    let _ = EntriesChanged.emit(app);
    let _ = ProjectsChanged.emit(app);
    tray::refresh(app);
}

pub fn projects_changed(app: &AppHandle) {
    let _ = ProjectsChanged.emit(app);
    tray::refresh(app);
}

pub fn entries_changed(app: &AppHandle) {
    let _ = EntriesChanged.emit(app);
}

/// Réglage modifié hors de la fenêtre des réglages (ex. suivi d'activité depuis le tray).
pub fn settings_changed(app: &AppHandle) {
    let _ = SettingsChanged.emit(app);
    tray::refresh(app);
}
