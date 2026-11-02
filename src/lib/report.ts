import { addDays, addWeeks, format, getISOWeek, getISOWeekYear, startOfDay, startOfWeek } from "date-fns";
import type { ReportRow } from "../bindings";

export type Period = "day" | "week";

export const ROUNDING_OPTIONS = [
  { value: 0, label: "Aucun" },
  { value: 5, label: "5 min" },
  { value: 15, label: "15 min" },
  { value: 30, label: "30 min" },
] as const;

/** Bornes `[from, to)` et jours de la période contenant `anchor` (semaine du lundi au dimanche). */
export function periodRange(period: Period, anchor: Date): { from: number; to: number; days: Date[] } {
  const first = period === "day" ? startOfDay(anchor) : startOfWeek(anchor, { weekStartsOn: 1 });
  const days = Array.from({ length: period === "day" ? 1 : 7 }, (_, i) => addDays(first, i));
  return { from: first.getTime(), to: addDays(first, days.length).getTime(), days };
}

export function shiftPeriod(period: Period, anchor: Date, delta: number): Date {
  return period === "day" ? addDays(anchor, delta) : addWeeks(anchor, delta);
}

/** Heures décimales à la française, comme dans le CSV : 5 400 000 ms → « 1,50 ». */
export function formatHours(ms: number): string {
  return (Math.round(ms / 36_000) / 100).toFixed(2).replace(".", ",");
}

export type PivotRow = { projectId: string; perDay: number[]; total: number; notes: string[] };

/** Tableau projets × jours, à partir des totaux arrondis. */
export function pivot(rows: ReportRow[], days: Date[]): { rows: PivotRow[]; dayTotals: number[]; total: number } {
  const index = new Map(days.map((d, i) => [format(d, "yyyy-MM-dd"), i]));
  const byProject = new Map<string, PivotRow>();
  const dayTotals = days.map(() => 0);
  for (const r of rows) {
    const i = index.get(r.date);
    if (i === undefined) continue;
    const row = byProject.get(r.projectId) ?? { projectId: r.projectId, perDay: days.map(() => 0), total: 0, notes: [] };
    row.perDay[i] += r.roundedMs;
    row.total += r.roundedMs;
    for (const n of r.notes) if (!row.notes.includes(n)) row.notes.push(n);
    dayTotals[i] += r.roundedMs;
    byProject.set(r.projectId, row);
  }
  const list = [...byProject.values()].sort((a, b) => b.total - a.total);
  return { rows: list, dayTotals, total: dayTotals.reduce((a, b) => a + b, 0) };
}

/** Nom proposé pour l'export : « trackify-2026-11-02.csv » ou « trackify-2026-S45.csv ». */
export function csvFileName(period: Period, anchor: Date): string {
  if (period === "day") return `trackify-${format(anchor, "yyyy-MM-dd")}.csv`;
  return `trackify-${getISOWeekYear(anchor)}-S${String(getISOWeek(anchor)).padStart(2, "0")}.csv`;
}
