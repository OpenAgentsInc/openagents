export type KhalaCodeDesktopCodexThreadSummary = {
  readonly id: string
  readonly sessionId: string | null
  readonly title: string
  readonly preview: string
  readonly cwd: string | null
  readonly projectLabel: string
  readonly status: string
  readonly statusLabel: string
  readonly modelProvider: string | null
  readonly source: string
  readonly forkedFromId: string | null
  readonly parentThreadId: string | null
  readonly createdAt: number | null
  readonly updatedAt: number | null
  readonly recencyAt: number | null
  readonly badges: readonly string[]
}

export type KhalaCodeDesktopCodexThreadGroup = {
  readonly key: string
  readonly label: string
  readonly threadIds: readonly string[]
}

export type KhalaCodeDesktopCodexThreadListProjection = {
  readonly activeThreadId: string | null
  readonly archived: boolean
  readonly searchTerm: string | null
  readonly threads: readonly KhalaCodeDesktopCodexThreadSummary[]
  readonly groups: readonly KhalaCodeDesktopCodexThreadGroup[]
}

const asRecord = (value: unknown): Record<string, unknown> =>
  value !== null && typeof value === "object" && !Array.isArray(value)
    ? value as Record<string, unknown>
    : {}

const optionalString = (value: unknown): string | null =>
  typeof value === "string" && value.length > 0 ? value : null

const optionalNumber = (value: unknown): number | null =>
  typeof value === "number" && Number.isFinite(value) ? value : null

const dataArray = (value: unknown): readonly unknown[] =>
  Array.isArray(asRecord(value).data) ? asRecord(value).data as readonly unknown[] : []

const sourceLabel = (source: unknown): string => {
  if (typeof source === "string") return source
  const record = asRecord(source)
  if (typeof record.custom === "string") return record.custom
  if (record.subAgent !== undefined) return "subAgent"
  return "unknown"
}

const statusKind = (status: unknown): string => {
  if (typeof status === "string") return status
  const record = asRecord(status)
  return optionalString(record.type) ?? "unknown"
}

const projectLabel = (cwd: string | null): string => {
  if (cwd === null) return "No working directory"
  const parts = cwd.split(/[\\/]+/).filter(Boolean)
  return parts.at(-1) ?? cwd
}

const titleFor = (thread: Record<string, unknown>, id: string): string =>
  optionalString(thread.name) ??
  optionalString(thread.preview)?.split(/\r?\n/u)[0]?.slice(0, 80) ??
  id

const badgesFor = (
  thread: Record<string, unknown>,
  status: string,
): readonly string[] => {
  const badges = new Set<string>()
  if (status === "active") badges.add("running")
  if (status === "systemError") badges.add("failed")
  if (optionalString(thread.forkedFromId) !== null) badges.add("fork")
  if (optionalString(thread.parentThreadId) !== null) badges.add("child")
  const gitInfo = asRecord(thread.gitInfo)
  if (optionalString(gitInfo.branch) !== null) badges.add("git")
  if (Array.isArray(thread.turns) && thread.turns.some(turn => statusKind(asRecord(turn).status) === "failed")) {
    badges.add("failed")
  }
  return [...badges].sort()
}

export const projectKhalaCodeDesktopCodexThread = (
  value: unknown,
): KhalaCodeDesktopCodexThreadSummary | null => {
  const thread = asRecord(value)
  const id = optionalString(thread.id)
  if (id === null) return null
  const status = statusKind(thread.status)
  const cwd = optionalString(thread.cwd)
  const preview = optionalString(thread.preview) ?? ""
  return {
    id,
    sessionId: optionalString(thread.sessionId),
    title: titleFor(thread, id),
    preview,
    cwd,
    projectLabel: projectLabel(cwd),
    status,
    statusLabel: status.replace(/([a-z])([A-Z])/g, "$1 $2").toLowerCase(),
    modelProvider: optionalString(thread.modelProvider),
    source: sourceLabel(thread.source),
    forkedFromId: optionalString(thread.forkedFromId),
    parentThreadId: optionalString(thread.parentThreadId),
    createdAt: optionalNumber(thread.createdAt),
    updatedAt: optionalNumber(thread.updatedAt),
    recencyAt: optionalNumber(thread.recencyAt),
    badges: badgesFor(thread, status),
  }
}

export const projectKhalaCodeDesktopCodexThreadList = (
  input: {
    readonly activeThreadId?: string | null
    readonly archived?: boolean
    readonly response: unknown
    readonly searchTerm?: string | null
  },
): KhalaCodeDesktopCodexThreadListProjection => {
  const threads = dataArray(input.response)
    .map(projectKhalaCodeDesktopCodexThread)
    .filter((thread): thread is KhalaCodeDesktopCodexThreadSummary => thread !== null)
  const groupMap = new Map<string, { label: string; threadIds: string[] }>()
  for (const thread of threads) {
    const key = thread.cwd ?? "cwd:none"
    const existing = groupMap.get(key)
    if (existing === undefined) {
      groupMap.set(key, {
        label: thread.projectLabel,
        threadIds: [thread.id],
      })
    } else {
      existing.threadIds.push(thread.id)
    }
  }
  return {
    activeThreadId: input.activeThreadId ?? null,
    archived: input.archived === true,
    searchTerm: input.searchTerm ?? null,
    threads,
    groups: [...groupMap.entries()].map(([key, group]) => ({
      key,
      label: group.label,
      threadIds: group.threadIds,
    })),
  }
}
