import { describe, expect, test } from "vite-plus/test"
import { Effect, Stream } from "@effect-native/core/effect"
import { decodeCodingComposerDraftSnapshot, emptyComposerState } from "@openagentsinc/khala-sync-client"

import type { MobileCodingComposerSession } from "../src/coding/mobile-coding-composer"
import {
  decodeMobileChangeSummary,
  decodeMobileFileDiff,
  decodeMobileReviewReceipt,
  type MobileRepositoryReviewPort,
} from "../src/coding/mobile-repository-review"
import { buildHomeProgram, renderContentView } from "../src/screens/home-core"

const scope = { sessionRef: "session.review", repositoryRef: "repository.review", worktreeRef: "worktree.review" }
const status = {
  ...scope,
  statusRef: "status.review.1",
  headRef: "commit.head.1",
  truncated: false,
  files: [{ pathRef: "src/app.ts", source: "unstaged", status: "modified", adds: 2, dels: 1, binary: false, revisionRef: "revision.app.1" }],
}
const diffRequest = { ...scope, statusRef: status.statusRef, pathRef: "src/app.ts", source: "unstaged" as const, expectedRevisionRef: "revision.app.1" }
const diff = {
  ...scope,
  statusRef: status.statusRef,
  pathRef: "src/app.ts",
  source: "unstaged",
  revisionRef: "revision.app.1",
  language: "typescript",
  hunks: [{ header: "@@ -1 +1,2 @@", rows: [
    { rowRef: "row.review.1", kind: "remove", text: "old", oldLine: 1, newLine: null },
    { rowRef: "row.review.2", kind: "add", text: "new", oldLine: null, newLine: 1 },
  ] }],
}

const composer = (): MobileCodingComposerSession => {
  const state = emptyComposerState()
  return {
    repositoryLabel: "openagents",
    worktreeLabel: "review",
    targetLabel: "Codex",
    draft: decodeCodingComposerDraftSnapshot({
      schema: "openagents.coding_composer_draft.v1",
      draftRef: "draft.review",
      ownerRef: "owner.review",
      sessionRef: scope.sessionRef,
      threadRef: "thread.review",
      revision: 1,
      doc: state.doc,
      selection: state.selection,
      view: state.view,
      context: [{ kind: "repository", repositoryRef: scope.repositoryRef, revisionRef: "revision.repository" }, {
        kind: "worktree", repositoryRef: scope.repositoryRef, worktreeRef: scope.worktreeRef, revisionRef: "revision.worktree",
      }],
      target: { laneRef: "lane.codex", providerRef: "provider.codex", modelRef: "model.codex", accountRef: "account.codex", executionTargetRef: "codex:review", readiness: "ready" },
      submission: { status: "editing" },
      updatedAt: "2026-07-17T23:10:00.000Z",
    }),
  }
}

const settle = Effect.gen(function* () {
  yield* Effect.promise<void>(() => new Promise(resolve => setTimeout(resolve, 0)))
  yield* Effect.yieldNow
})
const lastState = (program: ReturnType<typeof buildHomeProgram>) => Effect.map(Stream.runHead(program.stateChanges), option => {
  if (option._tag !== "Some") throw new Error("expected state")
  return option.value
})

