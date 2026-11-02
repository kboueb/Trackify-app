//! Propagation des changements : événements vers toutes les fenêtres + mise à jour du tray.

use tauri::AppHandle;
use tauri_specta::Event;

use crate::{
    events::{EntriesChanged, ProjectsChanged, TimerChanged},
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
