import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  OmniAcceptedOutcomeContractValidationError,
  createOmniAcceptedOutcomeContract,
  publicOmniAcceptedOutcomeContractProjection,
} from './omni-accepted-outcome-contracts'

type ContractRow = Readonly<{
  acceptance_state:
    | 'draft'
    | 'pending_review'
    | 'provisionally_accepted'
    | 'accepted'
    | 'rejected'
    | 'revision_requested'
    | 'reopened'
    | 'unavailable'
  archived_at: string | null
  closeout_requirements_json: string
  committed_deliverables_json: string
  created_at: string
  customer_ref: string | null
  economic_state:
    | 'free_beta'
    | 'paid_required'
    | 'credits_required'
    | 'sats_required'
    | 'internal_only'
  expected_artifacts_json: string
  fulfillment_receipts_json: string
  id: string
  idempotency_key: string
  legal_sensitive: number
  metadata_json: string
  proof_policy:
    | 'private_receipt'
    | 'customer_safe_summary'
    | 'public_safe_proof'
    | 'legal_sensitive_private'
  public_receipt_ref: string
  review_policy:
    | 'operator_review'
    | 'customer_review'
    | 'dual_review'
    | 'owner_review'
    | 'no_review'
  service_promise_state:
    | 'not_promised'
    | 'proposed'
    | 'active'
    | 'fulfilled'
    | 'paused'
    | 'breached'
    | 'cancelled'
  sla_terms_json: string
  subject_ref: string
  updated_at: string
  work_kind:
    | 'site'
    | 'coding'
    | 'adjustment'
    | 'existing_project_import'
    | 'business'
    | 'legal_sensitive'
}>

class AcceptedOutcomeContractStore {
  contracts: Array<ContractRow> = []
}

const runtime = {
  makeContractId: () => 'omni_accepted_outcome_contract_generated',
  nowIso: () => '2026-06-05T23:10:00.000Z',
}

class AcceptedOutcomeContractStatement implements D1PreparedStatement {
  private values: ReadonlyArray<unknown> = []

  constructor(
    private readonly query: string,
    private readonly store: AcceptedOutcomeContractStore,
  ) {}

  bind(...values: ReadonlyArray<unknown>): D1PreparedStatement {
    this.values = values

    return this
  }

  first<T = unknown>(colName: string): Promise<T | null>
  first<T = Record<string, unknown>>(): Promise<T | null>
  first<T = unknown>(): Promise<T | null> {
    if (this.query.includes('FROM omni_accepted_outcome_contracts')) {
      const idempotencyKey = String(this.values[0])
      const row =
        this.store.contracts.find(
          item =>
            item.idempotency_key === idempotencyKey &&
            item.archived_at === null,
        ) ?? null

      return Promise.resolve(row as T | null)
    }

    return Promise.reject(new Error(`Unexpected first: ${this.query}`))
  }

  run<T = Record<string, unknown>>(): Promise<D1Result<T>> {
    if (
      this.query.includes('INSERT OR IGNORE INTO omni_accepted_outcome_contracts')
    ) {
      const idempotencyKey = String(this.values[1])

      if (
        this.store.contracts.every(
          item => item.idempotency_key !== idempotencyKey,
        )
      ) {
        this.store.contracts.push({
          acceptance_state: this.values[7] as ContractRow['acceptance_state'],
          archived_at: null,
          closeout_requirements_json: String(this.values[10]),
          committed_deliverables_json: String(this.values[11]),
          created_at: String(this.values[18]),
          customer_ref: this.values[4] as string | null,
          economic_state: this.values[9] as ContractRow['economic_state'],
          expected_artifacts_json: String(this.values[5]),
          fulfillment_receipts_json: String(this.values[14]),
          id: String(this.values[0]),
          idempotency_key: idempotencyKey,
          legal_sensitive: Number(this.values[15]),
          metadata_json: String(this.values[17]),
          proof_policy: this.values[8] as ContractRow['proof_policy'],
          public_receipt_ref: String(this.values[16]),
          review_policy: this.values[6] as ContractRow['review_policy'],
          service_promise_state:
            this.values[12] as ContractRow['service_promise_state'],
          sla_terms_json: String(this.values[13]),
          subject_ref: String(this.values[3]),
          updated_at: String(this.values[19]),
          work_kind: this.values[2] as ContractRow['work_kind'],
        })
      }

      return Promise.resolve({ success: true } as D1Result<T>)
    }

    return Promise.reject(new Error(`Unexpected run: ${this.query}`))
  }

