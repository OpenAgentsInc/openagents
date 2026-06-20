// Pylon training cockpit CLI (issue #5035). Mirrors the Autopilot desktop
// training cockpit verbs (apps/autopilot-desktop/src/bun/training-runs.ts)
// against the SAME openagents.com training HTTP API, so an agent can drive the
// training lane headlessly without the desktop GUI.
//
// These are thin control wrappers over the public/admin training endpoints:
//   plan      POST /api/training/runs + POST /api/training/windows/plan   (admin)
//   activate  POST /api/training/windows/{ref}/activate                   (admin)
//   reconcile POST /api/training/windows/{ref}/reconcile                  (admin)
//   closeout  POST /api/training/windows/{ref}/closeout                   (admin)
//   claim     POST /api/training/leases/claim                            (public)
//   admit     POST /api/training/runs/{ref}/real-gradient-evidence        (admin)
//   status    GET  /api/training/runs                                    (public)
//
// No money/spend authority lives here — these are admin/public training-lane
// control calls. Admin verbs require an admin token (OA_TRAINING_ADMIN_TOKEN /
// --admin-token); without one they fail cleanly with a nonzero exit.

const TASSADAR_EXECUTOR_CAPABILITY_REF =
  "capability.tassadar_poc.numeric_model_executor"
const TASSADAR_EXECUTOR_SELF_TEST_RECEIPT_REF_PATTERN =
  /^receipt\.tassadar_executor\.self_test\.v1\.[0-9a-f]{16}$/

export type TrainingNetworkOptions = {
  baseUrl: string
  adminToken?: string
  fetchFn?: typeof fetch
  nowIso?: () => string
}

function normalizeBaseUrl(baseUrl: string): string {
  return baseUrl.replace(/\/+$/, "")
}

function safeRefStamp(iso: string): string {
  return iso.replace(/[^0-9a-zA-Z]/g, "").slice(0, 16)
}

const publicSafeRefPattern = /^[a-zA-Z0-9][a-zA-Z0-9._-]*$/

async function postJson(
  fetchFn: typeof fetch,
  url: string,
  token: string,
  body: unknown,
): Promise<{ ok: true; json: unknown } | { ok: false; status: number; error: string }> {
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
    return { ok: false, status: response.status, error: errorFromJson(json, `training admin ${response.status}`) }
  }
  return { ok: true, json }
}

async function postPublicJson(
  fetchFn: typeof fetch,
  url: string,
  body: unknown,
): Promise<{ ok: true; json: unknown } | { ok: false; status: number; error: string }> {
  const response = await fetchFn(url, {
    method: "POST",
    headers: { accept: "application/json", "content-type": "application/json" },
    body: JSON.stringify(body),
  })
  const json = (await response.json().catch(() => null)) as unknown
  if (!response.ok) {
    return { ok: false, status: response.status, error: errorFromJson(json, `training ${response.status}`) }
  }
  return { ok: true, json }
}

