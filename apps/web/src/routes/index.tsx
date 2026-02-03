import { createFileRoute } from '@tanstack/react-router';
import { AnnouncementBadge } from '@/components/elements/announcement-badge';
import { ButtonLink, PlainButtonLink } from '@/components/elements/button';
import { InstallCommand } from '@/components/elements/install-command';
import { Link as UiLink } from '@/components/elements/link';
import { Logo, LogoGrid } from '@/components/elements/logo-grid';
import { Main } from '@/components/elements/main';
import { Screenshot } from '@/components/elements/screenshot';
import { ChevronIcon } from '@/components/icons/chevron-icon';
import { GitHubIcon } from '@/components/icons/social/github-icon';
import { XIcon } from '@/components/icons/social/x-icon';
import { YouTubeIcon } from '@/components/icons/social/youtube-icon';
import { BrandCard, BrandsCardsMultiColumn } from '@/components/sections/brands-cards-multi-column';
import { CallToActionSimple } from '@/components/sections/call-to-action-simple';
import {
  Feature,
  FeaturesStackedAlternatingWithDemos,
} from '@/components/sections/features-stacked-alternating-with-demos';
import {
  FooterLink,
  FooterWithLinksAndSocialIcons,
  SocialLink,
} from '@/components/sections/footer-with-links-and-social-icons';
import { HeroCenteredWithDemo } from '@/components/sections/hero-centered-with-demo';
import {
  NavbarLink,
  NavbarLogo,
  NavbarWithLogoActionsAndLeftAlignedLinks,
} from '@/components/sections/navbar-with-logo-actions-and-left-aligned-links';
import { Stat, StatsWithGraph } from '@/components/sections/stats-with-graph';
import { TestimonialTwoColumnWithLargePhoto } from '@/components/sections/testimonial-two-column-with-large-photo';

export const Route = createFileRoute('/')({
  component: HomePage,
});

