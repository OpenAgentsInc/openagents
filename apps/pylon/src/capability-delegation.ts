import { createHash, createHmac, timingSafeEqual } from "node:crypto"

export const PYLON_CAPABILITY_DELEGATION_SCHEMA =
  "openagents.pylon.capability_delegation.v0.1" as const

export type PylonCapabilityDelegationSchema =
  typeof PYLON_CAPABILITY_DELEGATION_SCHEMA

export type PylonCapabilityDelegation = {
  readonly schema: PylonCapabilityDelegationSchema
  readonly audienceRef: string
  readonly issuerRef: string
  readonly subjectRef: string
  readonly keyRef: string
  readonly tokenRef: string
  readonly issuedAt: string
  readonly expiresAt: string
  readonly parentRef?: string
  readonly rootRef: string
  readonly actionRefs: ReadonlyArray<string>
  readonly scopeRefs: ReadonlyArray<string>
  readonly caveatRefs: ReadonlyArray<string>
  readonly signature: string
}

export type PylonCapabilityDelegationDraft = {
  readonly audienceRef: string
  readonly issuerRef: string
  readonly subjectRef: string
  readonly keyRef: string
  readonly issuedAt: string
  readonly expiresAt: string
  readonly actionRefs: ReadonlyArray<string>
  readonly scopeRefs: ReadonlyArray<string>
  readonly caveatRefs?: ReadonlyArray<string>
}

export type PylonCapabilityDelegationVerifyInput = {
  readonly token: PylonCapabilityDelegation
  readonly ancestors?: ReadonlyArray<PylonCapabilityDelegation>
  readonly actionRef: string
  readonly scopeRef: string
  readonly audienceRef: string
  readonly now?: Date
  readonly revokedTokenRefs?: ReadonlySet<string>
  readonly signingKeys: ReadonlyMap<string, string>
}

export type PylonCapabilityDelegationVerification =
  | {
      readonly ok: true
      readonly tokenRef: string
      readonly rootRef: string
      readonly acceptedRefs: ReadonlyArray<string>
    }
  | {
      readonly ok: false
      readonly tokenRef: string
      readonly blockerRefs: ReadonlyArray<string>
    }

type SignableDelegation = Omit<PylonCapabilityDelegation, "signature" | "tokenRef">

const uniqueSorted = (values: ReadonlyArray<string>): ReadonlyArray<string> =>
  [...new Set(values)].sort((left, right) => left.localeCompare(right))

const isRecord = (value: unknown): value is Record<string, unknown> =>
  typeof value === "object" && value !== null && !Array.isArray(value)

const canonicalJson = (value: unknown): string => {
  if (Array.isArray(value)) {
    return `[${value.map(canonicalJson).join(",")}]`
  }

  if (isRecord(value)) {
    return `{${Object.keys(value)
      .sort((left, right) => left.localeCompare(right))
      .map((key) => `${JSON.stringify(key)}:${canonicalJson(value[key])}`)
      .join(",")}}`
  }

  return JSON.stringify(value)
}

const sha256 = (input: string): string =>
  createHash("sha256").update(input).digest("hex")

const signDelegation = (
  payload: SignableDelegation,
  signingKey: string,
): string =>
  createHmac("sha256", signingKey).update(canonicalJson(payload)).digest("hex")

const verifySignature = (
  payload: SignableDelegation,
  signature: string,
  signingKey: string,
): boolean => {
  const expected = Buffer.from(signDelegation(payload, signingKey), "hex")
  const actual = Buffer.from(signature, "hex")
  return expected.length === actual.length && timingSafeEqual(expected, actual)
}

const tokenRefFor = (payload: SignableDelegation, signature: string): string =>
  `capability.pylon.delegation.${sha256(`${canonicalJson(payload)}.${signature}`).slice(0, 32)}`

