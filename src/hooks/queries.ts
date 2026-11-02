import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useEffect } from "react";
import { commands, events, type IdleResolution, type ProjectInput } from "../bindings";

export const keys = {
  projects: (includeArchived: boolean) => ["projects", includeArchived] as const,
  running: ["running"] as const,
  entries: (from: number, to: number) => ["entries", from, to] as const,
  settings: ["settings"] as const,
  autostart: ["autostart"] as const,
  pendingIdle: ["pendingIdle"] as const,
};

export function useProjects(includeArchived = false) {
  return useQuery({
    queryKey: keys.projects(includeArchived),
    queryFn: () => commands.listProjects(includeArchived),
  });
}

export function useRunning() {
  return useQuery({ queryKey: keys.running, queryFn: commands.getRunning });
}

export function useEntries(from: number, to: number) {
  return useQuery({ queryKey: keys.entries(from, to), queryFn: () => commands.listEntries(from, to) });
}

export function useSettings() {
  return useQuery({ queryKey: keys.settings, queryFn: commands.getSettings });
}

export function useAutostart() {
  return useQuery({ queryKey: keys.autostart, queryFn: commands.getAutostart });
}

// Réglages : propres à cette fenêtre, on met le cache à jour directement.

export function useSetShortcut() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: commands.setShortcut,
    onSuccess: (settings) => qc.setQueryData(keys.settings, settings),
  });
}

export function useSetReminderSettings() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: commands.setReminderSettings,
    onSuccess: (settings) => qc.setQueryData(keys.settings, settings),
  });
}

export function usePendingIdle() {
  return useQuery({ queryKey: keys.pendingIdle, queryFn: commands.listPendingIdle });
}

export const useResolveIdle = () =>
  useMutation({
    mutationFn: ({ id, resolution }: { id: string; resolution: IdleResolution }) =>
      commands.resolveIdle(id, resolution),
  });

export function useSetAutostart() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: commands.setAutostart,
    onSuccess: (enabled) => qc.setQueryData(keys.autostart, enabled),
  });
}

// Les mutations n'invalident rien elles-mêmes : Rust émet un événement après chaque
// changement, ce qui rafraîchit aussi les autres fenêtres (tray, palette).

export const useStartTimer = () => useMutation({ mutationFn: commands.startTimer });
export const useStopTimer = () => useMutation({ mutationFn: commands.stopTimer });

export const useSetEntryNote = () =>
  useMutation({ mutationFn: ({ id, note }: { id: string; note: string }) => commands.setEntryNote(id, note) });

export const useDeleteEntry = () => useMutation({ mutationFn: commands.deleteEntry });

export const useSaveProject = () =>
  useMutation({
    mutationFn: ({ id, input }: { id?: string; input: ProjectInput }) =>
      id ? commands.updateProject(id, input) : commands.createProject(input),
  });

export const useArchiveProject = () =>
  useMutation({
    mutationFn: ({ id, archived }: { id: string; archived: boolean }) => commands.setProjectArchived(id, archived),
  });

export const useDeleteProject = () => useMutation({ mutationFn: commands.deleteProject });

/** Branche les événements Rust sur le cache de requêtes. À monter une fois par fenêtre. */
export function useDataEvents() {
  const qc = useQueryClient();
  useEffect(() => {
    const subscriptions = [
      events.timerChanged.listen(() => qc.invalidateQueries({ queryKey: keys.running })),
      events.entriesChanged.listen(() => qc.invalidateQueries({ queryKey: ["entries"] })),
      events.projectsChanged.listen(() => qc.invalidateQueries({ queryKey: ["projects"] })),
      events.idleChanged.listen(() => qc.invalidateQueries({ queryKey: keys.pendingIdle })),
    ];
    return () => {
      subscriptions.forEach((p) => p.then((unlisten) => unlisten()));
    };
  }, [qc]);
}
