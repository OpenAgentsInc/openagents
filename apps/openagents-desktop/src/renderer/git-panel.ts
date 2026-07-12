/**
 * Git / GitHub review panel (EP250 capability E2–E5, #8712).
 *
 * A typed surface over ./git-github-contract.ts, mounted inside the review
 * workspace beside the existing diff viewer. Pure Effect Native data — state,
 * typed intents, and a `state -> View` projection over the shared catalog;
 * every host response is Effect-Schema decoded (via decodeGitGithubResult) and
 * never trusted raw. Styling rides the shared tokens only (no raw colors/px —
 * the design-conformance oracle enforces it). The audit
 * (docs/fable/2026-07-11-daily-coding-capability-audit.md §4E, §6.3) ranks
 * commit/push/issue/PR flows as a daily habit with zero typed UI today; this is
 * that UI.
 *
 * Honesty: a commit shows its real SHA receipt; a push shows the pushed ref;
 * an issue/PR create shows the returned url. Disabled controls carry a
 * hover-only reason (Tooltip) — Push without an upstream, gh actions when gh is
 * missing — never a fabricated success.
 */
import {
  Badge,
  Button,
  DiffView,
  Icon,
  IntentRef,
  Spacer,
  Stack,
  StaticPayload,
  Text,
  TextField,
  Tooltip,
  ComponentValueBinding,
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

// ---------------------------------------------------------------------------
// State
// ---------------------------------------------------------------------------

export type GitPanelCreateKind = "none" | "issue" | "pr"

export type GitPanelReceipt = Readonly<{
  kind: "commit" | "push" | "issue" | "pr"
  headline: string
  detail: string
}>

export type GitPanelState = Readonly<{
  phase: "idle" | "loading" | "ready" | "unavailable"
  status: GitStatusResult | null
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
  discardConfirmPath: string | null
}>

export const emptyGitPanelState = (): GitPanelState => ({
  phase: "idle",
  status: null,
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
  discardConfirmPath: null,
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
    return { ...git, phase: "ready", status: result, reason: null }
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
export const GitPanelContextAttached = defineIntent("GitPanelContextAttached", Schema.Null)

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
  attachContext?: (diff: GitDiffResult) => Effect.Effect<void, unknown>,
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
        const result = yield* Effect.promise(() => runOp(bridge, { op, paths: [relativePath] }))
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
        const result = yield* Effect.promise(() => runOp(bridge, { op: "commit", message }))
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
        if (current.git.pushing) return
        yield* setGit((git) => ({ ...git, pushing: true, actionError: null }))
        const result = yield* Effect.promise(() => runOp(bridge, { op: "push" }))
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
        const result = yield* Effect.promise(() => runOp(bridge, { op: "branchCreate", name, checkout: true }))
        if (result.ok && result.op === "branchCreate") {
          yield* setGit((git) => ({ ...git, newBranchName: "", actionError: null }))
          yield* refreshGitPanel(state, bridge)
        } else if (!result.ok) {
          yield* setGit((git) => ({ ...git, actionError: result.message }))
        }
      }),

    GitPanelBranchCheckoutRequested: (name: string) =>
      Effect.gen(function* () {
        const result = yield* Effect.promise(() => runOp(bridge, { op: "checkout", name }))
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
        const status = (yield* SubscriptionRef.get(state)).git.status
        if (status === null) return
        yield* setGit(git => ({ ...git, diffLoading: true, actionError: null, discardConfirmPath: null }))
        const result = yield* Effect.promise(() => runOp(bridge, {
          op: "diff",
          repositoryRef: status.repositoryRef,
          statusRef: status.statusRef,
          path,
          source,
        }))
        if (result.ok && result.op === "diff") {
          yield* setGit(git => ({ ...git, diff: result, diffLoading: false, actionError: null }))
        } else if (!result.ok) {
          yield* setGit(git => ({ ...git, diff: null, diffLoading: false, actionError: result.message }))
          if (result.error === "stale_status") yield* refreshGitPanel(state, bridge)
        }
      }),

    GitPanelDiffClosed: () => setGit(git => ({ ...git, diff: null, discardConfirmPath: null })),

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
        yield* setGit(git => ({ ...git, discardConfirmPath: null, diff: null, actionError: null }))
      }
      yield* refreshGitPanel(state, bridge)
    }),

    GitPanelContextAttached: () => Effect.gen(function* () {
      const diff = (yield* SubscriptionRef.get(state)).git.diff
      if (diff !== null && attachContext !== undefined) yield* attachContext(diff)
    }),
  }
}