const rootRefForDraft = (draft: PylonCapabilityDelegationDraft): string =>
  `capability.pylon.delegation_root.${sha256(
    canonicalJson({
      audienceRef: draft.audienceRef,
      issuerRef: draft.issuerRef,
      subjectRef: draft.subjectRef,
      keyRef: draft.keyRef,
      issuedAt: draft.issuedAt,
      expiresAt: draft.expiresAt,
      actionRefs: uniqueSorted(draft.actionRefs),
      scopeRefs: uniqueSorted(draft.scopeRefs),
      caveatRefs: uniqueSorted(draft.caveatRefs ?? []),
    }),
  ).slice(0, 32)}`

const signablePayload = (
  token: PylonCapabilityDelegation,
): SignableDelegation => ({
  schema: token.schema,
  audienceRef: token.audienceRef,
  issuerRef: token.issuerRef,
  subjectRef: token.subjectRef,
  keyRef: token.keyRef,
  issuedAt: token.issuedAt,
  expiresAt: token.expiresAt,
  ...(token.parentRef === undefined ? {} : { parentRef: token.parentRef }),
  rootRef: token.rootRef,
  actionRefs: token.actionRefs,
  scopeRefs: token.scopeRefs,
  caveatRefs: token.caveatRefs,
})

export const issuePylonCapabilityDelegation = (
  draft: PylonCapabilityDelegationDraft,
  signingKey: string,
): PylonCapabilityDelegation => {
  const payload: SignableDelegation = {
    schema: PYLON_CAPABILITY_DELEGATION_SCHEMA,
    audienceRef: draft.audienceRef,
    issuerRef: draft.issuerRef,
    subjectRef: draft.subjectRef,
    keyRef: draft.keyRef,
    issuedAt: draft.issuedAt,
    expiresAt: draft.expiresAt,
    rootRef: rootRefForDraft(draft),
    actionRefs: uniqueSorted(draft.actionRefs),
    scopeRefs: uniqueSorted(draft.scopeRefs),
    caveatRefs: uniqueSorted(draft.caveatRefs ?? []),
  }

  const signature = signDelegation(payload, signingKey)
  const tokenRef = tokenRefFor(payload, signature)

  return {
    ...payload,
    tokenRef,
    signature,
  }
}

const isSubset = (
  child: ReadonlyArray<string>,
  parent: ReadonlyArray<string>,
): boolean => child.every((value) => parent.includes(value))

export const attenuatePylonCapabilityDelegation = (
  parent: PylonCapabilityDelegation,
  draft: Omit<PylonCapabilityDelegationDraft, "issuerRef" | "keyRef"> &
    Pick<PylonCapabilityDelegationDraft, "keyRef">,
  signingKey: string,
): PylonCapabilityDelegation => {
  const actionRefs = uniqueSorted(draft.actionRefs)
  const scopeRefs = uniqueSorted(draft.scopeRefs)
  const caveatRefs = uniqueSorted([
    ...parent.caveatRefs,
    ...(draft.caveatRefs ?? []),
  ])
  const parentExpiry = Date.parse(parent.expiresAt)
  const childExpiry = Date.parse(draft.expiresAt)

  if (!isSubset(actionRefs, parent.actionRefs)) {
    throw new Error("capability attenuation cannot add action refs")
  }

  if (!isSubset(scopeRefs, parent.scopeRefs)) {
    throw new Error("capability attenuation cannot add scope refs")
  }

  if (!Number.isFinite(childExpiry) || childExpiry > parentExpiry) {
    throw new Error("capability attenuation cannot outlive parent")
  }

  const payload: SignableDelegation = {
    schema: PYLON_CAPABILITY_DELEGATION_SCHEMA,
    audienceRef: draft.audienceRef,
    issuerRef: parent.subjectRef,
    subjectRef: draft.subjectRef,
    keyRef: draft.keyRef,
    issuedAt: draft.issuedAt,
    expiresAt: draft.expiresAt,
    parentRef: parent.tokenRef,
    rootRef: parent.rootRef,
    actionRefs,
    scopeRefs,
    caveatRefs,
  }
  const signature = signDelegation(payload, signingKey)

  return {
    ...payload,
    tokenRef: tokenRefFor(payload, signature),
    signature,
  }
}

