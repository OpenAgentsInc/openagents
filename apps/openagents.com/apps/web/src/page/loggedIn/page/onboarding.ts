import { Match as M } from 'effect'
import type { Html } from 'foldkit/html'
import { html } from 'foldkit/html'

import type { OnboardingGitHubRepository } from '../../../domain/session'
import { iconView } from '../../../icon'
import * as Ui from '../../../ui'
import {
  ClickedNextOnboardingRepositoryPage,
  ClickedOnboardingStep,
  ClickedPreviousOnboardingRepositoryPage,
  ClickedPreviousOnboardingStep,
  ClickedSkipOnboardingBilling,
  ClickedSkipOnboardingRepository,
  Message,
  RequestedLoadOnboardingRepositories,
  SelectedOnboardingRepository,
  SubmittedOnboardingGoal,
  SubmittedOnboardingRepository,
  UpdatedOnboardingGoal,
  UpdatedOnboardingManualRepositoryName,
  UpdatedOnboardingManualRepositoryOwner,
  UpdatedOnboardingRepositorySearch,
} from '../message'
import {
  type Model,
  ONBOARDING_REPOSITORY_PAGE_SIZE,
  type OnboardingRepositoryList,
  clampOnboardingRepositoryPageIndex,
  filteredOnboardingRepositories,
  onboardingRepositoryPageCount,
} from '../model'

const stepTone = (
  model: Model,
  step: 'repository' | 'goal' | 'billing',
): 'active' | 'complete' | 'pending' => {
  if (model.auth.onboarding.step === step) {
    return 'active'
  }

  if (
    (step === 'repository' &&
      model.auth.onboarding.repository._tag !== 'RepositoryUnselected') ||
    (step === 'goal' && model.auth.onboarding.goal !== null) ||
    (step === 'billing' &&
      model.auth.onboarding.billing._tag !== 'BillingPending')
  ) {
    return 'complete'
  }

  return 'pending'
}

const stepRow = (
  index: string,
  title: string,
  step: 'repository' | 'goal' | 'billing',
  tone: 'active' | 'complete' | 'pending',
): Html => {
  const h = html<Message>()

  return h.button(
    [
      h.Type('button'),
      h.OnClick(ClickedOnboardingStep({ step })),
      Ui.className<Message>(
        'grid w-full grid-cols-[2rem_minmax(0,1fr)_auto] items-center gap-3 border-b border-[#222] py-3 text-left text-xs last:border-b-0 hover:bg-white/[0.03] focus-visible:outline focus-visible:outline-1 focus-visible:outline-[#ffb400]',
      ),
    ],
    [
      h.span(
        [
          Ui.className<Message>(
            tone === 'pending' ? 'text-white/30' : 'text-[#ffb400]',
          ),
        ],
        [index],
      ),
      h.span(
        [
          Ui.className<Message>(
            tone === 'pending' ? 'text-white/45' : 'text-white/80',
          ),
        ],
        [title],
      ),
      tone === 'complete'
        ? iconView<Message>('Check', 'size-3 text-[#00c853]')
        : h.span(
            [
              Ui.className<Message>(
                tone === 'active'
                  ? 'size-2 bg-[#ffb400]'
                  : 'size-2 border border-white/20',
              ),
            ],
            [],
          ),
    ],
  )
}

const actionText = (model: Model): string | undefined =>
  M.value(model.onboarding.action).pipe(
    M.tagsExhaustive({
      OnboardingActionIdle: () => undefined,
      OnboardingActionSubmitting: ({ label }) => `${label}...`,
      OnboardingActionFailed: ({ error }) => error,
    }),
  )

const actionIsBusy = (model: Model): boolean =>
  model.onboarding.action._tag === 'OnboardingActionSubmitting'

