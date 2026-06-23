// Sandbox compute service — sellable Cloud primitive SCAFFOLD (EPIC #5510, #5517).
//
// MVP/scaffold mirroring the inference-gateway pattern: a typed rentable-sandbox
// request surface + a provider/adapter seam + a metering/receipt hook, flag-gated
// INERT by default so NOTHING changes in production until the rest lands.
//
//   - flag-gated INERT by default (CLOUD_SANDBOX_COMPUTE_ENABLED, default off)
//   - typed sandbox request (scope/resources -> typed sandbox model)
//   - runtime-adapter seam (`SandboxRuntimeAdapter`) that the real isolated
//     execution substrate (the cloud-coding session lane,
//     `autopilot.cloud_coding_sessions.v1` / Codex/Claude sandboxed sessions)
//     plugs into; ships wired to a stub/accepting adapter so the surface is
//     exercisable end-to-end in tests
//   - metering/receipt hook seam (`SandboxMeteringHook`) for the per-second /
//     per-resource billing path; ships a no-op/log stub
//
// HONEST SCOPE: the promise `cloud.sandbox_compute_service.v1` STAYS red/planned.
// This is the typed surface + seams TOWARD a rentable, metered sandbox; NOT a
// claim it is live. No real sandbox is provisioned, no real credits move, the
// route is inert on the live Worker. A green flip requires a dereferenceable PAID
// sandbox receipt per `proof.claim_upgrade_receipts.v1`, which this scaffold does
// not (and must not) produce.

import { Effect, Schema as S } from 'effect'

import { noStoreJsonResponse } from '../http/responses'
import { workerLogEntry } from '../observability'
import { compactRandomId, currentIsoTimestamp } from '../runtime-primitives'
import {
  cloudChargeReceiptRef,
  type CloudMeteringDeps,
  type CloudMeteringOutcome,
  settleCloudPrimitiveCharge,
} from './cloud-metering'

// FLAG ---------------------------------------------------------------------
// Parse CLOUD_SANDBOX_COMPUTE_ENABLED. Default OFF: anything other than an
// explicit truthy token leaves the surface inert. Same parser shape as the
// inference + fine-tuning flags so operators have one mental model.
export const isSandboxComputeServiceEnabled = (
  value: string | undefined,
): boolean => {
  if (value === undefined) {
    return false
  }
  return ['1', 'true', 'yes', 'on'].includes(value.trim().toLowerCase())
}

// REQUEST SCHEMA -----------------------------------------------------------
// Typed intake for renting a scoped, isolated sandbox. `image` selects the
// runtime image; `ttlSeconds` bounds the rental window (an abuse/cost control);
// unknown extra options are preserved verbatim for the runtime adapter.
const SandboxRequestBody = S.Struct({
  image: S.optionalKey(S.String),
  ttlSeconds: S.optionalKey(S.Number),
})

export const DEFAULT_SANDBOX_IMAGE = 'oa-sandbox-base'
// Default + ceiling rental windows. The ceiling is a hard isolation/abuse
// control: a request over it is rejected before any provisioning, so a single
// renter cannot pin a sandbox indefinitely.
export const DEFAULT_SANDBOX_TTL_SECONDS = 900
export const MAX_SANDBOX_TTL_SECONDS = 3600

export type SandboxRequest = Readonly<{
  image: string
  ttlSeconds: number
  // Extra provisioning options forwarded verbatim to the runtime adapter.
  options: Readonly<Record<string, unknown>>
}>

// SANDBOX MODEL ------------------------------------------------------------
// The typed sandbox the surface returns and the runtime adapter accepts.
// `status` is the lifecycle the runtime drives; the scaffold only emits
// `provisioning` (the runtime moves it to ready/stopped/expired once wired).
export type SandboxStatus =
  | 'provisioning'
  | 'ready'
  | 'stopped'
  | 'expired'
  | 'failed'

