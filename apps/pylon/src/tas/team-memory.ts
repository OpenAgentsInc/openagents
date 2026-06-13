export type MemoryEntry = {
  readonly ref: string
  readonly scope: "team" | "private"
  readonly authorRef: string
  readonly createdAt: number
  readonly digestRef: string
}

export type MemoryViewer = {
  readonly ref: string
  readonly isMember: boolean
}

export function upsert(
  entries: readonly MemoryEntry[],
  entry: MemoryEntry,
): readonly MemoryEntry[] {
  const existingIndex = entries.findIndex((candidate) => candidate.ref === entry.ref)

  if (existingIndex === -1) {
    return [...entries, entry]
  }

  const existing = entries[existingIndex]

  if (existing === undefined || entry.createdAt < existing.createdAt) {
    return entries
  }

  return entries.map((candidate, index) =>
    index === existingIndex ? entry : candidate,
  )
}

export function visibleTo(
  entries: readonly MemoryEntry[],
  viewer: MemoryViewer,
): readonly string[] {
  return entries
    .filter((entry) => {
      if (entry.scope === "team") {
        return viewer.isMember
      }

      return entry.authorRef === viewer.ref
    })
    .map(({ ref }) => ref)
}
