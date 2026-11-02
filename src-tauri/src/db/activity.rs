use serde::Serialize;
use specta::Type;
use sqlx::SqliteExecutor;

use super::new_id;
use crate::error::AppResult;

macro_rules! columns {
    () => {
        "id, app, title, started_at, ended_at"
    };
}

#[derive(Debug, Clone, Serialize, Type, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct ActivitySegment {
    pub id: String,
    pub app: String,
    pub title: String,
    pub started_at: i64,
    pub ended_at: i64,
}

pub async fn last(db: impl SqliteExecutor<'_>) -> AppResult<Option<ActivitySegment>> {
    let sql = concat!("SELECT ", columns!(), " FROM activity_segments ORDER BY ended_at DESC, started_at DESC LIMIT 1");
    Ok(sqlx::query_as(sql).fetch_optional(db).await?)
}

pub async fn insert(db: impl SqliteExecutor<'_>, app: &str, title: &str, at: i64) -> AppResult<()> {
    sqlx::query("INSERT INTO activity_segments (id, app, title, started_at, ended_at) VALUES (?, ?, ?, ?, ?)")
        .bind(new_id())
        .bind(app)
        .bind(title)
        .bind(at)
        .bind(at)
        .execute(db)
        .await?;
    Ok(())
}

pub async fn extend(db: impl SqliteExecutor<'_>, id: &str, ended_at: i64) -> AppResult<()> {
    sqlx::query("UPDATE activity_segments SET ended_at = ? WHERE id = ?")
        .bind(ended_at)
        .bind(id)
        .execute(db)
        .await?;
    Ok(())
}

/// Segments qui chevauchent `[from, to)`.
pub async fn list_between(db: impl SqliteExecutor<'_>, from: i64, to: i64) -> AppResult<Vec<ActivitySegment>> {
    let sql = concat!(
        "SELECT ", columns!(), " FROM activity_segments
         WHERE started_at < ? AND ended_at > ? ORDER BY started_at"
    );
    Ok(sqlx::query_as(sql).bind(to).bind(from).fetch_all(db).await?)
}

/// Supprime définitivement ce qui a fini avant `before`. Renvoie le nombre de segments retirés.
pub async fn purge_before(db: impl SqliteExecutor<'_>, before: i64) -> AppResult<u64> {
    let res = sqlx::query("DELETE FROM activity_segments WHERE ended_at < ?")
        .bind(before)
        .execute(db)
        .await?;
    Ok(res.rows_affected())
}

/// Efface tout l'historique d'activité (bouton dans les réglages).
pub async fn clear(db: impl SqliteExecutor<'_>) -> AppResult<()> {
    sqlx::query("DELETE FROM activity_segments").execute(db).await?;
    Ok(())
}
