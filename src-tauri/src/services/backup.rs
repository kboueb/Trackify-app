//! Sauvegarde quotidienne de la base : une copie par jour, on garde les plus récentes.
//!
//! Restauration en deux temps, car la base ouverte ne peut pas être remplacée à chaud :
//! la sauvegarde choisie est vérifiée puis copiée en `restore-pending.db`, l'app redémarre,
//! et [`apply_pending`] fait l'échange au démarrage, avant l'ouverture de la base.
//! L'ancienne base n'est jamais supprimée : elle est gardée sous `avant-restauration-<date>.db`.

use std::{
    path::{Path, PathBuf},
    time::Duration,
};

use chrono::{Local, NaiveDate};
use serde::Serialize;
use specta::Type;
use sqlx::{
    ConnectOptions, SqlitePool,
    sqlite::SqliteConnectOptions,
};
use tauri::{AppHandle, Manager};

use crate::{
    AppState,
    db::{DB_FILE, settings},
    error::{AppError, AppResult},
};

const KEEP: usize = 7;
const CHECK_EVERY: Duration = Duration::from_secs(60 * 60);
const PREFIX: &str = "trackify-";
/// Sauvegarde validée, en attente d'échange au prochain démarrage.
const PENDING: &str = "restore-pending.db";
/// Dossier des sauvegardes, recopié hors de la base : si la base est illisible,
/// c'est le seul moyen de retrouver un dossier personnalisé (iCloud Drive…).
const DIR_MEMO: &str = "backup-dir.txt";

/// Sauvegarde proposée à la restauration.
#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct BackupFile {
    pub name: String,
    /// Jour de la sauvegarde, `AAAA-MM-JJ`.
    pub day: String,
    pub size_kb: u32,
}

pub fn data_dir(app: &AppHandle) -> AppResult<PathBuf> {
    app.path().app_data_dir().map_err(|e| AppError::Internal(e.to_string()))
}

/// Dossier choisi dans les réglages, sinon `backups/` à côté de la base.
pub fn dir(app: &AppHandle, custom: Option<&str>) -> AppResult<PathBuf> {
    match custom.filter(|d| !d.trim().is_empty()) {
        Some(d) => Ok(PathBuf::from(d)),
        None => Ok(data_dir(app)?.join("backups")),
    }
}

/// Note le dossier des sauvegardes à côté de la base (voir [`DIR_MEMO`]).
pub fn remember_dir(app: &AppHandle, dir: &Path) {
    if let Ok(data) = data_dir(app) {
        let _ = std::fs::write(data.join(DIR_MEMO), dir.to_string_lossy().as_bytes());
    }
}

