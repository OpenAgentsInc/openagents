import { Effect, Schema as S } from 'effect'

import {
  type RegisterTenantHostnameInput,
  type TenantCustomHostname,
  type TenantCustomHostnameError,
  TenantCustomHostnameStorageError,
  type TenantCustomHostnamesRuntime,
  makeTenantCustomHostnames,
  normalizeHostname,
  systemTenantCustomHostnamesRuntime,
} from './tenant-custom-hostnames'

// ===========================================================================
// CUSTOMER SELF-SERVE custom-hostname core (OpenAgents #4988 follow-up)
// ===========================================================================
//
// This module is the missing CUSTOMER self-serve path for custom tenant
// hostnames. Until now the only way to register/verify/activate a tenant
// hostname was the OPERATOR/admin provisioning core
// (tenant-hostname-provisioning.ts). This adds the path a SIGNED-IN CUSTOMER
// can drive for their OWN team:
//
//   1. CLAIM a hostname for a team they own/administer (creates the pending
//      row + a DNS verification token).
//   2. LIST the hostnames their team has claimed, each with the exact DNS
//      record the customer must publish to prove ownership.
//   3. READ the status of one of their own hostnames (pending/verified/active/
//      disabled) plus the verification instructions.
//
// AUTHORITY / SAFETY BOUNDARY (why this is safe to mount default-ON):
//   - It NEVER touches real DNS, never issues SSL, never binds a live origin,
//     and never spends. All it does is write/read rows in the existing
//     tenant_custom_hostnames table (migration 0182) through the same
//     transitions tenant-custom-hostnames.ts already exposes.
//   - A claimed hostname starts (and stays) `pending`. It does NOT resolve to
//     a live tenant — the resolver in tenant-custom-hostnames.ts only resolves
//     `active` rows, and ONLY the owner-gated provisioning core (which is
//     itself gated behind owner-provisioned Cloudflare secrets, default-OFF)
//     can drive a row to `active`. So a customer claiming a hostname here
//     cannot serve anything until the owner arms provisioning.
//   - Verification here is INERT/advisory: we return the TXT record the
//     customer must publish, but we do NOT perform a live DNS lookup and we do
//     NOT flip the row to `verified`/`active` from customer input. That step
//     remains the owner-gated provisioning core's job. The flag
//     `selfServeLiveDnsVerificationArmed` is reserved for a future, owner-armed
//     live-DNS check and is default false here.
//
// This module does NOT edit index.ts; the route layer
// (tenant-custom-hostname-self-serve-routes.ts) wires it behind a browser
// session + team-owner/admin gate.

// The team roles allowed to manage a team's custom hostnames. A plain `member`
// or `viewer` can see but not claim; only owner/admin can claim. The route
// layer enforces this against readActiveTeamMembershipRole.
export const HostnameManagerRoles: ReadonlySet<string> = new Set([
  'owner',
  'admin',
])

// The DNS instruction we hand the customer so they can prove control of the
// hostname out-of-band. This is advisory copy + a deterministic record shape,
// not a live verification. The actual ownership proof + certificate issuance
// is performed by the owner-gated Cloudflare-for-SaaS provisioning core.
export const HostnameVerificationInstruction = S.Struct({
  // The DNS record TYPE the customer publishes (a TXT record at a known name).
  recordType: S.Literal('TXT'),
  // The record NAME (host) the customer publishes the TXT at.
  recordName: S.String,
  // The exact value the customer must set, derived from our verification token.
  recordValue: S.String,
  // Human-readable guidance shown in the self-serve UI.
  note: S.String,
})
export type HostnameVerificationInstruction =
  typeof HostnameVerificationInstruction.Type

// What a customer sees for one of their hostnames. We deliberately DO NOT
// expose the raw verification_token as a standalone secret-ish field; it is
// only surfaced inside the DNS instruction the customer is meant to publish.
export const CustomerHostnameView = S.Struct({
  id: S.String,
  teamId: S.String,
  hostname: S.String,
  status: S.Literals(['pending', 'verified', 'active', 'disabled']),
  verifiedAt: S.NullOr(S.String),
  createdAt: S.String,
  updatedAt: S.String,
  // Present while the hostname is not yet live so the customer knows what to
  // do next. Null once the hostname is active (nothing left to publish).
  verification: S.NullOr(HostnameVerificationInstruction),
  // True only when the owner has armed live provisioning AND this row is
  // active. Customer-facing copy must not claim "live" otherwise.
  servingLive: S.Boolean,
})
export type CustomerHostnameView = typeof CustomerHostnameView.Type

// The prefix label under which the verification TXT record is published. Kept
// vendor-neutral and stable so instructions are reproducible.
const VERIFICATION_LABEL = '_openagents-verify'

const verificationInstruction = (
  hostname: string,
  verificationToken: string,
): HostnameVerificationInstruction => ({
  recordType: 'TXT',
  recordName: `${VERIFICATION_LABEL}.${hostname}`,
  recordValue: `openagents-site-verification=${verificationToken}`,
  note:
    'Publish this TXT record at your DNS provider to prove you control this hostname. After it propagates, OpenAgents completes verification, certificate issuance, and routing. Until then the hostname stays pending and does not serve your site.',
})

const toCustomerView = (
  record: TenantCustomHostname,
  selfServeLiveDnsVerificationArmed: boolean,
): CustomerHostnameView => {
  const isActive = record.status === 'active'

  return {
    id: record.id,
    teamId: record.teamId,
    hostname: record.hostname,
    status: record.status,
    verifiedAt: record.verifiedAt,
    createdAt: record.createdAt,
    updatedAt: record.updatedAt,
    // Once active there is nothing left for the customer to publish.
    verification: isActive
      ? null
      : verificationInstruction(record.hostname, record.verificationToken),
    // "Serving live" is only honest when provisioning is armed AND the row is
    // active. With the default-OFF flag this is always false, matching the
    // promise's INERT-until-armed posture.
    servingLive: selfServeLiveDnsVerificationArmed && isActive,
  }
}

