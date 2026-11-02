import type { Project } from "../bindings";
import { matchesSearch, normalize } from "./search";

export type ClientInfo = { name: string; projects: number };

export type ClientOption =
  | { kind: "existing"; name: string; projects: number }
  | { kind: "new"; name: string }
  | { kind: "none" };

/** Clients connus (projets archivés compris), triés, avec leur nombre de projets. */
export function knownClients(projects: Pick<Project, "client">[]): ClientInfo[] {
  const counts = new Map<string, number>();
  for (const p of projects) if (p.client) counts.set(p.client, (counts.get(p.client) ?? 0) + 1);
  return [...counts]
    .map(([name, n]) => ({ name, projects: n }))
    .sort((a, b) => a.name.localeCompare(b.name, "fr"));
}

/** Client existant qui correspond à la saisie aux accents et à la casse près. */
export function findClient(clients: ClientInfo[], value: string): ClientInfo | undefined {
  const key = normalize(value.trim());
  return key ? clients.find((c) => normalize(c.name) === key) : undefined;
}

/** Options de la liste pour une saisie : clients correspondants, puis « Ajouter » si c'est nouveau. */
export function clientOptions(clients: ClientInfo[], query: string): ClientOption[] {
  const q = query.trim();
  if (!q) {
    return [{ kind: "none" }, ...clients.map((c) => ({ kind: "existing" as const, ...c }))];
  }
  const options: ClientOption[] = clients
    .filter((c) => matchesSearch(q, [c.name]))
    .map((c) => ({ kind: "existing" as const, ...c }));
  if (!findClient(clients, q)) options.push({ kind: "new", name: q });
  return options;
}
