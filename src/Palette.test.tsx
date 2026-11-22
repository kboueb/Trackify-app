// @vitest-environment jsdom
import { screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it } from "vitest";
import { renderWithBackend } from "./test/render";

async function renderPalette() {
  const user = userEvent.setup();
  const view = await renderWithBackend(async () => {
    const { Palette } = await import("./Palette");
    return <Palette />;
  });
  // Le faux backend démarre avec « Multisite » en cours et « Refonte charte » juste avant.
  await screen.findByText("Refonte charte");
  return { user, ...view };
}

const runningName = async (commands: Awaited<ReturnType<typeof renderPalette>>["commands"]) => {
  const running = await commands.getRunning();
  const projects = await commands.listProjects(true);
  return projects.find((p) => p.id === running?.projectId)?.name;
};

describe("Palette", () => {
  it("Entrée bascule sur le projet précédent", async () => {
    const { user, commands } = await renderPalette();
    await user.keyboard("{Enter}");
    await waitFor(async () => expect(await runningName(commands)).toBe("Refonte charte"));
  });

  it("filtre les projets et démarre celui choisi", async () => {
    const { user, commands } = await renderPalette();
    await user.type(screen.getByPlaceholderText("Basculer sur un projet…"), "veil");
    expect(screen.queryByText("Refonte charte")).toBeNull();
    await user.keyboard("{Enter}");
    await waitFor(async () => expect(await runningName(commands)).toBe("Veille"));
  });

  it("crée un projet inconnu et le démarre", async () => {
    const { user, commands } = await renderPalette();
    await user.type(screen.getByPlaceholderText("Basculer sur un projet…"), "Audit SEO");
    await user.click(await screen.findByText("Créer « Audit SEO » et démarrer"));
    await waitFor(async () => expect(await runningName(commands)).toBe("Audit SEO"));
  });

  it("annote l'entrée en cours", async () => {
    const { user, commands } = await renderPalette();
    await user.click(screen.getByText("Modifier la note"));
    const input = await screen.findByPlaceholderText("Note pour Multisite…");
    await user.clear(input);
    await user.type(input, "Revue PR{Enter}");
    await waitFor(async () => expect((await commands.getRunning())?.note).toBe("Revue PR"));
  });
});
