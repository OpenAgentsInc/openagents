import { describe, expect, test } from 'vitest'

import type { AutopilotWorkProjection } from '../model'
import {
  buildForgePerformanceDiagnosticsInput,
  projectForgePerformanceDiagnostics,
} from './performance-diagnostics'

const baseInput = {
  generatedAt: '2026-06-18T03:20:00.000Z',
  profileRefs: ['performance-profile.public.redacted'],
  snapshotRef: 'performance-diagnostics-snapshot.public.work_1',
  versionRef: 'performance-diagnostics-version.public.v1',
  workOrderRef: 'work.public.work_1',
}

const readyEntry = {
  backpressureRefs: ['backpressure.public.queue.normal'],
  counterRefs: ['counter.public.model.first_token_ms'],
  freshness: 'fresh' as const,
  latencyClass: 'normal' as const,
  outputVolumeRefs: ['output-volume.public.bounded'],
  policyRefs: ['policy.public.performance.redacted'],
  redactionRefs: ['redaction.public.performance_profile'],
  resourceClass: 'model' as const,
  runRefs: ['run.public.work_1'],
  spanRef: 'performance-span.public.model.first_token',
  status: 'ok' as const,
}

describe('Forge performance diagnostics projection', () => {
  test('projects performance diagnostics as refs-only non-authoritative state', () => {
    const view = projectForgePerformanceDiagnostics({
      ...baseInput,
      entries: [readyEntry],
    })

    expect(view.status).toBe('ready')
    expect(view.publicSafe).toBe(true)
    expect(view.counts).toEqual({
      blocked: 0,
      entries: 1,
      slow: 0,
      stale: 0,
      truncated: 0,
    })
    expect(view.blockerRefs).toEqual([])
    expect(view.authority).toEqual({
      acceptedOutcomeAuthority: false,
      backpressureControlAuthority: false,
      budgetProviderMutationAuthority: false,
      deploymentAuthority: false,
      metricsRecordAuthority: false,
      profileExportAuthority: false,
      publicClaimAuthority: false,
      rawOutputReadAuthority: false,
      runPauseCancelAuthority: false,
      settlementAuthority: false,
      timeoutEnforcementAuthority: false,
      workerPayoutAuthority: false,
    })
  })

  test('treats missing performance state as empty', () => {
    const view = projectForgePerformanceDiagnostics({
      generatedAt: '2026-06-18T03:20:00.000Z',
      workOrderRef: 'work.public.empty',
    })

    expect(view.status).toBe('empty')
    expect(view.entries).toEqual([])
    expect(view.blockerRefs).toEqual([])
  })

  test('blocks stale performance evidence', () => {
    const view = projectForgePerformanceDiagnostics({
      ...baseInput,
      entries: [
        {
          ...readyEntry,
          freshness: 'stale',
          spanRef: 'performance-span.public.stale',
        },
      ],
    })

    expect(view.status).toBe('blocked')
    expect(view.blockerRefs).toContain(
      'forge-performance-diagnostics-blocker:work.public.work_1:stale-performance-evidence:performance-span.public.stale',
    )
  })

  test('blocks blocked spans without blocker refs', () => {
    const view = projectForgePerformanceDiagnostics({
      ...baseInput,
      entries: [
        {
          ...readyEntry,
          latencyClass: 'blocked',
          spanRef: 'performance-span.public.blocked',
          status: 'blocked',
        },
      ],
    })

    expect(view.status).toBe('blocked')
    expect(view.blockerRefs).toContain(
      'forge-performance-diagnostics-blocker:work.public.work_1:blocked-performance-without-blocker:performance-span.public.blocked',
    )
  })

  test('blocks truncation without preserved artifact refs', () => {
    const view = projectForgePerformanceDiagnostics({
      ...baseInput,
      entries: [
        {
          ...readyEntry,
          spanRef: 'performance-span.public.truncated',
          status: 'truncated',
          truncationRefs: ['truncation.public.large_output'],
        },
      ],
    })

    expect(view.status).toBe('blocked')
    expect(view.blockerRefs).toContain(
      'forge-performance-diagnostics-blocker:work.public.work_1:truncation-artifact-ref-missing:performance-span.public.truncated',
    )
  })

  test('blocks profile refs without redaction and policy refs', () => {
    const view = projectForgePerformanceDiagnostics({
      ...baseInput,
      entries: [
        {
          ...readyEntry,
          policyRefs: [],
          profileRefs: ['performance-profile.public.no_policy'],
          redactionRefs: [],
          spanRef: 'performance-span.public.profile',
        },
      ],
    })

    expect(view.status).toBe('blocked')
    expect(view.blockerRefs).toContain(
      'forge-performance-diagnostics-blocker:work.public.work_1:profile-redaction-policy-missing:performance-span.public.profile',
    )
  })

  test('blocks local pressure refs without policy refs', () => {
    const view = projectForgePerformanceDiagnostics({
      ...baseInput,
      entries: [
        {
          ...readyEntry,
          localResourcePressureRefs: ['local-pressure.public.memory'],
          policyRefs: [],
          resourceClass: 'local_resource',
          spanRef: 'performance-span.public.local_pressure',
        },
      ],
    })

    expect(view.status).toBe('blocked')
    expect(view.blockerRefs).toContain(
      'forge-performance-diagnostics-blocker:work.public.work_1:local-pressure-policy-missing:performance-span.public.local_pressure',
    )
  })

  test('blocks provider rate limits classified as local pressure', () => {
    const view = projectForgePerformanceDiagnostics({
      ...baseInput,
      entries: [
        {
          ...readyEntry,
          providerRateLimitRefs: ['provider-rate-limit.public.openai'],
          resourceClass: 'local_resource',
          spanRef: 'performance-span.public.rate_limit',
        },
      ],
    })

    expect(view.status).toBe('blocked')
    expect(view.blockerRefs).toContain(
      'forge-performance-diagnostics-blocker:work.public.work_1:provider-rate-limit-not-local-pressure:performance-span.public.rate_limit',
    )
  })

  test('blocks populated performance entries without snapshot refs', () => {
    const view = projectForgePerformanceDiagnostics({
      entries: [readyEntry],
      generatedAt: '2026-06-18T03:20:00.000Z',
      workOrderRef: 'work.public.no_snapshot',
    })

    expect(view.status).toBe('blocked')
    expect(view.blockerRefs).toContain(
      'forge-performance-diagnostics-blocker:work.public.no_snapshot:missing-performance-diagnostics-snapshot-ref',
    )
  })

  test('omits unsafe private performance material before projection', () => {
    const view = projectForgePerformanceDiagnostics({
      ...baseInput,
      blockerRefs: [
        'performance-blocker.public.safe',
        'raw output /Users/christopher/output.log',
      ],
      entries: [
        {
          ...readyEntry,
          counterRefs: ['counter.public.safe', 'raw prompt /Users/christopher/prompt.md'],
          outputVolumeRefs: ['output-volume.public.safe'],
          policyRefs: ['policy.public.safe', 'bearer token private'],
          profileRefs: ['performance-profile.public.safe', 'profile detail /Users/christopher/profile.json'],
          redactionRefs: ['redaction.public.safe'],
          spanRef: 'performance-span.public.safe',
          timeoutRefs: ['timeout.public.safe', 'provider payload sk-private'],
        },
      ],
    })
    const payload = JSON.stringify(view)

    expect(view.status).toBe('blocked')
    expect(view.entries[0]?.counterRefs).toEqual(['counter.public.safe'])
    expect(view.entries[0]?.timeoutRefs).toEqual(['timeout.public.safe'])
    expect(view.blockerRefs).toContain(
      'forge-performance-diagnostics-blocker:work.public.work_1:unsafe-performance-diagnostics-material-omitted',
    )
    expect(payload).not.toContain('/Users/christopher')
    expect(payload).not.toContain('raw output')
    expect(payload).not.toContain('raw prompt')
    expect(payload).not.toContain('profile detail')
    expect(payload).not.toContain('provider payload')
    expect(payload).not.toContain('bearer token')
    expect(payload).not.toContain('sk-private')
  })

  test('builds input from optional Run projection fields', () => {
    const work = {
      generatedAt: '2026-06-18T03:20:00.000Z',
      performanceDiagnostics: {
        entries: [readyEntry],
        generatedAt: '2026-06-18T03:21:00.000Z',
        profileRefs: ['performance-profile.public.work_2'],
        snapshotRef: 'performance-diagnostics-snapshot.public.work_2',
        versionRef: 'performance-diagnostics-version.public.v2',
      },
      workOrderRef: 'work.public.work_2',
    } as unknown as AutopilotWorkProjection

    expect(buildForgePerformanceDiagnosticsInput(work)).toEqual({
      entries: [readyEntry],
      generatedAt: '2026-06-18T03:21:00.000Z',
      profileRefs: ['performance-profile.public.work_2'],
      snapshotRef: 'performance-diagnostics-snapshot.public.work_2',
      versionRef: 'performance-diagnostics-version.public.v2',
      workOrderRef: 'work.public.work_2',
    })
  })
})
