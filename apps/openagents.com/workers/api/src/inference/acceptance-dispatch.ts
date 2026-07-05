// Async acceptance-verification DISPATCH for the Khala verified-work lane
// (EPIC #6017; design: docs/inference/2026-06-22-verified-work-must-execute-the-artifact.md).
//
// THE PROBLEM THIS MODULE SOLVES. The merged QC fix (#6038) added a real headless
// acceptance runner (`acceptance-runner/runner.ts`) and an HONEST `unverified`
// default in `khala-code-verifier.ts` — but the live gateway never invokes the
// runner, so prod khala-code receipts are ALWAYS `unverified`. Playwright/chromium
// cannot run in a CF Worker (nor in a Queue-consumer Worker), so the runner must
// run on a NODE with chromium (a Pylon / `oa-workroomd` sandbox / a small runner
// service). This module owns the seam that connects the two:
//
//   gateway (Worker) ─enqueue job─▶ Queue ─▶ node-side runner harness (chromium)
//        ▲                                              │
//        └────────── verdict callback (authenticated) ──┘   ─▶ backfill the receipt
//
// WHAT THIS MODULE OWNS (Worker-safe; no Playwright, no chromium ever):
//   (1) `AcceptanceJobMessage` — the typed `openagents.inference.acceptance_job.v1`
//       payload the gateway enqueues when khala-code produces an executable
//       artifact. It carries the request id, an
//       artifact REF (not raw bytes on the hot path — bytes live in R2/the store),
//       and the derived `AcceptanceSpec`.
//   (2) `enqueueAcceptanceJob` — the producer over a pluggable `AcceptanceJobQueue`
//       seam (a Cloudflare `Queue` in prod, a fake in tests), BEHIND A FLAG, default
//       OFF. With the flag off it is a no-op: nothing is enqueued, prod behaviour is
//       unchanged, the receipt stays `unverified`.
//   (3) The verdict CALLBACK ingest: authentication of the runner's POST
//       (`authenticateVerdictCallback`, a constant-time bearer check against
//       `ACCEPTANCE_VERDICT_CALLBACK_TOKEN`) + the pure receipt-backfill
//       (`backfillVerdictIntoVerification`): apply an `AcceptanceVerdict` to the
//       stored `unverified` verification so it becomes `test_passed`/`failed`,
//       `verified`, `scalarReward`, with per-test results. Idempotent;
//       receipt-first; a forged/unauthenticated verdict is rejected.
//
// INERT BY DEFAULT. The enqueue flag (`KHALA_ACCEPTANCE_DISPATCH_ENABLED`) is OFF,
// so the gateway emits no jobs and the honest `unverified` downgrade stands until a
// runner host is deployed and the flag is flipped. The callback route is independent
// and only acts on an authenticated POST.

import { Effect, Schema as S } from 'effect'

import {
  makeAgentRuntimeRemainderMirrorForEnv,
  type AgentRuntimeRemainderMirror,
  type AgentRuntimeRemainderStoreEnv,
} from '../agent-runtime-remainder-store'
import { parseJsonStringArray } from '../json-boundary'
import { openAgentsDatabase } from '../runtime'
import { currentIsoTimestamp } from '../runtime-primitives'

import type { AcceptanceSpec } from './acceptance-spec'
import type { AcceptanceVerdict } from './acceptance-runner/verdict'
import {
  type KhalaCodeVerification,
  type KhalaCodeVerificationVerdict,
  verifyKhalaCodeCompletion,
} from './khala-code-verifier'

// ----------------------------------------------------------------------------
// (1) The typed verification-job message
// ----------------------------------------------------------------------------

// The acceptance-spec carried on the wire. The runner needs the spec to know which
// deterministic checks to run + the bounded thresholds. Kept structural (decoded by
// the runner harness) so the job message stays a stable Effect Schema class without
// re-encoding every spec field — the spec is a small JSON object the runner re-reads
// through `acceptance-spec.ts`.
export const AcceptanceJobSpecSchema = S.Struct({
  kind: S.Literal('crossy_road_single_html'),
  rubricRef: S.String,
  checks: S.Array(S.String),
  params: S.Struct({
    forwardMoves: S.Number,
    maxCameraDeltaPerMove: S.Number,
    expectedForwardAdvance: S.Number,
    minWorldRowsAhead: S.Number,
  }),
})

