//! Détection des absences et planification des rappels, à partir d'échantillons
//! périodiques (heure courante + temps d'inactivité clavier/souris).
//!
//! Tout est piloté par `tick` : pas d'horloge ni d'E/S ici, ce qui rend la logique testable.

use crate::{Millis, durations::MINUTE};

/// Détecte les absences pendant qu'un timer tourne.
#[derive(Debug, Default)]
pub struct IdleTracker {
    idle_since: Option<Millis>,
    last_tick: Option<Millis>,
}

impl IdleTracker {
    /// À appeler à chaque échantillon. Renvoie `Some((début, fin))` au moment où
    /// l'utilisateur revient après une absence d'au moins `threshold`.
    ///
    /// Une mise en veille se voit comme un trou entre deux échantillons : elle compte
    /// comme une absence qui commence au dernier échantillon avant la veille.
    pub fn tick(&mut self, now: Millis, idle: Millis, timer_running: bool, threshold: Millis) -> Option<(Millis, Millis)> {
        let previous = self.last_tick.replace(now);
        if !timer_running {
            self.idle_since = None;
            return None;
        }
        let last_input = now - idle;
        if self.idle_since.is_none() {
            if let Some(prev) = previous
                && now - prev >= threshold
            {
                self.idle_since = Some(prev.min(last_input));
            } else if idle >= threshold {
                self.idle_since = Some(last_input);
            }
        }
        match self.idle_since {
            Some(since) if idle < threshold => {
                self.idle_since = None;
                (last_input > since).then_some((since, last_input))
            }
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReminderConfig {
    pub idle_threshold: Millis,
    pub no_timer_enabled: bool,
    /// Activité continue sans timer avant le rappel.
    pub no_timer_after: Millis,
    /// Délai avant de redemander après un rappel « sans timer ».
    pub no_timer_repeat: Millis,
    pub checkin_enabled: bool,
    /// Durée de timer continu entre deux « Toujours sur… ? ».
    pub checkin_every: Millis,
    /// Plage horaire locale des rappels, en minutes depuis minuit : `[from, until)`.
    pub active_from_min: u32,
    pub active_until_min: u32,
    pub weekdays_only: bool,
}

impl Default for ReminderConfig {
    fn default() -> Self {
        Self {
            idle_threshold: 5 * MINUTE,
            no_timer_enabled: true,
            no_timer_after: 5 * MINUTE,
            no_timer_repeat: 30 * MINUTE,
            checkin_enabled: true,
            checkin_every: 60 * MINUTE,
            active_from_min: 8 * 60,
            active_until_min: 19 * 60,
            weekdays_only: true,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reminder {
    NoTimer,
    CheckIn,
}

/// Contexte local d'un échantillon : minute du jour et jour de la semaine (0 = lundi).
#[derive(Debug, Clone, Copy)]
pub struct LocalClock {
    pub minute_of_day: u32,
    pub weekday: u32,
}

#[derive(Debug, Default)]
pub struct Reminders {
    active_since: Option<Millis>,
    no_timer_not_before: Millis,
    /// (entrée suivie, prochain point prévu).
    checkin: Option<(String, Millis)>,
}

impl Reminders {
    pub fn tick(
        &mut self,
        cfg: &ReminderConfig,
        now: Millis,
        idle: Millis,
        running: Option<(&str, Millis)>,
        clock: LocalClock,
    ) -> Option<Reminder> {
        let active = idle < cfg.idle_threshold;
        let in_hours = (!cfg.weekdays_only || clock.weekday < 5)
            && clock.minute_of_day >= cfg.active_from_min
            && clock.minute_of_day < cfg.active_until_min;

        match running {
            None => {
                self.checkin = None;
                if !active {
                    self.active_since = None;
                    return None;
                }
                let since = *self.active_since.get_or_insert(now);
                if cfg.no_timer_enabled
                    && in_hours
                    && now - since >= cfg.no_timer_after
                    && now >= self.no_timer_not_before
                {
                    self.no_timer_not_before = now + cfg.no_timer_repeat;
                    return Some(Reminder::NoTimer);
                }
                None
            }
            Some((entry_id, started_at)) => {
                self.active_since = None;
                if self.checkin.as_ref().is_none_or(|(id, _)| id != entry_id) {
                    self.checkin = Some((entry_id.to_string(), started_at + cfg.checkin_every));
                }
                let (_, next) = self.checkin.as_mut().expect("initialisé juste au-dessus");
                if cfg.checkin_enabled && in_hours && active && now >= *next {
                    *next = now + cfg.checkin_every;
                    return Some(Reminder::CheckIn);
                }
                None
            }
        }
    }

    /// « Plus tard » sur un rappel sans timer.
    pub fn snooze_no_timer(&mut self, until: Millis) {
        self.no_timer_not_before = self.no_timer_not_before.max(until);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const S: Millis = 1_000;
    const T: Millis = 5 * MINUTE;
    const WORK: LocalClock = LocalClock { minute_of_day: 10 * 60, weekday: 1 };

    // --- IdleTracker ---

    #[test]
    fn absence_detectee_au_retour() {
        let mut t = IdleTracker::default();
        let base = 1_000 * MINUTE;
        assert_eq!(t.tick(base, 0, true, T), None);
        // 6 min sans toucher à rien
        assert_eq!(t.tick(base + 6 * MINUTE, 6 * MINUTE, true, T), None);
        assert_eq!(t.tick(base + 20 * MINUTE, 20 * MINUTE, true, T), None);
        // retour : dernière action il y a 2 s
        assert_eq!(t.tick(base + 30 * MINUTE, 2 * S, true, T), Some((base, base + 30 * MINUTE - 2 * S)));
        // pas de double signalement
        assert_eq!(t.tick(base + 30 * MINUTE + 5 * S, 3 * S, true, T), None);
    }

    #[test]
    fn courte_pause_ignoree() {
        let mut t = IdleTracker::default();
        assert_eq!(t.tick(0, 0, true, T), None);
        assert_eq!(t.tick(4 * MINUTE, 4 * MINUTE, true, T), None);
        assert_eq!(t.tick(4 * MINUTE + 5 * S, 0, true, T), None);
    }

    #[test]
    fn pas_d_absence_sans_timer() {
        let mut t = IdleTracker::default();
        t.tick(0, 0, false, T);
        assert_eq!(t.tick(30 * MINUTE, 30 * MINUTE, false, T), None);
        assert_eq!(t.tick(31 * MINUTE, 0, false, T), None);
    }

    #[test]
    fn timer_arrete_pendant_l_absence_annule() {
        let mut t = IdleTracker::default();
        t.tick(0, 0, true, T);
        t.tick(10 * MINUTE, 10 * MINUTE, true, T);
        assert_eq!(t.tick(11 * MINUTE, 11 * MINUTE, false, T), None);
        assert_eq!(t.tick(12 * MINUTE, 0, true, T), None);
    }

    #[test]
    fn mise_en_veille_vue_comme_absence() {
        let mut t = IdleTracker::default();
        let base = 1_000 * MINUTE;
        t.tick(base, 10 * S, true, T);
        // Réveil 2 h plus tard, l'utilisateur vient de taper son mot de passe.
        let wake = base + 120 * MINUTE;
        assert_eq!(t.tick(wake, S, true, T), Some((base, wake - S)));
    }

    // --- Reminders ---

    #[test]
    fn rappel_sans_timer_apres_activite_continue() {
        let cfg = ReminderConfig::default();
        let mut r = Reminders::default();
        assert_eq!(r.tick(&cfg, 0, 0, None, WORK), None);
        assert_eq!(r.tick(&cfg, 4 * MINUTE, 10 * S, None, WORK), None);
        assert_eq!(r.tick(&cfg, 5 * MINUTE, 10 * S, None, WORK), Some(Reminder::NoTimer));
        // pas de répétition avant 30 min
        assert_eq!(r.tick(&cfg, 10 * MINUTE, 0, None, WORK), None);
        assert_eq!(r.tick(&cfg, 35 * MINUTE, 0, None, WORK), Some(Reminder::NoTimer));
    }

    #[test]
    fn absence_remet_a_zero_l_activite() {
        let cfg = ReminderConfig::default();
        let mut r = Reminders::default();
        r.tick(&cfg, 0, 0, None, WORK);
        r.tick(&cfg, 4 * MINUTE, 0, None, WORK);
        r.tick(&cfg, 15 * MINUTE, 10 * MINUTE, None, WORK); // parti
        assert_eq!(r.tick(&cfg, 16 * MINUTE, 0, None, WORK), None); // de retour, le compteur repart
        assert_eq!(r.tick(&cfg, 21 * MINUTE, 0, None, WORK), Some(Reminder::NoTimer));
    }

    #[test]
    fn pas_de_rappel_hors_horaires_ou_week_end() {
        let cfg = ReminderConfig::default();
        let soir = LocalClock { minute_of_day: 21 * 60, weekday: 1 };
        let samedi = LocalClock { minute_of_day: 10 * 60, weekday: 5 };
        for clock in [soir, samedi] {
            let mut r = Reminders::default();
            r.tick(&cfg, 0, 0, None, clock);
            assert_eq!(r.tick(&cfg, 60 * MINUTE, 0, None, clock), None);
        }
        let cfg = ReminderConfig { weekdays_only: false, ..cfg };
        let mut r = Reminders::default();
        r.tick(&cfg, 0, 0, None, samedi);
        assert_eq!(r.tick(&cfg, 6 * MINUTE, 0, None, samedi), Some(Reminder::NoTimer));
    }

    #[test]
    fn plus_tard_repousse_le_rappel() {
        let cfg = ReminderConfig::default();
        let mut r = Reminders::default();
        r.tick(&cfg, 0, 0, None, WORK);
        r.snooze_no_timer(60 * MINUTE);
        assert_eq!(r.tick(&cfg, 40 * MINUTE, 0, None, WORK), None);
        assert_eq!(r.tick(&cfg, 60 * MINUTE, 0, None, WORK), Some(Reminder::NoTimer));
    }

    #[test]
    fn point_regulier_toutes_les_heures_de_timer() {
        let cfg = ReminderConfig::default();
        let mut r = Reminders::default();
        let run = Some(("e1", 0));
        assert_eq!(r.tick(&cfg, 30 * MINUTE, 0, run, WORK), None);
        assert_eq!(r.tick(&cfg, 60 * MINUTE, 0, run, WORK), Some(Reminder::CheckIn));
        assert_eq!(r.tick(&cfg, 90 * MINUTE, 0, run, WORK), None);
        assert_eq!(r.tick(&cfg, 120 * MINUTE, 0, run, WORK), Some(Reminder::CheckIn));
    }

    #[test]
    fn changer_de_projet_relance_le_compte() {
        let cfg = ReminderConfig::default();
        let mut r = Reminders::default();
        r.tick(&cfg, 50 * MINUTE, 0, Some(("e1", 0)), WORK);
        assert_eq!(r.tick(&cfg, 60 * MINUTE, 0, Some(("e2", 55 * MINUTE)), WORK), None);
        assert_eq!(r.tick(&cfg, 115 * MINUTE, 0, Some(("e2", 55 * MINUTE)), WORK), Some(Reminder::CheckIn));
    }

    #[test]
    fn pas_de_point_si_absent() {
        let cfg = ReminderConfig::default();
        let mut r = Reminders::default();
        assert_eq!(r.tick(&cfg, 61 * MINUTE, 10 * MINUTE, Some(("e1", 0)), WORK), None);
        // au retour, le point arrive
        assert_eq!(r.tick(&cfg, 62 * MINUTE, 0, Some(("e1", 0)), WORK), Some(Reminder::CheckIn));
    }

    #[test]
    fn rappels_desactives() {
        let cfg = ReminderConfig { no_timer_enabled: false, checkin_enabled: false, ..Default::default() };
        let mut r = Reminders::default();
        r.tick(&cfg, 0, 0, None, WORK);
        assert_eq!(r.tick(&cfg, 60 * MINUTE, 0, None, WORK), None);
        assert_eq!(r.tick(&cfg, 200 * MINUTE, 0, Some(("e1", 0)), WORK), None);
    }
}
