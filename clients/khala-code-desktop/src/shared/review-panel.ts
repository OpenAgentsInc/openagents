import { Schema as S } from "effect"

// Shared diff-review side-panel contract: file-level diff-kind projection,
// active diff focus, revert availability, and side-panel layout persistence.
// Comment modeling is intentionally reused from `./diff-review.ts` rather
// than duplicated here; this module owns the file-list/review-panel shape
// that sits around those existing per-line comment primitives.

export const KHALA_CODE_REVIEW_LAYOUT_STORAGE_KEY =
  "khala-code:review-panel-layout.v1"

export const KhalaCodeReviewDiffKindSchema = S.Literals([
  "added",
  "deleted",
  "modified",
])
export type KhalaCodeReviewDiffKind =
  typeof KhalaCodeReviewDiffKindSchema.Type

export const KHALA_CODE_REVIEW_DIFF_KIND_ORDER: ReadonlyArray<KhalaCodeReviewDiffKind> = [
  "added",
  "modified",
  "deleted",
]

export const khalaCodeReviewDiffKindLabel = (
  kind: KhalaCodeReviewDiffKind,
): string => {
  switch (kind) {
    case "added":
      return "Added"
    case "deleted":
      return "Deleted"
    case "modified":
      return "Modified"
  }
}

export const KhalaCodeReviewFileEntrySchema = S.Struct({
  additions: S.Number,
  deletions: S.Number,
  diffKind: KhalaCodeReviewDiffKindSchema,
  path: S.String,
  renamedFrom: S.optional(S.String),
})
export type KhalaCodeReviewFileEntry =
  typeof KhalaCodeReviewFileEntrySchema.Type

export const khalaCodeGroupReviewFiles = (
  files: ReadonlyArray<KhalaCodeReviewFileEntry>,
): ReadonlyArray<
  Readonly<{ diffKind: KhalaCodeReviewDiffKind, files: ReadonlyArray<KhalaCodeReviewFileEntry> }>
> =>
  KHALA_CODE_REVIEW_DIFF_KIND_ORDER.map(diffKind => ({
    diffKind,
    files: files
      .filter(file => file.diffKind === diffKind)
      .slice()
      .sort((left, right) => left.path.localeCompare(right.path, undefined, { sensitivity: "base" })),
  })).filter(group => group.files.length > 0)

// --- Diff-kind projection -------------------------------------------------
//
// Parses a standard unified `git diff` text body into per-file review
// entries. This is the only source of file-level added/modified/deleted
// state; there is no separate structured git-status backend today, so the
// review panel derives its file list directly from the same
// `gitDiffToRemote` text the existing `/diff` slash command already renders.

const DIFF_GIT_HEADER = /^diff --git a\/(.+?) b\/(.+)$/
const RENAME_FROM = /^rename from (.+)$/

type MutableReviewFileEntry = {
  additions: number
  deletions: number
  isDeleted: boolean
  isNew: boolean
  path: string
  renamedFrom: string | null
}

export const khalaCodeProjectReviewDiff = (
  diffText: string,
): ReadonlyArray<KhalaCodeReviewFileEntry> => {
  if (diffText.trim().length === 0) return []

  const entries: KhalaCodeReviewFileEntry[] = []
  let current: MutableReviewFileEntry | null = null

  const flush = (): void => {
    if (current === null) return
    entries.push({
      additions: current.additions,
      deletions: current.deletions,
      diffKind: current.isNew ? "added" : current.isDeleted ? "deleted" : "modified",
      path: current.path,
      ...(current.renamedFrom === null ? {} : { renamedFrom: current.renamedFrom }),
    })
    current = null
  }

  for (const line of diffText.split("\n")) {
    const header = DIFF_GIT_HEADER.exec(line)
    if (header !== null) {
      flush()
      current = {
        additions: 0,
        deletions: 0,
        isDeleted: false,
        isNew: false,
        path: header[2],
        renamedFrom: null,
      }
      continue
    }
    if (current === null) continue
    if (line.startsWith("new file mode")) {
      current.isNew = true
      continue
    }
    if (line.startsWith("deleted file mode")) {
      current.isDeleted = true
      continue
    }
    const renameFrom = RENAME_FROM.exec(line)
    if (renameFrom !== null && renameFrom[1] !== current.path) {
      current.renamedFrom = renameFrom[1]
      continue
    }
    if (line.startsWith("+++") || line.startsWith("---")) continue
    if (line.startsWith("+")) {
      current.additions += 1
      continue
    }
    if (line.startsWith("-")) {
      current.deletions += 1
      continue
    }
  }
  flush()
  return entries
}

// --- Active diff focus -----------------------------------------------------

export type KhalaCodeReviewFocus = Readonly<{
  filePath: string | null
}>

export const khalaCodeDefaultReviewFocus = (): KhalaCodeReviewFocus => ({ filePath: null })

// --- Revert availability ----------------------------------------------------
//
// Khala Code review is display-only today: there is no safe scoped-revert
// backend (writing files is out of scope for this panel, and the only file
// write RPC is an unrelated full-file writer with no git-aware undo
// semantics). Model the state explicitly rather than rendering a revert
// control that silently does nothing.

export const KhalaCodeReviewRevertReasonSchema = S.Literals(["no_safe_backend"])
export type KhalaCodeReviewRevertReason =
  typeof KhalaCodeReviewRevertReasonSchema.Type

export const KhalaCodeReviewRevertStateSchema = S.Struct({
  kind: S.Literal("unavailable"),
  message: S.String,
  reason: KhalaCodeReviewRevertReasonSchema,
})
export type KhalaCodeReviewRevertState =
  typeof KhalaCodeReviewRevertStateSchema.Type