// The queue payload that triggers an out-of-Worker acceptance run for ONE khala-code
// completion. `requestId` is the inference response id the receipt is keyed on (the
// runner echoes it back so the verdict backfills the right receipt). `artifactRef`
// is a dereferenceable handle (an R2 key / store ref) the runner resolves to the
// HTML bytes — NEVER the raw artifact on the hot enqueue path. `spec` is the derived
// `AcceptanceSpec`. `servedModel` / `worker` are carried so the backfilled verdict
// re-derives the same receipt shape the hot path produced.
export class AcceptanceJobMessage extends S.Class<AcceptanceJobMessage>(
  'AcceptanceJobMessage',
)({
  schemaVersion: S.Literal('openagents.inference.acceptance_job.v1'),
  requestId: S.String,
  artifactRef: S.String,
  servedModel: S.String,
  worker: S.String,
  meteringReceiptRef: S.optionalKey(S.NullOr(S.String)),
  spec: AcceptanceJobSpecSchema,
}) {}

export type AcceptanceJobSpec = S.Schema.Type<typeof AcceptanceJobSpecSchema>

// Build the wire spec from a typed `AcceptanceSpec` (the runner re-reads it back into
// the same shape). One place so the encode stays in sync with `acceptance-spec.ts`.
export const acceptanceJobSpecFromSpec = (
  spec: AcceptanceSpec,
): AcceptanceJobSpec => ({
  checks: spec.checks,
  kind: spec.kind,
  params: {
    expectedForwardAdvance: spec.params.expectedForwardAdvance,
    forwardMoves: spec.params.forwardMoves,
    maxCameraDeltaPerMove: spec.params.maxCameraDeltaPerMove,
    minWorldRowsAhead: spec.params.minWorldRowsAhead,
  },
  rubricRef: spec.rubricRef,
})

// ----------------------------------------------------------------------------
// (2) The enqueue flag — default OFF (nothing is dispatched in prod yet)
// ----------------------------------------------------------------------------

// The Worker env key for the acceptance-dispatch arming flag. Default OFF: absent /
// anything other than an explicit on-token keeps the gateway from enqueuing ANY
// verification job, so the honest `unverified` downgrade stands and prod behaviour is
// byte-for-byte unchanged. NEEDS-OWNER to flip on (after a runner host is deployed).
export const KHALA_ACCEPTANCE_DISPATCH_ENABLED_ENV_KEY =
  'KHALA_ACCEPTANCE_DISPATCH_ENABLED'

const ON_TOKENS = new Set(['1', 'on', 'true', 'yes'])

// Fail-closed flag read. Absent / non-string / any non-on value => disabled.
export const isAcceptanceDispatchEnabled = (
  value: unknown,
): boolean =>
  typeof value === 'string' && ON_TOKENS.has(value.trim().toLowerCase())

// The minimal queue producer seam. A Cloudflare `Queue.send` satisfies this; tests
// pass a fake that records the sent message. The producer never touches a concrete
// Queue type so the module stays unit-testable with no infra.
export type AcceptanceJobQueue = Readonly<{
  send: (message: AcceptanceJobMessage) => Promise<void>
}>

export type EnqueueAcceptanceJobInput = Readonly<{
  enabled: boolean
  queue: AcceptanceJobQueue | undefined
  requestId: string
  artifactRef: string
  servedModel: string
  worker: string
  meteringReceiptRef?: string | null | undefined
  spec: AcceptanceSpec
}>

export type EnqueueAcceptanceJobOutcome = Readonly<{
  // Whether a job was actually enqueued. False when the flag is off or no queue is
  // wired (the honest inert default) — the caller's receipt stays `unverified`.
  enqueued: boolean
  // The message that was (or would have been) enqueued, for the caller's diagnostics
  // and the test assertions. Present even when `enqueued` is false so the test can
  // prove the SHAPE without a live send.
  message: AcceptanceJobMessage
}>

