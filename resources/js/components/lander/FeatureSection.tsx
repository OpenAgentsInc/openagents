import { Container } from "@/components/lander/container"
import { Screenshot } from "@/components/lander/screenshot"
import { Heading } from "@/components/lander/text"

export function FeatureSection() {
  return (
    <div className="overflow-hidden">
      <Container className="pb-24">
        <Heading as="h4" className="max-w-3xl">
          Command your agents, see the results and follow up immediately.
        </Heading>
        <Screenshot
          width={1216}
          height={768}
          src="/img/demo2.png"
          className="mt-16 h-[36rem] sm:h-auto sm:w-[76rem]"
        />
      </Container>
    </div>
  )
}
