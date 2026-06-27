import { createHash, verify as verifySignature } from "node:crypto"

import { Schema as S } from "effect"

export const PYLON_CAPABILITY_DELEGATION_SCHEMA =
  "openagents.pylon.capability_delegation.v0.1"

export const PylonDelegationFilesystemScope = S.Literals([
  "read_only",
  "workspace_write",
  "danger_full_access",
])
export type PylonDelegationFilesystemScope =
  typeof PylonDelegationFilesystemScope.Type

export const PylonDelegationNetworkScope = S.Literals([
  "denied",
  "loopback",
  "public_internet",
])
export type PylonDelegationNetworkScope =
  typeof PylonDelegationNetworkScope.Type

export const PylonDelegationSandboxMode = S.Literals([
  "read_only",
  "workspace_write",
  "owner_local_full_access",
])
export type PylonDelegationSandboxMode =
  typeof PylonDelegationSandboxMode.Type

export const PylonCapabilityDelegationConstraints = S.Struct({
  filesystem: PylonDelegationFilesystemScope,
  maxWallClockMs: S.Number,
  network: PylonDelegationNetworkScope,
  sandboxMode: PylonDelegationSandboxMode,
})
export type PylonCapabilityDelegationConstraints =
  typeof PylonCapabilityDelegationConstraints.Type

export const PylonCapabilityDelegationSignature = S.Struct({
  alg: S.Literal("Ed25519"),
  publicKeyPem: S.String,
  signatureBase64: S.String,
})
export type PylonCapabilityDelegationSignature =
  typeof PylonCapabilityDelegationSignature.Type

export const PylonCapabilityDelegationEnvelope = S.Struct({
  schema: S.Literal(PYLON_CAPABILITY_DELEGATION_SCHEMA),
  audienceRef: S.String,
  capabilityRefs: S.Array(S.String),
  constraints: PylonCapabilityDelegationConstraints,
  delegationRef: S.String,
  expiresAt: S.String,
  issuedAt: S.String,
  issuerRef: S.String,
  notBefore: S.optional(S.String),
  parentRef: S.optional(S.String),
  signature: PylonCapabilityDelegationSignature,
  subjectRef: S.String,
  toolRefs: S.Array(S.String),
  workflowRefs: S.Array(S.String),
})
export type PylonCapabilityDelegationEnvelope =
  typeof PylonCapabilityDelegationEnvelope.Type

export const decodePylonCapabilityDelegationEnvelope = S.decodeUnknownSync(
  PylonCapabilityDelegationEnvelope,
)

export type PylonDelegatedOperationRequest = {
  capabilityRef: string
  filesystem: PylonDelegationFilesystemScope
  maxWallClockMs: number
  network: PylonDelegationNetworkScope
  sandboxMode: PylonDelegationSandboxMode
  toolRef: string
  workflowRef: string
}

export type PylonCapabilityDelegationVerificationInput = {
  delegation: unknown
  now: Date
  operation: PylonDelegatedOperationRequest
  parentChain?: ReadonlyArray<unknown>
  revokedDelegationRefs?: ReadonlySet<string>
  trustedIssuerPublicKeys: ReadonlyMap<string, string>
}

export type PylonCapabilityDelegationVerification = {
  blockerRefs: string[]
  delegationRef: string | null
  ok: boolean
}

const publicRefPattern = /^[A-Za-z0-9_.:/=-]{3,260}$/
const delegationRefPrefix = "delegation.pylon.capability."

const filesystemRank: Record<PylonDelegationFilesystemScope, number> = {
  read_only: 0,
  workspace_write: 1,
  danger_full_access: 2,
}

const networkRank: Record<PylonDelegationNetworkScope, number> = {
  denied: 0,
  loopback: 1,
  public_internet: 2,
}

const sandboxRank: Record<PylonDelegationSandboxMode, number> = {
  read_only: 0,
  workspace_write: 1,
  owner_local_full_access: 2,
}

function stableJson(value: unknown): string {
  if (Array.isArray(value)) {
    return `[${value.map(stableJson).join(",")}]`
  }
  if (value !== null && typeof value === "object") {
    const record = value as Record<string, unknown>
    return `{${Object.keys(record)
      .sort()
      .map((key) => `${JSON.stringify(key)}:${stableJson(record[key])}`)
      .join(",")}}`
  }
  return JSON.stringify(value)
}

function withoutKeys(
  envelope: PylonCapabilityDelegationEnvelope,
  keys: ReadonlySet<string>,
): Record<string, unknown> {
  return Object.fromEntries(
    Object.entries(envelope).filter(([key]) => !keys.has(key)),
  )
}

