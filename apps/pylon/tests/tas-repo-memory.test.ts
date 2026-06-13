import { describe, expect, test } from "bun:test"

import {
  mergeObservation,
  recallForRepo,
  type RepoMemoryRecord,
} from "../src/tas/repo-memory"

const nowMs = Date.UTC(2026, 5, 13, 12, 0, 0)
const minuteMs = 60 * 1_000

const memory = (overrides: Partial<RepoMemoryRecord> = {}): RepoMemoryRecord => ({
  repoRef: "repo.fixture.primary",
  factRef: "repo-memory.fixture.default",
  kind: "note",
  confidence: 0.5,
  observedAt: nowMs - 30 * minuteMs,
  ...overrides,
})

describe("tas repo memory core", () => {
  test("orders recall by confidence then recency", () => {
    const records = [
      memory({
        factRef: "repo-memory.fixture.low_confidence_recent",
        confidence: 0.4,
        observedAt: nowMs,
      }),
      memory({
        factRef: "repo-memory.fixture.high_confidence_old",
        confidence: 0.9,
        observedAt: nowMs - 60 * minuteMs,
      }),
      memory({
        factRef: "repo-memory.fixture.high_confidence_recent",
        confidence: 0.9,
        observedAt: nowMs - minuteMs,
      }),
    ]

    expect(recallForRepo(records, "repo.fixture.primary", { nowMs }).map(({ factRef }) => factRef)).toEqual([
      "repo-memory.fixture.high_confidence_recent",
      "repo-memory.fixture.high_confidence_old",
      "repo-memory.fixture.low_confidence_recent",
    ])
  })

  test("merge keeps the higher confidence observation for a fact ref", () => {
    const original = [
      memory({ factRef: "repo-memory.fixture.first", confidence: 0.8 }),
      memory({
        factRef: "repo-memory.fixture.target",
        kind: "command",
        confidence: 0.6,
        observedAt: nowMs - 20 * minuteMs,
      }),
    ]

    const lowerConfidence = mergeObservation(
      original,
      memory({
        factRef: "repo-memory.fixture.target",
        kind: "layout",
        confidence: 0.4,
        observedAt: nowMs,
      }),
    )

    expect(lowerConfidence).toBe(original)
    expect(lowerConfidence[1]).toMatchObject({
      kind: "command",
      confidence: 0.6,
      observedAt: nowMs - 20 * minuteMs,
    })

    const higherConfidence = mergeObservation(
      original,
      memory({
        factRef: "repo-memory.fixture.target",
        kind: "layout",
        confidence: 0.9,
        observedAt: nowMs,
      }),
    )

    expect(higherConfidence.map(({ factRef }) => factRef)).toEqual([
      "repo-memory.fixture.first",
      "repo-memory.fixture.target",
    ])
    expect(higherConfidence[1]).toMatchObject({
      kind: "layout",
      confidence: 0.9,
      observedAt: nowMs,
    })
    expect(original[1]).toMatchObject({
      kind: "command",
      confidence: 0.6,
    })
  })

  test("recall is scoped per repository ref", () => {
    const records = [
      memory({ factRef: "repo-memory.fixture.primary" }),
      memory({
        repoRef: "repo.fixture.other",
        factRef: "repo-memory.fixture.other",
        confidence: 1,
        observedAt: nowMs,
      }),
    ]

    expect(recallForRepo(records, "repo.fixture.primary", { nowMs })).toEqual([
      memory({ factRef: "repo-memory.fixture.primary" }),
    ])
    expect(recallForRepo(records, "repo.fixture.other", { nowMs }).map(({ factRef }) => factRef)).toEqual([
      "repo-memory.fixture.other",
    ])
  })
})
