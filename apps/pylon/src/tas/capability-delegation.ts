import { createHmac, timingSafeEqual } from "node:crypto"

export type DelegationCapability = string
export type DelegationScope = string

export type DelegationCaveat =
  | {
      readonly kind: "expires_before"
      readonly at: string
    }
  | {
      readonly kind: "source_trust"
      readonly trust: "trusted_control" | "untrusted_data"
    }
  | {
      readonly kind: "effect"
      readonly effect: "read_only" | "effectful"
    }
  | {
      readonly kind: "tool_ref"
      readonly toolRef: string
    }

export type CapabilityDelegationEnvelope = {
  readonly schema: "openagents.pylon.capability_delegation.v1"
  readonly issuerRef: string
  readonly subjectRef: string
  readonly audienceRef: string
  readonly capabilityRefs: readonly DelegationCapability[]
  readonly scopeRefs: readonly DelegationScope[]
  readonly caveats: readonly DelegationCaveat[]
  readonly issuedAt: string
  readonly expiresAt: string
  readonly nonceRef: string
  readonly parentSignature?: string
}

export type CapabilityDelegationToken = {
  readonly envelope: CapabilityDelegationEnvelope
  readonly signature: string
}

export type CapabilityToolRequest = {
  readonly audienceRef: string
  readonly capabilityRef: string
  readonly scopeRef: string
  readonly toolRef: string
  readonly effect: "read_only" | "effectful"
  readonly sourceTrust: "trusted_control" | "untrusted_data"
}

export type CapabilityDelegationCheck =
  | {
      readonly ok: true
      readonly evidenceRefs: readonly string[]
    }
  | {
      readonly ok: false
      readonly blockerRefs: readonly string[]
    }

const schema = "openagents.pylon.capability_delegation.v1" as const
const hmacPrefix = "openagents.pylon.capability_delegation.hmac.v1"
const refPattern = /^[A-Za-z0-9][A-Za-z0-9_.:/-]{2,180}$/

export function mintCapabilityDelegation(input: {
  readonly issuerRef: string
  readonly subjectRef: string
  readonly audienceRef: string
  readonly capabilityRefs: readonly DelegationCapability[]
  readonly scopeRefs: readonly DelegationScope[]
  readonly caveats?: readonly DelegationCaveat[]
  readonly issuedAt: string
  readonly expiresAt: string
  readonly nonceRef: string
  readonly issuerSecret: string
}): CapabilityDelegationToken {
  const envelope = normalizeEnvelope({
    schema,
    issuerRef: input.issuerRef,
    subjectRef: input.subjectRef,
    audienceRef: input.audienceRef,
    capabilityRefs: input.capabilityRefs,
    scopeRefs: input.scopeRefs,
    caveats: input.caveats ?? [],
    issuedAt: input.issuedAt,
    expiresAt: input.expiresAt,
    nonceRef: input.nonceRef,
  })

  assertEnvelopeShape(envelope)

  return {
    envelope,
    signature: signRoot(envelope, input.issuerSecret),
  }
}

export function attenuateCapabilityDelegation(input: {
  readonly parent: CapabilityDelegationToken
  readonly subjectRef: string
  readonly capabilityRefs?: readonly DelegationCapability[]
  readonly scopeRefs?: readonly DelegationScope[]
  readonly caveats?: readonly DelegationCaveat[]
  readonly issuedAt: string
  readonly expiresAt: string
  readonly nonceRef: string
}): CapabilityDelegationToken {
  const parent = input.parent.envelope
  const envelope = normalizeEnvelope({
    schema,
    issuerRef: parent.subjectRef,
    subjectRef: input.subjectRef,
    audienceRef: parent.audienceRef,
    capabilityRefs: input.capabilityRefs ?? parent.capabilityRefs,
    scopeRefs: input.scopeRefs ?? parent.scopeRefs,
    caveats: [...parent.caveats, ...(input.caveats ?? [])],
    issuedAt: input.issuedAt,
    expiresAt: input.expiresAt,
    nonceRef: input.nonceRef,
    parentSignature: input.parent.signature,
  })

  assertEnvelopeShape(envelope)
  assertSubset(envelope.capabilityRefs, parent.capabilityRefs, "capability")
  assertSubset(envelope.scopeRefs, parent.scopeRefs, "scope")
  if (Date.parse(envelope.expiresAt) > Date.parse(parent.expiresAt)) {
    throw new Error("attenuated delegation cannot outlive parent")
  }

  return {
    envelope,
    signature: signAttenuation(envelope, input.parent.signature),
  }
}

