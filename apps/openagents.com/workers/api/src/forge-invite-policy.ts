import { verifyAgentOwnerAttestation } from '@openagentsinc/sarah/community'
import { Schema as S } from 'effect'
import * as nip19 from 'nostr-effect/nip19'
import {
  hashPayloadBytes,
  unpackEventFromToken,
  validateEventMethodTag,
  validateEventPayloadTag,
  validateEventUrlTag,
  verifyHttpAuthEvent,
} from 'nostr-effect/nip98'

import { sha256Hex } from './agent-registration'
import { readBearerToken } from './auth/bearer-token'
import {
  FORGE_GIT_TOKEN_PREFIX,
  type ForgeTenantGitAuthStore,
} from './forge-tenant-git-auth-store'

const HexPubkey = S.String.check(S.isPattern(/^[0-9a-f]{64}$/))
const IsoTimestamp = S.String.check(S.isMinLength(20), S.isMaxLength(64))
const PublicRef = S.String.check(S.isMinLength(1), S.isMaxLength(512))

export const ForgeActorKind = S.Literals(['human', 'agent'])
export type ForgeActorKind = typeof ForgeActorKind.Type

export const ForgeMembershipState = S.Literals(['active', 'tombstoned'])
export type ForgeMembershipState = typeof ForgeMembershipState.Type

export const ForgeActorBinding = S.Struct({
  bindingRef: PublicRef,
  tenantRef: PublicRef,
  accountRef: PublicRef,
  actorKind: ForgeActorKind,
  displayName: S.String.check(S.isMinLength(1), S.isMaxLength(200)),
  ownerBindingRef: S.NullOr(PublicRef),
  roleRefs: S.Array(PublicRef),
  membershipState: ForgeMembershipState,
  bindingGeneration: S.Number.check(S.isInt(), S.isGreaterThan(0)),
  createdAt: IsoTimestamp,
  revokedAt: S.NullOr(IsoTimestamp),
  nostrPubkey: S.NullOr(HexPubkey),
  nostrBindingEventId: S.NullOr(PublicRef),
  nostrBindingCreatedAt: S.NullOr(IsoTimestamp),
  nostrBindingSignatureValid: S.Boolean,
})
export interface ForgeActorBinding extends S.Schema.Type<
  typeof ForgeActorBinding
> {}

export const ForgeInviteBinding = S.Struct({
  inviteBindingRef: PublicRef,
  tenantRef: PublicRef,
  teamRef: PublicRef,
  inviteRef: PublicRef,
  inviteDigest: HexPubkey,
  inviteKind: S.Literals(['team_workspace']),
  inviterBindingRef: PublicRef,
  invitedSubjectRef: PublicRef,
  roleRefs: S.Array(PublicRef),
  issuedAt: IsoTimestamp,
  expiresAt: IsoTimestamp,
  acceptedAt: S.NullOr(IsoTimestamp),
  acceptedBindingRef: S.NullOr(PublicRef),
  provenanceSourceRefs: S.Array(PublicRef),
})
export interface ForgeInviteBinding extends S.Schema.Type<
  typeof ForgeInviteBinding
> {}

export const ForgeBurnedKeyFact = S.Struct({
  burnedKeyFactRef: PublicRef,
  tenantRef: PublicRef,
  keyKind: ForgeActorKind,
  publicKey: HexPubkey,
  bindingRef: PublicRef,
  burnReasonRef: PublicRef,
  burnedAt: IsoTimestamp,
  burnSequence: S.Number.check(S.isInt(), S.isGreaterThan(0)),
  sourceRefs: S.Array(PublicRef),
})
export interface ForgeBurnedKeyFact extends S.Schema.Type<
  typeof ForgeBurnedKeyFact
> {}

export const ForgeNip98ReplayConsumption = S.Struct({
  consumptionRef: PublicRef,
  tenantRef: PublicRef,
  requestDigest: HexPubkey,
  eventId: HexPubkey,
  actorPubkey: HexPubkey,
  httpMethod: S.String.check(S.isMinLength(1), S.isMaxLength(16)),
  canonicalPath: S.String.check(S.isMinLength(1), S.isMaxLength(2_048)),
  bodyDigest: HexPubkey,
  eventCreatedAt: IsoTimestamp,
  consumedAt: IsoTimestamp,
  expiresAt: IsoTimestamp,
  authorityGeneration: S.Number.check(S.isInt(), S.isGreaterThan(0)),
  result: S.Literals(['accepted', 'refused']),
})
export interface ForgeNip98ReplayConsumption extends S.Schema.Type<
  typeof ForgeNip98ReplayConsumption
> {}

export const ForgeMembershipReconciliationState = S.Struct({
  reconciliationRef: PublicRef,
  tenantRef: PublicRef,
  teamRef: PublicRef,
  bindingRef: PublicRef,
  sourceMembershipGeneration: S.Number.check(
    S.isInt(),
    S.isGreaterThanOrEqualTo(0),
  ),
  reconciliationGeneration: S.Number.check(S.isInt(), S.isGreaterThan(0)),
  observedPresent: S.Boolean,
  absenceFirstObservedAt: S.NullOr(IsoTimestamp),
  absenceConfirmedAt: S.NullOr(IsoTimestamp),
  hysteresisDeadline: S.NullOr(IsoTimestamp),
  state: S.Literals(['present', 'absence_pending', 'absence_confirmed']),
  reconciledAt: IsoTimestamp,
  sourceRefs: S.Array(PublicRef),
})
export interface ForgeMembershipReconciliationState extends S.Schema.Type<
  typeof ForgeMembershipReconciliationState
