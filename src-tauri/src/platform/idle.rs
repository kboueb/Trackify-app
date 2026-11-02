//! Temps écoulé depuis la dernière action clavier/souris de l'utilisateur.
//! Aucune de ces API ne demande de permission système.

use std::time::Duration;

#[cfg(target_os = "macos")]
pub fn idle_time() -> Option<Duration> {
    // CoreGraphics : état combiné de la session, tous types d'événements d'entrée.
    const COMBINED_SESSION_STATE: i32 = 0;
    const ANY_INPUT_EVENT: u32 = u32::MAX;
    #[link(name = "CoreGraphics", kind = "framework")]
    unsafe extern "C" {
        fn CGEventSourceSecondsSinceLastEventType(source_state: i32, event_type: u32) -> f64;
    }
    let seconds = unsafe { CGEventSourceSecondsSinceLastEventType(COMBINED_SESSION_STATE, ANY_INPUT_EVENT) };
    (seconds.is_finite() && seconds >= 0.0).then(|| Duration::from_secs_f64(seconds))
}

#[cfg(target_os = "windows")]
pub fn idle_time() -> Option<Duration> {
    use windows_sys::Win32::{
        System::SystemInformation::GetTickCount,
        UI::Input::KeyboardAndMouse::{GetLastInputInfo, LASTINPUTINFO},
    };
    let mut info = LASTINPUTINFO { cbSize: size_of::<LASTINPUTINFO>() as u32, dwTime: 0 };
    if unsafe { GetLastInputInfo(&mut info) } == 0 {
        return None;
    }
    // Les deux compteurs rebouclent tous les ~49 jours : wrapping_sub reste juste.
    let ms = unsafe { GetTickCount() }.wrapping_sub(info.dwTime);
    Some(Duration::from_millis(u64::from(ms)))
}

#[cfg(target_os = "linux")]
pub fn idle_time() -> Option<Duration> {
    // X11 uniquement ; sous Wayland, l'information n'est pas exposée de façon standard.
    user_idle::UserIdle::get_time().ok().map(|t| t.duration())
}

#[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
pub fn idle_time() -> Option<Duration> {
    None
}

#[cfg(test)]
mod tests {
    #[test]
    #[cfg(target_os = "macos")]
    fn renvoie_une_duree_plausible() {
        let idle = super::idle_time().expect("CoreGraphics doit répondre");
        assert!(idle.as_secs() < 365 * 24 * 3600);
    }
}
