import {
  buildPylonKhalaGitCheckoutWorkspace,
  type PylonKhalaRequestInput,
} from "./khala-requester.js"

export const PYLON_KHALA_DISPATCH_PLAN_SCHEMA = "openagents.pylon.khala_dispatch_plan.v0.1"

export type KhalaDispatchCandidateKind = "issue" | "pr"

export type KhalaDispatchCandidateRef = {
  kind: KhalaDispatchCandidateKind
  number: number
  ref: string
  objective: string
}

export type KhalaDispatchAccountTarget = {
  accountRef: string | null
  accountRefHash: string
  provider: "codex"
}

export type KhalaDispatchVerifier = {
  branch?: string
  commit: string
  command: string
  repository: string
}

export type KhalaDispatchSlot = {
  account: KhalaDispatchAccountTarget
  candidate: KhalaDispatchCandidateRef
  priorityLane: string
  requestInput: PylonKhalaRequestInput
  slotIndex: number
}

export type KhalaDispatchPlan = {
  schema: typeof PYLON_KHALA_DISPATCH_PLAN_SCHEMA
  blockerRefs: string[]
  concurrency: number
  priorityLane: string
  slots: KhalaDispatchSlot[]
  verifier: KhalaDispatchVerifier
}

export type KhalaDispatchLifecycleEvent =
  | {
      kind: "assignment_run.accepted"
      observedAt?: string
    }
  | {
      kind: "assignment_run.completed"
      observedAt?: string
      status: "accepted" | "rejected"
    }
  | {
      error: string
      kind: "request.failed"
      observedAt?: string
    }

export type KhalaDispatchLifecycleClassification = {
  action: "hold" | "release" | "complete"
  state:
    | "accepted_running"
    | "completed_accepted"
    | "completed_rejected"
    | "failed_before_accept"
    | "planned"
  finalStatus: "accepted" | "rejected" | null
}

export type KhalaDispatchStructuredRecord = {
  account: KhalaDispatchAccountTarget
  candidate: KhalaDispatchCandidateRef
  events: readonly KhalaDispatchLifecycleEvent[]
  legacyFilename?: string
}

export type KhalaDispatchRecordProjection = {
  accountRef: string | null
  action: KhalaDispatchLifecycleClassification["action"]
  candidateRef: string
  lifecycle: KhalaDispatchLifecycleClassification["state"]
  number: number
  priorityLane: string
}

