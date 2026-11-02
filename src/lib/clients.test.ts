import { describe, expect, it } from "vitest";
import { clientOptions, findClient, knownClients } from "./clients";

const clients = knownClients([
  { client: "Neemba" }, { client: "Neemba" }, { client: "Solthis" },
  { client: "École Aimé Césaire" }, { client: "" },
]);

describe("knownClients", () => {
  it("dédoublonne, compte les projets et ignore « sans client »", () => {
    expect(clients).toEqual([
      { name: "École Aimé Césaire", projects: 1 },
      { name: "Neemba", projects: 2 },
      { name: "Solthis", projects: 1 },
    ]);
  });
});

describe("findClient", () => {
  it("retrouve un client malgré la casse et les accents", () => {
    expect(findClient(clients, "  neemba ")?.name).toBe("Neemba");
    expect(findClient(clients, "ecole aime cesaire")?.name).toBe("École Aimé Césaire");
    expect(findClient(clients, "Globex")).toBeUndefined();
    expect(findClient(clients, "")).toBeUndefined();
  });
});

describe("clientOptions", () => {
  it("saisie vide : « Sans client » puis tous les clients", () => {
    const opts = clientOptions(clients, "");
    expect(opts[0]).toEqual({ kind: "none" });
    expect(opts).toHaveLength(4);
  });
  it("filtre et propose d'ajouter un client inconnu", () => {
    expect(clientOptions(clients, "glo")).toEqual([{ kind: "new", name: "glo" }]);
    const opts = clientOptions(clients, "so");
    expect(opts).toEqual([{ kind: "existing", name: "Solthis", projects: 1 }, { kind: "new", name: "so" }]);
  });
  it("ne propose pas d'ajouter un client qui existe déjà", () => {
    expect(clientOptions(clients, "NEEMBA")).toEqual([{ kind: "existing", name: "Neemba", projects: 2 }]);
  });
});
