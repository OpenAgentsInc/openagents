import { describe, expect, test } from "vite-plus/test"
import type { CodexHistoryItem } from "./codex-history-contract.ts"
import { historyForkFetchPlan, historyForkSeed } from "./history-thread-actions.ts"

const item = (sequence: number, kind: CodexHistoryItem["kind"], summary = `${kind} ${sequence}`): CodexHistoryItem => ({
  itemRef: `claude:source:${sequence}`,
  threadRef: "claude:source",
  sequence,
  timestamp: `2026-07-12T00:00:${String(sequence).padStart(2, "0")}Z`,
  kind,
  label: kind,
  summary,
  status: "completed",
  fields: [],
  redacted: false,
  sourceType: `fixture/${kind}`,
})

describe("H2 bounded fork seed", () => {
  test("plans a bounded host re-read ending exactly at the requested item", () => {
    expect(historyForkFetchPlan(2_000, 1_700)).toEqual({ offset: 1_201, limit: 500, throughSequence: 1_700 })
    expect(historyForkFetchPlan(30, null)).toEqual({ offset: 0, limit: 30, throughSequence: 29 })
    expect(historyForkFetchPlan(0, null)).toBeNull()
  })

  test("copies only the last 12 user/assistant messages through the cutoff and bounds text", () => {
    const source = [
      ...Array.from({ length: 15 }, (_, sequence) => item(sequence, sequence % 2 === 0 ? "user_message" : "assistant_message", sequence === 10 ? "x".repeat(3_000) : `message ${sequence}`)),
      item(15, "tool_call", "never seed tool payloads"),
      item(16, "assistant_message", "after cutoff"),
    ]
    const snapshot = structuredClone(source)
    const seed = historyForkSeed(source, 14)
    expect(seed).toHaveLength(12)
    expect(seed[0]?.text).toBe("message 3")
    expect(seed.at(-1)?.text).toBe("message 14")
    expect(seed.find(note => note.key.endsWith(":10"))?.text).toHaveLength(2_000)
    expect(seed.some(note => note.text.includes("tool payloads"))).toBe(false)
    expect(source).toEqual(snapshot)
  })
})
