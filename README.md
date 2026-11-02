# Trackify

Suivi du temps multi-projets pour le bureau (macOS en priorité, Windows et Linux ensuite).
Tout reste sur l'ordinateur : une base SQLite locale, aucun compte, aucun envoi sur Internet.

## Fonctions

- **Projets** : nom, client, couleur, archivage. Les noms doivent être identiques à ceux du timesheet officiel,
  car l'export les reprend tels quels. Un client peut être renommé sur tous ses projets d'un coup.
- **Timer** : un seul à la fois. Il survit à un redémarrage, car le temps est recalculé depuis l'heure de début enregistrée.
- **Barre des menus** : le projet en cours et le temps écoulé, les projets récents, l'arrêt du timer et la pause du suivi d'activité.
- **Sélecteur rapide** (⌘⇧Espace par défaut, modifiable) :
  - basculer de projet ;
  - créer un projet et le démarrer ;
  - annoter l'entrée en cours ;
  - arrêter le timer.
- **Absences** : sans clavier ni souris pendant 5 min (réglable), Trackify demande quoi faire de ce temps : le garder, le retirer ou l'attribuer à un autre projet.
  Il le demande aussi si l'app est restée fermée pendant qu'un timer tournait.
- **Rappels anti-oubli** :
  - « aucun timer » quand tu travailles sans rien compter ;
  - un point régulier pendant les longs timers ;
  - une revue de fin de journée (18 h par défaut).
- **Suivi d'activité** : toutes les 10 s, Trackify note l'app et la fenêtre au premier plan.
  - Les échantillons identiques sont fusionnés en segments.
  - Il fait une pause pendant les absences et peut être coupé à tout moment.
  - L'historique est effacé après 30 jours.
- **Revue de journée** :
  - frise de la journée et navigation jour par jour ;
  - ajout, modification, scission, fusion et suppression d'entrées ;
  - attribution du temps d'activité sans projet en un clic.
  - Une entrée saisie à la main l'emporte sur celles qu'elle recouvre, qui sont raccourcies ou coupées.
- **Rapports** : par jour et par semaine, arrondi aucun / 5 / 15 / 30 min sur le total de chaque jour et de chaque projet.
- **Export CSV**, pour Excel ou Numbers en français :
  - UTF-8 avec BOM, séparateur `;`, durée en heures décimales avec une virgule ;
  - une ligne par jour et par projet : `date;projet;client;durée (h);note`.
- **Sauvegardes** : une copie de la base chaque jour, les 7 dernières gardées. Le dossier est au choix, par exemple sur iCloud Drive.

## Installation (utilisateur)

1. Construis l'app (voir plus bas), ou récupère le `.dmg` produit dans `target/release/bundle/dmg/`.
2. Ouvre le `.dmg` et glisse **Trackify** dans **Applications**.
3. **Premier lancement :** l'app n'est pas signée par un développeur Apple identifié.
   - Ouvre-la une fois par clic droit › **Ouvrir**, puis confirme.
   - Ou, dans un terminal : `xattr -dr com.apple.quarantine /Applications/Trackify.app`.
4. Trackify se lance ensuite à chaque ouverture de session, en arrière-plan. Ce lancement automatique se désactive dans Réglages.

Fermer la fenêtre ne quitte pas l'app : elle reste dans la barre des menus. Pour quitter, passe par le menu de l'icône, puis **Quitter Trackify**.

## Permissions par système

| Système | Ce qui est demandé | Pourquoi |
|---|---|---|
| **macOS** | **Enregistrement de l'écran** (Réglages Système › Confidentialité et sécurité). Facultatif. | Lire le **titre** des fenêtres pour le suivi d'activité. Trackify ne capture jamais l'écran. Sans cette permission, seul le nom de l'app est noté. Le bouton **Autoriser…** dans Réglages ouvre le bon panneau. Après l'avoir accordée, macOS peut demander de relancer Trackify. |
| macOS | Rien d'autre | La détection d'absence et le raccourci global ne demandent aucune permission. L'Accessibilité n'est pas nécessaire. |
| **Windows** | Aucune | Fenêtre active et inactivité via les API Win32 standard. |
| **Linux** | Aucune, sous X11 | Inactivité et fenêtre active via X11. Sous **Wayland**, ces informations ne sont pas exposées de façon standard : la détection d'absence et le suivi d'activité peuvent ne rien remonter. |

Les données sont dans le dossier de l'app :
- **macOS :** `~/Library/Application Support/com.trackify.app/`
- **Windows :** `%APPDATA%\com.trackify.app\`
- **Linux :** `~/.local/share/com.trackify.app/`

Ce dossier contient `trackify.db` et le sous-dossier `backups/`.

**Restaurer une sauvegarde :**
1. Quitte Trackify.
2. Remplace `trackify.db` par une copie de `backups/trackify-AAAA-MM-JJ.db` renommée en `trackify.db`.
3. Supprime les fichiers `trackify.db-wal` et `trackify.db-shm` s'ils existent.
4. Relance Trackify.

## Développement

Prérequis : [Rust](https://rustup.rs) stable (édition 2024), Node.js 20+ et les
[dépendances système de Tauri v2](https://v2.tauri.app/start/prerequisites/). Sur macOS, ce sont les outils en ligne de commande Xcode.

```bash
npm install
npm run tauri dev      # app complète, avec rechargement à chaud
npm run dev            # interface seule dans le navigateur (http://localhost:1420), avec un faux backend en mémoire
```

Tests :

```bash
cargo test --workspace # logique métier (crates/core) et services avec SQLite en mémoire ; régénère aussi src/bindings.ts
npm test               # fonctions du frontend (vitest)
npx tsc --noEmit       # types
```

### Construire l'app

```bash
npm run tauri build
```

Le résultat se trouve dans `target/release/bundle/` :
- **macOS :** `macos/Trackify.app` et `dmg/Trackify_0.1.0_aarch64.dmg` ;
- **Windows :** `.msi` et installeur NSIS ;
- **Linux :** `.deb`, `.rpm` et `.AppImage`.

Chaque paquet se construit sur son propre système.

## Architecture

```
crates/core/        Logique pure, sans Tauri ni base, testée unitairement :
                    durées et découpage par jour (heure d'été), retrait de plages,
                    détection d'absence, rappels, fusion d'activité, rapports, arrondi, CSV.
src-tauri/src/
  db/               Tout le SQL (sqlx). Migrations dans src-tauri/migrations/.
  services/         Timer, édition des entrées, absences, rappels, activité, rapports, sauvegardes.
  platform/         Code par OS : inactivité, fenêtre active, permissions.
  commands.rs       Commandes exposées au frontend (types générés dans src/bindings.ts par tauri-specta).
  tray.rs, shortcut.rs, windows.rs
src/                React + TypeScript : pages (Aujourd'hui, Projets, Rapports, Réglages),
                    sélecteur rapide (#palette), fenêtre de rappel (#reminder).
```

Chaque modification côté Rust émet un événement : `timer-changed`, `entries-changed`, etc.
Toutes les fenêtres rafraîchissent alors leurs données (TanStack Query).

**Préparé pour une future synchronisation :**
- identifiants UUID v7 ;
- horodatages en millisecondes UTC ;
- `created_at` et `updated_at` sur chaque ligne ;
- suppression logique (`deleted_at`).
