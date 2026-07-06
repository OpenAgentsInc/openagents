/**
 * MM-B2 (#8472): pure client-side filter for the repo picker's search box —
 * the mobile-bearer repo API (#8471) has no server-side `q=` search param,
 * so filtering happens locally over whatever pages have been fetched so far.
 */
import type { KhalaMobileRepository } from "./khala-mobile-repos-api"

export const filterKhalaMobileRepositories = (
  repositories: ReadonlyArray<KhalaMobileRepository>,
  searchTerm: string,
): ReadonlyArray<KhalaMobileRepository> => {
  const term = searchTerm.trim().toLowerCase()
  if (term.length === 0) return repositories
  return repositories.filter(
    repo =>
      repo.fullName.toLowerCase().includes(term) ||
      repo.name.toLowerCase().includes(term) ||
      repo.owner.toLowerCase().includes(term) ||
      (repo.description?.toLowerCase().includes(term) ?? false),
  )
}

/** Recent-first, then alphabetical by fullName — GitHub's own list ordering
 * (affiliation-mixed, not recency-sorted) isn't guaranteed stable across
 * pages, so this gives the picker a deterministic, readable order. */
export const sortKhalaMobileRepositoriesForPicker = (
  repositories: ReadonlyArray<KhalaMobileRepository>,
): ReadonlyArray<KhalaMobileRepository> =>
  [...repositories].sort((a, b) => a.fullName.localeCompare(b.fullName))

/** De-dupes by `id` — paging can overlap if the upstream list shifts between
 * page fetches (a repo created/starred mid-scroll). */
export const dedupeKhalaMobileRepositoriesById = (
  repositories: ReadonlyArray<KhalaMobileRepository>,
): ReadonlyArray<KhalaMobileRepository> => {
  const seen = new Set<string>()
  const result: Array<KhalaMobileRepository> = []
  for (const repo of repositories) {
    if (seen.has(repo.id)) continue
    seen.add(repo.id)
    result.push(repo)
  }
  return result
}
