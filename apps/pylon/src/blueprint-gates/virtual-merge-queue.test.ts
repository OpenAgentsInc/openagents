import { describe, expect, test } from "bun:test"

import {
  projectVirtualMergeQueue,
  type VirtualMergeQueueCandidate,
} from "./virtual-merge-queue.js"

const A = "a".repeat(40)
const B = "b".repeat(40)
const C = "c".repeat(40)
const D = "d".repeat(40)

function candidate(
  overrides: Partial<VirtualMergeQueueCandidate>,
): VirtualMergeQueueCandidate {
  return {
    candidateRef: "candidate.public.pylon_vmq.default",
    issueNumber: 6691,
    baseCommit: A,
    patchCommit: B,
    verificationPassed: true,
    issueOpen: true,
    hasOpenPullRequest: false,
    queuedAt: "2026-06-28T00:00:00.000Z",
    ...overrides,
  }
}

describe("virtual merge queue", () => {
  test("advances virtual head through verified candidates while promoting only the first actual merge", () => {
    const projection = projectVirtualMergeQueue({
      actualHead: A,
      candidates: [
        candidate({
          candidateRef: "candidate.public.pylon_vmq.issue_6691",
          issueNumber: 6691,
          baseCommit: A,
          patchCommit: B,
          queuedAt: "2026-06-28T00:00:01.000Z",
        }),
        candidate({
          candidateRef: "candidate.public.pylon_vmq.issue_6692",
          issueNumber: 6692,
          baseCommit: B,
          patchCommit: C,
          queuedAt: "2026-06-28T00:00:02.000Z",
        }),
      ],
    })

    expect(projection.virtualHead).toBe(C)
    expect(projection.branchBaseForNextAssignment).toBe(C)
    expect(projection.ready.map((entry) => entry.issueNumber)).toEqual([6691, 6692])
    expect(projection.ready[0]?.waitsForActualHead).toBeNull()
    expect(projection.ready[1]?.waitsForActualHead).toBe(B)
    expect(projection.nextActualPromotion?.issueNumber).toBe(6691)
    expect(projection.blocked).toEqual([])
  })

  test("promotes the next candidate after actual head catches up", () => {
    const projection = projectVirtualMergeQueue({
      actualHead: B,
      candidates: [
        candidate({
          candidateRef: "candidate.public.pylon_vmq.issue_6692",
          issueNumber: 6692,
          baseCommit: B,
          patchCommit: C,
        }),
      ],
    })

    expect(projection.nextActualPromotion?.issueNumber).toBe(6692)
    expect(projection.nextActualPromotion?.waitsForActualHead).toBeNull()
    expect(projection.branchBaseForNextAssignment).toBe(C)
  })

  test("blocks stale candidates that were not branched from the virtual head", () => {
    const projection = projectVirtualMergeQueue({
      actualHead: A,
      candidates: [
        candidate({
          candidateRef: "candidate.public.pylon_vmq.issue_6691",
          issueNumber: 6691,
          baseCommit: A,
          patchCommit: B,
          queuedAt: "2026-06-28T00:00:01.000Z",
        }),
        candidate({
          candidateRef: "candidate.public.pylon_vmq.issue_6693",
          issueNumber: 6693,
          baseCommit: A,
          patchCommit: D,
          queuedAt: "2026-06-28T00:00:02.000Z",
        }),
      ],
    })

    expect(projection.ready.map((entry) => entry.issueNumber)).toEqual([6691])
    expect(projection.blocked).toHaveLength(1)
    expect(projection.blocked[0]?.blockedReasonRef).toBe(
      "virtual_merge_queue.blocked.stale_base",
    )
    expect(projection.branchBaseForNextAssignment).toBe(B)
  })

  test("keeps issue and PR lockouts load-bearing", () => {
    const projection = projectVirtualMergeQueue({
      actualHead: A,
      candidates: [
        candidate({
          candidateRef: "candidate.public.pylon_vmq.closed",
          issueNumber: 6694,
          issueOpen: false,
        }),
        candidate({
          candidateRef: "candidate.public.pylon_vmq.open_pr",
          issueNumber: 6695,
          hasOpenPullRequest: true,
        }),
        candidate({
          candidateRef: "candidate.public.pylon_vmq.unverified",
          issueNumber: 6696,
          verificationPassed: false,
        }),
      ],
    })

    expect(projection.ready).toEqual([])
    expect(projection.nextActualPromotion).toBeNull()
    expect(projection.blocked.map((entry) => entry.blockedReasonRef)).toEqual([
      "virtual_merge_queue.blocked.issue_closed",
      "virtual_merge_queue.blocked.open_pr_exists",
      "virtual_merge_queue.blocked.verification_not_passed",
    ])
  })

  test("blocks duplicate issue candidates before they can create duplicate PRs", () => {
    const projection = projectVirtualMergeQueue({
      actualHead: A,
      candidates: [
        candidate({
          candidateRef: "candidate.public.pylon_vmq.issue_6691_a",
          issueNumber: 6691,
          baseCommit: A,
          patchCommit: B,
          queuedAt: "2026-06-28T00:00:01.000Z",
        }),
        candidate({
          candidateRef: "candidate.public.pylon_vmq.issue_6691_b",
          issueNumber: 6691,
          baseCommit: B,
          patchCommit: C,
          queuedAt: "2026-06-28T00:00:02.000Z",
        }),
      ],
    })

    expect(projection.ready.map((entry) => entry.candidateRef)).toEqual([
      "candidate.public.pylon_vmq.issue_6691_a",
    ])
    expect(projection.blocked[0]?.blockedReasonRef).toBe(
      "virtual_merge_queue.blocked.duplicate_issue",
    )
  })
})
