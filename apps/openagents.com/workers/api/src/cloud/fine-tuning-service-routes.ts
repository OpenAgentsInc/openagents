// Fine-tuning service — sellable Cloud primitive SCAFFOLD (EPIC #5510, #5516).
//
// This is an MVP/scaffold that mirrors the inference-gateway pattern
// (`inference/chat-completions-routes.ts`): a typed request surface + a
// provider/adapter seam + a metering/receipt hook, flag-gated INERT by default
// so NOTHING changes in production until the rest of the build lands.
//
//   - flag-gated INERT by default (CLOUD_FINE_TUNING_ENABLED, default off)
//   - typed fine-tune job intake (base model + dataset ref -> typed job model)
//   - runtime-adapter seam (`FineTuningRuntimeAdapter`) that a real provider
//     runtime (the training lane, `training.*`) plugs into; ships wired to a
//     stub/accepting adapter so the surface is exercisable end-to-end in tests
//   - metering/receipt hook seam (`FineTuningMeteringHook`) that #5516's billing
//     path decrements credits through; ships a no-op/log stub
//
// HONEST SCOPE: the promise `cloud.fine_tuning_service.v1` STAYS red/planned.
// This module builds the typed surface + seams TOWARD a sellable service; it is
// NOT a claim the service is live. No real provider is wired here, no real
// credits move, and the route is inert on the live Worker. A green flip requires
// a dereferenceable PAID fine-tuning receipt per `proof.claim_upgrade_receipts.v1`,
// which this scaffold does not (and must not) produce.

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
// Parse the CLOUD_FINE_TUNING_ENABLED flag. Default OFF: anything other than an
// explicit truthy token leaves the surface inert. Mirrors
// `isInferenceGatewayEnabled` exactly so operators have one mental model.
export const isFineTuningServiceEnabled = (
  value: string | undefined,
): boolean => {
  if (value === undefined) {
    return false
  }
  return ['1', 'true', 'yes', 'on'].includes(value.trim().toLowerCase())
}

// REQUEST SCHEMA -----------------------------------------------------------
// Typed intake for a fine-tune job submission. `baseModel` + `datasetRef` are
// the load-bearing inputs; `suffix` names the resulting model; unknown extra
// hyperparameters are preserved verbatim for the runtime adapter to interpret.
const FineTuningJobRequestBody = S.Struct({
  baseModel: S.String,
  datasetRef: S.String,
  suffix: S.optionalKey(S.String),
})

export type FineTuningJobRequest = Readonly<{
  baseModel: string
  datasetRef: string
  suffix: string | undefined
  // Sampling/training hyperparameters forwarded verbatim to the runtime adapter.
  hyperparameters: Readonly<Record<string, unknown>>
}>

// JOB MODEL ----------------------------------------------------------------
// The typed fine-tune job the surface returns and the runtime adapter accepts.
// `status` is the lifecycle the runtime drives; the scaffold only ever emits
// `queued` (the runtime moves it to running/succeeded/failed once wired).
export type FineTuningJobStatus =
  | 'queued'
  | 'running'
  | 'succeeded'
  | 'failed'
  | 'cancelled'

export type FineTuningJob = Readonly<{
  jobId: string
  accountRef: string
  baseModel: string
  datasetRef: string
  suffix: string | undefined
  status: FineTuningJobStatus
  // Set once the runtime finishes; the model id servable through the inference
  // gateway (`/v1/chat/completions`). Null until a real run completes.
  fineTunedModel: string | null
  createdAt: string
}>

// RUNTIME-ADAPTER SEAM -----------------------------------------------------
// The provider/runtime seam the rest of #5516 plugs into. A real adapter wires
// this to the decentralized training lane (`training.*`): it accepts a typed job
// and returns the job with a runtime-assigned id + status. Adapters NEVER touch
// credits, payment, or public projection — that is the metering hook's job.
export type FineTuningRuntimeAdapter = Readonly<{
  id: string
  submit: (
    input: Readonly<{
      jobId: string
      accountRef: string
      request: FineTuningJobRequest
    }>,
  ) => Effect.Effect<FineTuningJob, FineTuningAdapterError>
  // Lifecycle READ seam (#5516). Resolves the current state of a previously
  // submitted job for the owning account, or undefined when the job is unknown
  // to this account (also the cross-account isolation point: a job is only
  // visible to the account that submitted it). A real adapter reads the training
  // lane's job store; the stub has no persistence and always returns undefined.
  get: (
    input: Readonly<{ jobId: string; accountRef: string }>,
  ) => Effect.Effect<FineTuningJob | undefined, FineTuningAdapterError>
}>

