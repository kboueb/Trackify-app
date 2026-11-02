import { describe, expect, it } from "vitest";
import { matchesSearch } from "./search";

describe("matchesSearch", () => {
  it("recherche vide : tout correspond", () => {
    expect(matchesSearch("", ["Site web"])).toBe(true);
  });
  it("ignore casse et accents", () => {
    expect(matchesSearch("equipe", ["Réunion d'Équipe"])).toBe(true);
  });
  it("chaque mot doit apparaître, dans le nom ou le client", () => {
    expect(matchesSearch("site acme", ["Site web", "ACME"])).toBe(true);
    expect(matchesSearch("site globex", ["Site web", "ACME"])).toBe(false);
  });
});
