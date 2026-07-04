export const DOWNLOAD_ONE_CLICK_READY = false

export const AUTOPILOT_DESKTOP_DMG_URL =
  'https://github.com/OpenAgentsInc/openagents/releases/download/autopilot-desktop-v1.0.0-rc.3/AutopilotDesktop-1.0.0-rc.3-macos-arm64.dmg'

const AUTOPILOT_DESKTOP_RELEASE_URL =
  'https://github.com/OpenAgentsInc/openagents/releases/tag/autopilot-desktop-v1.0.0-rc.3'

const PYLON_INSTALL_COMMAND = 'npx @openagentsinc/pylon'

const panelClass =
  'grid gap-3 border border-khala-border/80 bg-khala-surface p-5 text-khala-text-muted sm:p-6'

const eyebrowClass =
  'm-0 font-mono text-sm uppercase tracking-wide text-khala-text-faint'

const primaryLinkClass =
  'khala-focus inline-flex min-h-11 items-center justify-center border border-khala-text bg-khala-text px-5 font-mono text-sm font-semibold text-black hover:bg-white'

function PlatformRow({
  platform,
  status,
}: Readonly<{ platform: string; status: string }>) {
  return (
    <div className="flex items-baseline justify-between gap-4 border-b border-khala-border/60 py-2 last:border-b-0">
      <span className="font-mono text-sm text-khala-text">{platform}</span>
      <span className="text-right font-mono text-xs text-khala-text-faint">
        {status}
      </span>
    </div>
  )
}

export function DownloadPage() {
  return (
    <main className="min-h-dvh bg-black text-khala-text" data-route="download">
      <div className="mx-auto grid w-full max-w-5xl gap-8 px-4 py-8 font-mono lg:grid-cols-[minmax(0,1fr)_minmax(20rem,24rem)]">
        <div className="grid content-start gap-6">
          <section className="grid gap-3 border-b border-khala-border pb-8">
            <p className={eyebrowClass}>Autopilot Desktop</p>
            <h1 className="m-0 text-balance text-3xl font-medium tracking-normal text-white sm:text-4xl">
              Download Autopilot for Mac
            </h1>
            <p className="m-0 max-w-[68ch] text-pretty text-base/7 text-khala-text-muted">
              The signed macOS app is available now. The one-click
              auto-onboarding experience is shipping in the next signed build -
              see the status note below before you install.
            </p>
          </section>
          <section
            className="grid gap-2 border border-khala-warning/40 bg-khala-warning/10 px-4 py-3 text-base/7 text-khala-text"
            data-download-status={DOWNLOAD_ONE_CLICK_READY ? 'live' : 'gated'}
          >
            <p className="m-0 font-mono text-xs uppercase text-khala-warning">
              Status: auto-onboarding not in this build yet
            </p>
            <p className="m-0 text-sm/6">
              The currently published .dmg is the latest signed release, but it
              predates the new auto-onboarding flow. If you install it today it
              boots an isolated node - it does not yet self-register or join the
              run for you on its own. The next signed build will. If you want to
              start contributing right now, run a Pylon node instead.
            </p>
          </section>
          <section className={panelClass} data-download-platform="macos">
            <p className={eyebrowClass}>macOS · Apple Silicon</p>
            <h2 className="m-0 text-lg font-medium text-white">
              Signed + notarized .dmg
            </h2>
            <p className="m-0 text-base/7 text-khala-text-muted">
              Built with an Apple Developer ID and notarized, so macOS
              Gatekeeper opens it without warnings. Apple Silicon (M-series)
              Macs.
            </p>
            <a
              className={primaryLinkClass}
              data-cta="download-autopilot"
              href={AUTOPILOT_DESKTOP_DMG_URL}
            >
              Download for Mac (Apple Silicon)
            </a>
            <a
              className="khala-focus font-mono text-xs text-khala-text-faint underline underline-offset-2 hover:text-khala-text"
              href={AUTOPILOT_DESKTOP_RELEASE_URL}
            >
              View the release on GitHub
            </a>
          </section>
          <section className={panelClass}>
            <p className={eyebrowClass}>For agents + operators</p>
            <h2 className="m-0 text-lg font-medium text-white">
              Run a Pylon node from the terminal
            </h2>
            <p className="m-0 text-base/7 text-khala-text-muted">
              The contributor path that works today. Paste this to your coding
              agent or run it yourself.
            </p>
            <pre
              className="m-0 w-full select-all overflow-x-auto border border-khala-border bg-black px-3 py-2 text-left font-mono text-sm leading-none text-khala-energy-soft"
              data-cta="install-pylon-command"
            >
              <code>{PYLON_INSTALL_COMMAND}</code>
            </pre>
          </section>
        </div>
        <aside className="grid content-start gap-6">
          <section className={panelClass}>
            <p className={eyebrowClass}>Platform availability</p>
            <div className="grid">
              <PlatformRow
                platform="macOS · Apple Silicon"
                status="Available now (signed + notarized)"
              />
              <PlatformRow platform="macOS · Intel" status="Not published yet" />
              <PlatformRow
                platform="Windows"
                status="Pending the Authenticode signing certificate"
              />
              <PlatformRow platform="Linux" status="Not published yet" />
            </div>
          </section>
          <section className="grid gap-1.5 border border-khala-border bg-khala-surface px-4 py-3 font-mono text-xs text-khala-text-faint">
            <p className="m-0 uppercase tracking-wide">For the owner - to make one-click live</p>
            <p className="m-0">
              Build + sign + notarize a fresh DMG from current main (with
              AO-1..AO-4), publish it, update AUTOPILOT_DESKTOP_DMG_URL in
              page/download.ts, then set DOWNLOAD_ONE_CLICK_READY = true. See
              docs/launch/2026-06-18-autopilot-desktop-availability-audit.md
              section 4.
            </p>
          </section>
        </aside>
      </div>
    </main>
  )
}
