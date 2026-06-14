import { Context, Effect, Layer, Schema as S } from 'effect'

import { openAgentsDatabase } from './runtime'
import {
  type RegisterTenantHostnameInput,
  type TenantCustomHostname,
  type TenantCustomHostnameError,
  TenantCustomHostnameNotFoundError,
  TenantCustomHostnameStorageError,
  TenantCustomHostnameValidationError,
  type TenantCustomHostnamesRuntime,
  makeTenantCustomHostnames,
  normalizeHostname,
  systemTenantCustomHostnamesRuntime,
} from './tenant-custom-hostnames'

// COORDINATOR WIRING (OpenAgents #4988) — custom-hostname PROVISIONING core.
//
// What this module is:
//   A testable Effect service that drives the Cloudflare-for-SaaS custom
//   hostname lifecycle (register -> provision -> verify -> activate) over the
//   existing tenant_custom_hostnames storage (migration 0182, owned by
//   tenant-custom-hostnames.ts). The Cloudflare API is reached only through an
//   INJECTED `CustomHostnameClient`, so the whole service is unit-testable with
//   a fake client and NO live credentials.
//
// What stays OWNER-GATED (out of scope here, do NOT wire without owner sign-off):
//   - The LIVE `CustomHostnameClient` implementation that calls the real
//     Cloudflare for SaaS REST API
//     (POST/GET/DELETE /zones/{zone_id}/custom_hostnames).
//   - Its configuration: CLOUDFLARE_API_TOKEN (scoped to "SSL and Certificates
//     Edit" on the fallback origin zone) and CLOUDFLARE_ZONE_ID. These are
//     secrets/vars that must be provisioned by the owner in wrangler/D1/env.
//   - Binding the provisioning route into the Worker entry (index.ts) behind
//     the operator/admin gate.
//
// COORDINATOR follow-up (owner-gated), sketch only:
//   const liveClient: CustomHostnameClient = makeCloudflareCustomHostnameClient({
//     apiToken: env.CLOUDFLARE_API_TOKEN, // owner-provisioned secret
//     zoneId: env.CLOUDFLARE_ZONE_ID,     // owner-provisioned var
//     fetch: globalThis.fetch,
//   })
//   const layer = TenantHostnameProvisioning.layer(env, liveClient)
// then mount `provision` / `reconcile` on an operator-gated POST route in
// index.ts. This module intentionally does NOT edit index.ts and ships only a
// FAKE client for tests.
//
// NOTE ON STORAGE (no new migration): migration 0182 has no column for the
// Cloudflare custom_hostname id. Rather than reserve 0185, the live client's
// createCustomHostname acts as an idempotent UPSERT (Cloudflare returns the
// existing record when POSTing a hostname that already exists), and the CF id
// is returned to the caller in the provision outcome. `reconcile` therefore
// takes the cloudflareId from that outcome and reads fresh status via
// getStatus. If a future design needs durable CF-id storage, reserve 0185 then.

// ---------------------------------------------------------------------------
// Cloudflare for SaaS custom-hostname client contract (injected)
// ---------------------------------------------------------------------------

// Mirrors the Cloudflare "custom hostname" SSL status field. We keep the values
// that actually drive our state machine; any unrecognized live value is mapped
// to 'pending' by the live adapter (owner-gated) before reaching this service.
export const CloudflareCustomHostnameStatus = S.Literals([
  // Cloudflare is still validating ownership / issuing the certificate.
  'pending',
  // Ownership validated and certificate active; hostname is live.
  'active',
  // Validation or issuance failed and will not progress without intervention.
  'failed',
])
export type CloudflareCustomHostnameStatus =
  typeof CloudflareCustomHostnameStatus.Type

// The shape returned by the Cloudflare for SaaS custom-hostnames API for a
// single hostname. `id` is Cloudflare's custom_hostname id (NOT our row id).
export const CloudflareCustomHostname = S.Struct({
  id: S.String,
  hostname: S.String,
  status: CloudflareCustomHostnameStatus,
})
export type CloudflareCustomHostname = typeof CloudflareCustomHostname.Type

export type CreateCustomHostnameInput = Readonly<{
  hostname: string
  // Our own verification token, surfaced to Cloudflare as a custom metadata
  // value so a later reconcile can correlate the CF record with our row.
  verificationToken: string
}>

