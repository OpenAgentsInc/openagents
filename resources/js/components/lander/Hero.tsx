import { ChevronRightIcon } from "lucide-react"
import { Button } from "@/components/lander/button"
import { Container } from "@/components/lander/container"
import { Gradient } from "@/components/lander/gradient"
import { Link } from "@/components/lander/link"
import { Navbar } from "@/components/lander/navbar"

export function Hero() {
  return (
    <div className="relative">
      <Gradient className="absolute inset-2 bottom-0 rounded-4xl ring-1 ring-inset ring-border" />
      <Container className="relative">
        <Navbar />
        <div className="pb-24 pt-16 sm:pb-32 sm:pt-24 md:pb-48 md:pt-32">
          <h1 className="font-display text-balance text-6xl/[0.9] font-medium tracking-tight text-foreground sm:text-5xl/[0.8] md:text-6xl/[0.8]">
            Automate your business.
          </h1>
          <p className="mt-8 max-w-lg text-xl/7 font-medium text-muted-foreground sm:text-2xl/8">
            OpenAgents helps you work faster with AI agents that deeply understand your business.
          </p>
          <div className="mt-12 flex flex-col gap-x-6 gap-y-4 sm:flex-row">
            <Button href="/inquire">Request a demo</Button>
            {/* <Button variant="secondary" href="/pricing">
              See pricing
            </Button> */}
          </div>
        </div>
      </Container>
    </div>
  )
}
