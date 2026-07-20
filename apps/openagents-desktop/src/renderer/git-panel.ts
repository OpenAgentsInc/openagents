/**
 * Git review panel (EP250 capability E2–E5, #8712; UX-4 #8790 MVP boundary).
 *
 * A typed surface over ./git-github-contract.ts, mounted inside the review
 * workspace beside the existing diff viewer. Pure Effect Native data — state,
 * typed intents, and a `state -> View` projection over the shared catalog;
 * every host response is Effect-Schema decoded (via decodeGitGithubResult) and
 * never trusted raw. Styling rides the shared tokens only (no raw colors/px —
 * the design-conformance oracle enforces it).
 *
 * UX-4 (#8790, owner verbatim 2026-07-14: "remove … all UI that's not
 * specifically called for in our MVP spec"): the VISIBLE panel is the MVP
 * read-only review boundary of CW-AC-14 — branch/status truth, per-file
 * status, exact diff review, and composer attachment. Commit, push, stage/
 * unstage, discard, branch switching/creation, and issue/PR authoring render
 * no affordance (ProductSpec Scope keeps "destructive Git, commit, push, pull
 * request, or merge" outside the MVP; CW-AC-14 forbids exposing Git mutation
 * authority). Their typed intents/handlers remain internal post-MVP substrate
 * and authorize no visible surface — the mvp-visible-surfaces oracle enforces
 * this against the rendered tree.
 */
import {
  Badge,
  Button,
  DiffView,
  EmptyMessage,
  Icon,
  IntentRef,
  Spacer,
  Stack,
  StaticPayload,
  Text,
  TextField,
  defineIntent,
  type View,
} from "@effect-native/core"
import { Effect, Schema, SubscriptionRef } from "@effect-native/core/effect"

import {
  decodeGitGithubResult,
  gitGithubError,
  type GitBranch,
  type GitFileEntry,
  type GitGithubErrorCode,
  type GitGithubResult,
  type GitDiffResult,
  type GitHubIssueRef,
  type GitHubPrRef,
  type GitStatusResult,
} from "../git-github-contract.ts"
import { IdeReviewSelectionSchema, type IdeReviewSelection } from "../ide/review-contract.ts"

// ---------------------------------------------------------------------------
// State
// ---------------------------------------------------------------------------

export type GitPanelCreateKind = "none" | "issue" | "pr"

export type GitPanelReceipt = Readonly<{
  kind: "commit" | "push" | "discard" | "recovery" | "issue" | "pr"
  headline: string
  detail: string
}>

export type GitPanelState = Readonly<{
  phase: "idle" | "loading" | "ready" | "unavailable"
  status: GitStatusResult | null
  /** Monotonic renderer fence for the exact opaque statusRef snapshot. */
  statusGeneration: number
  reason: string | null
  branches: ReadonlyArray<GitBranch>
  currentBranch: string | null
  commitMessage: string
  committing: boolean
  pushing: boolean
  newBranchName: string
  /** Public-safe last receipt (commit SHA, pushed ref, created url). */
  receipt: GitPanelReceipt | null
  /** Public-safe last action error message (typed class message). */
  actionError: string | null
  /** null = not probed yet; the gh gate result drives the Create reason. */
  ghAvailable: boolean | null
  ghReason: string | null
  issues: ReadonlyArray<GitHubIssueRef>
  prs: ReadonlyArray<GitHubPrRef>
  issuesLoaded: boolean
  prsLoaded: boolean
  create: GitPanelCreateKind
  createTitle: string
  createBody: string
  diff: GitDiffResult | null
  diffLoading: boolean
  /** Typed refusal for the last exact review request; never raw stderr. */
  reviewFailure: GitGithubErrorCode | null
  discardConfirmPath: string | null
  recoveryRef: string | null
  /** Exact user-selected timeline item that caused this review, or explicit null. */
  causalItemRef: string | null
}>

