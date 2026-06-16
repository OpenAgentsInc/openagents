// Pylon contributor trace-completion client (issue #5054, epic #5051; design
// docs/tassadar/2026-06-15-executor-trace-contributor-completion-design.md
// §4.5). These are the two contributor-callable verbs that let a node that
// claimed a training window lease actually FINISH the work — the gap that left
// the Tassadar run at "claimed but never verified":
//
//   submit-trace  WORKER role:    run the dispatched workload locally
//                 (reuse executeTassadarNumericModel) and POST the worker's
//                 trace commitment to the agent-gated §4.1 route
//                 POST /api/training/leases/{leaseRef}/trace-submission.
//   validate      VALIDATOR role: re-execute (replay) the same workload on a
//                 DISTINCT device and POST the replay digest to the agent-gated
//                 §4.2 route POST /api/training/leases/{leaseRef}/replay-verdict.
//
// Replay is the trust anchor — the server never trusts the submitter's digest;
// the Verified/Rejected verdict is the separate-device replay match (enforced
// server-side along with device-distinctness and lease ownership). This module
// is CLIENT-ONLY: it carries no wallet, settlement, or payout authority, and it
// does not change default node behavior. Participating is opt-in (the verbs are
// run explicitly; the background assignment worker stays PYLON_ASSIGNMENT_WORKER
// gated and OFF by default until the #5061 two-device verification lands).
//
// Both verbs accept an injectable executor + fetch so tests never run the real
// workload or hit the network.

import {
  type TassadarAlmNumericModel,
  type TassadarNumericTrace,
  executeTassadarNumericModel,
} from "@openagentsinc/tassadar-executor"

// Mirrors the server-side TassadarExecutorTraceWorkloadFamilies
// (apps/openagents.com/workers/api/src/tassadar-executor-trace-homework.ts).
// Kept as a local literal because the executor package does not export it.
export const TASSADAR_TRACE_WORKLOAD_FAMILIES = [
  "article_closeout",
  "sudoku_trace",
  "hungarian_trace",
  "kernel_trace",
] as const
export type TassadarTraceWorkloadFamily =
  (typeof TASSADAR_TRACE_WORKLOAD_FAMILIES)[number]

// The local workload the verbs run. Same shape the assignment dispatch carries
// under `codingAssignment.tassadar` (see assignment.ts tassadarPayloadFrom): a
// digest-pinned numeric model plus the step inputs. `expectedTraceDigest`, when
// present, is the dispatched expectation we compare the local digest against.
export type TassadarTraceWorkload = {
  model: TassadarAlmNumericModel
  steps: ReadonlyArray<ReadonlyArray<number>>
  expectedTraceDigest?: string
}

export type TraceExecutor = (
  model: TassadarAlmNumericModel,
  steps: ReadonlyArray<ReadonlyArray<number>>,
) => Promise<TassadarNumericTrace>

export type TraceClientOptions = {
  baseUrl: string
  // Agent token — these routes are requireAgent (NOT admin). Read the same way
  // the other agent-gated Pylon verbs do (--agent-token / OPENAGENTS_AGENT_TOKEN).
  agentToken?: string
  fetchFn?: typeof fetch
  // Injectable executor so tests don't run the real digest-pinned workload.
  executor?: TraceExecutor
  nowIso?: () => string
}

const PUBLIC_SAFE_REF = /^[A-Za-z0-9][A-Za-z0-9_.:/-]*$/

function normalizeBaseUrl(baseUrl: string): string {
  return baseUrl.replace(/\/+$/, "")
}

function safeRefStamp(iso: string): string {
  return iso.replace(/[^0-9a-zA-Z]/g, "").slice(0, 16)
}

function requireAgentToken(options: TraceClientOptions): string {
  const token = options.agentToken?.trim() ?? ""
  if (token === "") {
    throw new Error(
      "trace contribution verb requires --agent-token or OPENAGENTS_AGENT_TOKEN",
    )
  }
  return token
}

function assertRef(value: string | undefined, label: string): string {
  const ref = value?.trim() ?? ""
  if (ref.length < 3 || ref.length > 260 || !PUBLIC_SAFE_REF.test(ref)) {
    throw new Error(`invalid ${label} (3-260 chars, [A-Za-z0-9._:/-])`)
  }
  return ref
}

