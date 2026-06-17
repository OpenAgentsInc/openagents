import { describe, expect, test } from 'vitest'

import type { AutopilotWorkProjection } from '../model'
import {
  buildForgeOnboardingCapabilityInput,
  projectForgeOnboardingCapabilityEvidence,
} from './onboarding-capability-evidence'

const baseInput = {
  generatedAt: '2026-06-18T07:00:00.000Z',
  snapshotRef: 'onboarding-capability-snapshot.public.work_1',
  versionRef: 'onboarding-capability-version.public.v1',
  workOrderRef: 'work.public.work_1',
}

const readyRepositoryStep = {
  capabilityProbeRefs: ['capability-probe.public.repo_profile'],
  completionReceiptRefs: ['onboarding-completion.public.repo_profile'],
  dataScopeRefs: ['data-scope.public.repo_refs_only'],
  freshness: 'fresh' as const,
  instructionRefs: ['instructions.public.project_refs'],
  invariantRefs: ['invariants.public.workspace_contract'],
  mode: 'local_only' as const,
  permissionDecisionRefs: ['permission.public.repo_read_refs'],
  repositoryProfileRefs: ['repo-profile.public.openagents'],
  status: 'ready' as const,
  stepKind: 'repository_profile' as const,
  stepRef: 'onboarding-step.public.repo_profile',
  workspaceRefs: ['workspace.public.openagents'],
}

