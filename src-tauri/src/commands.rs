//! Commandes exposées au frontend. Elles restent fines : validation et SQL
//! sont dans `db`, la logique dans `services`.

use std::path::PathBuf;

use serde::Serialize;
use specta::Type;
use tauri::{AppHandle, State};
use tauri_plugin_autostart::ManagerExt;
use tauri_specta::Event;

use crate::{
    AppState,
    db::{
        activity, entries,
        models::{Project, ProjectInput, TimeEntry},
        now_ms, projects,
        idle::{self, IdlePeriod},
        settings::{self, ReminderSettings, Settings, TrayTitle},
    },
    error::{AppError, AppResult},
    events::{IdleChanged, ReviewRequested},
    notify, services, tray,
    services::{
        activity::UnassignedBlock,
        backup,
        entries::EntryInput,
        idle::IdleResolution,
        presence::{Presence, ReminderAction, ReminderView, SNOOZE},
        report::{self, ReportRow},
        timer,
    },
    platform, shortcut, windows,
};

#[tauri::command]
#[specta::specta]
pub async fn list_projects(state: State<'_, AppState>, include_archived: bool) -> AppResult<Vec<Project>> {
    projects::list(&state.db, include_archived).await
}

#[tauri::command]
#[specta::specta]
pub async fn recent_projects(state: State<'_, AppState>, limit: u32) -> AppResult<Vec<Project>> {
    projects::recent(&state.db, i64::from(limit)).await
}

#[tauri::command]
#[specta::specta]
pub async fn create_project(app: AppHandle, state: State<'_, AppState>, input: ProjectInput) -> AppResult<Project> {
    let project = projects::create(&state.db, input, now_ms()).await?;
    notify::projects_changed(&app);
    Ok(project)
}

#[tauri::command]
#[specta::specta]
pub async fn update_project(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
    input: ProjectInput,
) -> AppResult<Project> {
    let project = projects::update(&state.db, &id, input, now_ms()).await?;
    notify::projects_changed(&app);
    Ok(project)
}

/// Archiver un projet arrête son timer s'il tournait.
#[tauri::command]
#[specta::specta]
pub async fn set_project_archived(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
    archived: bool,
) -> AppResult<Project> {
    let now = now_ms();
    if archived && running_on(&state, &id).await? {
        timer::stop(&state.db, now).await?;
        notify::timer_changed(&app);
    }
    let project = projects::set_archived(&state.db, &id, archived, now).await?;
    notify::projects_changed(&app);
    Ok(project)
}

#[tauri::command]
#[specta::specta]
pub async fn delete_project(app: AppHandle, state: State<'_, AppState>, id: String) -> AppResult<()> {
    let now = now_ms();
    if running_on(&state, &id).await? {
        timer::stop(&state.db, now).await?;
        notify::timer_changed(&app);
    }
    projects::soft_delete(&state.db, &id, now).await?;
    notify::projects_changed(&app);
    Ok(())
}

async fn running_on(state: &AppState, project_id: &str) -> AppResult<bool> {
    Ok(entries::running(&state.db)
        .await?
        .is_some_and(|e| e.project_id == project_id))
}

#[tauri::command]
#[specta::specta]
pub async fn get_running(state: State<'_, AppState>) -> AppResult<Option<TimeEntry>> {
    entries::running(&state.db).await
}

#[tauri::command]
#[specta::specta]
pub async fn start_timer(app: AppHandle, state: State<'_, AppState>, project_id: String) -> AppResult<TimeEntry> {
    let entry = timer::start(&state.db, &project_id, now_ms()).await?;
    notify::timer_changed(&app);
    Ok(entry)
}

#[tauri::command]
#[specta::specta]
pub async fn stop_timer(app: AppHandle, state: State<'_, AppState>) -> AppResult<Option<TimeEntry>> {
    let stopped = timer::stop(&state.db, now_ms()).await?;
    notify::timer_changed(&app);
    Ok(stopped)
}

/// Entrées qui chevauchent `[from, to)` (millisecondes UTC).
#[tauri::command]
#[specta::specta]
pub async fn list_entries(state: State<'_, AppState>, from: i64, to: i64) -> AppResult<Vec<TimeEntry>> {
    entries::list_between(&state.db, from, to).await
}

#[tauri::command]
#[specta::specta]
pub async fn set_entry_note(app: AppHandle, state: State<'_, AppState>, id: String, note: String) -> AppResult<TimeEntry> {
    let entry = entries::set_note(&state.db, &id, &note, now_ms()).await?;
    notify::entries_changed(&app);
    Ok(entry)
}

