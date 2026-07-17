import { describe, expect, test } from "vite-plus/test"
import { Effect, Stream } from "@effect-native/core/effect"
import { decodeCodingComposerDraftSnapshot, emptyComposerState } from "@openagentsinc/khala-sync-client"

import type { MobileCodingComposerSession } from "../src/coding/mobile-coding-composer"
import type { MobileExecutionTargetOption } from "../src/coding/mobile-execution-targets"
import type { MobileConversationHost, MobileConversationThread } from "../src/conversation/mobile-conversation"
import { buildHomeProgram, renderContentView } from "../src/screens/home-core"
import {
  mobileComposerSlashTrigger,
  projectMobileSlashCommands,
  renderMobileSlashCommandAutocomplete,
  type MobileSlashCommandContext,
} from "../src/screens/mobile-composer-discovery"

const now = "2026-07-17T21:53:00.000Z"
const thread: MobileConversationThread = {
  threadRef: "thread.mobile.commands",
  title: "Command discovery",
  status: "active",
  messageCount: 0,
  lastMessageAt: null,
  updatedAt: now,
  version: 1,
  messages: [],
}
const target: MobileExecutionTargetOption = {
  targetId: "codex:mobile-commands",
  label: "Codex work",
  accessibilityLabel: "Codex work, Codex, ready",
  providerLabel: "Codex",
  providerRef: "provider.openai.codex",
  modelRef: "model.gpt-5.6-sol",
  accountRef: "account.mobile.commands",
  runtimeTarget: { lane: "codex_app_server", executionTargetId: "codex:mobile-commands" },
  readiness: "ready",
}
const composer = (): MobileCodingComposerSession => {
  const state = emptyComposerState()
  return {
    repositoryLabel: "openagents",
    worktreeLabel: "main",
    targetLabel: target.label,
    draft: decodeCodingComposerDraftSnapshot({
      schema: "openagents.coding_composer_draft.v1",
      draftRef: "draft.mobile.commands",
      ownerRef: "owner.mobile.commands",
      sessionRef: "session.mobile.commands",
      threadRef: thread.threadRef,
      revision: 1,
      doc: state.doc,
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
const available: MobileSlashCommandContext = {
  composerAvailable: true,
  targetCatalogAvailable: true,
  attachmentPickerAvailable: true,
  activeTurnRef: null,
  activeTurnCancelable: false,
  pendingAction: false,
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

describe("T3M-B2.2a mobile slash-command discovery", () => {
  test("parses only an explicit trailing slash token and projects bounded typed options", () => {
    expect(mobileComposerSlashTrigger("Review this /tar")).toEqual({ query: "tar", replaceFrom: 12 })
    expect(mobileComposerSlashTrigger("Review /target then continue")).toBeNull()
    expect(projectMobileSlashCommands("Review this /tar", available)?.commands.map(value => value.id)).toEqual([
      "mobile.command.choose_target",
    ])
    const rendered = JSON.stringify(renderMobileSlashCommandAutocomplete("/", available))
    expect(rendered).toContain("/new · New chat")
    expect(rendered).toContain("/target · Choose target")
    expect(rendered).toContain("/attach · Add attachment")
    expect(rendered).toContain("No active turn")
    expect(rendered).toContain('"disabled":true')
    expect(rendered).toContain("No commands match this slash token.")
  })

  test("rewrites query and dispatches only the exact available command authority path", async () => {
    let active = composer()
    let picks = 0
    const host: MobileConversationHost = {
      listThreads: async () => [thread],
      newThread: async () => ({ ok: true, thread }),
      openThread: async () => thread,
      sendMessage: async () => ({ ok: true, thread }),
    }
    const program = buildHomeProgram({
      conversation: { mode: "sync", host, threads: [thread], archivedThreads: [], activeThread: thread },
      coding: {
        activeComposer: () => active,
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
        selectSession: async () => ({ thread, composer: active }),
        updateComposerText: async (session, text) => {
          const parsed = emptyComposerState()
          active = {
            ...session,
            draft: decodeCodingComposerDraftSnapshot({
              ...session.draft,
              revision: session.draft.revision + 1,
              doc: {
                ...parsed.doc,
                blocks: parsed.doc.blocks.map(block => block.kind === "paragraph" ? { ...block, text } : block),
              },
              selection: parsed.selection,
            }),
          }
          return active
        },
        selectComposerTarget: async session => session,
        pickComposerAttachments: async () => { picks += 1; return { status: "cancelled" } },
      },
    })

    program.khala.draftChanged("Keep this /tar")
    await Effect.runPromise(settle)
    program.coding.searchSlashCommands("attach")
    await Effect.runPromise(settle)
    let state = await Effect.runPromise(lastState(program))
    expect(state.khala.draft).toBe("Keep this /attach")
    expect(JSON.stringify(renderContentView(state))).toContain("/attach · Add attachment")

    program.coding.selectSlashCommand("mobile.command.attach")
    await Effect.runPromise(settle)
    state = await Effect.runPromise(lastState(program))
    expect(picks).toBe(1)
    expect(state.khala.draft).toBe("Keep this")

    program.khala.draftChanged("/target")
    await Effect.runPromise(settle)
    program.coding.selectSlashCommand("mobile.command.choose_target")
    await Effect.runPromise(settle)
    state = await Effect.runPromise(lastState(program))
    expect(state.codingComposerTargetPickerOpen).toBe(true)
    expect(state.khala.draft).toBe("")

    program.khala.draftChanged("/stop")
    await Effect.runPromise(settle)
    program.coding.selectSlashCommand("mobile.command.stop_turn")
    await Effect.runPromise(settle)
    state = await Effect.runPromise(lastState(program))
    expect(state.khala.draft).toBe("/stop")
  })
})
