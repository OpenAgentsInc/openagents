import { readFileSync } from 'node:fs'
import path from 'node:path'
import { renderToStaticMarkup } from 'react-dom/server'
import { describe, expect, test } from 'vitest'

import { SplashPage, splashPageDescription } from './-splash-page'

describe('Desktop splash', () => {
  test('server-renders the landing hero around the live workroom and its accessible controls', () => {
    const html = renderToStaticMarkup(<SplashPage />)

    expect(html).toContain('data-route="splash"')
    expect(html).toContain('Primary navigation')
    expect(html).toContain('Your last agent IDE.')
    expect(html).toContain(
      'Omega brings your project, agents, code, review, and evidence',
    )
    expect(html).toContain('View on GitHub')
    expect(html).toContain('href="https://github.com/OpenAgentsInc/omega"')
    expect(html).not.toContain('Explore OpenAgents')
    expect(html).not.toContain('href="/download"')
    expect(html).toContain('href="/login"')
    expect(html).toContain('>Log In</a>')
    expect(html).not.toContain('href="/install"')
    expect(html).toContain('class="splash-hero-canvas"')
    expect(html).toContain('data-khala-canvas="server-static"')
    expect(html).toContain('aria-hidden="true"')
    expect(html).toContain('splash-development-status')
    expect(html).toContain('In development')
    expect(html).not.toContain('Introducing Omega')
    expect(html).not.toContain('href="/blog')
    expect(html).toContain('OpenAgents on GitHub')
    expect(html).not.toContain('Open app')
    expect(html).not.toContain('href="#product"')
    expect(html).toContain('splash-product')
    expect(html).toContain('splash-window-bar')
    expect(html).toContain('Omega interactive product direction')
    expect(html).toContain('splash-demo-frame')
    expect(html).toContain('data-interactive="false"')
    expect(html).toContain('Activate the Omega product direction demo')
    expect(html).toContain('Click to interact')
    expect(html).toContain('>IN DEV<')
    expect(html).toContain('data-sidebar-destination-id="workspace-new-chat"')
    expect(html).toContain(
      'data-sidebar-destination-id="shell-settings-toggle"',
    )
    expect(html).toContain('aria-label="Settings"')
    expect(html).not.toContain('>Chat<')
    expect(html).not.toContain('>Project home<')
    expect(html).not.toContain('>Workspaces<')
    expect(html).toContain('IDENTITY ONBOARDING')
    expect(html).toContain('PROJECT REVIEW')
    expect(html).toContain('Prepare identity-first onboarding for Omega')
    expect(html).toContain('data-en-react-surface="true"')
    expect(html).toContain('data-chat-composer="true"')
    expect(html).toContain('data-composer-button-kind="action"')
    expect(html).toContain('data-composer-button-kind="toggle"')
    expect(html).toContain('data-composer-button-kind="submit"')
    expect(html).toContain('data-composer-button-kind="stop"')
    expect(html).toContain('crates/onboarding')
    expect(html).toContain('Steer the current work')
    expect(html).toContain('spawnAgent · implementation swarm')
    expect(html).toContain('a11y-oracle')
    expect(html).toContain('collabAgentToolCall')
    expect(html).toContain('data-kind="commandExecution"')
    expect(html).toContain('data-kind="fileChange"')
    expect(html).toContain('data-kind="mcpToolCall"')
    expect(html).toContain('data-kind="webSearch"')
    expect(html).toContain('data-kind="imageView"')
    expect(html).toContain('data-kind="contextCompaction"')
    expect(html).toContain('Command approval')
    expect(html).toContain('Queued follow-up (#1)')
    expect(html).toContain('What Omega is—and where it’s going.')
    expect(html).toContain('What is Omega?')
    expect(html).toContain('Is Omega available yet?')
    expect(html).toContain('Not yet. Omega is in active development')
    expect(html).toContain('aria-label="Product links"')
    expect(html).not.toContain('>Blog</a>')
    expect(html).toContain('aria-label="Community links"')
    expect(html).toContain('aria-label="Legal links"')
    expect(html).toContain('href="https://x.com/OpenAgents"')
    expect(html).toContain('>X (Twitter)</a>')
    expect(html.indexOf('>Terms</a>')).toBeLessThan(
      html.indexOf('>Privacy</a>'),
    )
    expect(html).toContain('href="https://openagents.com/discord"')
    expect(html).toContain('href="https://stacker.news/~openagents"')
    expect(html).not.toContain('>Build from source</a>')
    expect(html).toContain('© 2026 OpenAgents, Inc.')
    expect(html).toContain('Open source · local first · evidence backed')
    expect(html).toContain('Building Omega for durable, verifiable agent work.')
    expect(html).not.toContain('durable, reviewable Codex work')
    expect(html).not.toContain('<img')
  })

  test('loads the shared workbench CSS and gates internal scrolling behind activation', () => {
    const css = readFileSync(
      path.resolve(import.meta.dirname, '../splash.css'),
      'utf8',
    )
    const developmentStatusRule = css.match(
      /\.splash-development-status \{([\s\S]*?)\n\}/,
    )?.[1]
    const heroHeadingRule = css.match(/\.splash-hero h1 \{([\s\S]*?)\n\}/)?.[1]

    expect(css).toContain("@import '@openagentsinc/ui/desktop-workbench.css'")
    expect(css).toContain('.splash-demo-frame .oa-react-timeline-scroll')
    expect(css).toContain('overscroll-behavior-y: auto')
    expect(css).toContain('touch-action: pan-y')
    expect(css).toContain('.splash-demo-activation')
    expect(css).toContain('position: absolute')
    expect(css).toContain('z-index: 5')
    expect(developmentStatusRule).toContain('font-family: var(--font-mono)')
    expect(heroHeadingRule).toContain(
      'font-size: clamp(3.15rem, 6.1vw, 5.25rem)',
    )
  })

  test('owns the root homepage and remains available at the preview route', () => {
    const rootRoute = readFileSync(
      path.resolve(import.meta.dirname, 'index.tsx'),
      'utf8',
    )
    const previewRoute = readFileSync(
      path.resolve(import.meta.dirname, 'splash.tsx'),
      'utf8',
    )

    expect(rootRoute).toContain('component: SplashPage')
    expect(rootRoute).toContain("href: 'https://openagents.com/'")
    expect(rootRoute).not.toContain('HoldingPage')
    expect(previewRoute).toContain("createFileRoute('/splash')")
    expect(previewRoute).toContain('component: SplashPage')
    expect(previewRoute).toContain("href: 'https://openagents.com/splash'")
    expect(splashPageDescription).toContain('Omega')
  })
})
