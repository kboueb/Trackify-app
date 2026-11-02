//! Suivi d'activité : toutes les 10 s, la fenêtre au premier plan est notée, et les
//! échantillons identiques consécutifs sont fusionnés en segments. Rien ne quitte la machine ;
//! l'historique est purgé au-delà de 30 jours.

use std::time::Duration;

use serde::Serialize;
use specta::Type;
use sqlx::SqlitePool;
use tauri::{AppHandle, Manager};
use trackify_core::{
    activity::{self, Activity, Last},
    durations::MINUTE,
};

use crate::{
    AppState,
    db::{activity as store, entries, now_ms, settings},
    error::AppResult,
    platform,
};

const SAMPLE_EVERY: Duration = Duration::from_secs(10);
/// Au-delà, deux échantillons ne sont plus considérés comme continus (veille, pause).
const MAX_GAP: i64 = 30_000;
const RETENTION: i64 = 30 * 24 * 60 * MINUTE;
const PURGE_EVERY: i64 = 60 * MINUTE;
/// Regroupement et longueur minimale des plages « non attribuées ».
const JOIN_GAP: i64 = 5 * MINUTE;
const MIN_BLOCK: i64 = 5 * MINUTE;

pub fn start(app: &AppHandle) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let mut tick = tokio::time::interval(SAMPLE_EVERY);
        tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        let mut last_purge = 0;
        loop {
            tick.tick().await;
            let db = &app.state::<AppState>().db;
            let now = now_ms();
            if now - last_purge >= PURGE_EVERY {
                last_purge = now;
                if let Err(e) = store::purge_before(db, now - RETENTION).await {
                    eprintln!("activité : purge impossible : {e}");
                }
            }
            if let Err(e) = sample(db, now).await {
                eprintln!("activité : {e}");
            }
        }
    });
}

async fn sample(db: &SqlitePool, now: i64) -> AppResult<()> {
    let settings = settings::load(db).await?;
    if !settings.activity_enabled {
        return Ok(());
    }
    let idle = platform::idle::idle_time().map_or(0, |d| d.as_millis() as i64);
    if idle >= settings.reminders.to_config().idle_threshold {
        return Ok(()); // absent : on laisse un trou
    }
    // L'API système peut bloquer brièvement : hors du runtime asynchrone.
    let Ok(Some((app, title))) = tokio::task::spawn_blocking(platform::window::active_window).await else {
        return Ok(());
    };
    record(db, &app, &title, now).await
}

/// Prolonge le dernier segment s'il montre la même fenêtre, sinon en commence un nouveau.
pub async fn record(db: &SqlitePool, app: &str, title: &str, now: i64) -> AppResult<()> {
    let last = store::last(db).await?;
    let view = last.as_ref().map(|l| Last { app: &l.app, title: &l.title, end: l.ended_at });
    if activity::extends(view, app, title, now, MAX_GAP) {
        let last = last.expect("extends implique un segment");
        return store::extend(db, &last.id, now).await;
    }
    // Changement de fenêtre sans trou : l'ancienne était active jusqu'à maintenant.
    if let Some(l) = last.filter(|l| now >= l.ended_at && now - l.ended_at <= MAX_GAP) {
        store::extend(db, &l.id, now).await?;
    }
    store::insert(db, app, title, now).await
}

/// Met en pause ou relance le suivi.
pub async fn set_enabled(db: &SqlitePool, enabled: bool) -> AppResult<settings::Settings> {
    let mut current = settings::load(db).await?;
    current.activity_enabled = enabled;
    settings::save(db, &current).await?;
    Ok(current)
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct NamedTime {
    pub name: String,
    pub ms: i64,
}

/// Temps actif sans projet, à attribuer pendant la revue de journée.
#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct UnassignedBlock {
    pub started_at: i64,
    pub ended_at: i64,
    pub apps: Vec<NamedTime>,
    pub titles: Vec<NamedTime>,
}

pub async fn unassigned(db: &SqlitePool, from: i64, to: i64, now: i64) -> AppResult<Vec<UnassignedBlock>> {
    let segments = store::list_between(db, from, to).await?;
    let spans: Vec<(i64, i64)> = entries::list_between(db, from, to)
        .await?
        .iter()
        .map(|e| (e.started_at, e.ended_at.unwrap_or(now)))
        .collect();
    let activity: Vec<Activity> = segments
        .iter()
        .map(|s| Activity { start: s.started_at.max(from), end: s.ended_at.min(to), app: &s.app, title: &s.title })
        .collect();
    let named = |list: Vec<(String, i64)>| list.into_iter().map(|(name, ms)| NamedTime { name, ms }).collect();
    Ok(activity::unassigned(&activity, &spans, JOIN_GAP, MIN_BLOCK)
        .into_iter()
        .map(|u| UnassignedBlock { started_at: u.start, ended_at: u.end, apps: named(u.apps), titles: named(u.titles) })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db;

    const S: i64 = 1_000;

    #[tokio::test]
    async fn echantillons_fusionnes_en_segments() {
        let pool = db::open_in_memory().await;
        record(&pool, "Figma", "Maquette", 0).await.unwrap();
        record(&pool, "Figma", "Maquette", 10 * S).await.unwrap();
        record(&pool, "Figma", "Maquette", 20 * S).await.unwrap();
        record(&pool, "Mail", "Boîte", 30 * S).await.unwrap();
        // Trou de 5 min (veille) : le segment Mail n'est pas prolongé.
        record(&pool, "Mail", "Boîte", 330 * S).await.unwrap();
        let segs = store::list_between(&pool, -1, i64::MAX).await.unwrap();
        let view: Vec<(&str, i64, i64)> = segs.iter().map(|s| (s.app.as_str(), s.started_at, s.ended_at)).collect();
        assert_eq!(view, vec![("Figma", 0, 30 * S), ("Mail", 30 * S, 30 * S), ("Mail", 330 * S, 330 * S)]);
    }

    #[tokio::test]
    async fn la_nouvelle_fenetre_continue_apres_un_changement() {
        // Au changement, l'ancien segment et le nouveau finissent au même instant :
        // c'est bien le nouveau qui doit être prolongé ensuite.
        let pool = db::open_in_memory().await;
        for (app, t) in [("Figma", 0), ("Figma", 10), ("Mail", 20), ("Mail", 30), ("Mail", 40)] {
            record(&pool, app, "", t * S).await.unwrap();
        }
        let segs = store::list_between(&pool, -1, i64::MAX).await.unwrap();
        let view: Vec<(&str, i64, i64)> = segs.iter().map(|s| (s.app.as_str(), s.started_at, s.ended_at)).collect();
        assert_eq!(view, vec![("Figma", 0, 20 * S), ("Mail", 20 * S, 40 * S)]);
    }
}
