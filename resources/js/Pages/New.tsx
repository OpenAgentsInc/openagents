import { BentoSection } from "@/components/lander/BentoSection"
import { Container } from "@/components/lander/container"
import { DarkBentoSection } from "@/components/lander/DarkBentoSection"
import { FeatureSection } from "@/components/lander/FeatureSection"
import { Footer } from "@/components/lander/footer"
import { Hero } from "@/components/lander/Hero"
import { LogoCloud } from "@/components/lander/logo-cloud"
import { Testimonials } from "@/components/lander/testimonials"
import { Head } from "@inertiajs/react"

export default function Home() {
  return (
    <div className="overflow-hidden dark">
      <Head title="Home" />
      <Hero />
      <main>
        {/* <Container className="mt-10">
          <LogoCloud />
        </Container> */}
        {/* <div className="bg-gradient-to-b from-card from-50% to-muted py-32">
          <FeatureSection />
          <BentoSection />
        </div> */}
        {/* <DarkBentoSection /> */}
      </main>
      {/* <Testimonials /> */}
      <Footer />
    </div>
  )
}
