// KS-8.14 (#8325): business funnel / orders / referrals CONTRACT suite.
//
// Three layers, one behavioral spec:
//
//  1. `BusinessDomainWriteStore` contract — the row seam's converge
//     semantics run identically against BOTH implementations:
//     - D1: `makeD1BusinessDomainWriteStore` over real SQLite
//       (node:sqlite — the engine D1 is built on), schema from the worker
//       migrations (condensed in test/sqlite-d1.ts).
//     - Postgres: `makePostgresBusinessDomainStore` over a throwaway
//       local Postgres (initdb/pg_ctl), schema from khala-sync-server
//       migration 0022. Skipped when no local Postgres binaries exist.
//
//  2. CLASSIFIER pinning — the live domain write statements (copied
//     VERBATIM from the writer modules: funnel recorder, signup routes,
//     signup fulfillment upsert, referral consumption batch, claim
//     transitions, triage update-by-order-id, affiliate linkage,
//     buy-mode counters, pipeline stages, the adjutant CASE-WHEN order
//     update, promise receipts, viral funnel, QA swarm, cohort upsert)
//     classify as mirrored writes with the RIGHT addressed key. A
//     changed/new write either classifies cleanly or logs
//     `khala_sync_business_write_unclassified` — it can never silently
//     corrupt.
//
//  3. END-TO-END mirror fidelity — REAL writers
//     (`recordBusinessFunnelEvent`, the promise-receipt store) and the
//     verbatim statement shapes run UNCHANGED through the mirroring
//     database with SQLite as D1 authority and the real Postgres store as
//     the mirror; afterwards every touched table is row-for-row IDENTICAL
//     across both stores. Load-bearing regressions:
//     - INSERT OR IGNORE consume-once replay mirrors as a no-op (the
//       attribution uniqueness keys feed payouts — the dedupe decision is
//       D1's and the mirror only copies);
//     - the `ON CONFLICT(business_signup_request_id)` fulfillment upsert
//       reads back by the CONFLICT column (the surviving row keeps its
//       ORIGINAL id — a PK read-back would go stale);
//     - the triage UPDATE addressed by `software_order_id` (lookup
//       column) converges the active record;
//     - FAIL-SOFT: a Postgres outage NEVER fails a domain write; it logs
//       `khala_sync_business_dual_write_failed` and D1 stands (the
//       escalation pager can never double-page off the mirror);
//     - compare-mode scoped SELECTs serve D1 and log divergence;
//       `postgres` mode serves ONLY the allowlisted
//       `BUSINESS_DOMAIN_POSTGRES_SERVED_READ_TABLES` surface
//       (`business_funnel_events`, #8360's read-cutover follow-up) for
//       real, fails soft back to D1 on a Postgres read error, and still
//       logs the deferred diagnostic for every other comparable-select
//       (the escalation pager / referral-attribution tables never serve
//       from Postgres).

import {
  BUSINESS_DOMAIN_TABLE_SPECS,
  normalizeBusinessValue,
  type BusinessDomainTable,
} from '@openagentsinc/khala-sync-server'
import {
  hasLocalPostgres,
  startLocalPostgres,
} from '@openagentsinc/khala-sync-server/test/local-postgres'
import { readFileSync } from 'node:fs'
import * as path from 'node:path'
import { afterAll, beforeAll, describe, expect, test } from 'vitest'

import { recordBusinessFunnelEvent } from './business-funnel-dashboard'
import {
  businessDomainDatabaseForEnv,
  businessDomainFlagsFromEnv,
  classifyBusinessDomainStatement,
  isPostgresServableBusinessRead,
  makeBusinessDomainMirror,
  makeBusinessDomainMirroringDatabase,
  makeD1BusinessDomainWriteStore,
  makePostgresBusinessDomainStore,
  resolveBusinessDomainKey,
  type BusinessDomainDiagnostic,
  type BusinessDomainDiagnosticEvent,
  type BusinessDomainReadsMode,
  type BusinessDomainRow,
  type BusinessDomainWriteStore,
  type PostgresBusinessDomainStore,
} from './business-domain-store'
import { makeD1PromiseTransitionReceiptStore } from './promise-transition-receipt-routes'
import { BUSINESS_DOMAIN_D1_SCHEMA, makeSqliteD1 } from './test/sqlite-d1'

const MIGRATION_0022 = path.resolve(
  import.meta.dirname,
  '../../../../../packages/khala-sync-server/migrations/0023_business_funnel.sql',
)
const MIGRATION_0051 = path.resolve(
  import.meta.dirname,
  '../../../../../packages/khala-sync-server/migrations/0051_business_pipeline_subject_ref.sql',
)

type PgClient = {
  end: (options?: { timeout?: number }) => Promise<void>
  unsafe: (
    text: string,
    params?: Array<unknown>,
  ) => Promise<Array<Record<string, unknown>>>
}

type LogEntry = Readonly<{
  event: BusinessDomainDiagnosticEvent
  fields: BusinessDomainDiagnostic
}>

const makeLogSink = () => {
  const entries: Array<LogEntry> = []
  return {
    entries,
    log: (event: BusinessDomainDiagnosticEvent, fields: BusinessDomainDiagnostic) => {
      entries.push({ event, fields })
    },
  }
}

// ---------------------------------------------------------------------------
// Flags
// ---------------------------------------------------------------------------

describe('business domain flags', () => {
  test('dual-write defaults ON; reads default d1; typo falls back to d1', () => {
    expect(businessDomainFlagsFromEnv({})).toEqual({
      dualWrite: true,
      reads: 'd1',
    })
    expect(
      businessDomainFlagsFromEnv({ KHALA_SYNC_BUSINESS_DUAL_WRITE: 'off' })
        .dualWrite,
    ).toBe(false)
    expect(
      businessDomainFlagsFromEnv({ KHALA_SYNC_BUSINESS_READS: 'compare' })
        .reads,
    ).toBe('compare')
    expect(
      businessDomainFlagsFromEnv({ KHALA_SYNC_BUSINESS_READS: 'postgres' })
        .reads,
    ).toBe('postgres')
    expect(
      businessDomainFlagsFromEnv({ KHALA_SYNC_BUSINESS_READS: 'postgress' })
        .reads,
    ).toBe('d1')
  })
})

// ---------------------------------------------------------------------------
// Classifier pinning: the LIVE write statement set
// ---------------------------------------------------------------------------

/** Verbatim statement texts from the writer modules (shape-pinned). */
const LIVE_STATEMENTS: ReadonlyArray<
  Readonly<{
    name: string
    sql: string
    table: BusinessDomainTable
    keyColumn: string
    bindIndex?: number
  }>
