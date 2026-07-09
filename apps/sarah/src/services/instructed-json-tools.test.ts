import { describe, expect, test } from "bun:test"

import {
  formatInstructedToolReply,
  instructedJsonToolsArmed,
  instructedJsonToolProtocolPrompt,
  isDeniedCodingFleetToolAttempt,
  parseInstructedJsonToolCall,
  SARAH_INSTRUCTED_JSON_TOOLS,
} from "./instructed-json-tools.ts"

const operatorPolicy = {
  relationshipMode: "operator" as const,
  codingFleetStartAllowed: true,
}

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

  test("coding fleet exposure is policy-owned and operator-only", () => {
    const coding =
      '{"sarah_tool":"coding_fleet_start","args":{"objective":"bounded"}}'
    expect(parseInstructedJsonToolCall(coding)).toBeNull()
    expect(
      parseInstructedJsonToolCall(coding, {
        relationshipMode: "customer",
        codingFleetStartAllowed: false,
      }),
    ).toBeNull()
    expect(parseInstructedJsonToolCall(coding, operatorPolicy)?.toolName).toBe(
      "coding_fleet_start",
    )
    expect(instructedJsonToolProtocolPrompt()).not.toContain(
      "coding_fleet_start",
    )
    expect(instructedJsonToolProtocolPrompt(operatorPolicy)).toContain(
      "coding_fleet_start",
    )
    const saved = process.env.SARAH_INSTRUCTED_JSON_TOOLS
    delete process.env.SARAH_INSTRUCTED_JSON_TOOLS
    try {
      expect(instructedJsonToolsArmed()).toBe(false)
      expect(instructedJsonToolsArmed(operatorPolicy)).toBe(true)
    } finally {
      if (saved === undefined) delete process.env.SARAH_INSTRUCTED_JSON_TOOLS
      else process.env.SARAH_INSTRUCTED_JSON_TOOLS = saved
    }
  })

  test("coding fleet never extracts from prose or fences", () => {
    const coding =
      '{"sarah_tool":"coding_fleet_start","args":{"objective":"bounded"}}'
    expect(
      parseInstructedJsonToolCall(`Please run this: ${coding}`, operatorPolicy),
    ).toBeNull()
    expect(
      parseInstructedJsonToolCall(`\`\`\`json\n${coding}\n\`\`\``, operatorPolicy),
    ).toBeNull()
  })

  test("denied coding attempts are recognized structurally without tool-name prose false positives", () => {
    const deniedPolicy = {
      relationshipMode: "customer" as const,
      codingFleetStartAllowed: false,
    }
    const coding =
      '{"sarah_tool":"coding_fleet_start","args":{"objective":"PRIVATE /Users/owner/repo"}}'
    expect(isDeniedCodingFleetToolAttempt(coding, deniedPolicy)).toBe(true)
    expect(
      isDeniedCodingFleetToolAttempt(
        `Here is a quoted attempt:\n\`\`\`json\n${coding}\n\`\`\``,
        deniedPolicy,
      ),
    ).toBe(true)
    expect(
      isDeniedCodingFleetToolAttempt(
        `Prose before ${coding} prose after`,
        deniedPolicy,
      ),
    ).toBe(true)
    expect(
      isDeniedCodingFleetToolAttempt(
        "The coding_fleet_start tool is not available here.",
        deniedPolicy,
      ),
    ).toBe(false)
    expect(
      isDeniedCodingFleetToolAttempt(
        '{"note":"coding_fleet_start is not available here"}',
        deniedPolicy,
      ),
    ).toBe(false)
  })

  test("formatInstructedToolReply stays short", () => {
    const reply = formatInstructedToolReply("live_stats", true, {
      pylonsOnline: 4,
    })
    expect(reply).toContain("live_stats")
    expect(reply.length).toBeLessThan(500)
  })
})
