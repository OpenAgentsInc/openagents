import type { IntentReporter } from '@effect-native/core'
import { initialSwapWidgetState } from '@openagentsinc/mkt-swp/view'
import { catalogFor } from '@openagentsinc/swap-i18n'
import { Effect } from 'effect'

import { SwapNotYetAvailable } from './not-yet-available'
import { defaultSwapSettings } from './settings'
import { SwapSurfaceShell } from './shell'
import { SwapWidget } from './widget'

const disconnectedSwapReporter: IntentReporter = () => Effect.void

/**
 * /swap — the swap widget (SWAP-0, #9315) mounted in SWAP-7's shell.
 *
 * The widget renders its typed state and the primary-action law today. Its
 * slots are the seams the sibling packages fill: SWAP-1 (#9316) asset and
 * amount entry, SWAP-2 (#9317) destination entry, SWAP-3 (#9318) quote
 * compare and the verify-before-fund checklist, SWAP-4 (#9319) the rescue
 * ceremony, SWAP-6 (#9321) session status. Those packages have landed as
 * logic; wiring their live inputs — the relay subscription that folds
 * Offerings and Quotes, and the wasm engine binding — is the next step, so
 * the widget mounts in its honest `EngineLoading` state and the notice below
 * says exactly that rather than implying a working market.
 */
export function SwapIndexPage() {
  const settings = defaultSwapSettings()

  return (
    <SwapSurfaceShell active="swap">
      <header className="grid gap-2" data-route="swap">
        <h1 className="m-0 text-3xl font-semibold tracking-tight">Swap</h1>
        <p className="m-0 max-w-[70ch] text-pretty text-sm/6 text-khala-text-muted">
          Atomic swaps negotiated on open relays: many providers publish signed
          offers, your browser verifies everything before any funding, and keys
          never leave this device.
        </p>
      </header>
      <SwapWidget
        catalog={catalogFor(settings.locale)}
        report={disconnectedSwapReporter}
        settings={settings}
        state={initialSwapWidgetState}
      />
      <SwapNotYetAvailable
        body="The widget shell, its typed state machine, and the engine boundary that authorizes funding are in place. The relay subscription that supplies live offers and quotes, and the wasm engine that performs verify-before-fund, are not wired up yet, so nothing here can quote, order, or fund a swap."
        heading="This widget is not connected to a market yet"
        issue="SWAP-0 (openagents#9315)"
        marker="swap-live-inputs"
      />
    </SwapSurfaceShell>
  )
}
