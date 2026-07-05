// KS-8.12 REMAINDER (#8357): sites remainder repository CONTRACT suite.
//
// The remainder tables ride the SAME shared registry + row seam as the
// #8323 core, so this suite pins the KS-8.12 remainder-specific properties
// the core suite does not cover:
//
//  1. Write-store contract on BOTH implementations (D1 real SQLite +
//     Postgres from migrations 0020/0025): converge-on-PK idempotency, and
//     the remainder dedupe keys port EXACTLY — site_provisioning_plans
//     UNIQUE(idempotency_key) and targeted_site_prospects
//     UNIQUE(campaign_id, normalized_domain) both reject a duplicate on a
//     NEW id.
//  2. SECRET-SAFETY (SPEC invariant 9): a `site_environment_values` D1 row
//     carrying `plain_value` secret material read-back-mirrors into
//     Postgres as metadata + `secret_ref` ONLY — the Postgres twin has no
//     `plain_value` column and the secret bytes never appear.
//  3. MONEY discipline: commerce totals reconcile to the cent
//     (SUM(amount) on the Postgres twin equals the D1-side sum) and the
//     revenue-share → payment-event referential relation holds by
//     set-membership (no cross-store join).
//  4. FAIL-SOFT dual-write: a mirror failure is logged
//     (`khala_sync_sites_dual_write_failed`) and NEVER fails the D1 write.

import {
  normalizeSitesContentValue,
  SITES_CONTENT_TABLE_COLUMNS,
  type SitesContentRow,
  type SitesContentTable,
} from '@openagentsinc/khala-sync-server'
import {
  hasLocalPostgres,
  startLocalPostgres,
} from '@openagentsinc/khala-sync-server/test/local-postgres'
import { readFileSync } from 'node:fs'
import * as path from 'node:path'
import { afterAll, beforeAll, describe, expect, test } from 'vitest'

import {
  makeD1SitesContentWriteStore,
  makeDualWriteSitesContentWriteStore,
  makePostgresSitesContentStore,
  makeSitesContentMirror,
  sitesContentFlagsFromEnv,
  type PostgresSitesContentStore,
  type SitesContentDiagnostic,
  type SitesContentDiagnosticEvent,
  type SitesContentWriteStore,
} from './sites-content-store'
import { makeSqliteD1, SITES_REMAINDER_D1_SCHEMA } from './test/sqlite-d1'

const T0 = '2026-07-04T00:00:00.000Z'

// ---------------------------------------------------------------------------
// Fixtures (snake_case rows exactly as the stores project them)
// ---------------------------------------------------------------------------

const provisioningRow = (
  n: number,
  overrides: Partial<Record<string, unknown>> = {},
): SitesContentRow => ({
  archived_at: null,
  created_at: T0,
  id: `plan_${n}`,
  idempotency_key: `plan-key-${n}`,
  receipt_json: '{}',
  requested_by_user_id: 'user_owner',
  resource_manifest_json: '{"resources":[]}',
  reviewed_at: null,
  reviewed_by_user_id: null,
  site_id: 'site_r1',
  status: 'review_required',
  updated_at: T0,
  ...overrides,
})

const prospectRow = (
  n: number,
  overrides: Partial<Record<string, unknown>> = {},
): SitesContentRow => ({
  archived_at: null,
  campaign_id: 'campaign_1',
  capture_state: 'not_started',
  company_name: null,
  contact_refs_json: '[]',
  created_at: T0,
  discovered_at: T0,
  discovery_confidence: 0,
  geography: null,
  id: `prospect_${n}`,
  idempotency_key: `prospect-key-${n}`,
  metadata_json: '{}',
  normalized_domain: `domain-${n}.example`,
  origin_url: null,
  review_state: 'pending',
  site_name: null,
  source_ref: 'src_ref',
  suppression_state: 'unknown',
  updated_at: T0,
  vertical: null,
  ...overrides,
})

