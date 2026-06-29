import { PYLON_CLIENT_VERSION } from "./version.js"

export type LiveWorkerLoopSmokeEnv = Readonly<Record<string, string | undefined>>

export type LiveWorkerLoopSmokeOptions = Readonly<{
  adminToken?: string
  agentToken: string
  baseUrl: string
  createAssignment: boolean
  fetch?: typeof fetch
  now?: () => Date
  pylonRef: string
}>

export type LiveWorkerLoopSmokeStep =
  | "register"
  | "heartbeat"
  | "walletReadiness"
  | "readAssignments"
  | "createAssignment"
  | "acceptAssignment"
  | "progressAssignment"
  | "submitArtifacts"
  | "operatorCloseout"

export type LiveWorkerLoopSmokeResult = Readonly<{
  assignmentRef: string | null
  baseUrl: string
  blockerRefs: ReadonlyArray<string>
  pylonRef: string
  skippedRefs: ReadonlyArray<string>
  status: "passed" | "partial"
  stepRefs: ReadonlyArray<string>
}>

const defaultBaseUrl = "https://openagents.com"
const defaultPylonRefPrefix = "pylon.codex.live_smoke"
const capabilityRefs = [
  "capability.public.pylon_cli",
  "capability.public.probe_runtime",
  "capability.public.background_loop",
  "cap.gepa.retained.v1",
]

const requireEnv = (env: LiveWorkerLoopSmokeEnv, key: string) => {
  const value = env[key]?.trim()
  if (!value) {
    throw new Error(`${key} is required.`)
  }
  return value
}

const envFlag = (env: LiveWorkerLoopSmokeEnv, key: string) =>
  ["1", "true", "yes"].includes((env[key] ?? "").trim().toLowerCase())

const compactTimestamp = (now: Date) =>
  now.toISOString().replace(/\D/g, "").slice(0, 14)

export const buildLiveWorkerLoopSmokeOptions = (
  env: LiveWorkerLoopSmokeEnv = process.env,
  now: Date = new Date(),
): LiveWorkerLoopSmokeOptions => ({
  adminToken: env.OPENAGENTS_ADMIN_API_TOKEN?.trim() || undefined,
  agentToken: requireEnv(env, "OPENAGENTS_AGENT_TOKEN"),
  baseUrl: env.OPENAGENTS_BASE_URL?.trim() || defaultBaseUrl,
  createAssignment: env.PYLON_LIVE_SMOKE_CREATE_ASSIGNMENT === undefined
    ? Boolean(env.OPENAGENTS_ADMIN_API_TOKEN?.trim())
    : envFlag(env, "PYLON_LIVE_SMOKE_CREATE_ASSIGNMENT"),
  pylonRef:
    env.PYLON_LIVE_SMOKE_PYLON_REF?.trim() ||
    `${defaultPylonRefPrefix}.${compactTimestamp(now)}`,
})

export const redactSmokeText = (text: string) =>
  text
    .replace(/Bearer\s+[A-Za-z0-9._~+/-]+/g, "Bearer <redacted>")
    .replace(/oa_agent_[A-Za-z0-9._~+/-]+/g, "oa_agent_<redacted>")
    .replace(
      /(OPENAGENTS_(?:AGENT_TOKEN|ADMIN_API_TOKEN)=)[^\s]+/g,
      "$1<redacted>",
    )

const makeIdempotencyKey = (
  pylonRef: string,
  step: LiveWorkerLoopSmokeStep,
  now: Date,
) => `pylon-live-worker-loop-smoke:${pylonRef}:${step}:${compactTimestamp(now)}`

const jsonRequest = async (
  options: LiveWorkerLoopSmokeOptions,
  input: Readonly<{
    admin?: boolean
    body?: unknown
    method: "GET" | "POST"
    path: string
    step: LiveWorkerLoopSmokeStep
  }>,
) => {
  const now = options.now?.() ?? new Date()
  const url = new URL(input.path, options.baseUrl).toString()
  const token = input.admin ? options.adminToken : options.agentToken

  if (!token) {
    throw new Error(`${input.step} requires OPENAGENTS_ADMIN_API_TOKEN.`)
  }

  const headers = new Headers({
    Authorization: `Bearer ${token}`,
    "Idempotency-Key": makeIdempotencyKey(options.pylonRef, input.step, now),
  })
  let body: string | undefined

  if (input.body !== undefined) {
    headers.set("Content-Type", "application/json")
    body = JSON.stringify(input.body)
  }

  const fetchImpl = options.fetch ?? fetch
  const response = await fetchImpl(url, {
    body,
    headers,
    method: input.method,
  })
  const text = await response.text()
  const json = text.trim() ? JSON.parse(text) : {}

  if (!response.ok) {
    throw new Error(
      `${input.step} failed (${response.status}): ${redactSmokeText(text)}`,
    )
  }

  return json as Record<string, unknown>
}

