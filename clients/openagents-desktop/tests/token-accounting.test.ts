import { describe, expect, test } from "bun:test"

import {
  OPENAGENTS_DESKTOP_TOKEN_MISSING_ACCOUNTING_BLOCKER,
  assignmentRefsFromSpool,
  parseTokenFailureSpool,
  verificationFromProofPayload,
} from "../src/shared/token-accounting.js"

describe("openagents desktop token accounting", () => {
  test("detects public-safe Codex turn report failure spool rows", () => {
    const spool = parseTokenFailureSpool({
      byteLength: 512,
      path: "/Users/me/.pylon-fable/codex-turn-report-failures.jsonl",
      text: [
        JSON.stringify({
          observedAt: "2026-06-29T12:24:00.000Z",
          error: "Pylon Codex turn ingest failed (503)",
          report: {
            assignmentRef: "assignment.public.khala_coding.issue_7594",
            leaseRef: "lease.public.test",
            pylonRef: "pylon.public.test",
            turnIndex: 1,
            usage: {
              inputTokens: 100,
              outputTokens: 20,
              reasoningOutputTokens: 3,
            },
          },
        }),
        "not-json",
      ].join("\n"),
    })

    expect(spool).toMatchObject({
      byteLength: 512,
      exists: true,
      lineCount: 2,
      path: "/Users/me/.pylon-fable/codex-turn-report-failures.jsonl",
    })
    expect(spool.reports).toEqual([
      {
        assignmentRef: "assignment.public.khala_coding.issue_7594",
        error: "Pylon Codex turn ingest failed (503)",
        observedAt: "2026-06-29T12:24:00.000Z",
        totalTokens: 123,
        turnIndex: 1,
      },
    ])
    expect(assignmentRefsFromSpool(spool)).toEqual([
      "assignment.public.khala_coding.issue_7594",
    ])
  })

  test("normalizes exact proof totals and missing-accounting blockers", () => {
    const observedAt = "2026-06-29T12:30:00.000Z"
    const exact = verificationFromProofPayload(
      "assignment.public.khala_coding.issue_7594",
      {
        tokenUsage: {
          cacheReadTokens: 4,
          demandKind: "own_capacity",
          demandSource: "khala_coding_delegation",
          inputTokens: 100,
          model: "openagents/pylon-codex",
          outputTokens: 20,
          provider: "pylon-codex-own-capacity",
          reasoningTokens: 3,
          rowCount: 2,
          totalTokens: 123,
          usageTruth: "exact",
        },
      },
      observedAt,
    )

    expect(exact).toMatchObject({
      ok: true,
      assignmentRef: "assignment.public.khala_coding.issue_7594",
      rowCount: 2,
      totalTokens: 123,
      usageTruth: "exact",
    })

    const missing = verificationFromProofPayload(
      "assignment.public.khala_coding.issue_7594",
      {
        tokenUsage: {
          demandKind: "own_capacity",
          demandSource: "khala_coding_delegation",
          model: "openagents/pylon-codex",
          provider: "pylon-codex-own-capacity",
          rowCount: 0,
          totalTokens: 0,
          usageTruth: "exact",
        },
      },
      observedAt,
    )

    expect(missing).toEqual({
      ok: false,
      assignmentRef: "assignment.public.khala_coding.issue_7594",
      blockerRef: OPENAGENTS_DESKTOP_TOKEN_MISSING_ACCOUNTING_BLOCKER,
      observedAt,
    })
  })
})