const stepRail = (model: Model): Html => {
  const h = html<Message>()

  return h.aside(
    [Ui.className<Message>('border border-[#222] bg-black p-4')],
    [
      h.div([Ui.className<Message>(Ui.eyebrowClass)], ['Setup']),
      h.div(
        [Ui.className<Message>('mt-3 border-t border-[#222]')],
        [
          stepRow(
            '01',
            'Repository',
            'repository',
            stepTone(model, 'repository'),
          ),
          stepRow('02', 'Goal', 'goal', stepTone(model, 'goal')),
          stepRow('03', 'Confirm', 'billing', stepTone(model, 'billing')),
        ],
      ),
    ],
  )
}

const repositoryList = (
  repositories: OnboardingRepositoryList,
  model: Model,
): Html =>
  M.value(repositories).pipe(
    M.tagsExhaustive({
      OnboardingRepositoriesIdle: () => repositoryLoading(),
      OnboardingRepositoriesLoading: () => repositoryLoading(),
      OnboardingRepositoriesFailed: ({ error }) => repositoryFailed(error),
      OnboardingRepositoriesLoaded: ({ repositories, tokenStatus }) =>
        tokenStatus === 'missing'
          ? repositoryEmpty()
          : loadedRepositoryPicker(repositories, model),
    }),
  )

const repositoryLoading = (): Html => {
  const h = html<Message>()

  return h.div(
    [Ui.className<Message>('border border-[#222] p-4 text-sm text-white/45')],
    ['Loading...'],
  )
}

const repositoryFailed = (error: string): Html => {
  const h = html<Message>()

  return h.div(
    [Ui.className<Message>('grid gap-3 border border-[#333] p-4')],
    [
      h.p([Ui.className<Message>('m-0 text-sm text-[#d32f2f]')], [error]),
      Ui.button<Message>({
        label: 'Retry',
        size: 'sm',
        variant: 'secondary',
        attrs: [
          h.Type('button'),
          h.OnClick(RequestedLoadOnboardingRepositories()),
        ],
      }),
    ],
  )
}

const repositoryEmpty = (): Html => {
  const h = html<Message>()

  return h.div(
    [Ui.className<Message>('border border-[#222] p-4 text-sm text-white/45')],
    ['No repositories available.'],
  )
}

const manualRepositoryFields = (model: Model): Html => {
  const h = html<Message>()

  return h.div(
    [Ui.className<Message>('grid gap-3 border border-[#222] bg-black p-3')],
    [
      h.div([Ui.className<Message>(Ui.eyebrowClass)], ['Direct']),
      h.div(
        [Ui.className<Message>('grid gap-2 sm:grid-cols-2')],
        [
          h.label(
            [Ui.className<Message>('grid gap-1')],
            [
              h.span(
                [Ui.className<Message>('text-xs text-white/45')],
                ['Owner'],
              ),
              h.input([
                h.Type('text'),
                h.Name('repositoryOwner'),
                h.Value(model.onboarding.manualRepositoryOwner),
                h.Placeholder('OpenAgentsInc'),
                h.OnInput(value =>
                  UpdatedOnboardingManualRepositoryOwner({ value }),
                ),
                Ui.className<Message>(
                  `${Ui.inputClass} h-9 px-2 text-sm max-sm:text-base`,
                ),
              ]),
            ],
          ),
          h.label(
            [Ui.className<Message>('grid gap-1')],
            [
              h.span(
                [Ui.className<Message>('text-xs text-white/45')],
                ['Repository'],
              ),
              h.input([
                h.Type('text'),
                h.Name('repositoryName'),
                h.Value(model.onboarding.manualRepositoryName),
                h.Placeholder('openagents'),
                h.OnInput(value =>
                  UpdatedOnboardingManualRepositoryName({ value }),
                ),
                Ui.className<Message>(
                  `${Ui.inputClass} h-9 px-2 text-sm max-sm:text-base`,
                ),
              ]),
            ],
          ),
        ],
      ),
    ],
  )
}

const repositorySearchField = (model: Model): Html => {
  const h = html<Message>()

  return h.input([
    h.Type('search'),
    h.Name('repositorySearch'),
    h.AriaLabel('Search repositories'),
    h.Placeholder('Search repositories'),
    h.Value(model.onboarding.repositorySearch),
    h.OnInput(value => UpdatedOnboardingRepositorySearch({ value })),
    Ui.className<Message>(`${Ui.inputClass} h-9 px-2 text-sm max-sm:text-base`),
  ])
}

