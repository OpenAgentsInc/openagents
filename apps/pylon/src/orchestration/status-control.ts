import { createHash } from "node:crypto"

import {
  PYLON_AGENT_RUNNER_CONTROL_VERBS,
  encodeAgentRunnerStatusEvent,
  type AgentRunnerNeutralState,
  type AgentRunnerStatusEvent,
} from "../agent-status-reporter.js"
import type { PylonAssignmentRunLifecycleEvent } from "@openagentsinc/agent-runtime-schema"
import type { DispatchContext, OrchestrationTask } from "./store.js"

const stableRef = (prefix: string, value: string): string =>
  value.startsWith(`${prefix}.`)
    ? value
    : `${prefix}.${createHash("sha256").update(value).digest("hex").slice(0, 24)}`

export function neutralStateForDispatchContext(
  context: DispatchContext,
  task?: OrchestrationTask | null,
): AgentRunnerNeutralState {
  if (context.status === "circuit_broken") return "offline"
  if (context.status === "blocked") return "blocked"
  if (context.status === "failed") return "failed"
  if (context.status === "completed") return "done"
  if (context.status === "dispatched") return "working"
  if (task?.status === "pending") return "queued"
  return "idle"
}

export function agentRunnerStatusEventForDispatchContext(input: {
  context: DispatchContext
  task?: OrchestrationTask | null
  pylonRef?: string
  assignmentRef?: string
  now?: Date
}): AgentRunnerStatusEvent {
  const updatedAt = (input.now ?? new Date(input.context.updatedAt)).toISOString()
  const state = neutralStateForDispatchContext(input.context, input.task)
  const taskId = input.task?.id ?? input.context.currentTaskId ?? undefined
  const publicTaskId = taskId === undefined ? undefined : stableRef("task.public.pylon", taskId)
  const publicDispatchContextId = stableRef("dispatch-context.public.pylon", input.context.id)
  const runnerRef = stableRef(
    "runner.public.pylon",
    `${input.pylonRef ?? "pylon.local"}:${input.context.id}:${input.context.runnerKind}`,
  )
  const eventRef = stableRef(
    "event.public.pylon.runner_status",
    `${runnerRef}:${state}:${taskId ?? "idle"}:${input.context.updatedAt}`,
  )
  return {
    eventRef,
    runnerRef,
    runnerKind: input.context.runnerKind,
    state,
    stateStartedAt: input.context.updatedAt,
    updatedAt,
    ...(input.assignmentRef === undefined
      ? {}
      : { assignmentRef: stableRef("assignment.public.pylon", input.assignmentRef) }),
    ...(publicTaskId === undefined ? {} : { taskId: publicTaskId }),
    dispatchContextId: publicDispatchContextId,
    ...(input.pylonRef === undefined ? {} : { pylonRef: stableRef("pylon.public", input.pylonRef) }),
    ...(input.context.worktreePath === null
      ? {}
      : { worktreeRef: stableRef("worktree.public.pylon", `${input.context.id}:${input.context.worktreePath}`) }),
    supportedControlVerbs: PYLON_AGENT_RUNNER_CONTROL_VERBS,
    refs: [
      `runner-kind.pylon.${input.context.runnerKind}`,
      `dispatch-context-status.pylon.${input.context.status}`,
      ...(input.task === undefined || input.task === null ? [] : [`task-status.pylon.${input.task.status}`]),
    ],
    blockerRefs: input.context.status === "circuit_broken"
      ? ["blocker.pylon.runner.circuit_broken"]
      : [],
    stateHistory: [
      {
        state,
        stateStartedAt: input.context.updatedAt,
      },
    ],
  }
}

export function encodeAgentRunnerStatusEventForDispatchContext(input: {
  context: DispatchContext
  task?: OrchestrationTask | null
  pylonRef?: string
  assignmentRef?: string
  now?: Date
}): Record<string, unknown> {
  return encodeAgentRunnerStatusEvent(agentRunnerStatusEventForDispatchContext(input))
}

export function neutralStateForAssignmentLifecycleEvent(
  event: PylonAssignmentRunLifecycleEvent,
): AgentRunnerNeutralState {
  if (event.event === "assignment_run.no_assignment") return "waiting"
  if (event.event === "assignment_run.runtime_failed") return "failed"
  if (event.event === "assignment_run.completed") {
    return event.status === "accepted" ? "done" : "failed"
  }
  if (event.event === "assignment_run.poll_complete") return "queued"
  if (event.event === "assignment_run.accepted") return "queued"
  if (event.event === "assignment_run.progress_submitted") return "working"
  if (event.event === "assignment_run.artifacts_submitted") return "working"
  if (event.event === "assignment_run.closeout_submitted") return "done"
  return "working"
}

export function agentRunnerStatusEventForAssignmentLifecycleEvent(input: {
  event: PylonAssignmentRunLifecycleEvent
  pylonRef: string
  runnerKind?: string
}): AgentRunnerStatusEvent {
  const state = neutralStateForAssignmentLifecycleEvent(input.event)
  const assignmentRef = input.event.assignmentRef
  const leaseRef = input.event.leaseRef
  const runnerKind = input.runnerKind ?? "local_command"
  const runnerRef = stableRef(
    "runner.public.pylon",
    `${input.pylonRef}:${assignmentRef ?? leaseRef ?? "assignment-poll"}:${runnerKind}`,
  )
  const eventRef = stableRef(
    "event.public.pylon.runner_status",
    `${runnerRef}:${input.event.event}:${state}:${input.event.observedAt}:${assignmentRef ?? ""}:${leaseRef ?? ""}`,
  )
  return {
    eventRef,
    runnerRef,
    runnerKind,
    state,
    stateStartedAt: input.event.observedAt,
    updatedAt: input.event.observedAt,
    ...(assignmentRef === undefined
      ? {}
      : { assignmentRef: stableRef("assignment.public.pylon", assignmentRef) }),
    ...(input.pylonRef === undefined ? {} : { pylonRef: stableRef("pylon.public", input.pylonRef) }),
    supportedControlVerbs: PYLON_AGENT_RUNNER_CONTROL_VERBS,
    refs: [
      `assignment-event.pylon.${input.event.event.replaceAll("_", "-").replaceAll(".", "-")}`,
      ...(input.event.status === undefined ? [] : [`assignment-status.pylon.${input.event.status}`]),
      ...(input.event.statusRef === undefined ? [] : [stableRef("status.public.pylon", input.event.statusRef)]),
      ...(input.event.progressRef === undefined ? [] : [stableRef("status.public.pylon", input.event.progressRef)]),
      ...(input.event.artifactRef === undefined ? [] : [stableRef("status.public.pylon", input.event.artifactRef)]),
      ...(input.event.closeoutRef === undefined ? [] : [stableRef("status.public.pylon", input.event.closeoutRef)]),
    ],
    blockerRefs: input.event.blockerRefs ?? [],
    stateHistory: [
      {
        state,
        stateStartedAt: input.event.observedAt,
      },
    ],
  }
}

export function encodeAgentRunnerStatusEventForAssignmentLifecycleEvent(input: {
  event: PylonAssignmentRunLifecycleEvent
  pylonRef: string
  runnerKind?: string
}): Record<string, unknown> {
  return encodeAgentRunnerStatusEvent(agentRunnerStatusEventForAssignmentLifecycleEvent(input))
}
