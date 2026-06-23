import { describe, expect, test } from 'vitest'

import {
  AUTOPILOT_CONCIERGE_TOOLS,
  AUTOPILOT_CONCIERGE_TOOL_IDS,
  AUTOPILOT_CONCIERGE_TOOLS_PROMPT,
  autopilotConciergeToolDeclarations,
  getAutopilotConciergeTool,
  isAutopilotConciergeTool,
  runConciergeTool,
} from './autopilot-concierge-tools'

describe('Autopilot Concierge tool registry (seam)', () => {
  test('declares the bounded, closed tool set', () => {
    expect(AUTOPILOT_CONCIERGE_TOOL_IDS).toEqual([
      'web_search_enrichment',
      'prefilled_workspace_seeding',
      'checkout_credit_kickoff',
      'crm_write',
    ])
    expect(Object.keys(AUTOPILOT_CONCIERGE_TOOLS).sort()).toEqual(
      [...AUTOPILOT_CONCIERGE_TOOL_IDS].sort(),
    )
  })

  test('mutating and spending tools are human-review-gated; read tools are not', () => {
    expect(
      getAutopilotConciergeTool('prefilled_workspace_seeding')?.humanReviewGated,
    ).toBe(true)
    expect(
      getAutopilotConciergeTool('checkout_credit_kickoff')?.humanReviewGated,
    ).toBe(true)
    expect(getAutopilotConciergeTool('crm_write')?.humanReviewGated).toBe(true)
    expect(
      getAutopilotConciergeTool('web_search_enrichment')?.humanReviewGated,
    ).toBe(false)
    expect(getAutopilotConciergeTool('checkout_credit_kickoff')?.effectClass).toBe(
      'spend',
    )
  })

  test('rejects an unknown tool id', () => {
    expect(isAutopilotConciergeTool('not_a_tool')).toBe(false)
    expect(getAutopilotConciergeTool('not_a_tool')).toBeUndefined()
    expect(runConciergeTool('not_a_tool', {})).toEqual({
      status: 'unknown_tool',
      tool: 'not_a_tool',
    })
  })

  test('validates args against the typed schema before any (deferred) execution', () => {
    // Bad args => invalid_args, never a side effect.
    expect(
      runConciergeTool('checkout_credit_kickoff', { amountCents: -1 }).status,
    ).toBe('invalid_args')
    expect(
      runConciergeTool('web_search_enrichment', { query: '' }).status,
    ).toBe('invalid_args')
  })

  test('execution is DEFERRED: a well-formed call returns not_implemented (no side effect)', () => {
    expect(
      runConciergeTool('checkout_credit_kickoff', {
        amountCents: 50_000,
        label: 'Kick off with $500',
      }),
    ).toEqual({ status: 'not_implemented', tool: 'checkout_credit_kickoff' })
    expect(
      runConciergeTool('web_search_enrichment', { query: 'acme llc' }),
    ).toEqual({ status: 'not_implemented', tool: 'web_search_enrichment' })
  })

  test('declarations surface honest, agent-readable status (no args, no secrets)', () => {
    const declarations = autopilotConciergeToolDeclarations()
    expect(declarations.map(d => d.id)).toEqual([
      ...AUTOPILOT_CONCIERGE_TOOL_IDS,
    ])
    expect(declarations.every(d => d.status === 'declared_not_executed')).toBe(
      true,
    )
  })

  test('the tools prompt tells the model the set is declared, not live', () => {
    expect(AUTOPILOT_CONCIERGE_TOOLS_PROMPT).toContain('not yet executable')
    expect(AUTOPILOT_CONCIERGE_TOOLS_PROMPT).toContain('web_search_enrichment')
  })
})
