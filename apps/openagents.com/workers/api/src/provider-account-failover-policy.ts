import { isoTimestampAfterIso } from './runtime-primitives'

export type ProviderAccountFailoverFailureClass =
  | 'token_invalidated'
  | 'low_credits'
  | 'rate_limited'
  | 'quota_exhausted'
  | 'provider_outage'
  | 'launch_timeout'
  | 'grant_resolution_failed'
  | 'runner_failure'
  | 'unknown_provider_failure'

export type ProviderAccountHealthClassification =
  | 'healthy'
  | 'token_invalidated'
  | 'requires_reauth'
  | 'low_credits'
  | 'rate_limited'
  | 'quota_exhausted'
  | 'provider_outage'
  | 'launch_timeout'
  | 'grant_resolution_failed'
  | 'runner_failure'
  | 'wrong_account_or_collision'
  | 'unknown_failure'

export type ProviderAccountHealthEventInput = Readonly<{
  code?: string | undefined
  sanityClassification?: string | undefined
  collisionClass?: string | undefined
  runnerStatus?: string | undefined
  providerStatus?: number | undefined
}>

export type ProviderAccountHealthClassificationResult = Readonly<{
  classification: ProviderAccountHealthClassification
  accountStateAction:
    | ProviderAccountFailoverAction['accountStateAction']
    | 'none'
  health: ProviderAccountFailoverAction['health'] | 'healthy'
  cooldownUntil: string | null
  retryAnotherAccount: boolean
  poisonAccount: boolean
  operatorSummary: string
  customerSafeSummary: string | null
}>

export type ProviderAccountFailoverAction = Readonly<{
  failureClass: ProviderAccountFailoverFailureClass
  accountStateAction:
    | 'requires_reauth'
    | 'low_credit_cooldown'
    | 'timed_cooldown'
    | 'provider_outage_cooldown'
    | 'grant_path_failure'
    | 'do_not_poison_account'
    | 'unknown_failure_cooldown'
  health: 'healthy' | 'unhealthy' | 'requires_reauth' | null
  lowCredit: boolean
  cooldownUntil: string | null
  recentFailureClass: ProviderAccountFailoverFailureClass | null
  customerSafeStatus: string
  retryAllowed: boolean
}>

export const classifyProviderAccountFailover = (
  failureClass: ProviderAccountFailoverFailureClass,
  now: string,
): ProviderAccountFailoverAction => {
  switch (failureClass) {
    case 'token_invalidated':
      return {
        failureClass,
        accountStateAction: 'requires_reauth',
        health: 'requires_reauth',
        lowCredit: false,
        cooldownUntil: null,
        recentFailureClass: failureClass,
        customerSafeStatus:
          'Work is retrying with another connected account while one account is reconnected.',
        retryAllowed: true,
      }
    case 'low_credits':
    case 'quota_exhausted':
      return {
        failureClass,
        accountStateAction: 'low_credit_cooldown',
        health: 'unhealthy',
        lowCredit: true,
        cooldownUntil: isoTimestampAfterIso(now, 24 * 60 * 60 * 1_000),
        recentFailureClass: failureClass,
        customerSafeStatus:
          'Work is retrying with another account while capacity is refreshed.',
        retryAllowed: true,
      }
    case 'rate_limited':
      return {
        failureClass,
        accountStateAction: 'timed_cooldown',
        health: 'unhealthy',
        lowCredit: false,
        cooldownUntil: isoTimestampAfterIso(now, 60 * 60 * 1_000),
        recentFailureClass: failureClass,
        customerSafeStatus:
          'Work is retrying with another account after a temporary provider limit.',
        retryAllowed: true,
      }
    case 'provider_outage':
      return {
        failureClass,
        accountStateAction: 'provider_outage_cooldown',
        health: 'unhealthy',
        lowCredit: false,
        cooldownUntil: isoTimestampAfterIso(now, 10 * 60 * 1_000),
        recentFailureClass: failureClass,
        customerSafeStatus:
          'Work is retrying after a temporary provider availability issue.',
        retryAllowed: true,
      }
    case 'launch_timeout':
      return {
        failureClass,
        accountStateAction: 'timed_cooldown',
        health: 'unhealthy',
        lowCredit: false,
        cooldownUntil: isoTimestampAfterIso(now, 15 * 60 * 1_000),
        recentFailureClass: failureClass,
        customerSafeStatus:
          'Work is retrying after a launch timeout on one execution path.',
        retryAllowed: true,
      }
    case 'grant_resolution_failed':
      return {
        failureClass,
        accountStateAction: 'grant_path_failure',
        health: null,
        lowCredit: false,
        cooldownUntil: isoTimestampAfterIso(now, 5 * 60 * 1_000),
        recentFailureClass: failureClass,
        customerSafeStatus:
          'Work is retrying after a private account handoff failed.',
        retryAllowed: true,
      }
    case 'runner_failure':
      return {
        failureClass,
        accountStateAction: 'do_not_poison_account',
        health: null,
        lowCredit: false,
        cooldownUntil: null,
        recentFailureClass: failureClass,
        customerSafeStatus:
          'Work is retrying after an execution environment failure.',
        retryAllowed: true,
      }
    case 'unknown_provider_failure':
      return {
        failureClass,
        accountStateAction: 'unknown_failure_cooldown',
        health: 'unhealthy',
        lowCredit: false,
        cooldownUntil: isoTimestampAfterIso(now, 15 * 60 * 1_000),
        recentFailureClass: failureClass,
        customerSafeStatus:
          'Work is retrying after a temporary provider failure.',
        retryAllowed: true,
      }
  }
}

