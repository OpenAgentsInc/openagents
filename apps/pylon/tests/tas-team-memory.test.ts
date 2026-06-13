import { describe, expect, test } from "bun:test"

import {
  upsert,
  visibleTo,
  type MemoryEntry,
} from "../src/tas/team-memory"

const nowMs = Date.UTC(2026, 5, 13, 12, 0, 0)
const minuteMs = 60 * 1_000

const memory = (overrides: Partial<MemoryEntry> = {}): MemoryEntry => ({
  ref: "team-memory.fixture.default",
  scope: "team",
  authorRef: "agent.fixture.author",
  createdAt: nowMs,
  digestRef: "digest.fixture.default",
  ...overrides,
})

describe("tas team memory core", () => {
  test("deduplicates by ref and applies last-write-wins by createdAt", () => {
    const original = [
      memory({ ref: "team-memory.fixture.first" }),
      memory({
        ref: "team-memory.fixture.target",
        createdAt: nowMs - minuteMs,
        digestRef: "digest.fixture.old",
      }),
      memory({ ref: "team-memory.fixture.third" }),
    ]

    const stale = upsert(
      original,
      memory({
        ref: "team-memory.fixture.target",
        createdAt: nowMs - 2 * minuteMs,
        digestRef: "digest.fixture.stale",
      }),
    )

    expect(stale).toEqual(original)

    const updated = upsert(
      original,
      memory({
        ref: "team-memory.fixture.target",
        scope: "private",
        createdAt: nowMs,
        digestRef: "digest.fixture.new",
      }),
    )

    expect(updated.map(({ ref }) => ref)).toEqual([
      "team-memory.fixture.first",
      "team-memory.fixture.target",
      "team-memory.fixture.third",
    ])
    expect(updated[1]).toEqual(
      memory({
        ref: "team-memory.fixture.target",
        scope: "private",
        createdAt: nowMs,
        digestRef: "digest.fixture.new",
      }),
    )
    expect(original[1]?.digestRef).toBe("digest.fixture.old")
  })

  test("returns team memory refs to members", () => {
    const entries = [
      memory({ ref: "team-memory.fixture.team" }),
      memory({
        ref: "team-memory.fixture.private-other",
        scope: "private",
        authorRef: "agent.fixture.other",
      }),
    ]

    expect(
      visibleTo(entries, {
        ref: "agent.fixture.viewer",
        isMember: true,
      }),
    ).toEqual(["team-memory.fixture.team"])
  })

  test("hides private memory from non-authors", () => {
    const entries = [
      memory({
        ref: "team-memory.fixture.private-author",
        scope: "private",
        authorRef: "agent.fixture.author",
      }),
      memory({
        ref: "team-memory.fixture.private-other",
        scope: "private",
        authorRef: "agent.fixture.other",
      }),
    ]

    expect(
      visibleTo(entries, {
        ref: "agent.fixture.author",
        isMember: false,
      }),
    ).toEqual(["team-memory.fixture.private-author"])
    expect(
      visibleTo(entries, {
        ref: "agent.fixture.viewer",
        isMember: false,
      }),
    ).toEqual([])
  })
})