function parseTime(value: string): number | null {
  const time = Date.parse(value)
  return Number.isFinite(time) ? time : null
}

function refListIsSafe(refs: ReadonlyArray<string>): boolean {
  return refs.every((ref) => publicRefPattern.test(ref))
}

function isSubset(
  child: ReadonlyArray<string>,
  parent: ReadonlyArray<string>,
): boolean {
  const parentSet = new Set(parent)
  return child.every((ref) => parentSet.has(ref))
}

function deriveDelegationRef(envelope: PylonCapabilityDelegationEnvelope): string {
  const refPayload = stableJson(withoutKeys(envelope, new Set(["delegationRef", "signature"])))
  return `${delegationRefPrefix}${createHash("sha256").update(refPayload).digest("hex").slice(0, 32)}`
}

export function pylonCapabilityDelegationSigningPayload(
  envelope: PylonCapabilityDelegationEnvelope,
): Uint8Array {
  return new TextEncoder().encode(stableJson(withoutKeys(envelope, new Set(["signature"]))))
}

function verifyEnvelopeSignature(
  envelope: PylonCapabilityDelegationEnvelope,
  trustedIssuerPublicKeys: ReadonlyMap<string, string>,
): boolean {
  const trustedKey = trustedIssuerPublicKeys.get(envelope.issuerRef)
  if (!trustedKey || trustedKey !== envelope.signature.publicKeyPem) return false
  try {
    return verifySignature(
      null,
      pylonCapabilityDelegationSigningPayload(envelope),
      trustedKey,
      Buffer.from(envelope.signature.signatureBase64, "base64"),
    )
  } catch {
    return false
  }
}

function operationWithinEnvelope(
  envelope: PylonCapabilityDelegationEnvelope,
  operation: PylonDelegatedOperationRequest,
): string[] {
  const blockers: string[] = []
  if (!envelope.capabilityRefs.includes(operation.capabilityRef)) {
    blockers.push("blocker.pylon.capability_delegation.capability_not_granted")
  }
  if (!envelope.toolRefs.includes(operation.toolRef)) {
    blockers.push("blocker.pylon.capability_delegation.tool_not_granted")
  }
  if (!envelope.workflowRefs.includes(operation.workflowRef)) {
    blockers.push("blocker.pylon.capability_delegation.workflow_not_granted")
  }
  if (filesystemRank[operation.filesystem] > filesystemRank[envelope.constraints.filesystem]) {
    blockers.push("blocker.pylon.capability_delegation.filesystem_scope_exceeded")
  }
  if (networkRank[operation.network] > networkRank[envelope.constraints.network]) {
    blockers.push("blocker.pylon.capability_delegation.network_scope_exceeded")
  }
  if (sandboxRank[operation.sandboxMode] > sandboxRank[envelope.constraints.sandboxMode]) {
    blockers.push("blocker.pylon.capability_delegation.sandbox_scope_exceeded")
  }
  if (operation.maxWallClockMs > envelope.constraints.maxWallClockMs) {
    blockers.push("blocker.pylon.capability_delegation.wall_clock_scope_exceeded")
  }
  return blockers
}

function childIsAttenuatedByParent(
  child: PylonCapabilityDelegationEnvelope,
  parent: PylonCapabilityDelegationEnvelope,
): string[] {
  const blockers: string[] = []
  if (child.parentRef !== parent.delegationRef) {
    blockers.push("blocker.pylon.capability_delegation.parent_ref_mismatch")
  }
  if (!isSubset(child.capabilityRefs, parent.capabilityRefs)) {
    blockers.push("blocker.pylon.capability_delegation.parent_capability_exceeded")
  }
  if (!isSubset(child.toolRefs, parent.toolRefs)) {
    blockers.push("blocker.pylon.capability_delegation.parent_tool_scope_exceeded")
  }
  if (!isSubset(child.workflowRefs, parent.workflowRefs)) {
    blockers.push("blocker.pylon.capability_delegation.parent_workflow_scope_exceeded")
  }
  if (filesystemRank[child.constraints.filesystem] > filesystemRank[parent.constraints.filesystem]) {
    blockers.push("blocker.pylon.capability_delegation.parent_filesystem_scope_exceeded")
  }
  if (networkRank[child.constraints.network] > networkRank[parent.constraints.network]) {
    blockers.push("blocker.pylon.capability_delegation.parent_network_scope_exceeded")
  }
  if (sandboxRank[child.constraints.sandboxMode] > sandboxRank[parent.constraints.sandboxMode]) {
    blockers.push("blocker.pylon.capability_delegation.parent_sandbox_scope_exceeded")
  }
  if (child.constraints.maxWallClockMs > parent.constraints.maxWallClockMs) {
    blockers.push("blocker.pylon.capability_delegation.parent_wall_clock_scope_exceeded")
  }
  const childExpiresAt = parseTime(child.expiresAt)
  const parentExpiresAt = parseTime(parent.expiresAt)
  if (childExpiresAt !== null && parentExpiresAt !== null && childExpiresAt > parentExpiresAt) {
    blockers.push("blocker.pylon.capability_delegation.parent_expiry_exceeded")
  }
  return blockers
}

