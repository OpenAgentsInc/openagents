import { createHash } from "node:crypto"

import { Effect } from "effect"

import { describeExecutionProvider } from "./execution-provider.js"
import {
  cloudLaneForControlLane,
  isCloudTerminalEventKind,
  makeCloudControlClient,
  resolveCloudControlConfig,
  type CloudControlClient,
  type CloudControlConfig,
  type CloudPlacementAssignment,
  type CloudRunnerBinding,
  type CloudTerminalKind,
  type CloudWorkroomEvent,
} from "./cloud-control-client.js"
import type {
  ControlSessionExecutor,
  ControlSessionExecutorInput,
  ControlSessionExecutorResult,
  ControlSessionLane,
} from "./node/control-sessions.js"
import { makeOmegaGrantResolverFromEnv } from "../packages/runtime/src/index.js"

export type CloudSessionRequest = {
  providerKind: "openagents_cloud"
  objectiveRef: string
  verify: string[]
  workspaceRef: string
  timeoutSeconds: number
}

export type CloudSessionLease = {
  leaseRef: string
  state: "requested" | "ready" | "released"
}

export function buildCloudSessionRequest(input: {
  objective: string
  verify: string[]
  workspaceRef: string
  timeoutSeconds?: number
}): CloudSessionRequest {
  const provider = describeExecutionProvider("openagents_cloud")

  if (!provider.features.remoteRun) {
    throw new Error("OpenAgents Cloud execution provider cannot run remotely")
  }

  const objectiveHash = createHash("sha256")
    .update(input.objective)
    .digest("hex")
    .slice(0, 16)

  return {
    providerKind: "openagents_cloud",
    objectiveRef: `objective.${objectiveHash}`,
    verify: input.verify,
    workspaceRef: input.workspaceRef,
    timeoutSeconds: Math.min(
      1200,
      Math.max(1, input.timeoutSeconds ?? 600),
    ),
  }
}

// ---------------------------------------------------------------------------
// #4997 — OpenAgents Cloud execution-provider backend
// ---------------------------------------------------------------------------
//
// When a control session is spawned with a cloud lane (`cloud-gcp`,
// `cloud-shc`, or `auto` resolving to cloud) and a cloud control plane is
// configured, Pylon dispatches the run to the cloud instead of running it
// locally. The cloud executor:
//
//   1. Resolves the Codex auth grant via the Vortex-independent neutral
//      resolver (`makeOmegaGrantResolverFromEnv`, #4999) when grant refs are
//      present, confirming the grant is usable before placing the run.
//   2. Calls the cloud control plane `POST /v1/placement` to get a RunnerBinding
//      (GCE primary / SHC secondary) + externalRunId, then drives the run.
//   3. Polls the cloud run's `openagents.codex_workroom_event.v1` events and
//      MAPS them into the same `emit` callback the local executor uses, so the
//      existing `/sessions/:ref/events` stream is lane-transparent.
//   4. Records the runner binding/provenance (lane + runner id) so the desktop
//      "running on Google GCE / SHC" indicator is real, and surfaces artifacts
//      and the resource_usage_receipt ref on the terminal result.

export const CLOUD_PROVIDER_DISPLAY_REF = "session.pylon.cloud.openagents_cloud"

// Cloud-side input that the control-session layer threads through for a cloud
// lane. Grant/owner refs are supplied by the caller (Omega) for the cloud run;
// when absent, grant resolution is skipped and placement is attempted with the
// supplied (or empty) refs — the cloud endpoint enforces its own validation.
export type CloudSessionGrantBinding = {
  authGrantRef?: string
  providerAccountRef?: string
  ownerRef?: string
}

export type CloudExecutorOptions = {
  config: CloudControlConfig
  env: Readonly<Record<string, string | undefined>>
  // Injectable for tests; defaults to the real client over `fetch`.
  client?: CloudControlClient
  fetchImpl?: typeof fetch
  grantBindingForSession?: (sessionRef: string) => CloudSessionGrantBinding | undefined
  // Polling cadence for the events feed.
  pollIntervalMs?: number
  // Optional sink for the resolved runner binding so the session can record
  // provenance ("running on Google GCE / oa-shc-katy-01").
  onBinding?: (sessionRef: string, binding: CloudRunnerBinding) => void
}

function sleep(ms: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms))
}

