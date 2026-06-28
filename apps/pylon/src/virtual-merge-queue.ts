import { createHash } from "node:crypto"

export type VirtualMergeQueueItem = {
  assignmentRef: string
  issueNumber: number
  changedPaths: string[]
  verificationRef: string
}

export type VirtualMergeQueueAcceptedItem = {
  assignmentRef: string
  issueNumber: number
  changedPaths: string[]
  verificationRef: string
  branchPoint: string
  virtualHeadBefore: string
  virtualHeadAfter: string
}

export type VirtualMergeQueueConflict = {
  assignmentRef: string
  issueNumber: number
  conflictingAssignmentRef: string
  conflictingIssueNumber: number
  conflictingPaths: string[]
  reasonRef: "blocker.public.pylon_virtual_merge_queue.path_conflict"
}

export type VirtualMergeQueueSimulation = {
  schema: "openagents.pylon.virtual_merge_queue.simulation.v1"
  baseHead: string
  accepted: VirtualMergeQueueAcceptedItem[]
  conflicts: VirtualMergeQueueConflict[]
  finalVirtualHead: string
  sourceRefs: string[]
}

const refPattern = /^[A-Za-z0-9][A-Za-z0-9_.:-]{2,180}$/
const pathPattern = /^[A-Za-z0-9._/@+-][A-Za-z0-9._/@+ -]{0,240}$/

function stableVirtualHead(input: {
  previousHead: string
  assignmentRef: string
  issueNumber: number
  changedPaths: string[]
  verificationRef: string
}): string {
  const digest = createHash("sha256")
    .update(
      [
        input.previousHead,
        input.assignmentRef,
        String(input.issueNumber),
        input.verificationRef,
        ...input.changedPaths,
      ].join("\0"),
    )
    .digest("hex")
    .slice(0, 40)
  return `virtual-head.pylon.${digest}`
}

function cleanPaths(paths: string[]): string[] {
  return [
    ...new Set(
      paths.map((path) => path.trim()).filter((path) => pathPattern.test(path)),
    ),
  ].sort()
}

function validItem(item: VirtualMergeQueueItem): boolean {
  return (
    Number.isInteger(item.issueNumber) &&
    item.issueNumber > 0 &&
    refPattern.test(item.assignmentRef) &&
    refPattern.test(item.verificationRef)
  )
}

export function simulateVirtualMergeQueue(input: {
  baseHead: string
  items: VirtualMergeQueueItem[]
}): VirtualMergeQueueSimulation {
  const accepted: VirtualMergeQueueAcceptedItem[] = []
  const conflicts: VirtualMergeQueueConflict[] = []
  const pathOwners = new Map<string, VirtualMergeQueueAcceptedItem>()
  let virtualHead = input.baseHead

  for (const item of input.items) {
    if (!validItem(item)) continue
    const changedPaths = cleanPaths(item.changedPaths)
    const conflictingOwners = new Map<string, VirtualMergeQueueAcceptedItem>()
    for (const path of changedPaths) {
      const owner = pathOwners.get(path)
      if (owner !== undefined) conflictingOwners.set(owner.assignmentRef, owner)
    }

    if (conflictingOwners.size > 0) {
      const [conflicting] = [...conflictingOwners.values()].sort(
        (a, b) => a.issueNumber - b.issueNumber,
      )
      if (conflicting !== undefined) {
        conflicts.push({
          assignmentRef: item.assignmentRef,
          issueNumber: item.issueNumber,
          conflictingAssignmentRef: conflicting.assignmentRef,
          conflictingIssueNumber: conflicting.issueNumber,
          conflictingPaths: changedPaths.filter(
            (path) => pathOwners.get(path)?.assignmentRef === conflicting.assignmentRef,
          ),
          reasonRef: "blocker.public.pylon_virtual_merge_queue.path_conflict",
        })
      }
      continue
    }

    const virtualHeadBefore = virtualHead
    const virtualHeadAfter = stableVirtualHead({
      previousHead: virtualHeadBefore,
      assignmentRef: item.assignmentRef,
      issueNumber: item.issueNumber,
      changedPaths,
      verificationRef: item.verificationRef,
    })
    const acceptedItem: VirtualMergeQueueAcceptedItem = {
      assignmentRef: item.assignmentRef,
      issueNumber: item.issueNumber,
      changedPaths,
      verificationRef: item.verificationRef,
      branchPoint: virtualHeadBefore,
      virtualHeadBefore,
      virtualHeadAfter,
    }
    accepted.push(acceptedItem)
    for (const path of changedPaths) pathOwners.set(path, acceptedItem)
    virtualHead = virtualHeadAfter
  }

  return {
    schema: "openagents.pylon.virtual_merge_queue.simulation.v1",
    baseHead: input.baseHead,
    accepted,
    conflicts,
    finalVirtualHead: virtualHead,
    sourceRefs: accepted.map(
      (item) => `issue.public.github.OpenAgentsInc.openagents.${item.issueNumber}`,
    ),
  }
}
