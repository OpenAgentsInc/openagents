import type { Html } from 'foldkit/html'
import { describe, expect, it } from 'vitest'

import {
  type CostPreviewModel,
  type CostPreviewNextAction,
  type CostPreviewStatus,
  type CreditsBalanceModel,
  costPreviewCard,
  creditsBalancePanel,
  isLowCredit,
} from './credits-panel'

type VNodeLike = Readonly<{
  sel?: string
  text?: string
  children?: ReadonlyArray<VNodeLike | string | null>
  data?: {
    attrs?: Record<string, unknown>
    props?: Record<string, unknown>
    class?: Record<string, boolean>
  }
}>

const isVNodeLike = (value: unknown): value is VNodeLike =>
  typeof value === 'object' && value !== null

const attrsToString = (node: VNodeLike): string => {
  const attrs = node.data?.attrs ?? {}
  const props = node.data?.props ?? {}
  const classes = Object.entries(node.data?.class ?? {})
    .filter(([, enabled]) => enabled)
    .map(([className]) => className)
    .join(' ')
  const pairs = [
    ...Object.entries(attrs),
    ...Object.entries(props),
    ...(classes.length === 0 ? [] : [['class', classes] as const]),
  ]

  return pairs
    .filter(
      ([, value]) => value !== false && value !== undefined && value !== null,
    )
    .map(([name, value]) =>
      value === true ? ` ${name}` : ` ${name}="${String(value)}"`,
    )
    .join('')
}

const renderHtml = (html: Html): string => {
  if (html === null || !isVNodeLike(html)) {
    return ''
  }

  const tag = html.sel ?? 'node'
  const children = (html.children ?? [])
    .map(child =>
      typeof child === 'string'
        ? child
        : child === null
          ? ''
          : renderHtml(child),
    )
    .join('')
  const text = html.text ?? ''

  return `<${tag}${attrsToString(html)}>${text}${children}</${tag}>`
}

const baseBalance: CreditsBalanceModel = {
  balanceFormatted: '$42.50',
  status: 'active',
  containerRateLabel: '$0.05/min',
  codexRateLabel: '$0.02/1k',
  minimumRunCreditFormatted: '$1.00',
  balanceMinorUnits: 4250,
  lowCreditThresholdMinorUnits: 100,
}

const basePreview = (
  status: CostPreviewStatus,
  nextActions: ReadonlyArray<CostPreviewNextAction>,
): CostPreviewModel => ({
  actionLabel: 'Run Codex turn',
  status,
  priceFormatted: '$0.40',
  windowRemainingFormatted: '$9.60',
  maxPerWindowFormatted: '$10.00',
  nextActions,
})

describe('creditsBalancePanel', () => {
  it('renders the formatted balance and rate labels', () => {
    const rendered = renderHtml(creditsBalancePanel(baseBalance))

    expect(rendered).toContain('data-credits-balance-panel=""')
    expect(rendered).toContain('$42.50')
    expect(rendered).toContain('Account active')
    expect(rendered).toContain('$0.05/min')
    expect(rendered).toContain('$0.02/1k')
    expect(rendered).toContain('Minimum to run: $1.00')
  })

  it('does not show a low-credit warning when above the threshold', () => {
    expect(isLowCredit(baseBalance)).toBe(false)
    const rendered = renderHtml(creditsBalancePanel(baseBalance))
    expect(rendered).not.toContain('data-credits-low-warning=""')
    expect(rendered).not.toContain('Low credit.')
  })

  it('shows a low-credit warning when balance is below the threshold', () => {
    const low: CreditsBalanceModel = {
      ...baseBalance,
      balanceMinorUnits: 50,
    }
    expect(isLowCredit(low)).toBe(true)

    const rendered = renderHtml(creditsBalancePanel(low))
    expect(rendered).toContain('data-credits-low-warning=""')
    expect(rendered).toContain(
      'Low credit. New runs require at least $1.00 available.',
    )
  })

  it('treats balance exactly at the threshold as not low', () => {
    const atThreshold: CreditsBalanceModel = {
      ...baseBalance,
      balanceMinorUnits: 100,
    }
    expect(isLowCredit(atThreshold)).toBe(false)
  })

  it('never warns when no threshold is configured', () => {
    const noThreshold: CreditsBalanceModel = {
      balanceFormatted: '$0.00',
      status: 'active',
      containerRateLabel: '$0.05/min',
      codexRateLabel: '$0.02/1k',
      minimumRunCreditFormatted: '$1.00',
      balanceMinorUnits: 0,
    }
    expect(isLowCredit(noThreshold)).toBe(false)
    const rendered = renderHtml(creditsBalancePanel(noThreshold))
    expect(rendered).not.toContain('data-credits-low-warning=""')
  })

  it('omits the add-credits button unless action attrs are supplied', () => {
    const without = renderHtml(creditsBalancePanel(baseBalance))
    expect(without).not.toContain('Add credits')

    const withAction = renderHtml(
      creditsBalancePanel(baseBalance, { addCreditsAttrs: [] }),
    )
    expect(withAction).toContain('Add credits')
  })

  it('produces deterministic output for the same model', () => {
    expect(renderHtml(creditsBalancePanel(baseBalance))).toBe(
      renderHtml(creditsBalancePanel(baseBalance)),
    )
  })
})

