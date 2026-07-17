import { safeMobileComposerPathRef } from "./mobile-composer-path-context"
import type { MobileRepositoryScope } from "./mobile-repository-files"

export const MOBILE_REVIEW_MAX_FILES = 500
export const MOBILE_REVIEW_MAX_HUNKS = 200
export const MOBILE_REVIEW_MAX_ROWS = 5_000
export const MOBILE_REVIEW_COMMENT_MAX = 4_000

export type MobileChangedFile = Readonly<{
  pathRef: string
  source: "staged" | "unstaged" | "untracked"
  status: "added" | "modified" | "deleted" | "renamed" | "unmerged"
  adds: number | null
  dels: number | null
  binary: boolean
  revisionRef: string
}>

export type MobileChangeSummary = MobileRepositoryScope & Readonly<{
  statusRef: string
  headRef: string | null
  files: ReadonlyArray<MobileChangedFile>
  truncated: boolean
}>

export type MobileDiffRow = Readonly<{
  rowRef: string
  kind: "context" | "add" | "remove"
  text: string
  oldLine: number | null
  newLine: number | null
}>

export type MobileFileDiff = MobileRepositoryScope & Readonly<{
  statusRef: string
  pathRef: string
  source: "staged" | "unstaged"
  revisionRef: string
  language: string
  hunks: ReadonlyArray<Readonly<{ header: string; rows: ReadonlyArray<MobileDiffRow> }>>
}>

export type MobileReviewReceipt = MobileRepositoryScope & Readonly<{
  statusRef: string
  pathRef: string
  rowRef: string
  reviewRef: string
  receiptRef: string
  state: "recorded"
  recordedAt: string
  comment: string
}>

export type MobileRepositoryReviewPort = Readonly<{
  status: (request: MobileRepositoryScope) => Promise<unknown>
  diff: (request: MobileRepositoryScope & Readonly<{
    statusRef: string
    pathRef: string
    source: "staged" | "unstaged"
    expectedRevisionRef: string
  }>) => Promise<unknown>
  submitReview: (request: MobileRepositoryScope & Readonly<{
    statusRef: string
    pathRef: string
    rowRef: string
    expectedRevisionRef: string
    comment: string
    idempotencyRef: string
  }>) => Promise<unknown>
}>

export type MobileRepositoryReviewState = Readonly<{
  scope: MobileRepositoryScope | null
  state: "idle" | "loading" | "ready" | "unavailable" | "failed"
  summary: MobileChangeSummary | null
  diff: MobileFileDiff | null
  selectedRowRef: string | null
  commentDraft: string
  submitting: boolean
  receipts: ReadonlyArray<MobileReviewReceipt>
  requestEpoch: number
  message: string | null
}>

export const initialMobileRepositoryReviewState: MobileRepositoryReviewState = {
  scope: null,
  state: "idle",
  summary: null,
  diff: null,
  selectedRowRef: null,
  commentDraft: "",
  submitting: false,
  receipts: [],
  requestEpoch: 0,
  message: null,
}

const safeRef = (value: unknown): value is string => typeof value === "string" &&
  value.length > 0 && value.length <= 256 && /^[A-Za-z0-9][A-Za-z0-9._:-]*$/u.test(value)
const nonNegative = (value: unknown): value is number => typeof value === "number" && Number.isSafeInteger(value) && value >= 0
const exactScope = (value: Record<string, unknown>, scope: MobileRepositoryScope) =>
  value.sessionRef === scope.sessionRef && value.repositoryRef === scope.repositoryRef && value.worktreeRef === scope.worktreeRef

