import { describe, expect, test } from "vite-plus/test"
import { Effect, Stream } from "@effect-native/core/effect"
import type {
  ConfirmedAgentRun,
  ConfirmedAgentTimelineEvent,
  ConfirmedAgentTimelineItem,
} from "@openagentsinc/khala-sync-client"

import type {
  MobileConversationHost,
  MobileConversationSelection,
  MobileConversationThread,
} from "../src/conversation/mobile-conversation"
import {
  MOBILE_WORK_LOG_COLLAPSED_ITEMS,
  MOBILE_WORK_LOG_MAX_ITEMS,
  hasOwnerConversationActivity,
  projectMobileWorkGroup,
  renderOwnerConversationActivity,
  renderMobileWorkLog,
} from "../src/screens/mobile-work-log"
import {
  buildHomeProgram,
  renderContentView,
} from "../src/screens/home-core"
import { defaultMobileAccessibilityProfile } from "../src/screens/khala-core"

const startedAt = "2026-07-17T20:00:00.000Z"
const completedAt = "2026-07-17T20:02:03.000Z"

const run = (status: ConfirmedAgentRun["status"] = "completed"): ConfirmedAgentRun => ({
  runRef: "run.mobile.work.1",
  routeRef: "thread.mobile.work.1",
  runtime: "codex",
  backend: "pylon",
  status,
  createdAt: startedAt,
  updatedAt: completedAt,
  startedAt,
  completedAt: status === "completed" ? completedAt : null,
  failedAt: status === "failed" ? completedAt : null,
  canceledAt: status === "canceled" ? completedAt : null,
  version: 12,
})

const event = (
  sequence: number,
  item: ConfirmedAgentTimelineItem,
  eventRef = `event.mobile.work.${sequence}`,
): ConfirmedAgentTimelineEvent => ({
  eventRef,
  runRef: "run.mobile.work.1",
  sequence,
  eventType: item.kind,
  summary: item.kind === "plan" ? "Check implementation" : item.kind,
  status: null,
  artifactRefs: [],
  item,
  createdAt: `2026-07-17T20:00:${String(sequence).padStart(2, "0")}.000Z`,
  version: sequence,
})

const settle = Effect.gen(function* () {
  yield* Effect.promise<void>(() => new Promise(resolve => setTimeout(resolve, 0)))
  yield* Effect.yieldNow
})

const lastState = (program: ReturnType<typeof buildHomeProgram>) =>
  Effect.map(Stream.runHead(program.stateChanges), option => {
    if (option._tag !== "Some") throw new Error("expected state")
    return option.value
  })

