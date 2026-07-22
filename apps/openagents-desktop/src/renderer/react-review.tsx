import { Alert, AlertDescription, AlertTitle } from "#components/ui/alert"
import { Badge } from "#components/ui/badge"
import { Button } from "#components/ui/button"
import {
  Sheet, SheetContent, SheetDescription, SheetHeader, SheetTitle,
} from "#components/ui/sheet"
import { ComponentValueBinding, IntentRef, type IntentError, type IntentReporter, type JsonPayload } from "@effect-native/core"
import { Effect } from "@effect-native/core/effect"
import { useEffect, useState, type ReactElement, type RefObject } from "react"

import {
  CODEX_CHIP_REASON_NO_VERIFIED_ACCOUNT,
  CODEX_CHIP_REASON_POLICY_DENIED,
  CODEX_CHIP_REASON_QUOTA_EXHAUSTED,
  CODEX_CHIP_REASON_RATE_LIMITED,
  CODEX_CHIP_REASON_VERIFYING,
} from "../codex-local-contract.ts"
import type { GitFileEntry, GitGithubErrorCode } from "../git-github-contract.ts"
import { projectAgentConversationRunLinks } from "./agent-identity.ts"
import type { GitPanelState } from "./git-panel.ts"
import type { DesktopShellState } from "./shell.ts"
import { PierreReviewAdapter } from "../ide/pierre-diffs-adapter.tsx"
import type { IdeReviewIntent, IdeReviewSelection } from "../ide/review-contract.ts"
import { activeGitReviewSource } from "./ide/review-source.ts"
import { AgentProposalList, AgentProposalReviewPanel } from "./react-agent-code.tsx"

const dispatch = (report: IntentReporter, name: string, payload: JsonPayload = null): void => {
  void Effect.runPromise(report(
    payload === null ? IntentRef(name) : IntentRef(name, ComponentValueBinding()), payload,
  ) as Effect.Effect<void, IntentError>).catch(() => {})
}

export type ReactStatusKind =
  | "signed_out"
  | "incompatible"
  | "offline"
  | "quota_exhausted"
  | "rate_limited"
  | "policy_denied"
  | "revoked_grant"
  | "stream_gap"
  | "interrupted"
  | "failed"
  | "invalid_config"
  /** META-1 (#9180): a Full Auto run bound to this conversation, linked to
   * the existing read-only run view. Observability, never an alert. */
  | "full_auto_run"

export type ReactStatusNotice = Readonly<{
  key: string
  kind: ReactStatusKind
  title: string
  detail: string
  action: Readonly<{ label: string; intent: string; payload: JsonPayload }> | null
}>

const codexLaneNotice = (state: DesktopShellState): ReactStatusNotice | null => {
  const lane = state.harnessLanes.codex
  if (lane.available || lane.reason === null) return null
  if (lane.diagnostic?.kind === "invalid_config") {
    return {
      key: "codex-config-invalid",
      kind: "invalid_config",
      title: "Codex configuration error",
      detail: lane.diagnostic.detail,
      action: null,
    }
  }
  // Initial provider probing is passive readiness, not an alert. Keep it in
  // the composer status control (the same hierarchy used by the reference
  // workbench) so startup remains calm and the empty state stays primary.
  if (lane.reason === CODEX_CHIP_REASON_VERIFYING) return null
  if (lane.reason === CODEX_CHIP_REASON_QUOTA_EXHAUSTED) {
    return { key: "codex-quota", kind: "quota_exhausted", title: "Codex quota exhausted", detail: lane.reason, action: null }
  }
  if (lane.reason === CODEX_CHIP_REASON_RATE_LIMITED) {
    return { key: "codex-rate", kind: "rate_limited", title: "Codex is rate limited", detail: lane.reason, action: null }
  }
  if (lane.reason === CODEX_CHIP_REASON_POLICY_DENIED) {
    return { key: "codex-policy", kind: "policy_denied", title: "Blocked by policy", detail: lane.reason, action: null }
  }
  if (lane.reason === CODEX_CHIP_REASON_NO_VERIFIED_ACCOUNT) {
    return {
      key: "codex-sign-in", kind: "signed_out", title: "Codex sign-in required", detail: lane.reason,
      action: { label: "Open Settings", intent: "DesktopSettingsToggled", payload: null },
    }
  }
  return { key: "codex-failed", kind: "failed", title: "Codex is unavailable", detail: lane.reason, action: null }
}