export const KHALA_CODE_REVIEW_REVERT_UNAVAILABLE_MESSAGE =
  "Revert isn't available yet: Khala Code review has no safe backend to undo a file change."

export const khalaCodeReviewRevertState = (): KhalaCodeReviewRevertState => ({
  kind: "unavailable",
  message: KHALA_CODE_REVIEW_REVERT_UNAVAILABLE_MESSAGE,
  reason: "no_safe_backend",
})

// --- Side-panel layout persistence -----------------------------------------

export const KhalaCodeReviewLayoutTabSchema = S.Literals(["comments", "files"])
export type KhalaCodeReviewLayoutTab =
  typeof KhalaCodeReviewLayoutTabSchema.Type

export const KHALA_CODE_REVIEW_LAYOUT_MIN_WIDTH_PX = 240
export const KHALA_CODE_REVIEW_LAYOUT_MAX_WIDTH_PX = 640
export const KHALA_CODE_REVIEW_LAYOUT_DEFAULT_WIDTH_PX = 320

export const KhalaCodeReviewLayoutStateSchema = S.Struct({
  activeTab: KhalaCodeReviewLayoutTabSchema,
  collapsed: S.Boolean,
  widthPx: S.Number,
})
export type KhalaCodeReviewLayoutState =
  typeof KhalaCodeReviewLayoutStateSchema.Type

export const khalaCodeDefaultReviewLayout = (): KhalaCodeReviewLayoutState => ({
  activeTab: "files",
  collapsed: false,
  widthPx: KHALA_CODE_REVIEW_LAYOUT_DEFAULT_WIDTH_PX,
})

export const khalaCodeClampReviewLayoutWidth = (widthPx: number): number => {
  if (!Number.isFinite(widthPx)) return KHALA_CODE_REVIEW_LAYOUT_DEFAULT_WIDTH_PX
  return Math.min(
    KHALA_CODE_REVIEW_LAYOUT_MAX_WIDTH_PX,
    Math.max(KHALA_CODE_REVIEW_LAYOUT_MIN_WIDTH_PX, Math.round(widthPx)),
  )
}

export const khalaCodeParseReviewLayout = (
  raw: string | null,
): KhalaCodeReviewLayoutState => {
  if (raw === null || raw.length === 0) return khalaCodeDefaultReviewLayout()
  try {
    const parsed: unknown = JSON.parse(raw)
    const decoded = S.decodeUnknownSync(KhalaCodeReviewLayoutStateSchema)(parsed)
    return {
      activeTab: decoded.activeTab,
      collapsed: decoded.collapsed,
      widthPx: khalaCodeClampReviewLayoutWidth(decoded.widthPx),
    }
  } catch {
    return khalaCodeDefaultReviewLayout()
  }
}

export const khalaCodeSerializeReviewLayout = (
  state: KhalaCodeReviewLayoutState,
): string => JSON.stringify(state)

// --- RPC contract ------------------------------------------------------------

export const KhalaCodeReviewDiffReadRequestSchema = S.Struct({
  cwd: S.optional(S.String),
})
export type KhalaCodeReviewDiffReadRequest =
  typeof KhalaCodeReviewDiffReadRequestSchema.Type

export const KhalaCodeReviewDiffReadErrorCodeSchema = S.Literals([
  "provider_unavailable",
  "unknown",
])
export type KhalaCodeReviewDiffReadErrorCode =
  typeof KhalaCodeReviewDiffReadErrorCodeSchema.Type

export const KhalaCodeReviewDiffReadErrorSchema = S.Struct({
  code: KhalaCodeReviewDiffReadErrorCodeSchema,
  message: S.String,
})
export type KhalaCodeReviewDiffReadError =
  typeof KhalaCodeReviewDiffReadErrorSchema.Type

export const KhalaCodeReviewDiffReadResultSchema = S.Union([
  S.Struct({
    files: S.Array(KhalaCodeReviewFileEntrySchema),
    ok: S.Literal(true),
    sha: S.NullOr(S.String),
    truncated: S.Boolean,
  }),
  S.Struct({
    error: KhalaCodeReviewDiffReadErrorSchema,
    ok: S.Literal(false),
  }),
])
export type KhalaCodeReviewDiffReadResult =
  typeof KhalaCodeReviewDiffReadResultSchema.Type

// --- Public-safe projection ---------------------------------------------------
//
// If review-panel data is ever surfaced on a public/agent-facing projection
// (a product-promise report, a public trace, a Forum post), only these
// bucketed counts are safe: no file paths, no diff text, no provider/runtime
// material.

export const KhalaCodeReviewPublicSafeSummarySchema = S.Struct({
  added: S.Number,
  deleted: S.Number,
  modified: S.Number,
  totalAdditions: S.Number,
  totalDeletions: S.Number,
})
export type KhalaCodeReviewPublicSafeSummary =
  typeof KhalaCodeReviewPublicSafeSummarySchema.Type

export const khalaCodeReviewPublicSafeSummary = (
  files: ReadonlyArray<KhalaCodeReviewFileEntry>,
): KhalaCodeReviewPublicSafeSummary =>
  files.reduce<KhalaCodeReviewPublicSafeSummary>(
    (summary, file) => ({
      added: summary.added + (file.diffKind === "added" ? 1 : 0),
      deleted: summary.deleted + (file.diffKind === "deleted" ? 1 : 0),
      modified: summary.modified + (file.diffKind === "modified" ? 1 : 0),
      totalAdditions: summary.totalAdditions + file.additions,
      totalDeletions: summary.totalDeletions + file.deletions,
    }),
    { added: 0, deleted: 0, modified: 0, totalAdditions: 0, totalDeletions: 0 },
  )
