import {
  CollectionName,
  CursorGap,
  EntityId,
  IsoTimestamp,
  MutationId,
  type SyncCommand,
  SyncMutationAccepted,
  SyncMutationRejected,
  SyncPatch,
  SyncScope,
  SyncSequence,
  SyncSnapshot,
} from '@openagentsinc/sync-schema'
import { Effect, Layer, Schema as S } from 'effect'
import * as Context from 'effect/Context'

import { decodeJsonValueEffect } from './json-boundary'
import { currentIsoTimestamp } from './runtime-primitives'

export type WorkerBindings = Readonly<{
  OPENAGENTS_DB: D1Database
  SYNC_ROOM: DurableObjectNamespace
  MDK_SIDECAR: DurableObjectNamespace
  MDK_TREASURY?: DurableObjectNamespace
  // NOTE: the former INFERENCE_DURABLE_STREAM DO binding (#6058) was deleted
  // in CFG-6 (#8521): durable inference streams are Postgres-backed via the
  // KHALA_SYNC_DB Hyperdrive binding below.
  // GLM internal-stress scheduler (#6318). Optional SQLite-class DO namespace;
  // one named DO coordinates short-lived `internal_stress` leases so external
  // demand can preempt stress across Worker isolates. Absent => same-isolate
  // in-memory preemption only.
  GLM_STRESS_SCHEDULER?: DurableObjectNamespace
  // Khala Sync Hyperdrive binding (KS-0.2, #8284). Optional: absent until the
  // wrangler `hyperdrive` binding is deployed. Worker request paths reach the
  // Khala Sync Cloud SQL Postgres ONLY through this transaction-mode pool
  // (docs/khala-sync/SPEC.md §4); typed structurally so packages do not need
  // the workers-types `Hyperdrive` ambient.
  KHALA_SYNC_DB?: Readonly<{ connectionString: string }>
  // KS-8.1 (#8307) pylon assignments/dispatch migration flags. Dual-write
  // defaults ON wherever KHALA_SYNC_DB exists ('off'|'0'|'false'|'disabled'
  // to disable); reads default 'd1' ('d1'|'postgres'|'compare'). See
  // docs/khala-sync/RUNBOOK.md "Pylon dispatch domain cutover".
  KHALA_SYNC_PYLON_DUAL_WRITE?: string
  KHALA_SYNC_PYLON_READS?: string
  // KS-7.2 (#8306) CVR read-set diffing flag. '1' enables the flag-gated
  // POST /api/sync/cvr-pull recovery path (docs/khala-sync/CVR_DESIGN.md);
  // anything else (or absent) keeps the route answering 404 — zero
  // behavior change.
  KHALA_SYNC_CVR?: string
  // KS-8.2 (#8308) token ledger migration flags. Same semantics as the
  // pylon pair: dual-write defaults ON wherever KHALA_SYNC_DB exists
  // ('off'|'0'|'false'|'disabled' to disable); reads default 'd1'
  // ('d1'|'postgres'|'compare') and cover only the five public
  // tokens-served read paths. See docs/khala-sync/RUNBOOK.md "Token
  // ledger domain cutover".
  KHALA_SYNC_LEDGER_DUAL_WRITE?: string
  KHALA_SYNC_LEDGER_READS?: string
  // KS-8.9 (#8320) inference entitlements/quotas migration flags. Same
  // semantics as the pylon/ledger pairs: dual-write defaults ON wherever
  // KHALA_SYNC_DB exists ('off'|'0'|'false'|'disabled' to disable); reads
  // default 'd1' ('d1'|'postgres'|'compare') and cover only the six
  // serving-path enforcement gate reads. See docs/khala-sync/RUNBOOK.md
  // "Inference entitlements domain cutover".
  KHALA_SYNC_ENTITLEMENTS_DUAL_WRITE?: string
  KHALA_SYNC_ENTITLEMENTS_READS?: string
  // Khala Sync hub (KS-4.2, #8295). Optional SQLite-class DO namespace; one
  // KhalaSyncHubDO per scope (`idFromName(scope)`) holds the recent changelog
  // window + hibernating WebSockets (docs/khala-sync/SPEC.md §5). Absent
  // until the wrangler binding + migration are deployed.
  KHALA_SYNC_HUB?: DurableObjectNamespace
  // LiveHub cutover (CFG-5, #8520; epic #8515): when BOTH are set, Khala
  // Sync hub traffic (connect WS proxy, log hub-first read, internal hub
  // routes, access-changed) goes to the owned Cloud Run `khala-live-hub`
  // service over HTTPS instead of the KhalaSyncHubDO binding. URL is a
  // plain var; the token is a Worker secret (Secret Manager
  // `khala-live-hub-token` is the source of truth).
  KHALA_SYNC_LIVE_HUB_URL?: string
  KHALA_SYNC_LIVE_HUB_TOKEN?: string
  MARKET_RELAY_SERVICE?: Fetcher
  // Optional since #8516: the account-level Cloudflare R2 feature was
  // disabled (Cloudflare→GCP consolidation, #8515), so the `r2_buckets`
  // wrangler binding was removed to unfreeze deploys (API error 10136).
  // Consumers resolve through `artifactsBucketForEnv` (workers/api
  // `src/artifacts-binding.ts`): an injected `ARTIFACTS` object wins,
  // then the CFG-8 (#8523) GCS-backed adapter configured below, then
  // typed per-call rejections.
  ARTIFACTS?: R2Bucket
  // CFG-8 (#8523): GCS replacement for the R2 ARTIFACTS bucket. Bucket
  // name + optional endpoint are committed wrangler vars; the HMAC key
  // pair for the `oa-artifacts-rw` service account arrives via
  // `wrangler secret put` (source of truth: GCP Secret Manager secrets
  // `oa-artifacts-gcs-hmac-access-key-id` / `oa-artifacts-gcs-hmac-secret`,
  // project openagentsgemini). Absent config degrades per-call as before.
  ARTIFACTS_GCS_BUCKET?: string
  ARTIFACTS_GCS_ENDPOINT?: string
  ARTIFACTS_GCS_HMAC_ACCESS_KEY_ID?: string
  ARTIFACTS_GCS_HMAC_SECRET?: string
  RUNNER_EVENTS: Queue
  ADJUTANT_ENRICHMENT_QUEUE: Queue
  ASSETS: Fetcher
  // CFG-3 (#8518): the AUTH_STORAGE KV namespace is evacuated — auth
  // key/value state lives in Postgres (oa_infra_kv via KHALA_SYNC_DB;
  // workers/api/src/auth/auth-kv.ts). No KV binding remains.
  GITHUB_CLIENT_ID: string
  GITHUB_CLIENT_SECRET: string
  GEMINI_API_KEY?: string
  OPENAUTH_CLIENT_ID: string
  OPENAUTH_ISSUER_URL: string
  OPENAGENTS_APP_URL: string
  OPENAGENTS_ADMIN_API_TOKEN?: string
  SHC_CONTROL_API_URL?: string
  SHC_CONTROL_API_BEARER_TOKEN?: string
  SHC_DISPATCH_MODE?: string
  SHC_RUNNER_CALLBACK_TOKEN?: string
}>