> = [
  {
    bindIndex: 0,
    keyColumn: 'id',
    name: 'business-funnel-dashboard: funnel receipt',
    sql: `INSERT OR IGNORE INTO business_funnel_events (
        id,
        event_ref,
        stage,
        source_kind,
        source_ref,
        occurred_at,
        observed_at
      ) VALUES (?, ?, ?, ?, ?, ?, ?)`,
    table: 'business_funnel_events',
  },
  {
    bindIndex: 0,
    keyColumn: 'user_id',
    name: 'consumption: user attribution consume-once',
    sql: `INSERT OR IGNORE INTO user_referral_attributions
           (user_id,
            referral_attribution_id,
            referral_source_id,
            referral_invite_id,
            capture_path,
            target,
            first_verified_at,
            policy_state,
            created_at,
            updated_at,
            archived_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, 'active', ?, ?, NULL)`,
    table: 'user_referral_attributions',
  },
  {
    bindIndex: 3,
    keyColumn: 'id',
    name: 'consumption: claim transition',
    sql: `UPDATE referral_attributions
            SET policy_state = 'claimed',
                claimed_user_id = COALESCE(claimed_user_id, ?),
                first_verified_at = COALESCE(first_verified_at, ?),
                updated_at = ?
          WHERE id = ?
            AND policy_state = 'pending'
            AND archived_at IS NULL`,
    table: 'referral_attributions',
  },
  {
    bindIndex: 1,
    keyColumn: 'business_signup_request_id',
    name: 'signup fulfillment: ON CONFLICT secondary-unique upsert',
    sql: `INSERT INTO business_signup_fulfillments
        (id, business_signup_request_id, status, reason, enrichment_ref,
         team_id, project_id, workspace_id, invite_id, email_message_id,
         email_delivery_status, metadata_json, created_at, updated_at)
       VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
       ON CONFLICT(business_signup_request_id) DO UPDATE SET
         status = excluded.status,
         reason = excluded.reason,
         enrichment_ref = excluded.enrichment_ref,
         team_id = excluded.team_id,
         project_id = excluded.project_id,
         workspace_id = excluded.workspace_id,
         invite_id = excluded.invite_id,
         email_message_id = excluded.email_message_id,
         email_delivery_status = excluded.email_delivery_status,
         metadata_json = excluded.metadata_json,
         updated_at = excluded.updated_at`,
    table: 'business_signup_fulfillments',
  },
  {
    bindIndex: 10,
    keyColumn: 'software_order_id',
    name: 'operator triage: update-by-order-id (lookup column)',
    sql: `UPDATE order_triage_records
              SET classification = ?,
                  operator_priority = ?,
                  first_batch_eligible = ?,
                  hold_reason = ?,
                  next_action = ?,
                  customer_safe_status = ?,
                  customer_safe_summary = ?,
                  reviewer_user_id = ?,
                  reviewed_at = ?,
                  updated_at = ?
            WHERE software_order_id = ?
              AND archived_at IS NULL`,
    table: 'order_triage_records',
  },
  {
    bindIndex: 2,
    keyColumn: 'business_signup_request_id',
    name: 'affiliate linkage: update-by-signup-id (lookup column)',
    sql: `UPDATE business_affiliate_attributions
          SET pipeline_ref = COALESCE(pipeline_ref, ?),
              updated_at = ?
        WHERE business_signup_request_id = ?
          AND archived_at IS NULL
          AND policy_state = 'active'
          AND (pipeline_ref IS NULL OR pipeline_ref = ?)`,
    table: 'business_affiliate_attributions',
  },
  {
    bindIndex: 5,
    keyColumn: 'id',
    name: 'adjutant lifecycle: CASE-WHEN order status update',
    sql: `UPDATE software_orders
                SET current_run_id = ?,
                    status = ?,
                    agent_started_at = CASE WHEN ? = 'agent_running' THEN COALESCE(agent_started_at, ?) ELSE agent_started_at END,
                    updated_at = ?
              WHERE id = ?
                AND archived_at IS NULL`,
    table: 'software_orders',
  },
  {
    bindIndex: 2,
    keyColumn: 'campaign_id',
    name: 'buy-mode: halt campaign (literal SET, bound key)',
    sql: `UPDATE buy_mode_campaigns SET state = 'halted', last_alert_ref = ?, updated_at = ? WHERE campaign_id = ?`,
    table: 'buy_mode_campaigns',
  },
  {
    bindIndex: 0,
    keyColumn: 'team_cohort_ref',
    name: 'customer-one cohort: ON CONFLICT PK upsert',
    sql: `INSERT INTO customer_one_cohort_rows (
            team_cohort_ref, state, candidate_ref, invite_ref, vertical_ref,
            template_ref, workspace_ref, routing_ref, run_ref, artifact_ref,
            review_ref, verification_ref, completion_bundle_ref,
            privacy_review_ref, blocker_refs_json, caveat_refs_json,
            updated_at, created_at
          ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
          ON CONFLICT(team_cohort_ref) DO UPDATE SET
            state = excluded.state,
            updated_at = excluded.updated_at`,
    table: 'customer_one_cohort_rows',
  },
  {
    bindIndex: 0,
    keyColumn: 'id',
    name: 'promise transitions: receipt insert',
    sql: `INSERT INTO promise_transition_receipts (
          id, promise_id, from_state, to_state, registry_version, result,
          checks_json, evidence_refs_json, exception_json, checked_at,
          created_at
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`,
    table: 'promise_transition_receipts',
  },
  {
    bindIndex: 0,
    keyColumn: 'id',
    name: 'viral funnel: event insert',
    sql: `INSERT INTO viral_agent_funnel_events (id, event_kind, route, actor_class, user_agent_class, site_slug, proof_ref, metadata_json, created_at)
       VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)`,
    table: 'viral_agent_funnel_events',
  },
  {
    bindIndex: 0,
    keyColumn: 'receipt_ref',
    name: 'qa swarm: engagement receipt insert',
    sql: `INSERT INTO qa_swarm_first_engagements (
        receipt_ref, idempotency_key, package_kind, payment_path,
        business_signup_request_id, user_id, committed_amount_cents,
        intake_receipt_ref, checkout_or_deposit_receipt_ref,
        target_adapter_review_ref, package_contract_ref, workspace_id,
        service_promise_contract_id, commitment_ref, first_report_due_at,
        recorded_at, created_at, updated_at
      ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`,
    table: 'qa_swarm_first_engagements',
  },
  {
    bindIndex: 2,
    keyColumn: 'pipeline_ref',
    name: 'pipeline: receipt refs update',
    sql: `UPDATE business_pipeline_rows SET receipt_refs_json = ?, updated_at = ? WHERE pipeline_ref = ?`,
    table: 'business_pipeline_rows',
  },
  {
    bindIndex: 3,
    keyColumn: 'id',
    name: 'fulfillment loop: promise motion pointer update',
    sql: `UPDATE business_service_promises SET last_motion_receipt_ref = ?, next_motion_due_at = ?, updated_at = ? WHERE id = ?`,
    table: 'business_service_promises',
  },
  // KS-8.14 remainder (#8359): the order / checkout / referral writer
  // boundaries wired into the mirror seam this lane. Each statement is
  // copied VERBATIM from its writer module so the classifier pins the
  // exact addressed key the mirror will read back by.
  {
    bindIndex: 0,
    keyColumn: 'id',
    name: 'customer-orders: create software_order insert',
    sql: `INSERT INTO software_orders
             (id,
              user_id,
              status,
              visibility,
              request,
              repository_provider,
              repository_owner,
              repository_name,
              repository_full_name,
              repository_private,
              repository_default_branch,
              repository_html_url,
              public_work_acknowledged_at,
              data_use_acknowledged_at,
              compute_payment_acknowledged_at,
              provider_account_required,
              free_slice_cents,
              quote_cents,
              current_run_id,
              agent_started_at,
              agent_idempotency_key,
              created_at,
              updated_at)
           VALUES (?, ?, 'submitted', 'public', ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 0, 5000, NULL, NULL, NULL, ?, ?, ?)`,
    table: 'software_orders',
  },
  {
    bindIndex: 2,
    keyColumn: 'id',
    name: 'operator-triage: software_order status update',
    sql: `UPDATE software_orders
                SET status = ?,
                    updated_at = ?
              WHERE id = ?
                AND archived_at IS NULL`,
    table: 'software_orders',
  },
  {
    bindIndex: 4,
    keyColumn: 'id',
    name: 'operator-adjutant: software_order launch-state update',
    sql: `UPDATE software_orders
                SET current_run_id = ?,
                    status = ?,
                    agent_started_at = COALESCE(agent_started_at, ?),
                    updated_at = ?
              WHERE id = ?
                AND archived_at IS NULL`,
    table: 'software_orders',
  },
  {
    bindIndex: 1,
    keyColumn: 'id',
    name: 'github-pr-fulfillment: mark order delivered',
    sql: `UPDATE software_orders
       SET status = 'delivered',
           updated_at = ?
       WHERE id = ?
         AND archived_at IS NULL`,
    table: 'software_orders',
  },
  {
    bindIndex: 0,
    keyColumn: 'id',
    name: 'operator-triage: triage event insert',
    sql: `INSERT INTO order_triage_events
             (id,
              triage_record_id,
              software_order_id,
              site_id,
              assignment_id,
              event_type,
              visibility,
              summary,
              actor_user_id,
              payload_json,
              created_at)
           VALUES (?, ?, ?, ?, ?, ?, 'team', ?, ?, ?, ?)`,
    table: 'order_triage_events',
  },
  {
    bindIndex: 0,
    keyColumn: 'id',
    name: 'operator-triage: triage record insert',
    sql: `INSERT INTO order_triage_records
               (id,
                software_order_id,
                classification,
                operator_priority,
                first_batch_eligible,
                hold_reason,
                next_action,
                customer_safe_status,
                customer_safe_summary,
                reviewer_user_id,
                reviewed_at,
                created_at,
                updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`,
    table: 'order_triage_records',
  },
  {
    bindIndex: 0,
    keyColumn: 'id',
    name: 'github-writeback: authority receipt insert',
    sql: `INSERT INTO order_github_write_authority_receipts
        (id, software_order_id, assignment_id, user_id, repository_full_name,
         repository_private, requested_operation, decision, authority_mode,
         blocked_reason, connection_ref, grant_ref, approval_source,
         approved_at, customer_message, metadata_json, created_at, updated_at)
       VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`,
    table: 'order_github_write_authority_receipts',
  },
  {
    bindIndex: 0,
    keyColumn: 'id',
    name: 'github-pr-fulfillment: fulfillment artifact insert',
    sql: `INSERT INTO order_fulfillment_artifacts
        (id, software_order_id, assignment_id, run_id, kind, title, summary,
         url, repository_full_name, source_branch, target_branch, commit_sha,
         status, visibility, metadata_json, created_by_user_id, created_at,
         updated_at, archived_at)
       VALUES (?, ?, ?, NULL, 'pull_request', ?, ?, ?, ?, ?, ?, ?,
         'customer_review_ready', 'public', ?, NULL, ?, ?, NULL)`,
    table: 'order_fulfillment_artifacts',
  },
  {
    bindIndex: 0,
    keyColumn: 'checkout_session_id',
    name: 'stripe-billing: checkout kickoff insert-or-ignore',
    sql: `INSERT OR IGNORE INTO business_checkout_kickoffs
        (checkout_session_id,
         business_signup_request_id,
         user_id,
         total_amount_cents,
         setup_fee_cents,
         credit_grant_cents,
         workspace_id,
         service_promise_contract_id,
         public_receipt_ref,
         created_at,
         updated_at)
       VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`,
    table: 'business_checkout_kickoffs',
  },
  {
    bindIndex: 0,
    keyColumn: 'id',
    name: 'referral-source-capture: pending attribution insert',
    sql: `INSERT INTO referral_attributions (
         id,
         referral_source_id,
         referral_invite_id,
         public_source_ref,
         public_invite_ref,
         capture_path,
         target,
         policy_state,
         first_verified_at,
         claimed_user_id,
         expires_at,
         created_at,
         updated_at,
         archived_at
       ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`,
    table: 'referral_attributions',
  },
  {
    bindIndex: 0,
    keyColumn: 'software_order_id',
    name: 'onboarding consumption: order referral linkage insert-or-ignore',
    sql: `INSERT OR IGNORE INTO order_referral_attributions
           (software_order_id,
            user_id,
            referral_attribution_id,
            referral_source_id,
            referral_invite_id,
            capture_path,
            target,
            linked_at,
            policy_state,
            created_at,
            updated_at,
            archived_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, 'active', ?, ?, NULL)`,
    table: 'order_referral_attributions',
  },
  {
    bindIndex: 0,
    keyColumn: 'id',
    name: 'site-referral workflow: workflow event insert-or-ignore',
    sql: `INSERT OR IGNORE INTO referral_workflow_events (
         id,
         idempotency_key,
         event_kind,
         referral_attribution_id,
         referral_source_id,
         referral_invite_id,
         public_source_ref,
         public_invite_ref,
         software_order_id,
         site_id,
         site_version_id,
         product_id,
         paid_action_id,
         payment_event_id,
         payment_evidence_ref,
         entitlement_ref,
         accepted_work_ref,
         related_event_id,
         public_receipt_ref,
         policy_state,
         amount,
         asset,
         metadata_json,
         occurred_at,
         created_at,
         archived_at
       ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`,
    table: 'referral_workflow_events',
  },
]

