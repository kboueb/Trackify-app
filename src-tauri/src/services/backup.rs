//! Sauvegarde quotidienne de la base : une copie par jour, on garde les plus récentes.

use std::{
    path::{Path, PathBuf},
    time::Duration,
};

use chrono::{Local, NaiveDate};
use sqlx::SqlitePool;
use tauri::{AppHandle, Manager};

use crate::{
    AppState,
    db::settings,
    error::{AppError, AppResult},
};

const KEEP: usize = 7;
const CHECK_EVERY: Duration = Duration::from_secs(60 * 60);
const PREFIX: &str = "trackify-";

/// Dossier choisi dans les réglages, sinon `backups/` à côté de la base.
pub fn dir(app: &AppHandle, custom: Option<&str>) -> AppResult<PathBuf> {
    match custom.filter(|d| !d.trim().is_empty()) {
        Some(d) => Ok(PathBuf::from(d)),
        None => Ok(app
            .path()
            .app_data_dir()
            .map_err(|e| AppError::Internal(e.to_string()))?
            .join("backups")),
    }
}

fn file_for(dir: &Path, day: NaiveDate) -> PathBuf {
    dir.join(format!("{PREFIX}{}.db", day.format("%Y-%m-%d")))
}

/// Copie cohérente de la base (même pendant l'écriture) dans `dir`, puis ménage.
pub async fn run(pool: &SqlitePool, dir: &Path, day: NaiveDate) -> AppResult<PathBuf> {
    let io = |e: std::io::Error| AppError::Internal(format!("Sauvegarde impossible : {e}"));
    std::fs::create_dir_all(dir).map_err(io)?;
    let target = file_for(dir, day);
    if target.exists() {
        std::fs::remove_file(&target).map_err(io)?;
    }
    sqlx::query("VACUUM INTO ?")
        .bind(target.to_string_lossy().into_owned())
        .execute(pool)
        .await?;
    if !target.exists() {
        return Err(AppError::Internal(format!("Sauvegarde non écrite : {}", target.display())));
    }
    prune(dir).map_err(io)?;
    Ok(target)
}

/// Nom de la sauvegarde la plus récente de `dir`.
pub fn latest(dir: &Path) -> Option<String> {
    list(dir).ok()?.last()?.file_name()?.to_str().map(str::to_string)
}

fn list(dir: &Path) -> std::io::Result<Vec<PathBuf>> {
    let mut files: Vec<PathBuf> = std::fs::read_dir(dir)?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.starts_with(PREFIX) && n.ends_with(".db"))
        })
        .collect();
    files.sort();
    Ok(files)
}

/// Ne garde que les `KEEP` sauvegardes les plus récentes (le nom contient la date).
fn prune(dir: &Path) -> std::io::Result<()> {
    let files = list(dir)?;
    let excess = files.len().saturating_sub(KEEP);
    for old in &files[..excess] {
        std::fs::remove_file(old)?;
    }
    Ok(())
}

/// Vérifie toutes les heures qu'une sauvegarde du jour existe.
pub fn start(app: &AppHandle) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let mut tick = tokio::time::interval(CHECK_EVERY);
        loop {
            tick.tick().await;
            if let Err(e) = daily(&app).await {
                eprintln!("sauvegarde : {e}");
            }
        }
    });
}

async fn daily(app: &AppHandle) -> AppResult<()> {
    let db = &app.state::<AppState>().db;
    let settings = settings::load(db).await?;
    let dir = dir(app, settings.backup_dir.as_deref())?;
    let today = Local::now().date_naive();
    if !file_for(&dir, today).exists() {
        run(db, &dir, today).await?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db;

    #[tokio::test]
    async fn sauvegarde_et_menage() {
        let dir = std::env::temp_dir().join(format!("trackify-backups-{}", db::new_id()));
        // Base sur disque : `VACUUM INTO` n'écrit rien depuis une base en mémoire.
        let src = dir.with_extension("source.db");
        let pool = db::open(&src).await.unwrap();
        let day = |d: u32| NaiveDate::from_ymd_opt(2026, 3, d).unwrap();
        for d in 1..=9 {
            run(&pool, &dir, day(d)).await.unwrap();
        }
        // Une deuxième sauvegarde le même jour remplace la première.
        let last = run(&pool, &dir, day(9)).await.unwrap();
        assert!(last.ends_with("trackify-2026-03-09.db"));
        let mut names: Vec<String> = std::fs::read_dir(&dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().into_string().unwrap())
            .collect();
        names.sort();
        assert_eq!(names.len(), KEEP);
        assert_eq!(names[0], "trackify-2026-03-03.db");
        assert_eq!(latest(&dir).as_deref(), Some("trackify-2026-03-09.db"));
        let restored = db::open(&last).await.unwrap();
        assert!(db::projects::list(&restored, true).await.unwrap().is_empty());
        restored.close().await;
        pool.close().await;
        let _ = std::fs::remove_dir_all(&dir);
        for suffix in ["", "-wal", "-shm"] {
            let _ = std::fs::remove_file(format!("{}{suffix}", src.display()));
        }
    }
}
