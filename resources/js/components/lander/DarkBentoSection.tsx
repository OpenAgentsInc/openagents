import { Container } from "@/components/lander/container"
import { BentoCard } from "@/components/lander/bento-card"
import { LinkedAvatars } from "@/components/lander/linked-avatars"
import { LogoTimeline } from "@/components/lander/logo-timeline"
import { Heading, Subheading } from "@/components/lander/text"

export function DarkBentoSection() {
  return (
    <div className="mx-2 mt-2 rounded-4xl bg-background py-32">
      <Container>
        <Subheading>Platform Features</Subheading>
        <Heading as="h3" className="mt-2 max-w-3xl">
          Build and deploy AI agents with complete freedom.
        </Heading>

        <div className="mt-10 grid grid-cols-1 gap-4 sm:mt-16 lg:grid-cols-6 lg:grid-rows-2">
          <BentoCard
            dark
            eyebrow="Development"
            title="Open Source Foundation"
            description="Build AI agents using our fully open source stack - from models to protocols. Complete transparency and control over your agent's behavior."
            graphic={
              <div className="h-80 bg-[url(/screenshots/networking.png)] bg-[size:851px_344px] bg-no-repeat" />
            }
            fade={['top']}
            className="max-lg:rounded-t-4xl lg:col-span-4 lg:rounded-tl-4xl"
          />
          <BentoCard
            dark
            eyebrow="Marketplace"
            title="Buy & Sell Agents"
            description="Access a thriving marketplace of AI agents or monetize your own creations in an open economy."
            graphic={<LogoTimeline />}
            // `!overflow-visible` is needed to work around a Chrome bug that disables the mask on the graphic.
            className="z-10 !overflow-visible lg:col-span-2 lg:rounded-tr-4xl"
          />
          <BentoCard
            dark
            eyebrow="Integration"
            title="Universal Compatibility"
            description="Deploy your agents anywhere with our open protocols and extensive integration options."
            graphic={<LinkedAvatars />}
            className="lg:col-span-2 lg:rounded-bl-4xl"
          />
          <BentoCard
            dark
            eyebrow="Community"
            title="Join the Movement"
            description="Be part of the open agent economy. Collaborate with developers worldwide to shape the future of AI agents."
            graphic={
              <div className="h-80 bg-[url(/screenshots/engagement.png)] bg-[size:851px_344px] bg-no-repeat" />
            }
            fade={['top']}
            className="max-lg:rounded-b-4xl lg:col-span-4 lg:rounded-br-4xl"
          />
        </div>
      </Container>
    </div>
  )
}