/** Pure presentation over typed shell projections; no raw runtime/provider parsing. */
export const projectReactStatusNotices = (state: DesktopShellState): ReadonlyArray<ReactStatusNotice> => {
  const notices: ReactStatusNotice[] = []
  const lane = codexLaneNotice(state)
  if (lane !== null) notices.push(lane)
  const runtimeFailure = state.runtimeFailure
  if (runtimeFailure !== null && !notices.some(notice => notice.kind === runtimeFailure)) {
    const activeProvider = state.providerLaneCapabilities
      .find(candidate => candidate.laneRef === state.activeLaneRef)?.displayName
      ?? (state.selectedHarness === "claude" ? "Claude" : "Codex")
    const copy: Record<typeof runtimeFailure, Readonly<{ title: string; detail: string }>> = {
      signed_out: { title: `${activeProvider} sign-in required`, detail: `The admitted turn reported that its ${activeProvider} account is unavailable. No alternate account was selected.` },
      incompatible: { title: "Codex workflow incompatible", detail: "The installed runtime cannot perform this workflow. The turn was not rerouted." },
      offline: { title: "Conversation connection offline", detail: "The local conversation subscription is unavailable. No provider failure is being claimed." },
      quota_exhausted: { title: "Codex quota exhausted", detail: "The admitted account reported exhausted quota. No alternate account was selected." },
      rate_limited: { title: "Codex is rate limited", detail: "The admitted account reported a transient rate limit. Retry only after the lane becomes available." },
      policy_denied: { title: "Blocked by policy", detail: "The admitted turn was denied by policy; this is not an authentication failure." },
      interrupted: { title: "Turn interrupted", detail: "The runtime recorded an interrupted outcome. It was not silently resumed." },
      failed: { title: "Turn failed", detail: "The runtime recorded a failed outcome. Retry only when the command is safe to repeat." },
    }
    notices.push({ key: `runtime:${runtimeFailure}`, kind: runtimeFailure, ...copy[runtimeFailure], action: null })
  }
  if (state.workspaceBrowser.phase === "unavailable") {
    notices.push({
      key: "workspace-revoked",
      kind: "revoked_grant",
      title: "Repository access unavailable",
      detail: state.workspaceBrowser.reason ?? "The workspace grant is no longer available.",
      action: { label: "Choose workspace", intent: "DesktopWorkspacePickerRequested", payload: null },
    })
  }
  const page = state.history.page
  if (page !== null && (page.completeness.gaps > 0 || !page.completeness.complete)) {
    notices.push({
      key: `gap:${page.selectedThreadRef}`,
      kind: "stream_gap",
      title: "Repairing conversation history",
      detail: "A durable history gap is present. This is not a completed turn; repair precedes live updates.",
      action: null,
    })
  }
  const selected = page?.agents.find(agent => agent.threadRef === page.selectedThreadRef)
  if (selected?.status === "interrupted") {
    notices.push({ key: `interrupted:${selected.threadRef}`, kind: "interrupted", title: "Turn interrupted", detail: "The runtime recorded an interrupted outcome. It was not silently resumed.", action: null })
  } else if (selected?.status === "errored") {
    notices.push({ key: `failed:${selected.threadRef}`, kind: "failed", title: "Turn failed", detail: "The runtime recorded a failed outcome. Retry only when the command is safe to repeat.", action: null })
  }
  // META-1 (#9180): delegated Full Auto work bound to this conversation stays
  // attributed inside it — a linked run card naming the run, its state, and
  // its lane, opening the EXISTING read-only run view. Presentation over the
  // same run-list projection the dedicated Full Auto surface reads; no new
  // authority and no new intent.
  for (const link of projectAgentConversationRunLinks(state.fullAuto.runs, state.activeThreadId)) {
    notices.push({
      key: `full-auto-run:${link.runRef}`,
      kind: "full_auto_run",
      title: `Full Auto run · ${link.statusLabel}`,
      detail: link.lane === null ? link.title : `${link.title} · via ${link.lane}`,
      action: { label: "Open run", intent: "DesktopFullAutoRunOpened", payload: link.runRef },
    })
  }
  return notices
}

export const StatusNotices = ({ state, report }: {
  readonly state: DesktopShellState
  readonly report: IntentReporter
}): ReactElement | null => {
  const notices = projectReactStatusNotices(state)
  if (notices.length === 0) return null
  return <section className="oa-react-status-notices" aria-label="Session status notices" aria-live="polite">
    {notices.map(notice => <Alert key={notice.key} data-status-kind={notice.kind}
      variant={["failed", "policy_denied", "revoked_grant", "invalid_config"].includes(notice.kind) ? "destructive" : "default"}>
      <AlertTitle>{notice.title}</AlertTitle>
      <AlertDescription>{notice.detail}</AlertDescription>
      {notice.action === null ? null : <Button type="button" variant="outline" size="sm"
        onClick={() => dispatch(report, notice.action!.intent, notice.action!.payload)}>{notice.action.label}</Button>}
    </Alert>)}
  </section>
}

