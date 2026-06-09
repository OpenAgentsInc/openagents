import { Effect, Match as M, Option, Schema as S } from 'effect'
import { Command } from 'foldkit'
import { evo } from 'foldkit/struct'

import {
  OnboardingRepositoriesResponse,
  OnboardingStatusResponse,
  onboardingIsComplete,
} from '../../../domain/session'
import { errorMessageFromUnknown, requestJson } from '../commands/api'
import {
  CompletedOnboarding,
  FailedLoadOnboardingRepositories,
  FailedSelectOnboardingRepository,
  FailedSkipOnboardingBilling,
  FailedSkipOnboardingRepository,
  FailedSubmitOnboardingGoal,
  Message,
  SucceededLoadOnboardingRepositories,
  SucceededSelectOnboardingRepository,
  SucceededSkipOnboardingBilling,
  SucceededSkipOnboardingRepository,
  SucceededSubmitOnboardingGoal,
} from '../message'
import {
  FailedOnboardingAction,
  FailedOnboardingRepositories,
  IdleOnboardingAction,
  LoadingOnboardingRepositories,
  Model,
  OnboardingFlowModel,
  SubmittingOnboardingAction,
  authWithOnboarding,
  clampOnboardingRepositoryPageIndex,
  onboardingWithRepositories,
} from '../model'
import { type UpdateReturn } from '../transition'

const withUpdateReturn = M.withReturnType<UpdateReturn>()

export const LoadOnboardingRepositories = Command.define(
  'LoadOnboardingRepositories',
  SucceededLoadOnboardingRepositories,
  FailedLoadOnboardingRepositories,
)(
  Effect.gen(function* () {
    const response = yield* requestJson({
      init: {
        cache: 'no-store',
        credentials: 'include',
        headers: { accept: 'application/json' },
      },
      name: 'loggedIn.onboarding.repositories.load',
      request: '/api/onboarding/repositories',
      schema: OnboardingRepositoriesResponse,
    })

    return SucceededLoadOnboardingRepositories({ response })
  }).pipe(
    Effect.catch(error =>
      Effect.succeed(
        FailedLoadOnboardingRepositories({
          error: errorMessageFromUnknown(error),
        }),
      ),
    ),
  ),
)

const SelectOnboardingRepositoryPayload = S.Union([
  S.Struct({ repositoryId: S.String }),
  S.Struct({ owner: S.String, name: S.String }),
])
type SelectOnboardingRepositoryPayload =
  typeof SelectOnboardingRepositoryPayload.Type

const SelectOnboardingRepository = Command.define(
  'SelectOnboardingRepository',
  { selection: SelectOnboardingRepositoryPayload },
  SucceededSelectOnboardingRepository,
  FailedSelectOnboardingRepository,
)(({ selection }) =>
  Effect.gen(function* () {
    const response = yield* requestJson({
      init: {
        body: JSON.stringify(selection),
        cache: 'no-store',
        credentials: 'include',
        headers: {
          accept: 'application/json',
          'content-type': 'application/json',
        },
        method: 'POST',
      },
      name: 'loggedIn.onboarding.repository.select',
      request: '/api/onboarding/repository/select',
      schema: OnboardingStatusResponse,
    })

    return SucceededSelectOnboardingRepository({ response })
  }).pipe(
    Effect.catch(error =>
      Effect.succeed(
        FailedSelectOnboardingRepository({
          error: errorMessageFromUnknown(error),
        }),
      ),
    ),
  ),
)

const UpdateOnboardingRepository = Command.define(
  'UpdateOnboardingRepository',
  { selection: SelectOnboardingRepositoryPayload },
  SucceededSelectOnboardingRepository,
  FailedSelectOnboardingRepository,
)(({ selection }) =>
  Effect.gen(function* () {
    const response = yield* requestJson({
      init: {
        body: JSON.stringify(selection),
        cache: 'no-store',
        credentials: 'include',
        headers: {
          accept: 'application/json',
          'content-type': 'application/json',
        },
        method: 'POST',
      },
      name: 'loggedIn.settings.repository.update',
      request: '/api/onboarding/repository/update',
      schema: OnboardingStatusResponse,
    })

    return SucceededSelectOnboardingRepository({ response })
  }).pipe(
    Effect.catch(error =>
      Effect.succeed(
        FailedSelectOnboardingRepository({
          error: errorMessageFromUnknown(error),
        }),
      ),
    ),
  ),
)

