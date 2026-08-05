import { SwapNotYetAvailable } from './not-yet-available'
import { SwapSurfaceShell } from './shell'

/**
 * /swap — the mount point for the swap widget (SWAP-0, #9315).
 *
 * The widget shell, its typed state machine, and the engine boundary are
 * being built on #9315 and have not landed on main yet. Until that package
 * exists, this route renders an honest not-yet-available state instead of a
 * surface that looks functional. When `packages/mkt-swp` lands, mount its
 * widget here. Sibling packages that HAVE landed —
 * `@openagentsinc/mkt-swp-destination` (#9317) and
 * `@openagentsinc/swap-i18n` (#9323) — are inputs to that widget, not
 * standalone pages, so they mount with it.
 */
export function SwapIndexPage() {
  return (
    <SwapSurfaceShell active="swap">
      <header className="grid gap-2" data-route="swap">
        <h1 className="m-0 text-3xl font-semibold tracking-tight">Swap</h1>
        <p className="m-0 max-w-[70ch] text-pretty text-sm/6 text-khala-text-muted">
          Atomic swaps negotiated on open relays: many providers publish
          signed offers, your browser verifies everything before any funding,
          and keys never leave this device.
        </p>
      </header>
      <SwapNotYetAvailable
        marker="swap-widget"
        heading="The swap widget has not landed"
        body="The widget shell, its typed state machine, and the engine boundary that authorizes funding are being built. Nothing on this page can quote, order, or fund a swap yet, and no control will appear here until it is real."
        issue="SWAP-0 (openagents#9315)"
      />
    </SwapSurfaceShell>
  )
}