export const jsonResponse = (
  value: unknown,
  init: ResponseInit = {},
): Response => {
  const headers = new Headers(init.headers)
  headers.set('content-type', 'application/json')

  return Response.json(value, {
    ...init,
    headers,
  })
}

export const notFound = (): Response =>
  jsonResponse({ error: 'not_found' }, { status: 404 })

export const badRequest = (reason: string): Response =>
  jsonResponse({ error: 'bad_request', reason }, { status: 400 })

export const makeEmptySnapshot = (scope: string): SyncSnapshot =>
  new SyncSnapshot({
    scope: SyncScope.make(scope),
    cursor: SyncSequence.make(0),
    collections: {},
  })

export const acceptedMutation = (command: SyncCommand): SyncMutationAccepted =>
  new SyncMutationAccepted({
    status: 'accepted',
    mutationId: command.mutationId,
  })

export const scopeIdFromName = (
  namespace: DurableObjectNamespace,
  scope: string,
): DurableObjectId => namespace.idFromName(scope)

export type SyncChangeInput = Readonly<{
  actorId?: string | undefined
  collection: string
  id: string
  mutationId?: string | undefined
  op: 'put' | 'patch' | 'delete' | 'invalidate'
  patch?: unknown
  scope: string
  serverTime?: string | undefined
  value?: unknown
}>

export type SyncChangeRow = Readonly<{
  actor_id: string | null
  collection: string
  created_at: string
  entity_id: string
  mutation_id: string | null
  op: 'put' | 'patch' | 'delete' | 'invalidate'
  patch_json: string | null
  scope: string
  seq: number
  value_json: string | null
}>

