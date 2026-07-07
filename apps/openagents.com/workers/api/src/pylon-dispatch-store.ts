// KS-8.1 (#8307): pylon assignments/dispatch domain — D1 → Cloud SQL
// migration machinery. First KS-8 domain lane; this module is the template.
//
// Three pieces:
//
//  1. `makePostgresPylonDispatchStore` — the Postgres implementation of the
//     assignment/dispatch slice of `PylonApiStore` (create / claim /
//     event-append / state-update / gate-reads), over the structural
//     `SyncSql` seam via the KHALA_SYNC_DB Hyperdrive binding. Tables:
//     `pylon_registrations`, `pylon_assignments`, `pylon_assignment_events`
//     (khala-sync-server migration 0005). D1's dedupe-SELECT-then-INSERT
//     pairs collapse to `ON CONFLICT ... DO NOTHING` upserts on the SAME
//     idempotency keys (MIGRATION_PLAN universal porting rule — closes the
//     TOCTOU window without changing the key set).
//
//  2. `makeDualWritePylonApiStore` — the flag-routed wrapper that IS the
//     production `PylonApiStore`. Writes go D1-first (authority), then
//     mirror to Postgres best-effort: a Postgres failure NEVER fails the
//     request; it logs a typed diagnostic (`khala_sync_pylon_dual_write_failed`
//     — the drift metric) and moves on. Reads route per flag:
//       d1        — D1 only (default)
//       compare   — read both, SERVE D1, log mismatches with refs
//       postgres  — Postgres with bounded retry (the June-29 concurrency
//                   headroom fix), D1 fallback + diagnostic on exhaustion
//     KS-8.4 starts the same pattern for adjacent Pylon control-plane writes:
//     provider lifecycle, quarantines, and Spark payout targets mirror
//     D1-first to Postgres, while reads remain D1-authoritative until the
//     #8315 verify/compare/cutover evidence lands.
//
//  3. `makePylonApiStoreForEnv` — the drop-in factory index.ts call sites
//     use instead of bare `makeD1PylonApiStore`. Flags:
//       KHALA_SYNC_PYLON_DUAL_WRITE  (default ON; 'off'|'0'|'false'|'disabled')
//       KHALA_SYNC_PYLON_READS       (default 'd1'; 'd1'|'postgres'|'compare')
//       KHALA_SYNC_PYLON_WRITES      (default 'postgres' after #8515; 'd1'
//                                     restores D1 authority — see below)
//     With no KHALA_SYNC_DB binding everything degrades to plain D1.
//
// #8515 WRITE cutover (Cloudflare exit): the D1 HTTP bridge 401s account-wide
// (Workers Paid plan cancelled). `KHALA_SYNC_PYLON_WRITES=postgres` (now the
// default) makes Postgres the SOLE authority for the assignment / heartbeat /
// dispatch write surface — createAssignment, createEvent, updateAssignment,
// updateAssignmentIfState, upsertRegistration (presence/heartbeat + capacity),
// upsertProviderJobLifecycle, upsertQuarantine, sweepStaleAssignmentLeases.
// Dead D1 is no longer written or mirrored back to on that path, and there is
// no D1 fallback on a Postgres error (fail loud, never diverge).
//
// Cutover order (docs/khala-sync/RUNBOOK.md "Pylon dispatch domain"):
// dual-write on → backfill (scripts/backfill-pylon.ts) → verify → compare
// reads → postgres reads → decommission D1 tables in a follow-up issue.

import type { SyncSql } from '@openagentsinc/khala-sync-server'

import {
  defaultMakeKhalaSyncSqlClient,
  type KhalaSyncHyperdriveBinding,
  type KhalaSyncPushSqlClient,
  type MakeKhalaSyncPushSqlClient,
} from './khala-sync-push-routes'
import { parseJsonStringArray } from './json-boundary'
import { logWorkerRouteWarning } from './observability'
import {
  ACTIVE_LEASE_ASSIGNMENT_STATES,
  makeD1PylonApiStore,
  makeD1PylonSparkPayoutTargetStore,
  publicPylonApiAssignmentProjection,
  publicPylonApiRegistrationProjection,
  pylonApiAssignmentPaymentMode,
  providerJobLifecycleRecordFromAssignment,
  PylonApiStoreError,
  rowToAssignment,
  rowToEvent,
  rowToRegistration,
  type PylonApiAssignmentRecord,
  type PylonApiAssignmentRow,
  type PylonApiAssignmentState,
  type PylonApiEventRecord,
  type PylonApiEventRow,
  type PylonApiProviderJobLifecycleRecord,
  type PylonApiQuarantineRecord,
  type PylonApiRegistrationRecord,
  type PylonApiRegistrationRow,
  type PylonApiStore,
  type PylonSparkPayoutTargetRecord,
  type PylonSparkPayoutTargetStore,
} from './pylon-api'
import { openAgentsDatabase } from './runtime'

// ---------------------------------------------------------------------------
// Flags
// ---------------------------------------------------------------------------

export type PylonDispatchReadsMode = 'd1' | 'postgres' | 'compare'

// KS-8 WRITE cutover (#8515 Cloudflare exit): the D1 HTTP bridge 401s
// account-wide (Workers Paid plan cancelled — no token fix). `writes` moves
// the AUTHORITATIVE assignment/heartbeat/dispatch writes off dead D1 onto the
// same Postgres connection the reads already use. In 'postgres' mode Postgres
// is the SOLE write authority — there is deliberately NO silent D1 fallback
// (a Postgres outage must fail the request loud, not diverge), and D1 is not
// mirrored back to because it is a dead endpoint. This mirrors the forge
// canonical-write cutover discipline (`forgeGitCanonicalWritesFromEnv`).
export type PylonDispatchWritesMode = 'd1' | 'postgres'

export type PylonDispatchFlags = Readonly<{
  dualWrite: boolean
  reads: PylonDispatchReadsMode
  writes: PylonDispatchWritesMode
}>

export type PylonDispatchFlagEnv = Readonly<{
  KHALA_SYNC_PYLON_DUAL_WRITE?: string | undefined
  KHALA_SYNC_PYLON_READS?: string | undefined
  KHALA_SYNC_PYLON_WRITES?: string | undefined
}>

const FLAG_OFF_VALUES = new Set(['0', 'off', 'false', 'disabled', 'no'])

/**
 * Parse the KS-8.1 migration flags from Worker vars. Dual-write defaults ON
 * (this lane lands with the mirror active wherever the binding exists);
 * reads default to D1 authority until the runbook's cutover sequence flips
 * them. Unknown read values fall back to 'd1' — never fail open into an
 * unproven read path on a typo.
 *
 * `writes` defaults to 'postgres' (#8515): D1 is dead account-wide, so the
 * authoritative assignment/heartbeat/dispatch writes MUST live on Postgres.
 * Any value other than an explicit 'd1' resolves to 'postgres'; the only way
 * back to D1 authority is to set the flag to 'd1' explicitly. This is the
 * inverse typo-posture of `reads` on purpose — after the Cloudflare exit the
 * safe default is Postgres, and a typo must not silently route writes back to
 * the 401-dead bridge.
 */
export const pylonDispatchFlagsFromEnv = (
  env: PylonDispatchFlagEnv,
): PylonDispatchFlags => {
  const dualWriteRaw = env.KHALA_SYNC_PYLON_DUAL_WRITE?.trim().toLowerCase()
  const readsRaw = env.KHALA_SYNC_PYLON_READS?.trim().toLowerCase()
  const writesRaw = env.KHALA_SYNC_PYLON_WRITES?.trim().toLowerCase()

  return {
    dualWrite:
      dualWriteRaw === undefined || !FLAG_OFF_VALUES.has(dualWriteRaw),
    reads:
      readsRaw === 'postgres' || readsRaw === 'compare' ? readsRaw : 'd1',
    writes: writesRaw === 'd1' ? 'd1' : 'postgres',
  }
}

// ---------------------------------------------------------------------------
// Diagnostics (the drift metric)
// ---------------------------------------------------------------------------

export type PylonDispatchDiagnosticEvent =
  | 'khala_sync_pylon_dual_write_failed'
  | 'khala_sync_pylon_read_compare_mismatch'
  | 'khala_sync_pylon_postgres_read_failed'
  | 'khala_sync_pylon_postgres_read_fallback'

