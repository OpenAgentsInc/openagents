import { existsSync } from "node:fs"
import { readFile, writeFile } from "node:fs/promises"
import { createHash } from "node:crypto"
import type { BootstrapSummary } from "./bootstrap"
import { createSignedHeaders } from "./presence"
import {
  assertPublicProjectionSafe,
  ensurePylonLocalState,
  ensureStateDirectories,
  loadOrCreatePresenceState,
  type PylonLocalState,
} from "./state"
import { classifyMdkWallet, type WalletCommandRunner } from "./wallet"
import {
  admitGepaAssignmentToEnvelope,
  createDefaultGepaCapabilityEnvelope,
  type PylonGepaAssignmentRequirements,
  type PylonGepaCapabilityEnvelope,
} from "./gepa-capability"
import {
  PSIONIC_QWEN_MODEL_REFS,
  selectPsionicQwenModel,
  type PsionicQwenModelAdmission,
  type PsionicQwenTaskMode,
} from "../packages/runtime/src/index"

export type AssignmentPaymentMode = "no-spend" | "paid"
export type AssignmentStatus = "offered" | "accepted" | "running" | "closed" | "rejected" | "cancelled" | "timed-out" | "stale"

export type PylonAssignmentLease = {
  schema: "openagents.pylon.assignment_lease.v0.3"
  assignmentRef: string
  leaseRef: string
  goal: string
  paymentMode: AssignmentPaymentMode
  capabilityRefs: string[]
  codingAssignment?: AutopilotCodingAssignmentPayload
  backendRef?: string
  gepaRequirements?: PylonGepaAssignmentRequirements
  psionicQwenRequirements?: PylonPsionicQwenAssignmentRequirements
  expiresAt: string
  createdAt?: string
}

export type PylonPsionicQwenAssignmentRequirements = {
  workClass: "local_inference"
  mode: PsionicQwenTaskMode
  requiredModelRef?: string
  receiptRefs?: string[]
}

export type AssignmentPollResponse = {
  schema?: "openagents.pylon.assignment_poll_response.v0.3"
  leases?: PylonAssignmentLease[]
  assignment?: PylonAssignmentLease
}

export type AssignmentAcceptance = {
  ok: boolean
  accepted: boolean
  assignmentRef: string
  leaseRef: string
  statusRef: string
  denialRef?: string
  blockerRefs: string[]
}

export type AssignmentProgress = {
  schema: "openagents.pylon.assignment_progress.v0.3"
  assignmentRef: string
  leaseRef: string
  sequence: number
  status: "accepted" | "running" | "artifact-ready" | "proof-ready" | "closeout-submitted"
  message: string
  artifactRefs: string[]
  proofRefs: string[]
  observedAt: string
}

export type AssignmentCloseout = {
  schema: "openagents.pylon.assignment_closeout.v0.3"
  assignmentRef: string
  leaseRef: string
  status: "accepted" | "rejected" | "cancelled" | "timed-out" | "stale"
  paymentMode: AssignmentPaymentMode
  settlementState: "not_applicable" | "pending" | "recorded" | "blocked"
  payoutClaimAllowed: boolean
  artifactRefs: string[]
  blockerRefs: string[]
  buildRefs: string[]
  closeoutRefs: string[]
  previewRefs: string[]
  proofRefs: string[]
  receiptRefs: string[]
  resultRefs: string[]
  summaryRefs: string[]
  testRefs: string[]
  redacted: true
  completedAt: string
}

export type AssignmentClientOptions = {
  agentToken?: string
  baseUrl: string
  fetch?: typeof fetch
  now?: () => Date
  staleAfterMs?: number
  walletRunner?: WalletCommandRunner
  gepaEnvelope?: PylonGepaCapabilityEnvelope
  psionicQwenAdmission?: PsionicQwenModelAdmission
}

type AssignmentStore = {
  schema: "openagents.pylon.assignment_state.v0.3"
  leases: Record<string, { assignmentRef: string; status: AssignmentStatus; acceptedAt?: string; closedAt?: string }>
}

type JsonRecord = Record<string, unknown>
type AutopilotCodingAssignmentPayload = Readonly<Record<string, unknown>>
type PublicPylonAssignmentProjection = Readonly<{
  assignmentRef?: unknown
  codingAssignment?: unknown
  jobKind?: unknown
  leaseExpiresInSeconds?: unknown
  state?: unknown
  taskRefs?: unknown
}>

