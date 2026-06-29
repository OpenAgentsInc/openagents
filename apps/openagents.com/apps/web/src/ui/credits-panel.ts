import { clsx } from 'clsx'
import type { Attribute, Html } from 'foldkit/html'
import { html } from 'foldkit/html'

import {
  button,
  eyebrowClass,
  kitFamily,
  metaClass,
  statusDotClass,
  surfaceClass,
  titleClass,
  toneTextClass,
} from '@openagentsinc/ui'
import type { Tone } from '@openagentsinc/ui'

// COORDINATOR WIRING: add `export * from './credits-panel'` to
// apps/web/src/ui/index.ts. Not added here to avoid colliding with the
// parallel workroom-page lane (#4977), which embeds creditsBalancePanel and
// costPreviewCard from this module.
//
// Self-contained credits + cost-preview UI component (#4985).
//
// Pure Foldkit/HTML builder functions over typed view inputs. No data
// fetching, no message wiring beyond optional caller-supplied attributes on
// the add-credits action. The workroom page (#4977) embeds these over the
// existing billing balance/rates data and the agent spend-cap-preview
// projection. Amounts are formatted by the caller; these functions only
// arrange and tone the already-formatted text.

// Mirror of the spend-cap preview status literals exposed by the worker
// projection (workers/api/src/agent-spend-cap-preview.ts). Kept local so the
// web bundle does not depend on the worker package.
export type CostPreviewStatus =
  | 'blocked'
  | 'catalog_missing'
  | 'exact_cap'
  | 'malformed_amount'
  | 'over_cap'
  | 'owner_grant_required'
  | 'private_route'
  | 'stale_catalog_entry'
  | 'unauthenticated_agent'
  | 'under_cap'
  | 'unsupported_rail'
  | 'wrong_currency'

// Mirror of the spend-cap preview next-action literals.
export type CostPreviewNextAction =
  | 'add_credits'
  | 'ask_owner_for_grant'
  | 'fix_catalog'
  | 'fix_currency'
  | 'lower_spend_or_raise_cap'
  | 'pay_l402_mdk'
  | 'provide_agent_token'
  | 'request_manual_review'
  | 'spend_internal_credits'
  | 'stop'
  | 'use_entitlement'
  | 'use_free_beta'

export type CreditsBalanceModel = Readonly<{
  // Already-formatted USD balance, e.g. "$42.50".
  balanceFormatted: string
  // Billing account status, e.g. "active" / "suspended".
  status: string
  // Pre-formatted rate labels, mirroring the billing page.
  containerRateLabel: string
  codexRateLabel: string
  // Pre-formatted minimum-credit-to-run threshold, e.g. "$1.00".
  minimumRunCreditFormatted: string
  // Raw balance in minor units (USD cents) used only to evaluate the
  // low-credit warning threshold. The displayed value is always
  // balanceFormatted.
  balanceMinorUnits: number
  // Minor-units threshold below which the low-credit warning shows. Defaults
  // to the minimum-run-credit floor when omitted.
  lowCreditThresholdMinorUnits?: number
}>

export type CostPreviewModel = Readonly<{
  // Human label for the action being previewed, e.g. "Run Codex turn".
  actionLabel: string
  status: CostPreviewStatus
  // Already-formatted projected cost for this call, e.g. "$0.40".
  priceFormatted: string
  // Already-formatted remaining window allowance, e.g. "$9.60".
  windowRemainingFormatted: string
  // Already-formatted per-window cap, e.g. "$10.00".
  maxPerWindowFormatted: string
  // Ordered next-action guidance from the projection.
  nextActions: ReadonlyArray<CostPreviewNextAction>
}>

const statusTone = (status: CostPreviewStatus): Tone =>
  status === 'under_cap'
    ? 'positive'
    : status === 'exact_cap'
      ? 'accent'
      : status === 'over_cap'
        ? 'warning'
        : 'negative'

