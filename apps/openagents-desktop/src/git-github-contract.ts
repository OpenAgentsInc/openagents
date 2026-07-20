/**
 * Typed Git/GitHub surface contract (EP250 capability E2–E5, #8712).
 *
 * A single namespaced invoke channel (`openagents-desktop/git-github`) carries
 * a closed, discriminated operation set — never arbitrary argv. Every request
 * and every result is Effect-Schema decoded on BOTH sides (preload decodes the
 * request before it reaches main; the renderer decodes the result before it
 * touches state). The audit (docs/fable/2026-07-11-daily-coding-capability-
 * audit.md §4E, §6.3) ranks commit/push/PR/issue flows as a daily habit with
 * zero typed surface today; this contract is that surface.
 *
 * Public-safety: results carry bounded, public-safe fields only — a commit SHA
 * and summary, a pushed ref, an issue/PR number+url. No tokens, credentials,
 * raw command output, local absolute paths, or provider payloads ever cross
 * the bridge; typed failure classes replace raw stderr.
 */
import { Exit, Schema } from "@effect-native/core/effect"

export const GitGithubChannel = "openagents-desktop/git-github" as const

/** The closed operation set. A renderer can request nothing else. */
export const gitGithubOps = [
  "status",
  "diff",
  "discard",
  "stage",
  "unstage",
  "commit",
  "push",
  "branchList",
  "branchCreate",
  "checkout",
  "issueList",
  "issueView",
  "issueCreate",
  "prList",
  "prView",
  "prCreate",
] as const
export type GitGithubOp = (typeof gitGithubOps)[number]

/**
 * Typed failure classes. The host maps every real failure (missing binary,
 * rejected push, dirty tree, unauthenticated gh, …) onto one of these; the
 * renderer renders the class, never raw stderr.
 */
export const gitGithubErrorCodes = [
  "invalid_request",
  "no_workspace",
  "not_a_repo",
  "git_unavailable",
  "empty_message",
  "nothing_staged",
  "no_upstream",
  "non_fast_forward",
  "auth_failed",
  "blocked_by_hook",
  "dirty_tree",
  "stale_status",
  "unsafe_state",
  "binary_diff",
  "secret_diff",
  "diff_too_large",
  "branch_exists",
  "invalid_branch_name",
  "invalid_path",
  "gh_unavailable",
  "gh_unauthenticated",
  "not_found",
  "operation_failed",
] as const
export type GitGithubErrorCode = (typeof gitGithubErrorCodes)[number]

// ---------------------------------------------------------------------------
// Requests
// ---------------------------------------------------------------------------

