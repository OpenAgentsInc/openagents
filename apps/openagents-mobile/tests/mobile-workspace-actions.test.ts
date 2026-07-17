import { describe, expect, test } from "vite-plus/test"
import { Effect, Stream } from "@effect-native/core/effect"
import { IntentRef, StaticPayload } from "@effect-native/core"

import type {
  MobileConversationHost,
  MobileConversationSelection,
  MobileConversationThread,
} from "../src/conversation/mobile-conversation"
import { buildHomeProgram, renderDrawerView } from "../src/screens/home-core"

const now = "2026-07-17T20:00:00.000Z"
const thread: MobileConversationThread = {
  threadRef: "thread.actions",
  title: "Workspace actions",
  status: "active",
  messageCount: 1,
  lastMessageAt: now,
  updatedAt: now,
  version: 1,
  messages: [{
    messageRef: "message.actions",
    threadRef: "thread.actions",
    body: "Keep destructive actions explicit",
    createdAt: now,
    updatedAt: now,
    version: 1,
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
const report = async (
  program: ReturnType<typeof buildHomeProgram>,
  name: string,
  payload: string | Record<string, string> = {},
) => {
  await Effect.runPromise(program.report(IntentRef(name, StaticPayload(payload))) as Effect.Effect<unknown>)
  await Effect.runPromise(settle)
}

describe("contract openagents_mobile.workspace_row_actions.v1", () => {
  const setup = () => {
    const calls: string[] = []
    let current = { ...thread }
    const host: MobileConversationHost = {
      listThreads: async () => current.status === "active" ? [current] : [],
      listArchivedThreads: async () => current.status === "archived" ? [current] : [],
      newThread: async () => ({ ok: true, thread }),
      openThread: async () => current,
      sendMessage: async () => ({ ok: true, thread: current }),
      updateThread: async input => {
        calls.push(input.action)
        current = {
          ...current,
          status: input.action === "archive"
            ? "archived"
            : input.action === "restore"
              ? "active"
              : input.action === "delete"
                ? "deleted"
                : current.status,
          version: current.version + 1,
        }
        return { ok: true, thread: current }
      },
    }
    const conversation: Extract<MobileConversationSelection, { mode: "sync" }> = {
      mode: "sync",
      host,
      threads: [thread],
      archivedThreads: [],
      activeThread: thread,
    }
    return { calls, program: buildHomeProgram({ conversation, workspaceWidth: 390 }) }
  }

  test("rows expose reversible full-swipe and destructive press fallback through typed data", () => {
    const { program } = setup()
    const drawer = JSON.stringify(renderDrawerView(program.initialState))
    expect(drawer).toContain('"_tag":"SwipeableListItem"')
    expect(drawer).toContain('"fullSwipeActionId":"archive:thread.actions"')
    expect(drawer).toContain('"label":"Archive"')
    expect(drawer).toContain('"label":"Delete"')
    expect(drawer).not.toContain('"fullSwipeActionId":"delete:thread.actions"')
  })

  test("compact More presents one dismissable native sheet and restores navigation focus", async () => {
    const { program } = setup()
    await report(program, "WorkspaceRowActionsToggled", { threadRef: thread.threadRef })
    const open = JSON.stringify(renderDrawerView(await Effect.runPromise(lastState(program))))
    expect(open).toContain('"_tag":"Sheet"')
    expect(open).toContain('"open":true')
    expect(open).toContain('"presentationDetents":["half","full"]')
    await report(program, "WorkspaceLifecycleSheetDismissed")
    expect(await Effect.runPromise(lastState(program))).toMatchObject({
      workspaceFocusTarget: "navigation",
      threadLifecycle: { actionThreadRef: null, deleteConfirmThreadRef: null },
    })
  })

  test("rejects foreign/invalid actions and requires explicit delete confirmation", async () => {
    const { calls, program } = setup()
    await report(program, "WorkspaceRowActionSelected", "restore:thread.actions")
    await report(program, "WorkspaceRowActionSelected", "archive:thread.foreign")
    await report(program, "WorkspaceRowActionSelected", "bogus:thread.actions")
    expect(calls).toEqual([])
    await report(program, "WorkspaceRowActionSelected", "delete:thread.actions")
    expect(calls).toEqual([])
    expect(await Effect.runPromise(lastState(program))).toMatchObject({
      threadLifecycle: { deleteConfirmThreadRef: "thread.actions", pendingAction: null },
    })
    await report(program, "ConversationThreadDeleteConfirmed")
    expect(calls).toEqual(["delete"])
  })

  test("archive action confirms writeback and archived row switches full-swipe to restore", async () => {
    const { calls, program } = setup()
    await report(program, "WorkspaceRowActionSelected", "archive:thread.actions")
    expect(calls).toEqual(["archive"])
    const archivedState = await Effect.runPromise(lastState(program))
    const drawer = JSON.stringify(renderDrawerView({
      ...archivedState,
      workspaceStatusFilter: "archived",
    }))
    expect(drawer).toContain('"fullSwipeActionId":"restore:thread.actions"')
  })
})
