import { describe, expect, test } from 'vitest'

import type { AutopilotWorkProjection } from '../model'
import {
  buildForgeEnterpriseManagedPolicyInput,
  projectForgeEnterpriseManagedPolicyEvidence,
} from './enterprise-managed-policy-evidence'

const baseInput = {
  generatedAt: '2026-06-18T14:00:00.000Z',
  snapshotRef: 'enterprise-managed-policy-snapshot.public.work_1',
  versionRef: 'enterprise-managed-policy-version.public.v1',
  workOrderRef: 'work.public.work_1',
}

const readyPolicy = {
  allowRefs: ['allow.public.provider_openai'],
  auditRefs: ['policy-audit.public.created'],
  budgetPolicyRefs: ['budget-policy.public.team_cap'],
  changeRefs: ['policy-change.public.v1'],
  decision: 'allow' as const,
  effectiveAtRefs: ['effective-at.public.2026-06-18'],
  effectivePolicyRefs: ['effective-policy.public.team_provider'],
  enforcementModeRefs: ['enforcement-mode.public.enforce'],
  freshness: 'fresh' as const,
  organizationPolicyRefs: ['organization-policy.public.openagents'],
  ownerAdminRefs: ['admin.public.policy_owner'],
  policyRef: 'managed-policy.public.provider',
  providerPolicyRefs: ['provider-policy.public.allowlist'],
  publicSummaryRefs: ['policy-summary.public.safe'],
  ruleKindRefs: ['rule-kind.public.provider_allowlist'],
  runtimeCapabilityBoundaryRefs: ['runtime-capability-boundary.public.no_grant'],
  scopeRefs: ['scope.public.team'],
  status: 'ready' as const,
  teamPolicyRefs: ['team-policy.public.engineering'],
  versionRefs: ['policy-version.public.v1'],
}

