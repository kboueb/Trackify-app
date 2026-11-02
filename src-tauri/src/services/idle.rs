//! Décision de l'utilisateur sur une absence : garder, retirer ou attribuer à un autre projet.

use serde::Deserialize;
use specta::Type;
use sqlx::SqlitePool;
use trackify_core::gaps::{GapOutcome, Span, remove_gap};

use crate::{
    db::{entries, idle, models::EntrySource, projects},
    error::{AppError, AppResult},
    services::timer::MIN_ENTRY_MS,
};

#[derive(Debug, Clone, Deserialize, Type)]
#[serde(tag = "action", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum IdleResolution {
    /// Le temps reste compté sur le projet en cours.
    Keep,
    /// Le temps est retiré de l'entrée.
    Discard,
    /// Le temps est retiré de l'entrée et compté sur un autre projet.
    Reassign { project_id: String },
}

pub async fn resolve(pool: &SqlitePool, idle_id: &str, resolution: &IdleResolution, now: i64) -> AppResult<()> {
    let mut tx = pool.begin().await?;
    let Some(period) = idle::get_pending(&mut *tx, idle_id).await? else {
        return Ok(()); // déjà traitée (double clic, autre fenêtre)
    };

    let label = match resolution {
        IdleResolution::Keep => "keep",
        IdleResolution::Discard => "discard",
        IdleResolution::Reassign { project_id } => {
            let target = projects::get(&mut *tx, project_id).await?;
            if target.archived {
                return Err(AppError::Validation(format!("Le projet « {} » est archivé.", target.name)));
            }
            "reassign"
        }
    };

    if !matches!(resolution, IdleResolution::Keep) {
        match entries::get(&mut *tx, &period.entry_id).await {
            Ok(entry) => {
                let span = Span { start: entry.started_at, end: entry.ended_at };
                if let GapOutcome::Split { before, after, removed } =
                    remove_gap(span, period.started_at, period.ended_at, MIN_ENTRY_MS)
                {
                    // Clore ou supprimer l'entrée d'origine avant de recréer la suite :
                    // l'index « un seul timer en cours » l'exige.
                    match before {
                        Some((_, end)) => {
                            entries::set_ended(&mut *tx, &entry.id, end, now).await?;
                        }
                        None => entries::soft_delete(&mut *tx, &entry.id, now).await?,
                    }
                    if let Some(rest) = after {
                        entries::insert(&mut *tx, &entry.project_id, rest.start, rest.end, &entry.note, entry.source, now)
                            .await?;
                    }
                    if let IdleResolution::Reassign { project_id } = resolution {
                        entries::insert(&mut *tx, project_id, removed.0, Some(removed.1), "", EntrySource::Idle, now)
                            .await?;
                    }
                }
            }
            // L'entrée a été supprimée entre-temps : plus rien à couper.
            Err(AppError::NotFound(_)) => {}
            Err(e) => return Err(e),
        }
    }

    idle::set_resolution(&mut *tx, &period.id, label, now).await?;
    tx.commit().await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        db::{self, idle::IdleKind, models::ProjectInput},
        services::timer,
    };

    const T0: i64 = 1_780_000_000_000;
    const MIN: i64 = 60_000;

    async fn project(pool: &SqlitePool, name: &str) -> String {
        let input = ProjectInput { name: name.into(), client: "".into(), color: "#3366ff".into() };
        db::projects::create(pool, input, T0).await.unwrap().id
    }

    async fn day(pool: &SqlitePool) -> Vec<(String, i64, Option<i64>, EntrySource)> {
        entries::list_between(pool, T0 - 24 * 60 * MIN, T0 + 24 * 60 * MIN)
            .await
            .unwrap()
            .into_iter()
            .map(|e| (e.project_id, e.started_at - T0, e.ended_at.map(|t| t - T0), e.source))
            .collect()
    }

    /// Timer sur A depuis T0, absence de T0+30 à T0+45.
    async fn setup() -> (SqlitePool, String, String, String) {
        let pool = db::open_in_memory().await;
        let a = project(&pool, "A").await;
        let b = project(&pool, "B").await;
        let entry = timer::start(&pool, &a, T0).await.unwrap();
        let id = idle::insert(&pool, &entry.id, IdleKind::Idle, T0 + 30 * MIN, T0 + 45 * MIN, T0 + 45 * MIN)
            .await
            .unwrap();
        let pending = idle::pending(&pool).await.unwrap();
        assert_eq!(pending[0].project_id, a, "l'absence porte le projet du timer");
        (pool, a, b, id)
    }

    #[tokio::test]
    async fn garder_ne_change_rien() {
        let (pool, a, _, id) = setup().await;
        resolve(&pool, &id, &IdleResolution::Keep, T0 + 46 * MIN).await.unwrap();
        assert_eq!(day(&pool).await, vec![(a, 0, None, EntrySource::Timer)]);
        assert!(idle::pending(&pool).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn retirer_coupe_le_timer_qui_continue() {
        let (pool, a, _, id) = setup().await;
        resolve(&pool, &id, &IdleResolution::Discard, T0 + 46 * MIN).await.unwrap();
        assert_eq!(
            day(&pool).await,
            vec![
                (a.clone(), 0, Some(30 * MIN), EntrySource::Timer),
                (a, 45 * MIN, None, EntrySource::Timer),
            ]
        );
    }

    #[tokio::test]
    async fn attribuer_a_un_autre_projet() {
        let (pool, a, b, id) = setup().await;
        let r = IdleResolution::Reassign { project_id: b.clone() };
        resolve(&pool, &id, &r, T0 + 46 * MIN).await.unwrap();
        assert_eq!(
            day(&pool).await,
            vec![
                (a.clone(), 0, Some(30 * MIN), EntrySource::Timer),
                (b, 30 * MIN, Some(45 * MIN), EntrySource::Idle),
                (a, 45 * MIN, None, EntrySource::Timer),
            ]
        );
    }

    #[tokio::test]
    async fn entree_terminee_entre_temps_coupee_en_deux() {
        let (pool, a, _, id) = setup().await;
        timer::stop(&pool, T0 + 60 * MIN).await.unwrap();
        resolve(&pool, &id, &IdleResolution::Discard, T0 + 61 * MIN).await.unwrap();
        assert_eq!(
            day(&pool).await,
            vec![
                (a.clone(), 0, Some(30 * MIN), EntrySource::Timer),
                (a, 45 * MIN, Some(60 * MIN), EntrySource::Timer),
            ]
        );
    }

    #[tokio::test]
    async fn deuxieme_decision_ignoree() {
        let (pool, a, _, id) = setup().await;
        resolve(&pool, &id, &IdleResolution::Keep, T0 + 46 * MIN).await.unwrap();
        resolve(&pool, &id, &IdleResolution::Discard, T0 + 47 * MIN).await.unwrap();
        assert_eq!(day(&pool).await, vec![(a, 0, None, EntrySource::Timer)]);
    }

    #[tokio::test]
    async fn projet_archive_refuse() {
        let (pool, _, b, id) = setup().await;
        db::projects::set_archived(&pool, &b, true, T0).await.unwrap();
        let r = IdleResolution::Reassign { project_id: b };
        assert!(matches!(resolve(&pool, &id, &r, T0 + 46 * MIN).await, Err(AppError::Validation(_))));
        assert_eq!(idle::pending(&pool).await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn entree_supprimee_entre_temps() {
        let (pool, _, _, id) = setup().await;
        let running = entries::running(&pool).await.unwrap().unwrap();
        entries::soft_delete(&pool, &running.id, T0 + 46 * MIN).await.unwrap();
        resolve(&pool, &id, &IdleResolution::Discard, T0 + 47 * MIN).await.unwrap();
        assert!(day(&pool).await.is_empty());
        assert!(idle::pending(&pool).await.unwrap().is_empty());
    }
}
