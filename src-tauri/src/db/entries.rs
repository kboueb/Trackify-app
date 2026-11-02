use sqlx::SqliteExecutor;

use super::{
    models::{EntrySource, TimeEntry},
    new_id,
};
use crate::error::{AppError, AppResult};

macro_rules! columns {
    () => {
        "id, project_id, started_at, ended_at, note, source, created_at, updated_at"
    };
}

pub async fn running(db: impl SqliteExecutor<'_>) -> AppResult<Option<TimeEntry>> {
    let sql = concat!("SELECT ", columns!(), " FROM time_entries WHERE ended_at IS NULL AND deleted_at IS NULL");
    Ok(sqlx::query_as(sql).fetch_optional(db).await?)
}

pub async fn get(db: impl SqliteExecutor<'_>, id: &str) -> AppResult<TimeEntry> {
    let sql = concat!("SELECT ", columns!(), " FROM time_entries WHERE id = ? AND deleted_at IS NULL");
    sqlx::query_as(sql)
        .bind(id)
        .fetch_optional(db)
        .await?
        .ok_or_else(|| AppError::NotFound("Entrée introuvable.".into()))
}

/// Entrées qui chevauchent `[from, to)`, y compris celle en cours.
pub async fn list_between(db: impl SqliteExecutor<'_>, from: i64, to: i64) -> AppResult<Vec<TimeEntry>> {
    let sql = concat!(
        "SELECT ", columns!(), " FROM time_entries
         WHERE deleted_at IS NULL AND started_at < ? AND (ended_at IS NULL OR ended_at > ?)
         ORDER BY started_at"
    );
    Ok(sqlx::query_as(sql).bind(to).bind(from).fetch_all(db).await?)
}

pub async fn insert_running(
    db: impl SqliteExecutor<'_>,
    project_id: &str,
    started_at: i64,
    note: &str,
    source: EntrySource,
) -> AppResult<TimeEntry> {
    let sql = concat!(
        "INSERT INTO time_entries (id, project_id, started_at, ended_at, note, source, created_at, updated_at)
         VALUES (?, ?, ?, NULL, ?, ?, ?, ?) RETURNING ", columns!(), ""
    );
    Ok(sqlx::query_as(sql)
        .bind(new_id())
        .bind(project_id)
        .bind(started_at)
        .bind(note)
        .bind(source)
        .bind(started_at)
        .bind(started_at)
        .fetch_one(db)
        .await?)
}

/// Insère une entrée quelconque (terminée ou en cours).
pub async fn insert(
    db: impl SqliteExecutor<'_>,
    project_id: &str,
    started_at: i64,
    ended_at: Option<i64>,
    note: &str,
    source: EntrySource,
    now: i64,
) -> AppResult<TimeEntry> {
    Ok(sqlx::query_as(concat!(
        "INSERT INTO time_entries (id, project_id, started_at, ended_at, note, source, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?) RETURNING ",
        columns!()
    ))
    .bind(new_id())
    .bind(project_id)
    .bind(started_at)
    .bind(ended_at)
    .bind(note)
    .bind(source)
    .bind(now)
    .bind(now)
    .fetch_one(db)
    .await?)
}

pub async fn set_ended(db: impl SqliteExecutor<'_>, id: &str, ended_at: i64, now: i64) -> AppResult<TimeEntry> {
    let sql = concat!("UPDATE time_entries SET ended_at = ?, updated_at = ? WHERE id = ? RETURNING ", columns!(), "");
    Ok(sqlx::query_as(sql).bind(ended_at).bind(now).bind(id).fetch_one(db).await?)
}

pub async fn set_note(db: impl SqliteExecutor<'_>, id: &str, note: &str, now: i64) -> AppResult<TimeEntry> {
    let sql = concat!(
        "UPDATE time_entries SET note = ?, updated_at = ?
         WHERE id = ? AND deleted_at IS NULL RETURNING ", columns!(), ""
    );
    sqlx::query_as(sql)
        .bind(note.trim())
        .bind(now)
        .bind(id)
        .fetch_optional(db)
        .await?
        .ok_or_else(|| AppError::NotFound("Entrée introuvable.".into()))
}

pub async fn soft_delete(db: impl SqliteExecutor<'_>, id: &str, now: i64) -> AppResult<()> {
    let res = sqlx::query("UPDATE time_entries SET deleted_at = ?, updated_at = ? WHERE id = ? AND deleted_at IS NULL")
        .bind(now)
        .bind(now)
        .bind(id)
        .execute(db)
        .await?;
    if res.rows_affected() == 0 {
        return Err(AppError::NotFound("Entrée introuvable.".into()));
    }
    Ok(())
}
