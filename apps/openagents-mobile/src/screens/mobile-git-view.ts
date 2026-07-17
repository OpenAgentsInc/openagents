import {
  Button,
  ComponentValueBinding,
  IntentRef,
  Stack,
  StaticPayload,
  Text,
  TextField,
  type View,
} from "@effect-native/core"

import type { MobileRepositoryGitState } from "../coding/mobile-repository-git"
import type { MobileAccessibilityProfile } from "./khala-core"

const failureCopy = (code: MobileRepositoryGitState["failureCode"]): string | null => {
  switch (code) {
    case "conflict": return "The worktree has conflicts. Resolve them before continuing."
    case "non_fast_forward": return "Push was rejected because the remote moved. Refresh before deciding how to reconcile."
    case "auth_failed": return "The paired environment could not authenticate this Git operation."
    case "blocked_by_hook": return "A repository hook blocked the operation."
    case "stale_status": return "Repository state moved. Refresh before trying again."
    case "dirty_tree": return "Switching branches requires a clean worktree."
    case "nothing_to_commit": return "No selected changes were available to commit."
    case "no_upstream": return "This branch has no configured upstream."
    case "detached_head": return "Create or select a branch before committing or pushing."
    case "invalid_branch": return "That branch is no longer available."
    case "operation_failed": return "The Git operation failed without an authoritative receipt."
    case null: return null
  }
}

export const renderMobileGitView = (
  state: MobileRepositoryGitState,
  accessibility: MobileAccessibilityProfile,
): View => {
  const status = state.status
  const receipt = state.receipts.at(-1)
  const failure = failureCopy(state.failureCode)
  return Stack({
    key: "mobile-git-root",
    direction: "column",
    gap: "3",
    padding: "3",
    preserveScrollAnchor: true,
    style: { width: "full", height: "full", backgroundColor: "background" },
    a11y: { role: "region", label: "Git workbench" },
  }, [
    Stack({ key: "git-actions", direction: "row", gap: "2" }, [
      Button({
        key: "git-return-conversation",
        label: "Conversation",
        variant: "ghost",
        onPress: IntentRef("WorkbenchConversationOpened", StaticPayload({})),
        style: { minHeight: accessibility.minTouchTarget },
      }),
      Button({
        key: "git-refresh",
        label: "Refresh status",
        variant: "ghost",
        disabled: state.state === "loading" || state.scope === null || state.submitting,
        onPress: IntentRef("RepositoryGitRefreshed", StaticPayload({})),
        style: { minHeight: accessibility.minTouchTarget },
      }),
    ]),
    ...(state.state === "loading" ? [Text({ key: "git-loading", content: "Loading exact worktree status…", variant: "body", color: "textMuted" })] : []),
    ...(state.state === "unavailable" || state.state === "failed" ? [Text({
      key: "git-unavailable",
      content: state.message ?? "Git status is unavailable.",
      variant: "body",
      color: state.state === "failed" ? "danger" : "textMuted",
    })] : []),
    ...(status === null ? [] : [
      Text({ key: "git-branch", content: status.branch ?? "Detached HEAD", variant: "heading" }),
      Text({
        key: "git-summary",
        content: `${status.files.length} changed · ${status.ahead} ahead · ${status.behind} behind${status.upstream === null ? " · no upstream" : ` · ${status.upstream}`}${status.truncated ? " · list capped" : ""}`,
        variant: "caption",
        color: "textMuted",
      }),
      ...(status.defaultBranch ? [Text({ key: "git-default-warning", content: "Default branch — every mutation requires explicit confirmation.", variant: "caption", color: "warning" })] : []),
      Text({ key: "git-branches-title", content: "Branches", variant: "heading" }),
      ...status.branches.map(branch => Button({
        key: `git-branch-${branch.branchRef}`,
        label: `${branch.current ? "Current · " : ""}${branch.name}${branch.upstream === null ? "" : ` · ${branch.upstream}`}`,
        variant: branch.current ? "secondary" : "ghost",
        disabled: branch.current || state.submitting,
        onPress: IntentRef("RepositoryGitBranchSelected", StaticPayload({ branchRef: branch.branchRef, name: branch.name })),
        style: { width: "full", minHeight: accessibility.minTouchTarget },
      })),
      Text({ key: "git-files-title", content: "Commit changes", variant: "heading" }),
      ...status.files.map(file => Button({
        key: `git-file-${file.pathRef}`,
        label: `${state.selectedPaths.includes(file.pathRef) ? "Selected · " : ""}${file.pathRef} · ${file.staged ? "staged" : "unstaged"} · ${file.status}`,
        variant: state.selectedPaths.includes(file.pathRef) ? "secondary" : "ghost",
        disabled: file.status === "unmerged" || state.submitting,
        onPress: IntentRef("RepositoryGitFileToggled", StaticPayload({ pathRef: file.pathRef })),
        style: { width: "full", minHeight: accessibility.minTouchTarget },
      })),
      TextField({
        key: "git-commit-message",
        value: state.commitMessage,
        placeholder: "Commit message",
        multiline: true,
        disabled: state.submitting,
        onChange: IntentRef("RepositoryGitCommitMessageChanged", ComponentValueBinding()),
        a11y: { label: "Commit message" },
        style: { width: "full", minHeight: 96 },
      }),
      Stack({ key: "git-mutations", direction: "row", gap: "2" }, [
        Button({
          key: "git-commit",
          label: state.submitting ? "Working…" : "Commit selected",
          variant: "primary",
          disabled: state.submitting || status.detached || state.selectedPaths.length === 0 || state.commitMessage.trim() === "",
          onPress: IntentRef("RepositoryGitCommitRequested", StaticPayload({})),
          style: { minHeight: accessibility.minTouchTarget },
        }),
        Button({
          key: "git-push",
          label: state.submitting ? "Working…" : "Push",
          variant: "secondary",
          disabled: state.submitting || status.detached || status.upstream === null || status.ahead === 0,
          onPress: IntentRef("RepositoryGitPushRequested", StaticPayload({})),
          style: { minHeight: accessibility.minTouchTarget },
        }),
      ]),
    ]),
    ...(state.pendingConfirmation === null ? [] : [
      Text({
        key: "git-confirm-title",
        content: `Confirm ${state.pendingConfirmation.op} on ${state.status?.branch ?? "this branch"}`,
        variant: "heading",
      }),
      Text({
        key: "git-confirm-copy",
        content: "The request is fenced to the status and HEAD shown above. If either moved, the environment must refuse it.",
        variant: "body",
        color: "textMuted",
      }),
      Stack({ key: "git-confirm-actions", direction: "row", gap: "2" }, [
        Button({ key: "git-confirm-cancel", label: "Cancel", variant: "ghost", onPress: IntentRef("RepositoryGitConfirmationCancelled", StaticPayload({})) }),
        Button({ key: "git-confirm-run", label: `Confirm ${state.pendingConfirmation.op}`, variant: "primary", onPress: IntentRef("RepositoryGitConfirmationAccepted", StaticPayload({})) }),
      ]),
    ]),
    ...(failure === null ? [] : [Text({ key: `git-failure-${state.failureCode}`, content: failure, variant: "body", color: "danger" })]),
    ...(receipt === undefined ? [] : [Text({
      key: `git-receipt-${receipt.receiptRef}`,
      content: `${receipt.op === "checkout" ? "Branch selected" : receipt.op === "commit" ? "Commit recorded" : "Push recorded"} · ${receipt.summary} · ${receipt.receiptRef}`,
      variant: "caption",
      color: "success",
    })]),
  ])
}
