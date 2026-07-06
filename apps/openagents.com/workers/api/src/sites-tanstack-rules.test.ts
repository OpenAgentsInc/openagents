import { describe, expect, test } from 'vitest'

import {
  SITES_TANSTACK_RULES,
  SITES_TANSTACK_RULES_METADATA_KEY,
  SITES_TANSTACK_RULES_REF,
  generatedSiteBehaviorContractRegistry,
  lintGeneratedSiteMarketingClaims,
  runGeneratedSiteBehaviorContractSweep,
  sitesTanstackRulesSessionMetadata,
  validateGeneratedSiteBehaviorContractRegistry,
} from './sites-tanstack-rules'

const siteInput = {
  baseUrl: 'https://sites.openagents.com/openagents-funnel',
  customerSegment: 'teams buying AI-operated software delivery',
  description:
    'OpenAgents turns operator-reviewed AI work into public, verifiable software delivery.',
  primaryActionLabel: 'Book the audit',
  primaryActionPath: '/business',
  secondaryActionLabel: 'Read the agent guide',
  secondaryActionPath: '/llms.txt',
  siteId: 'site_project_openagents_funnel',
  slug: 'openagents-funnel',
  title: 'OpenAgents AI operations funnel',
  vertical: 'AI operations',
} as const

describe('Sites TanStack rules and generated-site behavior contracts', () => {
  test('injects the versioned rules pack metadata with a feedback row for every rule', () => {
    const metadata = sitesTanstackRulesSessionMetadata({
      source: 'self_serve_builder',
    })
    const injected = metadata[SITES_TANSTACK_RULES_METADATA_KEY] as {
      feedbackRefs: ReadonlyArray<string>
      ref: string
      ruleRefs: ReadonlyArray<string>
      sessionBrief: ReadonlyArray<string>
      version: string
    }
    const ruleIds = SITES_TANSTACK_RULES.rules.map(rule => rule.id).sort()
    const feedbackRuleIds = SITES_TANSTACK_RULES.feedbackLedger
      .map(row => row.ruleId)
      .sort()

    expect(metadata.source).toBe('self_serve_builder')
    expect(injected).toMatchObject({
      ref: SITES_TANSTACK_RULES_REF,
      version: '2026-07-04.1',
    })
    expect([...injected.ruleRefs].sort()).toEqual(ruleIds)
    expect(feedbackRuleIds).toEqual(ruleIds)
    expect(injected.feedbackRefs).toHaveLength(ruleIds.length)
    expect(injected.sessionBrief.join('\n')).toContain('createServerFn')
    expect(injected.sessionBrief.join('\n')).toContain('Worker bindings')
  })

  test('registers enforced starter behavior contracts for generated Start sites', () => {
    const registry = generatedSiteBehaviorContractRegistry({
      previewUrl: 'https://preview.openagents.com/sites/openagents-funnel',
      siteId: siteInput.siteId,
    })
    const validation = validateGeneratedSiteBehaviorContractRegistry({
      siteId: siteInput.siteId,
    })

    expect(validation).toMatchObject({ ok: true, issues: [] })
    expect(registry.contracts.map(contract => contract.contractId)).toEqual([
      'autopilot_sites.generated.dead_controls.v1',
      'autopilot_sites.generated.navigation_integrity.v1',
      'autopilot_sites.generated.claim_safety.v1',
      'autopilot_sites.generated.bundle_budget.v1',
    ])
    expect(
      registry.contracts.every(
        contract =>
          contract.state === 'enforced' &&
          contract.enforcementTier === 'test-sweep' &&
          contract.oracles[0]?.ref ===
            'apps/openagents.com/workers/api/src/sites-tanstack-rules.test.ts',
      ),
    ).toBe(true)
  })

  test('fails closed for a generated site with dead controls, broken nav, gated claims, and an oversized bundle', () => {
    const receipt = runGeneratedSiteBehaviorContractSweep({
      bundleBudgetBytes: 120,
      files: [
        {
          path: 'src/routes/index.tsx',
          text: `
export function BrokenLanding() {
  return <main>
    <a href="/missing">Go nowhere</a>
    <a>Empty action</a>
    <button>Launch</button>
    <p>HIPAA-ready self-serve delivery is available now for $499.</p>
  </main>
}
`,
        },
      ],
      knownRoutes: ['/'],
      marketingCopy:
        'HIPAA-ready self-serve delivery is available now for $499.',
      previewUrl: 'https://preview.openagents.com/sites/broken',
      siteId: 'site_project_broken',
    })

    expect(receipt).toMatchObject({
      readyForDeployReview: false,
      registryValid: true,
      status: 'fail',
    })
    expect(receipt.blockerRefs).toEqual([
      'blocker.sites.generated.bundle_budget',
      'blocker.sites.generated.claim_safety',
      'blocker.sites.generated.dead_controls',
      'blocker.sites.generated.navigation_integrity',
    ])
    expect(
      lintGeneratedSiteMarketingClaims(
        'HIPAA-ready self-serve delivery is available now for $499.',
      ),
    ).toEqual([
      'claim_lint.self_serve_delivery',
      'claim_lint.hipaa_sovereign',
      'claim_lint.published_prices',
    ])
  })
})