const statusLabel = (status: CostPreviewStatus): string => {
  switch (status) {
    case 'under_cap':
      return 'Under cap'
    case 'exact_cap':
      return 'At cap'
    case 'over_cap':
      return 'Over cap'
    case 'blocked':
      return 'Blocked'
    case 'catalog_missing':
      return 'Catalog missing'
    case 'malformed_amount':
      return 'Malformed amount'
    case 'owner_grant_required':
      return 'Owner grant required'
    case 'private_route':
      return 'Private route'
    case 'stale_catalog_entry':
      return 'Stale catalog entry'
    case 'unauthenticated_agent':
      return 'Unauthenticated agent'
    case 'unsupported_rail':
      return 'Unsupported rail'
    case 'wrong_currency':
      return 'Wrong currency'
  }
}

const nextActionGuidance = (action: CostPreviewNextAction): string => {
  switch (action) {
    case 'add_credits':
      return 'Add credits to cover this action.'
    case 'ask_owner_for_grant':
      return 'Ask the owner for a spend grant.'
    case 'fix_catalog':
      return 'Fix the product catalog entry.'
    case 'fix_currency':
      return 'Fix the price currency or denomination.'
    case 'lower_spend_or_raise_cap':
      return 'Lower the spend or raise the cap.'
    case 'pay_l402_mdk':
      return 'Pay the L402 invoice over MDK.'
    case 'provide_agent_token':
      return 'Provide a valid agent token.'
    case 'request_manual_review':
      return 'Request manual operator review.'
    case 'spend_internal_credits':
      return 'Spend internal credits.'
    case 'stop':
      return 'Stop. This action cannot proceed.'
    case 'use_entitlement':
      return 'Use an existing entitlement.'
    case 'use_free_beta':
      return 'Use the free beta allowance.'
  }
}

export const isLowCredit = (model: CreditsBalanceModel): boolean =>
  model.lowCreditThresholdMinorUnits === undefined
    ? false
    : model.balanceMinorUnits < model.lowCreditThresholdMinorUnits

// Balance + rates panel with an optional low-credit warning row. The optional
// add-credits action attrs let an embedding page wire a message; omit them for
// a read-only display.
export const creditsBalancePanel = <Message>(
  model: CreditsBalanceModel,
  options: {
    addCreditsAttrs?: ReadonlyArray<Attribute<Message>>
  } = {},
): Html => {
  const h = html<Message>()
  const balanceTone: Tone =
    model.status === 'active' && !isLowCredit(model) ? 'positive' : 'warning'
  const low = isLowCredit(model)

  return h.section(
    [
      kitFamily<Message>('data-display/stats'),
      h.Class(clsx(surfaceClass, 'grid gap-4 p-4')),
      h.DataAttribute('credits-balance-panel', ''),
    ],
    [
      h.div(
        [h.Class('grid gap-1')],
        [
          h.p([h.Class(eyebrowClass)], ['Credits balance']),
          h.p(
            [
              h.Class(
                clsx(
                  'm-0 text-3xl font-semibold tabular-nums',
                  toneTextClass(balanceTone),
                ),
              ),
              h.DataAttribute('credits-balance-value', ''),
            ],
            [model.balanceFormatted],
          ),
          h.p(
            [h.Class(metaClass)],
            [`Account ${model.status}`],
          ),
        ],
      ),
      h.div(
        [h.Class('grid gap-2 sm:grid-cols-2')],
        [
          h.div(
            [h.Class('grid gap-0.5')],
            [
              h.p([h.Class(eyebrowClass)], ['Computer time']),
              h.p(
                [h.Class(clsx(titleClass, toneTextClass('accent')))],
                [model.containerRateLabel],
              ),
            ],
          ),
          h.div(
            [h.Class('grid gap-0.5')],
            [
              h.p([h.Class(eyebrowClass)], ['Codex usage']),
              h.p(
                [h.Class(clsx(titleClass, toneTextClass('info')))],
                [model.codexRateLabel],
              ),
            ],
          ),
        ],
      ),
      low
        ? h.div(
            [
              kitFamily<Message>('feedback/alerts'),
              h.Class(
                'grid grid-cols-[auto_minmax(0,1fr)] gap-2 border border-[#ff6f00]/70 p-3 text-sm leading-5 text-[#ff6f00]',
              ),
              h.DataAttribute('credits-low-warning', ''),
            ],
            [
              h.span([h.Class(statusDotClass('warning'))], []),
              h.span(
                [h.Class('min-w-0')],
                [
                  `Low credit. New runs require at least ${model.minimumRunCreditFormatted} available.`,
                ],
              ),
            ],
          )
        : null,
      options.addCreditsAttrs === undefined
        ? null
        : button<Message>({
            label: 'Add credits',
            size: 'sm',
            variant: low ? 'primary' : 'secondary',
            block: true,
            attrs: options.addCreditsAttrs,
          }),
      h.p(
        [h.Class(metaClass)],
        [`Minimum to run: ${model.minimumRunCreditFormatted}`],
      ),
    ],
  )
}