export type Sandbox = Readonly<{
  sandboxId: string
  accountRef: string
  image: string
  ttlSeconds: number
  status: SandboxStatus
  // Connection ref for the rented sandbox (e.g. a scoped session URL). Null
  // until a real sandbox is provisioned. Public-safe ref, never raw creds.
  connectionRef: string | null
  createdAt: string
  expiresAtHint: string | null
}>

// RUNTIME-ADAPTER SEAM -----------------------------------------------------
// The provider/runtime seam the rest of #5517 plugs into. A real adapter wires
// this to the isolated cloud-coding session substrate. Adapters NEVER touch
// credits, payment, or public projection — that is the metering hook's job.
export type SandboxRuntimeAdapter = Readonly<{
  id: string
  provision: (
    input: Readonly<{
      sandboxId: string
      accountRef: string
      request: SandboxRequest
    }>,
  ) => Effect.Effect<Sandbox, SandboxAdapterError>
  // Lifecycle READ seam (#5517). Resolves the current state of a previously
  // provisioned sandbox for the owning account, or undefined when the sandbox is
  // unknown to this account (also the cross-account isolation point: a sandbox is
  // only visible to the account that rented it). A real adapter reads the
  // isolated-session substrate; the stub has no persistence and returns undefined.
  get: (
    input: Readonly<{ sandboxId: string; accountRef: string }>,
  ) => Effect.Effect<Sandbox | undefined, SandboxAdapterError>
}>

// Typed adapter failure so the route maps runtime problems to a stable JSON
// error instead of throwing.
export class SandboxAdapterError extends Error {
  readonly _tag = 'SandboxAdapterError'
  readonly adapterId: string
  readonly reason: string

  constructor(input: Readonly<{ adapterId: string; reason: string }>) {
    super(`[${input.adapterId}] ${input.reason}`)
    this.name = 'SandboxAdapterError'
    this.adapterId = input.adapterId
    this.reason = input.reason
  }
}

export const STUB_SANDBOX_ADAPTER_ID = 'stub-sandbox'

// Stub/accepting runtime adapter. Accepts a request and returns a
// `provisioning` sandbox with no real connection, so the route + metering seams
// are exercisable without provisioning real compute. It NEVER allocates real
// isolation, consumes compute, or exposes a usable session — `connectionRef`
// stays null. #5517 replaces dispatch to this with the real session-lane adapter.
export const stubSandboxAdapter: SandboxRuntimeAdapter = {
  id: STUB_SANDBOX_ADAPTER_ID,
  provision: ({ sandboxId, accountRef, request }) =>
    Effect.sync(
      (): Sandbox => ({
        sandboxId,
        accountRef,
        image: request.image,
        ttlSeconds: request.ttlSeconds,
        status: 'provisioning',
        connectionRef: null,
        createdAt: currentIsoTimestamp(),
        expiresAtHint: null,
      }),
    ),
  // The stub has no persistence: a provisioned sandbox is not retained, so a
  // later status read resolves to undefined (the route maps that to 404).
  // #5517's real adapter reads the isolated-session substrate.
  get: () => Effect.sync((): Sandbox | undefined => undefined),
}

// Public-safe primitive tag for sandbox charges, receipt refs, and metering
// diagnostics. Shared with the cloud metering seam (`cloud-metering.ts`).
export const SANDBOX_COMPUTE_PRIMITIVE = 'cloud.sandbox_compute.rental'

// METERING / RECEIPT HOOK SEAM --------------------------------------------
// The single typed point where #5517's billing path decrements credits for a
// sandbox rental (receipt-first, from real metered usage — never an estimate).
// Mirrors the inference `MeteringHook`. The scaffold ships a no-op/log stub.
export type SandboxMeteringContext = Readonly<{
  accountRef: string
  sandboxId: string
  image: string
  // Metered usage the charge is computed from once the rental ends (wall-seconds,
  // CPU-seconds, memory-GB-seconds, etc.). Absent at provision time (no usage yet).
  usage?: Readonly<Record<string, number>> | undefined
}>

