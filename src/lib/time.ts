import { addDays, format, startOfDay } from "date-fns";
import type { TimeEntry } from "../bindings";

export const MINUTE = 60_000;
export const HOUR = 60 * MINUTE;
export const DAY = 24 * HOUR;

const pad = (n: number) => String(n).padStart(2, "0");

/** Chronomètre : « 0:05:09 », « 12:00:00 ». */
export function formatClock(ms: number): string {
  const total = Math.max(0, Math.floor(ms / 1000));
  const h = Math.floor(total / 3600);
  const m = Math.floor((total % 3600) / 60);
  const s = total % 60;
  return `${h}:${pad(m)}:${pad(s)}`;
}

/** Durée lisible : « 45 min », « 1 h 05 », « 0 min ». */
export function formatDuration(ms: number): string {
  const minutes = Math.max(0, Math.floor(ms / MINUTE));
  const h = Math.floor(minutes / 60);
  const m = minutes % 60;
  return h === 0 ? `${m} min` : `${h} h ${pad(m)}`;
}

/** Heure locale « 14:05 ». */
export function formatTime(ts: number): string {
  return format(ts, "HH:mm");
}

/** Bornes `[début, fin)` de la journée locale contenant `date`, en ms UTC. */
export function dayRange(date: Date): [number, number] {
  const start = startOfDay(date);
  return [start.getTime(), addDays(start, 1).getTime()];
}

/** Durée d'une entrée ; une entrée en cours court jusqu'à `now`. */
export function entryDuration(entry: Pick<TimeEntry, "startedAt" | "endedAt">, now: number): number {
  return Math.max(0, (entry.endedAt ?? now) - entry.startedAt);
}

/** Part d'une entrée comprise dans `[from, to)` (pour les entrées qui passent minuit). */
export function durationWithin(
  entry: Pick<TimeEntry, "startedAt" | "endedAt">,
  from: number,
  to: number,
  now: number,
): number {
  const end = Math.min(entry.endedAt ?? now, to);
  return Math.max(0, end - Math.max(entry.startedAt, from));
}

/** 480 → « 08:00 » (format des champs `<input type="time">`). */
export function minutesToTime(minutes: number): string {
  return `${pad(Math.floor(minutes / 60))}:${pad(minutes % 60)}`;
}

/** « 08:30 » → 510 ; `null` si la saisie est incomplète. */
export function timeToMinutes(value: string): number | null {
  const match = /^(\d{1,2}):(\d{2})$/.exec(value);
  if (!match) return null;
  const [h, m] = [Number(match[1]), Number(match[2])];
  return h < 24 && m < 60 ? h * 60 + m : null;
}

/** « 14:05 » le jour contenant `day` → horodatage local ; `null` si la saisie est invalide. */
export function atTime(day: number, value: string): number | null {
  const minutes = timeToMinutes(value);
  if (minutes === null) return null;
  const d = new Date(day);
  d.setHours(Math.floor(minutes / 60), minutes % 60, 0, 0);
  return d.getTime();
}
