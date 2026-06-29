import { Match as M } from 'effect'
import { html } from 'foldkit/html'
import type { Html } from 'foldkit/html'

import type { OnboardingGitHubRepository } from '../../../domain/session'
import { iconView } from '../../../icon'
import { settingsSectionRouter } from '../../../route'
import { formatIsoDateTime } from '../../../time-format'
import * as Ui from '../../../ui'
import {
  ClickedLogout,
  ClickedNextOnboardingRepositoryPage,
  ClickedPollProviderDeviceLogin,
  ClickedPreviousOnboardingRepositoryPage,
  ClickedResetProviderAccountPoolAccount,
  ClickedStartProviderDeviceLogin,
  type Message,
  RequestedLoadOnboardingRepositories,
  RequestedLoadProviderAccountPool,
  SelectedOnboardingRepository,
  SubmittedOnboardingRepository,
  UpdatedOnboardingManualRepositoryName,
  UpdatedOnboardingManualRepositoryOwner,
  UpdatedOnboardingRepositorySearch,
} from '../message'
import type {
  Model,
  ProviderAccountPoolAccount,
  ProviderAccountPoolLease,
  ProviderAccountPoolResponse,
} from '../model'
import {
  ONBOARDING_REPOSITORY_PAGE_SIZE,
  clampOnboardingRepositoryPageIndex,
  filteredOnboardingRepositories,
  onboardingRepositoryPageCount,
  providerAccountBundleFromAuth,
} from '../model'

export type SettingsSectionKey = Ui.SettingsWorkspaceSectionKey

export const settingsSectionHref = (section: SettingsSectionKey): string =>
  section === 'general' ? '/settings' : settingsSectionRouter({ section })

export const normalizeSection = (section: string): SettingsSectionKey => {
  if (
    section === 'connections' ||
    section === 'organization' ||
    section === 'members'
  ) {
    return section
  }

  return 'general'
}

const fallback = (value: string | null | undefined, empty: string): string =>
  value === null || value === undefined || value.trim() === ''
    ? empty
    : value.trim()

const providerConnectionAction = (
  model: Model,
): Ui.SettingsProviderConnectionAction => {
  if (model.providerConnectionAction._tag === 'ProviderConnectionStarting') {
    return { kind: 'starting' }
  }

  if (model.providerConnectionAction._tag === 'ProviderConnectionPolling') {
    return {
      attemptId: model.providerConnectionAction.attemptId,
      kind: 'polling',
    }
  }

  if (model.providerConnectionAction._tag === 'ProviderConnectionSucceeded') {
    return {
      kind: 'succeeded',
      message: model.providerConnectionAction.message,
    }
  }

  if (model.providerConnectionAction._tag === 'ProviderConnectionFailed') {
    return {
      error: model.providerConnectionAction.error,
      kind: 'failed',
    }
  }

  return { kind: 'idle' }
}

const currentRepositoryDetail = (model: Model): string =>
  model.auth.onboarding.repository._tag === 'RepositorySelected'
    ? `${model.auth.onboarding.repository.repository.fullName} @ ${model.auth.onboarding.repository.repository.defaultBranch}`
    : 'not selected'

const settingsRepositoryActionText = (model: Model): string | undefined => {
  if (model.onboarding.action._tag === 'OnboardingActionSubmitting') {
    return `${model.onboarding.action.label}...`
  }

  if (model.onboarding.action._tag === 'OnboardingActionFailed') {
    return model.onboarding.action.error
  }

  return undefined
}

const settingsRepositoryBusy = (model: Model): boolean =>
  model.onboarding.action._tag === 'OnboardingActionSubmitting'

const savedRepository = (
  model: Model,
): OnboardingGitHubRepository | undefined =>
  model.auth.onboarding.repository._tag === 'RepositorySelected'
    ? model.auth.onboarding.repository.repository
    : undefined