export type SandboxMeteringOutcome = Readonly<{
  metered: boolean
  // Public-safe receipt ref when metering is live; null for the stub. Never a
  // raw amount, destination, or payment material.
  receiptRef: string | null
}>

export type SandboxMeteringHook = (
  context: SandboxMeteringContext,
) => Effect.Effect<SandboxMeteringOutcome>

// Public-safe receipt ref for a sandbox rental charge, resolvable without
// exposing any payment material. This MUST equal the `public_receipt_ref` the
// metering seam writes to the ledger (`cloudChargeReceiptRef`), so the ref the
// surface advertises is the same ref `GET /api/public/cloud/receipts/:ref`
// dereferences. (Previously this returned a `.rental.<id>` shape that did NOT
// match the ledger's `.rental.charge.<id>` row, so an advertised sandbox receipt
// could never be dereferenced.)
export const sandboxRentalReceiptRef = (sandboxId: string): string =>
  cloudChargeReceiptRef(SANDBOX_COMPUTE_PRIMITIVE, sandboxId)

// No-op stub. Logs (public-safe: account, sandbox, image only) and reports
// `metered: false`. Used on the inert path and as the default in tests.
export const stubSandboxMeteringHook: SandboxMeteringHook = context =>
  Effect.gen(function* () {
    yield* Effect.logInfo(
      workerLogEntry('cloud.sandbox_compute.metering.stub', {
        accountRef: context.accountRef,
        image: context.image,
        sandboxId: context.sandboxId,
      }),
    )
    return {
      metered: false,
      receiptRef: null,
    } satisfies SandboxMeteringOutcome
  })

// LIVE LEDGER METERING (#5517) --------------------------------------------
// The real receipt-first credit-debit hook the no-op stub becomes once a sandbox
// rental ends with REAL metered usage (wall-seconds, CPU-seconds, etc.). It
// computes the charge with an INJECTED pure pricing function (`priceUsd`, never a
// hardcoded price, never an estimate), converts to integer msat, and decrements
// the account's credit balance through the shared cloud-metering seam
// (`settleCloudPrimitiveCharge` -> `payments-ledger.ts`). Idempotent per sandbox
// id, never goes negative.
//
// HONEST SCOPE: wiring this hook does NOT make sandbox compute a live billed
// product — the scaffold keeps defaulting to the stub, and this only charges when
// a REAL rental ends with usage AND #5517 supplies both a real runtime adapter
// and a real `priceUsd`. The promise STAYS red; a green flip requires a
// dereferenceable PAID sandbox receipt + owner sign-off.
export type SandboxLedgerMeteringDeps = Readonly<
  CloudMeteringDeps & {
    priceUsd: (context: SandboxMeteringContext) => number
    usdToMsat: (chargeUsd: number) => number
  }
>

export const makeLedgerSandboxMeteringHook = (
  deps: SandboxLedgerMeteringDeps,
): SandboxMeteringHook => {
  return context =>
    Effect.gen(function* () {
      // At provision time there is no metered usage yet (the rental has not
      // ended), so there is nothing to charge — report metered:false without a
      // ledger row. The per-rental charge fires only once usage is reported.
      if (context.usage === undefined) {
        return {
          metered: false,
          receiptRef: null,
        } satisfies SandboxMeteringOutcome
      }
      const chargeMsat = Math.max(0, Math.ceil(deps.usdToMsat(deps.priceUsd(context))))
      const outcome: CloudMeteringOutcome = yield* settleCloudPrimitiveCharge(
        { db: deps.db, ...(deps.nowIso === undefined ? {} : { nowIso: deps.nowIso }) },
        {
          accountRef: context.accountRef,
          adapterId: 'sandbox-runtime',
          chargeId: context.sandboxId,
          chargeMsat,
          primitive: SANDBOX_COMPUTE_PRIMITIVE,
        },
      )
      // Project the scaffold's own public-safe receipt ref (the one this surface
      // already advertises) when the debit landed; null when it did not.
      return {
        metered: outcome.metered,
        receiptRef: outcome.metered ? sandboxRentalReceiptRef(context.sandboxId) : null,
      } satisfies SandboxMeteringOutcome
    })
}

