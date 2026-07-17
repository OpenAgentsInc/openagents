import { safeMobileComposerPathRef } from "./mobile-composer-path-context"

export const MOBILE_REPOSITORY_TREE_LIMIT = 100
export const MOBILE_REPOSITORY_MAX_DEPTH = 12
export const MOBILE_REPOSITORY_TEXT_MAX_BYTES = 512 * 1024
export const MOBILE_REPOSITORY_IMAGE_MAX_BYTES = 12 * 1024 * 1024

export type MobileRepositoryScope = Readonly<{
  sessionRef: string
  repositoryRef: string
  worktreeRef: string
}>

export type MobileRepositoryTreeEntry = Readonly<{
  name: string
  pathRef: string
  kind: "file" | "directory"
  expandable: boolean
  sizeBytes: number | null
  revisionRef: string
}>

export type MobileRepositoryTreePage = MobileRepositoryScope & Readonly<{
  directoryRef: string
  entries: ReadonlyArray<MobileRepositoryTreeEntry>
  nextCursor: string | null
  revisionRef: string
}>

export type MobileRepositoryPreview = MobileRepositoryScope & Readonly<{
  pathRef: string
  revisionRef: string
  sizeBytes: number
}> & (
  | Readonly<{ kind: "source"; language: string; content: string }>
  | Readonly<{ kind: "markdown"; content: string }>
  | Readonly<{ kind: "image"; mediaType: "image/png" | "image/jpeg" | "image/gif" | "image/webp"; contentUrl: string; sha256: string }>
)

export type MobileRepositoryFilesPort = Readonly<{
  tree: (request: MobileRepositoryScope & Readonly<{
    directoryRef: string
    cursor: string | null
    limit: number
  }>) => Promise<unknown>
  read: (request: MobileRepositoryScope & Readonly<{
    pathRef: string
    expectedRevisionRef: string
  }>) => Promise<unknown>
}>

export type MobileRepositoryPreviewState =
  | Readonly<{ state: "idle" }>
  | Readonly<{ state: "loading"; pathRef: string; revisionRef: string }>
  | Readonly<{ state: "ready"; preview: MobileRepositoryPreview }>
  | Readonly<{ state: "failed"; pathRef: string; message: string }>

export type MobileRepositoryBrowserState = Readonly<{
  scope: MobileRepositoryScope | null
  state: "idle" | "loading" | "ready" | "unavailable" | "failed"
  pages: Readonly<Record<string, MobileRepositoryTreePage>>
  expandedRefs: ReadonlyArray<string>
  preview: MobileRepositoryPreviewState
  requestEpoch: number
  message: string | null
}>

export const initialMobileRepositoryBrowserState: MobileRepositoryBrowserState = {
  scope: null,
  state: "idle",
  pages: {},
  expandedRefs: [],
  preview: { state: "idle" },
  requestEpoch: 0,
  message: null,
}

const safeIdentity = (value: unknown): value is string => typeof value === "string" &&
  value.length > 0 && value.length <= 256 && /^[A-Za-z0-9][A-Za-z0-9._:-]*$/u.test(value)

const safeDirectoryRef = (value: unknown): value is string => value === "" || safeMobileComposerPathRef(value)

const exactScope = (value: Record<string, unknown>, scope: MobileRepositoryScope): boolean =>
  value.sessionRef === scope.sessionRef && value.repositoryRef === scope.repositoryRef &&
  value.worktreeRef === scope.worktreeRef

const directChild = (directoryRef: string, pathRef: string): boolean => {
  const parent = pathRef.includes("/") ? pathRef.slice(0, pathRef.lastIndexOf("/")) : ""
  return parent === directoryRef
}

