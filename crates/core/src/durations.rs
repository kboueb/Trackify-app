//! Calcul des durées et découpage par jour local.

use chrono::{Duration, NaiveDate, NaiveDateTime, TimeZone};

/// Horodatage en millisecondes depuis l'époque Unix (UTC).
pub type Millis = i64;

pub const MINUTE: Millis = 60_000;
pub const HOUR: Millis = 60 * MINUTE;

/// Fin effective d'une entrée : une entrée en cours (`ended_at = None`) court jusqu'à `now`.
pub fn effective_end(ended_at: Option<Millis>, now: Millis) -> Millis {
    ended_at.unwrap_or(now)
}

/// Durée d'une entrée, jamais négative (horloge reculée, données incohérentes…).
pub fn duration(started_at: Millis, ended_at: Option<Millis>, now: Millis) -> Millis {
    (effective_end(ended_at, now) - started_at).max(0)
}

/// Longueur de l'intersection entre `[start, end)` et `[from, to)`.
pub fn overlap(start: Millis, end: Millis, from: Millis, to: Millis) -> Millis {
    (end.min(to) - start.max(from)).max(0)
}

/// Format chronomètre « H:MM:SS » (barre des menus).
pub fn format_hms(ms: Millis) -> String {
    let seconds = ms.max(0) / 1000;
    format!("{}:{:02}:{:02}", seconds / 3600, seconds / 60 % 60, seconds % 60)
}

/// Date locale (dans `tz`) d'un horodatage.
pub fn local_date<Tz: TimeZone>(ts: Millis, tz: &Tz) -> NaiveDate {
    tz.timestamp_millis_opt(ts)
        .single()
        .expect("horodatage hors limites")
        .date_naive()
}

/// Horodatage du début de la journée `date` dans `tz`.
///
/// Si minuit n'existe pas ce jour-là (passage à l'heure d'été à minuit dans certains
/// fuseaux), on prend le premier instant valide qui suit.
pub fn start_of_day<Tz: TimeZone>(date: NaiveDate, tz: &Tz) -> Millis {
    let midnight: NaiveDateTime = date.and_hms_opt(0, 0, 0).expect("minuit valide");
    let mut candidate = midnight;
    for _ in 0..4 {
        if let Some(dt) = tz.from_local_datetime(&candidate).earliest() {
            return dt.timestamp_millis();
        }
        candidate += Duration::minutes(30);
    }
    panic!("aucun instant valide pour le début du {date}");
}

/// Découpe `[start, end)` en morceaux par jour local, pour qu'une entrée
/// qui passe minuit soit comptée sur les bons jours.
pub fn split_by_day<Tz: TimeZone>(start: Millis, end: Millis, tz: &Tz) -> Vec<(NaiveDate, Millis)> {
    let mut parts = Vec::new();
    if end <= start {
        return parts;
    }
    let mut cursor = start;
    let mut day = local_date(start, tz);
    while cursor < end {
        let next_day = day.succ_opt().expect("date valide");
        let day_end = start_of_day(next_day, tz).min(end);
        parts.push((day, day_end - cursor));
        cursor = day_end;
        day = next_day;
    }
    parts
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono_tz::Europe::Paris;

    fn paris(y: i32, m: u32, d: u32, h: u32, min: u32) -> Millis {
        Paris
            .with_ymd_and_hms(y, m, d, h, min, 0)
            .single()
            .unwrap()
            .timestamp_millis()
    }

    fn date(y: i32, m: u32, d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, d).unwrap()
    }

    #[test]
    fn duree_entree_terminee() {
        assert_eq!(duration(1_000, Some(61_000), 999_999), MINUTE);
    }

    #[test]
    fn duree_entree_en_cours_utilise_now() {
        assert_eq!(duration(0, None, 2 * HOUR), 2 * HOUR);
    }

    #[test]
    fn duree_jamais_negative() {
        assert_eq!(duration(10_000, Some(5_000), 0), 0);
        assert_eq!(duration(10_000, None, 5_000), 0);
    }

    #[test]
    fn chevauchement_partiel_total_et_nul() {
        assert_eq!(overlap(0, 100, 50, 200), 50);
        assert_eq!(overlap(0, 100, 0, 100), 100);
        assert_eq!(overlap(0, 100, 100, 200), 0);
        assert_eq!(overlap(0, 100, 20, 30), 10);
    }

    #[test]
    fn date_locale_et_non_utc() {
        // 23h30 à Paris le 1er juillet = 21h30 UTC, toujours le 1er
        assert_eq!(local_date(paris(2026, 7, 1, 23, 30), &Paris), date(2026, 7, 1));
        // 0h30 à Paris le 2 juillet = 22h30 UTC le 1er : on veut le 2
        assert_eq!(local_date(paris(2026, 7, 2, 0, 30), &Paris), date(2026, 7, 2));
    }

    #[test]
    fn entree_dans_la_journee_un_seul_morceau() {
        let parts = split_by_day(paris(2026, 7, 1, 9, 0), paris(2026, 7, 1, 12, 30), &Paris);
        assert_eq!(parts, vec![(date(2026, 7, 1), 3 * HOUR + 30 * MINUTE)]);
    }

    #[test]
    fn entree_qui_passe_minuit_est_coupee() {
        let parts = split_by_day(paris(2026, 7, 1, 22, 0), paris(2026, 7, 2, 1, 15), &Paris);
        assert_eq!(
            parts,
            vec![(date(2026, 7, 1), 2 * HOUR), (date(2026, 7, 2), HOUR + 15 * MINUTE)]
        );
    }

    #[test]
    fn entree_sur_plusieurs_jours() {
        let parts = split_by_day(paris(2026, 7, 1, 12, 0), paris(2026, 7, 3, 12, 0), &Paris);
        assert_eq!(
            parts,
            vec![
                (date(2026, 7, 1), 12 * HOUR),
                (date(2026, 7, 2), 24 * HOUR),
                (date(2026, 7, 3), 12 * HOUR),
            ]
        );
    }

    #[test]
    fn jour_de_passage_heure_ete_dure_23h() {
        let parts = split_by_day(paris(2026, 3, 29, 0, 0), paris(2026, 3, 30, 0, 0), &Paris);
        assert_eq!(parts, vec![(date(2026, 3, 29), 23 * HOUR)]);
    }

    #[test]
    fn jour_de_passage_heure_hiver_dure_25h() {
        let parts = split_by_day(paris(2026, 10, 25, 0, 0), paris(2026, 10, 26, 0, 0), &Paris);
        assert_eq!(parts, vec![(date(2026, 10, 25), 25 * HOUR)]);
    }

    #[test]
    fn intervalle_vide_ou_inverse() {
        assert!(split_by_day(100, 100, &Paris).is_empty());
        assert!(split_by_day(200, 100, &Paris).is_empty());
    }

    #[test]
    fn format_chronometre() {
        assert_eq!(format_hms(0), "0:00:00");
        assert_eq!(format_hms(999), "0:00:00");
        assert_eq!(format_hms(5 * MINUTE + 9_000), "0:05:09");
        assert_eq!(format_hms(10 * HOUR + 7 * MINUTE + 59_999), "10:07:59");
        assert_eq!(format_hms(-HOUR), "0:00:00");
    }

    #[test]
    fn debut_de_journee() {
        assert_eq!(start_of_day(date(2026, 7, 2), &Paris), paris(2026, 7, 2, 0, 0));
    }
}