const reviewFailureCopy: Record<GitGithubErrorCode, Readonly<{ title: string; detail: string }>> = {
  invalid_request: { title: "Review request refused", detail: "The bounded review request was invalid. Refresh status before retrying." },
  no_workspace: { title: "No workspace", detail: "Choose a workspace before requesting repository review." },
  not_a_repo: { title: "Repository unavailable", detail: "The current WorkContext is not a Git repository." },
  git_unavailable: { title: "Git unavailable", detail: "Git is not available for this WorkContext." },
  empty_message: { title: "Review unavailable", detail: "This mutation-only failure is not actionable from read-only review." },
  nothing_staged: { title: "Review unavailable", detail: "No staged changes are available." },
  no_upstream: { title: "Review unavailable", detail: "No upstream is configured." },
  non_fast_forward: { title: "Review conflict", detail: "Repository state conflicts with the requested operation. Refresh before continuing." },
  auth_failed: { title: "Repository authentication failed", detail: "Review cannot access the repository authority." },
  blocked_by_hook: { title: "Blocked by policy", detail: "Repository policy refused the operation." },
  dirty_tree: { title: "Repository conflict", detail: "The repository changed. Refresh status before requesting the exact diff again." },
  stale_status: { title: "Stale review", detail: "The status snapshot changed. Status was refreshed; choose the file again." },
  unsafe_state: { title: "Unsafe repository state", detail: "Review refused this conflicting repository state." },
  binary_diff: { title: "Binary file", detail: "Binary content is not rendered in the review drawer." },
  secret_diff: { title: "Secret-shaped content", detail: "Potentially sensitive diff content was withheld." },
  diff_too_large: { title: "Diff too large", detail: "The exact diff exceeds the bounded review policy." },
  branch_exists: { title: "Review unavailable", detail: "This branch mutation failure is outside read-only review." },
  invalid_branch_name: { title: "Review unavailable", detail: "This branch mutation failure is outside read-only review." },
  invalid_path: { title: "Invalid review path", detail: "The relative path did not pass the bounded review policy." },
  gh_unavailable: { title: "GitHub unavailable", detail: "GitHub is not required for local read-only review." },
  gh_unauthenticated: { title: "GitHub signed out", detail: "GitHub sign-in is not required for local read-only review." },
  not_found: { title: "Review not found", detail: "The correlated review target no longer exists." },
  operation_failed: { title: "Review failed", detail: "The host returned a bounded review failure. Refresh status before retrying." },
}

export const reviewFailurePresentation = (failure: GitGithubErrorCode | null) =>
  failure === null ? null : reviewFailureCopy[failure]

const fileRows = (git: GitPanelState): ReadonlyArray<Readonly<{ entry: GitFileEntry; source: "staged" | "unstaged" }>> => {
  const status = git.status
  if (status === null) return []
  return [
    ...status.staged.map(entry => ({ entry, source: "staged" as const })),
    ...status.unstaged.map(entry => ({ entry, source: "unstaged" as const })),
    ...status.untracked.map(entry => ({ entry, source: "unstaged" as const })),
  ]
}

