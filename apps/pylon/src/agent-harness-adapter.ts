import { createHash } from "node:crypto"
import {
  decodeAgentRuntimeEvent,
  type AgentDefinition,
  type AgentRuntimeAdapterKind,
  type AgentRuntimeEvent,
  type AgentRuntimeRun,
} from "@openagentsinc/agent-runtime-schema"

import {
  type AssignmentRunLifecycleEvent,
  type PylonAssignmentLease,
} from "./assignment.js"
import { CLAUDE_AGENT_CAPABILITY_REF } from "./claude-agent.js"
import {
  CLAUDE_AGENT_SUM_REPAIR_FIXTURE_REF,
  CLAUDE_AGENT_TASK_AGENT_KIND,
  CLAUDE_AGENT_TASK_SCHEMA,
  claudeAgentTaskFrom,
  type ClaudeAgentTaskPayload,
} from "./claude-agent-executor.js"
import { CODEX_AGENT_CAPABILITY_REF } from "./codex-agent.js"
import {
  CODEX_AGENT_SUM_REPAIR_FIXTURE_REF,
  CODEX_AGENT_TASK_AGENT_KIND,
  CODEX_AGENT_TASK_SCHEMA,
  type CodexAgentTaskPayload,
  codexAgentTaskFrom,
} from "./codex-agent-executor.js"
import type { AgentRunnerCloseoutRecord } from "./agent-runner-registry.js"

export const AGENT_HARNESS_ADAPTER_SCHEMA = "openagents.agent_harness_adapter.v1" as const

export type AgentHarnessAdapterKind = Extract<AgentRuntimeAdapterKind, "codex" | "claude_code">

export type AgentHarnessAdapterStartInput = {
  assignmentRef?: string
  definition: AgentDefinition
  expiresAt?: string
  leaseRef?: string
  now?: Date
  triggerPayload: Record<string, unknown>
  triggerRef?: string
}

export type AgentHarnessAdapterStartResult =
  | {
      schema: typeof AGENT_HARNESS_ADAPTER_SCHEMA
      status: "started"
      adapterKind: AgentHarnessAdapterKind
      assignment: PylonAssignmentLease
      initialEvents: AgentRuntimeEvent[]
      sessionRef: string
    }
  | {
      schema: typeof AGENT_HARNESS_ADAPTER_SCHEMA
      status: "refused"
      adapterKind: AgentHarnessAdapterKind
      blockerRefs: string[]
      reasonRef: string
    }

export type AgentHarnessAdapterNormalizeEventInput = {
  event: AssignmentRunLifecycleEvent
  sequence: number
  sessionRef: string
}

export type AgentHarnessAdapterTerminalInput = {
  closeout: AgentRunnerCloseoutRecord
  generatedAt: string
  sequence: number
  sessionRef: string
}

export type AgentHarnessTerminalReport = {
  schema: typeof AGENT_HARNESS_ADAPTER_SCHEMA
  adapterKind: AgentHarnessAdapterKind
  blockerRefs: string[]
  event: AgentRuntimeEvent
  resultRefs: string[]
  sessionRef: string
  state: Extract<AgentRuntimeRun["state"], "completed" | "failed">
}

export type AgentHarnessAdapter = {
  readonly schema: typeof AGENT_HARNESS_ADAPTER_SCHEMA
  readonly adapterKind: AgentHarnessAdapterKind
  readonly harnessKind: AgentDefinition["harness"]["kind"]
  canStart: (definition: AgentDefinition) => boolean
  normalizeEvent: (input: AgentHarnessAdapterNormalizeEventInput) => AgentRuntimeEvent
  reportTerminalState: (input: AgentHarnessAdapterTerminalInput) => AgentHarnessTerminalReport
  start: (input: AgentHarnessAdapterStartInput) => Promise<AgentHarnessAdapterStartResult>
}

function stableRef(prefix: string, value: string): string {
  return `${prefix}.${createHash("sha256").update(value).digest("hex").slice(0, 24)}`
}

function stablePayloadFingerprint(value: unknown): string {
  try {
    return JSON.stringify(value) ?? "null"
  } catch {
    return "unserializable"
  }
}

