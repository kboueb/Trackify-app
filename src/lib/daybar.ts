import type { TimeEntry } from "../bindings";
import { HOUR } from "./time";

export type Segment = {
  id: string;
  projectId: string;
  /** Position et largeur en % de la frise. */
  left: number;
  width: number;
  live: boolean;
};

/**
 * Heures affichées par la frise : au moins 8h–19h, élargies pour couvrir
 * les entrées et l'heure courante. `dayStart` = minuit local du jour affiché.
 */
export function dayWindow(
  entries: Pick<TimeEntry, "startedAt" | "endedAt">[],
  dayStart: number,
  now: number,
): [number, number] {
  let first = 8;
  let last = 19;
  for (const e of entries) {
    first = Math.min(first, Math.floor((e.startedAt - dayStart) / HOUR));
    last = Math.max(last, Math.ceil(((e.endedAt ?? now) - dayStart) / HOUR));
  }
  if (now >= dayStart && now < dayStart + 24 * HOUR) {
    last = Math.max(last, Math.ceil((now - dayStart) / HOUR));
  }
  return [Math.max(0, first), Math.min(24, last)];
}

/** Segments colorés de la frise, bornés à la fenêtre `[fromHour, toHour]`. */
export function segments(
  entries: TimeEntry[],
  dayStart: number,
  [fromHour, toHour]: [number, number],
  now: number,
): Segment[] {
  const start = dayStart + fromHour * HOUR;
  const span = (toHour - fromHour) * HOUR;
  return entries
    .map((e) => {
      const s = Math.max(e.startedAt, start);
      const end = Math.min(e.endedAt ?? now, start + span);
      return {
        id: e.id,
        projectId: e.projectId,
        left: ((s - start) / span) * 100,
        width: (Math.max(0, end - s) / span) * 100,
        live: e.endedAt === null,
      };
    })
    .filter((seg) => seg.width > 0);
}
