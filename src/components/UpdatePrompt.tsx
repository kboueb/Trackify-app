import { getCurrentWindow } from "@tauri-apps/api/window";
import type { Update } from "@tauri-apps/plugin-updater";
import { Download } from "lucide-react";
import { useEffect, useState } from "react";
import { installLabel, useAvailableUpdate, useInstallUpdate } from "../hooks/updater";
import { errorMessage } from "../lib/errors";

/** Version pour laquelle « Plus tard » a été choisi : on ne la repropose pas (elle reste dans Réglages). */
const DISMISSED_KEY = "trackify.update.dismissed";

export function UpdatePrompt() {
  const { data: update } = useAvailableUpdate();
  const [dismissed, setDismissed] = useState(() => localStorage.getItem(DISMISSED_KEY));
  const visible = !!update && update.version !== dismissed;

  // Trackify vit surtout dans le tray : on affiche la fenêtre, une fois par version, pour que le popup se voie.
  useEffect(() => {
    if (!visible) return;
    const w = getCurrentWindow();
    void w.show().then(() => w.unminimize()).then(() => w.setFocus());
  }, [visible]);

  if (!update || !visible) return null;
  const later = () => {
    localStorage.setItem(DISMISSED_KEY, update.version);
    setDismissed(update.version);
  };
  return <UpdateDialog update={update} onLater={later} />;
}

function UpdateDialog({ update, onLater }: { update: Update; onLater: () => void }) {
  const install = useInstallUpdate();

  return (
    <div className="modal-backdrop">
      <div className="modal card" role="alertdialog" aria-labelledby="update-title" aria-modal="true">
        <span className="modal-icon" aria-hidden>
          <Download size={18} />
        </span>
        <h2 id="update-title">Trackify {update.version} est disponible</h2>
        <p>
          Tu as la version {update.currentVersion}. L'installation prend quelques secondes, puis Trackify redémarre ; le
          timer en cours continue.
        </p>
        {update.body && <p className="muted small update-notes">{update.body}</p>}

        <div className="idle-actions">
          <button type="button" className="btn" disabled={install.isPending} onClick={onLater}>
            Plus tard
          </button>
          <button
            type="button"
            className="btn btn-primary"
            disabled={install.isPending}
            onClick={() => install.mutate(update)}
          >
            {installLabel(install.isPending, install.progress)}
          </button>
        </div>

        {install.error && <p className="error">{errorMessage(install.error)}</p>}
      </div>
    </div>
  );
}