function refFragment(value: string): string {
  const fragment = value.replaceAll(/[^A-Za-z0-9_.:-]/g, "_").slice(0, 120)
  return fragment === "" ? "run" : fragment
}

function publicTriggerRef(input: AgentHarnessAdapterStartInput): string {
  if (typeof input.triggerRef === "string" && input.triggerRef.trim() !== "") {
    return input.triggerRef.trim()
  }
  const manual = input.definition.triggers.find((trigger) => trigger.kind === "manual")
  return manual?.triggerRef ?? input.definition.triggers[0]?.triggerRef ?? `trigger.manual.${refFragment(input.definition.id)}`
}

function fixtureRefFromTriggerPayload(payload: Record<string, unknown>, fallback: string): string {
  return typeof payload.fixtureRef === "string" && payload.fixtureRef.trim() !== ""
    ? payload.fixtureRef.trim()
    : fallback
}

function workspaceFromTriggerPayload(payload: Record<string, unknown>): unknown | undefined {
  const workspace = payload.workspace
  return workspace !== null && typeof workspace === "object" ? workspace : undefined
}

function eventTagForLifecycleEvent(event: AssignmentRunLifecycleEvent["event"]): AgentRuntimeEvent["tag"] {
  if (event === "assignment_run.runtime_started") return "external_agent.started"
  if (event === "assignment_run.runtime_failed") return "external_agent.failed"
  if (event === "assignment_run.completed") return "external_agent.completed"
  if (event === "assignment_run.closeout_submitted") return "external_agent.artifact_recorded"
  return "external_agent.event"
}

function runtimeEvent(input: {
  blockerRefs?: ReadonlyArray<string>
  generatedAt: string
  refs?: ReadonlyArray<string>
  runId: string
  sequence: number
  summary: string
  tag: AgentRuntimeEvent["tag"]
}): AgentRuntimeEvent {
  return decodeAgentRuntimeEvent({
    tag: input.tag,
    eventId: stableRef("agent_runtime_event.agent_harness", `${input.runId}:${input.sequence}:${input.tag}`),
    runId: input.runId,
    sequence: input.sequence,
    generatedAt: input.generatedAt,
    visibility: "private",
    redactionClass: "redacted_summary",
    summary: input.summary,
    refs: [...new Set(input.refs ?? [])].sort(),
    blockerRefs: [...new Set(input.blockerRefs ?? [])].sort(),
  })
}

function codexAssignmentFromDefinition(input: {
  definition: AgentDefinition
  sessionRef: string
  triggerPayload: Record<string, unknown>
  triggerRef: string
}): NonNullable<PylonAssignmentLease["codingAssignment"]> {
  const workspace = workspaceFromTriggerPayload(input.triggerPayload)
  const codexPayload: CodexAgentTaskPayload = {
    schema: CODEX_AGENT_TASK_SCHEMA,
    agentKind: CODEX_AGENT_TASK_AGENT_KIND,
    ...(workspace === undefined
      ? { fixtureRef: fixtureRefFromTriggerPayload(input.triggerPayload, CODEX_AGENT_SUM_REPAIR_FIXTURE_REF) }
      : {}),
    timeoutSeconds: Math.min(Math.max(Math.floor(input.definition.budget.maxRunSeconds), 1), 2400),
  }

  return {
    codex: codexPayload,
    objective: {
      objectiveRef: `workflow.public.agent_definition.${refFragment(input.definition.id)}`,
      publicSummary: `Run background agent definition ${input.definition.id}.`,
    },
    requiredCapabilityRefs: [CODEX_AGENT_CAPABILITY_REF],
    routing: {
      durableStreamRef: input.sessionRef,
      schema: AGENT_HARNESS_ADAPTER_SCHEMA,
      triggerRef: input.triggerRef,
    },
    ...(workspace === undefined ? {} : { workspace }),
  }
}

