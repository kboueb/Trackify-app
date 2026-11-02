import { FolderKanban, Settings, Square, Timer } from "lucide-react";
import { useMemo, useState, type ComponentType } from "react";
import { BrandMark } from "./components/BrandMark";
import { IdlePrompt } from "./components/IdlePrompt";
import { ColorDot } from "./components/ProjectLabel";
import { useDataEvents, useProjects, useRunning, useSettings, useStopTimer } from "./hooks/queries";
import { useNow } from "./hooks/useNow";
import { formatAccelerator } from "./lib/shortcut";
import { entryDuration, formatClock } from "./lib/time";
import { ProjectsPage } from "./pages/ProjectsPage";
import { SettingsPage } from "./pages/SettingsPage";
import { TimerPage } from "./pages/TimerPage";

type Page = "timer" | "projects" | "settings";

const NAV: { id: Page; label: string; icon: ComponentType<{ size?: number }> }[] = [
  { id: "timer", label: "Aujourd'hui", icon: Timer },
  { id: "projects", label: "Projets", icon: FolderKanban },
  { id: "settings", label: "Réglages", icon: Settings },
];

export default function App() {
  useDataEvents();
  const [page, setPage] = useState<Page>("timer");

  return (
    <div className="layout">
      {/* Zone de déplacement de la fenêtre (barre de titre intégrée sur macOS). */}
      <div className="drag-region" data-tauri-drag-region />
      <nav className="sidebar" aria-label="Navigation principale">
        <div className="brand">
          <BrandMark />
          Trackify
        </div>
        {NAV.map(({ id, label, icon: Icon }) => (
          <button
            key={id}
            type="button"
            className={`nav-item${page === id ? " active" : ""}`}
            aria-current={page === id ? "page" : undefined}
            onClick={() => setPage(id)}
          >
            <Icon size={16} />
            {label}
          </button>
        ))}
        <MiniStatus />
      </nav>
      <main className="content">
        {page === "timer" && <TimerPage onManageProjects={() => setPage("projects")} />}
        {page === "projects" && <ProjectsPage />}
        {page === "settings" && <SettingsPage />}
      </main>
      <IdlePrompt />
    </div>
  );
}

/** Rappel permanent, en bas de la barre latérale, de ce qui est en train d'être compté. */
function MiniStatus() {
  const now = useNow(1000);
  const { data: running } = useRunning();
  const { data: projects = [] } = useProjects(true);
  const { data: settings } = useSettings();
  const stop = useStopTimer();
  const project = useMemo(() => projects.find((p) => p.id === running?.projectId), [projects, running]);
  const shortcut = settings?.shortcut ? formatAccelerator(settings.shortcut) : null;

  if (!running) {
    return (
      <div className="mini-status">
        <span className="muted small">Aucun timer en cours</span>
        {shortcut && (
          <span className="subtle small">
            <span className="kbd">{shortcut}</span> pour démarrer
          </span>
        )}
      </div>
    );
  }
  return (
    <div className="mini-status">
      <div className="mini-status-head">
        <ColorDot color={project?.color ?? "#999"} live />
        <span className="mini-status-name">{project?.name ?? "Projet"}</span>
      </div>
      <div className="mini-status-row">
        <span className="mini-status-clock">{formatClock(entryDuration(running, now))}</span>
        <button type="button" className="icon-btn" title="Arrêter le timer" aria-label="Arrêter le timer" onClick={() => stop.mutate()}>
          <Square size={14} fill="currentColor" />
        </button>
      </div>
    </div>
  );
}