export type SyncMutationRow = Readonly<{
  actor_id: string
  created_at: string
  mutation_id: string
  result_json: string | null
  scope: string
  status: string
}>

export class SyncOutboxStorageError extends S.TaggedErrorClass<SyncOutboxStorageError>()(
  'SyncOutboxStorageError',
  {
    error: S.Defect,
    operation: S.String,
  },
) {}

export class SyncSequenceAllocationFailed extends S.TaggedErrorClass<SyncSequenceAllocationFailed>()(
  'SyncSequenceAllocationFailed',
  {
    scope: S.String,
  },
) {}

export class SyncPayloadDecodeError extends S.TaggedErrorClass<SyncPayloadDecodeError>()(
  'SyncPayloadDecodeError',
  {
    field: S.String,
    reason: S.String,
  },
) {}

export class SyncPayloadEncodeError extends S.TaggedErrorClass<SyncPayloadEncodeError>()(
  'SyncPayloadEncodeError',
  {
    field: S.String,
    reason: S.String,
  },
) {}

export class SyncScopeMismatch extends S.TaggedErrorClass<SyncScopeMismatch>()(
  'SyncScopeMismatch',
  {
    actualScope: S.String,
    expectedScope: S.String,
  },
) {}

export class SyncMutationAlreadyAccepted extends S.TaggedErrorClass<SyncMutationAlreadyAccepted>()(
  'SyncMutationAlreadyAccepted',
  {
    mutationId: S.String,
  },
) {}

export class SyncMutationAlreadyRejected extends S.TaggedErrorClass<SyncMutationAlreadyRejected>()(
  'SyncMutationAlreadyRejected',
  {
    mutationId: S.String,
    reason: S.String,
  },
) {}

export class SyncSnapshotMissing extends S.TaggedErrorClass<SyncSnapshotMissing>()(
  'SyncSnapshotMissing',
  {
    scope: S.String,
  },
) {}

export class SyncChangeMissing extends S.TaggedErrorClass<SyncChangeMissing>()(
  'SyncChangeMissing',
  {
    scope: S.String,
    seq: S.Number,
  },
) {}

export const SyncOutboxError = S.Union([
  SyncOutboxStorageError,
  SyncSequenceAllocationFailed,
  SyncPayloadDecodeError,
  SyncPayloadEncodeError,
  SyncScopeMismatch,
  SyncMutationAlreadyAccepted,
  SyncMutationAlreadyRejected,
  SyncSnapshotMissing,
  SyncChangeMissing,
])
export type SyncOutboxError = typeof SyncOutboxError.Type

export type SyncOutboxStoreShape = Readonly<{
  acceptMutation: (
    command: SyncCommand,
    actorId: string,
  ) => Effect.Effect<void, SyncOutboxError>
  acceptMutationForScope: (
    expectedScope: string,
    command: SyncCommand,
    actorId: string,
  ) => Effect.Effect<void, SyncOutboxError>
  appendChange: (
    input: SyncChangeInput,
  ) => Effect.Effect<SyncPatch, SyncOutboxError>
  appendChanges: (
    inputs: ReadonlyArray<SyncChangeInput>,
  ) => Effect.Effect<ReadonlyArray<SyncPatch>, SyncOutboxError>
  readChange: (
    scope: string,
    seq: number,
  ) => Effect.Effect<SyncPatch, SyncOutboxError>
  readChangesAfter: (
    scope: string,
    cursor: number,
    limit?: number,
  ) => Effect.Effect<ReadonlyArray<SyncPatch>, SyncOutboxError>
  readSnapshot: (scope: string) => Effect.Effect<SyncSnapshot, SyncOutboxError>
  readRequiredSnapshot: (
    scope: string,
  ) => Effect.Effect<SyncSnapshot, SyncOutboxError>
  rejectMutation: (
    command: SyncCommand,
    actorId: string,
    reason: string,
  ) => Effect.Effect<SyncMutationRejected, SyncOutboxError>
  rejectMutationForScope: (
    expectedScope: string,
    command: SyncCommand,
    actorId: string,
    reason: string,
  ) => Effect.Effect<SyncMutationRejected, SyncOutboxError>
}>

export class SyncOutboxStore extends Context.Service<
  SyncOutboxStore,
  SyncOutboxStoreShape
