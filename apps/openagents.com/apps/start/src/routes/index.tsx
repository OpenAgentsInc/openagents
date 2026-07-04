import { createFileRoute } from '@tanstack/react-router'
import { ArrowRight } from 'lucide-react'

export const Route = createFileRoute('/')({
  component: LandingPage,
  head: () => ({
    meta: [
      { title: 'OpenAgents' },
      {
        name: 'description',
        content:
          'OpenAgents builds public, verifiable AI agents for coding, research, payments, and operational work.',
      },
    ],
  }),
})

export function LandingPage() {
  return (
    <section
      data-route="landing"
      className="relative flex min-h-dvh w-full items-center justify-center overflow-hidden bg-khala-void px-6 py-16 text-khala-text"
    >
      <StaticLandingFold />
      <div
        data-landing-wordmark="openagents"
        className="relative z-10 flex flex-col items-center justify-center gap-10 text-center"
      >
        <h1 className="select-none text-balance text-5xl font-semibold text-white sm:text-7xl lg:text-8xl">
          OpenAgents
        </h1>
        <nav
          aria-label="OpenAgents landing actions"
          className="flex flex-col items-center gap-4 sm:flex-row sm:gap-5"
        >
          <LandingAction href="/khala">What is Khala?</LandingAction>
          <LandingAction href="/tassadar">
            Join the Tassadar training run
          </LandingAction>
        </nav>
      </div>
    </section>
  )
}

function LandingAction({
  children,
  href,
}: {
  children: string
  href: string
}) {
  return (
    <a
      className="khala-focus khala-glow group inline-flex min-h-12 items-center justify-center gap-2 rounded-full border border-khala-border-strong/55 bg-khala-surface-raised/80 py-3 pr-5 pl-7 font-mono text-sm font-semibold text-khala-text backdrop-blur-md transition-all duration-300 ease-out hover:border-khala-energy-cyan/85 hover:text-white hover:khala-glow-strong motion-reduce:transition-none"
      href={href}
    >
      <span>{children}</span>
      <ArrowRight
        aria-hidden="true"
        className="size-4 text-khala-energy-cyan transition-transform duration-300 ease-out group-hover:translate-x-0.5 motion-reduce:transition-none"
      />
    </a>
  )
}

function StaticLandingFold() {
  const nodes = [
    'left-[11%] top-[16%] size-1.5 opacity-70',
    'left-[18%] top-[62%] size-2 opacity-55',
    'left-[28%] top-[32%] size-1 opacity-45',
    'left-[37%] top-[75%] size-1.5 opacity-65',
    'left-[52%] top-[18%] size-2 opacity-60',
    'left-[66%] top-[67%] size-1 opacity-50',
    'left-[78%] top-[26%] size-1.5 opacity-70',
    'left-[87%] top-[58%] size-2 opacity-45',
  ]

  return (
    <div aria-hidden="true" className="absolute inset-0 z-0">
      <div className="absolute inset-0 bg-[radial-gradient(circle_at_50%_45%,rgba(58,123,255,0.22),transparent_30%),radial-gradient(circle_at_50%_100%,rgba(79,208,255,0.16),transparent_34%),linear-gradient(180deg,rgba(0,0,0,0.25),#000_78%)]" />
      <div className="absolute inset-0 opacity-35 [background-image:linear-gradient(rgba(58,123,255,0.16)_1px,transparent_1px),linear-gradient(90deg,rgba(58,123,255,0.16)_1px,transparent_1px)] [background-size:4rem_4rem] [mask-image:radial-gradient(circle_at_50%_45%,black,transparent_70%)]" />
      <div className="absolute top-1/2 left-1/2 aspect-square w-[min(76vw,34rem)] -translate-1/2 rounded-full border border-khala-energy/20 bg-khala-energy/5 khala-glow" />
      <div className="absolute top-1/2 left-1/2 aspect-square w-[min(48vw,22rem)] -translate-1/2 rotate-45 border border-khala-energy-cyan/25 bg-khala-surface/40 khala-glow" />
      {nodes.map((nodeClass) => (
        <span
          key={nodeClass}
          className={`absolute rounded-xs bg-khala-energy-cyan shadow-[0_0_18px_rgba(79,208,255,0.75)] ${nodeClass}`}
        />
      ))}
    </div>
  )
}
