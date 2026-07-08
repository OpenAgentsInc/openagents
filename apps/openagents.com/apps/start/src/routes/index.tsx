import { createFileRoute } from '@tanstack/react-router'

import CTA from '@/components/launch-ui/sections/cta/default'
import FAQ from '@/components/launch-ui/sections/faq/default'
import Footer from '@/components/launch-ui/sections/footer/default'
import Hero from '@/components/launch-ui/sections/hero/default'
import Items from '@/components/launch-ui/sections/items/default'
import Logos from '@/components/launch-ui/sections/logos/default'
import Navbar from '@/components/launch-ui/sections/navbar/default'
import Pricing from '@/components/launch-ui/sections/pricing/default'
import Stats from '@/components/launch-ui/sections/stats/default'
import { LayoutLines } from '@/components/launch-ui/ui/layout-lines'

export const Route = createFileRoute('/')({
  component: LandingPage,
  head: () => ({
    meta: [
      { title: 'Launch UI Replica' },
      {
        name: 'description',
        content:
          'Phase-1 Launch UI replica running on the OpenAgents TanStack Start staging app.',
      },
    ],
  }),
})

export function LandingPage() {
  return (
    <main
      className="min-h-dvh w-full bg-background text-foreground"
      data-launch-ui-replica="blue-minimal"
      data-route="landing"
    >
      <LayoutLines />
      <Navbar />
      <Hero />
      <Logos />
      <Items />
      <Stats />
      <Pricing />
      <FAQ />
      <CTA />
      <Footer showModeToggle={false} />
    </main>
  )
}