>()('@openagentsinc/sync-worker/SyncOutboxStore') {
  static layer = (
    db: D1Database,
    runtime: SyncWorkerRuntime = systemSyncWorkerRuntime,
  ) => Layer.succeed(SyncOutboxStore, makeD1SyncOutboxStore(db, runtime))

  static effectCfLayer = <E, R>(
    database: Effect.Effect<D1Database, E, R>,
    runtime: SyncWorkerRuntime = systemSyncWorkerRuntime,
  ) =>
    Layer.effect(
      SyncOutboxStore,
      Effect.map(database, db => makeD1SyncOutboxStore(db, runtime)),
    )
}

export type SyncOutboxRepository = Readonly<{
  acceptMutation: (command: SyncCommand, actorId: string) => Promise<void>
  appendChange: (input: SyncChangeInput) => Promise<SyncPatch>
  appendChanges: (
    inputs: ReadonlyArray<SyncChangeInput>,
  ) => Promise<ReadonlyArray<SyncPatch>>
  readChangesAfter: (
    scope: string,
    cursor: number,
    limit?: number,
  ) => Promise<ReadonlyArray<SyncPatch>>
  readSnapshot: (scope: string) => Promise<SyncSnapshot>
  rejectMutation: (
    command: SyncCommand,
    actorId: string,
    reason: string,
  ) => Promise<SyncMutationRejected>
}>

type SequenceRow = Readonly<{ last_seq: number }>

export type SyncWorkerRuntime = Readonly<{
  nowIso: () => string
}>

export const systemSyncWorkerRuntime: SyncWorkerRuntime = {
  nowIso: currentIsoTimestamp,
}

const errorReason = (error: unknown): string =>
  error instanceof Error ? error.message : String(error)

const d1Effect = <A>(
  operation: string,
  run: () => Promise<A>,
): Effect.Effect<A, SyncOutboxStorageError> =>
  Effect.tryPromise({
    catch: error => new SyncOutboxStorageError({ error, operation }),
    try: run,
  })

const encodeStoredJson = (
  field: string,
  value: unknown,
): Effect.Effect<string, SyncPayloadEncodeError> =>
  Effect.gen(function* () {
    const encoded = yield* Effect.try({
      catch: error =>
        new SyncPayloadEncodeError({ field, reason: errorReason(error) }),
      try: () => JSON.stringify(value),
    })

    if (encoded === undefined) {
      return yield* new SyncPayloadEncodeError({
        field,
        reason: 'Value is not JSON-serializable.',
      })
    }

    return encoded
  })

const encodeStoredJsonOrNull = (
  field: string,
  value: unknown | undefined,
): Effect.Effect<string | null, SyncPayloadEncodeError> =>
  value === undefined ? Effect.succeed(null) : encodeStoredJson(field, value)

const decodeStoredJson = (
  field: string,
  value: string,
): Effect.Effect<unknown, SyncPayloadDecodeError> =>
  decodeJsonValueEffect(
    value,
    reason => new SyncPayloadDecodeError({ field, reason }),
  )

const decodeStoredJsonOrUndefined = (
  field: string,
  value: string | null,
): Effect.Effect<unknown | undefined, SyncPayloadDecodeError> =>
  value === null ? Effect.sync(() => undefined) : decodeStoredJson(field, value)

const rowToPatch = (
  row: SyncChangeRow,
): Effect.Effect<SyncPatch, SyncPayloadDecodeError> =>
  Effect.gen(function* () {
    const value = yield* decodeStoredJsonOrUndefined(
      'sync_changes.value_json',
      row.value_json,
    )
    const patch = yield* decodeStoredJsonOrUndefined(
      'sync_changes.patch_json',
      row.patch_json,
    )

    return new SyncPatch({
      scope: SyncScope.make(row.scope),
      seq: SyncSequence.make(row.seq),
      collection: CollectionName.make(row.collection),
      op: row.op,
      id: EntityId.make(row.entity_id),
      ...(value === undefined ? {} : { value }),
      ...(patch === undefined ? {} : { patch }),
      serverTime: IsoTimestamp.make(row.created_at),
      ...(row.mutation_id === null
        ? {}
        : { mutationId: MutationId.make(row.mutation_id) }),
    })
  })