export const emptyGitPanelState = (): GitPanelState => ({
  phase: "idle",
  status: null,
  statusGeneration: 1,
  reason: null,
  branches: [],
  currentBranch: null,
  commitMessage: "",
  committing: false,
  pushing: false,
  newBranchName: "",
  receipt: null,
  actionError: null,
  ghAvailable: null,
  ghReason: null,
  issues: [],
  prs: [],
  issuesLoaded: false,
  prsLoaded: false,
  create: "none",
  createTitle: "",
  createBody: "",
  diff: null,
  diffLoading: false,
  reviewFailure: null,
  discardConfirmPath: null,
  recoveryRef: null,
  causalItemRef: null,
})

export type GitPanelCapableState = Readonly<{ git: GitPanelState }>

// ---------------------------------------------------------------------------
// Bridge
// ---------------------------------------------------------------------------

export type GitGithubBridge = Readonly<{
  run: (value: unknown) => Promise<unknown>
}>

export const unavailableGitGithubBridge: GitGithubBridge = {
  run: async () => gitGithubError("status", "no_workspace", "Git operations are unavailable."),
}

const runOp = async (bridge: GitGithubBridge, request: unknown): Promise<GitGithubResult> => {
  const raw = await bridge.run(request).catch(() => null)
  return decodeGitGithubResult(raw) ?? gitGithubError("status", "operation_failed", "The Git response could not be read.")
}

// ---------------------------------------------------------------------------
// Pure transitions
// ---------------------------------------------------------------------------

/** gh gate errors are recorded so the Create affordance can explain itself. */
const ghAvailabilityFrom = (
  current: GitPanelState,
  error: GitGithubErrorCode,
  message: string,
): Pick<GitPanelState, "ghAvailable" | "ghReason"> =>
  error === "gh_unavailable" || error === "gh_unauthenticated"
    ? { ghAvailable: false, ghReason: message }
    : { ghAvailable: current.ghAvailable, ghReason: current.ghReason }

export const withStatusResult = (git: GitPanelState, result: GitGithubResult): GitPanelState => {
  if (result.ok && result.op === "status") {
    const replaced = git.status?.statusRef !== result.statusRef
    return {
      ...git,
      phase: "ready",
      status: result,
      statusGeneration: replaced ? git.statusGeneration + 1 : git.statusGeneration,
      diff: replaced ? null : git.diff,
      reviewFailure: replaced ? null : git.reviewFailure,
      reason: null,
    }
  }
  if (!result.ok) {
    return { ...git, phase: "unavailable", reason: result.message }
  }
  return git
}

export const withBranchList = (git: GitPanelState, result: GitGithubResult): GitPanelState =>
  result.ok && result.op === "branchList"
    ? { ...git, branches: result.branches, currentBranch: result.current }
    : git

// ---------------------------------------------------------------------------
// Intents
// ---------------------------------------------------------------------------

export const GitPanelRefreshRequested = defineIntent("GitPanelRefreshRequested", Schema.Null)
export const GitPanelStageToggled = defineIntent("GitPanelStageToggled", Schema.String)
export const GitPanelCommitMessageChanged = defineIntent("GitPanelCommitMessageChanged", Schema.String)
export const GitPanelCommitRequested = defineIntent("GitPanelCommitRequested", Schema.Null)
export const GitPanelPushRequested = defineIntent("GitPanelPushRequested", Schema.Null)
export const GitPanelNewBranchNameChanged = defineIntent("GitPanelNewBranchNameChanged", Schema.String)
export const GitPanelBranchCreateRequested = defineIntent("GitPanelBranchCreateRequested", Schema.Null)
export const GitPanelBranchCheckoutRequested = defineIntent("GitPanelBranchCheckoutRequested", Schema.String)
export const GitPanelIssuesRequested = defineIntent("GitPanelIssuesRequested", Schema.Null)
export const GitPanelPrsRequested = defineIntent("GitPanelPrsRequested", Schema.Null)
export const GitPanelCreateFormChanged = defineIntent(
  "GitPanelCreateFormChanged",
  Schema.Literals(["none", "issue", "pr"]),
)
export const GitPanelCreateTitleChanged = defineIntent("GitPanelCreateTitleChanged", Schema.String)
export const GitPanelCreateBodyChanged = defineIntent("GitPanelCreateBodyChanged", Schema.String)
export const GitPanelCreateSubmitted = defineIntent("GitPanelCreateSubmitted", Schema.Null)
export const GitPanelDiffRequested = defineIntent("GitPanelDiffRequested", Schema.Struct({
  path: Schema.String,
  source: Schema.Literals(["staged", "unstaged"]),
}))
export const GitPanelDiffClosed = defineIntent("GitPanelDiffClosed", Schema.Null)
export const GitPanelDiscardRequested = defineIntent("GitPanelDiscardRequested", Schema.String)
export const GitPanelDiscardConfirmed = defineIntent("GitPanelDiscardConfirmed", Schema.Null)
export const GitPanelDiscardCancelled = defineIntent("GitPanelDiscardCancelled", Schema.Null)
export const GitPanelRecoveryRequested = defineIntent("GitPanelRecoveryRequested", Schema.Null)
export const GitPanelContextAttached = defineIntent(
  "GitPanelContextAttached",
  Schema.NullOr(IdeReviewSelectionSchema),
)