export type PylonDispatchDiagnostic = Readonly<{
  /** The store operation, e.g. 'createAssignment'. */
  op: string
  /** Public-safe refs identifying the affected rows (never payloads). */
  refs: ReadonlyArray<string>
  /** Public-safe failure summary (error class/message head, no SQL). */
  messageSafe: string
}>

export type PylonDispatchLog = (
  event: PylonDispatchDiagnosticEvent,
  fields: PylonDispatchDiagnostic,
) => void

const safeMessage = (error: unknown): string => {
  const raw = error instanceof Error ? error.message : String(error)
  // Bounded, single-line, no parameter values beyond what the driver puts in
  // the message head.
  return raw.replaceAll(/\s+/g, ' ').slice(0, 200)
}

// ---------------------------------------------------------------------------
// Postgres dispatch store
// ---------------------------------------------------------------------------

/**
 * The assignment/dispatch slice of `PylonApiStore` plus the KS-8.4
 * control-plane mirror operations the dual-write wrapper converges resolved
 * D1 records through.
 */
export type PostgresPylonDispatchStore = Readonly<{
  createAssignment: PylonApiStore['createAssignment']
  createEvent: PylonApiStore['createEvent']
  listAssignmentsForPylon: PylonApiStore['listAssignmentsForPylon']
  listAssignmentsForPylons: NonNullable<PylonApiStore['listAssignmentsForPylons']>
  listEventsForPylon: PylonApiStore['listEventsForPylon']
  listEventsForAssignment: PylonApiStore['listEventsForAssignment']
  listRegistrations: PylonApiStore['listRegistrations']
  listRegistrationsForOwnerAgentUserIds: NonNullable<
    PylonApiStore['listRegistrationsForOwnerAgentUserIds']
  >
  listProviderJobLifecycleForPylons: PylonApiStore['listProviderJobLifecycleForPylons']
  readAssignment: PylonApiStore['readAssignment']
  readAssignmentByIdempotencyKeyHash: PylonApiStore['readAssignmentByIdempotencyKeyHash']
  readEventByIdempotencyKeyHash: PylonApiStore['readEventByIdempotencyKeyHash']
  readRegistration: PylonApiStore['readRegistration']
  sweepStaleAssignmentLeases: NonNullable<
    PylonApiStore['sweepStaleAssignmentLeases']
  >
  updateAssignment: PylonApiStore['updateAssignment']
  updateAssignmentIfState: PylonApiStore['updateAssignmentIfState']
  upsertProviderJobLifecycle: PylonApiStore['upsertProviderJobLifecycle']
  upsertQuarantine: NonNullable<PylonApiStore['upsertQuarantine']>
  upsertRegistration: PylonApiStore['upsertRegistration']
  /**
   * Mirror operations (dual-write only): converge Postgres to the RESOLVED
   * record the authoritative D1 write produced — full-row upserts, so a row
   * touched by dual-write self-heals even before the backfill reaches it.
   */
  mirrorAssignment: (record: PylonApiAssignmentRecord) => Promise<void>
  mirrorEvent: (record: PylonApiEventRecord) => Promise<void>
  mirrorProviderJobLifecycle: (
    record: PylonApiProviderJobLifecycleRecord,
  ) => Promise<void>
  mirrorQuarantine: (record: PylonApiQuarantineRecord) => Promise<void>
  mirrorRegistration: (record: PylonApiRegistrationRecord) => Promise<void>
  mirrorSparkPayoutTarget: (
    record: PylonSparkPayoutTargetRecord,
  ) => Promise<void>
  /**
   * #8515 WRITE cutover: Postgres-authoritative Spark payout target
   * operations (dead D1 is no longer the authority). `upsertSparkPayoutTarget`
   * is the idempotent-by-pylon_ref upsert; the two reads back the settlement
   * resolver directly off Postgres. Raw Spark addresses are copied
   * byte-exactly and NEVER enter a projection or diagnostic.
   */
  upsertSparkPayoutTarget: (
    record: PylonSparkPayoutTargetRecord,
  ) => Promise<PylonSparkPayoutTargetRecord>
  readSparkPayoutTarget: (
    pylonRef: string,
  ) => Promise<PylonSparkPayoutTargetRecord | undefined>
  readSparkPayoutTargetByOwner: (
    ownerAgentUserId: string,
  ) => Promise<PylonSparkPayoutTargetRecord | undefined>
  mirrorStaleSweep: (
    assignmentRefs: ReadonlyArray<string>,
    nowIso: string,
  ) => Promise<void>
}>

export type MakePostgresPylonDispatchStoreDependencies = Readonly<{
  /**
   * Acquire a transaction-mode-safe SQL client (Hyperdrive in production,
   * a direct local URL in tests). One client per store operation; always
   * ended, even on error — the same discipline as the push route.
   */
  acquireSql: () => Promise<KhalaSyncPushSqlClient>
}>

const uniqueRefsInOrder = (
  refs: ReadonlyArray<string>,
): ReadonlyArray<string> => Array.from(new Set(refs))

type SparkPayoutTargetRow = Readonly<{
  pylon_ref: string
  owner_agent_user_id: string
  payout_target_ref: string
  raw_spark_address: string
  created_at: string
  updated_at: string
}>

const rowToSparkPayoutTarget = (
  row: SparkPayoutTargetRow | undefined,
): PylonSparkPayoutTargetRecord | undefined =>
  row === undefined
    ? undefined
    : {
        pylonRef: row.pylon_ref,
        ownerAgentUserId: row.owner_agent_user_id,
        payoutTargetRef: row.payout_target_ref,
        rawSparkAddress: row.raw_spark_address,
        createdAt: row.created_at,
        updatedAt: row.updated_at,
      }

