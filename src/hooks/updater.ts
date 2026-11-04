import { useMutation, useQuery } from "@tanstack/react-query";
import { getVersion } from "@tauri-apps/api/app";
import { relaunch } from "@tauri-apps/plugin-process";
import { check, type Update } from "@tauri-apps/plugin-updater";
import { useState } from "react";

/** Interface seule dans le navigateur (faux backend) : rien à mettre à jour. */
export const inTauri = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

const CHECK_EVERY_MS = 6 * 60 * 60 * 1000;

export function useAppVersion() {
  return useQuery({ queryKey: ["version"], queryFn: getVersion, enabled: inTauri });
}

/**
 * Version plus récente publiée sur GitHub (`latest.json` de la dernière release), sinon `null`.
 * Vérifiée au lancement puis toutes les 6 h, y compris fenêtre masquée.
 */
export function useAvailableUpdate() {
  return useQuery({
    queryKey: ["update"],
    queryFn: () => check(),
    enabled: inTauri,
    staleTime: CHECK_EVERY_MS,
    refetchInterval: CHECK_EVERY_MS,
    refetchIntervalInBackground: true,
  });
}

/** Télécharge et installe la mise à jour (signature vérifiée par le plugin), puis redémarre. */
export function useInstallUpdate() {
  const [progress, setProgress] = useState<number | null>(null);
  const mutation = useMutation({
    mutationFn: async (update: Update) => {
      let total = 0;
      let received = 0;
      await update.downloadAndInstall((event) => {
        if (event.event === "Started") {
          total = event.data.contentLength ?? 0;
        } else if (event.event === "Progress") {
          received += event.data.chunkLength;
          if (total > 0) setProgress(Math.min(100, Math.round((received * 100) / total)));
        }
      });
      // Sous Windows, l'installeur ferme et relance l'app lui-même : on n'arrive ici que sur macOS.
      await relaunch();
    },
  });
  return { ...mutation, progress };
}

export function installLabel(pending: boolean, progress: number | null): string {
  if (!pending) return "Mettre à jour";
  return progress === null || progress >= 100 ? "Installation…" : `Téléchargement… ${progress} %`;
}
