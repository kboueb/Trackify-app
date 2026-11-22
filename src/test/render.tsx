/**
 * Rendu d'un composant branché sur le faux backend en mémoire (src/dev/mockBackend.ts),
 * pour les tests de composants. À utiliser dans un fichier marqué `@vitest-environment jsdom`.
 */
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { cleanup, render } from "@testing-library/react";
import type { ReactElement } from "react";
import { afterEach, vi } from "vitest";

// jsdom n'implémente pas ce dont cmdk a besoin pour faire défiler la liste.
globalThis.ResizeObserver ??= class {
  observe() {}
  unobserve() {}
  disconnect() {}
};
Element.prototype.scrollIntoView ??= function () {};

afterEach(() => {
  cleanup();
  vi.resetModules();
});

/**
 * Installe un faux backend neuf, puis rend l'élément construit par `build`.
 * `build` reçoit les modules rechargés après l'installation : l'état du faux backend
 * repart de zéro à chaque test.
 */
export async function renderWithBackend(build: () => Promise<ReactElement>) {
  const { installMockBackend } = await import("../dev/mockBackend");
  installMockBackend();
  const { commands } = await import("../bindings");
  const client = new QueryClient({ defaultOptions: { queries: { staleTime: Infinity, retry: false } } });
  const element = await build();
  const view = render(<QueryClientProvider client={client}>{element}</QueryClientProvider>);
  return { ...view, commands };
}