describe('costPreviewCard', () => {
  it('renders the action label, projected cost, and window figures', () => {
    const rendered = renderHtml(
      costPreviewCard(basePreview('under_cap', ['spend_internal_credits'])),
    )

    expect(rendered).toContain('data-cost-preview-card=""')
    expect(rendered).toContain('data-cost-preview-status="under_cap"')
    expect(rendered).toContain('Run Codex turn')
    expect(rendered).toContain('$0.40')
    expect(rendered).toContain('$9.60')
    expect(rendered).toContain('$10.00')
  })

  const statusCases: ReadonlyArray<
    readonly [
      CostPreviewStatus,
      ReadonlyArray<CostPreviewNextAction>,
      string,
      string,
    ]
  > = [
    [
      'under_cap',
      ['spend_internal_credits'],
      'Under cap',
      'Spend internal credits.',
    ],
    ['exact_cap', ['use_entitlement'], 'At cap', 'Use an existing entitlement.'],
    [
      'over_cap',
      ['lower_spend_or_raise_cap'],
      'Over cap',
      'Lower the spend or raise the cap.',
    ],
    [
      'blocked',
      ['request_manual_review'],
      'Blocked',
      'Request manual operator review.',
    ],
    [
      'catalog_missing',
      ['fix_catalog'],
      'Catalog missing',
      'Fix the product catalog entry.',
    ],
    [
      'malformed_amount',
      ['fix_currency'],
      'Malformed amount',
      'Fix the price currency or denomination.',
    ],
    [
      'owner_grant_required',
      ['ask_owner_for_grant'],
      'Owner grant required',
      'Ask the owner for a spend grant.',
    ],
    ['private_route', ['stop'], 'Private route', 'Stop. This action cannot proceed.'],
    [
      'stale_catalog_entry',
      ['fix_catalog'],
      'Stale catalog entry',
      'Fix the product catalog entry.',
    ],
    [
      'unauthenticated_agent',
      ['provide_agent_token'],
      'Unauthenticated agent',
      'Provide a valid agent token.',
    ],
    ['unsupported_rail', ['stop'], 'Unsupported rail', 'Stop. This action cannot proceed.'],
    [
      'wrong_currency',
      ['fix_currency'],
      'Wrong currency',
      'Fix the price currency or denomination.',
    ],
  ]

  it.each(statusCases)(
    'renders %s with its status label and next-action guidance',
    (status, nextActions, label, guidance) => {
      const rendered = renderHtml(costPreviewCard(basePreview(status, nextActions)))

      expect(rendered).toContain(`data-cost-preview-status="${status}"`)
      expect(rendered).toContain(label)
      expect(rendered).toContain(guidance)
      expect(rendered).toContain(
        `data-cost-preview-next-action="${nextActions[0]}"`,
      )
    },
  )

  it('renders guidance for each of the other next actions', () => {
    const checks: ReadonlyArray<readonly [CostPreviewNextAction, string]> = [
      ['add_credits', 'Add credits to cover this action.'],
      ['pay_l402_mdk', 'Pay the L402 invoice over MDK.'],
      ['use_free_beta', 'Use the free beta allowance.'],
    ]

    for (const [action, guidance] of checks) {
      const rendered = renderHtml(
        costPreviewCard(basePreview('under_cap', [action])),
      )
      expect(rendered).toContain(guidance)
      expect(rendered).toContain(`data-cost-preview-next-action="${action}"`)
    }
  })

  it('falls back to a stop guidance line when no next actions are given', () => {
    const rendered = renderHtml(costPreviewCard(basePreview('blocked', [])))
    expect(rendered).toContain('data-cost-preview-next-action="stop"')
    expect(rendered).toContain('Stop. This action cannot proceed.')
  })

  it('renders every supplied next action in order', () => {
    const rendered = renderHtml(
      costPreviewCard(
        basePreview('under_cap', ['spend_internal_credits', 'use_entitlement']),
      ),
    )
    expect(rendered).toContain(
      'data-cost-preview-next-action="spend_internal_credits"',
    )
    expect(rendered).toContain('data-cost-preview-next-action="use_entitlement"')
    expect(
      rendered.indexOf('spend_internal_credits') <
        rendered.indexOf('use_entitlement'),
    ).toBe(true)
  })

  it('produces deterministic output for the same preview', () => {
    const preview = basePreview('over_cap', ['lower_spend_or_raise_cap'])
    expect(renderHtml(costPreviewCard(preview))).toBe(
      renderHtml(costPreviewCard(preview)),
    )
  })
})
