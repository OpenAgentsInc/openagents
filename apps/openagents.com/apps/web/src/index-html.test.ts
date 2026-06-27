/// <reference types="vite/client" />
import { readFileSync } from 'node:fs'

import { describe, expect, test } from 'vitest'

const indexHtml = import.meta.glob<string>('../index.html', {
  eager: true,
  import: 'default',
  query: '?raw',
})

const publicAssets = import.meta.glob<string>('../public/*.svg', {
  eager: true,
  import: 'default',
  query: '?raw',
})

const html = indexHtml['../index.html'] ?? ''
const title = 'OpenAgents'
const description =
  'OpenAgents builds public, verifiable AI agents for coding, research, payments, and operational work.'
const canonicalUrl = 'https://openagents.com/'

const getJsonLd = () => {
  const match = html.match(
    /<script type="application\/ld\+json">\s*([\s\S]*?)\s*<\/script>/,
  )

  return JSON.parse(match?.[1] ?? '{}') as Record<string, unknown>
}

describe('index html', () => {
  test('loads the Fathom analytics script on the app shell', () => {
    expect(html).toContain('src="https://cdn.usefathom.com/script.js"')
    expect(html).toContain('data-site="IVAXCCIT"')
    expect(html).toContain('defer')
  })

  test('describes the homepage for search result snippets', () => {
    expect(html).toContain(`<title>${title}</title>`)
    expect(html).toContain(`name="description"`)
    expect(html).toContain(`content="${description}"`)
    expect(html).toContain(`<link rel="canonical" href="${canonicalUrl}" />`)
  })

  test('publishes social card metadata for the homepage', () => {
    expect(html).toContain(`property="og:site_name" content="OpenAgents"`)
    expect(html).toContain(`property="og:title"`)
    expect(html).toContain(`content="${title}"`)
    expect(html).toContain(`property="og:description"`)
    expect(html).toContain(`content="${description}"`)
    expect(html).toContain(`property="og:url" content="${canonicalUrl}"`)
    expect(html).toContain(`name="twitter:card" content="summary"`)
    expect(html).toContain(`name="twitter:title"`)
    expect(html).toContain(`name="twitter:description"`)
    expect(html).toContain(`name="twitter:url" content="${canonicalUrl}"`)
  })

  test('publishes WebSite JSON-LD site-name metadata', () => {
    expect(getJsonLd()).toEqual({
      '@context': 'https://schema.org',
      '@type': 'WebSite',
      name: 'OpenAgents',
      alternateName: 'OpenAgents.com',
      url: canonicalUrl,
      description,
    })
  })

  test('links stable crawlable favicon assets', () => {
    expect(html).toContain(
      '<link rel="icon" href="/favicon.ico" sizes="32x32" />',
    )
    expect(html).toContain(
      '<link rel="icon" type="image/svg+xml" sizes="any" href="/favicon.svg" />',
    )
    expect(html).toContain(
      '<link rel="icon" type="image/svg+xml" sizes="any" href="/icon.svg" />',
    )
    expect(publicAssets['../public/favicon.svg']).toContain('<svg')
    expect(publicAssets['../public/icon.svg']).toContain('<svg')
    expect(publicAssets['../public/favicon.svg']).not.toContain('<html')
    expect(publicAssets['../public/icon.svg']).not.toContain('<html')

    const faviconIco = readFileSync('public/favicon.ico')
    expect([...faviconIco.subarray(0, 4)]).toEqual([0, 0, 1, 0])
  })
})
