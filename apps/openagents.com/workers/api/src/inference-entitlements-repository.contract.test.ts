// KS-8.9 (#8320): inference entitlements repository CONTRACT suite.
//
// One behavioral spec, TWO stores:
//   - D1: the PRODUCTION write paths (mint mark, `withFreeTierKhala`
//     accrual, premium/exemption grants + revokes, earned-allowance
//     accrual, privacy purchase, orange check grant) over real SQLite
//     (node:sqlite — the engine D1 is built on), schema condensed from the
//     worker migrations. Each write path emits its typed mirror op(s)
//     through a collector.
//   - Postgres: `makePostgresInferenceEntitlementsStore` over a throwaway
//     local Postgres (initdb/pg_ctl), schema from khala-sync-server
//     migration 0010. Skipped when no local Postgres binaries exist.
//
// The KS-8.9 load-bearing properties proven here:
//   - MIRROR-OP FIDELITY: applying the ops the production write paths
//     emitted reproduces the D1 decision state in Postgres — the six
//     enforcement gate reads return IDENTICAL decisions from
//     `makeD1InferenceEntitlementsGateReads` and the Postgres gate reads
//     after every step (the §3.6 denial-decision equivalence).
//   - INCREMENT IDEMPOTENCY: every op batch is applied to Postgres TWICE
//     (mirror re-delivery) and the D1 write paths are replayed with the
//     same request/accrual refs — neither side ever double-counts a
//     tally (event-keyed accrual: a doubled increment is a false denial).
//   - GENERIC-WRITE SEMANTICS: DO-NOTHING inserts replay to one row;
//     converge upserts update exactly the D1 DO-UPDATE column set;
//     unknown columns are refused; consume ops converge to the terminal
//     state idempotently.

import {
  hasLocalPostgres,
  startLocalPostgres,
} from '@openagentsinc/khala-sync-server/test/local-postgres'
import { Effect } from 'effect'
import { readFileSync } from 'node:fs'
import * as path from 'node:path'
import { afterAll, beforeAll, describe, expect, test } from 'vitest'

import {
  makeD1InferenceEntitlementsGateReads,
  makePostgresInferenceEntitlementsStore,
  type InferenceEntitlementsGateReads,
  type InferenceEntitlementsMirrorOp,
  type PostgresInferenceEntitlementsStore,
} from './inference-entitlements-store'
import {
  accrueEarnedAllowance,
  withFreeAllowance,
} from './inference/inference-free-allowance'
import {
  markAccountFreeTierAsync,
  recordFreeKeyMintAsync,
  withFreeTierKhala,
} from './inference/inference-free-tier-key'
import {
  grantOperatorExemption,
  revokeOperatorExemption,
} from './inference/inference-operator-exemption'
import {
  grantPremiumAccess,
  revokePremiumAccess,
} from './inference/inference-premium-allowlist'
import { grantPaidPrivacyEntitlement } from './inference/inference-privacy-receipt-routes'
import type {
  MeteringContext,
  MeteringHook,
} from './inference/metering-hook'
import { grantOrangeCheckEntitlement } from './orange-check-entitlements'
import { makeSqliteD1, type SqliteD1 } from './test/sqlite-d1'

// ---------------------------------------------------------------------------
// D1 schema (condensed from the worker migrations 0109/0116/0117/0150/
// 0210/0227/0231/0235/0256 — the tables this contract exercises)
// ---------------------------------------------------------------------------

