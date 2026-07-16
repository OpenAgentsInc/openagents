export const GITHUB_REPOSITORY_URL =
  'https://github.com/OpenAgentsInc/openagents'

export const X_URL = 'https://x.com/OpenAgents'

export const DISCORD_URL = 'https://openagents.com/discord'

export const STACKER_NEWS_URL = 'https://stacker.news/~openagents'

export const DOCS_URL = '/docs'

export const DOWNLOAD_URL = '/download'

// DIST-10 (#8923): the download CTA never links a handwritten artifact URL.
// `downloadUrl` goes through the server-side resolver redirect, which 302s to
// the artifact URL bound to the currently promoted SIGNED release feed and
// returns typed unavailability instead of a dead link. (The previously pinned
// rc.17 GitHub URL was a live 404 — no such tag/object ever existed; the
// promoted feed serves 0.1.0-rc.13.) The static version/size labels below are
// interim copy; #8924 replaces them with resolver-derived truth.
export const MAC_RELEASE = {
  version: '0.1.0-rc.13',
  platform: 'macOS',
  architecture: 'Apple silicon',
  size: '304 MB',
  downloadUrl: '/api/public/desktop-download/artifact?target=darwin-arm64&format=dmg',
  releaseUrl: `${GITHUB_REPOSITORY_URL}/releases`,
} as const

export const PRODUCT_BOUNDARIES = [
  'Uses your ordinary logged-in Codex session',
  'Works locally without an OpenAgents account',
  'Keeps repository review read-only',
] as const
