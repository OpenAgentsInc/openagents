import { Schema as S } from 'effect'

export class ProviderAccountCredentialMaterial extends S.TaggedErrorClass<ProviderAccountCredentialMaterial>()(
  'ProviderAccountCredentialMaterial',
  {
    fieldName: S.String,
    message: S.String,
  },
) {}

export class ProviderAccountInvalidVerificationUrl extends S.TaggedErrorClass<ProviderAccountInvalidVerificationUrl>()(
  'ProviderAccountInvalidVerificationUrl',
  {
    message: S.String,
  },
) {}

export class ProviderAccountClientRequestFailed extends S.TaggedErrorClass<ProviderAccountClientRequestFailed>()(
  'ProviderAccountClientRequestFailed',
  {
    endpoint: S.String,
    status: S.Number,
    message: S.String,
  },
) {}

export class ProviderAccountNotFound extends S.TaggedErrorClass<ProviderAccountNotFound>()(
  'ProviderAccountNotFound',
  {
    message: S.String,
  },
) {}

export class ProviderAccountRefMismatch extends S.TaggedErrorClass<ProviderAccountRefMismatch>()(
  'ProviderAccountRefMismatch',
  {
    message: S.String,
  },
) {}

export class ProviderDeviceLoginAttemptNotPending extends S.TaggedErrorClass<ProviderDeviceLoginAttemptNotPending>()(
  'ProviderDeviceLoginAttemptNotPending',
  {
    message: S.String,
  },
) {}

export class ProviderDeviceLoginAttemptExpired extends S.TaggedErrorClass<ProviderDeviceLoginAttemptExpired>()(
  'ProviderDeviceLoginAttemptExpired',
  {
    message: S.String,
  },
) {}

export class ProviderDeviceLoginAttemptAlreadyConnected extends S.TaggedErrorClass<ProviderDeviceLoginAttemptAlreadyConnected>()(
  'ProviderDeviceLoginAttemptAlreadyConnected',
  {
    message: S.String,
  },
) {}

export class ProviderAccountNotConnectedHealthy extends S.TaggedErrorClass<ProviderAccountNotConnectedHealthy>()(
  'ProviderAccountNotConnectedHealthy',
  {
    message: S.String,
  },
) {}

export class ProviderApiKeyInvalid extends S.TaggedErrorClass<ProviderApiKeyInvalid>()(
  'ProviderApiKeyInvalid',
  {
    message: S.String,
  },
) {}

export class ProviderApiKeyRejected extends S.TaggedErrorClass<ProviderApiKeyRejected>()(
  'ProviderApiKeyRejected',
  {
    message: S.String,
  },
) {}

export class ProviderGrantAccountMismatch extends S.TaggedErrorClass<ProviderGrantAccountMismatch>()(
  'ProviderGrantAccountMismatch',
  {
    message: S.String,
  },
) {}

export class ProviderGrantRunnerSessionMismatch extends S.TaggedErrorClass<ProviderGrantRunnerSessionMismatch>()(
  'ProviderGrantRunnerSessionMismatch',
  {
    message: S.String,
  },
) {}

export class ProviderGrantExpired extends S.TaggedErrorClass<ProviderGrantExpired>()(
  'ProviderGrantExpired',
  {
    message: S.String,
  },
) {}

export class ProviderGrantNotIssued extends S.TaggedErrorClass<ProviderGrantNotIssued>()(
  'ProviderGrantNotIssued',
  {
    message: S.String,
  },
) {}

export class ProviderAccountReloadFailed extends S.TaggedErrorClass<ProviderAccountReloadFailed>()(
  'ProviderAccountReloadFailed',
  {
    operation: S.String,
    message: S.String,
  },
) {}

export class ProviderAccountStorageFailed extends S.TaggedErrorClass<ProviderAccountStorageFailed>()(
  'ProviderAccountStorageFailed',
  {
    operation: S.String,
    message: S.String,
  },
) {}

export class ProviderTokenCustodyRefreshFailed extends S.TaggedErrorClass<ProviderTokenCustodyRefreshFailed>()(
  'ProviderTokenCustodyRefreshFailed',
  {
    providerAccountRef: S.String,
    failureClass: S.String,
    providerStatus: S.Number,
    message: S.String,
  },
) {}

export const ProviderAccountError = S.Union([
  ProviderAccountCredentialMaterial,
  ProviderAccountInvalidVerificationUrl,
  ProviderAccountClientRequestFailed,
  ProviderAccountNotFound,
  ProviderAccountRefMismatch,
  ProviderDeviceLoginAttemptNotPending,
  ProviderDeviceLoginAttemptExpired,
  ProviderDeviceLoginAttemptAlreadyConnected,
  ProviderAccountNotConnectedHealthy,
  ProviderApiKeyInvalid,
  ProviderApiKeyRejected,
  ProviderGrantAccountMismatch,
  ProviderGrantRunnerSessionMismatch,
  ProviderGrantExpired,
  ProviderGrantNotIssued,
  ProviderAccountReloadFailed,
  ProviderAccountStorageFailed,
  ProviderTokenCustodyRefreshFailed,
])
export type ProviderAccountError = typeof ProviderAccountError.Type

const providerAccountErrorTags = new Set([
  'ProviderAccountCredentialMaterial',
  'ProviderAccountInvalidVerificationUrl',
  'ProviderAccountClientRequestFailed',
  'ProviderAccountNotFound',
  'ProviderAccountRefMismatch',
  'ProviderDeviceLoginAttemptNotPending',
  'ProviderDeviceLoginAttemptExpired',
  'ProviderDeviceLoginAttemptAlreadyConnected',
  'ProviderAccountNotConnectedHealthy',
  'ProviderApiKeyInvalid',
  'ProviderApiKeyRejected',
  'ProviderGrantAccountMismatch',
  'ProviderGrantRunnerSessionMismatch',
  'ProviderGrantExpired',
  'ProviderGrantNotIssued',
  'ProviderAccountReloadFailed',
  'ProviderAccountStorageFailed',
  'ProviderTokenCustodyRefreshFailed',
])

export const isProviderAccountError = (
  error: unknown,
): error is ProviderAccountError =>
  typeof error === 'object' &&
  error !== null &&
  '_tag' in error &&
  typeof error._tag === 'string' &&
  providerAccountErrorTags.has(error._tag)

export const providerAccountErrorFromUnknown = (
  operation: string,
  error: unknown,
): ProviderAccountError =>
  isProviderAccountError(error)
    ? error
    : new ProviderAccountStorageFailed({
        operation,
        message: error instanceof Error ? error.message : String(error),
      })

export const providerAccountErrorMessage = (
  error: ProviderAccountError,
): string => error.message
