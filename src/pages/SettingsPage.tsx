import { open } from "@tauri-apps/plugin-dialog";
import { ShieldAlert, ShieldCheck } from "lucide-react";
import { useEffect, useState, type ReactNode } from "react";
import { commands, type ReminderSettings, type TrayTitle } from "../bindings";
import { ConfirmButton } from "../components/ConfirmButton";
import {
  useActivityStatus,
  useAutostart,
  useBackupInfo,
  useBackupNow,
  useSetActivityEnabled,
  useSetAutostart,
  useSetBackupDir,
  useSetReminderSettings,
  useSetShortcut,
  useSetTrayTitle,
  useSettings,
} from "../hooks/queries";
import { inTauri, installLabel, useAppVersion, useAvailableUpdate, useInstallUpdate } from "../hooks/updater";
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
      <ActivitySettings />
      <BackupSettings />
      <GeneralSettings />
      <AboutSettings />
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
  const time = (key: "activeFromMin" | "activeUntilMin" | "reviewAtMin") => (value: string) => {
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
          <Row label="Revue de fin de journée" hint="Une invitation à vérifier et compléter tes temps, une fois par jour.">
            <input
              type="time"
              className="input input-time"
              disabled={!draft.reviewEnabled}
              value={minutesToTime(draft.reviewAtMin)}
              onChange={(e) => time("reviewAtMin")(e.target.value)}
              aria-label="Heure de la revue de journée"
            />
            <input
              type="checkbox"
              className="switch"
              checked={draft.reviewEnabled}
              onChange={(e) => set({ reviewEnabled: e.target.checked })}
              aria-label="Activer la revue de fin de journée"
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

function ActivitySettings() {
  const { data: status } = useActivityStatus();
  const setEnabled = useSetActivityEnabled();
  const [cleared, setCleared] = useState(false);

  return (
    <section className="section">
      <div className="section-head">
        <h2>Suivi d'activité</h2>
      </div>
      <div className="card">
        <div className="settings-group">
          <Row
            label="Noter l'app et la fenêtre actives"
            hint="Toutes les 10 s, uniquement sur cet ordinateur, effacé après 30 jours. Sert à repérer le temps sans projet dans la revue de journée."
          >
            <input
              type="checkbox"
              className="switch"
              checked={status?.enabled ?? false}
              disabled={!status || setEnabled.isPending}
              onChange={(e) => setEnabled.mutate(e.target.checked)}
              aria-label="Activer le suivi d'activité"
            />
          </Row>
          {isMac && status && (
            <Row
              label="Titres des fenêtres"
              hint={
                status.titlesAllowed
                  ? "Autorisé : la revue montre le document ou l'onglet sur lequel tu travaillais."
                  : "macOS demande l'autorisation « Enregistrement de l'écran » pour lire les titres (Trackify ne capture jamais l'écran). Sans elle, seul le nom de l'app est noté."
              }
            >
              {status.titlesAllowed ? (
                <span className="permission ok">
                  <ShieldCheck size={15} /> Autorisé
                </span>
              ) : (
                <button type="button" className="btn" onClick={() => void commands.requestActivityPermission()}>
                  <ShieldAlert size={15} /> Autoriser…
                </button>
              )}
            </Row>
          )}
          <Row label="Historique d'activité" hint={cleared ? "Effacé." : "Supprime tout ce qui a été noté jusqu'ici (tes entrées de temps ne bougent pas)."}>
            <ConfirmButton
              className="btn"
              label="Effacer l'historique d'activité"
              confirmLabel="Effacer ?"
              onConfirm={() => void commands.clearActivity().then(() => setCleared(true))}
            >
              Effacer
            </ConfirmButton>
          </Row>
        </div>
      </div>
    </section>
  );
}

function BackupSettings() {
  const { data: info } = useBackupInfo();
  const backupNow = useBackupNow();
  const setDir = useSetBackupDir();
  const error = backupNow.error ?? setDir.error;

  const choose = async () => {
    const dir = await open({ directory: true, title: "Dossier des sauvegardes", defaultPath: info?.dir });
    if (typeof dir === "string") setDir.mutate(dir);
  };

  return (
    <section className="section">
      <div className="section-head">
        <h2>Sauvegardes</h2>
      </div>
      <div className="card">
        <div className="settings-group">
          <Row
            label="Copie quotidienne de la base"
            hint={
              <>
                Les 7 dernières sont gardées. Choisis un dossier iCloud Drive ou Dropbox pour les mettre à l'abri d'une
                panne de disque.
                <span className="backup-path" title={info?.dir}>
                  {info?.dir}
                </span>
              </>
            }
          >
            <button type="button" className="btn" onClick={() => void choose()}>
              Choisir…
            </button>
            {info?.custom && (
              <button type="button" className="btn btn-ghost" onClick={() => setDir.mutate(null)}>
                Par défaut
              </button>
            )}
          </Row>
          <Row
            label="Dernière sauvegarde"
            hint={error ? <span className="error">{errorMessage(error)}</span> : (info?.latest ?? "Aucune pour l'instant.")}
          >
            <button type="button" className="btn btn-ghost" onClick={() => void commands.revealBackups()}>
              Ouvrir le dossier
            </button>
            <button type="button" className="btn" disabled={backupNow.isPending} onClick={() => backupNow.mutate()}>
              Sauvegarder maintenant
            </button>
          </Row>
        </div>
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
          {isMac && <TrayTitleRow />}
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

function AboutSettings() {
  const { data: version } = useAppVersion();
  const update = useAvailableUpdate();
  const install = useInstallUpdate();

  let status: ReactNode = "Recherche automatique au lancement, puis toutes les 6 h.";
  if (!inTauri) status = "Indisponible hors de l'app installée.";
  else if (update.isFetching) status = "Recherche en cours…";
  else if (update.error) status = <span className="error">Vérification impossible : {errorMessage(update.error)}</span>;
  else if (update.data) status = `La version ${update.data.version} est disponible.`;
  else if (update.isFetched) status = "Trackify est à jour.";

  return (
    <section className="section">
      <div className="section-head">
        <h2>À propos</h2>
      </div>
      <div className="card">
        <div className="settings-group">
          <Row label="Trackify" hint={`Version ${version ?? "—"} · © 2026 Kani Bouébassihou`}>
            {null}
          </Row>
          <Row
            label="Mises à jour"
            hint={install.error ? <span className="error">{errorMessage(install.error)}</span> : status}
          >
            {update.data ? (
              <button
                type="button"
                className="btn btn-primary"
                disabled={install.isPending}
                onClick={() => update.data && install.mutate(update.data)}
              >
                {installLabel(install.isPending, install.progress)}
              </button>
            ) : (
              <button
                type="button"
                className="btn"
                disabled={!inTauri || update.isFetching}
                onClick={() => void update.refetch()}
              >
                Rechercher
              </button>
            )}
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

function TrayTitleRow() {
  const { data: settings } = useSettings();
  const setTrayTitle = useSetTrayTitle();

  return (
    <Row
      label="Barre des menus pendant un timer"
      hint={
        setTrayTitle.error ? (
          <span className="error">{errorMessage(setTrayTitle.error)}</span>
        ) : (
          "Si l'icône n'apparaît pas, la barre est trop pleine (macOS masque ce qui passe sous l'encoche) : choisis un affichage plus court."
        )
      }
    >
      <select
        className="input"
        value={settings?.trayTitle ?? "projectAndDuration"}
        disabled={!settings || setTrayTitle.isPending}
        onChange={(e) => setTrayTitle.mutate(e.target.value as TrayTitle)}
        aria-label="Barre des menus pendant un timer"
      >
        <option value="projectAndDuration">Projet et durée</option>
        <option value="duration">Durée seule</option>
        <option value="hidden">Icône seule</option>
      </select>
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
