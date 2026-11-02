//! Icône de la barre des menus / zone de notification.
//!
//! Le menu est reconstruit à chaque changement (timer, projets) ; le titre
//! (« Projet · 1:23:45 ») est rafraîchi chaque seconde par une boucle qui ne touche pas la base.

use std::{sync::Mutex, time::Duration};

use tauri::{
    AppHandle, Manager,
    image::Image,
    menu::{CheckMenuItem, Menu, MenuEvent, MenuItem, PredefinedMenuItem},
    tray::TrayIconBuilder,
};
use trackify_core::durations::format_hms;

use crate::{
    AppState,
    db::{entries, now_ms, projects},
    error::AppResult,
    notify,
    services::timer,
    windows,
};

const TRAY_ID: &str = "main";
const RECENT_COUNT: i64 = 8;
const NAME_MAX: usize = 24;
// Logo « Time Marker » (sources dans design/brand) : deux barres et un point « maintenant ».
const ICON_IDLE: &[u8] = include_bytes!("../icons/tray-idle.png");
const ICON_BARS: &[u8] = include_bytes!("../icons/tray-bars.png");
const ICON_DOT: &[u8] = include_bytes!("../icons/tray-dot.png");
/// Le point « maintenant » garde la couleur d'accent du logo, quel que soit le projet.
const DOT_COLOR: [u8; 3] = [0xff, 0x4d, 0x61];

/// Ce que la boucle d'affichage doit montrer : (début du timer, nom du projet).
#[derive(Default)]
struct TrayStatus {
    running: Mutex<Option<(i64, String)>>,
    last_title: Mutex<String>,
    /// Couleur de l'icône affichée (`None` : icône estompée, aucun timer).
    icon_color: Mutex<Option<String>>,
}

pub fn create(app: &AppHandle) -> tauri::Result<()> {
    app.manage(TrayStatus::default());
    let (icon, template) = icon_for(None)?;
    TrayIconBuilder::with_id(TRAY_ID)
        .icon(icon)
        .icon_as_template(template)
        .tooltip("Trackify")
        .show_menu_on_left_click(true)
        .on_menu_event(on_menu_event)
        .build(app)?;

    refresh(app);
    let handle = app.clone();
    tauri::async_runtime::spawn(async move {
        let mut tick = tokio::time::interval(Duration::from_secs(1));
        tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            tick.tick().await;
            update_title(&handle);
        }
    });
    Ok(())
}

/// Relit l'état en base et reconstruit le menu (en tâche de fond).
pub fn refresh(app: &AppHandle) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        if let Err(e) = rebuild(&app).await {
            eprintln!("tray : mise à jour impossible : {e}");
        }
    });
}

async fn rebuild(app: &AppHandle) -> AppResult<()> {
    let db = &app.state::<AppState>().db;
    let running = entries::running(db).await?;
    let current = match &running {
        Some(e) => projects::get(db, &e.project_id).await.ok(),
        None => None,
    };
    let recent = projects::recent(db, RECENT_COUNT).await?;

    let menu = Menu::new(app)?;
    let status = match (&running, &current) {
        (Some(e), Some(p)) => format!("En cours : {} (depuis {})", truncate(&p.name), local_hm(e.started_at)),
        _ => "Aucun timer en cours".to_string(),
    };
    menu.append(&MenuItem::with_id(app, "status", status, false, None::<&str>)?)?;
    if running.is_some() {
        menu.append(&MenuItem::with_id(app, "stop", "Arrêter le timer", true, None::<&str>)?)?;
    }
    menu.append(&PredefinedMenuItem::separator(app)?)?;

    if recent.is_empty() {
        menu.append(&MenuItem::with_id(app, "none", "Aucun projet", false, None::<&str>)?)?;
    }
    for p in &recent {
        let label = if p.client.is_empty() {
            truncate(&p.name)
        } else {
            format!("{} — {}", truncate(&p.name), truncate(&p.client))
        };
        let active = running.as_ref().is_some_and(|e| e.project_id == p.id);
        let id = format!("project:{}", p.id);
        menu.append(&CheckMenuItem::with_id(app, id, label, true, active, None::<&str>)?)?;
    }

    menu.append(&PredefinedMenuItem::separator(app)?)?;
    menu.append(&MenuItem::with_id(app, "palette", "Sélecteur rapide…", true, None::<&str>)?)?;
    menu.append(&MenuItem::with_id(app, "open", "Ouvrir Trackify", true, None::<&str>)?)?;
    menu.append(&PredefinedMenuItem::separator(app)?)?;
    menu.append(&MenuItem::with_id(app, "quit", "Quitter Trackify", true, None::<&str>)?)?;

    if let Some(tray) = app.tray_by_id(TRAY_ID) {
        tray.set_menu(Some(menu))?;
    }

    let status = app.state::<TrayStatus>();
    let color = current.as_ref().map(|p| p.color.clone());
    {
        let mut shown = status.icon_color.lock().unwrap();
        if *shown != color
            && let Some(tray) = app.tray_by_id(TRAY_ID)
        {
            let (icon, template) = icon_for(color.as_deref())?;
            tray.set_icon(Some(icon))?;
            tray.set_icon_as_template(template)?;
            *shown = color;
        }
    }
    *status.running.lock().unwrap() = match (running, current) {
        (Some(e), Some(p)) => Some((e.started_at, truncate(&p.name))),
        _ => None,
    };
    update_title(app);
    Ok(())
}

