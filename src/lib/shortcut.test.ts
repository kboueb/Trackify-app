import { describe, expect, it } from "vitest";
import { acceleratorFromEvent, formatAccelerator, hasModifier } from "./shortcut";

const key = (code: string, mods: Partial<Record<"metaKey" | "ctrlKey" | "altKey" | "shiftKey", boolean>> = {}) => ({
  metaKey: false, ctrlKey: false, altKey: false, shiftKey: false, code, ...mods,
});

describe("acceleratorFromEvent", () => {
  it("ignore les touches de modification seules", () => {
    expect(acceleratorFromEvent(key("ShiftLeft", { shiftKey: true }))).toBeNull();
    expect(acceleratorFromEvent(key("MetaLeft", { metaKey: true }))).toBeNull();
  });
  it("assemble modificateurs puis touche, dans un ordre stable", () => {
    expect(acceleratorFromEvent(key("KeyT", { metaKey: true, shiftKey: true }))).toBe("Shift+Super+KeyT");
    expect(acceleratorFromEvent(key("Space", { ctrlKey: true, altKey: true }))).toBe("Control+Alt+Space");
  });
});

describe("hasModifier", () => {
  it("détecte l'absence de modificateur", () => {
    expect(hasModifier("KeyT")).toBe(false);
    expect(hasModifier("Alt+KeyT")).toBe(true);
  });
});

describe("formatAccelerator", () => {
  it("utilise les symboles sur macOS", () => {
    expect(formatAccelerator("CommandOrControl+Shift+Space", true)).toBe("⌘⇧Espace");
    expect(formatAccelerator("Control+Alt+Super+KeyT", true)).toBe("⌃⌥⌘T");
  });
  it("utilise les noms ailleurs", () => {
    expect(formatAccelerator("CommandOrControl+Shift+Space", false)).toBe("Ctrl+Maj+Espace");
    expect(formatAccelerator("Alt+Digit1", false)).toBe("Alt+1");
  });
});