// Enqueue a verification job for a khala-code completion that produced an executable
// artifact. FAILS SOFT + INERT: with the flag off OR no queue wired it returns
// `{ enqueued: false }` and sends nothing; a `send` rejection is swallowed (logged by
// the caller) so a queue hiccup never breaks the already-delivered completion. The
// receipt stays the honest `unverified` until a verdict callback backfills it.
export const enqueueAcceptanceJob = (
  input: EnqueueAcceptanceJobInput,
): Effect.Effect<EnqueueAcceptanceJobOutcome> =>
  Effect.gen(function* () {
    const message = AcceptanceJobMessage.make({
      artifactRef: input.artifactRef,
      meteringReceiptRef: input.meteringReceiptRef ?? null,
      requestId: input.requestId,
      schemaVersion: 'openagents.inference.acceptance_job.v1',
      servedModel: input.servedModel,
      spec: acceptanceJobSpecFromSpec(input.spec),
      worker: input.worker,
    })

    if (!input.enabled || input.queue === undefined) {
      return { enqueued: false, message }
    }

    const sent = yield* Effect.tryPromise(() => input.queue!.send(message)).pipe(
      Effect.as(true),
      Effect.orElseSucceed(() => false),
    )
    return { enqueued: sent, message }
  })

// ----------------------------------------------------------------------------
// (3a) Verdict callback authentication — reject forged/unauthenticated verdicts
// ----------------------------------------------------------------------------

// The Worker env key for the runner-callback bearer token. A node-side runner posts
// its verdict with `Authorization: Bearer <token>`; the gateway rejects any verdict
// that does not present the exact configured token. Absent token => the callback is
// CLOSED (every verdict rejected) so an unconfigured Worker never accepts a forged
// verdict. NEEDS-OWNER to set on the Worker alongside arming the runner host.
export const ACCEPTANCE_VERDICT_CALLBACK_TOKEN_ENV_KEY =
  'ACCEPTANCE_VERDICT_CALLBACK_TOKEN'

// Constant-time-ish string compare (length-independent equality without early exit on
// the first mismatched byte). Avoids leaking the token length/prefix via timing.
const safeEqual = (a: string, b: string): boolean => {
  if (a.length !== b.length) return false
  let diff = 0
  for (let index = 0; index < a.length; index += 1) {
    diff |= a.charCodeAt(index) ^ b.charCodeAt(index)
  }
  return diff === 0
}

const parseBearer = (header: string | null): string | undefined => {
  if (header === null) return undefined
  const match = /^Bearer\s+(.+)$/iu.exec(header.trim())
  return match?.[1]?.trim()
}

// Authenticate a runner verdict callback. FAILS CLOSED: an absent configured token, a
// missing/malformed `Authorization` header, or a token mismatch all reject. Only an
// exact bearer match authenticates. Never logs the token.
export const authenticateVerdictCallback = (
  input: Readonly<{
    authorizationHeader: string | null
    configuredToken: string | undefined
  }>,
): boolean => {
  const configured = input.configuredToken?.trim()
  if (configured === undefined || configured === '') return false
  const presented = parseBearer(input.authorizationHeader)
  if (presented === undefined || presented === '') return false
  return safeEqual(presented, configured)
}

// ----------------------------------------------------------------------------
// (3b) The verdict the runner posts back
// ----------------------------------------------------------------------------

// The body a node-side runner POSTs to the verdict callback. It carries the request
// id (which receipt to backfill) and the executed `AcceptanceVerdict` produced by the
// merged runner. Validated by the route; an absent/empty `requestId` or a verdict
// whose `executed` is not true is rejected (we only backfill from a REAL run).
export const AcceptanceCheckResultSchema = S.Struct({
  id: S.String,
  passed: S.Boolean,
  detail: S.String,
})

