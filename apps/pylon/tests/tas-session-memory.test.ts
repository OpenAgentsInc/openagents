import { describe, expect, test } from "bun:test"

import {
  addOrUpdate,
  recallOrder,
  type MemoryRecord,
} from "../src/tas/session-memory"

const nowMs = Date.UTC(2026, 5, 13, 12, 0, 0)
const minuteMs = 60 * 1_000

const memory = (overrides: Partial<MemoryRecord> = {}): MemoryRecord => ({
  ref: "memory.fixture.default",
  kind: "project",
  createdAt: nowMs - 60 * minuteMs,
  lastUsedAt: nowMs - 60 * minuteMs,
  salience: 0.5,
  ...overrides,
})

describe("tas session memory core", () => {
  test("orders recall by deterministic salience and recency score", () => {
    const records = [
      memory({
        ref: "memory.fixture.low_salience_recent",
        lastUsedAt: nowMs,
        salience: 0.1,
      }),
      memory({
        ref: "memory.fixture.high_salience_old",
        lastUsedAt: nowMs - 60 * minuteMs,
        salience: 0.9,
      }),
      memory({
        ref: "memory.fixture.medium_salience_recent",
        lastUsedAt: nowMs - minuteMs,
        salience: 0.4,
      }),
    ]

    expect(recallOrder(records, { nowMs }).map(({ ref }) => ref)).toEqual([
      "memory.fixture.low_salience_recent",
      "memory.fixture.high_salience_old",
      "memory.fixture.medium_salience_recent",
    ])
  })

  test("deduplicates by ref and updates the existing position", () => {
    const original = [
      memory({ ref: "memory.fixture.first", lastUsedAt: nowMs - 10 * minuteMs }),
      memory({
        ref: "memory.fixture.target",
        kind: "feedback",
        lastUsedAt: nowMs - 20 * minuteMs,
        salience: 0.2,
      }),
      memory({ ref: "memory.fixture.third", lastUsedAt: nowMs - 30 * minuteMs }),
    ]

    const updated = addOrUpdate(
      original,
      memory({
        ref: "memory.fixture.target",
        kind: "project",
        lastUsedAt: nowMs,
        salience: 0.8,
      }),
    )

    expect(updated.map(({ ref }) => ref)).toEqual([
      "memory.fixture.first",
      "memory.fixture.target",
      "memory.fixture.third",
    ])
    expect(updated[1]).toMatchObject({
      ref: "memory.fixture.target",
      kind: "project",
      lastUsedAt: nowMs,
      salience: 0.8,
    })
    expect(original[1]?.lastUsedAt).toBe(nowMs - 20 * minuteMs)
  })

  test("keeps input order stable for recall ties", () => {
    const records = [
      memory({ ref: "memory.fixture.a", lastUsedAt: nowMs, salience: 0.5 }),
      memory({ ref: "memory.fixture.b", lastUsedAt: nowMs, salience: 0.5 }),
      memory({ ref: "memory.fixture.c", lastUsedAt: nowMs, salience: 0.5 }),
    ]

    expect(recallOrder(records, { nowMs }).map(({ ref }) => ref)).toEqual([
      "memory.fixture.a",
      "memory.fixture.b",
      "memory.fixture.c",
    ])
  })
})
