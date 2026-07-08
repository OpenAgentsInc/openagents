import type { ReactNode } from "react";

import { siteConfig } from "@/components/launch-ui/config/site";
import { cn } from "@/lib/utils";

import Github from "../../logos/github";
import { Badge } from "../../ui/badge";
import Glow from "../../ui/glow";
import { LinkButton, type LinkButtonProps } from "../../ui/link-button";
import { Mockup, MockupFrame } from "../../ui/mockup";
import Screenshot from "../../ui/screenshot";
import { Section } from "../../ui/section";

interface HeroButtonProps extends Omit<LinkButtonProps, "children"> {
  text: string;
}

interface HeroProps {
  title?: string;
  description?: string;
  mockup?: ReactNode | false;
  badge?: ReactNode | false;
  buttons?: HeroButtonProps[] | false;
  className?: string;
}

const DEFAULT_HERO_BUTTONS: HeroButtonProps[] = [
  {
    href: siteConfig.getStartedUrl,
    text: "Get Started",
    variant: "default",
  },
  {
    href: siteConfig.links.github,
    text: "Github",
    variant: "glow",
    icon: <Github className="mr-2 size-4" />,
  },
];

const DEFAULT_HERO_BADGE = (
  <Badge variant="outline" className="animate-appear gap-2 px-3 py-1.5">
    <span className="text-muted-foreground">Launch UI v2 is out!</span>
    <a href={siteConfig.getStartedUrl} className="flex items-center gap-1">
      Read more
      <span aria-hidden="true">→</span>
    </a>
  </Badge>
);

const DEFAULT_HERO_MOCKUP = (
  <Screenshot
    srcLight="/dashboard-light.png"
    srcDark="/dashboard-dark.png"
    alt="Launch UI app screenshot"
    width={1248}
    height={765}
    loading="eager"
    className="w-full"
  />
);

function SocialProof() {
  const avatars = [
    "from-sky-300 to-blue-600",
    "from-orange-200 to-rose-500",
    "from-emerald-200 to-cyan-600",
  ];

  return (
    <div className="animate-appear flex flex-wrap items-center gap-3 opacity-0 delay-500">
      <div className="flex -space-x-3">
        {avatars.map((avatar, index) => (
          <span
            key={avatar}
            className={cn(
              "size-8 rounded-full border-2 border-background bg-linear-to-br shadow-md",
              avatar,
            )}
            aria-label={`Builder avatar ${index + 1}`}
          />
        ))}
      </div>
      <div className="flex items-center gap-0.5 text-foreground">
        {Array.from({ length: 5 }, (_, index) => (
          <span
            key={index}
            className="text-sm text-foreground"
            aria-hidden="true"
          >
            ★
          </span>
        ))}
      </div>
      <p className="m-0 text-sm font-medium text-muted-foreground">
        Used by {siteConfig.stats.total} companies and builders
      </p>
    </div>
  );
}

export default function Hero({
  title = "Give your big idea the design it deserves",
  description = "Professionally designed blocks and templates built with React, Shadcn/ui and Tailwind that will help your product stand out.",
  mockup = DEFAULT_HERO_MOCKUP,
  badge = DEFAULT_HERO_BADGE,
  buttons = DEFAULT_HERO_BUTTONS,
  className,
}: HeroProps) {
  return (
    <Section
      className={cn(
        "fade-bottom relative min-h-[calc(100dvh-7.75rem)] overflow-hidden px-4 pt-44 pb-0 sm:pt-52 sm:pb-0 md:pt-56 md:pb-0",
        className,
      )}
    >
      <div className="max-w-container mx-auto">
        <div className="relative z-10 flex max-w-[760px] flex-col items-start gap-7 text-left sm:gap-9">
          {badge !== false && badge}
          <h1 className="animate-appear from-foreground to-muted-foreground relative z-10 inline-block max-w-[960px] bg-linear-to-r bg-clip-text text-5xl font-semibold tracking-tight text-balance text-transparent drop-shadow-2xl sm:text-6xl md:text-7xl">
            {title}
          </h1>
          <p className="animate-appear relative z-10 max-w-[660px] text-lg font-medium text-pretty text-muted-foreground opacity-0 delay-100 sm:text-xl">
            {description}
          </p>
          {buttons !== false && buttons.length > 0 && (
            <div className="animate-appear relative z-10 flex flex-wrap justify-start gap-4 opacity-0 delay-300">
              {buttons.map((button) => (
                <LinkButton
                  key={`${button.href}-${button.text}`}
                  variant={button.variant || "default"}
                  size="lg"
                  href={button.href}
                  icon={button.icon}
                  iconRight={button.iconRight}
                >
                  {button.text}
                </LinkButton>
              ))}
            </div>
          )}
          <SocialProof />
        </div>
        {mockup !== false && (
          <div className="pointer-events-none absolute inset-x-0 bottom-[-210px] z-0 h-[520px]">
            <div className="absolute top-20 left-1/2 w-[1500px] max-w-[150vw] -translate-x-[42%] rotate-[-11deg] sm:top-8 lg:top-0">
              <MockupFrame
                className="animate-appear rounded-[18px] bg-brand/15 p-2 opacity-0 shadow-mockup delay-700"
                size="small"
              >
                <Mockup
                  type="responsive"
                  className="w-full rounded-[12px] border-0 bg-background/90"
                >
                  {mockup}
                </Mockup>
              </MockupFrame>
            </div>
            <Glow
              variant="top"
              className="animate-appear-zoom top-0 opacity-0 delay-1000"
            />
          </div>
        )}
      </div>
    </Section>
  );
}
