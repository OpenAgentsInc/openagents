import { redactProviderAccountLogValue } from '@openagentsinc/provider-account-schema'

import {
  type ProviderAccountError,
  isProviderAccountError,
} from './provider-account-errors'

export const providerAccountHttpStatusByTag = {
  ProviderAccountClientRequestFailed: 502,
  ProviderAccountCredentialMaterial: 400,
  ProviderAccountInvalidVerificationUrl: 502,
  ProviderAccountNotFound: 404,
  ProviderAccountRefMismatch: 409,
  ProviderDeviceLoginAttemptAlreadyConnected: 409,
  ProviderDeviceLoginAttemptExpired: 409,
  ProviderDeviceLoginAttemptNotPending: 409,
  ProviderAccountNotConnectedHealthy: 409,
  ProviderApiKeyInvalid: 400,
  ProviderApiKeyRejected: 422,
  ProviderGrantAccountMismatch: 409,
  ProviderGrantExpired: 409,
  ProviderGrantNotIssued: 409,
  ProviderGrantRunnerSessionMismatch: 409,
  ProviderAccountReloadFailed: 500,
  ProviderAccountStorageFailed: 500,
  ProviderTokenCustodyRefreshFailed: 409,
} satisfies Record<ProviderAccountError['_tag'], number>

const providerAccountClientRequestHttpStatus = (
  error: Extract<
    ProviderAccountError,
    { _tag: 'ProviderAccountClientRequestFailed' }
  >,
): number => {
  if (error.status === 429) {
    return 429
  }

  return providerAccountHttpStatusByTag.ProviderAccountClientRequestFailed
}

export const providerAccountHttpStatus = (
  error: ProviderAccountError,
): number =>
  error._tag === 'ProviderAccountClientRequestFailed'
    ? providerAccountClientRequestHttpStatus(error)
    : providerAccountHttpStatusByTag[error._tag]

export const providerAccountRouteErrorStatus = (
  error: unknown,
  fallbackStatus = 400,
): number =>
  isProviderAccountError(error)
    ? providerAccountHttpStatus(error)
    : fallbackStatus

export const providerAccountRouteErrorName = (error: unknown): string =>
  isProviderAccountError(error)
    ? error._tag
    : error instanceof Error
      ? error.name
      : typeof error

export const providerAccountRouteErrorMessage = (error: unknown): string =>
  redactProviderAccountLogValue(
    error instanceof Error ? error.message : String(error),
  )
