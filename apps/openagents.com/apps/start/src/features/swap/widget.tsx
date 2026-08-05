import {
  type SwapWidgetState,
  derivePrimaryAction,
} from '@openagentsinc/mkt-swp/view'
import type { Catalog } from '@openagentsinc/swap-i18n'
import type { ReactNode } from 'react'

import type { SwapSettings } from './settings'

/**
 * The swap widget shell (SWAP-0, openagents#9315) mounted into SWAP-7's
 * `/swap` route.
 *
 * All behaviour is in `@openagentsinc/mkt-swp`: the typed widget state, the
 * composition order over SWAP-1/2/3's gates, and the primary-action law.
 * This file is markup only — it renders the state the package computes and
 * mounts the sibling slots. It makes no refusal decision of its own, so the
 * law cannot drift between this surface and Omega's `market_ui`, which
 * renders the same exported view-model.
 */
export type SwapWidgetSlots = Readonly<{
  /** SWAP-1 (#9316): side selectors, direction toggle, MAX. */
  assetSelection?: ReactNode
  /** SWAP-1 (#9316): both-sides amount entry. */
  amountEntry?: ReactNode
  /** SWAP-1 (#9316): the fee-as-promise breakdown. */
  feePanel?: ReactNode
  /** SWAP-2 (#9317): address/invoice entry, QR, typed parse failures. */
  destinationEntry?: ReactNode
  /** SWAP-3 (#9318): quote compare and the verify-before-fund checklist. */
  quoteCompare?: ReactNode
  /** SWAP-4 (#9319): the pre-funding rescue ceremony. */
  rescueCeremony?: ReactNode
  /** SWAP-6 (#9321): per-signer status lanes, gaps, forks, rungs. */
  sessionStatus?: ReactNode
}>

const SLOT_ORDER = [
  'assetSelection',
  'amountEntry',
  'feePanel',
  'destinationEntry',
  'quoteCompare',
  'rescueCeremony',
  'sessionStatus',
] as const satisfies ReadonlyArray<keyof SwapWidgetSlots>

const TONE_CLASS = {
  accent: 'border-white bg-white text-black hover:bg-khala-text-muted',
  danger: 'border-red-500/70 bg-red-500/10 text-red-200',
  neutral: 'border-khala-border bg-khala-surface/60 text-khala-text-muted',
} as const

export function SwapWidget({
  catalog,
  settings,
  slots,
  state,
}: Readonly<{
  catalog: Catalog
  settings: SwapSettings
  slots?: SwapWidgetSlots
  state: SwapWidgetState
}>) {
  const action = derivePrimaryAction(
    state,
    catalog,
    settings.denomination,
    settings.decimalSeparator,
  )
  const filled = slots ?? {}

  return (
    <section
      className="grid gap-4 border border-khala-border bg-khala-surface/40 p-6"
      data-swap-widget=""
      data-swap-widget-state={state._tag}
    >
      {SLOT_ORDER.map(slot =>
        filled[slot] === undefined ? null : (
          <div data-swap-widget-slot={slot} key={slot}>
            {filled[slot]}
          </div>
        ),
      )}
      <button
        aria-busy={action.busy}
        className={[
          'w-full border px-4 py-3 text-center text-sm font-semibold',
          TONE_CLASS[action.tone],
          action.disabled ? 'cursor-not-allowed opacity-70' : '',
        ].join(' ')}
        data-swap-primary-action=""
        data-swap-primary-action-busy={String(action.busy)}
        data-swap-primary-action-key={action.messageKey}
        {...(action.swpError === null
          ? {}
          : { 'data-swap-primary-action-error': action.swpError })}
        disabled={action.disabled}
        type="button"
      >
        {action.label}
      </button>
    </section>
  )
}
