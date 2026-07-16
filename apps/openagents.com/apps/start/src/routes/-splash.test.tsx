import { readFileSync } from 'node:fs'
import path from 'node:path'
import { renderToStaticMarkup } from 'react-dom/server'
import { describe, expect, test } from 'vitest'

import { SplashPage } from './-splash-page'

describe('Desktop splash', () => {
  test('server-renders the landing hero around the live workroom and its accessible controls', () => {
    const html = renderToStaticMarkup(<SplashPage />)

    expect(html).toContain('data-route="splash"')
    expect(html).toContain('Primary navigation')
    expect(html).toContain('Your last agent IDE.')
    expect(html).toContain('Download for Mac')
    expect(html).toContain('href="/download"')
    expect(html).not.toContain('href="/install"')
    expect(html).toContain('class="splash-hero-canvas"')
    expect(html).toContain('data-khala-canvas="server-static"')
    expect(html).toContain('aria-hidden="true"')
    expect(html).toContain('Or build from source')
    expect(html).toContain('Introducing OpenAgents Desktop')
    expect(html).toContain('href="/blog/introducing-openagents-desktop"')
    expect(html).toContain('OpenAgents on GitHub')
    expect(html).not.toContain('Open app')
    expect(html).not.toContain('href="#product"')
    expect(html).toContain('splash-product')
    expect(html).toContain('splash-window-bar')
    expect(html).toContain('OpenAgents Desktop live product preview')
    expect(html).toContain('splash-demo-frame')
    expect(html).toContain('data-interactive="false"')
    expect(html).toContain('Activate the OpenAgents Desktop demo')
    expect(html).toContain('Click to interact')
    expect(html).toContain('>ALPHA<')
    expect(html).not.toContain('>DEV<')
    expect(html).toContain('data-sidebar-destination-id="workspace-new-chat"')
    expect(html).toContain(
      'data-sidebar-destination-id="shell-settings-toggle"',
    )
    expect(html).toContain('aria-label="Settings"')
    expect(html).not.toContain('>Chat<')
    expect(html).not.toContain('>Project home<')
    expect(html).not.toContain('>Workspaces<')
    expect(html).toContain('APPSERVER')
    expect(html).toContain('T3CODE YOINK')
    expect(html).toContain('Show the whole Codex app-server workflow')
    expect(html).toContain('data-en-react-surface="true"')
    expect(html).toContain('data-chat-composer="true"')
    expect(html).toContain('data-composer-button-kind="action"')
    expect(html).toContain('data-composer-button-kind="toggle"')
    expect(html).toContain('data-composer-button-kind="submit"')
    expect(html).toContain('data-composer-button-kind="stop"')
    expect(html).toContain('packages/ui')
    expect(html).toContain('Steer a Codex message')
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
    expect(html).toContain('The important boundaries, plainly.')
    expect(html).toContain('Does OpenAgents replace Codex?')
    expect(html).toContain('What is available today?')
    expect(html).toContain('aria-label="Product links"')
    expect(html).toContain('aria-label="Community links"')
    expect(html).toContain('aria-label="Legal links"')
    expect(html).toContain('href="https://x.com/OpenAgents"')
    expect(html).toContain('href="https://openagents.com/discord"')
    expect(html).toContain('href="https://stacker.news/~openagents"')
    expect(html).not.toContain('>Build from source</a>')
    expect(html).toContain('© 2026 OpenAgents, Inc.')
    expect(html).toContain('Open source · local first · evidence backed')
    expect(html).not.toContain('<img')
  })

  test('loads the shared workbench CSS and gates internal scrolling behind activation', () => {
    const css = readFileSync(
      path.resolve(import.meta.dirname, '../splash.css'),
      'utf8',
    )
    const releaseLinkRule = css.match(
      /\.splash-release-link \{([\s\S]*?)\n\}/,
    )?.[1]
    const heroHeadingRule = css.match(/\.splash-hero h1 \{([\s\S]*?)\n\}/)?.[1]

    expect(css).toContain("@import '@openagentsinc/ui/desktop-workbench.css'")
    expect(css).toContain('.splash-demo-frame .oa-react-timeline-scroll')
    expect(css).toContain('overscroll-behavior-y: auto')
    expect(css).toContain('touch-action: pan-y')
    expect(css).toContain('.splash-demo-activation')
    expect(css).toContain('position: absolute')
    expect(css).toContain('z-index: 5')
    expect(releaseLinkRule).toContain('font-family: var(--font-sans)')
    expect(releaseLinkRule).not.toContain('font-family: var(--font-mono)')
    expect(heroHeadingRule).toContain(
      'font-size: clamp(3.15rem, 6.1vw, 5.25rem)',
    )
  })
})
