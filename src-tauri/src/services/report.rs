//! Rapports par jour et par projet, et export CSV pour le timesheet.

use std::{collections::HashMap, path::Path};

use chrono::Local;
use serde::Serialize;
use specta::Type;
use sqlx::SqlitePool;
use trackify_core::report::{self, CsvRow, Interval};

use crate::{
    db::{entries, models::Project, projects},
    error::{AppError, AppResult},
};

pub const ROUNDING_STEPS: [u32; 4] = [0, 5, 15, 30];

/// Temps d'un projet sur un jour, brut et arrondi.
#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ReportRow {
    /// Jour local, « AAAA-MM-JJ ».
    pub date: String,
    pub project_id: String,
    pub ms: i64,
    pub rounded_ms: i64,
    pub notes: Vec<String>,
}

pub fn check_rounding(step: u32) -> AppResult<()> {
    if ROUNDING_STEPS.contains(&step) {
        Ok(())
    } else {
        Err(AppError::Validation(format!("Arrondi non pris en charge : {step} min.")))
    }
}

/// Lignes du rapport sur `[from, to)`, dans l'ordre des jours. Le timer en cours compte jusqu'à `now`.
pub async fn rows(pool: &SqlitePool, from: i64, to: i64, rounding: u32, now: i64) -> AppResult<Vec<ReportRow>> {
    check_rounding(rounding)?;
    let list = entries::list_between(pool, from, to).await?;
    let intervals: Vec<Interval> = list
        .iter()
        .map(|e| Interval { project_id: &e.project_id, start: e.started_at, end: e.ended_at.unwrap_or(now), note: &e.note })
        .collect();
    Ok(report::daily_totals(&intervals, from, to, &Local)
        .into_iter()
        .map(|t| ReportRow {
            date: t.date.format("%Y-%m-%d").to_string(),
            rounded_ms: report::round(t.ms, rounding),
            project_id: t.project_id,
            ms: t.ms,
            notes: t.notes,
        })
        .collect())
}

/// Écrit le CSV (une ligne par jour et par projet) et renvoie le nombre de lignes.
pub async fn export_csv(pool: &SqlitePool, from: i64, to: i64, rounding: u32, path: &Path, now: i64) -> AppResult<usize> {
    let rows = rows(pool, from, to, rounding, now).await?;
    let by_id: HashMap<String, Project> =
        projects::list(pool, true).await?.into_iter().map(|p| (p.id.clone(), p)).collect();
    let mut csv_rows: Vec<CsvRow> = rows
        .into_iter()
        .map(|r| {
            let project = by_id.get(&r.project_id);
            CsvRow {
                date: chrono::NaiveDate::parse_from_str(&r.date, "%Y-%m-%d").expect("date produite par le rapport"),
                project: project.map_or_else(|| "(projet supprimé)".to_string(), |p| p.name.clone()),
                client: project.map(|p| p.client.clone()).unwrap_or_default(),
                ms: r.rounded_ms,
                note: r.notes.join(" · "),
            }
        })
        .collect();
    csv_rows.sort_by(|a, b| a.date.cmp(&b.date).then_with(|| a.client.cmp(&b.client)).then_with(|| a.project.cmp(&b.project)));
    std::fs::write(path, report::to_csv(&csv_rows))
        .map_err(|e| AppError::Internal(format!("Écriture du fichier impossible : {e}")))?;
    Ok(csv_rows.len())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        db::{self, models::ProjectInput},
        services::entries::{EntryInput, save},
    };

    const MIN: i64 = 60_000;

    #[tokio::test]
    async fn export_csv_arrondi() {
        let pool = db::open_in_memory().await;
        let input = ProjectInput { name: "Site".into(), client: "ACME".into(), color: "#3366ff".into() };
        let p = db::projects::create(&pool, input, 0).await.unwrap().id;
        let day = chrono::Local::now().date_naive().pred_opt().unwrap();
        let t0 = trackify_core::durations::start_of_day(day, &Local) + 9 * 60 * MIN;
        let now = t0 + 24 * 60 * MIN;
        let entry = |start: i64, end: i64, note: &str| EntryInput {
            project_id: p.clone(),
            started_at: t0 + start * MIN,
            ended_at: Some(t0 + end * MIN),
            note: note.into(),
        };
        save(&pool, None, entry(0, 52, "Maquette"), now).await.unwrap();
        save(&pool, None, entry(60, 61, ""), now).await.unwrap();

        let r = rows(&pool, t0 - 9 * 60 * MIN, now, 15, now).await.unwrap();
        assert_eq!(r.len(), 1);
        assert_eq!((r[0].ms, r[0].rounded_ms), (53 * MIN, 60 * MIN));
        assert!(rows(&pool, 0, now, 7, now).await.is_err());

        let path = std::env::temp_dir().join(format!("trackify-{}.csv", db::new_id()));
        assert_eq!(export_csv(&pool, t0 - 9 * 60 * MIN, now, 15, &path, now).await.unwrap(), 1);
        let csv = std::fs::read_to_string(&path).unwrap();
        let _ = std::fs::remove_file(&path);
        assert_eq!(csv.lines().nth(1).unwrap(), format!("{};Site;ACME;1,00;Maquette", day.format("%Y-%m-%d")));
    }
}