export function assertWorkloadFamily(
  value: string | undefined,
): TassadarTraceWorkloadFamily {
  const family = value?.trim() ?? ""
  if (
    !(TASSADAR_TRACE_WORKLOAD_FAMILIES as readonly string[]).includes(family)
  ) {
    throw new Error(
      `invalid --workload-family (one of: ${TASSADAR_TRACE_WORKLOAD_FAMILIES.join(", ")})`,
    )
  }
  return family as TassadarTraceWorkloadFamily
}

// Parse a workload JSON (the dispatch payload, e.g. from
// `--workload <file.json>`). Accepts either the bare workload
// ({model, steps, expectedTraceDigest?}) or the assignment dispatch wrapper
// ({tassadar: {...}}). It also restores the executor's `seed_writes` wire
// format when the payload used the public-projection-safe
// `initialChannelWrites` alias (same normalization assignment.ts applies).
export function parseTassadarWorkload(input: unknown): TassadarTraceWorkload {
  const root = input as Record<string, unknown> | null | undefined
  const raw =
    root && typeof root === "object" && "tassadar" in root
      ? (root.tassadar as Record<string, unknown> | undefined)
      : root
  if (!raw || typeof raw !== "object") {
    throw new Error("workload JSON must be an object with a model and steps")
  }
  const model = raw.model as
    | (Record<string, unknown> & {
        initialChannelWrites?: ReadonlyArray<readonly [number, number, number]>
        seed_writes?: ReadonlyArray<readonly [number, number, number]>
      })
    | undefined
  const steps = raw.steps
  if (model === undefined || typeof model !== "object" || !Array.isArray(steps)) {
    throw new Error("workload JSON must carry a numeric model and steps array")
  }
  const normalizedModel =
    model.seed_writes === undefined && model.initialChannelWrites !== undefined
      ? (() => {
          const { initialChannelWrites, ...rest } = model
          return { ...rest, seed_writes: initialChannelWrites }
        })()
      : model
  const expectedTraceDigest =
    typeof raw.expectedTraceDigest === "string"
      ? raw.expectedTraceDigest
      : undefined
  return {
    model: normalizedModel as unknown as TassadarAlmNumericModel,
    steps: steps as ReadonlyArray<ReadonlyArray<number>>,
    ...(expectedTraceDigest !== undefined ? { expectedTraceDigest } : {}),
  }
}

async function getAgentJson(
  fetchFn: typeof fetch,
  url: string,
  token: string,
): Promise<
  { ok: true; json: unknown } | { ok: false; status: number; error: string }
> {
  const response = await fetchFn(url, {
    method: "GET",
    headers: {
      accept: "application/json",
      authorization: `Bearer ${token}`,
    },
  })
  const json = (await response.json().catch(() => null)) as unknown
  if (!response.ok) {
    return {
      ok: false,
      status: response.status,
      error: errorFromJson(json, `trace discovery ${response.status}`),
    }
  }
  return { ok: true, json }
}

async function postAgentJson(
  fetchFn: typeof fetch,
  url: string,
  token: string,
  body: unknown,
): Promise<
  { ok: true; json: unknown } | { ok: false; status: number; error: string }
> {
  const response = await fetchFn(url, {
    method: "POST",
    headers: {
      accept: "application/json",
      authorization: `Bearer ${token}`,
      "content-type": "application/json",
    },
    body: JSON.stringify(body),
  })
  const json = (await response.json().catch(() => null)) as unknown
  if (!response.ok) {
    return {
      ok: false,
      status: response.status,
      error: errorFromJson(json, `trace contribution ${response.status}`),
    }
  }
  return { ok: true, json }
}

function errorFromJson(json: unknown, fallback: string): string {
  if (json && typeof json === "object") {
    const record = json as Record<string, unknown>
    const reason = record.reason ?? record.error ?? record.message
    if (typeof reason === "string" && reason.length > 0) return reason
  }
  return fallback
}

export type SubmitTraceInput = {
  leaseRef: string
  // The worker's own device ref (must equal its Pylon device; the validator
  // device must differ — enforced server-side). Default: the local node id.
  pylonDeviceRef: string
  workloadFamily: TassadarTraceWorkloadFamily
  workload: TassadarTraceWorkload
  // Optional sampled window override; defaults to the full executed range.
  sampledWindow?: { startStep: number; endStep: number }
  assignmentRef?: string
}