describe('Forge enterprise managed policy evidence projection', () => {
  test('projects enterprise managed policy evidence as refs-only non-authoritative state', () => {
    const view = projectForgeEnterpriseManagedPolicyEvidence({
      ...baseInput,
      entries: [readyPolicy],
    })

    expect(view.status).toBe('ready')
    expect(view.publicSafe).toBe(true)
    expect(view.counts).toEqual({
      askRestrict: 0,
      denied: 0,
      emergencyOverrides: 0,
      ready: 1,
      stale: 0,
      total: 1,
    })
    expect(view.blockerRefs).toEqual([])
    expect(view.authority).toEqual({
      acceptedOutcomeAuthority: false,
      budgetMutationAuthority: false,
      capabilityGrantAuthority: false,
      emergencyOverrideAuthority: false,
      integrationGateMutationAuthority: false,
      policyEnforcementAuthority: false,
      policyExportAuthority: false,
      policyInstallAuthority: false,
      policyLoadAuthority: false,
      policyMutationAuthority: false,
      providerMutationAuthority: false,
      publicProjectionMutationAuthority: false,
      retentionMutationAuthority: false,
      settlementAuthority: false,
      telemetryMutationAuthority: false,
      updateChannelMutationAuthority: false,
      workerPayoutAuthority: false,
    })
  })

  test('treats missing managed policy evidence as empty', () => {
    const view = projectForgeEnterpriseManagedPolicyEvidence({
      generatedAt: '2026-06-18T14:00:00.000Z',
      workOrderRef: 'work.public.empty',
    })

    expect(view.status).toBe('empty')
    expect(view.entries).toEqual([])
    expect(view.blockerRefs).toEqual([])
  })

  test('blocks ready managed policy without effective scope owner version rule enforcement audit summary and boundary refs', () => {
    const view = projectForgeEnterpriseManagedPolicyEvidence({
      ...baseInput,
      entries: [
        {
          ...readyPolicy,
          auditRefs: [],
          changeRefs: [],
          effectivePolicyRefs: [],
          enforcementModeRefs: [],
          ownerAdminRefs: [],
          publicSummaryRefs: [],
          ruleKindRefs: [],
          runtimeCapabilityBoundaryRefs: [],
          scopeRefs: [],
          versionRefs: [],
        },
      ],
    })

    expect(view.status).toBe('blocked')
    expect(view.blockerRefs).toContain(
      'forge-enterprise-managed-policy-blocker:work.public.work_1:ready-managed-policy-evidence-missing:managed-policy.public.provider',
    )
  })

  test('blocks denied policy without typed denial and user-safe reason refs', () => {
    const view = projectForgeEnterpriseManagedPolicyEvidence({
      ...baseInput,
      entries: [
        {
          ...readyPolicy,
          decision: 'deny',
          denialRefs: [],
          policyRef: 'managed-policy.public.denied',
          userSafeReasonRefs: [],
        },
      ],
    })

    expect(view.status).toBe('blocked')
    expect(view.blockerRefs).toContain(
      'forge-enterprise-managed-policy-blocker:work.public.work_1:managed-policy-denial-reason-missing:managed-policy.public.denied',
    )
  })

  test('blocks ask decisions without ask refs', () => {
    const view = projectForgeEnterpriseManagedPolicyEvidence({
      ...baseInput,
      entries: [
        {
          ...readyPolicy,
          askRefs: [],
          decision: 'ask',
          policyRef: 'managed-policy.public.ask',
        },
      ],
    })

    expect(view.status).toBe('blocked')
    expect(view.blockerRefs).toContain(
      'forge-enterprise-managed-policy-blocker:work.public.work_1:managed-policy-ask-ref-missing:managed-policy.public.ask',
    )
  })

  test('blocks restrict decisions without restrict refs', () => {
    const view = projectForgeEnterpriseManagedPolicyEvidence({
      ...baseInput,
      entries: [
        {
          ...readyPolicy,
          decision: 'restrict',
          policyRef: 'managed-policy.public.restrict',
          restrictRefs: [],
        },
      ],
    })

    expect(view.status).toBe('blocked')
    expect(view.blockerRefs).toContain(
      'forge-enterprise-managed-policy-blocker:work.public.work_1:managed-policy-restrict-ref-missing:managed-policy.public.restrict',
    )
  })

  test('blocks conflicts without conflict resolution and priority refs', () => {
    const view = projectForgeEnterpriseManagedPolicyEvidence({
      ...baseInput,
      entries: [
        {
          ...readyPolicy,
          conflictPriorityRefs: [],
          conflictRefs: ['policy-conflict.public.provider_vs_budget'],
          conflictResolutionRefs: [],
          policyRef: 'managed-policy.public.conflict',
        },
      ],
    })

    expect(view.status).toBe('blocked')
    expect(view.blockerRefs).toContain(
      'forge-enterprise-managed-policy-blocker:work.public.work_1:managed-policy-conflict-resolution-missing:managed-policy.public.conflict',
    )
  })

  test('blocks emergency overrides without expiration and receipt refs', () => {
    const view = projectForgeEnterpriseManagedPolicyEvidence({
      ...baseInput,
      entries: [
        {
          ...readyPolicy,
          emergencyOverrideReceiptRefs: [],
          expirationRefs: [],
          policyRef: 'managed-policy.public.emergency',
          ruleKindRefs: ['rule-kind.public.emergency_override'],
        },
      ],
    })

    expect(view.status).toBe('blocked')
    expect(view.blockerRefs).toContain(
      'forge-enterprise-managed-policy-blocker:work.public.work_1:emergency-override-expiration-receipt-missing:managed-policy.public.emergency',
    )
  })

  test('blocks stale managed policy evidence', () => {
    const view = projectForgeEnterpriseManagedPolicyEvidence({
      ...baseInput,
      entries: [
        {
          ...readyPolicy,
          freshness: 'stale',
        },
      ],
    })

    expect(view.status).toBe('blocked')
    expect(view.blockerRefs).toContain(
      'forge-enterprise-managed-policy-blocker:work.public.work_1:stale-enterprise-managed-policy-evidence:managed-policy.public.provider',
    )
  })

  test('blocks populated managed policy entries without snapshot refs', () => {
    const view = projectForgeEnterpriseManagedPolicyEvidence({
      entries: [readyPolicy],
      generatedAt: '2026-06-18T14:00:00.000Z',
      workOrderRef: 'work.public.no_snapshot',
    })

    expect(view.status).toBe('blocked')
    expect(view.blockerRefs).toContain(
      'forge-enterprise-managed-policy-blocker:work.public.no_snapshot:missing-enterprise-managed-policy-snapshot-ref',
    )
  })

  test('omits unsafe private managed policy material before projection', () => {
    const view = projectForgeEnterpriseManagedPolicyEvidence({
      ...baseInput,
      blockerRefs: [
        'managed-policy-blocker.public.safe',
        'raw policy /Users/christopher/policy.json',
      ],
      entries: [
        {
          ...readyPolicy,
          caveatRefs: ['policy-caveat.public.safe', 'silent broadening private'],
          organizationPolicyRefs: [
            'organization-policy.public.safe',
            'private org /Users/christopher/org.json',
          ],
          policyRef: 'managed-policy.public.safe',
          providerPolicyRefs: [
            'provider-policy.public.safe',
            'provider payload private',
          ],
          publicSummaryRefs: [
            'policy-summary.public.safe',
            'policy internals private',
          ],
        },
      ],
    })
    const payload = JSON.stringify(view)

    expect(view.status).toBe('blocked')
    expect(view.entries[0]?.organizationPolicyRefs).toEqual([
      'organization-policy.public.safe',
    ])
    expect(view.entries[0]?.providerPolicyRefs).toEqual([
      'provider-policy.public.safe',
    ])
    expect(view.entries[0]?.publicSummaryRefs).toEqual([
      'policy-summary.public.safe',
    ])
    expect(view.blockerRefs).toContain(
      'forge-enterprise-managed-policy-blocker:work.public.work_1:unsafe-managed-policy-material-omitted',
    )
    expect(payload).not.toContain('/Users/christopher')
    expect(payload).not.toContain('raw policy')
    expect(payload).not.toContain('private org')
    expect(payload).not.toContain('provider payload')
    expect(payload).not.toContain('policy internals')
    expect(payload).not.toContain('silent broadening')
  })

  test('builds input from optional Run projection fields', () => {
    const work = {
      enterpriseManagedPolicyEvidence: {
        entries: [readyPolicy],
        generatedAt: '2026-06-18T14:01:00.000Z',
        snapshotRef: 'enterprise-managed-policy-snapshot.public.work_2',
        versionRef: 'enterprise-managed-policy-version.public.v2',
      },
      generatedAt: '2026-06-18T14:00:00.000Z',
      workOrderRef: 'work.public.work_2',
    } as unknown as AutopilotWorkProjection

    expect(buildForgeEnterpriseManagedPolicyInput(work)).toEqual({
      entries: [readyPolicy],
      generatedAt: '2026-06-18T14:01:00.000Z',
      snapshotRef: 'enterprise-managed-policy-snapshot.public.work_2',
      versionRef: 'enterprise-managed-policy-version.public.v2',
      workOrderRef: 'work.public.work_2',
    })
  })
})