function stableRef(prefix: string, value: string) {
  return `${prefix}.${createHash("sha256").update(value).digest("hex").slice(0, 24)}`
}

function safeStringArray(value: unknown): string[] {
  return Array.isArray(value) ? value.filter((item): item is string => typeof item === "string") : []
}

async function loadAssignmentStore(state: PylonLocalState): Promise<AssignmentStore> {
  await ensureStateDirectories(state.paths)
  if (!existsSync(state.paths.assignmentState)) {
    return { schema: "openagents.pylon.assignment_state.v0.3", leases: {} }
  }
  return JSON.parse(await readFile(state.paths.assignmentState, "utf8")) as AssignmentStore
}

async function writeAssignmentStore(state: PylonLocalState, store: AssignmentStore) {
  assertPublicProjectionSafe(store)
  await writeFile(state.paths.assignmentState, `${JSON.stringify(store, null, 2)}\n`)
}

function isLegacyLease(value: unknown): value is PylonAssignmentLease {
  const lease = value as PylonAssignmentLease
  return (
    lease?.schema === "openagents.pylon.assignment_lease.v0.3" &&
    typeof lease.assignmentRef === "string" &&
    typeof lease.leaseRef === "string" &&
    typeof lease.goal === "string" &&
    (lease.paymentMode === "no-spend" || lease.paymentMode === "paid") &&
    Array.isArray(lease.capabilityRefs) &&
    typeof lease.expiresAt === "string"
  )
}

function codingAssignmentPaymentMode(codingAssignment: unknown): AssignmentPaymentMode {
  const budget = (codingAssignment as { budget?: { paymentMode?: unknown } } | null)?.budget
  return budget?.paymentMode === "buyer_funded" ? "paid" : "no-spend"
}

function codingAssignmentGoal(codingAssignment: unknown, fallback: string): string {
  const objective = (codingAssignment as { objective?: { objectiveRef?: unknown } } | null)?.objective
  return typeof objective?.objectiveRef === "string" ? objective.objectiveRef : fallback
}

function codingAssignmentCapabilityRefs(codingAssignment: unknown): string[] {
  return safeStringArray(
    (codingAssignment as { requiredCapabilityRefs?: unknown } | null)?.requiredCapabilityRefs,
  )
}

function normalizeProjectedAssignment(
  assignment: PublicPylonAssignmentProjection,
  now: Date,
): PylonAssignmentLease | null {
  if (typeof assignment.assignmentRef !== "string") {
    return null
  }
  const codingAssignment =
    assignment.codingAssignment !== null &&
    typeof assignment.codingAssignment === "object"
      ? assignment.codingAssignment as AutopilotCodingAssignmentPayload
      : undefined
  const expiresInSeconds =
    typeof assignment.leaseExpiresInSeconds === "number" &&
    Number.isFinite(assignment.leaseExpiresInSeconds)
      ? Math.max(0, assignment.leaseExpiresInSeconds)
      : 15 * 60
  const taskRef = safeStringArray(assignment.taskRefs)[0] ?? assignment.assignmentRef

  return {
    schema: "openagents.pylon.assignment_lease.v0.3",
    assignmentRef: assignment.assignmentRef,
    leaseRef: assignment.assignmentRef,
    goal: codingAssignmentGoal(codingAssignment, taskRef),
    paymentMode: codingAssignmentPaymentMode(codingAssignment),
    capabilityRefs: codingAssignmentCapabilityRefs(codingAssignment),
    ...(codingAssignment === undefined ? {} : { codingAssignment }),
    expiresAt: new Date(now.getTime() + expiresInSeconds * 1000).toISOString(),
  }
}

function normalizePollResponse(value: unknown, now: Date): PylonAssignmentLease[] {
  const response = value as AssignmentPollResponse
  const leases = response.leases ?? (response.assignment ? [response.assignment] : [])
  const assignments = (value as { assignments?: unknown }).assignments

  if (Array.isArray(assignments)) {
    return assignments.flatMap((assignment) => {
      if (isLegacyLease(assignment)) {
        return [assignment]
      }

      const normalized = normalizeProjectedAssignment(
        assignment as PublicPylonAssignmentProjection,
        now,
      )

      return normalized === null ? [] : [normalized]
    })
  }

  return leases.filter(isLegacyLease)
}

