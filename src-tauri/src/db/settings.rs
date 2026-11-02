//! Réglages de l'application, stockés en JSON sous une clé unique.

use serde::{Deserialize, Serialize};
use specta::Type;
use sqlx::SqliteExecutor;

use trackify_core::{durations::MINUTE, presence::ReminderConfig};

use crate::error::{AppError, AppResult};

const KEY: &str = "app";
const HEARTBEAT_KEY: &str = "heartbeat";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    /// Raccourci global du sélecteur rapide, au format accélérateur Tauri.
    pub shortcut: String,
    /// Vrai une fois l'autostart activé au premier lancement : on ne le réactive
    /// plus ensuite si l'utilisateur l'a coupé.
    pub autostart_initialized: bool,
    pub reminders: ReminderSettings,
    /// Arrondi des rapports et de l'export, en minutes (0, 5, 15 ou 30).
    pub rounding_min: u32,
    /// Suivi de la fenêtre active (désactivable à tout moment).
    pub activity_enabled: bool,
    /// Dossier des sauvegardes quotidiennes ; `None` : à côté de la base.
    pub backup_dir: Option<String>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            // ⌥⌘Espace est pris par la recherche Finder sur macOS.
            shortcut: "CommandOrControl+Shift+Space".into(),
            autostart_initialized: false,
            reminders: ReminderSettings::default(),
            rounding_min: 0,
            activity_enabled: true,
            backup_dir: None,
        }
    }
}

/// Inactivité et rappels anti-oubli. Durées en minutes, heures en minutes depuis minuit.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase", default)]
pub struct ReminderSettings {
    pub idle_threshold_min: u32,
    pub no_timer_enabled: bool,
    pub no_timer_after_min: u32,
    pub checkin_enabled: bool,
    pub checkin_every_min: u32,
    pub active_from_min: u32,
    pub active_until_min: u32,
    pub weekdays_only: bool,
    pub review_enabled: bool,
    pub review_at_min: u32,
}

impl Default for ReminderSettings {
    fn default() -> Self {
        Self::from_config(&ReminderConfig::default())
    }
}

impl ReminderSettings {
    fn from_config(c: &ReminderConfig) -> Self {
        let min = |ms: i64| (ms / MINUTE) as u32;
        Self {
            idle_threshold_min: min(c.idle_threshold),
            no_timer_enabled: c.no_timer_enabled,
            no_timer_after_min: min(c.no_timer_after),
            checkin_enabled: c.checkin_enabled,
            checkin_every_min: min(c.checkin_every),
            active_from_min: c.active_from_min,
            active_until_min: c.active_until_min,
            weekdays_only: c.weekdays_only,
            review_enabled: c.review_enabled,
            review_at_min: c.review_at_min,
        }
    }

    pub fn to_config(&self) -> ReminderConfig {
        let ms = |m: u32| i64::from(m) * MINUTE;
        ReminderConfig {
            idle_threshold: ms(self.idle_threshold_min),
            no_timer_enabled: self.no_timer_enabled,
            no_timer_after: ms(self.no_timer_after_min),
            checkin_enabled: self.checkin_enabled,
            checkin_every: ms(self.checkin_every_min),
            active_from_min: self.active_from_min,
            active_until_min: self.active_until_min,
            weekdays_only: self.weekdays_only,
            review_enabled: self.review_enabled,
            review_at_min: self.review_at_min,
            ..ReminderConfig::default()
        }
    }

    pub fn validate(&self) -> AppResult<()> {
        let check = |ok: bool, msg: &str| if ok { Ok(()) } else { Err(AppError::Validation(msg.into())) };
        check((1..=240).contains(&self.idle_threshold_min), "Le délai d'inactivité doit être entre 1 et 240 minutes.")?;
        check((1..=240).contains(&self.no_timer_after_min), "Le délai du rappel sans timer doit être entre 1 et 240 minutes.")?;
        check((5..=480).contains(&self.checkin_every_min), "L'intervalle du point régulier doit être entre 5 et 480 minutes.")?;
        check(
            self.active_from_min < self.active_until_min && self.active_until_min <= 24 * 60,
            "La plage horaire des rappels est invalide.",
        )?;
        check(self.review_at_min < 24 * 60, "L'heure de la revue de journée est invalide.")
    }
}