// ---------------------------------------------------------------------------
// View
// ---------------------------------------------------------------------------

/** Disabled controls carry a hover-only reason, matching the shell pattern. */
const withReason = (key: string, disabled: boolean, reason: string | null, control: View): View =>
  disabled && reason !== null && reason !== ""
    ? Tooltip({ key: `${key}-reason`, content: reason, placement: { side: "top", align: "start" } }, [control])
    : control

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
      ...(!staged && entry.status !== "untracked" && entry.status !== "unmerged" ? [Button({
        key: `git-discard-${entry.path}`,
        label: "Discard…",
        variant: "ghost",
        onPress: IntentRef("GitPanelDiscardRequested", StaticPayload(entry.path)),
        a11y: { label: `Discard unstaged changes in ${entry.path}` },
      })] : []),
      Button({
        key: `git-stage-toggle-${staged ? "s" : "u"}-${entry.path}`,
        label: staged ? "Unstage" : "Stage",
        variant: "ghost",
        onPress: IntentRef("GitPanelStageToggled", StaticPayload(entry.path)),
        a11y: { label: `${staged ? "Unstage" : "Stage"} ${entry.path}` },
      }),
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
    rows.push(Text({ key: "git-no-changes", content: "No local changes", variant: "body", color: "textMuted" }))
  }
  return [Stack({ key: "git-changes", direction: "column", gap: "1", style: { width: "full", minWidth: 0 } }, rows)]
}

const diffRows = (content: string): ReadonlyArray<{ kind: "context" | "add" | "remove"; tokens: ReadonlyArray<{ kind: "plain"; text: string }> }> =>
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
  const confirmation = git.discardConfirmPath === null ? [] : [Stack(
    { key: "git-discard-confirmation", direction: "row", gap: "2", align: "center", style: { width: "full" } },
    [
      Text({ key: "git-discard-warning", content: `Discard unstaged changes in ${git.discardConfirmPath}? This cannot be undone.`, variant: "body", color: "warning" }),
      Button({ key: "git-discard-confirm", label: "Discard changes", variant: "primary", onPress: IntentRef("GitPanelDiscardConfirmed"), style: { color: "danger" } }),
      Button({ key: "git-discard-cancel", label: "Cancel", variant: "ghost", onPress: IntentRef("GitPanelDiscardCancelled") }),
    ],
  )]
  if (diff === null) return confirmation
  return [...confirmation, Stack(
    { key: "git-review-diff", direction: "column", gap: "2", style: { width: "full", minWidth: 0 } },
    [
      Stack({ key: "git-review-heading", direction: "row", gap: "2", align: "center", style: { width: "full" } }, [
        Text({ key: "git-review-path", content: `${diff.path} · ${diff.source}`, variant: "label", color: "textPrimary" }),
        Text({ key: "git-review-hunks", content: `${diff.hunks.length} ${diff.hunks.length === 1 ? "hunk" : "hunks"}`, variant: "caption", color: "textMuted" }),
        Spacer({ key: "git-review-fill", flex: true }),
        Button({ key: "git-review-attach", label: "Add to composer", variant: "primary", onPress: IntentRef("GitPanelContextAttached"), a11y: { label: `Add the reviewed diff for ${diff.path} to composer context` } }),
        Button({ key: "git-review-close", label: "Close", variant: "ghost", onPress: IntentRef("GitPanelDiffClosed") }),
      ]),
      DiffView({
        key: "git-review-diff-view",
        language: diff.path.split(".").at(-1) ?? "text",
        layout: "unified",
        hunks: diff.hunks.map(hunk => ({ header: hunk.header, rows: diffRows(hunk.content) })),
        style: { width: "full", minWidth: 0 },
      }),
    ],
  )]
}