// Map a cloud `openagents.codex_workroom_event.v1` kind to a bounded,
// human-readable activity line for the Pylon session stream. The cloud side
// redacts token-like content; the message is further bounded by the Pylon
// emit/redaction layer before publication.
function activityForCloudEvent(event: CloudWorkroomEvent, binding: CloudRunnerBinding | null): string {
  const where = binding ? ` (${binding.providerLane}:${binding.runnerId})` : ""
  switch (event.kind) {
    case "queued":
      return `cloud run queued${where}`
    case "placement.bound":
      return `cloud run placed${where}`
    case "started":
      return `cloud run started${where}`
    case "log":
      return event.summary ?? "cloud run log"
    case "redacted":
      return "cloud run produced a redacted event"
    case "artifact":
      return event.summary ?? "cloud run produced an artifact"
    case "receipt":
      return event.summary ?? "cloud run recorded a usage receipt"
    // #5005 — cloud GCE per-session VM lease lifecycle provenance. These are
    // bounded "lane/runner/VM lifecycle" lines, redaction-scanned like other
    // log events, and are NON-terminal (the session terminal kinds stay
    // completed/failed/timeout/cancelled).
    case "cloud.gce.provisioned":
      return event.summary ?? `cloud GCE VM provisioned${where}`
    case "cloud.gce.cleanup":
      return event.summary ?? `cloud GCE VM released${where}`
    case "cloud.gce.degraded":
      // Visible by design: signals a failed VM acquire / fallback to the local
      // control host.
      return event.summary ?? `cloud GCE VM degraded (acquire failed; falling back)${where}`
    case "cloud.gce.resource_usage_receipt":
      // The refs-only resource_usage_receipt.v1 ref is surfaced through the same
      // receiptRefs path the `receipt` kind uses (see processEvent).
      return event.summary ?? `cloud GCE recorded a usage receipt${where}`
    case "completed":
      return "cloud run completed"
    case "failed":
      return event.summary ?? "cloud run failed"
    case "timeout":
      return "cloud run timed out"
    case "cancelled":
      return "cloud run cancelled"
    default:
      return event.summary ?? `cloud run event: ${event.kind}`
  }
}

// Synthesize a Pylon dev-check projection for a cloud run terminal outcome. The
// cloud side owns acceptance for cloud lanes, so the local dev-check is a
// faithful projection of the cloud terminal state rather than a re-run of the
// verify command locally. This keeps the existing `runSession` complete/fail
// logic lane-transparent.
function syntheticDevCheck(passed: boolean): ControlSessionExecutorResult["devCheck"] {
  return {
    schema: "openagents.pylon.dev_check.v0.3",
    observedAt: new Date().toISOString(),
    action: "check",
    state: passed ? "passed" : "failed",
    changeSummary: {
      repo: { state: "ready", rootRef: null, branch: null, commit: null },
      dirty: {
        state: "unknown",
        changedCount: 0,
        stagedCount: 0,
        unstagedCount: 0,
        untrackedCount: 0,
      },
      changedFileRefs: [],
      areaRefs: [],
      blockerRefs: [],
    },
    checkPlan: {
      state: "skipped",
      commandRefs: [],
      blockerRefs: passed ? [] : ["blocker.pylon.cloud.run_not_completed"],
    },
    commandResults: [],
    latestRecordRef: null,
    branchUntouched: true,
    commitUntouched: true,
    pushPerformed: false,
    blockerRefs: passed ? [] : ["blocker.pylon.cloud.run_not_completed"],
  }
}

function stableRef(prefix: string, value: string): string {
  return `${prefix}.${createHash("sha256").update(value).digest("hex").slice(0, 24)}`
}

