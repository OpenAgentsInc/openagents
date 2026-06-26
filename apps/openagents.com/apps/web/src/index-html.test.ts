/// <reference types="vite/client" />
import { describe, expect, test } from 'vitest'

const indexHtml = import.meta.glob<string>('../index.html', {
  eager: true,
  import: 'default',
  query: '?raw',
})

const faviconSvg = import.meta.glob<string>('../public/favicon.svg', {
  eager: true,
  import: 'default',
  query: '?raw',
})

const expectedTitle = 'OpenAgents - Paid AI agent work with public proof'
const expectedDescription =
  'OpenAgents is the agent network for paid AI work, public proof, product promises, and verifiable receipts.'

describe('index html', () => {
  test('declares homepage search result metadata', () => {
    const html = indexHtml['../index.html'] ?? ''

    expect(html).toContain(`<title>${expectedTitle}</title>`)
    expect(html).toContain(`name="description"`)
    expect(html).toContain(`content="${expectedDescription}"`)
    expect(html).toContain(
      '<link rel="canonical" href="https://openagents.com/" />',
    )
    expect(html).toContain(
      '<link rel="icon" type="image/svg+xml" href="/favicon.svg" />',
    )
    expect(html).toContain(`property="og:site_name" content="OpenAgents"`)
    expect(html).toContain(`property="og:title"`)
    expect(html).toContain(`property="og:description"`)
    expect(html).toContain(
      `property="og:url" content="https://openagents.com/"`,
    )
    expect(html).toContain(`name="twitter:card" content="summary"`)
    expect(html).toContain(`name="twitter:title"`)
    expect(html).toContain(`name="twitter:description"`)
  })

  test('declares WebSite structured data for the homepage', () => {
    const html = indexHtml['../index.html'] ?? ''
    const match = html.match(
      /<script type="application\/ld\+json">\s*([\s\S]*?)\s*<\/script>/,
    )

    expect(match).not.toBeNull()
    expect(JSON.parse(match?.[1] ?? '{}')).toEqual({
      '@context': 'https://schema.org',
      '@type': 'WebSite',
      name: 'OpenAgents',
      alternateName: 'OpenAgents.com',
      url: 'https://openagents.com/',
      description: expectedDescription,
    })
  })

  test('ships a crawlable favicon asset', () => {
    const svg = faviconSvg['../public/favicon.svg'] ?? ''

    expect(svg).toContain('<svg')
    expect(svg).toContain('viewBox="0 0 64 64"')
    expect(svg).toContain('aria-label="OpenAgents"')
  })

  test('loads the Fathom analytics script on the app shell', () => {
    const html = indexHtml['../index.html'] ?? ''

    expect(html).toContain('src="https://cdn.usefathom.com/script.js"')
    expect(html).toContain('data-site="IVAXCCIT"')
    expect(html).toContain('defer')
  })
})