const failoverClassForHealthClassification = (
  classification: ProviderAccountHealthClassification,
): ProviderAccountFailoverFailureClass | undefined =>
  classification === 'token_invalidated' || classification === 'requires_reauth'
    ? 'token_invalidated'
    : classification === 'low_credits'
      ? 'low_credits'
      : classification === 'rate_limited'
        ? 'rate_limited'
        : classification === 'quota_exhausted'
          ? 'quota_exhausted'
          : classification === 'provider_outage'
            ? 'provider_outage'
            : classification === 'launch_timeout'
              ? 'launch_timeout'
              : classification === 'grant_resolution_failed'
                ? 'grant_resolution_failed'
                : classification === 'runner_failure'
                  ? 'runner_failure'
                  : classification === 'wrong_account_or_collision' ||
                      classification === 'unknown_failure'
                    ? 'unknown_provider_failure'
                    : undefined

const operatorSummaryForClassification = (
  classification: ProviderAccountHealthClassification,
): string =>
  classification === 'healthy'
    ? 'Provider account is healthy.'
    : classification === 'wrong_account_or_collision'
      ? 'Provider account probe detected wrong-account or collision symptoms.'
      : classification === 'unknown_failure'
        ? 'Provider account event failed closed as an unknown failure.'
        : `Provider account event classified as ${classification}.`

const classificationFromStructuredCode = (
  code: string | undefined,
): ProviderAccountHealthClassification | undefined =>
  code === 'token_invalidated' ||
  code === 'token_revoked' ||
  code === 'invalid_token'
    ? 'token_invalidated'
    : code === 'requires_reauth'
      ? 'requires_reauth'
      : code === 'low_credits' || code === 'low_credit'
        ? 'low_credits'
        : code === 'rate_limited' || code === 'rate_limit'
          ? 'rate_limited'
          : code === 'quota_exhausted'
            ? 'quota_exhausted'
            : code === 'provider_outage'
              ? 'provider_outage'
              : code === 'launch_timeout' || code === 'timeout'
                ? 'launch_timeout'
                : code === 'grant_resolution_failed'
                  ? 'grant_resolution_failed'
                  : code === 'runner_failure'
                    ? 'runner_failure'
                    : code === 'wrong_account_identity' ||
                        code === 'auth_material_overwrite' ||
                        code === 'grant_account_mismatch' ||
                        code === 'lease_isolation_failed' ||
                        code === 'hidden_global_lock_detected'
                      ? 'wrong_account_or_collision'
                      : undefined

export const classifyProviderAccountHealthEvent = (
  input: ProviderAccountHealthEventInput,
  now: string,
): ProviderAccountHealthClassificationResult => {
  const classification =
    input.collisionClass !== undefined && input.collisionClass !== 'none'
      ? 'wrong_account_or_collision'
      : input.sanityClassification === 'healthy'
        ? 'healthy'
        : input.sanityClassification === 'requires_reauth'
          ? 'requires_reauth'
          : input.sanityClassification === 'low_credit'
            ? 'low_credits'
            : (classificationFromStructuredCode(
                input.code ?? input.sanityClassification ?? input.runnerStatus,
              ) ??
              (input.providerStatus !== undefined && input.providerStatus >= 500
                ? 'provider_outage'
                : 'unknown_failure'))

  const failoverClass = failoverClassForHealthClassification(classification)

  if (failoverClass === undefined) {
    return {
      classification,
      accountStateAction: 'none',
      health: 'healthy',
      cooldownUntil: null,
      customerSafeSummary: null,
      operatorSummary: operatorSummaryForClassification(classification),
      poisonAccount: false,
      retryAnotherAccount: false,
    }
  }

  const action = classifyProviderAccountFailover(failoverClass, now)
  const poisonAccount =
    action.accountStateAction !== 'do_not_poison_account' &&
    action.accountStateAction !== 'grant_path_failure'

  return {
    classification,
    accountStateAction: action.accountStateAction,
    health: action.health,
    cooldownUntil: action.cooldownUntil,
    customerSafeSummary: action.customerSafeStatus,
    operatorSummary: operatorSummaryForClassification(classification),
    poisonAccount,
    retryAnotherAccount: action.retryAllowed,
  }
}
