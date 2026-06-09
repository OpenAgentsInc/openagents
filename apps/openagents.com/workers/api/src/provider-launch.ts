import type {
  ProviderAccountBundle,
  PublicProviderAccount,
} from './provider-accounts'

export const connectedProviderAccountRef = (
  bundle: ProviderAccountBundle,
  requested: string | undefined,
): string | undefined => {
  if (requested !== undefined) {
    const matching = bundle.accounts.find(
      account =>
        account.providerAccountRef === requested || account.id === requested,
    )

    return matching?.publicStatus === 'connected' &&
      matching.status === 'connected' &&
      matching.health === 'healthy' &&
      matching.hasSecretRef
      ? matching.providerAccountRef
      : undefined
  }

  return bundle.accounts.find(
    account =>
      account.publicStatus === 'connected' &&
      account.status === 'connected' &&
      account.health === 'healthy' &&
      account.hasSecretRef,
  )?.providerAccountRef
}

export const providerAccountLaunchCandidate = (
  bundle: ProviderAccountBundle,
  requested: string | undefined,
): PublicProviderAccount | undefined => {
  if (requested !== undefined) {
    return bundle.accounts.find(
      account =>
        account.providerAccountRef === requested || account.id === requested,
    )
  }

  return (
    bundle.accounts.find(
      account => account.health !== 'healthy' || account.status !== 'connected',
    ) ?? bundle.accounts[0]
  )
}

const providerAccountLabel = (account: PublicProviderAccount): string =>
  account.accountLabel ?? 'ChatGPT account'

export const providerAccountReconnectReason = (
  account: PublicProviderAccount,
  latestHealthSummary: string | undefined,
): string => {
  if (latestHealthSummary?.includes('token_invalidated') === true) {
    return 'ChatGPT/Codex account token was invalidated by OpenAI.'
  }

  if (account.health === 'requires_reauth') {
    return 'The saved ChatGPT login needs to be refreshed.'
  }

  if (!account.hasSecretRef) {
    return 'No ChatGPT login is available.'
  }

  if (account.status !== 'connected' || account.publicStatus !== 'connected') {
    return 'The ChatGPT account is not connected.'
  }

  return 'The ChatGPT account is not ready.'
}

export const latestProviderAccountHealthSummary = async (
  db: D1Database,
  userId: string,
  account: PublicProviderAccount,
): Promise<string | undefined> => {
  const row = await db
    .prepare(
      `SELECT summary
       FROM provider_account_events
       WHERE user_id = ?
         AND kind IN (
           'account_health_updated',
           'login_denied',
           'login_expired',
           'login_failed',
           'account_disconnected',
           'auth_grant_failed'
         )
         AND (
           provider_account_id = ?
           OR target_ref = ?
         )
       ORDER BY created_at DESC
       LIMIT 1`,
    )
    .bind(userId, account.id, account.providerAccountRef)
    .first<Readonly<{ summary: string }>>()

  return row?.summary
}

export const providerAccountLaunchBlockMessage = async (
  db: D1Database,
  userId: string,
  bundle: ProviderAccountBundle,
  requested: string | undefined,
): Promise<string> => {
  const account = providerAccountLaunchCandidate(bundle, requested)

  if (account === undefined) {
    return 'Connect ChatGPT in Settings -> Connections before launching Autopilot.'
  }
  const latestHealthSummary = await latestProviderAccountHealthSummary(
    db,
    userId,
    account,
  )
  const reason = providerAccountReconnectReason(account, latestHealthSummary)

  return `${providerAccountLabel(account)} cannot launch Autopilot. ${reason} Reconnect ChatGPT in Settings -> Connections.`
}
