import { PropsWithChildren } from "react"
import { Container } from "@/components/lander/container"
import { Footer } from "@/components/lander/footer"
import { GradientBackground } from "@/components/lander/gradient"
import { Navbar } from "@/components/lander/navbar"

export function UnauthedLayout({ children }: PropsWithChildren) {
  return (
    <main className="overflow-hidden">
      <GradientBackground />
      <Container>
        <Navbar />
        <div className="mt-16 pb-24">
          {children}
        </div>
        <Footer />
      </Container>
    </main>
  )
}
