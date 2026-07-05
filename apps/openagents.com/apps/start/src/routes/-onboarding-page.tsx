const githubLoginHref = '/login/github'

// `openagents.com/onboarding` — the pre-login Autopilot teaser a real
// anonymous visitor sees today. The Foldkit original (apps/web/src/page/
// loggedOut/page/onboarding.ts) also defines a `funding` step with an
// interactive credit-amount slider, but that step is reachable only through
// a direct `ClickedOnboardingStep({ step: 'funding' })` dispatch that nothing
// in this route ever sends — `initOnboardingModel()` always starts at
// `step: 'github'`, and this standalone page has no click affordance that
// changes it. So this port covers the state a visitor actually gets: the
// GitHub-login landing. The funding-demo branch stays back-logged with the
// rest of the unmigrated `loggedOut` pages.

export function OnboardingPage() {
  return (
    <div className="border-b border-khala-border">
      <header className="border-b border-khala-border">
        <div className="mx-auto flex min-h-12 w-[min(100%,72rem)] flex-wrap items-center justify-between gap-3 border-x border-khala-border px-4 py-2">
          <a
            className="text-xs font-bold uppercase tracking-[0.08em] text-khala-text-muted no-underline hover:text-khala-text"
            href="/"
          >
            OpenAgents Autopilot
          </a>
          <a
            className="khala-focus inline-flex min-h-9 items-center border border-white/20 bg-white/10 px-3 text-xs font-semibold uppercase tracking-[0.08em] text-khala-text-muted no-underline hover:bg-white/[0.04] hover:text-khala-text"
            href={githubLoginHref}
          >
            Log in with GitHub
          </a>
        </div>
      </header>
      <main
        className="mx-auto grid min-h-[calc(100dvh-3rem)] w-[min(100%,72rem)] items-center border-x border-khala-border px-4 py-14 sm:px-6 lg:px-8"
        data-route="onboarding"
      >
        <section className="max-w-3xl">
          <p className="m-0 text-xs font-semibold uppercase tracking-[0.08em] text-khala-text-faint">
            OpenAgents Autopilot
          </p>
          <h1 className="m-0 mt-7 max-w-[13ch] text-balance text-5xl font-semibold leading-none text-khala-text sm:text-6xl lg:text-7xl">
            Stop Babysitting Your AI
          </h1>
          <p className="m-0 mt-7 max-w-[46ch] text-base leading-7 text-khala-text-muted sm:text-lg sm:leading-8">
            Launch coding agents. Close your laptop. Stay in the loop from
            anywhere.
          </p>
          <div className="mt-7 grid gap-1 text-khala-text">
            <p className="m-0 text-lg font-semibold text-khala-text">
              Start work. Walk away.
            </p>
            <p className="m-0 text-sm text-khala-text-muted">
              Your agents keep going.
            </p>
          </div>
          <div className="mt-9">
            <a
              className="khala-focus inline-grid min-h-11 place-items-center border border-khala-text bg-khala-text px-4 text-sm font-medium text-black no-underline hover:border-khala-warning"
              href={githubLoginHref}
            >
              Log in with GitHub
            </a>
          </div>
        </section>
      </main>
    </div>
  )
}
