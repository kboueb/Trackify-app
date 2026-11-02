import { format } from "date-fns";
import { fr } from "date-fns/locale";
import { Play, Search, Square, Trash2 } from "lucide-react";
import { useEffect, useMemo, useRef, useState, type CSSProperties } from "react";
import type { Project, TimeEntry } from "../bindings";
import { ConfirmButton } from "../components/ConfirmButton";
import { ColorDot } from "../components/ProjectLabel";
import {
  useDeleteEntry,
  useEntries,
  useProjects,
  useRunning,
  useSetEntryNote,
  useSettings,
  useStartTimer,
  useStopTimer,
} from "../hooks/queries";
import { useNow } from "../hooks/useNow";
import { dayWindow, segments } from "../lib/daybar";
import { errorMessage } from "../lib/errors";
import { matchesSearch } from "../lib/search";
import { formatAccelerator } from "../lib/shortcut";
import { HOUR, dayRange, durationWithin, entryDuration, formatClock, formatDuration, formatTime } from "../lib/time";

const projectStyle = (color: string | undefined) => ({ "--project": color ?? "var(--text-3)" }) as CSSProperties;

export function TimerPage({ onManageProjects }: { onManageProjects: () => void }) {
  const now = useNow(1000);
  const { data: running } = useRunning();
  // Projets archivés inclus : les entrées du jour peuvent y faire référence.
  const { data: allProjects = [] } = useProjects(true);
  const byId = useMemo(() => new Map(allProjects.map((p) => [p.id, p])), [allProjects]);

  const dayKey = new Date(now).toDateString();
  const [from, to] = useMemo(() => dayRange(new Date(now)), [dayKey]); // eslint-disable-line react-hooks/exhaustive-deps
  const { data: entries = [] } = useEntries(from, to);

  const totals = useMemo(() => {
    const map = new Map<string, number>();
    for (const e of entries) map.set(e.projectId, (map.get(e.projectId) ?? 0) + durationWithin(e, from, to, now));
    return map;
  }, [entries, from, to, now]);

  const start = useStartTimer();
  const stop = useStopTimer();
  const error = start.error ?? stop.error;
  const searchRef = useRef<HTMLInputElement>(null);

  return (
    <div className="page">
      <header className="page-head">
        <h1>Aujourd'hui</h1>
        <span className="muted">{format(now, "EEEE d MMMM", { locale: fr })}</span>
      </header>

      <Hero
        running={running ?? null}
        project={running ? byId.get(running.projectId) : undefined}
        now={now}
        onStop={() => stop.mutate()}
        onStart={() => searchRef.current?.focus()}
      />
      {error && <p className="error">{errorMessage(error)}</p>}

      <ProjectPicker
        projects={allProjects.filter((p) => !p.archived)}
        activeId={running?.projectId}
        totals={totals}
        searchRef={searchRef}
        onPick={(id) => start.mutate(id)}
        onManageProjects={onManageProjects}
      />

      <Day entries={entries} byId={byId} totals={totals} from={from} to={to} now={now} />
    </div>
  );
}

function Hero({
  running,
  project,
  now,
  onStop,
  onStart,
}: {
  running: TimeEntry | null;
  project: Project | undefined;
  now: number;
  onStop: () => void;
  onStart: () => void;
}) {
  const { data: settings } = useSettings();

  if (!running) {
    return (
      <div className="hero idle">
        <div className="hero-top">
          <span className="hero-project muted">Aucun timer en cours</span>
          {settings?.shortcut && (
            <span className="subtle small">
              Partout : <span className="kbd">{formatAccelerator(settings.shortcut)}</span>
            </span>
          )}
        </div>
        <div className="hero-main">
          <div className="hero-clock">0:00:00</div>
          <button type="button" className="stop-btn" onClick={onStart}>
            <Play size={14} fill="currentColor" /> Démarrer
          </button>
        </div>
      </div>
    );
  }

  return (
    <div className="hero" style={projectStyle(project?.color)}>
      <div className="hero-top">
        <span className="hero-project">
          <span className="dot live" aria-hidden />
          {project?.name ?? "Projet supprimé"}
          {project?.client && <span className="muted"> · {project.client}</span>}
        </span>
        <span className="subtle small">Depuis {formatTime(running.startedAt)}</span>
      </div>
      <div className="hero-main">
        <div className="hero-clock" aria-live="off">
          {formatClock(entryDuration(running, now))}
        </div>
        <button type="button" className="stop-btn" onClick={onStop}>
          <Square size={12} fill="currentColor" /> Arrêter
        </button>
      </div>
      <NoteInput entry={running} />
    </div>
  );
}

function NoteInput({ entry }: { entry: TimeEntry }) {
  const [value, setValue] = useState(entry.note);
  const setNote = useSetEntryNote();
  useEffect(() => setValue(entry.note), [entry.id, entry.note]);

  const save = () => {
    if (value.trim() !== entry.note) setNote.mutate({ id: entry.id, note: value });
  };
  return (
    <input
      className="note-input"
      placeholder="Ajouter une note…"
      aria-label="Note de l'entrée en cours"
      value={value}
      onChange={(e) => setValue(e.target.value)}
      onBlur={save}
      onKeyDown={(e) => e.key === "Enter" && e.currentTarget.blur()}
    />
  );
}