const StatusRequestSchema = Schema.Struct({ op: Schema.Literal("status") })
const GitReviewPathSchema = Schema.String.check(Schema.isMaxLength(1_024))
const GitReviewIdentitySchema = Schema.String.check(Schema.isMaxLength(160))
const GitTimelineItemRefSchema = Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(160))
const DiffRequestSchema = Schema.Struct({
  op: Schema.Literal("diff"),
  repositoryRef: GitReviewIdentitySchema,
  statusRef: GitReviewIdentitySchema,
  path: GitReviewPathSchema,
  source: Schema.Literals(["staged", "unstaged"]),
  causalItemRef: Schema.NullOr(GitTimelineItemRefSchema),
})
const DiscardRequestSchema = Schema.Struct({
  op: Schema.Literal("discard"),
  repositoryRef: GitReviewIdentitySchema,
  statusRef: GitReviewIdentitySchema,
  path: GitReviewPathSchema,
})
const expectedStatus = { repositoryRef: GitReviewIdentitySchema, statusRef: GitReviewIdentitySchema }
const StageRequestSchema = Schema.Struct({ op: Schema.Literal("stage"), ...expectedStatus, paths: Schema.Array(Schema.String) })
const UnstageRequestSchema = Schema.Struct({ op: Schema.Literal("unstage"), ...expectedStatus, paths: Schema.Array(Schema.String) })
const CommitRequestSchema = Schema.Struct({ op: Schema.Literal("commit"), ...expectedStatus, message: Schema.String })
const PushRequestSchema = Schema.Struct({ op: Schema.Literal("push"), ...expectedStatus })
const BranchListRequestSchema = Schema.Struct({ op: Schema.Literal("branchList") })
const BranchCreateRequestSchema = Schema.Struct({
  op: Schema.Literal("branchCreate"),
  name: Schema.String,
  checkout: Schema.Boolean,
  ...expectedStatus,
})
const CheckoutRequestSchema = Schema.Struct({ op: Schema.Literal("checkout"), ...expectedStatus, name: Schema.String })
const IssueListRequestSchema = Schema.Struct({
  op: Schema.Literal("issueList"),
  limit: Schema.Number.pipe(Schema.optionalKey),
})
const IssueViewRequestSchema = Schema.Struct({ op: Schema.Literal("issueView"), number: Schema.Number })
const IssueCreateRequestSchema = Schema.Struct({
  op: Schema.Literal("issueCreate"),
  title: Schema.String,
  body: Schema.String,
})
const PrListRequestSchema = Schema.Struct({
  op: Schema.Literal("prList"),
  limit: Schema.Number.pipe(Schema.optionalKey),
})
const PrViewRequestSchema = Schema.Struct({ op: Schema.Literal("prView"), number: Schema.Number })
const PrCreateRequestSchema = Schema.Struct({
  op: Schema.Literal("prCreate"),
  title: Schema.String,
  body: Schema.String,
  base: Schema.String.pipe(Schema.optionalKey),
  head: Schema.String.pipe(Schema.optionalKey),
})

export const GitGithubRequestSchema = Schema.Union([
  StatusRequestSchema,
  DiffRequestSchema,
  DiscardRequestSchema,
  StageRequestSchema,
  UnstageRequestSchema,
  CommitRequestSchema,
  PushRequestSchema,
  BranchListRequestSchema,
  BranchCreateRequestSchema,
  CheckoutRequestSchema,
  IssueListRequestSchema,
  IssueViewRequestSchema,
  IssueCreateRequestSchema,
  PrListRequestSchema,
  PrViewRequestSchema,
  PrCreateRequestSchema,
])
export type GitGithubRequest = Schema.Schema.Type<typeof GitGithubRequestSchema>

// ---------------------------------------------------------------------------
// Results
// ---------------------------------------------------------------------------

export const gitFileStatuses = [
  "added",
  "modified",
  "deleted",
  "renamed",
  "copied",
  "untracked",
  "type-changed",
  "unmerged",
] as const
export type GitFileStatus = (typeof gitFileStatuses)[number]

const GitFileEntrySchema = Schema.Struct({
  path: Schema.String,
  status: Schema.Literals(gitFileStatuses),
})
export type GitFileEntry = Schema.Schema.Type<typeof GitFileEntrySchema>

export const GitStatusResultSchema = Schema.Struct({
  ok: Schema.Literal(true),
  op: Schema.Literal("status"),
  branch: Schema.NullOr(Schema.String),
  upstream: Schema.NullOr(Schema.String),
  detached: Schema.Boolean,
  ahead: Schema.Number,
  behind: Schema.Number,
  staged: Schema.Array(GitFileEntrySchema),
  unstaged: Schema.Array(GitFileEntrySchema),
  untracked: Schema.Array(GitFileEntrySchema),
  truncated: Schema.Boolean,
  /** Opaque canonical-worktree identity; never the root itself. */
  repositoryRef: GitReviewIdentitySchema,
  /** Exact HEAD + porcelain snapshot fence for review and mutation requests. */
  statusRef: GitReviewIdentitySchema,
  headRef: Schema.NullOr(GitReviewIdentitySchema),
})
export type GitStatusResult = Schema.Schema.Type<typeof GitStatusResultSchema>