  all<T = Record<string, unknown>>(): Promise<D1Result<T>> {
    return Promise.resolve({ results: [] } as unknown as D1Result<T>)
  }

  raw<T = unknown[]>(options: {
    columnNames: true
  }): Promise<[Array<string>, ...Array<T>]>
  raw<T = unknown[]>(options?: { columnNames?: false }): Promise<Array<T>>
  raw<T = unknown[]>(options?: {
    columnNames?: boolean
  }): Promise<Array<T> | [Array<string>, ...Array<T>]> {
    return options?.columnNames === true ? Promise.resolve([[]]) : Promise.resolve([])
  }
}

const acceptedOutcomeContractDb = (
  store: AcceptedOutcomeContractStore,
): D1Database => ({
  batch: () => Promise.reject(new Error('D1 batch should not be used')),
  dump: () => Promise.reject(new Error('D1 dump should not be used')),
  exec: () => Promise.reject(new Error('D1 exec should not be used')),
  prepare: query => new AcceptedOutcomeContractStatement(query, store),
  withSession: () => {
    throw new Error('D1 session should not be used')
  },
})

const baseExpectedArtifacts = [
  {
    artifactKind: 'site_url',
    publicSafe: true,
    required: true,
    sourceRef: 'site_revision_latest_url_ref',
  },
  {
    artifactKind: 'build_log',
    publicSafe: false,
    required: true,
    sourceRef: 'worker_build_log_redacted_ref',
  },
] as const

const baseCloseoutRequirements = [
  {
    requirementKind: 'customer_review',
    required: true,
    sourceRef: 'customer_review_required_ref',
  },
  {
    requirementKind: 'redaction_passed',
    required: true,
    sourceRef: 'redaction_report_required_ref',
  },
] as const

const createContract = (
  store: AcceptedOutcomeContractStore,
  overrides: Partial<
    Parameters<typeof createOmniAcceptedOutcomeContract>[1]
  > = {},
) =>
  Effect.runPromise(
    createOmniAcceptedOutcomeContract(
      acceptedOutcomeContractDb(store),
      {
        closeoutRequirements: baseCloseoutRequirements,
        economicState: 'free_beta',
        expectedArtifacts: baseExpectedArtifacts,
        id: 'omni_accepted_outcome_contract_1',
        idempotencyKey: 'accepted-outcome-contract:site:1',
        proofPolicy: 'customer_safe_summary',
        reviewPolicy: 'dual_review',
        subjectRef: 'software_order_otec',
        workKind: 'site',
        ...overrides,
      },
      runtime,
    ),
  )