const applyPatchToCollections = (
  collections: Record<string, Record<string, unknown>>,
  patch: SyncPatch,
): Record<string, Record<string, unknown>> => {
  const collection = collections[patch.collection] ?? {}

  if (patch.op === 'delete' || patch.op === 'invalidate') {
    const { [patch.id]: _removed, ...nextCollection } = collection

    return { ...collections, [patch.collection]: nextCollection }
  }

  if (patch.op === 'patch') {
    const previous = collection[patch.id]
    const previousRecord =
      typeof previous === 'object' &&
      previous !== null &&
      !Array.isArray(previous)
        ? previous
        : {}
    const patchRecord =
      typeof patch.patch === 'object' &&
      patch.patch !== null &&
      !Array.isArray(patch.patch)
        ? patch.patch
        : {}

    return {
      ...collections,
      [patch.collection]: {
        ...collection,
        [patch.id]: {
          ...previousRecord,
          ...patchRecord,
        },
      },
    }
  }

  return {
    ...collections,
    [patch.collection]: {
      ...collection,
      [patch.id]: patch.value,
    },
  }
}

const claimNextSequence = (
  db: D1Database,
  scope: string,
  createdAt: string,
): Effect.Effect<
  number,
  SyncOutboxStorageError | SyncSequenceAllocationFailed
> =>
  Effect.gen(function* () {
    const row = yield* d1Effect('sync_outbox.claim_next_sequence', () =>
      db
        .prepare(
          `INSERT INTO sync_scopes (scope, last_seq, created_at, updated_at)
           VALUES (?, 1, ?, ?)
           ON CONFLICT(scope) DO UPDATE
           SET last_seq = sync_scopes.last_seq + 1,
               updated_at = excluded.updated_at
           RETURNING last_seq`,
        )
        .bind(scope, createdAt, createdAt)
        .first<SequenceRow>(),
    )

    if (row === null) {
      return yield* new SyncSequenceAllocationFailed({ scope })
    }

    return row.last_seq
  })

const rowsToPatches = (
  rows: ReadonlyArray<SyncChangeRow>,
): Effect.Effect<ReadonlyArray<SyncPatch>, SyncPayloadDecodeError> =>
  rows.reduce<Effect.Effect<ReadonlyArray<SyncPatch>, SyncPayloadDecodeError>>(
    (previousPatches, row) =>
      Effect.flatMap(previousPatches, patches =>
        Effect.map(rowToPatch(row), patch => [...patches, patch]),
      ),
    Effect.succeed([] as ReadonlyArray<SyncPatch>),
  )

const assertCommandScope = (
  expectedScope: string,
  command: SyncCommand,
): Effect.Effect<void, SyncScopeMismatch> => {
  const actualScope = String(command.scope)

  return actualScope === expectedScope
    ? Effect.sync(() => undefined)
    : Effect.fail(new SyncScopeMismatch({ actualScope, expectedScope }))
}

const mutationStatus = (
  status: string,
): 'accepted' | 'rejected' | undefined => {
  if (status === 'accepted' || status === 'rejected') {
    return status
  }

  return undefined
}

const reasonFromDecodedMutationResult = (value: unknown): string => {
  if (typeof value !== 'object' || value === null || Array.isArray(value)) {
    return 'already rejected'
  }

  const reason = Reflect.get(value, 'reason')

  return typeof reason === 'string' ? reason : 'already rejected'
}

const rejectedReasonFromRow = (
  row: SyncMutationRow,
): Effect.Effect<string, SyncPayloadDecodeError> =>
  row.result_json === null
    ? Effect.succeed('already rejected')
    : Effect.map(
        decodeStoredJson('sync_mutations.result_json', row.result_json),
        reasonFromDecodedMutationResult,
      )

const readMutationRow = (
  db: D1Database,
  mutationId: string,
): Effect.Effect<SyncMutationRow | null, SyncOutboxStorageError> =>
  d1Effect('sync_outbox.read_mutation', () =>
    db
      .prepare(
        `SELECT mutation_id, scope, actor_id, status, result_json, created_at
         FROM sync_mutations
         WHERE mutation_id = ?
         LIMIT 1`,
      )
      .bind(mutationId)
      .first<SyncMutationRow>(),
  )

const ensureMutationCanAccept = (
  db: D1Database,
  command: SyncCommand,
): Effect.Effect<
  void,
  | SyncOutboxStorageError
  | SyncPayloadDecodeError
  | SyncScopeMismatch
  | SyncMutationAlreadyRejected
> =>
  Effect.gen(function* () {
    const mutationId = String(command.mutationId)
    const row = yield* readMutationRow(db, mutationId)

    if (row === null) {
      return
    }

    if (row.scope !== String(command.scope)) {
      return yield* new SyncScopeMismatch({
        actualScope: row.scope,
        expectedScope: String(command.scope),
      })
    }

    const status = mutationStatus(row.status)

    if (status === undefined) {
      return yield* new SyncPayloadDecodeError({
        field: 'sync_mutations.status',
        reason: `Unsupported mutation status: ${row.status}`,
      })
    }

    if (status === 'rejected') {
      const reason = yield* rejectedReasonFromRow(row)

      return yield* new SyncMutationAlreadyRejected({ mutationId, reason })
    }
  })

