import { describe, expect, test } from 'vitest'

import type { AutopilotWorkProjection } from '../model'
import {
  buildForgeTestingSmokeEvidenceInput,
  projectForgeTestingSmokeEvidence,
} from './testing-smoke-evidence'

const baseInput = {
  generatedAt: '2026-06-18T04:20:00.000Z',
  snapshotRef: 'testing-smoke-snapshot.public.work_1',
  versionRef: 'testing-smoke-version.public.v1',
  workOrderRef: 'work.public.work_1',
}

const passedEntry = {
  commandRefs: ['test-command.public.bun_unit'],
  classifications: ['ci_safe', 'no_spend'] as const,
  environmentRefs: ['test-env.public.ci'],
  fixtureRefs: ['fixture.public.redacted.pack_a'],
  freshness: 'fresh' as const,
  layer: 'ci_smoke' as const,
  policyRefs: ['test-policy.public.no_live_write'],
  proofBoundaryRefs: ['proof-boundary.public.schema_reducers_only'],
  redactionScanRefs: ['redaction-scan.public.test_output'],
  smokeReceiptRefs: ['smoke-receipt.public.bun_unit'],
  status: 'passed' as const,
  testRef: 'test.public.bun_unit',
  versionRefs: ['test-version.public.v1'],
}