fn update_title(app: &AppHandle) {
    let status = app.state::<TrayStatus>();
    let title = match &*status.running.lock().unwrap() {
        Some((started_at, name)) => format!("{name} · {}", format_hms(now_ms() - started_at)),
        None => String::new(),
    };
    let mut last = status.last_title.lock().unwrap();
    if *last == title {
        return;
    }
    if let Some(tray) = app.tray_by_id(TRAY_ID) {
        // Le texte à côté de l'icône n'existe que sur macOS ; ailleurs, l'infobulle suffit.
        #[cfg(target_os = "macos")]
        let _ = tray.set_title(if title.is_empty() { None } else { Some(title.as_str()) });
        let tooltip = if title.is_empty() { "Trackify".to_string() } else { format!("Trackify — {title}") };
        let _ = tray.set_tooltip(Some(tooltip));
    }
    *last = title;
}

fn on_menu_event(app: &AppHandle, event: MenuEvent) {
    match event.id().as_ref() {
        "stop" => {
            let app = app.clone();
            tauri::async_runtime::spawn(async move {
                let db = &app.state::<AppState>().db;
                if let Err(e) = timer::stop(db, now_ms()).await {
                    eprintln!("tray : arrêt impossible : {e}");
                }
                notify::timer_changed(&app);
            });
        }
        "palette" => windows::toggle_palette(app),
        "open" => windows::show_main(app),
        "quit" => app.exit(0),
        id => {
            if let Some(project_id) = id.strip_prefix("project:") {
                let app = app.clone();
                let project_id = project_id.to_string();
                tauri::async_runtime::spawn(async move {
                    let db = &app.state::<AppState>().db;
                    if let Err(e) = timer::start(db, &project_id, now_ms()).await {
                        eprintln!("tray : démarrage impossible : {e}");
                    }
                    // Toujours rafraîchir : la coche du menu a pu changer toute seule.
                    notify::timer_changed(&app);
                });
            }
        }
    }
}

/// Icône du tray et indicateur « template » (macOS la recolore alors selon le thème).
/// - Timer en cours : barres à la couleur du projet, point corail.
/// - Aucun timer : logo monochrome, estompé.
fn icon_for(color: Option<&str>) -> tauri::Result<(Image<'static>, bool)> {
    if let Some(project) = color.and_then(parse_hex) {
        let bars = Image::from_bytes(ICON_BARS)?;
        let dot = Image::from_bytes(ICON_DOT)?;
        let mut rgba = Vec::with_capacity(bars.rgba().len());
        // Les deux masques ne se chevauchent pas : chaque pixel prend la couleur
        // de celui qui le couvre, avec son alpha (anticrénelage conservé).
        for (b, d) in bars.rgba().chunks_exact(4).zip(dot.rgba().chunks_exact(4)) {
            let (rgb, alpha) = if d[3] > b[3] { (DOT_COLOR, d[3]) } else { (project, b[3]) };
            rgba.extend_from_slice(&[rgb[0], rgb[1], rgb[2], alpha]);
        }
        return Ok((Image::new_owned(rgba, bars.width(), bars.height()), false));
    }
    let base = Image::from_bytes(ICON_IDLE)?;
    let mut rgba = base.rgba().to_vec();
    for px in rgba.chunks_exact_mut(4) {
        px[3] = (u16::from(px[3]) * 55 / 100) as u8;
    }
    Ok((Image::new_owned(rgba, base.width(), base.height()), true))
}

fn parse_hex(color: &str) -> Option<[u8; 3]> {
    let hex = color.strip_prefix('#')?;
    if hex.len() != 6 {
        return None;
    }
    let channel = |i: usize| u8::from_str_radix(&hex[i..i + 2], 16).ok();
    Some([channel(0)?, channel(2)?, channel(4)?])
}

fn truncate(s: &str) -> String {
    if s.chars().count() <= NAME_MAX {
        s.to_string()
    } else {
        format!("{}…", s.chars().take(NAME_MAX - 1).collect::<String>())
    }
}

fn local_hm(ts: i64) -> String {
    chrono::DateTime::from_timestamp_millis(ts)
        .map(|d| d.with_timezone(&chrono::Local).format("%H:%M").to_string())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn couleurs_hexadecimales() {
        assert_eq!(parse_hex("#3366ff"), Some([0x33, 0x66, 0xff]));
        assert_eq!(parse_hex("#FFC53D"), Some([0xff, 0xc5, 0x3d]));
        assert_eq!(parse_hex("3366ff"), None);
        assert_eq!(parse_hex("#36f"), None);
        assert_eq!(parse_hex("#zzzzzz"), None);
    }

    #[test]
    fn icones_generees() {
        let (_, template) = icon_for(None).unwrap();
        assert!(template);
        let (img, template) = icon_for(Some("#46a758")).unwrap();
        assert!(!template);
        assert_eq!((img.width(), img.height()), (44, 44));
        let px = |x: u32, y: u32| {
            let i = ((y * img.width() + x) * 4) as usize;
            img.rgba()[i..i + 4].to_vec()
        };
        // Coin : transparent. Milieu de la grande barre : couleur du projet. Centre du point : corail.
        assert_eq!(px(0, 0)[3], 0);
        assert_eq!(px(21, 22), vec![0x46, 0xa7, 0x58, 255]);
        assert_eq!(px(34, 7)[..3], DOT_COLOR);
    }

    #[test]
    fn noms_raccourcis() {
        assert_eq!(truncate("Court"), "Court");
        let long = "Refonte du site institutionnel ACME";
        assert_eq!(truncate(long).chars().count(), NAME_MAX);
        assert!(truncate(long).ends_with('…'));
    }
}
