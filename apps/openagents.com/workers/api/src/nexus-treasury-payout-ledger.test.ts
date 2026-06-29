import { readFileSync } from 'node:fs'

import { Schema as S } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  NexusTreasuryPayoutLedgerProjection,
  NexusTreasuryPayoutLedgerUnsafe,
  assertNexusTreasuryPayoutAttemptSafe,
  assertNexusTreasuryPayoutIntentSafe,
  makeD1NexusTreasuryPayoutLedgerStore,
  nexusTreasuryPayoutLedgerProjectionHasPrivateMaterial,
  projectNexusTreasuryPayoutLedgerRecord,
  type NexusPaymentAuthorityReceiptRecord,
  type NexusPayoutTargetApprovalRecord,
  type NexusReleaseGateRecord,
  type NexusTreasuryPayoutAttemptRecord,
  type NexusTreasuryPayoutIntentRecord,
  type NexusTreasuryPayoutReconciliationEventRecord,
} from './nexus-treasury-payout-ledger'

const now = '2026-06-07T06:30:00.000Z'

const approval: NexusPayoutTargetApprovalRecord = {
  agentRef: 'agent.openagents_pylon_smoke',
  approvalPolicyRef: 'policy.nexus_payout_target.operator_test',
  approvalRef: 'approval.nexus_payout_target.pylon_smoke_1',
  approvedByRef: 'operator.openagents.core_team',
  archivedAt: null,
  createdAt: now,
  expiresAt: null,
  id: 'nexus_payout_target_approval_1',
  idempotencyKeyHash: 'hash.approval.pylon_smoke_1',
  ownerUserId: 'user_owner_123',
  publicProjectionJson: '{}',
  payoutTargetRef: 'payout_target.pylon_smoke_1',
  pylonRef: 'pylon.smoke_1',
  redactedDestinationRef: 'destination.redacted.pylon_smoke_1',
  scopeRefs: ['scope.pylon.assignment_test'],
  status: 'active',
  updatedAt: now,
}

const intent: NexusTreasuryPayoutIntentRecord = {
  acceptedWorkRefs: ['accepted_work.pylon_smoke_1'],
  actorRef: 'agent.artanis',
  adapterKind: 'simulation',
  amount: {
    amountMinorUnits: 1_000,
    asset: 'bitcoin',
    denomination: 'bitcoin_millisatoshi',
  },
  archivedAt: null,
  artanisDispatchRef: 'artanis.dispatch.pylon_smoke_1',
  assignmentRef: 'assignment.pylon_smoke_1',
  buyerPaymentRef: 'buyer_payment.receipt.site_order_1',
  createdAt: now,
  id: 'nexus_treasury_payout_intent_1',
  idempotencyKeyHash: 'hash.intent.pylon_smoke_1',
  metadataRefs: ['metadata.nexus.intent.operator_test'],
  ownerUserId: 'user_owner_123',
  payoutIntentRef: 'payout_intent.pylon_smoke_1',
  payoutTargetApprovalRef: approval.approvalRef,
  payoutTargetRef: approval.payoutTargetRef,
  policySnapshotRef: 'policy_snapshot.nexus.spend_cap_1',
  publicProjectionJson: '{"kind":"operator_test"}',
  pylonJobRef: 'pylon_job.smoke_1',
  sourceKind: 'pylon_marketplace_assignment',
  spendCap: {
    amountMinorUnits: 2_000,
    asset: 'bitcoin',
    denomination: 'bitcoin_millisatoshi',
  },
  status: 'approved',
  updatedAt: now,
}

const attempt: NexusTreasuryPayoutAttemptRecord = {
  adapterAttemptRef: 'adapter_attempt.simulation.pylon_smoke_1',
  adapterKind: intent.adapterKind,
  amount: intent.amount,
  archivedAt: null,
  createdAt: now,
  id: 'nexus_treasury_payout_attempt_1',
  idempotencyKeyHash: 'hash.attempt.pylon_smoke_1',
  metadataRefs: ['metadata.nexus.attempt.simulation'],
  payoutAttemptRef: 'payout_attempt.pylon_smoke_1',
  payoutIntentRef: intent.payoutIntentRef,
  publicProjectionJson: '{"adapter":"simulation"}',
  redactedDestinationRef: approval.redactedDestinationRef,
  redactedPaymentRef: 'payment.redacted.simulation_1',
  status: 'dispatched',
  updatedAt: now,
}