const ENTITLEMENTS_D1_SCHEMA = `
CREATE TABLE inference_free_tier_keys (
  account_ref TEXT PRIMARY KEY,
  scope TEXT NOT NULL DEFAULT 'free_khala_daily',
  mint_source TEXT NOT NULL DEFAULT 'self_serve_anonymous',
  note TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);
CREATE TABLE inference_free_tier_usage (
  account_ref TEXT NOT NULL,
  usage_day TEXT NOT NULL,
  free_request_count INTEGER NOT NULL DEFAULT 0,
  free_total_tokens INTEGER NOT NULL DEFAULT 0,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  PRIMARY KEY (account_ref, usage_day)
);
CREATE TABLE inference_free_tier_usage_events (
  request_id TEXT PRIMARY KEY,
  account_ref TEXT NOT NULL,
  usage_day TEXT NOT NULL,
  served_model TEXT NOT NULL,
  total_tokens INTEGER NOT NULL DEFAULT 0,
  created_at TEXT NOT NULL
);
CREATE TABLE inference_free_key_mints (
  ip_hash TEXT NOT NULL,
  mint_day TEXT NOT NULL,
  mint_count INTEGER NOT NULL DEFAULT 0,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  PRIMARY KEY (ip_hash, mint_day)
);
CREATE TABLE inference_free_usage_tally (
  owner_key TEXT PRIMARY KEY,
  identity_kind TEXT NOT NULL,
  cumulative_free_usd_micros INTEGER NOT NULL DEFAULT 0,
  free_request_count INTEGER NOT NULL DEFAULT 0,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);
CREATE TABLE inference_free_usage_events (
  request_id TEXT PRIMARY KEY,
  owner_key TEXT NOT NULL,
  account_ref TEXT NOT NULL,
  served_model TEXT NOT NULL,
  free_usd_micros INTEGER NOT NULL,
  created_at TEXT NOT NULL
);
CREATE TABLE inference_premium_allowlist (
  owner_key TEXT PRIMARY KEY,
  scope TEXT NOT NULL DEFAULT 'all_premium',
  granted_by TEXT,
  note TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);
CREATE TABLE inference_earned_allowance (
  owner_key TEXT PRIMARY KEY,
  earned_free_usd_micros INTEGER NOT NULL DEFAULT 0,
  accrual_count INTEGER NOT NULL DEFAULT 0,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);
CREATE TABLE inference_earned_allowance_events (
  accrual_event_ref TEXT PRIMARY KEY,
  owner_key TEXT NOT NULL,
  accrual_kind TEXT NOT NULL,
  earned_usd_micros INTEGER NOT NULL,
  created_at TEXT NOT NULL
);
CREATE TABLE inference_operator_exemption (
  owner_key TEXT PRIMARY KEY,
  scope TEXT NOT NULL DEFAULT 'own_infra_non_premium',
  granted_by TEXT,
  note TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);
CREATE TABLE inference_privacy_entitlements (
  account_ref TEXT PRIMARY KEY,
  privacy_tier TEXT NOT NULL DEFAULT 'paid_privacy',
  note TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);
CREATE TABLE inference_privacy_entitlement_receipts (
  receipt_ref TEXT PRIMARY KEY,
  entitlement_ref TEXT NOT NULL UNIQUE,
  account_ref TEXT NOT NULL,
  purchase_ref TEXT NOT NULL UNIQUE,
  idempotency_key TEXT NOT NULL UNIQUE,
  privacy_tier TEXT NOT NULL DEFAULT 'paid_privacy',
  capture_excluded INTEGER NOT NULL DEFAULT 1,
  reason_ref TEXT NOT NULL,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);
CREATE TABLE orange_check_entitlements (
  id TEXT PRIMARY KEY,
  agent_user_id TEXT NOT NULL UNIQUE,
  actor_ref TEXT NOT NULL UNIQUE,
  state TEXT NOT NULL,
  receipt_ref TEXT NOT NULL UNIQUE,
  action_ref TEXT,
  paid_amount_cents INTEGER NOT NULL DEFAULT 500,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);
`

const MIGRATION_0013 = path.resolve(
  import.meta.dirname,
  '../../../../../packages/khala-sync-server/migrations/0013_inference_entitlements.sql',
)

type PgClient = {
  end: (options?: { timeout?: number }) => Promise<void>
  unsafe: (
    text: string,
    params?: Array<unknown>,
  ) => Promise<Array<Record<string, unknown>>>
}

const NOW = '2026-07-04T12:00:00.000Z'
const fixedNow = () => NOW

const meteringContext = (
  overrides: Partial<MeteringContext> = {},
): MeteringContext => ({
  accountRef: 'agent:contract-free',
  adapterId: 'hydralisk-vllm',
  fundingKind: 'card',
  requestId: 'req-contract-1',
  requestedModel: 'openagents/khala',
  servedModel: 'openagents/khala',
  streamed: false,
  usage: { completionTokens: 5, promptTokens: 10, totalTokens: 15 },
  ...overrides,
})

const innerHook: MeteringHook = () =>
  Effect.succeed({ metered: true, receiptRef: 'receipt.inner' })

const run = <A>(effect: Effect.Effect<A>): Promise<A> =>
  Effect.runPromise(effect)