describe('business domain statement classifier', () => {
  for (const statement of LIVE_STATEMENTS) {
    test(`classifies: ${statement.name}`, () => {
      const classified = classifyBusinessDomainStatement(statement.sql)
      expect(classified.kind).toBe('mirrored-write')
      if (classified.kind !== 'mirrored-write') return
      expect(classified.table).toBe(statement.table)
      expect(classified.keyColumn).toBe(statement.keyColumn)
      if (statement.bindIndex !== undefined) {
        expect(classified.keySource).toEqual({
          index: statement.bindIndex,
          kind: 'bind',
        })
      }
    })
  }

  test('fulfillment loop UPDATE with three SET binds resolves index 3 for the WHERE key', () => {
    const classified = classifyBusinessDomainStatement(
      `UPDATE business_service_promises SET last_motion_receipt_ref = ?, next_motion_due_at = ?, updated_at = ? WHERE id = ?`,
    )
    expect(classified.kind).toBe('mirrored-write')
    if (classified.kind !== 'mirrored-write') return
    expect(
      resolveBusinessDomainKey(classified.keySource, [
        'receipt',
        'due',
        'now',
        'promise-1',
      ]),
    ).toBe('promise-1')
  })

  test('DELETE on a scoped table is a loud unclassified write; non-scoped writes pass through', () => {
    expect(
      classifyBusinessDomainStatement(
        `DELETE FROM referral_attributions WHERE id = ?`,
      ).kind,
    ).toBe('unclassified-write')
    expect(
      classifyBusinessDomainStatement(
        `INSERT INTO users (id, email) VALUES (?, ?)`,
      ).kind,
    ).toBe('passthrough')
    expect(
      classifyBusinessDomainStatement(
        `UPDATE site_referral_sources SET policy_state = ? WHERE id = ?`,
      ).kind,
    ).toBe('passthrough')
  })

  test('scoped-only SELECTs are comparable; mixed-table SELECTs are not', () => {
    expect(
      classifyBusinessDomainStatement(
        `SELECT * FROM business_funnel_events WHERE stage = ? ORDER BY occurred_at DESC`,
      ).kind,
    ).toBe('comparable-select')
    expect(
      classifyBusinessDomainStatement(
        `SELECT o.id FROM software_orders o JOIN users u ON u.id = o.user_id`,
      ).kind,
    ).toBe('passthrough')
  })

  test('#8360: business_funnel_events is postgres-servable; every other comparable-select is not', () => {
    const funnelSelect = classifyBusinessDomainStatement(
      `SELECT COUNT(*) AS count FROM business_funnel_events`,
    )
    expect(funnelSelect.kind).toBe('comparable-select')
    expect(isPostgresServableBusinessRead(funnelSelect)).toBe(true)

    const pagerSelect = classifyBusinessDomainStatement(
      `SELECT * FROM business_service_promises WHERE state = 'blocked' AND blocking_reason_ref IS NOT NULL ORDER BY COALESCE(blocked_at, updated_at) ASC, updated_at ASC LIMIT ?`,
    )
    expect(pagerSelect.kind).toBe('comparable-select')
    expect(isPostgresServableBusinessRead(pagerSelect)).toBe(false)

    const attributionExistenceSelect = classifyBusinessDomainStatement(
      `SELECT referral_attribution_id FROM user_referral_attributions WHERE user_id = ?`,
    )
    expect(attributionExistenceSelect.kind).toBe('comparable-select')
    expect(isPostgresServableBusinessRead(attributionExistenceSelect)).toBe(
      false,
    )

    // A hypothetical join touching the allowlisted table AND another
    // scoped table must NOT be servable — the allowlist is per exact
    // table set, not "touches the allowlisted table at all".
    const joinedSelect = classifyBusinessDomainStatement(
      `SELECT f.id FROM business_funnel_events f JOIN business_service_promises p ON p.id = f.stage`,
    )
    expect(joinedSelect.kind).toBe('comparable-select')
    expect(isPostgresServableBusinessRead(joinedSelect)).toBe(false)

    expect(isPostgresServableBusinessRead({ kind: 'passthrough' })).toBe(false)
  })
})

