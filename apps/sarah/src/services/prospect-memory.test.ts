/**
 * KHS-2 (#8601) prospect memory v1 tests. Tests run without a database —
 * the module must fail soft (null memory), and the pure distillation +
 * formatting layers are unit-tested on fixture rows.
 */

import { describe, expect, test } from "bun:test"

import {
  distillProspectFacts,
  formatMemoryContext,
  getProspectMemoryContext,
  PROSPECT_MEMORY_MAX_CHARS,
  prospectRefAliases,
} from "./prospect-memory.ts"
import type { SarahMemoryTurnRow } from "./prospect-memory.ts"

const turn = (
  id: string,
  role: "user" | "assistant",
  text: string,
): SarahMemoryTurnRow => ({
  id,
  role,
  modality: "text",
  text,
  recordedAt: "2026-07-08T12:00:00.000Z",
})

describe("prospect memory scoping (KHS-3 seam)", () => {
  test("entry point takes exactly one prospect ref", () => {
    expect(getProspectMemoryContext.length).toBe(1)
  })

  test("aliases are deterministic re-encodings of ONE identity", () => {
    expect(prospectRefAliases("abc123")).toEqual(["abc123", "prospect:abc123"])
    expect(prospectRefAliases("prospect:abc123")).toEqual([
      "abc123",
      "prospect:abc123",
    ])
  })

  test("visitor refs never alias to a cookie ref", () => {
    expect(prospectRefAliases("visitor:9f2c")).toEqual(["visitor:9f2c"])
  })

  test("empty ref yields no aliases (no unscoped query is possible)", () => {
    expect(prospectRefAliases("")).toEqual([])
    expect(prospectRefAliases("   ")).toEqual([])
  })
})

describe("getProspectMemoryContext without a database", () => {
  test("fails soft to null", async () => {
    delete process.env.SARAH_DATABASE_URL
    delete process.env.KHALA_SYNC_DATABASE_URL
    expect(await getProspectMemoryContext("no-db-prospect")).toBeNull()
  })

  test("empty ref is null, not an unscoped read", async () => {
    expect(await getProspectMemoryContext("")).toBeNull()
  })
})

describe("distillProspectFacts (deterministic v1)", () => {
  test("captures stated company/role/need/stack with provenance turn ids", () => {
    const facts = distillProspectFacts([
      turn("1", "assistant", "Hi, I'm Sarah — how can I help?"),
      turn("2", "user", "I work at Nimbus Robotics, we build warehouse bots"),
      turn("3", "user", "I'm the CTO there"),
      turn("4", "user", "We're looking for an AI sales agent for our site"),
      turn("5", "user", "Our stack is TypeScript on GCP"),
    ])
    expect(facts.map((fact) => fact.sourceTurnId)).toEqual(["2", "3", "4", "5"])
    expect(facts[0]!.fact).toStartWith("company:")
    expect(facts[0]!.fact).toContain("Nimbus Robotics")
    expect(facts[1]!.fact).toStartWith("role:")
    expect(facts[2]!.fact).toStartWith("need:")
    expect(facts[3]!.fact).toStartWith("stack:")
    for (const fact of facts) expect(fact.at).toBeTruthy()
  })

  test("never distills from assistant turns and never invents facts", () => {
    const facts = distillProspectFacts([
      turn("1", "assistant", "We're looking for customers like you!"),
      turn("2", "user", "hello"),
      turn("3", "user", "what does openagents do?"),
    ])
    expect(facts).toEqual([])
  })

  test("facts are verbatim short quotes, bounded", () => {
    const long = `we need ${"a very long requirement ".repeat(30)}`
    const [fact] = distillProspectFacts([turn("7", "user", long)])
    expect(fact!.fact.length).toBeLessThanOrEqual(160)
    expect(fact!.fact).toContain('"we need')
  })
})

describe("formatMemoryContext (pure)", () => {
  test("null when there is nothing to remember", () => {
    expect(
      formatMemoryContext({ prospectRef: "abc", facts: [], recentTurns: [] }),
    ).toBeNull()
  })

  test("includes contact, facts with turn ids, and a recap", () => {
    const rows = [
      turn("2", "user", "I work at Nimbus Robotics"),
      turn("3", "assistant", "Great — what are you evaluating?"),
      turn("4", "user", "We need an AI sales agent"),
    ]
    const block = formatMemoryContext({
      prospectRef: "abc",
      contact: { contactEmail: "kim@nimbus.io", contactId: null },
      facts: distillProspectFacts(rows),
      recentTurns: rows,
    })
    expect(block).toContain("[prospect memory")
    expect(block).toContain("kim@nimbus.io")
    expect(block).toContain("(turn 2)")
    expect(block).toContain('sarah: "Great — what are you evaluating?"')
    expect(block).toContain('user: "We need an AI sales agent"')
    expect(block).toContain("Do not read turn ids aloud")
  })

  test("block is capped so prompts stay lean", () => {
    const rows = Array.from({ length: 40 }, (_, i) =>
      turn(
        String(i),
        i % 2 ? "assistant" : "user",
        `we need option ${i}: ${"detail ".repeat(40)}`,
      ),
    )
    const block = formatMemoryContext({
      prospectRef: "abc",
      facts: distillProspectFacts(rows),
      recentTurns: rows,
    })
    expect(block).not.toBeNull()
    expect(block!.length).toBeLessThanOrEqual(PROSPECT_MEMORY_MAX_CHARS)
    expect(block).toStartWith("[prospect memory")
  })
})
