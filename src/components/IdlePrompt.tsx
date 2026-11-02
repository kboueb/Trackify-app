import { format, isSameDay } from "date-fns";
import { fr } from "date-fns/locale";
import { Coffee, Power } from "lucide-react";
import { useEffect, useState } from "react";
import type { IdlePeriod, IdleResolution } from "../bindings";
import { useProjects, usePendingIdle, useResolveIdle } from "../hooks/queries";
import { errorMessage } from "../lib/errors";
import { formatDuration } from "../lib/time";
import { ColorDot } from "./ProjectLabel";

/** « 14h10 », précédé du jour si ce n'est pas aujourd'hui. */
function formatMoment(ts: number): string {
  return isSameDay(ts, Date.now()) ? format(ts, "HH'h'mm") : format(ts, "EEE d MMM HH'h'mm", { locale: fr });
}

/** Question bloquante sur la plus ancienne absence en attente. */
export function IdlePrompt() {
  const { data: pending = [] } = usePendingIdle();
  const period = pending[0];
  if (!period) return null;
  return <IdleDialog key={period.id} period={period} remaining={pending.length} />;
}

function IdleDialog({ period, remaining }: { period: IdlePeriod; remaining: number }) {
  const { data: projects = [] } = useProjects(true);
  const resolve = useResolveIdle();
  const [target, setTarget] = useState("");
  const current = projects.find((p) => p.id === period.projectId);
  const others = projects.filter((p) => !p.archived && p.id !== period.projectId);

  useEffect(() => setTarget(""), [period.id]);

  const send = (resolution: IdleResolution) => resolve.mutate({ id: period.id, resolution });
  const range = `de ${formatMoment(period.startedAt)} à ${formatMoment(period.endedAt)}`;
  const duration = formatDuration(period.endedAt - period.startedAt);

  return (
    <div className="modal-backdrop">
      <div className="modal card" role="alertdialog" aria-labelledby="idle-title" aria-modal="true">
        <span className="modal-icon" aria-hidden>
          {period.kind === "closed" ? <Power size={18} /> : <Coffee size={18} />}
        </span>
        <h2 id="idle-title">
          {period.kind === "closed" ? `Trackify était fermé ${range}` : `Tu as été absent ${range}`}
        </h2>
        <p>
          Soit <strong>{duration}</strong> comptées sur{" "}
          <span className="project-chip">
            {current && <ColorDot color={current.color} />}
            {current?.name ?? "un projet supprimé"}
          </span>
          . Que faire de ce temps ?
        </p>

        <div className="idle-actions">
          <button type="button" className="btn" disabled={resolve.isPending} onClick={() => send({ action: "discard" })}>
            Le retirer
          </button>
          <button type="button" className="btn btn-primary" disabled={resolve.isPending} onClick={() => send({ action: "keep" })}>
            Le garder
          </button>
        </div>

        <div className="idle-reassign">
          <select className="input" value={target} onChange={(e) => setTarget(e.target.value)} aria-label="Autre projet">
            <option value="">L'attribuer à un autre projet…</option>
            {others.map((p) => (
              <option key={p.id} value={p.id}>
                {p.client ? `${p.name} — ${p.client}` : p.name}
              </option>
            ))}
          </select>
          <button
            type="button"
            className="btn"
            disabled={!target || resolve.isPending}
            onClick={() => send({ action: "reassign", projectId: target })}
          >
            Attribuer
          </button>
        </div>

        {resolve.error && <p className="error">{errorMessage(resolve.error)}</p>}
        {remaining > 1 && <p className="muted small">Encore {remaining - 1} absence(s) à traiter ensuite.</p>}
      </div>
    </div>
  );
}
