import { useEffect, useState } from 'react'
import {
  ArrowRight,
  BadgeCheck,
  Boxes,
  Coins,
  GaugeCircle,
  GitBranch,
  ReceiptText,
  ShieldCheck,
  Terminal,
} from 'lucide-react'

import FAQ from '@/components/launch-ui/sections/faq/default'
import Footer from '@/components/launch-ui/sections/footer/default'
import Items from '@/components/launch-ui/sections/items/default'
import Stats from '@/components/launch-ui/sections/stats/default'
import { Badge } from '@/components/launch-ui/ui/badge'
import { Card } from '@/components/launch-ui/ui/card'
import Glow from '@/components/launch-ui/ui/glow'
import { LinkButton } from '@/components/launch-ui/ui/link-button'
import {
  Navbar as NavbarShell,
  NavbarLeft,
  NavbarRight,
} from '@/components/launch-ui/ui/navbar'
import { Section } from '@/components/launch-ui/ui/section'

import {
  SALES_LANDING_LINKS,
  fetchKhalaCodePlans,
  fetchKhalaTokensServed,
  formatCount,
  LIVE_VALUE_PENDING,
  type KhalaCodePlanCatalogProjection,
} from './-sales-landing-data'
import {
  fetchPylonStats,
  type PylonStatsSnapshot,
} from './-pylon-network'

// WEB-1 sales-landing PREVIEW page (GitHub issue #8565).
//
// Reviewable, rollback-safe scaffold served ONLY at `/preview/sales-landing`
// (alongside the existing `/preview/landing`). It does NOT touch the app root
// or any live product route; rollback = this route simply is not linked.
//
// Ports the launch-ui landing kit (MIT, vendored under
// `@/components/launch-ui`, THIRD_PARTY_NOTICES.md) themed to the single
// Protoss-blue theme (no light/dark toggle). The two sections whose value is
// LIVE data — network stats and pricing — fetch real public projections at
// runtime (see `-sales-landing-data.ts`); everything else is honest reuse of
// already-approved in-repo copy or clearly-labeled `TODO(owner-copy)`
// placeholders. No persuasive final marketing copy is authored here: homepage
// copy is owner-gated by standing policy, and the final words plus the
// production-flip decision are recorded owner gates (see NEEDS_OWNER.md).

