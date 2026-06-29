import { describe, expect, test } from 'vitest'

import { type PartnerAgreement } from './partner-attribution-policy'
import { type PartnerQualifyingPaidEvent } from './partner-attribution-eligibility'
import {
  type CreatePartnerPayoutEligibilityInput,
  type PartnerPayoutLedgerEntry,
  createPartnerPayoutEligibility,
} from './partner-payout-ledger'
import {
  type CreatePartnerAgreementInput,
  PartnerAgreementValidationError,
  type PartnerAgreementReader,
  readActivePartnerAgreementsForCustomer,
  recordPartnerAgreement,
  recordPartnerPayoutForPaidEvent,
} from './partner-payout-feed'

const baseEvent: PartnerQualifyingPaidEvent = {
  asset: 'usd',
  customerUserId: 'github:client',
  eventIso: '2026-06-10T09:00:00.000Z',
  idempotencyKey: 'partner-payout:feed:1',
  periodKey: '2026-06',
  qualifyingAmount: 10000,
  qualifyingEventKind: 'design_partner_engagement',
  qualifyingEventRef: 'partner_event_engagement_1',
}

const designPartnerAgreement: PartnerAgreement = {
  agreementRef: 'partner_agreement_acme',
  effectiveFromIso: '2026-01-01T00:00:00.000Z',
  effectiveUntilIso: null,
  partnerRef: 'design_partner_acme',
  partnerUserId: 'github:acme_agency',
  role: 'design_partner',
}

const affiliateAgreement: PartnerAgreement = {
  agreementRef: 'partner_agreement_aff',
  effectiveFromIso: '2026-02-01T00:00:00.000Z',
  effectiveUntilIso: null,
  partnerRef: 'affiliate_beta',
  partnerUserId: 'github:beta_affiliate',
  role: 'affiliate',
}

const stubReader =
  (agreements: ReadonlyArray<PartnerAgreement>): PartnerAgreementReader =>
  () =>
    Promise.resolve(agreements)

// A capturing fake of createPartnerPayoutEligibility: records the input it is
// handed and returns a minimal eligible entry. Lets us assert the feed mapping
// without a live ledger/D1.
const capturingCreate = (
  captured: Array<CreatePartnerPayoutEligibilityInput>,
): typeof createPartnerPayoutEligibility =>
  (_db, input) => {
    captured.push(input)

    const entry: PartnerPayoutLedgerEntry = {
      amount: 2000,
      archivedAt: null,
      asset: input.asset,
      beneficiaryUserId: input.beneficiaryUserId ?? null,
      caveatRefs: [],
      createdAt: input.nowIso,
      evidenceRefs: [input.qualifyingEventRef],
      id: 'partner_payout_entry_test',
      idempotencyKey: input.idempotencyKey,
      partnerRef: input.partnerRef,
      partnerRole: input.partnerRole,
      partnerUserId: input.partnerUserId,
      payoutRef:
        input.payoutRef ??
        `partner_payout_${input.partnerRole}_${input.partnerRef}`,
      periodKey: input.periodKey,
      policyRefs: [],
      previousEntryId: null,
      qualifyingAmount: input.qualifyingAmount,
      qualifyingEventKind: input.qualifyingEventKind,
      qualifyingEventRef: input.qualifyingEventRef,
      reversalOfEntryId: null,
      state: 'eligible',
      stateReasonRef: null,
    }

    return Promise.resolve(entry)
  }

const unusedDb = {} as D1Database