function hasRequiredCapabilities(state: PylonLocalState, lease: PylonAssignmentLease) {
  const local = new Set(state.runtime.capabilityRefs)
  return lease.capabilityRefs.every((ref) => local.has(ref))
}

function isExpired(lease: PylonAssignmentLease, now: Date) {
  return new Date(lease.expiresAt).getTime() <= now.getTime()
}

export async function computeAssignmentAdmission(
  state: PylonLocalState,
  lease: PylonAssignmentLease,
  options: Pick<AssignmentClientOptions, "now" | "staleAfterMs" | "walletRunner" | "psionicQwenAdmission"> = {},
) {
  const now = options.now?.() ?? new Date()
  const presence = await loadOrCreatePresenceState(state.paths, state.identity)
  const blockerRefs = new Set<string>()

  if (state.runtime.lifecycle === "paused") blockerRefs.add("blocker.assignment.lifecycle_paused")
  if (state.runtime.lifecycle === "offline") blockerRefs.add("blocker.assignment.lifecycle_offline")
  if (state.runtime.lifecycle === "degraded") blockerRefs.add("blocker.assignment.lifecycle_degraded")
  if (!presence.lastHeartbeatAt) blockerRefs.add("blocker.assignment.presence_never_heartbeat")
  if (presence.stale) blockerRefs.add("blocker.assignment.presence_stale")
  if (presence.lastHeartbeatAt) {
    const age = now.getTime() - new Date(presence.lastHeartbeatAt).getTime()
    if (age > (options.staleAfterMs ?? 120_000)) blockerRefs.add("blocker.assignment.presence_stale")
  }
  if (!hasRequiredCapabilities(state, lease)) blockerRefs.add("blocker.assignment.wrong_capability")
  if (lease.backendRef && !state.runtime.capabilityRefs.includes(lease.backendRef)) {
    blockerRefs.add("blocker.assignment.unsupported_backend")
  }
  if (lease.gepaRequirements) {
    const envelope = options.gepaEnvelope ?? createDefaultGepaCapabilityEnvelope()
    const gepaAdmission = admitGepaAssignmentToEnvelope(envelope, lease.gepaRequirements)
    for (const blockerRef of gepaAdmission.blockerRefs) blockerRefs.add(blockerRef)
  }
  if (lease.psionicQwenRequirements) {
    const psionicAdmission = options.psionicQwenAdmission ?? psionicAdmissionFromCapabilityRefs(state.runtime.capabilityRefs)
    const selection = selectPsionicQwenModel(psionicAdmission, lease.psionicQwenRequirements.mode)
    if (!selection.admitted) {
      for (const blockerRef of selection.blockerRefs) blockerRefs.add(blockerRef)
    }
    if (
      lease.psionicQwenRequirements.requiredModelRef &&
      selection.selectedModelRef !== lease.psionicQwenRequirements.requiredModelRef
    ) {
      blockerRefs.add(
        lease.psionicQwenRequirements.requiredModelRef === PSIONIC_QWEN_MODEL_REFS.qwen35_2b
          ? "blocker.psionic_qwen35.model_2b_missing"
          : "blocker.psionic_qwen35.required_model_missing",
      )
    }
  }
  if (isExpired(lease, now)) blockerRefs.add("blocker.assignment.lease_expired")
  if (lease.paymentMode === "paid") {
    const wallet = await classifyMdkWallet(options.walletRunner)
    if (!wallet.sendReady) blockerRefs.add("blocker.assignment.wallet_blocked")
  }

  return { admissible: blockerRefs.size === 0, blockerRefs: [...blockerRefs] }
}