export const decodeMobileRepositoryTreePage = (
  value: unknown,
  request: MobileRepositoryScope & Readonly<{ directoryRef: string; cursor: string | null; limit: number }>,
): MobileRepositoryTreePage | null => {
  if (typeof value !== "object" || value === null) return null
  const page = value as Record<string, unknown>
  if (!exactScope(page, request) || page.directoryRef !== request.directoryRef ||
    !safeIdentity(page.revisionRef) || !Array.isArray(page.entries) ||
    page.entries.length > Math.min(MOBILE_REPOSITORY_TREE_LIMIT, request.limit) ||
    !(page.nextCursor === null || safeIdentity(page.nextCursor))) return null
  const entries: MobileRepositoryTreeEntry[] = []
  const seen = new Set<string>()
  for (const candidate of page.entries) {
    if (typeof candidate !== "object" || candidate === null) return null
    const entry = candidate as Record<string, unknown>
    if (typeof entry.name !== "string" || entry.name.length === 0 || entry.name.length > 256 ||
      !safeMobileComposerPathRef(entry.pathRef) || !directChild(request.directoryRef, entry.pathRef) ||
      (entry.kind !== "file" && entry.kind !== "directory") ||
      entry.expandable !== (entry.kind === "directory") ||
      !(entry.sizeBytes === null || (typeof entry.sizeBytes === "number" && Number.isSafeInteger(entry.sizeBytes) && entry.sizeBytes >= 0)) ||
      !safeIdentity(entry.revisionRef) || seen.has(entry.pathRef)) return null
    seen.add(entry.pathRef)
    entries.push({
      name: entry.name,
      pathRef: entry.pathRef,
      kind: entry.kind,
      expandable: entry.expandable,
      sizeBytes: entry.sizeBytes,
      revisionRef: entry.revisionRef,
    })
  }
  return {
    ...request,
    entries,
    nextCursor: page.nextCursor as string | null,
    revisionRef: page.revisionRef as string,
  }
}

const safeHttpsUrl = (value: unknown): value is string => {
  if (typeof value !== "string" || value.length > 2_048) return false
  try { return new URL(value).protocol === "https:" } catch { return false }
}

export const decodeMobileRepositoryPreview = (
  value: unknown,
  request: MobileRepositoryScope & Readonly<{ pathRef: string; expectedRevisionRef: string }>,
): MobileRepositoryPreview | null => {
  if (typeof value !== "object" || value === null) return null
  const preview = value as Record<string, unknown>
  if (!exactScope(preview, request) || preview.pathRef !== request.pathRef ||
    preview.revisionRef !== request.expectedRevisionRef ||
    typeof preview.sizeBytes !== "number" || !Number.isSafeInteger(preview.sizeBytes) || preview.sizeBytes < 0) return null
  const base = { ...request, revisionRef: request.expectedRevisionRef, sizeBytes: preview.sizeBytes }
  if (preview.kind === "source" || preview.kind === "markdown") {
    if (preview.sizeBytes > MOBILE_REPOSITORY_TEXT_MAX_BYTES || typeof preview.content !== "string" ||
      new TextEncoder().encode(preview.content).byteLength !== preview.sizeBytes || preview.content.includes("\0")) return null
    return preview.kind === "markdown"
      ? { ...base, kind: "markdown", content: preview.content }
      : typeof preview.language === "string" && preview.language.length > 0 && preview.language.length <= 64
        ? { ...base, kind: "source", language: preview.language, content: preview.content }
        : null
  }
  if (preview.kind !== "image" || preview.sizeBytes > MOBILE_REPOSITORY_IMAGE_MAX_BYTES ||
    (preview.mediaType !== "image/png" && preview.mediaType !== "image/jpeg" &&
      preview.mediaType !== "image/gif" && preview.mediaType !== "image/webp") ||
    !safeHttpsUrl(preview.contentUrl) || typeof preview.sha256 !== "string" ||
    !/^[0-9a-f]{64}$/u.test(preview.sha256)) return null
  return { ...base, kind: "image", mediaType: preview.mediaType, contentUrl: preview.contentUrl, sha256: preview.sha256 }
}

export const loadMobileRepositoryTree = async (
  port: MobileRepositoryFilesPort,
  request: MobileRepositoryScope & Readonly<{ directoryRef: string; cursor: string | null; limit?: number }>,
) => {
  const bounded = { ...request, limit: Math.min(MOBILE_REPOSITORY_TREE_LIMIT, Math.max(1, request.limit ?? MOBILE_REPOSITORY_TREE_LIMIT)) }
  try {
    const page = decodeMobileRepositoryTreePage(await port.tree(bounded), bounded)
    return page === null
      ? { state: "failed" as const, message: "The environment returned an invalid repository tree." }
      : { state: "ready" as const, page }
  } catch {
    return { state: "failed" as const, message: "Repository files are unavailable right now." }
  }
}

export const loadMobileRepositoryPreview = async (
  port: MobileRepositoryFilesPort,
  request: MobileRepositoryScope & Readonly<{ pathRef: string; expectedRevisionRef: string }>,
) => {
  try {
    const preview = decodeMobileRepositoryPreview(await port.read(request), request)
    return preview === null
      ? { state: "failed" as const, message: "The environment returned an invalid or stale file preview." }
      : { state: "ready" as const, preview }
  } catch {
    return { state: "failed" as const, message: "That file preview is unavailable right now." }
  }
}
