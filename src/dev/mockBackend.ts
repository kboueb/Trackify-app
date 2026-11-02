/**
 * Faux backend en mémoire pour ouvrir l'interface dans un navigateur (`npm run dev`,
 * puis http://localhost:1420). Chargé uniquement en développement, hors de Tauri :
 * utile pour travailler l'UI sans recompiler le Rust.
 */
import { mockIPC } from "@tauri-apps/api/mocks";
import { emit } from "@tauri-apps/api/event";
import type { IdlePeriod, Project, ReminderView, Settings, TimeEntry } from "../bindings";

const MIN = 60_000;
const now = Date.now();
const today = new Date();
today.setHours(0, 0, 0, 0);
const at = (h: number, m = 0) => today.getTime() + (h * 60 + m) * MIN;

let id = 0;
const nextId = () => `mock-${++id}`;

const project = (name: string, client: string, color: string, lastUsedAt: number | null = null): Project => ({
  id: nextId(), name, client, color, archived: false, lastUsedAt, createdAt: now, updatedAt: now,
});

const projects: Project[] = [
  project("Multisite", "ACME", "#3e63dd", now),
  project("Refonte charte", "Globex", "#e5484d", now - 60 * MIN),
  project("Campagne printemps", "Initech", "#46a758", now - 120 * MIN),
  project("Support", "ACME", "#f76b15", now - 240 * MIN),
  project("Réunions internes", "", "#8e4ec6"),
  project("Veille", "", "#12a594"),
];

const entry = (p: Project, start: number, end: number | null, note = ""): TimeEntry => ({
  id: nextId(), projectId: p.id, startedAt: start, endedAt: end, note, source: "timer", createdAt: start, updatedAt: start,
});

const firstEnd = Math.min(at(10, 30), now - 3 * 60 * MIN);
const entries: TimeEntry[] = [
  entry(projects[1], Math.min(at(9), firstEnd - 90 * MIN), firstEnd, "Moodboard"),
  entry(projects[4], firstEnd, firstEnd + 30 * MIN, "Point hebdo"),
  entry(projects[3], firstEnd + 30 * MIN, now - 75 * MIN, "Tickets clients"),
  entry(projects[0], now - 75 * MIN, null, "Intégration page d'accueil"),
];

let settings: Settings = {
  shortcut: "CommandOrControl+Shift+Space",
  autostartInitialized: true,
  reminders: {
    idleThresholdMin: 5, noTimerEnabled: true, noTimerAfterMin: 5, checkinEnabled: true,
    checkinEveryMin: 60, activeFromMin: 480, activeUntilMin: 1140, weekdaysOnly: true,
  },
};
let autostart = true;
const idle: IdlePeriod[] = [];
const reminder: ReminderView = {
  kind: "checkIn", projectName: "Multisite", projectColor: "#3e63dd", startedAt: now - 75 * MIN,
};

const running = () => entries.find((e) => e.endedAt === null) ?? null;
const changed = (...names: string[]) => setTimeout(() => names.forEach((n) => void emit(n, null)), 0);
const byId = <T extends { id: string }>(list: T[], key: string) => list.find((x) => x.id === key)!;

function stop(t: number) {
  const r = running();
  if (r) r.endedAt = t;
  return r;
}

type Args = Record<string, any>; // eslint-disable-line @typescript-eslint/no-explicit-any

const handlers: Record<string, (a: Args) => unknown> = {
  list_projects: ({ includeArchived }) => projects.filter((p) => includeArchived || !p.archived),
  recent_projects: ({ limit }) =>
    [...projects].sort((a, b) => (b.lastUsedAt ?? 0) - (a.lastUsedAt ?? 0)).slice(0, limit),
  create_project: ({ input }) => {
    const p = project(input.name, input.client, input.color);
    projects.push(p);
    changed("projects-changed");
    return p;
  },
  update_project: ({ id, input }) => {
    const p = Object.assign(byId(projects, id), input);
    changed("projects-changed");
    return p;
  },
  set_project_archived: ({ id, archived }) => {
    const p = Object.assign(byId(projects, id), { archived });
    changed("projects-changed");
    return p;
  },
  delete_project: ({ id }) => {
    projects.splice(projects.indexOf(byId(projects, id)), 1);
    changed("projects-changed");
    return null;
  },
  get_running: () => running(),
  start_timer: ({ projectId }) => {
    const t = Date.now();
    const r = running();
    if (r?.projectId === projectId) return r;
    stop(t);
    const p = byId(projects, projectId);
    const e = entry(p, t, null);
    entries.push(e);
    p.lastUsedAt = t;
    changed("timer-changed", "entries-changed", "projects-changed");
    return e;
  },
  stop_timer: () => {
    const r = stop(Date.now());
    changed("timer-changed", "entries-changed");
    return r;
  },
  list_entries: ({ from, to }) => entries.filter((e) => e.startedAt < to && (e.endedAt ?? Infinity) > from),
  set_entry_note: ({ id, note }) => {
    const e = Object.assign(byId(entries, id), { note });
    changed("entries-changed");
    return e;
  },
  delete_entry: ({ id }) => {
    entries.splice(entries.indexOf(byId(entries, id)), 1);
    changed("timer-changed", "entries-changed");
    return null;
  },
  get_settings: () => settings,
  set_shortcut: ({ accelerator }) => (settings = { ...settings, shortcut: accelerator }),
  set_reminder_settings: ({ reminders }) => (settings = { ...settings, reminders }),
  get_autostart: () => autostart,
  set_autostart: ({ enabled }) => (autostart = enabled),
  list_pending_idle: () => idle,
  resolve_idle: ({ id }) => {
    idle.splice(idle.indexOf(byId(idle, id)), 1);
    changed("idle-changed");
    return null;
  },
  get_reminder: () => reminder,
  reminder_action: () => null,
  hide_palette: () => null,
  open_main_window: () => null,
};

/** Simule une absence en attente (console du navigateur : `__mockIdle()`). */
(window as unknown as { __mockIdle: () => void }).__mockIdle = () => {
  const r = running();
  if (!r) return;
  idle.push({
    id: nextId(), entryId: r.id, projectId: r.projectId, kind: "idle",
    startedAt: Date.now() - 50 * MIN, endedAt: Date.now() - 15 * MIN,
  });
  changed("idle-changed");
};

export function installMockBackend() {
  mockIPC(
    (cmd, payload) => {
      const handler = handlers[cmd];
      if (!handler) throw new Error(`Commande non simulée : ${cmd}`);
      return structuredClone(handler((payload ?? {}) as Args));
    },
    { shouldMockEvents: true },
  );
}
