/** Message affichable pour une erreur venant d'une commande Rust ou du runtime Tauri. */
export function errorMessage(error: unknown): string {
  if (typeof error === "string") return error;
  if (error && typeof error === "object" && "message" in error) return String(error.message);
  return "Une erreur inattendue est survenue.";
}