const reconciliationEvent: NexusTreasuryPayoutReconciliationEventRecord = {
  adapterKind: intent.adapterKind,
  archivedAt: null,
  createdAt: now,
  eventRef: 'reconciliation.nexus.pylon_smoke_1',
  externalEventRef: 'external_event.simulation.pylon_smoke_1',
  id: 'nexus_treasury_reconciliation_1',
  idempotencyKeyHash: 'hash.reconciliation.pylon_smoke_1',
  metadataRefs: ['metadata.nexus.reconciliation.matched'],
  payoutAttemptRef: attempt.payoutAttemptRef,
  payoutIntentRef: intent.payoutIntentRef,
  providerRef: 'provider.simulation',
  publicProjectionJson: '{}',
  resultRef: 'result.reconciliation.matched',
  status: 'matched',
}

const receipt: NexusPaymentAuthorityReceiptRecord = {
  archivedAt: null,
  audience: 'public',
  createdAt: now,
  eventRef: reconciliationEvent.eventRef,
  id: 'nexus_payment_authority_receipt_1',
  metadataRefs: ['metadata.receipt.public'],
  payoutAttemptRef: attempt.payoutAttemptRef,
  payoutIntentRef: intent.payoutIntentRef,
  publicProjectionJson: '{"receipt":"public_safe"}',
  receiptKind: 'settlement_recorded',
  receiptRef: 'receipt.nexus.payment_authority.pylon_smoke_1',
}

const releaseGate: NexusReleaseGateRecord = {
  archivedAt: null,
  blockerRefs: [],
  createdAt: now,
  evidenceRefs: ['receipt.nexus.payment_authority.pylon_smoke_1'],
  gateKind: 'simulation_adapter',
  gateRef: 'release_gate.nexus.simulation_adapter',
  id: 'nexus_release_gate_1',
  idempotencyKeyHash: 'hash.release_gate.simulation_adapter',
  publicProjectionJson: '{}',
  status: 'passed',
  updatedAt: now,
}

type IntentRow = Readonly<{
  accepted_work_refs_json: string
  actor_ref: string
  adapter_kind: NexusTreasuryPayoutIntentRecord['adapterKind']
  amount_asset: NexusTreasuryPayoutIntentRecord['amount']['asset']
  amount_denomination: NexusTreasuryPayoutIntentRecord['amount']['denomination']
  amount_minor_units: number
  archived_at: string | null
  artanis_dispatch_ref: string | null
  assignment_ref: string | null
  buyer_payment_ref: string | null
  created_at: string
  id: string
  idempotency_key_hash: string
  metadata_refs_json: string
  owner_user_id: string | null
  payout_intent_ref: string
  payout_target_approval_ref: string
  payout_target_ref: string
  policy_snapshot_ref: string
  public_projection_json: string
  pylon_job_ref: string | null
  source_kind: NexusTreasuryPayoutIntentRecord['sourceKind']
  spend_cap_asset: NexusTreasuryPayoutIntentRecord['spendCap']['asset']
  spend_cap_denomination: NexusTreasuryPayoutIntentRecord['spendCap']['denomination']
  spend_cap_amount_minor_units: number
  status: NexusTreasuryPayoutIntentRecord['status']
  updated_at: string
}>

type AttemptRow = Readonly<{
  adapter_attempt_ref: string
  adapter_kind: NexusTreasuryPayoutAttemptRecord['adapterKind']
  amount_asset: NexusTreasuryPayoutAttemptRecord['amount']['asset']
  amount_denomination: NexusTreasuryPayoutAttemptRecord['amount']['denomination']
  amount_minor_units: number
  archived_at: string | null
  created_at: string
  id: string
  idempotency_key_hash: string
  metadata_refs_json: string
  payout_attempt_ref: string
  payout_intent_ref: string
  public_projection_json: string
  redacted_destination_ref: string
  redacted_payment_ref: string | null
  status: NexusTreasuryPayoutAttemptRecord['status']
  updated_at: string
}>

class FakeStatement {
  values: ReadonlyArray<unknown> = []

  constructor(
    private readonly db: FakeD1Database,
    readonly query: string,
  ) {}

  bind = (...values: ReadonlyArray<unknown>) => {
    this.values = values
    this.db.bound.push({ query: this.query, values })

    return this
  }

