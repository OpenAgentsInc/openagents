import { describe, expect, test } from 'vitest'

import {
  GitHubWriteApiFailure,
  GitHubWriteCallbackMismatch,
  GitHubWriteGrantExpired,
  GitHubWriteGrantRunnerSessionMismatch,
  GitHubWriteMissingConnection,
  GitHubWritePermissionFailure,
  GitHubWriteTokenStorageFailure,
} from './github-write-connections'
import {
  gitHubWriteRouteErrorMessage,
  gitHubWriteRouteErrorName,
  gitHubWriteRouteErrorStatus,
} from './github-write-route-errors'

describe('GitHub write route errors', () => {
  test('maps typed GitHub write errors to route statuses by tag', () => {
    expect(
      gitHubWriteRouteErrorStatus(
        new GitHubWriteMissingConnection({
          message: 'GitHub write connection is not usable.',
        }),
      ),
    ).toBe(404)
    expect(
      gitHubWriteRouteErrorStatus(
        new GitHubWriteGrantExpired({
          message: 'GitHub write grant is expired.',
        }),
      ),
    ).toBe(409)
    expect(
      gitHubWriteRouteErrorStatus(
        new GitHubWriteGrantRunnerSessionMismatch({
          message: 'GitHub write grant runner session does not match request.',
        }),
      ),
    ).toBe(409)
    expect(
      gitHubWriteRouteErrorStatus(
        new GitHubWriteCallbackMismatch({
          message: 'Connected GitHub account does not match the signed-in user.',
        }),
      ),
    ).toBe(409)
    expect(
      gitHubWriteRouteErrorStatus(
        new GitHubWritePermissionFailure({
          message: 'GitHub write OAuth token is missing repo/workflow scopes.',
        }),
      ),
    ).toBe(409)
    expect(
      gitHubWriteRouteErrorStatus(
        new GitHubWriteApiFailure({
          operation: 'oauth_token_exchange',
          status: 503,
          message: 'GitHub OAuth token exchange failed with 503',
        }),
      ),
    ).toBe(502)
    expect(
      gitHubWriteRouteErrorStatus(
        new GitHubWriteTokenStorageFailure({
          operation: 'put',
          message: 'GitHub write token storage failed.',
        }),
      ),
    ).toBe(500)
  })

  test('uses typed names and redacted messages', () => {
    const error = new GitHubWriteTokenStorageFailure({
      operation: 'put',
      message: 'storage failed for gho_abcdefghijklmnopqrstuvwxyz',
    })

    expect(gitHubWriteRouteErrorName(error)).toBe(
      'GitHubWriteTokenStorageFailure',
    )
    expect(gitHubWriteRouteErrorMessage(error)).not.toContain(
      'gho_abcdefghijklmnopqrstuvwxyz',
    )
    expect(gitHubWriteRouteErrorMessage(error)).toContain('gho_[REDACTED]')
  })
})
