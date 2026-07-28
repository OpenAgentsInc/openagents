// Per-install self-provisioning for the Omega Nostr session endpoint.
//
// WHAT THIS OPENS
// ---------------
// `POST /api/omega/auth/session` used to accept exactly ONE pubkey — the
// configured `SARAH_NOSTR_OWNER_PUBKEY` — and mint a session for the primary
// OpenAgents ADMIN user. Every other install signed a perfectly valid NIP-98
// proof and got a 401 it could never pass. That is an owner backdoor, not the
// "every single new install gets free access to our hosted Gemini Flash lane"
// the product promises.
//
// With self-provisioning armed, ANY valid NIP-98 proof mints a session bound to
// a user keyed on the SIGNING pubkey (`nostr:<pubkey>`), never on the admin
// account. Keypairs are free to generate, so a per-user quota bounds NOTHING in
// aggregate — the bounds have to sit on ACCOUNT CREATION and on total spend.
// This module owns those bounds. Read `omegaNostrSelfProvisionAbuseBound` below
// before changing any number in it.
//
// WHAT THIS DELIBERATELY DOES NOT DO
// ----------------------------------
// A self-provisioned identity gets a plain `users` row with `kind='human'` and
// an `auth_identities` row under provider `nostr`. It gets NO admin email, NO
// membership, NO entitlement, and NO scope. Admin authorization in this API is
// an email allowlist (`isOpenAgentsAdminEmail`); the synthetic address below
// lives on the RFC 2606 reserved `.invalid` TLD, so it can never be delivered
// to, never receive a sign-in code, and never match the allowlist.

import type { AuthKvStore } from './auth-kv'
import {
  type KvWindowRateLimitBucket,
  type KvWindowRateLimitResult,
  reserveKvWindowRateLimit,
  stableRateLimitSubject,
} from './kv-window-rate-limit'

/** Kill switch. Absent or anything but a truthy token => self-provisioning is OFF. */
export const OMEGA_NOSTR_SELF_PROVISION_FLAG =
  'OMEGA_NOSTR_SELF_PROVISION_ENABLED' as const

/** `users.id` / session `userId` namespace. Never collides with `github:`/`email:`. */
export const OMEGA_NOSTR_USER_ID_PREFIX = 'nostr:' as const

/** Non-deliverable reserved TLD (RFC 2606). A synthetic address, not a mailbox. */
export const OMEGA_NOSTR_SYNTHETIC_EMAIL_DOMAIN = 'nostr.invalid' as const

const RATE_LIMIT_KEY_PREFIX = 'omega_nostr:self_provision_rate'

export type OmegaNostrSelfProvisionScope =
  /** New `nostr:` account creations from one client IP. */
  | 'creation_ip'
  /** New `nostr:` account creations across the whole deployment. */
  | 'creation_global'
  /** Session mints for one already-provisioned pubkey. */
  | 'mint_pubkey'
  /** Session mints from one client IP, provisioned or not. */
  | 'mint_ip'

export type OmegaNostrSelfProvisionPolicy = Readonly<{
  creationGlobal: Readonly<{ limit: number; windowSeconds: number }>
  creationIp: Readonly<{ limit: number; windowSeconds: number }>
  mintIp: Readonly<{ limit: number; windowSeconds: number }>
  mintPubkey: Readonly<{ limit: number; windowSeconds: number }>
}>

// Conservative defaults. The GLOBAL creation ceiling is the only number that
// bounds aggregate owner-funded spend, because an attacker can trivially rotate
// both keypairs and source IPs. Every number here is env-overridable so the
// owner can retune without a code change; see `omegaNostrSelfProvisionPolicy`.
export const defaultOmegaNostrSelfProvisionPolicy: OmegaNostrSelfProvisionPolicy =
  {
    creationGlobal: { limit: 200, windowSeconds: 24 * 60 * 60 },
    creationIp: { limit: 3, windowSeconds: 60 * 60 },
    mintIp: { limit: 60, windowSeconds: 60 * 60 },
    mintPubkey: { limit: 20, windowSeconds: 60 * 60 },
  }

/**
 * Per-identity daily served-token ceiling for self-provisioned installs.
 *
 * This mirrors `BUILTIN_COMPUTE_AGENT_DAILY_TOKEN_CEILING`, which the builtin
 * grant advertises but — as of this change — nothing enforced. It is enforced
 * for `nostr:` identities in `handleGoogleGeminiGenerateContentApi`.
 */
export const OMEGA_NOSTR_SELF_PROVISION_DAILY_TOKEN_CEILING = 1_000_000

export type OmegaNostrSelfProvisionEnv = Readonly<{
  OMEGA_NOSTR_SELF_PROVISION_DAILY_TOKEN_CEILING?: string | undefined
  OMEGA_NOSTR_SELF_PROVISION_ENABLED?: string | undefined
  OMEGA_NOSTR_SELF_PROVISION_GLOBAL_DAILY_LIMIT?: string | undefined
  OMEGA_NOSTR_SELF_PROVISION_IP_HOURLY_LIMIT?: string | undefined
}>

const TRUTHY = ['1', 'true', 'yes', 'on']

export const omegaNostrSelfProvisionEnabled = (
  env: OmegaNostrSelfProvisionEnv,
): boolean => {
  const value = env.OMEGA_NOSTR_SELF_PROVISION_ENABLED

  return value === undefined
    ? false
    : TRUTHY.includes(value.trim().toLowerCase())
}