describe.skipIf(!hasLocalPostgres())(
  'inference entitlements repository contract — D1 write paths vs Postgres mirror',
  () => {
    let pg: Awaited<ReturnType<typeof startLocalPostgres>>
    let client: PgClient | undefined
    let sqlite: SqliteD1
    let d1: D1Database
    let d1Reads: InferenceEntitlementsGateReads
    let store: PostgresInferenceEntitlementsStore
    let collected: Array<InferenceEntitlementsMirrorOp>
    /**
     * The collector mirror: production write paths push their ops here;
     * `flush()` applies them to Postgres TWICE — the re-delivery drill
     * that proves increment idempotency.
     */
    const mirror = (ops: ReadonlyArray<InferenceEntitlementsMirrorOp>) => {
      collected.push(...ops)
    }
    const flushTwice = async () => {
      const ops = collected.splice(0)
      await store.applyMirrorOps(ops)
      await store.applyMirrorOps(ops)
    }
    /** Both sides must agree on every enforcement decision. */
    const expectDecisionParity = async (input: {
      accountRef: string
      ownerKey: string
      usageDay: string
    }) => {
      const pgReads = store.gateReads
      expect(await pgReads.freeTierKeyExists(input.accountRef)).toBe(
        await d1Reads.freeTierKeyExists(input.accountRef),
      )
      expect(
        await pgReads.freeTierUsage(input.accountRef, input.usageDay),
      ).toEqual(await d1Reads.freeTierUsage(input.accountRef, input.usageDay))
      expect(await pgReads.freeUsageState(input.ownerKey)).toEqual(
        await d1Reads.freeUsageState(input.ownerKey),
      )
      expect(await pgReads.premiumAllowlisted(input.ownerKey)).toBe(
        await d1Reads.premiumAllowlisted(input.ownerKey),
      )
      expect(await pgReads.operatorExempt(input.ownerKey)).toBe(
        await d1Reads.operatorExempt(input.ownerKey),
      )
      expect(await pgReads.privacyEntitlementExists(input.accountRef)).toBe(
        await d1Reads.privacyEntitlementExists(input.accountRef),
      )
    }

    beforeAll(async () => {
      pg = await startLocalPostgres()
      const postgres = (await import('postgres')).default
      const admin = postgres(pg.url, { max: 1, prepare: false })
      await admin.unsafe('CREATE DATABASE entitlements_contract')
      await admin.end({ timeout: 5 })
      const raw = postgres(pg.urlFor('entitlements_contract'), {
        max: 4,
        prepare: false,
      })
      client = raw as unknown as PgClient
      await client.unsafe(readFileSync(MIGRATION_0013, 'utf8'))
      store = makePostgresInferenceEntitlementsStore({
        acquireSql: () =>
          Promise.resolve({
            end: () => Promise.resolve(),
            sql: raw as never,
          }),
      })

      sqlite = makeSqliteD1()
      sqlite.exec(ENTITLEMENTS_D1_SCHEMA)
      d1 = sqlite.db
      d1Reads = makeD1InferenceEntitlementsGateReads(d1)
      collected = []
    }, 120_000)

    afterAll(async () => {
      sqlite?.close()
      await client?.end({ timeout: 5 })
      await pg?.stop()
    }, 60_000)

    const accountRef = 'agent:contract-free'
    const ownerKey = 'owner:contract-owner'
    const usageDay = NOW.slice(0, 10)

    test('free-tier mint mark + mint counter mirror to decision parity', async () => {
      expect(
        await markAccountFreeTierAsync(
          d1,
          { accountRef, nowIso: NOW },
          mirror,
        ),
      ).toBe(true)
      expect(
        await recordFreeKeyMintAsync(
          d1,
          { ipHash: 'ip-hash-1', mintDay: usageDay, nowIso: NOW },
          mirror,
        ),
      ).toBe(true)
      await flushTwice()
      await expectDecisionParity({ accountRef, ownerKey, usageDay })
      // Converge upsert (NOT do-nothing): a re-mark with a new source
      // updates the D1 DO-UPDATE column set on both sides.
      expect(
        await markAccountFreeTierAsync(
          d1,
          { accountRef, mintSource: 'self_serve_email', nowIso: NOW },
          mirror,
        ),
      ).toBe(true)
      await flushTwice()
      const rows = await client?.unsafe(
        `SELECT mint_source FROM inference_free_tier_keys WHERE account_ref = $1`,
        [accountRef],
      )
      expect(rows?.[0]?.['mint_source']).toBe('self_serve_email')
    })

    test('free-tier accrual is event-keyed on BOTH sides: replayed request ids and re-delivered mirror ops never double-count', async () => {
      const wrapped = withFreeTierKhala(innerHook, {
        db: d1,
        mirror,
        nowIso: fixedNow,
      })
      const outcome = await run(
        wrapped(meteringContext({ requestId: 'req-a' })),
      )
      expect(outcome.metered).toBe(false)
      // SAME request id replayed at the D1 layer: idempotent no-op there
      // (the batch aborts on the UNIQUE event key), and it emits no
      // additional accrual op.
      const opsBefore = collected.length
      await run(wrapped(meteringContext({ requestId: 'req-a' })))
      expect(collected.length).toBe(opsBefore)
      // A second distinct request accrues.
      await run(wrapped(meteringContext({ requestId: 'req-b' })))

      // Mirror re-delivery: every op applies TWICE.
      await flushTwice()

      const d1Usage = await d1Reads.freeTierUsage(accountRef, usageDay)
      expect(d1Usage).toEqual({ requestsToday: 2, tokensToday: 30 })
      expect(await store.gateReads.freeTierUsage(accountRef, usageDay)).toEqual(
        d1Usage,
      )
      // Postgres event count matches the tally (the §3.6 invariant).
      const events = await client?.unsafe(
        `SELECT COUNT(*) AS c FROM inference_free_tier_usage_events WHERE account_ref = $1`,
        [accountRef],
      )
      expect(Number(events?.[0]?.['c'])).toBe(2)
    })

    test('earned-allowance accrual: the SAME contribution ref never grants twice, on either side', async () => {
      expect(
        await run(
          accrueEarnedAllowance(
            d1,
            {
              kind: 'referred_signup',
              nowIso: fixedNow,
              ownerKey,
              sourceRef: 'referred-user-1',
            },
            mirror,
          ),
        ),
      ).toBe(true)
      // Replayed contribution: D1 aborts on the UNIQUE accrual ref.
      await run(
        accrueEarnedAllowance(
          d1,
          {
            kind: 'referred_signup',
            nowIso: fixedNow,
            ownerKey,
            sourceRef: 'referred-user-1',
          },
          mirror,
        ),
      )
      await flushTwice()
      const d1State = await d1Reads.freeUsageState(ownerKey)
      expect(d1State.earnedFreeUsdMicros).toBeGreaterThan(0)
      expect(await store.gateReads.freeUsageState(ownerKey)).toEqual(d1State)
    })

    test('premium + exemption grants and revokes converge to identical allow/deny decisions', async () => {
      expect(
        await run(
          grantPremiumAccess(d1, { nowIso: fixedNow, ownerKey }, mirror),
        ),
      ).toBe(true)
      expect(
        await run(
          grantOperatorExemption(d1, { nowIso: fixedNow, ownerKey }, mirror),
        ),
      ).toBe(true)
      await flushTwice()
      expect(await store.gateReads.premiumAllowlisted(ownerKey)).toBe(true)
      expect(await store.gateReads.operatorExempt(ownerKey)).toBe(true)
      await expectDecisionParity({ accountRef, ownerKey, usageDay })

      expect(await run(revokePremiumAccess(d1, ownerKey, mirror))).toBe(true)
      expect(await run(revokeOperatorExemption(d1, ownerKey, mirror))).toBe(
        true,
      )
      await flushTwice()
      expect(await store.gateReads.premiumAllowlisted(ownerKey)).toBe(false)
      expect(await store.gateReads.operatorExempt(ownerKey)).toBe(false)
      await expectDecisionParity({ accountRef, ownerKey, usageDay })
    })

    test('privacy purchase mirrors the entitlement + receipt (capture opt-out parity)', async () => {
      const row = await grantPaidPrivacyEntitlement(
        d1,
        {
          accountRef,
          idempotencyKey: 'privacy-key-1',
          nowIso: NOW,
          purchaseRef: 'purchase-1',
        },
        mirror,
      )
      expect(row).not.toBeNull()
      await flushTwice()
      expect(await store.gateReads.privacyEntitlementExists(accountRef)).toBe(
        true,
      )
      await expectDecisionParity({ accountRef, ownerKey, usageDay })
      const receipts = await client?.unsafe(
        `SELECT COUNT(*) AS c FROM inference_privacy_entitlement_receipts WHERE account_ref = $1`,
        [accountRef],
      )
      expect(Number(receipts?.[0]?.['c'])).toBe(1)
    })

    test('orange check grant: DO-NOTHING insert replays to exactly one row', async () => {
      const grant = () =>
        run(
          grantOrangeCheckEntitlement(
            d1,
            {
              actionRef: 'forum_paid_action.orange_check.ch-1',
              actorRef: 'agent:orange-actor',
              agentUserId: 'orange-user',
              nowIso: NOW,
              paidAmountCents: 500,
              receiptRef: 'orange_check_receipt.ch-1',
            },
            mirror,
          ),
        )
      expect(await grant()).not.toBeNull()
      await grant()
      await flushTwice()
      const rows = await client?.unsafe(
        `SELECT COUNT(*) AS c FROM orange_check_entitlements WHERE agent_user_id = 'orange-user'`,
      )
      expect(Number(rows?.[0]?.['c'])).toBe(1)
    })

    test('consume_entitlement converges to the terminal state idempotently', async () => {
      await store.applyMirrorOps([
        {
          kind: 'write',
          row: {
            actor_ref: 'agent:searcher',
            agent_user_id: 'searcher',
            challenge_id: 'ch-search-1',
            created_at: NOW,
            credential_id: 'cred-1',
            entitlement_ref: 'ent-search-1',
            expires_at: '2026-07-05T00:00:00.000Z',
            id: 'ent-row-1',
            method: 'POST',
            mode: 'basic',
            path: '/api/agents/search',
            product_id: 'search-basic',
            receipt_ref: 'receipt-search-1',
            request_body_digest: 'digest-1',
            scope_ref: 'scope-1',
            status: 'active',
          },
          table: 'agent_search_entitlements',
        },
      ])
      const consume: InferenceEntitlementsMirrorOp = {
        consumedAt: NOW,
        entitlementRef: 'ent-search-1',
        kind: 'consume_entitlement',
        table: 'agent_search_entitlements',
      }
      await store.applyMirrorOps([consume])
      await store.applyMirrorOps([consume])
      const rows = await client?.unsafe(
        `SELECT status, consumed_at FROM agent_search_entitlements WHERE entitlement_ref = 'ent-search-1'`,
      )
      expect(rows?.[0]?.['status']).toBe('consumed')
      expect(rows?.[0]?.['consumed_at']).toBe(NOW)
    })

    test('unknown mirror columns are refused (column-fidelity ratchet)', async () => {
      await expect(
        store.applyMirrorOps([
          {
            kind: 'write',
            row: { account_ref: 'agent:x', nonexistent_column: 1 },
            table: 'inference_free_tier_keys',
          },
        ]),
      ).rejects.toThrow(/unknown column/)
    })

    test('withFreeAllowance mirrors the owner-pool accrual event-keyed (no double count on re-delivery)', async () => {
      const wrapped = withFreeAllowance(innerHook, {
        db: d1,
        mirror,
        nowIso: fixedNow,
        resolveOwnerIdentity: () =>
          Promise.resolve({ ownerUserId: 'contract-owner' } as never),
      })
      await run(
        wrapped(
          meteringContext({
            requestId: 'pool-req-1',
            servedModel: 'gemini-3.5-flash',
            requestedModel: 'gemini-3.5-flash',
          }),
        ),
      )
      // Replay the same request id — no additional op.
      const opsBefore = collected.length
      await run(
        wrapped(
          meteringContext({
            requestId: 'pool-req-1',
            servedModel: 'gemini-3.5-flash',
            requestedModel: 'gemini-3.5-flash',
          }),
        ),
      )
      expect(collected.length).toBe(opsBefore)
      await flushTwice()
      const d1State = await d1Reads.freeUsageState(ownerKey)
      expect(await store.gateReads.freeUsageState(ownerKey)).toEqual(d1State)
      const events = await client?.unsafe(
        `SELECT COUNT(*) AS c FROM inference_free_usage_events WHERE owner_key = $1`,
        [ownerKey],
      )
      const tally = await client?.unsafe(
        `SELECT free_request_count FROM inference_free_usage_tally WHERE owner_key = $1`,
        [ownerKey],
      )
      expect(Number(events?.[0]?.['c'])).toBe(
        Number(tally?.[0]?.['free_request_count']),
      )
    })
  },
)
