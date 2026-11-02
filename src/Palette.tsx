import { Command } from "cmdk";
import { AppWindow, NotebookPen, Plus, Search, Square } from "lucide-react";
import { useEffect, useMemo, useRef, useState } from "react";
import { commands, events } from "./bindings";
import { ColorDot } from "./components/ProjectLabel";
import { useDataEvents, useProjects, useRunning, useSaveProject, useSetEntryNote, useStartTimer, useStopTimer } from "./hooks/queries";
import { PROJECT_COLORS } from "./lib/colors";
import { useNow } from "./hooks/useNow";
import { errorMessage } from "./lib/errors";
import { matchesSearch, normalize } from "./lib/search";
import { entryDuration, formatClock } from "./lib/time";

const close = () => void commands.hidePalette();

/** Sélecteur rapide ouvert par le raccourci global : taper, Entrée, c'est basculé. */
export function Palette() {
  useDataEvents();
  const now = useNow(1000);
  const { data: projects = [] } = useProjects(false);
  const { data: running } = useRunning();
  const start = useStartTimer();
  const stop = useStopTimer();
  const create = useSaveProject();
  const setNote = useSetEntryNote();
  const [search, setSearch] = useState("");
  // « note » : la saisie sert à annoter l'entrée en cours au lieu de chercher un projet.
  const [mode, setMode] = useState<"pick" | "note">("pick");
  const inputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    const unlisten = events.paletteOpened.listen(() => {
      setSearch("");
      setMode("pick");
      start.reset();
      stop.reset();
      create.reset();
      setNote.reset();
      inputRef.current?.focus();
    });
    return () => void unlisten.then((u) => u());
  }, []); // eslint-disable-line react-hooks/exhaustive-deps

  // Projets récents en premier, celui en cours en dernier : le premier choix est alors
  // le projet précédent, et « raccourci + Entrée » fait l'aller-retour entre deux projets.
  const runningId = running?.projectId;
  const sorted = useMemo(
    () =>
      [...projects].sort(
        (a, b) =>
          Number(a.id === runningId) - Number(b.id === runningId) ||
          (b.lastUsedAt ?? 0) - (a.lastUsedAt ?? 0) ||
          a.name.localeCompare(b.name, "fr"),
      ),
    [projects, runningId],
  );
  const runningProject = running ? projects.find((p) => p.id === running.projectId) : undefined;

  // Sélection pilotée : sans ça, cmdk garde la ligne choisie avant le chargement des projets.
  const [selected, setSelected] = useState("");
  const firstId = sorted[0]?.id ?? "__open";
  useEffect(() => {
    if (!search) setSelected(firstId);
  }, [firstId, search]);
  const error = start.error ?? stop.error ?? create.error ?? setNote.error;

  const pick = async (projectId: string) => {
    await start.mutateAsync(projectId);
    close();
  };

  // Nom inconnu : proposer de créer le projet (sans client, modifiable ensuite) et de le démarrer.
  const name = search.trim();
  const exists = projects.some((p) => normalize(p.name) === normalize(name));
  const createAndStart = async () => {
    const color = PROJECT_COLORS[projects.length % PROJECT_COLORS.length];
    const project = await create.mutateAsync({ input: { name, client: "", color } });
    await pick(project.id);
  };

  if (mode === "note" && running) {
    const save = async () => {
      await setNote.mutateAsync({ id: running.id, note: search });
      close();
    };
    return (
      <form
        className="palette"
        onSubmit={(e) => {
          e.preventDefault();
          void save();
        }}
        onKeyDown={(e) => {
          if (e.key === "Escape") {
            e.preventDefault();
            setMode("pick");
            setSearch("");
          }
        }}
      >
        <div className="palette-search">
          <NotebookPen size={18} />
          <input
            ref={inputRef}
            autoFocus
            className="palette-input"
            placeholder={`Note pour ${runningProject?.name ?? "l'entrée en cours"}…`}
            value={search}
            onChange={(e) => setSearch(e.target.value)}
          />
        </div>
        {error && <p className="error palette-error">{errorMessage(error)}</p>}
        <div className="palette-footer">
          <span>
            <span className="kbd">↵</span>enregistrer
          </span>
          <span>
            <span className="kbd">Échap</span>retour
          </span>
        </div>
      </form>
    );
  }

  return (
    <Command
      className="palette"
      label="Sélecteur de projet"
      loop
      value={selected}
      onValueChange={setSelected}
      filter={(_value, query, keywords = []) => (matchesSearch(query, keywords) ? 1 : 0)}
      onKeyDown={(e) => {
        if (e.key === "Escape") {
          e.preventDefault();
          close();
        }
      }}
    >
      <div className="palette-search">
        <Search size={18} />
        <Command.Input
          ref={inputRef}
          autoFocus
          className="palette-input"
          placeholder="Basculer sur un projet…"
          value={search}
          onValueChange={setSearch}
        />
      </div>
      {error && <p className="error palette-error">{errorMessage(error)}</p>}
      <Command.List className="palette-list">
        <Command.Empty className="palette-empty">Aucun projet ne correspond.</Command.Empty>

        <Command.Group heading="Projets">
          {sorted.map((p) => (
            <Command.Item key={p.id} value={p.id} keywords={[p.name, p.client]} onSelect={() => pick(p.id)}>
              <ColorDot color={p.color} live={p.id === running?.projectId} />
              <span className="palette-main">{p.name}</span>
              <span className="palette-meta">{p.id === running?.projectId ? "en cours" : p.client}</span>
            </Command.Item>
          ))}
        </Command.Group>

        {name && !exists && (
          <Command.Group heading="Nouveau">
            <Command.Item value="__create" keywords={[name]} onSelect={() => void createAndStart()}>
              <Plus size={14} />
              <span className="palette-main">Créer « {name} » et démarrer</span>
              <span className="palette-meta">sans client</span>
            </Command.Item>
          </Command.Group>
        )}

        {running && (
          <Command.Group heading="Timer">
            <Command.Item
              value="__stop"
              keywords={["arrêter", "stop", "pause", runningProject?.name ?? ""]}
              onSelect={async () => {
                await stop.mutateAsync();
                close();
              }}
            >
              <span className="palette-stop" aria-hidden>
                <Square size={11} fill="currentColor" />
              </span>
              <span className="palette-main">Arrêter le timer</span>
              <span className="palette-meta">
                {runningProject?.name} · {formatClock(entryDuration(running, now))}
              </span>
            </Command.Item>
            <Command.Item
              value="__note"
              keywords={["note", "annoter", "commentaire", runningProject?.name ?? ""]}
              onSelect={() => {
                setMode("note");
                setSearch(running.note);
              }}
            >
              <NotebookPen size={14} />
              <span className="palette-main">{running.note ? "Modifier la note" : "Ajouter une note"}</span>
              <span className="palette-meta">{running.note || runningProject?.name}</span>
            </Command.Item>
          </Command.Group>
        )}

        <Command.Group heading="Trackify">
          <Command.Item
            value="__open"
            keywords={["ouvrir", "trackify", "fenêtre", "projets"]}
            onSelect={() => void commands.openMainWindow()}
          >
            <AppWindow size={14} />
            <span className="palette-main">Ouvrir Trackify</span>
          </Command.Item>
        </Command.Group>
      </Command.List>
      <div className="palette-footer">
        <span>
          <span className="kbd">↑↓</span>naviguer
        </span>
        <span>
          <span className="kbd">↵</span>démarrer
        </span>
        <span>
          <span className="kbd">Échap</span>fermer
        </span>
      </div>
    </Command>
  );
}
