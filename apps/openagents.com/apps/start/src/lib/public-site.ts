export const GITHUB_REPOSITORY_URL =
  'https://github.com/OpenAgentsInc/openagents'

export const X_URL = 'https://x.com/OpenAgents'

export const DISCORD_URL = 'https://openagents.com/discord'

export const STACKER_NEWS_URL = 'https://stacker.news/~openagents'

export const DOCS_URL = '/docs'

export const DOWNLOAD_URL = '/download'

export const MAC_RELEASE = {
  version: '0.1.0-rc.17',
  platform: 'macOS',
  architecture: 'Apple silicon',
  size: '290 MB',
  downloadUrl:
    'https://github.com/OpenAgentsInc/openagents/releases/download/openagents-desktop-v0.1.0-rc.17/OpenAgents-0.1.0-rc.17-arm64.dmg',
  releaseUrl: `${GITHUB_REPOSITORY_URL}/releases/tag/openagents-desktop-v0.1.0-rc.17`,
} as const

export const PRODUCT_BOUNDARIES = [
  'Uses your ordinary logged-in Codex session',
  'Works locally without an OpenAgents account',
  'Keeps repository review read-only',
] as const
