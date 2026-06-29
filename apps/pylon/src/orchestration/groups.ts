import type { DispatchContext, OrchestrationRunnerKind } from "./store.js"
import { normalizeOrchestrationRunnerKind } from "./store.js"

export type OrchestrationGroupAddress =
  | { kind: "all" }
  | { kind: "idle" }
  | { kind: "runner"; runnerKind: OrchestrationRunnerKind }
  | { kind: "worktree"; worktreeId: string }
  | { kind: "assignee"; assigneeHandle: string }

export function parseOrchestrationGroupAddress(address: string): OrchestrationGroupAddress {
  if (address === "@all") return { kind: "all" }
  if (address === "@idle") return { kind: "idle" }
  if (address.startsWith("@runner:")) {
    const rawRunnerKind = address.slice("@runner:".length).trim()
    if (rawRunnerKind.length === 0) throw new Error("empty @runner group address")
    if (rawRunnerKind !== "codex" && rawRunnerKind !== "claude_agent" && rawRunnerKind !== "claude" && rawRunnerKind !== "generic") {
      throw new Error(`unsupported @runner group address: ${address}`)
    }
    return { kind: "runner", runnerKind: normalizeOrchestrationRunnerKind(rawRunnerKind) }
  }
  if (address.startsWith("@worktree:")) {
    const worktreeId = address.slice("@worktree:".length).trim()
    if (worktreeId.length === 0) throw new Error("empty @worktree group address")
    return { kind: "worktree", worktreeId }
  }
  if (address.startsWith("@")) {
    const assigneeHandle = address.slice(1).trim()
    if (assigneeHandle.length === 0) throw new Error("empty orchestration group address")
    return { kind: "assignee", assigneeHandle }
  }
  throw new Error(`unsupported orchestration group address: ${address}`)
}

export function resolveOrchestrationGroup(
  address: string,
  contexts: readonly DispatchContext[],
): DispatchContext[] {
  const parsed = parseOrchestrationGroupAddress(address)
  switch (parsed.kind) {
    case "all":
      return contexts.filter((context) => context.status !== "circuit_broken")
    case "idle":
      return contexts.filter((context) => context.status === "idle")
    case "runner":
      return contexts.filter((context) => context.runnerKind === parsed.runnerKind)
    case "worktree":
      return contexts.filter((context) => context.worktreeId === parsed.worktreeId)
    case "assignee":
      return contexts.filter((context) => context.assigneeHandle === parsed.assigneeHandle)
  }
}