describe("T3M-D2 repository changes and review", () => {
  test("decodes only exact bounded status, diff, and writeback receipts", () => {
    expect(decodeMobileChangeSummary(status, scope)?.files).toHaveLength(1)
    expect(decodeMobileChangeSummary({ ...status, worktreeRef: "foreign" }, scope)).toBeNull()
    expect(decodeMobileFileDiff(diff, diffRequest)?.hunks[0]?.rows).toHaveLength(2)
    expect(decodeMobileFileDiff({ ...diff, revisionRef: "stale" }, diffRequest)).toBeNull()
    const request = { ...scope, statusRef: status.statusRef, pathRef: "src/app.ts", rowRef: "row.review.2", expectedRevisionRef: "revision.app.1", comment: "Keep the null guard.", idempotencyRef: "review.mobile.1" }
    const receipt = { ...scope, statusRef: status.statusRef, pathRef: request.pathRef, rowRef: request.rowRef, reviewRef: "review.recorded.1", receiptRef: "receipt.review.1", state: "recorded", recordedAt: "2026-07-17T23:10:01.000Z", comment: request.comment }
    expect(decodeMobileReviewReceipt(receipt, request)?.receiptRef).toBe("receipt.review.1")
    expect(decodeMobileReviewReceipt({ ...receipt, rowRef: "foreign" }, request)).toBeNull()
  })

  test("loads a current diff and records an exact row instruction without moving transcript state", async () => {
    const active = composer()
    const requests: unknown[] = []
    const program = buildHomeProgram({ coding: {
      directory: { authority: "confirmed", phase: "live", cacheState: "current", offlineCache: { accounting: "live_confirmed", ownerScopeRef: "scope.owner", cachedRepositoryCount: 1, cachedSessionCount: 1, lastConfirmedCursor: 1 }, repositories: [], sessions: [] },
      activeComposer: () => active,
      clearSelection: async () => undefined,
      selectSession: async () => null,
      updateComposerText: async session => session,
      pickComposerAttachments: async () => ({ status: "cancelled" }),
      repositoryReview: {
        status: async request => { requests.push(request); return status },
        diff: async request => { requests.push(request); return diff },
        submitReview: async request => {
          requests.push(request)
          return { ...scope, statusRef: status.statusRef, pathRef: request.pathRef, rowRef: request.rowRef, reviewRef: "review.recorded.1", receiptRef: "receipt.review.1", state: "recorded", recordedAt: "2026-07-17T23:10:01.000Z", comment: request.comment }
        },
      },
    } })
    const transcript = program.initialState.khala
    program.coding.openChanges()
    await Effect.runPromise(settle)
    program.coding.selectChangedFile("src/app.ts", "unstaged", "revision.app.1")
    await Effect.runPromise(settle)
    program.coding.selectReviewRow("row.review.2")
    program.coding.changeReviewComment("Keep the null guard.")
    program.coding.submitReview()
    await Effect.runPromise(settle)
    const state = await Effect.runPromise(lastState(program))
    expect(state.repositoryReview.receipts[0]?.receiptRef).toBe("receipt.review.1")
    expect(state.khala).toEqual(transcript)
    expect(JSON.stringify(renderContentView(state))).toContain("Review recorded")
    expect(requests).toHaveLength(3)
  })

  test("drops a late writeback receipt after the Changes route is dismissed", async () => {
    const active = composer()
    let resolveReceipt!: (value: unknown) => void
    let markStarted!: () => void
    const started = new Promise<void>(resolve => { markStarted = resolve })
    const pendingReceipt = new Promise<unknown>(resolve => { resolveReceipt = resolve })
    const submittedRequests: Array<Parameters<MobileRepositoryReviewPort["submitReview"]>[0]> = []
    const program = buildHomeProgram({ coding: {
      directory: { authority: "confirmed", phase: "live", cacheState: "current", offlineCache: { accounting: "live_confirmed", ownerScopeRef: "scope.owner", cachedRepositoryCount: 1, cachedSessionCount: 1, lastConfirmedCursor: 1 }, repositories: [], sessions: [] },
      activeComposer: () => active,
      clearSelection: async () => undefined,
      selectSession: async () => null,
      updateComposerText: async session => session,
      pickComposerAttachments: async () => ({ status: "cancelled" }),
      repositoryReview: {
        status: async () => status,
        diff: async () => diff,
        submitReview: async request => {
          submittedRequests.push(request)
          markStarted()
          return pendingReceipt
        },
      },
    } })
    program.coding.openChanges()
    await Effect.runPromise(settle)
    program.coding.selectChangedFile("src/app.ts", "unstaged", "revision.app.1")
    await Effect.runPromise(settle)
    program.coding.selectReviewRow("row.review.2")
    program.coding.changeReviewComment("Do not record after dismissal.")
    program.coding.submitReview()
    await started
    program.workspace.dispatchKeyboardCommand("dismiss")
    const submittedRequest = submittedRequests[0]
    if (submittedRequest === undefined) throw new Error("expected submitted review request")
    resolveReceipt({
      ...scope,
      statusRef: status.statusRef,
      pathRef: submittedRequest.pathRef,
      rowRef: submittedRequest.rowRef,
      reviewRef: "review.late.1",
      receiptRef: "receipt.late.1",
      state: "recorded",
      recordedAt: "2026-07-17T23:10:02.000Z",
      comment: submittedRequest.comment,
    })
    await Effect.runPromise(settle)
    const state = await Effect.runPromise(lastState(program))
    expect(state.workbenchRoute).toBe("conversation")
    expect(state.repositoryReview.receipts).toEqual([])
  })
})