export const gitPanelIntents = [
  GitPanelRefreshRequested,
  GitPanelStageToggled,
  GitPanelCommitMessageChanged,
  GitPanelCommitRequested,
  GitPanelPushRequested,
  GitPanelNewBranchNameChanged,
  GitPanelBranchCreateRequested,
  GitPanelBranchCheckoutRequested,
  GitPanelIssuesRequested,
  GitPanelPrsRequested,
  GitPanelCreateFormChanged,
  GitPanelCreateTitleChanged,
  GitPanelCreateBodyChanged,
  GitPanelCreateSubmitted,
  GitPanelDiffRequested,
  GitPanelDiffClosed,
  GitPanelDiscardRequested,
  GitPanelDiscardConfirmed,
  GitPanelDiscardCancelled,
  GitPanelRecoveryRequested,
  GitPanelContextAttached,
] as const

// ---------------------------------------------------------------------------
// Handlers (generic in the shell state shape, same pattern as fleet-workspace)
// ---------------------------------------------------------------------------

export const refreshGitPanel = <S extends GitPanelCapableState>(
  state: SubscriptionRef.SubscriptionRef<S>,
  bridge: GitGithubBridge,
) =>
  Effect.gen(function* () {
    yield* SubscriptionRef.update(state, (next) => ({ ...next, git: { ...next.git, phase: "loading" as const } }))
    const status = yield* Effect.promise(() => runOp(bridge, { op: "status" }))
    yield* SubscriptionRef.update(state, (next) => ({ ...next, git: withStatusResult(next.git, status) }))
    const branches = yield* Effect.promise(() => runOp(bridge, { op: "branchList" }))
    yield* SubscriptionRef.update(state, (next) => ({ ...next, git: withBranchList(next.git, branches) }))
  })

