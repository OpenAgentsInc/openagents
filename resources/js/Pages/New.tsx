import { Container } from "@/components/lander/container"
import { Footer } from "@/components/lander/footer"
import { LogoCloud } from "@/components/lander/logo-cloud"
import { Testimonials } from "@/components/lander/testimonials"
import { Head } from "@inertiajs/react"
import { Hero } from "@/components/lander/Hero"
import { FeatureSection } from "@/components/lander/FeatureSection"
import { BentoSection } from "@/components/lander/BentoSection"
import { DarkBentoSection } from "@/components/lander/DarkBentoSection"

export default function Home() {
  return (
    <div className="overflow-hidden dark">
      <Head title="Home" />
      <Hero />
      <main>
        <Container className="mt-10">
          <LogoCloud />
        </Container>
        <div className="bg-gradient-to-b from-background from-50% to-muted py-32">
          <FeatureSection />
          <BentoSection />
        </div>
        <DarkBentoSection />
      </main>
      <Testimonials />
      <Footer />
    </div>
  )
}