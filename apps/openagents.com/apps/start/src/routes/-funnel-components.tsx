import { ArrowRight, CheckCircle2, ExternalLink, Menu, Terminal } from 'lucide-react'
import type * as React from 'react'

import {
  blogPosts,
  businessOfferings,
  businessPackages,
  docsPages,
  khalaCodeInstall,
  ladderSteps,
  legalVerifiedStats,
  publicNavLinks,
  type BlogPost,
  type DocPage,
} from './-funnel-data'

type PageShellProps = Readonly<{
  children: React.ReactNode
  dataRoute: string
}>

export function PageShell({ children, dataRoute }: PageShellProps) {
  return (
    <div
      data-route={dataRoute}
      className="min-h-dvh bg-khala-void text-khala-text selection:bg-khala-energy selection:text-white"
    >
      <header className="sticky top-0 z-20 border-b border-khala-border/70 bg-khala-void/88 backdrop-blur-md">
        <div className="mx-auto flex w-[min(100%,1120px)] items-center justify-between gap-4 px-4 py-3">
          <a
            className="khala-focus font-mono text-sm text-khala-energy-cyan"
            href="/"
          >
            OpenAgents
          </a>
          <nav
            aria-label="Primary"
            className="hidden items-center gap-5 font-mono text-sm text-khala-text-muted lg:flex"
          >
            {publicNavLinks.slice(1).map(link => (
              <a
                className="khala-focus transition-colors hover:text-white motion-reduce:transition-none"
                href={link.href}
                key={link.href}
              >
                {link.label}
              </a>
            ))}
          </nav>
          <details className="relative lg:hidden">
            <summary className="khala-focus flex size-11 cursor-pointer list-none items-center justify-center border border-khala-border bg-khala-surface-raised text-khala-text">
              <Menu aria-hidden="true" className="size-5" />
              <span className="sr-only">Open navigation</span>
            </summary>
            <nav
              aria-label="Mobile primary"
              className="absolute right-0 mt-2 grid w-64 gap-1 border border-khala-border bg-khala-surface p-2 font-mono text-base text-khala-text shadow-2xl"
            >
              {publicNavLinks.map(link => (
                <a
                  className="khala-focus px-3 py-2 text-khala-text-muted hover:bg-khala-surface-raised hover:text-white"
                  href={link.href}
                  key={link.href}
                >
                  {link.label}
                </a>
              ))}
            </nav>
          </details>
        </div>
      </header>
      {children}
    </div>
  )
}

export function Hero({
  eyebrow,
  title,
  body,
  children,
}: Readonly<{
  body: string
  children?: React.ReactNode
  eyebrow: string
  title: string
}>) {
  return (
    <section className="border-b border-khala-border/70 bg-[linear-gradient(180deg,rgba(58,123,255,0.12),transparent_58%)]">
      <div className="mx-auto grid w-[min(100%,1120px)] gap-6 px-4 py-12 sm:py-16">
        <p className="m-0 font-mono text-sm uppercase tracking-wide text-khala-energy-soft">
          {eyebrow}
        </p>
        <h1 className="m-0 max-w-[13ch] text-balance text-4xl font-semibold tracking-tight text-white sm:text-6xl">
          {title}
        </h1>
        <p className="m-0 max-w-[74ch] text-pretty text-lg/8 text-khala-text-muted sm:text-base/7">
          {body}
        </p>
        {children}
      </div>
    </section>
  )
}

const panelClass =
  'border border-khala-border/80 bg-khala-surface p-5 text-khala-text-muted'

const eyebrowClass =
  'm-0 font-mono text-sm uppercase tracking-wide text-khala-text-faint'

