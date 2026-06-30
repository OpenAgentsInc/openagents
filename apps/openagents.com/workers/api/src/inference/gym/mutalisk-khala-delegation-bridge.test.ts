import { readFileSync } from 'node:fs'
import { dirname, join } from 'node:path'
import { fileURLToPath } from 'node:url'
import { describe, expect, test } from 'vitest'

import {
  createInMemoryMutaliskKhalaDelegationGymStore,
  GymRunProgressSchemaVersion,
  KhalaCodeDelegationGepaEnvironmentId,
  MutaliskKhalaDelegationBridgeOutputSchemaVersion,
  MutaliskKhalaDelegationDemandKind,
  MutaliskKhalaDelegationDemandSource,
  MutaliskKhalaDelegationGymBridgeUnsafe,
  MutaliskKhalaDelegationJobSchemaVersion,
  MutaliskKhalaDelegationSummarySchemaVersion,
  runMutaliskKhalaDelegationNoUiBridge,
} from './mutalisk-khala-delegation-bridge'

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

describe('Mutalisk Khala delegation Gym bridge (#7754)', () => {
  test('ingests the Mutalisk manifest summary and projects an admission-ready Gym result', () => {
    const store = createInMemoryMutaliskKhalaDelegationGymStore()
    const output = runMutaliskKhalaDelegationNoUiBridge(mutaliskManifest(), {
      observedAt: '2026-06-30T20:00:00.000Z',
      store,
    })

    expect(output.schemaVersion).toBe(
      MutaliskKhalaDelegationBridgeOutputSchemaVersion,
    )
    expect(output.job.schemaVersion).toBe(MutaliskKhalaDelegationJobSchemaVersion)
    expect(output.job.environmentId).toBe(KhalaCodeDelegationGepaEnvironmentId)
    expect(output.job.demandKind).toBe(MutaliskKhalaDelegationDemandKind)
    expect(output.job.demandSource).toBe(MutaliskKhalaDelegationDemandSource)
    expect(output.summary.schemaVersion).toBe(
      MutaliskKhalaDelegationSummarySchemaVersion,
    )
    expect(output.summary.metricValueBps).toBe(10000)
    expect(output.summary.evalEvidenceRefs).toEqual([
      'eval_result.eval.mutalisk.fixtures.khala_fleet_delegation_demo.load_gate',
    ])
    expect(output.summary.traceProvenanceRefs).toEqual([
      'trace.public.trace.mutalisk.fixtures.khala_fleet_delegation_demo.load_gate',
    ])
    expect(output.admissionDecision).toBe('gated_proposal_ready')
    expect(output.actionSubmissionProposalRef).toBe(
      'action_submission.khala_fleet_delegation.candidate.khala.fleet.delegation.c9e0b82e20ef23d7',
    )
    expect(output.decisionGrade).toBe(false)
    expect(output.blockerRefs).toEqual([])
    expect(output.progress.map(progress => progress.stage)).toEqual([
      'queued',
      'running',
      'summary_ingested',
      'admission_projected',
      'completed',
    ])
    expect(output.progress.every(progress => progress.decisionGrade === false)).toBe(
      true,
    )
    expect(output.progress.every(progress => progress.schemaVersion)).toBe(
      true,
    )
    expect(
      output.progress.every(
        progress => progress.schemaVersion === GymRunProgressSchemaVersion,
      ),
    ).toBe(true)
    expect(store.snapshot().jobs).toHaveLength(1)
    expect(store.snapshot().summaries).toHaveLength(1)
    expect(store.snapshot().progress).toHaveLength(5)
  })

  test('blocks instead of projecting unsafe refs or private-path traces', () => {
    expect(() =>
      runMutaliskKhalaDelegationNoUiBridge(
        mutaliskManifest({
          traceProvenanceRefs: ['/Users/operator/raw_trace.json'],
        }),
        { observedAt: '2026-06-30T20:00:00.000Z' },
      ),
    ).toThrow(MutaliskKhalaDelegationGymBridgeUnsafe)
  })

  test('drops unknown raw optimizer scratch fields from the public projection', () => {
    const output = runMutaliskKhalaDelegationNoUiBridge(
      mutaliskManifest({
        rawPrompt: 'raw_prompt: secret local scratch text',
        rawTrace: '/Users/operator/private/raw_trace.json',
      }),
      { observedAt: '2026-06-30T20:00:00.000Z' },
    )

    const serialized = JSON.stringify(output)
    expect(serialized).not.toContain('raw_prompt')
    expect(serialized).not.toContain('/Users/operator')
    expect(serialized).not.toContain('secret local scratch text')
  })

  test('keeps the bridge out of Mutalisk Python DSPy GEPA runtime code', () => {
    const root = join(
      dirname(fileURLToPath(import.meta.url)),
      '../../../../../../..',
    )
    const bridgeSource = readFileSync(
      join(
        root,
        'apps/openagents.com/workers/api/src/inference/gym/mutalisk-khala-delegation-bridge.ts',
      ),
      'utf8',
    )
    const scriptSource = readFileSync(
      join(
        root,
        'clients/khala-code-desktop/scripts/part2-gepa-manifest-bridge.ts',
      ),
      'utf8',
    )
    const combined = `${bridgeSource}\n${scriptSource}`.toLowerCase()

    expect(combined).not.toContain('child_process')
    expect(combined).not.toContain('spawn(')
    expect(combined).not.toContain('python')
    expect(combined).not.toMatch(/from\s+['"][^'"]*dspy/)
    expect(combined).not.toContain('import dspy')
    expect(combined).not.toContain('gepa.optimize')
    expect(combined).not.toContain('mutalisk-optimize')
  })
})