const SkipOnboardingRepository = Command.define(
  'SkipOnboardingRepository',
  SucceededSkipOnboardingRepository,
  FailedSkipOnboardingRepository,
)(
  Effect.gen(function* () {
    const response = yield* requestJson({
      init: {
        cache: 'no-store',
        credentials: 'include',
        headers: { accept: 'application/json' },
        method: 'POST',
      },
      name: 'loggedIn.onboarding.repository.skip',
      request: '/api/onboarding/repository/skip',
      schema: OnboardingStatusResponse,
    })

    return SucceededSkipOnboardingRepository({ response })
  }).pipe(
    Effect.catch(error =>
      Effect.succeed(
        FailedSkipOnboardingRepository({
          error: errorMessageFromUnknown(error),
        }),
      ),
    ),
  ),
)

const SkipOnboardingBilling = Command.define(
  'SkipOnboardingBilling',
  SucceededSkipOnboardingBilling,
  FailedSkipOnboardingBilling,
)(
  Effect.gen(function* () {
    const response = yield* requestJson({
      init: {
        cache: 'no-store',
        credentials: 'include',
        headers: { accept: 'application/json' },
        method: 'POST',
      },
      name: 'loggedIn.onboarding.billing.skip',
      request: '/api/onboarding/billing/skip',
      schema: OnboardingStatusResponse,
    })

    return SucceededSkipOnboardingBilling({ response })
  }).pipe(
    Effect.catch(error =>
      Effect.succeed(
        FailedSkipOnboardingBilling({
          error: errorMessageFromUnknown(error),
        }),
      ),
    ),
  ),
)

const SubmitOnboardingGoal = Command.define(
  'SubmitOnboardingGoal',
  { goal: S.String },
  SucceededSubmitOnboardingGoal,
  FailedSubmitOnboardingGoal,
)(({ goal }) =>
  Effect.gen(function* () {
    const response = yield* requestJson({
      init: {
        body: JSON.stringify({ goal }),
        cache: 'no-store',
        credentials: 'include',
        headers: {
          accept: 'application/json',
          'content-type': 'application/json',
        },
        method: 'POST',
      },
      name: 'loggedIn.onboarding.goal.submit',
      request: '/api/onboarding/goal',
      schema: OnboardingStatusResponse,
    })

    return SucceededSubmitOnboardingGoal({ response })
  }).pipe(
    Effect.catch(error =>
      Effect.succeed(
        FailedSubmitOnboardingGoal({
          error: errorMessageFromUnknown(error),
        }),
      ),
    ),
  ),
)

const applyOnboardingStatus = (
  model: Model,
  response: OnboardingStatusResponse,
): Model => {
  const auth = authWithOnboarding(model.auth, response.onboarding)
  const selectedRepositoryId =
    response.onboarding.repository._tag === 'RepositorySelected'
      ? response.onboarding.repository.repository.id
      : ''

  return evo(model, {
    auth: () => auth,
    onboarding: onboarding =>
      OnboardingFlowModel({
        ...onboarding,
        action: IdleOnboardingAction(),
        goalValue: response.onboarding.goal ?? onboarding.goalValue,
        manualRepositoryName: '',
        manualRepositoryOwner: '',
        selectedRepositoryId,
      }),
  })
}

const completionOutMessage = (response: OnboardingStatusResponse) =>
  onboardingIsComplete(response.onboarding)
    ? Option.some(CompletedOnboarding())
    : Option.none()

const nextRepositoryPageIndex = (
  onboarding: OnboardingFlowModel,
  delta: number,
): number =>
  M.value(onboarding.repositories).pipe(
    M.tagsExhaustive({
      OnboardingRepositoriesIdle: () => 0,
      OnboardingRepositoriesLoading: () => 0,
      OnboardingRepositoriesFailed: () => 0,
      OnboardingRepositoriesLoaded: ({ repositories }) =>
        clampOnboardingRepositoryPageIndex(
          onboarding.repositoryPageIndex + delta,
          repositories,
          onboarding.repositorySearch,
        ),
    }),
  )

