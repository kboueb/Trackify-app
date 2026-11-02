//! Suivi d'activité : fusion des échantillons en segments, et repérage du temps
//! actif qui n'est attribué à aucun projet (pour la revue de journée).

use std::collections::HashMap;

use crate::Millis;

/// Dernier segment enregistré, tel que vu par la fusion.
#[derive(Debug, Clone, Copy)]
pub struct Last<'a> {
    pub app: &'a str,
    pub title: &'a str,
    pub end: Millis,
}

/// Un échantillon (app + titre de fenêtre au premier plan, pris à `now`) prolonge
/// le dernier segment s'il montre la même fenêtre et qu'aucun trou ne les sépare
/// (veille, pause du suivi, absence).
pub fn extends(last: Option<Last>, app: &str, title: &str, now: Millis, max_gap: Millis) -> bool {
    last.is_some_and(|l| l.app == app && l.title == title && now >= l.end && now - l.end <= max_gap)
}

/// Segment d'activité stocké.
#[derive(Debug, Clone, Copy)]
pub struct Activity<'a> {
    pub start: Millis,
    pub end: Millis,
    pub app: &'a str,
    pub title: &'a str,
}

/// Plage d'activité sans projet, avec ce qui y a été fait (du plus long au plus court).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Unassigned {
    pub start: Millis,
    pub end: Millis,
    pub apps: Vec<(String, Millis)>,
    pub titles: Vec<(String, Millis)>,
}

const TOP: usize = 3;

/// Plages d'activité non couvertes par `entries` (intervalles `[début, fin)`).
///
/// Les morceaux libres séparés de moins de `join_gap` sont regroupés en un bloc,
/// sauf si une entrée se trouve entre eux. Les blocs plus courts que `min_len` sont ignorés.
pub fn unassigned(activity: &[Activity], entries: &[(Millis, Millis)], join_gap: Millis, min_len: Millis) -> Vec<Unassigned> {
    let mut covered: Vec<(Millis, Millis)> = entries.iter().copied().filter(|(s, e)| e > s).collect();
    covered.sort();
    let mut sorted: Vec<&Activity> = activity.iter().filter(|a| a.end > a.start).collect();
    sorted.sort_by_key(|a| a.start);

    // 1. Parties de chaque segment qui ne sont couvertes par aucune entrée.
    let mut free: Vec<(Millis, Millis, &Activity)> = Vec::new();
    for a in sorted {
        let mut cursor = a.start;
        for &(s, e) in &covered {
            if e <= cursor || s >= a.end {
                continue;
            }
            if s > cursor {
                free.push((cursor, s, a));
            }
            cursor = cursor.max(e);
            if cursor >= a.end {
                break;
            }
        }
        if cursor < a.end {
            free.push((cursor, a.end, a));
        }
    }

    // 2. Regroupement en blocs.
    let between_entry = |from: Millis, to: Millis| covered.iter().any(|&(s, e)| s < to && e > from);
    let mut blocks: Vec<Vec<(Millis, Millis, &Activity)>> = Vec::new();
    for part in free {
        match blocks.last_mut() {
            Some(block) => {
                let prev_end = block.iter().map(|p| p.1).max().unwrap_or(part.0);
                if part.0 - prev_end <= join_gap && !between_entry(prev_end, part.0) {
                    block.push(part);
                } else {
                    blocks.push(vec![part]);
                }
            }
            None => blocks.push(vec![part]),
        }
    }

    // 3. Résumé de chaque bloc.
    blocks
        .into_iter()
        .filter_map(|parts| {
            let start = parts.iter().map(|p| p.0).min()?;
            let end = parts.iter().map(|p| p.1).max()?;
            if end - start < min_len {
                return None;
            }
            let mut apps: HashMap<&str, Millis> = HashMap::new();
            let mut titles: HashMap<&str, Millis> = HashMap::new();
            for (s, e, a) in &parts {
                *apps.entry(a.app).or_default() += e - s;
                if !a.title.is_empty() {
                    *titles.entry(a.title).or_default() += e - s;
                }
            }
            Some(Unassigned { start, end, apps: top(apps), titles: top(titles) })
        })
        .collect()
}