async function postJson(options: AssignmentClientOptions, path: string, body: JsonRecord, state: PylonLocalState) {
  assertPublicProjectionSafe(body)
  const fetchImpl = options.fetch ?? fetch
  const url = new URL(path, options.baseUrl).toString()
  const text = JSON.stringify(body)
  const idempotencyKey = `pylon.assignment.${state.identity.pylonRef}.${stableRef("request", `${path}:${text}`)}`
  const headers = options.agentToken
    ? {
        authorization: `Bearer ${options.agentToken}`,
        "content-type": "application/json",
        "Idempotency-Key": idempotencyKey,
      }
    : {
        ...(await createSignedHeaders({
          method: "POST",
          url,
          body: text,
          pylonRef: state.identity.pylonRef,
          paths: state.paths,
          now: options.now?.(),
        })),
        "Idempotency-Key": idempotencyKey,
      }
  const response = await fetchImpl(url, { method: "POST", headers, body: text })
  const responseText = await response.text()
  const json = responseText.trim() ? (JSON.parse(responseText) as JsonRecord) : {}
  assertPublicProjectionSafe(json)
  if (!response.ok) {
    throw new Error(`OpenAgents assignment request failed (${response.status}): ${responseText}`)
  }
  return json
}

async function getJson(options: AssignmentClientOptions, path: string, state: PylonLocalState) {
  const fetchImpl = options.fetch ?? fetch
  const url = new URL(path, options.baseUrl).toString()
  const headers = options.agentToken
    ? {
        authorization: `Bearer ${options.agentToken}`,
      }
    : await createSignedHeaders({
        method: "GET",
        url,
        body: "",
        pylonRef: state.identity.pylonRef,
        paths: state.paths,
        now: options.now?.(),
      })
  const response = await fetchImpl(url, { method: "GET", headers })
  const responseText = await response.text()
  const json = responseText.trim() ? (JSON.parse(responseText) as JsonRecord) : {}
  assertPublicProjectionSafe(json)
  if (!response.ok) {
    throw new Error(`OpenAgents assignment request failed (${response.status}): ${responseText}`)
  }
  return json
}

export async function pollAssignments(summary: BootstrapSummary, options: AssignmentClientOptions) {
  const state = await ensurePylonLocalState(summary)
  const response = await getJson(
    options,
    `/api/pylons/${encodeURIComponent(state.identity.pylonRef)}/assignments`,
    state,
  )
  return normalizePollResponse(response, options.now?.() ?? new Date())
}

export async function acceptAssignment(
  summary: BootstrapSummary,
  lease: PylonAssignmentLease,
  options: AssignmentClientOptions,
): Promise<AssignmentAcceptance> {
  const state = await ensurePylonLocalState(summary)
  const store = await loadAssignmentStore(state)
  const existing = store.leases[lease.leaseRef]
  if (existing?.status === "accepted" || existing?.status === "running" || existing?.status === "closed") {
    return {
      ok: false,
      accepted: false,
      assignmentRef: lease.assignmentRef,
      leaseRef: lease.leaseRef,
      statusRef: stableRef("assignment.denial.duplicate", lease.leaseRef),
      denialRef: "denial.assignment.duplicate_lease",
      blockerRefs: ["blocker.assignment.duplicate_lease"],
    }
  }

  const admission = await computeAssignmentAdmission(state, lease, options)
  if (!admission.admissible) {
    return {
      ok: false,
      accepted: false,
      assignmentRef: lease.assignmentRef,
      leaseRef: lease.leaseRef,
      statusRef: stableRef("assignment.denial", `${lease.leaseRef}:${admission.blockerRefs.join(",")}`),
      denialRef: "denial.assignment.admission_blocked",
      blockerRefs: admission.blockerRefs,
    }
  }

  const body = {
    acceptanceRefs: [
      stableRef("assignment.acceptance", `${lease.assignmentRef}:${lease.goal}`),
    ],
    accepted: true,
    status: "accepted",
  }
  let response: JsonRecord
  try {
    response = await postJson(
      options,
      `/api/pylons/${encodeURIComponent(state.identity.pylonRef)}/assignments/${encodeURIComponent(lease.leaseRef)}/accept`,
      body,
      state,
    )
  } catch (error) {
    return {
      ok: false,
      accepted: false,
      assignmentRef: lease.assignmentRef,
      leaseRef: lease.leaseRef,
      statusRef: stableRef("assignment.denial.server", `${lease.leaseRef}:${String(error)}`),
      denialRef: "denial.assignment.server_rejected",
      blockerRefs: ["blocker.assignment.server_rejected"],
    }
  }
  const statusRef = String(response.statusRef ?? stableRef("assignment.accepted", lease.leaseRef))
  store.leases[lease.leaseRef] = {
    assignmentRef: lease.assignmentRef,
    status: "accepted",
    acceptedAt: (options.now?.() ?? new Date()).toISOString(),
  }
  await writeAssignmentStore(state, store)
  return { ok: true, accepted: true, assignmentRef: lease.assignmentRef, leaseRef: lease.leaseRef, statusRef, blockerRefs: [] }
}