  first = async <Row,>(): Promise<Row | null> => {
    if (this.query.includes('FROM nexus_treasury_payout_intents')) {
      if (this.query.includes('payout_intent_ref = ?')) {
        return (this.db.intentRowsByRef.get(String(this.values[0])) ?? null) as
          | Row
          | null
      }

      if (this.query.includes('idempotency_key_hash = ?')) {
        return (
          this.db.intentRowsByIdempotency.get(String(this.values[0])) ?? null
        ) as Row | null
      }
    }

    if (this.query.includes('FROM nexus_treasury_payout_attempts')) {
      if (this.query.includes('payout_attempt_ref = ?')) {
        return (this.db.attemptRowsByRef.get(String(this.values[0])) ?? null) as
          | Row
          | null
      }

      if (this.query.includes('idempotency_key_hash = ?')) {
        return (
          this.db.attemptRowsByIdempotency.get(String(this.values[0])) ?? null
        ) as Row | null
      }
    }

    return null
  }

  run = async () => {
    this.db.ran.push({ query: this.query, values: this.values })

    if (this.query.includes('INSERT OR IGNORE INTO nexus_treasury_payout_intents')) {
      // Model real SQLite `INSERT OR IGNORE` conflict semantics: when a UNIQUE
      // constraint (or, with FKs enforced, a foreign-key constraint) would be
      // violated, the row is silently dropped and `.run()` still resolves with
      // zero changes. The store must not treat that as a successful persist.
      if (this.db.dropNextIntentInsert) {
        this.db.dropNextIntentInsert = false

        return { meta: { changes: 0 } }
      }

      const row: IntentRow = {
        id: String(this.values[0]),
        payout_intent_ref: String(this.values[1]),
        idempotency_key_hash: String(this.values[2]),
        actor_ref: String(this.values[3]),
        owner_user_id: this.values[4] === null ? null : String(this.values[4]),
        source_kind: this.values[5] as IntentRow['source_kind'],
        buyer_payment_ref:
          this.values[6] === null ? null : String(this.values[6]),
        accepted_work_refs_json: String(this.values[7]),
        assignment_ref:
          this.values[8] === null ? null : String(this.values[8]),
        artanis_dispatch_ref:
          this.values[9] === null ? null : String(this.values[9]),
        pylon_job_ref:
          this.values[10] === null ? null : String(this.values[10]),
        payout_target_ref: String(this.values[11]),
        payout_target_approval_ref: String(this.values[12]),
        adapter_kind: this.values[13] as IntentRow['adapter_kind'],
        amount_asset: this.values[14] as IntentRow['amount_asset'],
        amount_denomination:
          this.values[15] as IntentRow['amount_denomination'],
        amount_minor_units: Number(this.values[16]),
        spend_cap_asset: this.values[17] as IntentRow['spend_cap_asset'],
        spend_cap_denomination:
          this.values[18] as IntentRow['spend_cap_denomination'],
        spend_cap_amount_minor_units: Number(this.values[19]),
        policy_snapshot_ref: String(this.values[20]),
        status: this.values[21] as IntentRow['status'],
        metadata_refs_json: String(this.values[22]),
        public_projection_json: String(this.values[23]),
        created_at: String(this.values[24]),
        updated_at: String(this.values[25]),
        archived_at:
          this.values[26] === null ? null : String(this.values[26]),
      }

      this.db.intentRowsByRef.set(row.payout_intent_ref, row)
      this.db.intentRowsByIdempotency.set(row.idempotency_key_hash, row)
    }

    if (this.query.includes('INSERT OR IGNORE INTO nexus_treasury_payout_attempts')) {
      const row: AttemptRow = {
        id: String(this.values[0]),
        payout_attempt_ref: String(this.values[1]),
        payout_intent_ref: String(this.values[2]),
        idempotency_key_hash: String(this.values[3]),
        adapter_kind: this.values[4] as AttemptRow['adapter_kind'],
        adapter_attempt_ref: String(this.values[5]),
        status: this.values[6] as AttemptRow['status'],
        redacted_payment_ref:
          this.values[7] === null ? null : String(this.values[7]),
        redacted_destination_ref: String(this.values[8]),
        amount_asset: this.values[9] as AttemptRow['amount_asset'],
        amount_denomination:
          this.values[10] as AttemptRow['amount_denomination'],
        amount_minor_units: Number(this.values[11]),
        metadata_refs_json: String(this.values[12]),
        public_projection_json: String(this.values[13]),
        created_at: String(this.values[14]),
        updated_at: String(this.values[15]),
        archived_at:
          this.values[16] === null ? null : String(this.values[16]),
      }

      this.db.attemptRowsByRef.set(row.payout_attempt_ref, row)
      this.db.attemptRowsByIdempotency.set(row.idempotency_key_hash, row)
    }

    return { meta: { changes: 1 } }
  }
}

