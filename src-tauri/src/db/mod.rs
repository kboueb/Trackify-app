//! Accès à la base SQLite. Tout le SQL de l'application vit dans ce module.

pub mod activity;
pub mod entries;
pub mod idle;
pub mod models;
pub mod projects;
pub mod settings;

use std::{path::Path, time::Duration};

use sqlx::{
    SqlitePool,
    migrate::Migrator,
    sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions},
};

use crate::error::AppResult;

static MIGRATOR: Migrator = sqlx::migrate!("./migrations");

/// Ouvre (ou crée) la base sur disque et applique les migrations.
pub async fn open(path: &Path) -> AppResult<SqlitePool> {
    let options = SqliteConnectOptions::new()
        .filename(path)
        .create_if_missing(true)
        .journal_mode(SqliteJournalMode::Wal)
        .foreign_keys(true)
        .busy_timeout(Duration::from_secs(5));
    let pool = SqlitePoolOptions::new()
        .max_connections(4)
        .connect_with(options)
        .await?;
    MIGRATOR.run(&pool).await?;
    Ok(pool)
}

/// Base en mémoire pour les tests (une seule connexion, jamais recyclée).
#[cfg(test)]
pub async fn open_in_memory() -> SqlitePool {
    use std::str::FromStr;
    let options = SqliteConnectOptions::from_str("sqlite::memory:")
        .unwrap()
        .foreign_keys(true);
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .idle_timeout(None)
        .max_lifetime(None)
        .connect_with(options)
        .await
        .unwrap();
    MIGRATOR.run(&pool).await.unwrap();
    pool
}

/// Horodatage courant en millisecondes UTC.
pub fn now_ms() -> i64 {
    chrono::Utc::now().timestamp_millis()
}

/// Nouvel identifiant (UUID v7 : triable par date de création).
pub fn new_id() -> String {
    uuid::Uuid::now_v7().to_string()
}
