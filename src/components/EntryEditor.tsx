import { Merge, Scissors, Trash2 } from "lucide-react";
import { useState, type FormEvent } from "react";
import type { Project, TimeEntry } from "../bindings";
import { useDeleteEntry, useMergeEntries, useSaveEntry, useSplitEntry } from "../hooks/queries";
import { errorMessage } from "../lib/errors";
import { DAY, atTime, formatDuration, formatTime } from "../lib/time";
import { ConfirmButton } from "./ConfirmButton";

type Props = {
  /** Entrée modifiée ; absente pour un ajout manuel. */
  entry?: TimeEntry;
  /** Minuit local du jour affiché. */
  day: number;
  projects: Project[];
  /** Entrées voisines du même projet, qu'on peut fusionner avec celle-ci. */
  mergeable?: { before?: TimeEntry; after?: TimeEntry };
  /** Valeurs proposées pour un ajout. */
  defaults?: { projectId?: string; startedAt?: number; endedAt?: number };
  onClose: () => void;
};

/** Modifier, ajouter, scinder, fusionner ou supprimer une entrée. */
export function EntryEditor({ entry, day, projects, mergeable = {}, defaults = {}, onClose }: Props) {
  const running = entry?.endedAt === null;
  const [projectId, setProjectId] = useState(entry?.projectId ?? defaults.projectId ?? "");
  const [start, setStart] = useState(time(entry?.startedAt ?? defaults.startedAt));
  const [end, setEnd] = useState(time(entry?.endedAt ?? defaults.endedAt));
  const [note, setNote] = useState(entry?.note ?? "");
  const [splitAt, setSplitAt] = useState("");
  const [localError, setLocalError] = useState<string | null>(null);

  const save = useSaveEntry();
  const split = useSplitEntry();
  const merge = useMergeEntries();
  const remove = useDeleteEntry();
  const failure = save.error ?? split.error ?? merge.error ?? remove.error;
  const error = localError ?? (failure ? errorMessage(failure) : null);
  const busy = save.isPending || split.isPending || merge.isPending || remove.isPending;
  const done = { onSuccess: onClose };

  // Un projet archivé reste proposé s'il est déjà celui de l'entrée.
  const choices = projects.filter((p) => !p.archived || p.id === projectId);

  const submit = (e: FormEvent) => {
    e.preventDefault();
    const startedAt = atTime(day, start);
    let endedAt = running ? null : atTime(day, end);
    if (!projectId) return setLocalError("Choisis un projet.");
    if (startedAt === null || (!running && endedAt === null)) return setLocalError("Heures invalides (format HH:MM).");
    // Fin avant le début : l'entrée passe minuit.
    if (endedAt !== null && endedAt <= startedAt) endedAt += DAY;
    setLocalError(null);
    save.mutate({ id: entry?.id, input: { projectId, startedAt, endedAt, note } }, done);
  };

  const doSplit = () => {
    const at = entry ? atTime(day, splitAt) : null;
    if (!entry || at === null) return setLocalError("Heure de coupure invalide.");
    setLocalError(null);
    split.mutate({ id: entry.id, at }, done);
  };

  const s = atTime(day, start);
  const e = running ? Date.now() : atTime(day, end);
  const duration = s === null || e === null ? null : e > s ? e - s : e + DAY - s;

  return (
    <div className="modal-backdrop" onMouseDown={(ev) => ev.target === ev.currentTarget && onClose()}>
      <form
        className="modal card entry-editor"
        role="dialog"
        aria-modal="true"
        aria-labelledby="entry-editor-title"
        onSubmit={submit}
        onKeyDown={(ev) => ev.key === "Escape" && onClose()}
      >
        <h2 id="entry-editor-title">{entry ? "Modifier l'entrée" : "Ajouter une entrée"}</h2>

        <label className="field">
          Projet
          <select className="input" value={projectId} onChange={(ev) => setProjectId(ev.target.value)} autoFocus={!entry}>
            <option value="" disabled>
              Choisir un projet…
            </option>
            {choices.map((p) => (
              <option key={p.id} value={p.id}>
                {p.client ? `${p.name} — ${p.client}` : p.name}
              </option>
            ))}
          </select>
        </label>

        <div className="form-row times">
          <label className="field">
            Début
            <input type="time" className="input" value={start} onChange={(ev) => setStart(ev.target.value)} required />
          </label>
          <label className="field">
            Fin
            {running ? (
              <input className="input" value="En cours" disabled />
            ) : (
              <input type="time" className="input" value={end} onChange={(ev) => setEnd(ev.target.value)} required />
            )}
          </label>
          <span className="entry-editor-duration num">{duration !== null ? formatDuration(duration) : "—"}</span>
        </div>

        <label className="field">
          Note
          <input className="input" value={note} onChange={(ev) => setNote(ev.target.value)} placeholder="Facultatif" />
        </label>

        <p className="muted small">Les autres entrées recouvertes par ces horaires seront raccourcies ou coupées.</p>

        {entry && (
          <div className="entry-editor-tools">
            <div className="tool">
              <Scissors size={14} />
              <span>Couper à</span>
              <input
                type="time"
                className="input"
                value={splitAt}
                onChange={(ev) => setSplitAt(ev.target.value)}
                aria-label="Heure de coupure"
              />
              <button type="button" className="btn" disabled={!splitAt || busy} onClick={doSplit}>
                Scinder
              </button>
            </div>
            {mergeable.before && (
              <MergeButton
                other={mergeable.before}
                label="précédente"
                busy={busy}
                onMerge={(id) => merge.mutate([id, entry.id], done)}
              />
            )}
            {mergeable.after && (
              <MergeButton
                other={mergeable.after}
                label="suivante"
                busy={busy}
                onMerge={(id) => merge.mutate([entry.id, id], done)}
              />
            )}
          </div>
        )}

        {error && <p className="error">{error}</p>}

        <div className="form-actions">
          {entry && (
            <ConfirmButton label="Supprimer l'entrée" onConfirm={() => remove.mutate(entry.id, done)}>
              <Trash2 size={14} />
            </ConfirmButton>
          )}
          <span className="spacer" />
          <button type="button" className="btn btn-ghost" onClick={onClose}>
            Annuler
          </button>
          <button type="submit" className="btn btn-primary" disabled={busy}>
            {entry ? "Enregistrer" : "Ajouter"}
          </button>
        </div>
      </form>
    </div>
  );
}

function MergeButton({
  other,
  label,
  busy,
  onMerge,
}: {
  other: TimeEntry;
  label: string;
  busy: boolean;
  onMerge: (id: string) => void;
}) {
  const range = `${formatTime(other.startedAt)} – ${other.endedAt ? formatTime(other.endedAt) : "en cours"}`;
  return (
    <button type="button" className="btn tool-btn" disabled={busy} onClick={() => onMerge(other.id)}>
      <Merge size={14} /> Fusionner avec la {label} ({range})
    </button>
  );
}

function time(ts: number | null | undefined): string {
  return ts ? formatTime(ts) : "";
}