const GitDiffHunkSchema = Schema.Struct({
  header: Schema.String.check(Schema.isMaxLength(400)),
  oldStart: Schema.Number.check(Schema.isInt(), Schema.isGreaterThanOrEqualTo(0)),
  oldLines: Schema.Number.check(Schema.isInt(), Schema.isGreaterThanOrEqualTo(0)),
  newStart: Schema.Number.check(Schema.isInt(), Schema.isGreaterThanOrEqualTo(0)),
  newLines: Schema.Number.check(Schema.isInt(), Schema.isGreaterThanOrEqualTo(0)),
  content: Schema.String.check(Schema.isMaxLength(120_000)),
})
export type GitDiffHunk = Schema.Schema.Type<typeof GitDiffHunkSchema>

export const GitDiffResultSchema = Schema.Struct({
  ok: Schema.Literal(true),
  op: Schema.Literal("diff"),
  repositoryRef: GitReviewIdentitySchema,
  statusRef: GitReviewIdentitySchema,
  path: GitReviewPathSchema,
  source: Schema.Literals(["staged", "unstaged"]),
  causalItemRef: Schema.NullOr(GitTimelineItemRefSchema),
  content: Schema.String.check(Schema.isMaxLength(120_000)),
  hunks: Schema.Array(GitDiffHunkSchema).check(Schema.isMaxLength(500)),
  truncated: Schema.Literal(false),
})
export type GitDiffResult = Schema.Schema.Type<typeof GitDiffResultSchema>

const DiscardResultSchema = Schema.Struct({
  ok: Schema.Literal(true),
  op: Schema.Literal("discard"),
  repositoryRef: GitReviewIdentitySchema,
  path: GitReviewPathSchema,
  statusRef: GitReviewIdentitySchema,
})

const PathsResultSchema = Schema.Struct({
  ok: Schema.Literal(true),
  op: Schema.Literals(["stage", "unstage"]),
  paths: Schema.Array(Schema.String),
})

const CommitResultSchema = Schema.Struct({
  ok: Schema.Literal(true),
  op: Schema.Literal("commit"),
  /** The RECEIPT: the new commit SHA. */
  sha: Schema.String,
  shortSha: Schema.String,
  summary: Schema.String,
})
export type GitCommitResult = Schema.Schema.Type<typeof CommitResultSchema>

const PushResultSchema = Schema.Struct({
  ok: Schema.Literal(true),
  op: Schema.Literal("push"),
  /** The RECEIPT: the ref that was pushed and the SHA it now points at. */
  ref: Schema.String,
  remote: Schema.String,
  sha: Schema.String,
})
export type GitPushResult = Schema.Schema.Type<typeof PushResultSchema>

const BranchSchema = Schema.Struct({
  name: Schema.String,
  current: Schema.Boolean,
  upstream: Schema.NullOr(Schema.String),
})
export type GitBranch = Schema.Schema.Type<typeof BranchSchema>

const BranchListResultSchema = Schema.Struct({
  ok: Schema.Literal(true),
  op: Schema.Literal("branchList"),
  current: Schema.NullOr(Schema.String),
  branches: Schema.Array(BranchSchema),
  truncated: Schema.Boolean,
})
export type GitBranchListResult = Schema.Schema.Type<typeof BranchListResultSchema>

const BranchCreateResultSchema = Schema.Struct({
  ok: Schema.Literal(true),
  op: Schema.Literal("branchCreate"),
  name: Schema.String,
  checkedOut: Schema.Boolean,
})

const CheckoutResultSchema = Schema.Struct({
  ok: Schema.Literal(true),
  op: Schema.Literal("checkout"),
  name: Schema.String,
})

const IssueRefSchema = Schema.Struct({
  number: Schema.Number,
  title: Schema.String,
  url: Schema.String,
  state: Schema.String,
})
export type GitHubIssueRef = Schema.Schema.Type<typeof IssueRefSchema>