// ---------------------------------------------------------------------------
// Composition seam (options.d1) — the KS-8.14 remainder (#8359) layers the
// business mirror OVER another domain's proxy (sites / CRM) at route files
// that write BOTH domains. These run WITHOUT local Postgres: the mirror's
// Postgres twin is a fake in-memory upsert recorder, so the wiring contract
// is pinned on every CI run.
// ---------------------------------------------------------------------------

describe('business domain composition (options.d1)', () => {
  test('no KHALA_SYNC_DB binding returns the provided inner handle unchanged', () => {
    const inner = makeSqliteD1()
    inner.exec(BUSINESS_DOMAIN_D1_SCHEMA)
    // OPENAGENTS_DB is intentionally absent: with options.d1 the factory
    // never falls back to openAgentsDatabase(env), so the composed inner
    // handle is returned verbatim (zero overhead when the binding is off).
    const handle = businessDomainDatabaseForEnv({}, { d1: inner.db })
    expect(handle).toBe(inner.db)
  })

  test('composed over an inner handle: business writes mirror; non-business writes pass through only', async () => {
    const inner = makeSqliteD1()
    inner.exec(BUSINESS_DOMAIN_D1_SCHEMA)
    inner.exec('CREATE TABLE scratch_passthrough (id TEXT PRIMARY KEY, v TEXT)')
    const sink = makeLogSink()

    const upserts: Array<{ params: ReadonlyArray<unknown>; text: string }> = []
    const fakeSql = {
      unsafe: async (text: string, params: ReadonlyArray<unknown>) => {
        upserts.push({ params, text })
        return []
      },
    }
    const handle = businessDomainDatabaseForEnv(
      { KHALA_SYNC_DB: { connectionString: 'postgres://fake/x' } },
      {
        d1: inner.db,
        log: sink.log,
        makeSqlClient: async () => ({
          end: async () => {},
          sql: fakeSql as never,
        }),
      },
    )

    // A newly-wired business/order write: lands in the inner D1 authority
    // AND read-back mirrors to the Postgres twin.
    await handle
      .prepare(
        `INSERT OR IGNORE INTO business_checkout_kickoffs
          (checkout_session_id, business_signup_request_id, user_id,
           total_amount_cents, setup_fee_cents, credit_grant_cents,
           workspace_id, service_promise_contract_id, public_receipt_ref,
           created_at, updated_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`,
      )
      .bind(
        'checkout_compose_1',
        'signup_1',
        'user_1',
        500000,
        100000,
        400000,
        'workspace_1',
        'contract_1',
        'receipt.compose.1',
        '2026-07-04T00:00:00.000Z',
        '2026-07-04T00:00:00.000Z',
      )
      .run()

    const inD1 = await inner.db
      .prepare(
        'SELECT checkout_session_id FROM business_checkout_kickoffs WHERE checkout_session_id = ?',
      )
      .bind('checkout_compose_1')
      .first<{ checkout_session_id: string }>()
    expect(inD1?.checkout_session_id).toBe('checkout_compose_1')
    expect(upserts).toHaveLength(1)
    expect(upserts[0]!.text).toContain('business_checkout_kickoffs')

    // A non-business statement on the same composed handle executes on the
    // inner D1 only, never touches the Postgres twin, and never logs an
    // unclassified-write drift diagnostic.
    await handle
      .prepare('INSERT INTO scratch_passthrough (id, v) VALUES (?, ?)')
      .bind('p1', 'v1')
      .run()

    const passthrough = await inner.db
      .prepare('SELECT v FROM scratch_passthrough WHERE id = ?')
      .bind('p1')
      .first<{ v: string }>()
    expect(passthrough?.v).toBe('v1')
    expect(upserts).toHaveLength(1)
    expect(sink.entries).toHaveLength(0)
  })
})