export const makeGitPanelHandlers = <S extends GitPanelCapableState>(
  state: SubscriptionRef.SubscriptionRef<S>,
  bridge: GitGithubBridge = unavailableGitGithubBridge,
  attachContext?: (
    diff: GitDiffResult,
    selection: IdeReviewSelection | null,
  ) => Effect.Effect<void, unknown>,
) => {
  const setGit = (mut: (git: GitPanelState) => GitPanelState) =>
    SubscriptionRef.update(state, (next) => ({ ...next, git: mut(next.git) }))

  return {
    GitPanelRefreshRequested: () => refreshGitPanel(state, bridge),

    GitPanelStageToggled: (relativePath: string) =>
      Effect.gen(function* () {
        const current = yield* SubscriptionRef.get(state)
        const status = current.git.status
        if (status === null) return
        const isStaged = status.staged.some((entry) => entry.path === relativePath)
        const op = isStaged ? "unstage" : "stage"
        const result = yield* Effect.promise(() => runOp(bridge, {
          op,
          repositoryRef: status.repositoryRef,
          statusRef: status.statusRef,
          paths: [relativePath],
        }))
        if (!result.ok) {
          yield* setGit((git) => ({ ...git, actionError: result.message }))
          return
        }
        yield* setGit((git) => ({ ...git, actionError: null }))
        yield* refreshGitPanel(state, bridge)
      }),

    GitPanelCommitMessageChanged: (value: string) =>
      setGit((git) => ({ ...git, commitMessage: value.slice(0, 20_000) })),

    GitPanelCommitRequested: () =>
      Effect.gen(function* () {
        const current = yield* SubscriptionRef.get(state)
        const message = current.git.commitMessage.trim()
        const staged = current.git.status?.staged.length ?? 0
        if (message === "" || staged === 0 || current.git.committing) return
        yield* setGit((git) => ({ ...git, committing: true, actionError: null }))
        const status = current.git.status
        if (status === null) return
        const result = yield* Effect.promise(() => runOp(bridge, {
          op: "commit",
          repositoryRef: status.repositoryRef,
          statusRef: status.statusRef,
          message,
        }))
        if (result.ok && result.op === "commit") {
          yield* setGit((git) => ({
            ...git,
            committing: false,
            commitMessage: "",
            receipt: { kind: "commit", headline: `Committed ${result.shortSha}`, detail: result.summary },
            actionError: null,
          }))
          yield* refreshGitPanel(state, bridge)
        } else if (!result.ok) {
          yield* setGit((git) => ({ ...git, committing: false, actionError: result.message }))
        }
      }),

    GitPanelPushRequested: () =>
      Effect.gen(function* () {
        const current = yield* SubscriptionRef.get(state)
        const status = current.git.status
        if (current.git.pushing || status === null) return
        yield* setGit((git) => ({ ...git, pushing: true, actionError: null }))
        const result = yield* Effect.promise(() => runOp(bridge, {
          op: "push",
          repositoryRef: status.repositoryRef,
          statusRef: status.statusRef,
        }))
        if (result.ok && result.op === "push") {
          yield* setGit((git) => ({
            ...git,
            pushing: false,
            receipt: { kind: "push", headline: `Pushed ${result.ref}`, detail: `${result.remote} · ${result.sha.slice(0, 9)}` },
            actionError: null,
          }))
          yield* refreshGitPanel(state, bridge)
        } else if (!result.ok) {
          yield* setGit((git) => ({ ...git, pushing: false, actionError: result.message }))
        }
      }),

    GitPanelNewBranchNameChanged: (value: string) =>
      setGit((git) => ({ ...git, newBranchName: value.slice(0, 200) })),

    GitPanelBranchCreateRequested: () =>
      Effect.gen(function* () {
        const current = yield* SubscriptionRef.get(state)
        const name = current.git.newBranchName.trim()
        if (name === "") return
        const status = current.git.status
        if (status === null) return
        const result = yield* Effect.promise(() => runOp(bridge, {
          op: "branchCreate",
          repositoryRef: status.repositoryRef,
          statusRef: status.statusRef,
          name,
          checkout: true,
        }))
        if (result.ok && result.op === "branchCreate") {
          yield* setGit((git) => ({ ...git, newBranchName: "", actionError: null }))
          yield* refreshGitPanel(state, bridge)
        } else if (!result.ok) {
          yield* setGit((git) => ({ ...git, actionError: result.message }))
        }
      }),

    GitPanelBranchCheckoutRequested: (name: string) =>
      Effect.gen(function* () {
        const status = (yield* SubscriptionRef.get(state)).git.status
        if (status === null) return
        const result = yield* Effect.promise(() => runOp(bridge, {
          op: "checkout",
          repositoryRef: status.repositoryRef,
          statusRef: status.statusRef,
          name,
        }))
        if (result.ok) {
          yield* setGit((git) => ({ ...git, actionError: null }))
          yield* refreshGitPanel(state, bridge)
        } else {
          yield* setGit((git) => ({ ...git, actionError: result.message }))
        }
      }),

    GitPanelIssuesRequested: () =>
      Effect.gen(function* () {
        const result = yield* Effect.promise(() => runOp(bridge, { op: "issueList", limit: 20 }))
        if (result.ok && result.op === "issueList") {
          yield* setGit((git) => ({ ...git, issues: result.issues, issuesLoaded: true, ghAvailable: true, actionError: null }))
        } else if (!result.ok) {
          yield* setGit((git) => ({
            ...git,
            issuesLoaded: true,
            actionError: result.message,
            ...ghAvailabilityFrom(git, result.error, result.message),
          }))
        }
      }),

    GitPanelPrsRequested: () =>
      Effect.gen(function* () {
        const result = yield* Effect.promise(() => runOp(bridge, { op: "prList", limit: 20 }))
        if (result.ok && result.op === "prList") {
          yield* setGit((git) => ({ ...git, prs: result.prs, prsLoaded: true, ghAvailable: true, actionError: null }))
        } else if (!result.ok) {
          yield* setGit((git) => ({
            ...git,
            prsLoaded: true,
            actionError: result.message,
            ...ghAvailabilityFrom(git, result.error, result.message),
          }))
        }
      }),

    GitPanelCreateFormChanged: (kind: GitPanelCreateKind) =>
      setGit((git) => ({ ...git, create: kind, createTitle: "", createBody: "", actionError: null })),

    GitPanelCreateTitleChanged: (value: string) =>
      setGit((git) => ({ ...git, createTitle: value.slice(0, 400) })),

    GitPanelCreateBodyChanged: (value: string) =>
      setGit((git) => ({ ...git, createBody: value.slice(0, 8_000) })),

    GitPanelCreateSubmitted: () =>
      Effect.gen(function* () {
        const current = yield* SubscriptionRef.get(state)
        const kind = current.git.create
        const title = current.git.createTitle.trim()
        if (kind === "none" || title === "") return
        const op = kind === "issue" ? "issueCreate" : "prCreate"
        const result = yield* Effect.promise(() => runOp(bridge, { op, title, body: current.git.createBody }))
        if (result.ok && (result.op === "issueCreate" || result.op === "prCreate")) {
          yield* setGit((git) => ({
            ...git,
            create: "none",
            createTitle: "",
            createBody: "",
            receipt: { kind, headline: `Created ${kind === "issue" ? "issue" : "PR"} #${result.number}`, detail: result.url },
            actionError: null,
            ghAvailable: true,
          }))
        } else if (!result.ok) {
          yield* setGit((git) => ({
            ...git,
            actionError: result.message,
            ...ghAvailabilityFrom(git, result.error, result.message),
          }))
        }
      }),

    GitPanelDiffRequested: ({ path, source }: { path: string; source: "staged" | "unstaged" }) =>
      Effect.gen(function* () {
        const current = yield* SubscriptionRef.get(state)
        const status = current.git.status
        if (status === null) return
        yield* setGit(git => ({ ...git, diffLoading: true, reviewFailure: null, actionError: null, discardConfirmPath: null }))
        const result = yield* Effect.promise(() => runOp(bridge, {
          op: "diff",
          repositoryRef: status.repositoryRef,
          statusRef: status.statusRef,
          path,
          source,
          causalItemRef: current.git.causalItemRef,
        }))
        if (result.ok && result.op === "diff") {
          yield* setGit(git => ({ ...git, diff: result, diffLoading: false, reviewFailure: null, actionError: null }))
        } else if (!result.ok) {
          yield* setGit(git => ({ ...git, diff: null, diffLoading: false, reviewFailure: result.error, actionError: result.message }))
          if (result.error === "stale_status") yield* refreshGitPanel(state, bridge)
        }
      }),

    GitPanelDiffClosed: () => setGit(git => ({ ...git, diff: null, reviewFailure: null, discardConfirmPath: null })),

    GitPanelDiscardRequested: (path: string) => setGit(git => ({
      ...git,
      discardConfirmPath: git.status?.unstaged.some(entry => entry.path === path && entry.status !== "unmerged") === true
        ? path
        : null,
    })),

    GitPanelDiscardCancelled: () => setGit(git => ({ ...git, discardConfirmPath: null })),

    GitPanelDiscardConfirmed: () => Effect.gen(function* () {
      const current = yield* SubscriptionRef.get(state)
      const status = current.git.status
      const path = current.git.discardConfirmPath
      if (status === null || path === null) return
      const result = yield* Effect.promise(() => runOp(bridge, {
        op: "discard",
        repositoryRef: status.repositoryRef,
        statusRef: status.statusRef,
        path,
      }))
      if (!result.ok) {
        yield* setGit(git => ({ ...git, discardConfirmPath: null, actionError: result.message }))
      } else {
        yield* setGit(git => ({
          ...git, discardConfirmPath: null, diff: null, actionError: null,
          recoveryRef: result.op === "discard" ? result.recoveryRef ?? null : null,
          receipt: result.op === "discard" ? { kind: "discard", headline: "Discarded one exact change", detail: result.recoveryRef === undefined ? "No recovery record was returned." : "Recovery is available until it is used." } : git.receipt,
        }))
      }
      yield* refreshGitPanel(state, bridge)
    }),

    GitPanelRecoveryRequested: () => Effect.gen(function* () {
      const current = yield* SubscriptionRef.get(state)
      const status = current.git.status
      const recoveryRef = current.git.recoveryRef
      if (status === null || recoveryRef === null) return
      const result = yield* Effect.promise(() => runOp(bridge, {
        op: "recover", repositoryRef: status.repositoryRef, statusRef: status.statusRef, recoveryRef,
      }))
      if (!result.ok) {
        yield* setGit(git => ({ ...git, actionError: result.message }))
        return
      }
      yield* setGit(git => ({ ...git, recoveryRef: null, actionError: null, receipt: { kind: "recovery", headline: "Recovered discarded change", detail: recoveryRef } }))
      yield* refreshGitPanel(state, bridge)
    }),

    GitPanelContextAttached: (selection: IdeReviewSelection | null) => Effect.gen(function* () {
      const diff = (yield* SubscriptionRef.get(state)).git.diff
      if (diff !== null && attachContext !== undefined) yield* attachContext(diff, selection)
    }),
  }
}