// Cost-preview card: projected cost + status + next-action guidance for a
// single previewed action.
export const costPreviewCard = <Message>(preview: CostPreviewModel): Html => {
  const h = html<Message>()
  const tone = statusTone(preview.status)
  const actions =
    preview.nextActions.length === 0 ? (['stop'] as const) : preview.nextActions

  return h.section(
    [
      kitFamily<Message>('forms/action-panels'),
      h.Class(clsx(surfaceClass, 'grid gap-3 p-4')),
      h.DataAttribute('cost-preview-card', ''),
      h.DataAttribute('cost-preview-status', preview.status),
    ],
    [
      h.div(
        [h.Class('flex min-w-0 items-center justify-between gap-3')],
        [
          h.div(
            [h.Class('min-w-0')],
            [
              h.p([h.Class(eyebrowClass)], ['Cost preview']),
              h.p([h.Class(titleClass)], [preview.actionLabel]),
            ],
          ),
          h.div(
            [h.Class(clsx('flex flex-none items-center gap-2', toneTextClass(tone)))],
            [
              h.span([h.Class(statusDotClass(tone))], []),
              h.span(
                [h.Class('text-xs font-medium uppercase tracking-[0.08em]')],
                [statusLabel(preview.status)],
              ),
            ],
          ),
        ],
      ),
      h.div(
        [h.Class('grid gap-2 sm:grid-cols-3')],
        [
          h.div(
            [h.Class('grid gap-0.5')],
            [
              h.p([h.Class(eyebrowClass)], ['Projected cost']),
              h.p(
                [
                  h.Class(clsx('m-0 text-lg font-semibold tabular-nums text-[#f1efe8]')),
                  h.DataAttribute('cost-preview-price', ''),
                ],
                [preview.priceFormatted],
              ),
            ],
          ),
          h.div(
            [h.Class('grid gap-0.5')],
            [
              h.p([h.Class(eyebrowClass)], ['Window remaining']),
              h.p(
                [h.Class(clsx(titleClass, 'tabular-nums'))],
                [preview.windowRemainingFormatted],
              ),
            ],
          ),
          h.div(
            [h.Class('grid gap-0.5')],
            [
              h.p([h.Class(eyebrowClass)], ['Per-window cap']),
              h.p(
                [h.Class(clsx(metaClass, 'tabular-nums'))],
                [preview.maxPerWindowFormatted],
              ),
            ],
          ),
        ],
      ),
      h.div(
        [
          h.Class('grid gap-1'),
          h.DataAttribute('cost-preview-next-actions', ''),
        ],
        [
          h.p([h.Class(eyebrowClass)], ['Next action']),
          ...actions.map(action =>
            h.p(
              [
                h.Class('m-0 text-sm leading-5 text-white/70'),
                h.DataAttribute('cost-preview-next-action', action),
              ],
              [nextActionGuidance(action)],
            ),
          ),
        ],
      ),
    ],
  )
}