function codexLeaseFromDefinition(input: AgentHarnessAdapterStartInput & {
  now: Date
  sessionRef: string
  triggerRef: string
}): PylonAssignmentLease {
  const assignmentRef = input.assignmentRef ??
    stableRef(
      "assignment.public.agent_harness.codex",
      `${input.definition.id}:${input.triggerRef}:${input.sessionRef}`,
    )
  const leaseRef = input.leaseRef ?? stableRef("lease.public.agent_harness.codex", assignmentRef)
  const expiresAt = input.expiresAt ??
    new Date(input.now.getTime() + Math.max(1, input.definition.budget.maxRunSeconds) * 1000).toISOString()

  return {
    schema: "openagents.pylon.assignment_lease.v0.3",
    assignmentRef,
    leaseRef,
    goal: `workflow.public.agent_definition.${refFragment(input.definition.id)}`,
    paymentMode: "no-spend",
    capabilityRefs: [CODEX_AGENT_CAPABILITY_REF],
    codingAssignment: codexAssignmentFromDefinition(input),
    expiresAt,
    createdAt: input.now.toISOString(),
  }
}

function claudeAssignmentFromDefinition(input: {
  definition: AgentDefinition
  sessionRef: string
  triggerPayload: Record<string, unknown>
  triggerRef: string
}): NonNullable<PylonAssignmentLease["codingAssignment"]> {
  const workspace = workspaceFromTriggerPayload(input.triggerPayload)
  const claudePayload: ClaudeAgentTaskPayload = {
    schema: CLAUDE_AGENT_TASK_SCHEMA,
    agentKind: CLAUDE_AGENT_TASK_AGENT_KIND,
    ...(workspace === undefined
      ? { fixtureRef: fixtureRefFromTriggerPayload(input.triggerPayload, CLAUDE_AGENT_SUM_REPAIR_FIXTURE_REF) }
      : {}),
    timeoutSeconds: Math.min(Math.max(Math.floor(input.definition.budget.maxRunSeconds), 1), 1200),
  }

  return {
    kind: "claude_agent_task",
    claudeAgent: claudePayload,
    objective: {
      objectiveRef: `workflow.public.agent_definition.${refFragment(input.definition.id)}`,
      publicSummary: `Run background agent definition ${input.definition.id}.`,
    },
    requiredCapabilityRefs: [CLAUDE_AGENT_CAPABILITY_REF],
    routing: {
      durableStreamRef: input.sessionRef,
      schema: AGENT_HARNESS_ADAPTER_SCHEMA,
      triggerRef: input.triggerRef,
    },
    ...(workspace === undefined ? {} : { workspace }),
  }
}

function claudeLeaseFromDefinition(input: AgentHarnessAdapterStartInput & {
  now: Date
  sessionRef: string
  triggerRef: string
}): PylonAssignmentLease {
  const assignmentRef = input.assignmentRef ??
    stableRef(
      "assignment.public.agent_harness.claude_code",
      `${input.definition.id}:${input.triggerRef}:${input.sessionRef}`,
    )
  const leaseRef = input.leaseRef ?? stableRef("lease.public.agent_harness.claude_code", assignmentRef)
  const expiresAt = input.expiresAt ??
    new Date(input.now.getTime() + Math.max(1, input.definition.budget.maxRunSeconds) * 1000).toISOString()

  return {
    schema: "openagents.pylon.assignment_lease.v0.3",
    assignmentRef,
    leaseRef,
    goal: `workflow.public.agent_definition.${refFragment(input.definition.id)}`,
    paymentMode: "no-spend",
    capabilityRefs: [CLAUDE_AGENT_CAPABILITY_REF],
    codingAssignment: claudeAssignmentFromDefinition(input),
    expiresAt,
    createdAt: input.now.toISOString(),
  }
}

function codexCanStart(definition: AgentDefinition): boolean {
  return (
    definition.lane === "own_pylon" &&
    (definition.harness.kind === "codex" || definition.harness.kind === "khala")
  )
}

function claudeCanStart(definition: AgentDefinition): boolean {
  return (
    definition.lane === "own_pylon" &&
    (definition.harness.kind === "claude_code" || definition.harness.kind === "khala")
  )
}

