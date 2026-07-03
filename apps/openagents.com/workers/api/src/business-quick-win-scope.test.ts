import { describe, expect, test } from 'vitest'

import {
  buildBusinessQuickWinReceipt,
} from './business-quick-win-receipt'
import {
  publicQuickWinScopeProjection,
  QuickWinScopeInvariantError,
  scopeQuickWinFromIntake,
  type QuickWinScopeInput,
} from './business-quick-win-scope'

const codingIntake: QuickWinScopeInput = {
  signupId: 'business_signup_abc123',
  helpWith: 'Please fix our failing checkout test suite and refactor the module.',
}

describe('scopeQuickWinFromIntake', () => {
  test('routes a coding request to the coding quick-win offering', () => {
    const scope = scopeQuickWinFromIntake(codingIntake)
    expect(scope.category).toBe('coding_agent_work')
    expect(scope.offeringPromiseId).toBe('business.coding_quick_win.v1')
    expect(scope.availability).toBe('available_now')
    expect(scope.unmatched).toBe(false)
  })

  test('routes a batch request before generic inference', () => {
    const scope = scopeQuickWinFromIntake({
      signupId: 'business_signup_b',
      helpWith: 'Run a batch of classifications through a model.',
    })
    expect(scope.category).toBe('inference_batch')
    expect(scope.offeringPromiseId).toBe('inference.batch_processing_jobs.v1')
    expect(scope.availability).toBe('roadmap')
    expect(scope.deliveryMode).toBe('not_deliverable')
  })

  test('routes a generic inference request to the free taste offering', () => {
    const scope = scopeQuickWinFromIntake({
      signupId: 'business_signup_c',
      helpWith: 'Wire a Gemini model behind one of our internal tools.',
    })
    expect(scope.category).toBe('inference_ai')
    expect(scope.offeringPromiseId).toBe('inference.free_tier_taste.v1')
  })

  test('routes vertical-pack keywords to their packs', () => {
    expect(
      scopeQuickWinFromIntake({
        signupId: 's1',
        helpWith: 'inventory-aware ad campaigns for our store',
      }).category,
    ).toBe('ecommerce_workspace')
    expect(
      scopeQuickWinFromIntake({
        signupId: 's2',
        helpWith: 'a client intake form for our legal practice',
      }).category,
    ).toBe('legal_workspace')
    expect(
      scopeQuickWinFromIntake({
        signupId: 's3',
        helpWith: 'white-label marketing campaign pages',
      }).category,
    ).toBe('marketing_workspace')

    const marketingProgramScope = scopeQuickWinFromIntake({
      signupId: 's3b',
      helpWith: 'GEO content and outbound assist as a monthly marketing program',
    })
    expect(marketingProgramScope.category).toBe('marketing_workspace')
    expect(marketingProgramScope.definitionOfDone).toContain(
      'Marketing program package scoped across content, GEO, and outbound assist as applicable.',
    )
  })

  test('routes a site request to the site build offering', () => {
    const scope = scopeQuickWinFromIntake({
      signupId: 's4',
      helpWith: 'We need a branded landing page with a welcome email sequence.',
    })
    expect(scope.category).toBe('sites_commerce')
    expect(scope.offeringPromiseId).toBe('autopilot_sites.site_build_and_host.v1')
  })

  test('unmatched request routes to operator triage, not a force-fit offering', () => {
    const scope = scopeQuickWinFromIntake({
      signupId: 's5',
      helpWith: 'Can you cater lunch for our office party?',
    })
    expect(scope.category).toBe('unmatched_operator_triage')
    expect(scope.unmatched).toBe(true)
    expect(scope.deliveryMode).toBe('operator_assisted')
  })

  test('blank help text routes to operator triage', () => {
    const scope = scopeQuickWinFromIntake({ signupId: 's6', helpWith: null })
    expect(scope.unmatched).toBe(true)
    expect(scope.requestedHelp).toBe('')
    expect(scope.category).toBe('unmatched_operator_triage')
  })

  test('no current route claims self_serve delivery (honesty: blocker open)', () => {
    const samples: ReadonlyArray<string | null> = [
      'fix a bug',
      'run a batch of extractions',
      'wire a model',
      'inventory campaigns',
      'legal forms',
      'marketing pages',
      'GEO and outbound assist',
      'a landing site',
      'fine-tune a model',
      null,
    ]
    for (const helpWith of samples) {
      const scope = scopeQuickWinFromIntake({ signupId: 'sx', helpWith })
      expect(scope.deliveryMode).not.toBe('self_serve')
      expect(scope.needsOperator).toBe(true)
    }
  })

  test('is deterministic: identical input yields identical scope', () => {
    expect(scopeQuickWinFromIntake(codingIntake)).toEqual(
      scopeQuickWinFromIntake(codingIntake),
    )
  })

  test('normalizes whitespace in the requested help text', () => {
    const scope = scopeQuickWinFromIntake({
      signupId: 's7',
      helpWith: '  fix   a    failing   test  ',
    })
    expect(scope.requestedHelp).toBe('fix a failing test')
  })

  test('rejects an empty signupId', () => {
    expect(() =>
      scopeQuickWinFromIntake({ signupId: '   ', helpWith: 'fix a bug' }),
    ).toThrow(QuickWinScopeInvariantError)
  })

  test('quickWinScopedRef is stable and feeds the receipt scoped line', () => {
    const scope = scopeQuickWinFromIntake(codingIntake)
    expect(scope.quickWinScopedRef).toBe(
      'quick-win-scope:business_signup_abc123:coding_agent_work',
    )

    const receipt = buildBusinessQuickWinReceipt({
      signupId: scope.signupId,
      offeringPromiseId: scope.offeringPromiseId,
      quickWinSummary: scope.requestedHelp,
      quickWinScopedRef: scope.quickWinScopedRef,
    })
    const scopedLine = receipt.lines.find(
      line => line.stateId === 'quick_win_scoped',
    )
    expect(scopedLine?.evidenceState).toBe('evidenced')
    expect(scopedLine?.evidenceRef).toBe(scope.quickWinScopedRef)
    expect(receipt.paidQuickWin).toBe(false)
  })
})

describe('publicQuickWinScopeProjection', () => {
  test('drops the raw request text but keeps the routing decision', () => {
    const scope = scopeQuickWinFromIntake(codingIntake)
    const projection = publicQuickWinScopeProjection(scope)
    expect(projection).not.toHaveProperty('requestedHelp')
    expect(projection).not.toHaveProperty('signupId')
    expect(projection.offeringPromiseId).toBe('business.coding_quick_win.v1')
    expect(projection.deliveryMode).toBe('operator_assisted')
  })
})