// Errors raised by the injected client. The live adapter (owner-gated) is
// responsible for mapping HTTP/transport failures onto this tagged error so the
// provisioning service can branch deterministically.
export class CustomHostnameClientError extends S.TaggedErrorClass<CustomHostnameClientError>()(
  'CustomHostnameClientError',
  {
    operation: S.String,
    error: S.Defect,
  },
) {}

// The injected dependency. A live implementation talks to Cloudflare; the test
// implementation is an in-memory fake. The service depends ONLY on this shape.
export type CustomHostnameClient = Readonly<{
  // Idempotent upsert: creating an already-existing hostname returns the
  // existing Cloudflare record (matching the Cloudflare for SaaS API).
  createCustomHostname: (
    input: CreateCustomHostnameInput,
  ) => Effect.Effect<CloudflareCustomHostname, CustomHostnameClientError>
  getStatus: (
    cloudflareId: string,
  ) => Effect.Effect<CloudflareCustomHostname, CustomHostnameClientError>
  deleteCustomHostname: (
    cloudflareId: string,
  ) => Effect.Effect<void, CustomHostnameClientError>
}>

// ---------------------------------------------------------------------------
// Provisioning result / errors
// ---------------------------------------------------------------------------

// What the provisioning step settled on this pass. The record is the current
// tenant_custom_hostnames row; `cloudflareId` lets a caller poll/reconcile.
export type TenantHostnameProvisionOutcome = Readonly<{
  record: TenantCustomHostname
  cloudflareId: string
  cloudflareStatus: CloudflareCustomHostnameStatus
}>

// Raised when Cloudflare reports the certificate/validation failed. The service
// drives our row to 'disabled' before failing so it never resolves to a live
// tenant; an operator can inspect the Cloudflare record by id.
export class TenantHostnameProvisionFailedError extends S.TaggedErrorClass<TenantHostnameProvisionFailedError>()(
  'TenantHostnameProvisionFailedError',
  {
    hostname: S.String,
    cloudflareId: S.String,
  },
) {}

export type TenantHostnameProvisioningError =
  | TenantCustomHostnameError
  | CustomHostnameClientError
  | TenantHostnameProvisionFailedError

// ---------------------------------------------------------------------------
// Provisioning service
// ---------------------------------------------------------------------------

export type TenantHostnameProvisioningShape = Readonly<{
  // Idempotently register (if needed) and ask Cloudflare to provision the
  // hostname, then reconcile our row against Cloudflare's reported status:
  //   CF pending -> our row stays/returns 'pending'  (verification-pending)
  //   CF active  -> our row -> 'verified' -> 'active' (happy path)
  //   CF failed  -> our row -> 'disabled' + fail with TenantHostnameProvisionFailedError
  provision: (
    input: RegisterTenantHostnameInput,
  ) => Effect.Effect<
    TenantHostnameProvisionOutcome,
    TenantHostnameProvisioningError
  >
  // Re-poll Cloudflare (by the id returned from `provision`) for an already
  // registered hostname and re-apply the same reconcile rules. Used to advance
  // a 'pending' provision to 'active' later (verification-pending -> retry).
  reconcile: (
    input: Readonly<{ hostname: string; cloudflareId: string }>,
  ) => Effect.Effect<
    TenantHostnameProvisionOutcome,
    TenantHostnameProvisioningError
  >
}>

export class TenantHostnameProvisioning extends Context.Service<
  TenantHostnameProvisioning,
  TenantHostnameProvisioningShape
>()('@openagentsinc/autopilot-omega/TenantHostnameProvisioning') {
  static layer = (
    env: Readonly<{ OPENAGENTS_DB: D1Database }>,
    client: CustomHostnameClient,
    runtime: TenantCustomHostnamesRuntime = systemTenantCustomHostnamesRuntime,
  ) =>
    Layer.succeed(
      TenantHostnameProvisioning,
      makeTenantHostnameProvisioning(openAgentsDatabase(env), client, runtime),
    )
}

