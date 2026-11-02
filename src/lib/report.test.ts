import { describe, expect, it } from "vitest";
import type { ReportRow } from "../bindings";
import { csvFileName, formatHours, periodRange, pivot, shiftPeriod } from "./report";

const H = 3_600_000;
const row = (date: string, projectId: string, roundedMs: number, notes: string[] = []): ReportRow => ({
  date, projectId, ms: roundedMs, roundedMs, notes,
});

describe("periodRange", () => {
  it("semaine du lundi au dimanche", () => {
    const { from, to, days } = periodRange("week", new Date(2026, 10, 5)); // jeudi 5 novembre
    expect(days).toHaveLength(7);
    expect(new Date(from)).toEqual(new Date(2026, 10, 2));
    expect(new Date(to)).toEqual(new Date(2026, 10, 9));
  });

  it("jour", () => {
    const { from, to, days } = periodRange("day", new Date(2026, 10, 5, 15, 30));
    expect(days).toHaveLength(1);
    expect(new Date(from)).toEqual(new Date(2026, 10, 5));
    expect(new Date(to)).toEqual(new Date(2026, 10, 6));
  });

  it("navigation", () => {
    expect(shiftPeriod("week", new Date(2026, 10, 5), -1)).toEqual(new Date(2026, 9, 29));
    expect(shiftPeriod("day", new Date(2026, 10, 5), 1)).toEqual(new Date(2026, 10, 6));
  });
});

describe("formatHours", () => {
  it("virgule décimale, deux chiffres", () => {
    expect(formatHours(0)).toBe("0,00");
    expect(formatHours(1.5 * H)).toBe("1,50");
    expect(formatHours(H / 3)).toBe("0,33");
  });
});

describe("pivot", () => {
  it("projets × jours, triés par total", () => {
    const { days } = periodRange("week", new Date(2026, 10, 2));
    const result = pivot(
      [
        row("2026-11-02", "a", H, ["Maquette"]),
        row("2026-11-03", "a", 2 * H, ["Maquette", "Revue"]),
        row("2026-11-02", "b", 4 * H),
        row("2026-12-01", "b", H), // hors période
      ],
      days,
    );
    expect(result.rows.map((r) => r.projectId)).toEqual(["b", "a"]);
    expect(result.rows[1].perDay.slice(0, 2)).toEqual([H, 2 * H]);
    expect(result.rows[1].notes).toEqual(["Maquette", "Revue"]);
    expect(result.dayTotals[0]).toBe(5 * H);
    expect(result.total).toBe(7 * H);
  });
});

describe("csvFileName", () => {
  it("jour et semaine ISO", () => {
    expect(csvFileName("day", new Date(2026, 10, 2))).toBe("trackify-2026-11-02.csv");
    expect(csvFileName("week", new Date(2026, 10, 2))).toBe("trackify-2026-S45.csv");
  });
});