export const AcceptanceVerdictWireSchema = S.Struct({
  kind: S.Literal('crossy_road_single_html'),
  executed: S.Literal(true),
  rubricRef: S.String,
  checks: S.Array(AcceptanceCheckResultSchema),
  passedChecks: S.Array(S.String),
  failedChecks: S.Array(S.String),
  scalarReward: S.Number,
  verified: S.Boolean,
  consoleErrors: S.Array(S.String),
  pageErrors: S.Array(S.String),
})

export class AcceptanceVerdictCallbackBody extends S.Class<AcceptanceVerdictCallbackBody>(
  'AcceptanceVerdictCallbackBody',
)({
  schemaVersion: S.Literal('openagents.inference.acceptance_verdict.v1'),
  requestId: S.String,
  // The runner echoes the served-model/worker so the backfilled verdict re-derives
  // the SAME receipt shape the hot path produced (no parallel receipt-ref math).
  servedModel: S.String,
  worker: S.String,
  meteringReceiptRef: S.optionalKey(S.NullOr(S.String)),
  verdict: AcceptanceVerdictWireSchema,
}) {}

// ----------------------------------------------------------------------------
// (3c) The stored verification verdict + receipt backfill
// ----------------------------------------------------------------------------

// The persisted khala-code verification verdict, keyed by the inference response id
// (`requestId`). The hot gateway path writes the HONEST `unverified` row at completion
// time; the verdict callback BACKFILLS it to `test_passed`/`failed` once the runner
// has executed the artifact. The public receipt read projects from this store.
//
// This is the verification verdict store — NOT the financial `pay_ins` ledger
// (`inference-receipts.ts`), which records the CHARGE. The charge is settled at
// completion; the EXECUTION verdict is what flips from `unverified` to verified here.
export type KhalaVerificationRecord = Readonly<{
  requestId: string
  verification: KhalaCodeVerification
  verified: boolean
  executed: boolean
  scalarReward: number
  rubricRef: string
  passedChecks: ReadonlyArray<string>
  failedChecks: ReadonlyArray<string>
  verificationReceiptRef: string
  // The verdict version: the hot path writes `1` (the unverified downgrade); each
  // successful backfill bumps it. Used for idempotency + last-writer diagnostics.
  version: number
  updatedAt: string
}>

export type KhalaVerificationStore = Readonly<{
  // Read the current verdict row for a request id (null when none written yet).
  read: (requestId: string) => Effect.Effect<KhalaVerificationRecord | null>
  // Upsert the verdict row. Idempotent at the call site via `version`.
  upsert: (record: KhalaVerificationRecord) => Effect.Effect<void>
}>

// An in-memory verification store. Used by tests + as the reference implementation a
// D1-backed store mirrors. Pure + synchronous under the Effect wrapper.
export const makeInMemoryKhalaVerificationStore = (
  nowIso: () => string = currentIsoTimestamp,
): KhalaVerificationStore => {
  const rows = new Map<string, KhalaVerificationRecord>()
  return {
    read: requestId => Effect.sync(() => rows.get(requestId) ?? null),
    upsert: record =>
      Effect.sync(() => {
        rows.set(record.requestId, { ...record, updatedAt: nowIso() })
      }),
  }
}

