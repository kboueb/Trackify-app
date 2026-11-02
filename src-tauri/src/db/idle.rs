use serde::Serialize;
use specta::Type;
use sqlx::SqliteExecutor;

use super::new_id;
use crate::error::AppResult;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Type, sqlx::Type)]
#[serde(rename_all = "lowercase")]
#[sqlx(rename_all = "lowercase")]
pub enum IdleKind {
    /// Pas d'activité clavier/souris, ou machine en veille.
    Idle,
    /// Trackify était fermé alors qu'un timer tournait.
    Closed,
}

#[derive(Debug, Clone, Serialize, Type, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct IdlePeriod {
    pub id: String,
    pub entry_id: String,
    /// Projet du timer qui tournait pendant l'absence.
    pub project_id: String,
    pub kind: IdleKind,
    pub started_at: i64,
    pub ended_at: i64,
}

macro_rules! select {
    () => {
        "SELECT i.id, i.entry_id, e.project_id, i.kind, i.started_at, i.ended_at
         FROM idle_periods i JOIN time_entries e ON e.id = i.entry_id"
    };
}

pub async fn insert(
    db: impl SqliteExecutor<'_>,
    entry_id: &str,
    kind: IdleKind,
    started_at: i64,
    ended_at: i64,
    now: i64,
) -> AppResult<String> {
    let id = new_id();
    sqlx::query(
        "INSERT INTO idle_periods (id, entry_id, kind, started_at, ended_at, created_at)
         VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(entry_id)
    .bind(kind)
    .bind(started_at)
    .bind(ended_at)
    .bind(now)
    .execute(db)
    .await?;
    Ok(id)
}

pub async fn pending(db: impl SqliteExecutor<'_>) -> AppResult<Vec<IdlePeriod>> {
    Ok(sqlx::query_as(concat!(select!(), " WHERE i.resolution IS NULL ORDER BY i.started_at"))
    .fetch_all(db)
    .await?)
}

/// Renvoie la période si elle est encore en attente.
pub async fn get_pending(db: impl SqliteExecutor<'_>, id: &str) -> AppResult<Option<IdlePeriod>> {
    Ok(sqlx::query_as(concat!(select!(), " WHERE i.id = ? AND i.resolution IS NULL"))
    .bind(id)
    .fetch_optional(db)
    .await?)
}

pub async fn set_resolution(db: impl SqliteExecutor<'_>, id: &str, resolution: &str, now: i64) -> AppResult<()> {
    sqlx::query("UPDATE idle_periods SET resolution = ?, resolved_at = ? WHERE id = ?")
        .bind(resolution)
        .bind(now)
        .bind(id)
        .execute(db)
        .await?;
    Ok(())
}