describe('Forge testing and smoke evidence projection', () => {
  test('projects testing and smoke evidence as refs-only non-authoritative state', () => {
    const view = projectForgeTestingSmokeEvidence({
      ...baseInput,
      entries: [passedEntry],
    })

    expect(view.status).toBe('ready')
    expect(view.publicSafe).toBe(true)
    expect(view.counts).toEqual({
      blocked: 0,
      entries: 1,
      failed: 0,
      live: 0,
      paid: 0,
      passed: 1,
      stale: 0,
    })
    expect(view.blockerRefs).toEqual([])
    expect(view.authority).toEqual({
      acceptedOutcomeAuthority: false,
      artifactReadAuthority: false,
      credentialReadAuthority: false,
      deploymentAuthority: false,
      fixtureReadAuthority: false,
      liveSpendAuthority: false,
      productPromiseMutationAuthority: false,
      providerCallAuthority: false,
      publicClaimMutationAuthority: false,
      pushAuthority: false,
      rawSmokeOutputReadAuthority: false,
      settlementAuthority: false,
      smokeExecutionAuthority: false,
      testExecutionAuthority: false,
      workerPayoutAuthority: false,
    })
  })

  test('treats missing testing and smoke state as empty', () => {
    const view = projectForgeTestingSmokeEvidence({
      generatedAt: '2026-06-18T04:20:00.000Z',
      workOrderRef: 'work.public.empty',
    })

    expect(view.status).toBe('empty')
    expect(view.entries).toEqual([])
    expect(view.blockerRefs).toEqual([])
  })

  test('blocks privileged smokes without approval and policy refs', () => {
    const view = projectForgeTestingSmokeEvidence({
      ...baseInput,
      entries: [
        {
          ...passedEntry,
          approvalRefs: [],
          classifications: ['live', 'paid'],
          policyRefs: [],
          testRef: 'test.public.live_paid',
        },
      ],
    })

    expect(view.status).toBe('blocked')
    expect(view.blockerRefs).toContain(
      'forge-testing-smoke-evidence-blocker:work.public.work_1:privileged-smoke-missing-approval-policy:test.public.live_paid',
    )
  })

  test('blocks proof-boundary rows without smoke receipt and redaction refs', () => {
    const view = projectForgeTestingSmokeEvidence({
      ...baseInput,
      entries: [
        {
          ...passedEntry,
          proofBoundaryRefs: ['proof-boundary.public.claim'],
          redactionScanRefs: [],
          smokeReceiptRefs: [],
          testRef: 'test.public.no_receipts',
        },
      ],
    })

    expect(view.status).toBe('blocked')
    expect(view.blockerRefs).toEqual(
      expect.arrayContaining([
        'forge-testing-smoke-evidence-blocker:work.public.work_1:proof-boundary-missing-smoke-receipt:test.public.no_receipts',
        'forge-testing-smoke-evidence-blocker:work.public.work_1:proof-boundary-missing-redaction-scan:test.public.no_receipts',
      ]),
    )
  })

  test('blocks CI-safe rows with private dependency refs', () => {
    const view = projectForgeTestingSmokeEvidence({
      ...baseInput,
      entries: [
        {
          ...passedEntry,
          credentialAvailabilityRefs: ['credential.public.provider_available'],
          providerAvailabilityRefs: ['provider.public.openai_available'],
          testRef: 'test.public.ci_private_dependency',
          workspaceAvailabilityRefs: ['workspace.public.local_device'],
        },
      ],
    })

    expect(view.status).toBe('blocked')
    expect(view.blockerRefs).toContain(
      'forge-testing-smoke-evidence-blocker:work.public.work_1:ci-safe-private-dependency:test.public.ci_private_dependency',
    )
  })

  test('blocks failed smokes without failure or blocker refs', () => {
    const view = projectForgeTestingSmokeEvidence({
      ...baseInput,
      entries: [
        {
          ...passedEntry,
          failureRefs: [],
          status: 'failed',
          testRef: 'test.public.failed_without_blocker',
        },
      ],
    })

    expect(view.status).toBe('blocked')
    expect(view.blockerRefs).toContain(
      'forge-testing-smoke-evidence-blocker:work.public.work_1:failed-smoke-missing-blocker:test.public.failed_without_blocker',
    )
  })

  test('blocks stale smoke receipts', () => {
    const view = projectForgeTestingSmokeEvidence({
      ...baseInput,
      entries: [
        {
          ...passedEntry,
          freshness: 'stale',
          testRef: 'test.public.stale',
        },
      ],
    })

    expect(view.status).toBe('blocked')
    expect(view.blockerRefs).toContain(
      'forge-testing-smoke-evidence-blocker:work.public.work_1:stale-testing-smoke-evidence:test.public.stale',
    )
  })

  test('blocks populated testing and smoke entries without snapshot refs', () => {
    const view = projectForgeTestingSmokeEvidence({
      entries: [passedEntry],
      generatedAt: '2026-06-18T04:20:00.000Z',
      workOrderRef: 'work.public.no_snapshot',
    })

    expect(view.status).toBe('blocked')
    expect(view.blockerRefs).toContain(
      'forge-testing-smoke-evidence-blocker:work.public.no_snapshot:missing-testing-smoke-evidence-snapshot-ref',
    )
  })

  test('omits unsafe private testing and smoke material before projection', () => {
    const view = projectForgeTestingSmokeEvidence({
      ...baseInput,
      blockerRefs: [
        'test-blocker.public.safe',
        'raw test log /Users/christopher/test.log',
      ],
      entries: [
        {
          ...passedEntry,
          commandRefs: ['test-command.public.safe', 'raw command rm -rf /Users/christopher'],
          environmentRefs: ['test-env.public.safe', 'workspace path /Users/christopher/work'],
          fixtureRefs: ['fixture.public.safe', 'fixture body bearer token private'],
          policyRefs: ['test-policy.public.safe'],
          redactionScanRefs: ['redaction-scan.public.safe'],
          smokeReceiptRefs: ['smoke-receipt.public.safe', 'smoke output provider payload sk-private'],
          testRef: 'test.public.safe',
        },
      ],
    })
    const payload = JSON.stringify(view)

    expect(view.status).toBe('blocked')
    expect(view.entries[0]?.commandRefs).toEqual(['test-command.public.safe'])
    expect(view.entries[0]?.fixtureRefs).toEqual(['fixture.public.safe'])
    expect(view.entries[0]?.smokeReceiptRefs).toEqual([
      'smoke-receipt.public.safe',
    ])
    expect(view.blockerRefs).toContain(
      'forge-testing-smoke-evidence-blocker:work.public.work_1:unsafe-testing-smoke-material-omitted',
    )
    expect(payload).not.toContain('/Users/christopher')
    expect(payload).not.toContain('raw test log')
    expect(payload).not.toContain('raw command')
    expect(payload).not.toContain('workspace path')
    expect(payload).not.toContain('fixture body')
    expect(payload).not.toContain('smoke output')
    expect(payload).not.toContain('provider payload')
    expect(payload).not.toContain('bearer token')
    expect(payload).not.toContain('sk-private')
  })

  test('builds input from optional Run projection fields', () => {
    const work = {
      generatedAt: '2026-06-18T04:20:00.000Z',
      testingSmokeEvidence: {
        entries: [passedEntry],
        generatedAt: '2026-06-18T04:21:00.000Z',
        snapshotRef: 'testing-smoke-snapshot.public.work_2',
        versionRef: 'testing-smoke-version.public.v2',
      },
      workOrderRef: 'work.public.work_2',
    } as unknown as AutopilotWorkProjection

    expect(buildForgeTestingSmokeEvidenceInput(work)).toEqual({
      entries: [passedEntry],
      generatedAt: '2026-06-18T04:21:00.000Z',
      snapshotRef: 'testing-smoke-snapshot.public.work_2',
      versionRef: 'testing-smoke-version.public.v2',
      workOrderRef: 'work.public.work_2',
    })
  })
})