const ReviewBody = ({ state, report }: { readonly state: DesktopShellState; readonly report: IntentReporter }): ReactElement => {
  const git = state.git
  const rows = fileRows(git)
  const failure = reviewFailurePresentation(git.reviewFailure)
  const [layout, setLayout] = useState<"unified" | "split">("unified")
  const [contextLines, setContextLines] = useState(20)
  const [selection, setSelection] = useState<IdeReviewSelection | null>(null)
  const reviewSource = activeGitReviewSource(state)
  const onReviewIntent = (intent: IdeReviewIntent): void => {
    if (intent.action === "select") setSelection(intent.selection)
  }
  const openInEditor = (): void => {
    const identity = state.workspaceBrowser.pathIndexSnapshot?.identity
    if (reviewSource === null || reviewSource.pathRef === null || state.workspaceBrowser.grantRef === null || identity === undefined) return
    dispatch(report, "WorkspaceEditorOpenRequested", {
      grantRef: state.workspaceBrowser.grantRef,
      pathRef: reviewSource.pathRef,
      source: "review",
      identity,
    })
  }
  if (state.agentReviewProposalRef !== null) return <div className="oa-react-review-scroll">
    <AgentProposalReviewPanel state={state} report={report} />
  </div>
  return <div className="oa-react-review-scroll">
    <AgentProposalList state={state} report={report} />
    <p className="oa-react-readonly-boundary">Exact-version source control · every mutation is fenced to the visible repository snapshot</p>
    {git.phase === "loading" || git.diffLoading ? <p role="status">Loading exact repository snapshot…</p> : null}
    {git.phase === "unavailable" ? <Alert variant="destructive"><AlertTitle>Repository review unavailable</AlertTitle><AlertDescription>{git.reason ?? "No bounded status is available."}</AlertDescription></Alert> : null}
    {failure === null ? null : <Alert variant="destructive" data-review-failure={git.reviewFailure ?? undefined}><AlertTitle>{failure.title}</AlertTitle><AlertDescription>{failure.detail}</AlertDescription></Alert>}
    {git.diff === null ? <section className="oa-react-review-files" aria-label="Repository changes">
      {rows.length === 0 && git.phase === "ready" ? <p>No local changes.</p> : rows.map(({ entry, source }) => <div className="oa-react-review-file" key={`${source}:${entry.path}`}>
        <Badge variant="outline">{entry.status}</Badge><span>{entry.path}</span>
        {entry.status === "untracked" ? <small>Review is available after staging creates an index image.</small> : <Button type="button" variant="ghost" size="sm"
          onClick={() => dispatch(report, "GitPanelDiffRequested", { path: entry.path, source })}>Review</Button>}
        <Button type="button" variant="outline" size="sm" onClick={() => dispatch(report, "GitPanelStageToggled", entry.path)}>
          {source === "staged" ? "Unstage" : "Stage"}
        </Button>
        {source === "unstaged" && entry.status !== "untracked" && entry.status !== "unmerged" ? <Button type="button" variant="destructive" size="sm"
          onClick={() => dispatch(report, "GitPanelDiscardRequested", entry.path)}>Discard…</Button> : null}
      </div>)}
    </section> : <section className="oa-react-exact-diff" aria-label={`Read-only diff for ${git.diff.path}`}>
      <header><strong>{git.diff.path}</strong><Badge variant="secondary">{reviewSource?._tag ?? git.diff.source}</Badge>
        <span>{git.diff.hunks.length} {git.diff.hunks.length === 1 ? "hunk" : "hunks"}</span></header>
      <p>{git.diff.causalItemRef === null ? "No timeline item correlation" : `Timeline item ${git.diff.causalItemRef}`}</p>
      {reviewSource === null ? <Alert variant="destructive"><AlertTitle>Version identity unavailable</AlertTitle><AlertDescription>Refresh Files and Git status before rendering this exact diff.</AlertDescription></Alert> : <>
        <p className="oa-react-review-source-label"><strong>{reviewSource.base.label}</strong><span aria-hidden="true"> → </span><strong>{reviewSource.target.label}</strong></p>
        <div className="oa-react-review-toolbar" role="toolbar" aria-label="Diff layout and context">
          <Button type="button" size="sm" variant={layout === "unified" ? "secondary" : "ghost"} aria-pressed={layout === "unified"} onClick={() => setLayout("unified")}>Unified</Button>
          <Button type="button" size="sm" variant={layout === "split" ? "secondary" : "ghost"} aria-pressed={layout === "split"} onClick={() => setLayout("split")}>Split</Button>
          <Button type="button" size="sm" variant="ghost" disabled={contextLines <= 5} onClick={() => setContextLines(value => Math.max(5, value - 5))}>Less context</Button>
          <span aria-live="polite">{contextLines} context lines</span>
          <Button type="button" size="sm" variant="ghost" disabled={contextLines >= 100} onClick={() => setContextLines(value => Math.min(100, value + 5))}>More context</Button>
          <Button type="button" size="sm" variant="ghost" disabled={reviewSource.pathRef === null || state.workspaceBrowser.grantRef === null} onClick={openInEditor}>Open in editor</Button>
          <Button type="button" size="sm" variant="outline" onClick={() => dispatch(report, "GitPanelContextAttached", selection)}>Add {selection === null ? "diff" : "selection"} to composer</Button>
        </div>
        <PierreReviewAdapter source={reviewSource} options={{ mode: layout, contextLines, selection: null, annotations: [] }} onIntent={onReviewIntent} />
      </>}
    </section>}
    {git.discardConfirmPath === null ? null : <Alert variant="destructive" data-git-discard-confirm>
      <AlertTitle>Discard the visible worktree change?</AlertTitle>
      <AlertDescription>This restores only {git.discardConfirmPath} from the exact status snapshot. Staged and conflicted changes are refused.</AlertDescription>
      <div className="oa-react-review-actions">
        <Button type="button" variant="destructive" onClick={() => dispatch(report, "GitPanelDiscardConfirmed")}>Discard change</Button>
        <Button type="button" variant="outline" onClick={() => dispatch(report, "GitPanelDiscardCancelled")}>Cancel</Button>
      </div>
    </Alert>}
    {git.actionError === null ? null : <Alert variant="destructive"><AlertTitle>Source-control operation refused</AlertTitle><AlertDescription>{git.actionError}</AlertDescription></Alert>}
    {git.receipt === null ? null : <Alert><AlertTitle>{git.receipt.headline}</AlertTitle><AlertDescription>{git.receipt.detail}</AlertDescription></Alert>}
    {git.recoveryRef === null ? null : <Button type="button" variant="outline" onClick={() => dispatch(report, "GitPanelRecoveryRequested")}>Recover discarded change</Button>}
    <section className="oa-react-review-files" aria-label="Commit and delivery">
      <label htmlFor="git-commit-message">Commit message</label>
      <textarea id="git-commit-message" value={git.commitMessage} maxLength={20_000}
        onChange={(event) => dispatch(report, "GitPanelCommitMessageChanged", event.currentTarget.value)} />
      <div className="oa-react-review-actions">
        <Button type="button" disabled={git.committing || git.commitMessage.trim() === "" || (git.status?.staged.length ?? 0) === 0}
          onClick={() => dispatch(report, "GitPanelCommitRequested")}>{git.committing ? "Committing…" : "Commit staged"}</Button>
        <Button type="button" variant="outline" disabled={git.pushing || git.status?.upstream === null}
          onClick={() => dispatch(report, "GitPanelPushRequested")}>{git.pushing ? "Pushing…" : "Push exact HEAD"}</Button>
      </div>
      <label htmlFor="git-new-branch">New branch</label>
      <input id="git-new-branch" value={git.newBranchName} maxLength={200}
        onChange={(event) => dispatch(report, "GitPanelNewBranchNameChanged", event.currentTarget.value)} />
      <div className="oa-react-review-actions">
        <Button type="button" variant="outline" disabled={git.newBranchName.trim() === ""}
          onClick={() => dispatch(report, "GitPanelBranchCreateRequested")}>Create and switch</Button>
        {git.branches.filter((branch) => !branch.current).slice(0, 20).map((branch) => <Button key={branch.name} type="button" variant="ghost"
          onClick={() => dispatch(report, "GitPanelBranchCheckoutRequested", branch.name)}>Switch to {branch.name}</Button>)}
      </div>
    </section>
    <div className="oa-react-review-actions">
      <Button type="button" variant="outline" onClick={() => dispatch(report, "GitPanelRefreshRequested")}>Refresh status</Button>
      {git.diff === null ? null : <Button type="button" variant="ghost" onClick={() => dispatch(report, "GitPanelDiffClosed")}>Back to changes</Button>}
    </div>
  </div>
}

