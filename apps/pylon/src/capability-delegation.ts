import { createHash } from "node:crypto"

export const PYLON_DELEGATION_CAPABILITY_REF = "capability.pylon.delegation.revocable.v0.1"

export type PylonDelegationChain = {
  schema: "openagents.pylon.capability_delegation_chain.v0.1"
  rootIssuerRef: string
  subjectRef: string
  audienceRef: string
  issuedAt: string
  expiresAt: string
  invocationRef?: string
  capabilities: readonly PylonDelegatedCapability[]
  attenuation?: PylonDelegationAttenuation
  revocation?: PylonDelegationRevocation
  proofRefs?: readonly string[]
}

export type PylonDelegatedCapability = {
  capabilityRef: string
  action: string
  resourceRef: string
}

export type PylonDelegationAttenuation = {
  maxTtlSeconds?: number
  allowedCapabilityRefs?: readonly string[]
  allowedActions?: readonly string[]
  allowedResourceRefs?: readonly string[]
  denyNetwork?: boolean
  requirePromptInjectionScreen?: boolean
}

export type PylonDelegationRevocation = {
  revokedRefs?: readonly string[]
  revokedAt?: string
  reasonRef?: string
}

export type PylonDelegationAdmissionInput = {
  chain: PylonDelegationChain
  now: Date
  localPylonRef: string
  localCapabilityRefs: readonly string[]
  requestedCapabilityRefs: readonly string[]
  objectiveText?: string
}

export type PylonDelegationAdmission = {
  admitted: boolean
  delegationRef: string
  blockerRefs: string[]
}

const safeRefPattern = /^[A-Za-z0-9][A-Za-z0-9_.:/-]{1,180}$/
const allowedActionPattern = /^(assignment|tool|workspace|codex|claude|pylon)\.[A-Za-z0-9_.:-]{1,96}$/
const promptInjectionRiskPattern =
  /\b(ignore|bypass|override|disable|reveal|exfiltrate|leak|print|dump)\b.{0,48}\b(previous|system|developer|instruction|policy|secret|token|credential|private key|wallet|mnemonic|sandbox|capability)\b/i

const unique = (values: readonly string[]): string[] => [...new Set(values)]

const parseTime = (value: string): number => {
  const millis = new Date(value).getTime()
  return Number.isFinite(millis) ? millis : Number.NaN
}

const validRef = (value: string): boolean => safeRefPattern.test(value)

const stableDelegationRef = (chain: PylonDelegationChain): string =>
  `delegation.pylon.${createHash("sha256")
    .update(JSON.stringify({
      audienceRef: chain.audienceRef,
      capabilities: chain.capabilities,
      expiresAt: chain.expiresAt,
      invocationRef: chain.invocationRef ?? null,
      rootIssuerRef: chain.rootIssuerRef,
      subjectRef: chain.subjectRef,
    }))
    .digest("hex")
    .slice(0, 24)}`

function isStringArray(value: unknown): value is string[] {
  return Array.isArray(value) && value.every((entry) => typeof entry === "string")
}

export function pylonDelegationChainFrom(value: unknown): PylonDelegationChain | null {
  if (value === null || typeof value !== "object") return null
  const record = value as Record<string, unknown>
  if (record.schema !== "openagents.pylon.capability_delegation_chain.v0.1") return null
  if (
    typeof record.rootIssuerRef !== "string" ||
    typeof record.subjectRef !== "string" ||
    typeof record.audienceRef !== "string" ||
    typeof record.issuedAt !== "string" ||
    typeof record.expiresAt !== "string" ||
    !Array.isArray(record.capabilities)
  ) {
    return null
  }

  const capabilities = record.capabilities.flatMap((entry): PylonDelegatedCapability[] => {
    if (entry === null || typeof entry !== "object") return []
    const cap = entry as Record<string, unknown>
    return typeof cap.capabilityRef === "string" &&
      typeof cap.action === "string" &&
      typeof cap.resourceRef === "string"
      ? [{ capabilityRef: cap.capabilityRef, action: cap.action, resourceRef: cap.resourceRef }]
      : []
  })
  if (capabilities.length !== record.capabilities.length) return null

  const attenuation =
    record.attenuation !== null && typeof record.attenuation === "object"
      ? record.attenuation as Record<string, unknown>
      : undefined
  const revocation =
    record.revocation !== null && typeof record.revocation === "object"
      ? record.revocation as Record<string, unknown>
      : undefined

  return {
    schema: "openagents.pylon.capability_delegation_chain.v0.1",
    rootIssuerRef: record.rootIssuerRef,
    subjectRef: record.subjectRef,
    audienceRef: record.audienceRef,
    issuedAt: record.issuedAt,
    expiresAt: record.expiresAt,
    ...(typeof record.invocationRef === "string" ? { invocationRef: record.invocationRef } : {}),
    capabilities,
    ...(attenuation === undefined
      ? {}
      : {
          attenuation: {
            ...(typeof attenuation.maxTtlSeconds === "number"
              ? { maxTtlSeconds: attenuation.maxTtlSeconds }
              : {}),
            ...(isStringArray(attenuation.allowedCapabilityRefs)
              ? { allowedCapabilityRefs: attenuation.allowedCapabilityRefs }
              : {}),
            ...(isStringArray(attenuation.allowedActions)
              ? { allowedActions: attenuation.allowedActions }
              : {}),
            ...(isStringArray(attenuation.allowedResourceRefs)
              ? { allowedResourceRefs: attenuation.allowedResourceRefs }
              : {}),
            ...(typeof attenuation.denyNetwork === "boolean"
              ? { denyNetwork: attenuation.denyNetwork }
              : {}),
            ...(typeof attenuation.requirePromptInjectionScreen === "boolean"
              ? { requirePromptInjectionScreen: attenuation.requirePromptInjectionScreen }
              : {}),
          },
        }),
    ...(revocation === undefined
      ? {}
      : {
          revocation: {
            ...(isStringArray(revocation.revokedRefs) ? { revokedRefs: revocation.revokedRefs } : {}),
            ...(typeof revocation.revokedAt === "string" ? { revokedAt: revocation.revokedAt } : {}),
            ...(typeof revocation.reasonRef === "string" ? { reasonRef: revocation.reasonRef } : {}),
          },
        }),
    ...(isStringArray(record.proofRefs) ? { proofRefs: record.proofRefs } : {}),
  }
}