describe("T3M-A2 mobile grouped work log", () => {
  test("compacts causal events and reports settled identity and elapsed time", () => {
    const group = projectMobileWorkGroup(run(), [
      event(1, { kind: "connected", turnRef: "turn.mobile.work.1", lane: "codex_app_server" }),
      event(2, { kind: "reasoning", messageRef: "reasoning.mobile.1", text: "Inspecting " }),
      event(3, { kind: "reasoning", messageRef: "reasoning.mobile.1", text: "the transcript." }),
      event(4, { kind: "tool", toolCallRef: "tool.mobile.1", toolName: "shell", status: "called" }),
      event(5, { kind: "tool", toolCallRef: "tool.mobile.1", toolName: "shell", status: "completed" }),
      event(6, { kind: "plan", stepRef: "step.mobile.1", status: "running" }),
      event(7, { kind: "plan", stepRef: "step.mobile.1", status: "completed" }),
      event(8, { kind: "usage", inputTokens: 1200, outputTokens: 300, totalTokens: 1500 }),
      event(9, { kind: "terminal", status: "completed" }),
    ])

    expect(group).not.toBeNull()
    expect(group?.summary).toBe("Worked for 2m 3s")
    expect(group?.identityLabel).toBe("Codex · Pylon")
    expect(group?.status).toBe("success")
    expect(group?.items).toHaveLength(6)
    expect(group?.items.find(item => item.itemRef === "reasoning:reasoning.mobile.1")).toMatchObject({
      detail: "Inspecting the transcript.",
      status: "neutral",
    })
    expect(group?.items.find(item => item.itemRef === "tool:tool.mobile.1")).toMatchObject({
      detail: "Completed",
      status: "success",
    })
    expect(group?.items.find(item => item.itemRef === "plan:step.mobile.1")).toMatchObject({
      detail: "Completed",
      status: "success",
    })
  })

  test("prefers server-authored model and provider identity over runtime labels", () => {
    const started = {
      ...event(1, { kind: "connected", turnRef: "turn.mobile.work.1", lane: "hosted_khala" }),
      source: {
        lane: "hosted_khala",
        adapterKind: "openagents_native",
        surface: "server",
        providerRef: "google-ai-studio",
        modelRef: "gemma-4-31b-it",
      },
    }
    expect(projectMobileWorkGroup({
      ...run("running"),
      runtime: "openagents_native",
      backend: "hosted",
    }, [started])?.identityLabel).toBe("Gemma 4 31B · Google AI Studio")
  })

  test("presents confirmed Sarah tool evidence conversationally without internal refs", () => {
    const runningGroup = projectMobileWorkGroup({
      ...run("running"),
      runtime: "openagents_native",
      backend: "hosted",
    }, [event(1, {
      kind: "tool",
      toolCallRef: "call.sarah.private.123",
      toolName: "sarah_harness_status",
      status: "called",
    })])
    if (runningGroup === null) throw new Error("expected Sarah activity")

    expect(hasOwnerConversationActivity(runningGroup)).toBe(true)
    const running = JSON.stringify(renderOwnerConversationActivity(runningGroup))
    expect(running).toContain("Inspecting Sarah's harness…")
    expect(running).toContain("Using an OpenAgents tool")
    expect(running).not.toContain("sarah_harness_status")
    expect(running).not.toContain("hosted runtime")

    const completedGroup = projectMobileWorkGroup({
      ...run("running"),
      runtime: "openagents_native",
      backend: "hosted",
    }, [
      event(1, {
        kind: "tool",
        toolCallRef: "call.sarah.private.123",
        toolName: "sarah_harness_status",
        status: "called",
      }),
      event(2, {
        kind: "tool",
        toolCallRef: "call.sarah.private.123",
        toolName: "sarah_harness_status",
        status: "completed",
      }),
    ])
    if (completedGroup === null) throw new Error("expected completed Sarah activity")
    const completed = JSON.stringify(renderOwnerConversationActivity(completedGroup))
    expect(completed).toContain("Sarah's harness inspected")
    expect(completed).toContain("Tool result received")
    expect(completed).not.toContain("sarah_harness_status")
  })

  test("names collapsed and safety-bound remainders exactly", () => {
    const events = Array.from({ length: MOBILE_WORK_LOG_MAX_ITEMS + 7 }, (_, index) =>
      event(index + 1, { kind: "heartbeat", detail: `Heartbeat ${index + 1}` }))
    const group = projectMobileWorkGroup(run("running"), events)
    if (group === null) throw new Error("expected work group")
    const collapsed = JSON.stringify(renderMobileWorkLog(
      group,
      false,
      {},
      defaultMobileAccessibilityProfile,
    ))

    expect(group.items).toHaveLength(MOBILE_WORK_LOG_MAX_ITEMS)
    expect(group.omittedItemCount).toBe(7)
    expect(collapsed).toContain(`+${MOBILE_WORK_LOG_MAX_ITEMS - MOBILE_WORK_LOG_COLLAPSED_ITEMS} previous activities`)
    expect(collapsed).toContain("7 older activities withheld by the mobile safety bound")
    expect(collapsed).toContain('"expanded":false')
  })

  test("renders one transcript work group and dispatches typed group and row disclosure", async () => {
    const activeRun = run("running")
    const events = [
      event(1, { kind: "reasoning", messageRef: "reasoning.mobile.disclosure", text: "Inspecting the mobile tree in full detail." }),
      ...Array.from({ length: 6 }, (_, index) =>
        event(index + 2, { kind: "heartbeat", detail: `Runtime heartbeat ${index + 1}` })),
      event(8, { kind: "tool", toolCallRef: "tool.mobile.disclosure", toolName: "shell", status: "called" }),
    ]
    const thread: MobileConversationThread = {
      threadRef: activeRun.routeRef,
      title: "Work log parity",
      status: "active",
      messageCount: 1,
      lastMessageAt: completedAt,
      updatedAt: completedAt,
      version: 12,
      messages: [{
        messageRef: "message.mobile.work.user",
        threadRef: activeRun.routeRef,
        body: "Inspect the transcript",
        createdAt: startedAt,
        updatedAt: startedAt,
        version: 1,
      }],
      timeline: {
        status: { phase: "live", cursor: 12, pendingMutationCount: 0 },
        run: activeRun,
        events,
      },
    }
    const host: MobileConversationHost = {
      listThreads: async () => [thread],
      newThread: async () => ({ ok: true, thread }),
      openThread: async () => thread,
      sendMessage: async () => ({ ok: true, thread }),
    }
    const conversation: Extract<MobileConversationSelection, { mode: "sync" }> = {
      mode: "sync",
      host,
      threads: [thread],
      archivedThreads: [],
      activeThread: thread,
    }
    const program = buildHomeProgram({ conversation })
    const initial = JSON.stringify(renderContentView(program.initialState))

    expect(program.initialState.khala.entries.filter(entry => entry.work !== undefined)).toHaveLength(1)
    expect(initial).toContain("Working · 2m 3s")
    expect(initial).toContain("Codex · Pylon")
    expect(initial).toContain("+3 previous activities")
    expect(initial).not.toContain('"content":"shell · called"')

    program.khala.toggleWorkGroup("work:run.mobile.work.1")
    program.khala.toggleWorkItem("reasoning:reasoning.mobile.disclosure")
    program.khala.toggleWorkGroup("work:foreign")
    program.khala.toggleWorkItem("reasoning:foreign")
    await Effect.runPromise(settle)
    const expanded = await Effect.runPromise(lastState(program))
    const expandedView = JSON.stringify(renderContentView(expanded))

    expect(expanded.khala.expandedWorkGroups["work:run.mobile.work.1"]).toBe(true)
    expect(expanded.khala.expandedWorkItems["reasoning:reasoning.mobile.disclosure"]).toBe(true)
    expect(expanded.khala.expandedWorkGroups["work:foreign"]).toBeUndefined()
    expect(expanded.khala.expandedWorkItems["reasoning:foreign"]).toBeUndefined()
    expect(expandedView).toContain("Show fewer activities")
    expect(expandedView).toContain("Inspecting the mobile tree in full detail.")
    expect(expandedView).toContain("Copy detail")
  })
})
