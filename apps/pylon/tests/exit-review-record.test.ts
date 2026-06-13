import { describe, expect, test } from "bun:test"

import {
  buildExitReviewRecord,
  EXIT_REVIEW_RECORD_SCHEMA,
} from "../src/coordinator/exit-review-record"

describe("exit review record", () => {
  test("opens the door when every gate passes", () => {
    expect(
      buildExitReviewRecord({
        gates: [
          {
            name: "smoke",
            passed: true,
            evidenceRef: "docs/autopilot-coder/smoke.md",
          },
          { name: "copy-gate", passed: true },
        ],
        decidedBy: "release-coordinator",
        decidedAt: "2026-06-13T14:00:00.000Z",
      }),
    ).toEqual({
      schema: EXIT_REVIEW_RECORD_SCHEMA,
      decision: "open",
      gates: [
        {
          name: "smoke",
          passed: true,
          evidenceRef: "docs/autopilot-coder/smoke.md",
        },
        { name: "copy-gate", passed: true },
      ],
      blockers: [],
      decidedBy: "release-coordinator",
      decidedAt: "2026-06-13T14:00:00.000Z",
    })
  })

  test("holds the door when one gate fails", () => {
    const result = buildExitReviewRecord({
      gates: [
        { name: "smoke", passed: true },
        { name: "payment-boundary", passed: false },
      ],
      decidedBy: "release-coordinator",
      decidedAt: "2026-06-13T14:05:00.000Z",
    })

    expect(result.decision).toBe("hold")
    expect(result.blockers).toEqual(["payment-boundary"])
  })

  test("lists every failed gate as a blocker in gate order", () => {
    const result = buildExitReviewRecord({
      gates: [
        { name: "smoke", passed: false },
        { name: "copy-gate", passed: true },
        { name: "public-proof", passed: false },
      ],
      decidedBy: "release-coordinator",
      decidedAt: "2026-06-13T14:10:00.000Z",
    })

    expect(result).toMatchObject({
      decision: "hold",
      blockers: ["smoke", "public-proof"],
    })
  })

  test("preserves gate evidence references on both passed and failed gates", () => {
    const gates = [
      {
        name: "transcript-audit",
        passed: true,
        evidenceRef: "docs/transcripts/README.md",
      },
      {
        name: "mvp-checklist",
        passed: false,
        evidenceRef: "docs/autopilot-coder/m14-exit.md",
      },
    ]

    expect(
      buildExitReviewRecord({
        gates,
        decidedBy: "release-coordinator",
        decidedAt: "2026-06-13T14:15:00.000Z",
      }).gates,
    ).toEqual(gates)
  })

  test("uses the supplied decider and timestamp without generating time", () => {
    expect(
      buildExitReviewRecord({
        gates: [{ name: "smoke", passed: true }],
        decidedBy: "operator-role",
        decidedAt: "manual-time-value",
      }),
    ).toMatchObject({
      decision: "open",
      decidedBy: "operator-role",
      decidedAt: "manual-time-value",
    })
  })

  test("opens when there are no gates to block the decision", () => {
    expect(
      buildExitReviewRecord({
        gates: [],
        decidedBy: "release-coordinator",
        decidedAt: "2026-06-13T14:20:00.000Z",
      }),
    ).toEqual({
      schema: EXIT_REVIEW_RECORD_SCHEMA,
      decision: "open",
      gates: [],
      blockers: [],
      decidedBy: "release-coordinator",
      decidedAt: "2026-06-13T14:20:00.000Z",
    })
  })
})