export const makePostgresPylonDispatchStore = (
  deps: MakePostgresPylonDispatchStoreDependencies,
): PostgresPylonDispatchStore => {
  const withSql = async <A>(fn: (sql: SyncSql) => Promise<A>): Promise<A> => {
    const client = await deps.acquireSql()
    try {
      return await fn(client.sql)
    } finally {
      try {
        await client.end()
      } catch {
        // best-effort teardown, same discipline as the push route.
      }
    }
  }

  const activeStates = [...ACTIVE_LEASE_ASSIGNMENT_STATES]

  const readAssignmentBy = async (
    sql: SyncSql,
    column: 'assignment_ref' | 'idempotency_key_hash',
    value: string,
  ): Promise<PylonApiAssignmentRecord | undefined> => {
    const rows: Array<PylonApiAssignmentRow> =
      column === 'assignment_ref'
        ? await sql`
            SELECT * FROM pylon_assignments
             WHERE assignment_ref = ${value} AND archived_at IS NULL
             LIMIT 1`
        : await sql`
            SELECT * FROM pylon_assignments
             WHERE idempotency_key_hash = ${value} AND archived_at IS NULL
             LIMIT 1`
    const row = rows[0]
    return row === undefined ? undefined : rowToAssignment(row)
  }

  const readEventByHash = async (
    sql: SyncSql,
    idempotencyKeyHash: string,
  ): Promise<PylonApiEventRecord | undefined> => {
    const rows: Array<PylonApiEventRow> = await sql`
      SELECT * FROM pylon_assignment_events
       WHERE idempotency_key_hash = ${idempotencyKeyHash}
         AND archived_at IS NULL
       LIMIT 1`
    const row = rows[0]
    return row === undefined ? undefined : rowToEvent(row)
  }

  const readRegistrationByRef = async (
    sql: SyncSql,
    pylonRef: string,
  ): Promise<PylonApiRegistrationRecord | undefined> => {
    const rows: Array<PylonApiRegistrationRow> = await sql`
      SELECT * FROM pylon_registrations
       WHERE pylon_ref = ${pylonRef} AND archived_at IS NULL
       LIMIT 1`
    const row = rows[0]
    return row === undefined ? undefined : rowToRegistration(row)
  }

  const insertAssignment = async (
    sql: SyncSql,
    record: PylonApiAssignmentRecord,
    onConflict: 'nothing' | 'converge',
  ): Promise<number> => {
    const codingAssignmentJson =
      record.codingAssignment === null
        ? null
        : JSON.stringify(record.codingAssignment)
    const conflictAction =
      onConflict === 'nothing'
        ? sql`
            INSERT INTO pylon_assignments
              (id, assignment_ref, pylon_ref, owner_agent_user_id,
               idempotency_key_hash, job_kind, state, payment_mode,
               lease_expires_at, task_refs_json, acceptance_criteria_refs_json,
               result_expectation_refs_json, artifact_refs_json,
               proof_refs_json, accepted_work_refs_json, rejection_refs_json,
               closeout_refs_json, coding_assignment_json,
               public_projection_json, created_at, updated_at, archived_at)
            VALUES
              (${record.id}, ${record.assignmentRef}, ${record.pylonRef},
               ${record.ownerAgentUserId}, ${record.idempotencyKeyHash},
               ${record.jobKind}, ${record.state},
               ${pylonApiAssignmentPaymentMode(record)},
               ${record.leaseExpiresAt}, ${JSON.stringify(record.taskRefs)},
               ${JSON.stringify(record.acceptanceCriteriaRefs)},
               ${JSON.stringify(record.resultExpectationRefs)},
               ${JSON.stringify(record.artifactRefs)},
               ${JSON.stringify(record.proofRefs)},
               ${JSON.stringify(record.acceptedWorkRefs)},
               ${JSON.stringify(record.rejectionRefs)},
               ${JSON.stringify(record.closeoutRefs)},
               ${codingAssignmentJson}, ${record.publicProjectionJson},
               ${record.createdAt}, ${record.updatedAt}, NULL)
            ON CONFLICT (idempotency_key_hash) DO NOTHING
            RETURNING assignment_ref`
        : sql`
            INSERT INTO pylon_assignments
              (id, assignment_ref, pylon_ref, owner_agent_user_id,
               idempotency_key_hash, job_kind, state, payment_mode,
               lease_expires_at, task_refs_json, acceptance_criteria_refs_json,
               result_expectation_refs_json, artifact_refs_json,
               proof_refs_json, accepted_work_refs_json, rejection_refs_json,
               closeout_refs_json, coding_assignment_json,
               public_projection_json, created_at, updated_at, archived_at)
            VALUES
              (${record.id}, ${record.assignmentRef}, ${record.pylonRef},
               ${record.ownerAgentUserId}, ${record.idempotencyKeyHash},
               ${record.jobKind}, ${record.state},
               ${pylonApiAssignmentPaymentMode(record)},
               ${record.leaseExpiresAt}, ${JSON.stringify(record.taskRefs)},
               ${JSON.stringify(record.acceptanceCriteriaRefs)},
               ${JSON.stringify(record.resultExpectationRefs)},
               ${JSON.stringify(record.artifactRefs)},
               ${JSON.stringify(record.proofRefs)},
               ${JSON.stringify(record.acceptedWorkRefs)},
               ${JSON.stringify(record.rejectionRefs)},
               ${JSON.stringify(record.closeoutRefs)},
               ${codingAssignmentJson}, ${record.publicProjectionJson},
               ${record.createdAt}, ${record.updatedAt}, NULL)
            ON CONFLICT (assignment_ref) DO UPDATE SET
              id = EXCLUDED.id,
              pylon_ref = EXCLUDED.pylon_ref,
              owner_agent_user_id = EXCLUDED.owner_agent_user_id,
              idempotency_key_hash = EXCLUDED.idempotency_key_hash,
              job_kind = EXCLUDED.job_kind,
              state = EXCLUDED.state,
              payment_mode = EXCLUDED.payment_mode,
              lease_expires_at = EXCLUDED.lease_expires_at,
              task_refs_json = EXCLUDED.task_refs_json,
              acceptance_criteria_refs_json = EXCLUDED.acceptance_criteria_refs_json,
              result_expectation_refs_json = EXCLUDED.result_expectation_refs_json,
              artifact_refs_json = EXCLUDED.artifact_refs_json,
              proof_refs_json = EXCLUDED.proof_refs_json,
              accepted_work_refs_json = EXCLUDED.accepted_work_refs_json,
              rejection_refs_json = EXCLUDED.rejection_refs_json,
              closeout_refs_json = EXCLUDED.closeout_refs_json,
              coding_assignment_json = EXCLUDED.coding_assignment_json,
              public_projection_json = EXCLUDED.public_projection_json,
              created_at = EXCLUDED.created_at,
              updated_at = EXCLUDED.updated_at,
              archived_at = NULL
            RETURNING assignment_ref`
    const rows: Array<{ assignment_ref: string }> = await conflictAction
    return rows.length
  }

  const insertRegistration = async (
    sql: SyncSql,
    record: PylonApiRegistrationRecord,
    onConflict: 'nothing' | 'converge',
  ): Promise<number> => {
    const values = {
      capabilityRefsJson: JSON.stringify(record.capabilityRefs),
      latestCapacityRefsJson: JSON.stringify(record.latestCapacityRefs),
      latestHealthRefsJson: JSON.stringify(record.latestHealthRefs),
      latestLoadRefsJson: JSON.stringify(record.latestLoadRefs),
      providerMarketRelayRefsJson: JSON.stringify(
        record.providerMarketRelayRefs,
      ),
      providerNip90LaneRefsJson: JSON.stringify(record.providerNip90LaneRefs),
      walletReady: record.walletReady ? 1 : 0,
    }
    const rows: Array<{ pylon_ref: string }> =
      onConflict === 'nothing'
        ? await sql`
            INSERT INTO pylon_registrations
              (id, pylon_ref, owner_agent_user_id, owner_agent_credential_id,
               owner_agent_token_prefix, display_name, status, resource_mode,
               capability_refs_json, client_version, client_protocol_version,
               wallet_ref, wallet_ready, latest_heartbeat_at,
               latest_heartbeat_status, latest_resource_mode,
               latest_health_refs_json, latest_load_refs_json,
               latest_capacity_refs_json, provider_nostr_pubkey,
               provider_nostr_npub, provider_market_relay_refs_json,
               provider_nip90_lane_refs_json, public_projection_json,
               created_at, updated_at, archived_at)
            VALUES
              (${record.id}, ${record.pylonRef}, ${record.ownerAgentUserId},
               ${record.ownerAgentCredentialId}, ${record.ownerAgentTokenPrefix},
               ${record.displayName}, ${record.status}, ${record.resourceMode},
               ${values.capabilityRefsJson}, ${record.clientVersion},
               ${record.clientProtocolVersion}, ${record.walletRef},
               ${values.walletReady}, ${record.latestHeartbeatAt},
               ${record.latestHeartbeatStatus}, ${record.latestResourceMode},
               ${values.latestHealthRefsJson}, ${values.latestLoadRefsJson},
               ${values.latestCapacityRefsJson}, ${record.providerNostrPubkey},
               ${record.providerNostrNpub}, ${values.providerMarketRelayRefsJson},
               ${values.providerNip90LaneRefsJson}, ${record.publicProjectionJson},
               ${record.createdAt}, ${record.updatedAt}, NULL)
            ON CONFLICT (pylon_ref) DO NOTHING
            RETURNING pylon_ref`
        : await sql`
            INSERT INTO pylon_registrations
              (id, pylon_ref, owner_agent_user_id, owner_agent_credential_id,
               owner_agent_token_prefix, display_name, status, resource_mode,
               capability_refs_json, client_version, client_protocol_version,
               wallet_ref, wallet_ready, latest_heartbeat_at,
               latest_heartbeat_status, latest_resource_mode,
               latest_health_refs_json, latest_load_refs_json,
               latest_capacity_refs_json, provider_nostr_pubkey,
               provider_nostr_npub, provider_market_relay_refs_json,
               provider_nip90_lane_refs_json, public_projection_json,
               created_at, updated_at, archived_at)
            VALUES
              (${record.id}, ${record.pylonRef}, ${record.ownerAgentUserId},
               ${record.ownerAgentCredentialId}, ${record.ownerAgentTokenPrefix},
               ${record.displayName}, ${record.status}, ${record.resourceMode},
               ${values.capabilityRefsJson}, ${record.clientVersion},
               ${record.clientProtocolVersion}, ${record.walletRef},
               ${values.walletReady}, ${record.latestHeartbeatAt},
               ${record.latestHeartbeatStatus}, ${record.latestResourceMode},
               ${values.latestHealthRefsJson}, ${values.latestLoadRefsJson},
               ${values.latestCapacityRefsJson}, ${record.providerNostrPubkey},
               ${record.providerNostrNpub}, ${values.providerMarketRelayRefsJson},
               ${values.providerNip90LaneRefsJson}, ${record.publicProjectionJson},
               ${record.createdAt}, ${record.updatedAt}, NULL)
            ON CONFLICT (pylon_ref) DO UPDATE SET
              id = EXCLUDED.id,
              owner_agent_user_id = EXCLUDED.owner_agent_user_id,
              owner_agent_credential_id = EXCLUDED.owner_agent_credential_id,
              owner_agent_token_prefix = EXCLUDED.owner_agent_token_prefix,
              display_name = EXCLUDED.display_name,
              status = EXCLUDED.status,
              resource_mode = EXCLUDED.resource_mode,
              capability_refs_json = EXCLUDED.capability_refs_json,
              client_version = EXCLUDED.client_version,
              client_protocol_version = EXCLUDED.client_protocol_version,
              wallet_ref = EXCLUDED.wallet_ref,
              wallet_ready = EXCLUDED.wallet_ready,
              latest_heartbeat_at = EXCLUDED.latest_heartbeat_at,
              latest_heartbeat_status = EXCLUDED.latest_heartbeat_status,
              latest_resource_mode = EXCLUDED.latest_resource_mode,
              latest_health_refs_json = EXCLUDED.latest_health_refs_json,
              latest_load_refs_json = EXCLUDED.latest_load_refs_json,
              latest_capacity_refs_json = EXCLUDED.latest_capacity_refs_json,
              provider_nostr_pubkey = EXCLUDED.provider_nostr_pubkey,
              provider_nostr_npub = EXCLUDED.provider_nostr_npub,
              provider_market_relay_refs_json = EXCLUDED.provider_market_relay_refs_json,
              provider_nip90_lane_refs_json = EXCLUDED.provider_nip90_lane_refs_json,
              public_projection_json = EXCLUDED.public_projection_json,
              created_at = EXCLUDED.created_at,
              updated_at = EXCLUDED.updated_at,
              archived_at = NULL
            RETURNING pylon_ref`
    return rows.length
  }

  const insertEvent = async (
    sql: SyncSql,
    record: PylonApiEventRecord,
  ): Promise<number> => {
    const rows: Array<{ event_ref: string }> = await sql`
      INSERT INTO pylon_assignment_events
        (id, event_ref, pylon_ref, owner_agent_user_id, idempotency_key_hash,
         event_kind, assignment_ref, status, event_body_json,
         public_projection_json, created_at, archived_at)
      VALUES
        (${record.id}, ${record.eventRef}, ${record.pylonRef},
         ${record.ownerAgentUserId}, ${record.idempotencyKeyHash},
         ${record.eventKind}, ${record.assignmentRef}, ${record.status},
         ${JSON.stringify(record.eventBody)}, ${record.publicProjectionJson},
         ${record.createdAt}, NULL)
      ON CONFLICT (idempotency_key_hash) DO NOTHING
      RETURNING event_ref`
    return rows.length
  }

  const insertProviderJobLifecycle = async (
    sql: SyncSql,
    record: PylonApiProviderJobLifecycleRecord,
  ): Promise<number> => {
    const rows: Array<{ assignment_ref: string }> = await sql`
      INSERT INTO pylon_provider_job_lifecycle
        (id, pylon_ref, assignment_ref, owner_agent_user_id, job_kind, stage,
         task_refs_json, artifact_refs_json, proof_refs_json, closeout_refs_json,
         accepted_work_refs_json, public_projection_json, created_at,
         updated_at, archived_at)
      VALUES
        (${record.id}, ${record.pylonRef}, ${record.assignmentRef},
         ${record.ownerAgentUserId}, ${record.jobKind}, ${record.stage},
         ${JSON.stringify(record.taskRefs)},
         ${JSON.stringify(record.artifactRefs)},
         ${JSON.stringify(record.proofRefs)},
         ${JSON.stringify(record.closeoutRefs)},
         ${JSON.stringify(record.acceptedWorkRefs)},
         ${record.publicProjectionJson}, ${record.createdAt},
         ${record.updatedAt}, NULL)
      ON CONFLICT (assignment_ref) DO UPDATE SET
        pylon_ref = EXCLUDED.pylon_ref,
        owner_agent_user_id = EXCLUDED.owner_agent_user_id,
        job_kind = EXCLUDED.job_kind,
        stage = EXCLUDED.stage,
        task_refs_json = EXCLUDED.task_refs_json,
        artifact_refs_json = EXCLUDED.artifact_refs_json,
        proof_refs_json = EXCLUDED.proof_refs_json,
        closeout_refs_json = EXCLUDED.closeout_refs_json,
        accepted_work_refs_json = EXCLUDED.accepted_work_refs_json,
        public_projection_json = EXCLUDED.public_projection_json,
        updated_at = EXCLUDED.updated_at,
        archived_at = NULL
      RETURNING assignment_ref`
    return rows.length
  }

  const insertQuarantine = async (
    sql: SyncSql,
    record: PylonApiQuarantineRecord,
  ): Promise<number> => {
    const rows: Array<{ quarantine_ref: string }> = await sql`
      INSERT INTO pylon_quarantines
        (id, quarantine_ref, pylon_ref, owner_agent_user_id, state,
         reason_refs_json, action_refs_json, source_refs_json, expires_at,
         released_at, public_projection_json, created_at, updated_at,
         archived_at)
      VALUES
        (${record.id}, ${record.quarantineRef}, ${record.pylonRef},
         ${record.ownerAgentUserId}, ${record.state},
         ${JSON.stringify(record.reasonRefs)},
         ${JSON.stringify(record.actionRefs)},
         ${JSON.stringify(record.sourceRefs)}, ${record.expiresAt},
         ${record.releasedAt}, ${record.publicProjectionJson},
         ${record.createdAt}, ${record.updatedAt}, NULL)
      ON CONFLICT (quarantine_ref) DO UPDATE SET
        owner_agent_user_id = EXCLUDED.owner_agent_user_id,
        state = EXCLUDED.state,
        reason_refs_json = EXCLUDED.reason_refs_json,
        action_refs_json = EXCLUDED.action_refs_json,
        source_refs_json = EXCLUDED.source_refs_json,
        expires_at = EXCLUDED.expires_at,
        released_at = EXCLUDED.released_at,
        public_projection_json = EXCLUDED.public_projection_json,
        updated_at = EXCLUDED.updated_at,
        archived_at = NULL
      RETURNING quarantine_ref`
    return rows.length
  }

  const insertSparkPayoutTarget = async (
    sql: SyncSql,
    record: PylonSparkPayoutTargetRecord,
  ): Promise<number> => {
    const rows: Array<{ pylon_ref: string }> = await sql`
      INSERT INTO pylon_spark_payout_targets
        (pylon_ref, owner_agent_user_id, payout_target_ref, raw_spark_address,
         created_at, updated_at)
      VALUES
        (${record.pylonRef}, ${record.ownerAgentUserId},
         ${record.payoutTargetRef}, ${record.rawSparkAddress},
         ${record.createdAt}, ${record.updatedAt})
      ON CONFLICT (pylon_ref) DO UPDATE SET
        owner_agent_user_id = EXCLUDED.owner_agent_user_id,
        payout_target_ref = EXCLUDED.payout_target_ref,
        raw_spark_address = EXCLUDED.raw_spark_address,
        updated_at = EXCLUDED.updated_at
      RETURNING pylon_ref`
    return rows.length
  }

  const updateAssignmentRow = async (
    sql: SyncSql,
    record: PylonApiAssignmentRecord,
    publicProjectionJson: string,
    expectedState: PylonApiAssignmentState | undefined,
  ): Promise<number> => {
    const codingAssignmentJson =
      record.codingAssignment === null
        ? null
        : JSON.stringify(record.codingAssignment)
    const rows: Array<{ assignment_ref: string }> =
      expectedState === undefined
        ? await sql`
            UPDATE pylon_assignments SET
              state = ${record.state},
              payment_mode = ${pylonApiAssignmentPaymentMode(record)},
              lease_expires_at = ${record.leaseExpiresAt},
              artifact_refs_json = ${JSON.stringify(record.artifactRefs)},
              proof_refs_json = ${JSON.stringify(record.proofRefs)},
              accepted_work_refs_json = ${JSON.stringify(record.acceptedWorkRefs)},
              rejection_refs_json = ${JSON.stringify(record.rejectionRefs)},
              closeout_refs_json = ${JSON.stringify(record.closeoutRefs)},
              coding_assignment_json = ${codingAssignmentJson},
              public_projection_json = ${publicProjectionJson},
              updated_at = ${record.updatedAt}
            WHERE assignment_ref = ${record.assignmentRef}
              AND pylon_ref = ${record.pylonRef}
              AND archived_at IS NULL
            RETURNING assignment_ref`
        : await sql`
            UPDATE pylon_assignments SET
              state = ${record.state},
              payment_mode = ${pylonApiAssignmentPaymentMode(record)},
              lease_expires_at = ${record.leaseExpiresAt},
              artifact_refs_json = ${JSON.stringify(record.artifactRefs)},
              proof_refs_json = ${JSON.stringify(record.proofRefs)},
              accepted_work_refs_json = ${JSON.stringify(record.acceptedWorkRefs)},
              rejection_refs_json = ${JSON.stringify(record.rejectionRefs)},
              closeout_refs_json = ${JSON.stringify(record.closeoutRefs)},
              coding_assignment_json = ${codingAssignmentJson},
              public_projection_json = ${publicProjectionJson},
              updated_at = ${record.updatedAt}
            WHERE assignment_ref = ${record.assignmentRef}
              AND pylon_ref = ${record.pylonRef}
              AND state = ${expectedState}
              AND archived_at IS NULL
            RETURNING assignment_ref`
    return rows.length
  }

  return {
    createAssignment: record =>
      withSql(async sql => {
        const inserted = await insertAssignment(sql, record, 'nothing')
        if (inserted > 0) {
          await insertProviderJobLifecycle(
            sql,
            providerJobLifecycleRecordFromAssignment(record),
          )
          return { idempotent: false, record }
        }
        const existing = await readAssignmentBy(
          sql,
          'idempotency_key_hash',
          record.idempotencyKeyHash,
        )
        if (existing === undefined) {
          throw new PylonApiStoreError({
            kind: 'storage_error',
            reason: 'Pylon assignment write did not return a stored row.',
          })
        }
        return { idempotent: true, record: existing }
      }),

    createEvent: record =>
      withSql(async sql => {
        await insertEvent(sql, record)
        const stored = await readEventByHash(sql, record.idempotencyKeyHash)
        if (stored === undefined) {
          throw new PylonApiStoreError({
            kind: 'storage_error',
            reason: 'Pylon event write did not return a stored event.',
          })
        }
        return { idempotent: stored.id !== record.id, record: stored }
      }),

    listAssignmentsForPylon: (pylonRef, limit) =>
      withSql(async sql => {
        const rows: Array<PylonApiAssignmentRow> = await sql`
          SELECT * FROM pylon_assignments
           WHERE pylon_ref = ${pylonRef}
             AND state = ANY(${activeStates})
             AND archived_at IS NULL
           ORDER BY updated_at DESC
           LIMIT ${limit}`
        return rows.map(rowToAssignment)
      }),

    listAssignmentsForPylons: (pylonRefs, limit) =>
      pylonRefs.length === 0
        ? Promise.resolve([])
        : withSql(async sql => {
            const rows: Array<PylonApiAssignmentRow> = await sql`
              SELECT * FROM pylon_assignments
               WHERE pylon_ref = ANY(${uniqueRefsInOrder(pylonRefs)})
                 AND state = ANY(${activeStates})
                 AND archived_at IS NULL
               ORDER BY updated_at DESC
               LIMIT ${limit}`
            return rows.map(rowToAssignment)
          }),

    listEventsForPylon: (pylonRef, limit) =>
      withSql(async sql => {
        const rows: Array<PylonApiEventRow> = await sql`
          SELECT * FROM pylon_assignment_events
           WHERE pylon_ref = ${pylonRef}
             AND archived_at IS NULL
           ORDER BY created_at DESC
           LIMIT ${limit}`
        return rows.map(rowToEvent)
      }),

    listEventsForAssignment: (assignmentRef, limit) =>
      withSql(async sql => {
        const rows: Array<PylonApiEventRow> = await sql`
          SELECT * FROM pylon_assignment_events
           WHERE assignment_ref = ${assignmentRef}
             AND archived_at IS NULL
           ORDER BY created_at DESC
           LIMIT ${limit}`
        return rows.map(rowToEvent)
      }),

    listRegistrations: limit =>
      withSql(async sql => {
        const rows: Array<PylonApiRegistrationRow> = await sql`
          SELECT * FROM pylon_registrations
           WHERE archived_at IS NULL
           ORDER BY updated_at DESC
           LIMIT ${limit}`
        return rows.map(rowToRegistration)
      }),

    listRegistrationsForOwnerAgentUserIds: (ownerAgentUserIds, limit) => {
      if (ownerAgentUserIds.length === 0) {
        return Promise.resolve([])
      }
      const boundedLimit = Math.max(1, Math.min(Math.trunc(limit), 200))
      const boundedIds = uniqueRefsInOrder(ownerAgentUserIds).slice(0, 200)
      return withSql(async sql => {
        const rows: Array<PylonApiRegistrationRow> = await sql`
          SELECT * FROM pylon_registrations
           WHERE owner_agent_user_id = ANY(${boundedIds})
             AND archived_at IS NULL
           ORDER BY updated_at DESC
           LIMIT ${boundedLimit}`
        return rows.map(rowToRegistration)
      })
    },

    listProviderJobLifecycleForPylons: (pylonRefs, limit) =>
      pylonRefs.length === 0
        ? Promise.resolve([])
        : withSql(async sql => {
            const rows: Array<{
              accepted_work_refs_json: string
              artifact_refs_json: string
              assignment_ref: string
              closeout_refs_json: string
              created_at: string
              id: string
              job_kind: PylonApiAssignmentRecord['jobKind']
              owner_agent_user_id: string
              proof_refs_json: string
              public_projection_json: string
              pylon_ref: string
              stage: PylonApiProviderJobLifecycleRecord['stage']
              task_refs_json: string
              updated_at: string
            }> = await sql`
              SELECT * FROM pylon_provider_job_lifecycle
               WHERE pylon_ref = ANY(${uniqueRefsInOrder(pylonRefs)})
                 AND archived_at IS NULL
               ORDER BY updated_at DESC
               LIMIT ${limit}`
            return rows.map(row => ({
              acceptedWorkRefs: parseJsonStringArray(
                row.accepted_work_refs_json,
              ),
              artifactRefs: parseJsonStringArray(row.artifact_refs_json),
              assignmentRef: row.assignment_ref,
              closeoutRefs: parseJsonStringArray(row.closeout_refs_json),
              createdAt: row.created_at,
              id: row.id,
              jobKind: row.job_kind,
              ownerAgentUserId: row.owner_agent_user_id,
              proofRefs: parseJsonStringArray(row.proof_refs_json),
              publicProjectionJson: row.public_projection_json,
              pylonRef: row.pylon_ref,
              stage: row.stage,
              taskRefs: parseJsonStringArray(row.task_refs_json),
              updatedAt: row.updated_at,
            }))
          }),

    readAssignment: assignmentRef =>
      withSql(sql => readAssignmentBy(sql, 'assignment_ref', assignmentRef)),

    readAssignmentByIdempotencyKeyHash: idempotencyKeyHash =>
      withSql(sql =>
        readAssignmentBy(sql, 'idempotency_key_hash', idempotencyKeyHash),
      ),

    readEventByIdempotencyKeyHash: idempotencyKeyHash =>
      withSql(sql => readEventByHash(sql, idempotencyKeyHash)),

    readRegistration: pylonRef =>
      withSql(sql => readRegistrationByRef(sql, pylonRef)),

    sweepStaleAssignmentLeases: (pylonRef, nowIso, staleBeforeIso) =>
      withSql(async sql => {
        // One statement where D1 needs SELECT-then-UPDATE: the swept refs
        // come straight from the UPDATE (no TOCTOU window).
        const rows: Array<{ assignment_ref: string }> = await sql`
          UPDATE pylon_assignments SET
            state = 'stale',
            lease_expires_at = ${nowIso},
            updated_at = ${nowIso}
          WHERE pylon_ref = ${pylonRef}
            AND state = ANY(${activeStates})
            AND lease_expires_at > ${nowIso}
            AND updated_at < ${staleBeforeIso}
            AND archived_at IS NULL
          RETURNING assignment_ref`
        return rows.map(row => row.assignment_ref)
      }),

    updateAssignment: record =>
      withSql(async sql => {
        const publicProjectionJson = JSON.stringify(
          publicPylonApiAssignmentProjection(record, record.updatedAt),
        )
        const next = { ...record, publicProjectionJson }
        await updateAssignmentRow(sql, next, publicProjectionJson, undefined)
        await insertProviderJobLifecycle(
          sql,
          providerJobLifecycleRecordFromAssignment(next),
        )
        return next
      }),

    updateAssignmentIfState: (record, expectedState) =>
      withSql(async sql => {
        const publicProjectionJson = JSON.stringify(
          publicPylonApiAssignmentProjection(record, record.updatedAt),
        )
        const next = { ...record, publicProjectionJson }
        const changed = await updateAssignmentRow(
          sql,
          next,
          publicProjectionJson,
          expectedState,
        )
        if (changed < 1) {
          return undefined
        }
        await insertProviderJobLifecycle(
          sql,
          providerJobLifecycleRecordFromAssignment(next),
        )
        return next
      }),

    upsertProviderJobLifecycle: record =>
      withSql(async sql => {
        await insertProviderJobLifecycle(sql, record)
        return record
      }),

    upsertQuarantine: record =>
      withSql(async sql => {
        await insertQuarantine(sql, record)
        return record
      }),

    upsertRegistration: (record, options) =>
      withSql(async sql => {
        const inserted = await insertRegistration(sql, record, 'nothing')
        if (inserted > 0) {
          return record
        }
        const existing = await readRegistrationByRef(sql, record.pylonRef)
        if (existing === undefined) {
          throw new PylonApiStoreError({
            kind: 'storage_error',
            reason: 'Pylon registration write did not return a stored row.',
          })
        }
        if (
          existing.ownerAgentUserId !== record.ownerAgentUserId &&
          options?.allowOwnerTransferFrom !== existing.ownerAgentUserId
        ) {
          throw new PylonApiStoreError({
            kind: 'conflict',
            reason: 'Pylon ref is already owned by another registered agent.',
          })
        }
        const next: PylonApiRegistrationRecord = {
          ...record,
          createdAt: existing.createdAt,
          id: existing.id,
        }
        const publicProjectionJson = JSON.stringify(
          publicPylonApiRegistrationProjection(next, record.updatedAt),
        )
        await sql`
          UPDATE pylon_registrations SET
            owner_agent_user_id = ${record.ownerAgentUserId},
            owner_agent_credential_id = ${record.ownerAgentCredentialId},
            owner_agent_token_prefix = ${record.ownerAgentTokenPrefix},
            display_name = ${record.displayName},
            status = ${record.status},
            resource_mode = ${record.resourceMode},
            capability_refs_json = ${JSON.stringify(record.capabilityRefs)},
            client_version = ${record.clientVersion},
            client_protocol_version = ${record.clientProtocolVersion},
            wallet_ref = ${record.walletRef},
            wallet_ready = ${record.walletReady ? 1 : 0},
            latest_heartbeat_at = ${record.latestHeartbeatAt},
            latest_heartbeat_status = ${record.latestHeartbeatStatus},
            latest_resource_mode = ${record.latestResourceMode},
            latest_health_refs_json = ${JSON.stringify(record.latestHealthRefs)},
            latest_load_refs_json = ${JSON.stringify(record.latestLoadRefs)},
            latest_capacity_refs_json = ${JSON.stringify(record.latestCapacityRefs)},
            provider_nostr_pubkey = ${record.providerNostrPubkey},
            provider_nostr_npub = ${record.providerNostrNpub},
            provider_market_relay_refs_json = ${JSON.stringify(record.providerMarketRelayRefs)},
            provider_nip90_lane_refs_json = ${JSON.stringify(record.providerNip90LaneRefs)},
            public_projection_json = ${publicProjectionJson},
            updated_at = ${record.updatedAt}
          WHERE pylon_ref = ${record.pylonRef}
            AND owner_agent_user_id = ${existing.ownerAgentUserId}
            AND archived_at IS NULL`
        return { ...next, publicProjectionJson }
      }),

    mirrorAssignment: record =>
      withSql(async sql => {
        await insertAssignment(sql, record, 'converge')
        await insertProviderJobLifecycle(
          sql,
          providerJobLifecycleRecordFromAssignment(record),
        )
      }),

    mirrorEvent: record =>
      withSql(async sql => {
        await insertEvent(sql, record)
      }),

    mirrorProviderJobLifecycle: record =>
      withSql(async sql => {
        await insertProviderJobLifecycle(sql, record)
      }),

    mirrorQuarantine: record =>
      withSql(async sql => {
        await insertQuarantine(sql, record)
      }),

    mirrorRegistration: record =>
      withSql(async sql => {
        await insertRegistration(sql, record, 'converge')
      }),

    mirrorSparkPayoutTarget: record =>
      withSql(async sql => {
        await insertSparkPayoutTarget(sql, record)
      }),

    // #8515 WRITE cutover: Postgres-authoritative Spark payout target ops.
    upsertSparkPayoutTarget: record =>
      withSql(async sql => {
        await insertSparkPayoutTarget(sql, record)
        return record
      }),

    readSparkPayoutTarget: pylonRef =>
      withSql(async sql => {
        const rows: Array<SparkPayoutTargetRow> = await sql`
          SELECT pylon_ref, owner_agent_user_id, payout_target_ref,
                 raw_spark_address, created_at, updated_at
            FROM pylon_spark_payout_targets
           WHERE pylon_ref = ${pylonRef}
           LIMIT 1`
        return rowToSparkPayoutTarget(rows[0])
      }),

    readSparkPayoutTargetByOwner: ownerAgentUserId =>
      withSql(async sql => {
        const rows: Array<SparkPayoutTargetRow> = await sql`
          SELECT pylon_ref, owner_agent_user_id, payout_target_ref,
                 raw_spark_address, created_at, updated_at
            FROM pylon_spark_payout_targets
           WHERE owner_agent_user_id = ${ownerAgentUserId}
           ORDER BY updated_at DESC
           LIMIT 1`
        return rowToSparkPayoutTarget(rows[0])
      }),

    mirrorStaleSweep: (assignmentRefs, nowIso) =>
      assignmentRefs.length === 0
        ? Promise.resolve()
        : withSql(async sql => {
            await sql`
              UPDATE pylon_assignments SET
                state = 'stale',
                lease_expires_at = ${nowIso},
                updated_at = ${nowIso}
              WHERE assignment_ref = ANY(${[...assignmentRefs]})
                AND archived_at IS NULL`
          }),
  }
}

