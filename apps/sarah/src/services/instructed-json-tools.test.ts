import { describe, expect, test } from "bun:test"

import {
  formatInstructedToolReply,
  parseInstructedJsonToolCall,
  SARAH_INSTRUCTED_JSON_TOOLS,
} from "./instructed-json-tools.ts"

describe("AV-3 instructed-JSON tool calling (#8598)", () => {
  test("parses bare sarah_tool JSON", () => {
    const parsed = parseInstructedJsonToolCall(
      '{"sarah_tool":"promise_lookup","args":{"query":"mobile"}}',
    )
    expect(parsed).not.toBeNull()
    expect(parsed!.toolName).toBe("promise_lookup")
    expect(parsed!.args.query).toBe("mobile")
  })

  test("parses fenced json and tool alias", () => {
    const parsed = parseInstructedJsonToolCall(
      'Sure.\n```json\n{"tool":"live_stats","args":{}}\n```\n',
    )
    expect(parsed?.toolName).toBe("live_stats")
  })

  test("rejects money-moving tools", () => {
    expect(
      parseInstructedJsonToolCall(
        '{"sarah_tool":"checkout_link_create","args":{"amount":10}}',
      ),
    ).toBeNull()
    expect(
      parseInstructedJsonToolCall(
        '{"sarah_tool":"deal_rules_evaluate","args":{}}',
      ),
    ).toBeNull()
  })

  test("rejects prose without JSON", () => {
    expect(parseInstructedJsonToolCall("Hello, how can I help?")).toBeNull()
  })

  test("allowed list is public-safe only", () => {
    expect(SARAH_INSTRUCTED_JSON_TOOLS).not.toContain("checkout_link_create")
    expect(SARAH_INSTRUCTED_JSON_TOOLS).toContain("human_handoff")
  })

  test("formatInstructedToolReply stays short", () => {
    const reply = formatInstructedToolReply("live_stats", true, {
      pylonsOnline: 4,
    })
    expect(reply).toContain("live_stats")
    expect(reply.length).toBeLessThan(500)
  })
})
