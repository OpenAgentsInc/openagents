import { describe, expect, test } from "bun:test"
import {
  khalaCodeHeadlessThreadStarted,
  khalaCodeHeadlessTurnCompleted,
  khalaCodeHeadlessTurnStarted,
  projectKhalaCodeDesktopEventToThreadEvents,
  stringifyKhalaCodeHeadlessThreadEvent,
} from "../src/shared/headless-events"
import type { KhalaCodeDesktopChatTurnEvent } from "../src/shared/rpc"

describe("Khala Code headless ThreadEvent schema", () => {
  test("pins thread, turn, message, tool, and usage wire shape", () => {
    const events = [
      khalaCodeHeadlessThreadStarted({
        sessionId: "session-1",
        threadId: "thread-1",
      }),
      khalaCodeHeadlessTurnStarted("turn-1"),
      ...projectKhalaCodeDesktopEventToThreadEvents({
        message: { body: "", id: "assistant-1", role: "assistant" },
        turnId: "turn-1",
        type: "message_start",
      }),
      ...projectKhalaCodeDesktopEventToThreadEvents({
        delta: "Hel",
        messageId: "assistant-1",
        turnId: "turn-1",
        type: "message_delta",
      }),
      ...projectKhalaCodeDesktopEventToThreadEvents({
        event: {
          eventId: "turn-1.call-1.tool_started",
          invocationId: "call-1",
          kind: "tool_started",
          payload: { command: "printf hi", name: "exec_command" },
          sessionId: "session-1",
        },
        turnId: "turn-1",
        type: "tool_event",
      } satisfies KhalaCodeDesktopChatTurnEvent),
      ...projectKhalaCodeDesktopEventToThreadEvents({
        event: {
          eventId: "turn-1.call-1.stdout",
          invocationId: "call-1",
          kind: "stdout_chunk",
          payload: { text: "hi\n" },
          sessionId: "session-1",
        },
        turnId: "turn-1",
        type: "tool_event",
      } satisfies KhalaCodeDesktopChatTurnEvent),
      khalaCodeHeadlessTurnCompleted({
        finalMessage: "Hello",
        ok: true,
        turnId: "turn-1",
        usage: {
          cachedInput: 2,
          input: 11,
          output: 7,
          reasoningOutput: 3,
        },
      }),
    ].map(event => JSON.parse(stringifyKhalaCodeHeadlessThreadEvent(event)))

    expect(events).toEqual([
      {
        session_id: "session-1",
        thread_id: "thread-1",
        type: "thread.started",
      },
      {
        turn_id: "turn-1",
        type: "turn.started",
      },
      {
        item: {
          id: "assistant-1",
          kind: "message",
          role: "assistant",
        },
        turn_id: "turn-1",
        type: "item.started",
      },
      {
        delta: "Hel",
        item_id: "assistant-1",
        turn_id: "turn-1",
        type: "item.delta",
      },
      {
        item: {
          id: "call-1",
          kind: "command_execution",
          tool_name: "exec_command",
        },
        turn_id: "turn-1",
        type: "item.started",
      },
      {
        delta: "hi\n",
        item_id: "call-1",
        turn_id: "turn-1",
        type: "item.delta",
      },
      {
        final_message: "Hello",
        ok: true,
        turn_id: "turn-1",
        type: "turn.completed",
        usage: {
          cached_input: 2,
          input: 11,
          output: 7,
          reasoning_output: 3,
        },
      },
    ])
  })
})
