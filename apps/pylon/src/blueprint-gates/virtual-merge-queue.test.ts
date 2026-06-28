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
    expect(projection.blocked[0]?.status).toBe("needs-rebase")
    expect(projection.branchBaseForNextAssignment).toBe(B)
  })

  test("blocks a stale-based branch before a squash can delete files added since its base", () => {
    const projection = projectVirtualMergeQueue({
      actualHead: C,
      candidates: [
        candidate({
          candidateRef: "candidate.public.pylon_vmq.stale_deletion_poison",
          issueNumber: 6723,
          baseCommit: A,
          patchCommit: D,
          changedFiles: [
            { path: "clients/mobile/Khala/README.md", status: "deleted" },
            { path: "INVARIANTS.md", status: "deleted" },
          ],
        }),
      ],
    })

    expect(projection.ready).toEqual([])
    expect(projection.nextActualPromotion).toBeNull()
    expect(projection.blocked[0]?.blockedReasonRef).toBe(
      "virtual_merge_queue.blocked.stale_base",
    )
    expect(projection.blocked[0]?.status).toBe("needs-rebase")
  })

  test("blocks protected path deletions even when the branch is otherwise fresh", () => {
    const projection = projectVirtualMergeQueue({
      actualHead: A,
      candidates: [
        candidate({
          candidateRef: "candidate.public.pylon_vmq.protected_delete",
          issueNumber: 6724,
          baseCommit: A,
          patchCommit: B,
          changedFiles: [
            { path: "apps/pylon/src/index.ts", status: "deleted" },
          ],
        }),
      ],
    })

    expect(projection.ready).toEqual([])
    expect(projection.blocked[0]?.blockedReasonRef).toBe(
      "virtual_merge_queue.blocked.protected_path_deleted",
    )
  })

  test("blocks candidates over the mass deletion threshold", () => {
    const projection = projectVirtualMergeQueue({
      actualHead: A,
      candidates: [
        candidate({
          candidateRef: "candidate.public.pylon_vmq.mass_delete",
          issueNumber: 6725,
          baseCommit: A,
          patchCommit: B,
          changedFiles: Array.from({ length: 6 }, (_, index) => ({
            path: `docs/refactor/pruned-${index}.md`,
            status: "deleted" as const,
          })),
        }),
      ],
    })

    expect(projection.ready).toEqual([])
    expect(projection.blocked[0]?.blockedReasonRef).toBe(
      "virtual_merge_queue.blocked.mass_deletion_threshold",
    )
  })

  test("lets a clean in-scope candidate merge normally", () => {
    const projection = projectVirtualMergeQueue({
      actualHead: A,
      candidates: [
        candidate({
          candidateRef: "candidate.public.pylon_vmq.clean",
          issueNumber: 6726,
          baseCommit: A,
          patchCommit: B,
          changedFiles: [
            { path: "packages/world-contract/src/index.ts", status: "modified" },
          ],
        }),
      ],
    })

    expect(projection.ready.map((entry) => entry.issueNumber)).toEqual([6726])
    expect(projection.nextActualPromotion?.issueNumber).toBe(6726)
    expect(projection.blocked).toEqual([])
  })

  test("honors only an explicit signed operator override for stale or large deletion guards", () => {
    const projection = projectVirtualMergeQueue({
      actualHead: C,
      candidates: [
        candidate({
          candidateRef: "candidate.public.pylon_vmq.unsigned_override",
          issueNumber: 6727,
          baseCommit: A,
          patchCommit: D,
          signedOperatorOverrideRef: "operator-said-ok",
          changedFiles: [
            { path: "INVARIANTS.md", status: "deleted" },
          ],
        }),
        candidate({
          candidateRef: "candidate.public.pylon_vmq.signed_override",
          issueNumber: 6728,
          baseCommit: C,
          patchCommit: D,
          queuedAt: "2026-06-28T00:00:01.000Z",
          signedOperatorOverrideRef: "override.signed.operator.issue_6728.sig_abcdef",
          changedFiles: [
            { path: "INVARIANTS.md", status: "deleted" },
          ],
        }),
      ],
    })

    expect(projection.blocked.map((entry) => entry.issueNumber)).toEqual([6727])
    expect(projection.blocked[0]?.blockedReasonRef).toBe(
      "virtual_merge_queue.blocked.stale_base",
    )
    expect(projection.ready.map((entry) => entry.issueNumber)).toEqual([6728])
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
