import { SwapNotYetAvailable } from './not-yet-available'
import { SwapSurfaceShell } from './shell'

/**
 * /swap/rescue — the coordinator-absent recovery surface (SWAP-4, #9319).
 *
 * SWAP-4 (in-browser key material, the rescue ceremony, and the Rescue page
 * that works with every provider, handler, and relay gone) is not built.
 * This shell renders the honest state and the deep-link coordinate for a
 * specific recovery action; mount the SWAP-4 surface here when it lands.
 */
export function SwapRescuePage({
  actionRef,
}: Readonly<{ actionRef?: string | undefined }>) {
  return (
    <SwapSurfaceShell active="rescue">
      <header className="grid gap-2" data-route="swap-rescue">
        <h1 className="m-0 text-3xl font-semibold tracking-tight">Rescue</h1>
        <p className="m-0 max-w-[70ch] text-pretty text-sm/6 text-khala-text-muted">
          Recovery without the coordinator: your browser holds the refund and
          claim key material, and every swap persists an exit package before
          funding, so a swap can be recovered with every provider and relay
          gone.
        </p>
      </header>
      {actionRef === undefined ? null : (
        <p
          className="m-0 border border-khala-border/60 bg-khala-surface/40 p-3 font-mono text-xs text-khala-text-muted"
          data-swap-rescue-action-ref={actionRef}
        >
          This link addresses recovery action {actionRef}. It will resolve
          against your locally persisted exit packages once the Rescue surface
          lands.
        </p>
      )}
      <SwapNotYetAvailable
        marker="rescue"
        heading="The Rescue surface has not landed"
        body="The secret store, the verified backup ceremony, and the recovery flow are being built. No key material exists in this browser yet, so there is nothing this page could recover, and no control will appear here until recovery is real."
        issue="SWAP-4 (openagents#9319)"
      />
    </SwapSurfaceShell>
  )
}
