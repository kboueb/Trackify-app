//! Édition des entrées pour la revue de journée : modifier, ajouter, scinder, fusionner.
//!
//! Règle des chevauchements : l'entrée qu'on vient de saisir a raison. Les autres
//! entrées qu'elle recouvre sont raccourcies, coupées en deux ou supprimées.

use serde::Deserialize;
use specta::Type;
use sqlx::{SqliteConnection, SqlitePool};
use trackify_core::gaps::{GapOutcome, Span, remove_gap};

use crate::{
    db::{
        entries,
        models::{EntrySource, TimeEntry},
        projects,
    },
    error::{AppError, AppResult},
    services::timer::MIN_ENTRY_MS,
};

/// Tolérance sur une fin saisie « dans le futur » (horloge, saisie à la minute près).
const FUTURE_TOLERANCE: i64 = 60_000;

#[derive(Debug, Clone, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct EntryInput {
    pub project_id: String,
    pub started_at: i64,
    /// `None` : l'entrée tourne encore (seulement pour le timer en cours).
    pub ended_at: Option<i64>,
    pub note: String,
}

fn invalid(msg: &str) -> AppError {
    AppError::Validation(msg.into())
}

/// Retire `[from, to)` de `entry` : raccourcit, coupe en deux ou supprime.
/// Renvoie la plage effectivement retirée.
pub async fn cut(conn: &mut SqliteConnection, entry: &TimeEntry, from: i64, to: i64, now: i64) -> AppResult<Option<(i64, i64)>> {
    let span = Span { start: entry.started_at, end: entry.ended_at };
    let GapOutcome::Split { before, after, removed } = remove_gap(span, from, to, MIN_ENTRY_MS) else {
        return Ok(None);
    };
    // Clore ou supprimer l'entrée d'origine avant de recréer la suite :
    // l'index « un seul timer en cours » l'exige.
    match before {
        Some((_, end)) => {
            entries::set_ended(&mut *conn, &entry.id, end, now).await?;
        }
        None => entries::soft_delete(&mut *conn, &entry.id, now).await?,
    }
    if let Some(rest) = after {
        entries::insert(&mut *conn, &entry.project_id, rest.start, rest.end, &entry.note, entry.source, now).await?;
    }
    Ok(Some(removed))
}

/// Crée (`id = None`) ou modifie une entrée, puis fait de la place autour d'elle.
pub async fn save(pool: &SqlitePool, id: Option<&str>, input: EntryInput, now: i64) -> AppResult<TimeEntry> {
    let mut tx = pool.begin().await?;
    projects::get(&mut *tx, &input.project_id).await?;

    let existing = match id {
        Some(id) => Some(entries::get(&mut *tx, id).await?),
        None => None,
    };
    let was_running = existing.as_ref().is_some_and(|e| e.ended_at.is_none());
    if input.ended_at.is_none() && !was_running {
        return Err(invalid("Indique une heure de fin."));
    }
    let end = input.ended_at.unwrap_or(now);
    if input.started_at > now || end > now + FUTURE_TOLERANCE {
        return Err(invalid("Une entrée ne peut pas être dans le futur."));
    }
    if end - input.started_at < MIN_ENTRY_MS {
        return Err(invalid("La fin doit être après le début."));
    }
    let end = end.min(now);

    for other in entries::list_between(&mut *tx, input.started_at, end).await? {
        if Some(other.id.as_str()) != id {
            cut(&mut tx, &other, input.started_at, end, now).await?;
        }
    }

    let ended_at = input.ended_at.map(|_| end);
    let saved = match id {
        Some(id) => entries::update(&mut *tx, id, &input.project_id, input.started_at, ended_at, &input.note, now).await?,
        None => {
            entries::insert(&mut *tx, &input.project_id, input.started_at, ended_at, input.note.trim(), EntrySource::Manual, now)
                .await?
        }
    };
    tx.commit().await?;
    Ok(saved)
}

/// Coupe une entrée en deux à l'instant `at`. Les deux morceaux gardent projet et note.
pub async fn split(pool: &SqlitePool, id: &str, at: i64, now: i64) -> AppResult<(TimeEntry, TimeEntry)> {
    let mut tx = pool.begin().await?;
    let entry = entries::get(&mut *tx, id).await?;
    let end = entry.ended_at.unwrap_or(now);
    if at - entry.started_at < MIN_ENTRY_MS || end - at < MIN_ENTRY_MS {
        return Err(invalid("Choisis une heure à l'intérieur de l'entrée."));
    }
    let first = entries::set_ended(&mut *tx, &entry.id, at, now).await?;
    let second = entries::insert(&mut *tx, &entry.project_id, at, entry.ended_at, &entry.note, entry.source, now).await?;
    tx.commit().await?;
    Ok((first, second))
}