describe('recordPartnerPayoutForPaidEvent', () => {
  test('records nothing when the customer has no active agreement', async () => {
    const captured: Array<CreatePartnerPayoutEligibilityInput> = []
    const result = await recordPartnerPayoutForPaidEvent(unusedDb, baseEvent, {
      createEligibility: capturingCreate(captured),
      readAgreements: stubReader([]),
    })

    expect(result._tag).toBe('no_active_agreement')
    expect(captured).toHaveLength(0)
  })

  test('records an eligibility row mapped from the winning agreement', async () => {
    const captured: Array<CreatePartnerPayoutEligibilityInput> = []
    const result = await recordPartnerPayoutForPaidEvent(unusedDb, baseEvent, {
      createEligibility: capturingCreate(captured),
      readAgreements: stubReader([designPartnerAgreement]),
    })

    expect(result).toMatchObject({
      _tag: 'recorded',
      agreementRef: 'partner_agreement_acme',
      policyRef: 'policy.partner_attribution.v1',
    })
    expect(captured).toHaveLength(1)
    expect(captured[0]).toMatchObject({
      asset: 'usd',
      beneficiaryUserId: 'github:client',
      // the explicit attribution basis is carried onto the ledger input
      evidenceRefs: ['partner_agreement_acme'],
      idempotencyKey: 'partner-payout:feed:1',
      partnerRef: 'design_partner_acme',
      partnerRole: 'design_partner',
      partnerUserId: 'github:acme_agency',
      policyRefs: ['policy.partner_attribution.v1'],
      qualifyingAmount: 10000,
      qualifyingEventRef: 'partner_event_engagement_1',
    })
  })

  test('credits design_partner over affiliate when both agreements are active', async () => {
    const captured: Array<CreatePartnerPayoutEligibilityInput> = []
    const result = await recordPartnerPayoutForPaidEvent(unusedDb, baseEvent, {
      createEligibility: capturingCreate(captured),
      readAgreements: stubReader([affiliateAgreement, designPartnerAgreement]),
    })

    expect(result._tag).toBe('recorded')
    expect(captured[0]?.partnerRole).toBe('design_partner')
  })

  test('records nothing for a self-attributed paying customer', async () => {
    const captured: Array<CreatePartnerPayoutEligibilityInput> = []
    const selfEvent: PartnerQualifyingPaidEvent = {
      ...baseEvent,
      customerUserId: 'github:acme_agency',
    }
    const result = await recordPartnerPayoutForPaidEvent(unusedDb, selfEvent, {
      createEligibility: capturingCreate(captured),
      readAgreements: stubReader([designPartnerAgreement]),
    })

    expect(result).toMatchObject({
      _tag: 'self_attribution',
      partnerRef: 'design_partner_acme',
    })
    expect(captured).toHaveLength(0)
  })

  test('records nothing when the only agreement is outside its active window', async () => {
    const captured: Array<CreatePartnerPayoutEligibilityInput> = []
    const expired: PartnerAgreement = {
      ...designPartnerAgreement,
      effectiveUntilIso: '2026-03-01T00:00:00.000Z',
    }
    const result = await recordPartnerPayoutForPaidEvent(unusedDb, baseEvent, {
      createEligibility: capturingCreate(captured),
      readAgreements: stubReader([expired]),
    })

    expect(result._tag).toBe('no_active_agreement')
    expect(captured).toHaveLength(0)
  })
})

// Minimal D1 mock exercising the default storage reader's query/binding path.
type AgreementStoreRow = Readonly<{
  agreement_ref: string
  effective_from: string
  effective_until: string | null
  partner_ref: string
  partner_user_id: string
  role: 'affiliate' | 'design_partner'
}>

const agreementReaderDb = (
  rowsByCustomer: ReadonlyMap<string, ReadonlyArray<AgreementStoreRow>>,
  calls: Array<{ customerUserId: string; query: string }>,
): D1Database => {
  const statement = (query: string) => {
    const make = (boundCustomer: string | null): D1PreparedStatement =>
      ({
        bind: (...values: ReadonlyArray<unknown>) =>
          make(String(values[0] ?? '')),
        all: <T,>() => {
          calls.push({ customerUserId: boundCustomer ?? '', query })

          return Promise.resolve({
            meta: {} as D1Meta,
            results: ((rowsByCustomer.get(boundCustomer ?? '') ??
              []) as unknown) as Array<T>,
            success: true,
          } as D1Result<T>)
        },
        first: () => Promise.reject(new Error('first should not be used')),
        raw: () => Promise.reject(new Error('raw should not be used')),
        run: () => Promise.reject(new Error('run should not be used')),
      }) as unknown as D1PreparedStatement

    return make(null)
  }

  return {
    batch: () => Promise.reject(new Error('batch should not be used')),
    dump: () => Promise.reject(new Error('dump should not be used')),
    exec: () => Promise.reject(new Error('exec should not be used')),
    prepare: (query: string) => statement(query),
    withSession: () => {
      throw new Error('session should not be used')
    },
  } as unknown as D1Database
}

describe('readActivePartnerAgreementsForCustomer', () => {
  test('reads and maps active agreements for a customer', async () => {
    const calls: Array<{ customerUserId: string; query: string }> = []
    const db = agreementReaderDb(
      new Map([
        [
          'github:client',
          [
            {
              agreement_ref: 'partner_agreement_acme',
              effective_from: '2026-01-01T00:00:00.000Z',
              effective_until: null,
              partner_ref: 'design_partner_acme',
              partner_user_id: 'github:acme_agency',
              role: 'design_partner',
            },
          ],
        ],
      ]),
      calls,
    )

    const agreements = await readActivePartnerAgreementsForCustomer(
      db,
      'github:client',
    )

    expect(agreements).toEqual([
      {
        agreementRef: 'partner_agreement_acme',
        effectiveFromIso: '2026-01-01T00:00:00.000Z',
        effectiveUntilIso: null,
        partnerRef: 'design_partner_acme',
        partnerUserId: 'github:acme_agency',
        role: 'design_partner',
      },
    ])
    expect(calls).toHaveLength(1)
    expect(calls[0]?.query).toContain('FROM partner_agreements')
    expect(calls[0]?.query).toContain("policy_state = 'active'")
  })

  test('returns nothing and issues no query for a malformed user id', async () => {
    const calls: Array<{ customerUserId: string; query: string }> = []
    const db = agreementReaderDb(new Map(), calls)

    const agreements = await readActivePartnerAgreementsForCustomer(
      db,
      'not a safe id!!',
    )

    expect(agreements).toEqual([])
    expect(calls).toHaveLength(0)
  })
})

