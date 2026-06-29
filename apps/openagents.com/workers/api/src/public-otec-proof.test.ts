import { Effect, Schema as S } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  OTEC_SOFTWARE_ORDER_ID,
  PublicOtecProofCloseout,
} from './public-otec-proof'
import { handlePublicOtecProofApi } from './public-otec-proof-routes'

type BaseRow = Readonly<{
  active_deployment_id: string | null
  active_deployment_status: string | null
  active_deployment_updated_at: string | null
  active_deployment_url: string | null
  active_version_id: string | null
  assignment_current_run_id: string | null
  assignment_id: string | null
  assignment_kind: string | null
  assignment_status: string | null
  assignment_updated_at: string | null
  order_id: string
  order_request: string
  order_status: string
  order_updated_at: string
  repository_full_name: string | null
  site_access_mode: string | null
  site_id: string | null
  site_slug: string | null
  site_status: string | null
  site_title: string | null
  site_updated_at: string | null
  site_visibility: string | null
}>

type ResearchRow = Readonly<{
  approved_at: string | null
  approved_source_count: number
  research_brief_id: string | null
  run_id: string
  source_count: number
  status: string
  updated_at: string
}>

type VersionRow = Readonly<{
  build_status: string
  created_at: string
  id: string
  saved_at: string | null
  source_commit_sha: string | null
}>

type CompatibilityRow = Readonly<{
  blockers_json: string
  customer_safe_next_action: string
  customer_safe_status: string
  id: string
  status: string
  warnings_json: string
}>

type BuildValidationRow = Readonly<{
  blockers_json: string
  customer_safe_next_action: string
  customer_safe_status: string
  id: string
  source_hash: string
  status: string
  warnings_json: string
}>

type ReceiptRow = Readonly<{
  created_at: string
  id: string
  software_order_id: string
  visibility: string
}>

class PublicOtecProofStore {
  base: BaseRow | null = baseRow()
  research: ResearchRow | null = researchRow()
  latestVersion: VersionRow | null = versionRow()
  compatibility: CompatibilityRow | null = compatibilityRow()
  buildValidation: BuildValidationRow | null = buildValidationRow()
  receipts: Array<ReceiptRow> = [
    {
      created_at: '2026-06-05T00:10:00.000Z',
      id: 'adjutant_usage_receipt_public',
      software_order_id: OTEC_SOFTWARE_ORDER_ID,
      visibility: 'public',
    },
  ]
}

const d1Meta = (): D1Meta & Record<string, unknown> => ({
  changed_db: false,
  changes: 0,
  duration: 0,
  last_row_id: 0,
  rows_read: 0,
  rows_written: 0,
  size_after: 0,
})

class PublicOtecProofStatement implements D1PreparedStatement {
  private values: ReadonlyArray<unknown> = []

  constructor(
    private readonly query: string,
    private readonly store: PublicOtecProofStore,
  ) {}

  bind(...values: ReadonlyArray<unknown>): D1PreparedStatement {
    this.values = values

    return this
  }

  first<T = unknown>(colName: string): Promise<T | null>
  first<T = Record<string, unknown>>(): Promise<T | null>
  first<T = unknown>(): Promise<T | null> {
    if (this.query.includes('FROM software_orders')) {
      const [softwareOrderId] = this.values

      if (
        this.store.base === null ||
        this.store.base.order_id !== softwareOrderId
      ) {
        return Promise.resolve(null)
      }

      return Promise.resolve(this.store.base as T)
    }

    if (this.query.includes('FROM exa_enrichment_runs')) {
      return Promise.resolve((this.store.research as T | null) ?? null)
    }

    if (this.query.includes('FROM site_versions')) {
      return Promise.resolve((this.store.latestVersion as T | null) ?? null)
    }

    if (this.query.includes('FROM site_compatibility_checks')) {
      return Promise.resolve((this.store.compatibility as T | null) ?? null)
    }

    if (this.query.includes('FROM site_build_validations')) {
      return Promise.resolve((this.store.buildValidation as T | null) ?? null)
    }

    return Promise.reject(new Error(`Unexpected D1 first: ${this.query}`))
  }

  run<T = Record<string, unknown>>(): Promise<D1Result<T>> {
    return Promise.reject(new Error('D1 run should not be used'))
  }

  all<T = Record<string, unknown>>(): Promise<D1Result<T>> {
    if (this.query.includes('FROM adjutant_usage_receipts')) {
      const [softwareOrderId] = this.values

      return Promise.resolve({
        meta: d1Meta(),
        results: this.store.receipts
          .filter(
            receipt =>
              receipt.software_order_id === softwareOrderId &&
              receipt.visibility === 'public',
          )
          .sort((left, right) => right.created_at.localeCompare(left.created_at))
          .slice(0, 25) as Array<T>,
        success: true,
      })
    }

    return Promise.reject(new Error(`Unexpected D1 all: ${this.query}`))
  }