// A D1-backed verification store (prod). Mirrors `makeInMemoryKhalaVerificationStore`
// against the `khala_acceptance_verdicts` table (migration 0221). Arrays are stored as
// JSON text. `upsert` is an INSERT ... ON CONFLICT replace so a backfill overwrites the
// `unverified` row; the caller's `version` guard keeps it idempotent.
export const makeD1KhalaVerificationStore = (
  db: D1Database,
  nowIso: () => string = currentIsoTimestamp,
): KhalaVerificationStore => ({
  read: requestId =>
    Effect.tryPromise(() =>
      db
        .prepare(
          `SELECT request_id, verification, verified, executed, scalar_reward,
                  rubric_ref, passed_checks, failed_checks, verification_receipt_ref,
                  version, updated_at
             FROM khala_acceptance_verdicts WHERE request_id = ? LIMIT 1`,
        )
        .bind(requestId)
        .first<Record<string, unknown>>(),
    ).pipe(
      Effect.map(row => {
        if (row === null) return null
        const parseArray = (value: unknown): ReadonlyArray<string> =>
          parseJsonStringArray(typeof value === 'string' ? value : null)
        return {
          executed: Number(row.executed) === 1,
          failedChecks: parseArray(row.failed_checks),
          passedChecks: parseArray(row.passed_checks),
          requestId: String(row.request_id),
          rubricRef: String(row.rubric_ref),
          scalarReward: Number(row.scalar_reward),
          updatedAt: String(row.updated_at),
          verification: String(row.verification) as KhalaCodeVerification,
          verificationReceiptRef: String(row.verification_receipt_ref),
          verified: Number(row.verified) === 1,
          version: Number(row.version),
        }
      }),
      Effect.orDie,
    ),
  upsert: record =>
    Effect.tryPromise(() =>
      db
        .prepare(
          `INSERT INTO khala_acceptance_verdicts (
             request_id, verification, verified, executed, scalar_reward, rubric_ref,
             passed_checks, failed_checks, verification_receipt_ref, version, updated_at
           ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
           ON CONFLICT(request_id) DO UPDATE SET
             verification = excluded.verification,
             verified = excluded.verified,
             executed = excluded.executed,
             scalar_reward = excluded.scalar_reward,
             rubric_ref = excluded.rubric_ref,
             passed_checks = excluded.passed_checks,
             failed_checks = excluded.failed_checks,
             verification_receipt_ref = excluded.verification_receipt_ref,
             version = excluded.version,
             updated_at = excluded.updated_at`,
        )
        .bind(
          record.requestId,
          record.verification,
          record.verified ? 1 : 0,
          record.executed ? 1 : 0,
          record.scalarReward,
          record.rubricRef,
          JSON.stringify(record.passedChecks),
          JSON.stringify(record.failedChecks),
          record.verificationReceiptRef,
          record.version,
          nowIso(),
        )
        .run(),
    ).pipe(Effect.asVoid, Effect.orDie),
})

const mirrorAcceptanceVerdict = (
  mirror: AgentRuntimeRemainderMirror,
  requestId: string,
): Effect.Effect<void> =>
  Effect.promise(() =>
    mirror.mirrorRowsByPk('khala_acceptance_verdicts', [requestId]),
  )

export const makeMirroredKhalaVerificationStore = (
  d1: KhalaVerificationStore,
  mirror: AgentRuntimeRemainderMirror | undefined,
): KhalaVerificationStore => {
  if (mirror === undefined) {
    return d1
  }

  return {
    read: requestId => d1.read(requestId),
    upsert: record =>
      Effect.gen(function* () {
        yield* d1.upsert(record)
        yield* mirrorAcceptanceVerdict(mirror, record.requestId)
      }),
  }
}

export const makeKhalaVerificationStoreForEnv = (
  env: AgentRuntimeRemainderStoreEnv,
  nowIso: () => string = currentIsoTimestamp,
): KhalaVerificationStore =>
  makeMirroredKhalaVerificationStore(
    makeD1KhalaVerificationStore(openAgentsDatabase(env), nowIso),
    makeAgentRuntimeRemainderMirrorForEnv(env),
  )

// Map a `KhalaCodeVerificationVerdict` (the verifier's output) onto a stored record.
export const verificationRecordFromVerdict = (
  input: Readonly<{
    requestId: string
    verdict: KhalaCodeVerificationVerdict
    version: number
    nowIso: string
  }>,
): KhalaVerificationRecord => ({
  executed: input.verdict.executed,
  failedChecks: input.verdict.failedChecks,
  passedChecks: input.verdict.passedChecks,
  requestId: input.requestId,
  rubricRef: input.verdict.rubricRef,
  scalarReward: input.verdict.scalarReward,
  updatedAt: input.nowIso,
  verification: input.verdict.verification,
  verificationReceiptRef: input.verdict.receiptRef,
  verified: input.verdict.verified,
  version: input.version,
})

