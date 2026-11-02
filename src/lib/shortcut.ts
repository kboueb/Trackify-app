/** Conversion entre événements clavier, accélérateurs Tauri et affichage. */

export const isMac = typeof navigator !== "undefined" && /Mac/i.test(navigator.userAgent);

type KeyInfo = Pick<KeyboardEvent, "metaKey" | "ctrlKey" | "altKey" | "shiftKey" | "code">;

const MODIFIER_CODES = new Set([
  "MetaLeft", "MetaRight", "ControlLeft", "ControlRight",
  "AltLeft", "AltRight", "ShiftLeft", "ShiftRight", "CapsLock", "Fn",
]);

/**
 * Accélérateur Tauri pour une combinaison tapée, ou `null` tant que seules des
 * touches de modification sont enfoncées. `e.code` est utilisé tel quel (« KeyT »,
 * « Digit1 », « Space ») : il ne dépend pas de la disposition du clavier.
 */
export function acceleratorFromEvent(e: KeyInfo): string | null {
  if (MODIFIER_CODES.has(e.code)) return null;
  const parts: string[] = [];
  if (e.ctrlKey) parts.push("Control");
  if (e.altKey) parts.push("Alt");
  if (e.shiftKey) parts.push("Shift");
  if (e.metaKey) parts.push("Super");
  parts.push(e.code);
  return parts.join("+");
}

/** Vrai si l'accélérateur contient au moins une touche de modification. */
export function hasModifier(accelerator: string): boolean {
  return accelerator.split("+").length > 1;
}

const MAC_SYMBOLS: Record<string, string> = {
  COMMANDORCONTROL: "⌘", CMDORCTRL: "⌘", COMMAND: "⌘", CMD: "⌘", SUPER: "⌘",
  CONTROL: "⌃", CTRL: "⌃", ALT: "⌥", OPTION: "⌥", SHIFT: "⇧",
};

const PC_NAMES: Record<string, string> = {
  COMMANDORCONTROL: "Ctrl", CMDORCTRL: "Ctrl", COMMAND: "Win", CMD: "Win", SUPER: "Win",
  CONTROL: "Ctrl", CTRL: "Ctrl", ALT: "Alt", OPTION: "Alt", SHIFT: "Maj",
};

const KEY_NAMES: Record<string, string> = {
  SPACE: "Espace", ENTER: "Entrée", ESCAPE: "Échap", TAB: "Tab", BACKSPACE: "⌫",
  ARROWUP: "↑", ARROWDOWN: "↓", ARROWLEFT: "←", ARROWRIGHT: "→",
};

function keyLabel(key: string): string {
  const upper = key.toUpperCase();
  if (KEY_NAMES[upper]) return KEY_NAMES[upper];
  if (/^KEY[A-Z]$/.test(upper)) return upper.slice(3);
  if (/^DIGIT\d$/.test(upper)) return upper.slice(5);
  return key;
}

/** « CommandOrControl+Shift+Space » → « ⌘⇧Espace » (macOS) ou « Ctrl+Maj+Espace ». */
export function formatAccelerator(accelerator: string, mac = isMac): string {
  const tokens = accelerator.split("+").filter(Boolean);
  const labels = tokens.map((t, i) => {
    const upper = t.toUpperCase();
    const isLast = i === tokens.length - 1;
    if (!isLast) return (mac ? MAC_SYMBOLS : PC_NAMES)[upper] ?? t;
    return keyLabel(t);
  });
  return labels.join(mac ? "" : "+");
}