const useWideReview = (): boolean => {
  // Kept in lockstep with the CSS media rule without masquerading as a style object.
  const query = ["(min-width", "1120px)"].join(": ")
  const [wide, setWide] = useState(() => typeof window !== "undefined" && (window.matchMedia?.(query).matches ?? false))
  useEffect(() => {
    const media = window.matchMedia?.(query)
    if (media === undefined) return
    const update = () => setWide(media.matches)
    media.addEventListener("change", update)
    return () => media.removeEventListener("change", update)
  }, [])
  return wide
}

export const ReviewSurface = ({ state, report, open, onOpenChange, triggerRef }: {
  readonly state: DesktopShellState
  readonly report: IntentReporter
  readonly open: boolean
  readonly onOpenChange: (open: boolean) => void
  readonly triggerRef: RefObject<HTMLButtonElement | null>
}): ReactElement | null => {
  const wide = useWideReview()
  const close = (): void => {
    onOpenChange(false)
    triggerRef.current?.focus()
  }
  if (!open) return null
  if (wide) return <aside className="oa-react-review-drawer" aria-label="Repository review">
    <header><div><h2>Repository review</h2><p>{state.git.currentBranch ?? state.git.status?.branch ?? "Current WorkContext"}</p></div>
      <Button type="button" variant="ghost" size="sm" onClick={close}>Close</Button></header>
    <ReviewBody state={state} report={report} />
  </aside>
  return <Sheet open={open} onOpenChange={onOpenChange}>
    <SheetContent className="oa-react-review-sheet" side="right">
      <SheetHeader><SheetTitle>Repository review</SheetTitle><SheetDescription>Exact bounded status and diff for the current WorkContext.</SheetDescription></SheetHeader>
      <ReviewBody state={state} report={report} />
    </SheetContent>
  </Sheet>
}