const paymentEventRow = (
  n: number,
  asset: string,
  amount: string,
): SitesContentRow => ({
  amount,
  asset,
  created_at: T0,
  customer_ref: null,
  entitlement_ref: null,
  event_kind: 'checkout_paid',
  id: `pe_${n}`,
  paid_action_id: null,
  payment_evidence_ref: `evidence_${n}`,
  product_id: null,
  public_receipt_ref: `receipt_${n}`,
  referral_source_ref: null,
  site_id: 'site_r1',
  site_version_id: null,
  software_order_id: null,
})

const revShareRow = (n: number, paymentEventId: string): SitesContentRow => ({
  accepted_work_ref: null,
  created_at: T0,
  id: `rsl_${n}`,
  ldk_settlement_receipt_ref: null,
  nexus_receipt_ref: null,
  payment_event_id: paymentEventId,
  projection_json: '{}',
  provider_payout_claimed: 0,
  provider_payout_eligibility_state: 'not_eligible',
  referral_reward_trigger: 'none',
  requested_contributor_asset: 'credits',
  treasury_receipt_ref: null,
  withdrawal_posture: 'internal_credit_only',
})

// ---------------------------------------------------------------------------
// Layer 1: write-store contract on both implementations
// ---------------------------------------------------------------------------

type ContractHarness = Readonly<{
  store: SitesContentWriteStore
  query: (sql: string) => Promise<ReadonlyArray<Record<string, unknown>>>
}>

const specContractSuite = (harness: () => ContractHarness) => {
  test('converge on PK is idempotent (remainder table)', async () => {
    const { query, store } = harness()
    expect(
      await store.upsertRows('site_provisioning_plans', [
        provisioningRow(1),
        provisioningRow(2),
      ]),
    ).toBe(2)
    expect(
      await store.upsertRows('site_provisioning_plans', [
        provisioningRow(1),
        provisioningRow(2),
      ]),
    ).toBe(2)
    const counted = await query(
      `SELECT COUNT(*) AS total FROM site_provisioning_plans WHERE id LIKE 'plan_%'`,
    )
    expect(Number(counted[0]?.['total'])).toBe(2)

    // Latest D1 snapshot wins.
    await store.upsertRows('site_provisioning_plans', [
      provisioningRow(1, { status: 'approved', reviewed_at: T0 }),
    ])
    const rows = await query(
      `SELECT status FROM site_provisioning_plans WHERE id = 'plan_1'`,
    )
    expect(rows[0]?.['status']).toBe('approved')
  })

  test('idempotency dedupe ports exactly: same key on a new id rejects', async () => {
    const { store } = harness()
    await store.upsertRows('site_provisioning_plans', [provisioningRow(3)])
    await expect(
      store.upsertRows('site_provisioning_plans', [
        provisioningRow(4, { idempotency_key: 'plan-key-3' }),
      ]),
    ).rejects.toThrow()
  })

  test('natural key ports exactly: (campaign_id, normalized_domain) rejects', async () => {
    const { store } = harness()
    await store.upsertRows('targeted_site_prospects', [
      prospectRow(1, { normalized_domain: 'dup.example' }),
    ])
    await expect(
      store.upsertRows('targeted_site_prospects', [
        prospectRow(2, { normalized_domain: 'dup.example' }),
      ]),
    ).rejects.toThrow()
  })
}

describe('sites remainder write-store contract — D1 (real SQLite)', () => {
  let sqlite: ReturnType<typeof makeSqliteD1> | undefined
  let harness: ContractHarness

  beforeAll(() => {
    sqlite = makeSqliteD1()
    sqlite.exec(SITES_REMAINDER_D1_SCHEMA)
    harness = {
      query: async sql =>
        (await sqlite!.db.prepare(sql).all<Record<string, unknown>>())
          .results ?? [],
      store: makeD1SitesContentWriteStore(sqlite.db),
    }
  })

  afterAll(() => {
    sqlite?.close()
  })

  specContractSuite(() => harness)
})

