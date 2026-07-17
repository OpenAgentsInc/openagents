import { safeMobileComposerPathRef } from "./mobile-composer-path-context"
import type { MobileRepositoryScope } from "./mobile-repository-files"

export const MOBILE_GIT_MAX_FILES = 500
export const MOBILE_GIT_MAX_BRANCHES = 300
export const MOBILE_GIT_COMMIT_MESSAGE_MAX = 20_000

export type MobileGitFile = Readonly<{
  pathRef: string
  status: "added" | "modified" | "deleted" | "renamed" | "untracked" | "unmerged"
  staged: boolean
}>

export type MobileGitBranch = Readonly<{
  branchRef: string
  name: string
  current: boolean
  upstream: string | null
}>

export type MobileGitStatus = MobileRepositoryScope & Readonly<{
  statusRef: string
  headRef: string | null
  branch: string | null
  detached: boolean
  upstream: string | null
  ahead: number
  behind: number
  defaultBranch: boolean
  files: ReadonlyArray<MobileGitFile>
  branches: ReadonlyArray<MobileGitBranch>
  truncated: boolean
}>

export const mobileGitFailureCodes = [
  "stale_status",
  "dirty_tree",
  "conflict",
  "non_fast_forward",
  "auth_failed",
  "blocked_by_hook",
  "nothing_to_commit",
  "no_upstream",
  "detached_head",
  "invalid_branch",
  "operation_failed",
] as const
export type MobileGitFailureCode = (typeof mobileGitFailureCodes)[number]
export type MobileGitOperation = "checkout" | "commit" | "push"

export type MobileGitMutationRequest = MobileRepositoryScope & Readonly<{
  op: MobileGitOperation
  statusRef: string
  expectedHeadRef: string | null
  idempotencyRef: string
  confirmationRef: string
  branchRef?: string
  branchName?: string
  paths?: ReadonlyArray<string>
  message?: string
}>

export type MobileGitReceipt = MobileRepositoryScope & Readonly<{
  op: MobileGitOperation
  requestStatusRef: string
  receiptRef: string
  recordedAt: string
  branch: string
  commitRef: string | null
  remote: string | null
  summary: string
  status: MobileGitStatus
}>

export type MobileGitFailure = MobileRepositoryScope & Readonly<{
  op: MobileGitOperation
  requestStatusRef: string
  code: MobileGitFailureCode
  message: string
}>

export type MobileRepositoryGitPort = Readonly<{
  gitStatus: (request: MobileRepositoryScope) => Promise<unknown>
  gitMutate: (request: MobileGitMutationRequest) => Promise<unknown>
}>

export type MobileRepositoryGitState = Readonly<{
  scope: MobileRepositoryScope | null
  state: "idle" | "loading" | "ready" | "unavailable" | "failed"
  status: MobileGitStatus | null
  selectedPaths: ReadonlyArray<string>
  commitMessage: string
  pendingConfirmation: MobileGitMutationRequest | null
  submitting: boolean
  receipts: ReadonlyArray<MobileGitReceipt>
  requestEpoch: number
  message: string | null
  failureCode: MobileGitFailureCode | null
}>

export const initialMobileRepositoryGitState: MobileRepositoryGitState = {
  scope: null,
  state: "idle",
  status: null,
  selectedPaths: [],
  commitMessage: "",
  pendingConfirmation: null,
  submitting: false,
  receipts: [],
  requestEpoch: 0,
  message: null,
  failureCode: null,
}

const safeRef = (value: unknown): value is string => typeof value === "string" &&
  value.length > 0 && value.length <= 256 && /^[A-Za-z0-9][A-Za-z0-9._:-]*$/u.test(value)
const safeBranchName = (value: unknown): value is string => typeof value === "string" && value.length > 0 &&
  value.length <= 201 && /^[A-Za-z0-9][A-Za-z0-9._/-]*$/u.test(value) && !value.includes("..") &&
  !value.includes("//") && !value.endsWith("/") && !value.endsWith(".lock") && !value.includes("@{")
const nonNegative = (value: unknown): value is number => typeof value === "number" && Number.isSafeInteger(value) && value >= 0
const exactScope = (value: Record<string, unknown>, scope: MobileRepositoryScope) =>
  value.sessionRef === scope.sessionRef && value.repositoryRef === scope.repositoryRef && value.worktreeRef === scope.worktreeRef

