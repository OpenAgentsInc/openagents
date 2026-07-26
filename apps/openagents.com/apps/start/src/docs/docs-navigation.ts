import type { DocsNavigationGroupDefinition } from './content-schema'

export const docsNavigationDefinition: ReadonlyArray<DocsNavigationGroupDefinition> = [
  {
    collapsed: false,
    label: 'Omega',
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
  desktop: '/docs',
  openagents: '/',
  'openagents-desktop': '/docs',
  'product-promises': '/docs/agent-readable',
}
