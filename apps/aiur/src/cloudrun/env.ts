import type { AiurEnv } from '../auth/config'

/**
 * Builds the Aiur env from process env vars on Cloud Run (CFG-11, #8526).
 * Only the known `AiurEnv` fields are picked up — never the whole process
 * environment. `AIUR_OWNER_USER_IDS` arrives via GCP Secret Manager
 * (`--set-secrets`); a missing/empty value stays `undefined` so the owner
 * gate fails closed exactly as on Workers.
 */
export const aiurEnvFromProcessEnv = (
  processEnv: Readonly<Record<string, string | undefined>>,
): AiurEnv => {
  const pick = (name: string): Record<string, string> => {
    const value = processEnv[name]?.trim()
    return value === undefined || value === '' ? {} : { [name]: value }
  }

  return {
    ...pick('OPENAUTH_CLIENT_ID'),
    ...pick('OPENAUTH_ISSUER_URL'),
    ...pick('KHALA_SYNC_UPSTREAM_BASE_URL'),
    ...pick('AIUR_OWNER_USER_IDS'),
  }
}
