export const GITHUB_REPOSITORY_URL =
  'https://github.com/OpenAgentsInc/openagents'

export const X_URL = 'https://x.com/OpenAgents'

export const DISCORD_URL = 'https://openagents.com/discord'

export const STACKER_NEWS_URL = 'https://stacker.news/~openagents'

export const DOCS_URL = '/docs'

export const DOWNLOAD_URL = '/download'

// DIST-11 (#8924): there is no hard-coded release constant. Version, size,
// format, and artifact links are resolver-derived on /download from the
// currently promoted SIGNED release set (see
// `../desktop-download-resolver.server.ts`); every public CTA routes to
// DOWNLOAD_URL, never to a mutable artifact.

export const PRODUCT_BOUNDARIES = [
  'Uses your ordinary logged-in Codex session',
  'Works locally without an OpenAgents account',
  'Keeps repository review read-only',
] as const
