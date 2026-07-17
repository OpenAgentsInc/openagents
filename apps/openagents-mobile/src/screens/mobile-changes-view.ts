import {
  Button,
  ComponentValueBinding,
  DiffView,
  IntentRef,
  Stack,
  StaticPayload,
  Text,
  TextField,
  type View,
} from "@effect-native/core"

import type { MobileRepositoryReviewState } from "../coding/mobile-repository-review"
import type { MobileAccessibilityProfile } from "./khala-core"

export const renderMobileChangesView = (
  state: MobileRepositoryReviewState,
  accessibility: MobileAccessibilityProfile,
): View => {
  const receipt = state.receipts.at(-1)
  return Stack({
    key: "mobile-changes-root",
    direction: "column",
    gap: "3",
    padding: "3",
    preserveScrollAnchor: true,
    style: { width: "full", height: "full", backgroundColor: "background" },
    a11y: { role: "region", label: "Repository changes and review" },
  }, [
    Stack({ key: "changes-actions", direction: "row", gap: "2" }, [
      Button({
        key: "changes-return-conversation",
        label: "Conversation",
        variant: "ghost",
        onPress: IntentRef("WorkbenchConversationOpened", StaticPayload({})),
        style: { minHeight: accessibility.minTouchTarget },
      }),
      Button({
        key: "changes-refresh",
        label: "Refresh changes",
        variant: "ghost",
        disabled: state.state === "loading" || state.scope === null,
        onPress: IntentRef("RepositoryChangesRefreshed", StaticPayload({})),
        style: { minHeight: accessibility.minTouchTarget },
      }),
    ]),
    ...(state.state === "loading" ? [Text({ key: "changes-loading", content: "Loading exact worktree changes…", variant: "body", color: "textMuted" })] : []),
    ...(state.state === "failed" || state.state === "unavailable" ? [Text({
      key: "changes-error",
      content: state.message ?? "Repository changes are unavailable.",
      variant: "body",
      color: state.state === "failed" ? "danger" : "textMuted",
    })] : []),
    ...(state.summary === null ? [] : [
      Text({
        key: "changes-summary",
        content: `${state.summary.files.length} changed ${state.summary.files.length === 1 ? "file" : "files"}${state.summary.truncated ? " · list capped" : ""}`,
        variant: "heading",
      }),
      ...state.summary.files.map(file => Button({
        key: `change-file-${file.source}-${file.pathRef}`,
        label: `${file.pathRef} · ${file.source} · ${file.status}${file.binary ? " · binary" : file.adds === null ? "" : ` · +${file.adds} −${file.dels ?? 0}`}`,
        variant: state.diff?.pathRef === file.pathRef && state.diff.source === file.source ? "secondary" : "ghost",
        disabled: file.binary || file.source === "untracked" || file.status === "unmerged",
        onPress: IntentRef("RepositoryChangedFileSelected", StaticPayload({
          pathRef: file.pathRef,
          source: file.source,
          revisionRef: file.revisionRef,
        })),
        style: { width: "full", minHeight: accessibility.minTouchTarget },
        a11y: { label: `Review ${file.pathRef}, ${file.source}, ${file.status}` },
      })),
    ]),
    ...(state.summary !== null && state.summary.files.length === 0
      ? [Text({ key: "changes-empty", content: "No changes in this worktree.", variant: "body", color: "textMuted" })]
      : []),
    ...(state.diff === null ? [] : [
      Text({ key: "changes-diff-title", content: state.diff.pathRef, variant: "heading" }),
      DiffView({
        key: `changes-diff-${state.diff.revisionRef}`,
        language: state.diff.language,
        layout: "unified",
        hunks: state.diff.hunks.map(hunk => ({
          header: hunk.header,
          rows: hunk.rows.map(row => ({
            id: row.rowRef,
            kind: row.kind,
            tokens: [{ kind: "plain", text: row.text }],
            ...(row.oldLine === null ? {} : { oldLine: row.oldLine }),
            ...(row.newLine === null ? {} : { newLine: row.newLine }),
            ...(state.receipts.find(item => item.rowRef === row.rowRef) === undefined
              ? {}
              : { comment: state.receipts.find(item => item.rowRef === row.rowRef)!.comment }),
          })),
        })),
        onLineComment: IntentRef("RepositoryReviewRowSelected", ComponentValueBinding()),
        style: { width: "full", borderRadius: "md", padding: "2" },
      }),
    ]),
    ...(state.selectedRowRef === null ? [] : [
      Text({ key: "review-row-label", content: `Review instruction for ${state.selectedRowRef}`, variant: "caption", color: "textMuted" }),
      TextField({
        key: "review-comment-draft",
        value: state.commentDraft,
        placeholder: "Leave a precise review instruction",
        multiline: true,
        disabled: state.submitting,
        onChange: IntentRef("RepositoryReviewCommentChanged", ComponentValueBinding()),
        a11y: { label: "Review instruction" },
        style: { width: "full", minHeight: 96 },
      }),
      Button({
        key: "review-comment-submit",
        label: state.submitting ? "Recording…" : "Record review instruction",
        variant: "primary",
        disabled: state.submitting || state.commentDraft.trim() === "",
        onPress: IntentRef("RepositoryReviewSubmitted", StaticPayload({})),
        style: { width: "full", minHeight: accessibility.minTouchTarget },
      }),
    ]),
    ...(receipt === undefined ? [] : [Text({
      key: `review-receipt-${receipt.receiptRef}`,
      content: `Review recorded · ${receipt.receiptRef}`,
      variant: "caption",
      color: "success",
    })]),
  ])
}
