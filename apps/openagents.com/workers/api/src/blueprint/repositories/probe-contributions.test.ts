import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import { BLUEPRINT_DEVELOPER_PACKAGE_CONTRIBUTION_NO_AUTHORITY } from '../services/developer-package-contribution'
import { BLUEPRINT_SIGNATURE_CONTRIBUTION_NO_AUTHORITY } from '../services/signature-contribution'
import {
  BLUEPRINT_STUDYBENCH_CONTRIBUTION_CAPABILITY_FAMILY,
  BlueprintProbeContributionValidationError,
  isBlueprintStudybenchProbeContributionKind,
  listBlueprintProbeContributions,
  recordBlueprintProbeContribution,
} from './probe-contributions'

type ProbeContributionKind =
  | 'developer_package_contribution'
  | 'repo_study_packet.v0'
  | 'signature_contribution'
  | 'studybench.evidence_span_extraction.v0'
  | 'studybench.rubric_authoring.v0'
  | 'studybench.rubric_judging.v0'
  | 'studybench.task_authoring.v0'

type ProbeContributionRow = Readonly<{
  archived_at: string | null
  blocker_refs_json: string
  candidate_runtime_allowed: number
  contribution_kind: ProbeContributionKind
  created_at: string
  developer_package_contribution_json: string | null
  fixture_refs_json: string
  id: string
  idempotency_key: string
  metadata_json: string
  production_runtime_allowed: number
  projection_json: string
  release_gate_ready: number
  release_gate_refs_json: string
  retained_failure_refs_json: string
  review_status: string
  signature_contribution_json: string | null
  status: string
  target_refs_json: string
  updated_at: string
}>

class ProbeContributionStore {
  rows: Array<ProbeContributionRow> = []
}

class ProbeContributionStatement implements D1PreparedStatement {
  private values: ReadonlyArray<unknown> = []

  constructor(
    private readonly query: string,
    private readonly store: ProbeContributionStore,
  ) {}

  bind(...values: ReadonlyArray<unknown>): D1PreparedStatement {
    this.values = values

    return this
  }

