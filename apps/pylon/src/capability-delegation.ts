import { createHmac, createHash, timingSafeEqual } from "node:crypto"

import { assertPublicProjectionSafe } from "./state.js"

export const PYLON_CAPABILITY_DELEGATION_SCHEMA_REF =
  "openagents.pylon.capability_delegation.v0.1"
export const PYLON_CAPABILITY_DELEGATION_ALGORITHM = "hmac-sha256.v0.1"

export type PylonCapabilityCaveats = {
  allowedToolRefs: string[]
  sandboxProfileRef: string
  maxUses: number
}

export type PylonCapabilityDelegation = {
  schema: typeof PYLON_CAPABILITY_DELEGATION_SCHEMA_REF
  algorithm: typeof PYLON_CAPABILITY_DELEGATION_ALGORITHM
  issuerRef: string
  audienceRef: string
  subjectRef: string
  capabilityRefs: string[]
  caveats: PylonCapabilityCaveats
  notBefore: string
  expiresAt: string
  nonceRef: string
  parentDigestRef: string | null
  proofRef: string
}

export type PylonCapabilityRevocationSet = {
  revokedRefs: string[]
}

export type PylonCapabilityEvaluation = {
  admitted: boolean
  selectedCapabilityRefs: string[]
  blockerRefs: string[]
  expiresAt: string
}

type DelegationSigningInput = Omit<PylonCapabilityDelegation, "proofRef">

const publicRefPattern = /^[A-Za-z0-9][A-Za-z0-9_.:/-]{2,220}$/

const unique = (values: readonly string[]): string[] => [...new Set(values)]

function assertPublicRef(value: string, field: string): void {
  if (!publicRefPattern.test(value)) {
    throw new Error(`${field} must be a bounded public-safe ref`)
  }
}

function assertPositiveInteger(value: number, field: string): void {
  if (!Number.isInteger(value) || value < 1 || value > 1_000_000) {
    throw new Error(`${field} must be a positive bounded integer`)
  }
}

function assertTimestamp(value: string, field: string): void {
  if (Number.isNaN(Date.parse(value))) {
    throw new Error(`${field} must be an ISO timestamp`)
  }
}

function canonicalJson(value: unknown): string {
  if (Array.isArray(value)) return `[${value.map(canonicalJson).join(",")}]`
  if (value === null || typeof value !== "object") return JSON.stringify(value)
  return `{${Object.entries(value as Record<string, unknown>)
    .sort(([left], [right]) => left.localeCompare(right))
    .map(([key, child]) => `${JSON.stringify(key)}:${canonicalJson(child)}`)
    .join(",")}}`
}

function digestRef(value: unknown): string {
  return `digest.pylon.capability_delegation.${createHash("sha256")
    .update(canonicalJson(value))
    .digest("hex")
    .slice(0, 32)}`
}

function proofRef(input: DelegationSigningInput, signingKey: Uint8Array): string {
  return `proof.pylon.capability_delegation.${createHmac("sha256", signingKey)
    .update(canonicalJson(input))
    .digest("hex")
    .slice(0, 48)}`
}

function signingInput(token: PylonCapabilityDelegation): DelegationSigningInput {
  const { proofRef: _proofRef, ...input } = token
  return input
}

function assertValidDelegationShape(input: DelegationSigningInput): void {
  assertPublicRef(input.issuerRef, "issuerRef")
  assertPublicRef(input.audienceRef, "audienceRef")
  assertPublicRef(input.subjectRef, "subjectRef")
  assertPublicRef(input.caveats.sandboxProfileRef, "sandboxProfileRef")
  assertPublicRef(input.nonceRef, "nonceRef")
  if (input.parentDigestRef !== null) assertPublicRef(input.parentDigestRef, "parentDigestRef")
  for (const ref of input.capabilityRefs) assertPublicRef(ref, "capabilityRef")
  for (const ref of input.caveats.allowedToolRefs) assertPublicRef(ref, "allowedToolRef")
  assertPositiveInteger(input.caveats.maxUses, "maxUses")
  assertTimestamp(input.notBefore, "notBefore")
  assertTimestamp(input.expiresAt, "expiresAt")
  if (Date.parse(input.expiresAt) <= Date.parse(input.notBefore)) {
    throw new Error("expiresAt must be after notBefore")
  }
}

