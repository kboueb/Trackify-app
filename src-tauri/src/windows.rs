//! Fenêtre principale et sélecteur rapide (palette).

use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindowBuilder, WindowEvent};
use tauri_specta::Event;

use crate::events::PaletteOpened;

pub const MAIN: &str = "main";
pub const PALETTE: &str = "palette";

pub fn show_main(app: &AppHandle) {
    if let Some(w) = app.get_webview_window(MAIN) {
        let _ = w.show();
        let _ = w.unminimize();
        let _ = w.set_focus();
    }
}

/// Crée la palette une fois pour toutes, masquée : l'afficher ensuite est instantané.
pub fn create_palette(app: &AppHandle) -> tauri::Result<()> {
    let window = WebviewWindowBuilder::new(app, PALETTE, WebviewUrl::App("index.html#palette".into()))
        .title("Trackify — sélecteur rapide")
        .inner_size(560.0, 380.0)
        .resizable(false)
        .decorations(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .visible(false)
        .center()
        .build()?;

    let handle = app.clone();
    window.on_window_event(move |event| {
        if let WindowEvent::Focused(false) = event {
            hide_palette(&handle);
        }
    });
    Ok(())
}

pub fn toggle_palette(app: &AppHandle) {
    let Some(w) = app.get_webview_window(PALETTE) else { return };
    if w.is_visible().unwrap_or(false) {
        hide_palette(app);
        return;
    }
    let _ = PaletteOpened.emit(app);
    let _ = w.center();
    let _ = w.show();
    let _ = w.set_focus();
}

/// Masque la palette. Sur macOS, si la fenêtre principale est cachée, on rend aussi la main
/// à l'application précédente pour ne pas laisser Trackify actif sans fenêtre.
pub fn hide_palette(app: &AppHandle) {
    let Some(w) = app.get_webview_window(PALETTE) else { return };
    if !w.is_visible().unwrap_or(false) {
        return;
    }
    let _ = w.hide();
    #[cfg(target_os = "macos")]
    {
        let main_visible = app
            .get_webview_window(MAIN)
            .and_then(|m| m.is_visible().ok())
            .unwrap_or(false);
        if !main_visible {
            let _ = app.hide();
        }
    }
}

pub const REMINDER: &str = "reminder";
const REMINDER_WIDTH: f64 = 360.0;
const REMINDER_HEIGHT: f64 = 128.0;

/// Petite fenêtre de rappel, en haut à droite, qui ne prend pas le focus.
pub fn create_reminder(app: &AppHandle) -> tauri::Result<()> {
    WebviewWindowBuilder::new(app, REMINDER, WebviewUrl::App("index.html#reminder".into()))
        .title("Trackify — rappel")
        .inner_size(REMINDER_WIDTH, REMINDER_HEIGHT)
        .resizable(false)
        .decorations(false)
        .always_on_top(true)
        .visible_on_all_workspaces(true)
        .skip_taskbar(true)
        .focused(false)
        .visible(false)
        .build()?;
    Ok(())
}

pub fn show_reminder(app: &AppHandle) {
    let Some(w) = app.get_webview_window(REMINDER) else { return };
    if let Ok(Some(monitor)) = w.primary_monitor() {
        let scale = monitor.scale_factor();
        let size = monitor.size().to_logical::<f64>(scale);
        let origin = monitor.position().to_logical::<f64>(scale);
        let x = origin.x + size.width - REMINDER_WIDTH - 16.0;
        let y = origin.y + 40.0; // sous la barre des menus macOS
        let _ = w.set_position(tauri::LogicalPosition::new(x, y));
    }
    let _ = w.show();
}

pub fn hide_reminder(app: &AppHandle) {
    if let Some(w) = app.get_webview_window(REMINDER) {
        let _ = w.hide();
    }
}