// AUTH SEAM ----------------------------------------------------------------
export type SandboxAuth = (
  request: Request,
) => Promise<Readonly<{ accountRef: string }> | undefined>

export type SandboxComputeServiceDeps = Readonly<{
  // Whether the surface is enabled (env.CLOUD_SANDBOX_COMPUTE_ENABLED, default OFF).
  enabled: boolean
  authenticate: SandboxAuth
  // Runtime adapter. Defaults to the stub/accepting adapter.
  adapter?: SandboxRuntimeAdapter
  // Metering/receipt hook. Defaults to the no-op/log stub.
  meteringHook?: SandboxMeteringHook
  // Deterministic id injection for tests.
  newId?: () => string
}>

const decodeBody = (value: unknown) => {
  try {
    return S.decodeUnknownSync(SandboxRequestBody)(value)
  } catch {
    return undefined
  }
}

const toSandboxRequest = (
  body: typeof SandboxRequestBody.Type,
  raw: Record<string, unknown>,
): SandboxRequest => {
  const { image: _i, ttlSeconds: _t, ...rest } = raw
  const image =
    body.image === undefined || body.image.trim() === ''
      ? DEFAULT_SANDBOX_IMAGE
      : body.image.trim()
  const ttlSeconds = body.ttlSeconds ?? DEFAULT_SANDBOX_TTL_SECONDS
  return { image, ttlSeconds, options: rest }
}

// ROUTE: POST /v1/sandboxes. INERT (404) by default until the EPIC lands.
export const handleSandboxRequest = (
  request: Request,
  deps: SandboxComputeServiceDeps,
) =>
  Effect.gen(function* () {
    // INERT GATE.
    if (!deps.enabled) {
      return noStoreJsonResponse(
        { error: 'sandbox_compute_service_disabled' },
        { status: 404 },
      )
    }

    if (request.method !== 'POST') {
      return noStoreJsonResponse({ error: 'method_not_allowed' }, { status: 405 })
    }

    const session = yield* Effect.promise(() => deps.authenticate(request))
    if (session === undefined) {
      const headers = new Headers({ 'www-authenticate': 'Bearer' })
      return noStoreJsonResponse({ error: 'unauthorized' }, { headers, status: 401 })
    }

    // Read the body via the request's own JSON decoder (keeps raw JSON.parse out
    // of domain code). An empty body is valid here — every field has a default —
    // so a blank/whitespace body decodes to `{}` rather than a 400.
    const rawBody = yield* Effect.promise(async () => {
      const text = await request.text()
      if (text.trim() === '') {
        return {} as Record<string, unknown>
      }
      try {
        return (await new Response(text).json()) as Record<string, unknown>
      } catch {
        return undefined
      }
    })
    if (rawBody === undefined) {
      return noStoreJsonResponse({ error: 'invalid_json' }, { status: 400 })
    }

    const body = decodeBody(rawBody)
    if (body === undefined) {
      return noStoreJsonResponse({ error: 'invalid_request' }, { status: 400 })
    }

    const sandboxRequest = toSandboxRequest(body, rawBody)
    // ISOLATION / ABUSE CONTROL: a non-positive or over-ceiling TTL is rejected
    // before any provisioning, so a renter cannot pin a sandbox indefinitely.
    if (
      sandboxRequest.ttlSeconds <= 0 ||
      sandboxRequest.ttlSeconds > MAX_SANDBOX_TTL_SECONDS
    ) {
      return noStoreJsonResponse(
        {
          error: 'invalid_ttl',
          maxTtlSeconds: MAX_SANDBOX_TTL_SECONDS,
        },
        { status: 400 },
      )
    }

    const adapter = deps.adapter ?? stubSandboxAdapter
    const meteringHook = deps.meteringHook ?? stubSandboxMeteringHook
    const newId = deps.newId ?? (() => compactRandomId('sbx'))
    const sandboxId = newId()

    const provisioned = yield* adapter
      .provision({ sandboxId, accountRef: session.accountRef, request: sandboxRequest })
      .pipe(
        Effect.map(sandbox => ({ ok: true as const, sandbox })),
        Effect.catch(error =>
          Effect.succeed({ ok: false as const, reason: error.reason }),
        ),
      )
    if (!provisioned.ok) {
      return noStoreJsonResponse(
        { error: 'runtime_error', reason: provisioned.reason },
        { status: 502 },
      )
    }

    // Metering/receipt hook. At provision time there is no metered usage yet, so
    // the stub reports `metered: false`; a live hook records the rental-open
    // receipt (and the per-second charge once the rental ends with real usage).
    const metering = yield* meteringHook({
      accountRef: session.accountRef,
      sandboxId,
      image: sandboxRequest.image,
    })

    return noStoreJsonResponse({
      object: 'sandbox',
      id: provisioned.sandbox.sandboxId,
      image: provisioned.sandbox.image,
      ttl_seconds: provisioned.sandbox.ttlSeconds,
      status: provisioned.sandbox.status,
      connection_ref: provisioned.sandbox.connectionRef,
      created_at: provisioned.sandbox.createdAt,
      // Honest receipt projection: reports whether metering is live (stub =>
      // metered:false). NEVER claims a paid/usable sandbox result.
      metered: metering.metered,
      receipt_ref: metering.receiptRef,
    })
  })