// §4.5 WORKER: run the dispatched workload locally + submit the trace commitment
// to the §4.1 route. Returns the computed digest, whether it matched the
// dispatched expectation, and the server's pending-contribution projection.
export async function submitTraceContribution(
  options: TraceClientOptions,
  input: SubmitTraceInput,
): Promise<Record<string, unknown>> {
  const fetchFn = options.fetchFn ?? fetch
  const executor = options.executor ?? executeTassadarNumericModel
  const token = requireAgentToken(options)
  const baseUrl = normalizeBaseUrl(options.baseUrl)
  const leaseRef = assertRef(input.leaseRef, "--lease-ref")
  const pylonDeviceRef = assertRef(input.pylonDeviceRef, "--device-ref")
  const fetchedAt = options.nowIso?.() ?? new Date().toISOString()
  const stamp = safeRefStamp(fetchedAt)

  const trace = await executor(input.workload.model, input.workload.steps)
  const digestMatchesExpectation =
    input.workload.expectedTraceDigest === undefined ||
    input.workload.expectedTraceDigest === trace.traceDigest

  const sampledWindow = input.sampledWindow ?? {
    endStep: trace.stepCount,
    startStep: 0,
  }
  const traceCommitmentDigestRef = `trace.tassadar.commitment.${trace.traceDigest}`
  const sampledWindowRef = `trace.tassadar.window.${sampledWindow.startStep}_${sampledWindow.endStep}`
  const assignmentRef =
    input.assignmentRef ?? `assignment.pylon.trace.${stamp}`
  const workerReceiptRef = `receipt.pylon.trace.worker.${stamp}`

  const result = await postAgentJson(
    fetchFn,
    `${baseUrl}/api/training/leases/${encodeURIComponent(leaseRef)}/trace-submission`,
    token,
    {
      assignmentRef,
      pylonDeviceRef,
      sampledWindow,
      sampledWindowRef,
      traceCommitmentDigestRef,
      workerReceiptRef,
      workloadFamily: input.workloadFamily,
    },
  )

  if (!result.ok) {
    return {
      digestMatchesExpectation,
      error: result.error,
      leaseRef,
      ok: false,
      reason: "submit_trace_failed",
      stepCount: trace.stepCount,
      traceDigest: trace.traceDigest,
    }
  }

  return {
    contribution:
      (result.json as Record<string, unknown> | null)?.contribution ?? null,
    digestMatchesExpectation,
    leaseRef,
    ok: true,
    reason: "submitted",
    stepCount: trace.stepCount,
    traceCommitmentDigestRef,
    traceDigest: trace.traceDigest,
  }
}

export type ValidateInput = {
  leaseRef: string
  // The validator's own device ref. MUST differ from the worker's device — the
  // server rejects self-validation. Default: the local node id.
  validatorDeviceRef: string
  workloadFamily: TassadarTraceWorkloadFamily
  workload: TassadarTraceWorkload
}

// §4.5 VALIDATOR: re-execute (replay) the same workload locally and submit the
// replay digest to the §4.2 route. The digest match vs. the paired worker
// contribution is computed server-side by the exact_trace_replay challenge.
//
// TODO(#5053): auto-discovery of "the next unpaired contribution assigned to
// this node" (the server-side selection endpoint) is not on main yet. Until it
// lands, the caller passes an explicit lease/contribution ref + workload here.
export async function submitReplayVerdict(
  options: TraceClientOptions,
  input: ValidateInput,
): Promise<Record<string, unknown>> {
  const fetchFn = options.fetchFn ?? fetch
  const executor = options.executor ?? executeTassadarNumericModel
  const token = requireAgentToken(options)
  const baseUrl = normalizeBaseUrl(options.baseUrl)
  const leaseRef = assertRef(input.leaseRef, "--lease-ref")
  const validatorDeviceRef = assertRef(
    input.validatorDeviceRef,
    "--device-ref",
  )

  const trace = await executor(input.workload.model, input.workload.steps)
  const replayDigestRef = `trace.tassadar.replay.${trace.traceDigest}`

  const result = await postAgentJson(
    fetchFn,
    `${baseUrl}/api/training/leases/${encodeURIComponent(leaseRef)}/replay-verdict`,
    token,
    {
      replayDigestRef,
      validatorDeviceRef,
      workloadFamily: input.workloadFamily,
    },
  )

  if (!result.ok) {
    return {
      error: result.error,
      leaseRef,
      ok: false,
      reason: "validate_failed",
      replayDigest: trace.traceDigest,
    }
  }

  const json = result.json as Record<string, unknown> | null
  return {
    challenge: json?.challenge ?? null,
    contribution: json?.contribution ?? null,
    leaseRef,
    ok: true,
    reason: "verdict_submitted",
    replayDigest: trace.traceDigest,
    replayDigestRef,
  }
}