const commitBox = (git: GitPanelState): View => {
  const staged = git.status?.staged.length ?? 0
  const messageEmpty = git.commitMessage.trim() === ""
  const disabled = git.committing || staged === 0 || messageEmpty
  const reason = staged === 0 ? "Stage changes to commit" : messageEmpty ? "Enter a commit message" : null
  return Stack({ key: "git-commit-box", direction: "column", gap: "2", style: { width: "full", minWidth: 0 } }, [
    TextField({
      key: "git-commit-message",
      value: git.commitMessage,
      placeholder: "Commit message",
      disabled: git.committing,
      a11y: { label: "Commit message" },
      onChange: IntentRef("GitPanelCommitMessageChanged", ComponentValueBinding()),
      onSubmit: IntentRef("GitPanelCommitRequested"),
      style: { width: "full" },
    }),
    Stack({ key: "git-commit-row", direction: "row", gap: "2", align: "center", style: { width: "full" } }, [
      withReason(
        "git-commit",
        disabled,
        reason,
        Button({
          key: "git-commit",
          label: git.committing ? "Committing…" : "Commit",
          variant: "primary",
          disabled,
          onPress: IntentRef("GitPanelCommitRequested"),
          a11y: { label: reason ?? "Create a commit from the staged changes" },
        }),
      ),
      Spacer({ key: "git-commit-fill", flex: true }),
      pushControl(git),
    ]),
  ])
}

const pushControl = (git: GitPanelState): View => {
  const noUpstream = git.status !== null && git.status.upstream === null && !git.status.detached
  const detached = git.status?.detached ?? false
  const disabled = git.pushing || noUpstream || detached
  const reason = detached
    ? "A detached HEAD has no branch to push"
    : noUpstream
      ? "This branch has no upstream yet"
      : null
  return withReason(
    "git-push",
    disabled,
    reason,
    Button({
      key: "git-push",
      label: git.pushing ? "Pushing…" : "Push",
      variant: "secondary",
      disabled,
      onPress: IntentRef("GitPanelPushRequested"),
      a11y: { label: reason ?? "Push the current branch to its upstream" },
    }),
  )
}

const receiptRow = (git: GitPanelState): ReadonlyArray<View> => {
  const out: View[] = []
  if (git.receipt !== null) {
    out.push(Stack({ key: "git-receipt", direction: "row", gap: "2", align: "center", style: { width: "full", minWidth: 0 } }, [
      Icon({ key: "git-receipt-icon", name: "Check", size: "sm", color: "success", label: "Success" }),
      Text({ key: "git-receipt-headline", content: git.receipt.headline, variant: "caption", color: "success" }),
      Text({ key: "git-receipt-detail", content: git.receipt.detail, variant: "caption", color: "textMuted" }),
    ]))
  }
  if (git.actionError !== null) {
    out.push(Text({ key: "git-action-error", content: git.actionError, variant: "caption", color: "warning" }))
  }
  return out
}

const branchSwitcher = (git: GitPanelState): View =>
  Stack({ key: "git-branches", direction: "column", gap: "1", style: { width: "full", minWidth: 0 } }, [
    Text({ key: "git-branches-title", content: "Branches", variant: "label", color: "textMuted" }),
    Stack({ key: "git-branches-list", direction: "column", gap: "1", style: { width: "full", minWidth: 0 } },
      git.branches.slice(0, 40).map((branch) => Button({
        key: `git-branch-${branch.name}`,
        label: branch.current ? `● ${branch.name}` : branch.name,
        variant: branch.current ? "secondary" : "ghost",
        disabled: branch.current,
        onPress: IntentRef("GitPanelBranchCheckoutRequested", StaticPayload(branch.name)),
        a11y: { label: branch.current ? `On branch ${branch.name}` : `Switch to branch ${branch.name}` },
      })),
    ),
    Stack({ key: "git-branch-create-row", direction: "row", gap: "2", align: "center", style: { width: "full" } }, [
      TextField({
        key: "git-new-branch",
        value: git.newBranchName,
        placeholder: "New branch name",
        a11y: { label: "New branch name" },
        onChange: IntentRef("GitPanelNewBranchNameChanged", ComponentValueBinding()),
        onSubmit: IntentRef("GitPanelBranchCreateRequested"),
        style: { flex: 1 },
      }),
      withReason(
        "git-branch-create",
        git.newBranchName.trim() === "",
        git.newBranchName.trim() === "" ? "Enter a branch name" : null,
        Button({
          key: "git-branch-create",
          label: "Create",
          variant: "ghost",
          disabled: git.newBranchName.trim() === "",
          onPress: IntentRef("GitPanelBranchCreateRequested"),
          a11y: { label: "Create and switch to a new branch" },
        }),
      ),
    ]),
  ])

