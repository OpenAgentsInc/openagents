import { readFileSync } from 'node:fs'
import path from 'node:path'

import { describe, expect, test } from 'vitest'

import { docsHead } from './docs-head'
import {
  docsCompatibilityRedirects,
  docsNavigationDefinition,
} from './docs-navigation'
import {
  docsManifest,
  loadDocsPage,
} from './generated/docs-manifest.generated'

const publicDocsDirectory = path.resolve(import.meta.dirname, '../../public/docs')

describe('unified TanStack Start docs content', () => {
  test('publishes the complete curated navigation graph', async () => {
    const navigationSlugs = docsNavigationDefinition.flatMap(group => group.slugs)

    expect(docsManifest).toHaveLength(12)
    expect(docsManifest.map(page => page.slug)).toEqual(navigationSlugs)
    await expect(Promise.all(navigationSlugs.map(loadDocsPage))).resolves.not.toContain(undefined)
  })

  test('preserves the corrected legacy redirects', () => {
    expect(docsCompatibilityRedirects).toEqual({
      api: '/docs/agent-readable',
      'connect-codex-fleet': '/docs/getting-started',
      openagents: '/',
      'product-promises': '/docs/agent-readable',
    })
  })

  test.each([
    ['index.md', 'OpenAgents Desktop'],
    ['search.json', 'OpenAgents Desktop'],
    ['llms.txt', 'https://openagents.com/docs/getting-started'],
    ['llms-full.txt', '# OpenAgents Desktop'],
    ['agent-readability.json', 'openagents-tanstack-start'],
    ['sitemap.xml', 'https://openagents.com/docs/getting-started'],
  ])('generates the agent-readable artifact %s', (relativePath, marker) => {
    const artifact = readFileSync(path.join(publicDocsDirectory, relativePath), 'utf8')
    expect(artifact).toContain(marker)
  })

  test('generates canonical and structured metadata for every page', async () => {
    const page = await loadDocsPage('security-and-privacy')
    expect(page).toBeDefined()

    const head = docsHead(page)
    expect(head.links).toContainEqual({
      href: 'https://openagents.com/docs/security-and-privacy',
      rel: 'canonical',
    })
    expect(JSON.stringify(head.scripts)).toContain('TechArticle')
  })
})
