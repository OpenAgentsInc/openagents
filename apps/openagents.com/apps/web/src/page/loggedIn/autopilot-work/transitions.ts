import { Effect, Match as M, Option, Schema as S } from 'effect'
import { Command } from 'foldkit'
import { evo } from 'foldkit/struct'

import { errorMessageFromUnknown, requestJson } from '../commands/api'
import { automationDraftForId } from '../forge-automations'
import {
  FailedAutopilotWorkComposer,
  FailedAutopilotWorkReview,
  FailedLoadAutopilotMorningReport,
  FailedLoadAutopilotWorkBriefing,
  FailedLoadAutopilotWorkDetail,
  FailedLoadAutopilotWorkEvents,
  FailedLoadAutopilotWorkList,
  FailedLoadCustomerOneCohort,
  Message,
  SucceededAutopilotWorkComposer,
  SucceededAutopilotWorkReview,
  SucceededLoadAutopilotMorningReport,
  SucceededLoadAutopilotWorkBriefing,
  SucceededLoadAutopilotWorkDetail,
  SucceededLoadAutopilotWorkEvents,
  SucceededLoadAutopilotWorkList,
  SucceededLoadCustomerOneCohort,
} from '../message'
import {
  AUTOPILOT_WORK_LIST_PROMISE_ID,
  AutopilotMorningReportFailed,
  AutopilotMorningReportLoaded,
  AutopilotMorningReportLoading,
  AutopilotMorningReportResponse,
  AutopilotWorkBriefingFailed,
  AutopilotWorkBriefingLoaded,
  AutopilotWorkBriefingLoading,
  AutopilotWorkBriefingResponse,
  AutopilotWorkComposerDraft,
  AutopilotWorkComposerFailed,
  AutopilotWorkComposerIdle,
  AutopilotWorkComposerSubmitting,
  AutopilotWorkComposerSucceeded,
  AutopilotWorkDetailFailed,
  AutopilotWorkDetailLoaded,
  AutopilotWorkDetailLoading,
  AutopilotWorkEventsFailed,
  AutopilotWorkEventsLoaded,
  AutopilotWorkEventsLoading,
  AutopilotWorkEventsResponse,
  AutopilotWorkListFailed,
  AutopilotWorkListLoaded,
  AutopilotWorkListLoading,
  AutopilotWorkListResponse,
  AutopilotWorkResponse,
  AutopilotWorkReviewAction,
  AutopilotWorkReviewFailed,
  AutopilotWorkReviewIdle,
  AutopilotWorkReviewSubmitting,
  AutopilotWorkReviewSucceeded,
  CustomerOneCohortFailed,
  CustomerOneCohortLoaded,
  CustomerOneCohortLoading,
  CustomerOneCohortProjection,
  Model,
} from '../model'
import { type UpdateReturn } from '../transition'

const withUpdateReturn = M.withReturnType<UpdateReturn>()

const autopilotWorkListPath = (): string =>
  `/api/autopilot/work?promiseId=${encodeURIComponent(AUTOPILOT_WORK_LIST_PROMISE_ID)}`

const customerOneCohortPath = (): string => '/api/public/customer-one-cohort'

const autopilotWorkPath = (workOrderRef: string): string =>
  `/api/autopilot/work/${encodeURIComponent(workOrderRef)}`

const autopilotWorkEventsPath = (workOrderRef: string): string =>
  `${autopilotWorkPath(workOrderRef)}/events`

const autopilotWorkBriefingPath = (workOrderRef: string): string =>
  `${autopilotWorkPath(workOrderRef)}/briefing`

const autopilotMorningReportPath = (): string => '/api/autopilot/morning-report'

const reviewRefs = (
  action: AutopilotWorkReviewAction,
  workOrderRef: string,
) => {
  const ref = `review.browser.${action}.${workOrderRef}`

  return action === 'accept'
    ? { decisionRefs: [ref] }
    : action === 'reject'
      ? { rejectionRefs: [ref] }
      : { revisionRequestRefs: [ref] }
}

