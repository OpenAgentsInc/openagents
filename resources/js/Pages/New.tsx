import { ChevronRightIcon } from "lucide-react"
import { BentoCard } from "@/components/lander/bento-card"
import { Button } from "@/components/lander/button"
import { Container } from "@/components/lander/container"
import { Footer } from "@/components/lander/footer"
import { Gradient } from "@/components/lander/gradient"
import { Keyboard } from "@/components/lander/keyboard"
import { Link } from "@/components/lander/link"
import { LinkedAvatars } from "@/components/lander/linked-avatars"
import { LogoCloud } from "@/components/lander/logo-cloud"
import { LogoCluster } from "@/components/lander/logo-cluster"
import { LogoTimeline } from "@/components/lander/logo-timeline"
import { Map } from "@/components/lander/map"
import { Navbar } from "@/components/lander/navbar"
import { Screenshot } from "@/components/lander/screenshot"
import { Testimonials } from "@/components/lander/testimonials"
import { Heading, Subheading } from "@/components/lander/text"

function Hero() {
  return (
    <div className="relative">
      <Gradient className="absolute inset-2 bottom-0 rounded-4xl ring-1 ring-inset ring-black/5" />
      <Container className="relative">
        <Navbar
          banner={
            <Link
              href="/blog/radiant-raises-100m-series-a-from-tailwind-ventures"
              className="flex items-center gap-1 rounded-full bg-fuchsia-950/35 px-3 py-0.5 text-sm/6 font-medium text-white data-[hover]:bg-fuchsia-950/30"
            >
              Radiant raises $100M Series A from Tailwind Ventures
              <ChevronRightIcon className="size-4" />
            </Link>
          }
        />
        <div className="pb-24 pt-16 sm:pb-32 sm:pt-24 md:pb-48 md:pt-32">
          <h1 className="font-display text-balance text-6xl/[0.9] font-medium tracking-tight text-gray-950 sm:text-8xl/[0.8] md:text-9xl/[0.8]">
            Close every deal.
          </h1>
          <p className="mt-8 max-w-lg text-xl/7 font-medium text-gray-950/75 sm:text-2xl/8">
            Radiant helps you sell more by revealing sensitive information about
            your customers.
          </p>
          <div className="mt-12 flex flex-col gap-x-6 gap-y-4 sm:flex-row">
            <Button href="#">Get started</Button>
            <Button variant="secondary" href="/pricing">
              See pricing
            </Button>
          </div>
        </div>
      </Container>
    </div>
  )
}

export default function New() {

}
