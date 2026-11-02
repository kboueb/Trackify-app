import { formatDistanceToNowStrict } from "date-fns";
import { fr } from "date-fns/locale";
import { Archive, ArchiveRestore, Pencil, Plus, Trash2 } from "lucide-react";
import { useMemo, useState, type FormEvent } from "react";
import type { Project } from "../bindings";
import { ClientCombobox } from "../components/ClientCombobox";
import { ConfirmButton } from "../components/ConfirmButton";
import { ColorDot } from "../components/ProjectLabel";
import { useArchiveProject, useDeleteProject, useProjects, useRenameClient, useSaveProject } from "../hooks/queries";
import { knownClients } from "../lib/clients";
import { PROJECT_COLORS } from "../lib/colors";
import { errorMessage } from "../lib/errors";

const PALETTE = PROJECT_COLORS;

type Draft = { id?: string; name: string; client: string; color: string };

export function ProjectsPage() {
  const [showArchived, setShowArchived] = useState(false);
  const [draft, setDraft] = useState<Draft | null>(null);
  const { data: projects = [], isLoading } = useProjects(showArchived);
  const archive = useArchiveProject();
  const remove = useDeleteProject();
  const actionError = archive.error ?? remove.error;

  // Regroupés par client (« Sans client » en dernier), dans l'ordre renvoyé par Rust.
  const groups = useMemo(() => {
    const map = new Map<string, Project[]>();
    for (const p of projects) map.set(p.client, [...(map.get(p.client) ?? []), p]);
    return [...map].sort(([a], [b]) => Number(a === "") - Number(b === "") || a.localeCompare(b, "fr"));
  }, [projects]);

  const newDraft = (client = ""): Draft => ({
    name: "",
    client,
    color: clientColor(projects, client) ?? PALETTE[projects.length % PALETTE.length],
  });

  return (
    <div className="page">
      <header className="page-head">
        <h1>Projets</h1>
        <span className="muted">{projects.filter((p) => !p.archived).length} actifs</span>
        <div className="actions">
          <label className="checkbox small">
            <input type="checkbox" className="switch" checked={showArchived} onChange={(e) => setShowArchived(e.target.checked)} />
            Archivés
          </label>
          <button type="button" className="btn btn-primary" onClick={() => setDraft(newDraft())}>
            <Plus size={15} /> Nouveau projet
          </button>
        </div>
      </header>

      {draft && !draft.id && <ProjectForm draft={draft} onDone={() => setDraft(null)} />}
      {actionError && <p className="error">{errorMessage(actionError)}</p>}

      {!isLoading && projects.length === 0 && !draft && (
        <div className="card empty">
          Crée tes projets avec <strong>exactement</strong> les mêmes noms que dans ton timesheet officiel :
          <br />
          l'export les reprendra tels quels.
        </div>
      )}

      <div className="client-groups">
        {groups.map(([client, items]) => (
          <section key={client} className="client-group">
            <ClientHeader client={client} count={items.length} onAdd={() => setDraft(newDraft(client))} />
            <ul className="card list">
              {items.map((p) =>
                draft?.id === p.id ? (
                  <li key={p.id}>
                    <ProjectForm draft={draft} onDone={() => setDraft(null)} />
                  </li>
                ) : (
                  <ProjectRow
                    key={p.id}
                    project={p}
                    onEdit={() => setDraft({ id: p.id, name: p.name, client: p.client, color: p.color })}
                    onArchive={() => archive.mutate({ id: p.id, archived: !p.archived })}
                    onDelete={() => remove.mutate(p.id)}
                  />
                ),
              )}
            </ul>
          </section>
        ))}
      </div>
    </div>
  );
}

/** Couleur du projet le plus récent de ce client, pour garder une teinte par client. */
function clientColor(projects: Project[], client: string): string | undefined {
  if (!client) return undefined;
  return projects
    .filter((p) => p.client === client)
    .sort((a, b) => (b.lastUsedAt ?? b.createdAt) - (a.lastUsedAt ?? a.createdAt))[0]?.color;
}