const previousOnboardingStep = (
  step: Model['auth']['onboarding']['step'],
): Model['auth']['onboarding']['step'] =>
  M.value(step).pipe(
    M.when('repository', () => 'repository' as const),
    M.when('goal', () => 'repository' as const),
    M.when('billing', () => 'goal' as const),
    M.when('complete', () => 'billing' as const),
    M.exhaustive,
  )

const loadRepositoriesOnReturn = (
  model: Model,
  step: Model['auth']['onboarding']['step'],
): ReadonlyArray<ReturnType<typeof LoadOnboardingRepositories>> =>
  step === 'repository' &&
  model.onboarding.repositories._tag === 'OnboardingRepositoriesIdle'
    ? [LoadOnboardingRepositories()]
    : []

const isSettingsRepositoryUpdate = (model: Model): boolean =>
  model.route._tag === 'SettingsSection' &&
  model.route.section === 'connections'

const failRepositoryAction = (model: Model, error: string): UpdateReturn => [
  evo(model, {
    onboarding: onboarding =>
      OnboardingFlowModel({
        ...onboarding,
        action: FailedOnboardingAction({ error }),
      }),
  }),
  [],
  Option.none(),
]

export const updateOnboarding = (
  model: Model,
  message: Message,
): UpdateReturn =>
  M.value(message).pipe(
    withUpdateReturn,
    M.tags({
      RequestedLoadOnboardingRepositories: () => [
        evo(model, {
          onboarding: onboarding =>
            OnboardingFlowModel({
              ...onboarding,
              repositories: LoadingOnboardingRepositories(),
            }),
        }),
        [LoadOnboardingRepositories()],
        Option.none(),
      ],
      SucceededLoadOnboardingRepositories: ({ response }) => [
        evo(model, {
          onboarding: onboarding =>
            onboardingWithRepositories(onboarding, response),
        }),
        [],
        Option.none(),
      ],
      FailedLoadOnboardingRepositories: ({ error }) => [
        evo(model, {
          onboarding: onboarding =>
            OnboardingFlowModel({
              ...onboarding,
              repositories: FailedOnboardingRepositories({ error }),
            }),
        }),
        [],
        Option.none(),
      ],
      UpdatedOnboardingRepositorySearch: ({ value }) => [
        evo(model, {
          onboarding: onboarding =>
            OnboardingFlowModel({
              ...onboarding,
              repositoryPageIndex: 0,
              repositorySearch: value,
            }),
        }),
        [],
        Option.none(),
      ],
      ClickedPreviousOnboardingRepositoryPage: () => [
        evo(model, {
          onboarding: onboarding =>
            OnboardingFlowModel({
              ...onboarding,
              repositoryPageIndex: nextRepositoryPageIndex(onboarding, -1),
            }),
        }),
        [],
        Option.none(),
      ],
      ClickedNextOnboardingRepositoryPage: () => [
        evo(model, {
          onboarding: onboarding =>
            OnboardingFlowModel({
              ...onboarding,
              repositoryPageIndex: nextRepositoryPageIndex(onboarding, 1),
            }),
        }),
        [],
        Option.none(),
      ],
      ClickedPreviousOnboardingStep: () => {
        const step = previousOnboardingStep(model.auth.onboarding.step)

        return [
          evo(model, {
            auth: auth =>
              authWithOnboarding(auth, {
                ...auth.onboarding,
                step,
              }),
            onboarding: onboarding =>
              OnboardingFlowModel({
                ...onboarding,
                action: IdleOnboardingAction(),
              }),
          }),
          loadRepositoriesOnReturn(model, step),
          Option.none(),
        ]
      },
      ClickedOnboardingStep: ({ step }) => [
        evo(model, {
          auth: auth =>
            authWithOnboarding(auth, {
              ...auth.onboarding,
              step,
            }),
          onboarding: onboarding =>
            OnboardingFlowModel({
              ...onboarding,
              action: IdleOnboardingAction(),
            }),
        }),
        loadRepositoriesOnReturn(model, step),
        Option.none(),
      ],
      SelectedOnboardingRepository: ({ repositoryId }) => [
        evo(model, {
          onboarding: onboarding =>
            OnboardingFlowModel({
              ...onboarding,
              action: IdleOnboardingAction(),
              manualRepositoryName: '',
              manualRepositoryOwner: '',
              selectedRepositoryId: repositoryId,
            }),
        }),
        [],
        Option.none(),
      ],
      UpdatedOnboardingManualRepositoryOwner: ({ value }) => [
        evo(model, {
          onboarding: onboarding =>
            OnboardingFlowModel({
              ...onboarding,
              action: IdleOnboardingAction(),
              manualRepositoryOwner: value,
            }),
        }),
        [],
        Option.none(),
      ],
      UpdatedOnboardingManualRepositoryName: ({ value }) => [
        evo(model, {
          onboarding: onboarding =>
            OnboardingFlowModel({
              ...onboarding,
              action: IdleOnboardingAction(),
              manualRepositoryName: value,
            }),
        }),
        [],
        Option.none(),
      ],
      SubmittedOnboardingRepository: () => {
        const repositoryId = model.onboarding.selectedRepositoryId.trim()
        const owner = model.onboarding.manualRepositoryOwner.trim()
        const name = model.onboarding.manualRepositoryName.trim()
        const hasManualRepository = owner !== '' || name !== ''

        if (hasManualRepository && (owner === '' || name === '')) {
          return failRepositoryAction(
            model,
            'Enter owner and repository, or clear both.',
          )
        }

        if (!hasManualRepository && repositoryId === '') {
          return failRepositoryAction(model, 'Select a repository or skip.')
        }

        return [
          evo(model, {
            onboarding: onboarding =>
              OnboardingFlowModel({
                ...onboarding,
                action: SubmittingOnboardingAction({
                  label: 'Saving repository',
                }),
              }),
          }),
          [
            (isSettingsRepositoryUpdate(model)
              ? UpdateOnboardingRepository
              : SelectOnboardingRepository)({
              selection: hasManualRepository
                ? { owner, name }
                : { repositoryId },
            }),
          ],
          Option.none(),
        ]
      },
      ClickedSkipOnboardingRepository: () => [
        evo(model, {
          onboarding: onboarding =>
            OnboardingFlowModel({
              ...onboarding,
              action: SubmittingOnboardingAction({ label: 'Skipping' }),
            }),
        }),
        [SkipOnboardingRepository()],
        Option.none(),
      ],
      SucceededSelectOnboardingRepository: ({ response }) => [
        applyOnboardingStatus(model, response),
        [],
        Option.none(),
      ],
      FailedSelectOnboardingRepository: ({ error }) =>
        failRepositoryAction(model, error),
      SucceededSkipOnboardingRepository: ({ response }) => [
        applyOnboardingStatus(model, response),
        [],
        Option.none(),
      ],
      FailedSkipOnboardingRepository: ({ error }) =>
        failRepositoryAction(model, error),
      ClickedSkipOnboardingBilling: () => [
        evo(model, {
          onboarding: onboarding =>
            OnboardingFlowModel({
              ...onboarding,
              action: SubmittingOnboardingAction({ label: 'Continuing' }),
            }),
        }),
        [SkipOnboardingBilling()],
        Option.none(),
      ],
      SucceededSkipOnboardingBilling: ({ response }) => [
        applyOnboardingStatus(model, response),
        [],
        completionOutMessage(response),
      ],
      FailedSkipOnboardingBilling: ({ error }) =>
        failRepositoryAction(model, error),
      UpdatedOnboardingGoal: ({ value }) => [
        evo(model, {
          onboarding: onboarding =>
            OnboardingFlowModel({
              ...onboarding,
              action: IdleOnboardingAction(),
              goalValue: value,
            }),
        }),
        [],
        Option.none(),
      ],
      SubmittedOnboardingGoal: () => {
        const goal = model.onboarding.goalValue.trim()

        if (goal === '') {
          return failRepositoryAction(model, 'Enter a goal.')
        }

        return [
          evo(model, {
            onboarding: onboarding =>
              OnboardingFlowModel({
                ...onboarding,
                action: SubmittingOnboardingAction({ label: 'Saving goal' }),
              }),
          }),
          [SubmitOnboardingGoal({ goal })],
          Option.none(),
        ]
      },
      SucceededSubmitOnboardingGoal: ({ response }) => [
        applyOnboardingStatus(model, response),
        [],
        completionOutMessage(response),
      ],
      FailedSubmitOnboardingGoal: ({ error }) =>
        failRepositoryAction(model, error),
    }),
    M.orElse(() => [model, [], Option.none()]),
  )