const IssueDetailSchema = Schema.Struct({
  number: Schema.Number,
  title: Schema.String,
  url: Schema.String,
  state: Schema.String,
  body: Schema.String,
})
export type GitHubIssueDetail = Schema.Schema.Type<typeof IssueDetailSchema>

const IssueListResultSchema = Schema.Struct({
  ok: Schema.Literal(true),
  op: Schema.Literal("issueList"),
  issues: Schema.Array(IssueRefSchema),
})
const IssueViewResultSchema = Schema.Struct({
  ok: Schema.Literal(true),
  op: Schema.Literal("issueView"),
  issue: IssueDetailSchema,
})
const CreateRefResultSchema = Schema.Struct({
  ok: Schema.Literal(true),
  op: Schema.Literals(["issueCreate", "prCreate"]),
  /** The RECEIPT: the number and dereferenceable url of the created issue/PR. */
  number: Schema.Number,
  url: Schema.String,
})
export type GitHubCreateResult = Schema.Schema.Type<typeof CreateRefResultSchema>

const PrRefSchema = Schema.Struct({
  number: Schema.Number,
  title: Schema.String,
  url: Schema.String,
  state: Schema.String,
  headRefName: Schema.String,
  baseRefName: Schema.String,
})
export type GitHubPrRef = Schema.Schema.Type<typeof PrRefSchema>

const PrDetailSchema = Schema.Struct({
  number: Schema.Number,
  title: Schema.String,
  url: Schema.String,
  state: Schema.String,
  headRefName: Schema.String,
  baseRefName: Schema.String,
  body: Schema.String,
})
export type GitHubPrDetail = Schema.Schema.Type<typeof PrDetailSchema>

const PrListResultSchema = Schema.Struct({
  ok: Schema.Literal(true),
  op: Schema.Literal("prList"),
  prs: Schema.Array(PrRefSchema),
})
const PrViewResultSchema = Schema.Struct({
  ok: Schema.Literal(true),
  op: Schema.Literal("prView"),
  pr: PrDetailSchema,
})

const ErrorResultSchema = Schema.Struct({
  ok: Schema.Literal(false),
  op: Schema.Literals(gitGithubOps),
  error: Schema.Literals(gitGithubErrorCodes),
  /** A public-safe message; never raw stderr, tokens, or absolute paths. */
  message: Schema.String,
})
export type GitGithubErrorResult = Schema.Schema.Type<typeof ErrorResultSchema>

export const GitGithubResultSchema = Schema.Union([
  GitStatusResultSchema,
  GitDiffResultSchema,
  DiscardResultSchema,
  PathsResultSchema,
  CommitResultSchema,
  PushResultSchema,
  BranchListResultSchema,
  BranchCreateResultSchema,
  CheckoutResultSchema,
  IssueListResultSchema,
  IssueViewResultSchema,
  CreateRefResultSchema,
  PrListResultSchema,
  PrViewResultSchema,
  ErrorResultSchema,
])
export type GitGithubResult = Schema.Schema.Type<typeof GitGithubResultSchema>

// ---------------------------------------------------------------------------
// Decoders (both sides)
// ---------------------------------------------------------------------------

export const decodeGitGithubRequest = (value: unknown): GitGithubRequest | null => {
  const result = Schema.decodeUnknownExit(GitGithubRequestSchema)(value)
  return Exit.isSuccess(result) ? result.value : null
}

export const decodeGitGithubResult = (value: unknown): GitGithubResult | null => {
  const result = Schema.decodeUnknownExit(GitGithubResultSchema)(value)
  return Exit.isSuccess(result) ? result.value : null
}

/** A typed, decodable error result — used by host and bridge fallbacks. */
export const gitGithubError = (
  op: GitGithubOp,
  error: GitGithubErrorCode,
  message: string,
): GitGithubErrorResult => ({ ok: false, op, error, message })