describe('Forge onboarding capability evidence projection', () => {
  test('projects onboarding capability evidence as refs-only non-authoritative state', () => {
    const view = projectForgeOnboardingCapabilityEvidence({
      ...baseInput,
      entries: [readyRepositoryStep],
    })

    expect(view.status).toBe('ready')
    expect(view.publicSafe).toBe(true)
    expect(view.counts).toEqual({
      blocked: 0,
      providerConnected: 0,
      ready: 1,
      skipped: 0,
      smokes: 0,
      stale: 0,
      steps: 1,
    })
    expect(view.blockerRefs).toEqual([])
    expect(view.authority).toEqual({
      acceptedOutcomeAuthority: false,
      capabilityEnablementAuthority: false,
      credentialStorageAuthority: false,
      dataScopeMutationAuthority: false,
      firstRunSmokeExecutionAuthority: false,
      integrationEnablementAuthority: false,
      onboardingStepMutationAuthority: false,
      paidWorkflowActivationAuthority: false,
      permissionGrantAuthority: false,
      providerConnectionAuthority: false,
      repositoryScanAuthority: false,
      repositoryWriteAuthority: false,
      secretCollectionAuthority: false,
      settingsMutationAuthority: false,
      settlementAuthority: false,
      teamInvitationAcceptanceAuthority: false,
      workerPayoutAuthority: false,
    })
  })

  test('treats missing onboarding evidence as empty', () => {
    const view = projectForgeOnboardingCapabilityEvidence({
      generatedAt: '2026-06-18T07:00:00.000Z',
      workOrderRef: 'work.public.empty',
    })

    expect(view.status).toBe('empty')
    expect(view.entries).toEqual([])
    expect(view.blockerRefs).toEqual([])
  })

  test('blocks ready required steps without probe or receipt refs', () => {
    const view = projectForgeOnboardingCapabilityEvidence({
      ...baseInput,
      entries: [
        {
          mode: 'local_only',
          status: 'ready',
          stepKind: 'capability_probe',
          stepRef: 'onboarding-step.public.capability_probe',
        },
      ],
    })

    expect(view.status).toBe('blocked')
    expect(view.blockerRefs).toContain(
      'forge-onboarding-capability-blocker:work.public.work_1:required-onboarding-step-evidence-missing:onboarding-step.public.capability_probe',
    )
  })

  test('blocks provider-connected modes without readiness and credential policy refs', () => {
    const view = projectForgeOnboardingCapabilityEvidence({
      ...baseInput,
      entries: [
        {
          capabilityProbeRefs: ['capability-probe.public.provider'],
          completionReceiptRefs: ['onboarding-completion.public.provider'],
          mode: 'api_connected',
          providerReadinessRefs: [],
          status: 'ready',
          stepKind: 'provider',
          stepRef: 'onboarding-step.public.provider',
        },
      ],
    })

    expect(view.status).toBe('blocked')
    expect(view.blockerRefs).toContain(
      'forge-onboarding-capability-blocker:work.public.work_1:provider-connected-mode-evidence-missing:onboarding-step.public.provider',
    )
  })

  test('blocks repository and workspace setup without profile instruction invariant permission and data scope refs', () => {
    const view = projectForgeOnboardingCapabilityEvidence({
      ...baseInput,
      entries: [
        {
          capabilityProbeRefs: ['capability-probe.public.repo_profile'],
          completionReceiptRefs: ['onboarding-completion.public.repo_profile'],
          mode: 'local_only',
          repositoryProfileRefs: ['repo-profile.public.openagents'],
          status: 'ready',
          stepKind: 'repository_profile',
          stepRef: 'onboarding-step.public.repo_profile',
        },
      ],
    })

    expect(view.status).toBe('blocked')
    expect(view.blockerRefs).toContain(
      'forge-onboarding-capability-blocker:work.public.work_1:repository-workspace-setup-evidence-missing:onboarding-step.public.repo_profile',
    )
  })

  test('blocks skipped optional integrations without skip receipt refs', () => {
    const view = projectForgeOnboardingCapabilityEvidence({
      ...baseInput,
      entries: [
        {
          integrationRefs: ['integration.public.slack'],
          mode: 'local_only',
          status: 'skipped',
          stepKind: 'integration',
          stepRef: 'onboarding-step.public.slack',
        },
      ],
    })

    expect(view.status).toBe('blocked')
    expect(view.blockerRefs).toContain(
      'forge-onboarding-capability-blocker:work.public.work_1:optional-onboarding-skip-receipt-missing:onboarding-step.public.slack',
    )
  })

  test('blocks first-run ready states without smoke receipt refs', () => {
    const view = projectForgeOnboardingCapabilityEvidence({
      ...baseInput,
      entries: [
        {
          completionReceiptRefs: ['onboarding-completion.public.first_smoke'],
          mode: 'local_only',
          status: 'ready',
          stepKind: 'first_run_smoke',
          stepRef: 'onboarding-step.public.first_smoke',
        },
      ],
    })

    expect(view.status).toBe('blocked')
    expect(view.blockerRefs).toContain(
      'forge-onboarding-capability-blocker:work.public.work_1:first-run-smoke-receipt-missing:onboarding-step.public.first_smoke',
    )
  })

  test('blocks stale onboarding capability evidence', () => {
    const view = projectForgeOnboardingCapabilityEvidence({
      ...baseInput,
      entries: [
        {
          ...readyRepositoryStep,
          freshness: 'stale',
          stepRef: 'onboarding-step.public.stale_repo',
        },
      ],
    })

    expect(view.status).toBe('blocked')
    expect(view.blockerRefs).toContain(
      'forge-onboarding-capability-blocker:work.public.work_1:stale-onboarding-capability-evidence:onboarding-step.public.stale_repo',
    )
  })

  test('surfaces planned capabilities as in-progress rather than ready', () => {
    const view = projectForgeOnboardingCapabilityEvidence({
      ...baseInput,
      entries: [
        {
          capabilityProbeRefs: ['capability-probe.public.mobile'],
          mode: 'team',
          status: 'planned',
          stepKind: 'provider',
          stepRef: 'onboarding-step.public.mobile_companion',
        },
      ],
    })

    expect(view.status).toBe('in_progress')
    expect(view.blockerRefs).toEqual([])
  })

  test('blocks populated onboarding entries without snapshot refs', () => {
    const view = projectForgeOnboardingCapabilityEvidence({
      entries: [readyRepositoryStep],
      generatedAt: '2026-06-18T07:00:00.000Z',
      workOrderRef: 'work.public.no_snapshot',
    })

    expect(view.status).toBe('blocked')
    expect(view.blockerRefs).toContain(
      'forge-onboarding-capability-blocker:work.public.no_snapshot:missing-onboarding-capability-snapshot-ref',
    )
  })

  test('omits unsafe private onboarding material before projection', () => {
    const view = projectForgeOnboardingCapabilityEvidence({
      ...baseInput,
      blockerRefs: [
        'onboarding-blocker.public.safe',
        'raw secret /Users/christopher/secret.txt',
      ],
      entries: [
        {
          ...readyRepositoryStep,
          capabilityProbeRefs: [
            'capability-probe.public.safe',
            'raw device id private',
          ],
          credentialPolicyRefs: ['credential-policy.public.safe'],
          dataScopeRefs: ['data-scope.public.safe', 'workspace path /Users/christopher/work'],
          firstRunSmokeRefs: ['first-smoke.public.safe', 'smoke log bearer token private'],
          instructionRefs: ['instructions.public.safe', 'instruction body secret'],
          integrationRefs: ['integration.public.safe', 'integration payload sk-private'],
          permissionDecisionRefs: ['permission.public.safe'],
          providerReadinessRefs: ['provider-readiness.public.safe'],
          repositoryProfileRefs: ['repo-profile.public.safe', 'repository private data'],
          stepRef: 'onboarding-step.public.safe',
          userDeviceRefs: ['device.public.safe', 'user email private'],
        },
      ],
    })
    const payload = JSON.stringify(view)

    expect(view.status).toBe('blocked')
    expect(view.entries[0]?.capabilityProbeRefs).toEqual([
      'capability-probe.public.safe',
    ])
    expect(view.entries[0]?.firstRunSmokeRefs).toEqual(['first-smoke.public.safe'])
    expect(view.entries[0]?.instructionRefs).toEqual(['instructions.public.safe'])
    expect(view.entries[0]?.integrationRefs).toEqual(['integration.public.safe'])
    expect(view.entries[0]?.repositoryProfileRefs).toEqual([
      'repo-profile.public.safe',
    ])
    expect(view.entries[0]?.userDeviceRefs).toEqual(['device.public.safe'])
    expect(view.blockerRefs).toContain(
      'forge-onboarding-capability-blocker:work.public.work_1:unsafe-onboarding-capability-material-omitted',
    )
    expect(payload).not.toContain('/Users/christopher')
    expect(payload).not.toContain('raw secret')
    expect(payload).not.toContain('raw device id')
    expect(payload).not.toContain('workspace path')
    expect(payload).not.toContain('smoke log')
    expect(payload).not.toContain('instruction body')
    expect(payload).not.toContain('integration payload')
    expect(payload).not.toContain('repository private data')
    expect(payload).not.toContain('user email')
    expect(payload).not.toContain('bearer token')
    expect(payload).not.toContain('sk-private')
  })

  test('builds input from optional Run projection fields', () => {
    const work = {
      generatedAt: '2026-06-18T07:00:00.000Z',
      onboardingCapabilityEvidence: {
        entries: [readyRepositoryStep],
        generatedAt: '2026-06-18T07:01:00.000Z',
        snapshotRef: 'onboarding-capability-snapshot.public.work_2',
        versionRef: 'onboarding-capability-version.public.v2',
      },
      workOrderRef: 'work.public.work_2',
    } as unknown as AutopilotWorkProjection

    expect(buildForgeOnboardingCapabilityInput(work)).toEqual({
      entries: [readyRepositoryStep],
      generatedAt: '2026-06-18T07:01:00.000Z',
      snapshotRef: 'onboarding-capability-snapshot.public.work_2',
      versionRef: 'onboarding-capability-version.public.v2',
      workOrderRef: 'work.public.work_2',
    })
  })
})
