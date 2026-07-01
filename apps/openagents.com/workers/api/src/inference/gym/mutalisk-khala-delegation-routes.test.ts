import { readFileSync } from 'node:fs'
import { dirname, join } from 'node:path'
import { fileURLToPath } from 'node:url'
import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  handleOperatorMutaliskKhalaDelegationProgressApi,
  handleOperatorMutaliskKhalaDelegationRunsApi,
  handleOperatorMutaliskKhalaDelegationSummaryApi,
  handlePublicMutaliskKhalaDelegationRunsApi,
} from './mutalisk-khala-delegation-routes'
import { createInMemoryMutaliskKhalaDelegationWorkflowStore } from './mutalisk-khala-delegation-store'

const run = <A>(effect: Effect.Effect<A>): Promise<A> => Effect.runPromise(effect)

const mutaliskManifest = (overrides: Record<string, unknown> = {}) => ({
  baseModuleRef: 'module.mutalisk.khala_fleet_delegate_seed.0.0.1',
  candidateManifestRef:
    'candidate_manifest.khala.fleet.delegation.c9e0b82e20ef23d7',
  candidateRef: 'candidate.khala.fleet.delegation.c9e0b82e20ef23d7',
  evalEvidenceRefs: [
    'eval_result.eval.mutalisk.fixtures.khala_fleet_delegation_demo.load_gate',
  ],
  metricName: 'khala.fleet.delegation',
  metricValueBps: 10000,
  optimizedModuleRef:
    'module.khala.fleet.delegation.optimized.c9e0b82e20ef23d7',
  schemaVersion: 'psionic.probe_gepa_candidate_manifest.v1',
  signature: 'khala.fleet.delegation',
  traceProvenanceRefs: [
    'trace.public.trace.mutalisk.fixtures.khala_fleet_delegation_demo.load_gate',
  ],
  ...overrides,
})

const operatorRequest = (path: string, body: unknown) =>
  new Request(`https://openagents.com${path}`, {
    body: JSON.stringify(body),
    method: 'POST',
  })