export async function submitAssignmentProgress(
  summary: BootstrapSummary,
  progress: AssignmentProgress,
  options: AssignmentClientOptions,
) {
  const state = await ensurePylonLocalState(summary)
  const response = await postJson(
    options,
    `/api/pylons/${encodeURIComponent(state.identity.pylonRef)}/assignments/${encodeURIComponent(progress.leaseRef)}/progress`,
    progress,
    state,
  )
  return { progressRef: String(response.progressRef ?? stableRef("assignment.progress", `${progress.leaseRef}:${progress.sequence}`)) }
}

export async function submitAssignmentArtifacts(
  summary: BootstrapSummary,
  input: Readonly<{
    artifactRefs: ReadonlyArray<string>
    assignmentRef: string
    leaseRef: string
    proofRefs: ReadonlyArray<string>
  }>,
  options: AssignmentClientOptions,
) {
  const state = await ensurePylonLocalState(summary)
  const response = await postJson(
    options,
    `/api/pylons/${encodeURIComponent(state.identity.pylonRef)}/assignments/${encodeURIComponent(input.leaseRef)}/artifacts`,
    {
      artifactRefs: [...input.artifactRefs],
      proofRefs: [...input.proofRefs],
      status: "submitted",
      storageRefs: [
        stableRef("assignment.storage", `${input.assignmentRef}:${input.leaseRef}`),
      ],
    },
    state,
  )
  return { artifactRef: String(response.artifactRef ?? stableRef("assignment.artifacts", input.leaseRef)) }
}

export async function submitAssignmentCloseout(
  summary: BootstrapSummary,
  closeout: AssignmentCloseout,
  options: AssignmentClientOptions,
) {
  const state = await ensurePylonLocalState(summary)
  const response = await postJson(
    options,
    `/api/pylons/${encodeURIComponent(state.identity.pylonRef)}/assignments/${encodeURIComponent(closeout.leaseRef)}/closeout`,
    closeout,
    state,
  )
  const store = await loadAssignmentStore(state)
  store.leases[closeout.leaseRef] = {
    assignmentRef: closeout.assignmentRef,
    status: closeout.status === "accepted" ? "closed" : closeout.status,
    acceptedAt: store.leases[closeout.leaseRef]?.acceptedAt,
    closedAt: closeout.completedAt,
  }
  await writeAssignmentStore(state, store)
  return { closeoutRef: String(response.closeoutRef ?? stableRef("assignment.closeout", closeout.leaseRef)) }
}