export const makeTenantHostnameProvisioning = (
  db: D1Database,
  client: CustomHostnameClient,
  runtime: TenantCustomHostnamesRuntime = systemTenantCustomHostnamesRuntime,
): TenantHostnameProvisioningShape => {
  const repo = makeTenantCustomHostnames(db, runtime)

  // Apply Cloudflare's reported status to our storage row. The CF status is the
  // source of truth for whether the hostname is live; our local statuses are a
  // projection of it that the resolver trusts (only 'active' resolves).
  const reconcileAgainst = (
    record: TenantCustomHostname,
    cf: CloudflareCustomHostname,
  ): Effect.Effect<
    TenantHostnameProvisionOutcome,
    TenantHostnameProvisioningError
  > =>
    Effect.gen(function* () {
      if (cf.status === 'failed') {
        // Drive the row to 'disabled' so it never resolves, then surface the
        // failure so the operator/caller can react.
        const disabled = yield* repo.markDisabled(record.hostname)

        return yield* new TenantHostnameProvisionFailedError({
          hostname: disabled.hostname,
          cloudflareId: cf.id,
        })
      }

      if (cf.status === 'active') {
        // Idempotent: markVerified preserves an existing verifiedAt, and
        // markActive is safe to re-run on an already-active row.
        yield* repo.markVerified(record.hostname)
        const active = yield* repo.markActive(record.hostname)

        return {
          record: active,
          cloudflareId: cf.id,
          cloudflareStatus: cf.status,
        }
      }

      // cf.status === 'pending': leave our row in its current (pending) state.
      return {
        record,
        cloudflareId: cf.id,
        cloudflareStatus: cf.status,
      }
    })

  const provision: TenantHostnameProvisioningShape['provision'] = Effect.fn(
    'TenantHostnameProvisioning.provision',
  )((input: RegisterTenantHostnameInput) =>
    Effect.gen(function* () {
      const hostname = yield* normalizeHostname(input.hostname)
      // Idempotent register: if the hostname already exists, reuse the row when
      // it belongs to the same team; a different team owning it is a real
      // conflict (surfaced as a validation error, not a silent takeover).
      const record = yield* repo.register(input).pipe(
        Effect.catchTag('TenantCustomHostnameConflictError', () =>
          Effect.gen(function* () {
            const existing = yield* readRecord(hostname)

            if (existing.teamId !== input.teamId) {
              return yield* new TenantCustomHostnameValidationError({
                reason: `hostname ${hostname} is already provisioned for another team.`,
              })
            }

            return existing
          }),
        ),
      )

      const cf = yield* client.createCustomHostname({
        hostname: record.hostname,
        verificationToken: record.verificationToken,
      })

      return yield* reconcileAgainst(record, cf)
    }),
  )

  const reconcile: TenantHostnameProvisioningShape['reconcile'] = Effect.fn(
    'TenantHostnameProvisioning.reconcile',
  )((input: Readonly<{ hostname: string; cloudflareId: string }>) =>
    Effect.gen(function* () {
      const hostname = yield* normalizeHostname(input.hostname)
      const record = yield* readRecord(hostname)
      const cf = yield* client.getStatus(input.cloudflareId)

      return yield* reconcileAgainst(record, cf)
    }),
  )

  // Read the current row WITHOUT mutating it. tenant-custom-hostnames.ts only
  // exposes mutating transitions plus an active-only resolver, and we must not
  // edit that shared file (#4988 integration is deferred). We therefore read
  // the row directly against the 0182 table here. This is a plain SELECT over a
  // fixed-shape table, not a parallel write path: all writes still go through
  // tenant-custom-hostnames' transitions.
  function readRecord(
    hostname: string,
  ): Effect.Effect<TenantCustomHostname, TenantCustomHostnameError> {
    return Effect.tryPromise({
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
              WHERE hostname = ?
              LIMIT 1`,
          )
          .bind(hostname)
          .first<TenantCustomHostnameRow>(),
      catch: error =>
        new TenantCustomHostnameStorageError({
          operation: 'tenantHostnameProvisioning.readRecord',
          error,
        }),
    }).pipe(
      Effect.flatMap(row =>
        row === null
          ? Effect.fail(new TenantCustomHostnameNotFoundError({ hostname }))
          : Effect.succeed<TenantCustomHostname>({
              id: row.id,
              teamId: row.team_id,
              hostname: row.hostname,
              status: row.status,
              verificationToken: row.verification_token,
              verifiedAt: row.verified_at,
              createdAt: row.created_at,
              updatedAt: row.updated_at,
            }),
      ),
    )
  }

  return { provision, reconcile }
}

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
