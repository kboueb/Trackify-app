import { save } from "@tauri-apps/plugin-dialog";
import { format, isSameDay } from "date-fns";
import { fr } from "date-fns/locale";
import { ChevronLeft, ChevronRight, Download } from "lucide-react";
import { useMemo, useState } from "react";
import { commands, type Project } from "../bindings";
import { ColorDot } from "../components/ProjectLabel";
import { useProjects, useReport, useSetRounding, useSettings } from "../hooks/queries";
import { errorMessage } from "../lib/errors";
import { ROUNDING_OPTIONS, csvFileName, formatHours, periodRange, pivot, shiftPeriod, type Period } from "../lib/report";

export function ReportsPage() {
  const [period, setPeriod] = useState<Period>("week");
  const [anchor, setAnchor] = useState(() => new Date());
  const { data: settings } = useSettings();
  const setRounding = useSetRounding();
  const rounding = settings?.roundingMin ?? 0;
  const { from, to, days } = useMemo(() => periodRange(period, anchor), [period, anchor]);
  const { data: rows = [], isLoading } = useReport(from, to, rounding);
  const { data: projects = [] } = useProjects(true);
  const byId = useMemo(() => new Map(projects.map((p) => [p.id, p])), [projects]);
  const table = useMemo(() => pivot(rows, days), [rows, days]);
  const [status, setStatus] = useState<{ ok: boolean; text: string } | null>(null);

  const isCurrent = periodRange(period, new Date()).from === from;
  const title =
    period === "day"
      ? format(anchor, "EEEE d MMMM yyyy", { locale: fr })
      : `Semaine du ${format(days[0], "d MMMM", { locale: fr })} au ${format(days[6], "d MMMM yyyy", { locale: fr })}`;

  const exportCsv = async () => {
    setStatus(null);
    try {
      const path = await save({
        title: "Exporter en CSV",
        defaultPath: csvFileName(period, anchor),
        filters: [{ name: "CSV", extensions: ["csv"] }],
      });
      if (!path) return;
      const count = await commands.exportCsv(from, to, rounding, path);
      setStatus({ ok: true, text: `${count} ligne${count > 1 ? "s" : ""} exportée${count > 1 ? "s" : ""}.` });
    } catch (e) {
      setStatus({ ok: false, text: errorMessage(e) });
    }
  };

  return (
    <div className="page">
      <header className="page-head">
        <h1>Rapports</h1>
        <div className="actions">
          <div className="segmented" role="radiogroup" aria-label="Période">
            {(["day", "week"] as const).map((p) => (
              <button key={p} type="button" role="radio" aria-checked={period === p} className={period === p ? "on" : ""} onClick={() => setPeriod(p)}>
                {p === "day" ? "Jour" : "Semaine"}
              </button>
            ))}
          </div>
          <button type="button" className="btn btn-primary" onClick={exportCsv} disabled={rows.length === 0}>
            <Download size={15} /> Exporter CSV
          </button>
        </div>
      </header>

      <div className="report-toolbar">
        <div className="day-nav">
          <button type="button" className="icon-btn" aria-label="Période précédente" onClick={() => setAnchor((a) => shiftPeriod(period, a, -1))}>
            <ChevronLeft size={16} />
          </button>
          <button type="button" className="icon-btn" aria-label="Période suivante" onClick={() => setAnchor((a) => shiftPeriod(period, a, 1))}>
            <ChevronRight size={16} />
          </button>
          <strong className="report-title">{title}</strong>
          {!isCurrent && (
            <button type="button" className="btn btn-ghost small-btn" onClick={() => setAnchor(new Date())}>
              {period === "day" ? "Aujourd'hui" : "Cette semaine"}
            </button>
          )}
        </div>
        <label className="report-rounding">
          Arrondi
          <select className="input" value={rounding} onChange={(e) => setRounding.mutate(Number(e.target.value))}>
            {ROUNDING_OPTIONS.map((o) => (
              <option key={o.value} value={o.value}>
                {o.label}
              </option>
            ))}
          </select>
        </label>
      </div>

      {status && <p className={status.ok ? "success" : "error"}>{status.text}</p>}

      {!isLoading && table.rows.length === 0 ? (
        <div className="card empty">Aucun temps enregistré sur cette période.</div>
      ) : period === "week" ? (
        <WeekTable table={table} days={days} byId={byId} />
      ) : (
        <DayTable table={table} byId={byId} />
      )}

      <p className="subtle small report-hint">
        Heures décimales{rounding ? `, arrondies à ${rounding} min par jour et par projet` : ""}. Le CSV reprend ces
        valeurs : une ligne par jour et par projet (date ; projet ; client ; durée ; note).
      </p>
    </div>
  );
}

type Table = ReturnType<typeof pivot>;

function ProjectCell({ project }: { project: Project | undefined }) {
  return (
    <th scope="row">
      <span className="report-project">
        <ColorDot color={project?.color ?? "#999"} />
        <span>
          {project?.name ?? "Projet supprimé"}
          {project?.client && <span className="subtle"> · {project.client}</span>}
        </span>
      </span>
    </th>
  );
}

function WeekTable({ table, days, byId }: { table: Table; days: Date[]; byId: Map<string, Project> }) {
  const today = new Date();
  return (
    <div className="card report-card">
      <table className="report-table">
        <thead>
          <tr>
            <th scope="col">Projet</th>
            {days.map((d) => (
              <th key={d.getTime()} scope="col" className={`num${isSameDay(d, today) ? " today" : ""}`}>
                {format(d, "EEE d", { locale: fr })}
              </th>
            ))}
            <th scope="col" className="num">
              Total
            </th>
          </tr>
        </thead>
        <tbody>
          {table.rows.map((r) => (
            <tr key={r.projectId}>
              <ProjectCell project={byId.get(r.projectId)} />
              {r.perDay.map((ms, i) => (
                <td key={i} className="num">
                  {ms ? formatHours(ms) : <span className="subtle">–</span>}
                </td>
              ))}
              <td className="num strong">{formatHours(r.total)}</td>
            </tr>
          ))}
        </tbody>
        <tfoot>
          <tr>
            <th scope="row">Total</th>
            {table.dayTotals.map((ms, i) => (
              <td key={i} className="num">
                {ms ? formatHours(ms) : ""}
              </td>
            ))}
            <td className="num strong">{formatHours(table.total)}</td>
          </tr>
        </tfoot>
      </table>
    </div>
  );
}

function DayTable({ table, byId }: { table: Table; byId: Map<string, Project> }) {
  return (
    <div className="card report-card">
      <table className="report-table">
        <thead>
          <tr>
            <th scope="col">Projet</th>
            <th scope="col">Notes</th>
            <th scope="col" className="num">
              Heures
            </th>
          </tr>
        </thead>
        <tbody>
          {table.rows.map((r) => (
            <tr key={r.projectId}>
              <ProjectCell project={byId.get(r.projectId)} />
              <td className="report-notes">{r.notes.join(" · ")}</td>
              <td className="num strong">{formatHours(r.total)}</td>
            </tr>
          ))}
        </tbody>
        <tfoot>
          <tr>
            <th scope="row">Total</th>
            <td />
            <td className="num strong">{formatHours(table.total)}</td>
          </tr>
        </tfoot>
      </table>
    </div>
  );
}
