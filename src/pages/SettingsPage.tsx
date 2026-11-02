import { useEffect, useState, type ReactNode } from "react";
import type { ReminderSettings } from "../bindings";
import {
  useAutostart,
  useSetAutostart,
  useSetReminderSettings,
  useSetShortcut,
  useSettings,
} from "../hooks/queries";
import { errorMessage } from "../lib/errors";
import { acceleratorFromEvent, formatAccelerator, hasModifier, isMac } from "../lib/shortcut";
import { minutesToTime, timeToMinutes } from "../lib/time";

export function SettingsPage() {
  return (
    <div className="page">
      <header className="page-head">
        <h1>Réglages</h1>
      </header>
      <ReminderSettingsCard />
      <GeneralSettings />
    </div>
  );
}

function Row({ label, hint, children }: { label: string; hint?: ReactNode; children: ReactNode }) {
  return (
    <div className="setting-row">
      <div className="setting-label">
        <strong>{label}</strong>
        {hint && <span>{hint}</span>}
      </div>
      <div className="setting-control">{children}</div>
    </div>
  );
}

// Rust envoie toujours tous les champs ; le type les marque optionnels à cause de `serde(default)`.
type Reminders = Required<ReminderSettings>;

function ReminderSettingsCard() {
  const { data: settings } = useSettings();
  const save = useSetReminderSettings();
  const saved = settings?.reminders as Reminders | undefined;
  const [draft, setDraft] = useState<Reminders | null>(null);

  useEffect(() => {
    if (saved) setDraft(saved);
  }, [saved]);

  if (!draft || !saved) return null;
  const dirty = JSON.stringify(draft) !== JSON.stringify(saved);
  const set = (patch: Partial<Reminders>) => setDraft({ ...draft, ...patch });
  const minutes = (value: string) => Math.max(0, Math.round(Number(value) || 0));
  const time = (key: "activeFromMin" | "activeUntilMin") => (value: string) => {
    const m = timeToMinutes(value);
    if (m !== null) set({ [key]: m });
  };

  return (
    <section className="section">
      <div className="section-head">
        <h2>Rappels et inactivité</h2>
      </div>
      <div className="card">
        <div className="settings-group">
          <Row label="Détection d'absence" hint="Sans clavier ni souris pendant que le timer tourne : on te demande quoi faire de ce temps.">
            <input
              type="number"
              className="input input-num"
              min={1}
              max={240}
              value={draft.idleThresholdMin}
              onChange={(e) => set({ idleThresholdMin: minutes(e.target.value) })}
              aria-label="Minutes avant absence"
            />
            <span className="muted">min</span>
          </Row>
          <Row label="Rappel si aucun timer" hint="Tu travailles mais rien n'est compté.">
            <input
              type="number"
              className="input input-num"
              min={1}
              max={240}
              disabled={!draft.noTimerEnabled}
              value={draft.noTimerAfterMin}
              onChange={(e) => set({ noTimerAfterMin: minutes(e.target.value) })}
              aria-label="Minutes d'activité avant le rappel"
            />
            <span className="muted">min</span>
            <input
              type="checkbox"
              className="switch"
              checked={draft.noTimerEnabled}
              onChange={(e) => set({ noTimerEnabled: e.target.checked })}
              aria-label="Activer le rappel si aucun timer"
            />
          </Row>
          <Row label="Point régulier" hint="« Toujours sur ce projet ? » pendant un long timer.">
            <input
              type="number"
              className="input input-num"
              min={5}
              max={480}
              disabled={!draft.checkinEnabled}
              value={draft.checkinEveryMin}
              onChange={(e) => set({ checkinEveryMin: minutes(e.target.value) })}
              aria-label="Minutes entre deux points"
            />
            <span className="muted">min</span>
            <input
              type="checkbox"
              className="switch"
              checked={draft.checkinEnabled}
              onChange={(e) => set({ checkinEnabled: e.target.checked })}
              aria-label="Activer le point régulier"
            />
          </Row>
          <Row label="Horaires des rappels" hint="En dehors, aucun rappel (l'absence reste détectée).">
            <input
              type="time"
              className="input input-time"
              value={minutesToTime(draft.activeFromMin)}
              onChange={(e) => time("activeFromMin")(e.target.value)}
              aria-label="Début de la plage des rappels"
            />
            <span className="muted">à</span>
            <input
              type="time"
              className="input input-time"
              value={minutesToTime(draft.activeUntilMin)}
              onChange={(e) => time("activeUntilMin")(e.target.value)}
              aria-label="Fin de la plage des rappels"
            />
          </Row>
          <Row label="Jours ouvrés uniquement" hint="Pas de rappel le week-end.">
            <input
              type="checkbox"
              className="switch"
              checked={draft.weekdaysOnly}
              onChange={(e) => set({ weekdaysOnly: e.target.checked })}
              aria-label="Jours ouvrés uniquement"
            />
          </Row>
        </div>
        {(dirty || save.error) && (
          <div className="settings-footer">
            {save.error && <p className="error small">{errorMessage(save.error)}</p>}
            <button type="button" className="btn btn-ghost" onClick={() => setDraft(saved)}>
              Annuler
            </button>
            <button type="button" className="btn btn-primary" disabled={save.isPending} onClick={() => save.mutate(draft)}>
              Enregistrer
            </button>
          </div>
        )}
      </div>
    </section>
  );
}