// Typed adapter failure so the route maps runtime problems to a stable JSON
// error instead of throwing. Mirrors `InferenceAdapterError`'s shape.
export class FineTuningAdapterError extends Error {
  readonly _tag = 'FineTuningAdapterError'
  readonly adapterId: string
  readonly reason: string

  constructor(input: Readonly<{ adapterId: string; reason: string }>) {
    super(`[${input.adapterId}] ${input.reason}`)
    this.name = 'FineTuningAdapterError'
    this.adapterId = input.adapterId
    this.reason = input.reason
  }
}

export const STUB_FINE_TUNING_ADAPTER_ID = 'stub-fine-tuning'

// Stub/accepting runtime adapter. Accepts a job and returns it `queued` with a
// runtime-assigned echo of the request, so the route + metering seams are
// exercisable end-to-end without a real training run. It NEVER starts a real
// run, consumes compute, or produces a servable model — `fineTunedModel` stays
// null. #5516 replaces dispatch to this with the real training-lane adapter.
export const stubFineTuningAdapter: FineTuningRuntimeAdapter = {
  id: STUB_FINE_TUNING_ADAPTER_ID,
  submit: ({ jobId, accountRef, request }) =>
    Effect.sync(
      (): FineTuningJob => ({
        jobId,
        accountRef,
        baseModel: request.baseModel,
        datasetRef: request.datasetRef,
        suffix: request.suffix,
        status: 'queued',
        fineTunedModel: null,
        createdAt: currentIsoTimestamp(),
      }),
    ),
  // The stub has no persistence: a submitted job is not retained, so a later
  // status read resolves to undefined (the route maps that to 404). #5516's real
  // adapter reads the training-lane job store.
  get: () => Effect.sync((): FineTuningJob | undefined => undefined),
}

// Public-safe primitive tag for fine-tuning charges, receipt refs, and metering
// diagnostics. Shared with the cloud metering seam (`cloud-metering.ts`).
export const FINE_TUNING_PRIMITIVE = 'cloud.fine_tuning.job'

// METERING / RECEIPT HOOK SEAM --------------------------------------------
// The single typed point where #5516's billing path decrements credits for a
// fine-tune job (receipt-first, from real runtime usage — never an estimate).
// Mirrors the inference `MeteringHook`. The scaffold ships a no-op/log stub that
// reports `metered: false`; a live hook wires to the same PayIn-shaped credit
// ledger the inference metering hook uses (`payments-ledger.ts`).
export type FineTuningMeteringContext = Readonly<{
  accountRef: string
  jobId: string
  baseModel: string
  // Runtime usage the charge is computed from once a real run completes
  // (trained tokens, GPU-seconds, etc.). Absent at intake (no run yet).
  usage?: Readonly<Record<string, number>> | undefined
}>

export type FineTuningMeteringOutcome = Readonly<{
  metered: boolean
  // Public-safe receipt ref when metering is live; null for the stub. Never a
  // raw amount, destination, or payment material.
  receiptRef: string | null
}>

export type FineTuningMeteringHook = (
  context: FineTuningMeteringContext,
) => Effect.Effect<FineTuningMeteringOutcome>

// Public-safe receipt ref for a fine-tune job charge, resolvable without
// exposing any payment material. This MUST equal the `public_receipt_ref` the
// metering seam writes to the ledger (`cloudChargeReceiptRef`), so the ref the
// surface advertises is the same ref `GET /api/public/cloud/receipts/:ref`
// dereferences. (Previously this returned a `.job.<id>` shape that did NOT match
// the ledger's `.job.charge.<id>` row, so an advertised fine-tune receipt could
// never be dereferenced.)
export const fineTuningJobReceiptRef = (jobId: string): string =>
  cloudChargeReceiptRef(FINE_TUNING_PRIMITIVE, jobId)

// No-op stub. Logs (public-safe: account, job, model only) and reports
// `metered: false`. Used on the inert path and as the default in tests.
export const stubFineTuningMeteringHook: FineTuningMeteringHook = context =>
  Effect.gen(function* () {
    yield* Effect.logInfo(
      workerLogEntry('cloud.fine_tuning.metering.stub', {
        accountRef: context.accountRef,
        baseModel: context.baseModel,
        jobId: context.jobId,
      }),
    )
    return {
      metered: false,
      receiptRef: null,
    } satisfies FineTuningMeteringOutcome
  })