// ---------------------------------------------------------------------------
// Dual-write / flag-routed wrapper
// ---------------------------------------------------------------------------

export type MakeDualWritePylonApiStoreDependencies = Readonly<{
  /** The authoritative D1 store (extracted, behavior-identical). */
  d1: PylonApiStore
  /** The Postgres store, or undefined when no KHALA_SYNC_DB binding. */
  postgres: PostgresPylonDispatchStore | undefined
  flags: PylonDispatchFlags
  log?: PylonDispatchLog | undefined
  /**
   * Bounded-retry backoff hook (tests inject a no-op). Default: small
   * real delays (50ms, 150ms) — sane for a Worker request path.
   */
  wait?: ((ms: number) => Promise<void>) | undefined
}>

export type MakeDualWritePylonSparkPayoutTargetStoreDependencies = Readonly<{
  d1: PylonSparkPayoutTargetStore
  postgres:
    | Pick<
        PostgresPylonDispatchStore,
        | 'mirrorSparkPayoutTarget'
        | 'upsertSparkPayoutTarget'
        | 'readSparkPayoutTarget'
        | 'readSparkPayoutTargetByOwner'
      >
    | undefined
  flags: Pick<PylonDispatchFlags, 'dualWrite' | 'writes'>
  log?: PylonDispatchLog | undefined
}>

