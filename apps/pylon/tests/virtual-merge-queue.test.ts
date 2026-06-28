import { describe, expect, test } from "bun:test"
import {
  simulateVirtualMergeQueue,
  type VirtualMergeQueueItem,
} from "../src/virtual-merge-queue"

const item = (issueNumber: number, path: string): VirtualMergeQueueItem => ({
  assignmentRef: `assignment.public.codex_agent_task.vmq_${issueNumber}`,
  issueNumber,
  changedPaths: [path],
  verificationRef: `command.public.pylon_khala.verify.fixture_${issueNumber}`,
})

describe("simulateVirtualMergeQueue", () => {
  test("projects 20+ parallel virtual merges through one advancing virtual head", () => {
    const items = Array.from({ length: 24 }, (_, index) =>
      item(6693 + index, `apps/pylon/tests/virtual-merge-${index.toString().padStart(2, "0")}.test.ts`),
    )

    const plan = simulateVirtualMergeQueue({
      baseHead: "main.351276003542c4267665e60767d3e8feb5b97f64",
      items,
    })

    expect(plan.schema).toBe("openagents.pylon.virtual_merge_queue.simulation.v1")
    expect(plan.accepted).toHaveLength(24)
    expect(plan.conflicts).toHaveLength(0)
    expect(plan.accepted[0].branchPoint).toBe(plan.baseHead)

    for (let index = 1; index < plan.accepted.length; index += 1) {
      expect(plan.accepted[index].branchPoint).toBe(plan.accepted[index - 1].virtualHeadAfter)
      expect(plan.accepted[index].virtualHeadBefore).toBe(plan.accepted[index - 1].virtualHeadAfter)
      expect(plan.accepted[index].virtualHeadAfter).not.toBe(plan.accepted[index].virtualHeadBefore)
    }

    expect(new Set(plan.accepted.map((entry) => entry.virtualHeadAfter)).size).toBe(24)
    expect(plan.finalVirtualHead).toBe(plan.accepted[23].virtualHeadAfter)
    expect(plan.sourceRefs).toContain("issue.public.github.OpenAgentsInc.openagents.6693")
  })

  test("keeps the virtual head stable when a later item conflicts with an accepted path", () => {
    const plan = simulateVirtualMergeQueue({
      baseHead: "main.aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
      items: [
        item(7001, "apps/pylon/src/codex-pr-publisher.ts"),
        item(7002, "apps/pylon/src/codex-pr-publisher.ts"),
        item(7003, "apps/pylon/src/virtual-merge-queue.ts"),
      ],
    })

    expect(plan.accepted.map((entry) => entry.issueNumber)).toEqual([7001, 7003])
    expect(plan.conflicts).toEqual([
      {
        assignmentRef: "assignment.public.codex_agent_task.vmq_7002",
        issueNumber: 7002,
        conflictingAssignmentRef: "assignment.public.codex_agent_task.vmq_7001",
        conflictingIssueNumber: 7001,
        conflictingPaths: ["apps/pylon/src/codex-pr-publisher.ts"],
        reasonRef: "blocker.public.pylon_virtual_merge_queue.path_conflict",
      },
    ])
    expect(plan.accepted[1].branchPoint).toBe(plan.accepted[0].virtualHeadAfter)
    expect(plan.finalVirtualHead).toBe(plan.accepted[1].virtualHeadAfter)
  })
})