const repositoryPagination = (
  repositories: ReadonlyArray<OnboardingGitHubRepository>,
  model: Model,
): Html => {
  const h = html<Message>()
  const filtered = filteredOnboardingRepositories(
    repositories,
    model.onboarding.repositorySearch,
  )
  const pageCount = onboardingRepositoryPageCount(
    repositories,
    model.onboarding.repositorySearch,
  )
  const pageIndex = clampOnboardingRepositoryPageIndex(
    model.onboarding.repositoryPageIndex,
    repositories,
    model.onboarding.repositorySearch,
  )
  const start =
    filtered.length === 0 ? 0 : pageIndex * ONBOARDING_REPOSITORY_PAGE_SIZE + 1
  const end = Math.min(
    filtered.length,
    pageIndex * ONBOARDING_REPOSITORY_PAGE_SIZE +
      ONBOARDING_REPOSITORY_PAGE_SIZE,
  )

  return h.div(
    [
      Ui.className<Message>(
        'flex flex-wrap items-center justify-between gap-2 text-xs text-white/45',
      ),
    ],
    [
      h.span([], [`${start}-${end} of ${filtered.length}`]),
      h.div(
        [Ui.className<Message>('flex items-center gap-2')],
        [
          Ui.button<Message>({
            label: 'Previous',
            size: 'sm',
            variant: 'secondary',
            attrs: [
              h.Type('button'),
              ...(pageIndex === 0 ? [h.Disabled(true)] : []),
              h.OnClick(ClickedPreviousOnboardingRepositoryPage()),
            ],
          }),
          h.span([], [`${pageIndex + 1} / ${pageCount}`]),
          Ui.button<Message>({
            label: 'Next',
            size: 'sm',
            variant: 'secondary',
            attrs: [
              h.Type('button'),
              ...(pageIndex >= pageCount - 1 ? [h.Disabled(true)] : []),
              h.OnClick(ClickedNextOnboardingRepositoryPage()),
            ],
          }),
        ],
      ),
    ],
  )
}

const loadedRepositoryPicker = (
  repositories: ReadonlyArray<OnboardingGitHubRepository>,
  model: Model,
): Html => {
  const h = html<Message>()
  const filtered = filteredOnboardingRepositories(
    repositories,
    model.onboarding.repositorySearch,
  )
  const pageIndex = clampOnboardingRepositoryPageIndex(
    model.onboarding.repositoryPageIndex,
    repositories,
    model.onboarding.repositorySearch,
  )
  const pageStart = pageIndex * ONBOARDING_REPOSITORY_PAGE_SIZE
  const pageRepositories = filtered.slice(
    pageStart,
    pageStart + ONBOARDING_REPOSITORY_PAGE_SIZE,
  )

  return h.div(
    [Ui.className<Message>('grid gap-3')],
    [
      manualRepositoryFields(model),
      repositorySearchField(model),
      pageRepositories.length === 0
        ? h.div(
            [
              Ui.className<Message>(
                'border border-[#222] p-4 text-sm text-white/45',
              ),
            ],
            ['No matching repositories.'],
          )
        : repositoryOptions(pageRepositories, model),
      repositoryPagination(repositories, model),
    ],
  )
}

