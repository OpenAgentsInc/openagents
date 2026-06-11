import { Effect, Match as M, Option, Schema as S } from 'effect'
import { Command } from 'foldkit'
import { evo } from 'foldkit/struct'

import {
  errorMessageFromUnknown,
  requestJson,
} from '../commands/api'
import {
  FailedAutopilotWorkReview,
  FailedLoadAutopilotWorkBriefing,
  FailedLoadAutopilotWorkDetail,
  FailedLoadAutopilotWorkEvents,
  FailedLoadAutopilotWorkList,
  Message,
  SucceededAutopilotWorkReview,
  SucceededLoadAutopilotWorkBriefing,
  SucceededLoadAutopilotWorkDetail,
  SucceededLoadAutopilotWorkEvents,
  SucceededLoadAutopilotWorkList,
} from '../message'
import {
  AUTOPILOT_WORK_LIST_PROMISE_ID,
  AutopilotWorkBriefingFailed,
  AutopilotWorkBriefingLoaded,
  AutopilotWorkBriefingLoading,
  AutopilotWorkBriefingResponse,
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
  Model,
} from '../model'
import { type UpdateReturn } from '../transition'

const withUpdateReturn = M.withReturnType<UpdateReturn>()

const autopilotWorkListPath = (): string =>
  `/api/autopilot/work?promiseId=${encodeURIComponent(AUTOPILOT_WORK_LIST_PROMISE_ID)}`

const autopilotWorkPath = (workOrderRef: string): string =>
  `/api/autopilot/work/${encodeURIComponent(workOrderRef)}`

const autopilotWorkEventsPath = (workOrderRef: string): string =>
  `${autopilotWorkPath(workOrderRef)}/events`

const autopilotWorkBriefingPath = (workOrderRef: string): string =>
  `${autopilotWorkPath(workOrderRef)}/briefing`

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
        evo(model, { autopilotWorkList: () => AutopilotWorkListLoading() }),
        [LoadAutopilotWorkList({})],
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
          autopilotWorkReview: () =>
            AutopilotWorkReviewSubmitting({ action }),
        }),
        [SubmitAutopilotWorkReview({ action, workOrderRef })],
        Option.none(),
      ],
      SucceededAutopilotWorkReview: ({ response }) => [
        evo(model, {
          autopilotWorkDetail: () => AutopilotWorkDetailLoaded({ response }),
          autopilotWorkReview: () =>
            AutopilotWorkReviewSucceeded({ response }),
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