export function authorizeCapabilityToolRequest(input: {
  readonly chain: readonly CapabilityDelegationToken[]
  readonly issuerSecret: string
  readonly request: CapabilityToolRequest
  readonly now: string
  readonly revokedSignatureRefs?: ReadonlySet<string>
}): CapabilityDelegationCheck {
  if (input.chain.length === 0) {
    return blocked("blocker.pylon.capability_delegation.chain_missing")
  }

  const chainCheck = verifyDelegationChain(input)
  if (!chainCheck.ok) return chainCheck

  const leaf = input.chain[input.chain.length - 1]
  const blockers = [
    ...authorizeLeafEnvelope(leaf.envelope, input.request),
    ...authorizeCaveats(leaf.envelope.caveats, input.request, input.now),
  ]

  if (blockers.length > 0) return { ok: false, blockerRefs: unique(blockers) }

  return {
    ok: true,
    evidenceRefs: [
      "receipt.pylon.capability_delegation.chain_verified",
      signatureRef(leaf.signature),
    ],
  }
}

export function signatureRef(signature: string): string {
  return `capability_delegation.signature.${signature.slice(0, 24)}`
}

function verifyDelegationChain(input: {
  readonly chain: readonly CapabilityDelegationToken[]
  readonly issuerSecret: string
  readonly now: string
  readonly revokedSignatureRefs?: ReadonlySet<string>
}): CapabilityDelegationCheck {
  const blockers: string[] = []
  let parent: CapabilityDelegationToken | null = null

  for (const token of input.chain) {
    const envelope = normalizeEnvelope(token.envelope)
    try {
      assertEnvelopeShape(envelope)
    } catch {
      blockers.push("blocker.pylon.capability_delegation.malformed")
      continue
    }

    const expected = parent === null
      ? signRoot(envelope, input.issuerSecret)
      : signAttenuation(envelope, parent.signature)

    if (!constantTimeEqual(token.signature, expected)) {
      blockers.push("blocker.pylon.capability_delegation.bad_signature")
    }

    if (parent !== null) {
      if (envelope.parentSignature !== parent.signature) {
        blockers.push("blocker.pylon.capability_delegation.parent_mismatch")
      }
      if (envelope.issuerRef !== parent.envelope.subjectRef) {
        blockers.push("blocker.pylon.capability_delegation.issuer_not_parent_subject")
      }
      if (Date.parse(envelope.expiresAt) > Date.parse(parent.envelope.expiresAt)) {
        blockers.push("blocker.pylon.capability_delegation.expands_expiry")
      }
      if (!isSubset(envelope.capabilityRefs, parent.envelope.capabilityRefs)) {
        blockers.push("blocker.pylon.capability_delegation.expands_capability")
      }
      if (!isSubset(envelope.scopeRefs, parent.envelope.scopeRefs)) {
        blockers.push("blocker.pylon.capability_delegation.expands_scope")
      }
    }

    if (Date.parse(input.now) > Date.parse(envelope.expiresAt)) {
      blockers.push("blocker.pylon.capability_delegation.expired")
    }

    if (input.revokedSignatureRefs?.has(signatureRef(token.signature))) {
      blockers.push("blocker.pylon.capability_delegation.revoked")
    }

    parent = token
  }

  return blockers.length === 0
    ? { ok: true, evidenceRefs: ["receipt.pylon.capability_delegation.chain_verified"] }
    : { ok: false, blockerRefs: unique(blockers) }
}

function authorizeLeafEnvelope(
  envelope: CapabilityDelegationEnvelope,
  request: CapabilityToolRequest,
): string[] {
  const blockers: string[] = []

  if (envelope.audienceRef !== request.audienceRef) {
    blockers.push("blocker.pylon.capability_delegation.audience_mismatch")
  }
  if (!envelope.capabilityRefs.includes(request.capabilityRef)) {
    blockers.push("blocker.pylon.capability_delegation.capability_missing")
  }
  if (!envelope.scopeRefs.includes(request.scopeRef)) {
    blockers.push("blocker.pylon.capability_delegation.scope_missing")
  }
  if (request.sourceTrust === "untrusted_data" && request.effect === "effectful") {
    blockers.push("blocker.pylon.capability_delegation.untrusted_effectful_tool")
  }

  return blockers
}