export function BusinessPage() {
  return (
    <PageShell dataRoute="business">
      <Hero
        eyebrow="OpenAgents Business"
        title="Agents that work."
        body="Hire agents from the OpenAgents network to get real work done — software built fast, campaigns drafted, batches processed — delivered with verifiable receipts."
      >
        <p className="m-0 max-w-[74ch] text-pretty text-lg/8 text-khala-text-muted sm:text-base/7">
          Start with a fast quick win we can deliver in days, then put recurring
          work on Autopilot as trust builds. Every accepted outcome ties to
          evidence; every paid run is scoped with a receipt plan up front; a
          human-review gate sits before anything ships, sends, or spends.
        </p>
        <div className="flex flex-col gap-3 sm:flex-row">
          <a
            className="khala-focus inline-flex min-h-12 items-center justify-center gap-2 border border-khala-energy-cyan bg-khala-energy-cyan px-4 font-mono text-sm font-semibold text-black"
            href="#business-intake"
          >
            Talk to Khala
            <ArrowRight aria-hidden="true" className="size-4" />
          </a>
          <a
            className="khala-focus inline-flex min-h-12 items-center justify-center border border-khala-border-strong/55 bg-khala-surface-raised px-4 font-mono text-sm font-semibold text-khala-text"
            href="#business-signup"
          >
            Use the form
          </a>
        </div>
      </Hero>
      <main
        aria-label="Business"
        className="mx-auto grid w-[min(100%,1120px)] gap-10 px-4 py-10 lg:grid-cols-[minmax(0,1fr)_minmax(20rem,26rem)]"
      >
        <div className="grid content-start gap-10">
          <section
            className={`${panelClass} grid gap-0 overflow-hidden p-0`}
            data-business-intake-chat=""
            id="business-intake"
          >
            <div className="flex flex-wrap items-center gap-x-3 gap-y-1 border-b border-khala-border/70 px-4 py-2.5 font-mono text-sm">
              <span className="font-medium tracking-wide text-white">
                KHALA · INTAKE
              </span>
              <span className="text-khala-text-muted">
                describe what you need — Khala scopes the quick win
              </span>
              <span className="ml-auto hidden text-khala-text-faint sm:inline">
                bounded interview · no credentials · receipt-first
              </span>
            </div>
            <div
              aria-live="polite"
              className="grid min-h-36 content-start gap-3 px-4 py-4 font-mono text-base/7 text-khala-text-muted sm:text-sm/6"
              data-intake-chat-transcript=""
            >
              <p
                className="m-0 max-w-[62ch]"
                data-intake-chat-empty=""
              >
                Tell Khala what your business needs — a stuck task, a
                repetitive grind, software you wish existed. It runs a short
                interview, matches you to what OpenAgents can honestly deliver
                today, and drafts your intake spec.
              </p>
              <noscript>
                <p className="m-0">
                  JavaScript is off — use the form below instead. Same intake,
                  same receipt.
                </p>
              </noscript>
            </div>
            <div className="flex items-stretch gap-2 border-t border-khala-border/70 p-2.5">
              <textarea
                aria-label="Message Khala"
                className="min-h-12 flex-1 resize-none border border-khala-border bg-black px-3 py-2 font-mono text-base text-khala-text outline-none placeholder:text-khala-text-faint focus:border-khala-border-strong sm:text-sm"
                data-intake-chat-input=""
                placeholder="e.g. rebuild our outdated internal dashboard"
                rows={1}
              />
              <button
                className="border border-khala-border bg-khala-surface-raised px-4 font-mono text-sm text-khala-text"
                data-intake-chat-send=""
                type="button"
              >
                Send
              </button>
            </div>
          </section>
          <section className="grid gap-5">
            <div className="grid gap-2">
              <p className={eyebrowClass}>What we can do</p>
              <p className="m-0 max-w-[72ch] text-pretty text-base/7 text-khala-text-muted">
                An honest menu of what OpenAgents can deliver. Availability is
                grounded in our public product-promise registry - shipped now,
                operator-assisted with a caveat, or planned roadmap. We say so
                in writing and scope the smallest honest version.
              </p>
            </div>
            <div
              className="grid gap-3 md:grid-cols-2"
              data-ui-family="business/offering-menus"
            >
              {businessOfferings.map(offering => (
                <article
                  className={`${panelClass} grid gap-3`}
                  data-ui-family="business/offering-cards"
                  key={offering.title}
                >
                  <div className="flex flex-wrap items-start justify-between gap-3">
                    <h2 className="m-0 text-xl font-semibold tracking-tight text-white">
                      {offering.title}
                    </h2>
                    <span
                      className="border border-khala-border-strong/55 bg-khala-surface-raised px-2 py-1 font-mono text-sm text-khala-energy-soft"
                      data-ui-family="business/availability-badges"
                    >
                      {offering.availability}
                    </span>
                  </div>
                  <p className="m-0 text-base/7">{offering.what}</p>
                  <p className="m-0 text-base/7">
                    <strong className="text-khala-text">Live now:</strong>{' '}
                    {offering.liveNow}
                  </p>
                  <p className="m-0 text-base/7">
                    <strong className="text-khala-text">Current caveat:</strong>{' '}
                    {offering.caveat}
                  </p>
                  <p className="m-0 text-base/7 text-khala-energy-soft">
                    {offering.quickWin}
                  </p>
                </article>
              ))}
            </div>
          </section>
          <section className="grid gap-5" data-ui-family="business/rate-cards">
            <div className="grid gap-2">
              <p className={eyebrowClass}>Rate card</p>
              <p className="m-0 max-w-[72ch] text-pretty text-base/7 text-khala-text-muted">
                Public package bands for operator-assisted work. The rate card
                is a quote starter, not a self-serve checkout: each engagement
                still gets a written scope, receipt plan, and review gate before
                work begins.
              </p>
            </div>
            <div className="grid gap-3 md:grid-cols-2">
              {businessPackages.map(item => (
                <article
                  className={`${panelClass} grid gap-3`}
                  data-ui-family="business/rate-card-packages"
                  key={item.title}
                >
                  <div>
                    <h2 className="m-0 text-xl font-semibold tracking-tight text-white">
                      {item.title}
                    </h2>
                    <p className="m-0 font-mono text-sm text-khala-energy-cyan">
                      {item.price}
                    </p>
                  </div>
                  <p className="m-0 text-base/7">{item.scope}</p>
                  <ul className="m-0 grid gap-2 p-0">
                    {item.receiptPlan.map(plan => (
                      <li className="flex gap-2 text-base/7" key={plan}>
                        <CheckCircle2
                          aria-hidden="true"
                          className="mt-1 size-4 shrink-0 text-khala-energy-cyan"
                        />
                        <span>{plan}</span>
                      </li>
                    ))}
                  </ul>
                  <p className="m-0 text-base/7 text-khala-text-faint">
                    {item.caveat}
                  </p>
                </article>
              ))}
            </div>
          </section>
          <section
            className={`${panelClass} grid gap-4`}
            data-ui-family="business/quick-win-ladders"
          >
            <div className="grid gap-2">
              <p className={eyebrowClass}>
                Quick win -&gt; put your business on Autopilot
              </p>
              <p className="m-0 max-w-[72ch] text-pretty text-base/7">
                You do not commit to the whole journey up front. We pick one
                small first win, then grow the relationship only if it works.
              </p>
            </div>
            <div className="grid gap-3 md:grid-cols-3">
              {ladderSteps.map(step => (
                <article
                  className="grid gap-2 border border-khala-border/70 bg-khala-surface-raised p-4"
                  data-business-ladder-step={step.when}
                  key={step.when}
                >
                  <h2 className="m-0 text-lg font-semibold tracking-tight text-white">
                    {step.when} - {step.title}
                  </h2>
                  <p className="m-0 text-base/7 text-khala-text-muted">
                    {step.body}
                  </p>
                </article>
              ))}
            </div>
          </section>
          <section className={`${panelClass} grid gap-2`}>
            <p className={eyebrowClass}>Project invite</p>
            <h2 className="m-0 text-xl font-semibold tracking-tight text-white">
              We prepare the workspace before you open it
            </h2>
            <p className="m-0 text-base/7">
              Your invite opens a named project with seeded notes, starter
              workflows, and an intro receipt.
            </p>
          </section>
        </div>
        <form
          action="/api/public/business-signup"
          className={`${panelClass} grid content-start gap-4 self-start`}
          id="business-signup"
          method="post"
        >
          <div className="grid gap-2">
            <p className={eyebrowClass}>Tell us what to hand off</p>
            <p className="m-0 text-base/7 text-khala-text-muted">
              Packages start with a fixed scope and receipt plan before
              funding. Delivery is operator-assisted today; checkout and
              self-serve hosting are not implied by the rate card.
            </p>
          </div>
          {[
            ['businessName', 'Business name', 'text'],
            ['contactEmail', 'Contact email', 'email'],
            ['website', 'Website', 'url'],
            ['phone', 'Phone', 'tel'],
          ].map(([name, label, type]) => (
            <label className="grid gap-1" data-ui-family="forms/input-groups" key={name}>
              <span className="font-mono text-sm text-khala-text-muted">
                {label}
              </span>
              <input
                className="min-h-11 border border-khala-border bg-black px-3 text-base text-khala-text"
                name={name}
                type={type}
              />
            </label>
          ))}
          <label className="grid gap-1" data-ui-family="forms/input-groups">
            <span className="font-mono text-sm text-khala-text-muted">
              What should agents help with?
            </span>
            <textarea
              className="min-h-28 border border-khala-border bg-black px-3 py-2 text-base text-khala-text"
              name="helpWith"
            />
          </label>
          <label className="flex items-start gap-2 text-base/7 text-khala-text-muted">
            <input
              className="mt-1"
              name="requestSlackChannel"
              type="checkbox"
              value="yes"
            />
            <span>Request a shared Slack channel</span>
          </label>
          <input id="business-referral-code" name="referralCode" type="hidden" />
          <input id="business-source-ref" name="sourceRef" type="hidden" />
          <button className="khala-focus min-h-12 border border-khala-energy-cyan bg-khala-energy-cyan px-4 font-mono text-sm font-semibold text-black" type="submit">
            Send intake
          </button>
        </form>
      </main>
    </PageShell>
  )
}