const issuePrSection = (git: GitPanelState): View => {
  const ghDisabled = git.ghAvailable === false
  const ghReason = git.ghReason
  const createButton = (kind: "issue" | "pr", label: string): View =>
    withReason(
      `git-create-${kind}`,
      ghDisabled,
      ghReason,
      Button({
        key: `git-create-${kind}`,
        label,
        variant: git.create === kind ? "secondary" : "ghost",
        disabled: ghDisabled,
        onPress: IntentRef("GitPanelCreateFormChanged", StaticPayload(git.create === kind ? "none" : kind)),
        a11y: { label: ghReason ?? `Create a new ${kind === "issue" ? "issue" : "pull request"}` },
      }),
    )
  const issueRows = git.issues.slice(0, 20).map((issue) =>
    Text({ key: `git-issue-${issue.number}`, content: `#${issue.number} · ${issue.state} · ${issue.title}`, variant: "caption", color: "textPrimary" }))
  const prRows = git.prs.slice(0, 20).map((pr) =>
    Text({ key: `git-pr-${pr.number}`, content: `#${pr.number} · ${pr.state} · ${pr.title}`, variant: "caption", color: "textPrimary" }))
  const createForm: ReadonlyArray<View> = git.create === "none"
    ? []
    : [Stack({ key: "git-create-form", direction: "column", gap: "2", style: { width: "full", minWidth: 0 } }, [
        TextField({
          key: "git-create-title",
          value: git.createTitle,
          placeholder: git.create === "issue" ? "Issue title" : "Pull request title",
          a11y: { label: git.create === "issue" ? "Issue title" : "Pull request title" },
          onChange: IntentRef("GitPanelCreateTitleChanged", ComponentValueBinding()),
          style: { width: "full" },
        }),
        TextField({
          key: "git-create-body",
          value: git.createBody,
          placeholder: "Body (optional)",
          a11y: { label: "Body" },
          onChange: IntentRef("GitPanelCreateBodyChanged", ComponentValueBinding()),
          style: { width: "full" },
        }),
        withReason(
          "git-create-submit",
          git.createTitle.trim() === "",
          git.createTitle.trim() === "" ? "Enter a title" : null,
          Button({
            key: "git-create-submit",
            label: git.create === "issue" ? "Create issue" : "Create pull request",
            variant: "primary",
            disabled: git.createTitle.trim() === "",
            onPress: IntentRef("GitPanelCreateSubmitted"),
            a11y: { label: git.create === "issue" ? "Create the issue" : "Create the pull request" },
          }),
        ),
      ])]
  return Stack({ key: "git-issues-prs", direction: "column", gap: "2", style: { width: "full", minWidth: 0 } }, [
    Stack({ key: "git-issues-prs-heading", direction: "row", gap: "2", align: "center", style: { width: "full" } }, [
      Text({ key: "git-issues-prs-title", content: "Issues & PRs", variant: "label", color: "textMuted" }),
      Spacer({ key: "git-issues-prs-fill", flex: true }),
      Button({ key: "git-load-issues", label: git.issuesLoaded ? "Reload issues" : "Load issues", variant: "ghost", onPress: IntentRef("GitPanelIssuesRequested"), a11y: { label: "Load open issues" } }),
      Button({ key: "git-load-prs", label: git.prsLoaded ? "Reload PRs" : "Load PRs", variant: "ghost", onPress: IntentRef("GitPanelPrsRequested"), a11y: { label: "Load open pull requests" } }),
      createButton("issue", "New issue"),
      createButton("pr", "New PR"),
    ]),
    ...(ghDisabled && ghReason !== null
      ? [Text({ key: "git-gh-reason", content: ghReason, variant: "caption", color: "warning" })]
      : []),
    ...createForm,
    ...(issueRows.length > 0 ? [Stack({ key: "git-issue-list", direction: "column", gap: "1", style: { width: "full", minWidth: 0 } }, issueRows)] : []),
    ...(prRows.length > 0 ? [Stack({ key: "git-pr-list", direction: "column", gap: "1", style: { width: "full", minWidth: 0 } }, prRows)] : []),
  ])
}

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
    body.push(commitBox(git))
    body.push(...receiptRow(git))
    body.push(branchSwitcher(git))
  }
  body.push(issuePrSection(git))
  return Stack(
    { key: "git-panel", direction: "column", gap: "3", style: { width: "full", minWidth: 0, paddingTop: "2" } },
    body,
  )
}
