import { Check, ChevronDown, Plus } from "lucide-react";
import { useId, useMemo, useRef, useState } from "react";
import { clientOptions, findClient, type ClientInfo, type ClientOption } from "../lib/clients";

/**
 * Champ « Client » : liste des clients existants, filtrée à la frappe, avec
 * « Ajouter « … » » quand le client n'existe pas encore. La saisie libre reste
 * possible ; en quittant le champ, on reprend l'orthographe d'un client existant
 * (« neemba » → « Neemba ») pour ne pas créer de doublon.
 */
export function ClientCombobox({
  value,
  onChange,
  clients,
}: {
  value: string;
  onChange: (client: string) => void;
  clients: ClientInfo[];
}) {
  const id = useId();
  const listId = `${id}-list`;
  const inputRef = useRef<HTMLInputElement>(null);
  const [open, setOpen] = useState(false);
  const [active, setActive] = useState(0);
  const options = useMemo(() => clientOptions(clients, value), [clients, value]);
  const current = Math.min(active, options.length - 1);

  const choose = (option: ClientOption) => {
    onChange(option.kind === "none" ? "" : option.name);
    setOpen(false);
  };

  const openList = () => {
    setActive(0);
    setOpen(true);
  };

  return (
    <div className="combobox">
      <input
        ref={inputRef}
        className="input"
        role="combobox"
        aria-expanded={open}
        aria-controls={listId}
        aria-autocomplete="list"
        aria-activedescendant={open && options.length ? `${id}-opt-${current}` : undefined}
        placeholder="Choisir ou ajouter un client"
        value={value}
        onFocus={openList}
        onClick={openList}
        onChange={(e) => {
          onChange(e.target.value);
          setActive(0);
          setOpen(true);
        }}
        onBlur={() => {
          setOpen(false);
          const existing = findClient(clients, value);
          if (existing && existing.name !== value) onChange(existing.name);
          else if (value !== value.trim()) onChange(value.trim());
        }}
        onKeyDown={(e) => {
          if (e.key === "ArrowDown" || e.key === "ArrowUp") {
            e.preventDefault();
            if (!open) return openList();
            const delta = e.key === "ArrowDown" ? 1 : -1;
            setActive((current + delta + options.length) % options.length);
          } else if (e.key === "Enter" && open && options[current]) {
            e.preventDefault(); // ne pas soumettre le formulaire
            choose(options[current]);
          } else if (e.key === "Escape" && open) {
            e.stopPropagation(); // ferme la liste sans annuler le formulaire
            setOpen(false);
          }
        }}
      />
      <button
        type="button"
        className="combobox-toggle"
        tabIndex={-1}
        aria-label="Afficher les clients"
        onMouseDown={(e) => e.preventDefault()}
        onClick={() => {
          if (open) setOpen(false);
          else inputRef.current?.focus();
        }}
      >
        <ChevronDown size={14} />
      </button>

      {open && options.length > 0 && (
        <ul className="combobox-list" id={listId} role="listbox" aria-label="Clients">
          {options.map((option, i) => {
            const selected = option.kind === "none" ? value.trim() === "" : option.name === value;
            return (
              <li
                key={option.kind === "none" ? "__none" : `${option.kind}:${option.name}`}
                id={`${id}-opt-${i}`}
                role="option"
                aria-selected={selected}
                className={`combobox-option${i === current ? " active" : ""}${option.kind === "new" ? " new" : ""}`}
                // mousedown plutôt que click : l'option est choisie avant que le champ perde le focus
                onMouseDown={(e) => {
                  e.preventDefault();
                  choose(option);
                }}
                onMouseEnter={() => setActive(i)}
              >
                {option.kind === "new" ? (
                  <>
                    <Plus size={14} />
                    <span>
                      Ajouter « <strong>{option.name}</strong> »
                    </span>
                  </>
                ) : option.kind === "none" ? (
                  <span className="subtle">Sans client</span>
                ) : (
                  <>
                    <span className="combobox-name">{option.name}</span>
                    <span className="subtle small">
                      {option.projects} projet{option.projects > 1 ? "s" : ""}
                    </span>
                  </>
                )}
                {selected && <Check size={14} className="combobox-check" />}
              </li>
            );
          })}
        </ul>
      )}
    </div>
  );
}