// Build a cloud executor compatible with the control-session layer. It returns
// a terminal `ControlSessionExecutorResult` so the existing session lifecycle
// (artifact write, completed/failed projection, SSE publish) is unchanged.
export function makeCloudControlSessionExecutor(
  options: CloudExecutorOptions,
): ControlSessionExecutor {
  const client =
    options.client ?? makeCloudControlClient(options.config, options.fetchImpl ?? fetch)
  const grantResolver = makeOmegaGrantResolverFromEnv(options.env, options.fetchImpl)
  const pollIntervalMs = options.pollIntervalMs ?? 1000

  return async (input: ControlSessionExecutorInput): Promise<ControlSessionExecutorResult> => {
    const binding = options.grantBindingForSession?.(input.sessionRef) ?? {}

    // 1) Resolve the Codex auth grant (when grant refs are present) so we never
    // place a cloud run against an unusable grant. The resolver is the neutral,
    // Vortex-independent contract from #4999.
    if (binding.authGrantRef !== undefined && binding.providerAccountRef !== undefined) {
      input.emit({ phase: "composer_event", message: "resolving cloud Codex auth grant" })
      try {
        await Effect.runPromise(
          grantResolver.resolveGrant({
            assignmentId: input.sessionRef,
            runnerSessionId: input.sessionRef,
            goal: input.objective,
            provider: "chatgpt_codex",
            providerAccountRef: binding.providerAccountRef as never,
            authGrantRef: binding.authGrantRef as never,
          }),
        )
      } catch (error) {
        // A grant resolution failure is a typed Probe error (no raw secrets).
        // Surface a bounded reason without leaking raw error text.
        const reason =
          error && typeof error === "object" && "_tag" in error
            ? String((error as { _tag: unknown })._tag)
            : "grant_resolve_error"
        throw new Error(`cloud grant resolution failed: ${reason}`)
      }
      input.emit({ phase: "composer_event", message: "cloud Codex auth grant resolved" })
    }

    // 2) Place the run on a concrete cloud runner.
    const lane: ControlSessionLane = input.lane ?? "auto"
    const assignment: CloudPlacementAssignment = {
      contract_version: "openagents.codex_placement_assignment.v1",
      run_id: input.sessionRef,
      owner_ref: binding.ownerRef ?? stableRef("owner.pylon.cloud", input.sessionRef),
      provider_account_ref: binding.providerAccountRef ?? "",
      auth_grant_ref: binding.authGrantRef ?? "",
      goal: input.objective,
      lane: cloudLaneForControlLane(lane),
      sandbox_mode: "danger_full_access",
      wallet_authority: false,
      created_at_ms: Date.now(),
    }
    const ack = await client.placeRun(assignment)
    const runnerBinding = ack.binding
    options.onBinding?.(input.sessionRef, runnerBinding)
    input.emit({
      phase: "composer_event",
      message: `placed on ${runnerBinding.providerLane}:${runnerBinding.runnerId} (${runnerBinding.lane})`,
    })

    // 3) Drive the run: process the ack events, then poll the events feed until
    // a terminal event, mapping each cloud event into the Pylon session stream.
    let composerEventIndex = 0
    let terminalKind: CloudTerminalKind | null = null
    const artifactRefs: string[] = []
    let receiptRef: string | null = null

    const processEvent = (event: CloudWorkroomEvent) => {
      composerEventIndex += 1
      input.emit({
        phase: "composer_event",
        message: activityForCloudEvent(event, runnerBinding),
        composerEventIndex,
      })
      if (event.artifactRefs) artifactRefs.push(...event.artifactRefs)
      if (
        event.receiptRefs &&
        event.receiptRefs.length > 0 &&
        receiptRef === null &&
        (event.kind === "receipt" || event.kind === "cloud.gce.resource_usage_receipt")
      ) {
        receiptRef = event.receiptRefs[0]
      }
      if (isCloudTerminalEventKind(event.kind)) {
        terminalKind = event.kind
      }
    }

    for (const event of ack.events) processEvent(event)

    let cursor = ack.events.length
    while (terminalKind === null) {
      if (input.abortSignal.aborted) {
        await client.cancelRun(ack.externalRunId)
        throw new Error("control session cancelled")
      }
      await sleep(pollIntervalMs)
      const page = await client.fetchEvents(ack.externalRunId, cursor)
      for (const event of page.events) processEvent(event)
      cursor = page.cursor
      // Defensive: honor a terminal status even if the terminal event arrived
      // without a recognized kind.
      if (
        terminalKind === null &&
        (page.status === "completed" || page.status === "failed" || page.status === "cancelled")
      ) {
        terminalKind =
          page.status === "completed"
            ? "completed"
            : page.status === "cancelled"
              ? "cancelled"
              : "failed"
      }
    }

    const finalKind: CloudTerminalKind = terminalKind
    if (finalKind === "cancelled") {
      throw new Error("control session cancelled")
    }
    const completed = finalKind === "completed"

    // 4) Return a terminal result. The runner binding provenance and the
    // resource_usage_receipt ref are surfaced through the executor result.
    return {
      commandCount: 0,
      devCheck: syntheticDevCheck(completed),
      editedFileCount: 0,
      eventCount: composerEventIndex,
      executionMode: "local_bounded",
      externalSessionRef: stableRef(
        "session.pylon.cloud.external_run",
        ack.externalRunId,
      ),
      responseDigestRef:
        receiptRef === null
          ? null
          : stableRef("digest.pylon.cloud.receipt", receiptRef),
      totalTokens: 0,
      cloudRunner: {
        lane: runnerBinding.lane,
        providerLane: runnerBinding.providerLane,
        runnerId: runnerBinding.runnerId,
        externalRunId: ack.externalRunId,
      },
      resourceUsageReceiptRef: receiptRef,
    }
  }
}