const settingsRepositoryHasDraft = (model: Model): boolean => {
  const saved = savedRepository(model)
  const owner = model.onboarding.manualRepositoryOwner.trim()
  const name = model.onboarding.manualRepositoryName.trim()
  const hasManualDraft = owner !== '' || name !== ''

  if (hasManualDraft) {
    return saved === undefined || owner !== saved.owner || name !== saved.name
  }

  const repositoryId = model.onboarding.selectedRepositoryId.trim()

  if (repositoryId === '') {
    return false
  }

  return saved === undefined || repositoryId !== saved.id
}

const settingsRepositoryManualFields = (model: Model): Html => {
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

const settingsRepositoryOptions = (
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

const settingsRepositoryPagination = (
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

const settingsRepositoryLoaded = (
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
      h.input([
        h.Type('search'),
        h.Name('repositorySearch'),
        h.AriaLabel('Search repositories'),
        h.Placeholder('Search repositories'),
        h.Value(model.onboarding.repositorySearch),
        h.OnInput(value => UpdatedOnboardingRepositorySearch({ value })),
        Ui.className<Message>(
          `${Ui.inputClass} h-9 px-2 text-sm max-sm:text-base`,
        ),
      ]),
      pageRepositories.length === 0
        ? h.div(
            [
              Ui.className<Message>(
                'border border-[#222] p-4 text-sm text-white/45',
              ),
            ],
            ['No matching repositories.'],
          )
        : settingsRepositoryOptions(pageRepositories, model),
      settingsRepositoryPagination(repositories, model),
    ],
  )
}

const settingsRepositoryPicker = (model: Model): Html => {
  const h = html<Message>()

  if (model.onboarding.repositories._tag === 'OnboardingRepositoriesIdle') {
    return h.div(
      [Ui.className<Message>('border border-[#222] p-3')],
      [
        Ui.button<Message>({
          label: 'Load repositories',
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

  if (model.onboarding.repositories._tag === 'OnboardingRepositoriesLoading') {
    return h.div(
      [Ui.className<Message>('border border-[#222] p-4 text-sm text-white/45')],
      ['Loading...'],
    )
  }

  if (model.onboarding.repositories._tag === 'OnboardingRepositoriesFailed') {
    return h.div(
      [Ui.className<Message>('grid gap-3 border border-[#333] p-4')],
      [
        h.p(
          [Ui.className<Message>('m-0 text-sm text-[#d32f2f]')],
          [model.onboarding.repositories.error],
        ),
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

  if (model.onboarding.repositories.tokenStatus === 'missing') {
    return h.div(
      [Ui.className<Message>('border border-[#222] p-4 text-sm text-white/45')],
      ['GitHub repository access is unavailable.'],
    )
  }

  return settingsRepositoryLoaded(
    model.onboarding.repositories.repositories,
    model,
  )
}

const settingsGitHubRepositoryPanel = (model: Model): Html => {
  const h = html<Message>()
  const busy = settingsRepositoryBusy(model)
  const message = settingsRepositoryActionText(model)
  const saved = savedRepository(model)
  const hasDraft = settingsRepositoryHasDraft(model)
  const canSave = hasDraft && !busy

  return h.form(
    [
      h.OnSubmit(SubmittedOnboardingRepository()),
      Ui.className<Message>('mt-4 grid gap-3'),
    ],
    [
      h.div(
        [Ui.className<Message>('grid gap-1 border border-[#222] p-3 text-sm')],
        [
          h.span(
            [Ui.className<Message>('text-white/35')],
            ['Default repository'],
          ),
          h.span(
            [
              Ui.className<Message>(
                'overflow-hidden text-ellipsis whitespace-nowrap text-white/80',
              ),
            ],
            [currentRepositoryDetail(model)],
          ),
        ],
      ),
      settingsRepositoryManualFields(model),
      settingsRepositoryPicker(model),
      h.div(
        [Ui.className<Message>('flex flex-wrap items-center gap-2')],
        [
          Ui.button<Message>({
            label: busy
              ? 'Saving...'
              : hasDraft
                ? 'Save repository'
                : saved === undefined
                  ? 'Save repository'
                  : 'Repository saved',
            size: 'sm',
            variant: 'primary',
            attrs: [h.Type('submit'), ...(canSave ? [] : [h.Disabled(true)])],
          }),
        ],
      ),
      message === undefined
        ? h.p(
            [Ui.className<Message>('m-0 text-sm text-white/45')],
            [
              hasDraft
                ? 'Save to update the default repository.'
                : saved === undefined
                  ? 'Select or enter a repository.'
                  : 'Default repository saved.',
            ],
          )
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

export const formatRateLimitCountdown = (
  remainingSeconds: number | null,
): string => {
  if (remainingSeconds === null) {
    return 'reset pending'
  }

  const bounded = Math.max(0, Math.floor(remainingSeconds))
  const hours = Math.floor(bounded / 3600)
  const minutes = Math.floor((bounded % 3600) / 60)
  const seconds = bounded % 60
  const parts =
    hours > 0
      ? [hours, minutes, seconds]
      : [minutes, seconds]

  return parts.map(part => String(part).padStart(2, '0')).join(':')
}

export const rateLimitCountdownTitle = (
  cooldownUntil: string | null,
): string =>
  cooldownUntil === null
    ? 'Rate limit reset time unavailable'
    : `Rate limit resets at ${formatIsoDateTime(cooldownUntil)}`

const poolValueContentRow = (label: string, content: Html): Html => {
  const h = html<Message>()

  return h.div(
    [
      Ui.className<Message>(
        'grid grid-cols-[minmax(8rem,0.42fr)_minmax(0,1fr)] gap-4 border-b border-[#222] py-2 text-sm last:border-b-0',
      ),
    ],
    [
      h.span([Ui.className<Message>('min-w-0 text-white/35')], [label]),
      content,
    ],
  )
}

const poolValueRow = (label: string, value: string): Html => {
  const h = html<Message>()

  return poolValueContentRow(
    label,
    h.span(
      [
        Ui.className<Message>(
          'min-w-0 overflow-hidden text-ellipsis whitespace-nowrap text-white/80',
        ),
      ],
      [value],
    ),
  )
}

const poolCooldownLabel = (account: ProviderAccountPoolAccount): string =>
  account.cooldownUntil === null
    ? 'none'
    : account.cooldownRemainingSeconds === null
      ? `ended ${formatIsoDateTime(account.cooldownUntil)}`
      : `until ${formatIsoDateTime(account.cooldownUntil)} (~${Math.max(1, Math.ceil(account.cooldownRemainingSeconds / 60))}m)`

export const rateLimitCountdownView = (
  account: Pick<
    ProviderAccountPoolAccount,
    'cooldownRemainingSeconds' | 'cooldownUntil'
  >,
): Html => {
  const h = html<Message>()
  const elapsed =
    account.cooldownRemainingSeconds !== null &&
    account.cooldownRemainingSeconds <= 0

  return h.time(
    [
      h.Attribute('data-rate-limit-countdown', 'true'),
      ...(account.cooldownUntil === null
        ? []
        : [h.Attribute('datetime', account.cooldownUntil)]),
      h.Attribute('aria-label', rateLimitCountdownTitle(account.cooldownUntil)),
      Ui.className<Message>(
        `inline-flex h-6 min-w-[4.75rem] items-center justify-center border px-2 font-mono text-xs tabular-nums ${elapsed ? 'border-[#333] text-white/45' : 'border-[#ff6f00]/60 text-[#ffb400]'}`,
      ),
    ],
    [formatRateLimitCountdown(account.cooldownRemainingSeconds)],
  )
}

const poolAccountTone = (account: ProviderAccountPoolAccount): string =>
  account.reconnect.needed
    ? 'text-[#d32f2f]'
    : account.eligibility === 'eligible'
      ? 'text-[#00c853]'
      : 'text-[#ff6f00]'

const poolAccountHeadline = (account: ProviderAccountPoolAccount): string =>
  account.reconnect.needed
    ? 'reconnect needed'
    : account.eligibility === 'eligible'
      ? 'ready'
      : (account.eligibilityReasons[0] ?? 'unavailable')

const poolAccountCanReset = (account: ProviderAccountPoolAccount): boolean =>
  account.cooldownUntil !== null || account.recentFailureClass === 'rate_limited'

const poolAccountView = (account: ProviderAccountPoolAccount): Html => {
  const h = html<Message>()

  return h.div(
    [Ui.className<Message>('grid gap-2 bg-[#010102] p-3')],
    [
      h.div(
        [
          Ui.className<Message>(
            'grid gap-3 lg:grid-cols-[minmax(0,1fr)_auto] lg:items-center',
          ),
        ],
        [
          h.div(
            [Ui.className<Message>('min-w-0')],
            [
              h.p(
                [
                  Ui.className<Message>(
                    'm-0 overflow-hidden text-ellipsis whitespace-nowrap text-sm text-[#f1efe8]',
                  ),
                ],
                [account.accountLabel ?? account.providerAccountRef],
              ),
              h.p(
                [
                  Ui.className<Message>(
                    `m-0 mt-1 text-xs ${poolAccountTone(account)}`,
                  ),
                ],
                [poolAccountHeadline(account)],
              ),
              h.p(
                [Ui.className<Message>('m-0 mt-1 text-xs text-white/35')],
                [account.provider.replaceAll('_', ' ')],
              ),
            ],
          ),
          account.reconnect.needed && account.provider === 'chatgpt_codex'
            ? Ui.button<Message>({
                label: 'Reconnect',
                size: 'sm',
                variant: 'primary',
                attrs: [
                  h.OnClick(
                    ClickedStartProviderDeviceLogin({
                      providerAccountRef: account.providerAccountRef,
                    }),
                  ),
                ],
              })
            : account.reconnect.needed
              ? h.span(
                  [Ui.className<Message>('text-xs text-[#d32f2f]')],
                  ['Reconnect required'],
                )
              : poolAccountCanReset(account)
                ? Ui.button<Message>({
                    label: 'Reset',
                    size: 'sm',
                    variant: 'secondary',
                    attrs: [
                      h.OnClick(
                        ClickedResetProviderAccountPoolAccount({
                          providerAccountRef: account.providerAccountRef,
                        }),
                      ),
                    ],
                  })
              : h.span(
                  [Ui.className<Message>('text-xs text-white/35')],
                  [`priority ${account.operatorPriority}`],
                ),
        ],
      ),
      h.div(
        [Ui.className<Message>('grid border-t border-[#222]')],
        [
          poolValueRow(
            'Leases',
            `${account.activeLeaseCount}/${account.leaseLimit} active`,
          ),
          poolValueRow('Status', `${account.status} / ${account.health}`),
          poolValueRow('Cooldown', poolCooldownLabel(account)),
          ...(account.cooldownUntil === null
            ? []
            : [
                poolValueContentRow(
                  'Rate limit',
                  h.span(
                    [
                      Ui.className<Message>(
                        'flex min-w-0 flex-wrap items-center gap-2 text-white/80',
                      ),
                    ],
                    [
                      rateLimitCountdownView(account),
                      h.span(
                        [
                          Ui.className<Message>(
                            'min-w-0 overflow-hidden text-ellipsis whitespace-nowrap text-xs text-white/35',
                          ),
                        ],
                        ['until reset'],
                      ),
                    ],
                  ),
                ),
              ]),
          ...(account.lowCredit ? [poolValueRow('Credits', 'low')] : []),
          ...(account.recentFailureClass === null
            ? []
            : [poolValueRow('Recent failure', account.recentFailureClass)]),
          poolValueRow(
            'Last selected',
            account.lastSelectedAt === null
              ? 'never'
              : formatIsoDateTime(account.lastSelectedAt),
          ),
          ...(account.lastSanityCheckAt === null
            ? []
            : [
                poolValueRow(
                  'Last check',
                  `${account.lastSanityCheckResult ?? 'unknown'} at ${formatIsoDateTime(account.lastSanityCheckAt)}`,
                ),
              ]),
          ...(account.eligibility === 'ineligible'
            ? [
                poolValueRow(
                  'Unavailable',
                  account.eligibilityReasons.join(', '),
                ),
              ]
            : []),
        ],
      ),
    ],
  )
}

const poolLeaseView = (lease: ProviderAccountPoolLease): Html => {
  const h = html<Message>()

  return h.div(
    [Ui.className<Message>('grid gap-1 bg-[#010102] p-3 text-sm')],
    [
      h.p(
        [
          Ui.className<Message>(
            'm-0 overflow-hidden text-ellipsis whitespace-nowrap text-white/80',
          ),
        ],
        [
          `${lease.accountLabel ?? lease.providerAccountRef} - ${lease.requestedAction}`,
        ],
      ),
      h.p(
        [
          Ui.className<Message>(
            'm-0 overflow-hidden text-ellipsis whitespace-nowrap text-xs text-white/40',
          ),
        ],
        [
          `${lease.runId ?? lease.orderId ?? lease.leaseRef} - expires ${formatIsoDateTime(lease.expiresAt)}`,
        ],
      ),
    ],
  )
}

const poolLoadedView = (pool: ProviderAccountPoolResponse): Html => {
  const h = html<Message>()

  return h.div(
    [Ui.className<Message>('grid gap-4')],
    [
      h.div(
        [
          Ui.className<Message>(
            'flex flex-wrap items-center justify-between gap-2',
          ),
        ],
        [
          h.p(
            [Ui.className<Message>('m-0 text-sm text-white/50')],
            [
              `${pool.summary.eligible}/${pool.summary.total} ready - ${pool.summary.activeLeaseCount} active lease(s) - generated ${formatIsoDateTime(pool.generatedAt)}`,
            ],
          ),
          Ui.button<Message>({
            label: 'Refresh',
            size: 'sm',
            variant: 'secondary',
            attrs: [h.OnClick(RequestedLoadProviderAccountPool())],
          }),
        ],
      ),
      pool.accounts.length === 0
        ? h.div(
            [
              Ui.className<Message>(
                'border border-[#222] p-4 text-sm text-white/45',
              ),
            ],
            ['No provider account connected.'],
          )
        : h.div(
            [
              Ui.className<Message>(
                'grid gap-px border border-[#222] bg-[#222]',
              ),
            ],
            pool.accounts.map(poolAccountView),
          ),
      h.div(
        [Ui.className<Message>('grid gap-2')],
        [
          h.p([Ui.className<Message>(Ui.eyebrowClass)], ['Active leases']),
          pool.activeLeases.length === 0
            ? h.p(
                [Ui.className<Message>('m-0 text-sm text-white/45')],
                ['No active leases.'],
              )
            : h.div(
                [
                  Ui.className<Message>(
                    'grid gap-px border border-[#222] bg-[#222]',
                  ),
                ],
                pool.activeLeases.map(poolLeaseView),
              ),
        ],
      ),
      h.div(
        [Ui.className<Message>('grid gap-2')],
        [
          h.p([Ui.className<Message>(Ui.eyebrowClass)], ['Next account']),
          h.p(
            [Ui.className<Message>('m-0 text-sm text-white/60')],
            [
              pool.nextSelection.status === 'selected'
                ? `${pool.nextSelection.accountLabel ?? pool.nextSelection.providerAccountRef ?? 'unknown'}${pool.nextSelection.provider === null ? '' : ` - ${pool.nextSelection.provider.replaceAll('_', ' ')}`} (${pool.nextSelection.activeLeaseCount ?? 0}/${pool.nextSelection.leaseLimit ?? 1} leases)`
                : 'No account is currently available for new work.',
            ],
          ),
        ],
      ),
    ],
  )
}

export const accountPoolPanel = (model: Model): Html => {
  const h = html<Message>()

  return M.value(model.providerAccountPool).pipe(
    M.tagsExhaustive({
      ProviderAccountPoolIdle: () =>
        h.div(
          [
            Ui.className<Message>(
              'border border-[#222] p-4 text-sm text-white/45',
            ),
          ],
          ['Loading account pool...'],
        ),
      ProviderAccountPoolLoading: () =>
        h.div(
          [
            Ui.className<Message>(
              'border border-[#222] p-4 text-sm text-white/45',
            ),
          ],
          ['Loading account pool...'],
        ),
      ProviderAccountPoolFailed: ({ error }) =>
        h.div(
          [Ui.className<Message>('grid gap-3 border border-[#333] p-4')],
          [
            h.p([Ui.className<Message>('m-0 text-sm text-[#d32f2f]')], [error]),
            Ui.button<Message>({
              label: 'Retry',
              size: 'sm',
              variant: 'secondary',
              attrs: [h.OnClick(RequestedLoadProviderAccountPool())],
            }),
          ],
        ),
      ProviderAccountPoolLoaded: ({ response }) => poolLoadedView(response),
    }),
  )
}

export const view = (model: Model, section: SettingsSectionKey): Html => {
  const h = html<Message>()
  const providerAccounts = providerAccountBundleFromAuth(model.auth)
  const pendingAttempt = providerAccounts.attempts.find(
    attempt => attempt.status === 'pending',
  )

  return Ui.settingsWorkspacePage<Message>({
    section,
    userName: model.session.name,
    userEmail: model.session.email,
    userId: model.session.userId,
    ...(model.session.avatarUrl === undefined ||
    model.session.avatarUrl === null ||
    model.session.avatarUrl === ''
      ? {}
      : { userAvatarUrl: model.session.avatarUrl }),
    ...(model.session.login === undefined
      ? {}
      : { githubLogin: model.session.login }),
    ...(model.session.githubId === undefined
      ? {}
      : { githubId: model.session.githubId }),
    teams: model.auth.teams.map(team => ({
      id: team.id,
      memberCount: team.members.length,
      name: team.name,
      role: team.role,
      slug: fallback(team.slug, team.id),
    })),
    members: model.auth.teams.flatMap(team =>
      team.members.map(member => ({
        id: member.userId,
        name: member.name,
        detail: `${fallback(member.githubUsername, fallback(member.email, member.userId))} - ${team.name}`,
        role: member.role,
        ...(member.avatarUrl === null || member.avatarUrl === ''
          ? {}
          : { avatarUrl: member.avatarUrl }),
      })),
    ),
    providerAccounts: providerAccounts.accounts,
    providerAttempts: providerAccounts.attempts,
    providerConnectionAction: providerConnectionAction(model),
    currentRepositoryDetail: currentRepositoryDetail(model),
    githubRepositoryPanel: settingsGitHubRepositoryPanel(model),
    accountPoolPanel: accountPoolPanel(model),
    startProviderLoginAttrs: [
      h.OnClick(ClickedStartProviderDeviceLogin({ createNew: true })),
    ],
    reconnectProviderLoginAttrs: providerAccounts.accounts.map(account => ({
      attrs: [
        h.OnClick(
          ClickedStartProviderDeviceLogin({
            providerAccountRef: account.providerAccountRef,
          }),
        ),
      ],
      providerAccountRef: account.providerAccountRef,
    })),
    pollProviderLoginAttrs:
      pendingAttempt === undefined
        ? []
        : [
            h.OnClick(
              ClickedPollProviderDeviceLogin({ attemptId: pendingAttempt.id }),
            ),
          ],
    signOutAttrs: [h.OnClick(ClickedLogout())],
  })
}
