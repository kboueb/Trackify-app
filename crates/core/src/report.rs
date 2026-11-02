//! Rapports : totaux par jour et par projet, arrondi, export CSV.

use std::collections::BTreeMap;

use chrono::{NaiveDate, TimeZone};

use crate::{
    Millis,
    durations::{MINUTE, split_by_day},
};

/// Une entrée vue par le rapport (déjà terminée, ou bornée à « maintenant »).
#[derive(Debug, Clone, Copy)]
pub struct Interval<'a> {
    pub project_id: &'a str,
    pub start: Millis,
    pub end: Millis,
    pub note: &'a str,
}

/// Temps passé sur un projet un jour donné, avec les notes des entrées concernées.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DayTotal {
    pub date: NaiveDate,
    pub project_id: String,
    pub ms: Millis,
    /// Notes non vides, sans doublon, dans l'ordre chronologique.
    pub notes: Vec<String>,
}

/// Totaux par (jour local, projet) des entrées bornées à `[from, to)`.
/// Une entrée qui passe minuit est répartie sur les deux jours.
pub fn daily_totals<Tz: TimeZone>(entries: &[Interval], from: Millis, to: Millis, tz: &Tz) -> Vec<DayTotal> {
    let mut sorted: Vec<&Interval> = entries.iter().collect();
    sorted.sort_by_key(|e| e.start);
    let mut totals: BTreeMap<(NaiveDate, &str), DayTotal> = BTreeMap::new();
    for e in sorted {
        let (start, end) = (e.start.max(from), e.end.min(to));
        for (date, ms) in split_by_day(start, end, tz) {
            let total = totals.entry((date, e.project_id)).or_insert_with(|| DayTotal {
                date,
                project_id: e.project_id.to_string(),
                ms: 0,
                notes: Vec::new(),
            });
            total.ms += ms;
            let note = e.note.trim();
            if !note.is_empty() && !total.notes.iter().any(|n| n == note) {
                total.notes.push(note.to_string());
            }
        }
    }
    totals.into_values().collect()
}

/// Arrondit au multiple de `step_min` minutes le plus proche (0 : pas d'arrondi).
/// Un temps non nul n'est jamais arrondi à zéro : il compte au moins un pas,
/// pour ne pas faire disparaître une ligne du timesheet.
pub fn round(ms: Millis, step_min: u32) -> Millis {
    if step_min == 0 || ms <= 0 {
        return ms.max(0);
    }
    let step = i64::from(step_min) * MINUTE;
    let rounded = (ms + step / 2) / step * step;
    rounded.max(step)
}

/// Heures décimales à la française : 5 400 000 ms → « 1,50 ».
pub fn decimal_hours(ms: Millis) -> String {
    let hundredths = (ms.max(0) * 100 + 1_800_000) / 3_600_000;
    format!("{},{:02}", hundredths / 100, hundredths % 100)
}

/// Une ligne du fichier exporté.
#[derive(Debug, Clone)]
pub struct CsvRow {
    pub date: NaiveDate,
    pub project: String,
    pub client: String,
    pub ms: Millis,
    pub note: String,
}

/// CSV pour tableur français : BOM UTF-8 (Excel), séparateur `;`, virgule décimale.
pub fn to_csv(rows: &[CsvRow]) -> String {
    let mut out = String::from("\u{feff}date;projet;client;durée (h);note\r\n");
    for r in rows {
        let fields = [
            r.date.format("%Y-%m-%d").to_string(),
            r.project.clone(),
            r.client.clone(),
            decimal_hours(r.ms),
            r.note.clone(),
        ];
        let line: Vec<String> = fields.iter().map(|f| escape(f)).collect();
        out.push_str(&line.join(";"));
        out.push_str("\r\n");
    }
    out
}