const repositoryOptions = (
  repositories: ReadonlyArray<OnboardingGitHubRepository>,
  model: Model,
): Html => {
  const h = html<Message>()

  return h.div(
    [Ui.className<Message>('grid gap-px border border-[#222] bg-[#222]')],
    repositories.map(repository => {
      const selected = model.onboarding.selectedRepositoryId === repository.id

      return h.button(
        [
          h.Type('button'),
          h.Attribute('aria-pressed', selected ? 'true' : 'false'),
          h.OnClick(
            SelectedOnboardingRepository({ repositoryId: repository.id }),
          ),
          Ui.className<Message>(
            `grid min-w-0 grid-cols-[auto_minmax(0,1fr)_auto] items-center gap-3 bg-black p-3 text-left hover:bg-[#080808] ${selected ? 'outline outline-1 outline-[#ffb400]' : ''}`,
          ),
        ],
        [
          iconView<Message>('Folder', 'size-4 text-white/45'),
          h.span(
            [Ui.className<Message>('min-w-0')],
            [
              h.span(
                [
                  Ui.className<Message>(
                    'block overflow-hidden text-ellipsis whitespace-nowrap text-sm text-white/80',
                  ),
                ],
                [repository.fullName],
              ),
              h.span(
                [
                  Ui.className<Message>(
                    'mt-1 block overflow-hidden text-ellipsis whitespace-nowrap text-xs text-white/35',
                  ),
                ],
                [
                  `${repository.private ? 'private' : 'public'} / ${repository.defaultBranch}`,
                ],
              ),
            ],
          ),
          selected
            ? iconView<Message>('Check', 'size-3 text-[#00c853]')
            : h.span([Ui.className<Message>('size-3')], []),
        ],
      )
    }),
  )
}

const repositoryScreen = (model: Model): Html => {
  const h = html<Message>()
  const busy = actionIsBusy(model)
  const message = actionText(model)

  return h.form(
    [
      h.OnSubmit(SubmittedOnboardingRepository()),
      Ui.className<Message>('grid gap-5'),
    ],
    [
      sectionHeader('Repository', 'Choose the repo'),
      repositoryList(model.onboarding.repositories, model),
      h.div(
        [Ui.className<Message>('flex flex-wrap items-center gap-2')],
        [
          Ui.button<Message>({
            label: busy ? 'Saving...' : 'Continue',
            size: 'sm',
            variant: 'primary',
            attrs: [h.Type('submit'), ...(busy ? [h.Disabled(true)] : [])],
          }),
          Ui.button<Message>({
            label: 'Skip',
            size: 'sm',
            variant: 'secondary',
            attrs: [
              h.Type('button'),
              ...(busy ? [h.Disabled(true)] : []),
              h.OnClick(ClickedSkipOnboardingRepository()),
            ],
          }),
        ],
      ),
      message === undefined
        ? h.empty
        : h.p(
            [
              Ui.className<Message>(
                `m-0 text-sm ${model.onboarding.action._tag === 'OnboardingActionFailed' ? 'text-[#d32f2f]' : 'text-white/45'}`,
              ),
            ],
            [message],
          ),
    ],
  )
}

const billingScreen = (model: Model): Html => {
  const h = html<Message>()
  const busy = actionIsBusy(model)
  const message = actionText(model)

  return h.section(
    [Ui.className<Message>('grid gap-5')],
    [
      sectionHeader('Confirm', 'Submit for $0'),
      h.div(
        [Ui.className<Message>('grid gap-px border border-[#222] bg-[#222]')],
        [
          publicWorkRow(
            'Public by default',
            'This request, the generated work, and the resulting learning data may be public.',
          ),
          publicWorkRow(
            'Compute included',
            'OpenAgents pays for the agent compute during this beta intake.',
          ),
          publicWorkRow(
            "That's it",
            "We'll email you within 24 hours with your completed work.",
          ),
        ],
      ),
      h.div(
        [Ui.className<Message>('flex flex-wrap items-center gap-2')],
        [
          Ui.button<Message>({
            label: 'Back',
            size: 'sm',
            variant: 'secondary',
            attrs: [
              h.Type('button'),
              ...(busy ? [h.Disabled(true)] : []),
              h.OnClick(ClickedPreviousOnboardingStep()),
            ],
          }),
          Ui.button<Message>({
            label: busy ? 'Submitting...' : 'Submit public order',
            size: 'sm',
            variant: 'primary',
            attrs: [
              h.Type('button'),
              ...(busy ? [h.Disabled(true)] : []),
              h.OnClick(ClickedSkipOnboardingBilling()),
            ],
          }),
        ],
      ),
      message === undefined
        ? h.empty
        : h.p(
            [
              Ui.className<Message>(
                `m-0 text-sm ${model.onboarding.action._tag === 'OnboardingActionFailed' ? 'text-[#d32f2f]' : 'text-white/45'}`,
              ),
            ],
            [message],
          ),
    ],
  )
}