/// Réglages enregistrés, complétés par les valeurs par défaut pour les champs absents.
pub async fn load(db: impl SqliteExecutor<'_>) -> AppResult<Settings> {
    let raw: Option<String> = sqlx::query_scalar("SELECT value FROM settings WHERE key = ?")
        .bind(KEY)
        .fetch_optional(db)
        .await?;
    // Un JSON illisible (version future, édition manuelle) ne doit pas bloquer l'app.
    Ok(raw.and_then(|r| serde_json::from_str(&r).ok()).unwrap_or_default())
}

pub async fn save(db: impl SqliteExecutor<'_>, settings: &Settings) -> AppResult<()> {
    let json = serde_json::to_string(settings).expect("réglages sérialisables");
    sqlx::query("INSERT INTO settings (key, value) VALUES (?, ?) ON CONFLICT(key) DO UPDATE SET value = excluded.value")
        .bind(KEY)
        .bind(json)
        .execute(db)
        .await?;
    Ok(())
}

/// Dernier signe de vie de l'app : sert à détecter, au lancement suivant,
/// qu'elle a été fermée (ou a planté) alors qu'un timer tournait.
pub async fn heartbeat(db: impl SqliteExecutor<'_>) -> AppResult<Option<i64>> {
    let raw: Option<String> = sqlx::query_scalar("SELECT value FROM settings WHERE key = ?")
        .bind(HEARTBEAT_KEY)
        .fetch_optional(db)
        .await?;
    Ok(raw.and_then(|r| r.parse().ok()))
}

pub async fn set_heartbeat(db: impl SqliteExecutor<'_>, now: i64) -> AppResult<()> {
    sqlx::query("INSERT INTO settings (key, value) VALUES (?, ?) ON CONFLICT(key) DO UPDATE SET value = excluded.value")
        .bind(HEARTBEAT_KEY)
        .bind(now.to_string())
        .execute(db)
        .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db;

    #[tokio::test]
    async fn valeurs_par_defaut_si_rien_en_base() {
        let pool = db::open_in_memory().await;
        assert_eq!(load(&pool).await.unwrap(), Settings::default());
    }

    #[tokio::test]
    async fn aller_retour() {
        let pool = db::open_in_memory().await;
        let s = Settings {
            shortcut: "Alt+KeyT".into(),
            autostart_initialized: true,
            reminders: ReminderSettings { checkin_every_min: 90, ..Default::default() },
            rounding_min: 15,
            activity_enabled: false,
            backup_dir: Some("/tmp/sauvegardes".into()),
        };
        save(&pool, &s).await.unwrap();
        save(&pool, &s).await.unwrap();
        assert_eq!(load(&pool).await.unwrap(), s);
    }

    #[tokio::test]
    async fn champs_manquants_completes() {
        let pool = db::open_in_memory().await;
        sqlx::query("INSERT INTO settings (key, value) VALUES ('app', '{\"shortcut\":\"Alt+KeyT\"}')")
            .execute(&pool)
            .await
            .unwrap();
        let s = load(&pool).await.unwrap();
        assert_eq!(s.shortcut, "Alt+KeyT");
        assert!(!s.autostart_initialized);
    }

    #[tokio::test]
    async fn json_corrompu_ignore() {
        let pool = db::open_in_memory().await;
        sqlx::query("INSERT INTO settings (key, value) VALUES ('app', 'pas du json')")
            .execute(&pool)
            .await
            .unwrap();
        assert_eq!(load(&pool).await.unwrap(), Settings::default());
    }

    #[test]
    fn conversion_reglages_config() {
        let r = ReminderSettings::default();
        assert_eq!(r.idle_threshold_min, 5);
        assert_eq!(r.checkin_every_min, 60);
        assert_eq!(r.to_config(), ReminderConfig::default());
    }

    #[test]
    fn validation_des_rappels() {
        assert!(ReminderSettings::default().validate().is_ok());
        let bad = ReminderSettings { idle_threshold_min: 0, ..Default::default() };
        assert!(bad.validate().is_err());
        let bad = ReminderSettings { active_from_min: 600, active_until_min: 500, ..Default::default() };
        assert!(bad.validate().is_err());
    }

    #[tokio::test]
    async fn battement_de_coeur() {
        let pool = db::open_in_memory().await;
        assert_eq!(heartbeat(&pool).await.unwrap(), None);
        set_heartbeat(&pool, 42).await.unwrap();
        set_heartbeat(&pool, 43).await.unwrap();
        assert_eq!(heartbeat(&pool).await.unwrap(), Some(43));
    }
}
