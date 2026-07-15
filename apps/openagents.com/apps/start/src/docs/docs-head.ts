import type { DocsPage } from './content-schema'

export const docsHead = (page: DocsPage | undefined) => {
  const title = page === undefined ? 'OpenAgents Docs' : `${page.title} — OpenAgents Docs`
  const description = page?.description ??
    'Use, understand, and verify the local-first OpenAgents Desktop Codex workroom.'
  const canonical = `https://openagents.com${page?.path ?? '/docs'}`
  const structuredData = page?.slug === ''
    ? {
        '@context': 'https://schema.org',
        '@type': 'WebSite',
        name: 'OpenAgents Docs',
        description,
        url: canonical,
      }
    : {
        '@context': 'https://schema.org',
        '@type': 'TechArticle',
        headline: page?.title ?? 'OpenAgents Docs',
        description,
        dateModified: page?.lastModified,
        mainEntityOfPage: canonical,
        publisher: {
          '@type': 'Organization',
          name: 'OpenAgents',
          url: 'https://openagents.com',
        },
      }

  return {
    meta: [
      { title },
      { name: 'description', content: description },
      { name: 'theme-color', content: '#05070d' },
      { property: 'og:title', content: title },
      { property: 'og:description', content: description },
      { property: 'og:url', content: canonical },
      { property: 'og:type', content: page?.slug === '' ? 'website' : 'article' },
      { name: 'twitter:title', content: title },
      { name: 'twitter:description', content: description },
    ],
    links: [{ rel: 'canonical', href: canonical }],
    scripts: [
      {
        type: 'application/ld+json',
        children: JSON.stringify(structuredData),
      },
    ],
  }
}
