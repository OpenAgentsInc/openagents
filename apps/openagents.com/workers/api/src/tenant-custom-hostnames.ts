import { Effect, Layer, Schema as S } from 'effect'
import * as Context from 'effect/Context'

import { openAgentsDatabase } from './runtime'
import { compactRandomId, currentIsoTimestamp } from './runtime-primitives'

// COORDINATOR WIRING: This module supplies the hostname->tenant resolver used to
// render a branded subdomain host-scoped. The Worker request pipeline must, on
// each inbound request, read the Host header, call
// `makeTenantCustomHostnames(db).resolveTenantByHostname(host)` and, when it
// returns a non-null TenantRef, scope the response to that team/tenant. Wire
// that lookup into the Worker entry (index.ts) / host-resolution middleware.
// This module intentionally does NOT edit index.ts.

type TenantCustomHostnamesEnv = Readonly<{
  OPENAGENTS_DB: D1Database
}>

export type TenantCustomHostnamesRuntime = Readonly<{
  makeHostnameId: () => string
  makeVerificationToken: () => string
  nowIso: () => string
}>

export const systemTenantCustomHostnamesRuntime: TenantCustomHostnamesRuntime =
  {
    makeHostnameId: () => compactRandomId('tenant_hostname'),
    makeVerificationToken: () =>
      compactRandomId('tenant_hostname_verify'),
    nowIso: currentIsoTimestamp,
  }

const MAX_HOSTNAME_CHARS = 255

export const TenantCustomHostnameStatus = S.Literals([
  'pending',
  'verified',
  'active',
  'disabled',
])
export type TenantCustomHostnameStatus =
  typeof TenantCustomHostnameStatus.Type

export const TenantCustomHostname = S.Struct({
  id: S.String,
  teamId: S.String,
  hostname: S.String,
  status: TenantCustomHostnameStatus,
  verificationToken: S.String,
  verifiedAt: S.NullOr(S.String),
  createdAt: S.String,
  updatedAt: S.String,
})
export type TenantCustomHostname = typeof TenantCustomHostname.Type

// The tenant reference handed back to host-scoped rendering. Today a tenant is
// identified by its team, but this struct keeps the resolver result distinct
// from the raw hostname row so callers depend on the tenant boundary, not the
// storage shape.
export const TenantRef = S.Struct({
  teamId: S.String,
  hostname: S.String,
  status: TenantCustomHostnameStatus,
})
export type TenantRef = typeof TenantRef.Type

export const RegisterTenantHostnameInput = S.Struct({
  teamId: S.String,
  hostname: S.String,
})
export type RegisterTenantHostnameInput =
  typeof RegisterTenantHostnameInput.Type

type TenantCustomHostnameRow = Readonly<{
  id: string
  team_id: string
  hostname: string
  status: TenantCustomHostnameStatus
  verification_token: string
  verified_at: string | null
  created_at: string
  updated_at: string
}>

export class TenantCustomHostnameStorageError extends S.TaggedErrorClass<TenantCustomHostnameStorageError>()(
  'TenantCustomHostnameStorageError',
  {
    operation: S.String,
    error: S.Defect,
  },
) {}

export class TenantCustomHostnameValidationError extends S.TaggedErrorClass<TenantCustomHostnameValidationError>()(
  'TenantCustomHostnameValidationError',
  {
    reason: S.String,
  },
) {}

export class TenantCustomHostnameConflictError extends S.TaggedErrorClass<TenantCustomHostnameConflictError>()(
  'TenantCustomHostnameConflictError',
  {
    hostname: S.String,
  },
) {}

export class TenantCustomHostnameNotFoundError extends S.TaggedErrorClass<TenantCustomHostnameNotFoundError>()(
  'TenantCustomHostnameNotFoundError',
  {
    hostname: S.String,
  },
) {}

export type TenantCustomHostnameError =
  | TenantCustomHostnameStorageError
  | TenantCustomHostnameValidationError
  | TenantCustomHostnameConflictError
  | TenantCustomHostnameNotFoundError

const d1Effect = <A>(
  operation: string,
  run: () => Promise<A>,
): Effect.Effect<A, TenantCustomHostnameStorageError> =>
  Effect.tryPromise({
    try: run,
    catch: error =>
      new TenantCustomHostnameStorageError({ operation, error }),
  })

