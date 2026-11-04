mod commands;
mod db;
mod error;
mod events;
mod notify;
mod platform;
mod services;
mod shortcut;
mod tray;
mod windows;

use sqlx::SqlitePool;
use tauri::{Manager, WindowEvent};
use tauri_plugin_autostart::{MacosLauncher, ManagerExt};
use tauri_specta::{Builder, ErrorHandlingMode, collect_commands, collect_events};

/// Argument passé par le lancement automatique : démarrer sans afficher la fenêtre.
const HIDDEN_ARG: &str = "--hidden";

/// État partagé par toutes les commandes et tâches de fond.
pub struct AppState {
    pub db: SqlitePool,
}

fn specta_builder() -> Builder<tauri::Wry> {
    Builder::<tauri::Wry>::new()
        .commands(collect_commands![
            commands::list_projects,
            commands::recent_projects,
            commands::create_project,
            commands::update_project,
            commands::set_project_archived,
            commands::delete_project,
            commands::get_running,
            commands::start_timer,
            commands::stop_timer,
            commands::list_entries,
            commands::set_entry_note,
            commands::delete_entry,
            commands::get_settings,
            commands::set_shortcut,
            commands::get_autostart,
            commands::set_autostart,
            commands::hide_palette,
            commands::open_main_window,
            commands::set_reminder_settings,
            commands::list_pending_idle,
            commands::resolve_idle,
            commands::get_reminder,
            commands::reminder_action,
            commands::save_entry,
            commands::split_entry,
            commands::merge_entries,
            commands::list_unassigned,
            commands::rename_client,
            commands::get_report,
            commands::export_csv,
            commands::set_rounding,
            commands::set_tray_title,
            commands::activity_status,
            commands::set_activity_enabled,
            commands::request_activity_permission,
            commands::clear_activity,
            commands::backup_info,
            commands::backup_now,
            commands::set_backup_dir,
            commands::reveal_backups,
        ])
        .events(collect_events![
            events::TimerChanged,
            events::ProjectsChanged,
            events::EntriesChanged,
            events::PaletteOpened,
            events::IdleChanged,
            events::ReminderShown,
            events::ReviewRequested,
            events::SettingsChanged,
        ])
        .error_handling(ErrorHandlingMode::Throw)
        .disable_serde_phases()
        // Les horodatages en millisecondes tiennent largement dans un `number` JS.
        .dangerously_cast_bigints_to_number()
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let builder = specta_builder();

    #[cfg(debug_assertions)]
    builder
        .export(specta_typescript::Typescript::default(), "../src/bindings.ts")
        .expect("échec de l'export des bindings TypeScript");

    let app = tauri::Builder::default()
        // En premier : une seconde instance se contente de réafficher la première.
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| windows::show_main(app)))
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_autostart::init(MacosLauncher::LaunchAgent, Some(vec![HIDDEN_ARG])))
        .plugin(shortcut::plugin())
        .invoke_handler(builder.invoke_handler())
        .on_window_event(|window, event| {
            // Fermer la fenêtre principale la masque : l'app continue dans le tray.
            if let WindowEvent::CloseRequested { api, .. } = event
                && window.label() == windows::MAIN
            {
                api.prevent_close();
                let _ = window.hide();
            }
        })
        .setup(move |app| {
            builder.mount_events(app);

            let dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&dir)?;
            let pool = tauri::async_runtime::block_on(db::open(&dir.join("trackify.db")))?;
            let mut settings = tauri::async_runtime::block_on(db::settings::load(&pool))?;

            // Autostart activé d'office au premier lancement d'une version installée
            // (pas en développement : ça enregistrerait le binaire de debug).
            if !settings.autostart_initialized && !cfg!(debug_assertions) {
                let _ = app.autolaunch().enable();
                settings.autostart_initialized = true;
                tauri::async_runtime::block_on(db::settings::save(&pool, &settings))?;
            }

            app.manage(AppState { db: pool });
            let handle = app.handle();
            tray::create(handle)?;
            windows::create_palette(handle)?;
            windows::create_reminder(handle)?;
            services::presence::start(handle);
            services::activity::start(handle);
            services::backup::start(handle);
            if let Err(e) = shortcut::register(handle, &settings.shortcut) {
                eprintln!("raccourci global : {e}");
            }

            if !std::env::args().any(|a| a == HIDDEN_ARG) {
                windows::show_main(handle);
            }
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("erreur au lancement de l'application");

    app.run(|app, event| {
        // macOS : clic sur l'icône du Dock alors que la fenêtre est masquée.
        #[cfg(target_os = "macos")]
        if let tauri::RunEvent::Reopen { .. } = event {
            windows::show_main(app);
        }
        let _ = (app, event);
    });
}

#[cfg(test)]
mod tests {
    /// `cargo test` régénère aussi les bindings, pour que le frontend ne soit jamais en retard.
    #[test]
    fn export_bindings() {
        super::specta_builder()
            .export(specta_typescript::Typescript::default(), "../src/bindings.ts")
            .expect("échec de l'export des bindings TypeScript");
    }
}
