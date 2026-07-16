import type { DocsNavigationGroupDefinition } from './content-schema'

export const docsNavigationDefinition: ReadonlyArray<DocsNavigationGroupDefinition> = [
  {
    collapsed: false,
    label: 'Docs',
    slugs: [
      '',
      'getting-started',
      'workroom',
      'full-auto',
      'review-and-recovery',
      'security-and-privacy',
      'troubleshooting',
      'agent-readable',
    ],
  },
]

export const docsCompatibilityRedirects: Readonly<Record<string, string>> = {
  api: '/docs/agent-readable',
  'connect-codex-fleet': '/docs/getting-started',
  openagents: '/',
  'product-promises': '/docs/agent-readable',
}