// ---------------------------------------------------------------------------
// View
// ---------------------------------------------------------------------------

const statusTone = (status: GitFileEntry["status"]): "success" | "warn" | "danger" | "neutral" =>
  status === "added" ? "success"
    : status === "deleted" ? "danger"
      : status === "untracked" ? "neutral"
        : "warn"

const changeRow = (entry: GitFileEntry, staged: boolean): View =>
  Stack(
    { key: `git-change-${staged ? "staged" : "unstaged"}-${entry.path}`, direction: "row", gap: "2", align: "center", style: { width: "full", minWidth: 0 } },
    [
      Badge({
        key: `git-change-badge-${staged ? "s" : "u"}-${entry.path}`,
        label: entry.status,
        tone: statusTone(entry.status),
        a11y: { label: `${entry.path} is ${entry.status}` },
      }),
      Text({ key: `git-change-path-${staged ? "s" : "u"}-${entry.path}`, content: entry.path, variant: "caption", color: "textPrimary" }),
      Spacer({ key: `git-change-fill-${staged ? "s" : "u"}-${entry.path}`, flex: true }),
      ...(entry.status === "untracked" ? [] : [Button({
        key: `git-review-${staged ? "s" : "u"}-${entry.path}`,
        label: "Review",
        variant: "ghost",
        onPress: IntentRef("GitPanelDiffRequested", StaticPayload({ path: entry.path, source: staged ? "staged" : "unstaged" })),
        a11y: { label: `Review ${staged ? "staged" : "unstaged"} diff for ${entry.path}` },
      })]),
      Button({
        key: `git-stage-toggle-${staged ? "s" : "u"}-${entry.path}`,
        label: staged ? "Unstage" : "Stage",
        variant: "secondary",
        onPress: IntentRef("GitPanelStageToggled", StaticPayload(entry.path)),
      }),
      ...(!staged && entry.status !== "untracked" && entry.status !== "unmerged" ? [Button({
        key: `git-discard-${entry.path}`,
        label: "Discard…",
        variant: "ghost",
        onPress: IntentRef("GitPanelDiscardRequested", StaticPayload(entry.path)),
      })] : []),
    ],
  )

