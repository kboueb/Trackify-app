import { format, parseISO } from "date-fns";
import { fr } from "date-fns/locale";
import type { BackupFile } from "../bindings";

/** « lundi 9 mars 2026 · 84 Ko », pour choisir une sauvegarde à restaurer. */
export function backupLabel(file: BackupFile): string {
  const day = parseISO(file.day);
  const date = Number.isNaN(day.getTime()) ? file.name : format(day, "EEEE d MMMM yyyy", { locale: fr });
  return `${date} · ${formatSize(file.sizeKb)}`;
}

export function formatSize(kb: number): string {
  if (kb < 1024) return `${Math.max(kb, 1)} Ko`;
  return `${(kb / 1024).toFixed(1).replace(".", ",")} Mo`;
}
