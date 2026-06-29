import { describe, expect, test } from 'vitest'

import {
  buildForgeHelpDoctorDebugInput,
  projectForgeHelpDoctorDebug,
} from './help-doctor-debug'
import type { AutopilotWorkProjection } from '../model'

const baseInput = {
  generatedAt: '2026-06-17T23:00:00.000Z',
  snapshotRef: 'help-doctor-debug-snapshot.public.work_1',
  versionRef: 'help-doctor-debug-version.public.v1',
  workOrderRef: 'work.public.work_1',
}

describe('Forge help/doctor/debug projection', () => {
  test('projects public help and doctor evidence as refs-only non-authoritative state', () => {
    const view = projectForgeHelpDoctorDebug({
      ...baseInput,
      entries: [
        {
          debugBundleRefs: ['debug-bundle.public.safe'],
          diagnosticRefs: ['diagnostic.public.doctor.ok'],
          doctorCheckRefs: ['doctor-check.public.context'],
          freshness: 'fresh',
          helpTopicRefs: ['help-topic.public.context'],
          policyRefs: ['policy.public.debug.safe'],
          remediationRefs: ['remediation.public.none'],
          severity: 'info',
          sourceRefs: ['source.public.pylon_doctor'],
          state: 'passed',
          surfaceRef: 'help-doctor-debug.public.context',
        },
      ],
    })

    expect(view.status).toBe('ready')
    expect(view.publicSafe).toBe(true)
    expect(view.counts).toEqual({
      blocked: 0,
      failed: 0,
      passed: 1,
      total: 1,
      warnings: 0,
    })
    expect(view.blockerRefs).toEqual([])
    expect(view.authority).toEqual({
      acceptedOutcomeAuthority: false,
      debugBundleCollectionAuthority: false,
      deploymentAuthority: false,
      diagnosticsExecutionAuthority: false,
      doctorExecutionAuthority: false,
      fileReadAuthority: false,
      logCollectionAuthority: false,
      providerAuthority: false,
      publicClaimAuthority: false,
      runStateMutationAuthority: false,
      settingsWriteAuthority: false,
      settlementAuthority: false,
      shellExecutionAuthority: false,
      toolGrantAuthority: false,
      workerPayoutAuthority: false,
    })
  })

  test('treats missing help/doctor/debug state as empty', () => {
    const view = projectForgeHelpDoctorDebug({
      generatedAt: '2026-06-17T23:00:00.000Z',
      workOrderRef: 'work.public.empty',
    })

    expect(view.status).toBe('empty')
    expect(view.snapshotRef).toBeNull()
    expect(view.entries).toEqual([])
    expect(view.blockerRefs).toEqual([])
  })

  test('blocks stale doctor evidence', () => {
    const view = projectForgeHelpDoctorDebug({
      ...baseInput,
      entries: [
        {
          doctorCheckRefs: ['doctor-check.public.stale'],
          freshness: 'stale',
          severity: 'warning',
          state: 'warning',
          surfaceRef: 'help-doctor-debug.public.stale',
        },
      ],
    })

    expect(view.status).toBe('blocked')
    expect(view.blockerRefs).toContain(
      'forge-help-doctor-debug-blocker:work.public.work_1:stale-doctor-evidence:help-doctor-debug.public.stale',
    )
  })

  test('blocks failed checks without remediation refs', () => {
    const view = projectForgeHelpDoctorDebug({
      ...baseInput,
      entries: [
        {
          doctorCheckRefs: ['doctor-check.public.failed'],
          freshness: 'fresh',
          severity: 'error',
          state: 'failed',
          surfaceRef: 'help-doctor-debug.public.failed',
        },
      ],
    })

    expect(view.status).toBe('blocked')
    expect(view.blockerRefs).toContain(
      'forge-help-doctor-debug-blocker:work.public.work_1:remediation-ref-missing:help-doctor-debug.public.failed',
    )
  })

  test('blocks debug bundles without policy refs', () => {
    const view = projectForgeHelpDoctorDebug({
      ...baseInput,
      entries: [
        {
          debugBundleRefs: ['debug-bundle.public.missing_policy'],
          diagnosticRefs: ['diagnostic.public.safe'],
          freshness: 'fresh',
          remediationRefs: ['remediation.public.safe'],
          severity: 'warning',
          state: 'warning',
          surfaceRef: 'help-doctor-debug.public.debug_policy',
        },
      ],
    })

    expect(view.status).toBe('blocked')
    expect(view.blockerRefs).toContain(
      'forge-help-doctor-debug-blocker:work.public.work_1:debug-bundle-policy-missing:help-doctor-debug.public.debug_policy',
    )
  })

  test('blocks entries without help doctor or diagnostic evidence', () => {
    const view = projectForgeHelpDoctorDebug({
      ...baseInput,
      entries: [
        {
          freshness: 'fresh',
          severity: 'info',
          state: 'unknown',
          surfaceRef: 'help-doctor-debug.public.no_source',
        },
      ],
    })

    expect(view.status).toBe('blocked')
    expect(view.blockerRefs).toContain(
      'forge-help-doctor-debug-blocker:work.public.work_1:help-doctor-source-missing:help-doctor-debug.public.no_source',
    )
  })

  test('omits unsafe private help doctor debug material before projection', () => {
    const view = projectForgeHelpDoctorDebug({
      ...baseInput,
      blockerRefs: ['debug-blocker.public.safe', 'raw debug /Users/christopher/debug.log'],
      entries: [
        {
          debugBundleRefs: ['debug-bundle.public.safe', 'raw log sk-private'],
          diagnosticRefs: ['diagnostic.public.safe', 'raw diagnostic /Users/christopher/diag'],
          doctorCheckRefs: ['doctor-check.public.safe'],
          freshness: 'fresh',
          helpTopicRefs: ['help-topic.public.safe'],
          policyRefs: ['policy.public.safe', 'bearer token private'],
          remediationRefs: ['remediation.public.safe'],
          severity: 'warning',
          sourceRefs: ['source.public.safe', 'private debug token'],
          state: 'warning',
          surfaceRef: 'help-doctor-debug.public.safe',
        },
      ],
    })

    const payload = JSON.stringify(view)

    expect(view.status).toBe('blocked')
    expect(view.entries[0]?.debugBundleRefs).toEqual(['debug-bundle.public.safe'])
    expect(view.entries[0]?.diagnosticRefs).toEqual(['diagnostic.public.safe'])
    expect(view.entries[0]?.policyRefs).toEqual(['policy.public.safe'])
    expect(view.blockerRefs).toContain(
      'forge-help-doctor-debug-blocker:work.public.work_1:unsafe-help-doctor-debug-material-omitted',
    )
    expect(payload).not.toContain('/Users/christopher')
    expect(payload).not.toContain('raw debug')
    expect(payload).not.toContain('raw log')
    expect(payload).not.toContain('raw diagnostic')
    expect(payload).not.toContain('private debug')
    expect(payload).not.toContain('bearer token')
    expect(payload).not.toContain('sk-private')
  })

  test('builds input from optional Run projection fields', () => {
    const work = {
      generatedAt: '2026-06-17T23:01:00.000Z',
      helpDoctorDebug: {
        entries: [
          {
            doctorCheckRefs: ['doctor-check.public.work_2'],
            freshness: 'fresh',
            helpTopicRefs: ['help-topic.public.work_2'],
            remediationRefs: ['remediation.public.work_2'],
            severity: 'info',
            state: 'passed',
            surfaceRef: 'help-doctor-debug.public.work_2',
          },
        ],
        snapshotRef: 'help-doctor-debug-snapshot.public.work_2',
        versionRef: 'help-doctor-debug-version.public.v2',
      },
      workOrderRef: 'work.public.work_2',
    } as unknown as AutopilotWorkProjection

    expect(buildForgeHelpDoctorDebugInput(work)).toEqual({
      entries: [
        {
          doctorCheckRefs: ['doctor-check.public.work_2'],
          freshness: 'fresh',
          helpTopicRefs: ['help-topic.public.work_2'],
          remediationRefs: ['remediation.public.work_2'],
          severity: 'info',
          state: 'passed',
          surfaceRef: 'help-doctor-debug.public.work_2',
        },
      ],
      generatedAt: '2026-06-17T23:01:00.000Z',
      snapshotRef: 'help-doctor-debug-snapshot.public.work_2',
      versionRef: 'help-doctor-debug-version.public.v2',
      workOrderRef: 'work.public.work_2',
    })
  })
})
