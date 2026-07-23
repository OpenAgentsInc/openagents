import {
  decodeKhalaRuntimeControlIntent,
  decodePushRequest,
  type KhalaRuntimeControlIntent,
} from '@openagentsinc/khala-sync'
import {
  CHAT_APPEND_MESSAGE_MUTATOR_NAME,
  CHAT_BIND_THREAD_REPO_MUTATOR_NAME,
  CHAT_CREATE_THREAD_MUTATOR_NAME,
  RUNTIME_START_TURN_MUTATOR_NAME,
  chatMutators,
  executePush as executePushEngine,
  makeMutatorRegistry,
  runtimeMutators,
  type MutatorRegistry,
  type SyncSql,
} from '@openagentsinc/khala-sync-server'
import { Effect, Schema as S } from 'effect'

import {
  AgentComputerHarnessId,
  managedAgentComputerHarnessExecutionTargetId,
} from './khala-cloud-runtime-inference-block'
import { currentIsoTimestamp } from './runtime-primitives'

const PublicRef = S.Trim.check(
  S.isMinLength(1),
  S.isMaxLength(256),
  S.isPattern(/^[A-Za-z0-9][A-Za-z0-9._:-]*$/u),
)
const RepositoryFullName = S.Trim.check(
  S.isMinLength(3),
  S.isMaxLength(180),
  S.isPattern(/^[A-Za-z0-9_.-]+\/[A-Za-z0-9_.-]+$/u),
)
const ImmutableCommit = S.Trim.check(
  S.isPattern(/^[a-f0-9]{40}$/iu),
)
const Objective = S.Trim.check(S.isMinLength(3), S.isMaxLength(8_000))

const SarahCloudCodingDispatchInput = S.Struct({
  commit: ImmutableCommit,
  harnessId: S.optionalKey(AgentComputerHarnessId),
  objective: Objective,
  ownerUserId: PublicRef,
  parentThreadRef: PublicRef,
  repository: RepositoryFullName,
  toolCallId: PublicRef,
  turnId: PublicRef,
})

export interface SarahCloudCodingDispatchInput
  extends S.Schema.Type<typeof SarahCloudCodingDispatchInput> {}

export type SarahCloudCodingDispatchReceipt = Readonly<{
  cloudTurnRef: string
  dispatchRef: string
  threadRef: string
  workContextRef: string
}>

export type SarahCloudCodingDispatch = (
  input: SarahCloudCodingDispatchInput,
) => Effect.Effect<SarahCloudCodingDispatchReceipt, SarahCloudCodingDispatchError>

export class SarahCloudCodingDispatchError extends S.TaggedErrorClass<SarahCloudCodingDispatchError>()(
  'SarahCloudCodingDispatchError',
  {
    reason: S.String,
  },
) {}

export type SarahCloudCodingExecutePush = typeof executePushEngine

export type SarahCloudCodingDispatchDependencies = Readonly<{
  sql: SyncSql
  executePush?: SarahCloudCodingExecutePush | undefined
  nowIso?: (() => string) | undefined
  registry?: MutatorRegistry | undefined
}>

const refSegment = (value: string): string =>
  value.replaceAll(/[^A-Za-z0-9_.:-]/g, '_').slice(0, 80)

const makeRefs = (input: SarahCloudCodingDispatchInput) => {
  const seed = `${refSegment(input.turnId)}.${refSegment(input.toolCallId)}`
  return {
    cloudTurnRef: `turn.sarah_cloud.${seed}`,
    dispatchRef: `dispatch.sarah_cloud.${seed}`,
    intentRef: `intent.sarah_cloud.${seed}`,
    messageRef: `message.sarah_cloud.${seed}`,
    threadRef: `thread.sarah_cloud.${seed}`,
  }
}

