import { describe, expect, test } from 'vitest'

import {
  SITE_SPEED_BUDGETS,
  SITE_SPEED_ROUTE_TARGETS,
  evaluateSiteSpeedBudgets,
  parseArgs,
} from './site-speed-landing'

describe('site-speed landing harness configuration', () => {
  test('the funnel route set measures the canonical optimized business route', () => {
    const options = parseArgs(['--route-set', 'funnel', '--runs', '1'])

    expect(options.routeTargets.map(route => route.path)).toEqual(['/business'])
    expect(SITE_SPEED_ROUTE_TARGETS.business).toMatchObject({
      budgetProfile: 'funnel',
      class: 'funnel',
      path: '/business',
    })
  })

  test('budgets are encoded as data and evaluate baseline medians', () => {
    expect(SITE_SPEED_BUDGETS.map(budget => budget.budgetId)).toEqual([
      'ttfb.doc',
      'fcp',
      'lcp',
      'cls',
      'tbt',
      'long_task.max',
      'js.wire',
      'counter.value_rendered',
      'request.count',
    ])

    const evaluations = evaluateSiteSpeedBudgets('desktop-fast', {
      clsMilli: 0,
      counterValueRenderedMs: 400,
      fcpMs: 500,
      jsHeapUsedMb: 6,
      jsTransferKb: 80,
      lcpMs: 900,
      longTaskMaxMs: 75,
      requestCount: 8,
      scriptDurationMs: 50,
      tbtMs: 40,
      transferTotalKb: 90,
      ttfbMs: 120,
      webSocketConnectMs: 650,
    })

    expect(evaluations.every(evaluation => evaluation.status === 'pass')).toBe(true)
    expect(
      evaluateSiteSpeedBudgets('desktop-fast', {
        clsMilli: 0,
        counterValueRenderedMs: 400,
        fcpMs: 500,
        jsHeapUsedMb: 6,
        jsTransferKb: 80,
        lcpMs: 900,
        longTaskMaxMs: 75,
        requestCount: 30,
        scriptDurationMs: 50,
        tbtMs: 40,
        transferTotalKb: 90,
        ttfbMs: 120,
        webSocketConnectMs: 650,
      }).find(evaluation => evaluation.budgetId === 'request.count'),
    ).toMatchObject({ actual: 30, status: 'fail', threshold: 18 })
  })
})