const ensureMutationCanReject = (
  db: D1Database,
  command: SyncCommand,
): Effect.Effect<
  void,
  | SyncOutboxStorageError
  | SyncPayloadDecodeError
  | SyncScopeMismatch
  | SyncMutationAlreadyAccepted
  | SyncMutationAlreadyRejected
> =>
  Effect.gen(function* () {
    const mutationId = String(command.mutationId)
    const row = yield* readMutationRow(db, mutationId)

    if (row === null) {
      return
    }

    if (row.scope !== String(command.scope)) {
      return yield* new SyncScopeMismatch({
        actualScope: row.scope,
        expectedScope: String(command.scope),
      })
    }

    const status = mutationStatus(row.status)

    if (status === undefined) {
      return yield* new SyncPayloadDecodeError({
        field: 'sync_mutations.status',
        reason: `Unsupported mutation status: ${row.status}`,
      })
    }

    if (status === 'accepted') {
      return yield* new SyncMutationAlreadyAccepted({ mutationId })
    }

    const reason = yield* rejectedReasonFromRow(row)

    return yield* new SyncMutationAlreadyRejected({ mutationId, reason })
  })

export const workspaceScope = (workspaceId: string): string =>
  `workspace:${workspaceId}`

export const personalWorkroomScope = (userId: string): string =>
  workspaceScope(userId)

export const teamScope = (teamId: string): string => `team:${teamId}`

export const threadScope = (threadId: string): string => `thread:${threadId}`

export const agentRunScope = (runId: string): string => `agent-run:${runId}`

export const publicAgentScope = (agentId: string): string =>
  `public-agent:${agentId}`

export const publicGoalScope = (goalId: string): string =>
  `public-goal:${goalId}`

export const publicAgentRunScope = (runId: string): string =>
  `public-agent-run:${runId}`

// Single public, read-only firehose scope for live settled-feed updates. The id
// is a stable feed key (not a per-run id) so the homepage and public surfaces
// can subscribe to one room that streams every public-safe settlement as sats
// stream in. Public-safe payloads only; never raw payment material.
export const PUBLIC_SETTLED_FEED_ID = 'tassadar'

export const publicSettledFeedScope = (
  feedId: string = PUBLIC_SETTLED_FEED_ID,
): string => `public-settled-feed:${feedId}`

// Single public, read-only firehose scope for the live "Khala Tokens Served"
// counter (openagents #6231). Each served Khala completion publishes ONE
// public-safe `{ tokensServedDelta, observedAt }` patch onto this scope so the
// homepage / stats odometer rolls up instantly instead of polling a per-second
// D1 SUM. The id is a stable network key (not a per-request id) so every public
// surface subscribes to one room. Public-safe payloads only — a bare integer
// delta + timestamp, never per-user/team/provider/secret material.
export const PUBLIC_KHALA_TOKENS_SERVED_ID = 'network'

export const publicKhalaTokensServedScope = (
  feedId: string = PUBLIC_KHALA_TOKENS_SERVED_ID,
): string => `public-khala-tokens-served:${feedId}`

// Single public, read-only scope for the live Gym / Harbor "Follow an active
// Terminal-Bench run" panel (openagents #6261). Each operator run-progress
// ingest publishes the public-safe projected snapshot (one put per `runRef`)
// onto this scope so the `/gym` follow-along updates the instant a snapshot is
// ingested instead of polling every ~12s. The id is a stable network key so
// every public surface subscribes to one room. Public-safe payloads only — the
// already-redacted `openagents.gym.run_progress.v1` projection (counts /
// denominators / public-safe refs), never raw prompts/responses/logs/keys.
export const PUBLIC_GYM_RUN_PROGRESS_ID = 'network'

export const publicGymRunProgressScope = (
  feedId: string = PUBLIC_GYM_RUN_PROGRESS_ID,
): string => `public-gym-run-progress:${feedId}`

