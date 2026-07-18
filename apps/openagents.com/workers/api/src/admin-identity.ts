export const OPENAGENTS_ADMIN_EMAILS = ['chris@openagents.com'] as const

export const isOpenAgentsAdminEmail = (email: string): boolean =>
  OPENAGENTS_ADMIN_EMAILS.some(
    adminEmail => adminEmail === email.trim().toLowerCase(),
  )