// ---------------------------------------------------------------------------
// Preview banner
// ---------------------------------------------------------------------------
function PreviewBanner() {
  return (
    <div
      className="border-b border-border/40 bg-black px-4 py-2 text-center font-mono text-xs text-muted-foreground"
      data-sales-landing-preview-banner=""
    >
      preview — proposed sales landing, not the live homepage · copy pending
      owner sign-off (#8565)
    </div>
  )
}

// ---------------------------------------------------------------------------
// Navbar — built from vendored launch-ui navbar primitives so none of the
// upstream launch-ui promo chrome (course banner, version badge, X/GitHub
// marketing icons) leaks into an OpenAgents surface.
// ---------------------------------------------------------------------------
const NAV_LINKS = [
  { text: 'Khala', href: SALES_LANDING_LINKS.khala },
  { text: 'For business', href: SALES_LANDING_LINKS.businessIntake },
  { text: 'Promises', href: SALES_LANDING_LINKS.promises },
  { text: 'Docs', href: SALES_LANDING_LINKS.docs },
  { text: 'Stats', href: SALES_LANDING_LINKS.stats },
]

function SalesNavbar() {
  return (
    <header className="relative z-50 border-b border-border/10 px-4">
      <div className="max-w-container relative mx-auto">
        <NavbarShell className="py-4">
          <NavbarLeft className="gap-7">
            <a
              href="/preview/sales-landing"
              className="flex items-center gap-2 text-lg font-semibold text-foreground"
            >
              <Boxes className="size-5 text-brand" aria-hidden="true" />
              <span>OpenAgents</span>
            </a>
            <nav
              aria-label="OpenAgents sections"
              className="hidden items-center gap-7 md:flex"
            >
              {NAV_LINKS.map((link) => (
                <a
                  key={link.href}
                  href={link.href}
                  className="text-sm font-medium text-muted-foreground transition-colors hover:text-foreground"
                >
                  {link.text}
                </a>
              ))}
            </nav>
          </NavbarLeft>
          <NavbarRight className="gap-2">
            <a
              href={SALES_LANDING_LINKS.businessIntake}
              className="hidden text-sm font-medium text-muted-foreground transition-colors hover:text-foreground md:block"
            >
              Start a project
            </a>
            <LinkButton href={SALES_LANDING_LINKS.talkToSarah} size="sm">
              Talk to Sarah
            </LinkButton>
          </NavbarRight>
        </NavbarShell>
      </div>
    </header>
  )
}

// ---------------------------------------------------------------------------
// Hero — headline + subhead reuse the already-approved in-repo copy from the
// existing `/preview/landing` candidate (`-landing-preview-page.tsx`), so no
// new persuasive copy is authored. Primary CTAs are the two the issue names:
// Talk to Sarah + business intake.
// ---------------------------------------------------------------------------
function SalesHero() {
  return (
    <Section className="fade-bottom relative overflow-hidden pb-0 sm:pb-0 md:pb-0">
      <div className="max-w-container mx-auto flex flex-col items-center gap-8 pt-12 text-center sm:gap-12 sm:pt-20">
        <Badge variant="outline" className="gap-2 px-3 py-1.5">
          <span className="text-muted-foreground">
            Preview — pending owner copy sign-off
          </span>
        </Badge>
        {/* Reused-approved copy (verbatim from -landing-preview-page.tsx). */}
        <h1 className="from-foreground to-brand-foreground relative z-10 inline-block max-w-[18ch] bg-linear-to-r bg-clip-text text-4xl leading-tight font-semibold text-balance text-transparent sm:text-6xl sm:leading-tight md:text-7xl md:leading-tight">
          Software, built by agents.
        </h1>
        <p className="text-md text-muted-foreground relative z-10 max-w-[620px] font-medium text-balance sm:text-xl">
          One open network where coding agents do real work — yours, or ours.
          Every outcome lands with verifiable receipts.
        </p>
        <div className="relative z-10 flex flex-col justify-center gap-4 sm:flex-row">
          <LinkButton
            href={SALES_LANDING_LINKS.talkToSarah}
            variant="default"
            iconRight={<ArrowRight className="size-4" />}
          >
            Talk to Sarah
          </LinkButton>
          <LinkButton
            href={SALES_LANDING_LINKS.businessIntake}
            variant="glow"
          >
            Start a project
          </LinkButton>
        </div>
        <div className="relative h-12 w-full">
          <Glow variant="center" className="opacity-60" />
        </div>
      </div>
    </Section>
  )
}

// ---------------------------------------------------------------------------
// Live network stats — WIRED to real public projections:
//   tokens served -> GET /api/public/khala-tokens-served (tokensServed)
//   pylons online / work-ready / training -> GET /api/public/pylon-stats
// Fail-soft: any error keeps the "—" placeholder. SSR / no-JS renders "—"
// until the client fetch resolves (same posture as the /pylons route).
// ---------------------------------------------------------------------------
function LiveStats() {
  const [tokensServed, setTokensServed] = useState<number | null>(null)
  const [pylons, setPylons] = useState<PylonStatsSnapshot | null>(null)

  useEffect(() => {
    let cancelled = false
    const poll = async (): Promise<void> => {
      const [tokens, pylonSnapshot] = await Promise.all([
        fetchKhalaTokensServed(),
        fetchPylonStats(),
      ])
      if (cancelled) return
      setTokensServed(tokens?.tokensServed ?? null)
      setPylons(pylonSnapshot)
    }
    void poll()
    const timer = setInterval(() => void poll(), 15000)
    return () => {
      cancelled = true
      clearInterval(timer)
    }
  }, [])

  const items = [
    {
      label: 'served',
      value: formatCount(tokensServed),
      description: 'real input + output tokens across OpenAgents products',
    },
    {
      label: 'online now',
      value: formatCount(pylons?.pylonsOnlineNow ?? null),
      description: 'Pylon nodes with a live heartbeat',
    },
    {
      label: 'work-ready',
      value: formatCount(pylons?.pylonsAssignmentReadyNow ?? null),
      description: 'Pylons ready to accept assignments now',
    },
    {
      label: 'contributing',
      value: formatCount(pylons?.trainingModelProgressContributors ?? null),
      description: 'contributors to live training-model progress',
    },
  ]

  return (
    <div data-sales-landing-live-stats="">
      <div className="max-w-container mx-auto px-4 pt-8 text-center">
        <p className="font-mono text-xs uppercase tracking-wide text-muted-foreground">
          Live network activity · public projections, no dummy values
        </p>
      </div>
      <Stats items={items} className="pt-6" />
    </div>
  )
}

// ---------------------------------------------------------------------------
// Features — vendored Items section. Titles are neutral functional labels;
// descriptions are verbatim already-approved in-repo product facts (from the
// existing `/preview/landing` door cards). Final feature copy is owner-gated.
// ---------------------------------------------------------------------------
const FEATURE_ITEMS = [
  {
    title: 'Open source',
    description: '100% open source.',
    icon: <GitBranch className="size-5 stroke-[1.25]" />,
  },
  {
    title: 'Free API',
    description: 'OpenAI-compatible free API — one base URL swap.',
    icon: <Terminal className="size-5 stroke-[1.25]" />,
  },
  {
    title: 'Token accounting',
    description: 'Exact public token accounting.',
    icon: <GaugeCircle className="size-5 stroke-[1.25]" />,
  },
  {
    title: 'Human review',
    description: 'Human-review gate before publish, send, or spend.',
    icon: <ShieldCheck className="size-5 stroke-[1.25]" />,
  },
  {
    title: 'Receipts',
    description: 'Receipts on every accepted outcome.',
    icon: <ReceiptText className="size-5 stroke-[1.25]" />,
  },
  {
    title: 'Dollars or Bitcoin',
    description: 'Pay in dollars or Bitcoin.',
    icon: <Coins className="size-5 stroke-[1.25]" />,
  },
]

function SalesFeatures() {
  return (
    <Items
      title="One open network, verifiable end to end"
      items={FEATURE_ITEMS}
    />
  )
}

// ---------------------------------------------------------------------------
// Live pricing — WIRED to the public Khala Code plan catalog
// (GET /api/public/khala-code/plans), the only public, config-backed pricing
// projection. Card-checkout credit packs (STRIPE_CREDIT_PACKAGES_JSON) and the
// mobile IAP catalog are server-side only and not exposed publicly, so they
// are intentionally not shown here. Honest: the paid plan advertises its real
// purchasability state from the catalog and never a fabricated price.
// ---------------------------------------------------------------------------
function PlanCard({
  plan,
}: Readonly<{
  plan: KhalaCodePlanCatalogProjection['plans'][number]
}>) {
  const purchasable = plan.purchase?.armed ?? plan.kind === 'free'
  return (
    <Card className="glass-2 flex flex-col gap-6 rounded-2xl p-8">
      <header className="flex flex-col gap-2">
        <div className="flex items-center gap-2">
          <BadgeCheck className="size-4 text-brand" aria-hidden="true" />
          <h3 className="text-lg font-semibold text-foreground">
            {plan.label}
          </h3>
          {plan.isDefault ? <Badge variant="outline">Default</Badge> : null}
        </div>
        <p className="text-muted-foreground text-sm">{plan.tagline}</p>
      </header>
      <div className="text-3xl font-semibold text-foreground">
        {plan.priceLabel}
      </div>
      <ul className="flex flex-col gap-2">
        {plan.terms.slice(0, 3).map((term) => (
          <li
            key={term}
            className="text-muted-foreground flex gap-2 text-sm text-pretty"
          >
            <span aria-hidden="true" className="text-brand">
              ·
            </span>
            {term}
          </li>
        ))}
      </ul>
      <LinkButton
        href={
          plan.kind === 'free'
            ? SALES_LANDING_LINKS.khala
            : SALES_LANDING_LINKS.businessIntake
        }
        variant={purchasable ? 'default' : 'glow'}
        size="lg"
      >
        {plan.kind === 'free'
          ? 'Get started'
          : purchasable
            ? 'Talk to us'
            : 'Not yet purchasable'}
      </LinkButton>
    </Card>
  )
}

function LivePricing() {
  const [catalog, setCatalog] = useState<KhalaCodePlanCatalogProjection | null>(
    null,
  )

  useEffect(() => {
    let cancelled = false
    void fetchKhalaCodePlans().then((next) => {
      if (!cancelled) setCatalog(next)
    })
    return () => {
      cancelled = true
    }
  }, [])

  return (
    <Section data-sales-landing-live-pricing="">
      <div className="mx-auto flex max-w-6xl flex-col items-center gap-10">
        <div className="flex flex-col items-center gap-4 px-4 text-center">
          <h2 className="text-3xl leading-tight font-semibold sm:text-4xl">
            Plans
          </h2>
          <p className="text-muted-foreground max-w-[560px] font-medium">
            {/* Live catalog summary when available; honest fallback otherwise. */}
            {catalog?.summary ??
              'Live plan catalog loads from the public Khala Code plans projection.'}
          </p>
        </div>
        {catalog === null ? (
          <p
            className="text-muted-foreground font-mono text-sm"
            role="status"
            data-pricing-state="pending"
          >
            {LIVE_VALUE_PENDING} loading live plan catalog…
          </p>
        ) : (
          <div className="grid w-full grid-cols-1 gap-6 sm:grid-cols-2">
            {catalog.plans.map((plan) => (
              <PlanCard key={plan.planId} plan={plan} />
            ))}
          </div>
        )}
      </div>
    </Section>
  )
}

// ---------------------------------------------------------------------------
// FAQ — vendored accordion section. All Q&A is TODO(owner-copy): no FAQ copy
// is approved yet, so nothing persuasive is authored. Placeholders are clearly
// labeled and get replaced at owner copy sign-off.
// ---------------------------------------------------------------------------
const FAQ_TODO_ITEMS = [
  {
    question: 'TODO(owner-copy): How does OpenAgents actually build software?',
    answer: (
      <p className="text-muted-foreground mb-4 max-w-[640px] text-balance">
        TODO(owner-copy) — placeholder pending owner copy sign-off (#8565).
      </p>
    ),
  },
  {
    question: 'TODO(owner-copy): What do I pay, and how?',
    answer: (
      <p className="text-muted-foreground mb-4 max-w-[640px] text-balance">
        TODO(owner-copy) — placeholder pending owner copy sign-off (#8565).
      </p>
    ),
  },
  {
    question: 'TODO(owner-copy): What happens to my data and privacy?',
    answer: (
      <p className="text-muted-foreground mb-4 max-w-[640px] text-balance">
        TODO(owner-copy) — placeholder pending owner copy sign-off (#8565).
      </p>
    ),
  },
  {
    question: 'TODO(owner-copy): How do I get started?',
    answer: (
      <p className="text-muted-foreground mb-4 max-w-[640px] text-balance">
        TODO(owner-copy) — placeholder pending owner copy sign-off (#8565).
      </p>
    ),
  },
]

function SalesFaq() {
  return <FAQ title="Questions and answers" items={FAQ_TODO_ITEMS} />
}

// ---------------------------------------------------------------------------
// CTA — headline is TODO(owner-copy). Buttons are the two named CTAs.
// ---------------------------------------------------------------------------
function SalesCta() {
  return (
    <Section className="group relative overflow-hidden">
      <div className="max-w-container relative z-10 mx-auto flex flex-col items-center gap-8 text-center">
        <h2 className="max-w-[640px] text-3xl leading-tight font-semibold text-balance sm:text-4xl sm:leading-tight">
          {/* TODO(owner-copy): final CTA headline pending owner sign-off (#8565). */}
          TODO(owner-copy): closing call-to-action headline
        </h2>
        <div className="flex flex-col justify-center gap-4 sm:flex-row">
          <LinkButton
            href={SALES_LANDING_LINKS.talkToSarah}
            variant="default"
            iconRight={<ArrowRight className="size-4" />}
          >
            Talk to Sarah
          </LinkButton>
          <LinkButton href={SALES_LANDING_LINKS.businessIntake} variant="glow">
            Start a project
          </LinkButton>
        </div>
      </div>
      <div className="absolute top-0 left-0 h-full w-full translate-y-[1rem] opacity-70 transition-all duration-500 ease-in-out group-hover:translate-y-[-1rem] group-hover:opacity-100">
        <Glow variant="bottom" />
      </div>
    </Section>
  )
}

// ---------------------------------------------------------------------------
// Footer — OpenAgents links to real surfaces. No mode toggle (single theme).
// ---------------------------------------------------------------------------
const FOOTER_COLUMNS = [
  {
    title: 'Product',
    links: [
      { text: 'Khala', href: SALES_LANDING_LINKS.khala },
      { text: 'For business', href: SALES_LANDING_LINKS.businessIntake },
      { text: 'Docs', href: SALES_LANDING_LINKS.docs },
    ],
  },
  {
    title: 'Evidence',
    links: [
      { text: 'Product promises', href: SALES_LANDING_LINKS.promises },
      { text: 'Network stats', href: SALES_LANDING_LINKS.stats },
      { text: 'Promises JSON', href: SALES_LANDING_LINKS.promisesJson },
    ],
  },
  {
    title: 'Community',
    links: [
      { text: 'Forum', href: SALES_LANDING_LINKS.forum },
      { text: 'GitHub', href: SALES_LANDING_LINKS.github },
      { text: 'Talk to Sarah', href: SALES_LANDING_LINKS.talkToSarah },
    ],
  },
]

function SalesFooter() {
  return (
    <Footer
      logo={<Boxes className="size-5 text-brand" aria-hidden="true" />}
      name="OpenAgents"
      columns={FOOTER_COLUMNS}
      copyright="© 2026 OpenAgents. All rights reserved."
      policies={[
        { text: 'Privacy', href: '/privacy' },
        { text: 'Terms', href: '/terms' },
      ]}
      showModeToggle={false}
    />
  )
}

// ---------------------------------------------------------------------------
export function SalesLandingPage() {
  return (
    <main
      className="min-h-dvh w-full bg-background text-foreground"
      data-route="sales-landing-preview"
      data-sales-landing-preview=""
    >
      <PreviewBanner />
      <SalesNavbar />
      <SalesHero />
      <LiveStats />
      <SalesFeatures />
      <LivePricing />
      <SalesFaq />
      <SalesCta />
      <SalesFooter />
    </main>
  )
}