describe('Omni accepted outcome contracts', () => {
  test('records Site contracts idempotently with public-safe projection', async () => {
    const store = new AcceptedOutcomeContractStore()
    const contract = await createContract(store)
    const replay = await createContract(store, {
      subjectRef: 'software_order_changed',
    })
    const projection = publicOmniAcceptedOutcomeContractProjection(contract)

    expect(replay.subjectRef).toBe('software_order_otec')
    expect(contract).toMatchObject({
      acceptanceState: 'draft',
      economicState: 'free_beta',
      publicReceiptRef:
        'omni_accepted_outcome:site:accepted-outcome-contract:site:1',
      reviewPolicy: 'dual_review',
      workKind: 'site',
    })
    expect(projection).toEqual({
      acceptanceState: 'draft',
      closeoutRequirementCount: 2,
      committedDeliverableCount: 0,
      economicState: 'free_beta',
      expectedArtifactCount: 2,
      fulfillmentReceiptSummary: {
        blocked: 0,
        failed: 0,
        fulfilled: 0,
        partial: 0,
        total: 0,
      },
      legalSensitive: false,
      proofPolicy: 'customer_safe_summary',
      publicCommittedDeliverables: [],
      publicExpectedArtifacts: [
        {
          artifactKind: 'site_url',
          required: true,
          sourceRef: 'site_revision_latest_url_ref',
        },
      ],
      publicReceiptRef:
        'omni_accepted_outcome:site:accepted-outcome-contract:site:1',
      reviewPolicy: 'dual_review',
      servicePromiseState: 'not_promised',
      slaTermCount: 0,
      subjectRef: 'software_order_otec',
      workKind: 'site',
    })
    expect(projection).not.toHaveProperty('metadata')
    expect(projection).not.toHaveProperty('customerRef')
  })

  test('supports coding and existing-project import contracts', async () => {
    const store = new AcceptedOutcomeContractStore()
    const coding = await createContract(store, {
      closeoutRequirements: [
        {
          requirementKind: 'tests_passed',
          required: true,
          sourceRef: 'test_report_required_ref',
        },
      ],
      economicState: 'credits_required',
      expectedArtifacts: [
        {
          artifactKind: 'pull_request',
          publicSafe: true,
          required: true,
          sourceRef: 'github_pull_request_ref',
        },
        {
          artifactKind: 'diff',
          publicSafe: true,
          required: true,
          sourceRef: 'source_diff_ref',
        },
      ],
      id: 'omni_accepted_outcome_contract_coding',
      idempotencyKey: 'accepted-outcome-contract:coding:1',
      proofPolicy: 'public_safe_proof',
      reviewPolicy: 'operator_review',
      subjectRef: 'github_repo_issue_ref',
      workKind: 'coding',
    })

    expect(coding).toMatchObject({
      economicState: 'credits_required',
      proofPolicy: 'public_safe_proof',
      reviewPolicy: 'operator_review',
      subjectRef: 'github_repo_issue_ref',
      workKind: 'coding',
    })
  })

  test('records per-customer service promises with SLA terms and verifier receipts', async () => {
    const store = new AcceptedOutcomeContractStore()
    const contract = await createContract(store, {
      committedDeliverables: [
        {
          backingCapabilityRef: 'promise.autopilot.agentic_labor_products.v1',
          backingCapabilityState: 'yellow',
          deliverableRef: 'deliverable.customer_safe_site_launch',
          expectedArtifactKind: 'site_url',
          required: true,
          sourceRef: 'scope.customer_safe_site_launch',
        },
      ],
      fulfillmentReceipts: [
        {
          blockerRefs: [],
          deliverableRef: 'deliverable.customer_safe_site_launch',
          evaluatedAt: '2026-07-02T12:00:00.000Z',
          evidenceRef: 'evidence.site_launch_verifier_passed',
          receiptRef: 'fulfillment_receipt.site_launch.1',
          state: 'fulfilled',
          verifierRef: 'verifier.promise_fulfillment.site_launch.v1',
        },
      ],
      idempotencyKey: 'accepted-outcome-contract:service-promise:1',
      servicePromiseState: 'active',
      slaTerms: [
        {
          dueAt: '2026-07-09T00:00:00.000Z',
          metricRef: 'metric.first_delivery',
          sourceRef: 'sla.quick_win.first_delivery',
          target: 5,
          termRef: 'sla.first_delivery.business_days',
          unit: 'business_days',
        },
      ],
    })
    const projection = publicOmniAcceptedOutcomeContractProjection(contract)

    expect(contract.servicePromiseState).toBe('active')
    expect(contract.committedDeliverables).toHaveLength(1)
    expect(contract.slaTerms).toHaveLength(1)
    expect(contract.fulfillmentReceipts).toHaveLength(1)
    expect(projection).toMatchObject({
      committedDeliverableCount: 1,
      fulfillmentReceiptSummary: {
        blocked: 0,
        failed: 0,
        fulfilled: 1,
        partial: 0,
        total: 1,
      },
      publicCommittedDeliverables: [
        {
          backingCapabilityRef: 'promise.autopilot.agentic_labor_products.v1',
          backingCapabilityState: 'yellow',
          deliverableRef: 'deliverable.customer_safe_site_launch',
          expectedArtifactKind: 'site_url',
          required: true,
        },
      ],
      servicePromiseState: 'active',
      slaTermCount: 1,
    })
  })

  test('fulfillment receipts evidence promises without flipping service promise state', async () => {
    const contract = await createContract(new AcceptedOutcomeContractStore(), {
      committedDeliverables: [
        {
          backingCapabilityRef: 'promise.autopilot.agentic_labor_products.v1',
          backingCapabilityState: 'green',
          deliverableRef: 'deliverable.reviewed_campaign_receipt',
          expectedArtifactKind: 'operator_receipt',
          required: true,
          sourceRef: 'scope.reviewed_campaign_receipt',
        },
      ],
      fulfillmentReceipts: [
        {
          blockerRefs: [],
          deliverableRef: 'deliverable.reviewed_campaign_receipt',
          evaluatedAt: '2026-07-02T13:00:00.000Z',
          evidenceRef: 'evidence.campaign_receipt_verified',
          receiptRef: 'fulfillment_receipt.campaign.1',
          state: 'fulfilled',
          verifierRef: 'verifier.promise_fulfillment.campaign.v1',
        },
      ],
      idempotencyKey: 'accepted-outcome-contract:service-promise:decoupled',
      servicePromiseState: 'breached',
    })

    expect(contract.fulfillmentReceipts[0]?.state).toBe('fulfilled')
    expect(contract.servicePromiseState).toBe('breached')
  })

  test('rejects committed deliverables backed by red or planned capabilities', async () => {
    await expect(
      createContract(new AcceptedOutcomeContractStore(), {
        committedDeliverables: [
          {
            backingCapabilityRef: 'promise.autopilot.superapp_desktop_business.v1',
            backingCapabilityState: 'red',
            deliverableRef: 'deliverable.unsupported_business_suite',
            expectedArtifactKind: 'operator_receipt',
            required: true,
            sourceRef: 'scope.unsupported_business_suite',
          },
        ],
        idempotencyKey: 'accepted-outcome-contract:service-promise:red',
        servicePromiseState: 'active',
      }),
    ).rejects.toBeInstanceOf(OmniAcceptedOutcomeContractValidationError)

    await expect(
      createContract(new AcceptedOutcomeContractStore(), {
        committedDeliverables: [
          {
            backingCapabilityRef: 'promise.future.private_compute_tier.v1',
            backingCapabilityState: 'planned',
            deliverableRef: 'deliverable.future_private_compute',
            expectedArtifactKind: 'operator_receipt',
            required: true,
            sourceRef: 'scope.future_private_compute',
          },
        ],
        idempotencyKey: 'accepted-outcome-contract:service-promise:planned',
        servicePromiseState: 'active',
      }),
    ).rejects.toBeInstanceOf(OmniAcceptedOutcomeContractValidationError)
  })

  test('enforces legal-sensitive private proof policy', async () => {
    await expect(
      createContract(new AcceptedOutcomeContractStore(), {
        idempotencyKey: 'accepted-outcome-contract:legal:invalid',
        proofPolicy: 'public_safe_proof',
        subjectRef: 'legal_review_order_ref',
        workKind: 'legal_sensitive',
      }),
    ).rejects.toBeInstanceOf(OmniAcceptedOutcomeContractValidationError)

    const legal = await createContract(new AcceptedOutcomeContractStore(), {
      idempotencyKey: 'accepted-outcome-contract:legal:valid',
      proofPolicy: 'legal_sensitive_private',
      subjectRef: 'legal_review_order_ref',
      workKind: 'legal_sensitive',
    })

    expect(legal).toMatchObject({
      legalSensitive: true,
      proofPolicy: 'legal_sensitive_private',
      workKind: 'legal_sensitive',
    })
  })

  test('rejects public proof contracts with private required artifacts', async () => {
    await expect(
      createContract(new AcceptedOutcomeContractStore(), {
        expectedArtifacts: baseExpectedArtifacts,
        idempotencyKey: 'accepted-outcome-contract:public-private',
        proofPolicy: 'public_safe_proof',
      }),
    ).rejects.toBeInstanceOf(OmniAcceptedOutcomeContractValidationError)
  })

  test('rejects raw provider, run log, email, payment, wallet, and customer material', async () => {
    await expect(
      createContract(new AcceptedOutcomeContractStore(), {
        idempotencyKey: 'accepted-outcome-contract:private-metadata',
        metadata: { rawEmail: 'ben@example.com' },
      }),
    ).rejects.toBeInstanceOf(OmniAcceptedOutcomeContractValidationError)

    await expect(
      createContract(new AcceptedOutcomeContractStore(), {
        idempotencyKey: 'accepted-outcome-contract:payment-secret',
        publicReceiptRef: 'lnbc1rawinvoice',
      }),
    ).rejects.toBeInstanceOf(OmniAcceptedOutcomeContractValidationError)
  })
})
