import { useEffect, useState, type ReactNode } from "react";

/** Bouton destructif : un premier clic demande confirmation, un second exécute. */
export function ConfirmButton({
  onConfirm,
  children,
  label,
  confirmLabel = "Confirmer ?",
  className = "icon-btn danger",
}: {
  onConfirm: () => void;
  children: ReactNode;
  /** Libellé accessible (le contenu est souvent une simple icône). */
  label: string;
  confirmLabel?: string;
  className?: string;
}) {
  const [armed, setArmed] = useState(false);
  useEffect(() => {
    if (!armed) return;
    const id = window.setTimeout(() => setArmed(false), 3000);
    return () => window.clearTimeout(id);
  }, [armed]);

  return (
    <button
      type="button"
      className={armed ? `${className} confirm-armed` : className}
      title={label}
      aria-label={armed ? `${label} : cliquer pour confirmer` : label}
      onClick={() => {
        if (armed) {
          setArmed(false);
          onConfirm();
        } else {
          setArmed(true);
        }
      }}
    >
      {armed ? confirmLabel : children}
    </button>
  );
}