const READ_RETRY_DELAYS_MS: ReadonlyArray<number> = [50, 150]

const stableStringify = (value: unknown): string =>
  JSON.stringify(value, (_key, val: unknown) =>
    val !== null && typeof val === 'object' && !Array.isArray(val)
      ? Object.fromEntries(
          Object.entries(val as Record<string, unknown>).sort(([a], [b]) =>
            a < b ? -1 : a > b ? 1 : 0,
          ),
        )
      : val,
  )

/**
 * The production `PylonApiStore` for the assignments/dispatch domain during
 * the KS-8.1 migration. See the module doc for the write/read contract.
 */
export const makeDualWritePylonApiStore = (
  deps: MakeDualWritePylonApiStoreDependencies,
): PylonApiStore => {
  const { d1, flags, postgres } = deps
  const log = deps.log ?? (() => {})
  const wait =
    deps.wait ?? ((ms: number) => new Promise(resolve => setTimeout(resolve, ms)))

  if (postgres === undefined) {
    // No binding: plain D1, no routing, no mirroring — fail-safe.
    return d1
  }

  /** Best-effort Postgres mirror after the authoritative D1 write. */
  const mirror = (
    op: string,
    refs: ReadonlyArray<string>,
    run: () => Promise<void>,
  ): Promise<void> =>
    !flags.dualWrite
      ? Promise.resolve()
      : run().catch((error: unknown) => {
          log('khala_sync_pylon_dual_write_failed', {
            messageSafe: safeMessage(error),
            op,
            refs,
          })
        })

  /** Flag-routed read: d1 | postgres (bounded retry + D1 fallback) | compare. */
  const read = async <A>(
    op: string,
    refs: ReadonlyArray<string>,
    readD1: () => Promise<A>,
    readPostgres: () => Promise<A>,
  ): Promise<A> => {
    if (flags.reads === 'postgres') {
      // The June-29 headroom fix: dispatch-gate reads (owner registration +
      // capacity) get real concurrency on Postgres, with bounded retry —
      // D1 had none. On exhaustion, fall back to the still-dual-written D1
      // so public surfaces never regress mid-cutover (MIGRATION_PLAN §1.4).
      for (let attempt = 0; ; attempt++) {
        try {
          return await readPostgres()
        } catch (error) {
          const delay = READ_RETRY_DELAYS_MS[attempt]
          if (delay === undefined) {
            log('khala_sync_pylon_postgres_read_fallback', {
              messageSafe: safeMessage(error),
              op,
              refs,
            })
            return readD1()
          }
          log('khala_sync_pylon_postgres_read_failed', {
            messageSafe: safeMessage(error),
            op,
            refs,
          })
          await wait(delay)
        }
      }
    }

    if (flags.reads === 'compare') {
      const d1Result = await readD1()
      try {
        const postgresResult = await readPostgres()
        if (stableStringify(d1Result) !== stableStringify(postgresResult)) {
          log('khala_sync_pylon_read_compare_mismatch', {
            messageSafe: 'postgres read differs from d1 authority',
            op,
            refs,
          })
        }
      } catch (error) {
        log('khala_sync_pylon_postgres_read_failed', {
          messageSafe: safeMessage(error),
          op,
          refs,
        })
      }
      return d1Result
    }

    return readD1()
  }

  // #8515 WRITE cutover: when 'postgres', Postgres is the SOLE authority for
  // assignment/heartbeat/dispatch writes. D1 is not written at all (it 401s
  // account-wide — a mirror-back would only log failures and add latency), and
  // there is NO D1 fallback on a Postgres error: the request fails loud so a
  // Postgres outage cannot silently diverge state. In 'd1' mode the original
  // D1-first + best-effort Postgres-mirror path is preserved intact.
  const writesPostgres = flags.writes === 'postgres'

  return {
    // KS-8.4 read path remains D1-authoritative until #8315 cutover evidence.
    ...(d1.readActiveQuarantineForPylon === undefined
      ? {}
      : { readActiveQuarantineForPylon: d1.readActiveQuarantineForPylon }),
    ...(d1.upsertQuarantine === undefined
      ? {}
      : {
          upsertQuarantine: async (record: PylonApiQuarantineRecord) => {
            if (writesPostgres) {
              return postgres.upsertQuarantine(record)
            }
            const next = await d1.upsertQuarantine!(record)
            await mirror(
              'upsertQuarantine',
              [next.quarantineRef, next.pylonRef],
              () => postgres.mirrorQuarantine(next),
            )
            return next
          },
        }),

    createAssignment: async record => {
      if (writesPostgres) {
        return postgres.createAssignment(record)
      }
      const result = await d1.createAssignment(record)
      await mirror('createAssignment', [result.record.assignmentRef], () =>
        postgres.mirrorAssignment(result.record),
      )
      return result
    },

    createEvent: async record => {
      if (writesPostgres) {
        return postgres.createEvent(record)
      }
      const result = await d1.createEvent(record)
      await mirror('createEvent', [result.record.eventRef], () =>
        postgres.mirrorEvent(result.record),
      )
      return result
    },

    listAssignmentsForPylon: (pylonRef, limit) =>
      read(
        'listAssignmentsForPylon',
        [pylonRef],
        () => d1.listAssignmentsForPylon(pylonRef, limit),
        () => postgres.listAssignmentsForPylon(pylonRef, limit),
      ),

    listAssignmentsForPylons: (pylonRefs, limit) =>
      read(
        'listAssignmentsForPylons',
        pylonRefs,
        () =>
          d1.listAssignmentsForPylons === undefined
            ? Promise.resolve([])
            : d1.listAssignmentsForPylons(pylonRefs, limit),
        () => postgres.listAssignmentsForPylons(pylonRefs, limit),
      ),

    listEventsForPylon: (pylonRef, limit) =>
      read(
        'listEventsForPylon',
        [pylonRef],
        () => d1.listEventsForPylon(pylonRef, limit),
        () => postgres.listEventsForPylon(pylonRef, limit),
      ),

    listEventsForAssignment: (assignmentRef, limit) =>
      read(
        'listEventsForAssignment',
        [assignmentRef],
        () => d1.listEventsForAssignment(assignmentRef, limit),
        () => postgres.listEventsForAssignment(assignmentRef, limit),
      ),

    listRegistrations: limit =>
      read(
        'listRegistrations',
        [],
        () => d1.listRegistrations(limit),
        () => postgres.listRegistrations(limit),
      ),

    listRegistrationsForOwnerAgentUserIds: (ownerAgentUserIds, limit) =>
      read(
        'listRegistrationsForOwnerAgentUserIds',
        ownerAgentUserIds,
        () =>
          d1.listRegistrationsForOwnerAgentUserIds === undefined
            ? Promise.resolve([])
            : d1.listRegistrationsForOwnerAgentUserIds(
                ownerAgentUserIds,
                limit,
              ),
        () =>
          postgres.listRegistrationsForOwnerAgentUserIds(
            ownerAgentUserIds,
            limit,
          ),
      ),

    // KS-8.4 read path remains D1-authoritative until #8315 cutover evidence.
    listProviderJobLifecycleForPylons: (pylonRefs, limit) =>
      d1.listProviderJobLifecycleForPylons(pylonRefs, limit),

    readAssignment: assignmentRef =>
      read(
        'readAssignment',
        [assignmentRef],
        () => d1.readAssignment(assignmentRef),
        () => postgres.readAssignment(assignmentRef),
      ),

    readAssignmentByIdempotencyKeyHash: idempotencyKeyHash =>
      read(
        'readAssignmentByIdempotencyKeyHash',
        [],
        () => d1.readAssignmentByIdempotencyKeyHash(idempotencyKeyHash),
        () => postgres.readAssignmentByIdempotencyKeyHash(idempotencyKeyHash),
      ),

    readEventByIdempotencyKeyHash: idempotencyKeyHash =>
      read(
        'readEventByIdempotencyKeyHash',
        [],
        () => d1.readEventByIdempotencyKeyHash(idempotencyKeyHash),
        () => postgres.readEventByIdempotencyKeyHash(idempotencyKeyHash),
      ),

    readRegistration: pylonRef =>
      read(
        'readRegistration',
        [pylonRef],
        () => d1.readRegistration(pylonRef),
        () => postgres.readRegistration(pylonRef),
      ),

    sweepStaleAssignmentLeases: async (pylonRef, nowIso, staleBeforeIso) => {
      if (writesPostgres) {
        return postgres.sweepStaleAssignmentLeases(
          pylonRef,
          nowIso,
          staleBeforeIso,
        )
      }
      if (d1.sweepStaleAssignmentLeases === undefined) {
        return []
      }
      const sweptRefs = await d1.sweepStaleAssignmentLeases(
        pylonRef,
        nowIso,
        staleBeforeIso,
      )
      await mirror('sweepStaleAssignmentLeases', sweptRefs, () =>
        postgres.mirrorStaleSweep(sweptRefs, nowIso),
      )
      return sweptRefs
    },

    updateAssignment: async record => {
      if (writesPostgres) {
        return postgres.updateAssignment(record)
      }
      const next = await d1.updateAssignment(record)
      await mirror('updateAssignment', [next.assignmentRef], () =>
        postgres.mirrorAssignment(next),
      )
      return next
    },

    updateAssignmentIfState: async (record, expectedState) => {
      if (writesPostgres) {
        return postgres.updateAssignmentIfState(record, expectedState)
      }
      const next = await d1.updateAssignmentIfState(record, expectedState)
      if (next !== undefined) {
        await mirror('updateAssignmentIfState', [next.assignmentRef], () =>
          postgres.mirrorAssignment(next),
        )
      }
      return next
    },

    upsertProviderJobLifecycle: async record => {
      if (writesPostgres) {
        return postgres.upsertProviderJobLifecycle(record)
      }
      const next = await d1.upsertProviderJobLifecycle(record)
      await mirror(
        'upsertProviderJobLifecycle',
        [next.assignmentRef, next.pylonRef],
        () => postgres.mirrorProviderJobLifecycle(next),
      )
      return next
    },

    upsertRegistration: async (record, options) => {
      if (writesPostgres) {
        return postgres.upsertRegistration(record, options)
      }
      const next = await d1.upsertRegistration(record, options)
      await mirror('upsertRegistration', [next.pylonRef], () =>
        postgres.mirrorRegistration(next),
      )
      return next
    },
  }
}

