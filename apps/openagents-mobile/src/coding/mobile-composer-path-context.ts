export const MAX_MOBILE_COMPOSER_PATH_RESULTS = 20
export const MAX_MOBILE_COMPOSER_PATH_QUERY = 128

export type MobileComposerPathEntry = Readonly<{
  pathRef: string
  kind: "file" | "directory"
  revisionRef: string
}>

export type MobileComposerPathSearchRequest = Readonly<{
  repositoryRef: string
  worktreeRef: string
  query: string
  limit: number
}>

export type MobileComposerPathSearchPage = Readonly<{
  repositoryRef: string
  worktreeRef: string
  query: string
  entries: ReadonlyArray<MobileComposerPathEntry>
}>

export type MobileComposerPathSearchPort = Readonly<{
  search: (request: MobileComposerPathSearchRequest) => Promise<unknown>
}>

const safeRef = (value: unknown): value is string => typeof value === "string" &&
  value.length > 0 && value.length <= 256 && /^[A-Za-z0-9][A-Za-z0-9._:-]*$/u.test(value)

export const safeMobileComposerPathRef = (value: unknown): value is string => {
  if (typeof value !== "string" || value.length === 0 || value.length > 512 ||
    value.startsWith("/") || value.includes("\\") || value.includes("\0")) return false
  const segments = value.split("/")
  return segments.every(segment => segment.length > 0 && segment !== "." && segment !== "..")
}

export const normalizeMobileComposerPathQuery = (query: string): string =>
  query.replace(/^@/u, "").trim().slice(0, MAX_MOBILE_COMPOSER_PATH_QUERY)

export const decodeMobileComposerPathSearchPage = (
  value: unknown,
  request: MobileComposerPathSearchRequest,
): MobileComposerPathSearchPage | null => {
  if (typeof value !== "object" || value === null) return null
  const page = value as Record<string, unknown>
  if (page.repositoryRef !== request.repositoryRef || page.worktreeRef !== request.worktreeRef ||
    page.query !== request.query || !Array.isArray(page.entries) ||
    page.entries.length > Math.min(request.limit, MAX_MOBILE_COMPOSER_PATH_RESULTS)) return null
  const entries: MobileComposerPathEntry[] = []
  const seen = new Set<string>()
  for (const candidate of page.entries) {
    if (typeof candidate !== "object" || candidate === null) return null
    const entry = candidate as Record<string, unknown>
    if (!safeMobileComposerPathRef(entry.pathRef) ||
      (entry.kind !== "file" && entry.kind !== "directory") || !safeRef(entry.revisionRef) ||
      seen.has(entry.pathRef)) return null
    seen.add(entry.pathRef)
    entries.push({ pathRef: entry.pathRef, kind: entry.kind, revisionRef: entry.revisionRef })
  }
  return {
    repositoryRef: request.repositoryRef,
    worktreeRef: request.worktreeRef,
    query: request.query,
    entries,
  }
}

export const searchMobileComposerPaths = async (
  port: MobileComposerPathSearchPort,
  input: Omit<MobileComposerPathSearchRequest, "query" | "limit"> & Readonly<{ query: string }>,
): Promise<Readonly<
  | { state: "ready"; page: MobileComposerPathSearchPage }
  | { state: "failed"; message: string }
>> => {
  const query = normalizeMobileComposerPathQuery(input.query)
  if (query === "") return { state: "ready", page: { ...input, query, entries: [] } }
  const request: MobileComposerPathSearchRequest = {
    repositoryRef: input.repositoryRef,
    worktreeRef: input.worktreeRef,
    query,
    limit: MAX_MOBILE_COMPOSER_PATH_RESULTS,
  }
  try {
    const page = decodeMobileComposerPathSearchPage(await port.search(request), request)
    return page === null
      ? { state: "failed", message: "The environment returned an invalid file-search result." }
      : { state: "ready", page }
  } catch {
    return { state: "failed", message: "Repository file search is unavailable right now." }
  }
}
