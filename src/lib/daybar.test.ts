import { describe, expect, it } from "vitest";
import type { TimeEntry } from "../bindings";
import { dayWindow, segments } from "./daybar";
import { HOUR } from "./time";

const DAY = 1_000 * 24 * HOUR;
const e = (id: string, start: number, end: number | null): TimeEntry => ({
  id, projectId: "p", startedAt: DAY + start * HOUR, endedAt: end === null ? null : DAY + end * HOUR,
  note: "", source: "timer", createdAt: 0, updatedAt: 0,
});

describe("dayWindow", () => {
  it("8h–19h par défaut", () => {
    expect(dayWindow([], DAY, DAY + 10 * HOUR)).toEqual([8, 19]);
  });
  it("s'élargit pour couvrir les entrées et l'heure courante", () => {
    expect(dayWindow([e("a", 6.5, 7)], DAY, DAY + 10 * HOUR)).toEqual([6, 19]);
    expect(dayWindow([], DAY, DAY + 21.2 * HOUR)).toEqual([8, 22]);
    expect(dayWindow([e("a", 20, null)], DAY, DAY + 23.5 * HOUR)).toEqual([8, 24]);
  });
});

describe("segments", () => {
  it("positions en pourcentage", () => {
    const [seg] = segments([e("a", 9, 10)], DAY, [8, 18], DAY + 12 * HOUR);
    expect(seg.left).toBeCloseTo(10);
    expect(seg.width).toBeCloseTo(10);
    expect(seg.live).toBe(false);
  });
  it("entrée en cours jusqu'à maintenant", () => {
    const [seg] = segments([e("a", 16, null)], DAY, [8, 18], DAY + 17 * HOUR);
    expect(seg.width).toBeCloseTo(10);
    expect(seg.live).toBe(true);
  });
  it("bornée à la fenêtre (entrée commencée la veille)", () => {
    const [seg] = segments([e("a", -2, 9)], DAY, [8, 18], DAY + 12 * HOUR);
    expect(seg.left).toBe(0);
    expect(seg.width).toBeCloseTo(10);
  });
});