async function getPublicJson(
  fetchFn: typeof fetch,
  url: string,
): Promise<{ ok: true; json: unknown } | { ok: false; status: number; error: string }> {
  const response = await fetchFn(url, { headers: { accept: "application/json" } })
  const json = (await response.json().catch(() => null)) as unknown
  if (!response.ok) {
    return { ok: false, status: response.status, error: errorFromJson(json, `training ${response.status}`) }
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

function requireAdminToken(options: TrainingNetworkOptions): string {
  const token = options.adminToken?.trim() ?? ""
  if (token === "") {
    throw new Error(
      "training admin verb requires --admin-token or OA_TRAINING_ADMIN_TOKEN",
    )
  }
  return token
}

function assertWindowRef(windowRef: string | undefined): string {
  const ref = windowRef?.trim() ?? ""
  if (ref.length < 3 || ref.length > 260 || !publicSafeRefPattern.test(ref)) {
    throw new Error("invalid --window-ref (3-260 chars, [a-zA-Z0-9._-])")
  }
  return ref
}

function assertRunRef(runRef: string | undefined): string {
  const ref = runRef?.trim() ?? ""
  if (ref.length < 3 || ref.length > 260 || !publicSafeRefPattern.test(ref)) {
    throw new Error("invalid --run-ref (3-260 chars, [a-zA-Z0-9._-])")
  }
  return ref
}

// plan: create the run + window in one shot (mirrors the desktop plan button).
export async function planTrainingWindow(options: TrainingNetworkOptions): Promise<unknown> {
  const fetchFn = options.fetchFn ?? fetch
  const token = requireAdminToken(options)
  const baseUrl = normalizeBaseUrl(options.baseUrl)
  const fetchedAt = options.nowIso?.() ?? new Date().toISOString()
  const stamp = safeRefStamp(fetchedAt)
  const trainingRunRef = `training.run.pylon.cli.${stamp}`
  const windowRef = `training.window.pylon.cli.${stamp}`
  const sourceRefs = ["issue.github.openagents.5035"]

  const runResult = await postJson(fetchFn, `${baseUrl}/api/training/runs`, token, {
    maxAllowedStale: 5,
    promiseRef: "pylon.first_real_model_training_run.v1",
    receiptRefs: [`receipt.pylon.cli.training.run.planned.${stamp}`],
    sealPublicationCadenceWindows: 1,
    sourceRefs,
    trainingRunRef,
  })
  if (!runResult.ok) {
    return { ok: false, reason: "run_plan_failed", trainingRunRef, windowRef: null, error: runResult.error }
  }

  const windowResult = await postJson(fetchFn, `${baseUrl}/api/training/windows/plan`, token, {
    datasetRefs: ["dataset.cs336.a1.public"],
    homeworkKind: "admin_dispatched_homework",
    priority: 100,
    receiptRefs: [`receipt.pylon.cli.training.window.planned.${stamp}`],
    sourceRefs,
    trainingRunRef,
    windowRef,
  })
  if (!windowResult.ok) {
    return { ok: false, reason: "window_plan_failed", trainingRunRef, windowRef, error: windowResult.error }
  }

  return {
    ok: true,
    reason: "planned",
    trainingRunRef,
    windowRef,
    run: (runResult.json as Record<string, unknown> | null)?.run ?? null,
    window: (windowResult.json as Record<string, unknown> | null)?.window ?? null,
  }
}

async function transitionWindow(
  options: TrainingNetworkOptions,
  action: "activate" | "reconcile" | "closeout",
  windowRef: string,
): Promise<unknown> {
  const fetchFn = options.fetchFn ?? fetch
  const token = requireAdminToken(options)
  const baseUrl = normalizeBaseUrl(options.baseUrl)
  const fetchedAt = options.nowIso?.() ?? new Date().toISOString()
  const ref = assertWindowRef(windowRef)
  const stamp = safeRefStamp(fetchedAt)
  const url = `${baseUrl}/api/training/windows/${encodeURIComponent(ref)}/${action}`
  const result = await postJson(fetchFn, url, token, {
    actorRef: "operator.openagents.pylon_cli",
    receiptRef: `receipt.pylon.cli.training.window.${action}.${stamp}`,
  })
  if (!result.ok) {
    return { ok: false, reason: `${action}_failed`, windowRef: ref, error: result.error }
  }
  return {
    ok: true,
    reason: action === "activate" ? "activated" : action === "reconcile" ? "reconciled" : "closed_out",
    windowRef: ref,
    window: (result.json as Record<string, unknown> | null)?.window ?? null,
  }
}

export function activateTrainingWindow(options: TrainingNetworkOptions, windowRef: string): Promise<unknown> {
  return transitionWindow(options, "activate", windowRef)
}

export function reconcileTrainingWindow(options: TrainingNetworkOptions, windowRef: string): Promise<unknown> {
  return transitionWindow(options, "reconcile", windowRef)
}

export function closeoutTrainingWindow(options: TrainingNetworkOptions, windowRef: string): Promise<unknown> {
  return transitionWindow(options, "closeout", windowRef)
}

// Gap #2 (v1.0 self-serve shakeout): a contributor can claim + run verified
// work but is SILENTLY not paid if no Spark payout target is registered. This
// is the public-safe warning a `training claim` surfaces (human + --json) so a
// fresh contributor knows to register BEFORE earning, instead of discovering
// the verified pair settled to nothing. It is a WARNING, not a hard block:
// claiming still succeeds; the contributor is just told to register.
export type ClaimPayoutTargetWarning = {
  schema: "openagents.pylon.training_claim_payout_warning.v0.1"
  warningRef: "warning.training.claim.payout_target_unregistered"
  message: string
  actionRef: "action.wallet.register_payout_target"
  command: "pylon wallet register-payout-target"
}

const CLAIM_PAYOUT_TARGET_WARNING: ClaimPayoutTargetWarning = {
  schema: "openagents.pylon.training_claim_payout_warning.v0.1",
  warningRef: "warning.training.claim.payout_target_unregistered",
  message:
    "No payout target is registered: verified work will NOT pay until you run `pylon wallet register-payout-target`.",
  actionRef: "action.wallet.register_payout_target",
  command: "pylon wallet register-payout-target",
}

/**
 * Public-safe payout-target warning for the claim path, or `null` when a target
 * is registered. Pure + projection-safe (carries only refs + a human message,
 * never raw payment material), so the CLI can surface it in both human and
 * `--json` output. `payoutTargetRegistered === undefined` means the caller did
 * not resolve local payout-target state, so no warning is emitted.
 */
export function claimPayoutTargetWarning(
  payoutTargetRegistered: boolean | undefined,
): ClaimPayoutTargetWarning | null {
  return payoutTargetRegistered === false ? CLAIM_PAYOUT_TARGET_WARNING : null
}

export type TrainingPreflightCheck = {
  ok: boolean
  state: "ready" | "blocked"
  blockerRefs: string[]
  commandRefs: string[]
}

export type TrainingPreflightReport = {
  schema: "openagents.pylon.training_preflight.v0.1"
  ok: boolean
  reason: "ready" | "blocked"
  pylonRef: string
  lifecycle: string
  checks: {
    payoutTarget: TrainingPreflightCheck & {
      payoutTargetRef: string | null
    }
    tassadarExecutorCapability: TrainingPreflightCheck & {
      capabilityRef: string
      capabilityPresent: boolean
      selfTestReceiptRefs: string[]
    }
  }
  blockerRefs: string[]
  recommendedCommands: string[]
  authorityBoundary: string
}

/**
 * Local, read-only Tassadar preflight. It deliberately does not register a
 * payout target, run the executor self-test, heartbeat, claim work, or spend.
 * It only tells a contributor what their current local state implies before
 * they call `pylon training claim`.
 */
export function trainingPreflightReport(
  input: {
    pylonRef: string
    lifecycle: string
    capabilityRefs: string[]
    blockerRefs: string[]
    sparkPayoutTargetRef: string | null
  },
  options: { baseUrl: string },
): TrainingPreflightReport {
  const payoutTargetRef = input.sparkPayoutTargetRef?.trim() || null
  const payoutTargetReady = payoutTargetRef !== null
  const capabilityPresent = input.capabilityRefs.includes(
    TASSADAR_EXECUTOR_CAPABILITY_REF,
  )
  const selfTestReceiptRefs = input.capabilityRefs.filter(ref =>
    TASSADAR_EXECUTOR_SELF_TEST_RECEIPT_REF_PATTERN.test(ref),
  )
  const executorReady = capabilityPresent && selfTestReceiptRefs.length > 0
  const payoutBlockerRefs = payoutTargetReady
    ? []
    : ["blocker.training_preflight.payout_target_unregistered"]
  const executorBlockerRefs = executorReady
    ? []
    : ["blocker.training_preflight.tassadar_executor_self_test_missing"]
  const recommendedCommands = [
    ...(payoutTargetReady
      ? []
      : [
          `pylon wallet register-payout-target --kind spark-address --base-url ${options.baseUrl}`,
        ]),
    ...(executorReady
      ? []
      : [
          "pylon provider go-online",
          `pylon presence heartbeat --base-url ${options.baseUrl}`,
        ]),
  ]
  const blockerRefs = [
    ...payoutBlockerRefs,
    ...executorBlockerRefs,
    ...input.blockerRefs,
  ]

  return {
    authorityBoundary:
      "Read-only local preflight. It does not register payout material, run the executor self-test, heartbeat, claim work, spend, accept work, or settle payouts.",
    blockerRefs,
    checks: {
      payoutTarget: {
        blockerRefs: payoutBlockerRefs,
        commandRefs: payoutTargetReady
          ? []
          : ["pylon wallet register-payout-target --kind spark-address"],
        ok: payoutTargetReady,
        payoutTargetRef,
        state: payoutTargetReady ? "ready" : "blocked",
      },
      tassadarExecutorCapability: {
        blockerRefs: executorBlockerRefs,
        capabilityPresent,
        capabilityRef: TASSADAR_EXECUTOR_CAPABILITY_REF,
        commandRefs: executorReady
          ? []
          : ["pylon provider go-online", "pylon presence heartbeat"],
        ok: executorReady,
        selfTestReceiptRefs,
        state: executorReady ? "ready" : "blocked",
      },
    },
    lifecycle: input.lifecycle,
    ok: blockerRefs.length === 0,
    pylonRef: input.pylonRef,
    reason: blockerRefs.length === 0 ? "ready" : "blocked",
    recommendedCommands,
    schema: "openagents.pylon.training_preflight.v0.1",
  }
}

// claim: a Pylon claims a lease on an active window (public endpoint).
export async function claimTrainingLease(
  options: TrainingNetworkOptions,
  input: { pylonRef: string; leaseSeconds?: number; payoutTargetRegistered?: boolean },
): Promise<unknown> {
  const fetchFn = options.fetchFn ?? fetch
  const baseUrl = normalizeBaseUrl(options.baseUrl)
  const fetchedAt = options.nowIso?.() ?? new Date().toISOString()
  const pylonRef = input.pylonRef?.trim() ?? ""
  if (pylonRef.length < 3 || pylonRef.length > 120 || !publicSafeRefPattern.test(pylonRef)) {
    throw new Error("invalid --pylon-ref (3-120 chars, [a-zA-Z0-9._-])")
  }
  const stamp = safeRefStamp(fetchedAt)
  // Gap #2: surface the missing payout target alongside the claim result so a
  // contributor SEES they are not set up to be paid. A warning never blocks the
  // claim. `null` (target registered, or state not resolved) carries no field.
  const payoutTargetWarning = claimPayoutTargetWarning(input.payoutTargetRegistered)
  const result = await postPublicJson(fetchFn, `${baseUrl}/api/training/leases/claim`, {
    ...(input.leaseSeconds === undefined ? {} : { leaseSeconds: input.leaseSeconds }),
    pylonRef,
    receiptRefs: [`receipt.pylon.cli.training.lease.claim.${stamp}`],
  })
  if (!result.ok) {
    return {
      ok: false,
      reason: "claim_failed",
      pylonRef,
      lease: null,
      error: result.error,
      ...(payoutTargetWarning ? { payoutTargetWarning } : {}),
    }
  }
  const lease = (result.json as Record<string, unknown> | null)?.lease ?? null
  return {
    ok: lease !== null,
    reason: lease === null ? "claim_failed" : "claimed",
    pylonRef,
    lease,
    ...(payoutTargetWarning ? { payoutTargetWarning } : {}),
  }
}

// admit: admit real-gradient evidence for a training run (admin endpoint).
export async function admitTrainingEvidence(
  options: TrainingNetworkOptions,
  input: { trainingRunRef: string; packet: unknown },
): Promise<unknown> {
  const fetchFn = options.fetchFn ?? fetch
  const token = requireAdminToken(options)
  const baseUrl = normalizeBaseUrl(options.baseUrl)
  const runRef = assertRunRef(input.trainingRunRef)
  const url = `${baseUrl}/api/training/runs/${encodeURIComponent(runRef)}/real-gradient-evidence`
  const result = await postJson(fetchFn, url, token, input.packet)
  if (!result.ok) {
    return { ok: false, reason: "admit_failed", trainingRunRef: runRef, error: result.error }
  }
  return {
    ok: true,
    reason: "admitted",
    trainingRunRef: runRef,
    admission: (result.json as Record<string, unknown> | null) ?? null,
  }
}

// status: read the public training runs projection.
export async function readTrainingStatus(options: TrainingNetworkOptions): Promise<unknown> {
  const fetchFn = options.fetchFn ?? fetch
  const baseUrl = normalizeBaseUrl(options.baseUrl)
  const result = await getPublicJson(fetchFn, `${baseUrl}/api/training/runs`)
  if (!result.ok) {
    return { ok: false, reason: "status_failed", error: result.error }
  }
  return { ok: true, reason: "ok", ...(result.json as Record<string, unknown>) }
}