export const makeSarahCloudCodingDispatch = (
  deps: SarahCloudCodingDispatchDependencies,
): SarahCloudCodingDispatch =>
  Effect.fn('SarahCloudCoding.dispatch')(function* (rawInput) {
    const input = yield* S.decodeUnknownEffect(SarahCloudCodingDispatchInput)(
      rawInput,
      { onExcessProperty: 'error' },
    ).pipe(
      Effect.mapError(() =>
        new SarahCloudCodingDispatchError({
          reason: 'invalid_cloud_coding_dispatch_input',
        }),
      ),
    )
    const [repositoryOwner, repositoryName] = input.repository.split('/')
    if (repositoryOwner === undefined || repositoryName === undefined) {
      return yield* new SarahCloudCodingDispatchError({
        reason: 'invalid_cloud_coding_repository',
      })
    }
    const refs = makeRefs(input)
    const nowIso = (deps.nowIso ?? currentIsoTimestamp)()
    const intent: KhalaRuntimeControlIntent =
      decodeKhalaRuntimeControlIntent({
        bodyRef: `chat_message.${refs.messageRef}`,
        causalityRefs: [
          input.parentThreadRef,
          input.turnId,
          input.toolCallId,
        ],
        createdAt: nowIso,
        idempotencyKey: `idempotency.${refs.intentRef}`,
        intentId: refs.intentRef,
        kind: 'turn.start',
        origin: {
          lane: 'hosted_khala',
          surface: 'server',
        },
        redactionClass: 'private_ref',
        schema: 'openagents.khala_runtime_control_intent.v1',
        target: {
          adapterKind: 'openagents_native',
          lane: 'managed_cloud',
          ...(input.harnessId === undefined
            ? {}
            : {
                executionTargetId:
                  managedAgentComputerHarnessExecutionTargetId(input.harnessId),
              }),
        },
        threadId: refs.threadRef,
        turnId: refs.cloudTurnRef,
        visibility: 'private',
      })
    const executePush = deps.executePush ?? executePushEngine
    const registry =
      deps.registry ?? makeMutatorRegistry([...chatMutators, ...runtimeMutators])
    const response = yield* Effect.tryPromise({
      try: () =>
        executePush({
          registry,
          request: decodePushRequest({
            clientGroupId: `server.sarah.cloud_coding.${refSegment(input.ownerUserId)}`,
            clientId: `server.sarah.cloud_coding.${refs.dispatchRef}`,
            mutations: [
              {
                argsJson: JSON.stringify({
                  threadId: refs.threadRef,
                  title: 'Sarah Agent Computer task',
                }),
                mutationId: 1,
                name: CHAT_CREATE_THREAD_MUTATOR_NAME,
              },
              {
                argsJson: JSON.stringify({
                  repo: {
                    defaultBranch: input.commit,
                    name: repositoryName,
                    owner: repositoryOwner,
                  },
                  threadId: refs.threadRef,
                }),
                mutationId: 2,
                name: CHAT_BIND_THREAD_REPO_MUTATOR_NAME,
              },
              {
                argsJson: JSON.stringify({
                  attachments: [],
                  body: input.objective,
                  messageId: refs.messageRef,
                  threadId: refs.threadRef,
                }),
                mutationId: 3,
                name: CHAT_APPEND_MESSAGE_MUTATOR_NAME,
              },
              {
                argsJson: JSON.stringify(intent),
                mutationId: 4,
                name: RUNTIME_START_TURN_MUTATOR_NAME,
              },
            ],
            protocolVersion: 1,
            schemaVersion: 1,
          }),
          sql: deps.sql,
          userId: input.ownerUserId,
        }),
      catch: error =>
        new SarahCloudCodingDispatchError({
          reason:
            error instanceof Error
              ? error.message
              : 'cloud_coding_dispatch_storage_failed',
        }),
    })
    const rejected = response.results.find(result => result.status !== 'applied')
    if (rejected !== undefined) {
      return yield* new SarahCloudCodingDispatchError({
        reason:
          rejected.errorCode === undefined
            ? 'cloud_coding_dispatch_rejected'
            : `cloud_coding_dispatch_rejected.${rejected.errorCode}`,
      })
    }
    return {
      cloudTurnRef: refs.cloudTurnRef,
      dispatchRef: refs.dispatchRef,
      threadRef: refs.threadRef,
      workContextRef: `work_context.thread.${refs.threadRef}`,
    }
  })