describe('Mutalisk Khala delegation workflow routes (#7799)', () => {
  test('creates a durable run and returns a runRef immediately', async () => {
    const store = createInMemoryMutaliskKhalaDelegationWorkflowStore()
    const response = await run(
      handleOperatorMutaliskKhalaDelegationRunsApi(
        operatorRequest('/api/operator/gym/mutalisk-khala-delegation/runs', {
          refSeed: 'route.create.test',
        }),
        {
          nowIso: () => '2026-07-01T10:00:00.000Z',
          requireAdminApiToken: () => Promise.resolve(true),
          store,
        },
      ),
    )
    const body = (await response.json()) as {
      jobRef: string
      run: { decisionGrade: boolean; latestStage: string; progress: ReadonlyArray<unknown> }
      runRef: string
    }

    expect(response.status).toBe(201)
    expect(body.runRef).toBe(
      'gym.run.khala_code_delegation_gepa.route.create.test',
    )
    expect(body.jobRef).toBe(
      'gym.job.mutalisk_khala_delegation.route.create.test',
    )
    expect(body.run.latestStage).toBe('queued')
    expect(body.run.decisionGrade).toBe(false)
    expect(body.run.progress).toHaveLength(1)
  })

  test('requires operator auth for create/list/progress/summary writes', async () => {
    const store = createInMemoryMutaliskKhalaDelegationWorkflowStore()
    const response = await run(
      handleOperatorMutaliskKhalaDelegationRunsApi(
        operatorRequest('/api/operator/gym/mutalisk-khala-delegation/runs', {}),
        {
          requireAdminApiToken: () => Promise.resolve(false),
          store,
        },
      ),
    )

    expect(response.status).toBe(401)
    expect(store.snapshot()).toEqual([])
  })

  test('accepts runner progress, ingests summary, and exposes a compact public projection', async () => {
    const store = createInMemoryMutaliskKhalaDelegationWorkflowStore()
    const routeInput = {
      nowIso: () => '2026-07-01T10:00:00.000Z',
      requireAdminApiToken: () => Promise.resolve(true),
      store,
    }

    const create = await run(
      handleOperatorMutaliskKhalaDelegationRunsApi(
        operatorRequest('/api/operator/gym/mutalisk-khala-delegation/runs', {
          refSeed: 'route.summary.test',
        }),
        routeInput,
      ),
    )
    const createBody = (await create.json()) as { runRef: string }

    const progress = await run(
      handleOperatorMutaliskKhalaDelegationProgressApi(
        operatorRequest('/api/operator/gym/mutalisk-khala-delegation/progress', {
          runRef: createBody.runRef,
          stage: 'optimizing',
          updatedAt: '2026-07-01T10:02:00.000Z',
        }),
        routeInput,
      ),
    )
    expect(progress.status).toBe(201)

    const summary = await run(
      handleOperatorMutaliskKhalaDelegationSummaryApi(
        operatorRequest('/api/operator/gym/mutalisk-khala-delegation/summary', {
          manifestSummary: mutaliskManifest(),
          observedAt: '2026-07-01T10:05:00.000Z',
          runRef: createBody.runRef,
        }),
        routeInput,
      ),
    )
    const summaryBody = (await summary.json()) as {
      actionSubmissionProposalRef: string | null
      admissionDecision: string
      candidateManifestRef: string
      candidateRef: string
      decisionGrade: boolean
      metricValueBps: number
      run: { latestStage: string; progress: ReadonlyArray<{ stage: string }> }
      runRef: string
    }

    expect(summary.status).toBe(201)
    expect(summaryBody.runRef).toBe(createBody.runRef)
    expect(summaryBody.candidateManifestRef).toBe(
      'candidate_manifest.khala.fleet.delegation.c9e0b82e20ef23d7',
    )
    expect(summaryBody.candidateRef).toBe(
      'candidate.khala.fleet.delegation.c9e0b82e20ef23d7',
    )
    expect(summaryBody.metricValueBps).toBe(10000)
    expect(summaryBody.admissionDecision).toBe('gated_proposal_ready')
    expect(summaryBody.actionSubmissionProposalRef).toBe(
      'action_submission.khala_fleet_delegation.candidate.khala.fleet.delegation.c9e0b82e20ef23d7',
    )
    expect(summaryBody.decisionGrade).toBe(false)
    expect(summaryBody.run.latestStage).toBe('completed')
    expect(summaryBody.run.progress.map(progress => progress.stage)).toEqual([
      'queued',
      'dataset_resolved',
      'feedback_resolved',
      'optimizing',
      'candidate_emitted',
      'summary_ingested',
      'admission_projected',
      'completed',
    ])

    const publicResponse = await run(
      handlePublicMutaliskKhalaDelegationRunsApi(
        new Request(
          'https://openagents.com/api/public/gym/mutalisk-khala-delegation/runs',
        ),
        { store },
      ),
    )
    const publicBody = (await publicResponse.json()) as {
      runs: ReadonlyArray<{
        actionSubmissionProposalRef: string
        candidateManifestRef: string
        decisionGrade: boolean
        latestStage: string
        metricValueBps: number
      }>
    }
    expect(publicResponse.status).toBe(200)
    expect(publicBody.runs).toHaveLength(1)
    expect(publicBody.runs[0]?.latestStage).toBe('completed')
    expect(publicBody.runs[0]?.decisionGrade).toBe(false)
    expect(publicBody.runs[0]?.candidateManifestRef).toBe(
      'candidate_manifest.khala.fleet.delegation.c9e0b82e20ef23d7',
    )
    expect(publicBody.runs[0]?.metricValueBps).toBe(10000)
    expect(publicBody.runs[0]?.actionSubmissionProposalRef).toBe(
      'action_submission.khala_fleet_delegation.candidate.khala.fleet.delegation.c9e0b82e20ef23d7',
    )
  })

  test.each([
    ['raw prompt', { rawPrompt: 'prompt: hidden operator instructions' }],
    ['raw trace path', { traceProvenanceRefs: ['/Users/operator/raw_trace.json'] }],
    ['secret', { secretRef: 'sk-live-not-a-real-key' }],
    ['private endpoint', { endpointRef: 'https://hydralisk.internal/v1/chat' }],
    ['provider payload', { providerPayload: 'provider secret token' }],
    ['optimizer scratch log', { optimizerScratchLog: 'raw_log: local scratch' }],
  ])('rejects %s leakage at summary ingest', async (_label, leak) => {
    const store = createInMemoryMutaliskKhalaDelegationWorkflowStore()
    const routeInput = {
      nowIso: () => '2026-07-01T10:00:00.000Z',
      requireAdminApiToken: () => Promise.resolve(true),
      store,
    }
    const create = await run(
      handleOperatorMutaliskKhalaDelegationRunsApi(
        operatorRequest('/api/operator/gym/mutalisk-khala-delegation/runs', {
          refSeed: 'route.reject.test',
        }),
        routeInput,
      ),
    )
    const createBody = (await create.json()) as { runRef: string }
    const response = await run(
      handleOperatorMutaliskKhalaDelegationSummaryApi(
        operatorRequest('/api/operator/gym/mutalisk-khala-delegation/summary', {
          manifestSummary: mutaliskManifest(leak),
          runRef: createBody.runRef,
        }),
        routeInput,
      ),
    )

    expect(response.status).toBe(400)
    const body = (await response.json()) as { reason: string }
    expect(body.reason.toLowerCase()).toMatch(/private|raw|unsafe/)
    expect(store.snapshot()[0]?.latestStage).toBe('queued')
  })

  test('rejects unsafe progress refs before storage', async () => {
    const store = createInMemoryMutaliskKhalaDelegationWorkflowStore()
    const routeInput = {
      requireAdminApiToken: () => Promise.resolve(true),
      store,
    }
    const create = await run(
      handleOperatorMutaliskKhalaDelegationRunsApi(
        operatorRequest('/api/operator/gym/mutalisk-khala-delegation/runs', {
          refSeed: 'route.progress.reject.test',
        }),
        routeInput,
      ),
    )
    const createBody = (await create.json()) as { runRef: string }
    const response = await run(
      handleOperatorMutaliskKhalaDelegationProgressApi(
        operatorRequest('/api/operator/gym/mutalisk-khala-delegation/progress', {
          blockerRefs: ['provider_secret=hidden'],
          runRef: createBody.runRef,
          stage: 'failed',
        }),
        routeInput,
      ),
    )

    expect(response.status).toBe(400)
    expect(store.snapshot()[0]?.latestStage).toBe('queued')
  })

  test('Worker seam does not import Mutalisk runtime execution code', () => {
    const root = join(
      dirname(fileURLToPath(import.meta.url)),
      '../../../../../../..',
    )
    const files = [
      'apps/openagents.com/workers/api/src/inference/gym/mutalisk-khala-delegation-bridge.ts',
      'apps/openagents.com/workers/api/src/inference/gym/mutalisk-khala-delegation-routes.ts',
      'apps/openagents.com/workers/api/src/inference/gym/mutalisk-khala-delegation-store.ts',
    ]
    const combined = files
      .map(file => readFileSync(join(root, file), 'utf8'))
      .join('\n')
      .toLowerCase()

    expect(combined).not.toContain('child_process')
    expect(combined).not.toContain('spawn(')
    expect(combined).not.toMatch(/from\s+['"][^'"]*dspy/)
    expect(combined).not.toContain('import dspy')
    expect(combined).not.toContain('python-shell')
    expect(combined).not.toContain('gepa.optimize')
    expect(combined).not.toMatch(/\.py['"]/)
  })
})