export async function runNoSpendAssignment(summary: BootstrapSummary, options: AssignmentClientOptions) {
  const leases = await pollAssignments(summary, options)
  const lease = leases.find((candidate) => candidate.paymentMode === "no-spend")
  if (!lease) {
    return { ok: false, reason: "no no-spend assignment lease available", leases }
  }

  const acceptance = await acceptAssignment(summary, lease, options)
  if (!acceptance.accepted) {
    return { ok: false, acceptance }
  }

  const observedAt = (options.now?.() ?? new Date()).toISOString()
  const artifactRef = stableRef("assignment.artifact", `${lease.assignmentRef}:${lease.goal}`)
  const proofRef = stableRef("assignment.proof", `${lease.leaseRef}:${artifactRef}`)
  const progress: AssignmentProgress = {
    schema: "openagents.pylon.assignment_progress.v0.3",
    assignmentRef: lease.assignmentRef,
    leaseRef: lease.leaseRef,
    sequence: 1,
    status: "proof-ready",
    message: "No-spend assignment executed in bounded local Pylon runtime.",
    artifactRefs: [artifactRef],
    proofRefs: [proofRef],
    observedAt,
  }
  let progressReceipt: { progressRef: string }
  let artifactReceipt: { artifactRef: string } | null = null
  try {
    progressReceipt = await submitAssignmentProgress(summary, progress, options)
    artifactReceipt = await submitAssignmentArtifacts(
      summary,
      {
        artifactRefs: [artifactRef],
        assignmentRef: lease.assignmentRef,
        leaseRef: lease.leaseRef,
        proofRefs: [proofRef],
      },
      options,
    )
  } catch (error) {
    const message = String(error)
    const status = message.includes("(410)") ? "cancelled" : message.includes("(408)") ? "timed-out" : "rejected"
    const failureProofRef = stableRef("assignment.proof.failure", `${lease.leaseRef}:${status}:${message}`)
    const closeout: AssignmentCloseout = {
      schema: "openagents.pylon.assignment_closeout.v0.3",
      assignmentRef: lease.assignmentRef,
      leaseRef: lease.leaseRef,
      status,
      paymentMode: "no-spend",
      settlementState: "not_applicable",
      payoutClaimAllowed: false,
      artifactRefs: [],
      blockerRefs: ["blocker.assignment.progress_or_artifact_rejected"],
      buildRefs: [],
      closeoutRefs: [stableRef("assignment.closeout.failure", `${lease.leaseRef}:${status}`)],
      previewRefs: [],
      proofRefs: [failureProofRef],
      receiptRefs: [acceptance.statusRef, ...psionicCloseoutReceiptRefs(lease, options)],
      resultRefs: [],
      summaryRefs: [stableRef("assignment.summary.failure", `${lease.leaseRef}:${status}`)],
      testRefs: [],
      redacted: true,
      completedAt: observedAt,
    }
    const closeoutReceipt = await submitAssignmentCloseout(summary, closeout, options)
    return { ok: false, lease, acceptance, closeout, closeoutReceipt }
  }
  const closeout: AssignmentCloseout = {
    schema: "openagents.pylon.assignment_closeout.v0.3",
    assignmentRef: lease.assignmentRef,
    leaseRef: lease.leaseRef,
    status: "accepted",
    paymentMode: "no-spend",
    settlementState: "not_applicable",
    payoutClaimAllowed: false,
    artifactRefs: [artifactRef],
    blockerRefs: [],
    buildRefs: [stableRef("assignment.build.not_required", lease.leaseRef)],
    closeoutRefs: [stableRef("assignment.closeout.summary", lease.leaseRef)],
    previewRefs: [stableRef("assignment.preview.not_required", lease.leaseRef)],
    proofRefs: [proofRef],
    receiptRefs: [
      acceptance.statusRef,
      progressReceipt.progressRef,
      ...(artifactReceipt === null ? [] : [artifactReceipt.artifactRef]),
      ...psionicCloseoutReceiptRefs(lease, options),
    ],
    resultRefs: [stableRef("assignment.result.public_safe", lease.assignmentRef)],
    summaryRefs: [stableRef("assignment.summary.public_safe", lease.assignmentRef)],
    testRefs: [stableRef("assignment.test.not_required", lease.leaseRef)],
    redacted: true,
    completedAt: observedAt,
  }
  assertPublicProjectionSafe(closeout)
  const closeoutReceipt = await submitAssignmentCloseout(summary, closeout, options)
  return { ok: true, lease, acceptance, progress, closeout, progressReceipt, closeoutReceipt }
}

function psionicAdmissionFromCapabilityRefs(capabilityRefs: string[]): PsionicQwenModelAdmission {
  const admittedModelRefs = capabilityRefs.filter((ref) =>
    ref === PSIONIC_QWEN_MODEL_REFS.qwen35_0_8b || ref === PSIONIC_QWEN_MODEL_REFS.qwen35_2b
  ) as PsionicQwenModelAdmission["admittedModelRefs"]

  return {
    rows: [],
    admittedModelRefs,
    observedModelRefs: admittedModelRefs,
    blockerRefs: admittedModelRefs.length === 0 ? ["blocker.psionic_qwen35.qwen35_model_missing"] : [],
  }
}

function psionicCloseoutReceiptRefs(lease: PylonAssignmentLease, options: AssignmentClientOptions): string[] {
  if (!lease.psionicQwenRequirements) return []
  const admission = options.psionicQwenAdmission ?? {
    rows: [],
    admittedModelRefs: [],
    observedModelRefs: [],
    blockerRefs: [],
  }
  const selection = selectPsionicQwenModel(admission, lease.psionicQwenRequirements.mode)
  const refs = new Set<string>([
    "backend.psionic.qwen35",
    ...(lease.psionicQwenRequirements.receiptRefs ?? []),
  ])
  if (selection.selectedModelRef) refs.add(selection.selectedModelRef)
  return [...refs]
}