export type TenantCustomHostnameSelfServeConfig = Readonly<{
  // Reserved owner-armed flag. When false (default) the self-serve path is
  // claim + read-only-status + advisory-DNS only: it performs NO live DNS
  // lookup and reports servingLive=false for every hostname. Flipping it true
  // is an owner decision wired alongside the Cloudflare provisioning secrets;
  // this module never flips it itself.
  selfServeLiveDnsVerificationArmed: boolean
}>

export const defaultSelfServeConfig: TenantCustomHostnameSelfServeConfig = {
  selfServeLiveDnsVerificationArmed: false,
}

export const ClaimHostnameInput = S.Struct({
  teamId: S.String,
  hostname: S.String,
})
export type ClaimHostnameInput = typeof ClaimHostnameInput.Type

// Reading a team's hostnames needs a small read that tenant-custom-hostnames.ts
// does not expose (it only reads-by-hostname and resolves active). This is a
// plain SELECT over the fixed-shape 0182 table; all WRITES still go through the
// shared transitions in tenant-custom-hostnames.ts.
type TenantCustomHostnameRow = Readonly<{
  id: string
  team_id: string
  hostname: string
  status: TenantCustomHostname['status']
  verification_token: string
  verified_at: string | null
  created_at: string
  updated_at: string
}>

const rowToRecord = (row: TenantCustomHostnameRow): TenantCustomHostname => ({
  id: row.id,
  teamId: row.team_id,
  hostname: row.hostname,
  status: row.status,
  verificationToken: row.verification_token,
  verifiedAt: row.verified_at,
  createdAt: row.created_at,
  updatedAt: row.updated_at,
})

const readHostnamesForTeam = (
  db: D1Database,
  teamId: string,
): Effect.Effect<
  ReadonlyArray<TenantCustomHostname>,
  TenantCustomHostnameStorageError
> =>
  Effect.tryPromise({
    try: () =>
      db
        .prepare(
          `SELECT id,
                  team_id,
                  hostname,
                  status,
                  verification_token,
                  verified_at,
                  created_at,
                  updated_at
             FROM tenant_custom_hostnames
            WHERE team_id = ?
            ORDER BY created_at ASC`,
        )
        .bind(teamId)
        .all<TenantCustomHostnameRow>(),
    catch: error =>
      new TenantCustomHostnameStorageError({
        operation: 'tenantCustomHostnameSelfServe.readHostnamesForTeam',
        error,
      }),
  }).pipe(Effect.map(result => (result.results ?? []).map(rowToRecord)))

export type TenantCustomHostnameSelfServeShape = Readonly<{
  // Customer claims a hostname for a team they manage. The route layer has
  // ALREADY verified team-management role before calling this; this core
  // assumes the teamId is authorized.
  claim: (
    input: ClaimHostnameInput,
  ) => Effect.Effect<CustomerHostnameView, TenantCustomHostnameError>
  // List all hostnames a team has claimed, as customer-facing views.
  listForTeam: (
    teamId: string,
  ) => Effect.Effect<
    ReadonlyArray<CustomerHostnameView>,
    TenantCustomHostnameStorageError
  >
}>

export const makeTenantCustomHostnameSelfServe = (
  db: D1Database,
  config: TenantCustomHostnameSelfServeConfig = defaultSelfServeConfig,
  runtime: TenantCustomHostnamesRuntime = systemTenantCustomHostnamesRuntime,
): TenantCustomHostnameSelfServeShape => {
  const repo = makeTenantCustomHostnames(db, runtime)

  const claim: TenantCustomHostnameSelfServeShape['claim'] = Effect.fn(
    'TenantCustomHostnameSelfServe.claim',
  )((input: ClaimHostnameInput) =>
    Effect.gen(function* () {
      // Normalize first so a re-claim of an already-claimed-by-this-team
      // hostname is idempotent rather than a hard conflict.
      const hostname = yield* normalizeHostname(input.hostname)
      const registerInput: RegisterTenantHostnameInput = {
        teamId: input.teamId,
        hostname,
      }

      const record = yield* repo.register(registerInput).pipe(
        Effect.catchTag('TenantCustomHostnameConflictError', () =>
          // Already claimed: reuse it iff THIS team owns it, otherwise the
          // hostname belongs to another tenant and the customer cannot take it.
          readHostnamesForTeam(db, input.teamId).pipe(
            Effect.flatMap(records => {
              const existing = records.find(r => r.hostname === hostname)

              return existing === undefined
                ? // The hostname exists but for a DIFFERENT team. Surface a
                  // conflict the route can turn into a 409 without leaking the
                  // owning team.
                  new TenantCustomHostnameStorageError({
                    operation: 'tenantCustomHostnameSelfServe.claim.conflict',
                    error: new Error('hostname_taken'),
                  })
                : Effect.succeed(existing)
            }),
          ),
        ),
      )

      return toCustomerView(
        record,
        config.selfServeLiveDnsVerificationArmed,
      )
    }),
  )

  const listForTeam: TenantCustomHostnameSelfServeShape['listForTeam'] =
    Effect.fn('TenantCustomHostnameSelfServe.listForTeam')((teamId: string) =>
      readHostnamesForTeam(db, teamId).pipe(
        Effect.map(records =>
          records.map(record =>
            toCustomerView(
              record,
              config.selfServeLiveDnsVerificationArmed,
            ),
          ),
        ),
      ),
    )

  return { claim, listForTeam }
}
