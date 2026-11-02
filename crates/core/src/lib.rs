//! Logique métier pure de Trackify.
//!
//! Ce crate ne dépend ni de Tauri ni de la base de données : il ne manipule que des
//! horodatages (millisecondes UTC) et des structures simples, ce qui le rend facile à tester.

pub mod durations;
pub mod gaps;
pub mod presence;

pub use durations::Millis;