/// Dossiers où chercher une sauvegarde sans pouvoir lire la base : celui par défaut,
/// et le dernier dossier personnalisé connu.
pub fn fallback_dirs(data_dir: &Path) -> Vec<PathBuf> {
    let mut dirs = vec![data_dir.join("backups")];
    if let Ok(memo) = std::fs::read_to_string(data_dir.join(DIR_MEMO)) {
        let custom = PathBuf::from(memo.trim());
        if !memo.trim().is_empty() && !dirs.contains(&custom) {
            dirs.push(custom);
        }
    }
    dirs
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

/// Sauvegardes de `dir`, la plus récente en premier.
pub fn files(dir: &Path) -> Vec<BackupFile> {
    let mut out: Vec<BackupFile> = list(dir)
        .unwrap_or_default()
        .iter()
        .filter_map(|p| {
            let name = p.file_name()?.to_str()?.to_string();
            let day = name.strip_prefix(PREFIX)?.strip_suffix(".db")?.to_string();
            let size_kb = std::fs::metadata(p).map(|m| (m.len() / 1024) as u32).unwrap_or(0);
            Some(BackupFile { name, day, size_kb })
        })
        .collect();
    out.reverse();
    out
}

/// Chemin d'une sauvegarde de `dir` désignée par son nom, sans sortir du dossier.
pub fn resolve(dir: &Path, name: &str) -> AppResult<PathBuf> {
    list(dir)
        .unwrap_or_default()
        .into_iter()
        .find(|p| p.file_name().and_then(|n| n.to_str()) == Some(name))
        .ok_or_else(|| AppError::NotFound(format!("Sauvegarde introuvable : {name}")))
}

/// Vérifie qu'un fichier est une base Trackify intacte, sans jamais l'écrire.
pub async fn verify(path: &Path) -> AppResult<()> {
    let unreadable = |e: sqlx::Error| AppError::Validation(format!("Sauvegarde illisible ({}) : {e}", path.display()));
    let mut conn = SqliteConnectOptions::new()
        .filename(path)
        .read_only(true)
        .immutable(true)
        .connect()
        .await
        .map_err(unreadable)?;
    let check: String = sqlx::query_scalar("PRAGMA integrity_check")
        .fetch_one(&mut conn)
        .await
        .map_err(unreadable)?;
    if check != "ok" {
        return Err(AppError::Validation(format!("Sauvegarde endommagée : {check}")));
    }
    let tables: i64 =
        sqlx::query_scalar("SELECT count(*) FROM sqlite_master WHERE type = 'table' AND name IN ('projects', 'time_entries')")
            .fetch_one(&mut conn)
            .await
            .map_err(unreadable)?;
    if tables != 2 {
        return Err(AppError::Validation("Ce fichier n'est pas une sauvegarde Trackify.".into()));
    }
    Ok(())
}

/// Sauvegarde intacte la plus récente parmi `dirs`.
pub async fn latest_valid(dirs: &[PathBuf]) -> Option<(PathBuf, String)> {
    let mut candidates: Vec<(String, PathBuf)> = dirs
        .iter()
        .flat_map(|d| files(d).into_iter().map(move |f| (f.day, d.join(f.name))))
        .collect();
    candidates.sort_by(|a, b| b.0.cmp(&a.0));
    for (day, path) in candidates {
        if verify(&path).await.is_ok() {
            return Some((path, day));
        }
    }
    None
}

/// Vérifie la sauvegarde puis la prépare pour le prochain démarrage.
pub async fn stage_restore(data_dir: &Path, backup: &Path) -> AppResult<()> {
    verify(backup).await?;
    std::fs::copy(backup, data_dir.join(PENDING))
        .map_err(|e| AppError::Internal(format!("Restauration impossible : {e}")))?;
    Ok(())
}

fn sidecar(db: &Path, suffix: &str) -> PathBuf {
    PathBuf::from(format!("{}{suffix}", db.display()))
}

/// Au démarrage, avant d'ouvrir la base : met en place une restauration préparée.
/// Renvoie le fichier sous lequel l'ancienne base est gardée, s'il y en avait une.
pub fn apply_pending(data_dir: &Path, stamp: &str) -> std::io::Result<Option<PathBuf>> {
    let pending = data_dir.join(PENDING);
    if !pending.exists() {
        return Ok(None);
    }
    let db = data_dir.join(DB_FILE);
    let kept = data_dir.join(format!("avant-restauration-{stamp}.db"));
    let had_db = db.exists();
    if had_db {
        std::fs::rename(&db, &kept)?;
    }
    // Le journal WAL suit sa base : il contient peut-être les dernières écritures.
    for suffix in ["-wal", "-shm"] {
        let file = sidecar(&db, suffix);
        if file.exists() {
            if had_db {
                std::fs::rename(&file, sidecar(&kept, suffix))?;
            } else {
                std::fs::remove_file(&file)?;
            }
        }
    }
    if let Err(e) = std::fs::rename(&pending, &db) {
        // Échec du dernier pas : remettre l'ancienne base, sinon l'app repartirait d'une base vide.
        if had_db {
            let _ = std::fs::rename(&kept, &db);
            for suffix in ["-wal", "-shm"] {
                let _ = std::fs::rename(sidecar(&kept, suffix), sidecar(&db, suffix));
            }
        }
        return Err(e);
    }
    Ok(had_db.then_some(kept))
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
    remember_dir(app, &dir);
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

    /// Dossier temporaire avec une vraie base sur disque et une sauvegarde du 2026-03-09.
    async fn fixture() -> (PathBuf, PathBuf, PathBuf) {
        let root = std::env::temp_dir().join(format!("trackify-restore-{}", db::new_id()));
        let backups = root.join("backups");
        std::fs::create_dir_all(&backups).unwrap();
        let pool = db::open(&root.join(DB_FILE)).await.unwrap();
        let backup = run(&pool, &backups, NaiveDate::from_ymd_opt(2026, 3, 9).unwrap()).await.unwrap();
        pool.close().await;
        (root, backups, backup)
    }

    #[tokio::test]
    async fn verification_des_sauvegardes() {
        let (root, backups, backup) = fixture().await;
        verify(&backup).await.unwrap();

        let garbage = backups.join("trackify-2026-03-10.db");
        std::fs::write(&garbage, b"pas une base SQLite, juste du texte assez long pour un en-tete").unwrap();
        assert!(verify(&garbage).await.is_err());

        // Base SQLite valide, mais sans les tables de Trackify.
        let other = root.join("autre.db");
        let pool = sqlx::SqlitePool::connect(&format!("sqlite://{}?mode=rwc", other.display())).await.unwrap();
        sqlx::query("CREATE TABLE t (x INTEGER)").execute(&pool).await.unwrap();
        pool.close().await;
        assert!(verify(&other).await.is_err());

        // La plus récente est corrompue : on retombe sur la précédente.
        let (path, day) = latest_valid(std::slice::from_ref(&backups)).await.unwrap();
        assert_eq!(path, backup);
        assert_eq!(day, "2026-03-09");
        assert_eq!(files(&backups)[0].day, "2026-03-10");

        assert!(resolve(&backups, "trackify-2026-03-09.db").is_ok());
        assert!(resolve(&backups, "../trackify.db").is_err());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[tokio::test]
    async fn restauration_garde_l_ancienne_base() {
        let (root, _backups, backup) = fixture().await;
        // Données ajoutées après la sauvegarde : elles doivent survivre dans l'ancienne base.
        let pool = db::open(&root.join(DB_FILE)).await.unwrap();
        sqlx::query("INSERT INTO projects (id, name, client, color, created_at, updated_at) VALUES ('p', 'Après', '', '#000000', 0, 0)")
            .execute(&pool)
            .await
            .unwrap();
        pool.close().await;

        assert_eq!(apply_pending(&root, "x").unwrap(), None, "rien à faire sans restauration préparée");
        stage_restore(&root, &backup).await.unwrap();
        let kept = apply_pending(&root, "2026-03-10-0900").unwrap().unwrap();
        assert!(!root.join(PENDING).exists());

        let restored = db::open(&root.join(DB_FILE)).await.unwrap();
        assert!(db::projects::list(&restored, true).await.unwrap().is_empty());
        restored.close().await;
        let old = db::open(&kept).await.unwrap();
        assert_eq!(db::projects::list(&old, true).await.unwrap().len(), 1);
        old.close().await;
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn dossiers_de_secours() {
        let root = std::env::temp_dir().join(format!("trackify-memo-{}", db::new_id()));
        std::fs::create_dir_all(&root).unwrap();
        assert_eq!(fallback_dirs(&root), vec![root.join("backups")]);
        std::fs::write(root.join(DIR_MEMO), "/Volumes/iCloud/Trackify\n").unwrap();
        assert_eq!(fallback_dirs(&root), vec![root.join("backups"), PathBuf::from("/Volumes/iCloud/Trackify")]);
        let _ = std::fs::remove_dir_all(&root);
    }
}
