// Shared owner-identity resolution for the inference gateway free tier
// (EPIC #5474). Both the Sybil-resistant free pool (inference-free-allowance.ts)
// and the premium-model allowlist (inference-premium-allowlist.ts) key on the
// SAME owner identity, so the resolution + key-derivation lives here once.
//
// The gateway authenticates a request to an account ref shaped `agent:<userId>`
// (chat-completions-routes.ts). The VERIFIED OWNER identity behind that account
// is the approved/verified X owner claim — the exact surface the #5486
// light-KYC gate reads (`readVerifiedPublicIdentityForAgentUserId` on the
// `AgentOwnerClaimStore`). All accounts/autopilots under one verified owner
// resolve to the SAME owner key, so a swarm of autopilots cannot multiply a
// per-owner free pool or each independently clear a per-owner premium grant.
//
// An UNCLAIMED / unverified account has no owner claim, so it resolves to a
// synthetic `account:<accountRef>` key (taste-only free tier; never premium).

import { type VerifiedPublicIdentityClaim } from '../agent-owner-claim-routes'

// Resolves the verified owner identity for an authenticated account ref, or
// undefined when the account is unclaimed/unverified. The Worker wires this to
// the agent-owner-claim store; tests inject a fake. It accepts the account ref
// (`agent:<userId>`) and is responsible for extracting the agent user id and
// reading the owner-claim surface.
export type VerifiedOwnerIdentityResolver = (
  accountRef: string,
) => Promise<VerifiedPublicIdentityClaim | undefined>

// Bounded, structural parse of an `agent:<userId>` account ref into the agent
// user id (NOT intent routing — a fixed-shape prefix on an already-authenticated
// ref). Returns undefined for a non-agent-shaped ref.
const SAFE_ID_PATTERN = /^[A-Za-z0-9][A-Za-z0-9_.:-]{0,190}$/

export const agentUserIdFromAccountRef = (
  accountRef: string,
): string | undefined => {
  const trimmed = accountRef.trim()
  if (!trimmed.startsWith('agent:')) {
    return undefined
  }
  const userId = trimmed.slice('agent:'.length)
  return SAFE_ID_PATTERN.test(userId) ? userId : undefined
}

// Derive the owner key the free pool / premium allowlist key on. A verified
// owner claim keys to `owner:<ownerUserId>` (SHARED across all of that owner's
// accounts — Sybil resistance); an unclaimed account keys to a synthetic
// `account:<accountRef>` key (taste-only; never premium).
export const resolveOwnerKey = (
  accountRef: string,
  identity: VerifiedPublicIdentityClaim | undefined,
): string =>
  identity === undefined
    ? `account:${accountRef.trim()}`
    : `owner:${identity.ownerUserId}`

// Whether an owner key is a verified owner identity (vs a bare account). Used by
// callers that gate behavior on a verified claim (e.g. premium eligibility).
export const isVerifiedOwnerKey = (ownerKey: string): boolean =>
  ownerKey.startsWith('owner:')

// Build a resolver from an agent-owner-claim store method. The store's
// `readVerifiedPublicIdentityForAgentUserId` is the canonical surface the
// #5486 KYC gate uses. A non-agent-shaped account ref resolves to undefined
// (treated as unclaimed).
export const makeVerifiedOwnerIdentityResolver = (
  readVerifiedPublicIdentityForAgentUserId: (
    agentUserId: string,
  ) => Promise<VerifiedPublicIdentityClaim | undefined>,
): VerifiedOwnerIdentityResolver => {
  return async (accountRef: string) => {
    const agentUserId = agentUserIdFromAccountRef(accountRef)
    if (agentUserId === undefined) {
      return undefined
    }
    return readVerifiedPublicIdentityForAgentUserId(agentUserId)
  }
}
