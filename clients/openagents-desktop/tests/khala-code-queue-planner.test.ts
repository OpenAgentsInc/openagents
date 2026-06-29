import { describe, expect, test } from "bun:test"

import {
  khalaCodeCandidateRef,
  planKhalaCodeQueue,
  type KhalaCodeQueueCandidate,
} from "../src/shared/khala-code-queue-planner.js"

const now = "2026-06-29T18:00:00.000Z"

const candidate = (
  overrides: Partial<KhalaCodeQueueCandidate> &
    Pick<KhalaCodeQueueCandidate, "kind" | "number" | "title">,
): KhalaCodeQueueCandidate => ({
  laneRefs: ["lane.pylon_codex_rate_limits"],
  priority: 50,
  repository: "OpenAgentsInc/openagents",
  state: "open",
  ...overrides,
})

describe("Khala Code queue planner", () => {
  test("targets the requested priority lane before lower-priority candidates", () => {
    const highValuePr = candidate({
      kind: "pull_request",
      number: 7557,
      priority: 100,
      title: "Expose Codex fleet quota cooldown state",
    })
    const genericIssue = candidate({
      kind: "issue",
      laneRefs: ["lane.generic_burndown"],
      number: 7595,
      priority: 999,
      title: "Build unrelated queue work",
    })

    const plan = planKhalaCodeQueue({
      candidates: [genericIssue, highValuePr],
      lanes: [
        {
          laneRef: "lane.pylon_codex_rate_limits",
          priority: 100,
          title: "Pylon/Codex rate limits",
        },
        {
          laneRef: "lane.generic_burndown",
          priority: 1,
          title: "Generic burndown",
        },
      ],
      maxClaims: 1,
      now,
      targetLaneRef: "lane.pylon_codex_rate_limits",
    })

    expect(plan.lane?.title).toBe("Pylon/Codex rate limits")
    expect(plan.claims).toEqual([
      expect.objectContaining({
        candidateRef: khalaCodeCandidateRef(highValuePr),
        laneRef: "lane.pylon_codex_rate_limits",
      }),
    ])
    expect(plan.rows.map(row => [row.candidate.number, row.decision])).toEqual([
      [7557, "claim"],
      [7595, "skip"],
    ])
    expect(plan.rows[1]?.reason).toEqual({
      kind: "skip",
      reason: "lane_mismatch",
    })
  })

  test("skips closed, merged, and superseded candidates before dispatch", () => {
    const plan = planKhalaCodeQueue({
      candidates: [
        candidate({
          kind: "pull_request",
          mergedAt: "2026-06-29T17:00:00.000Z",
          number: 7486,
          state: "closed",
          title: "Harden Pylon agent runner registry contract",
        }),
        candidate({
          closedAt: "2026-06-29T17:05:00.000Z",
          kind: "pull_request",
          number: 6831,
          state: "closed",
          title: "Close stale queue work",
        }),
        candidate({
          kind: "issue",
          number: 7590,
          state: "closed",
          title: "Parent planning issue",
        }),
        candidate({
          kind: "pull_request",
          number: 7558,
          supersededBy: "OpenAgentsInc/openagents#pr.7560",
          title: "Classify Codex account execution refusals",
        }),
      ],
      lanes: [
        {
          laneRef: "lane.pylon_codex_rate_limits",
          priority: 100,
          title: "Pylon/Codex rate limits",
        },
      ],
      maxClaims: 4,
      now,
      targetLaneRef: "lane.pylon_codex_rate_limits",
    })

    expect(plan.claims).toEqual([])
    expect(
      Object.fromEntries(plan.rows.map(row => [row.candidate.number, row.reason])),
    ).toEqual({
      7486: { kind: "merge", reason: "merged_upstream" },
      6831: { kind: "close", reason: "closed_unmerged" },
      7590: { kind: "close", reason: "issue_closed" },
      7558: { kind: "skip", reason: "superseded" },
    })
  })

  test("prevents duplicate claims for the same PR or issue", () => {
    const pr = candidate({
      kind: "pull_request",
      number: 7579,
      title: "Expose Codex quota reset policy status",
    })
    const issue = candidate({
      kind: "issue",
      number: 7595,
      title: "Build GitHub PR/issue queue planner",
    })

    const plan = planKhalaCodeQueue({
      activeClaims: [
        {
          candidateRef: khalaCodeCandidateRef(pr),
          claimedAt: "2026-06-29T17:55:00.000Z",
          claimRef: "claim.existing",
          laneRef: "lane.pylon_codex_rate_limits",
        },
      ],
      candidates: [pr, issue, issue],
      lanes: [
        {
          laneRef: "lane.pylon_codex_rate_limits",
          priority: 100,
          title: "Pylon/Codex rate limits",
        },
      ],
      maxClaims: 2,
      now,
      targetLaneRef: "lane.pylon_codex_rate_limits",
    })

    expect(plan.claims).toHaveLength(1)
    expect(plan.claims[0]?.candidateRef).toBe(khalaCodeCandidateRef(issue))
    expect(plan.rows.map(row => row.reason)).toEqual([
      { kind: "skip", reason: "duplicate_claim" },
      { kind: "claim", reason: "priority_lane_capacity" },
      { kind: "skip", reason: "duplicate_candidate" },
    ])
  })

  test("records typed retry and block reasons without claiming", () => {
    const plan = planKhalaCodeQueue({
      candidates: [
        candidate({
          dispatch: {
            reason: "provider_cooldown",
            retryAt: "2026-06-29T20:00:00.000Z",
            state: "retry",
          },
          kind: "pull_request",
          number: 7523,
          title: "Parse provider quota reset hints",
        }),
        candidate({
          dispatch: {
            reason: "missing_verification",
            state: "blocked",
          },
          kind: "issue",
          number: 7600,
          title: "Needs a verifier before fanout",
        }),
      ],
      lanes: [
        {
          laneRef: "lane.pylon_codex_rate_limits",
          priority: 100,
          title: "Pylon/Codex rate limits",
        },
      ],
      maxClaims: 2,
      now,
      targetLaneRef: "lane.pylon_codex_rate_limits",
    })

    expect(plan.claims).toEqual([])
    expect(plan.rows.map(row => row.reason)).toEqual([
      {
        kind: "retry",
        reason: "provider_cooldown",
        retryAt: "2026-06-29T20:00:00.000Z",
      },
      { kind: "block", reason: "missing_verification" },
    ])
  })
})
