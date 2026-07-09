import { describe, expect, it } from 'vitest'

import { liveAtReadStaleness } from '../public-projection-staleness'
import {
  KHALA_CODE_FREE_PLAN_ID,
  KHALA_CODE_PAID_PLANS_ENABLED_ENV_KEY,
  KHALA_CODE_PAID_PLAN_ID,
  KHALA_CODE_PLAN_PROMISE_ID,
  KHALA_CODE_PLAN_PURCHASE_ROUTE,
  isKhalaCodePaidPlansEnabled,
  khalaCodePlanCatalog,
} from './khala-code-plan-catalog'

const freshness = () => ({
  generatedAt: '2026-07-01T00:00:00.000Z',
  staleness: liveAtReadStaleness([
    'module:khala-code-plan-catalog.ts',
    'env:KHALA_CODE_PAID_PLANS_ENABLED',
  ]),
})

describe('khalaCodePlanCatalog', () => {
  it('projects the two-plan structure with the free plan as default', () => {
    const catalog = khalaCodePlanCatalog({ ...freshness(), paidPlanPurchaseArmed: false })

    expect(catalog.schemaVersion).toBe('openagents.khala_code.plan_catalog.v1')
    expect(catalog.promiseId).toBe(KHALA_CODE_PLAN_PROMISE_ID)
    expect(catalog.plans).toHaveLength(2)

    const free = catalog.plans.find(plan => plan.kind === 'free')
    const paid = catalog.plans.find(plan => plan.kind === 'paid')
    expect(free?.planId).toBe(KHALA_CODE_FREE_PLAN_ID)
    expect(free?.isDefault).toBe(true)
    expect(free?.captureExcluded).toBe(false)
    expect(paid?.planId).toBe(KHALA_CODE_PAID_PLAN_ID)
    expect(paid?.isDefault).toBe(false)
    expect(paid?.captureExcluded).toBe(true)
  })

  it('reports the paid plan as retired and not purchasable', () => {
    const catalog = khalaCodePlanCatalog({ ...freshness(), paidPlanPurchaseArmed: false })
    const paid = catalog.plans.find(plan => plan.kind === 'paid')

    expect(paid?.priceLabel).toBe('Retired — not purchasable')
    expect(paid?.purchase).toEqual({
      armed: false,
      envFlag: KHALA_CODE_PAID_PLANS_ENABLED_ENV_KEY,
      route: KHALA_CODE_PLAN_PURCHASE_ROUTE,
    })
    expect(catalog.blockerRefs).toContain(
      'blocker.product_promises.khala_code_paid_plan_not_purchasable',
    )
    expect(catalog.blockerRefs).toContain(
      'blocker.product_promises.khala_code_product_retired',
    )
  })

  it('ignores a legacy armed input and stays retired', () => {
    const catalog = khalaCodePlanCatalog({ ...freshness(), paidPlanPurchaseArmed: true })
    const paid = catalog.plans.find(plan => plan.kind === 'paid')

    expect(paid?.purchase?.armed).toBe(false)
    expect(paid?.priceLabel).toContain('Retired')
    expect(catalog.summary).toContain('cannot rearm')
  })

  it('keeps the copy honest: no live-capture or purchasable claims', () => {
    const catalog = khalaCodePlanCatalog({ ...freshness(), paidPlanPurchaseArmed: false })
    const text = JSON.stringify(catalog)

    // The launch-copy discipline for khala_code.free_paid_plans.v1: the
    // catalog must carry the not-live framing, cite the adjacent promises,
    // and grant no authority.
    expect(text).toContain('RETIRED and not purchasable')
    expect(text).toContain('NOT captured for training today')
    expect(catalog.relatedPromiseIds).toContain(
      'khala_code.free_plan_trace_capture.v1',
    )
    expect(catalog.relatedPromiseIds).toContain(
      'privacy.khala_paid_capture_optout.v1',
    )
    expect(catalog.authorityBoundary).toContain('grants no capture')
  })
})

describe('isKhalaCodePaidPlansEnabled', () => {
  it('is permanently fail-closed after product retirement', () => {
    expect(isKhalaCodePaidPlansEnabled(undefined)).toBe(false)
    expect(isKhalaCodePaidPlansEnabled('')).toBe(false)
    expect(isKhalaCodePaidPlansEnabled('false')).toBe(false)
    expect(isKhalaCodePaidPlansEnabled('0')).toBe(false)
    expect(isKhalaCodePaidPlansEnabled(1)).toBe(false)
    expect(isKhalaCodePaidPlansEnabled('true')).toBe(false)
    expect(isKhalaCodePaidPlansEnabled(' ON ')).toBe(false)
    expect(isKhalaCodePaidPlansEnabled('1')).toBe(false)
  })
})
