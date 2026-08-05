import { SwapNotYetAvailable } from './not-yet-available'
import { SwapSurfaceShell } from './shell'

/**
 * /swap/history — the local session store and History surface (SWAP-5,
 * #9320), being built in parallel with this route work.
 *
 * At pull time `packages/` carries no SWAP-5 session-store package on main,
 * so this route renders the honest not-yet-available state. When the SWAP-5
 * package lands, mount its History table and resumable-session list here;
 * the `/swap/s/$sessionId` deep link below already addresses one stored
 * session for it.
 */
export function SwapHistoryPage({
  sessionId,
}: Readonly<{ sessionId?: string | undefined }>) {
  return (
    <SwapSurfaceShell active="history">
      <header className="grid gap-2" data-route="swap-history">
        <h1 className="m-0 text-3xl font-semibold tracking-tight">History</h1>
        <p className="m-0 max-w-[70ch] text-pretty text-sm/6 text-khala-text-muted">
          Your swaps, from your own locally stored signed records — no
          provider account, no server-side history. In-flight sessions resume
          from here.
        </p>
      </header>
      {sessionId === undefined ? null : (
        <p
          className="m-0 border border-khala-border/60 bg-khala-surface/40 p-3 font-mono text-xs text-khala-text-muted"
          data-swap-session-ref={sessionId}
        >
          This link resumes session {sessionId}. It will resolve against your
          local session store once the History surface lands.
        </p>
      )}
      <SwapNotYetAvailable
        marker="history"
        heading="The local session store has not landed"
        body="The local signed-record store, resumable in-flight sessions, and export with import are being built. This browser holds no swap sessions yet, so there is nothing this page could list, and no fixture rows will stand in for real ones."
        issue="SWAP-5 (openagents#9320)"
      />
    </SwapSurfaceShell>
  )
}
