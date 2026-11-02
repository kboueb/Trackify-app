//! Événements envoyés au frontend pour qu'il rafraîchisse ses données.

use serde::{Deserialize, Serialize};
use specta::Type;
use tauri_specta::Event;

/// Le timer a démarré, changé de projet ou s'est arrêté.
#[derive(Debug, Clone, Serialize, Deserialize, Type, Event)]
pub struct TimerChanged;

/// Un projet a été créé, modifié, archivé ou supprimé.
#[derive(Debug, Clone, Serialize, Deserialize, Type, Event)]
pub struct ProjectsChanged;

/// Une entrée de temps a été modifiée.
#[derive(Debug, Clone, Serialize, Deserialize, Type, Event)]
pub struct EntriesChanged;

/// Le sélecteur rapide vient d'être affiché : il doit se réinitialiser.
#[derive(Debug, Clone, Serialize, Deserialize, Type, Event)]
pub struct PaletteOpened;

/// Une absence a été détectée ou traitée.
#[derive(Debug, Clone, Serialize, Deserialize, Type, Event)]
pub struct IdleChanged;

/// Un rappel vient d'être affiché dans la fenêtre de rappel.
#[derive(Debug, Clone, Serialize, Deserialize, Type, Event)]
pub struct ReminderShown;

/// L'utilisateur veut revoir sa journée (rappel de fin de journée) : la fenêtre
/// principale affiche la journée du jour.
#[derive(Debug, Clone, Serialize, Deserialize, Type, Event)]
pub struct ReviewRequested;

/// Le suivi d'activité a été mis en pause ou relancé (depuis le tray ou les réglages).
#[derive(Debug, Clone, Serialize, Deserialize, Type, Event)]
pub struct SettingsChanged;