export function createCodexAgentHarnessAdapter(): AgentHarnessAdapter {
  return {
    schema: AGENT_HARNESS_ADAPTER_SCHEMA,
    adapterKind: "codex",
    harnessKind: "codex",
    canStart: codexCanStart,
    normalizeEvent: ({ event, sequence, sessionRef }) =>
      runtimeEvent({
        blockerRefs: event.blockerRefs,
        generatedAt: event.observedAt,
        refs: [
          event.assignmentRef,
          event.leaseRef,
          ...(event.closeoutRef === undefined ? [] : [event.closeoutRef]),
          ...(event.progressRef === undefined ? [] : [event.progressRef]),
          ...(event.artifactRef === undefined ? [] : [event.artifactRef]),
          ...(event.statusRef === undefined ? [] : [event.statusRef]),
        ].filter((ref): ref is string => typeof ref === "string" && ref.trim() !== ""),
        runId: sessionRef,
        sequence,
        summary: `Pylon Codex adapter observed ${event.event}.`,
        tag: eventTagForLifecycleEvent(event.event),
      }),
    reportTerminalState: ({ closeout, generatedAt, sequence, sessionRef }) => {
      const completed = closeout.status === "accepted"
      const event = runtimeEvent({
        blockerRefs: closeout.blockerRefs,
        generatedAt,
        refs: [
          ...closeout.artifactRefs,
          ...closeout.proofRefs,
          ...closeout.resultRefs,
          ...closeout.runRefs,
          ...closeout.testRefs,
        ],
        runId: sessionRef,
        sequence,
        summary: completed
          ? "Pylon Codex adapter completed the background agent run."
          : "Pylon Codex adapter failed the background agent run.",
        tag: completed ? "external_agent.completed" : "external_agent.failed",
      })
      return {
        schema: AGENT_HARNESS_ADAPTER_SCHEMA,
        adapterKind: "codex",
        blockerRefs: [...closeout.blockerRefs],
        event,
        resultRefs: [...closeout.resultRefs],
        sessionRef,
        state: completed ? "completed" : "failed",
      }
    },
    start: async (input) => {
      if (!codexCanStart(input.definition)) {
        return {
          schema: AGENT_HARNESS_ADAPTER_SCHEMA,
          status: "refused",
          adapterKind: "codex",
          blockerRefs: [
            input.definition.lane === "own_pylon"
              ? "blocker.agent_harness_adapter.codex_harness_unsupported"
              : "blocker.agent_harness_adapter.own_pylon_required",
          ],
          reasonRef: "reason.agent_harness_adapter.codex_start_refused",
        }
      }

      const now = input.now ?? new Date()
      const triggerRef = publicTriggerRef(input)
      const sessionRef = stableRef(
        "session.agent_harness.codex",
        `${input.definition.id}:${triggerRef}:${stablePayloadFingerprint(input.triggerPayload)}:${now.toISOString()}`,
      )
      const assignment = codexLeaseFromDefinition({ ...input, now, sessionRef, triggerRef })

      return {
        schema: AGENT_HARNESS_ADAPTER_SCHEMA,
        status: "started",
        adapterKind: "codex",
        assignment,
        initialEvents: [
          runtimeEvent({
            generatedAt: now.toISOString(),
            refs: [input.definition.id, triggerRef, assignment.assignmentRef, assignment.leaseRef],
            runId: sessionRef,
            sequence: 0,
            summary: "Background agent definition accepted by the Pylon Codex harness adapter.",
            tag: "run.input_accepted",
          }),
          runtimeEvent({
            generatedAt: now.toISOString(),
            refs: [assignment.assignmentRef, assignment.leaseRef],
            runId: sessionRef,
            sequence: 1,
            summary: "Pylon Codex harness adapter created a codex_agent_task lease.",
            tag: "external_agent.started",
          }),
        ],
        sessionRef,
      }
    },
  }
}

export const codexAgentHarnessAdapter = createCodexAgentHarnessAdapter()