export const makeD1SyncOutboxStore = (
  db: D1Database,
  runtime: SyncWorkerRuntime = systemSyncWorkerRuntime,
): SyncOutboxStoreShape => {
  const appendChange = Effect.fn('SyncOutboxStore.appendChange')(
    (input: SyncChangeInput): Effect.Effect<SyncPatch, SyncOutboxError> =>
      Effect.gen(function* () {
        const createdAt = input.serverTime ?? runtime.nowIso()
        const seq = yield* claimNextSequence(db, input.scope, createdAt)
        const valueJson = yield* encodeStoredJsonOrNull(
          'sync_changes.value_json',
          input.value,
        )
        const patchJson = yield* encodeStoredJsonOrNull(
          'sync_changes.patch_json',
          input.patch,
        )

        yield* d1Effect('sync_outbox.append_change', () =>
          db
            .prepare(
              `INSERT INTO sync_changes
                (scope, seq, collection, op, entity_id, value_json, patch_json,
                 mutation_id, actor_id, created_at)
               VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`,
            )
            .bind(
              input.scope,
              seq,
              input.collection,
              input.op,
              input.id,
              valueJson,
              patchJson,
              input.mutationId ?? null,
              input.actorId ?? null,
              createdAt,
            )
            .run(),
        )

        return new SyncPatch({
          scope: SyncScope.make(input.scope),
          seq: SyncSequence.make(seq),
          collection: CollectionName.make(input.collection),
          op: input.op,
          id: EntityId.make(input.id),
          ...(input.value === undefined ? {} : { value: input.value }),
          ...(input.patch === undefined ? {} : { patch: input.patch }),
          serverTime: IsoTimestamp.make(createdAt),
          ...(input.mutationId === undefined
            ? {}
            : { mutationId: MutationId.make(input.mutationId) }),
        })
      }),
  )

  const readChange = Effect.fn('SyncOutboxStore.readChange')(
    (scope: string, seq: number): Effect.Effect<SyncPatch, SyncOutboxError> =>
      Effect.gen(function* () {
        const row = yield* d1Effect('sync_outbox.read_change', () =>
          db
            .prepare(
              `SELECT scope, seq, collection, op, entity_id, value_json,
                      patch_json, mutation_id, actor_id, created_at
               FROM sync_changes
               WHERE scope = ? AND seq = ?
               LIMIT 1`,
            )
            .bind(scope, seq)
            .first<SyncChangeRow>(),
        )

        if (row === null) {
          return yield* new SyncChangeMissing({ scope, seq })
        }

        return yield* rowToPatch(row)
      }),
  )

  const readChangesAfter = Effect.fn('SyncOutboxStore.readChangesAfter')(
    (
      scope: string,
      cursor: number,
      limit = 500,
    ): Effect.Effect<ReadonlyArray<SyncPatch>, SyncOutboxError> =>
      Effect.gen(function* () {
        const rows = yield* d1Effect('sync_outbox.read_changes_after', () =>
          db
            .prepare(
              `SELECT scope, seq, collection, op, entity_id, value_json, patch_json,
                      mutation_id, actor_id, created_at
               FROM sync_changes
               WHERE scope = ? AND seq > ?
               ORDER BY seq ASC
               LIMIT ?`,
            )
            .bind(scope, cursor, limit)
            .all<SyncChangeRow>(),
        )

        return yield* rowsToPatches(rows.results)
      }),
  )

  const readSnapshot = Effect.fn('SyncOutboxStore.readSnapshot')(
    (scope: string): Effect.Effect<SyncSnapshot, SyncOutboxError> =>
      Effect.gen(function* () {
        const rows = yield* d1Effect('sync_outbox.read_snapshot', () =>
          db
            .prepare(
              `SELECT scope, seq, collection, op, entity_id, value_json, patch_json,
                      mutation_id, actor_id, created_at
               FROM sync_changes
               WHERE scope = ?
               ORDER BY seq ASC`,
            )
            .bind(scope)
            .all<SyncChangeRow>(),
        )
        const patches = yield* rowsToPatches(rows.results)
        const collections = patches.reduce(applyPatchToCollections, {})
        const cursor = patches.at(-1)?.seq ?? 0

        return new SyncSnapshot({
          scope: SyncScope.make(scope),
          cursor: SyncSequence.make(cursor),
          collections,
        })
      }),
  )

  const readRequiredSnapshot = Effect.fn(
    'SyncOutboxStore.readRequiredSnapshot',
  )(
    (scope: string): Effect.Effect<SyncSnapshot, SyncOutboxError> =>
      Effect.gen(function* () {
        const snapshot = yield* readSnapshot(scope)

        if (snapshot.cursor === 0) {
          return yield* new SyncSnapshotMissing({ scope })
        }

        return snapshot
      }),
  )

  const acceptMutationForScope = Effect.fn(
    'SyncOutboxStore.acceptMutationForScope',
  )(
    (
      expectedScope: string,
      command: SyncCommand,
      actorId: string,
    ): Effect.Effect<void, SyncOutboxError> =>
      Effect.gen(function* () {
        yield* assertCommandScope(expectedScope, command)
        yield* ensureMutationCanAccept(db, command)
        const resultJson = yield* encodeStoredJson(
          'sync_mutations.result_json',
          acceptedMutation(command),
        )

        yield* d1Effect('sync_outbox.accept_mutation', () =>
          db
            .prepare(
              `INSERT OR IGNORE INTO sync_mutations
                (mutation_id, scope, actor_id, status, result_json, created_at)
               VALUES (?, ?, ?, 'accepted', ?, ?)`,
            )
            .bind(
              command.mutationId,
              command.scope,
              actorId,
              resultJson,
              runtime.nowIso(),
            )
            .run(),
        )
      }),
  )

  const rejectMutationForScope = Effect.fn(
    'SyncOutboxStore.rejectMutationForScope',
  )(
    (
      expectedScope: string,
      command: SyncCommand,
      actorId: string,
      reason: string,
    ): Effect.Effect<SyncMutationRejected, SyncOutboxError> =>
      Effect.gen(function* () {
        yield* assertCommandScope(expectedScope, command)
        yield* ensureMutationCanReject(db, command)

        const rejection = new SyncMutationRejected({
          status: 'rejected',
          mutationId: command.mutationId,
          reason,
        })
        const resultJson = yield* encodeStoredJson(
          'sync_mutations.result_json',
          rejection,
        )

        yield* d1Effect('sync_outbox.reject_mutation', () =>
          db
            .prepare(
              `INSERT OR REPLACE INTO sync_mutations
                (mutation_id, scope, actor_id, status, result_json, created_at)
               VALUES (?, ?, ?, 'rejected', ?, ?)`,
            )
            .bind(
              command.mutationId,
              command.scope,
              actorId,
              resultJson,
              runtime.nowIso(),
            )
            .run(),
        )

        return rejection
      }),
  )

  return {
    acceptMutation: (command, actorId) =>
      acceptMutationForScope(String(command.scope), command, actorId),
    acceptMutationForScope,
    appendChange,
    appendChanges: inputs =>
      inputs.reduce<Effect.Effect<ReadonlyArray<SyncPatch>, SyncOutboxError>>(
        (previousPatches, input) =>
          Effect.flatMap(previousPatches, patches =>
            Effect.map(appendChange(input), patch => [...patches, patch]),
          ),
        Effect.succeed([] as ReadonlyArray<SyncPatch>),
      ),
    readChange,
    readChangesAfter,
    readSnapshot,
    readRequiredSnapshot,
    rejectMutation: (command, actorId, reason) =>
      rejectMutationForScope(String(command.scope), command, actorId, reason),
    rejectMutationForScope,
  }
}