const rowToHostname = (
  row: TenantCustomHostnameRow,
): TenantCustomHostname => ({
  id: row.id,
  teamId: row.team_id,
  hostname: row.hostname,
  status: row.status,
  verificationToken: row.verification_token,
  verifiedAt: row.verified_at,
  createdAt: row.created_at,
  updatedAt: row.updated_at,
})

const rowToTenantRef = (row: TenantCustomHostnameRow): TenantRef => ({
  teamId: row.team_id,
  hostname: row.hostname,
  status: row.status,
})

// Hostnames are case-insensitive and a trailing dot is the root label; we
// normalize before storage and lookup so the UNIQUE index and resolver agree.
export const normalizeHostname = (
  value: string,
): Effect.Effect<string, TenantCustomHostnameValidationError> => {
  const hostname = value.trim().toLowerCase().replace(/\.$/, '')

  if (hostname === '') {
    return Effect.fail(
      new TenantCustomHostnameValidationError({
        reason: 'hostname is required.',
      }),
    )
  }

  if (hostname.length > MAX_HOSTNAME_CHARS) {
    return Effect.fail(
      new TenantCustomHostnameValidationError({
        reason: `hostname exceeds ${MAX_HOSTNAME_CHARS} characters.`,
      }),
    )
  }

  // Conservative DNS label check: dot-separated labels of letters, digits, and
  // internal hyphens. This is deterministic, bounded validation of an already
  // structured field, not user-facing intent routing.
  const labelPattern = /^[a-z0-9](?:[a-z0-9-]*[a-z0-9])?$/
  const labels = hostname.split('.')

  if (
    labels.length < 2 ||
    labels.some(label => label === '' || !labelPattern.test(label))
  ) {
    return Effect.fail(
      new TenantCustomHostnameValidationError({
        reason: `hostname is not a valid DNS hostname: ${value}`,
      }),
    )
  }

  return Effect.succeed(hostname)
}

const readByHostname = (
  db: D1Database,
  hostname: string,
): Effect.Effect<
  TenantCustomHostnameRow | null,
  TenantCustomHostnameStorageError
> =>
  d1Effect('tenantCustomHostnames.readByHostname', () =>
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
  )

const registerHostname = (
  db: D1Database,
  runtime: TenantCustomHostnamesRuntime,
  input: RegisterTenantHostnameInput,
): Effect.Effect<TenantCustomHostname, TenantCustomHostnameError> =>
  Effect.gen(function* () {
    const hostname = yield* normalizeHostname(input.hostname)
    const existing = yield* readByHostname(db, hostname)

    if (existing !== null) {
      return yield* new TenantCustomHostnameConflictError({ hostname })
    }

    const id = runtime.makeHostnameId()
    const verificationToken = runtime.makeVerificationToken()
    const now = runtime.nowIso()
    const status: TenantCustomHostnameStatus = 'pending'

    yield* d1Effect('tenantCustomHostnames.insert', () =>
      db
        .prepare(
          `INSERT INTO tenant_custom_hostnames
             (id,
              team_id,
              hostname,
              status,
              verification_token,
              verified_at,
              created_at,
              updated_at)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?)`,
        )
        .bind(
          id,
          input.teamId,
          hostname,
          status,
          verificationToken,
          null,
          now,
          now,
        )
        .run(),
    )

    return {
      id,
      teamId: input.teamId,
      hostname,
      status,
      verificationToken,
      verifiedAt: null,
      createdAt: now,
      updatedAt: now,
    }
  })