const cleanRefSegment = (value: string): string =>
  value
    .trim()
    .replace(/[^A-Za-z0-9_-]+/g, '_')
    .slice(0, 80) || 'request'

const argvFromCommand = (value: string): ReadonlyArray<string> =>
  value.trim().split(/\s+/).filter(Boolean)

const spendCentsFromDraft = (draft: AutopilotWorkComposerDraft): number => {
  const parsed = Number.parseInt(draft.maxSpendCents.trim(), 10)

  return Number.isFinite(parsed) ? Math.max(0, Math.min(parsed, 100_000)) : 0
}

const workRequestFromDraft = (draft: AutopilotWorkComposerDraft) => {
  const objective = draft.objective.trim()
  const repositoryFullName = draft.repositoryFullName.trim()
  const branch = draft.branch.trim() || 'main'
  const args = argvFromCommand(draft.verificationCommand)
  const maxSpendCents = spendCentsFromDraft(draft)
  const taskRef = `task.autopilot_coder.browser.${cleanRefSegment(repositoryFullName)}.${cleanRefSegment(objective)}`
  const repository =
    repositoryFullName === ''
      ? undefined
      : {
          branch,
          fullName: repositoryFullName,
          provider: 'github' as const,
          visibility: 'public' as const,
        }

  return {
    caller: { kind: 'browser_session' as const, ownerRef: 'owner_ref.browser' },
    clientRequestRef: `client.browser.${taskRef}`,
    intent: 'delegate_to_autopilot' as const,
    mode:
      maxSpendCents === 0
        ? ('free_slice_or_paid_quote' as const)
        : ('free_slice_or_paid_quote_or_l402' as const),
    paymentPolicy: {
      buyerPaymentMode:
        maxSpendCents === 0 ? ('free_slice' as const) : ('l402' as const),
      maxSpendCents,
      quoteRef: maxSpendCents === 0 ? null : `quote.${taskRef}`,
      quotedAmountCents: maxSpendCents === 0 ? null : maxSpendCents,
      settlementMode:
        maxSpendCents === 0
          ? ('no_worker_payout' as const)
          : ('no_worker_payout_until_accepted_work' as const),
    },
    placementPolicy: {
      allowedRunnerKinds: ['requester_pylon', 'openagents_shc'] as const,
      disallowedRunnerKinds: [] as const,
      localOnlyAllowed: false,
      preferredRunnerKinds: ['requester_pylon', 'openagents_shc'] as const,
      privacyTier:
        maxSpendCents === 0
          ? ('public_beta' as const)
          : ('openagents_shc' as const),
      publicTraceAllowed: maxSpendCents === 0,
      requiresSecretBroker: false,
    },
    promiseRef: {
      blockerRefs: [] as ReadonlyArray<string>,
      promiseId: AUTOPILOT_WORK_LIST_PROMISE_ID,
      registryVersion: '2026-06-11.1',
    },
    schema: 'openagents.autopilot_work_request.v1' as const,
    tasks: [
      {
        acceptanceCriteriaRefs: ['acceptance.web_request.customer_review'],
        accessRequests:
          repository === undefined
            ? [
                {
                  kind: 'repository_selection' as const,
                  reasonRef: 'access.repository.selection_required',
                },
              ]
            : [
                {
                  kind: 'github_repo_read' as const,
                  reasonRef: 'access.github.public_read',
                },
              ],
        ...(repository === undefined
          ? {}
          : {
              checkout: {
                commitSha: '1111111111111111111111111111111111111111',
                kind: 'git_checkout' as const,
                verificationCommand: {
                  args: args.length === 0 ? ['bun', 'test'] : args,
                  commandRef: `command.${cleanRefSegment(draft.verificationCommand)}`,
                },
              },
              repository,
            }),
        forumReporting: { mode: 'operator_approved_only' as const },
        kind: 'code_change' as const,
        objective: objective || 'Triage the requested public repository work.',
        taskRef,
      },
    ],
  }
}

