import { createHash } from "node:crypto"

export type VirtualMergeQueueItem = {
  assignmentRef: string
  issueNumber: number
  changedPaths: readonly string[]
  verificationRef: string
}

export type VirtualMergeQueueEntry = VirtualMergeQueueItem & {
  branchPoint: string
  virtualHeadBefore: string
  virtualHeadAfter: string
  queueIndex: number
}

export type VirtualMergeQueueConflict = {
  assignmentRef: string
  issueNumber: number
  conflictingAssignmentRef: string
  conflictingIssueNumber: number
  conflictingPaths: readonly string[]
  reasonRef: "blocker.public.pylon_virtual_merge_queue.path_conflict"
}

export type VirtualMergeQueuePlan = {
  schema: "openagents.pylon.virtual_merge_queue.simulation.v1"
  baseHead: string
  accepted: readonly VirtualMergeQueueEntry[]
  conflicts: readonly VirtualMergeQueueConflict[]
  finalVirtualHead: string
  sourceRefs: readonly string[]
}

const shaLikePattern = /^[A-Za-z0-9._:-]{6,120}$/
const pathPattern = /^(?!\/)(?!.*(?:^|\/)\.\.(?:\/|$))[A-Za-z0-9._/@+-]+(?:\/[A-Za-z0-9._/@+-]+)*$/

function assertNonEmptyRef(value: string, field: string): string {
  const trimmed = value.trim()
  if (!shaLikePattern.test(trimmed)) {
    throw new Error(`invalid virtual merge queue ${field}`)
  }
  return trimmed
}

function normalizeChangedPaths(paths: readonly string[]): readonly string[] {
  const normalized = [...new Set(paths.map((path) => path.trim()).filter((path) => path.length > 0))]
    .sort((a, b) => a.localeCompare(b))
  if (normalized.length === 0) {
    throw new Error("virtual merge queue item must include changed paths")
  }
  for (const path of normalized) {
    if (!pathPattern.test(path)) {
      throw new Error(`invalid virtual merge queue changed path: ${path}`)
    }
  }
  return normalized
}

function virtualHeadFor(input: {
  previousHead: string
  assignmentRef: string
  issueNumber: number
  changedPaths: readonly string[]
  verificationRef: string
}): string {
  const digest = createHash("sha256")
    .update(input.previousHead)
    .update("\0")
    .update(input.assignmentRef)
    .update("\0")
    .update(String(input.issueNumber))
    .update("\0")
    .update(input.changedPaths.join("\0"))
    .update("\0")
    .update(input.verificationRef)
    .digest("hex")
  return `virtual-head.${digest.slice(0, 24)}`
}

export function simulateVirtualMergeQueue(input: {
  baseHead: string
  items: readonly VirtualMergeQueueItem[]
}): VirtualMergeQueuePlan {
  const baseHead = assertNonEmptyRef(input.baseHead, "baseHead")
  const accepted: VirtualMergeQueueEntry[] = []
  const conflicts: VirtualMergeQueueConflict[] = []
  const pathOwner = new Map<string, VirtualMergeQueueEntry>()
  let virtualHead = baseHead

  input.items.forEach((item, index) => {
    const assignmentRef = assertNonEmptyRef(item.assignmentRef, "assignmentRef")
    const verificationRef = assertNonEmptyRef(item.verificationRef, "verificationRef")
    if (!Number.isInteger(item.issueNumber) || item.issueNumber <= 0) {
      throw new Error("virtual merge queue issueNumber must be a positive integer")
    }
    const changedPaths = normalizeChangedPaths(item.changedPaths)
    const conflictingEntries = [
      ...new Map(
        changedPaths
          .map((path) => pathOwner.get(path))
          .filter((entry): entry is VirtualMergeQueueEntry => entry !== undefined)
          .map((entry) => [entry.assignmentRef, entry] as const),
      ).values(),
    ]
    if (conflictingEntries.length > 0) {
      const first = conflictingEntries[0]
      conflicts.push({
        assignmentRef,
        issueNumber: item.issueNumber,
        conflictingAssignmentRef: first.assignmentRef,
        conflictingIssueNumber: first.issueNumber,
        conflictingPaths: changedPaths.filter((path) => pathOwner.get(path)?.assignmentRef === first.assignmentRef),
        reasonRef: "blocker.public.pylon_virtual_merge_queue.path_conflict",
      })
      return
    }

    const virtualHeadBefore = virtualHead
    const virtualHeadAfter = virtualHeadFor({
      previousHead: virtualHeadBefore,
      assignmentRef,
      issueNumber: item.issueNumber,
      changedPaths,
      verificationRef,
    })
    const entry: VirtualMergeQueueEntry = {
      assignmentRef,
      issueNumber: item.issueNumber,
      changedPaths,
      verificationRef,
      branchPoint: virtualHeadBefore,
      virtualHeadBefore,
      virtualHeadAfter,
      queueIndex: index,
    }
    accepted.push(entry)
    for (const path of changedPaths) {
      pathOwner.set(path, entry)
    }
    virtualHead = virtualHeadAfter
  })

  return {
    schema: "openagents.pylon.virtual_merge_queue.simulation.v1",
    baseHead,
    accepted,
    conflicts,
    finalVirtualHead: virtualHead,
    sourceRefs: [
      "issue.public.github.OpenAgentsInc.openagents.6693",
      "audit.public.docs.artanis.gitafter_cloudflare_artifacts_coordination.2026-06-28",
    ],
  }
}