export function createClaudeCodeAgentHarnessAdapter(): AgentHarnessAdapter {
  return {
    schema: AGENT_HARNESS_ADAPTER_SCHEMA,
    adapterKind: "claude_code",
    harnessKind: "claude_code",
    canStart: claudeCanStart,
    normalizeEvent: ({ event, sequence, sessionRef }) =>
      runtimeEvent({
        blockerRefs: event.blockerRefs,
        generatedAt: event.observedAt,
        refs: [
          event.assignmentRef,
          event.leaseRef,
          ...(event.closeoutRef === undefined ? [] : [event.closeoutRef]),
          ...(event.progressRef === undefined ? [] : [event.progressRef]),
          ...(event.artifactRef === undefined ? [] : [event.artifactRef]),
          ...(event.statusRef === undefined ? [] : [event.statusRef]),
        ].filter((ref): ref is string => typeof ref === "string" && ref.trim() !== ""),
        runId: sessionRef,
        sequence,
        summary: `Pylon Claude adapter observed ${event.event}.`,
        tag: eventTagForLifecycleEvent(event.event),
      }),
    reportTerminalState: ({ closeout, generatedAt, sequence, sessionRef }) => {
      const completed = closeout.status === "accepted"
      const event = runtimeEvent({
        blockerRefs: closeout.blockerRefs,
        generatedAt,
        refs: [
          ...closeout.artifactRefs,
          ...closeout.proofRefs,
          ...closeout.resultRefs,
          ...closeout.runRefs,
          ...closeout.testRefs,
        ],
        runId: sessionRef,
        sequence,
        summary: completed
          ? "Pylon Claude adapter completed the background agent run."
          : "Pylon Claude adapter failed the background agent run.",
        tag: completed ? "external_agent.completed" : "external_agent.failed",
      })
      return {
        schema: AGENT_HARNESS_ADAPTER_SCHEMA,
        adapterKind: "claude_code",
        blockerRefs: [...closeout.blockerRefs],
        event,
        resultRefs: [...closeout.resultRefs],
        sessionRef,
        state: completed ? "completed" : "failed",
      }
    },
    start: async (input) => {
      if (!claudeCanStart(input.definition)) {
        return {
          schema: AGENT_HARNESS_ADAPTER_SCHEMA,
          status: "refused",
          adapterKind: "claude_code",
          blockerRefs: [
            input.definition.lane === "own_pylon"
              ? "blocker.agent_harness_adapter.claude_code_harness_unsupported"
              : "blocker.agent_harness_adapter.own_pylon_required",
          ],
          reasonRef: "reason.agent_harness_adapter.claude_code_start_refused",
        }
      }

      const now = input.now ?? new Date()
      const triggerRef = publicTriggerRef(input)
      const sessionRef = stableRef(
        "session.agent_harness.claude_code",
        `${input.definition.id}:${triggerRef}:${stablePayloadFingerprint(input.triggerPayload)}:${now.toISOString()}`,
      )
      const assignment = claudeLeaseFromDefinition({ ...input, now, sessionRef, triggerRef })

      return {
        schema: AGENT_HARNESS_ADAPTER_SCHEMA,
        status: "started",
        adapterKind: "claude_code",
        assignment,
        initialEvents: [
          runtimeEvent({
            generatedAt: now.toISOString(),
            refs: [input.definition.id, triggerRef, assignment.assignmentRef, assignment.leaseRef],
            runId: sessionRef,
            sequence: 0,
            summary: "Background agent definition accepted by the Pylon Claude harness adapter.",
            tag: "run.input_accepted",
          }),
          runtimeEvent({
            generatedAt: now.toISOString(),
            refs: [assignment.assignmentRef, assignment.leaseRef],
            runId: sessionRef,
            sequence: 1,
            summary: "Pylon Claude harness adapter created a claude_agent_task lease.",
            tag: "external_agent.started",
          }),
        ],
        sessionRef,
      }
    },
  }
}

export const claudeCodeAgentHarnessAdapter = createClaudeCodeAgentHarnessAdapter()

export function assignmentCarriesCodexHarnessTask(assignment: PylonAssignmentLease): boolean {
  return codexAgentTaskFrom(assignment.codingAssignment) !== null
}

export function assignmentCarriesClaudeHarnessTask(assignment: PylonAssignmentLease): boolean {
  return claudeAgentTaskFrom(assignment.codingAssignment) !== null
}