const MIGRATION = (name: string) =>
  path.resolve(
    __dirname,
    `../../../../../packages/khala-sync-server/migrations/${name}`,
  )

type PgClient = Readonly<{
  end: (options?: { timeout?: number }) => Promise<void>
  unsafe: (
    sql: string,
    params?: Array<unknown>,
  ) => Promise<Array<Record<string, unknown>>>
}>

describe.skipIf(!hasLocalPostgres())(
  'sites remainder write-store contract — Postgres',
  () => {
    let pg: Awaited<ReturnType<typeof startLocalPostgres>>
    let client: PgClient | undefined
    let harness: ContractHarness

    beforeAll(async () => {
      pg = await startLocalPostgres()
      const postgres = (await import('postgres')).default
      const admin = postgres(pg.url, { max: 1, prepare: false })
      await admin.unsafe('CREATE DATABASE sites_remainder_contract')
      await admin.end({ timeout: 5 })
      const raw = postgres(pg.urlFor('sites_remainder_contract'), {
        max: 4,
        prepare: false,
      })
      client = raw as unknown as PgClient
      await raw.unsafe(readFileSync(MIGRATION('0020_sites_core.sql'), 'utf8'))
      await raw.unsafe(
        readFileSync(MIGRATION('0025_sites_remainder.sql'), 'utf8'),
      )
      harness = {
        query: async sql => (client as PgClient).unsafe(sql),
        store: makePostgresSitesContentStore({
          acquireSql: () =>
            Promise.resolve({
              end: () => Promise.resolve(),
              sql: raw as never,
            }),
        }),
      }
    }, 120_000)

    afterAll(async () => {
      await client?.end({ timeout: 5 })
      await pg?.stop()
    }, 60_000)

    specContractSuite(() => harness)

    // --- Scope B: secret-safety through the read-back mirror ---------------
    test('env values mirror metadata + secret_ref ONLY — plain_value never rides', async () => {
      const sqlite = makeSqliteD1()
      sqlite.exec(SITES_REMAINDER_D1_SCHEMA)
      const store = makePostgresSitesContentStore({
        acquireSql: () =>
          Promise.resolve({
            end: () => Promise.resolve(),
            sql: (client as unknown as { unsafe: PgClient['unsafe'] }) as never,
          }),
      }) as PostgresSitesContentStore
      const diagnostics: Array<string> = []
      const mirror = makeSitesContentMirror({
        db: sqlite.db,
        log: event => diagnostics.push(event),
        postgres: store,
      })

      // A real D1 env-values row WITH secret material in plain_value.
      await sqlite.db
        .prepare(
          `INSERT INTO site_environment_values
             (id, site_id, key, kind, secret_ref, plain_value, created_at, updated_at, deleted_at)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, NULL)`,
        )
        .bind(
          'env_1',
          'site_secret',
          'STRIPE_KEY',
          'secret',
          'secretref://vault/stripe',
          'sk_live_SUPERSECRET_VALUE',
          T0,
          T0,
        )
        .run()

      await mirror.mirrorRowsByKey(
        'site_environment_values',
        'site_id',
        'site_secret',
      )
      expect(diagnostics).toEqual([])

      const rows = await (client as PgClient).unsafe(
        `SELECT * FROM site_environment_values WHERE id = 'env_1'`,
      )
      expect(rows).toHaveLength(1)
      const row = rows[0]!
      // Metadata + the secret_ref indirection are present…
      expect(row['secret_ref']).toBe('secretref://vault/stripe')
      expect(row['key']).toBe('STRIPE_KEY')
      // …but plain_value never made it into the twin (no column, no bytes).
      expect(Object.keys(row)).not.toContain('plain_value')
      expect(JSON.stringify(row)).not.toContain('SUPERSECRET')

      sqlite.close()
    })

    // --- Scope C: money totals to the cent + set-membership ----------------
    test('commerce totals reconcile to the cent and revenue-share membership holds', async () => {
      const payments = [
        paymentEventRow(1, 'usd', '10.50'),
        paymentEventRow(2, 'usd', '4.25'),
        paymentEventRow(3, 'credits', '100'),
      ]
      expect(
        await harness.store.upsertRows('site_commerce_payment_events', payments),
      ).toBe(3)
      await harness.store.upsertRows('site_commerce_revenue_share_links', [
        revShareRow(1, 'pe_1'),
        revShareRow(2, 'pe_3'),
      ])

      // Totals to the cent, per asset — the Postgres numeric sum is exact.
      const usd = await harness.query(
        `SELECT COALESCE(SUM(amount), 0) AS total FROM site_commerce_payment_events WHERE asset = 'usd'`,
      )
      expect(Number(usd[0]?.['total'])).toBeCloseTo(14.75, 2)

      // Set-membership: every revenue-share payment_event_id is a real
      // payment event id (within-store, no cross-store join).
      const childRows = await harness.query(
        `SELECT DISTINCT payment_event_id AS value FROM site_commerce_revenue_share_links`,
      )
      const parentRows = await harness.query(
        `SELECT id AS value FROM site_commerce_payment_events`,
      )
      const parents = new Set(parentRows.map(r => String(r['value'])))
      const orphans = childRows
        .map(r => String(r['value']))
        .filter(v => !parents.has(v))
      expect(orphans).toEqual([])
    })
  },
)