fn top(map: HashMap<&str, Millis>) -> Vec<(String, Millis)> {
    let mut list: Vec<(String, Millis)> = map.into_iter().map(|(k, v)| (k.to_string(), v)).collect();
    list.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    list.truncate(TOP);
    list
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::durations::MINUTE;

    const S: Millis = 1_000;
    const GAP: Millis = 30 * S;

    fn act(start: Millis, end: Millis, app: &'static str, title: &'static str) -> Activity<'static> {
        Activity { start: start * MINUTE, end: end * MINUTE, app, title }
    }

    #[test]
    fn fusion_des_echantillons() {
        let last = Some(Last { app: "Figma", title: "Maquette", end: 100 * S });
        assert!(extends(last, "Figma", "Maquette", 110 * S, GAP));
        assert!(!extends(last, "Figma", "Autre fichier", 110 * S, GAP), "autre fenêtre");
        assert!(!extends(last, "Safari", "Maquette", 110 * S, GAP), "autre app");
        assert!(!extends(last, "Figma", "Maquette", 200 * S, GAP), "trou (veille, pause)");
        assert!(!extends(None, "Figma", "Maquette", 110 * S, GAP));
    }

    #[test]
    fn rien_quand_tout_est_couvert() {
        let a = [act(0, 60, "Figma", "Maquette")];
        assert!(unassigned(&a, &[(0, 60 * MINUTE)], 5 * MINUTE, MINUTE).is_empty());
    }

    #[test]
    fn trou_entre_deux_entrees() {
        let a = [act(0, 30, "Mail", "Boîte"), act(30, 50, "Figma", "Maquette"), act(50, 90, "Figma", "Logo")];
        let entries = [(0, 20 * MINUTE), (60 * MINUTE, 120 * MINUTE)];
        let gaps = unassigned(&a, &entries, 5 * MINUTE, MINUTE);
        assert_eq!(gaps.len(), 1);
        let g = &gaps[0];
        assert_eq!((g.start, g.end), (20 * MINUTE, 60 * MINUTE));
        assert_eq!(g.apps, vec![("Figma".to_string(), 30 * MINUTE), ("Mail".to_string(), 10 * MINUTE)]);
        assert_eq!(g.titles[0], ("Maquette".to_string(), 20 * MINUTE));
    }

    #[test]
    fn petits_trous_regroupes_mais_pas_par_dessus_une_entree() {
        let a = [act(0, 10, "Mail", ""), act(12, 20, "Mail", ""), act(22, 30, "Mail", "")];
        // Pause de 2 min sans activité entre les deux premiers : même bloc.
        // Une entrée de 2 min entre les deux derniers : deux blocs.
        let entries = [(20 * MINUTE, 22 * MINUTE)];
        let gaps = unassigned(&a, &entries, 5 * MINUTE, MINUTE);
        assert_eq!(gaps.iter().map(|g| (g.start, g.end)).collect::<Vec<_>>(), vec![
            (0, 20 * MINUTE),
            (22 * MINUTE, 30 * MINUTE)
        ]);
        assert_eq!(gaps[0].apps, vec![("Mail".to_string(), 18 * MINUTE)]);
        assert!(gaps[0].titles.is_empty());
    }

    #[test]
    fn blocs_trop_courts_ignores() {
        let a = [act(0, 2, "Mail", ""), act(30, 40, "Figma", "")];
        let gaps = unassigned(&a, &[], 5 * MINUTE, 5 * MINUTE);
        assert_eq!(gaps.len(), 1);
        assert_eq!(gaps[0].start, 30 * MINUTE);
    }

    #[test]
    fn entree_au_milieu_d_un_segment() {
        let a = [act(0, 60, "Figma", "Maquette")];
        let gaps = unassigned(&a, &[(20 * MINUTE, 40 * MINUTE)], 5 * MINUTE, MINUTE);
        assert_eq!(gaps.iter().map(|g| (g.start, g.end)).collect::<Vec<_>>(), vec![
            (0, 20 * MINUTE),
            (40 * MINUTE, 60 * MINUTE)
        ]);
    }
}