export const decodeMobileChangeSummary = (value: unknown, scope: MobileRepositoryScope): MobileChangeSummary | null => {
  if (typeof value !== "object" || value === null) return null
  const row = value as Record<string, unknown>
  if (!exactScope(row, scope) || !safeRef(row.statusRef) || !(row.headRef === null || safeRef(row.headRef)) ||
    !Array.isArray(row.files) || row.files.length > MOBILE_REVIEW_MAX_FILES || typeof row.truncated !== "boolean") return null
  const seen = new Set<string>()
  const files: MobileChangedFile[] = []
  for (const candidate of row.files) {
    if (typeof candidate !== "object" || candidate === null) return null
    const file = candidate as Record<string, unknown>
    if (!safeMobileComposerPathRef(file.pathRef) ||
      (file.source !== "staged" && file.source !== "unstaged" && file.source !== "untracked") ||
      (file.status !== "added" && file.status !== "modified" && file.status !== "deleted" && file.status !== "renamed" && file.status !== "unmerged") ||
      !(file.adds === null || nonNegative(file.adds)) || !(file.dels === null || nonNegative(file.dels)) ||
      typeof file.binary !== "boolean" || !safeRef(file.revisionRef) || seen.has(`${file.source}:${file.pathRef}`)) return null
    seen.add(`${file.source}:${file.pathRef}`)
    files.push(file as MobileChangedFile)
  }
  return { ...scope, statusRef: row.statusRef, headRef: row.headRef as string | null, files, truncated: row.truncated }
}

export const decodeMobileFileDiff = (
  value: unknown,
  request: MobileRepositoryScope & Readonly<{ statusRef: string; pathRef: string; source: "staged" | "unstaged"; expectedRevisionRef: string }>,
): MobileFileDiff | null => {
  if (typeof value !== "object" || value === null) return null
  const diff = value as Record<string, unknown>
  if (!exactScope(diff, request) || diff.statusRef !== request.statusRef || diff.pathRef !== request.pathRef ||
    diff.source !== request.source || diff.revisionRef !== request.expectedRevisionRef ||
    typeof diff.language !== "string" || diff.language.length > 64 || !Array.isArray(diff.hunks) ||
    diff.hunks.length > MOBILE_REVIEW_MAX_HUNKS) return null
  let rowCount = 0
  const rowRefs = new Set<string>()
  const hunks: Array<{ header: string; rows: MobileDiffRow[] }> = []
  for (const candidate of diff.hunks) {
    if (typeof candidate !== "object" || candidate === null) return null
    const hunk = candidate as Record<string, unknown>
    if (typeof hunk.header !== "string" || hunk.header.length > 400 || !Array.isArray(hunk.rows)) return null
    const rows: MobileDiffRow[] = []
    for (const item of hunk.rows) {
      rowCount += 1
      if (rowCount > MOBILE_REVIEW_MAX_ROWS || typeof item !== "object" || item === null) return null
      const row = item as Record<string, unknown>
      if (!safeRef(row.rowRef) || rowRefs.has(row.rowRef) ||
        (row.kind !== "context" && row.kind !== "add" && row.kind !== "remove") ||
        typeof row.text !== "string" || row.text.length > 4_000 || row.text.includes("\0") ||
        !(row.oldLine === null || nonNegative(row.oldLine)) || !(row.newLine === null || nonNegative(row.newLine))) return null
      rowRefs.add(row.rowRef)
      rows.push(row as MobileDiffRow)
    }
    hunks.push({ header: hunk.header, rows })
  }
  return { ...request, revisionRef: request.expectedRevisionRef, language: diff.language, hunks }
}

export const decodeMobileReviewReceipt = (
  value: unknown,
  request: MobileRepositoryScope & Readonly<{ statusRef: string; pathRef: string; rowRef: string; expectedRevisionRef: string; comment: string; idempotencyRef: string }>,
): MobileReviewReceipt | null => {
  if (typeof value !== "object" || value === null) return null
  const receipt = value as Record<string, unknown>
  return exactScope(receipt, request) && receipt.statusRef === request.statusRef && receipt.pathRef === request.pathRef &&
    receipt.rowRef === request.rowRef && receipt.comment === request.comment && receipt.state === "recorded" &&
    safeRef(receipt.reviewRef) && safeRef(receipt.receiptRef) &&
    typeof receipt.recordedAt === "string" && /^\d{4}-\d{2}-\d{2}T/u.test(receipt.recordedAt)
    ? receipt as MobileReviewReceipt
    : null
}
