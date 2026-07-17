import { describe, expect, test } from "vite-plus/test"
import { Effect, Stream } from "@effect-native/core/effect"
import {
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
import {
  groupedMobileExecutionTargets,
  renderMobileComposerToolbar,
} from "../src/screens/mobile-composer-toolbar"

const now = "2026-07-17T21:00:00.000Z"
const thread: MobileConversationThread = {
  threadRef: "thread.mobile.toolbar",
  title: "Composer toolbar",
  status: "active",
  messageCount: 0,
  lastMessageAt: null,
  updatedAt: now,
  version: 1,
  messages: [],
}

const target = (
  targetId: string,
  label: string,
  providerLabel: MobileExecutionTargetOption["providerLabel"],
  readiness: MobileExecutionTargetOption["readiness"] = "ready",
  reasonRef?: MobileExecutionTargetOption["reasonRef"],
): MobileExecutionTargetOption => ({
  targetId,
  label,
  providerLabel,
  accessibilityLabel: `${label}, ${providerLabel}, ${readiness}`,
  providerRef: `provider.${providerLabel.toLowerCase()}`,
  modelRef: providerLabel === "Claude" ? "model.claude-fable-5" : "model.gpt-5.6-sol",
  accountRef: `${targetId}.account`,
  runtimeTarget: {
    lane: providerLabel === "Claude" ? "claude_pylon" : providerLabel === "Codex" ? "codex_app_server" : "managed_cloud",
    executionTargetId: targetId,
  },
  readiness,
  ...(reasonRef === undefined ? {} : { reasonRef }),
})

const openAgents = target("agent-computer", "Agent Computer", "OpenAgents")
const codex = target("codex:ready", "Work Codex", "Codex")
const claude = target(
  "claude:reauth",
  "Claude personal",
  "Claude",
  "revoked",
  "reason.account_requires_reauth",
)
const targets = [claude, codex, openAgents]

const composerFor = (selected: MobileExecutionTargetOption): MobileCodingComposerSession => {
  const state = emptyComposerState()
  return {
    repositoryLabel: "openagents",
    worktreeLabel: "feature/mobile",
    targetLabel: selected.label,
    draft: decodeCodingComposerDraftSnapshot({
      schema: "openagents.coding_composer_draft.v1",
      draftRef: "draft.mobile.toolbar",
      ownerRef: "owner.mobile.toolbar",
      sessionRef: "session.mobile.toolbar",
      threadRef: thread.threadRef,
      revision: 1,
      doc: state.doc,
      selection: state.selection,
      view: state.view,
      context: [],
      target: {
        laneRef: `lane.${selected.runtimeTarget.lane}`,
        providerRef: selected.providerRef,
        modelRef: selected.modelRef,
        accountRef: selected.accountRef,
        executionTargetRef: selected.targetId,
        readiness: selected.readiness,
        ...(selected.reasonRef === undefined ? {} : { reasonRef: selected.reasonRef }),
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

describe("T3M-B1 mobile composer toolbar", () => {
  test("groups authoritative targets in product order and searches model/provider/account labels", () => {
    expect(groupedMobileExecutionTargets(targets, "").map(group => group.providerLabel)).toEqual([
      "OpenAgents",
      "Codex",
      "Claude",
    ])
    expect(groupedMobileExecutionTargets(targets, "gpt").flatMap(group => group.options.map(option => option.targetId))).toEqual([
      "agent-computer",
      "codex:ready",
    ])
    expect(groupedMobileExecutionTargets(targets, "personal").flatMap(group => group.options.map(option => option.targetId))).toEqual([
      "claude:reauth",
    ])
  })

  test("renders compact current target/model/mode and complete grouped picker states", () => {
    const content = JSON.stringify(renderMobileComposerToolbar(
      composerFor(codex),
      targets,
      { pickerOpen: true, search: "" },
      normalizeMobileAccessibilityProfile({ fontScale: 2 }),
    ))
    expect(content).toContain("openagents · feature/mobile")
    expect(content).toContain("Work Codex · gpt 5.6 sol")
    expect(content).toContain("Composer mode, Code")
    expect(content).toContain('"open":true')
    expect(content).toContain("OpenAgents")
    expect(content).toContain("Codex")
    expect(content).toContain("Claude")
    expect(content).toContain("Sign in again to use this account")
    expect(content).toContain('"label":"Claude personal","variant":"ghost","selected":false,"disabled":true')
    expect(content).toContain('"minHeight":56')
    expect(content).not.toContain('"mode":"shell"')

    const empty = JSON.stringify(renderMobileComposerToolbar(
      composerFor(codex),
      targets,
      { pickerOpen: true, search: "nonexistent" },
      normalizeMobileAccessibilityProfile(),
    ))
    expect(empty).toContain("No targets match “nonexistent”")
  })

  test("opens/searches/dismisses and persists only an exact ready selection", async () => {
    const initialComposer = composerFor(codex)
    const selected: string[] = []
    const host: MobileConversationHost = {
      listThreads: async () => [thread],
      newThread: async () => ({ ok: true, thread }),
      openThread: async () => thread,
      sendMessage: async () => ({ ok: true, thread }),
    }
    const program = buildHomeProgram({
      conversation: { mode: "sync", host, threads: [thread], archivedThreads: [], activeThread: thread },
      coding: {
        activeComposer: () => initialComposer,
        directory: {
          authority: "confirmed",
          phase: "live",
          cacheState: "current",
          offlineCache: { accounting: "live_confirmed", ownerScopeRef: "scope.owner", cachedRepositoryCount: 0, cachedSessionCount: 0, lastConfirmedCursor: 1 },
          repositories: [],
          sessions: [],
        },
        executionTargets: targets,
        clearSelection: async () => undefined,
        selectSession: async () => ({ thread, composer: initialComposer }),
        updateComposerText: async session => session,
        selectComposerTarget: async (_session, next) => {
          selected.push(next.targetId)
          return composerFor(next)
        },
        pickComposerAttachments: async () => ({ status: "cancelled" }),
      },
    })

    program.coding.openTargetPicker()
    program.coding.searchTargets("Claude")
    await Effect.runPromise(settle)
    let state = await Effect.runPromise(lastState(program))
    expect(state.codingComposerTargetPickerOpen).toBe(true)
    expect(state.codingComposerTargetSearch).toBe("Claude")
    expect(JSON.stringify(renderContentView(state))).toContain("Claude personal")
    expect(JSON.stringify(renderContentView(state))).not.toContain('"label":"Work Codex","variant"')

    program.coding.selectTarget(claude.targetId)
    program.coding.selectTarget("foreign-target")
    await Effect.runPromise(settle)
    expect(selected).toEqual([])

    program.coding.searchTargets("")
    program.coding.selectTarget(openAgents.targetId)
    await Effect.runPromise(settle)
    state = await Effect.runPromise(lastState(program))
    expect(selected).toEqual([openAgents.targetId])
    expect(state.codingComposer?.draft.target.executionTargetRef).toBe(openAgents.targetId)
    expect(state.codingComposerTargetPickerOpen).toBe(false)
    expect(state.codingComposerTargetSearch).toBe("")

    program.coding.openTargetPicker()
    program.coding.dismissTargetPicker()
    await Effect.runPromise(settle)
    state = await Effect.runPromise(lastState(program))
    expect(state.codingComposerTargetPickerOpen).toBe(false)
  })
})
