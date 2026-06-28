import { Match as M, Option } from 'effect'
import { evo } from 'foldkit/struct'

import {
  ProviderDeviceLoginStartResponse,
  ProviderDeviceLoginStatusResponse,
} from '../../../domain/session'
import { Message } from '../message'
import {
  FailedProviderConnectionAction,
  Model,
  PollingProviderDeviceLogin,
  ProviderAccountPoolFailed,
  ProviderAccountPoolLoaded,
  ProviderAccountPoolLoading,
  StartingProviderDeviceLogin,
  SucceededProviderConnectionAction,
  authWithProviderAccounts,
  providerAccountBundleFromAuth,
} from '../model'
import { type UpdateReturn, noUpdate } from '../transition'
import {
  LoadProviderAccountPool,
  PollProviderDeviceLogin,
  ResetProviderAccountPoolAccount,
  StartProviderDeviceLogin,
} from './commands'

const withUpdateReturn = M.withReturnType<UpdateReturn>()

const applyProviderConnection = (
  model: Model,
  response:
    | ProviderDeviceLoginStartResponse
    | ProviderDeviceLoginStatusResponse,
): Model => {
  const bundle = providerAccountBundleFromAuth(model.auth)
  const accounts = [
    response.account,
    ...bundle.accounts.filter(account => account.id !== response.account.id),
  ]
  const attempts = [
    response.attempt,
    ...bundle.attempts.filter(attempt => attempt.id !== response.attempt.id),
  ]

  return evo(model, {
    auth: () =>
      authWithProviderAccounts(model.auth, {
        accounts,
        attempts,
      }),
  })
}

const optimisticResetPoolAccount = (
  model: Model,
  providerAccountRef: string,
): Model => {
  if (model.providerAccountPool._tag !== 'ProviderAccountPoolLoaded') {
    return model
  }

  const response = model.providerAccountPool.response
  const accounts = response.accounts.map(account => {
    if (account.providerAccountRef !== providerAccountRef) {
      return account
    }

    const eligibilityReasons = account.eligibilityReasons.filter(
      reason => reason !== 'cooldown',
    )

    return {
      ...account,
      cooldownRemainingSeconds: null,
      cooldownUntil: null,
      eligibility:
        eligibilityReasons.length === 0 ? 'eligible' : account.eligibility,
      eligibilityReasons,
      recentFailureClass:
        account.recentFailureClass === 'rate_limited'
          ? null
          : account.recentFailureClass,
    }
  })

  return evo(model, {
    providerAccountPool: () =>
      ProviderAccountPoolLoaded({
        response: {
          ...response,
          accounts,
          summary: {
            ...response.summary,
            cooldown: accounts.filter(account =>
              account.eligibilityReasons.includes('cooldown'),
            ).length,
            eligible: accounts.filter(
              account => account.eligibility === 'eligible',
            ).length,
          },
        },
      }),
  })
}

export const updateProviders = (model: Model, message: Message): UpdateReturn =>
  M.value(message).pipe(
    withUpdateReturn,
    M.tags({
      ClickedStartProviderDeviceLogin: ({ createNew, providerAccountRef }) => [
        evo(model, {
          providerConnectionAction: () => StartingProviderDeviceLogin(),
        }),
        [
          StartProviderDeviceLogin({
            ...(createNew === undefined ? {} : { createNew }),
            ...(providerAccountRef === undefined ? {} : { providerAccountRef }),
          }),
        ],
        Option.none(),
      ],
      ClickedPollProviderDeviceLogin: ({ attemptId }) => [
        evo(model, {
          providerConnectionAction: () =>
            PollingProviderDeviceLogin({ attemptId }),
        }),
        [PollProviderDeviceLogin({ attemptId })],
        Option.none(),
      ],
      RequestedLoadProviderAccountPool: () => [
        evo(model, {
          providerAccountPool: () => ProviderAccountPoolLoading(),
        }),
        [LoadProviderAccountPool({})],
        Option.none(),
      ],
      SucceededLoadProviderAccountPool: ({ response }) => [
        evo(model, {
          providerAccountPool: () => ProviderAccountPoolLoaded({ response }),
        }),
        [],
        Option.none(),
      ],
      FailedLoadProviderAccountPool: ({ error }) => [
        evo(model, {
          providerAccountPool: () => ProviderAccountPoolFailed({ error }),
        }),
        [],
        Option.none(),
      ],
      ClickedResetProviderAccountPoolAccount: ({ providerAccountRef }) => [
        optimisticResetPoolAccount(model, providerAccountRef),
        [ResetProviderAccountPoolAccount({ providerAccountRef })],
        Option.none(),
      ],
      SucceededResetProviderAccountPoolAccount: () => [
        model,
        [LoadProviderAccountPool({})],
        Option.none(),
      ],
      FailedResetProviderAccountPoolAccount: () => [
        model,
        [LoadProviderAccountPool({})],
        Option.none(),
      ],
      SucceededStartProviderDeviceLogin: ({ response }) => {
        const nextModel = applyProviderConnection(model, response)

        if (response.attempt.status === 'connected') {
          return [
            evo(nextModel, {
              providerConnectionAction: () =>
                SucceededProviderConnectionAction({
                  message: 'ChatGPT account connected.',
                }),
            }),
            [LoadProviderAccountPool({})],
            Option.none(),
          ]
        }

        return [
          evo(nextModel, {
            providerConnectionAction: () =>
              PollingProviderDeviceLogin({ attemptId: response.attempt.id }),
          }),
          [
            PollProviderDeviceLogin({
              attemptId: response.attempt.id,
              delayMillis: Math.max(1, response.intervalSeconds) * 1000,
            }),
          ],
          Option.none(),
        ]
      },
      FailedStartProviderDeviceLogin: ({ error }) => [
        evo(model, {
          providerConnectionAction: () =>
            FailedProviderConnectionAction({ error }),
        }),
        [],
        Option.none(),
      ],
      SucceededPollProviderDeviceLogin: ({ response }) => {
        const nextModel = applyProviderConnection(model, response)

        if (response.attempt.status === 'connected') {
          return [
            evo(nextModel, {
              providerConnectionAction: () =>
                SucceededProviderConnectionAction({
                  message: 'ChatGPT account connected.',
                }),
            }),
            [LoadProviderAccountPool({})],
            Option.none(),
          ]
        }

        if (response.attempt.status !== 'pending') {
          return [
            evo(nextModel, {
              providerConnectionAction: () =>
                FailedProviderConnectionAction({
                  error: `Device login ${response.attempt.status}.`,
                }),
            }),
            [],
            Option.none(),
          ]
        }

        return [
          evo(nextModel, {
            providerConnectionAction: () =>
              PollingProviderDeviceLogin({ attemptId: response.attempt.id }),
          }),
          [
            PollProviderDeviceLogin({
              attemptId: response.attempt.id,
              delayMillis: 3000,
            }),
          ],
          Option.none(),
        ]
      },
      FailedPollProviderDeviceLogin: ({ error }) => [
        evo(model, {
          providerConnectionAction: () =>
            FailedProviderConnectionAction({ error }),
        }),
        [],
        Option.none(),
      ],
    }),
    M.orElse(() => noUpdate(model)),
  )
