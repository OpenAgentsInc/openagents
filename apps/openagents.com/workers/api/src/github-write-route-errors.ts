import { redactProviderAccountLogValue } from '@openagentsinc/provider-account-schema'

import {
  type GitHubWriteError,
  isGitHubWriteError,
} from './github-write-connections'

export const gitHubWriteHttpStatusByTag = {
  GitHubWriteApiFailure: 502,
  GitHubWriteCallbackMismatch: 409,
  GitHubWriteConnectionNotUsable: 409,
  GitHubWriteGrantExpired: 409,
  GitHubWriteGrantNotIssued: 409,
  GitHubWriteGrantRunnerSessionMismatch: 409,
  GitHubWriteMissingConnection: 404,
  GitHubWritePermissionFailure: 409,
  GitHubWriteReloadFailure: 500,
  GitHubWriteRepositoryFailure: 500,
  GitHubWriteTokenStorageFailure: 500,
} satisfies Record<GitHubWriteError['_tag'], number>

export const gitHubWriteRouteErrorStatus = (
  error: unknown,
  fallbackStatus = 400,
): number =>
  isGitHubWriteError(error)
    ? gitHubWriteHttpStatusByTag[error._tag]
    : fallbackStatus

export const gitHubWriteRouteErrorName = (error: unknown): string =>
  isGitHubWriteError(error)
    ? error._tag
    : error instanceof Error
      ? error.name
      : typeof error

export const gitHubWriteRouteErrorMessage = (error: unknown): string =>
  redactProviderAccountLogValue(
    error instanceof Error ? error.message : String(error),
  )