// LIVE LEDGER METERING (#5516) --------------------------------------------
// The real receipt-first credit-debit hook the no-op stub becomes once a
// fine-tune job reports REAL runtime usage. It computes the charge with an
// INJECTED pure pricing function (`priceUsd`, never a hardcoded price, never an
// estimate) from the runtime usage, converts to integer msat, and decrements the
// account's credit balance through the shared cloud-metering seam
// (`settleCloudPrimitiveCharge` -> `payments-ledger.ts`). Idempotent per job id,
// never goes negative.
//
// HONEST SCOPE: wiring this hook does NOT make fine-tuning a live billed product
// — the scaffold keeps defaulting to the stub, and this only ever charges when a
// REAL job completes with usage AND #5516 supplies both a real runtime adapter
// and a real `priceUsd`. The promise STAYS red; a green flip requires a
// dereferenceable PAID fine-tuning receipt + owner sign-off.
export type FineTuningLedgerMeteringDeps = Readonly<
  CloudMeteringDeps & {
    // Pure pricing: REAL runtime usage -> USD charge. No default — a live hook
    // MUST supply the real price basis; there is no implicit pricing.
    priceUsd: (
      context: FineTuningMeteringContext,
    ) => number
    // USD -> integer msat. Shares the inference gateway's single-source
    // conversion so a cloud charge and an inference charge convert identically.
    usdToMsat: (chargeUsd: number) => number
  }
>

export const makeLedgerFineTuningMeteringHook = (
  deps: FineTuningLedgerMeteringDeps,
): FineTuningMeteringHook => {
  return context =>
    Effect.gen(function* () {
      // At intake there is no runtime usage yet (no run), so there is nothing to
      // charge — report metered:false without writing a ledger row. The per-run
      // charge fires only once the runtime reports real usage on this job.
      if (context.usage === undefined) {
        return {
          metered: false,
          receiptRef: null,
        } satisfies FineTuningMeteringOutcome
      }
      const chargeMsat = Math.max(0, Math.ceil(deps.usdToMsat(deps.priceUsd(context))))
      const outcome: CloudMeteringOutcome = yield* settleCloudPrimitiveCharge(
        { db: deps.db, ...(deps.nowIso === undefined ? {} : { nowIso: deps.nowIso }) },
        {
          accountRef: context.accountRef,
          adapterId: 'fine-tuning-runtime',
          chargeId: context.jobId,
          chargeMsat,
          primitive: FINE_TUNING_PRIMITIVE,
        },
      )
      // Project the scaffold's own public-safe receipt ref (the one this surface
      // already advertises) when the debit landed; null when it did not.
      return {
        metered: outcome.metered,
        receiptRef: outcome.metered ? fineTuningJobReceiptRef(context.jobId) : null,
      } satisfies FineTuningMeteringOutcome
    })
}

// AUTH SEAM ----------------------------------------------------------------
// Resolves the per-account API key to an account ref. Returns undefined when the
// key is missing/invalid. The Worker wires this to the same programmatic-agent
// auth the inference gateway uses; tests inject a fake.
export type FineTuningAuth = (
  request: Request,
) => Promise<Readonly<{ accountRef: string }> | undefined>

export type FineTuningServiceDeps = Readonly<{
  // Whether the surface is enabled (env.CLOUD_FINE_TUNING_ENABLED, default OFF).
  enabled: boolean
  authenticate: FineTuningAuth
  // Runtime adapter. Defaults to the stub/accepting adapter.
  adapter?: FineTuningRuntimeAdapter
  // Metering/receipt hook. Defaults to the no-op/log stub.
  meteringHook?: FineTuningMeteringHook
  // Deterministic id injection for tests.
  newId?: () => string
}>

const decodeBody = (value: unknown) => {
  try {
    return S.decodeUnknownSync(FineTuningJobRequestBody)(value)
  } catch {
    return undefined
  }
}

const toJobRequest = (
  body: typeof FineTuningJobRequestBody.Type,
  raw: Record<string, unknown>,
): FineTuningJobRequest => {
  const { baseModel: _b, datasetRef: _d, suffix: _s, ...rest } = raw
  return {
    baseModel: body.baseModel,
    datasetRef: body.datasetRef,
    suffix: body.suffix,
    hyperparameters: rest,
  }
}

