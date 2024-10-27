import { Container } from "@/components/lander/container"
import { BentoCard } from "@/components/lander/bento-card"
import { LinkedAvatars } from "@/components/lander/linked-avatars"
import { LogoTimeline } from "@/components/lander/logo-timeline"
import { Heading, Subheading } from "@/components/lander/text"

export function DarkBentoSection() {
  return (
    <div className="mx-2 mt-2 rounded-4xl bg-background py-32">
      <Container>
        <Subheading>Business Automation</Subheading>
        <Heading as="h3" className="mt-2 max-w-3xl">
          Transform your business with intelligent AI agents
        </Heading>

        <div className="mt-10 grid grid-cols-1 gap-4 sm:mt-16 lg:grid-cols-6 lg:grid-rows-2">
          <BentoCard
            dark
            eyebrow="Process Automation"
            title="Automate repetitive tasks"
            description="Deploy AI agents to handle routine operations, from data entry to document processing. Built on open source technology you can trust."
            graphic={
              <div className="h-80 bg-[url(/screenshots/networking.png)] bg-[size:851px_344px] bg-no-repeat" />
            }
            fade={['top']}
            className="max-lg:rounded-t-4xl lg:col-span-4 lg:rounded-tl-4xl"
          />
          <BentoCard
            dark
            eyebrow="Customization"
            title="Build custom agents"
            description="Create specialized AI agents tailored to your business needs using our intuitive agent builder and plugin system."
            graphic={<LogoTimeline />}
            className="z-10 !overflow-visible lg:col-span-2 lg:rounded-tr-4xl"
          />
          <BentoCard
            dark
            eyebrow="Integration"
            title="Seamless connectivity"
            description="Connect your agents to existing tools and workflows with our open protocol system and extensive plugin marketplace."
            graphic={<LinkedAvatars />}
            className="lg:col-span-2 lg:rounded-bl-4xl"
          />
          <BentoCard
            dark
            eyebrow="Control"
            title="Full transparency"
            description="Monitor and manage your AI agents with complete visibility. Understand exactly how they work and maintain control over your automation."
            graphic={
              <div className="h-80 bg-[url(/img/demo2.png)] bg-cover bg-center" />
            }
            fade={['top']}
            className="max-lg:rounded-b-4xl lg:col-span-4 lg:rounded-br-4xl"
          />
        </div>
      </Container>
    </div>
  )
}