const positiveInteger = (
  value: string | undefined,
  fallback: number,
): number => {
  if (value === undefined) {
    return fallback
  }

  const parsed = Number.parseInt(value.trim(), 10)

  return Number.isSafeInteger(parsed) && parsed >= 0 ? parsed : fallback
}

export const omegaNostrSelfProvisionPolicy = (
  env: OmegaNostrSelfProvisionEnv,
): OmegaNostrSelfProvisionPolicy => ({
  ...defaultOmegaNostrSelfProvisionPolicy,
  creationGlobal: {
    ...defaultOmegaNostrSelfProvisionPolicy.creationGlobal,
    limit: positiveInteger(
      env.OMEGA_NOSTR_SELF_PROVISION_GLOBAL_DAILY_LIMIT,
      defaultOmegaNostrSelfProvisionPolicy.creationGlobal.limit,
    ),
  },
  creationIp: {
    ...defaultOmegaNostrSelfProvisionPolicy.creationIp,
    limit: positiveInteger(
      env.OMEGA_NOSTR_SELF_PROVISION_IP_HOURLY_LIMIT,
      defaultOmegaNostrSelfProvisionPolicy.creationIp.limit,
    ),
  },
})

export const omegaNostrSelfProvisionDailyTokenCeiling = (
  env: OmegaNostrSelfProvisionEnv,
): number =>
  positiveInteger(
    env.OMEGA_NOSTR_SELF_PROVISION_DAILY_TOKEN_CEILING,
    OMEGA_NOSTR_SELF_PROVISION_DAILY_TOKEN_CEILING,
  )

export const isOmegaNostrPubkey = (value: string): boolean =>
  /^[0-9a-f]{64}$/.test(value)

export const omegaNostrUserId = (pubkey: string): string =>
  `${OMEGA_NOSTR_USER_ID_PREFIX}${pubkey.trim().toLowerCase()}`

export const isOmegaNostrSelfProvisionedUserId = (userId: string): boolean =>
  userId.startsWith(OMEGA_NOSTR_USER_ID_PREFIX)

export const omegaNostrSyntheticEmail = (pubkey: string): string =>
  `${pubkey.trim().toLowerCase()}@${OMEGA_NOSTR_SYNTHETIC_EMAIL_DOMAIN}`

export const omegaNostrDisplayName = (pubkey: string): string =>
  `nostr-${pubkey.trim().toLowerCase().slice(0, 12)}`

export type OmegaNostrSelfProvisionReservation = KvWindowRateLimitResult<OmegaNostrSelfProvisionScope>

/**
 * Charge the buckets for one session mint.
 *
 * `creating` is TRUE only when the pubkey has no `users` row yet. A returning
 * install therefore never consumes the (deliberately tight) creation budget, so
 * a whole office behind one NAT is not locked out after its third laptop — only
 * genuinely NEW accounts are.
 */
export const reserveOmegaNostrSelfProvision = async (
  kv: AuthKvStore,
  input: Readonly<{
    creating: boolean
    ipAddress: string
    pubkey: string
  }>,
  nowMs: number,
  policy: OmegaNostrSelfProvisionPolicy = defaultOmegaNostrSelfProvisionPolicy,
): Promise<OmegaNostrSelfProvisionReservation> => {
  const ipSubject = await stableRateLimitSubject(input.ipAddress)
  const pubkeySubject = await stableRateLimitSubject(
    input.pubkey.trim().toLowerCase(),
  )
  const creationBuckets: ReadonlyArray<
    KvWindowRateLimitBucket<OmegaNostrSelfProvisionScope>
  > = input.creating
    ? [
        { ...policy.creationIp, scope: 'creation_ip', subject: ipSubject },
        {
          ...policy.creationGlobal,
          scope: 'creation_global',
          subject: 'all',
        },
      ]
    : []

  return reserveKvWindowRateLimit(
    kv,
    RATE_LIMIT_KEY_PREFIX,
    [
      // Creation buckets are listed FIRST so a rejection names the tighter,
      // more informative bound.
      ...creationBuckets,
      { ...policy.mintPubkey, scope: 'mint_pubkey', subject: pubkeySubject },
      { ...policy.mintIp, scope: 'mint_ip', subject: ipSubject },
    ],
    nowMs,
  )
}

/**
 * The honest, arithmetic abuse bound for this surface — kept in code so it
 * cannot drift away from the numbers it describes.
 *
 * Read `docs/audits/2026-07-28-omega-nostr-self-provisioning-abuse-bounds.md`
 * for the full analysis, including the two PRE-EXISTING holes that dominate
 * this one.
 */
export const omegaNostrSelfProvisionAbuseBound = (
  policy: OmegaNostrSelfProvisionPolicy,
  dailyTokenCeiling: number,
): Readonly<{
  newAccountsPerDay: number
  tokensPerHourSingleIp: number
  tokensPerDayCeiling: number
}> => ({
  newAccountsPerDay: policy.creationGlobal.limit,
  tokensPerDayCeiling: policy.creationGlobal.limit * dailyTokenCeiling,
  tokensPerHourSingleIp: policy.creationIp.limit * dailyTokenCeiling,
})
