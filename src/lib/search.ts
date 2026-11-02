/** Minuscules sans accents, pour une recherche tolérante (« equipe » trouve « Équipe »). */
export function normalize(s: string): string {
  return s.normalize("NFD").replace(/\p{Diacritic}/gu, "").toLowerCase();
}

/** Vrai si chaque mot de la recherche apparaît dans l'un des mots-clés. */
export function matchesSearch(search: string, keywords: string[]): boolean {
  const haystack = normalize(keywords.join(" "));
  return normalize(search)
    .split(/\s+/)
    .filter(Boolean)
    .every((word) => haystack.includes(word));
}