const publicRefPattern = /^[A-Za-z0-9_.:/#=-]{1,200}$/
const accountHashPattern = /^account\.pylon\.codex\.[a-f0-9]{6,64}$/

const uniqueByRef = <T extends { ref: string }>(values: readonly T[]): T[] => {
  const seen = new Set<string>()
  const out: T[] = []
  for (const value of values) {
    if (seen.has(value.ref)) continue
    seen.add(value.ref)
    out.push(value)
  }
  return out
}

export function normalizeKhalaDispatchCandidateRefs(
  values: readonly (number | string | KhalaDispatchCandidateRef)[],
): KhalaDispatchCandidateRef[] {
  return uniqueByRef(
    values.flatMap((value) => {
      if (typeof value === "number") {
        return Number.isSafeInteger(value) && value > 0
          ? [{ kind: "issue" as const, number: value, objective: `Implement OpenAgents issue #${value}.`, ref: `issue:${value}` }]
          : []
      }
      if (typeof value === "string") {
        const trimmed = value.trim()
        const match = trimmed.match(/^(?:(pr|issue):)?#?([1-9][0-9]{0,9})$/i)
        if (match === null) return []
        const kind = match[1]?.toLowerCase() === "pr" ? "pr" : "issue"
        const number = Number.parseInt(match[2], 10)
        return [{
          kind,
          number,
          objective: kind === "pr"
            ? `Resolve OpenAgents pull request #${number}.`
            : `Implement OpenAgents issue #${number}.`,
          ref: `${kind}:${number}`,
        }]
      }
      if (
        Number.isSafeInteger(value.number) &&
        value.number > 0 &&
        (value.kind === "issue" || value.kind === "pr") &&
        publicRefPattern.test(value.ref) &&
        value.objective.trim().length >= 3
      ) {
        return [{ ...value, objective: value.objective.trim() }]
      }
      return []
    }),
  )
}

export function buildPylonKhalaDispatchPlan(input: {
  accountTargets: readonly KhalaDispatchAccountTarget[]
  candidateRefs: readonly KhalaDispatchCandidateRef[]
  concurrency: number
  priorityLane: string
  targetPylonRef: string
  verifier: KhalaDispatchVerifier
}): KhalaDispatchPlan {
  const candidateRefs = normalizeKhalaDispatchCandidateRefs(input.candidateRefs)
  const accountTargets = input.accountTargets.filter((account) =>
    account.provider === "codex" && accountHashPattern.test(account.accountRefHash)
  )
  const concurrency = Math.max(1, Math.floor(input.concurrency))
  const selectedCount = Math.min(concurrency, candidateRefs.length, accountTargets.length)
  const priorityLane = input.priorityLane.trim() || "default"
  const blockerRefs = [
    ...(candidateRefs.length === 0 ? ["blocker.khala_dispatch.no_candidate_refs"] : []),
    ...(accountTargets.length === 0 ? ["blocker.khala_dispatch.no_account_targets"] : []),
    ...(selectedCount === 0 ? ["blocker.khala_dispatch.no_dispatch_slots"] : []),
  ]
  const workspace = buildPylonKhalaGitCheckoutWorkspace({
    branch: input.verifier.branch,
    commit: input.verifier.commit,
    repository: input.verifier.repository,
    verificationCommand: input.verifier.command,
  })
  const slots = candidateRefs.slice(0, selectedCount).map((candidate, index) => {
    const account = accountTargets[index % accountTargets.length]
    const objective = `${candidate.objective} Priority lane: ${priorityLane}. Run verifier: ${input.verifier.command}.`
    return {
      account,
      candidate,
      priorityLane,
      requestInput: {
        objectiveSummary: objective,
        prompt: objective,
        targetAccountRefHash: account.accountRefHash,
        targetPylonRef: input.targetPylonRef,
        workflow: "codex_agent_task",
        workspace,
      },
      slotIndex: index,
    } satisfies KhalaDispatchSlot
  })
  return {
    schema: PYLON_KHALA_DISPATCH_PLAN_SCHEMA,
    blockerRefs,
    concurrency,
    priorityLane,
    slots,
    verifier: input.verifier,
  }
}

export function classifyKhalaDispatchLifecycle(
  events: readonly KhalaDispatchLifecycleEvent[],
): KhalaDispatchLifecycleClassification {
  const completed = [...events]
    .reverse()
    .find((event): event is Extract<KhalaDispatchLifecycleEvent, { kind: "assignment_run.completed" }> =>
      event.kind === "assignment_run.completed"
    )
  if (completed !== undefined) {
    return completed.status === "accepted"
      ? { action: "complete", finalStatus: "accepted", state: "completed_accepted" }
      : { action: "release", finalStatus: "rejected", state: "completed_rejected" }
  }
  if (events.some((event) => event.kind === "assignment_run.accepted")) {
    return { action: "hold", finalStatus: null, state: "accepted_running" }
  }
  if (events.some((event) => event.kind === "request.failed")) {
    return { action: "release", finalStatus: null, state: "failed_before_accept" }
  }
  return { action: "hold", finalStatus: null, state: "planned" }
}

export function projectKhalaDispatchRecord(
  record: KhalaDispatchStructuredRecord,
  priorityLane = "default",
): KhalaDispatchRecordProjection {
  const lifecycle = classifyKhalaDispatchLifecycle(record.events)
  return {
    accountRef: record.account.accountRef,
    action: lifecycle.action,
    candidateRef: record.candidate.ref,
    lifecycle: lifecycle.state,
    number: record.candidate.number,
    priorityLane,
  }
}

export function enforceSingleKhalaDispatchController(input: {
  activeControllerIds: readonly string[]
  namespace: string
  requestedControllerId: string
}): { ok: true; controllerId: string } | { ok: false; blockerRefs: string[] } {
  const active = [...new Set(input.activeControllerIds.filter((id) => id.trim() !== ""))]
  if (active.length === 0 || (active.length === 1 && active[0] === input.requestedControllerId)) {
    return { ok: true, controllerId: input.requestedControllerId }
  }
  return {
    ok: false,
    blockerRefs: [`blocker.khala_dispatch.controller_conflict.${input.namespace}`],
  }
}