function assertAttenuates(parent: PylonCapabilityDelegation, child: DelegationSigningInput): void {
  const parentCapabilities = new Set(parent.capabilityRefs)
  const parentTools = new Set(parent.caveats.allowedToolRefs)
  const blockers: string[] = []

  if (!child.capabilityRefs.every((ref) => parentCapabilities.has(ref))) {
    blockers.push("blocker.pylon.capability_delegation.capability_widened")
  }
  if (!child.caveats.allowedToolRefs.every((ref) => parentTools.has(ref))) {
    blockers.push("blocker.pylon.capability_delegation.tool_scope_widened")
  }
  if (child.caveats.maxUses > parent.caveats.maxUses) {
    blockers.push("blocker.pylon.capability_delegation.use_budget_widened")
  }
  if (child.caveats.sandboxProfileRef !== parent.caveats.sandboxProfileRef) {
    blockers.push("blocker.pylon.capability_delegation.sandbox_profile_changed")
  }
  if (Date.parse(child.notBefore) < Date.parse(parent.notBefore)) {
    blockers.push("blocker.pylon.capability_delegation.not_before_widened")
  }
  if (Date.parse(child.expiresAt) > Date.parse(parent.expiresAt)) {
    blockers.push("blocker.pylon.capability_delegation.expiry_widened")
  }
  if (child.parentDigestRef !== digestRef(parent)) {
    blockers.push("blocker.pylon.capability_delegation.parent_digest_mismatch")
  }
  if (blockers.length > 0) {
    throw new Error(blockers.join(","))
  }
}

export function createPylonCapabilityDelegation(input: {
  issuerRef: string
  audienceRef: string
  subjectRef: string
  capabilityRefs: readonly string[]
  caveats: PylonCapabilityCaveats
  notBefore: string
  expiresAt: string
  nonceRef: string
  signingKey: Uint8Array
  parent?: PylonCapabilityDelegation
}): PylonCapabilityDelegation {
  const signing: DelegationSigningInput = {
    schema: PYLON_CAPABILITY_DELEGATION_SCHEMA_REF,
    algorithm: PYLON_CAPABILITY_DELEGATION_ALGORITHM,
    issuerRef: input.issuerRef,
    audienceRef: input.audienceRef,
    subjectRef: input.subjectRef,
    capabilityRefs: unique(input.capabilityRefs),
    caveats: {
      allowedToolRefs: unique(input.caveats.allowedToolRefs),
      sandboxProfileRef: input.caveats.sandboxProfileRef,
      maxUses: input.caveats.maxUses,
    },
    notBefore: input.notBefore,
    expiresAt: input.expiresAt,
    nonceRef: input.nonceRef,
    parentDigestRef: input.parent === undefined ? null : digestRef(input.parent),
  }
  assertValidDelegationShape(signing)
  if (input.parent !== undefined) assertAttenuates(input.parent, signing)

  const token: PylonCapabilityDelegation = {
    ...signing,
    proofRef: proofRef(signing, input.signingKey),
  }
  assertPublicProjectionSafe(token)
  return token
}

export function verifyPylonCapabilityProof(
  token: PylonCapabilityDelegation,
  signingKey: Uint8Array,
): boolean {
  const expected = proofRef(signingInput(token), signingKey)
  const expectedBytes = Buffer.from(expected)
  const actualBytes = Buffer.from(token.proofRef)
  return expectedBytes.length === actualBytes.length && timingSafeEqual(expectedBytes, actualBytes)
}

export function evaluatePylonCapabilityDelegation(input: {
  token: PylonCapabilityDelegation
  signingKey: Uint8Array
  requiredCapabilityRefs: readonly string[]
  requiredToolRef: string
  revocations?: PylonCapabilityRevocationSet
  now?: Date
}): PylonCapabilityEvaluation {
  const blockerRefs = new Set<string>()
  const now = input.now ?? new Date()
  const token = input.token

  try {
    assertValidDelegationShape(signingInput(token))
    assertPublicRef(input.requiredToolRef, "requiredToolRef")
  } catch {
    blockerRefs.add("blocker.pylon.capability_delegation.invalid_shape")
  }

  if (!verifyPylonCapabilityProof(token, input.signingKey)) {
    blockerRefs.add("blocker.pylon.capability_delegation.bad_proof")
  }
  if (Date.parse(token.notBefore) > now.getTime()) {
    blockerRefs.add("blocker.pylon.capability_delegation.not_yet_valid")
  }
  if (Date.parse(token.expiresAt) <= now.getTime()) {
    blockerRefs.add("blocker.pylon.capability_delegation.expired")
  }
  const revokedRefs = new Set(input.revocations?.revokedRefs ?? [])
  if (
    revokedRefs.has(token.proofRef) ||
    revokedRefs.has(digestRef(token)) ||
    token.parentDigestRef !== null && revokedRefs.has(token.parentDigestRef)
  ) {
    blockerRefs.add("blocker.pylon.capability_delegation.revoked")
  }

  const capabilitySet = new Set(token.capabilityRefs)
  const selectedCapabilityRefs = input.requiredCapabilityRefs.filter((ref) => capabilitySet.has(ref))
  if (!input.requiredCapabilityRefs.every((ref) => capabilitySet.has(ref))) {
    blockerRefs.add("blocker.pylon.capability_delegation.missing_capability")
  }
  if (!token.caveats.allowedToolRefs.includes(input.requiredToolRef)) {
    blockerRefs.add("blocker.pylon.capability_delegation.tool_not_allowed")
  }

  return {
    admitted: blockerRefs.size === 0,
    selectedCapabilityRefs,
    blockerRefs: [...blockerRefs],
    expiresAt: token.expiresAt,
  }
}
