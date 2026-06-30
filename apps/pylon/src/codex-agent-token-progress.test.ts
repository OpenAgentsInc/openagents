import { describe, expect, test } from "bun:test"

import {
  estimatedTokenDeltaFromCodexRawEvent,
  liveTokenTotalFromCodexRawEvent,
} from "./codex-agent-executor.js"

describe("Codex live token progress", () => {
  test("extracts exact cumulative token_count events when the SDK provides them", () => {
    expect(
      liveTokenTotalFromCodexRawEvent({
        type: "event_msg",
        payload: {
          type: "token_count",
          info: {
            total_token_usage: {
              input_tokens: 1000,
              output_tokens: 250,
              reasoning_output_tokens: 75,
            },
          },
        },
      }),
    ).toBe(1325)
  })

  test("estimates only known text-bearing raw event fields", () => {
    const estimate = estimatedTokenDeltaFromCodexRawEvent({
      type: "response_item",
      payload: {
        type: "message",
        content: [{ type: "output_text", text: "abcd efgh" }],
      },
      ignored_ref: "assignment.public.this_should_not_count",
    })

    expect(estimate).toBe(3)
  })
})