export const LoadAutopilotWorkList = Command.define(
  'LoadAutopilotWorkList',
  {},
  SucceededLoadAutopilotWorkList,
  FailedLoadAutopilotWorkList,
)(() =>
  Effect.gen(function* () {
    const response = yield* requestJson({
      init: {
        cache: 'no-store',
        credentials: 'include',
        headers: { accept: 'application/json' },
      },
      name: 'loggedIn.autopilotWork.list.load',
      request: autopilotWorkListPath(),
      schema: AutopilotWorkListResponse,
    })

    return SucceededLoadAutopilotWorkList({ response })
  }).pipe(
    Effect.catch(error =>
      Effect.succeed(
        FailedLoadAutopilotWorkList({
          error: errorMessageFromUnknown(error),
        }),
      ),
    ),
  ),
)

export const LoadCustomerOneCohort = Command.define(
  'LoadCustomerOneCohort',
  {},
  SucceededLoadCustomerOneCohort,
  FailedLoadCustomerOneCohort,
)(() =>
  Effect.gen(function* () {
    const response = yield* requestJson({
      init: {
        cache: 'no-store',
        credentials: 'include',
        headers: { accept: 'application/json' },
      },
      name: 'loggedIn.customerOneCohort.load',
      request: customerOneCohortPath(),
      schema: CustomerOneCohortProjection,
    })

    return SucceededLoadCustomerOneCohort({ response })
  }).pipe(
    Effect.catch(error =>
      Effect.succeed(
        FailedLoadCustomerOneCohort({
          error: errorMessageFromUnknown(error),
        }),
      ),
    ),
  ),
)

export const LoadAutopilotMorningReport = Command.define(
  'LoadAutopilotMorningReport',
  {},
  SucceededLoadAutopilotMorningReport,
  FailedLoadAutopilotMorningReport,
)(() =>
  Effect.gen(function* () {
    const response = yield* requestJson({
      init: {
        cache: 'no-store',
        credentials: 'include',
        headers: { accept: 'application/json' },
      },
      name: 'loggedIn.autopilotWork.morningReport.load',
      request: autopilotMorningReportPath(),
      schema: AutopilotMorningReportResponse,
    })

    return SucceededLoadAutopilotMorningReport({ response })
  }).pipe(
    Effect.catch(error =>
      Effect.succeed(
        FailedLoadAutopilotMorningReport({
          error: errorMessageFromUnknown(error),
        }),
      ),
    ),
  ),
)

export const LoadAutopilotWorkDetail = Command.define(
  'LoadAutopilotWorkDetail',
  {
    workOrderRef: S.String,
  },
  SucceededLoadAutopilotWorkDetail,
  FailedLoadAutopilotWorkDetail,
)(({ workOrderRef }) =>
  Effect.gen(function* () {
    const response = yield* requestJson({
      init: {
        cache: 'no-store',
        credentials: 'include',
        headers: { accept: 'application/json' },
      },
      name: 'loggedIn.autopilotWork.detail.load',
      request: autopilotWorkPath(workOrderRef),
      schema: AutopilotWorkResponse,
    })

    return SucceededLoadAutopilotWorkDetail({ response })
  }).pipe(
    Effect.catch(error =>
      Effect.succeed(
        FailedLoadAutopilotWorkDetail({
          error: errorMessageFromUnknown(error),
        }),
      ),
    ),
  ),
)

export const LoadAutopilotWorkEvents = Command.define(
  'LoadAutopilotWorkEvents',
  {
    workOrderRef: S.String,
  },
  SucceededLoadAutopilotWorkEvents,
  FailedLoadAutopilotWorkEvents,
)(({ workOrderRef }) =>
  Effect.gen(function* () {
    const response = yield* requestJson({
      init: {
        cache: 'no-store',
        credentials: 'include',
        headers: { accept: 'application/json' },
      },
      name: 'loggedIn.autopilotWork.events.load',
      request: autopilotWorkEventsPath(workOrderRef),
      schema: AutopilotWorkEventsResponse,
    })

    return SucceededLoadAutopilotWorkEvents({ response })
  }).pipe(
    Effect.catch(error =>
      Effect.succeed(
        FailedLoadAutopilotWorkEvents({
          error: errorMessageFromUnknown(error),
        }),
      ),
    ),
  ),
)