// ---------------------------------------------------------------------------
// Both-store contract + end-to-end mirroring (local Postgres required)
// ---------------------------------------------------------------------------

const scopedRows = async (
  db: D1Database,
  table: BusinessDomainTable,
): Promise<ReadonlyArray<BusinessDomainRow>> => {
  const spec = BUSINESS_DOMAIN_TABLE_SPECS[table]
  const result = await db
    .prepare(
      `SELECT * FROM ${table} ORDER BY ${spec.keyColumns.join(', ')} ASC`,
    )
    .all<BusinessDomainRow>()
  return result.results ?? []
}

// Hash-style normalization (matches `businessRowHash`): postgres.js
// returns bigint columns as strings while D1 returns numbers — both sides
// normalize to the same display string, exactly as the verify hashes do.
const normalizedProjection = (
  table: BusinessDomainTable,
  rows: ReadonlyArray<BusinessDomainRow>,
): ReadonlyArray<ReadonlyArray<string | null>> =>
  rows.map(row =>
    BUSINESS_DOMAIN_TABLE_SPECS[table].columns.map(column => {
      const value = normalizeBusinessValue(row[column])
      return value === null ? null : String(value)
    }),
  )

describe.skipIf(!hasLocalPostgres())(
  'business domain stores + mirroring database (local Postgres)',
  () => {
    let stopPostgres: (() => Promise<void>) | undefined
    let pgUrl: string
    let adminClient: PgClient | undefined

    const pgQuery = async (
      text: string,
      params: Array<unknown> = [],
    ): Promise<Array<Record<string, unknown>>> => {
      const postgres = (await import('postgres')).default
      const client = postgres(pgUrl, { max: 1, prepare: false })
      try {
        return await client.unsafe(text, params as never[])
      } finally {
        await client.end({ timeout: 5 })
      }
    }

    const makePostgresStore = (): PostgresBusinessDomainStore =>
      makePostgresBusinessDomainStore({
        acquireSql: async () => {
          const postgres = (await import('postgres')).default
          const client = postgres(pgUrl, { max: 1, prepare: false })
          return {
            end: async () => {
              await client.end({ timeout: 5 })
            },
            sql: client as never,
          }
        },
      })

    const pgScopedRows = async (
      table: BusinessDomainTable,
    ): Promise<ReadonlyArray<BusinessDomainRow>> => {
      const spec = BUSINESS_DOMAIN_TABLE_SPECS[table]
      return pgQuery(
        `SELECT * FROM ${table} ORDER BY ${spec.keyColumns.join(', ')} ASC`,
      )
    }

    const expectTableParity = async (
      db: D1Database,
      table: BusinessDomainTable,
    ): Promise<void> => {
      const d1Rows = await scopedRows(db, table)
      const pgRows = await pgScopedRows(table)
      expect(normalizedProjection(table, pgRows)).toEqual(
        normalizedProjection(table, d1Rows),
      )
    }

    beforeAll(async () => {
      const pg = await startLocalPostgres()
      stopPostgres = pg.stop
      const postgres = (await import('postgres')).default
      const admin = postgres(pg.url, { max: 1, prepare: false })
      adminClient = admin as unknown as PgClient
      await admin.unsafe('CREATE DATABASE business_contract')
      pgUrl = pg.urlFor('business_contract')
      const migration = readFileSync(MIGRATION_0022, 'utf8')
      const pipelineSubjectMigration = readFileSync(MIGRATION_0051, 'utf8')
      const target = postgres(pgUrl, { max: 1, prepare: false })
      await target.unsafe(migration)
      await target.unsafe(pipelineSubjectMigration)
      await target.end({ timeout: 5 })
    }, 120_000)

    afterAll(async () => {
      await adminClient?.end({ timeout: 5 })
      await stopPostgres?.()
    }, 120_000)

    const truncate = async (tables: ReadonlyArray<BusinessDomainTable>) => {
      for (const table of tables) {
        await pgQuery(`TRUNCATE TABLE ${table}`)
      }
    }

    const makeMirroringDb = (options?: {
      compare?: boolean
      brokenPostgres?: boolean
      /** Read mode; defaults to 'compare' when `compare: true`, else 'd1'. */
      reads?: BusinessDomainReadsMode
      /** Make the READ-serving Postgres store (not the write mirror) fail. */
      brokenReadsPostgres?: boolean
    }) => {
      const sqlite = makeSqliteD1()
      sqlite.exec(BUSINESS_DOMAIN_D1_SCHEMA)
      const db = sqlite.db
      const sink = makeLogSink()
      const postgres = makePostgresStore()
      const postgresForMirror: BusinessDomainWriteStore =
        options?.brokenPostgres === true
          ? {
              upsertRows: () => Promise.reject(new Error('postgres down')),
            }
          : postgres
      const readsMode: BusinessDomainReadsMode =
        options?.reads ?? (options?.compare === true ? 'compare' : 'd1')
      const compareStore: PostgresBusinessDomainStore | undefined =
        readsMode === 'd1'
          ? undefined
          : options?.brokenReadsPostgres === true
            ? {
                ...postgres,
                queryRows: () =>
                  Promise.reject(new Error('postgres reads down')),
              }
            : postgres
      const mirroring = makeBusinessDomainMirroringDatabase({
        compareStore,
        db,
        log: sink.log,
        mirror: makeBusinessDomainMirror({
          db,
          log: sink.log,
          postgres: postgresForMirror,
        }),
        reads: readsMode,
      })
      return { db, mirroring, sink }
    }

    // -----------------------------------------------------------------
    // 1. Row-seam contract, both implementations
    // -----------------------------------------------------------------

    test('row seam: converge upsert is idempotent and stale rows converge on BOTH stores', async () => {
      await truncate(['business_pipeline_rows'])
      const sqlite = makeSqliteD1()
      sqlite.exec(BUSINESS_DOMAIN_D1_SCHEMA)
      const stores: ReadonlyArray<
        readonly [string, BusinessDomainWriteStore]
      > = [
        ['d1', makeD1BusinessDomainWriteStore(sqlite.db)],
        ['postgres', makePostgresStore()],
      ]
      const row: BusinessDomainRow = {
        blocker_ref: null,
        business_signup_request_id: null,
        created_at: '2026-07-04T00:00:00.000Z',
        next_action_due_at: null,
        owner_role: 'operator',
        partner_approval_receipt_ref: null,
        partner_budget_range_ref: null,
        partner_due_window_ref: null,
        partner_offer_ref: null,
        partner_peer_ref: null,
        partner_privacy_tier_ref: null,
        partner_route_flag: 0,
        partner_route_state: 'none',
        partner_route_updated_at: null,
        partner_scope_summary_ref: null,
        pipeline_ref: 'pipeline_contract_1',
        quoted_band_label: 'unquoted',
        quoted_max_usd_cents: 250000,
        quoted_min_usd_cents: 100000,
        receipt_refs_json: '[]',
        source_ref: 'direct',
        stage: 'intake_received',
        stage_updated_at: '2026-07-04T00:00:00.000Z',
        subject_ref: null,
        updated_at: '2026-07-04T00:00:00.000Z',
        vertical: 'vertical.test',
      }
      for (const [, store] of stores) {
        expect(await store.upsertRows('business_pipeline_rows', [row])).toBe(1)
        // replay: byte-stable no-op
        expect(await store.upsertRows('business_pipeline_rows', [row])).toBe(1)
        // stage transition converges
        await store.upsertRows('business_pipeline_rows', [
          {
            ...row,
            stage: 'closed_won',
            stage_updated_at: '2026-07-04T01:00:00.000Z',
            updated_at: '2026-07-04T01:00:00.000Z',
          },
        ])
      }
      const d1Rows = await scopedRows(sqlite.db, 'business_pipeline_rows')
      const pgRows = await pgScopedRows('business_pipeline_rows')
      expect(d1Rows).toHaveLength(1)
      expect(d1Rows[0]?.['stage']).toBe('closed_won')
      expect(
        normalizedProjection('business_pipeline_rows', pgRows),
      ).toEqual(normalizedProjection('business_pipeline_rows', d1Rows))
    })

    // -----------------------------------------------------------------
    // 2. End-to-end mirroring: real writers through the wrapped database
    // -----------------------------------------------------------------

    test('funnel recorder: real writes mirror byte-exactly; event_ref dedupe replay is a mirror no-op', async () => {
      await truncate(['business_funnel_events'])
      const { db, mirroring, sink } = makeMirroringDb()

      let n = 0
      const runtime = {
        makeId: (prefix: string) => `${prefix}_${++n}`,
        nowIso: () => '2026-07-04T00:00:01.000Z',
      }
      await recordBusinessFunnelEvent(
        mirroring,
        {
          eventRef: 'business.funnel.contract.1',
          occurredAt: '2026-07-04T00:00:00.000Z',
          sourceKind: 'direct',
          sourceRef: 'direct',
          stage: 'visit',
        },
        runtime,
      )
      await recordBusinessFunnelEvent(
        mirroring,
        {
          eventRef: 'business.funnel.contract.2',
          occurredAt: '2026-07-04T00:00:00.500Z',
          sourceKind: 'referral',
          sourceRef: 'affiliate_alpha',
          stage: 'signup',
        },
        runtime,
      )
      // Replay the SAME event_ref with a fresh id: D1's INSERT OR IGNORE
      // discards it (UNIQUE event_ref); the mirror reads back the bound id,
      // finds nothing, and stays converged.
      await recordBusinessFunnelEvent(
        mirroring,
        {
          eventRef: 'business.funnel.contract.1',
          occurredAt: '2026-07-04T00:00:02.000Z',
          sourceKind: 'direct',
          sourceRef: 'direct',
          stage: 'visit',
        },
        runtime,
      )

      const d1Rows = await scopedRows(db, 'business_funnel_events')
      expect(d1Rows).toHaveLength(2)
      await expectTableParity(db, 'business_funnel_events')
      expect(
        sink.entries.filter(
          entry => entry.event === 'khala_sync_business_dual_write_failed',
        ),
      ).toHaveLength(0)
      expect(
        sink.entries.filter(
          entry => entry.event === 'khala_sync_business_write_unclassified',
        ),
      ).toHaveLength(0)
    })

    test('promise receipts: the real store mirrors receipts hash-identically', async () => {
      await truncate(['promise_transition_receipts'])
      const { db, mirroring, sink } = makeMirroringDb()
      const store = makeD1PromiseTransitionReceiptStore(mirroring)
      await store.createReceipt({
        checkedAt: '2026-07-04T00:00:00.000Z',
        checks: [{ check: 'endpoint', ok: true }] as never,
        evidenceRefs: ['evidence.contract.1'],
        exception: null,
        fromState: 'yellow',
        promiseId: 'promise.contract',
        receiptId: 'ptr_contract_1',
        registryVersion: 'v1',
        result: 'pass',
        toState: 'green',
      } as never)
      await expectTableParity(db, 'promise_transition_receipts')
      expect(sink.entries).toHaveLength(0)
    })

    test('consume-once attribution batch mirrors; replay is a no-op on BOTH sides', async () => {
      await truncate(['referral_attributions', 'user_referral_attributions'])
      const { db, mirroring, sink } = makeMirroringDb()

      // Seed a pending attribution (capture path — also scoped + mirrored).
      await mirroring
        .prepare(
          `INSERT INTO referral_attributions ( id, referral_source_id, referral_invite_id, public_source_ref, public_invite_ref, capture_path, target, policy_state, first_verified_at, claimed_user_id, expires_at, created_at, updated_at, archived_at ) VALUES (?, ?, NULL, ?, NULL, 'human', 'home', 'pending', NULL, NULL, ?, ?, ?, NULL)`,
        )
        .bind(
          'attr_contract_1',
          'source_contract_1',
          'src.public.1',
          '2026-08-01T00:00:00.000Z',
          '2026-07-04T00:00:00.000Z',
          '2026-07-04T00:00:00.000Z',
        )
        .run()

      const consumeStatements = () => [
        mirroring
          .prepare(
            `INSERT OR IGNORE INTO user_referral_attributions
           (user_id,
            referral_attribution_id,
            referral_source_id,
            referral_invite_id,
            capture_path,
            target,
            first_verified_at,
            policy_state,
            created_at,
            updated_at,
            archived_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, 'active', ?, ?, NULL)`,
          )
          .bind(
            'user_contract_1',
            'attr_contract_1',
            'source_contract_1',
            null,
            'human',
            'home',
            '2026-07-04T00:00:01.000Z',
            '2026-07-04T00:00:01.000Z',
            '2026-07-04T00:00:01.000Z',
          ),
        mirroring
          .prepare(
            `UPDATE referral_attributions
            SET policy_state = 'claimed',
                claimed_user_id = COALESCE(claimed_user_id, ?),
                first_verified_at = COALESCE(first_verified_at, ?),
                updated_at = ?
          WHERE id = ?
            AND policy_state = 'pending'
            AND archived_at IS NULL`,
          )
          .bind(
            'user_contract_1',
            '2026-07-04T00:00:01.000Z',
            '2026-07-04T00:00:01.000Z',
            'attr_contract_1',
          ),
      ]

      // The live consumption path runs these as db.batch(...).
      await mirroring.batch(consumeStatements())
      // Consume-once replay: INSERT OR IGNORE discards, UPDATE matches no
      // pending row — both sides stay put.
      await mirroring.batch(consumeStatements())

      const d1Users = await scopedRows(db, 'user_referral_attributions')
      expect(d1Users).toHaveLength(1)
      const d1Attributions = await scopedRows(db, 'referral_attributions')
      expect(d1Attributions[0]?.['policy_state']).toBe('claimed')
      await expectTableParity(db, 'user_referral_attributions')
      await expectTableParity(db, 'referral_attributions')
      expect(
        sink.entries.filter(
          entry => entry.event !== 'khala_sync_business_read_compare_mismatch',
        ),
      ).toHaveLength(0)
    })

    test('signup fulfillment ON CONFLICT(secondary unique): the SURVIVING row mirrors', async () => {
      await truncate(['business_signup_fulfillments'])
      const { db, mirroring, sink } = makeMirroringDb()
      const upsertSql = `INSERT INTO business_signup_fulfillments
        (id, business_signup_request_id, status, reason, enrichment_ref,
         team_id, project_id, workspace_id, invite_id, email_message_id,
         email_delivery_status, metadata_json, created_at, updated_at)
       VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
       ON CONFLICT(business_signup_request_id) DO UPDATE SET
         status = excluded.status,
         reason = excluded.reason,
         enrichment_ref = excluded.enrichment_ref,
         team_id = excluded.team_id,
         project_id = excluded.project_id,
         workspace_id = excluded.workspace_id,
         invite_id = excluded.invite_id,
         email_message_id = excluded.email_message_id,
         email_delivery_status = excluded.email_delivery_status,
         metadata_json = excluded.metadata_json,
         updated_at = excluded.updated_at`
      const bind = (id: string, status: string, at: string) =>
        mirroring
          .prepare(upsertSql)
          .bind(
            id,
            'signup_contract_1',
            status,
            null,
            'enrichment.contract',
            null,
            null,
            null,
            null,
            null,
            'not_attempted',
            '{}',
            at,
            at,
          )
      await bind('fulfillment_a', 'operator_parked', '2026-07-04T00:00:00.000Z').run()
      // Retry with a NEW id conflicts on the signup id: D1 keeps id
      // 'fulfillment_a' and updates it. The mirror reads back by the
      // CONFLICT column, so Postgres converges the surviving row.
      await bind('fulfillment_b', 'invited', '2026-07-04T01:00:00.000Z').run()

      const d1Rows = await scopedRows(db, 'business_signup_fulfillments')
      expect(d1Rows).toHaveLength(1)
      expect(d1Rows[0]?.['id']).toBe('fulfillment_a')
      expect(d1Rows[0]?.['status']).toBe('invited')
      await expectTableParity(db, 'business_signup_fulfillments')
      expect(sink.entries).toHaveLength(0)
    })

    test('triage update addressed by software_order_id (lookup column) converges', async () => {
      await truncate(['order_triage_records'])
      const { db, mirroring, sink } = makeMirroringDb()
      await mirroring
        .prepare(
          `INSERT INTO order_triage_records (id, software_order_id, classification, operator_priority, first_batch_eligible, hold_reason, next_action, customer_safe_status, customer_safe_summary, reviewer_user_id, reviewed_at, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`,
        )
        .bind(
          'triage_contract_1',
          'order_contract_1',
          'runnable_site',
          100,
          0,
          null,
          'Scope the first slice.',
          'scoping',
          'Preparing this order.',
          null,
          null,
          '2026-07-04T00:00:00.000Z',
          '2026-07-04T00:00:00.000Z',
        )
        .run()
      await mirroring
        .prepare(
          `UPDATE order_triage_records
              SET classification = ?,
                  operator_priority = ?,
                  first_batch_eligible = ?,
                  hold_reason = ?,
                  next_action = ?,
                  customer_safe_status = ?,
                  customer_safe_summary = ?,
                  reviewer_user_id = ?,
                  reviewed_at = ?,
                  updated_at = ?
            WHERE software_order_id = ?
              AND archived_at IS NULL`,
        )
        .bind(
          'needs_clarification',
          50,
          1,
          null,
          'Ask the customer for scope.',
          'needs_scope',
          'Scoping continues.',
          'reviewer_1',
          '2026-07-04T01:00:00.000Z',
          '2026-07-04T01:00:00.000Z',
          'order_contract_1',
        )
        .run()
      const d1Rows = await scopedRows(db, 'order_triage_records')
      expect(d1Rows[0]?.['classification']).toBe('needs_clarification')
      await expectTableParity(db, 'order_triage_records')
      expect(sink.entries).toHaveLength(0)
    })

    // -----------------------------------------------------------------
    // 3. Fail-soft + read routing
    // -----------------------------------------------------------------

    test('FAIL-SOFT: a Postgres outage never fails a domain write; the drift diagnostic fires', async () => {
      const { db, mirroring, sink } = makeMirroringDb({ brokenPostgres: true })
      let n = 0
      await recordBusinessFunnelEvent(
        mirroring,
        {
          eventRef: 'business.funnel.contract.failsoft',
          occurredAt: '2026-07-04T00:00:00.000Z',
          sourceKind: 'direct',
          sourceRef: 'direct',
          stage: 'visit',
        },
        {
          makeId: (prefix: string) => `${prefix}_failsoft_${++n}`,
          nowIso: () => '2026-07-04T00:00:01.000Z',
        },
      )
      // The D1 write stands.
      expect(await scopedRows(db, 'business_funnel_events')).toHaveLength(1)
      expect(
        sink.entries.filter(
          entry => entry.event === 'khala_sync_business_dual_write_failed',
        ),
      ).toHaveLength(1)
      // Keys only in the diagnostic — never payloads.
      const diagnostic = sink.entries[0]?.fields
      expect(diagnostic?.op).toBe('mirror:business_funnel_events')
    })

    test('compare mode: scoped SELECT serves D1 and logs divergence when the twin drifts', async () => {
      await truncate(['business_funnel_events'])
      const { mirroring, sink } = makeMirroringDb({ compare: true })
      let n = 0
      await recordBusinessFunnelEvent(
        mirroring,
        {
          eventRef: 'business.funnel.contract.compare',
          occurredAt: '2026-07-04T00:00:00.000Z',
          sourceKind: 'direct',
          sourceRef: 'direct',
          stage: 'visit',
        },
        {
          makeId: (prefix: string) => `${prefix}_compare_${++n}`,
          nowIso: () => '2026-07-04T00:00:01.000Z',
        },
      )
      // In-sync: compare read shows no mismatch and serves D1.
      const inSync = await mirroring
        .prepare(
          `SELECT * FROM business_funnel_events ORDER BY occurred_at ASC`,
        )
        .all<Record<string, unknown>>()
      expect(inSync.results).toHaveLength(1)
      expect(
        sink.entries.filter(
          entry => entry.event === 'khala_sync_business_read_compare_mismatch',
        ),
      ).toHaveLength(0)

      // Drift the twin, then read again: D1 is still served, mismatch logs.
      await pgQuery(`DELETE FROM business_funnel_events`)
      const drifted = await mirroring
        .prepare(
          `SELECT * FROM business_funnel_events ORDER BY occurred_at ASC`,
        )
        .all<Record<string, unknown>>()
      expect(drifted.results).toHaveLength(1)
      expect(
        sink.entries.filter(
          entry => entry.event === 'khala_sync_business_read_compare_mismatch',
        ),
      ).toHaveLength(1)
    })

    // -----------------------------------------------------------------
    // 4. `postgres` mode: bounded real read serving (#8360)
    // -----------------------------------------------------------------

    test('postgres mode: business_funnel_events (allowlisted) serves the POSTGRES row set, not D1', async () => {
      await truncate(['business_funnel_events'])
      const { db, mirroring, sink } = makeMirroringDb({ reads: 'postgres' })
      let n = 0
      await recordBusinessFunnelEvent(
        mirroring,
        {
          eventRef: 'business.funnel.contract.postgres_serve',
          occurredAt: '2026-07-04T00:00:00.000Z',
          sourceKind: 'direct',
          sourceRef: 'direct',
          stage: 'visit',
        },
        {
          makeId: (prefix: string) => `${prefix}_pgserve_${++n}`,
          nowIso: () => '2026-07-04T00:00:01.000Z',
        },
      )
      // D1 and Postgres agree right after the write mirrors.
      const inSync = await mirroring
        .prepare(
          `SELECT * FROM business_funnel_events ORDER BY occurred_at ASC`,
        )
        .all<Record<string, unknown>>()
      expect(inSync.results).toHaveLength(1)

      // Diverge the twin directly (simulate a mirror-ahead read): the
      // allowlisted table must now read back the POSTGRES state, proving
      // this is real serving and not a D1-served shadow compare.
      await pgQuery(
        `UPDATE business_funnel_events SET source_ref = 'postgres_only_marker'`,
      )
      const served = await mirroring
        .prepare(
          `SELECT * FROM business_funnel_events ORDER BY occurred_at ASC`,
        )
        .all<Record<string, unknown>>()
      expect(served.results?.[0]?.['source_ref']).toBe('postgres_only_marker')
      const d1Rows = await scopedRows(db, 'business_funnel_events')
      expect(d1Rows[0]?.['source_ref']).toBe('direct')
      // No serve-failed or compare-mismatch diagnostic: this was a clean
      // real serve, not a failed attempt or a shadow-compare.
      expect(
        sink.entries.filter(entry =>
          entry.event.startsWith('khala_sync_business_postgres_read'),
        ),
      ).toHaveLength(0)
      expect(
        sink.entries.filter(
          entry => entry.event === 'khala_sync_business_read_compare_mismatch',
        ),
      ).toHaveLength(0)
    })

    test('postgres mode: a non-allowlisted table (business_service_promises) stays D1-served', async () => {
      const { db, mirroring } = makeMirroringDb({ reads: 'postgres' })
      // Insert THROUGH the mirroring database (a real mirrored-write
      // statement) so the write mirrors into Postgres exactly like
      // production — this test only cares about read routing, not write
      // fidelity (already covered above).
      await mirroring
        .prepare(
          `INSERT INTO business_service_promises (
             id, promise_ref, accepted_outcome_contract_id, workspace_ref,
             crm_state_ref, stakeholder_refs_json, state, cadence,
             next_motion_due_at, last_motion_receipt_ref, source_refs_json,
             metadata_json, created_at, updated_at, blocking_reason_ref,
             blocked_at, last_escalation_page_ref
           ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`,
        )
        .bind(
          'promise_contract_pg_1',
          'promise.contract.pg.1',
          null,
          'workspace_contract_1',
          'crm_state_contract_1',
          '[]',
          'active',
          'weekly',
          null,
          null,
          '[]',
          '{}',
          '2026-07-04T00:00:00.000Z',
          '2026-07-04T00:00:00.000Z',
          null,
          null,
          null,
        )
        .run()
      await expectTableParity(db, 'business_service_promises')

      // Diverge the twin directly (to a DIFFERENT valid state so the
      // escalation-pager-shaped `WHERE state = 'active'` filter below would
      // return ZERO rows if it were wrongly served from Postgres): since
      // this table is NOT in BUSINESS_DOMAIN_POSTGRES_SERVED_READ_TABLES,
      // the mirroring database must still answer from D1 even in
      // `postgres` mode — the escalation pager's read path rides this
      // exact statement shape.
      await pgQuery(`UPDATE business_service_promises SET state = 'paused'`)
      const rows = await mirroring
        .prepare(
          `SELECT * FROM business_service_promises WHERE state = 'active' AND cadence IN ('daily', 'weekly') AND (next_motion_due_at IS NULL OR next_motion_due_at <= ?) ORDER BY COALESCE(next_motion_due_at, created_at) ASC, updated_at ASC LIMIT ?`,
        )
        .bind('2026-07-05T00:00:00.000Z', 10)
        .all<Record<string, unknown>>()
      expect(rows.results).toHaveLength(1)
      expect(rows.results?.[0]?.['state']).toBe('active')
    })

    test('postgres mode: a Postgres read failure on the allowlisted table falls back to D1 and logs the typed diagnostic', async () => {
      await truncate(['business_funnel_events'])
      const { db, mirroring, sink } = makeMirroringDb({
        reads: 'postgres',
        brokenReadsPostgres: true,
      })
      let n = 0
      await recordBusinessFunnelEvent(
        mirroring,
        {
          eventRef: 'business.funnel.contract.postgres_serve_failed',
          occurredAt: '2026-07-04T00:00:00.000Z',
          sourceKind: 'direct',
          sourceRef: 'direct',
          stage: 'visit',
        },
        {
          makeId: (prefix: string) => `${prefix}_pgservefail_${++n}`,
          nowIso: () => '2026-07-04T00:00:01.000Z',
        },
      )
      const rows = await mirroring
        .prepare(
          `SELECT * FROM business_funnel_events ORDER BY occurred_at ASC`,
        )
        .all<Record<string, unknown>>()
      // Fail-soft: D1 still answers even though the Postgres read store
      // is broken.
      expect(rows.results).toHaveLength(1)
      expect(await scopedRows(db, 'business_funnel_events')).toHaveLength(1)
      expect(
        sink.entries.filter(
          entry =>
            entry.event === 'khala_sync_business_postgres_read_serve_failed',
        ),
      ).toHaveLength(1)
    })
  },
)
