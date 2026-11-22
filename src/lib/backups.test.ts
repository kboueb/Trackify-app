import { describe, expect, it } from "vitest";
import { backupLabel, formatSize } from "./backups";

describe("backupLabel", () => {
  it("affiche le jour en toutes lettres et la taille", () => {
    expect(backupLabel({ name: "trackify-2026-03-09.db", day: "2026-03-09", sizeKb: 84 })).toBe("lundi 9 mars 2026 · 84 Ko");
  });

  it("retombe sur le nom du fichier si la date est illisible", () => {
    expect(backupLabel({ name: "trackify-xx.db", day: "xx", sizeKb: 2048 })).toBe("trackify-xx.db · 2,0 Mo");
  });
});

describe("formatSize", () => {
  it("n'affiche jamais 0 Ko", () => {
    expect(formatSize(0)).toBe("1 Ko");
    expect(formatSize(1536)).toBe("1,5 Mo");
  });
});