export const LoadAutopilotWorkBriefing = Command.define(
  'LoadAutopilotWorkBriefing',
  {
    workOrderRef: S.String,
  },
  SucceededLoadAutopilotWorkBriefing,
  FailedLoadAutopilotWorkBriefing,
)(({ workOrderRef }) =>
  Effect.gen(function* () {
    const response = yield* requestJson({
      init: {
        cache: 'no-store',
        credentials: 'include',
        headers: { accept: 'application/json' },
      },
      name: 'loggedIn.autopilotWork.briefing.load',
      request: autopilotWorkBriefingPath(workOrderRef),
      schema: AutopilotWorkBriefingResponse,
    })

    return SucceededLoadAutopilotWorkBriefing({ response })
  }).pipe(
    Effect.catch(error =>
      Effect.succeed(
        FailedLoadAutopilotWorkBriefing({
          error: errorMessageFromUnknown(error),
        }),
      ),
    ),
  ),
)

export const SubmitAutopilotWorkComposer = Command.define(
  'SubmitAutopilotWorkComposer',
  {
    draft: AutopilotWorkComposerDraft,
  },
  SucceededAutopilotWorkComposer,
  FailedAutopilotWorkComposer,
)(({ draft }) =>
  Effect.gen(function* () {
    const response = yield* Effect.tryPromise({
      try: () =>
        fetch('/api/autopilot/work', {
          body: JSON.stringify(workRequestFromDraft(draft)),
          cache: 'no-store',
          credentials: 'include',
          headers: {
            accept: 'application/json',
            'content-type': 'application/json',
            'idempotency-key': `browser-composer:${cleanRefSegment(draft.repositoryFullName)}:${cleanRefSegment(draft.objective)}`,
          },
          method: 'POST',
        }),
      catch: errorMessageFromUnknown,
    })
    const payload = yield* Effect.tryPromise({
      try: () => response.json(),
      catch: errorMessageFromUnknown,
    })

    if (!response.ok && response.status !== 402) {
      return yield* Effect.fail(payload)
    }

    return SucceededAutopilotWorkComposer({
      response: S.decodeUnknownSync(AutopilotWorkResponse)(payload),
    })
  }).pipe(
    Effect.catch(error =>
      Effect.succeed(
        FailedAutopilotWorkComposer({
          error: errorMessageFromUnknown(error),
        }),
      ),
    ),
  ),
)

export const SubmitAutopilotWorkReview = Command.define(
  'SubmitAutopilotWorkReview',
  {
    action: AutopilotWorkReviewAction,
    workOrderRef: S.String,
  },
  SucceededAutopilotWorkReview,
  FailedAutopilotWorkReview,
)(({ action, workOrderRef }) =>
  Effect.gen(function* () {
    const response = yield* requestJson({
      init: {
        body: JSON.stringify({
          action,
          ...reviewRefs(action, workOrderRef),
        }),
        cache: 'no-store',
        credentials: 'include',
        headers: {
          accept: 'application/json',
          'content-type': 'application/json',
          'idempotency-key': `browser-review:${action}:${workOrderRef}`,
        },
        method: 'POST',
      },
      name: 'loggedIn.autopilotWork.review.submit',
      request: `${autopilotWorkPath(workOrderRef)}/review`,
      schema: AutopilotWorkResponse,
    })

    return SucceededAutopilotWorkReview({ response })
  }).pipe(
    Effect.catch(error =>
      Effect.succeed(
        FailedAutopilotWorkReview({
          error: errorMessageFromUnknown(error),
        }),
      ),
    ),
  ),
)