const transitionStatus = (
  db: D1Database,
  runtime: TenantCustomHostnamesRuntime,
  operation: string,
  status: TenantCustomHostnameStatus,
  setVerifiedAt: boolean,
  rawHostname: string,
): Effect.Effect<TenantCustomHostname, TenantCustomHostnameError> =>
  Effect.gen(function* () {
    const hostname = yield* normalizeHostname(rawHostname)
    const existing = yield* readByHostname(db, hostname)

    if (existing === null) {
      return yield* new TenantCustomHostnameNotFoundError({ hostname })
    }

    const now = runtime.nowIso()
    const verifiedAt = setVerifiedAt
      ? (existing.verified_at ?? now)
      : existing.verified_at

    yield* d1Effect(operation, () =>
      db
        .prepare(
          `UPDATE tenant_custom_hostnames
              SET status = ?,
                  verified_at = ?,
                  updated_at = ?
            WHERE hostname = ?`,
        )
        .bind(status, verifiedAt, now, hostname)
        .run(),
    )

    return {
      ...rowToHostname(existing),
      status,
      verifiedAt,
      updatedAt: now,
    }
  })

const markVerified = (
  db: D1Database,
  runtime: TenantCustomHostnamesRuntime,
  hostname: string,
): Effect.Effect<TenantCustomHostname, TenantCustomHostnameError> =>
  transitionStatus(
    db,
    runtime,
    'tenantCustomHostnames.markVerified',
    'verified',
    true,
    hostname,
  )

const markActive = (
  db: D1Database,
  runtime: TenantCustomHostnamesRuntime,
  hostname: string,
): Effect.Effect<TenantCustomHostname, TenantCustomHostnameError> =>
  transitionStatus(
    db,
    runtime,
    'tenantCustomHostnames.markActive',
    'active',
    true,
    hostname,
  )

const markDisabled = (
  db: D1Database,
  runtime: TenantCustomHostnamesRuntime,
  hostname: string,
): Effect.Effect<TenantCustomHostname, TenantCustomHostnameError> =>
  transitionStatus(
    db,
    runtime,
    'tenantCustomHostnames.markDisabled',
    'disabled',
    false,
    hostname,
  )

// Maps an incoming custom hostname to its tenant. Only an `active` mapping
// resolves to a live tenant; pending/verified/disabled rows return null so a
// branded subdomain only renders host-scoped once it is fully provisioned.
const resolveTenantByHostname = (
  db: D1Database,
  rawHostname: string,
): Effect.Effect<TenantRef | null, TenantCustomHostnameStorageError> =>
  normalizeHostname(rawHostname).pipe(
    // A non-hostname value cannot map to a tenant; treat as a miss rather than
    // an error so the resolver is safe to call on every inbound Host header.
    Effect.catchTag('TenantCustomHostnameValidationError', () =>
      Effect.succeed(null),
    ),
    Effect.flatMap(hostname =>
      hostname === null
        ? Effect.succeed(null)
        : readByHostname(db, hostname).pipe(
            Effect.map(row =>
              row === null || row.status !== 'active'
                ? null
                : rowToTenantRef(row),
            ),
          ),
    ),
  )

export const makeTenantCustomHostnames = (
  db: D1Database,
  runtime: TenantCustomHostnamesRuntime = systemTenantCustomHostnamesRuntime,
) => ({
  register: Effect.fn('TenantCustomHostnames.register')(
    (input: RegisterTenantHostnameInput) =>
      registerHostname(db, runtime, input),
  ),
  markVerified: Effect.fn('TenantCustomHostnames.markVerified')(
    (hostname: string) => markVerified(db, runtime, hostname),
  ),
  markActive: Effect.fn('TenantCustomHostnames.markActive')(
    (hostname: string) => markActive(db, runtime, hostname),
  ),
  markDisabled: Effect.fn('TenantCustomHostnames.markDisabled')(
    (hostname: string) => markDisabled(db, runtime, hostname),
  ),
  resolveTenantByHostname: Effect.fn(
    'TenantCustomHostnames.resolveTenantByHostname',
  )((hostname: string) => resolveTenantByHostname(db, hostname)),
})

export class TenantCustomHostnames extends Context.Service<
  TenantCustomHostnames,
  ReturnType<typeof makeTenantCustomHostnames>
>()('@openagentsinc/autopilot-omega/TenantCustomHostnames') {
  static layer = (
    env: TenantCustomHostnamesEnv,
    runtime: TenantCustomHostnamesRuntime = systemTenantCustomHostnamesRuntime,
  ) =>
    Layer.succeed(
      TenantCustomHostnames,
      makeTenantCustomHostnames(openAgentsDatabase(env), runtime),
    )
}