const validateTokenShape = (
  token: PylonCapabilityDelegation,
): ReadonlyArray<string> => {
  const blockers: string[] = []
  if (token.schema !== PYLON_CAPABILITY_DELEGATION_SCHEMA) {
    blockers.push("blocker.pylon.capability_delegation.schema")
  }
  if (token.actionRefs.length === 0) {
    blockers.push("blocker.pylon.capability_delegation.empty_actions")
  }
  if (token.scopeRefs.length === 0) {
    blockers.push("blocker.pylon.capability_delegation.empty_scopes")
  }
  return blockers
}

export const verifyPylonCapabilityDelegation = (
  input: PylonCapabilityDelegationVerifyInput,
): PylonCapabilityDelegationVerification => {
  const blockers: string[] = []
  const now = input.now ?? new Date()
  const chain = [...(input.ancestors ?? []), input.token]
  const tokenByRef = new Map(chain.map((token) => [token.tokenRef, token]))

  for (const token of chain) {
    blockers.push(...validateTokenShape(token))

    const signingKey = input.signingKeys.get(token.keyRef)
    if (signingKey === undefined) {
      blockers.push("blocker.pylon.capability_delegation.unknown_key")
      continue
    }

    const payload = signablePayload(token)
    if (!verifySignature(payload, token.signature, signingKey)) {
      blockers.push("blocker.pylon.capability_delegation.bad_signature")
    }

    if (tokenRefFor(payload, token.signature) !== token.tokenRef) {
      blockers.push("blocker.pylon.capability_delegation.bad_token_ref")
    }

    if (input.revokedTokenRefs?.has(token.tokenRef) === true) {
      blockers.push("blocker.pylon.capability_delegation.revoked")
    }

    if (Date.parse(token.expiresAt) <= now.getTime()) {
      blockers.push("blocker.pylon.capability_delegation.expired")
    }
  }

  const token = input.token
  if (token.audienceRef !== input.audienceRef) {
    blockers.push("blocker.pylon.capability_delegation.wrong_audience")
  }

  if (!token.actionRefs.includes(input.actionRef)) {
    blockers.push("blocker.pylon.capability_delegation.action_denied")
  }

  if (!token.scopeRefs.includes(input.scopeRef)) {
    blockers.push("blocker.pylon.capability_delegation.scope_denied")
  }

  let cursor = token
  while (cursor.parentRef !== undefined) {
    const parent = tokenByRef.get(cursor.parentRef)
    if (parent === undefined) {
      blockers.push("blocker.pylon.capability_delegation.missing_parent")
      break
    }

    if (cursor.rootRef !== parent.rootRef) {
      blockers.push("blocker.pylon.capability_delegation.root_mismatch")
    }

    if (!isSubset(cursor.actionRefs, parent.actionRefs)) {
      blockers.push("blocker.pylon.capability_delegation.action_expansion")
    }

    if (!isSubset(cursor.scopeRefs, parent.scopeRefs)) {
      blockers.push("blocker.pylon.capability_delegation.scope_expansion")
    }

    if (Date.parse(cursor.expiresAt) > Date.parse(parent.expiresAt)) {
      blockers.push("blocker.pylon.capability_delegation.expiry_expansion")
    }

    cursor = parent
  }

  if (blockers.length > 0) {
    return {
      ok: false,
      tokenRef: token.tokenRef,
      blockerRefs: uniqueSorted(blockers),
    }
  }

  return {
    ok: true,
    tokenRef: token.tokenRef,
    rootRef: token.rootRef,
    acceptedRefs: [
      `accepted.${input.actionRef}`,
      `accepted.${input.scopeRef}`,
      `accepted.${input.audienceRef}`,
    ],
  }
}