const firstAssignmentRef = (value: unknown): string | null => {
  const assignments = (value as { assignments?: Array<Record<string, unknown>> })
    .assignments

  if (!Array.isArray(assignments)) {
    return null
  }

  const active = assignments.find(assignment =>
    ["offered", "accepted", "running", "proof_submitted"].includes(
      String(assignment.state ?? ""),
    ),
  )

  return typeof active?.assignmentRef === "string" ? active.assignmentRef : null
}

export async function runLiveWorkerLoopSmoke(
  options: LiveWorkerLoopSmokeOptions,
): Promise<LiveWorkerLoopSmokeResult> {
  const now = options.now?.() ?? new Date()
  const stepRefs: string[] = []
  const skippedRefs: string[] = []
  const blockerRefs: string[] = []

  await jsonRequest(options, {
    body: {
      capabilityRefs,
      clientProtocolVersion: "0.3.0",
      clientVersion: PYLON_CLIENT_VERSION,
      displayName: "Pylon live worker-loop smoke",
      pylonRef: options.pylonRef,
      resourceMode: "background_20",
      statusRefs: ["status.public.live_worker_loop_smoke.registered"],
    },
    method: "POST",
    path: "/api/pylons/register",
    step: "register",
  })
  stepRefs.push("smoke.pylon.register")

  await jsonRequest(options, {
    body: {
      capacityRefs: ["capacity.public.operator_smoke"],
      clientProtocolVersion: "0.3.0",
      clientVersion: PYLON_CLIENT_VERSION,
      healthRefs: ["health.public.live_worker_loop_smoke.ok"],
      loadRefs: ["load.public.low"],
      resourceMode: "background_20",
      status: "online",
    },
    method: "POST",
    path: `/api/pylons/${encodeURIComponent(options.pylonRef)}/heartbeat`,
    step: "heartbeat",
  })
  stepRefs.push("smoke.pylon.heartbeat")

  await jsonRequest(options, {
    body: {
      balanceRefs: ["balance.public.live_worker_loop_smoke.not_reported"],
      liquidityRefs: ["liquidity.public.no_spend_smoke_not_required"],
      readinessRefs: ["readiness.public.live_worker_loop_smoke.receive_ready"],
      status: "ready",
      walletReady: true,
      walletRef: "wallet.public.live_worker_loop_smoke.redacted",
    },
    method: "POST",
    path: `/api/pylons/${encodeURIComponent(options.pylonRef)}/wallet-readiness`,
    step: "walletReadiness",
  })
  stepRefs.push("smoke.pylon.wallet_readiness")

  if (options.createAssignment) {
    await jsonRequest(options, {
      admin: true,
      body: {
        acceptanceCriteriaRefs: ["acceptance.public.pylon_runtime_gate.bounded_fixture_test_passes"],
        assignmentRef: `assignment.public.pylon_runtime_gate.${compactTimestamp(now)}`,
        campaignPaused: false,
        campaignPolicyRefs: ["policy.public.no_spend_smoke"],
        campaignRef: "campaign.public.pylon_runtime_gate_smoke",
        codingAssignment: {
          assignmentRef: `assignment.public.pylon_runtime_gate.${compactTimestamp(now)}`,
          budget: {
            paymentMode: "unpaid_smoke",
          },
          objective: {
            objectiveRef: "objective.public.pylon_runtime_gate.fixture_repair",
          },
          publicSafe: true,
          requiredCapabilityRefs: ["cap.gepa.retained.v1"],
          runtimeGate: {
            agentKind: "codex_cli_or_fixture",
            fixtureRef: "fixture.public.pylon.codex_runtime.sum_repair.v1",
            schema: "openagents.pylon.runtime_gate.v0.3",
          },
          schema: "openagents.autopilot_coding_assignment.v1",
        },
        closeoutPathRefs: ["closeout.public.operator_review_required"],
        forumAutoPublishAllowed: false,
        idempotencyRefs: ["idempotency.public.pylon_runtime_gate"],
        jobKind: "validation",
        leaseSeconds: 600,
        noDuplicateAssignmentRefs: ["dedupe.public.pylon_runtime_gate_smoke"],
        noForumAutoPublishRefs: ["policy.public.no_forum_auto_publish"],
        operatorPauseRefs: ["pause.public.pylon_runtime_gate.not_paused"],
        paymentMode: "unpaid_smoke",
        pylonRef: options.pylonRef,
        requiredCapabilityRefs: capabilityRefs,
        resultExpectationRefs: ["result.public.pylon_runtime_gate.fixture_repair_passed"],
        rollbackRefs: ["rollback.public.cancel_smoke_assignment"],
        selectionPolicyRefs: ["selection.public.explicit_pylon_ref"],
        spendCapRefs: ["spend_cap.public.no_spend"],
        taskRefs: ["task.public.pylon_runtime_gate.fixture_repair"],
      },
      method: "POST",
      path: "/api/operator/pylons/assignments",
      step: "createAssignment",
    })
    stepRefs.push("smoke.pylon.assignment_create")
  } else {
    skippedRefs.push("skip.pylon.assignment_create.admin_token_missing")
  }

  const assignments = await jsonRequest(options, {
    method: "GET",
    path: `/api/pylons/${encodeURIComponent(options.pylonRef)}/assignments`,
    step: "readAssignments",
  })
  stepRefs.push("smoke.pylon.assignments_read")

  const assignmentRef = firstAssignmentRef(assignments)

  if (assignmentRef === null) {
    blockerRefs.push("blocker.pylon.live_worker_loop.no_assignment_available")
    return {
      assignmentRef,
      baseUrl: options.baseUrl,
      blockerRefs,
      pylonRef: options.pylonRef,
      skippedRefs,
      status: "partial",
      stepRefs,
    }
  }

  await jsonRequest(options, {
    body: {
      acceptanceRefs: ["acceptance.public.live_worker_loop_smoke"],
      accepted: true,
      resourceMode: "background_20",
      status: "accepted",
    },
    method: "POST",
    path: `/api/pylons/${encodeURIComponent(options.pylonRef)}/assignments/${encodeURIComponent(assignmentRef)}/accept`,
    step: "acceptAssignment",
  })
  stepRefs.push("smoke.pylon.assignment_accept")

  await jsonRequest(options, {
    body: {
      progressPercent: 100,
      progressRefs: ["progress.public.live_worker_loop_smoke.done"],
      status: "proof_ready",
    },
    method: "POST",
    path: `/api/pylons/${encodeURIComponent(options.pylonRef)}/assignments/${encodeURIComponent(assignmentRef)}/progress`,
    step: "progressAssignment",
  })
  stepRefs.push("smoke.pylon.assignment_progress")

  await jsonRequest(options, {
    body: {
      artifactRefs: ["artifact.public.live_worker_loop_smoke.echo"],
      proofRefs: ["proof.public.live_worker_loop_smoke.echo"],
      status: "submitted",
      storageRefs: ["storage.public.openagents_d1_pylon_events"],
    },
    method: "POST",
    path: `/api/pylons/${encodeURIComponent(options.pylonRef)}/assignments/${encodeURIComponent(assignmentRef)}/artifacts`,
    step: "submitArtifacts",
  })
  stepRefs.push("smoke.pylon.artifacts")

  if (options.adminToken) {
    await jsonRequest(options, {
      admin: true,
      body: {
        accepted: true,
        acceptedWorkRefs: ["accepted_work.public.live_worker_loop_smoke"],
        closeoutRefs: ["closeout.public.live_worker_loop_smoke"],
        status: "accepted",
      },
      method: "POST",
      path: `/api/operator/pylons/assignments/${encodeURIComponent(assignmentRef)}/closeout`,
      step: "operatorCloseout",
    })
    stepRefs.push("smoke.pylon.operator_closeout")
  } else {
    skippedRefs.push("skip.pylon.operator_closeout.admin_token_missing")
  }

  return {
    assignmentRef,
    baseUrl: options.baseUrl,
    blockerRefs,
    pylonRef: options.pylonRef,
    skippedRefs,
    status: "passed",
    stepRefs,
  }
}