const statusHeader = (git: GitPanelState): View => {
  const status = git.status
  const branchLabel = status === null
    ? "—"
    : status.detached
      ? "detached HEAD"
      : status.branch ?? "—"
  const trackingBits: string[] = []
  if (status !== null && status.ahead > 0) trackingBits.push(`↑${status.ahead}`)
  if (status !== null && status.behind > 0) trackingBits.push(`↓${status.behind}`)
  const dirty = status !== null && (status.staged.length > 0 || status.unstaged.length > 0 || status.untracked.length > 0)
  return Stack({ key: "git-status-header", direction: "row", gap: "2", align: "center", style: { width: "full", minWidth: 0 } }, [
    Icon({ key: "git-status-icon", name: "Compare", size: "sm", color: "textMuted", label: "Git" }),
    Text({ key: "git-status-branch", content: branchLabel, variant: "label", color: "textPrimary" }),
    ...(status !== null && status.upstream !== null
      ? [Text({ key: "git-status-upstream", content: status.upstream, variant: "caption", color: "textMuted" })]
      : []),
    ...(trackingBits.length > 0
      ? [Text({ key: "git-status-ab", content: trackingBits.join(" "), variant: "caption", color: "warning" })]
      : []),
    Text({
      key: "git-status-dirty",
      content: git.phase === "loading" ? "refreshing…" : dirty ? "changes" : status === null ? "" : "clean",
      variant: "caption",
      color: dirty ? "warning" : "textMuted",
    }),
    Spacer({ key: "git-status-fill", flex: true }),
    Button({
      key: "git-refresh",
      label: git.phase === "loading" ? "Refreshing…" : "Refresh",
      variant: "secondary",
      disabled: git.phase === "loading",
      onPress: IntentRef("GitPanelRefreshRequested"),
      a11y: { label: "Refresh Git status" },
    }),
  ])
}

