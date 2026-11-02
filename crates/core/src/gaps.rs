//! Retrait d'une plage (absence) d'une entrée de temps.

use crate::Millis;

/// Une entrée vue comme un intervalle ; `end = None` si le timer tourne.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Span {
    pub start: Millis,
    pub end: Option<Millis>,
}

/// Effet du retrait d'une plage sur une entrée.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GapOutcome {
    /// La plage ne recoupe pas l'entrée.
    Unchanged,
    Split {
        /// Partie conservée avant la plage (`None` : trop courte, l'entrée est supprimée).
        before: Option<(Millis, Millis)>,
        /// Partie après la plage. Si l'entrée tournait, elle continue de tourner à partir de la fin de la plage.
        after: Option<Span>,
        /// Ce qui a effectivement été retiré (plage bornée à l'entrée).
        removed: (Millis, Millis),
    },
}

/// Retire `[gap_start, gap_end)` de `entry`. Les morceaux terminés plus courts que
/// `min_len` sont abandonnés ; un morceau en cours est toujours gardé.
pub fn remove_gap(entry: Span, gap_start: Millis, gap_end: Millis, min_len: Millis) -> GapOutcome {
    let start = gap_start.max(entry.start);
    let end = match entry.end {
        Some(e) => gap_end.min(e),
        None => gap_end,
    };
    if end <= start {
        return GapOutcome::Unchanged;
    }
    let before = (start - entry.start >= min_len).then_some((entry.start, start));
    let after = match entry.end {
        None => Some(Span { start: end, end: None }),
        Some(e) => (e - end >= min_len).then_some(Span { start: end, end: Some(e) }),
    };
    GapOutcome::Split { before, after, removed: (start, end) }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MIN: Millis = 60_000;
    const KEEP: Millis = 5_000;

    #[test]
    fn plage_hors_entree() {
        let e = Span { start: 10 * MIN, end: Some(20 * MIN) };
        assert_eq!(remove_gap(e, 0, 5 * MIN, KEEP), GapOutcome::Unchanged);
        assert_eq!(remove_gap(e, 20 * MIN, 30 * MIN, KEEP), GapOutcome::Unchanged);
    }

    #[test]
    fn timer_en_cours_coupe_et_continue() {
        let e = Span { start: 0, end: None };
        assert_eq!(
            remove_gap(e, 30 * MIN, 45 * MIN, KEEP),
            GapOutcome::Split {
                before: Some((0, 30 * MIN)),
                after: Some(Span { start: 45 * MIN, end: None }),
                removed: (30 * MIN, 45 * MIN),
            }
        );
    }

    #[test]
    fn entree_terminee_coupee_en_deux() {
        let e = Span { start: 0, end: Some(60 * MIN) };
        assert_eq!(
            remove_gap(e, 20 * MIN, 30 * MIN, KEEP),
            GapOutcome::Split {
                before: Some((0, 20 * MIN)),
                after: Some(Span { start: 30 * MIN, end: Some(60 * MIN) }),
                removed: (20 * MIN, 30 * MIN),
            }
        );
    }

    #[test]
    fn plage_qui_deborde_est_bornee() {
        // Absence commencée avant l'entrée et finie après : tout est retiré.
        let e = Span { start: 10 * MIN, end: Some(20 * MIN) };
        assert_eq!(
            remove_gap(e, 0, 30 * MIN, KEEP),
            GapOutcome::Split { before: None, after: None, removed: (10 * MIN, 20 * MIN) }
        );
    }

    #[test]
    fn morceaux_trop_courts_abandonnes() {
        let e = Span { start: 0, end: Some(60 * MIN) };
        assert_eq!(
            remove_gap(e, 2_000, 60 * MIN - 1_000, KEEP),
            GapOutcome::Split { before: None, after: None, removed: (2_000, 60 * MIN - 1_000) }
        );
    }

    #[test]
    fn morceau_en_cours_toujours_garde() {
        let e = Span { start: 0, end: None };
        let out = remove_gap(e, 0, 10 * MIN, KEEP);
        assert_eq!(
            out,
            GapOutcome::Split {
                before: None,
                after: Some(Span { start: 10 * MIN, end: None }),
                removed: (0, 10 * MIN),
            }
        );
    }
}
