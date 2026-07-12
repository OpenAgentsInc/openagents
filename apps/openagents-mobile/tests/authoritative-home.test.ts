import { describe, expect, test } from "bun:test"
import { Effect, Stream } from "@effect-native/core/effect"
import {
  composerAttachmentId,
  composerBlockId,
  decodeCodingComposerDraftSnapshot,
  emptyComposerSelection,
  emptyComposerState,
  parseComposerMarkdown,
} from "@openagentsinc/khala-sync-client"

import type {
  MobileConversationHost,
  MobileConversationSelection,
  MobileConversationThread,
} from "../src/conversation/mobile-conversation"
import type {
  MobileCodingDirectory,
  MobileCodingOfflineCacheAccounting,
  MobileCodingTarget,
} from "../src/coding/mobile-coding-navigation"
import type { MobileCodingComposerSession } from "../src/coding/mobile-coding-composer"
import {
  buildHomeProgram,
  chromeProps,
  renderContentView,
  renderDrawerView,
} from "../src/screens/home-core"

const now = "2026-07-10T20:15:00.000Z"
const initialThread: MobileConversationThread = {
  threadRef: "thread.synced.1",
  title: "Synced",
  messageCount: 1,
  lastMessageAt: now,
  updatedAt: now,
  version: 3,
  messages: [{
    messageRef: "message.synced.1",
    threadRef: "thread.synced.1",
    body: "Confirmed",
    createdAt: now,
    updatedAt: now,
    version: 5,
  }],
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

const selection = (host: MobileConversationHost): Extract<MobileConversationSelection, { mode: "sync" }> => ({
  mode: "sync",
  host,
  threads: [initialThread],
  activeThread: initialThread,
})

const liveOfflineCache = (
  cachedRepositoryCount: number,
  cachedSessionCount: number,
): MobileCodingOfflineCacheAccounting => ({
  accounting: "live_confirmed",
  ownerScopeRef: "scope.user.owner.fixture",
  cachedRepositoryCount,
  cachedSessionCount,
  lastConfirmedCursor: 4,
})

const codingComposerSession = (text: string): MobileCodingComposerSession => {
  const state = emptyComposerState()
  const attachmentId = composerAttachmentId("attachment-mobile-screen")
  return {
    repositoryLabel: "openagents",
    worktreeLabel: "main",
    targetLabel: "Claude",
    draft: decodeCodingComposerDraftSnapshot({
      schema: "openagents.coding_composer_draft.v1",
      draftRef: "draft.mobile.home",
      ownerRef: "local_mobile_home",
      sessionRef: "session.mobile.home",
      threadRef: initialThread.threadRef,
      revision: 1,
      doc: {
        ...state.doc,
        blocks: [
          { id: composerBlockId("block-mobile-home"), kind: "paragraph", text, marks: [] },
          { id: composerBlockId("block-mobile-attachment"), kind: "attachmentRef", attachmentId },
        ],
        attachments: [{
          id: attachmentId,
          kind: "image",
          name: "screen.png",
          mime: "image/png",
          sizeBytes: 128,
          source: "manual",
          status: "ready",
          contentRef: "attachment.native-local.sha256.fixture.screen.png",
        }],
      },
      selection: state.selection,
      view: state.view,
      context: [{
        kind: "repository",
        repositoryRef: "repository.mobile.home",
        revisionRef: "revision.mobile.home",
      }],
      target: {
        laneRef: "lane.claude_pylon",
        providerRef: "provider.claude",
        readiness: "ready",
      },
      submission: { status: "editing" },
      updatedAt: now,
    }),
  }
}

describe("contract openagents_mobile.chat.authoritative_sync_mode.v1 Home", () => {
  test("restores, renders, edits, and accepted-clears the canonical coding draft", async () => {
    const host: MobileConversationHost = {
      listThreads: async () => [initialThread],
      newThread: async () => ({ ok: true, thread: initialThread }),
      openThread: async () => initialThread,
      sendMessage: async () => ({ ok: true, thread: initialThread }),
    }
    const activeComposer = codingComposerSession("Restored private draft")
    const writes: string[] = []
    const updateComposerText = async (
      session: MobileCodingComposerSession,
      text: string,
    ): Promise<MobileCodingComposerSession> => {
      writes.push(text)
      const parsed = parseComposerMarkdown(text)
      const first = parsed.blocks[0]!
      return {
        ...session,
        draft: decodeCodingComposerDraftSnapshot({
          ...session.draft,
          revision: session.draft.revision + 1,
          doc: {
            ...parsed,
            attachments: session.draft.doc.attachments,
            blocks: [
              ...parsed.blocks,
              ...session.draft.doc.blocks.filter(block => block.kind === "attachmentRef"),
            ],
          },
          selection: emptyComposerSelection(first.id),
          updatedAt: now,
        }),
      }
    }
    const program = buildHomeProgram({
      conversation: selection(host),
      coding: {
        activeComposer: () => activeComposer,
        directory: {
          authority: "confirmed",
          phase: "live",
          cacheState: "current",
          offlineCache: liveOfflineCache(0, 0),
          repositories: [],
          sessions: [],
        },
        clearSelection: async () => undefined,
        selectSession: async () => ({ thread: initialThread, composer: activeComposer }),
        updateComposerText,
        prepareComposerSubmission: async (_session, message) => ({
          ok: true,
          body: `verified attachment\n${message}`,
        }),
        clearComposer: async session => ({
          ...session,
          draft: decodeCodingComposerDraftSnapshot({
            ...session.draft,
            revision: session.draft.revision + 1,
            doc: emptyComposerState().doc,
            selection: emptyComposerState().selection,
            view: emptyComposerState().view,
            updatedAt: now,
          }),
        }),
        pickComposerAttachments: async session => ({
          status: "updated",
          session,
          addedCount: 1,
        }),
      },
    })

    const initial = JSON.stringify(renderContentView(program.initialState))
    expect(initial).toContain("Restored private draft")
    expect(initial).toContain("openagents · main")
    expect(initial).toContain("Claude · provider.claude · Model not selected · Account not selected")
    expect(initial).toContain("screen.png")
    expect(initial).toContain("Add file or image")

    program.coding.pickAttachments()
    await Effect.runPromise(settle)
    const attachmentReady = await Effect.runPromise(lastState(program))
    expect(attachmentReady.codingAttachmentStatus).toEqual({
      kind: "ready",
      message: "1 attachment stored on this device.",
    })

    program.khala.draftChanged("Edited on mobile")
    await Effect.runPromise(settle)
    const edited = await Effect.runPromise(lastState(program))
    expect(edited.khala.draft).toBe("Edited on mobile")
    expect(edited.codingComposer?.draft.revision).toBe(2)

    program.khala.submitTurn("Edited on mobile")
    await Effect.runPromise(settle)
    await Effect.runPromise(settle)
    const accepted = await Effect.runPromise(lastState(program))
    expect(writes).toEqual(["Edited on mobile"])
    expect(accepted.khala.draft).toBe("")
    expect(accepted.codingComposer?.draft.doc.attachments).toEqual([])
  })

  test("keeps an unavailable-target draft editable but withholds Send authority", async () => {
    const base = codingComposerSession("Offline work")
    const activeComposer: MobileCodingComposerSession = {
      ...base,
      targetLabel: "Runtime unavailable",
      draft: decodeCodingComposerDraftSnapshot({
        ...base.draft,
        target: {
          laneRef: "lane.unselected",
          readiness: "unavailable",
          reasonRef: "reason.runtime_lane_unavailable",
        },
      }),
    }
    let sends = 0
    const host: MobileConversationHost = {
      listThreads: async () => [initialThread],
      newThread: async () => ({ ok: true, thread: initialThread }),
      openThread: async () => initialThread,
      sendMessage: async () => {
        sends += 1
        return { ok: true, thread: initialThread }
      },
    }
    const program = buildHomeProgram({
      conversation: selection(host),
      coding: {
        activeComposer: () => activeComposer,
        directory: { authority: "confirmed", phase: "live", cacheState: "current", offlineCache: liveOfflineCache(0, 0), repositories: [], sessions: [] },
        clearSelection: async () => undefined,
        selectSession: async () => ({ thread: initialThread, composer: activeComposer }),
        updateComposerText: async (session) => session,
        pickComposerAttachments: async () => ({ status: "cancelled" }),
      },
    })
    const view = JSON.stringify(renderContentView(program.initialState))
    expect(view).toContain("Runtime unavailable")
    expect(view).toContain('"name":"KhalaDraftChanged"')
    expect(view).not.toContain('"name":"KhalaTurnSubmitted"')

    program.khala.submitTurn("must not dispatch")
    await Effect.runPromise(settle)
    expect(sends).toBe(0)
    expect((await Effect.runPromise(lastState(program))).khala.draft).toBe("Offline work")
  })

  test("renders the confirmed coding directory and selects a session through one typed intent", async () => {
    const host: MobileConversationHost = {
      listThreads: async () => [initialThread],
      newThread: async () => ({ ok: true, thread: initialThread }),
      openThread: async () => initialThread,
      sendMessage: async () => ({ ok: true, thread: initialThread }),
    }
    const selected: MobileCodingTarget[] = []
    const program = buildHomeProgram({
      conversation: selection(host),
      coding: {
        activeComposer: () => null,
        directory: {
          authority: "confirmed",
          phase: "live",
          cacheState: "current",
          offlineCache: liveOfflineCache(1, 1),
          repositories: [{
            repositoryRef: "repository.mobile",
            projectRef: "project.mobile",
            displayName: "openagents",
            sessionCount: 1,
          }],
          sessions: [{
            repositoryRef: "repository.mobile",
            sessionRef: "session.mobile",
            threadRef: initialThread.threadRef,
            state: "active",
            lastActiveAt: now,
          }],
        },
        clearSelection: async () => undefined,
        updateComposerText: async () => null,
        pickComposerAttachments: async () => ({ status: "cancelled" }),
        selectSession: async target => {
          selected.push(target)
          return { thread: initialThread, composer: null }
        },
      },
    })

    const drawer = JSON.stringify(renderDrawerView({
      ...program.initialState,
      drawerOpen: true,
    }))
    expect(drawer).toContain("Coding sessions")
    expect(drawer).toContain("drawer-coding-session-session.mobile")
    expect(drawer).toContain("openagents · 1 session")
    expect(drawer).toContain('"label":"Active"')

    program.coding.selectSession({
      schema: "openagents.mobile.coding_target.v1",
      repositoryRef: "repository.mobile",
      sessionRef: "session.mobile",
      threadRef: initialThread.threadRef,
    })
    await Effect.runPromise(settle)
    await Effect.runPromise(settle)
    expect(selected).toEqual([{
      schema: "openagents.mobile.coding_target.v1",
      repositoryRef: "repository.mobile",
      sessionRef: "session.mobile",
      threadRef: initialThread.threadRef,
    }])
    const current = await Effect.runPromise(lastState(program))
    expect(current.activeThreadRef).toBe(initialThread.threadRef)
    expect(current.khala.pending).toBe(false)
  })

  test("names the loss-accounted withheld coding cache without exposing cached refs", async () => {
    const host: MobileConversationHost = {
      listThreads: async () => [initialThread],
      newThread: async () => ({ ok: true, thread: initialThread }),
      openThread: async () => initialThread,
      sendMessage: async () => ({ ok: true, thread: initialThread }),
    }
    const withheldDirectory: MobileCodingDirectory = {
      authority: "withheld",
      phase: "must_refetch",
      cacheState: "hidden_until_reconnect",
      offlineCache: {
        accounting: "withheld_counted",
        ownerScopeRef: "scope.user.owner.fixture",
        cachedRepositoryCount: 2,
        cachedSessionCount: 3,
        lastConfirmedCursor: 9,
      },
      repositories: [],
      sessions: [],
    }
    const coding = {
      activeComposer: () => null,
      clearSelection: async () => undefined,
      selectSession: async () => null,
      updateComposerText: async () => null,
      pickComposerAttachments: async () => ({ status: "cancelled" as const }),
    }
    const withheldProgram = buildHomeProgram({
      conversation: selection(host),
      coding: { ...coding, directory: withheldDirectory },
    })
    const withheldDrawer = JSON.stringify(renderDrawerView({
      ...withheldProgram.initialState,
      drawerOpen: true,
    }))
    expect(withheldDrawer).toContain("Coding cache · 2 repositories · 3 sessions hidden until reconnect")
    expect(withheldDrawer).not.toContain("Coding sessions")
    expect(withheldDrawer).not.toContain("scope.user.owner.fixture")

    const deniedProgram = buildHomeProgram({
      conversation: selection(host),
      coding: {
        ...coding,
        directory: {
          ...withheldDirectory,
          phase: "denied",
          cacheState: "purged_after_denial",
          offlineCache: { ...withheldDirectory.offlineCache, cachedRepositoryCount: 1, cachedSessionCount: 1 },
        },
      },
    })
    const deniedDrawer = JSON.stringify(renderDrawerView({
      ...deniedProgram.initialState,
      drawerOpen: true,
    }))
    expect(deniedDrawer).toContain("Coding cache · 1 repository · 1 session withheld after denial")

    const unaccountedProgram = buildHomeProgram({
      conversation: selection(host),
      coding: {
        ...coding,
        directory: {
          ...withheldDirectory,
          phase: "signed_out",
          offlineCache: {
            accounting: "unaccounted_signed_out",
            ownerScopeRef: null,
            cachedRepositoryCount: 0,
            cachedSessionCount: 0,
            lastConfirmedCursor: null,
          },
        },
      },
    })
    const unaccountedDrawer = JSON.stringify(renderDrawerView({
      ...unaccountedProgram.initialState,
      drawerOpen: true,
    }))
    expect(unaccountedDrawer).not.toContain("Coding cache")
  })

  test("boots from confirmed refs/versions and exposes confirmed thread navigation", () => {
    const host: MobileConversationHost = {
      listThreads: async () => [initialThread],
      newThread: async () => ({ ok: true, thread: initialThread }),
      openThread: async () => initialThread,
      sendMessage: async () => ({ ok: true, thread: initialThread }),
    }
    const program = buildHomeProgram({ conversation: selection(host) })

    expect(program.initialState).toMatchObject({
      conversationAuthority: "sync",
      syncPhase: "live",
      activeThreadRef: "thread.synced.1",
    })
    expect(program.initialState.khala.entries[0]).toMatchObject({
      key: "message.synced.1",
      text: "Confirmed",
      version: 5,
      status: "done",
    })
    const content = JSON.stringify(renderContentView(program.initialState))
    expect(content).toContain("Confirmed conversation, continuous across your devices.")
    expect(content).toContain('"senderLabel":"YOU"')
    const drawer = JSON.stringify(renderDrawerView(program.initialState))
    expect(drawer).toContain("drawer-thread-thread.synced.1")
    expect(drawer).toContain('"label":"Synced"')
  })

  test("renders a confirmed running timeline while keeping safe follow-up available", () => {
    const running: MobileConversationThread = {
      ...initialThread,
      timeline: {
        status: { phase: "live", cursor: 9, pendingMutationCount: 0 },
        run: {
          runRef: "run.mobile.visible",
          routeRef: initialThread.threadRef,
          runtime: "codex",
          backend: "pylon",
          status: "running",
          createdAt: now,
          updatedAt: now,
          startedAt: now,
          completedAt: null,
          failedAt: null,
          canceledAt: null,
          version: 8,
        },
        events: [{
          eventRef: "event.mobile.visible",
          runRef: "run.mobile.visible",
          sequence: 1,
          eventType: "tool.call",
          summary: "Called shell",
          status: "running",
          artifactRefs: [],
          item: { kind: "tool", toolCallRef: "tool.mobile.visible", toolName: "shell", status: "called" },
          createdAt: now,
          version: 9,
        }],
      },
    }
    const host: MobileConversationHost = {
      listThreads: async () => [running],
      newThread: async () => ({ ok: true, thread: running }),
      openThread: async () => running,
      sendMessage: async () => ({ ok: true, thread: running }),
    }
    const program = buildHomeProgram({ conversation: {
      ...selection(host),
      threads: [running],
      activeThread: running,
    } })

    expect(program.initialState.khala.entries.at(-1)).toMatchObject({ text: "shell · called" })
    expect(program.initialState.khala.pending).toBe(false)
    expect(chromeProps(program.initialState).sending).toBe(false)
  })

  test("renders grouped pending questions and resolves only after the confirmed decision", async () => {
    const pending: MobileConversationThread = {
      ...initialThread,
      timeline: {
        status: { phase: "live", cursor: 10, pendingMutationCount: 0 },
        run: {
          runRef: "turn.mobile.question",
          routeRef: initialThread.threadRef,
          status: "waiting_for_input",
          createdAt: now, updatedAt: now, startedAt: now,
          completedAt: null, failedAt: null, canceledAt: null, version: 10,
        },
        events: [{
          eventRef: "interaction.mobile.question",
          runRef: "turn.mobile.question",
          sequence: 2,
          eventType: "runtime.interaction.provider_question",
          summary: "Choose verification",
          status: "pending",
          artifactRefs: [],
          item: {
            kind: "question",
            questionRef: "interaction.mobile.question",
            title: "Choose verification",
            prompt: "Select both answers before continuing.",
            status: "pending",
            expiresAt: "2026-07-10T20:20:00.000Z",
            questions: [
              { questionRef: "question.tests", displayText: "Which tests?", multiSelect: true, options: [
                { optionRef: "option.unit", label: "Unit", description: "Fast focused suite" },
                { optionRef: "option.e2e", label: "End to end" },
              ] },
              { questionRef: "question.target", displayText: "Which target?", multiSelect: false, options: [
                { optionRef: "option.mobile", label: "Mobile" },
              ] },
            ],
          },
          createdAt: now,
          version: 10,
        }],
      },
    }
    const resolved: MobileConversationThread = {
      ...pending,
      timeline: {
        ...pending.timeline!,
        events: pending.timeline!.events.map(event => ({
          ...event,
          status: "resolved",
          version: 11,
          item: event.item?.kind === "question"
            ? { ...event.item, status: "resolved", decisionRef: "decision.mobile.question" }
            : event.item,
        })),
      },
    }
    const decisions: unknown[] = []
    let finish: ((value: { ok: true; thread: MobileConversationThread }) => void) | undefined
    const host: MobileConversationHost = {
      listThreads: async () => [pending],
      newThread: async () => ({ ok: true, thread: pending }),
      openThread: async () => pending,
      sendMessage: async () => ({ ok: true, thread: pending }),
      decideInteraction: input => new Promise(resolve => {
        decisions.push(input)
        finish = resolve
      }),
    }
    const program = buildHomeProgram({ conversation: {
      ...selection(host), threads: [pending], activeThread: pending,
    } })
    const initial = JSON.stringify(renderContentView(program.initialState))
    expect(initial).toContain("Needs your response")
    expect(initial).toContain("Fast focused suite")
    expect(initial).toContain('"label":"Submit answers","variant":"primary","disabled":true')

    program.khala.toggleInteractionOption({ interactionRef: "interaction.mobile.question", questionRef: "question.tests", optionRef: "option.unit", multiSelect: true })
    program.khala.toggleInteractionOption({ interactionRef: "interaction.mobile.question", questionRef: "question.target", optionRef: "option.mobile", multiSelect: false })
    await Effect.runPromise(settle)
    const selected = await Effect.runPromise(lastState(program))
    expect(JSON.stringify(renderContentView(selected))).toContain('"label":"Submit answers","variant":"primary","disabled":false')

    program.khala.submitInteractionDecision({ interactionRef: "interaction.mobile.question", turnRef: "turn.mobile.question", kind: "provider_question" })
    await Effect.runPromise(settle)
    const submitting = await Effect.runPromise(lastState(program))
    expect(submitting.khala.interactionSubmittingRef).toBe("interaction.mobile.question")
    expect(JSON.stringify(renderContentView(submitting))).toContain("Submitting…")
    expect(decisions).toMatchObject([{ decision: { kind: "provider_question", answers: [
      { questionRef: "question.tests", optionRefs: ["option.unit"] },
      { questionRef: "question.target", optionRefs: ["option.mobile"] },
    ] } }])

    finish?.({ ok: true, thread: resolved })
    await Effect.runPromise(settle)
    const confirmed = await Effect.runPromise(lastState(program))
    const confirmedView = JSON.stringify(renderContentView(confirmed))
    expect(confirmedView).toContain("Resolved")
    expect(confirmedView).not.toContain("Submit answers")
  })

  test("renders expired and revoked controls as terminal read-only state", () => {
    const terminal: MobileConversationThread = {
      ...initialThread,
      timeline: {
        status: { phase: "live", cursor: 12, pendingMutationCount: 0 },
        run: { runRef: "turn.mobile.terminal", routeRef: initialThread.threadRef, status: "waiting_for_input", createdAt: now, updatedAt: now, startedAt: now, completedAt: null, failedAt: null, canceledAt: null, version: 12 },
        events: [
          { eventRef: "interaction.mobile.expired", runRef: "turn.mobile.terminal", sequence: 2, eventType: "runtime.interaction.tool_approval", summary: "Approve tool", status: "expired", artifactRefs: [], item: { kind: "approval", interactionRef: "interaction.mobile.expired", prompt: "Run shell?", status: "expired" }, createdAt: now, version: 12 },
          { eventRef: "interaction.mobile.revoked", runRef: "turn.mobile.terminal", sequence: 3, eventType: "runtime.interaction.plan_review", summary: "Review plan", status: "revoked", artifactRefs: [], item: { kind: "plan", stepRef: "interaction.mobile.revoked", interactionRef: "interaction.mobile.revoked", prompt: "Apply this plan?", status: "revoked" }, createdAt: now, version: 13 },
        ],
      },
    }
    const host: MobileConversationHost = { listThreads: async () => [terminal], newThread: async () => ({ ok: true, thread: terminal }), openThread: async () => terminal, sendMessage: async () => ({ ok: true, thread: terminal }), decideInteraction: async () => ({ ok: true, thread: terminal }) }
    const program = buildHomeProgram({ conversation: { ...selection(host), threads: [terminal], activeThread: terminal } })
    const content = JSON.stringify(renderContentView(program.initialState))
    expect(content).toContain("Expired")
    expect(content).toContain("Access revoked")
    expect(content).not.toContain('"label":"Approve"')
    expect(content).not.toContain('"label":"Accept plan"')
  })

  test("renders exact mobile cancel then confirmed resume/retry/close controls", async () => {
    const running: MobileConversationThread = {
      ...initialThread,
      timeline: {
        status: { phase: "live", cursor: 20, pendingMutationCount: 0 },
        run: {
          runRef: "turn.mobile.control",
          routeRef: initialThread.threadRef,
          runtime: "claude_code",
          backend: "pylon",
          status: "running",
          createdAt: now,
          updatedAt: now,
          startedAt: now,
          completedAt: null,
          failedAt: null,
          canceledAt: null,
          version: 20,
        },
        events: [],
      },
    }
    const canceled: MobileConversationThread = {
      ...running,
      timeline: {
        ...running.timeline!,
        run: {
          ...running.timeline!.run!,
          status: "canceled",
          canceledAt: now,
          version: 21,
        },
      },
    }
    const controls: unknown[] = []
    let finish: ((value: { ok: true; thread: MobileConversationThread }) => void) | undefined
    const host: MobileConversationHost = {
      listThreads: async () => [running],
      newThread: async () => ({ ok: true, thread: running }),
      openThread: async () => running,
      sendMessage: async () => ({ ok: true, thread: running }),
      controlTurn: input => new Promise(resolve => {
        controls.push(input)
        finish = resolve
      }),
    }
    const program = buildHomeProgram({ conversation: {
      mode: "sync",
      host,
      threads: [running],
      activeThread: running,
    } })

    const initial = JSON.stringify(renderContentView(program.initialState))
    expect(initial).toContain('"label":"Cancel turn"')
    expect(initial).not.toContain('"label":"Retry"')

    program.khala.controlTurn({ action: "cancel", runRef: "turn.mobile.control" })
    await Effect.runPromise(settle)
    const submitting = await Effect.runPromise(lastState(program))
    expect(submitting.khala.runtimeControlSubmittingAction).toBe("cancel")
    expect(JSON.stringify(renderContentView(submitting))).toContain('"label":"Canceling…","variant":"secondary","disabled":true')
    expect(controls).toMatchObject([{
      action: "cancel",
      runRef: "turn.mobile.control",
      threadRef: initialThread.threadRef,
    }])

    finish?.({ ok: true, thread: canceled })
    await Effect.runPromise(settle)
    const confirmed = await Effect.runPromise(lastState(program))
    expect(confirmed.khala.runtimeControlSubmittingAction).toBeNull()
    const confirmedView = JSON.stringify(renderContentView(confirmed))
    expect(confirmedView).toContain('"label":"Resume"')
    expect(confirmedView).toContain('"label":"Retry"')
    expect(confirmedView).toContain('"label":"Close turn"')
    expect(confirmedView).not.toContain('"label":"Cancel turn"')
  })

  test("marks a submitted draft pending, then replaces it only with exact confirmed state", async () => {
    let resolveSend: ((value: Awaited<ReturnType<MobileConversationHost["sendMessage"]>>) => void) | undefined
    const confirmed: MobileConversationThread = {
      ...initialThread,
      messageCount: 2,
      version: 7,
      messages: [
        ...initialThread.messages,
        {
          messageRef: "message.mobile.confirmed",
          threadRef: initialThread.threadRef,
          body: "Continue this",
          createdAt: now,
          updatedAt: now,
          version: 7,
        },
      ],
    }
    const host: MobileConversationHost = {
      listThreads: async () => [initialThread],
      newThread: async () => ({ ok: true, thread: initialThread }),
      openThread: async () => initialThread,
      sendMessage: () => new Promise(resolve => { resolveSend = resolve }),
    }
    const program = buildHomeProgram({ conversation: selection(host) })

    program.khala.submitTurn("Continue this")
    await Effect.runPromise(settle)
    const pending = await Effect.runPromise(lastState(program))
    expect(pending.khala.entries.at(-1)).toMatchObject({
      key: "pending-mobile-1",
      text: "Continue this",
      status: "pending",
    })
    expect(JSON.stringify(renderContentView(pending))).toContain('"senderLabel":"YOU · PENDING"')

    resolveSend?.({ ok: true, thread: confirmed })
    await Effect.runPromise(settle)
    const completed = await Effect.runPromise(lastState(program))
    expect(completed.khala.pending).toBe(false)
    expect(completed.khala.entries.some(entry => entry.key.startsWith("pending-"))).toBe(false)
    expect(completed.khala.entries.at(-1)).toMatchObject({
      key: "message.mobile.confirmed",
      version: 7,
      status: "done",
    })
  })

  test("removes an unconfirmed draft and clears account-linked state on denial", async () => {
    const host: MobileConversationHost = {
      listThreads: async () => [initialThread],
      newThread: async () => ({ ok: true, thread: initialThread }),
      openThread: async () => initialThread,
      sendMessage: async () => ({ ok: false, error: "Message is still pending reconciliation." }),
    }
    const program = buildHomeProgram({ conversation: selection(host) })

    program.khala.submitTurn("Never confirmed")
    await Effect.runPromise(settle)
    await Effect.runPromise(settle)
    const failed = await Effect.runPromise(lastState(program))
    expect(failed.khala.entries.some(entry => entry.key.startsWith("pending-"))).toBe(false)
    expect(failed.khala.entries.at(-1)).toMatchObject({
      role: "system",
      status: "failed",
      text: "Message is still pending reconciliation.",
    })

    program.sync.setPhase("denied")
    await Effect.runPromise(settle)
    const denied = await Effect.runPromise(lastState(program))
    expect(denied.activeThreadRef).toBeNull()
    expect(denied.conversationThreads).toEqual([])
    expect(denied.khala.entries).toEqual([])
  })
})