function ProjectPicker({
  projects,
  activeId,
  totals,
  searchRef,
  onPick,
  onManageProjects,
}: {
  projects: Project[];
  activeId: string | undefined;
  totals: Map<string, number>;
  searchRef: React.RefObject<HTMLInputElement | null>;
  onPick: (id: string) => void;
  onManageProjects: () => void;
}) {
  const [query, setQuery] = useState("");
  // Les plus récemment utilisés d'abord : ce sont ceux entre lesquels on jongle.
  const sorted = useMemo(
    () =>
      [...projects].sort(
        (a, b) => (b.lastUsedAt ?? 0) - (a.lastUsedAt ?? 0) || a.name.localeCompare(b.name, "fr"),
      ),
    [projects],
  );
  const filtered = sorted.filter((p) => matchesSearch(query, [p.name, p.client]));

  return (
    <section className="section">
      <div className="section-head">
        <h2>Projets</h2>
        {projects.length > 0 && (
          <div className="search aside">
            <Search size={14} />
            <input
              ref={searchRef}
              className="input"
              style={{ width: 220 }}
              placeholder="Rechercher…"
              aria-label="Rechercher un projet"
              value={query}
              onChange={(e) => setQuery(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === "Enter" && filtered[0]) {
                  onPick(filtered[0].id);
                  setQuery("");
                  e.currentTarget.blur();
                }
                if (e.key === "Escape") setQuery("");
              }}
            />
          </div>
        )}
      </div>

      {projects.length === 0 ? (
        <div className="card empty">
          Aucun projet pour l'instant.{" "}
          <button type="button" className="link" onClick={onManageProjects}>
            Créer un projet
          </button>
        </div>
      ) : filtered.length === 0 ? (
        <div className="card empty">Aucun projet ne correspond à « {query} ».</div>
      ) : (
        <div className="project-picker">
          {filtered.map((p) => {
            const active = p.id === activeId;
            const total = totals.get(p.id);
            return (
              <button
                key={p.id}
                type="button"
                className={`pick${active ? " active" : ""}`}
                style={projectStyle(p.color)}
                onClick={() => onPick(p.id)}
                aria-pressed={active}
                title={active ? "En cours" : `Démarrer « ${p.name} »`}
              >
                <span className={active ? "dot live" : "dot"} aria-hidden />
                <span className="pick-text">
                  <span className="pick-name">{p.name}</span>
                  <span className="pick-client">{p.client || "Sans client"}</span>
                </span>
                {total ? <span className="pick-total">{formatDuration(total)}</span> : null}
                <span className="pick-icon" aria-hidden>
                  <Play size={11} fill="currentColor" />
                </span>
              </button>
            );
          })}
        </div>
      )}
    </section>
  );
}

function Day({
  entries,
  byId,
  totals,
  from,
  to,
  now,
}: {
  entries: TimeEntry[];
  byId: Map<string, Project>;
  totals: Map<string, number>;
  from: number;
  to: number;
  now: number;
}) {
  const remove = useDeleteEntry();
  const total = [...totals.values()].reduce((a, b) => a + b, 0);
  const visible = dayWindow(entries, from, now);
  const [fromHour, toHour] = visible;
  const hours = toHour - fromHour;
  const segs = segments(entries, from, visible, now);
  const nowPct = (((now - from) / HOUR - fromHour) / hours) * 100;
  const colorOf = (projectId: string) => byId.get(projectId)?.color ?? "#999999";

  return (
    <section className="section">
      <div className="section-head">
        <h2>Journée</h2>
        <span className="aside num">
          <strong>{formatDuration(total)}</strong>
        </span>
      </div>

      <div className="card">
        <div className="daybar">
          <div className="daybar-track" style={{ "--hour": `${100 / hours}%` } as CSSProperties}>
            {segs.map((s) => (
              <div
                key={s.id}
                className={`daybar-seg${s.live ? " live" : ""}`}
                style={{ left: `${s.left}%`, width: `${s.width}%`, background: colorOf(s.projectId) }}
                title={byId.get(s.projectId)?.name}
              />
            ))}
            {nowPct > 0 && nowPct < 100 && <div className="daybar-now" style={{ left: `${nowPct}%` }} />}
          </div>
          <div className="daybar-hours" aria-hidden>
            {Array.from({ length: hours + 1 }, (_, i) => fromHour + i)
              .filter((h) => h % 2 === 0)
              .map((h) => (
                <span key={h} style={{ left: `${((h - fromHour) / hours) * 100}%` }}>
                  {h}h
                </span>
              ))}
          </div>
        </div>
        {totals.size > 0 && (
          <div className="legend">
            {[...totals]
              .sort((a, b) => b[1] - a[1])
              .map(([projectId, ms]) => (
                <span key={projectId}>
                  <ColorDot color={colorOf(projectId)} />
                  {byId.get(projectId)?.name ?? "?"} <strong>{formatDuration(ms)}</strong>
                </span>
              ))}
          </div>
        )}
      </div>

      {entries.length === 0 ? (
        <div className="card empty">Rien d'enregistré aujourd'hui pour l'instant.</div>
      ) : (
        <ul className="card list">
          {[...entries].reverse().map((e) => {
            const project = byId.get(e.projectId);
            return (
              <li key={e.id} className="entry">
                <ColorDot color={colorOf(e.projectId)} live={e.endedAt === null} />
                <div className="entry-main">
                  <span className="entry-name">{project?.name ?? "Projet supprimé"}</span>
                  <span className="entry-note">{e.note || project?.client}</span>
                </div>
                <span className="entry-time">
                  {formatTime(e.startedAt)} – {e.endedAt ? formatTime(e.endedAt) : "en cours"}
                </span>
                <span className="entry-duration">{formatDuration(durationWithin(e, from, to, now))}</span>
                <span className="row-actions">
                  <ConfirmButton label="Supprimer l'entrée" onConfirm={() => remove.mutate(e.id)}>
                    <Trash2 size={14} />
                  </ConfirmButton>
                </span>
              </li>
            );
          })}
        </ul>
      )}
    </section>
  );
}
