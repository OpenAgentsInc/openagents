/**
 * AI SDK public docs contract (owner-directed addition, 2026-07-21).
 *
 * `/aisdk` presents the extracted OpenAgents AI SDK
 * (https://github.com/OpenAgentsInc/ai, npm `@openagentsinc/ai@rc`) and
 * `/aisdk/docs` renders the Markdown kept in the repository `docs/ai-sdk/`
 * tree. The compiler (`scripts/generate-aisdk-docs.ts`) reads ONLY the exact
 * source files named below — never the repository-wide `docs/` directory —
 * so the public content graph stays a bounded explicit allowlist, mirroring
 * the `/docs` invariant model in `apps/openagents.com/INVARIANTS.md`.
 */

export type AisdkHeading = Readonly<{
  depth: 2 | 3
  id: string
  text: string
}>

export type AisdkDocsPageManifestEntry = Readonly<{
  description: string
  editUrl: string
  headings: ReadonlyArray<AisdkHeading>
  path: string
  sidebarLabel: string
  slug: string
  title: string
}>

export type AisdkDocsPage = AisdkDocsPageManifestEntry &
  Readonly<{
    html: string
  }>

export type AisdkDocsSourceDefinition = Readonly<{
  /** File name inside the repository `docs/ai-sdk/` tree. */
  file: string
  /** Route slug: '' renders at /aisdk/docs, others at /aisdk/docs/{slug}. */
  slug: string
  sidebarLabel: string
  description: string
}>

/**
 * The exact public content graph for /aisdk/docs. Order is navigation order.
 * Adding a file here is a public-surface decision, not a build detail.
 */
export const aisdkDocsSourceDefinitions: ReadonlyArray<AisdkDocsSourceDefinition> = [
  {
    file: 'README.md',
    slug: '',
    sidebarLabel: 'Overview',
    description:
      'The Effect-native OpenAgents AI SDK: where it lives, the current published train, and the monorepo consumption contract.',
  },
  {
    file: 'getting-started.md',
    slug: 'getting-started',
    sidebarLabel: 'Getting started',
    description:
      'Install the rc train and run three real programs: harness suspend/continue, UI chunk projection, and Tier D recall.',
  },
  {
    file: 'packages.md',
    slug: 'packages',
    sidebarLabel: 'Packages',
    description:
      'Every published SDK package with its key exports and when to use it.',
  },
]

export const aisdkDocsRoutePath = (slug: string): string =>
  slug === '' ? '/aisdk/docs' : `/aisdk/docs/${slug}`