export function DocsIndexPage() {
  const listedPages = docsPages.filter(page => page.listed)

  return (
    <PageShell dataRoute="docs">
      <Hero
        eyebrow="Docs"
        title="OpenAgents docs"
        body="Public documentation for Khala Code, OpenAgents, product promises, forum participation, and developer API surfaces."
      />
      <main className="mx-auto grid w-[min(100%,920px)] gap-4 px-4 py-10">
        {listedPages.map(page => (
          <a
            className={`${panelClass} khala-focus grid gap-2 transition-colors hover:border-khala-border-strong motion-reduce:transition-none`}
            href={`/docs/${page.slug}`}
            key={page.slug}
          >
            <h2 className="m-0 text-xl font-semibold tracking-tight text-white">
              {page.title}
            </h2>
            <p className="m-0 text-base/7 text-khala-text-muted">
              {page.summary}
            </p>
          </a>
        ))}
      </main>
    </PageShell>
  )
}

export function DocPageView({ page }: Readonly<{ page: DocPage }>) {
  return (
    <PageShell dataRoute="docs-page">
      <Hero eyebrow="Docs" title={page.title} body={page.summary} />
      <main className="mx-auto grid w-[min(100%,920px)] gap-8 px-4 py-10">
        <article className={`${panelClass} grid gap-4`}>
          {page.description.map(paragraph => (
            <p className="m-0 text-pretty text-base/7" key={paragraph}>
              {paragraph}
            </p>
          ))}
          {page.sections?.map(section => (
            <section className="grid gap-3 border-t border-khala-border/70 pt-5" key={section.heading}>
              <h2 className="m-0 text-2xl font-semibold tracking-tight text-white">
                {section.heading}
              </h2>
              <ul className="m-0 grid gap-2 p-0">
                {section.items.map(item => (
                  <li className="flex gap-2 text-base/7" key={item}>
                    <CheckCircle2 aria-hidden="true" className="mt-1 size-4 shrink-0 text-khala-energy-cyan" />
                    <span>{item}</span>
                  </li>
                ))}
              </ul>
            </section>
          ))}
          {page.links === undefined ? null : (
            <nav className="flex flex-wrap gap-3 border-t border-khala-border/70 pt-5" aria-label="Doc links">
              {page.links.map(link => (
                <a className="khala-focus inline-flex items-center gap-2 font-mono text-sm text-khala-energy-cyan underline underline-offset-4" href={link.href} key={link.href}>
                  {link.label}
                  <ExternalLink aria-hidden="true" className="size-4" />
                </a>
              ))}
            </nav>
          )}
        </article>
      </main>
    </PageShell>
  )
}

