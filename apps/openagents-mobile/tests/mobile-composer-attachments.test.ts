import { describe, expect, test } from "vite-plus/test"
import { Effect, Stream } from "@effect-native/core/effect"
import {
  composerAttachmentId,
  composerBlockId,
  decodeCodingComposerDraftSnapshot,
  emptyComposerState,
} from "@openagentsinc/khala-sync-client"

import type { MobileCodingComposerSession } from "../src/coding/mobile-coding-composer"
import type { MobileExecutionTargetOption } from "../src/coding/mobile-execution-targets"
import type {
  MobileConversationHost,
  MobileConversationThread,
} from "../src/conversation/mobile-conversation"
import {
  buildHomeProgram,
  normalizeMobileAccessibilityProfile,
  renderContentView,
} from "../src/screens/home-core"
import { renderMobileComposerAttachments } from "../src/screens/mobile-composer-attachments"

const now = "2026-07-17T21:45:00.000Z"
const readyId = composerAttachmentId("mobile-ready-image")
const failedId = composerAttachmentId("mobile-failed-file")
const thread: MobileConversationThread = {
  threadRef: "thread.mobile.attachments",
  title: "Attachment editing",
  status: "active",
  messageCount: 0,
  lastMessageAt: null,
  updatedAt: now,
  version: 1,
  messages: [],
}
const target: MobileExecutionTargetOption = {
  targetId: "codex:mobile-attachments",
  label: "Codex work",
  accessibilityLabel: "Codex work, Codex, ready",
  providerLabel: "Codex",
  providerRef: "provider.openai.codex",
  modelRef: "model.gpt-5.6-sol",
  accountRef: "account.mobile.attachments",
  runtimeTarget: { lane: "codex_app_server", executionTargetId: "codex:mobile-attachments" },
  readiness: "ready",
}

const composerSession = (): MobileCodingComposerSession => {
  const state = emptyComposerState()
  const text = state.doc.blocks[0]!
  return {
    repositoryLabel: "openagents",
    worktreeLabel: "feature/mobile",
    targetLabel: target.label,
    draft: decodeCodingComposerDraftSnapshot({
      schema: "openagents.coding_composer_draft.v1",
      draftRef: "draft.mobile.attachments",
      ownerRef: "owner.mobile.attachments",
      sessionRef: "session.mobile.attachments",
      threadRef: thread.threadRef,
      revision: 4,
      doc: {
        ...state.doc,
        blocks: [text, {
          id: composerBlockId("mobile-ready-ref"),
          kind: "attachmentRef",
          attachmentId: readyId,
        }, {
          id: composerBlockId("mobile-failed-ref"),
          kind: "attachmentRef",
          attachmentId: failedId,
        }],
        attachments: [{
          id: readyId,
          kind: "image",
          name: "screen.png",
          mime: "image/png",
          sizeBytes: 1536,
          digest: "aa".repeat(32),
          previewUrl: "file:///attachments/screen.png",
          contentRef: `attachment.native-local.sha256.${"aa".repeat(32)}.screen.png`,
          source: "manual",
          status: "ready",
          uploadAttempt: 1,
        }, {
          id: failedId,
          kind: "file",
          name: "trace.json",
          mime: "application/json",
          sizeBytes: 18,
          digest: "bb".repeat(32),
          contentRef: `attachment.native-local.sha256.${"bb".repeat(32)}.trace.json`,
          source: "manual",
          status: "error",
          uploadAttempt: 1,
          errorText: "Local bytes could not be read.",
        }],
      },
      selection: state.selection,
      view: state.view,
      context: [],
      target: {
        laneRef: "lane.codex_app_server",
        providerRef: target.providerRef,
        modelRef: target.modelRef,
        accountRef: target.accountRef,
        executionTargetRef: target.targetId,
        readiness: "ready",
      },
      submission: { status: "editing" },
      updatedAt: now,
    }),
  }
}

const settle = Effect.gen(function* () {
  yield* Effect.promise<void>(() => new Promise(resolve => setTimeout(resolve, 0)))
  yield* Effect.yieldNow
})