/// Fusionne des entrées d'un même projet en une seule, du premier début à la dernière fin.
/// Refusé si une autre entrée se trouve entre elles.
pub async fn merge(pool: &SqlitePool, ids: &[String], now: i64) -> AppResult<TimeEntry> {
    if ids.len() < 2 {
        return Err(invalid("Sélectionne au moins deux entrées."));
    }
    let mut tx = pool.begin().await?;
    let mut list = Vec::with_capacity(ids.len());
    for id in ids {
        list.push(entries::get(&mut *tx, id).await?);
    }
    list.sort_by_key(|e| e.started_at);
    let first = list[0].clone();
    if list.iter().any(|e| e.project_id != first.project_id) {
        return Err(invalid("Seules des entrées du même projet peuvent être fusionnées."));
    }
    let start = first.started_at;
    let running = list.iter().any(|e| e.ended_at.is_none());
    let end = list.iter().map(|e| e.ended_at.unwrap_or(now)).max().unwrap_or(now);
    let blocking = entries::list_between(&mut *tx, start, end)
        .await?
        .into_iter()
        .any(|e| !ids.contains(&e.id));
    if blocking {
        return Err(invalid("Une autre entrée se trouve entre elles : impossible de fusionner."));
    }

    let mut notes: Vec<&str> = Vec::new();
    for e in &list {
        let note = e.note.trim();
        if !note.is_empty() && !notes.contains(&note) {
            notes.push(note);
        }
    }
    let note = notes.join(" · ");
    for e in &list[1..] {
        entries::soft_delete(&mut *tx, &e.id, now).await?;
    }
    let ended_at = (!running).then_some(end);
    let merged = entries::update(&mut *tx, &first.id, &first.project_id, start, ended_at, &note, now).await?;
    tx.commit().await?;
    Ok(merged)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        db::{self, models::ProjectInput},
        services::timer,
    };

    const T0: i64 = 1_780_000_000_000;
    const MIN: i64 = 60_000;
    const NOW: i64 = T0 + 600 * MIN;

    async fn setup() -> (SqlitePool, String, String) {
        let pool = db::open_in_memory().await;
        let mk = |name: &str| ProjectInput { name: name.into(), client: "".into(), color: "#3366ff".into() };
        let a = db::projects::create(&pool, mk("A"), T0).await.unwrap().id;
        let b = db::projects::create(&pool, mk("B"), T0).await.unwrap().id;
        (pool, a, b)
    }

    fn input(project: &str, start: i64, end: Option<i64>) -> EntryInput {
        EntryInput { project_id: project.into(), started_at: T0 + start * MIN, ended_at: end.map(|e| T0 + e * MIN), note: "".into() }
    }

    async fn spans(pool: &SqlitePool) -> Vec<(String, i64, Option<i64>)> {
        entries::list_between(pool, 0, i64::MAX)
            .await
            .unwrap()
            .into_iter()
            .map(|e| (e.project_id, (e.started_at - T0) / MIN, e.ended_at.map(|x| (x - T0) / MIN)))
            .collect()
    }

    #[tokio::test]
    async fn ajout_manuel_sans_chevauchement() {
        let (pool, a, _) = setup().await;
        let e = save(&pool, None, input(&a, 0, Some(60)), NOW).await.unwrap();
        assert_eq!(e.source, EntrySource::Manual);
        assert_eq!(spans(&pool).await, vec![(a, 0, Some(60))]);
    }

    #[tokio::test]
    async fn ajout_au_milieu_coupe_l_entree_existante() {
        let (pool, a, b) = setup().await;
        save(&pool, None, input(&a, 0, Some(120)), NOW).await.unwrap();
        save(&pool, None, input(&b, 30, Some(60)), NOW).await.unwrap();
        assert_eq!(spans(&pool).await, vec![(a.clone(), 0, Some(30)), (b, 30, Some(60)), (a, 60, Some(120))]);
    }

    #[tokio::test]
    async fn modifier_raccourcit_la_voisine_et_supprime_celle_recouverte() {
        let (pool, a, b) = setup().await;
        let first = save(&pool, None, input(&a, 0, Some(60)), NOW).await.unwrap();
        save(&pool, None, input(&b, 60, Some(90)), NOW).await.unwrap();
        save(&pool, None, input(&b, 90, Some(120)), NOW).await.unwrap();
        // On étend la première jusqu'à 100 : 60–90 disparaît, 90–120 devient 100–120.
        save(&pool, Some(&first.id), input(&a, 0, Some(100)), NOW).await.unwrap();
        assert_eq!(spans(&pool).await, vec![(a, 0, Some(100)), (b, 100, Some(120))]);
    }

    #[tokio::test]
    async fn ajout_dans_le_timer_en_cours_le_coupe_sans_l_arreter() {
        let (pool, a, b) = setup().await;
        timer::start(&pool, &a, T0).await.unwrap();
        save(&pool, None, input(&b, 30, Some(60)), NOW).await.unwrap();
        assert_eq!(spans(&pool).await, vec![(a.clone(), 0, Some(30)), (b, 30, Some(60)), (a.clone(), 60, None)]);
        assert_eq!(entries::running(&pool).await.unwrap().unwrap().project_id, a);
    }

    #[tokio::test]
    async fn modifier_le_debut_du_timer_en_cours() {
        let (pool, a, b) = setup().await;
        save(&pool, None, input(&b, 0, Some(60)), NOW).await.unwrap();
        let running = timer::start(&pool, &a, T0 + 60 * MIN).await.unwrap();
        // « En fait j'ai commencé à 0:45 »
        save(&pool, Some(&running.id), input(&a, 45, None), NOW).await.unwrap();
        assert_eq!(spans(&pool).await, vec![(b, 0, Some(45)), (a, 45, None)]);
    }

    #[tokio::test]
    async fn saisies_invalides() {
        let (pool, a, _) = setup().await;
        assert!(save(&pool, None, input(&a, 60, Some(30)), NOW).await.is_err(), "fin avant début");
        assert!(save(&pool, None, input(&a, 0, None), NOW).await.is_err(), "pas de fin");
        assert!(save(&pool, None, input(&a, 590, Some(700)), NOW).await.is_err(), "futur");
        let done = save(&pool, None, input(&a, 0, Some(10)), NOW).await.unwrap();
        assert!(save(&pool, Some(&done.id), input(&a, 0, None), NOW).await.is_err(), "relancer une entrée terminée");
    }

    #[tokio::test]
    async fn scinder() {
        let (pool, a, _) = setup().await;
        let mut e = input(&a, 0, Some(60));
        e.note = "Maquette".into();
        let e = save(&pool, None, e, NOW).await.unwrap();
        let (x, y) = split(&pool, &e.id, T0 + 20 * MIN, NOW).await.unwrap();
        assert_eq!((x.ended_at, y.started_at, y.note.as_str()), (Some(T0 + 20 * MIN), T0 + 20 * MIN, "Maquette"));
        assert!(split(&pool, &x.id, T0 + 90 * MIN, NOW).await.is_err(), "hors de l'entrée");
    }

    #[tokio::test]
    async fn scinder_le_timer_en_cours() {
        let (pool, a, _) = setup().await;
        let e = timer::start(&pool, &a, T0).await.unwrap();
        split(&pool, &e.id, T0 + 30 * MIN, NOW).await.unwrap();
        assert_eq!(spans(&pool).await, vec![(a.clone(), 0, Some(30)), (a, 30, None)]);
    }

    #[tokio::test]
    async fn fusionner() {
        let (pool, a, b) = setup().await;
        let mut x = input(&a, 0, Some(30));
        x.note = "Maquette".into();
        let x = save(&pool, None, x, NOW).await.unwrap();
        let mut y = input(&a, 40, Some(60));
        y.note = "Revue".into();
        let y = save(&pool, None, y, NOW).await.unwrap();
        let merged = merge(&pool, &[y.id.clone(), x.id.clone()], NOW).await.unwrap();
        assert_eq!(merged.note, "Maquette · Revue");
        assert_eq!(spans(&pool).await, vec![(a.clone(), 0, Some(60))]);

        let other = save(&pool, None, input(&b, 70, Some(80)), NOW).await.unwrap();
        let z = save(&pool, None, input(&a, 90, Some(100)), NOW).await.unwrap();
        assert!(merge(&pool, &[merged.id.clone(), z.id.clone()], NOW).await.is_err(), "entrée au milieu");
        assert!(merge(&pool, &[other.id, z.id], NOW).await.is_err(), "projets différents");
    }

    #[tokio::test]
    async fn fusionner_avec_le_timer_en_cours() {
        let (pool, a, _) = setup().await;
        let x = save(&pool, None, input(&a, 0, Some(30)), NOW).await.unwrap();
        let r = timer::start(&pool, &a, T0 + 40 * MIN).await.unwrap();
        let merged = merge(&pool, &[x.id, r.id], NOW).await.unwrap();
        assert_eq!((merged.started_at, merged.ended_at), (T0, None));
    }
}
