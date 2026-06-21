import { existsSync, readFileSync, writeFileSync } from "node:fs"
import { join } from "node:path"
import {
  DEFAULT_OPENAGENTS_BASE_URL,
  type ReadFile,
  type WriteFile,
} from "./agent-onboarding.js"

// AF-4 (#5901): automated read-only work-search (discovery only).
//
// The onboarding chain ends at the Tassadar assignment worker; it never looks at
// the Forum labor market. This module adds a bounded, observable *discovery* step
// over the `work-requests` lane — the HTTP face of the NIP-90 agentic-coding
// labor market (apps/openagents.com/workers/api/src/forum-work-requests.ts,
// `ForumWorkRequestsForumSlug = 'work-requests'`). It surfaces a count of open
// work items for the wizard.
//
// Typed routing (workspace rule): there is NO keyword classification here. The
// `work-requests` lane is structurally typed — every entry is a work request by
// construction, with a typed `state` (open → quote_received → … → delivered). We
// read the lane and count items in the typed `open` state. No fuzzy text routing.
//
// Strict boundary: this is READ-ONLY discovery. It NEVER bids, quotes, accepts,
// commits to work, or spends — committing stays inside the existing owner-gated
// Tassadar claim path. The endpoint read is public (no token, no secrets).

const WORK_SEARCH_FILENAME = "forum-work-search.json"
const DEFAULT_USER_AGENT = "autopilot-desktop"
// Bounded read size; the lane endpoint clamps to 100 server-side anyway.
const DEFAULT_LIMIT = 50

const defaultReadFile: ReadFile = (path: string) => {
  try {
    if (!existsSync(path)) return null
    return readFileSync(path, "utf8")
  } catch {
    return null
  }
}

const defaultWriteFile: WriteFile = (path: string, content: string) => {
  writeFileSync(path, content, { mode: 0o600 })
}

export type PersistedWorkSearchReceipt = {
  // Count of work items in the typed `open` state at the last successful read.
  readonly openCount: number
  // Total work items returned by the open-lane read (open + active states).
  readonly totalCount: number
  readonly firstSearchedAt: string
  readonly lastSearchedAt: string
}

/**
 * Load the persisted work-search receipt from the managed home. Returns null
 * until the first successful search. Never throws.
 */
export const loadWorkSearchReceipt = (
  home: string,
  readFile: ReadFile = defaultReadFile,
): PersistedWorkSearchReceipt | null => {
  const raw = readFile(join(home, WORK_SEARCH_FILENAME))
  if (raw === null) return null
  let parsed: unknown
  try {
    parsed = JSON.parse(raw)
  } catch {
    return null
  }
  if (typeof parsed !== "object" || parsed === null) return null
  const r = parsed as Record<string, unknown>
  if (typeof r.lastSearchedAt !== "string" || r.lastSearchedAt.length === 0) {
    return null
  }
  return {
    openCount: typeof r.openCount === "number" ? r.openCount : 0,
    totalCount: typeof r.totalCount === "number" ? r.totalCount : 0,
    firstSearchedAt:
      typeof r.firstSearchedAt === "string" && r.firstSearchedAt.length > 0
        ? r.firstSearchedAt
        : r.lastSearchedAt,
    lastSearchedAt: r.lastSearchedAt,
  }
}

/**
 * Observable for the wizard: has a work-search completed at least once?
 */
export const hasSearchedForumWork = (
  home: string,
  readFile: ReadFile = defaultReadFile,
): boolean => loadWorkSearchReceipt(home, readFile) !== null

export type WorkSearchFetch = (
  url: string,
  init: {
    readonly method: string
    readonly headers: Record<string, string>
  },
) => Promise<{
  readonly status: number
  json(): Promise<unknown>
}>

export type SearchForumWorkOptions = {
  readonly home: string
  readonly baseUrl?: string
  readonly limit?: number
  readonly userAgent?: string
  readonly fetchImpl?: WorkSearchFetch
  readonly readFile?: ReadFile
  readonly writeFile?: WriteFile
  readonly log?: (message: string) => void
}

export type SearchForumWorkResult =
  | {
      readonly outcome: "searched"
      readonly openCount: number
      readonly totalCount: number
    }
  | { readonly outcome: "deferred"; readonly reason: string }

const endpoint = (baseUrl: string, path: string): string =>
  new URL(path, baseUrl.endsWith("/") ? baseUrl : `${baseUrl}/`).toString()

/**
 * AF-4: read-only discovery over the typed `work-requests` lane. Counts open
 * work items and persists/refreshes a receipt for the wizard. Offline-tolerant;
 * never bids, quotes, accepts, or spends.
 */
export const searchForumWork = async (
  options: SearchForumWorkOptions,
): Promise<SearchForumWorkResult> => {
  const baseUrl = options.baseUrl ?? DEFAULT_OPENAGENTS_BASE_URL
  const fetchImpl =
    options.fetchImpl ?? (globalThis.fetch as unknown as WorkSearchFetch)
  const readFile = options.readFile ?? defaultReadFile
  const writeFile = options.writeFile ?? defaultWriteFile
  const userAgent = options.userAgent ?? DEFAULT_USER_AGENT
  const log = options.log ?? (() => {})
  const limit = Math.max(1, Math.min(100, options.limit ?? DEFAULT_LIMIT))

  let response: { readonly status: number; json(): Promise<unknown> }
  try {
    response = await fetchImpl(
      endpoint(baseUrl, `/api/forum/work-requests?limit=${limit}`),
      { method: "GET", headers: { "user-agent": userAgent } },
    )
  } catch (error) {
    const reason = error instanceof Error ? error.message : String(error)
    log(`[forum-work] work-search deferred (offline): ${reason}`)
    return { outcome: "deferred", reason }
  }
  if (response.status !== 200) {
    log(`[forum-work] work-search deferred: status ${response.status}`)
    return { outcome: "deferred", reason: `status_${response.status}` }
  }

  let workRequests: ReadonlyArray<{ readonly state?: unknown }> = []
  try {
    const parsed = (await response.json()) as {
      readonly workRequests?: unknown
    }
    workRequests = Array.isArray(parsed.workRequests)
      ? (parsed.workRequests as ReadonlyArray<{ readonly state?: unknown }>)
      : []
  } catch {
    return { outcome: "deferred", reason: "malformed_response" }
  }

  const totalCount = workRequests.length
  // Typed state filter — not keyword matching.
  const openCount = workRequests.filter(w => w.state === "open").length

  const nowIso = new Date().toISOString()
  const prior = loadWorkSearchReceipt(options.home, readFile)
  const receipt: PersistedWorkSearchReceipt = {
    openCount,
    totalCount,
    firstSearchedAt: prior?.firstSearchedAt ?? nowIso,
    lastSearchedAt: nowIso,
  }
  writeFile(
    join(options.home, WORK_SEARCH_FILENAME),
    `${JSON.stringify(receipt, null, 2)}\n`,
  )
  log(`[forum-work] work-search complete: ${openCount} open work item(s)`)
  return { outcome: "searched", openCount, totalCount }
}