export function BlogIndexPage() {
  const listedPosts = blogPosts.filter(post => post.listed)

  return (
    <PageShell dataRoute="blog">
      <Hero
        eyebrow="Blog"
        title="OpenAgents Blog"
        body="Build notes and launch notes from the OpenAgents network."
      />
      <main className="mx-auto grid w-[min(100%,920px)] gap-4 px-4 py-10">
        {listedPosts.map(post => (
          <a
            className={`${panelClass} khala-focus grid gap-2 transition-colors hover:border-khala-border-strong motion-reduce:transition-none`}
            href={`/blog/${post.slug}`}
            key={post.slug}
          >
            <p className="m-0 font-mono text-sm text-khala-text-faint">
              {post.date} · {post.readTime}
            </p>
            <h2 className="m-0 text-xl font-semibold tracking-tight text-white">
              {post.title}
            </h2>
            <p className="m-0 text-base/7 text-khala-text-muted">
              {post.excerpt}
            </p>
          </a>
        ))}
      </main>
    </PageShell>
  )
}

export function BlogPostPage({ post }: Readonly<{ post: BlogPost }>) {
  return (
    <PageShell dataRoute="blog-post">
      <Hero eyebrow="Blog" title={post.title} body={post.excerpt}>
        <p className="m-0 font-mono text-sm text-khala-text-faint">
          {post.date} · {post.readTime}
        </p>
      </Hero>
      <main className="mx-auto grid w-[min(100%,920px)] gap-8 px-4 py-10">
        <article className={`${panelClass} grid gap-8`}>
          {post.sections.map(section => (
            <section className="grid gap-3" key={section.title}>
              <h2 className="m-0 text-2xl font-semibold tracking-tight text-white">
                {section.title}
              </h2>
              {section.paragraphs.map(paragraph => (
                <p className="m-0 text-pretty text-base/7" key={paragraph}>
                  {paragraph}
                </p>
              ))}
            </section>
          ))}
        </article>
      </main>
    </PageShell>
  )
}