export const makeDualWritePylonSparkPayoutTargetStore = (
  deps: MakeDualWritePylonSparkPayoutTargetStoreDependencies,
): PylonSparkPayoutTargetStore => {
  const { d1, flags, postgres } = deps
  const log = deps.log ?? (() => {})

  if (postgres === undefined) {
    return d1
  }

  // #8515 WRITE cutover: with writes='postgres' (the default after the
  // Cloudflare exit) Postgres is the SOLE authority for Spark payout targets —
  // dead D1 is neither written nor read. There is deliberately NO D1 fallback
  // (a Postgres outage must fail the request loud, not silently diverge onto a
  // 401-dead bridge). Reads serve Postgres too, since the D1 authority is gone.
  if (flags.writes === 'postgres') {
    return {
      read: pylonRef => postgres.readSparkPayoutTarget(pylonRef),
      readByOwner: ownerAgentUserId =>
        postgres.readSparkPayoutTargetByOwner(ownerAgentUserId),
      upsert: record => postgres.upsertSparkPayoutTarget(record),
    }
  }

  const mirror = (
    refs: ReadonlyArray<string>,
    run: () => Promise<void>,
  ): Promise<void> =>
    !flags.dualWrite
      ? Promise.resolve()
      : run().catch((error: unknown) => {
          log('khala_sync_pylon_dual_write_failed', {
            messageSafe: safeMessage(error),
            op: 'upsertSparkPayoutTarget',
            refs,
          })
        })

  return {
    read: d1.read,
    readByOwner: d1.readByOwner,
    upsert: async record => {
      const next = await d1.upsert(record)
      await mirror([next.pylonRef, next.payoutTargetRef], () =>
        postgres.mirrorSparkPayoutTarget(next),
      )
      return next
    },
  }
}

