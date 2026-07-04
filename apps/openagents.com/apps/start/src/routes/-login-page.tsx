import { GitBranch, Mail } from 'lucide-react'

const inputClass =
  'min-h-11 w-full border border-khala-border bg-black px-3 py-2 font-mono text-base text-khala-text outline-none placeholder:text-khala-text-faint sm:text-sm'

const buttonClass =
  'khala-focus inline-flex min-h-11 items-center justify-center gap-2 border border-khala-warning/45 bg-khala-warning px-4 py-2 font-mono text-sm font-semibold text-black'

const secondaryLinkClass =
  'khala-focus inline-flex min-h-11 items-center justify-center gap-2 border border-khala-border bg-khala-surface-raised px-4 py-2 font-mono text-sm font-semibold text-khala-text'

export function LoginPage() {
  return (
    <main
      aria-label="Log in"
      className="grid min-h-dvh place-items-center overflow-y-auto bg-black px-4 py-12 text-khala-text"
      data-persistent-scene-overlay="login"
      data-route="login"
    >
      <section className="grid w-full max-w-[420px] gap-6 border border-khala-border bg-khala-surface p-8 font-mono">
        <div className="grid gap-2">
          <h1 className="m-0 text-2xl font-medium tracking-tight text-white">
            Log in to OpenAgents
          </h1>
          <p className="m-0 text-base/7 text-khala-text-muted sm:text-sm/6">
            Enter your email and we will send a one-time sign-in code, or
            continue with GitHub.
          </p>
        </div>
        <form action="/login/email" className="grid gap-3" method="get">
          <label className="grid gap-1.5 text-sm text-khala-text-muted">
            <span>Email</span>
            <input
              autoComplete="email"
              className={inputClass}
              name="email"
              placeholder="you@example.com"
              type="email"
            />
          </label>
          <button className={buttonClass} type="submit">
            <Mail aria-hidden="true" size={16} strokeWidth={1.8} />
            <span>Email me a code</span>
          </button>
        </form>
        <a className={secondaryLinkClass} href="/login/github">
          <GitBranch aria-hidden="true" size={16} strokeWidth={1.8} />
          <span>Log in with GitHub</span>
        </a>
        <p className="m-0 text-xs/5 text-khala-text-faint">
          Signing in creates your account. Access to workrooms and the operator
          console is granted to approved accounts.
        </p>
      </section>
    </main>
  )
}
