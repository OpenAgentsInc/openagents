const linkClass =
  'khala-focus border border-khala-border px-3 py-2 text-sm font-semibold text-khala-text-muted underline-offset-4 hover:border-khala-text-faint hover:text-white hover:underline'

export function RunPage() {
  return (
    <main
      aria-label="Live Tassadar run"
      className="relative grid min-h-dvh place-items-center overflow-hidden bg-black p-5 text-khala-text"
      data-route="tassadar"
    >
      <section
        aria-label="Retired Tassadar web scene"
        className="grid w-full max-w-xl gap-4 border border-khala-border bg-khala-surface p-5 font-mono shadow-2xl shadow-black/60"
        data-tassadar-scene="retired"
      >
        <p className="m-0 text-xs font-semibold uppercase leading-none tracking-wide text-khala-text-faint">
          Retired scene
        </p>
        <h1 className="m-0 text-balance text-2xl font-semibold leading-tight text-white sm:text-3xl">
          Tassadar lives in the Verse
        </h1>
        <p className="m-0 text-pretty text-base/7 text-khala-text-muted sm:text-sm/6">
          The old web training-run scene is deprecated. Use Autopilot Desktop
          Verse for the in-world Pylon and Tassadar surface.
        </p>
        <div className="flex flex-wrap gap-2 pt-1">
          <a className={linkClass} href="/api/public/tassadar-run-summary">
            Public summary API
          </a>
          <a className={linkClass} href="/tassadar/replay/first-real-settlement">
            Proof replay
          </a>
        </div>
      </section>
    </main>
  )
}