/** Titre d'un groupe : ajouter un projet à ce client, ou renommer le client partout. */
function ClientHeader({ client, count, onAdd }: { client: string; count: number; onAdd: () => void }) {
  const [renaming, setRenaming] = useState(false);
  const [value, setValue] = useState(client);
  const rename = useRenameClient();

  const submit = () => {
    const to = value.trim();
    if (!to || to === client) return setRenaming(false);
    rename.mutate({ from: client, to }, { onSuccess: () => setRenaming(false) });
  };

  return (
    <div className="client-head">
      {renaming ? (
        <input
          className="input client-rename"
          autoFocus
          value={value}
          aria-label={`Nouveau nom pour ${client}`}
          onChange={(e) => setValue(e.target.value)}
          onBlur={submit}
          onKeyDown={(e) => {
            if (e.key === "Enter") submit();
            if (e.key === "Escape") {
              setValue(client);
              setRenaming(false);
            }
          }}
        />
      ) : (
        <h3>{client || "Sans client"}</h3>
      )}
      <span className="client-actions">
        {client && !renaming && (
          <button
            type="button"
            className="icon-btn"
            title={`Renommer « ${client} » sur ses ${count} projet${count > 1 ? "s" : ""}`}
            aria-label={`Renommer le client ${client}`}
            onClick={() => {
              setValue(client);
              setRenaming(true);
            }}
          >
            <Pencil size={13} />
          </button>
        )}
        <button
          type="button"
          className="icon-btn"
          title={client ? `Nouveau projet pour ${client}` : "Nouveau projet sans client"}
          aria-label={client ? `Nouveau projet pour ${client}` : "Nouveau projet sans client"}
          onClick={onAdd}
        >
          <Plus size={14} />
        </button>
      </span>
      {rename.error && <span className="error small">{errorMessage(rename.error)}</span>}
    </div>
  );
}

function ProjectRow({
  project,
  onEdit,
  onArchive,
  onDelete,
}: {
  project: Project;
  onEdit: () => void;
  onArchive: () => void;
  onDelete: () => void;
}) {
  return (
    <li className={`project-item${project.archived ? " archived" : ""}`}>
      <ColorDot color={project.color} />
      <span className="project-item-name">{project.name}</span>
      {project.archived && <span className="badge">Archivé</span>}
      <span className="subtle small">
        {project.lastUsedAt
          ? `Utilisé il y a ${formatDistanceToNowStrict(project.lastUsedAt, { locale: fr })}`
          : "Jamais utilisé"}
      </span>
      <span className="row-actions">
        <button type="button" className="icon-btn" title="Modifier" aria-label={`Modifier ${project.name}`} onClick={onEdit}>
          <Pencil size={14} />
        </button>
        <button
          type="button"
          className="icon-btn"
          title={project.archived ? "Désarchiver" : "Archiver"}
          aria-label={`${project.archived ? "Désarchiver" : "Archiver"} ${project.name}`}
          onClick={onArchive}
        >
          {project.archived ? <ArchiveRestore size={14} /> : <Archive size={14} />}
        </button>
        <ConfirmButton label={`Supprimer ${project.name}`} onConfirm={onDelete}>
          <Trash2 size={14} />
        </ConfirmButton>
      </span>
    </li>
  );
}

function ProjectForm({ draft, onDone }: { draft: Draft; onDone: () => void }) {
  const [value, setValue] = useState(draft);
  // Tant que la couleur n'a pas été choisie à la main, un nouveau projet prend celle de son client.
  const [colorTouched, setColorTouched] = useState(Boolean(draft.id));
  const save = useSaveProject();
  // Archivés compris : un client peut n'avoir que des projets archivés.
  const { data: allProjects = [] } = useProjects(true);
  const clients = useMemo(() => knownClients(allProjects), [allProjects]);

  const submit = (e: FormEvent) => {
    e.preventDefault();
    save.mutate(
      { id: value.id, input: { name: value.name, client: value.client, color: value.color } },
      { onSuccess: onDone },
    );
  };

  return (
    <form className="card project-form" onSubmit={submit} onKeyDown={(e) => e.key === "Escape" && onDone()}>
      <div className="form-row">
        <label className="field">
          Nom
          <input
            className="input"
            autoFocus
            value={value.name}
            onChange={(e) => setValue({ ...value, name: e.target.value })}
            placeholder="Exactement comme dans le timesheet"
          />
        </label>
        <div className="field">
          <span>Client</span>
          <ClientCombobox
            value={value.client}
            clients={clients}
            onChange={(client) =>
              setValue((v) => ({
                ...v,
                client,
                color: colorTouched ? v.color : (clientColor(allProjects, client) ?? draft.color),
              }))
            }
          />
        </div>
      </div>
      <div className="swatches" role="radiogroup" aria-label="Couleur">
        {PALETTE.map((c) => (
          <button
            key={c}
            type="button"
            role="radio"
            aria-checked={value.color === c}
            aria-label={c}
            className={`swatch${value.color === c ? " selected" : ""}`}
            style={{ background: c }}
            onClick={() => {
              setColorTouched(true);
              setValue({ ...value, color: c });
            }}
          />
        ))}
        <input
          type="color"
          className="swatch-custom"
          value={value.color}
          onChange={(e) => {
            setColorTouched(true);
            setValue({ ...value, color: e.target.value });
          }}
          aria-label="Couleur personnalisée"
        />
      </div>
      {save.error && <p className="error">{errorMessage(save.error)}</p>}
      <div className="form-actions">
        <button type="button" className="btn btn-ghost" onClick={onDone}>
          Annuler
        </button>
        <button type="submit" className="btn btn-primary" disabled={save.isPending}>
          {value.id ? "Enregistrer" : "Créer le projet"}
        </button>
      </div>
    </form>
  );
}