class FakeD1Database {
  attemptRowsByIdempotency = new Map<string, AttemptRow>()
  attemptRowsByRef = new Map<string, AttemptRow>()
  bound: Array<{ query: string; values: ReadonlyArray<unknown> }> = []
  dropNextIntentInsert = false
  intentRowsByIdempotency = new Map<string, IntentRow>()
  intentRowsByRef = new Map<string, IntentRow>()
  ran: Array<{ query: string; values: ReadonlyArray<unknown> }> = []

  prepare = (query: string) => new FakeStatement(this, query)
}

describe('Nexus treasury payout authority ledger', () => {
  test('migration defines payout authority tables and replay-safe constraints', () => {
    const migration = readFileSync(
      'migrations/0122_nexus_treasury_payout_authority.sql',
      'utf8',
    )

    for (const table of [
      'nexus_payout_target_approvals',
      'nexus_treasury_payout_intents',
      'nexus_treasury_payout_attempts',
      'nexus_treasury_payout_reconciliation_events',
      'nexus_payment_authority_receipts',
      'nexus_release_gates',
    ]) {
      expect(migration).toContain(`CREATE TABLE IF NOT EXISTS ${table}`)
    }

    expect(migration).toContain('idempotency_key_hash TEXT NOT NULL UNIQUE')
    expect(migration).toContain('UNIQUE (provider_ref, external_event_ref)')
    expect(migration).toContain('payout_target_approval_ref TEXT NOT NULL')
    expect(migration).not.toMatch(/mnemonic|preimage|raw_invoice/i)
  })

  test('validates payout intent gates and rejects unsafe payment material', () => {
    expect(() => assertNexusTreasuryPayoutIntentSafe(intent)).not.toThrow()
    expect(() => assertNexusTreasuryPayoutAttemptSafe(attempt)).not.toThrow()

    expect(() =>
      assertNexusTreasuryPayoutIntentSafe({
        ...intent,
        acceptedWorkRefs: [],
      }),
    ).toThrow(NexusTreasuryPayoutLedgerUnsafe)

    expect(() =>
      assertNexusTreasuryPayoutIntentSafe({
        ...intent,
        payoutTargetApprovalRef: null,
      }),
    ).toThrow(NexusTreasuryPayoutLedgerUnsafe)

    expect(() =>
      assertNexusTreasuryPayoutIntentSafe({
        ...intent,
        amount: {
          amountMinorUnits: 3_000,
          asset: 'bitcoin',
          denomination: 'bitcoin_millisatoshi',
        },
      }),
    ).toThrow(NexusTreasuryPayoutLedgerUnsafe)

    expect(() =>
      assertNexusTreasuryPayoutIntentSafe({
        ...intent,
        idempotencyKeyHash: 'lnbcrawinvoice',
      }),
    ).toThrow(NexusTreasuryPayoutLedgerUnsafe)

    expect(() =>
      assertNexusTreasuryPayoutAttemptSafe({
        ...attempt,
        redactedPaymentRef: 'payment_preimage=secret',
      }),
    ).toThrow(NexusTreasuryPayoutLedgerUnsafe)
  })

  test('projects public and operator views without private material', () => {
    const publicProjection = projectNexusTreasuryPayoutLedgerRecord(
      'intent',
      intent,
      'public',
    )
    const operatorProjection = projectNexusTreasuryPayoutLedgerRecord(
      'attempt',
      attempt,
      'operator',
    )

    expect(publicProjection.ownerUserId).toBeNull()
    expect(publicProjection.payoutTargetRef).toBeNull()
    expect(operatorProjection.payoutTargetRef).toBeNull()
    expect(operatorProjection.redactedPaymentRef).toBe(
      attempt.redactedPaymentRef,
    )

    for (const projection of [publicProjection, operatorProjection]) {
      expect(S.decodeUnknownSync(NexusTreasuryPayoutLedgerProjection)(projection))
        .toEqual(projection)
      expect(nexusTreasuryPayoutLedgerProjectionHasPrivateMaterial(projection))
        .toBe(false)
    }
  })

  test('D1 store writes ledger records and rejects attempts without an intent', async () => {
    const fakeDb = new FakeD1Database()
    const store = makeD1NexusTreasuryPayoutLedgerStore(
      fakeDb as unknown as D1Database,
    )

    await store.createPayoutTargetApproval(approval)

    await expect(store.createPayoutAttempt(attempt))
      .rejects.toBeInstanceOf(NexusTreasuryPayoutLedgerUnsafe)

    await store.createPayoutIntent(intent)
    await store.createPayoutAttempt(attempt)
    await expect(
      store.readPayoutAttemptByIdempotencyKeyHash(attempt.idempotencyKeyHash),
    ).resolves.toEqual(attempt)
    await store.createReconciliationEvent(reconciliationEvent)
    await store.createPaymentAuthorityReceipt(receipt)
    await store.createReleaseGate(releaseGate)

    const queries = fakeDb.ran.map(item => item.query).join('\n')

    expect(queries).toContain(
      'INSERT OR IGNORE INTO nexus_payout_target_approvals',
    )
    expect(queries).toContain(
      'INSERT OR IGNORE INTO nexus_treasury_payout_intents',
    )
    expect(queries).toContain(
      'INSERT OR IGNORE INTO nexus_treasury_payout_attempts',
    )
    expect(queries).toContain(
      'INSERT OR IGNORE INTO nexus_treasury_payout_reconciliation_events',
    )
    expect(queries).toContain(
      'INSERT OR IGNORE INTO nexus_payment_authority_receipts',
    )
    expect(queries).toContain('INSERT OR IGNORE INTO nexus_release_gates')

    await expect(
      store.createPayoutIntent({
        ...intent,
        acceptedWorkRefs: [],
        id: 'nexus_treasury_payout_intent_invalid',
        idempotencyKeyHash: 'hash.intent.invalid',
        payoutIntentRef: 'payout_intent.invalid',
      }),
    ).rejects.toBeInstanceOf(NexusTreasuryPayoutLedgerUnsafe)
  })

  // openagents #5232: the first real settlement attempt failed CLOSED with
  // `payout_intent_not_found` at dispatch even though createPayoutIntent had
  // "succeeded". Root cause: `INSERT OR IGNORE` silently drops the intent row
  // on a constraint conflict and still resolves successfully, so the intent was
  // never durably persisted and dispatchPayout's readPayoutIntentByRef returned
  // undefined. createPayoutIntent must verify durable presence by ref and fail
  // loudly at the persistence boundary instead of leaking a misleading
  // not-found downstream.
  test('createPayoutIntent fails loudly when INSERT OR IGNORE silently drops the row (#5232)', async () => {
    const fakeDb = new FakeD1Database()
    const store = makeD1NexusTreasuryPayoutLedgerStore(
      fakeDb as unknown as D1Database,
    )

    await store.createPayoutTargetApproval(approval)

    // Force the next intent insert to be ignored (constraint conflict).
    fakeDb.dropNextIntentInsert = true

    await expect(store.createPayoutIntent(intent)).rejects.toBeInstanceOf(
      NexusTreasuryPayoutLedgerUnsafe,
    )

    // The silent drop is observable: nothing was persisted by ref.
    await expect(
      store.readPayoutIntentByRef(intent.payoutIntentRef),
    ).resolves.toBeUndefined()
  })

  // A genuine idempotent replay re-inserts the identical row. The row is
  // already present, so the post-insert verification finds it and the call
  // succeeds — retry-safety is preserved, no double-create occurs.
  test('createPayoutIntent stays idempotent on a faithful replay (#5232)', async () => {
    const fakeDb = new FakeD1Database()
    const store = makeD1NexusTreasuryPayoutLedgerStore(
      fakeDb as unknown as D1Database,
    )

    await store.createPayoutTargetApproval(approval)
    await store.createPayoutIntent(intent)
    await expect(store.createPayoutIntent(intent)).resolves.toBeUndefined()
    await expect(
      store.readPayoutIntentByRef(intent.payoutIntentRef),
    ).resolves.toEqual(intent)
  })
})