  raw<T = unknown[]>(options: {
    columnNames: true
  }): Promise<[string[], ...T[]]>
  raw<T = unknown[]>(options?: { columnNames?: false }): Promise<T[]>
  raw<T = unknown[]>(): Promise<[string[], ...T[]] | T[]> {
    return Promise.reject(new Error('D1 raw should not be used'))
  }
}

const publicOtecProofDb = (store: PublicOtecProofStore): D1Database => ({
  batch: () => Promise.reject(new Error('D1 batch should not be used')),
  dump: () => Promise.reject(new Error('D1 dump should not be used')),
  exec: () => Promise.reject(new Error('D1 exec should not be used')),
  prepare: query => new PublicOtecProofStatement(query, store),
  withSession: () => {
    throw new Error('D1 session should not be used')
  },
})

function baseRow(input: Partial<BaseRow> = {}): BaseRow {
  return {
    active_deployment_id: 'site_deployment_otec_active',
    active_deployment_status: 'active',
    active_deployment_updated_at: '2026-06-05T00:09:00.000Z',
    active_deployment_url: 'https://sites.openagents.com/ben-otec',
    active_version_id: 'site_version_otec_saved',
    assignment_current_run_id: 'agent_run_otec',
    assignment_id: 'adjutant_assignment_otec',
    assignment_kind: 'site_generation',
    assignment_status: 'deployed',
    assignment_updated_at: '2026-06-05T00:08:00.000Z',
    order_id: OTEC_SOFTWARE_ORDER_ID,
    order_request: 'Build a public OTEC proof site with safe research summaries.',
    order_status: 'delivered',
    order_updated_at: '2026-06-05T00:07:00.000Z',
    repository_full_name: 'OpenAgentsInc/otec-site',
    site_access_mode: 'public',
    site_id: 'site_project_otec',
    site_slug: 'ben-otec',
    site_status: 'approved',
    site_title: 'Ben OTEC',
    site_updated_at: '2026-06-05T00:08:30.000Z',
    site_visibility: 'public',
    ...input,
  }
}

function researchRow(input: Partial<ResearchRow> = {}): ResearchRow {
  return {
    approved_at: '2026-06-05T00:05:00.000Z',
    approved_source_count: 4,
    research_brief_id: 'adjutant_research_brief_otec',
    run_id: 'exa_enrichment_run_otec',
    source_count: 6,
    status: 'approved',
    updated_at: '2026-06-05T00:05:00.000Z',
    ...input,
  }
}

function versionRow(input: Partial<VersionRow> = {}): VersionRow {
  return {
    build_status: 'saved',
    created_at: '2026-06-05T00:06:00.000Z',
    id: 'site_version_otec_saved',
    saved_at: '2026-06-05T00:06:30.000Z',
    source_commit_sha: 'abc1234',
    ...input,
  }
}

function compatibilityRow(input: Partial<CompatibilityRow> = {}): CompatibilityRow {
  return {
    blockers_json: '[]',
    customer_safe_next_action: 'Run the build validation before deployment.',
    customer_safe_status: 'The site shape is compatible with Sites.',
    id: 'site_compatibility_check_otec',
    status: 'ready',
    warnings_json: '[{"code":"manual_review"}]',
    ...input,
  }
}

function buildValidationRow(
  input: Partial<BuildValidationRow> = {},
): BuildValidationRow {
  return {
    blockers_json: '[]',
    customer_safe_next_action: 'Review the saved Site version.',
    customer_safe_status: 'The latest build validation passed.',
    id: 'site_build_validation_otec',
    source_hash: 'sha256:otec',
    status: 'passed',
    warnings_json: '[]',
    ...input,
  }
}

const runRoute = (store: PublicOtecProofStore): Promise<Response> =>
  Effect.runPromise(
    handlePublicOtecProofApi(
      new Request('https://openagents.com/api/public/proof/otec'),
      { OPENAGENTS_DB: publicOtecProofDb(store) },
    ),
  )