// ROUTE: POST /v1/fine_tuning/jobs (OpenAI-shaped path so off-the-shelf clients
// work by changing only base URL + key once the service is live). INERT (404)
// by default until the EPIC lands.
export const handleFineTuningJobSubmit = (
  request: Request,
  deps: FineTuningServiceDeps,
) =>
  Effect.gen(function* () {
    // INERT GATE.
    if (!deps.enabled) {
      return noStoreJsonResponse(
        { error: 'fine_tuning_service_disabled' },
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

    const rawBody = yield* Effect.promise(async () => {
      try {
        return (await request.json()) as Record<string, unknown>
      } catch {
        return undefined
      }
    })
    if (rawBody === undefined) {
      return noStoreJsonResponse({ error: 'invalid_json' }, { status: 400 })
    }

    const body = decodeBody(rawBody)
    if (
      body === undefined ||
      body.baseModel.trim() === '' ||
      body.datasetRef.trim() === ''
    ) {
      return noStoreJsonResponse({ error: 'invalid_request' }, { status: 400 })
    }

    const adapter = deps.adapter ?? stubFineTuningAdapter
    const meteringHook = deps.meteringHook ?? stubFineTuningMeteringHook
    const newId = deps.newId ?? (() => compactRandomId('ftjob'))
    const jobId = newId()
    const jobRequest = toJobRequest(body, rawBody)

    const submitted = yield* adapter
      .submit({ jobId, accountRef: session.accountRef, request: jobRequest })
      .pipe(
        Effect.map(job => ({ ok: true as const, job })),
        Effect.catch(error =>
          Effect.succeed({ ok: false as const, reason: error.reason }),
        ),
      )
    if (!submitted.ok) {
      return noStoreJsonResponse(
        { error: 'runtime_error', reason: submitted.reason },
        { status: 502 },
      )
    }

    // Metering/receipt hook. At intake there is no runtime usage yet, so the
    // stub reports `metered: false`; a live hook records the job-level intake
    // receipt (and the per-run charge once the runtime reports usage).
    const metering = yield* meteringHook({
      accountRef: session.accountRef,
      jobId,
      baseModel: jobRequest.baseModel,
    })

    return noStoreJsonResponse({
      object: 'fine_tuning.job',
      id: submitted.job.jobId,
      model: submitted.job.baseModel,
      status: submitted.job.status,
      fine_tuned_model: submitted.job.fineTunedModel,
      created_at: submitted.job.createdAt,
      // Honest receipt projection: the scaffold reports whether metering is live
      // (stub => metered:false). It NEVER claims a paid/servable result.
      metered: metering.metered,
      receipt_ref: metering.receiptRef,
    })
  })

// ROUTE: GET /v1/fine_tuning/jobs/:jobId (OpenAI-shaped lifecycle read). INERT
// (404) by default until the EPIC lands. Resolves the current state of a job for
// the AUTHENTICATED account only — the adapter's `get` enforces that a job is
// visible only to the account that submitted it (cross-account isolation). The
// stub adapter has no persistence, so it always resolves to 404; #5516's real
// adapter reads the training-lane job store.
export const handleFineTuningJobGet = (
  request: Request,
  jobId: string,
  deps: FineTuningServiceDeps,
) =>
  Effect.gen(function* () {
    // INERT GATE.
    if (!deps.enabled) {
      return noStoreJsonResponse(
        { error: 'fine_tuning_service_disabled' },
        { status: 404 },
      )
    }

    if (request.method !== 'GET') {
      return noStoreJsonResponse({ error: 'method_not_allowed' }, { status: 405 })
    }

    if (jobId.trim() === '') {
      return noStoreJsonResponse({ error: 'invalid_request' }, { status: 400 })
    }

    const session = yield* Effect.promise(() => deps.authenticate(request))
    if (session === undefined) {
      const headers = new Headers({ 'www-authenticate': 'Bearer' })
      return noStoreJsonResponse({ error: 'unauthorized' }, { headers, status: 401 })
    }

    const adapter = deps.adapter ?? stubFineTuningAdapter
    const resolved = yield* adapter
      .get({ jobId, accountRef: session.accountRef })
      .pipe(
        Effect.map(job => ({ ok: true as const, job })),
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

    if (resolved.job === undefined) {
      return noStoreJsonResponse({ error: 'not_found' }, { status: 404 })
    }

    return noStoreJsonResponse({
      object: 'fine_tuning.job',
      id: resolved.job.jobId,
      model: resolved.job.baseModel,
      status: resolved.job.status,
      fine_tuned_model: resolved.job.fineTunedModel,
      created_at: resolved.job.createdAt,
    })
  })
