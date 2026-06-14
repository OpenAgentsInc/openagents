import { Effect, Match as M, Option, Schema as S } from 'effect'
import { Command } from 'foldkit'
import { evo } from 'foldkit/struct'

import { errorMessageFromUnknown, requestJson } from '../commands/api'
import {
  FailedLoadWorkroomLifecycle,
  FailedLoadWorkroomSurface,
  FailedWorkroomLifecycleDecision,
  Message,
  SucceededLoadWorkroomLifecycle,
  SucceededLoadWorkroomSurface,
  SucceededWorkroomLifecycleDecision,
} from '../message'
import { Model } from '../model'
import {
  type Msg as WorkroomMsg,
  OmniLifecycleDecisionKind,
  OmniLifecycleDecisionResponse,
  OmniLifecycleHistoryResponse,
  OmniWorkroomSurfaceResponse,
  lifecycleDecisionRequestInfo,
  lifecycleRequestInfo,
  surfaceRequestInfo,
  update as updateWorkroomModel,
} from '../page/workroom'
import { type UpdateReturn } from '../transition'

const withUpdateReturn = M.withReturnType<UpdateReturn>()

// -----------------------------------------------------------------------------
// Commands. These map the workroom page's pure Cmd ADT (emitted by
// updateWorkroomModel) into foldkit Command.define handlers, reusing the page's
// request-shape helpers (paths, customer surface query, Idempotency-Key header)
// and response schemas so the request contract stays owned by the page.
// -----------------------------------------------------------------------------

export const LoadWorkroomSurface = Command.define(
  'LoadWorkroomSurface',
  { workroomId: S.String },
  SucceededLoadWorkroomSurface,
  FailedLoadWorkroomSurface,
)(({ workroomId }) =>
  Effect.gen(function* () {
    const info = surfaceRequestInfo(workroomId)
    const response = yield* requestJson({
      init: info.init,
      name: 'loggedIn.workroom.surface.load',
      request: info.request,
      schema: OmniWorkroomSurfaceResponse,
    })

    return SucceededLoadWorkroomSurface({ response })
  }).pipe(
    Effect.catch(error =>
      Effect.succeed(
        FailedLoadWorkroomSurface({ error: errorMessageFromUnknown(error) }),
      ),
    ),
  ),
)

export const LoadWorkroomLifecycle = Command.define(
  'LoadWorkroomLifecycle',
  { workroomId: S.String },
  SucceededLoadWorkroomLifecycle,
  FailedLoadWorkroomLifecycle,
)(({ workroomId }) =>
  Effect.gen(function* () {
    const info = lifecycleRequestInfo(workroomId)
    const response = yield* requestJson({
      init: info.init,
      name: 'loggedIn.workroom.lifecycle.load',
      request: info.request,
      schema: OmniLifecycleHistoryResponse,
    })

    return SucceededLoadWorkroomLifecycle({ response })
  }).pipe(
    Effect.catch(error =>
      Effect.succeed(
        FailedLoadWorkroomLifecycle({ error: errorMessageFromUnknown(error) }),
      ),
    ),
  ),
)

export const SubmitWorkroomLifecycleDecision = Command.define(
  'SubmitWorkroomLifecycleDecision',
  {
    customerSafeExplanationRef: S.String,
    decisionKind: OmniLifecycleDecisionKind,
    idempotencyKey: S.String,
    receiptRef: S.String,
    workKind: S.String,
    workroomId: S.String,
  },
  SucceededWorkroomLifecycleDecision,
  FailedWorkroomLifecycleDecision,
)(input =>
  Effect.gen(function* () {
    const info = lifecycleDecisionRequestInfo(input)
    const response = yield* requestJson({
      init: info.init,
      name: 'loggedIn.workroom.lifecycle.decision.submit',
      request: info.request,
      schema: OmniLifecycleDecisionResponse,
    })

    return SucceededWorkroomLifecycleDecision({ response })
  }).pipe(
    Effect.catch(error =>
      Effect.succeed(
        FailedWorkroomLifecycleDecision({
          error: errorMessageFromUnknown(error),
        }),
      ),
    ),
  ),
)

// -----------------------------------------------------------------------------
// Cmd translation. The page emits a pure Cmd ADT from its own update(); the
// coordinator translates each variant into the foldkit commands defined above.
// -----------------------------------------------------------------------------

const commandsFromCmd = (
  cmd: ReturnType<typeof updateWorkroomModel>[1],
): ReadonlyArray<Command.Command<Message>> =>
  M.value(cmd).pipe(
    M.withReturnType<ReadonlyArray<Command.Command<Message>>>(),
    M.tag('None', () => []),
    M.tag('LoadSurface', ({ workroomId }) => [
      LoadWorkroomSurface({ workroomId }),
    ]),
    M.tag('LoadLifecycle', ({ workroomId }) => [
      LoadWorkroomLifecycle({ workroomId }),
    ]),
    M.tag(
      'SubmitLifecycleDecision',
      ({
        customerSafeExplanationRef,
        decisionKind,
        idempotencyKey,
        receiptRef,
        workKind,
        workroomId,
      }) => [
        SubmitWorkroomLifecycleDecision({
          customerSafeExplanationRef,
          decisionKind,
          idempotencyKey,
          receiptRef,
          workKind,
          workroomId,
        }),
      ],
    ),
    M.exhaustive,
  )

const runWorkroom = (model: Model, message: WorkroomMsg): UpdateReturn => {
  const [nextWorkroom, cmd] = updateWorkroomModel(model.workroom, message)

  return [
    evo(model, { workroom: () => nextWorkroom }),
    commandsFromCmd(cmd),
    Option.none(),
  ]
}

export const updateWorkroom = (model: Model, message: Message): UpdateReturn =>
  M.value(message).pipe(
    withUpdateReturn,
    M.tags({
      SelectedWorkroomTab: message => runWorkroom(model, message),
      RequestedLoadWorkroomSurface: message => runWorkroom(model, message),
      SucceededLoadWorkroomSurface: message => runWorkroom(model, message),
      FailedLoadWorkroomSurface: message => runWorkroom(model, message),
      RequestedLoadWorkroomLifecycle: message => runWorkroom(model, message),
      SucceededLoadWorkroomLifecycle: message => runWorkroom(model, message),
      FailedLoadWorkroomLifecycle: message => runWorkroom(model, message),
      SubmittedWorkroomLifecycleDecision: message =>
        runWorkroom(model, message),
      SucceededWorkroomLifecycleDecision: message =>
        runWorkroom(model, message),
      FailedWorkroomLifecycleDecision: message => runWorkroom(model, message),
    }),
    M.orElse(() => [model, [], Option.none()] as UpdateReturn),
  )