describe('public OTEC proof API', () => {
  test('returns customer-safe proof closeout for the OTEC order', async () => {
    const store = new PublicOtecProofStore()
    const response = await runRoute(store)
    const body = (await response.json()) as Record<string, unknown>
    const serialized = JSON.stringify(body)

    expect(response.status).toBe(200)
    expect(S.decodeUnknownSync(PublicOtecProofCloseout)(body)).toEqual(body)
    expect(body).toEqual(
      expect.objectContaining({
        slug: 'ben-otec',
        orderId: OTEC_SOFTWARE_ORDER_ID,
        customerSafeStatus: 'OTEC public site is deployed.',
        nextAction:
          'Inspect the deployed site and keep proof receipts current as follow-up work lands.',
      }),
    )
    expect(body.site).toEqual(
      expect.objectContaining({
        slug: 'ben-otec',
        activeUrl: 'https://sites.openagents.com/ben-otec',
      }),
    )
    expect(body.version).toEqual(
      expect.objectContaining({
        activeVersionUrl:
          'https://sites.openagents.com/ben-otec/versions/site_version_otec_saved',
        latestSavedVersionUrl:
          'https://sites.openagents.com/ben-otec/versions/site_version_otec_saved',
        versionRefs: [
          'version:site_version_otec_saved',
          'https://sites.openagents.com/ben-otec/versions/site_version_otec_saved',
        ],
      }),
    )
    expect(body.siteUrlRefs).toEqual([
      'https://sites.openagents.com/ben-otec',
    ])
    expect(body.revisionUrlRefs).toEqual([
      'https://sites.openagents.com/ben-otec/versions/site_version_otec_saved',
    ])
    expect(body.evidenceRefs).toEqual(
      expect.arrayContaining([
        `order:${OTEC_SOFTWARE_ORDER_ID}`,
        'site:ben-otec',
        'research:exa_enrichment_run_otec',
        'version:site_version_otec_saved',
        'deployment:site_deployment_otec_active',
        'https://sites.openagents.com/ben-otec',
        'https://sites.openagents.com/ben-otec/versions/site_version_otec_saved',
        'usage_receipt:adjutant_usage_receipt_public',
      ]),
    )
    expect(body.claimProjections).toEqual(
      expect.arrayContaining([
        expect.objectContaining({
          claimId: 'claim_otec_closeout_overall',
          state: expect.objectContaining({ state: 'verified' }),
        }),
        expect.objectContaining({
          claimId: 'claim_otec_latest_saved_version',
          state: expect.objectContaining({ state: 'verified' }),
        }),
        expect.objectContaining({
          claimId: 'claim_otec_public_receipts',
          state: expect.objectContaining({ state: 'verified' }),
        }),
      ]),
    )
    expect(body.research).toEqual(
      expect.objectContaining({
        approvedSourceCount: 4,
        runId: 'exa_enrichment_run_otec',
      }),
    )
    expect(body.buildValidation).toEqual(
      expect.objectContaining({
        latestValidationId: 'site_build_validation_otec',
        status: 'passed',
      }),
    )
    expect(body.receipts).toEqual(
      expect.objectContaining({
        usageReceiptCount: 1,
        publicReceiptRefs: ['usage_receipt:adjutant_usage_receipt_public'],
        acceptedWorkSettlementRefs: [],
        paymentCaveats: expect.arrayContaining([
          'Buyer-payment and Site-checkout receipts are separate from accepted-work settlement evidence.',
        ]),
      }),
    )
    expect(body.referralCta).toEqual(
      expect.objectContaining({
        title: 'Get your own OpenAgents Site',
        referralJoinUrl:
          'https://openagents.com/r/site/site_ref_otec_ben?target=order',
        agentReferralJoinUrl:
          'https://openagents.com/r/site/site_ref_otec_ben?target=agent&path=agent',
      }),
    )
    expect(body.agentInstructionCard).toEqual(
      expect.objectContaining({
        title: 'Send your agent to this Site',
        preset: 'proof_and_challenge',
        siteSlug: 'ben-otec',
        siteUrl: 'https://sites.openagents.com/ben-otec',
        proofUrl: 'https://openagents.com/api/public/proof/otec',
        instructionDocUrl: 'https://openagents.com/AGENTS.md',
        requiresOwnerClaimForMutation: true,
        referralCta: expect.objectContaining({
          referralJoinUrl:
            'https://openagents.com/r/site/site_ref_otec_ben?target=order',
        }),
      }),
    )
    expect(
      (body.agentInstructionCard as { copyableInstruction: string })
        .copyableInstruction,
    ).toContain('https://openagents.com/.well-known/openagents.json')
    expect(body.agentChallenges).toEqual([
      expect.objectContaining({
        id: 'ben-otec-proof-copy-source-challenge',
        title: 'Improve public proof for Ben OTEC',
        status: 'open',
        challengeUrl:
          'https://openagents.com/api/public/proof/otec#agent-challenges',
        fundingStatus: 'planned_not_live',
        acceptedOutcomeClaim: null,
      }),
    ])
    expect(
      (
        body.agentChallenges as Array<{
          requiredEvidence: ReadonlyArray<string>
        }>
      )[0]?.requiredEvidence,
    ).toContain('Public URL and source title for any proposed source.')
    expect(serialized).not.toContain('exa_request_id')
    expect(serialized).not.toContain('provider_account')
    expect(serialized).not.toContain('auth_grant')
    expect(serialized).not.toContain('callback_token')
    expect(serialized).not.toContain('runner_payload')
  })

  test('returns not found when the public OTEC order is missing', async () => {
    const store = new PublicOtecProofStore()
    store.base = null

    const response = await runRoute(store)

    expect(response.status).toBe(404)
    await expect(response.json()).resolves.toEqual({
      error: 'public_otec_proof_not_found',
    })
  })

  test('honestly reports saved review state when no deployment exists', async () => {
    const store = new PublicOtecProofStore()
    store.base = baseRow({
      active_deployment_id: null,
      active_deployment_status: null,
      active_deployment_updated_at: null,
      active_deployment_url: null,
    })

    const response = await runRoute(store)
    const body = (await response.json()) as {
      customerSafeStatus: string
      nextAction: string
      deployment: { url: string | null }
      claimState: { state: string }
      caveats: ReadonlyArray<string>
      revisionUrlRefs: ReadonlyArray<string>
      agentInstructionCard: {
        siteUrl: string | null
        copyableInstruction: string
      }
    }

    expect(response.status).toBe(200)
    expect(body.customerSafeStatus).toBe('The latest build validation passed.')
    expect(body.nextAction).toBe('Review the saved Site version.')
    expect(body.deployment.url).toBeNull()
    expect(body.claimState.state).toBe('measured')
    expect(body.revisionUrlRefs).toEqual([
      'https://sites.openagents.com/ben-otec/versions/site_version_otec_saved',
    ])
    expect(body.caveats).toContain(
      'No active deployment is claimed until a deployment receipt and URL are present.',
    )
    expect(body.agentInstructionCard.siteUrl).toBeNull()
    expect(body.agentInstructionCard.copyableInstruction).toContain(
      'Do not claim a live deployment exists',
    )
  })

  test('keeps claim states planned before proof receipts', async () => {
    const store = new PublicOtecProofStore()
    store.base = baseRow({
      active_deployment_id: null,
      active_deployment_status: null,
      active_deployment_updated_at: null,
      active_deployment_url: null,
      site_id: null,
      site_slug: null,
      site_title: null,
      site_status: null,
    })
    store.research = null
    store.latestVersion = null
    store.compatibility = null
    store.buildValidation = null
    store.receipts = []

    const response = await runRoute(store)
    const body = (await response.json()) as {
      claimState: { state: string }
      research: { approvedSourceCount: number; claimState: { state: string } }
      site: { claimState: { state: string } }
    }

    expect(response.status).toBe(200)
    expect(body.claimState.state).toBe('planned')
    expect(body.site.claimState.state).toBe('planned')
    expect(body.research.approvedSourceCount).toBe(0)
    expect(body.research.claimState.state).toBe('planned')
  })

  test('fails closed when public projection contains secret-shaped material', async () => {
    const store = new PublicOtecProofStore()
    store.base = baseRow({
      site_title: 'Bearer gho_abcdefghijklmnopqrstuvwxyz',
    })

    const response = await runRoute(store)

    expect(response.status).toBe(500)
    await expect(response.json()).resolves.toEqual({
      error: 'public_otec_proof_unsafe',
    })
  })

  test('fails closed on wallet and raw payment shaped public refs', async () => {
    const walletStore = new PublicOtecProofStore()
    walletStore.buildValidation = buildValidationRow({
      source_hash: 'wallet_state:abc',
    })
    const paymentStore = new PublicOtecProofStore()
    paymentStore.receipts = [
      {
        created_at: '2026-06-05T00:10:00.000Z',
        id: 'payment_preimage_abc',
        software_order_id: OTEC_SOFTWARE_ORDER_ID,
        visibility: 'public',
      },
    ]

    const walletResponse = await runRoute(walletStore)
    const paymentResponse = await runRoute(paymentStore)

    expect(walletResponse.status).toBe(500)
    expect(paymentResponse.status).toBe(500)
    await expect(walletResponse.json()).resolves.toEqual({
      error: 'public_otec_proof_unsafe',
    })
    await expect(paymentResponse.json()).resolves.toEqual({
      error: 'public_otec_proof_unsafe',
    })
  })
})
