import { existsSync, readFileSync } from 'node:fs'
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
const generatedManifestPath = path.resolve(
  import.meta.dirname,
  'generated/docs-manifest.generated.ts',
)

describe('unified TanStack Start docs content', () => {
  test('publishes the complete curated navigation graph', async () => {
    const navigationSlugs = docsNavigationDefinition.flatMap(group => group.slugs)

    expect(docsManifest).toHaveLength(8)
    expect(docsManifest.map(page => page.slug)).toEqual(navigationSlugs)
    await expect(Promise.all(navigationSlugs.map(loadDocsPage))).resolves.not.toContain(undefined)
  })

  test('bundles reader pages without deploy-fragile dynamic imports', () => {
    const generatedManifest = readFileSync(generatedManifestPath, 'utf8')

    expect(generatedManifest).toContain("import docsPage0 from './pages/index.generated'")
    expect(generatedManifest).toContain('const docsPages: Readonly<Record<string, DocsPage>>')
    expect(generatedManifest).not.toContain('() => import(')
    expect(generatedManifest).not.toContain('docsPageLoaders')
  })

  test('publishes the bounded Omega Full Auto status', async () => {
    const page = await loadDocsPage('full-auto')

    expect(page?.title).toBe('Full Auto')
    expect(page?.html).toContain('Full Auto is in active development')
    expect(page?.html).toContain('does not prove general release availability')
    expect(page?.html).toContain('must not create a hidden workroom')
  })

  test('publishes Omega without the legacy Desktop product frame', async () => {
    const pages = await Promise.all(
      docsNavigationDefinition.flatMap(group => group.slugs).map(loadDocsPage),
    )
    const published = JSON.stringify(pages)

    expect(pages[0]?.title).toBe('Omega')
    expect(published).toContain('Omega is in active development')
    expect(published).not.toContain('OpenAgents Desktop')
    expect(published).not.toContain('Electron runs')
    expect(published).not.toContain('apps/openagents-desktop')
  })

  test('preserves the corrected legacy redirects', () => {
    expect(docsCompatibilityRedirects).toEqual({
      api: '/docs/agent-readable',
      'connect-codex-fleet': '/docs/getting-started',
      desktop: '/docs',
      openagents: '/',
      'openagents-desktop': '/docs',
      'product-promises': '/docs/agent-readable',
    })
  })

  test.each([
    ['index.md', 'title: Omega'],
    ['search.json', 'Omega is in active development'],
    ['llms.txt', 'The native OpenAgents workspace'],
    ['llms-full.txt', '# Omega'],
    ['agent-readability.json', 'openagents-tanstack-start'],
    ['sitemap.xml', 'https://openagents.com/docs/getting-started'],
  ])('generates the agent-readable artifact %s', (relativePath, marker) => {
    const artifact = readFileSync(path.join(publicDocsDirectory, relativePath), 'utf8')
    expect(artifact).toContain(marker)
  })

  test.each(['index.md', 'search.json', 'llms.txt', 'llms-full.txt'])(
    'removes the legacy Desktop product from %s',
    relativePath => {
      const artifact = readFileSync(path.join(publicDocsDirectory, relativePath), 'utf8')
      expect(artifact).not.toContain('OpenAgents Desktop')
    },
  )

  test('keeps archived documentation out of public artifacts', () => {
    expect(existsSync(path.join(publicDocsDirectory, 'future.md'))).toBe(false)
    expect(existsSync(path.join(publicDocsDirectory, 'future'))).toBe(false)
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

  test('uses Omega metadata for the parent docs route', () => {
    const head = docsHead(undefined)
    const serialized = JSON.stringify(head)

    expect(serialized).toContain('Understand Omega')
    expect(serialized).not.toContain('OpenAgents Desktop')
  })
})
