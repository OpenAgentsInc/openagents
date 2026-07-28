import {
  ArrowRight,
  BadgeCheck,
  Bot,
  Boxes,
  Clock3,
  Coins,
  ListChecks,
  ReceiptText,
  ShieldCheck,
} from 'lucide-react'

import FAQ from '@/components/launch-ui/sections/faq/default'
import Footer from '@/components/launch-ui/sections/footer/default'
import Items from '@/components/launch-ui/sections/items/default'
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

import { businessPackages } from './-funnel-data'

// /work — public sales landing page (owner direction 2026-07-28).
//
// Positioning: OpenAgents sells AI employees and agent fleets that do real
// business work with human verification and receipts. Every CTA drives into
// the Sarah sales agent at sarah.openagents.com — the conversation IS the
// product demo. Pricing reuses the already-public /business rate card
// verbatim from `-funnel-data.ts` (single source, no new numbers).

const SARAH_URL = 'https://sarah.openagents.com'

// ---------------------------------------------------------------------------
// Navbar
// ---------------------------------------------------------------------------
const NAV_LINKS = [{ text: 'For business', href: '/business' }]

function WorkNavbar() {
  return (
    <header className="relative z-50 border-b border-border/10 px-4">
      <div className="max-w-container relative mx-auto">
        <NavbarShell className="py-4">
          <NavbarLeft className="gap-7">
            <a
              href="/work"
              className="text-lg font-semibold text-foreground"
            >
              OpenAgents
            </a>
            <nav
              aria-label="OpenAgents sections"
              className="hidden items-center gap-7 md:flex"
            >
              {NAV_LINKS.map(link => (
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
            <LinkButton href={SARAH_URL} size="sm">
              Talk to Sarah
            </LinkButton>
          </NavbarRight>
        </NavbarShell>
      </div>
    </header>
  )
}

// ---------------------------------------------------------------------------
// Hero
// ---------------------------------------------------------------------------
function WorkHero() {
  return (
    <Section className="fade-bottom relative overflow-hidden py-8 pb-0 sm:py-10 sm:pb-0 md:py-12 md:pb-0">
      <div className="max-w-container mx-auto flex flex-col items-center gap-6 pt-2 text-center sm:gap-8 sm:pt-4">
        <Badge variant="outline" className="gap-2 px-3 py-1.5">
          <span className="text-muted-foreground">
            Our sales rep is an AI. You&apos;ll be talking to the product.
          </span>
        </Badge>
        <h1 className="relative z-10 inline-block max-w-[16ch] text-4xl leading-tight font-semibold text-balance text-foreground sm:text-6xl sm:leading-tight md:text-7xl md:leading-tight">
          AI employees that work.
        </h1>
        <p className="text-md text-muted-foreground relative z-10 max-w-[640px] font-medium text-balance sm:text-xl">
          OpenAgents builds agent fleets that do real business work — software,
          lead generation, QA, operations — with human verification and a
          receipt on every accepted outcome.
        </p>
        <div className="relative z-10 flex flex-col justify-center gap-4 sm:flex-row">
          <LinkButton
            href={SARAH_URL}
            variant="default"
            size="lg"
            iconRight={<ArrowRight className="size-4" />}
          >
            Talk to Sarah — our AI sales employee
          </LinkButton>
          <LinkButton href="#offers" variant="glow" size="lg">
            See the offers
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
// Features
// ---------------------------------------------------------------------------
const FEATURE_ITEMS = [
  {
    title: 'Employees, not tools',
    description:
      'Each agent ships as a hire: a role, a permission stack, skills, a schedule, and proof of work.',
    icon: <Bot className="size-5 stroke-[1.25]" />,
  },
  {
    title: 'Agents that work',
    description:
      'Fleets build and ship software, run lead generation, QA your site, and automate operations.',
    icon: <Boxes className="size-5 stroke-[1.25]" />,
  },
  {
    title: 'Verified, not vibes',
    description:
      'Human verification gates the work, and every accepted outcome carries a receipt you can check.',
    icon: <ReceiptText className="size-5 stroke-[1.25]" />,
  },
  {
    title: 'Promoted, never unleashed',
    description:
      'Authority is explicit and staged: observe, draft, act with approval, act within policy.',
    icon: <ShieldCheck className="size-5 stroke-[1.25]" />,
  },
  {
    title: 'Always on',
    description:
      'Your AI employee does not sleep when your laptop does. Managed, supervised, running.',
    icon: <Clock3 className="size-5 stroke-[1.25]" />,
  },
  {
    title: 'Dollars or Bitcoin',
    description:
      'Pay by card or Lightning. Compute credits never expire, and prepaying earns bonus credits.',
    icon: <Coins className="size-5 stroke-[1.25]" />,
  },
]

function WorkFeatures() {
  return (
    <Items title="What hiring an AI employee gets you" items={FEATURE_ITEMS} />
  )
}

// ---------------------------------------------------------------------------
// Meet Sarah — the product demoing itself.
// ---------------------------------------------------------------------------
const SARAH_CARD_CAN = [
  'Clearly labeled AI — never pretends to be human',
  'Quotes public pricing only, with no improvised discounts',
  'Closes deals up to $10,000 with a real payment link',
  'Escalates bigger deals to the founder, a human',
]

const SARAH_CARD_CANT = [
  'Cannot send email, spend money, or touch your systems',
  'Cannot invent case studies, metrics, or guarantees',
]

function MeetSarah() {
  return (
    <Section data-work-meet-sarah="">
      <div className="mx-auto flex max-w-4xl flex-col items-center gap-10 px-4">
        <div className="flex flex-col items-center gap-4 text-center">
          <h2 className="text-3xl leading-tight font-semibold sm:text-4xl">
            Meet Sarah. She&apos;s in sales — and she&apos;s the demo.
          </h2>
          <p className="text-muted-foreground max-w-[600px] font-medium">
            You are not booking a call with a founder. You talk to Sarah by
            voice or text, she qualifies you, quotes real prices, and can close
            a deal herself under a stated authority ceiling. If she is a good
            employee, that is the pitch.
          </p>
        </div>
        <Card className="glass-2 w-full rounded-2xl p-8">
          <div className="flex flex-col gap-6 sm:flex-row sm:gap-10">
            <div className="flex-1">
              <h3 className="mb-3 text-sm font-semibold tracking-wide text-foreground uppercase">
                Sarah&apos;s employee card
              </h3>
              <ul className="flex flex-col gap-2">
                {SARAH_CARD_CAN.map(line => (
                  <li
                    key={line}
                    className="text-muted-foreground flex gap-2 text-sm"
                  >
                    <BadgeCheck
                      className="mt-0.5 size-4 shrink-0 text-brand"
                      aria-hidden="true"
                    />
                    {line}
                  </li>
                ))}
                {SARAH_CARD_CANT.map(line => (
                  <li
                    key={line}
                    className="text-muted-foreground flex gap-2 text-sm"
                  >
                    <ShieldCheck
                      className="mt-0.5 size-4 shrink-0 text-muted-foreground"
                      aria-hidden="true"
                    />
                    {line}
                  </li>
                ))}
              </ul>
            </div>
            <div className="flex flex-col items-start justify-center gap-4 sm:w-56">
              <p className="text-muted-foreground text-sm">
                Speak or type. No forms. A human reviews every conversation.
              </p>
              <LinkButton
                href={SARAH_URL}
                variant="default"
                iconRight={<ArrowRight className="size-4" />}
              >
                Start talking
              </LinkButton>
            </div>
          </div>
        </Card>
      </div>
    </Section>
  )
}

// ---------------------------------------------------------------------------
// Offers — the public /business rate card, verbatim from -funnel-data.ts.
// ---------------------------------------------------------------------------
function WorkOffers() {
  return (
    <Section id="offers" data-work-offers="">
      <div className="mx-auto flex max-w-6xl flex-col items-center gap-10">
        <div className="flex flex-col items-center gap-4 px-4 text-center">
          <h2 className="text-3xl leading-tight font-semibold sm:text-4xl">
            Ways to start
          </h2>
          <p className="text-muted-foreground max-w-[560px] font-medium">
            Public pricing, operator-assisted delivery, receipts on everything.
            Sarah can walk you to the right one and take payment.
          </p>
        </div>
        <div className="grid w-full grid-cols-1 gap-6 px-4 sm:grid-cols-2">
          {businessPackages.map(pkg => (
            <Card
              key={pkg.title}
              className="glass-2 flex flex-col gap-5 rounded-2xl p-8"
            >
              <header className="flex flex-col gap-1">
                <h3 className="text-lg font-semibold text-foreground">
                  {pkg.title}
                </h3>
                <div className="text-2xl font-semibold text-foreground">
                  {pkg.price}
                </div>
              </header>
              <p className="text-muted-foreground text-sm">{pkg.scope}</p>
              <ul className="flex flex-col gap-2">
                {pkg.receiptPlan.map(line => (
                  <li
                    key={line}
                    className="text-muted-foreground flex gap-2 text-sm text-pretty"
                  >
                    <ListChecks
                      className="mt-0.5 size-4 shrink-0 text-brand"
                      aria-hidden="true"
                    />
                    {line}
                  </li>
                ))}
              </ul>
              <p className="text-muted-foreground text-xs">{pkg.caveat}</p>
              <LinkButton href={SARAH_URL} variant="default" size="lg">
                Talk to Sarah about {pkg.title}
              </LinkButton>
            </Card>
          ))}
        </div>
      </div>
    </Section>
  )
}

// ---------------------------------------------------------------------------
// FAQ
// ---------------------------------------------------------------------------
const FAQ_ITEMS = [
  {
    question: 'Am I really buying from an AI?',
    answer: (
      <p className="text-muted-foreground mb-4 max-w-[640px] text-balance">
        Yes, and she says so up front. Sarah quotes public pricing only and can
        close deals up to $10,000 with a real payment link. Anything bigger, or
        any custom terms, she escalates to the founder — a human who reviews
        every conversation anyway.
      </p>
    ),
  },
  {
    question: 'How do I know the work actually happened?',
    answer: (
      <p className="text-muted-foreground mb-4 max-w-[640px] text-balance">
        Every engagement is scoped to verifiable outcomes before it starts, a
        human verification gate sits in front of acceptance, and accepted
        outcomes carry receipts. We build in public — the machinery that
        produces those receipts is open source.
      </p>
    ),
  },
  {
    question: 'What kind of work can agents actually do?',
    answer: (
      <p className="text-muted-foreground mb-4 max-w-[640px] text-balance">
        Software builds and fixes, QA audits, lead generation and outreach
        drafting, workflow automation, and recurring operations. Human approval
        stays in front of anything that sends, publishes, or spends. If your
        job is not a fit, Sarah will say so instead of overpromising.
      </p>
    ),
  },
  {
    question: 'What if my deal is bigger than a package?',
    answer: (
      <p className="text-muted-foreground mb-4 max-w-[640px] text-balance">
        Tell Sarah. She scopes it, writes it up, and brings in the founder for
        the terms. The packages are entry points, not the ceiling.
      </p>
    ),
  },
]

function WorkFaq() {
  return <FAQ title="Questions, answered honestly" items={FAQ_ITEMS} />
}

// ---------------------------------------------------------------------------
// CTA
// ---------------------------------------------------------------------------
function WorkCta() {
  return (
    <Section className="group relative overflow-hidden">
      <div className="max-w-container relative z-10 mx-auto flex flex-col items-center gap-8 text-center">
        <h2 className="max-w-[640px] text-3xl leading-tight font-semibold text-balance sm:text-4xl sm:leading-tight">
          Hire your first AI employee.
        </h2>
        <div className="flex flex-col justify-center gap-4 sm:flex-row">
          <LinkButton
            href={SARAH_URL}
            variant="default"
            size="lg"
            iconRight={<ArrowRight className="size-4" />}
          >
            Talk to Sarah
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
// Footer
// ---------------------------------------------------------------------------
const FOOTER_COLUMNS = [
  {
    title: 'Product',
    links: [
      { text: 'Talk to Sarah', href: SARAH_URL },
      { text: 'For business', href: '/business' },
      { text: 'Product promises', href: '/promises' },
    ],
  },
  {
    title: 'Community',
    links: [
      { text: 'Forum', href: '/forum' },
      { text: 'GitHub', href: 'https://github.com/OpenAgentsInc/openagents' },
    ],
  },
]

function WorkFooter() {
  return (
    <Footer
      logo={null}
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
export function WorkPage() {
  return (
    <main
      className="min-h-dvh w-full bg-background text-foreground"
      data-route="work"
      data-work-landing=""
    >
      <WorkNavbar />
      <WorkHero />
      <WorkFeatures />
      <MeetSarah />
      <WorkOffers />
      <WorkFaq />
      <WorkCta />
      <WorkFooter />
    </main>
  )
}