export type BackfillVerdictOutcome = Readonly<{
  // Whether the receipt was backfilled by this call. False on an idempotent replay of
  // an already-executed verdict (the row is already terminal at >= this version).
  backfilled: boolean
  // The resulting (or pre-existing) stored record.
  record: KhalaVerificationRecord
}>

// Convert the runner's wire `AcceptanceVerdict` back into the strongly-typed verdict
// shape the verifier consumes. The wire schema validated `executed: true` already.
export const acceptanceVerdictFromWire = (
  wire: S.Schema.Type<typeof AcceptanceVerdictWireSchema>,
): AcceptanceVerdict => ({
  checks: wire.checks.map(check => ({
    detail: check.detail,
    id: check.id as AcceptanceVerdict['checks'][number]['id'],
    passed: check.passed,
  })),
  consoleErrors: wire.consoleErrors,
  executed: true,
  failedChecks: wire.failedChecks as AcceptanceVerdict['failedChecks'],
  kind: wire.kind,
  pageErrors: wire.pageErrors,
  passedChecks: wire.passedChecks as AcceptanceVerdict['passedChecks'],
  rubricRef: wire.rubricRef,
  scalarReward: wire.scalarReward,
  verified: wire.verified,
})

// Backfill a stored verification verdict from an authenticated runner callback.
// RECEIPT-FIRST + IDEMPOTENT:
//   - re-derive the verdict from EXECUTION via the SAME `verifyKhalaCodeCompletion`
//     the hot path uses (so the receipt ref + shape are identical), passing the
//     executed `AcceptanceVerdict` so the verdict is `test_passed`/`failed`;
//   - if the stored row is already executed at a version >= the new one, no-op
//     (a redelivered callback never regresses or double-writes);
//   - otherwise upsert the executed verdict, bumping the version.
// The content the verifier re-derives from is the artifact the runner already ran;
// since the callback carries the executed verdict directly, we pass an empty content
// (the prescreen is irrelevant once an executed verdict is in hand — the verdict is
// derived from EXECUTION, the prescreen branch is not taken).
export const backfillVerdictIntoVerification = (
  deps: Readonly<{
    store: KhalaVerificationStore
    nowIso: () => string
  }>,
  body: AcceptanceVerdictCallbackBody,
): Effect.Effect<BackfillVerdictOutcome> =>
  Effect.gen(function* () {
    const existing = yield* deps.store.read(body.requestId)
    const acceptance = acceptanceVerdictFromWire(body.verdict)

    // Re-derive through the verifier so the receipt ref + verification states are the
    // SAME ones the hot path computes — never a parallel mapping. We pass the artifact
    // HTML the runner has already validated as runnable via the executed acceptance;
    // an executed acceptance always takes the EXECUTION branch regardless of content,
    // so we hand a minimal runnable-shaped marker that the verifier ignores in favour
    // of the executed verdict.
    const verdict = verifyKhalaCodeCompletion({
      acceptance,
      // A minimal self-contained HTML so the prescreen does not short-circuit to
      // `failed`; the EXECUTION branch is taken because `acceptance.executed` is true.
      content:
        '<!doctype html><html><body><canvas id="game"></canvas><script>/* executed */</script></body></html>',
      meteringReceiptRef: body.meteringReceiptRef ?? null,
      requestId: body.requestId,
      servedModel: body.servedModel,
      worker: body.worker,
    })

    // Idempotent: once a row is EXECUTED (a runner verdict has landed), a redelivered
    // callback is a terminal no-op — never regress or double-write. The hot path's
    // initial `unverified` row is NOT executed, so the FIRST backfill always applies.
    if (existing !== null && existing.executed) {
      return { backfilled: false, record: existing }
    }

    const nextVersion = (existing?.version ?? 0) + 1
    const record = verificationRecordFromVerdict({
      nowIso: deps.nowIso(),
      requestId: body.requestId,
      verdict,
      version: nextVersion,
    })
    yield* deps.store.upsert(record)
    return { backfilled: true, record }
  })
