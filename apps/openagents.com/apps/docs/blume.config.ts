import { defineConfig } from 'blume'

export default defineConfig({
  title: 'OpenAgents Docs',
  description:
    'Use, understand, and verify the local-first OpenAgents Desktop Codex workroom.',
  content: {
    root: 'content',
  },
  deployment: {
    base: '/docs',
    output: 'static',
    site: 'https://openagents.com',
  },
  search: {
    provider: 'orama',
  },
  redirects: [
    { from: '/openagents', to: '/', status: 301 },
    { from: '/api', to: '/agent-readable', status: 301 },
    { from: '/connect-codex-fleet', to: '/getting-started', status: 301 },
  ],
  theme: {
    accent: '#2979ff',
    action: '#2979ff',
    background: '#000000',
    mode: 'dark',
    radius: 'sm',
  },
  markdown: {
    code: {
      icons: false,
      wrap: false,
    },
    codeBlocks: {
      theme: {
        light: 'vesper',
        dark: 'vesper',
      },
    },
  },
  ai: {
    ask: { enabled: false },
    mcp: { enabled: false },
    llmsTxt: true,
  },
  feedback: false,
  export: false,
  lastModified: { type: 'frontmatter' },
  seo: {
    agentReadability: true,
    contentSignals: {
      search: true,
      aiInput: true,
      aiTrain: false,
    },
    og: { enabled: false },
    robots: false,
    sitemap: true,
    structuredData: true,
  },
  github: {
    owner: 'OpenAgentsInc',
    repo: 'openagents',
    branch: 'main',
    dir: 'apps/openagents.com/apps/docs',
  },
  logo: {
    text: 'OpenAgents Docs',
    href: '/docs',
  },
  toc: {
    minHeadingLevel: 2,
    maxHeadingLevel: 3,
  },
})