> {}

export const ForgeInvitePolicyErrorCode = S.Literals([
  'credential_missing',
  'credential_invalid',
  'membership_required',
  'membership_tombstoned',
  'scope_forbidden',
  'nip98_replayed',
  'npub_invalid',
  'binding_conflict',
  'key_burned',
  'owner_membership_required',
  'owner_attestation_invalid',
])
export type ForgeInvitePolicyErrorCode = typeof ForgeInvitePolicyErrorCode.Type

export class ForgeInvitePolicyError extends S.TaggedErrorClass<ForgeInvitePolicyError>()(
  'ForgeInvitePolicyError',
  {
    code: ForgeInvitePolicyErrorCode,
    reason: S.String,
  },
) {}

export type ForgeInvitePolicyStore = Readonly<{
  readActorBindingByRef: (
    tenantRef: string,
    bindingRef: string,
  ) => Promise<ForgeActorBinding | undefined>
  readActorBindingByNostrPubkey: (
    tenantRef: string,
    nostrPubkey: string,
  ) => Promise<ForgeActorBinding | undefined>
  consumeNip98Replay: (
    consumption: ForgeNip98ReplayConsumption,
  ) => Promise<boolean>
}>

export type ForgeGitTransportSession = Readonly<{
  credentialMode: 'nip98' | 'scoped_token'
  tenantRef: string
  bindingRef: string
  subjectRef: string
  repositoryRef: string
  scopes: ReadonlyArray<'git:upload-pack' | 'git:receive-pack' | 'git:admin'>
  refRestrictions: ReadonlyArray<string>
  tokenRef: string
  authenticatedAt: string
}>

const invalid = (
  code: ForgeInvitePolicyErrorCode,
  reason: string,
): ForgeInvitePolicyError => new ForgeInvitePolicyError({ code, reason })

export const decodeForgeNpub = (value: string): string => {
  try {
    const decoded = nip19.decode(value.trim())
    if (
      decoded.type !== 'npub' ||
      typeof decoded.data !== 'string' ||
      nip19.npubEncode(decoded.data) !== value.trim()
    ) {
      throw invalid(
        'npub_invalid',
        'The Nostr identity must be one canonical npub.',
      )
    }
    return decoded.data.toLowerCase()
  } catch (error) {
    if (error instanceof ForgeInvitePolicyError) {
      throw error
    }
    throw invalid('npub_invalid', 'The Nostr identity must be one valid npub.')
  }
}

export const verifyForgeOwnerAttestation = (
  input: Readonly<{
    agentPubkey: string
    ownerPubkey: string
    ownerAuthTag: ReadonlyArray<string>
  }>,
): void => {
  try {
    verifyAgentOwnerAttestation({
      agentPubkey: input.agentPubkey,
      expectedOperatorPubkey: input.ownerPubkey,
      ownerAuthTag: input.ownerAuthTag,
    })
  } catch {
    throw invalid(
      'owner_attestation_invalid',
      'The agent owner attestation is not valid for the invited owner.',
    )
  }
}

const roleAllowsScope = (
  roleRefs: ReadonlyArray<string>,
  requiredScope: 'git:upload-pack' | 'git:receive-pack',
): boolean => {
  if (roleRefs.includes('forge:admin') || roleRefs.includes('forge:member')) {
    return true
  }
  return (
    requiredScope === 'git:upload-pack' && roleRefs.includes('forge:viewer')
  )
}

const requireActiveBinding = (
  binding: ForgeActorBinding | undefined,
  requiredScope: 'git:upload-pack' | 'git:receive-pack',
): ForgeActorBinding => {
  if (binding === undefined) {
    throw invalid(
      'membership_required',
      'An active Forge invitation and actor binding are required.',
    )
  }
  if (binding.membershipState !== 'active') {
    throw invalid(
      'membership_tombstoned',
      'The Forge actor binding was revoked.',
    )
  }
  if (!roleAllowsScope(binding.roleRefs, requiredScope)) {
    throw invalid(
      'scope_forbidden',
      `The Forge membership does not allow ${requiredScope}.`,
    )
  }
  return binding
}

const isoFromEpochSeconds = (seconds: number): string =>
  new Date(seconds * 1_000).toISOString()

export type VerifiedForgeNip98Proof = Readonly<{
  actorPubkey: string
  bodyDigest: string
  eventCreatedAt: string
  eventId: string
  requestDigest: string
}>