const detailCommands = (
  workOrderRef: string,
): ReadonlyArray<Command.Command<Message>> => [
  LoadAutopilotWorkDetail({ workOrderRef }),
  LoadAutopilotWorkEvents({ workOrderRef }),
  LoadAutopilotWorkBriefing({ workOrderRef }),
]

export const updateAutopilotWork = (
  model: Model,
  message: Message,
): UpdateReturn =>
  M.value(message).pipe(
    withUpdateReturn,
    M.tags({
      RequestedLoadAutopilotWorkList: () => [
        evo(model, {
          autopilotMorningReport: () => AutopilotMorningReportLoading(),
          autopilotWorkList: () => AutopilotWorkListLoading(),
        }),
        [LoadAutopilotWorkList({}), LoadAutopilotMorningReport({})],
        Option.none(),
      ],
      SucceededLoadAutopilotWorkList: ({ response }) => [
        evo(model, {
          autopilotWorkList: () => AutopilotWorkListLoaded({ response }),
        }),
        [],
        Option.none(),
      ],
      FailedLoadAutopilotWorkList: ({ error }) => [
        evo(model, {
          autopilotWorkList: () => AutopilotWorkListFailed({ error }),
        }),
        [],
        Option.none(),
      ],
      RequestedLoadCustomerOneCohort: () => [
        evo(model, {
          customerOneCohort: () => CustomerOneCohortLoading(),
        }),
        [LoadCustomerOneCohort({})],
        Option.none(),
      ],
      SucceededLoadCustomerOneCohort: ({ response }) => [
        evo(model, {
          customerOneCohort: () => CustomerOneCohortLoaded({ response }),
        }),
        [],
        Option.none(),
      ],
      FailedLoadCustomerOneCohort: ({ error }) => [
        evo(model, {
          customerOneCohort: () => CustomerOneCohortFailed({ error }),
        }),
        [],
        Option.none(),
      ],
      RequestedLoadAutopilotMorningReport: () => [
        evo(model, {
          autopilotMorningReport: () => AutopilotMorningReportLoading(),
        }),
        [LoadAutopilotMorningReport({})],
        Option.none(),
      ],
      SucceededLoadAutopilotMorningReport: ({ response }) => [
        evo(model, {
          autopilotMorningReport: () =>
            AutopilotMorningReportLoaded({ response }),
        }),
        [],
        Option.none(),
      ],
      FailedLoadAutopilotMorningReport: ({ error }) => [
        evo(model, {
          autopilotMorningReport: () => AutopilotMorningReportFailed({ error }),
        }),
        [],
        Option.none(),
      ],
      UpdatedAutopilotWorkComposerField: ({ field, value }) => [
        evo(model, {
          autopilotWorkComposerDraft: draft => ({ ...draft, [field]: value }),
          autopilotWorkComposer: () => AutopilotWorkComposerIdle(),
        }),
        [],
        Option.none(),
      ],
      SubmittedAutopilotWorkComposer: () => [
        evo(model, {
          autopilotWorkComposer: () => AutopilotWorkComposerSubmitting(),
        }),
        [
          SubmitAutopilotWorkComposer({
            draft: model.autopilotWorkComposerDraft,
          }),
        ],
        Option.none(),
      ],
      SucceededAutopilotWorkComposer: ({ response }) => [
        evo(model, {
          autopilotWorkComposer: () =>
            AutopilotWorkComposerSucceeded({ response }),
          autopilotWorkDetail: () => AutopilotWorkDetailLoaded({ response }),
        }),
        [
          LoadAutopilotWorkList({}),
          ...detailCommands(response.work.workOrderRef),
        ],
        Option.none(),
      ],
      FailedAutopilotWorkComposer: ({ error }) => [
        evo(model, {
          autopilotWorkComposer: () => AutopilotWorkComposerFailed({ error }),
        }),
        [],
        Option.none(),
      ],
      SelectedForgeAutomationTemplate: ({ automationId }) => {
        const draft = automationDraftForId(automationId)

        return draft === null
          ? [
              evo(model, {
                autopilotWorkComposer: () =>
                  AutopilotWorkComposerFailed({
                    error: `Unknown Forge automation: ${automationId}`,
                  }),
              }),
              [],
              Option.none(),
            ]
          : [
              evo(model, {
                autopilotWorkComposer: () => AutopilotWorkComposerIdle(),
                autopilotWorkComposerDraft: () => draft,
              }),
              [],
              Option.none(),
            ]
      },
      SubmittedForgeAutomationRun: ({ automationId }) => {
        const draft = automationDraftForId(automationId)

        return draft === null
          ? [
              evo(model, {
                autopilotWorkComposer: () =>
                  AutopilotWorkComposerFailed({
                    error: `Unknown Forge automation: ${automationId}`,
                  }),
              }),
              [],
              Option.none(),
            ]
          : [
              evo(model, {
                autopilotWorkComposer: () => AutopilotWorkComposerSubmitting(),
                autopilotWorkComposerDraft: () => draft,
              }),
              [SubmitAutopilotWorkComposer({ draft })],
              Option.none(),
            ]
      },
      RequestedLoadAutopilotWorkDetail: ({ workOrderRef }) => [
        evo(model, {
          autopilotWorkBriefing: () => AutopilotWorkBriefingLoading(),
          autopilotWorkDetail: () => AutopilotWorkDetailLoading(),
          autopilotWorkEvents: () => AutopilotWorkEventsLoading(),
          autopilotWorkReview: () => AutopilotWorkReviewIdle(),
        }),
        detailCommands(workOrderRef),
        Option.none(),
      ],
      SucceededLoadAutopilotWorkDetail: ({ response }) => [
        evo(model, {
          autopilotWorkDetail: () => AutopilotWorkDetailLoaded({ response }),
        }),
        [],
        Option.none(),
      ],
      FailedLoadAutopilotWorkDetail: ({ error }) => [
        evo(model, {
          autopilotWorkDetail: () => AutopilotWorkDetailFailed({ error }),
        }),
        [],
        Option.none(),
      ],
      SucceededLoadAutopilotWorkEvents: ({ response }) => [
        evo(model, {
          autopilotWorkEvents: () => AutopilotWorkEventsLoaded({ response }),
        }),
        [],
        Option.none(),
      ],
      FailedLoadAutopilotWorkEvents: ({ error }) => [
        evo(model, {
          autopilotWorkEvents: () => AutopilotWorkEventsFailed({ error }),
        }),
        [],
        Option.none(),
      ],
      SucceededLoadAutopilotWorkBriefing: ({ response }) => [
        evo(model, {
          autopilotWorkBriefing: () =>
            AutopilotWorkBriefingLoaded({ response }),
        }),
        [],
        Option.none(),
      ],
      FailedLoadAutopilotWorkBriefing: ({ error }) => [
        evo(model, {
          autopilotWorkBriefing: () => AutopilotWorkBriefingFailed({ error }),
        }),
        [],
        Option.none(),
      ],
      SubmittedAutopilotWorkReview: ({ action, workOrderRef }) => [
        evo(model, {
          autopilotWorkReview: () => AutopilotWorkReviewSubmitting({ action }),
        }),
        [SubmitAutopilotWorkReview({ action, workOrderRef })],
        Option.none(),
      ],
      SucceededAutopilotWorkReview: ({ response }) => [
        evo(model, {
          autopilotWorkDetail: () => AutopilotWorkDetailLoaded({ response }),
          autopilotWorkReview: () => AutopilotWorkReviewSucceeded({ response }),
        }),
        [
          LoadAutopilotWorkEvents({
            workOrderRef: response.work.workOrderRef,
          }),
          LoadAutopilotWorkBriefing({
            workOrderRef: response.work.workOrderRef,
          }),
        ],
        Option.none(),
      ],
      FailedAutopilotWorkReview: ({ error }) => [
        evo(model, {
          autopilotWorkReview: () => AutopilotWorkReviewFailed({ error }),
        }),
        [],
        Option.none(),
      ],
    }),
    M.orElse(() => [model, [], Option.none()]),
  )