fn escape(field: &str) -> String {
    if field.contains([';', '"', '\n', '\r']) {
        format!("\"{}\"", field.replace('"', "\"\""))
    } else {
        field.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono_tz::Europe::Paris;

    fn paris(d: u32, h: u32, m: u32) -> Millis {
        Paris.with_ymd_and_hms(2026, 3, d, h, m, 0).single().unwrap().timestamp_millis()
    }

    fn date(d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, 3, d).unwrap()
    }

    #[test]
    fn totaux_par_jour_et_projet() {
        let entries = [
            Interval { project_id: "a", start: paris(2, 9, 0), end: paris(2, 10, 0), note: "Maquette" },
            Interval { project_id: "b", start: paris(2, 10, 0), end: paris(2, 10, 30), note: "" },
            Interval { project_id: "a", start: paris(2, 14, 0), end: paris(2, 14, 45), note: "Maquette" },
            Interval { project_id: "a", start: paris(2, 15, 0), end: paris(2, 15, 15), note: "Revue" },
        ];
        let totals = daily_totals(&entries, paris(2, 0, 0), paris(3, 0, 0), &Paris);
        assert_eq!(totals.len(), 2);
        assert_eq!(totals[0].project_id, "a");
        assert_eq!(totals[0].ms, 120 * MINUTE);
        assert_eq!(totals[0].notes, vec!["Maquette", "Revue"]);
        assert_eq!(totals[1].ms, 30 * MINUTE);
    }

    #[test]
    fn entree_qui_passe_minuit_et_bornes() {
        let entries = [Interval { project_id: "a", start: paris(2, 23, 0), end: paris(3, 1, 30), note: "" }];
        let totals = daily_totals(&entries, paris(1, 0, 0), paris(8, 0, 0), &Paris);
        assert_eq!(totals.iter().map(|t| (t.date, t.ms)).collect::<Vec<_>>(), vec![
            (date(2), 60 * MINUTE),
            (date(3), 90 * MINUTE)
        ]);
        // Seule la partie dans la période compte.
        let only_3 = daily_totals(&entries, paris(3, 0, 0), paris(4, 0, 0), &Paris);
        assert_eq!(only_3.len(), 1);
        assert_eq!(only_3[0].ms, 90 * MINUTE);
    }

    #[test]
    fn arrondis() {
        assert_eq!(round(52 * MINUTE + 10_000, 0), 52 * MINUTE + 10_000);
        assert_eq!(round(52 * MINUTE, 5), 50 * MINUTE);
        assert_eq!(round(53 * MINUTE, 5), 55 * MINUTE);
        assert_eq!(round(52 * MINUTE, 15), 45 * MINUTE);
        assert_eq!(round(53 * MINUTE, 15), 60 * MINUTE, "7 min 30 = moitié : on arrondit au-dessus");
        assert_eq!(round(44 * MINUTE, 30), 30 * MINUTE);
        assert_eq!(round(45 * MINUTE, 30), 60 * MINUTE);
        assert_eq!(round(3 * MINUTE, 15), 15 * MINUTE, "jamais zéro");
        assert_eq!(round(0, 15), 0);
    }

    #[test]
    fn heures_decimales() {
        assert_eq!(decimal_hours(0), "0,00");
        assert_eq!(decimal_hours(90 * MINUTE), "1,50");
        assert_eq!(decimal_hours(20 * MINUTE), "0,33");
        assert_eq!(decimal_hours(8 * 60 * MINUTE + 45 * MINUTE), "8,75");
    }

    #[test]
    fn csv_echappe_et_bom() {
        let rows = [CsvRow {
            date: date(2),
            project: "Site; v2".into(),
            client: "ACME".into(),
            ms: 75 * MINUTE,
            note: "Dit \"ok\"".into(),
        }];
        let csv = to_csv(&rows);
        assert!(csv.starts_with('\u{feff}'));
        let line = csv.lines().nth(1).unwrap();
        assert_eq!(line, "2026-03-02;\"Site; v2\";ACME;1,25;\"Dit \"\"ok\"\"\"");
    }
}
