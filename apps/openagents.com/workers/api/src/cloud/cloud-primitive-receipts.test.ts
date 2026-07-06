import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import { readAgentBalance } from '../payments-ledger'
import type { PaymentsLedgerDb } from '../payments-ledger-db'
import { makeLedgerSqliteDb } from '../test/payments-ledger-sqlite'
import { settleCloudPrimitiveCharge } from './cloud-metering'
import {
  isPublicSafeCloudPrimitiveReceiptProjection,
  makeLedgerCloudPrimitiveReceiptStore,
  publicCloudPrimitiveReceiptFromRecord,
} from './cloud-primitive-receipts'
import {
  SANDBOX_COMPUTE_PRIMITIVE,
  sandboxRentalReceiptRef,
} from './sandbox-compute-service-routes'

const run = <A>(effect: Effect.Effect<A>): Promise<A> => Effect.runPromise(effect)

// CFG-4 (#8519): `pay_ins` is Postgres-authoritative — the metered debit AND
// the receipt read run against the credits-domain `PaymentsLedgerDb`, backed
// here by the in-memory ledger SQLite adapter (real load-bearing constraints:
// the balance CHECK and the idempotency_key UNIQUE), so the receipt this test
// dereferences is a real settled ledger row, not a mock. The Postgres contract
// suite proves the same semantics on the production dialect.

const NOW = '2026-06-23T12:00:00.000Z'
const ACCOUNT = 'agent:sandbox-receipt-test'

const makeDb = (): PaymentsLedgerDb => makeLedgerSqliteDb()

const seedBalance = async (
  ledgerDb: PaymentsLedgerDb,
  msat: number,
): Promise<void> => {
  await ledgerDb.batch([
    {
      params: [ACCOUNT, msat, NOW, NOW],
      sql: `INSERT INTO agent_balances (actor_ref, balance_msat, created_at, updated_at)
            VALUES (?, ?, ?, ?)`,
    },
  ])
}

describe('cloud-primitive receipt projection', () => {
  test('only a PAID adjustment charge with a cloud prefix projects', () => {
    expect(
      publicCloudPrimitiveReceiptFromRecord(
        {
          contextRef: null,
          createdAt: NOW,
          payInType: 'adjustment',
          receiptRef: 'receipt.cloud.sandbox_compute.rental.charge.s1',
          state: 'paid',
          stateChangedAt: NOW,
        },
        NOW,
      ),
    ).not.toBeNull()

    // A still-pending charge is NOT a dereferenceable receipt.
    expect(
      publicCloudPrimitiveReceiptFromRecord(
        {
          contextRef: null,
          createdAt: NOW,
          payInType: 'adjustment',
          receiptRef: 'receipt.cloud.sandbox_compute.rental.charge.s1',
          state: 'pending',
          stateChangedAt: NOW,
        },
        NOW,
      ),
    ).toBeNull()

    // A non-cloud ref does NOT resolve here.
    expect(
      publicCloudPrimitiveReceiptFromRecord(
        {
          contextRef: null,
          createdAt: NOW,
          payInType: 'adjustment',
          receiptRef: 'receipt.inference.charge.req1',
          state: 'paid',
          stateChangedAt: NOW,
        },
        NOW,
      ),
    ).toBeNull()
  })

  test('the projection is public-safe (no payment material)', () => {
    const projection = publicCloudPrimitiveReceiptFromRecord(
      {
        contextRef: null,
        createdAt: NOW,
        payInType: 'adjustment',
        receiptRef: 'receipt.cloud.fine_tuning.job.charge.j1',
        state: 'paid',
        stateChangedAt: NOW,
      },
      NOW,
    )
    expect(projection).not.toBeNull()
    expect(projection?.kind).toBe('fine_tuning_job')
    expect(projection?.ledgerState).toBe('paid')
    expect(isPublicSafeCloudPrimitiveReceiptProjection(projection)).toBe(true)
  })
})