export function KhalaCodeDownloadPage() {
  return (
    <PageShell dataRoute="khala-code-download">
      <Hero
        eyebrow="Khala Code install truth"
        title="Install paths, with the Codex requirement kept visible"
        body="Khala Code is the OpenAgents coding app around your own local Codex install. The default desktop harness requires the Codex CLI and a signed-in primary Codex home before it can run coding turns."
      />
      <main
        aria-label="Khala Code install paths"
        className="mx-auto grid w-[min(100%,980px)] gap-8 px-4 py-8 lg:grid-cols-[minmax(0,1fr)_minmax(20rem,24rem)]"
      >
        <div className="grid content-start gap-6">
          <InstallPanel label="Required first" title="Install and sign in to Codex">
            <p className="m-0 text-base/7">
              Khala Code does not bundle or replace Codex Core. Run the Codex
              install and login yourself for the primary user Codex home.
            </p>
            <CommandBlock
              command={`${khalaCodeInstall.codexInstallCommand}\n${khalaCodeInstall.codexLoginCommand}`}
            />
          </InstallPanel>
          <InstallPanel label="Desktop DMG" status="public artifact pending" title="macOS release lane">
            <p className="m-0 text-base/7">
              The Khala Code desktop release lane exists for signed/notarized
              macOS builds, but no public signed DMG receipt is recorded here
              yet. Use the source build path until a public artifact appears
              with owner release receipts.
            </p>
            <dl className="m-0 grid gap-2 border-t border-khala-border/70 pt-3 font-mono text-sm">
              <div className="grid gap-1 sm:grid-cols-[8rem_1fr]">
                <dt className="text-khala-text-faint">Product</dt>
                <dd className="m-0">{khalaCodeInstall.desktopProduct}</dd>
              </div>
              <div className="grid gap-1 sm:grid-cols-[8rem_1fr]">
                <dt className="text-khala-text-faint">Feed</dt>
                <dd className="m-0 break-all">{khalaCodeInstall.releaseFeedUrl}</dd>
              </div>
            </dl>
          </InstallPanel>
          <InstallPanel label="Terminal" status="npm package" title="Install the khala CLI">
            <p className="m-0 text-base/7">
              The CLI is the public terminal path for Khala chat, Codex account
              connection, and fleet commands. Fleet coding still requires your
              own Codex account.
            </p>
            <CommandBlock command={khalaCodeInstall.khalaCliInstallCommand} />
          </InstallPanel>
          <InstallPanel label="Desktop from source" title="Run Khala Code from the repo">
            <p className="m-0 text-base/7">
              This is the supported desktop path while the signed DMG remains
              receipt-gated. Clone shallow and install workspace dependencies at
              the repo root.
            </p>
            <CommandBlock command={khalaCodeInstall.sourceBuildCommands} />
          </InstallPanel>
        </div>
        <div className="grid content-start gap-6">
          <section className={`${panelClass} grid gap-2`} data-promise-gate={khalaCodeInstall.promiseId}>
            <p className={eyebrowClass}>Copy gate</p>
            <p className="m-0 text-base/7">{khalaCodeInstall.promiseSafeCopy}</p>
            <a className="khala-focus w-fit font-mono text-sm text-khala-energy-cyan underline underline-offset-4" href="/api/public/product-promises">
              Product promise registry
            </a>
          </section>
          <section className={`${panelClass} grid gap-2`} data-download-counter="khala-code">
            <p className={eyebrowClass}>Counter</p>
            <h2 className="m-0 text-xl font-semibold tracking-tight text-white">
              Exact rows only
            </h2>
            <p className="m-0 text-base/7">
              The public counter endpoint exposes only exact download ledger
              rows. If there are no rows, it returns an empty counts array
              instead of a synthesized number.
            </p>
            <a className="khala-focus break-all font-mono text-sm text-khala-energy-cyan underline underline-offset-4" href={khalaCodeInstall.counterEndpoint}>
              {khalaCodeInstall.counterEndpoint}
            </a>
          </section>
        </div>
      </main>
    </PageShell>
  )
}