export const decodeMobileGitStatus = (value: unknown, scope: MobileRepositoryScope): MobileGitStatus | null => {
  if (typeof value !== "object" || value === null) return null
  const row = value as Record<string, unknown>
  if (!exactScope(row, scope) || !safeRef(row.statusRef) || !(row.headRef === null || safeRef(row.headRef)) ||
    !(row.branch === null || safeBranchName(row.branch)) || typeof row.detached !== "boolean" ||
    !(row.upstream === null || safeBranchName(row.upstream)) || !nonNegative(row.ahead) || !nonNegative(row.behind) ||
    typeof row.defaultBranch !== "boolean" || !Array.isArray(row.files) || row.files.length > MOBILE_GIT_MAX_FILES ||
    !Array.isArray(row.branches) || row.branches.length > MOBILE_GIT_MAX_BRANCHES || typeof row.truncated !== "boolean") return null
  if ((row.detached && row.branch !== null) || (!row.detached && row.branch === null)) return null
  const seenFiles = new Set<string>()
  const files: MobileGitFile[] = []
  for (const candidate of row.files) {
    if (typeof candidate !== "object" || candidate === null) return null
    const file = candidate as Record<string, unknown>
    if (!safeMobileComposerPathRef(file.pathRef) || seenFiles.has(file.pathRef) ||
      (file.status !== "added" && file.status !== "modified" && file.status !== "deleted" && file.status !== "renamed" && file.status !== "untracked" && file.status !== "unmerged") ||
      typeof file.staged !== "boolean") return null
    seenFiles.add(file.pathRef)
    files.push(file as MobileGitFile)
  }
  const seenBranches = new Set<string>()
  const branches: MobileGitBranch[] = []
  let currentCount = 0
  for (const candidate of row.branches) {
    if (typeof candidate !== "object" || candidate === null) return null
    const branch = candidate as Record<string, unknown>
    if (!safeRef(branch.branchRef) || !safeBranchName(branch.name) || seenBranches.has(branch.branchRef) ||
      typeof branch.current !== "boolean" || !(branch.upstream === null || safeBranchName(branch.upstream))) return null
    if (branch.current) currentCount += 1
    seenBranches.add(branch.branchRef)
    branches.push(branch as MobileGitBranch)
  }
  if ((!row.detached && currentCount !== 1) || (row.detached && currentCount !== 0) ||
    (!row.detached && !branches.some(branch => branch.current && branch.name === row.branch))) return null
  return { ...scope, statusRef: row.statusRef, headRef: row.headRef as string | null, branch: row.branch as string | null,
    detached: row.detached, upstream: row.upstream as string | null, ahead: row.ahead, behind: row.behind,
    defaultBranch: row.defaultBranch, files, branches, truncated: row.truncated }
}

export const decodeMobileGitMutationResult = (
  value: unknown,
  request: MobileGitMutationRequest,
): MobileGitReceipt | MobileGitFailure | null => {
  if (typeof value !== "object" || value === null) return null
  const row = value as Record<string, unknown>
  if (!exactScope(row, request) || row.op !== request.op || row.requestStatusRef !== request.statusRef) return null
  if (row.ok === false) {
    return mobileGitFailureCodes.includes(row.code as MobileGitFailureCode) && typeof row.message === "string" &&
      row.message.length > 0 && row.message.length <= 400 && !row.message.includes("\0")
      ? { ...request, requestStatusRef: request.statusRef, code: row.code as MobileGitFailureCode, message: row.message } : null
  }
  if (row.ok !== true || !safeRef(row.receiptRef) || typeof row.recordedAt !== "string" ||
    !/^\d{4}-\d{2}-\d{2}T/u.test(row.recordedAt) || !safeBranchName(row.branch) ||
    !(row.commitRef === null || safeRef(row.commitRef)) || !(row.remote === null || safeBranchName(row.remote)) ||
    typeof row.summary !== "string" || row.summary.length > 400) return null
  const status = decodeMobileGitStatus(row.status, request)
  if (status === null || status.branch !== row.branch || (request.op !== "checkout" && row.commitRef === null) ||
    (request.op === "push" && row.remote === null)) return null
  return { ...request, requestStatusRef: request.statusRef, receiptRef: row.receiptRef, recordedAt: row.recordedAt, branch: row.branch,
    commitRef: row.commitRef as string | null, remote: row.remote as string | null, summary: row.summary, status }
}