// THE PROOF: rent -> real metered debit -> dereferenceable PAID receipt.
// This is the receipt artifact `cloud.sandbox_compute_service.v1` was missing.
//
// CFG-4 NOTE: this used to drive `makeLedgerSandboxMeteringHook` end to end;
// that hook lives in `sandbox-compute-service-routes.ts` (its own suite covers
// it) and is migrated to the ledger seam separately, so this test settles the
// SAME shared cloud-metering charge the hook delegates to, with the SAME
// primitive tag — the advertised-surface-ref == settled-ledger-ref alignment
// assertion is preserved below.
describe('end-to-end: metered sandbox rental yields a dereferenceable receipt', () => {
  test('a closed rental debits credits AND the receipt the surface advertises resolves', async () => {
    const ledgerDb = makeDb()
    await seedBalance(ledgerDb, 10_000)

    const sandboxId = 'sbx_proof_1'

    // (1) RENT closes with REAL metered usage. Price it from usage (never an
    // estimate) with integer-msat-per-second math so the charge is exact (no
    // float drift): 300 wall-seconds * 10 msat = 3000 msat, debited
    // receipt-first through the shared cloud-metering seam and marked PAID.
    const usage = { wallSeconds: 300 }
    const chargeMsat = usage.wallSeconds * 10

    const metering = await run(
      settleCloudPrimitiveCharge(
        { ledgerDb, nowIso: () => NOW },
        {
          accountRef: ACCOUNT,
          adapterId: 'sandbox-runtime',
          chargeId: sandboxId,
          chargeMsat,
          primitive: SANDBOX_COMPUTE_PRIMITIVE,
        },
      ),
    )

    expect(metering.metered).toBe(true)
    // The ref the surface advertises is the ref we will dereference.
    const advertisedRef = sandboxRentalReceiptRef(sandboxId)
    expect(metering.receiptRef).toBe(advertisedRef)
    expect(advertisedRef).toBe(
      'receipt.cloud.sandbox_compute.rental.charge.sbx_proof_1',
    )

    // The metered debit actually moved credits (300s * 10 msat = 3000 msat).
    const balance = await readAgentBalance(ledgerDb, ACCOUNT)
    expect(balance?.availableMsat).toBe(7000)

    // (2) DEREFERENCE: the advertised receipt resolves against the real ledger
    // row written above, and projects a public-safe PAID receipt. THIS is the
    // dereferenceable proof of rent -> metered -> charge.
    const store = makeLedgerCloudPrimitiveReceiptStore(ledgerDb)
    const record = await store.readCloudPrimitiveReceiptByRef(advertisedRef)
    expect(record).not.toBeNull()
    expect(record?.state).toBe('paid')

    const receipt = publicCloudPrimitiveReceiptFromRecord(record!, NOW)
    expect(receipt).not.toBeNull()
    expect(receipt?.kind).toBe('sandbox_compute_rental')
    expect(receipt?.ledgerState).toBe('paid')
    expect(receipt?.receiptRef).toBe(advertisedRef)
    expect(receipt?.sourceRefs).toContain(
      `route:/api/public/cloud/receipts/${advertisedRef}`,
    )
    // Honest: the receipt itself never claims the promise is green.
    expect(receipt?.caveatRefs).toContain(
      'caveat.public.cloud_primitive_demand_provenance_and_owner_signoff_pending',
    )
    expect(isPublicSafeCloudPrimitiveReceiptProjection(receipt)).toBe(true)
  })

  test('an unknown / unsettled ref does not resolve', async () => {
    const ledgerDb = makeDb()
    const store = makeLedgerCloudPrimitiveReceiptStore(ledgerDb)
    expect(
      await store.readCloudPrimitiveReceiptByRef(
        'receipt.cloud.sandbox_compute.rental.charge.nope',
      ),
    ).toBeNull()
  })
})