function authorizeCaveats(
  caveats: readonly DelegationCaveat[],
  request: CapabilityToolRequest,
  now: string,
): string[] {
  const blockers: string[] = []

  for (const caveat of caveats) {
    switch (caveat.kind) {
      case "expires_before":
        if (Date.parse(now) > Date.parse(caveat.at)) {
          blockers.push("blocker.pylon.capability_delegation.caveat_expired")
        }
        break
      case "source_trust":
        if (request.sourceTrust !== caveat.trust) {
          blockers.push("blocker.pylon.capability_delegation.source_trust_caveat")
        }
        break
      case "effect":
        if (request.effect !== caveat.effect) {
          blockers.push("blocker.pylon.capability_delegation.effect_caveat")
        }
        break
      case "tool_ref":
        if (request.toolRef !== caveat.toolRef) {
          blockers.push("blocker.pylon.capability_delegation.tool_caveat")
        }
        break
    }
  }

  return blockers
}

function normalizeEnvelope(
  envelope: CapabilityDelegationEnvelope,
): CapabilityDelegationEnvelope {
  return {
    ...envelope,
    capabilityRefs: unique([...envelope.capabilityRefs]).sort(),
    scopeRefs: unique([...envelope.scopeRefs]).sort(),
    caveats: [...envelope.caveats].sort((a, b) =>
      canonicalJson(a).localeCompare(canonicalJson(b)),
    ),
  }
}

function assertEnvelopeShape(envelope: CapabilityDelegationEnvelope): void {
  for (const value of [
    envelope.issuerRef,
    envelope.subjectRef,
    envelope.audienceRef,
    envelope.nonceRef,
  ]) {
    if (!refPattern.test(value)) throw new Error("delegation refs must be public-safe")
  }
  if (envelope.capabilityRefs.length === 0 || envelope.scopeRefs.length === 0) {
    throw new Error("delegation requires at least one capability and scope")
  }
  for (const value of [...envelope.capabilityRefs, ...envelope.scopeRefs]) {
    if (!refPattern.test(value)) throw new Error("delegation refs must be public-safe")
  }
  if (Number.isNaN(Date.parse(envelope.issuedAt)) || Number.isNaN(Date.parse(envelope.expiresAt))) {
    throw new Error("delegation timestamps must be valid ISO timestamps")
  }
  if (Date.parse(envelope.expiresAt) <= Date.parse(envelope.issuedAt)) {
    throw new Error("delegation expiresAt must be after issuedAt")
  }
}

function assertSubset(
  candidate: readonly string[],
  allowed: readonly string[],
  label: string,
): void {
  if (!isSubset(candidate, allowed)) {
    throw new Error(`attenuated delegation cannot expand ${label}`)
  }
}

function isSubset(candidate: readonly string[], allowed: readonly string[]): boolean {
  return candidate.every((value) => allowed.includes(value))
}

function signRoot(envelope: CapabilityDelegationEnvelope, issuerSecret: string): string {
  return hmac(issuerSecret, canonicalJson(envelope))
}

function signAttenuation(envelope: CapabilityDelegationEnvelope, parentSignature: string): string {
  return hmac(parentSignature, canonicalJson(envelope))
}

function hmac(secret: string, value: string): string {
  return createHmac("sha256", `${hmacPrefix}:${secret}`).update(value).digest("hex")
}

function canonicalJson(value: unknown): string {
  if (Array.isArray(value)) return `[${value.map(canonicalJson).join(",")}]`
  if (value === null || typeof value !== "object") return JSON.stringify(value)
  const record = value as Record<string, unknown>
  return `{${Object.keys(record)
    .sort()
    .filter((key) => record[key] !== undefined)
    .map((key) => `${JSON.stringify(key)}:${canonicalJson(record[key])}`)
    .join(",")}}`
}

function constantTimeEqual(left: string, right: string): boolean {
  const leftBuffer = Buffer.from(left)
  const rightBuffer = Buffer.from(right)
  return leftBuffer.length === rightBuffer.length && timingSafeEqual(leftBuffer, rightBuffer)
}

function blocked(blockerRef: string): CapabilityDelegationCheck {
  return { ok: false, blockerRefs: [blockerRef] }
}

function unique<T>(values: readonly T[]): T[] {
  return [...new Set(values)]
}
