import { describe, expect, it } from 'vitest'

import { estimateBudgetCapacity } from './budget-estimate'
import { estimateRequestCost } from './cost-estimate'
import { usdCentsToMsatFloor } from './usd-msat-conversion'

const SHAPE = {
  completionTokens: 500,
  model: 'sonnet',
  promptTokens: 1000,
} as const

describe('estimateBudgetCapacity', () => {
  it('embeds the SAME per-request estimate the forward quote returns', () => {
    const forward = estimateRequestCost({ ...SHAPE, fundingKind: 'card' })
    const budget = estimateBudgetCapacity({
      ...SHAPE,
      budgetCredits: 1000,
      fundingKind: 'card',
    })
    // The two surfaces cannot disagree on the per-request price.
    expect(budget.perRequest).toEqual(forward)
    expect(budget.isEstimate).toBe(true)
  })

  it('affords exactly floor(budget / per-request cost) whole requests', () => {
    const forward = estimateRequestCost({ ...SHAPE, fundingKind: 'card' })
    const budgetCredits = forward.estimatedCredits * 10
    const budget = estimateBudgetCapacity({
      ...SHAPE,
      budgetCredits,
      fundingKind: 'card',
    })
    expect(budget.affordableRequests).toBe(10)
    expect(budget.affordableRequestsUnbounded).toBe(false)
  })

  it('floors a fractional budget down to whole requests with leftover', () => {
    const forward = estimateRequestCost({ ...SHAPE, fundingKind: 'card' })
    // 3.5 requests' worth of budget -> 3 affordable, ~0.5 left over.
    const budgetCredits = forward.estimatedCredits * 3.5
    const budget = estimateBudgetCapacity({
      ...SHAPE,
      budgetCredits,
      fundingKind: 'card',
    })
    expect(budget.affordableRequests).toBe(3)
    expect(budget.leftoverCredits).toBeGreaterThan(0)
    // spent + leftover reconciles to the budget.
    expect(budget.spentCredits + budget.leftoverCredits).toBeCloseTo(
      budget.budgetCredits,
      4,
    )
  })

  it('totals tokens across all affordable requests', () => {
    const forward = estimateRequestCost({ ...SHAPE, fundingKind: 'card' })
    const budget = estimateBudgetCapacity({
      ...SHAPE,
      budgetCredits: forward.estimatedCredits * 4,
      fundingKind: 'card',
    })
    expect(budget.totalPromptTokens).toBe(4 * forward.promptTokens)
    expect(budget.totalCompletionTokens).toBe(4 * forward.completionTokens)
    expect(budget.totalTokens).toBe(
      4 * (forward.promptTokens + forward.completionTokens),
    )
  })

  it('affords MORE requests on the Bitcoin rail for the same budget', () => {
    const common = {
      ...SHAPE,
      budgetCredits: 100_000,
    } as const
    const card = estimateBudgetCapacity({ ...common, fundingKind: 'card' })
    const bitcoin = estimateBudgetCapacity({ ...common, fundingKind: 'bitcoin' })
    expect(bitcoin.affordableRequests).toBeGreaterThan(card.affordableRequests)
  })

  it('affords 0 requests when the budget cannot cover one', () => {
    const forward = estimateRequestCost({ ...SHAPE, fundingKind: 'card' })
    const budget = estimateBudgetCapacity({
      ...SHAPE,
      budgetCredits: forward.estimatedCredits / 2,
      fundingKind: 'card',
    })
    expect(budget.affordableRequests).toBe(0)
    expect(budget.spentCredits).toBe(0)
    expect(budget.leftoverCredits).toBeCloseTo(budget.budgetCredits, 4)
  })

  it('maps the credit budget to spendable msat at the grant-floor rate', () => {
    const budget = estimateBudgetCapacity({
      ...SHAPE,
      budgetCredits: 2500,
      fundingKind: 'card',
    })
    // 1 credit = 1 cent, converted with the same floor the bridge grants with.
    expect(budget.budgetMsat).toBe(usdCentsToMsatFloor(2500))
    expect(budget.budgetUsd).toBeCloseTo(25, 6)
  })

  it('clamps a negative / NaN budget to zero affordability', () => {
    const budget = estimateBudgetCapacity({
      ...SHAPE,
      budgetCredits: Number.NaN,
      fundingKind: 'card',
    })
    expect(budget.budgetCredits).toBe(0)
    expect(budget.budgetMsat).toBe(0)
    expect(budget.affordableRequests).toBe(0)
  })

  it('flags an unbounded (zero-cost) representative request', () => {
    const budget = estimateBudgetCapacity({
      budgetCredits: 1000,
      completionTokens: 0,
      fundingKind: 'card',
      model: 'sonnet',
      promptTokens: 0,
    })
    expect(budget.perRequest.estimatedCredits).toBe(0)
    expect(budget.affordableRequestsUnbounded).toBe(true)
    expect(budget.affordableRequests).toBe(0)
    expect(budget.leftoverCredits).toBeCloseTo(budget.budgetCredits, 4)
  })

  it('carries the unknown-model and free-tier flags from the per-request quote', () => {
    const unknown = estimateBudgetCapacity({
      budgetCredits: 1000,
      completionTokens: 100,
      fundingKind: 'card',
      model: 'no-such-model-xyz',
      promptTokens: 100,
    })
    expect(unknown.perRequest.isUnknownModel).toBe(true)

    const free = estimateBudgetCapacity({
      budgetCredits: 1000,
      completionTokens: 100,
      fundingKind: 'card',
      model: 'gemini-3.5-flash',
      promptTokens: 100,
    })
    expect(free.perRequest.freeTierEligible).toBe(true)
  })
})