export const verifyForgeNip98Proof = async (
  input: Readonly<{
    authorization: string
    body?: Uint8Array | undefined
    method: string
    nowIso: string
    url: string
  }>,
): Promise<VerifiedForgeNip98Proof> => {
  try {
    const event = await unpackEventFromToken(input.authorization)
    const nowSeconds = Math.floor(Date.parse(input.nowIso) / 1_000)
    const body = input.body

    if (
      event.content !== '' ||
      !verifyHttpAuthEvent(event) ||
      !validateEventUrlTag(event, input.url) ||
      !validateEventMethodTag(event, input.method) ||
      !Number.isInteger(event.created_at) ||
      Math.abs(nowSeconds - event.created_at) > 60 ||
      (body !== undefined && !validateEventPayloadTag(event, body))
    ) {
      throw new Error('invalid NIP-98 proof')
    }

    const bodyDigest = hashPayloadBytes(body ?? new Uint8Array())
    const requestDigest = await sha256Hex(
      `${event.id}\n${input.method.toUpperCase()}\n${input.url}\n${bodyDigest}`,
    )
    return {
      actorPubkey: event.pubkey.toLowerCase(),
      bodyDigest,
      eventCreatedAt: isoFromEpochSeconds(event.created_at),
      eventId: event.id.toLowerCase(),
      requestDigest,
    }
  } catch {
    throw invalid(
      'credential_invalid',
      'The NIP-98 transport proof is invalid or stale.',
    )
  }
}

const nip98Expiry = (eventCreatedAt: string): string =>
  new Date(Date.parse(eventCreatedAt) + 60_000).toISOString()

export const makeForgeInvitePolicyAuthority = (
  dependencies: Readonly<{
    policyStore: ForgeInvitePolicyStore
    tokenStore: ForgeTenantGitAuthStore
  }>,
) => ({
  authorizeGitTransport: async (
    input: Readonly<{
      body?: Uint8Array | undefined
      nowIso: string
      repositoryRef: string
      request: Request
      requiredScope: 'git:upload-pack' | 'git:receive-pack'
      tenantRef: string
    }>,
  ): Promise<ForgeGitTransportSession> => {
    const authorization = input.request.headers.get('authorization')
    if (authorization === null) {
      throw invalid(
        'credential_missing',
        'Forge Git transport authentication is required.',
      )
    }

    const bearer = readBearerToken(input.request)
    if (bearer?.startsWith(FORGE_GIT_TOKEN_PREFIX) === true) {
      const tokenSession =
        await dependencies.tokenStore.authenticateGitAccessToken({
          nowIso: input.nowIso,
          repositoryRef: input.repositoryRef,
          requiredScope: input.requiredScope,
          token: bearer,
        })
      if (
        tokenSession === undefined ||
        tokenSession.tenantRef !== input.tenantRef
      ) {
        throw invalid(
          'credential_invalid',
          'The scoped Forge Git credential is not valid for this target.',
        )
      }
      const binding = requireActiveBinding(
        await dependencies.policyStore.readActorBindingByRef(
          input.tenantRef,
          tokenSession.subjectRef,
        ),
        input.requiredScope,
      )
      return {
        authenticatedAt: input.nowIso,
        bindingRef: binding.bindingRef,
        credentialMode: 'scoped_token',
        refRestrictions: tokenSession.refRestrictions,
        repositoryRef: input.repositoryRef,
        scopes: tokenSession.scopes,
        subjectRef: binding.bindingRef,
        tenantRef: input.tenantRef,
        tokenRef: tokenSession.tokenRef,
      }
    }

    if (!authorization.startsWith('Nostr ')) {
      throw invalid(
        'credential_invalid',
        'Forge Git transport accepts NIP-98 or a scoped Forge Git credential.',
      )
    }

    const proof = await verifyForgeNip98Proof({
      authorization,
      body: input.body,
      method: input.request.method,
      nowIso: input.nowIso,
      url: input.request.url,
    })
    const binding = requireActiveBinding(
      await dependencies.policyStore.readActorBindingByNostrPubkey(
        input.tenantRef,
        proof.actorPubkey,
      ),
      input.requiredScope,
    )
    const consumed = await dependencies.policyStore.consumeNip98Replay({
      actorPubkey: proof.actorPubkey,
      authorityGeneration: binding.bindingGeneration,
      bodyDigest: proof.bodyDigest,
      canonicalPath: new URL(input.request.url).pathname,
      consumedAt: input.nowIso,
      consumptionRef: `forge_nip98_consumption.${proof.eventId}`,
      eventCreatedAt: proof.eventCreatedAt,
      eventId: proof.eventId,
      expiresAt: nip98Expiry(proof.eventCreatedAt),
      httpMethod: input.request.method.toUpperCase(),
      requestDigest: proof.requestDigest,
      result: 'accepted',
      tenantRef: input.tenantRef,
    })
    if (!consumed) {
      throw invalid('nip98_replayed', 'This NIP-98 event was already consumed.')
    }

    return {
      authenticatedAt: input.nowIso,
      bindingRef: binding.bindingRef,
      credentialMode: 'nip98',
      refRestrictions: [],
      repositoryRef: input.repositoryRef,
      scopes: [input.requiredScope],
      subjectRef: binding.bindingRef,
      tenantRef: input.tenantRef,
      tokenRef: `nip98:${proof.eventId}`,
    }
  },
})
