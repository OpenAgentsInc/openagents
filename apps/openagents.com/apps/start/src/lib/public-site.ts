export const GITHUB_REPOSITORY_URL =
  'https://github.com/OpenAgentsInc/openagents'

export const DOCS_URL = '/docs'

export const INSTALL_URL = '/install'

export const MAC_RELEASE = {
  version: '0.1.0-rc.12',
  platform: 'macOS',
  architecture: 'Apple silicon',
  size: '290 MB',
  downloadUrl:
    'https://github.com/OpenAgentsInc/openagents/releases/download/openagents-desktop-v0.1.0-rc.12/OpenAgents-0.1.0-rc.12-arm64.dmg',
  releaseUrl: `${GITHUB_REPOSITORY_URL}/releases/tag/openagents-desktop-v0.1.0-rc.12`,
} as const

export const PRODUCT_BOUNDARIES = [
  'Uses your ordinary logged-in Codex session',
  'Works locally without an OpenAgents account',
  'Keeps repository review read-only',
] as const
