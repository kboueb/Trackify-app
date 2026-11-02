//! Timer : un seul actif à la fois. L'état vit uniquement en base (`ended_at IS NULL`),
//! le temps écoulé est toujours recalculé depuis `started_at`.

use sqlx::{SqliteConnection, SqlitePool};

use crate::{
    db::{
        entries,
        models::{EntrySource, TimeEntry},
        projects,
    },
    error::{AppError, AppResult},
};

/// En dessous de cette durée, une entrée arrêtée est jetée (clic par erreur, bascule immédiate).
pub const MIN_ENTRY_MS: i64 = 5_000;

/// Démarre un timer sur `project_id`, en arrêtant d'abord celui en cours.
/// Si ce projet tourne déjà, ne fait rien et renvoie l'entrée en cours.
pub async fn start(pool: &SqlitePool, project_id: &str, now: i64) -> AppResult<TimeEntry> {
    let mut tx = pool.begin().await?;
    let project = projects::get(&mut *tx, project_id).await?;
    if project.archived {
        return Err(AppError::Validation(format!(
            "Le projet « {} » est archivé.",
            project.name
        )));
    }
    if let Some(current) = entries::running(&mut *tx).await? {
        if current.project_id == project_id {
            tx.commit().await?;
            return Ok(current);
        }
        close(&mut tx, current, now).await?;
    }
    let entry = entries::insert_running(&mut *tx, project_id, now, "", EntrySource::Timer).await?;
    projects::touch_last_used(&mut *tx, project_id, now).await?;
    tx.commit().await?;
    Ok(entry)
}

/// Arrête le timer en cours. Renvoie l'entrée terminée, ou `None` s'il n'y avait
/// rien à arrêter (ou si l'entrée était trop courte pour être gardée).
pub async fn stop(pool: &SqlitePool, now: i64) -> AppResult<Option<TimeEntry>> {
    let mut tx = pool.begin().await?;
    let stopped = match entries::running(&mut *tx).await? {
        Some(current) => close(&mut tx, current, now).await?,
        None => None,
    };
    tx.commit().await?;
    Ok(stopped)
}

