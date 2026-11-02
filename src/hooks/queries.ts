import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useEffect } from "react";
import { commands, events, type EntryInput, type IdleResolution, type ProjectInput } from "../bindings";

export const keys = {
  projects: (includeArchived: boolean) => ["projects", includeArchived] as const,
  running: ["running"] as const,
  entries: (from: number, to: number) => ["entries", from, to] as const,
  settings: ["settings"] as const,
  autostart: ["autostart"] as const,
  pendingIdle: ["pendingIdle"] as const,
  unassigned: (from: number, to: number) => ["unassigned", from, to] as const,
  report: (from: number, to: number, rounding: number) => ["report", from, to, rounding] as const,
  activity: ["activity"] as const,
  backup: ["backup"] as const,
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

export const useSaveEntry = () =>
  useMutation({ mutationFn: ({ id, input }: { id?: string; input: EntryInput }) => commands.saveEntry(id ?? null, input) });

export const useSplitEntry = () =>
  useMutation({ mutationFn: ({ id, at }: { id: string; at: number }) => commands.splitEntry(id, at) });

export const useMergeEntries = () => useMutation({ mutationFn: commands.mergeEntries });

/** Temps actif sans projet (suivi d'activité). Rafraîchi chaque minute : l'activité n'émet pas d'événement. */
export function useUnassigned(from: number, to: number) {
  return useQuery({
    queryKey: keys.unassigned(from, to),
    queryFn: () => commands.listUnassigned(from, to),
    refetchInterval: 60_000,
  });
}

export const useRenameClient = () =>
  useMutation({ mutationFn: ({ from, to }: { from: string; to: string }) => commands.renameClient(from, to) });

export function useReport(from: number, to: number, rounding: number) {
  return useQuery({ queryKey: keys.report(from, to, rounding), queryFn: () => commands.getReport(from, to, rounding) });
}

export function useSetRounding() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: commands.setRounding,
    onSuccess: (settings) => qc.setQueryData(keys.settings, settings),
  });
}

/** Statut du suivi ; relu au retour sur la fenêtre (la permission se donne dans les Réglages Système). */
export function useActivityStatus() {
  return useQuery({ queryKey: keys.activity, queryFn: commands.activityStatus, refetchOnWindowFocus: true });
}

export const useSetActivityEnabled = () => useMutation({ mutationFn: commands.setActivityEnabled });

export function useBackupInfo() {
  return useQuery({ queryKey: keys.backup, queryFn: commands.backupInfo });
}

export function useBackupNow() {
  const qc = useQueryClient();
  return useMutation({ mutationFn: commands.backupNow, onSuccess: () => qc.invalidateQueries({ queryKey: keys.backup }) });
}

export function useSetBackupDir() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: commands.setBackupDir,
    onSuccess: (settings) => {
      qc.setQueryData(keys.settings, settings);
      void qc.invalidateQueries({ queryKey: keys.backup });
    },
  });
}

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
      events.entriesChanged.listen(() => {
        void qc.invalidateQueries({ queryKey: ["entries"] });
        void qc.invalidateQueries({ queryKey: ["unassigned"] });
        void qc.invalidateQueries({ queryKey: ["report"] });
      }),
      events.projectsChanged.listen(() => {
        void qc.invalidateQueries({ queryKey: ["projects"] });
        void qc.invalidateQueries({ queryKey: ["report"] });
      }),
      events.settingsChanged.listen(() => {
        void qc.invalidateQueries({ queryKey: keys.settings });
        void qc.invalidateQueries({ queryKey: keys.activity });
      }),
      events.idleChanged.listen(() => qc.invalidateQueries({ queryKey: keys.pendingIdle })),
    ];
    return () => {
      subscriptions.forEach((p) => p.then((unlisten) => unlisten()));
    };
  }, [qc]);
}
