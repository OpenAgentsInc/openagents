import { describe, expect, test } from "bun:test"

import {
  RETENTION_POLICIES,
  classifyRecord,
  decideDeletion,
  type RetentionRecord,
} from "../src/tas/retention"

const nowMs = Date.UTC(2026, 5, 13, 12, 0, 0)

const record = (overrides: Partial<RetentionRecord> = {}): RetentionRecord => ({
  id: "record.fixture.retention",
  createdAtMs: nowMs,
  retentionClass: "standard",
  content: {
    privatePayload: "delete me",
  },
  ...overrides,
})

describe("tas retention and deletion core", () => {
  test("keeps records within their retention age", () => {
    const input = record({
      createdAtMs: nowMs - RETENTION_POLICIES.standard.maxAgeMs + 1,
    })

    expect(decideDeletion(input, nowMs)).toEqual({
      action: "keep",
      reason: "within_retention_window",
      retentionClass: "standard",
      expiresAtMs: input.createdAtMs + RETENTION_POLICIES.standard.maxAgeMs,
    })
  })

  test("deletes expired records by producing tombstones and invalidating projections", () => {
    expect(
      decideDeletion(
        record({
          id: "record.fixture.expired",
          createdAtMs: nowMs - RETENTION_POLICIES.standard.maxAgeMs,
        }),
        nowMs,
      ),
    ).toEqual({
      action: "tombstone",
      reason: "retention_window_expired",
      retentionClass: "standard",
      tombstone: {
        id: "record.fixture.expired",
        deletedAt: nowMs,
      },
      projectionInvalidation: {
        invalidate: true,
        recordId: "record.fixture.expired",
        reason: "record_tombstoned",
      },
    })
  })

  test("tombstones do not retain deleted content", () => {
    const decision = decideDeletion(
      record({
        id: "record.fixture.secret",
        createdAtMs: nowMs - RETENTION_POLICIES.ephemeral.maxAgeMs,
        retentionClass: "ephemeral",
        content: "secret payload",
      }),
      nowMs,
    )

    expect(decision.action).toBe("tombstone")
    if (decision.action !== "keep") {
      expect(decision.tombstone).toEqual({
        id: "record.fixture.secret",
        deletedAt: nowMs,
      })
      expect(decision.tombstone).not.toHaveProperty("content")
      expect(JSON.stringify(decision.tombstone)).not.toContain("secret payload")
    }
  })

  test("audit class is retained longer than standard and ephemeral classes", () => {
    expect(RETENTION_POLICIES.audit.maxAgeMs).toBeGreaterThan(
      RETENTION_POLICIES.standard.maxAgeMs,
    )
    expect(RETENTION_POLICIES.standard.maxAgeMs).toBeGreaterThan(
      RETENTION_POLICIES.ephemeral.maxAgeMs,
    )

    expect(classifyRecord(record({ retentionClass: undefined, kind: "product_receipt" }))).toBe(
      "audit",
    )
    expect(
      decideDeletion(
        record({
          createdAtMs: nowMs - RETENTION_POLICIES.standard.maxAgeMs,
          retentionClass: "audit",
        }),
        nowMs,
      ),
    ).toMatchObject({
      action: "keep",
      reason: "within_retention_window",
      retentionClass: "audit",
    })
  })
})
