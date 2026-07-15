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
import type { GitPanelState } from "./git-panel.ts"
import type { DesktopShellState } from "./shell.ts"

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
    const copy: Record<typeof runtimeFailure, Readonly<{ title: string; detail: string }>> = {
      signed_out: { title: "Codex sign-in required", detail: "The admitted turn reported that its Codex account is unavailable. No alternate account was selected." },
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

const ReviewBody = ({ git, report }: { readonly git: GitPanelState; readonly report: IntentReporter }): ReactElement => {
  const rows = fileRows(git)
  const failure = reviewFailurePresentation(git.reviewFailure)
  return <div className="oa-react-review-scroll">
    <p className="oa-react-readonly-boundary">Read-only review · no stage, discard, commit, branch, push, or terminal authority</p>
    {git.phase === "loading" || git.diffLoading ? <p role="status">Loading exact repository snapshot…</p> : null}
    {git.phase === "unavailable" ? <Alert variant="destructive"><AlertTitle>Repository review unavailable</AlertTitle><AlertDescription>{git.reason ?? "No bounded status is available."}</AlertDescription></Alert> : null}
    {failure === null ? null : <Alert variant="destructive" data-review-failure={git.reviewFailure ?? undefined}><AlertTitle>{failure.title}</AlertTitle><AlertDescription>{failure.detail}</AlertDescription></Alert>}
    {git.diff === null ? <section className="oa-react-review-files" aria-label="Repository changes">
      {rows.length === 0 && git.phase === "ready" ? <p>No local changes.</p> : rows.map(({ entry, source }) => <div className="oa-react-review-file" key={`${source}:${entry.path}`}>
        <Badge variant="outline">{entry.status}</Badge><span>{entry.path}</span>
        {entry.status === "untracked" ? <small>Diff unavailable until Git tracks this file.</small> : <Button type="button" variant="ghost" size="sm"
          onClick={() => dispatch(report, "GitPanelDiffRequested", { path: entry.path, source })}>Review</Button>}
      </div>)}
    </section> : <section className="oa-react-exact-diff" aria-label={`Read-only diff for ${git.diff.path}`}>
      <header><strong>{git.diff.path}</strong><Badge variant="secondary">{git.diff.source}</Badge>
        <span>{git.diff.hunks.length} {git.diff.hunks.length === 1 ? "hunk" : "hunks"}</span></header>
      <p>{git.diff.causalItemRef === null ? "No timeline item correlation" : `Timeline item ${git.diff.causalItemRef}`}</p>
      <pre>{git.diff.content.split("\n").map((line, index) => {
        const kind = line.startsWith("+") && !line.startsWith("+++") ? "addition"
          : line.startsWith("-") && !line.startsWith("---") ? "deletion" : "context"
        return <code key={index} data-diff-kind={kind}><span className="oa-react-sr-only">{kind}. </span>{line}{"\n"}</code>
      })}</pre>
    </section>}
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
    <ReviewBody git={state.git} report={report} />
  </aside>
  return <Sheet open={open} onOpenChange={onOpenChange}>
    <SheetContent className="oa-react-review-sheet" side="right">
      <SheetHeader><SheetTitle>Repository review</SheetTitle><SheetDescription>Exact bounded status and diff for the current WorkContext.</SheetDescription></SheetHeader>
      <ReviewBody git={state.git} report={report} />
    </SheetContent>
  </Sheet>
}
