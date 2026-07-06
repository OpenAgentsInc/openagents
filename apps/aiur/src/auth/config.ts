/**
 * Aiur Worker environment bindings (AIUR-1, #8499). Kept as a small plain
 * type (not Effect Config) since this Worker's `fetch` handler needs to
 * read `env` before any Effect runtime is available.
 */
export type AiurEnv = Readonly<{
  OPENAUTH_CLIENT_ID?: string
  OPENAUTH_ISSUER_URL?: string
  KHALA_SYNC_UPSTREAM_BASE_URL?: string
  AIUR_OWNER_USER_IDS?: string
  ASSETS?: Fetcher
}>

export const DEFAULT_OPENAUTH_ISSUER_URL = 'https://auth.openagents.com'
export const DEFAULT_OPENAUTH_CLIENT_ID = 'openagents-web'
export const DEFAULT_KHALA_SYNC_UPSTREAM_BASE_URL = 'https://openagents.com'

const trimmedOrUndefined = (value: string | undefined): string | undefined => {
  const trimmed = value?.trim()

  return trimmed === undefined || trimmed === '' ? undefined : trimmed
}

export const openAuthIssuerUrl = (env: AiurEnv): string =>
  trimmedOrUndefined(env.OPENAUTH_ISSUER_URL) ?? DEFAULT_OPENAUTH_ISSUER_URL

export const openAuthClientId = (env: AiurEnv): string =>
  trimmedOrUndefined(env.OPENAUTH_CLIENT_ID) ?? DEFAULT_OPENAUTH_CLIENT_ID

export const khalaSyncUpstreamBaseUrl = (env: AiurEnv): string =>
  trimmedOrUndefined(env.KHALA_SYNC_UPSTREAM_BASE_URL) ??
  DEFAULT_KHALA_SYNC_UPSTREAM_BASE_URL