const runSyncOutboxEffect = <A>(
  effect: Effect.Effect<A, SyncOutboxError>,
): Promise<A> => Effect.runPromise(effect)

export const makeD1SyncOutboxRepository = (
  db: D1Database,
  runtime: SyncWorkerRuntime = systemSyncWorkerRuntime,
): SyncOutboxRepository => {
  const store = makeD1SyncOutboxStore(db, runtime)

  return {
    acceptMutation: (command, actorId) =>
      runSyncOutboxEffect(store.acceptMutation(command, actorId)),
    appendChange: input => runSyncOutboxEffect(store.appendChange(input)),
    appendChanges: inputs => runSyncOutboxEffect(store.appendChanges(inputs)),
    readChangesAfter: (scope, cursor, limit) =>
      runSyncOutboxEffect(store.readChangesAfter(scope, cursor, limit)),
    readSnapshot: scope => runSyncOutboxEffect(store.readSnapshot(scope)),
    rejectMutation: (command, actorId, reason) =>
      runSyncOutboxEffect(store.rejectMutation(command, actorId, reason)),
  }
}

export const cursorGap = (
  scope: string,
  expectedSeq: number,
  receivedSeq: number,
): CursorGap =>
  new CursorGap({
    scope: SyncScope.make(scope),
    expectedSeq: SyncSequence.make(expectedSeq),
    receivedSeq: SyncSequence.make(receivedSeq),
  })