function envelopeBaseBlockers(
  envelope: PylonCapabilityDelegationEnvelope,
  now: Date,
): string[] {
  const blockers: string[] = []
  if (
    !publicRefPattern.test(envelope.audienceRef) ||
    !publicRefPattern.test(envelope.delegationRef) ||
    !publicRefPattern.test(envelope.issuerRef) ||
    !publicRefPattern.test(envelope.subjectRef) ||
    !refListIsSafe(envelope.capabilityRefs) ||
    !refListIsSafe(envelope.toolRefs) ||
    !refListIsSafe(envelope.workflowRefs)
  ) {
    blockers.push("blocker.pylon.capability_delegation.unsafe_ref")
  }
  if (deriveDelegationRef(envelope) !== envelope.delegationRef) {
    blockers.push("blocker.pylon.capability_delegation.ref_mismatch")
  }
  const issuedAt = parseTime(envelope.issuedAt)
  const expiresAt = parseTime(envelope.expiresAt)
  const notBefore = envelope.notBefore ? parseTime(envelope.notBefore) : null
  const nowMs = now.getTime()
  if (issuedAt === null || expiresAt === null || (envelope.notBefore && notBefore === null)) {
    blockers.push("blocker.pylon.capability_delegation.invalid_time")
  }
  if (expiresAt !== null && nowMs >= expiresAt) {
    blockers.push("blocker.pylon.capability_delegation.expired")
  }
  if (notBefore !== null && nowMs < notBefore) {
    blockers.push("blocker.pylon.capability_delegation.not_yet_valid")
  }
  if (envelope.constraints.maxWallClockMs <= 0) {
    blockers.push("blocker.pylon.capability_delegation.invalid_wall_clock")
  }
  return blockers
}

export function pylonCapabilityDelegationRef(
  envelope: Omit<PylonCapabilityDelegationEnvelope, "delegationRef"> & {
    delegationRef?: string
  },
): string {
  return deriveDelegationRef({
    ...envelope,
    delegationRef: envelope.delegationRef ?? `${delegationRefPrefix}pending`,
  })
}

export function verifyPylonCapabilityDelegation(
  input: PylonCapabilityDelegationVerificationInput,
): PylonCapabilityDelegationVerification {
  const blockerRefs: string[] = []
  let delegation: PylonCapabilityDelegationEnvelope
  let parentChain: PylonCapabilityDelegationEnvelope[]

  try {
    delegation = decodePylonCapabilityDelegationEnvelope(input.delegation)
    parentChain = (input.parentChain ?? []).map((parent) =>
      decodePylonCapabilityDelegationEnvelope(parent),
    )
  } catch {
    return {
      blockerRefs: ["blocker.pylon.capability_delegation.invalid_envelope"],
      delegationRef: null,
      ok: false,
    }
  }

  const chain = [delegation, ...parentChain]
  for (const envelope of chain) {
    blockerRefs.push(...envelopeBaseBlockers(envelope, input.now))
    if (!verifyEnvelopeSignature(envelope, input.trustedIssuerPublicKeys)) {
      blockerRefs.push("blocker.pylon.capability_delegation.signature_untrusted")
    }
    if (input.revokedDelegationRefs?.has(envelope.delegationRef)) {
      blockerRefs.push("blocker.pylon.capability_delegation.revoked")
    }
  }

  for (let index = 0; index < chain.length - 1; index += 1) {
    blockerRefs.push(...childIsAttenuatedByParent(chain[index], chain[index + 1]))
  }
  if (delegation.parentRef && parentChain.length === 0) {
    blockerRefs.push("blocker.pylon.capability_delegation.parent_missing")
  }

  blockerRefs.push(...operationWithinEnvelope(delegation, input.operation))

  const uniqueBlockers = [...new Set(blockerRefs)]
  return {
    blockerRefs: uniqueBlockers,
    delegationRef: delegation.delegationRef,
    ok: uniqueBlockers.length === 0,
  }
}
