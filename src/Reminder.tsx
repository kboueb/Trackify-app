import { BellRing, Hourglass } from "lucide-react";
import { useEffect, useState, type CSSProperties } from "react";
import { commands, events, type ReminderAction, type ReminderView } from "./bindings";
import { formatDuration } from "./lib/time";

/** Disparition automatique si l'utilisateur ne répond pas. */
const AUTO_HIDE_MS = 60_000;

const act = (action: ReminderAction) => void commands.reminderAction(action);

/** Fenêtre de rappel discrète (en haut à droite), sans prise de focus. */
export function Reminder() {
  const [view, setView] = useState<ReminderView | null>(null);

  useEffect(() => {
    const load = () => void commands.getReminder().then(setView);
    load();
    const unlisten = events.reminderShown.listen(load);
    return () => void unlisten.then((u) => u());
  }, []);

  useEffect(() => {
    if (!view) return;
    const id = window.setTimeout(() => act("dismiss"), AUTO_HIDE_MS);
    return () => window.clearTimeout(id);
  }, [view]);

  if (!view) return null;

  if (view.kind === "checkIn") {
    const elapsed = view.startedAt ? formatDuration(Date.now() - view.startedAt) : "";
    return (
      <div className="reminder" style={{ "--project": view.projectColor ?? undefined } as CSSProperties}>
        <div className="reminder-head">
          <span className="reminder-icon" aria-hidden>
            <Hourglass size={16} />
          </span>
          <div className="reminder-text">
            <strong>Toujours sur {view.projectName} ?</strong>
            <span className="muted small">Timer en cours depuis {elapsed}.</span>
          </div>
        </div>
        <div className="reminder-actions">
          <button type="button" className="btn btn-primary" onClick={() => act("dismiss")}>
            Oui
          </button>
          <button type="button" className="btn" onClick={() => act("switch")}>
            Changer…
          </button>
          <button type="button" className="btn btn-ghost" onClick={() => act("stop")}>
            Arrêter
          </button>
        </div>
      </div>
    );
  }

  return (
    <div className="reminder">
      <div className="reminder-head">
        <span className="reminder-icon" aria-hidden>
          <BellRing size={16} />
        </span>
        <div className="reminder-text">
          <strong>Aucun timer en cours</strong>
          <span className="muted small">Sur quoi travailles-tu ?</span>
        </div>
      </div>
      <div className="reminder-actions">
        <button type="button" className="btn btn-primary" onClick={() => act("switch")}>
          Choisir un projet…
        </button>
        <button type="button" className="btn btn-ghost" onClick={() => act("later")}>
          Plus tard (1 h)
        </button>
      </div>
    </div>
  );
}