const publicWorkRow = (title: string, body: string): Html => {
  const h = html<Message>()

  return h.div(
    [Ui.className<Message>('grid gap-1 bg-black p-4 text-base/7 sm:text-sm/6')],
    [
      h.div([Ui.className<Message>('text-white/85')], [title]),
      h.p([Ui.className<Message>('m-0 text-white/45')], [body]),
    ],
  )
}

const goalScreen = (model: Model): Html => {
  const h = html<Message>()
  const busy = actionIsBusy(model)
  const message = actionText(model)

  return h.form(
    [
      h.OnSubmit(SubmittedOnboardingGoal()),
      Ui.className<Message>('grid gap-5'),
    ],
    [
      sectionHeader('Request', 'What should the agent build?'),
      h.label(
        [Ui.className<Message>('grid gap-2')],
        [
          h.span([Ui.className<Message>(Ui.eyebrowClass)], ['Build request']),
          h.textarea(
            [
              h.Name('goal'),
              h.Rows(5),
              h.Placeholder('Add Stripe credits and show the new status page'),
              h.OnInput(value => UpdatedOnboardingGoal({ value })),
              Ui.className<Message>(
                `${Ui.inputClass} min-h-32 resize-y leading-6 max-sm:text-base`,
              ),
            ],
            [model.onboarding.goalValue],
          ),
        ],
      ),
      h.div(
        [Ui.className<Message>('flex flex-wrap items-center gap-2')],
        [
          Ui.button<Message>({
            label: 'Back',
            size: 'sm',
            variant: 'secondary',
            attrs: [
              h.Type('button'),
              ...(busy ? [h.Disabled(true)] : []),
              h.OnClick(ClickedPreviousOnboardingStep()),
            ],
          }),
          Ui.button<Message>({
            label: busy ? 'Saving...' : 'Continue',
            size: 'sm',
            variant: 'primary',
            attrs: [h.Type('submit'), ...(busy ? [h.Disabled(true)] : [])],
          }),
        ],
      ),
      message === undefined
        ? h.empty
        : h.p(
            [
              Ui.className<Message>(
                `m-0 text-sm ${model.onboarding.action._tag === 'OnboardingActionFailed' ? 'text-[#d32f2f]' : 'text-white/45'}`,
              ),
            ],
            [message],
          ),
    ],
  )
}

const sectionHeader = (eyebrow: string, title: string): Html => {
  const h = html<Message>()

  return h.div(
    [Ui.className<Message>('grid gap-1')],
    [
      h.div([Ui.className<Message>(Ui.eyebrowClass)], [eyebrow]),
      h.h1(
        [Ui.className<Message>('m-0 text-2xl font-semibold text-white/90')],
        [title],
      ),
    ],
  )
}

const currentScreen = (model: Model): Html =>
  M.value(model.auth.onboarding.step).pipe(
    M.when('repository', () => repositoryScreen(model)),
    M.when('billing', () => billingScreen(model)),
    M.when('goal', () => goalScreen(model)),
    M.when('complete', () => goalScreen(model)),
    M.exhaustive,
  )

export const view = (model: Model): Html => {
  const h = html<Message>()

  return h.section(
    [
      Ui.className<Message>(
        'grid min-h-[calc(100dvh-3rem)] gap-4 p-4 lg:grid-cols-[18rem_minmax(0,44rem)] lg:items-start lg:justify-center lg:p-8',
      ),
    ],
    [
      stepRail(model),
      h.div(
        [Ui.className<Message>('border border-[#222] bg-black p-5')],
        [currentScreen(model)],
      ),
    ],
  )
}
