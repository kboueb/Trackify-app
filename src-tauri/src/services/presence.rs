//! Boucle de fond : détection des absences, battement de cœur et rappels anti-oubli.
//!
//! La logique de décision vit dans `trackify_core::presence` ; ici on se contente
//! d'échantillonner (heure, inactivité, timer en cours) et d'appliquer les effets.

use std::{sync::Mutex, time::Duration};

use chrono::{Datelike, Local, Timelike};
use serde::{Deserialize, Serialize};
use specta::Type;
use tauri::{AppHandle, Manager};
use tauri_specta::Event;
use trackify_core::{
    durations::MINUTE,
    presence::{IdleTracker, LocalClock, Reminder, Reminders},
};

use crate::{
    AppState,
    db::{
        entries,
        idle::{self, IdleKind},
        now_ms, projects, settings,
    },
    error::AppResult,
    events::{IdleChanged, ReminderShown},
    platform, windows,
};

const TICK: Duration = Duration::from_secs(5);
const HEARTBEAT_EVERY: i64 = 30_000;
/// « Plus tard » sur un rappel sans timer.
pub const SNOOZE: i64 = 60 * MINUTE;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum ReminderKind {
    NoTimer,
    CheckIn,
    /// Fin de journée : revoir et compléter ses temps.
    Review,
}

/// Contenu de la fenêtre de rappel.
#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ReminderView {
    pub kind: ReminderKind,
    pub project_name: Option<String>,
    pub project_color: Option<String>,
    pub started_at: Option<i64>,
}

/// Réponse de l'utilisateur au rappel.
#[derive(Debug, Clone, Copy, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum ReminderAction {
    /// Fermer (« Oui, toujours dessus », ou disparition automatique).
    Dismiss,
    /// Ne plus rappeler l'absence de timer pendant un moment.
    Later,
    /// Ouvrir le sélecteur rapide.
    Switch,
    /// Arrêter le timer en cours.
    Stop,
    /// Ouvrir la revue de la journée.
    Review,
}

#[derive(Default)]
pub struct Presence {
    pub reminders: Mutex<Reminders>,
    pub current: Mutex<Option<ReminderView>>,
}

pub fn start(app: &AppHandle) {
    app.manage(Presence::default());
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        if let Err(e) = detect_closed_gap(&app).await {
            eprintln!("présence : contrôle au lancement impossible : {e}");
        }
        let mut tracker = IdleTracker::default();
        let mut last_heartbeat = now_ms();
        let mut tick = tokio::time::interval(TICK);
        tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            tick.tick().await;
            if let Err(e) = step(&app, &mut tracker, &mut last_heartbeat).await {
                eprintln!("présence : {e}");
            }
        }
    });
}

/// Au lancement : si un timer tournait et que l'app est restée fermée (ou a planté)
/// plus longtemps que le seuil d'inactivité, on demande quoi faire de ce temps.
async fn detect_closed_gap(app: &AppHandle) -> AppResult<()> {
    let db = &app.state::<AppState>().db;
    let now = now_ms();
    let threshold = settings::load(db).await?.reminders.to_config().idle_threshold;
    let heartbeat = settings::heartbeat(db).await?;
    settings::set_heartbeat(db, now).await?;

    if let (Some(entry), Some(last_seen)) = (entries::running(db).await?, heartbeat) {
        let from = last_seen.max(entry.started_at);
        if now - from >= threshold {
            idle::insert(db, &entry.id, IdleKind::Closed, from, now, now).await?;
            announce_idle(app);
        }
    }
    Ok(())
}

async fn step(app: &AppHandle, tracker: &mut IdleTracker, last_heartbeat: &mut i64) -> AppResult<()> {
    let db = &app.state::<AppState>().db;
    let now = now_ms();
    let idle_ms = platform::idle::idle_time().map_or(0, |d| d.as_millis() as i64);
    let cfg = settings::load(db).await?.reminders.to_config();
    let running = entries::running(db).await?;

    if let Some((from, to)) = tracker.tick(now, idle_ms, running.is_some(), cfg.idle_threshold)
        && let Some(entry) = &running
    {
        let from = from.max(entry.started_at);
        if to > from {
            idle::insert(db, &entry.id, IdleKind::Idle, from, to, now).await?;
            announce_idle(app);
        }
    }

    if now - *last_heartbeat >= HEARTBEAT_EVERY {
        settings::set_heartbeat(db, now).await?;
        *last_heartbeat = now;
    }

    // Pas de rappel tant qu'une absence attend une décision : une question à la fois.
    if !idle::pending(db).await?.is_empty() {
        return Ok(());
    }
    let local = Local::now();
    let clock = LocalClock {
        minute_of_day: local.hour() * 60 + local.minute(),
        weekday: local.weekday().num_days_from_monday(),
    };
    let presence = app.state::<Presence>();
    let day = i64::from(local.date_naive().num_days_from_ce());
    if presence.reminders.lock().unwrap().review_due(&cfg, day, idle_ms, clock) {
        let view = ReminderView { kind: ReminderKind::Review, project_name: None, project_color: None, started_at: None };
        show(app, &presence, view);
        return Ok(());
    }
    let reminder = presence.reminders.lock().unwrap().tick(
        &cfg,
        now,
        idle_ms,
        running.as_ref().map(|e| (e.id.as_str(), e.started_at)),
        clock,
    );
    let Some(reminder) = reminder else { return Ok(()) };

    let view = match (reminder, &running) {
        (Reminder::CheckIn, Some(entry)) => {
            let project = projects::get(db, &entry.project_id).await.ok();
            ReminderView {
                kind: ReminderKind::CheckIn,
                project_name: project.as_ref().map(|p| p.name.clone()),
                project_color: project.map(|p| p.color),
                started_at: Some(entry.started_at),
            }
        }
        _ => ReminderView { kind: ReminderKind::NoTimer, project_name: None, project_color: None, started_at: None },
    };
    show(app, &presence, view);
    Ok(())
}

fn show(app: &AppHandle, presence: &Presence, view: ReminderView) {
    *presence.current.lock().unwrap() = Some(view);
    let _ = ReminderShown.emit(app);
    windows::show_reminder(app);
}

/// Une absence attend une décision : on affiche la fenêtre principale qui pose la question.
fn announce_idle(app: &AppHandle) {
    let _ = IdleChanged.emit(app);
    windows::hide_reminder(app);
    windows::show_main(app);
}
