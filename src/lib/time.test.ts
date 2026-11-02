import { describe, expect, it } from "vitest";
import { HOUR, MINUTE, dayRange, durationWithin, entryDuration, formatClock, formatDuration } from "./time";

describe("formatClock", () => {
  it("formate heures, minutes, secondes", () => {
    expect(formatClock(0)).toBe("0:00:00");
    expect(formatClock(5 * MINUTE + 9_000)).toBe("0:05:09");
    expect(formatClock(12 * HOUR)).toBe("12:00:00");
  });
  it("ne descend jamais sous zéro", () => {
    expect(formatClock(-5000)).toBe("0:00:00");
  });
});

describe("formatDuration", () => {
  it("affiche les minutes seules sous une heure", () => {
    expect(formatDuration(45 * MINUTE + 59_000)).toBe("45 min");
  });
  it("affiche h et minutes au-delà", () => {
    expect(formatDuration(HOUR + 5 * MINUTE)).toBe("1 h 05");
  });
});

describe("durées d'entrée", () => {
  it("une entrée en cours court jusqu'à maintenant", () => {
    expect(entryDuration({ startedAt: 0, endedAt: null }, 30 * MINUTE)).toBe(30 * MINUTE);
  });
  it("ne compte que la partie dans la journée", () => {
    const [from, to] = dayRange(new Date(2026, 6, 2, 12));
    const entry = { startedAt: from - HOUR, endedAt: from + 2 * HOUR };
    expect(durationWithin(entry, from, to, to)).toBe(2 * HOUR);
  });
  it("une journée locale dure 24 h hors changement d'heure", () => {
    const [from, to] = dayRange(new Date(2026, 6, 2, 12));
    expect(to - from).toBe(24 * HOUR);
  });
});

describe("conversion des horaires", () => {
  it("minutes vers HH:MM et retour", async () => {
    const { minutesToTime, timeToMinutes } = await import("./time");
    expect(minutesToTime(480)).toBe("08:00");
    expect(minutesToTime(1140)).toBe("19:00");
    expect(timeToMinutes("08:30")).toBe(510);
    expect(timeToMinutes("24:00")).toBeNull();
    expect(timeToMinutes("")).toBeNull();
  });
});