async fn close(conn: &mut SqliteConnection, entry: TimeEntry, now: i64) -> AppResult<Option<TimeEntry>> {
    if now - entry.started_at < MIN_ENTRY_MS {
        entries::soft_delete(&mut *conn, &entry.id, now).await?;
        return Ok(None);
    }
    Ok(Some(entries::set_ended(&mut *conn, &entry.id, now, now).await?))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{self, models::ProjectInput};

    const T0: i64 = 1_780_000_000_000;
    const MIN: i64 = 60_000;

    async fn project(pool: &SqlitePool, name: &str) -> String {
        let input = ProjectInput { name: name.into(), client: "ACME".into(), color: "#3366ff".into() };
        db::projects::create(pool, input, T0).await.unwrap().id
    }

    #[tokio::test]
    async fn demarrer_cree_une_entree_en_cours() {
        let pool = db::open_in_memory().await;
        let p = project(&pool, "Site web").await;
        let e = start(&pool, &p, T0).await.unwrap();
        assert_eq!(e.ended_at, None);
        assert_eq!(e.started_at, T0);
        assert_eq!(entries::running(&pool).await.unwrap().unwrap().id, e.id);
    }

    #[tokio::test]
    async fn demarrer_un_autre_projet_arrete_le_precedent() {
        let pool = db::open_in_memory().await;
        let a = project(&pool, "A").await;
        let b = project(&pool, "B").await;
        let ea = start(&pool, &a, T0).await.unwrap();
        let eb = start(&pool, &b, T0 + 30 * MIN).await.unwrap();

        let a_done = entries::get(&pool, &ea.id).await.unwrap();
        assert_eq!(a_done.ended_at, Some(T0 + 30 * MIN));
        assert_eq!(entries::running(&pool).await.unwrap().unwrap().id, eb.id);
    }

    #[tokio::test]
    async fn redemarrer_le_meme_projet_ne_fait_rien() {
        let pool = db::open_in_memory().await;
        let a = project(&pool, "A").await;
        let first = start(&pool, &a, T0).await.unwrap();
        let again = start(&pool, &a, T0 + 10 * MIN).await.unwrap();
        assert_eq!(first.id, again.id);
        assert_eq!(again.started_at, T0);
    }

    #[tokio::test]
    async fn arreter_termine_l_entree() {
        let pool = db::open_in_memory().await;
        let a = project(&pool, "A").await;
        start(&pool, &a, T0).await.unwrap();
        let stopped = stop(&pool, T0 + 45 * MIN).await.unwrap().unwrap();
        assert_eq!(stopped.ended_at, Some(T0 + 45 * MIN));
        assert!(entries::running(&pool).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn arreter_sans_timer_renvoie_none() {
        let pool = db::open_in_memory().await;
        assert!(stop(&pool, T0).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn entree_trop_courte_est_jetee() {
        let pool = db::open_in_memory().await;
        let a = project(&pool, "A").await;
        let b = project(&pool, "B").await;
        let ea = start(&pool, &a, T0).await.unwrap();
        start(&pool, &b, T0 + 2_000).await.unwrap();
        assert!(entries::get(&pool, &ea.id).await.is_err());
        assert_eq!(entries::list_between(&pool, T0, T0 + MIN).await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn projet_archive_refuse() {
        let pool = db::open_in_memory().await;
        let a = project(&pool, "A").await;
        db::projects::set_archived(&pool, &a, true, T0).await.unwrap();
        assert!(matches!(start(&pool, &a, T0).await, Err(AppError::Validation(_))));
    }

    #[tokio::test]
    async fn la_base_interdit_deux_timers_en_cours() {
        let pool = db::open_in_memory().await;
        let a = project(&pool, "A").await;
        entries::insert_running(&pool, &a, T0, "", EntrySource::Timer).await.unwrap();
        let second = entries::insert_running(&pool, &a, T0 + MIN, "", EntrySource::Manual).await;
        assert!(second.is_err());
    }

    #[tokio::test]
    async fn le_timer_survit_a_un_redemarrage() {
        let path = std::env::temp_dir().join(format!("trackify-test-{}.db", db::new_id()));
        let started_id = {
            let pool = db::open(&path).await.unwrap();
            let a = project(&pool, "A").await;
            let id = start(&pool, &a, T0).await.unwrap().id;
            pool.close().await;
            id
        };
        let pool = db::open(&path).await.unwrap();
        let running = entries::running(&pool).await.unwrap().unwrap();
        assert_eq!(running.id, started_id);
        assert_eq!(running.started_at, T0);
        pool.close().await;
        for suffix in ["", "-wal", "-shm"] {
            let _ = std::fs::remove_file(format!("{}{suffix}", path.display()));
        }
    }

    #[tokio::test]
    async fn liste_inclut_l_entree_en_cours() {
        let pool = db::open_in_memory().await;
        let a = project(&pool, "A").await;
        start(&pool, &a, T0).await.unwrap();
        let listed = entries::list_between(&pool, T0 + MIN, T0 + 2 * MIN).await.unwrap();
        assert_eq!(listed.len(), 1);
    }

    #[tokio::test]
    async fn noms_de_projet_uniques_par_client() {
        let pool = db::open_in_memory().await;
        let mk = |name: &str, client: &str| ProjectInput {
            name: name.into(),
            client: client.into(),
            color: "#000000".into(),
        };
        let a = db::projects::create(&pool, mk("Site", "ACME"), T0).await.unwrap();
        assert!(db::projects::create(&pool, mk("  Site ", "ACME"), T0).await.is_err());
        assert!(db::projects::create(&pool, mk("Site", "Globex"), T0).await.is_ok());
        // la casse compte : le nom doit coller exactement au timesheet
        assert!(db::projects::create(&pool, mk("site", "ACME"), T0).await.is_ok());
        db::projects::soft_delete(&pool, &a.id, T0).await.unwrap();
        assert!(db::projects::create(&pool, mk("Site", "ACME"), T0).await.is_ok());
    }

    #[tokio::test]
    async fn validation_du_projet() {
        let pool = db::open_in_memory().await;
        let bad_name = ProjectInput { name: "   ".into(), client: "".into(), color: "#000000".into() };
        let bad_color = ProjectInput { name: "X".into(), client: "".into(), color: "rouge".into() };
        assert!(db::projects::create(&pool, bad_name, T0).await.is_err());
        assert!(db::projects::create(&pool, bad_color, T0).await.is_err());
    }
}