const lastState = (program: ReturnType<typeof buildHomeProgram>) =>
  Effect.map(Stream.runHead(program.stateChanges), option => {
    if (option._tag !== "Some") throw new Error("expected state")
    return option.value
  })

describe("T3M-B2.1 mobile composer attachments", () => {
  test("renders image/file previews, exact state, errors, and accessible actions", () => {
    const content = JSON.stringify(renderMobileComposerAttachments(
      composerSession().draft.doc.attachments,
      null,
      normalizeMobileAccessibilityProfile({ fontScale: 2 }),
    ))
    expect(content).toContain("file:///attachments/screen.png")
    expect(content).toContain("screen.png")
    expect(content).toContain("2 KB · image/png · Ready")
    expect(content).toContain("trace.json")
    expect(content).toContain("Needs attention")
    expect(content).toContain("Local bytes could not be read.")
    expect(content).toContain(`CodingComposerAttachmentRetryRequested`)
    expect(content).toContain(`CodingComposerAttachmentRemoved`)
    expect(content).toContain('"minHeight":56')
  })

  test("accepts only exact active-draft remove/retry outcomes and preserves the transcript", async () => {
    const initial = composerSession()
    const removals: string[] = []
    const retries: string[] = []
    const host: MobileConversationHost = {
      listThreads: async () => [thread],
      newThread: async () => ({ ok: true, thread }),
      openThread: async () => thread,
      sendMessage: async () => ({ ok: true, thread }),
    }
    const update = (session: MobileCodingComposerSession, attachmentId: string, retry: boolean) => ({
      ...session,
      draft: decodeCodingComposerDraftSnapshot({
        ...session.draft,
        revision: session.draft.revision + 1,
        doc: retry
          ? {
              ...session.draft.doc,
              attachments: session.draft.doc.attachments.map(attachment =>
                attachment.id === attachmentId
                  ? { ...attachment, status: "ready" as const, errorText: undefined }
                  : attachment),
            }
          : {
              ...session.draft.doc,
              blocks: session.draft.doc.blocks.filter(block =>
                block.kind !== "attachmentRef" || block.attachmentId !== attachmentId),
              attachments: session.draft.doc.attachments.filter(attachment => attachment.id !== attachmentId),
            },
      }),
    })
    const program = buildHomeProgram({
      conversation: { mode: "sync", host, threads: [thread], archivedThreads: [], activeThread: thread },
      coding: {
        activeComposer: () => initial,
        directory: {
          authority: "confirmed",
          phase: "live",
          cacheState: "current",
          offlineCache: { accounting: "live_confirmed", ownerScopeRef: "scope.owner", cachedRepositoryCount: 0, cachedSessionCount: 0, lastConfirmedCursor: 1 },
          repositories: [],
          sessions: [],
        },
        executionTargets: [target],
        clearSelection: async () => undefined,
        selectSession: async () => ({ thread, composer: initial }),
        updateComposerText: async session => session,
        selectComposerTarget: async session => session,
        pickComposerAttachments: async () => ({ status: "cancelled" }),
        removeComposerAttachment: async (session, attachmentId) => {
          removals.push(attachmentId)
          return update(session, attachmentId, false)
        },
        retryComposerAttachment: async (session, attachmentId) => {
          retries.push(attachmentId)
          return update(session, attachmentId, true)
        },
      },
    })

    program.coding.removeAttachment("attachment.foreign")
    program.coding.retryAttachment(readyId)
    await Effect.runPromise(settle)
    expect(removals).toEqual([])
    expect(retries).toEqual([])

    program.coding.retryAttachment(failedId)
    await Effect.runPromise(settle)
    let state = await Effect.runPromise(lastState(program))
    expect(retries).toEqual([failedId])
    expect(state.codingComposer?.draft.doc.attachments.find(value => value.id === failedId)?.status).toBe("ready")
    expect(state.khala.entries).toEqual([])

    program.coding.removeAttachment(readyId)
    await Effect.runPromise(settle)
    state = await Effect.runPromise(lastState(program))
    expect(removals).toEqual([readyId])
    expect(state.codingComposer?.draft.doc.attachments.map(value => value.id)).toEqual([failedId])
    expect(JSON.stringify(renderContentView(state))).toContain("Attachment removed from this draft.")
  })
})