export function admitPylonDelegation(input: PylonDelegationAdmissionInput): PylonDelegationAdmission {
  const { chain } = input
  const blockerRefs = new Set<string>()
  const delegationRef = stableDelegationRef(chain)
  const issuedAt = parseTime(chain.issuedAt)
  const expiresAt = parseTime(chain.expiresAt)
  const now = input.now.getTime()

  if (!validRef(chain.rootIssuerRef) || !validRef(chain.subjectRef) || !validRef(chain.audienceRef)) {
    blockerRefs.add("blocker.delegation.invalid_ref")
  }
  if (chain.audienceRef !== input.localPylonRef) {
    blockerRefs.add("blocker.delegation.wrong_audience")
  }
  if (!Number.isFinite(issuedAt) || !Number.isFinite(expiresAt) || expiresAt <= issuedAt) {
    blockerRefs.add("blocker.delegation.invalid_time_bounds")
  }
  if (Number.isFinite(expiresAt) && expiresAt <= now) {
    blockerRefs.add("blocker.delegation.expired")
  }
  if (Number.isFinite(issuedAt) && issuedAt - now > 60_000) {
    blockerRefs.add("blocker.delegation.not_yet_valid")
  }

  const attenuation = chain.attenuation
  if (
    attenuation?.maxTtlSeconds !== undefined &&
    Number.isFinite(issuedAt) &&
    Number.isFinite(expiresAt) &&
    expiresAt - issuedAt > attenuation.maxTtlSeconds * 1000
  ) {
    blockerRefs.add("blocker.delegation.ttl_exceeds_attenuation")
  }

  const localCapabilityRefs = new Set(input.localCapabilityRefs)
  const delegatedCapabilityRefs = new Set(chain.capabilities.map((cap) => cap.capabilityRef))
  for (const ref of input.requestedCapabilityRefs) {
    if (!delegatedCapabilityRefs.has(ref)) blockerRefs.add("blocker.delegation.capability_not_delegated")
    if (!localCapabilityRefs.has(ref)) blockerRefs.add("blocker.delegation.capability_not_local")
  }

  for (const cap of chain.capabilities) {
    if (!validRef(cap.capabilityRef) || !validRef(cap.resourceRef) || !allowedActionPattern.test(cap.action)) {
      blockerRefs.add("blocker.delegation.invalid_capability")
    }
  }

  const revokedRefs = new Set<string>()
  for (const ref of chain.revocation?.revokedRefs ?? []) revokedRefs.add(ref)
  for (const ref of [
    delegationRef,
    chain.invocationRef,
    chain.subjectRef,
    chain.rootIssuerRef,
    ...chain.capabilities.map((cap) => cap.capabilityRef),
  ]) {
    if (ref && revokedRefs.has(ref)) blockerRefs.add("blocker.delegation.revoked")
  }
  if (chain.revocation?.revokedAt !== undefined && parseTime(chain.revocation.revokedAt) <= now) {
    blockerRefs.add("blocker.delegation.revoked")
  }

  const allowedCapabilityRefs = attenuation?.allowedCapabilityRefs
  if (allowedCapabilityRefs !== undefined) {
    const allowed = new Set(allowedCapabilityRefs)
    for (const ref of unique(chain.capabilities.map((cap) => cap.capabilityRef))) {
      if (!allowed.has(ref)) blockerRefs.add("blocker.delegation.attenuation_capability")
    }
  }
  const allowedActions = attenuation?.allowedActions
  if (allowedActions !== undefined) {
    const allowed = new Set(allowedActions)
    for (const action of unique(chain.capabilities.map((cap) => cap.action))) {
      if (!allowed.has(action)) blockerRefs.add("blocker.delegation.attenuation_action")
    }
  }
  const allowedResourceRefs = attenuation?.allowedResourceRefs
  if (allowedResourceRefs !== undefined) {
    const allowed = new Set(allowedResourceRefs)
    for (const ref of unique(chain.capabilities.map((cap) => cap.resourceRef))) {
      if (!allowed.has(ref)) blockerRefs.add("blocker.delegation.attenuation_resource")
    }
  }

  if (
    attenuation?.requirePromptInjectionScreen === true &&
    input.objectiveText !== undefined &&
    promptInjectionRiskPattern.test(input.objectiveText)
  ) {
    blockerRefs.add("blocker.delegation.prompt_injection_risk")
  }

  return { admitted: blockerRefs.size === 0, delegationRef, blockerRefs: [...blockerRefs].sort() }
}
