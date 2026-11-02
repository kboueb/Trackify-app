//! Raccourci clavier global qui ouvre le sélecteur rapide.

use std::str::FromStr;

use tauri::AppHandle;
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};

use crate::{
    db::settings::{self, Settings},
    error::{AppError, AppResult},
    windows,
};

pub fn plugin() -> tauri::plugin::TauriPlugin<tauri::Wry> {
    tauri_plugin_global_shortcut::Builder::new()
        .with_handler(|app, _shortcut, event| {
            // Un seul raccourci est enregistré : c'est forcément celui de la palette.
            if event.state() == ShortcutState::Pressed {
                windows::toggle_palette(app);
            }
        })
        .build()
}

/// Analyse un accélérateur et refuse ceux sans touche de modification
/// (une simple lettre capturée globalement rendrait la saisie impossible ailleurs).
pub fn parse(accelerator: &str) -> AppResult<Shortcut> {
    let shortcut = Shortcut::from_str(accelerator)
        .map_err(|_| AppError::Validation(format!("Raccourci invalide : « {accelerator} ».")))?;
    if shortcut.mods.is_empty() {
        return Err(AppError::Validation(
            "Le raccourci doit contenir au moins une touche de modification (⌘, ⌃, ⌥ ou ⇧).".into(),
        ));
    }
    Ok(shortcut)
}

pub fn register(app: &AppHandle, accelerator: &str) -> AppResult<()> {
    let shortcut = parse(accelerator)?;
    app.global_shortcut().register(shortcut).map_err(|e| {
        AppError::Validation(format!(
            "Impossible d'utiliser « {accelerator} », il est peut-être déjà pris par une autre application ({e})."
        ))
    })
}

/// Remplace le raccourci. Le nouveau est enregistré avant de libérer l'ancien :
/// en cas d'échec, l'ancien reste actif.
pub async fn change(app: &AppHandle, db: &sqlx::SqlitePool, accelerator: &str) -> AppResult<Settings> {
    let mut current = settings::load(db).await?;
    let accelerator = accelerator.trim();
    if current.shortcut == accelerator {
        return Ok(current);
    }
    register(app, accelerator)?;
    if let Ok(old) = Shortcut::from_str(&current.shortcut) {
        let _ = app.global_shortcut().unregister(old);
    }
    current.shortcut = accelerator.to_string();
    settings::save(db, &current).await?;
    Ok(current)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepte_les_formats_du_frontend() {
        assert!(parse("CommandOrControl+Shift+Space").is_ok());
        assert!(parse("Super+Alt+KeyT").is_ok());
        assert!(parse("Control+Shift+Digit1").is_ok());
    }

    #[test]
    fn refuse_sans_modificateur() {
        assert!(matches!(parse("KeyT"), Err(AppError::Validation(_))));
    }

    #[test]
    fn refuse_les_inconnus() {
        assert!(parse("Shift+Banane").is_err());
        assert!(parse("").is_err());
    }
}