function GeneralSettings() {
  return (
    <section className="section">
      <div className="section-head">
        <h2>Général</h2>
      </div>
      <div className="card">
        <div className="settings-group">
          <ShortcutRow />
          <AutostartRow />
          <Row
            label="Fermer la fenêtre"
            hint={`Trackify reste dans ${isMac ? "la barre des menus" : "la zone de notification"} et le timer continue. Pour quitter : menu de l'icône › Quitter.`}
          >
            {null}
          </Row>
        </div>
      </div>
    </section>
  );
}

function ShortcutRow() {
  const { data: settings } = useSettings();
  const setShortcut = useSetShortcut();
  const [recording, setRecording] = useState(false);
  const [hint, setHint] = useState<string | null>(null);

  useEffect(() => {
    if (!recording) return;
    const onKey = (e: KeyboardEvent) => {
      e.preventDefault();
      e.stopPropagation();
      if (e.key === "Escape") {
        setRecording(false);
        return;
      }
      const accelerator = acceleratorFromEvent(e);
      if (!accelerator) return;
      if (!hasModifier(accelerator)) {
        setHint("Ajoute au moins une touche de modification (⌘, ⌃, ⌥ ou ⇧).");
        return;
      }
      setRecording(false);
      setShortcut.mutate(accelerator);
    };
    window.addEventListener("keydown", onKey, true);
    return () => window.removeEventListener("keydown", onKey, true);
  }, [recording, setShortcut]);

  const description = setShortcut.error
    ? <span className="error">{errorMessage(setShortcut.error)}</span>
    : recording
      ? hint ?? "Tape la combinaison voulue (Échap pour annuler)."
      : "Ouvre le sélecteur de projet depuis n'importe quelle application.";

  return (
    <Row label="Raccourci du sélecteur rapide" hint={description}>
      {recording ? (
        <span className="kbd recording">En écoute…</span>
      ) : (
        <span className="kbd">{settings?.shortcut ? formatAccelerator(settings.shortcut) : "…"}</span>
      )}
      <button
        type="button"
        className="btn"
        onClick={() => {
          setHint(null);
          setShortcut.reset();
          setRecording(!recording);
        }}
      >
        {recording ? "Annuler" : "Modifier"}
      </button>
    </Row>
  );
}

function AutostartRow() {
  const { data: enabled, error: loadError } = useAutostart();
  const setAutostart = useSetAutostart();
  const error = loadError ?? setAutostart.error;

  return (
    <Row
      label="Lancer à l'ouverture de session"
      hint={error ? <span className="error">{errorMessage(error)}</span> : "Démarre en arrière-plan, fenêtre masquée."}
    >
      <input
        type="checkbox"
        className="switch"
        checked={enabled ?? false}
        disabled={enabled === undefined || setAutostart.isPending}
        onChange={(e) => setAutostart.mutate(e.target.checked)}
        aria-label="Lancer à l'ouverture de session"
      />
    </Row>
  );
}