  first<T = unknown>(colName: string): Promise<T | null>
  first<T = Record<string, unknown>>(): Promise<T | null>
  first<T = unknown>(): Promise<T | null> {
    if (this.query.includes('WHERE idempotency_key = ?')) {
      const idempotencyKey = String(this.values[0])
      const row =
        this.store.rows.find(
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
      this.query.includes('INSERT OR IGNORE INTO blueprint_probe_contributions')
    ) {
      const idempotencyKey = String(this.values[1])

      if (
        this.store.rows.every(item => item.idempotency_key !== idempotencyKey)
      ) {
        this.store.rows.push({
          archived_at: null,
          blocker_refs_json: String(this.values[8]),
          candidate_runtime_allowed: Number(this.values[6]),
          contribution_kind: this.values[2] as ProbeContributionKind,
          created_at: String(this.values[17]),
          developer_package_contribution_json:
            this.values[14] === null ? null : String(this.values[14]),
          fixture_refs_json: String(this.values[10]),
          id: String(this.values[0]),
          idempotency_key: idempotencyKey,
          metadata_json: String(this.values[16]),
          production_runtime_allowed: Number(this.values[7]),
          projection_json: String(this.values[15]),
          release_gate_ready: Number(this.values[5]),
          release_gate_refs_json: String(this.values[9]),
          retained_failure_refs_json: String(this.values[11]),
          review_status: String(this.values[4]),
          signature_contribution_json:
            this.values[13] === null ? null : String(this.values[13]),
          status: String(this.values[3]),
          target_refs_json: String(this.values[12]),
          updated_at: String(this.values[18]),
        })
      }

      return Promise.resolve({ success: true } as D1Result<T>)
    }

    return Promise.reject(new Error(`Unexpected run: ${this.query}`))
  }

  all<T = Record<string, unknown>>(): Promise<D1Result<T>> {
    if (this.query.includes('FROM blueprint_probe_contributions')) {
      return Promise.resolve({
        results: this.store.rows.filter(item => item.archived_at === null),
      } as unknown as D1Result<T>)
    }

    return Promise.reject(new Error(`Unexpected all: ${this.query}`))
  }

  raw<T = unknown[]>(options: {
    columnNames: true
  }): Promise<[Array<string>, ...Array<T>]>
  raw<T = unknown[]>(options?: { columnNames?: false }): Promise<Array<T>>
  raw<T = unknown[]>(options?: {
    columnNames?: boolean
  }): Promise<Array<T> | [Array<string>, ...Array<T>]> {
    return options?.columnNames === true
      ? Promise.resolve([[]])
      : Promise.resolve([])
  }
}

const db = (store: ProbeContributionStore): D1Database => ({
  batch: () => Promise.reject(new Error('D1 batch should not be used')),
  dump: () => Promise.reject(new Error('D1 dump should not be used')),
  exec: () => Promise.reject(new Error('D1 exec should not be used')),
  prepare: query => new ProbeContributionStatement(query, store),
  withSession: () => {
    throw new Error('D1 session should not be used')
  },
})

const runtime = {
  makeContributionId: () => 'blueprint_probe_contribution_generated',
  nowIso: () => '2026-06-07T21:00:00.000Z',
}

const signatureContribution = () => ({
  authority: BLUEPRINT_SIGNATURE_CONTRIBUTION_NO_AUTHORITY,
  capabilitySummaryRef: 'capability.probe.signature.summary',
  contributorRefs: ['contributor.openagents.probe'],
  createdAt: '2026-06-07T21:00:00.000Z',
  id: 'probe_blueprint_contribution.signature.v1',
  intendedFamily: 'action_planning' as const,
  promotionRef: null,
  proposedModuleVersionRef: 'module_version.probe.tool_menu.seed.v1',
  proposedProgramSignatureRef: 'program_signature.probe.tool_menu.project.v1',
  proposedProgramTypeRef: 'program_type.probe.tool_menu.project',
  rejectionRef: null,
  releaseGateRefs: ['release_gate.probe.tool_menu.seed.v1'],
  requiredFixtureRefs: ['fixture.probe.tool_menu.decode.v1'],
  reviewStatus: 'approved' as const,
  riskClass: 'medium' as const,
  sourceRefs: ['source_ref.probe.contribution.audit.v1'],
  status: 'approved_for_release_gate' as const,
  updatedAt: '2026-06-07T21:00:00.000Z',
})

const developerContribution = () => ({
  authority: BLUEPRINT_DEVELOPER_PACKAGE_CONTRIBUTION_NO_AUTHORITY,
  backendProjectionAdapterRefs: ['adapter.probe.apple_fm.tools.v1'],
  capabilityFamily: 'tool_package' as const,
  capabilitySummaryRef: 'capability.probe.tool_package.summary',
  contextPackageRefs: ['context_package.probe.repo.readonly.v1'],
  contributorRefs: ['contributor.openagents.probe'],
  createdAt: '2026-06-07T21:00:00.000Z',
  dogfoodScopeRef: 'dogfood.probe.assignment_only.v1',
  id: 'probe_blueprint_contribution.package.v1',
  intendedProgramFamily: 'action_planning' as const,
  noProductionRuntimeAuthority: true,
  outcomeTemplateRefs: [],
  paymentAttributionRefs: ['payment_attribution.probe.package.v1'],
  promotionRef: 'promotion.probe.tool_package.v1',
  proposedModuleVersionRefs: [],
  proposedProgramSignatureRefs: [],
  proposedProgramTypeRefs: [],
  rejectionRef: null,
  releaseGateRefs: ['release_gate.probe.tool_package.seed.v1'],
  requiredFixtureRefs: ['fixture.probe.tool_package.decode.v1'],
  retainedFailureRefs: ['failure.probe.tool_package.fixture.v1'],
  reviewStatus: 'approved' as const,
  riskClass: 'medium' as const,
  selfPromotionAttempt: false,
  sourceRefs: ['source_ref.probe.package.audit.v1'],
  status: 'promoted' as const,
  toolPackageRefs: ['tool_package.probe.repo_read_tools.v1'],
  uiBindingRefs: [],
  updatedAt: '2026-06-07T21:00:00.000Z',
})

const studybenchContribution = (
  contributionKind: Exclude<
    ProbeContributionKind,
    'developer_package_contribution' | 'signature_contribution'
  > = 'studybench.task_authoring.v0',
) => ({
  ...developerContribution(),
  capabilityFamily:
    BLUEPRINT_STUDYBENCH_CONTRIBUTION_CAPABILITY_FAMILY[contributionKind],
  capabilitySummaryRef: `capability.openagents_studybench.${contributionKind}`,
  contextPackageRefs:
    BLUEPRINT_STUDYBENCH_CONTRIBUTION_CAPABILITY_FAMILY[contributionKind] ===
    'context_package'
      ? [`context_package.openagents_studybench.${contributionKind}`]
      : [],
  dogfoodScopeRef: 'dogfood.openagents_studybench.authoring.v0',
  id: `probe_blueprint_contribution.${contributionKind}`,
  intendedProgramFamily: 'research_policy' as const,
  outcomeTemplateRefs:
    BLUEPRINT_STUDYBENCH_CONTRIBUTION_CAPABILITY_FAMILY[contributionKind] ===
    'outcome_template'
      ? [`outcome_template.openagents_studybench.${contributionKind}`]
      : [],
  paymentAttributionRefs: [],
  promotionRef: null,
  releaseGateRefs: [`release_gate.openagents_studybench.${contributionKind}`],
  requiredFixtureRefs: [`fixture.openagents_studybench.${contributionKind}`],
  retainedFailureRefs: [
    `failure.openagents_studybench.${contributionKind}.retained.v0`,
  ],
  sourceRefs: ['source_ref.openagents_studybench.public_retained.v0'],
  status: 'approved_for_release_gate' as const,
  toolPackageRefs:
    BLUEPRINT_STUDYBENCH_CONTRIBUTION_CAPABILITY_FAMILY[contributionKind] ===
    'retrieval_package'
      ? [`tool_package.openagents_studybench.${contributionKind}`]
      : [],
})

describe('Blueprint Probe contribution repository', () => {
  test('records idempotent signature contributions as release-gate ready candidate refs', async () => {
    const store = new ProbeContributionStore()
    const contribution = await Effect.runPromise(
      recordBlueprintProbeContribution(
        db(store),
        {
          candidateRuntimeAllowed: true,
          contributionKind: 'signature_contribution',
          dogfoodScopeRef: 'dogfood.probe.assignment_only.v1',
          id: 'probe_blueprint_contribution.signature.v1',
          idempotencyKey:
            'probe_blueprint_contribution:probe_blueprint_contribution.signature.v1',
          productionRuntimeAllowed: false,
          retainedFailureRefs: ['failure.probe.tool_menu.fixture.v1'],
          signatureContribution: signatureContribution(),
        },
        runtime,
      ),
    )
    const replay = await Effect.runPromise(
      recordBlueprintProbeContribution(
        db(store),
        {
          candidateRuntimeAllowed: false,
          contributionKind: 'signature_contribution',
          id: 'probe_blueprint_contribution.signature.v1',
          idempotencyKey:
            'probe_blueprint_contribution:probe_blueprint_contribution.signature.v1',
          productionRuntimeAllowed: false,
          retainedFailureRefs: ['failure.changed'],
          signatureContribution: signatureContribution(),
        },
        runtime,
      ),
    )

    expect(contribution).toStrictEqual(replay)
    expect(contribution).toMatchObject({
      blockerRefs: [],
      candidateRuntimeAllowed: true,
      productionRuntimeAllowed: false,
      releaseGateReady: true,
      targetRefs: [
        'program_type.probe.tool_menu.project',
        'program_signature.probe.tool_menu.project.v1',
        'module_version.probe.tool_menu.seed.v1',
      ],
    })
  })

  test('records promoted developer package contributions as production eligible only after release gates', async () => {
    const store = new ProbeContributionStore()
    const contribution = await Effect.runPromise(
      recordBlueprintProbeContribution(
        db(store),
        {
          candidateRuntimeAllowed: false,
          contributionKind: 'developer_package_contribution',
          developerPackageContribution: developerContribution(),
          id: 'probe_blueprint_contribution.package.v1',
          idempotencyKey:
            'probe_blueprint_contribution:probe_blueprint_contribution.package.v1',
          productionRuntimeAllowed: true,
          retainedFailureRefs: ['failure.probe.tool_package.fixture.v1'],
        },
        runtime,
      ),
    )
    const listed = await Effect.runPromise(
      listBlueprintProbeContributions(db(store), 10),
    )

    expect(contribution).toMatchObject({
      blockerRefs: [],
      candidateRuntimeAllowed: false,
      productionRuntimeAllowed: true,
      releaseGateReady: true,
    })
    expect(contribution.targetRefs).toEqual([
      'adapter.probe.apple_fm.tools.v1',
      'context_package.probe.repo.readonly.v1',
      'tool_package.probe.repo_read_tools.v1',
    ])
    expect(listed).toEqual([contribution])
  })

  test('rejects production runtime eligibility without release-gate readiness', async () => {
    const store = new ProbeContributionStore()

    await expect(
      Effect.runPromise(
        recordBlueprintProbeContribution(
          db(store),
          {
            candidateRuntimeAllowed: false,
            contributionKind: 'signature_contribution',
            idempotencyKey: 'probe_blueprint_contribution:blocked',
            productionRuntimeAllowed: true,
            retainedFailureRefs: ['failure.probe.tool_menu.fixture.v1'],
            signatureContribution: {
              ...signatureContribution(),
              status: 'submitted',
            },
          },
          runtime,
        ),
      ),
    ).rejects.toBeInstanceOf(BlueprintProbeContributionValidationError)
  })

  test('records StudyBench contribution kinds as evidence-only release-gate records', async () => {
    const store = new ProbeContributionStore()
    const contributionKind = 'studybench.rubric_judging.v0'
    const contribution = await Effect.runPromise(
      recordBlueprintProbeContribution(
        db(store),
        {
          candidateRuntimeAllowed: false,
          contributionKind,
          developerPackageContribution: studybenchContribution(contributionKind),
          id: 'probe_blueprint_contribution.studybench.rubric_judging.v0',
          idempotencyKey:
            'probe_blueprint_contribution:studybench:rubric_judging:v0',
          productionRuntimeAllowed: false,
          retainedFailureRefs: [
            'failure.openagents_studybench.rubric_judging.retained.v0',
          ],
        },
        runtime,
      ),
    )

    expect(isBlueprintStudybenchProbeContributionKind(contributionKind)).toBe(
      true,
    )
    expect(contribution).toMatchObject({
      blockerRefs: [],
      candidateRuntimeAllowed: false,
      contributionKind,
      productionRuntimeAllowed: false,
      releaseGateReady: true,
      retainedFailureRefs: [
        'failure.openagents_studybench.rubric_judging.retained.v0',
      ],
      reviewStatus: 'approved',
      status: 'approved_for_release_gate',
    })
    expect(contribution.projection).toMatchObject({
      capabilityFamily: 'outcome_template',
      nonAuthoritative: true,
      noProductionRuntimeAuthority: true,
      paymentAttributionRefs: [],
      releaseGateReady: true,
    })
  })

  test('rejects StudyBench release-gate readiness without retained failures', async () => {
    const store = new ProbeContributionStore()

    await expect(
      Effect.runPromise(
        recordBlueprintProbeContribution(
          db(store),
          {
            candidateRuntimeAllowed: false,
            contributionKind: 'studybench.task_authoring.v0',
            developerPackageContribution: studybenchContribution(
              'studybench.task_authoring.v0',
            ),
            idempotencyKey: 'probe_blueprint_contribution:studybench:blocked',
            productionRuntimeAllowed: false,
            retainedFailureRefs: [],
          },
          runtime,
        ),
      ),
    ).rejects.toBeInstanceOf(BlueprintProbeContributionValidationError)
  })

  test('rejects StudyBench contribution runtime authority and unsafe refs', async () => {
    const store = new ProbeContributionStore()

    await expect(
      Effect.runPromise(
        recordBlueprintProbeContribution(
          db(store),
          {
            candidateRuntimeAllowed: false,
            contributionKind: 'studybench.task_authoring.v0',
            developerPackageContribution: {
              ...studybenchContribution('studybench.task_authoring.v0'),
              authority: {
                ...BLUEPRINT_DEVELOPER_PACKAGE_CONTRIBUTION_NO_AUTHORITY,
                canDispatchRuntime: true,
              },
            },
            idempotencyKey:
              'probe_blueprint_contribution:studybench:runtime_authority',
            productionRuntimeAllowed: false,
            retainedFailureRefs: [
              'failure.openagents_studybench.task_authoring.retained.v0',
            ],
          },
          runtime,
        ),
      ),
    ).rejects.toBeInstanceOf(BlueprintProbeContributionValidationError)

    await expect(
      Effect.runPromise(
        recordBlueprintProbeContribution(
          db(store),
          {
            candidateRuntimeAllowed: false,
            contributionKind: 'studybench.task_authoring.v0',
            developerPackageContribution: studybenchContribution(
              'studybench.task_authoring.v0',
            ),
            idempotencyKey:
              'probe_blueprint_contribution:studybench:raw_source',
            productionRuntimeAllowed: false,
            retainedFailureRefs: ['raw_source_archive.openagents.private'],
          },
          runtime,
        ),
      ),
    ).rejects.toBeInstanceOf(BlueprintProbeContributionValidationError)

    await expect(
      Effect.runPromise(
        recordBlueprintProbeContribution(
          db(store),
          {
            candidateRuntimeAllowed: false,
            contributionKind: 'studybench.task_authoring.v0',
            developerPackageContribution: {
              ...studybenchContribution('studybench.task_authoring.v0'),
              paymentAttributionRefs: ['payment_attribution.not_allowed'],
            },
            idempotencyKey:
              'probe_blueprint_contribution:studybench:payment_attribution',
            productionRuntimeAllowed: false,
            retainedFailureRefs: [
              'failure.openagents_studybench.task_authoring.retained.v0',
            ],
          },
          runtime,
        ),
      ),
    ).rejects.toBeInstanceOf(BlueprintProbeContributionValidationError)
  })
})
