import { Duration, Effect, Schema as S } from 'effect'
import { Command } from 'foldkit'

import {
  ProviderDeviceLoginStartResponse,
  ProviderDeviceLoginStatusResponse,
} from '../../../domain/session'
import { errorMessageFromUnknown, requestJson } from '../commands/api'
import {
  FailedLoadProviderAccountPool,
  FailedPollProviderDeviceLogin,
  FailedResetProviderAccountPoolAccount,
  FailedStartProviderDeviceLogin,
  SucceededLoadProviderAccountPool,
  SucceededPollProviderDeviceLogin,
  SucceededResetProviderAccountPoolAccount,
  SucceededStartProviderDeviceLogin,
} from '../message'
import {
  ProviderAccountPoolManualResetResponse,
  ProviderAccountPoolResponse,
} from '../model'

export const StartProviderDeviceLogin = Command.define(
  'StartProviderDeviceLogin',
  {
    createNew: S.optionalKey(S.Boolean),
    providerAccountRef: S.optionalKey(S.String),
  },
  SucceededStartProviderDeviceLogin,
  FailedStartProviderDeviceLogin,
)(({ createNew, providerAccountRef }) =>
  Effect.gen(function* () {
    const response = yield* requestJson({
      init: {
        body: JSON.stringify({
          ...(createNew === undefined ? {} : { createNew }),
          ...(providerAccountRef === undefined ? {} : { providerAccountRef }),
        }),
        cache: 'no-store',
        credentials: 'include',
        headers: {
          accept: 'application/json',
          'content-type': 'application/json',
        },
        method: 'POST',
      },
      name: 'loggedIn.providerDeviceLogin.start',
      request: '/api/provider-accounts/chatgpt-codex/device-login/start',
      schema: ProviderDeviceLoginStartResponse,
    })

    return SucceededStartProviderDeviceLogin({ response })
  }).pipe(
    Effect.catch(error =>
      Effect.succeed(
        FailedStartProviderDeviceLogin({
          error: errorMessageFromUnknown(error),
        }),
      ),
    ),
  ),
)

export const LoadProviderAccountPool = Command.define(
  'LoadProviderAccountPool',
  {},
  SucceededLoadProviderAccountPool,
  FailedLoadProviderAccountPool,
)(() =>
  Effect.gen(function* () {
    const response = yield* requestJson({
      init: {
        cache: 'no-store',
        credentials: 'include',
        headers: { accept: 'application/json' },
      },
      name: 'loggedIn.providerAccountPool.load',
      request: '/api/provider-accounts/pool',
      schema: ProviderAccountPoolResponse,
    })

    return SucceededLoadProviderAccountPool({ response })
  }).pipe(
    Effect.catch(error =>
      Effect.succeed(
        FailedLoadProviderAccountPool({
          error: errorMessageFromUnknown(error),
        }),
      ),
    ),
  ),
)

export const ResetProviderAccountPoolAccount = Command.define(
  'ResetProviderAccountPoolAccount',
  { providerAccountRef: S.String },
  SucceededResetProviderAccountPoolAccount,
  FailedResetProviderAccountPoolAccount,
)(({ providerAccountRef }) =>
  Effect.gen(function* () {
    const response = yield* requestJson({
      init: {
        body: JSON.stringify({ providerAccountRef }),
        cache: 'no-store',
        credentials: 'include',
        headers: {
          accept: 'application/json',
          'content-type': 'application/json',
        },
        method: 'POST',
      },
      name: 'loggedIn.providerAccountPool.resetAccount',
      request: '/api/provider-accounts/pool/reset',
      schema: ProviderAccountPoolManualResetResponse,
    })

    return SucceededResetProviderAccountPoolAccount({ response })
  }).pipe(
    Effect.catch(error =>
      Effect.succeed(
        FailedResetProviderAccountPoolAccount({
          error: errorMessageFromUnknown(error),
          providerAccountRef,
        }),
      ),
    ),
  ),
)

export const PollProviderDeviceLogin = Command.define(
  'PollProviderDeviceLogin',
  { attemptId: S.String, delayMillis: S.optionalKey(S.Number) },
  SucceededPollProviderDeviceLogin,
  FailedPollProviderDeviceLogin,
)(({ attemptId, delayMillis }) =>
  Effect.gen(function* () {
    if (delayMillis !== undefined && delayMillis > 0) {
      yield* Effect.sleep(Duration.millis(delayMillis))
    }

    const response = yield* requestJson({
      init: {
        cache: 'no-store',
        credentials: 'include',
        headers: { accept: 'application/json' },
      },
      name: 'loggedIn.providerDeviceLogin.poll',
      request: `/api/provider-accounts/chatgpt-codex/device-login/${encodeURIComponent(attemptId)}`,
      schema: ProviderDeviceLoginStatusResponse,
    })

    return SucceededPollProviderDeviceLogin({ response })
  }).pipe(
    Effect.catch(error =>
      Effect.succeed(
        FailedPollProviderDeviceLogin({
          error: errorMessageFromUnknown(error),
        }),
      ),
    ),
  ),
)
