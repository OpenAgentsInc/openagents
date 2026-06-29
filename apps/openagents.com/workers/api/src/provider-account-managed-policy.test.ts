import { describe, expect, test } from 'vitest'

import {
  PROVIDER_ACCOUNT_MANAGED_POLICY_VERSION,
  resolveProviderAccountManagedPolicy,
} from './provider-account-managed-policy'

describe('provider account managed policy snapshots', () => {
  const base = {
    approvedUserGate: 'enabled' as const,
    approvedUserRefs: ['user:approved-1'],
    attachments: {
      leaseRefs: ['lease:provider-account:1'],
      receiptRefs: ['receipt:provider-account:1'],
      runRefs: ['run:provider-account:1'],
      workOrderRefs: ['work-order:provider-account:1'],
    },
    budgetCaveatRefs: ['budget-caveat:team-monthly-cap'],
    budgetDecision: 'within_budget' as const,
    decisionKind: 'provider_account_lease' as const,
    decisionRef: 'managed-policy-decision:lease:1',
    evaluatedAt: '2026-06-11T18:00:00.000Z',
    generatedAt: '2026-06-11T18:00:10.000Z',
    policyRefs: {
      budgetPolicyRef: 'policy:budget:team-alpha',
      devicePolicyRef: 'policy:device:runner-local',
      organizationPolicyRef: 'policy:org:openagents',
      providerPolicyRef: 'policy:provider:allowlist',
      repositoryPolicyRef: 'policy:repo:openagents',
      retentionPolicyRef: 'policy:retention:standard',
      teamPolicyRef: 'policy:team:alpha',
      telemetryPolicyRef: 'policy:telemetry:aggregate',
      userPolicyRef: 'policy:user:approved-1',
    },
    policyState: 'active' as const,
    provider: 'anthropic_claude' as const,
    providerAllowlist: ['anthropic_claude', 'google_gemini'] as const,
    requestingUserRef: 'user:approved-1',
    retentionCaveatRefs: ['retention-caveat:standard-window'],
    retentionDecision: 'allowed' as const,
    snapshotRef: 'effective-policy:managed-policy-decision:lease:1',
    staleAfterMs: 60_000,
    telemetryCaveatRefs: ['telemetry-caveat:aggregate-only'],
    telemetryDecision: 'aggregate' as const,
  }

  test('allows an active provider-account lease with immutable policy refs', () => {
    const projection = resolveProviderAccountManagedPolicy(base)

    expect(projection).toEqual({
      generatedAt: '2026-06-11T18:00:10.000Z',
      managedPolicyVersion: PROVIDER_ACCOUNT_MANAGED_POLICY_VERSION,
      decisionKind: 'provider_account_lease',
      decisionRef: 'managed-policy-decision:lease:1',
      effectivePolicyRef: 'effective-policy:managed-policy-decision:lease:1',
      evaluatedAt: '2026-06-11T18:00:00.000Z',
      staleAt: '2026-06-11T18:01:00.000Z',
      ageMs: 10_000,
      policyState: 'active',
      status: 'allowed',
      provider: 'anthropic_claude',
      providerAllowlist: ['anthropic_claude', 'google_gemini'],
      providerDisallowReasonRefs: [],
      approvedUserGate: 'enabled',
      requestingUserRef: 'user:approved-1',
      approvedUserRefs: ['user:approved-1'],
      budgetDecision: 'within_budget',
      retentionDecision: 'allowed',
      telemetryDecision: 'aggregate',
      governedByRefs: [
        'policy:org:openagents',
        'policy:team:alpha',
        'policy:repo:openagents',
        'policy:user:approved-1',
        'policy:device:runner-local',
        'policy:provider:allowlist',
        'policy:budget:team-alpha',
        'policy:retention:standard',
        'policy:telemetry:aggregate',
      ],
      caveatRefs: [
        'budget-caveat:team-monthly-cap',
        'retention-caveat:standard-window',
        'telemetry-caveat:aggregate-only',
      ],
      denialReasonRefs: [],
      attachmentRefs: {
        leaseRefs: ['lease:provider-account:1'],
        receiptRefs: ['receipt:provider-account:1'],
        runRefs: ['run:provider-account:1'],
        workOrderRefs: ['work-order:provider-account:1'],
      },
    })
  })

  test('denies disallowed providers, unapproved users, and over-budget decisions', () => {
    const projection = resolveProviderAccountManagedPolicy({
      ...base,
      budgetDecision: 'over_budget',
      provider: 'chatgpt_codex',
      providerDisallowReasonRefs: ['policy-denial:provider:codex-not-in-scope'],
      requestingUserRef: 'user:not-approved',
    })

    expect(projection.status).toBe('denied')
    expect(projection.denialReasonRefs).toEqual([
      'provider-account-managed-policy-denial:managed-policy-decision:lease:1:provider_disallowed:chatgpt_codex',
      'provider-account-managed-policy-denial:managed-policy-decision:lease:1:user_not_approved',
      'provider-account-managed-policy-denial:managed-policy-decision:lease:1:budget_over_budget',
      'policy-denial:provider:codex-not-in-scope',
    ])
  })

  test('treats stale policy snapshots as stale before normal allow decisions', () => {
    const projection = resolveProviderAccountManagedPolicy({
      ...base,
      generatedAt: '2026-06-11T18:03:00.000Z',
    })

    expect(projection).toMatchObject({
      ageMs: 180_000,
      status: 'stale',
      denialReasonRefs: [
        'provider-account-managed-policy-denial:managed-policy-decision:lease:1:policy_stale',
      ],
    })
  })

  test('treats unknown policy state as unknown for team-budget decisions', () => {
    const projection = resolveProviderAccountManagedPolicy({
      ...base,
      budgetDecision: 'missing',
      decisionKind: 'team_budget',
      decisionRef: 'managed-policy-decision:team-budget:1',
      policyState: 'unknown',
      snapshotRef: undefined,
    })

    expect(projection).toMatchObject({
      decisionKind: 'team_budget',
      effectivePolicyRef:
        'provider-account-effective-policy:managed-policy-decision:team-budget:1:2026-06-11T18:00:00.000Z',
      status: 'unknown',
      denialReasonRefs: [
        'provider-account-managed-policy-denial:managed-policy-decision:team-budget:1:policy_unknown',
      ],
    })
  })

  test('rejects credentials, raw prompts, private repo data, and raw provider responses', () => {
    expect(() =>
      resolveProviderAccountManagedPolicy({
        ...base,
        decisionRef: 'managed-policy-decision:ANTHROPIC_API_KEY=secret',
      }),
    ).toThrow(/provider credential material/)

    expect(() =>
      resolveProviderAccountManagedPolicy({
        ...base,
        attachments: {
          runRefs: ['run:raw prompt: customer code'],
        },
      }),
    ).toThrow(/private managed policy material/)

    expect(() =>
      resolveProviderAccountManagedPolicy({
        ...base,
        policyRefs: {
          ...base.policyRefs,
          repositoryPolicyRef: 'git@github.com:OpenAgentsInc/private-repo.git',
        },
      }),
    ).toThrow(/private managed policy material/)

    expect(() =>
      resolveProviderAccountManagedPolicy({
        ...base,
        providerDisallowReasonRefs: [
          'policy-denial:raw provider response: choices',
        ],
        provider: 'chatgpt_codex',
      }),
    ).toThrow(/private managed policy material/)
  })
})