#[tauri::command]
#[specta::specta]
pub async fn delete_entry(app: AppHandle, state: State<'_, AppState>, id: String) -> AppResult<()> {
    entries::soft_delete(&state.db, &id, now_ms()).await?;
    notify::timer_changed(&app);
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub async fn get_settings(state: State<'_, AppState>) -> AppResult<Settings> {
    settings::load(&state.db).await
}

#[tauri::command]
#[specta::specta]
pub async fn set_shortcut(app: AppHandle, state: State<'_, AppState>, accelerator: String) -> AppResult<Settings> {
    shortcut::change(&app, &state.db, &accelerator).await
}

#[tauri::command]
#[specta::specta]
pub fn get_autostart(app: AppHandle) -> AppResult<bool> {
    app.autolaunch().is_enabled().map_err(|e| AppError::Internal(e.to_string()))
}

#[tauri::command]
#[specta::specta]
pub fn set_autostart(app: AppHandle, enabled: bool) -> AppResult<bool> {
    let launcher = app.autolaunch();
    let result = if enabled { launcher.enable() } else { launcher.disable() };
    result.map_err(|e| AppError::Internal(format!("Lancement automatique : {e}")))?;
    Ok(enabled)
}

#[tauri::command]
#[specta::specta]
pub fn hide_palette(app: AppHandle) {
    windows::hide_palette(&app);
}

#[tauri::command]
#[specta::specta]
pub fn open_main_window(app: AppHandle) {
    windows::hide_palette(&app);
    windows::show_main(&app);
}

#[tauri::command]
#[specta::specta]
pub async fn set_reminder_settings(state: State<'_, AppState>, reminders: ReminderSettings) -> AppResult<Settings> {
    reminders.validate()?;
    let mut current = settings::load(&state.db).await?;
    current.reminders = reminders;
    settings::save(&state.db, &current).await?;
    Ok(current)
}

/// Absences en attente de décision, de la plus ancienne à la plus récente.
#[tauri::command]
#[specta::specta]
pub async fn list_pending_idle(state: State<'_, AppState>) -> AppResult<Vec<IdlePeriod>> {
    idle::pending(&state.db).await
}

#[tauri::command]
#[specta::specta]
pub async fn resolve_idle(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
    resolution: IdleResolution,
) -> AppResult<()> {
    services::idle::resolve(&state.db, &id, &resolution, now_ms()).await?;
    let _ = IdleChanged.emit(&app);
    notify::timer_changed(&app);
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn get_reminder(presence: State<'_, Presence>) -> Option<ReminderView> {
    presence.current.lock().unwrap().clone()
}

#[tauri::command]
#[specta::specta]
pub async fn reminder_action(
    app: AppHandle,
    state: State<'_, AppState>,
    presence: State<'_, Presence>,
    action: ReminderAction,
) -> AppResult<()> {
    windows::hide_reminder(&app);
    presence.current.lock().unwrap().take();
    match action {
        ReminderAction::Dismiss => {}
        ReminderAction::Later => presence.reminders.lock().unwrap().snooze_no_timer(now_ms() + SNOOZE),
        ReminderAction::Switch => windows::toggle_palette(&app),
        ReminderAction::Stop => {
            timer::stop(&state.db, now_ms()).await?;
            notify::timer_changed(&app);
        }
        ReminderAction::Review => {
            windows::show_main(&app);
            let _ = ReviewRequested.emit(&app);
        }
    }
    Ok(())
}

// --- Revue de journée -------------------------------------------------------------

/// Crée (`id` absent) ou modifie une entrée ; les entrées recouvertes sont ajustées.
#[tauri::command]
#[specta::specta]
pub async fn save_entry(app: AppHandle, state: State<'_, AppState>, id: Option<String>, input: EntryInput) -> AppResult<TimeEntry> {
    let entry = services::entries::save(&state.db, id.as_deref(), input, now_ms()).await?;
    notify::timer_changed(&app);
    Ok(entry)
}

#[tauri::command]
#[specta::specta]
pub async fn split_entry(app: AppHandle, state: State<'_, AppState>, id: String, at: i64) -> AppResult<()> {
    services::entries::split(&state.db, &id, at, now_ms()).await?;
    notify::timer_changed(&app);
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub async fn merge_entries(app: AppHandle, state: State<'_, AppState>, ids: Vec<String>) -> AppResult<TimeEntry> {
    let entry = services::entries::merge(&state.db, &ids, now_ms()).await?;
    notify::timer_changed(&app);
    Ok(entry)
}

/// Temps actif sans projet sur `[from, to)`.
#[tauri::command]
#[specta::specta]
pub async fn list_unassigned(state: State<'_, AppState>, from: i64, to: i64) -> AppResult<Vec<UnassignedBlock>> {
    services::activity::unassigned(&state.db, from, to, now_ms()).await
}

// --- Projets : clients ------------------------------------------------------------

/// Renomme un client sur tous ses projets. Renvoie le nombre de projets modifiés.
#[tauri::command]
#[specta::specta]
pub async fn rename_client(app: AppHandle, state: State<'_, AppState>, from: String, to: String) -> AppResult<u32> {
    let count = projects::rename_client(&state.db, &from, &to, now_ms()).await?;
    notify::projects_changed(&app);
    Ok(count as u32)
}

// --- Rapports ---------------------------------------------------------------------

#[tauri::command]
#[specta::specta]
pub async fn get_report(state: State<'_, AppState>, from: i64, to: i64, rounding: u32) -> AppResult<Vec<ReportRow>> {
    report::rows(&state.db, from, to, rounding, now_ms()).await
}

/// Écrit le CSV à `path` (choisi par l'utilisateur). Renvoie le nombre de lignes.
#[tauri::command]
#[specta::specta]
pub async fn export_csv(state: State<'_, AppState>, from: i64, to: i64, rounding: u32, path: String) -> AppResult<u32> {
    let count = report::export_csv(&state.db, from, to, rounding, &PathBuf::from(path), now_ms()).await?;
    Ok(count as u32)
}

#[tauri::command]
#[specta::specta]
pub async fn set_rounding(state: State<'_, AppState>, rounding: u32) -> AppResult<Settings> {
    report::check_rounding(rounding)?;
    let mut current = settings::load(&state.db).await?;
    current.rounding_min = rounding;
    settings::save(&state.db, &current).await?;
    Ok(current)
}

#[tauri::command]
#[specta::specta]
pub async fn set_tray_title(app: AppHandle, state: State<'_, AppState>, mode: TrayTitle) -> AppResult<Settings> {
    let mut current = settings::load(&state.db).await?;
    current.tray_title = mode;
    settings::save(&state.db, &current).await?;
    tray::refresh(&app);
    Ok(current)
}

// --- Suivi d'activité -------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ActivityStatus {
    pub enabled: bool,
    /// macOS : permission « Enregistrement de l'écran » accordée (titres de fenêtres).
    pub titles_allowed: bool,
}

#[tauri::command]
#[specta::specta]
pub async fn activity_status(state: State<'_, AppState>) -> AppResult<ActivityStatus> {
    let enabled = settings::load(&state.db).await?.activity_enabled;
    Ok(ActivityStatus { enabled, titles_allowed: platform::window::titles_allowed() })
}

#[tauri::command]
#[specta::specta]
pub async fn set_activity_enabled(app: AppHandle, state: State<'_, AppState>, enabled: bool) -> AppResult<Settings> {
    let current = services::activity::set_enabled(&state.db, enabled).await?;
    notify::settings_changed(&app);
    Ok(current)
}

#[tauri::command]
#[specta::specta]
pub fn request_activity_permission() {
    platform::window::request_titles();
}

#[tauri::command]
#[specta::specta]
pub async fn clear_activity(state: State<'_, AppState>) -> AppResult<()> {
    activity::clear(&state.db).await
}

// --- Sauvegardes ------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct BackupInfo {
    pub dir: String,
    pub custom: bool,
    /// Nom du fichier de la sauvegarde la plus récente.
    pub latest: Option<String>,
}

#[tauri::command]
#[specta::specta]
pub async fn backup_info(app: AppHandle, state: State<'_, AppState>) -> AppResult<BackupInfo> {
    let custom = settings::load(&state.db).await?.backup_dir;
    let dir = backup::dir(&app, custom.as_deref())?;
    Ok(BackupInfo { dir: dir.to_string_lossy().into_owned(), custom: custom.is_some(), latest: backup::latest(&dir) })
}

#[tauri::command]
#[specta::specta]
pub async fn backup_now(app: AppHandle, state: State<'_, AppState>) -> AppResult<String> {
    let custom = settings::load(&state.db).await?.backup_dir;
    let dir = backup::dir(&app, custom.as_deref())?;
    let path = backup::run(&state.db, &dir, chrono::Local::now().date_naive()).await?;
    Ok(path.to_string_lossy().into_owned())
}

/// `None` : revenir au dossier par défaut.
#[tauri::command]
#[specta::specta]
pub async fn set_backup_dir(state: State<'_, AppState>, dir: Option<String>) -> AppResult<Settings> {
    let mut current = settings::load(&state.db).await?;
    current.backup_dir = dir.filter(|d| !d.trim().is_empty());
    settings::save(&state.db, &current).await?;
    Ok(current)
}

#[tauri::command]
#[specta::specta]
pub async fn reveal_backups(app: AppHandle, state: State<'_, AppState>) -> AppResult<()> {
    let custom = settings::load(&state.db).await?.backup_dir;
    let dir = backup::dir(&app, custom.as_deref())?;
    std::fs::create_dir_all(&dir).map_err(|e| AppError::Internal(e.to_string()))?;
    tauri_plugin_opener::open_path(&dir, None::<&str>).map_err(|e| AppError::Internal(e.to_string()))
}