// ---------------------------------------------------------------------------
// Env factory (the index.ts drop-in)
// ---------------------------------------------------------------------------

export type PylonDispatchStoreEnv = PylonDispatchFlagEnv &
  Readonly<{
    OPENAGENTS_DB: D1Database
    KHALA_SYNC_DB?: KhalaSyncHyperdriveBinding | undefined
  }>

export type MakePylonApiStoreForEnvOptions = Readonly<{
  /** Injectable client factory (tests). Default: postgres.js/Hyperdrive. */
  makeSqlClient?: MakeKhalaSyncPushSqlClient | undefined
  log?: PylonDispatchLog | undefined
}>

const defaultLog: PylonDispatchLog = (event, fields) => {
  logWorkerRouteWarning(event, {
    messageSafe: fields.messageSafe,
    op: fields.op,
    refs: fields.refs.slice(0, 10).join(','),
  })
}

/**
 * The production `PylonApiStore` factory for the assignments/dispatch
 * domain: D1 authority + flag-gated Postgres dual-write/reads. Replaces
 * bare `makeD1PylonApiStore(openAgentsDatabase(env))` at Worker call sites.
 */
export const makePylonApiStoreForEnv = (
  env: PylonDispatchStoreEnv,
  options: MakePylonApiStoreForEnvOptions = {},
): PylonApiStore => {
  const d1 = makeD1PylonApiStore(openAgentsDatabase(env))
  const connectionString = env.KHALA_SYNC_DB?.connectionString
  const flags = pylonDispatchFlagsFromEnv(env)

  // Degrade to plain D1 ONLY when there is no Postgres binding, or when every
  // Postgres lever is off (no dual-write mirror, D1 reads, AND D1 writes). With
  // writes='postgres' (the #8515 default) we must build the Postgres-authoritative
  // store even if dual-write/reads happen to be D1.
  if (
    connectionString === undefined ||
    connectionString.length === 0 ||
    (!flags.dualWrite && flags.reads === 'd1' && flags.writes === 'd1')
  ) {
    return d1
  }

  const makeSqlClient = options.makeSqlClient ?? defaultMakeKhalaSyncSqlClient
  const postgres = makePostgresPylonDispatchStore({
    acquireSql: () => makeSqlClient(connectionString),
  })

  return makeDualWritePylonApiStore({
    d1,
    flags,
    log: options.log ?? defaultLog,
    postgres,
  })
}

export const makePylonSparkPayoutTargetStoreForEnv = (
  env: PylonDispatchStoreEnv,
  options: MakePylonApiStoreForEnvOptions = {},
): PylonSparkPayoutTargetStore => {
  const d1 = makeD1PylonSparkPayoutTargetStore(openAgentsDatabase(env))
  const connectionString = env.KHALA_SYNC_DB?.connectionString
  const flags = pylonDispatchFlagsFromEnv(env)

  // Degrade to plain D1 only when there is no Postgres binding, or when every
  // Postgres lever is off (no dual-write mirror AND D1 write authority). With
  // writes='postgres' (the #8515 default) build the Postgres-authoritative
  // store even if dual-write happens to be off.
  if (
    connectionString === undefined ||
    connectionString.length === 0 ||
    (!flags.dualWrite && flags.writes === 'd1')
  ) {
    return d1
  }

  const makeSqlClient = options.makeSqlClient ?? defaultMakeKhalaSyncSqlClient
  const postgres = makePostgresPylonDispatchStore({
    acquireSql: () => makeSqlClient(connectionString),
  })
  return makeDualWritePylonSparkPayoutTargetStore({
    d1,
    flags,
    log: options.log ?? defaultLog,
    postgres,
  })
}
