# Trackify — The Time Marker

Pack SVG source initial, conçu autour de deux barres verticales et d'un point représentant le moment présent.

## Fichiers
- `trackify-symbol-color.svg` : symbole couleur, viewBox 256 × 256.
- `trackify-symbol-black.svg` / `trackify-symbol-white.svg` : versions monochromes.
- `trackify-app-icon.svg` : icône carrée 1024 × 1024 pour `npm run tauri icon`.
- `trackify-app-icon-monochrome-*.svg` : variantes monochromes de l'icône.
- `trackify-menubar-idle.svg` : icône template monochrome, 44 × 44 (@2x pour 22 pt).
- `trackify-menubar-active.svg` : état actif. Recolorer le chemin `#project-color` avec la couleur du projet; le point `#time-marker` reste un accent.
- `trackify-sidebar-mark.svg` : marque pour la barre latérale.
- `trackify-wordmark.svg` : logotype horizontal.

## Palette
- Indigo : `#4F5BD5`
- Lavande : `#9B91FF`
- Corail : `#FF4D61`
- Texte : `#171923`

## Notes d'intégration
- Pour macOS, utiliser `trackify-menubar-idle.svg` comme image template (rendu monochrome par le système).
- Pour l'état actif, appliquer la couleur du projet au chemin `project-color` ou produire une image rasterisée à la volée.
- Le logotype utilise une pile de polices `Inter`, `SF Pro Display`, `Helvetica Neue`, `Arial`. Le texte reste du texte SVG éditable et n'est pas vectorisé en tracés.
- Les SVG sont des sources vectorielles; vérifier le rendu dans le pipeline Tauri et ajuster les marges selon les plateformes.
- Les aperçus à 16 px et 22 px sont indicatifs : valider dans l'interface réelle, notamment les règles de template images de macOS.

## Ajouts lors de l'intégration dans l'app
- `trackify-app-icon-macos.svg` : `trackify-app-icon.svg` ramené dans la grille macOS (carré de 824 px centré, marge de 100 px) pour ne pas paraître plus gros que les autres icônes du Dock. C'est la source de `npx tauri icon` (icônes dans `src-tauri/icons/`).
- `tray/` : rendus PNG 44 × 44 utilisés par la barre des menus (`src-tauri/icons/tray-*.png`) :
  - `trackify-menubar-idle.png` → `tray-idle.png` (image template, affichée estompée sans timer) ;
  - `bars.svg` / `bars.png` → `tray-bars.png` (masque des barres, recoloré avec la couleur du projet actif) ;
  - `dot.svg` / `dot.png` → `tray-dot.png` (masque du point, toujours corail `#FF4D61`).
- Barre latérale : le logo est dessiné en ligne dans `src/components/BrandMark.tsx`.
