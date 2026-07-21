import type { AisdkDocsPage } from './aisdk-content'

/** Head metadata for the /aisdk/docs reader (mirrors docs/docs-head.ts). */
export const aisdkDocsHead = (page: AisdkDocsPage | undefined) => {
  const title = page === undefined
    ? 'AI SDK docs - OpenAgents'
    : `${page.title} - OpenAgents AI SDK`
  const description = page?.description ??
    'Documentation for the Effect-native OpenAgents AI SDK.'
  const canonical = `https://openagents.com${page?.path ?? '/aisdk/docs'}`

  return {
    meta: [
      { title },
      { name: 'description', content: description },
      { property: 'og:title', content: title },
      { property: 'og:description', content: description },
      { property: 'og:url', content: canonical },
    ],
    links: [{ rel: 'canonical', href: canonical }],
  }
}
