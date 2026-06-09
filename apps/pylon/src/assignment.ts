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
  proofRefs: string[]
  receiptRefs: string[]
  redacted: true
  completedAt: string
}

export type AssignmentClientOptions = {
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

function stableRef(prefix: string, value: string) {
  return `${prefix}.${createHash("sha256").update(value).digest("hex").slice(0, 24)}`
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

function normalizePollResponse(value: unknown): PylonAssignmentLease[] {
  const response = value as AssignmentPollResponse
  const leases = response.leases ?? (response.assignment ? [response.assignment] : [])
  return leases.filter((lease): lease is PylonAssignmentLease => {
    return (
      lease?.schema === "openagents.pylon.assignment_lease.v0.3" &&
      typeof lease.assignmentRef === "string" &&
      typeof lease.leaseRef === "string" &&
      typeof lease.goal === "string" &&
      (lease.paymentMode === "no-spend" || lease.paymentMode === "paid") &&
      Array.isArray(lease.capabilityRefs) &&
      typeof lease.expiresAt === "string"
    )
  })
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
  const headers = await createSignedHeaders({
    method: "POST",
    url,
    body: text,
    pylonRef: state.identity.pylonRef,
    identityPath: state.paths.identity,
    now: options.now?.(),
  })
  const response = await fetchImpl(url, { method: "POST", headers, body: text })
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
  const body = {
    schema: "openagents.pylon.assignment_poll.v0.3",
    pylonRef: state.identity.pylonRef,
    capabilityRefs: state.runtime.capabilityRefs,
    lifecycle: state.runtime.lifecycle,
  }
  const response = await postJson(options, `/api/pylons/${encodeURIComponent(state.identity.pylonRef)}/assignments/poll`, body, state)
  return normalizePollResponse(response)
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
    schema: "openagents.pylon.assignment_accept.v0.3",
    pylonRef: state.identity.pylonRef,
    assignmentRef: lease.assignmentRef,
    leaseRef: lease.leaseRef,
    paymentMode: lease.paymentMode,
    acceptedAt: (options.now?.() ?? new Date()).toISOString(),
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
    acceptedAt: body.acceptedAt,
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
  try {
    progressReceipt = await submitAssignmentProgress(summary, progress, options)
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
      proofRefs: [failureProofRef],
      receiptRefs: [acceptance.statusRef, ...psionicCloseoutReceiptRefs(lease, options)],
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
    proofRefs: [proofRef],
    receiptRefs: [acceptance.statusRef, progressReceipt.progressRef, ...psionicCloseoutReceiptRefs(lease, options)],
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
