"use client";

import type { ReactNode } from "react";

import { siteConfig } from "@/components/launch-ui/config/site";
import { cn } from "@/lib/utils";

import Github from "../../logos/github";
import LaunchUI from "../../logos/launch-ui";
import {
  Navbar as NavbarComponent,
  NavbarLeft,
  NavbarRight,
} from "../../ui/navbar";

interface NavbarLink {
  text: string;
  href: string;
}

interface NavbarProps {
  logo?: ReactNode;
  name?: string;
  homeUrl?: string;
  links?: ReadonlyArray<NavbarLink>;
  className?: string;
}

const defaultLinks: ReadonlyArray<NavbarLink> = [
  { text: "Docs", href: siteConfig.getStartedUrl },
  { text: "Components", href: `${siteConfig.url}/docs/components` },
  { text: "Blocks", href: `${siteConfig.url}/blocks` },
  { text: "Illustrations", href: `${siteConfig.url}/illustrations` },
  { text: "Templates", href: `${siteConfig.url}/templates` },
  { text: "Pricing", href: `${siteConfig.url}/pricing` },
];

function NavLinks({ links }: { links: ReadonlyArray<NavbarLink> }) {
  return (
    <nav
      aria-label="Launch UI sections"
      className="hidden items-center gap-7 md:flex"
    >
      {links.map((link) => (
        <a
          key={`${link.href}-${link.text}`}
          href={link.href}
          className="text-sm font-medium text-muted-foreground transition-colors hover:text-foreground"
        >
          {link.text}
        </a>
      ))}
    </nav>
  );
}

function IconLink({
  children,
  href,
  label,
}: {
  children: ReactNode;
  href: string;
  label: string;
}) {
  return (
    <a
      aria-label={label}
      href={href}
      className="inline-flex size-9 items-center justify-center rounded-md text-muted-foreground transition-colors hover:bg-foreground/5 hover:text-foreground"
    >
      {children}
    </a>
  );
}

export default function Navbar({
  logo = <LaunchUI />,
  name = "Launch UI",
  homeUrl = siteConfig.url,
  links = defaultLinks,
  className,
}: NavbarProps) {
  return (
    <header className={cn("relative z-50 border-b border-border/10", className)}>
      <div className="border-b border-border/10 px-4 py-3 text-center text-sm font-medium text-muted-foreground">
        Start building top quality designs yourself
        <span className="px-2 text-muted-foreground/70">·</span>
        Check out my free course{" "}
        <a
          className="text-brand transition-colors hover:text-brand-foreground"
          href="https://designwithcode.dev"
        >
          designwithcode.dev →
        </a>
      </div>
      <div className="px-4">
        <div className="max-w-container relative mx-auto">
          <NavbarComponent className="py-5">
            <NavbarLeft className="gap-7">
              <a
                href={homeUrl}
                className="flex items-center gap-3 text-xl font-semibold text-foreground"
              >
                {logo}
                <span>{name}</span>
                <span className="rounded-full border border-border/20 px-2.5 py-1 text-xs font-semibold text-muted-foreground">
                  {siteConfig.version}
                </span>
              </a>
              <NavLinks links={links} />
            </NavbarLeft>
            <NavbarRight className="gap-2">
              <div className="hidden items-center gap-2 md:flex">
                <IconLink href={siteConfig.links.github} label="GitHub">
                  <Github className="size-4" />
                </IconLink>
                <IconLink href={siteConfig.links.twitter} label="X">
                  <span className="text-lg leading-none">×</span>
                </IconLink>
              </div>
              <details className="group relative md:hidden">
                <summary className="inline-flex size-9 cursor-pointer list-none items-center justify-center rounded-md text-muted-foreground transition-colors hover:bg-foreground/5 hover:text-foreground [&::-webkit-details-marker]:hidden">
                  <span aria-hidden="true" className="text-lg leading-none">☰</span>
                  <span className="sr-only">Toggle navigation menu</span>
                </summary>
                <nav className="absolute top-12 right-0 z-50 grid w-64 gap-4 rounded-md border border-border/15 bg-background p-5 text-base font-medium shadow-2xl">
                  <a href={homeUrl} className="flex items-center gap-3 text-xl font-semibold">
                    {logo}
                    <span>{name}</span>
                  </a>
                  {links.map((link) => (
                    <a
                      key={`${link.href}-${link.text}`}
                      href={link.href}
                      className="text-muted-foreground hover:text-foreground"
                    >
                      {link.text}
                    </a>
                  ))}
                </nav>
              </details>
            </NavbarRight>
          </NavbarComponent>
        </div>
      </div>
    </header>
  );
}
