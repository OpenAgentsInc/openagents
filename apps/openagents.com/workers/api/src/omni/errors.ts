import { Schema as S } from 'effect'

const unknownMessage = (error: unknown): string =>
  error instanceof Error ? error.message : String(error)

export class OmniAssignmentError extends S.TaggedErrorClass<OmniAssignmentError>()(
  'OmniAssignmentError',
  {
    message: S.String,
    operation: S.String,
  },
) {}

export class OmniRepositoryError extends S.TaggedErrorClass<OmniRepositoryError>()(
  'OmniRepositoryError',
  {
    message: S.String,
    operation: S.String,
  },
) {}

export class OmniDispatchError extends S.TaggedErrorClass<OmniDispatchError>()(
  'OmniDispatchError',
  {
    message: S.String,
    operation: S.String,
  },
) {}

export class OmniDispatchMissingCredentials extends S.TaggedErrorClass<OmniDispatchMissingCredentials>()(
  'OmniDispatchMissingCredentials',
  {
    message: S.String,
    operation: S.String,
  },
) {}

export class OmniDispatchUnavailableEndpoint extends S.TaggedErrorClass<OmniDispatchUnavailableEndpoint>()(
  'OmniDispatchUnavailableEndpoint',
  {
    endpoint: S.String,
    message: S.String,
    operation: S.String,
  },
) {}

export class OmniDispatchRejectedRequest extends S.TaggedErrorClass<OmniDispatchRejectedRequest>()(
  'OmniDispatchRejectedRequest',
  {
    endpoint: S.String,
    message: S.String,
    operation: S.String,
    status: S.Number,
  },
) {}

export class OmniDispatchMalformedResponse extends S.TaggedErrorClass<OmniDispatchMalformedResponse>()(
  'OmniDispatchMalformedResponse',
  {
    endpoint: S.String,
    message: S.String,
    operation: S.String,
  },
) {}

export class OmniDispatchTimeout extends S.TaggedErrorClass<OmniDispatchTimeout>()(
  'OmniDispatchTimeout',
  {
    endpoint: S.String,
    message: S.String,
    operation: S.String,
    timeoutMs: S.Number,
  },
) {}

export class OmniDispatchTransportFailure extends S.TaggedErrorClass<OmniDispatchTransportFailure>()(
  'OmniDispatchTransportFailure',
  {
    endpoint: S.String,
    message: S.String,
    operation: S.String,
  },
) {}

export class OmniRunnerCallbackDecodeError extends S.TaggedErrorClass<OmniRunnerCallbackDecodeError>()(
  'OmniRunnerCallbackDecodeError',
  {
    message: S.String,
    operation: S.String,
  },
) {}

export class OmniBillingError extends S.TaggedErrorClass<OmniBillingError>()(
  'OmniBillingError',
  {
    message: S.String,
    userId: S.String,
  },
) {}

export class OmniProjectionError extends S.TaggedErrorClass<OmniProjectionError>()(
  'OmniProjectionError',
  {
    message: S.String,
    operation: S.String,
  },
) {}

export const OmniError = S.Union([
  OmniAssignmentError,
  OmniRepositoryError,
  OmniDispatchError,
  OmniDispatchMissingCredentials,
  OmniDispatchUnavailableEndpoint,
  OmniDispatchRejectedRequest,
  OmniDispatchMalformedResponse,
  OmniDispatchTimeout,
  OmniDispatchTransportFailure,
  OmniRunnerCallbackDecodeError,
  OmniBillingError,
  OmniProjectionError,
])
export type OmniError = typeof OmniError.Type

const omniErrorTags = new Set([
  'OmniAssignmentError',
  'OmniRepositoryError',
  'OmniDispatchError',
  'OmniDispatchMissingCredentials',
  'OmniDispatchUnavailableEndpoint',
  'OmniDispatchRejectedRequest',
  'OmniDispatchMalformedResponse',
  'OmniDispatchTimeout',
  'OmniDispatchTransportFailure',
  'OmniRunnerCallbackDecodeError',
  'OmniBillingError',
  'OmniProjectionError',
])

export const isOmniError = (error: unknown): error is OmniError =>
  typeof error === 'object' &&
  error !== null &&
  '_tag' in error &&
  typeof error._tag === 'string' &&
  omniErrorTags.has(error._tag)

export const omniAssignmentErrorFromUnknown = (
  operation: string,
  error: unknown,
): OmniAssignmentError =>
  new OmniAssignmentError({
    operation,
    message: unknownMessage(error),
  })

export const omniErrorFromUnknown = (
  operation: string,
  error: unknown,
): OmniRepositoryError =>
  new OmniRepositoryError({
    operation,
    message: unknownMessage(error),
  })

export const omniDispatchErrorFromUnknown = (
  operation: string,
  error: unknown,
): OmniError =>
  isOmniError(error)
    ? error
    : new OmniDispatchError({
        operation,
        message: unknownMessage(error),
      })

export const omniRunnerCallbackDecodeErrorFromUnknown = (
  operation: string,
  error: unknown,
): OmniRunnerCallbackDecodeError =>
  new OmniRunnerCallbackDecodeError({
    operation,
    message: unknownMessage(error),
  })

export const omniProjectionErrorFromUnknown = (
  operation: string,
  error: unknown,
): OmniProjectionError =>
  new OmniProjectionError({
    operation,
    message: unknownMessage(error),
  })
