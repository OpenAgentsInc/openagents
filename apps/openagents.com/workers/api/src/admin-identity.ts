// SARAH-ACT-1 (#9065): the admin allowlist used to be one hardcoded literal.
// isOpenAgentsAdminEmail is called from 30+ sites across index.ts for
// general platform admin authorization (not only Sarah), so its signature
// stays unchanged. Admission is widened only by an explicit owner-set env
// value, never inferred from conversation context, and the default below is
// unchanged unless that env value is present — existing deployments keep
// their exact current behavior with no action required.
const DEFAULT_ADMIN_EMAILS = ['chris@openagents.com'] as const

let configuredAdminEmails: ReadonlyArray<string> = DEFAULT_ADMIN_EMAILS

const normalizeEmail = (email: string): string => email.trim().toLowerCase()

const parseAdminEmailsEnvValue = (
  rawValue: string | undefined,
): ReadonlyArray<string> => {
  if (rawValue === undefined || rawValue.trim().length === 0) {
    return DEFAULT_ADMIN_EMAILS
  }

  const parsed = Array.from(
    new Set(
      rawValue
        .split(',')
        .map(normalizeEmail)
        .filter(email => email.length > 0),
    ),
  )

  return parsed.length > 0 ? parsed : DEFAULT_ADMIN_EMAILS
}

// Called once per request (a split + a Set, cheap) so a Worker isolate
// reused across requests never serves a stale allowlist after a config
// change, and so no other call site needs to thread env through.
export const configureOpenAgentsAdminEmailsFromEnv = (
  rawValue: string | undefined,
): void => {
  configuredAdminEmails = parseAdminEmailsEnvValue(rawValue)
}

// Test-only: restore the compiled default without needing a fresh module
// instance between test cases.
export const resetOpenAgentsAdminEmailsForTest = (): void => {
  configuredAdminEmails = DEFAULT_ADMIN_EMAILS
}

export const getOpenAgentsAdminEmails = (): ReadonlyArray<string> =>
  configuredAdminEmails

// parseAdminEmailsEnvValue always falls back to DEFAULT_ADMIN_EMAILS when
// empty, so configuredAdminEmails is provably non-empty; this helper avoids
// scattering `?? fallback` at call sites that need one definite admin email.
export const getPrimaryOpenAgentsAdminEmail = (): string =>
  configuredAdminEmails[0] ?? DEFAULT_ADMIN_EMAILS[0]

export const isOpenAgentsAdminEmail = (email: string): boolean =>
  configuredAdminEmails.some(adminEmail => adminEmail === normalizeEmail(email))