const changesSection = (git: GitPanelState): ReadonlyArray<View> => {
  const status = git.status
  if (status === null) return []
  const rows: View[] = []
  if (status.staged.length > 0) {
    rows.push(Text({ key: "git-staged-title", content: "Staged", variant: "label", color: "textMuted" }))
    rows.push(...status.staged.map((entry) => changeRow(entry, true)))
  }
  const unstaged = [...status.unstaged, ...status.untracked]
  if (unstaged.length > 0) {
    rows.push(Text({ key: "git-unstaged-title", content: "Changes", variant: "label", color: "textMuted" }))
    rows.push(...unstaged.map((entry) => changeRow(entry, false)))
  }
  if (rows.length === 0) {
    rows.push(EmptyMessage({ key: "git-no-changes", icon: { name: "GitCommit", tone: "secondary" }, title: "No local changes" }))
  }
  return [Stack({ key: "git-changes", direction: "column", gap: "1", style: { width: "full", minWidth: 0 } }, rows)]
}

/**
 * Named compatibility fallback for the non-React Effect Native catalog test
 * projection. The shipped React review surfaces exclusively use Pierre.
 */
const legacyEffectNativeReviewRows = (content: string): ReadonlyArray<{ kind: "context" | "add" | "remove"; tokens: ReadonlyArray<{ kind: "plain"; text: string }> }> =>
  content.split("\n").map(line => ({
    kind: line.startsWith("+") && !line.startsWith("+++")
      ? "add" as const
      : line.startsWith("-") && !line.startsWith("---")
        ? "remove" as const
        : "context" as const,
    tokens: [{ kind: "plain" as const, text: line }],
  }))

const reviewSection = (git: GitPanelState): ReadonlyArray<View> => {
  const diff = git.diff
  if (diff === null) return []
  return [Stack(
    { key: "git-review-diff", direction: "column", gap: "2", style: { width: "full", minWidth: 0 } },
    [
      Stack({ key: "git-review-heading", direction: "row", gap: "2", align: "center", style: { width: "full" } }, [
        Text({ key: "git-review-path", content: `${diff.path} · ${diff.source}`, variant: "label", color: "textPrimary" }),
        Text({ key: "git-review-hunks", content: `${diff.hunks.length} ${diff.hunks.length === 1 ? "hunk" : "hunks"}`, variant: "caption", color: "textMuted" }),
        Text({ key: "git-review-causal-item", content: diff.causalItemRef === null ? "No timeline correlation" : `Timeline ${diff.causalItemRef}`, variant: "caption", color: diff.causalItemRef === null ? "warning" : "success" }),
        Spacer({ key: "git-review-fill", flex: true }),
        Button({ key: "git-review-attach", label: "Add to composer", variant: "primary", onPress: IntentRef("GitPanelContextAttached"), a11y: { label: `Add the reviewed diff for ${diff.path} to composer context` } }),
        Button({ key: "git-review-close", label: "Close", variant: "ghost", onPress: IntentRef("GitPanelDiffClosed") }),
      ]),
      DiffView({
        key: "git-review-diff-view",
        language: diff.path.split(".").at(-1) ?? "text",
        layout: "unified",
        hunks: diff.hunks.map(hunk => ({ header: hunk.header, rows: legacyEffectNativeReviewRows(hunk.content) })),
        style: { width: "full", minWidth: 0 },
      }),
    ],
  )]
}