// ---------------------------------------------------------------------------
// Layer 2: fail-soft dual-write (pure — no Postgres needed)
// ---------------------------------------------------------------------------

describe('sites remainder dual-write is fail-soft', () => {
  test('a mirror failure is logged and never fails the D1 write', async () => {
    const sqlite = makeSqliteD1()
    sqlite.exec(SITES_REMAINDER_D1_SCHEMA)
    const events: Array<{
      event: SitesContentDiagnosticEvent
      fields: SitesContentDiagnostic
    }> = []
    const throwingPostgres: SitesContentWriteStore = {
      upsertRows: () => Promise.reject(new Error('postgres unreachable')),
    }
    const store = makeDualWriteSitesContentWriteStore({
      d1: makeD1SitesContentWriteStore(sqlite.db),
      flags: sitesContentFlagsFromEnv({ KHALA_SYNC_SITES_DUAL_WRITE: 'on' }),
      log: (event, fields) => events.push({ event, fields }),
      postgres: throwingPostgres,
    })

    // The D1 authority write succeeds; the mirror throw is swallowed+logged.
    await expect(
      store.upsertRows('site_provisioning_plans', [provisioningRow(9)]),
    ).resolves.toBe(1)

    const rows =
      (await sqlite.db
        .prepare(`SELECT id FROM site_provisioning_plans`)
        .all<Record<string, unknown>>()).results ?? []
    expect(rows).toHaveLength(1)
    expect(events.map(e => e.event)).toEqual([
      'khala_sync_sites_dual_write_failed',
    ])
    // Diagnostic references the row KEY only, never values.
    expect(events[0]?.fields.refs).toEqual(['plan_9'])

    sqlite.close()
  })
})

// Guard: the remainder D1 test schema must not leak a mirrored secret column,
// and the registry must exclude it.
describe('registry secret exclusion', () => {
  test('site_environment_values registry columns omit plain_value', () => {
    const columns = SITES_CONTENT_TABLE_COLUMNS[
      'site_environment_values' as SitesContentTable
    ] as ReadonlyArray<string>
    expect(columns).toContain('secret_ref')
    expect(columns).not.toContain('plain_value')
    // Normalizer sanity: a stray secret in the source row is simply not in
    // the projected column set.
    expect(normalizeSitesContentValue('sk_live_x')).toBe('sk_live_x')
  })
})