// Minimal in-memory D1 mock for the agreement writer: INSERT OR IGNORE keyed on
// agreement_ref, SELECT ... WHERE agreement_ref = ? returns the stored row.
type StoredAgreementRow = Readonly<{
  agreement_ref: string
  effective_from: string
  effective_until: string | null
  partner_ref: string
  partner_user_id: string
  role: 'affiliate' | 'design_partner'
}>

const agreementWriterDb = (
  store: Map<string, StoredAgreementRow>,
  ops: Array<string>,
): D1Database => {
  const statement = (
    query: string,
    bound: ReadonlyArray<unknown> = [],
  ): D1PreparedStatement => {
    return ({
      bind: (...values: ReadonlyArray<unknown>) => statement(query, values),
      first: <T,>() => {
        ops.push('first')
        const ref = String(bound[0] ?? '')
        const row = store.get(ref) ?? null

        return Promise.resolve((row as unknown) as T | null)
      },
      run: () => {
        ops.push('run')
        const [, agreementRef, partnerRef, partnerUserId, , role, from, until] =
          bound

        if (!store.has(String(agreementRef))) {
          store.set(String(agreementRef), {
            agreement_ref: String(agreementRef),
            effective_from: String(from),
            effective_until: until === null ? null : String(until),
            partner_ref: String(partnerRef),
            partner_user_id: String(partnerUserId),
            role: role as 'affiliate' | 'design_partner',
          })
        }

        return Promise.resolve(({
          meta: {} as D1Meta,
          results: [],
          success: true,
        } as unknown) as D1Result)
      },
      all: () => Promise.reject(new Error('all should not be used')),
      raw: () => Promise.reject(new Error('raw should not be used')),
    } as unknown) as D1PreparedStatement
  }

  return {
    batch: () => Promise.reject(new Error('batch should not be used')),
    dump: () => Promise.reject(new Error('dump should not be used')),
    exec: () => Promise.reject(new Error('exec should not be used')),
    prepare: (query: string) => statement(query),
    withSession: () => {
      throw new Error('session should not be used')
    },
  } as unknown as D1Database
}

const baseAgreementInput: CreatePartnerAgreementInput = {
  agreementRef: 'partner_agreement_acme',
  customerUserId: 'github:client',
  effectiveFromIso: '2026-01-01T00:00:00.000Z',
  effectiveUntilIso: null,
  nowIso: '2026-01-01T00:00:00.000Z',
  partnerRef: 'design_partner_acme',
  partnerUserId: 'github:acme_agency',
  role: 'design_partner',
}

describe('recordPartnerAgreement', () => {
  test('seeds a policy-conformant agreement and reads it back', async () => {
    const store = new Map<string, StoredAgreementRow>()
    const ops: Array<string> = []
    const db = agreementWriterDb(store, ops)

    const agreement = await recordPartnerAgreement(db, baseAgreementInput)

    expect(agreement).toEqual({
      agreementRef: 'partner_agreement_acme',
      effectiveFromIso: '2026-01-01T00:00:00.000Z',
      effectiveUntilIso: null,
      partnerRef: 'design_partner_acme',
      partnerUserId: 'github:acme_agency',
      role: 'design_partner',
    })
    expect(ops).toContain('run')
  })

  test('is idempotent on agreementRef (no second insert)', async () => {
    const store = new Map<string, StoredAgreementRow>()
    const ops: Array<string> = []
    const db = agreementWriterDb(store, ops)

    await recordPartnerAgreement(db, baseAgreementInput)
    const runsAfterFirst = ops.filter(op => op === 'run').length
    await recordPartnerAgreement(db, baseAgreementInput)

    expect(ops.filter(op => op === 'run').length).toBe(runsAfterFirst)
  })

  test('rejects the referral role before touching storage', async () => {
    const store = new Map<string, StoredAgreementRow>()
    const ops: Array<string> = []
    const db = agreementWriterDb(store, ops)

    await expect(
      recordPartnerAgreement(db, {
        ...baseAgreementInput,
        role: 'referral' as CreatePartnerAgreementInput['role'],
      }),
    ).rejects.toBeInstanceOf(PartnerAgreementValidationError)
    expect(ops).toHaveLength(0)
  })

  test('rejects a self-agreement (partner == customer)', async () => {
    const store = new Map<string, StoredAgreementRow>()
    const ops: Array<string> = []
    const db = agreementWriterDb(store, ops)

    await expect(
      recordPartnerAgreement(db, {
        ...baseAgreementInput,
        customerUserId: 'github:acme_agency',
      }),
    ).rejects.toBeInstanceOf(PartnerAgreementValidationError)
    expect(ops).toHaveLength(0)
  })

  test('rejects a non-public-safe ref before touching storage', async () => {
    const store = new Map<string, StoredAgreementRow>()
    const ops: Array<string> = []
    const db = agreementWriterDb(store, ops)

    await expect(
      recordPartnerAgreement(db, {
        ...baseAgreementInput,
        agreementRef: 'not a safe ref!!',
      }),
    ).rejects.toBeInstanceOf(PartnerAgreementValidationError)
    expect(ops).toHaveLength(0)
  })
})