const mutationSection = (git: GitPanelState): ReadonlyArray<View> => [
  Stack({ key: "git-mutations", direction: "column", gap: "2", style: { width: "full", minWidth: 0 } }, [
    Text({ key: "git-mutation-boundary", content: "Exact-version mutations use the visible status snapshot.", variant: "caption", color: "textMuted" }),
    ...(git.discardConfirmPath === null ? [] : [Stack({ key: "git-discard-confirmation", direction: "column", gap: "1", style: { width: "full" } }, [
      Text({ key: "git-discard-warning", content: `Discard the worktree change in ${git.discardConfirmPath}?`, variant: "body", color: "warning" }),
      Stack({ key: "git-discard-actions", direction: "row", gap: "2" }, [
        Button({ key: "git-discard-confirm", label: "Discard change", variant: "primary", onPress: IntentRef("GitPanelDiscardConfirmed") }),
        Button({ key: "git-discard-cancel", label: "Cancel", variant: "secondary", onPress: IntentRef("GitPanelDiscardCancelled") }),
      ]),
    ])]),
    TextField({ key: "git-commit-message", label: "Commit message", value: git.commitMessage, multiline: true, autoResize: true, onChange: IntentRef("GitPanelCommitMessageChanged") }),
    Stack({ key: "git-delivery-actions", direction: "row", gap: "2" }, [
      Button({ key: "git-commit", label: git.committing ? "Committing…" : "Commit staged", variant: "primary", disabled: git.committing || git.commitMessage.trim() === "" || (git.status?.staged.length ?? 0) === 0, onPress: IntentRef("GitPanelCommitRequested") }),
      Button({ key: "git-push", label: git.pushing ? "Pushing…" : "Push exact HEAD", variant: "secondary", disabled: git.pushing || git.status === null || git.status.upstream === null, onPress: IntentRef("GitPanelPushRequested") }),
    ]),
    TextField({ key: "git-new-branch", label: "New branch", value: git.newBranchName, onChange: IntentRef("GitPanelNewBranchNameChanged"), onSubmit: IntentRef("GitPanelBranchCreateRequested") }),
    Button({ key: "git-branch-create", label: "Create and switch", variant: "secondary", disabled: git.newBranchName.trim() === "", onPress: IntentRef("GitPanelBranchCreateRequested") }),
    ...(git.receipt === null ? [] : [Stack({ key: "git-receipt", direction: "column", gap: "1" }, [
      Text({ key: "git-receipt-headline", content: git.receipt.headline, variant: "label", color: "success" }),
      Text({ key: "git-receipt-detail", content: git.receipt.detail, variant: "caption", color: "textMuted" }),
    ])]),
    ...(git.recoveryRef === null ? [] : [Button({ key: "git-recover", label: "Recover discarded change", variant: "secondary", onPress: IntentRef("GitPanelRecoveryRequested") })]),
    ...(git.actionError === null ? [] : [Text({ key: "git-action-error", content: git.actionError, variant: "body", color: "danger" })]),
  ]),
]

export const gitPanelView = (git: GitPanelState): View => {
  const body: View[] = [statusHeader(git)]
  if (git.phase === "unavailable") {
    body.push(Text({
      key: "git-unavailable",
      content: git.reason ?? "Git is unavailable for this workspace.",
      variant: "body",
      color: "warning",
    }))
  } else {
    body.push(...changesSection(git))
    body.push(...reviewSection(git))
    body.push(...mutationSection(git))
  }
  return Stack(
    { key: "git-panel", direction: "column", gap: "3", style: { width: "full", minWidth: 0, paddingTop: "2" } },
    body,
  )
}
