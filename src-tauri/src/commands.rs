//! Commandes exposées au frontend. Elles restent fines : validation et SQL
//! sont dans `db`, la logique dans `services`.

use tauri::{AppHandle, State};
use tauri_plugin_autostart::ManagerExt;
use tauri_specta::Event;

use crate::{
    AppState,
    db::{
        entries,
        models::{Project, ProjectInput, TimeEntry},
        now_ms, projects,
        idle::{self, IdlePeriod},
        settings::{self, ReminderSettings, Settings},
    },
    error::{AppError, AppResult},
    events::IdleChanged,
    notify, services,
    services::{
        idle::IdleResolution,
        presence::{Presence, ReminderAction, ReminderView, SNOOZE},
        timer,
    },
    shortcut, windows,
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
    }
    Ok(())
}