function InstallPanel({
  children,
  label,
  status,
  title,
}: Readonly<{
  children: React.ReactNode
  label: string
  status?: string
  title: string
}>) {
  return (
    <section className={`${panelClass} grid gap-3`}>
      <div className="flex flex-wrap items-center justify-between gap-3">
        <p className={eyebrowClass}>{label}</p>
        {status === undefined ? null : (
          <span className="border border-khala-border-strong/55 bg-khala-surface-raised px-2 py-1 font-mono text-sm text-khala-energy-soft">
            {status}
          </span>
        )}
      </div>
      <h2 className="m-0 text-xl font-semibold tracking-tight text-white">
        {title}
      </h2>
      {children}
    </section>
  )
}

function CommandBlock({ command }: Readonly<{ command: string }>) {
  return (
    <pre className="m-0 flex w-full gap-3 overflow-x-auto border border-khala-border/70 bg-black px-3 py-3 text-left font-mono text-sm leading-6 text-khala-energy-cyan">
      <Terminal aria-hidden="true" className="mt-1 size-4 shrink-0" />
      <code>{command}</code>
    </pre>
  )
}

export function AutopilotPage({ legal = false }: Readonly<{ legal?: boolean }>) {
  return (
    <PageShell dataRoute={legal ? 'autopilot-legal' : 'autopilot'}>
      <Hero
        eyebrow={legal ? 'For legal teams' : 'Autopilot'}
        title="Autopilot"
        body={
          legal
            ? 'Describe your legal work. Autopilot scopes it, shows you a quick win, and keeps a review gate before anything is sent — no client-identifying detail leaves without your consent.'
            : 'Describe what you want done. Autopilot scopes the work, shows you a quick win, and keeps a human-review gate before anything ships.'
        }
      />
      <main className="mx-auto grid w-[min(100%,1040px)] gap-6 px-4 py-10 lg:grid-cols-[minmax(0,1fr)_minmax(20rem,24rem)]">
        <section className={`${panelClass} grid gap-4`}>
          <p className={eyebrowClass}>
            {legal ? 'Bounded first moves' : 'Start with a bounded first move'}
          </p>
          <ul className="m-0 grid gap-2 p-0">
            {(legal
              ? [
                  'Prepare a draft NDA prep packet for a routine vendor conversation.',
                  'Find a fitting formation/intake template and list the missing facts.',
                  'Build a lawyer-review checklist for a routine document.',
                ]
              : [
                  'Describe a stuck task.',
                  'Autopilot scopes the smallest honest quick win.',
                  'A human-review gate stays before anything ships.',
                ]
            ).map(item => (
              <li className="flex gap-2 text-base/7" key={item}>
                <CheckCircle2 aria-hidden="true" className="mt-1 size-4 shrink-0 text-khala-energy-cyan" />
                <span>{item}</span>
              </li>
            ))}
          </ul>
        </section>
        {legal ? (
          <section className={`${panelClass} grid gap-4`} data-autopilot-onboarding-legal-overlay="">
            <p className="m-0 max-w-[60ch] text-base/7">
              Stay in expert review mode. You share only the source material you
              choose; Autopilot prepares a bounded, template-driven,
              source-linked work surface — a draft prep packet, intake
              questions, and a lawyer-review checklist — with an
              attorney-review gate before anything is sent. Not an AI lawyer,
              not case-law research.
            </p>
            <div
              aria-label="Legal overview video"
              className="grid aspect-video place-items-center border border-khala-border/70 bg-black text-center"
              data-autopilot-onboarding-legal-vsl=""
            >
              <span className="text-base text-khala-text-faint">
                Overview video — coming soon
              </span>
            </div>
            <div
              className="grid gap-3 border-t border-khala-border/70 pt-3"
              data-autopilot-onboarding-legal-stats=""
            >
              {legalVerifiedStats.map(stat => (
                <a
                  className="khala-focus grid gap-1 border border-khala-border/60 bg-khala-surface-raised p-3"
                  href={stat.sourceUrl}
                  key={stat.value}
                >
                  <strong className="text-lg text-white">{stat.value}</strong>
                  <span className="text-base/7 text-khala-text-muted">
                    {stat.label}
                  </span>
                  <span className="font-mono text-sm text-khala-energy-soft">
                    {stat.source}
                  </span>
                </a>
              ))}
            </div>
          </section>
        ) : null}
      </main>
    </PageShell>
  )
}