// ROUTE: GET /v1/sandboxes/:sandboxId (lifecycle read). INERT (404) by default
// until the EPIC lands. Resolves the current state of a sandbox for the
// AUTHENTICATED account only — the adapter's `get` enforces that a sandbox is
// visible only to the account that rented it (cross-account isolation). The stub
// adapter has no persistence, so it always resolves to 404; #5517's real adapter
// reads the isolated-session substrate.
export const handleSandboxGet = (
  request: Request,
  sandboxId: string,
  deps: SandboxComputeServiceDeps,
) =>
  Effect.gen(function* () {
    // INERT GATE.
    if (!deps.enabled) {
      return noStoreJsonResponse(
        { error: 'sandbox_compute_service_disabled' },
        { status: 404 },
      )
    }

    if (request.method !== 'GET') {
      return noStoreJsonResponse({ error: 'method_not_allowed' }, { status: 405 })
    }

    if (sandboxId.trim() === '') {
      return noStoreJsonResponse({ error: 'invalid_request' }, { status: 400 })
    }

    const session = yield* Effect.promise(() => deps.authenticate(request))
    if (session === undefined) {
      const headers = new Headers({ 'www-authenticate': 'Bearer' })
      return noStoreJsonResponse({ error: 'unauthorized' }, { headers, status: 401 })
    }

    const adapter = deps.adapter ?? stubSandboxAdapter
    const resolved = yield* adapter
      .get({ sandboxId, accountRef: session.accountRef })
      .pipe(
        Effect.map(sandbox => ({ ok: true as const, sandbox })),
        Effect.catch(error =>
          Effect.succeed({ ok: false as const, reason: error.reason }),
        ),
      )
    if (!resolved.ok) {
      return noStoreJsonResponse(
        { error: 'runtime_error', reason: resolved.reason },
        { status: 502 },
      )
    }

    if (resolved.sandbox === undefined) {
      return noStoreJsonResponse({ error: 'not_found' }, { status: 404 })
    }

    return noStoreJsonResponse({
      object: 'sandbox',
      id: resolved.sandbox.sandboxId,
      image: resolved.sandbox.image,
      ttl_seconds: resolved.sandbox.ttlSeconds,
      status: resolved.sandbox.status,
      connection_ref: resolved.sandbox.connectionRef,
      created_at: resolved.sandbox.createdAt,
    })
  })