function HomePage() {
  return (
    <>
      <NavbarWithLogoActionsAndLeftAlignedLinks
        id="navbar"
        logo={
          <NavbarLogo href="/">
            <span className="font-display text-xl font-medium text-mauve-950 dark:text-white">OpenAgents</span>
          </NavbarLogo>
        }
        links={
          <>
            <NavbarLink href="/setup">Setup</NavbarLink>
            <NavbarLink href="https://github.com/OpenAgentsInc/openagents">Docs</NavbarLink>
            <NavbarLink href="/setup" className="sm:hidden">
              Log in
            </NavbarLink>
          </>
        }
        actions={
          <>
            <PlainButtonLink href="/setup" className="max-sm:hidden">
              Log in
            </PlainButtonLink>
            <ButtonLink href="/setup">Get started</ButtonLink>
          </>
        }
      />

      <Main>
        <HeroCenteredWithDemo
          id="hero"
          eyebrow={
            <AnnouncementBadge href="/setup" text="OpenAgents: runtime + compiler for autonomous agents" cta="Get started" />
          }
          headline="Agents that ship."
          subheadline={
            <p>
              Runtime, compiler, and optional market for autonomous agents. Execute tools and jobs, enforce schemas,
              record replayable receipts — then optimize behavior with signatures and metrics.
            </p>
          }
          cta={<InstallCommand className="min-w-xs" snippet="cargo run -p autopilot" />}
          demo={
            <>
              <Screenshot className="rounded-md lg:hidden" wallpaper="purple" placement="bottom-right">
                <img
                  src="https://assets.tailwindplus.com/screenshots/1.webp?left=1670&top=1408"
                  alt=""
                  width={1670}
                  height={1408}
                  className="md:hidden dark:hidden"
                />
                <img
                  src="https://assets.tailwindplus.com/screenshots/1.webp?left=1670&top=1408&color=mauve"
                  alt=""
                  width={1670}
                  height={1408}
                  className="not-dark:hidden md:hidden"
                />
                <img
                  src="https://assets.tailwindplus.com/screenshots/1.webp?left=2000&top=1408"
                  alt=""
                  width={2000}
                  height={1408}
                  className="max-md:hidden dark:hidden"
                />
                <img
                  src="https://assets.tailwindplus.com/screenshots/1.webp?left=2000&top=1408&color=mauve"
                  alt=""
                  width={2000}
                  height={1408}
                  className="not-dark:hidden max-md:hidden"
                />
              </Screenshot>
              <Screenshot className="rounded-lg max-lg:hidden" wallpaper="purple" placement="bottom">
                <img
                  className="dark:hidden"
                  src="https://assets.tailwindplus.com/screenshots/1.webp"
                  alt=""
                  width={3440}
                  height={1990}
                />
                <img
                  className="not-dark:hidden"
                  src="https://assets.tailwindplus.com/screenshots/1.webp?color=mauve"
                  alt=""
                  width={3440}
                  height={1990}
                />
              </Screenshot>
            </>
          }
          footer={
            <LogoGrid>
              <Logo>
                <span className="text-sm font-medium text-mauve-600 dark:text-mauve-400">Rust</span>
              </Logo>
              <Logo>
                <span className="text-sm font-medium text-mauve-600 dark:text-mauve-400">Convex</span>
              </Logo>
              <Logo>
                <span className="text-sm font-medium text-mauve-600 dark:text-mauve-400">TanStack</span>
              </Logo>
            </LogoGrid>
          }
        />

        <FeaturesStackedAlternatingWithDemos
          id="features"
          headline="Runtime, compiler, and verification."
          subheadline={
            <p>
              Typed contracts, replayable receipts, and policy-driven behavior. No stubs; everything is logged and
              auditable.
            </p>
          }
          features={
            <>
              <Feature
                headline="Runtime"
                subheadline={
                  <p>Execute tool and job actions with schema validation, retries, and deterministic receipts.</p>
                }
                cta={
                  <UiLink href="https://github.com/OpenAgentsInc/openagents">
                    Learn more <ChevronIcon />
                  </UiLink>
                }
                demo={
                  <Screenshot wallpaper="blue" placement="bottom-right">
                    <img
                      src="https://assets.tailwindplus.com/screenshots/1.webp?left=1500&top=1240"
                      alt=""
                      className="bg-white/75 max-lg:hidden dark:hidden"
                      width={1500}
                      height={1240}
                    />
                    <img
                      width={1500}
                      height={1240}
                      src="https://assets.tailwindplus.com/screenshots/1.webp?left=1500&top=1240&color=mauve"
                      alt=""
                      className="bg-black/75 not-dark:hidden max-lg:hidden"
                    />
                  </Screenshot>
                }
              />
              <Feature
                headline="Compiler (dsrs)"
                subheadline={
                  <p>Express behavior as Signatures and Modules; optimize via metrics into policy bundles.</p>
                }
                cta={
                  <UiLink href="https://github.com/OpenAgentsInc/openagents">
                    Learn more <ChevronIcon />
                  </UiLink>
                }
                demo={
                  <Screenshot wallpaper="purple" placement="top-left">
                    <img
                      src="https://assets.tailwindplus.com/screenshots/1.webp?right=1500&bottom=1240"
                      alt=""
                      className="bg-white/75 max-lg:hidden dark:hidden"
                      width={1500}
                      height={1240}
                    />
                    <img
                      src="https://assets.tailwindplus.com/screenshots/1.webp?right=1500&bottom=1240&color=mauve"
                      width={1500}
                      height={1240}
                      alt=""
                      className="bg-black/75 not-dark:hidden max-lg:hidden"
                    />
                  </Screenshot>
                }
              />
              <Feature
                headline="Verified Patch Bundle"
                subheadline={
                  <p>Canonical output: PR_SUMMARY.md, RECEIPT.json, REPLAY.jsonl. Tests and builds anchor correctness.</p>
                }
                cta={
                  <UiLink href="https://github.com/OpenAgentsInc/openagents">
                    Learn more <ChevronIcon />
                  </UiLink>
                }
                demo={
                  <Screenshot wallpaper="brown" placement="bottom-left">
                    <img
                      src="https://assets.tailwindplus.com/screenshots/1.webp?right=1500&top=1240"
                      alt=""
                      className="bg-white/75 max-lg:hidden dark:hidden"
                      width={1500}
                      height={1240}
                    />
                    <img
                      src="https://assets.tailwindplus.com/screenshots/1.webp?right=1500&top=1240&color=mauve"
                      alt=""
                      width={1500}
                      height={1240}
                      className="bg-black/75 not-dark:hidden max-lg:hidden"
                    />
                  </Screenshot>
                }
              />
            </>
          }
        />

        <BrandsCardsMultiColumn
          id="brands"
          eyebrow="Built with"
          headline="Open stack."
          subheadline={
            <p>Rust, Convex, TanStack, WorkOS. See ROADMAP.md and SYNTHESIS_EXECUTION.md for what’s wired today.</p>
          }
        >
          <BrandCard
            logo={<span className="text-lg font-semibold text-mauve-950 dark:text-white">autopilot</span>}
            text="Product CLI: sessions, run, export, replay, policy."
            footnote="cargo run -p autopilot"
          />
          <BrandCard
            logo={<span className="text-lg font-semibold text-mauve-950 dark:text-white">pylon</span>}
            text="Network node: jobs, wallet, provider mode."
            footnote="cargo run -p pylon"
          />
          <BrandCard
            logo={<span className="text-lg font-semibold text-mauve-950 dark:text-white">dsrs</span>}
            text="Compiler layer: signatures, modules, metrics, optimizers."
            footnote="crates/dsrs"
          />
        </BrandsCardsMultiColumn>

        <TestimonialTwoColumnWithLargePhoto
          id="testimonial"
          quote={
            <p>
              OpenAgents gives us a single contract for agent behavior: typed tools, replayable runs, and policy we can
              optimize. Verification first — everything else is optimization.
            </p>
          }
          img={
            <img
              src="https://assets.tailwindplus.com/avatars/16.webp?w=1400&h=1000"
              alt=""
              className="not-dark:bg-white/75 dark:bg-black/75"
              width={1400}
              height={1000}
            />
          }
          name="OpenAgents"
          byline="Runtime + compiler for autonomous agents"
        />

        <StatsWithGraph
          id="stats"
          eyebrow="Built for scale"
          headline="From local runs to federated lanes."
          subheadline={
            <p>
              Run autopilot locally, deploy workers with pylon, and plug into the protocol surface. RLM/FRLM for
              out-of-core reasoning; market layer for compute and sandbox execution.
            </p>
          }
        >
          <Stat stat="Verified" text="Patch bundles with PR_SUMMARY, RECEIPT, REPLAY." />
          <Stat stat="Typed" text="Tools have schemas; runtime validates before execution." />
        </StatsWithGraph>

        <CallToActionSimple
          id="call-to-action"
          headline="Ready to run agents that ship?"
          subheadline={
            <p>
              Get started with the setup flow: Convex + TanStack Start + WorkOS AuthKit. Then explore the repo and
              ROADMAP.
            </p>
          }
          cta={
            <div className="flex items-center gap-4">
              <ButtonLink href="/setup" size="lg">
                Get started
              </ButtonLink>
              <PlainButtonLink href="/setup" size="lg">
                Log in <ChevronIcon />
              </PlainButtonLink>
            </div>
          }
        />
      </Main>

      <FooterWithLinksAndSocialIcons
        id="footer"
        links={
          <>
            <FooterLink href="/setup">Setup</FooterLink>
            <FooterLink href="https://github.com/OpenAgentsInc/openagents">GitHub</FooterLink>
            <FooterLink href="https://github.com/OpenAgentsInc/openagents/blob/main/ROADMAP.md">Roadmap</FooterLink>
            <FooterLink href="https://github.com/OpenAgentsInc/openagents/blob/main/AGENTS.md">AGENTS.md</FooterLink>
          </>
        }
        socialLinks={
          <>
            <SocialLink href="https://x.com/openagents" name="X">
              <XIcon />
            </SocialLink>
            <SocialLink href="https://github.com/OpenAgentsInc/openagents" name="GitHub">
              <GitHubIcon />
            </SocialLink>
            <SocialLink href="https://www.youtube.com" name="YouTube">
              <YouTubeIcon />
            </SocialLink>
          </>
        }
        fineprint="© 2025 OpenAgents"
      />
    </>
  );
}