// #5121: the next pending worker contribution this validator should replay,
// returned by GET /api/training/contributions/next-unpaired. Public-safe refs
// only; the workload itself is the committed fixture both sides run.
export type DiscoveredContribution = {
  contributionRef: string
  leaseRef: string
  trainingRunRef: string
  windowRef: string
  workloadFamily: TassadarTraceWorkloadFamily
  sampledWindow: { startStep: number; endStep: number }
  workerPylonDeviceRef: string
}

export type DiscoverNextInput = {
  validatorDeviceRef: string
  trainingRunRef?: string
}

// VALIDATOR auto-discovery: ask the server for the oldest pending worker
// contribution from a DISTINCT device. Returns `contribution: null` when nothing
// is pending (the node is idle, not in error).
export async function discoverNextUnpaired(
  options: TraceClientOptions,
  input: DiscoverNextInput,
): Promise<{
  ok: boolean
  contribution: DiscoveredContribution | null
  error?: string
}> {
  const fetchFn = options.fetchFn ?? fetch
  const token = requireAgentToken(options)
  const baseUrl = normalizeBaseUrl(options.baseUrl)
  const validatorDeviceRef = assertRef(input.validatorDeviceRef, "--device-ref")
  const params = new URLSearchParams({ validatorDeviceRef })
  if (input.trainingRunRef !== undefined && input.trainingRunRef !== "") {
    params.set("trainingRunRef", assertRef(input.trainingRunRef, "--run-ref"))
  }

  const result = await getAgentJson(
    fetchFn,
    `${baseUrl}/api/training/contributions/next-unpaired?${params.toString()}`,
    token,
  )

  if (!result.ok) {
    return { contribution: null, error: result.error, ok: false }
  }

  const contribution =
    (result.json as Record<string, unknown> | null)?.contribution ?? null

  return {
    contribution: contribution as DiscoveredContribution | null,
    ok: true,
  }
}

export type ValidatorAutoInput = {
  validatorDeviceRef: string
  // The committed fixture workload both sides run (e.g. the pinned self-test
  // workload). Replayed against whatever contribution discovery returns.
  workload: TassadarTraceWorkload
  trainingRunRef?: string
}

// §4.5 VALIDATOR, automated (#5121): discover the next unpaired contribution from
// a DISTINCT worker, then replay the fixture and submit the verdict — no manual
// lease/workload. Opt-in (only runs when the caller invokes it). Returns an idle
// result when nothing is pending so a watch loop can simply wait and retry.
export async function runValidatorAuto(
  options: TraceClientOptions,
  input: ValidatorAutoInput,
): Promise<Record<string, unknown>> {
  const discovery = await discoverNextUnpaired(options, {
    validatorDeviceRef: input.validatorDeviceRef,
    ...(input.trainingRunRef !== undefined
      ? { trainingRunRef: input.trainingRunRef }
      : {}),
  })

  if (!discovery.ok) {
    return { error: discovery.error, ok: false, reason: "discover_failed" }
  }

  if (discovery.contribution === null) {
    return { ok: true, paired: false, reason: "idle_no_pending" }
  }

  const target = discovery.contribution
  const verdict = await submitReplayVerdict(options, {
    leaseRef: target.leaseRef,
    validatorDeviceRef: input.validatorDeviceRef,
    workload: input.workload,
    workloadFamily: target.workloadFamily,
  })

  return {
    ...verdict,
    discoveredContributionRef: target.contributionRef,
    paired: verdict.ok === true,
    workerPylonDeviceRef: target.workerPylonDeviceRef,
  }
}
