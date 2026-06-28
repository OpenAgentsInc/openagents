import { Effect, Schema as S } from 'effect'

export const FRLM_CONDUCTOR_SIGNATURE_REF =
  'program_signature.frlm_conductor.v1'
export const RLM_LEAF_EXECUTOR_SIGNATURE_REF =
  'program_signature.rlm_leaf_executor.v1'
export const FRLM_BLUEPRINT_EVIDENCE_ONLY_SIGNATURE_REF =
  'program_signature.blueprint_action_submission.evidence_only.v1'

export const FrlmExecutorKind = S.Literals([
  'local',
  'swarm_nip90',
  'remote',
  'codex',
])
export type FrlmExecutorKind = typeof FrlmExecutorKind.Type

export const FrlmConductorEventKind = S.Literals([
  'Run.Init',
  'SubQuery.Submit',
  'SubQuery.Return',
  'Run.Done',
])
export type FrlmConductorEventKind = typeof FrlmConductorEventKind.Type

export const FrlmConductorStatus = S.Literals([
  'completed',
  'blocked_quorum_not_met',
])
export type FrlmConductorStatus = typeof FrlmConductorStatus.Type

export const FrlmEnvironmentFragment = S.Struct({
  fragmentRef: S.String,
  text: S.String,
})
export type FrlmEnvironmentFragment = typeof FrlmEnvironmentFragment.Type

export const FrlmEnvironment = S.Struct({
  contextVars: S.Record(S.String, S.String),
  fragments: S.Array(FrlmEnvironmentFragment),
})
export type FrlmEnvironment = typeof FrlmEnvironment.Type

export const FrlmBudgetPolicy = S.Struct({
  maxInputTokens: S.Number,
  maxOutputTokens: S.Number,
  maxWallClockMs: S.Number,
})
export type FrlmBudgetPolicy = typeof FrlmBudgetPolicy.Type

export const FrlmSchedulerPolicy = S.Struct({
  allowedExecutors: S.Array(FrlmExecutorKind),
  blueprintSignatureRefs: S.Array(S.String),
  evidenceRefs: S.Array(S.String),
  maxDepth: S.Number,
  maxParallelism: S.Number,
  maxSubQueries: S.Number,
  quorum: S.Number,
  budget: FrlmBudgetPolicy,
})
export type FrlmSchedulerPolicy = typeof FrlmSchedulerPolicy.Type

export const FrlmSubQuery = S.Struct({
  depth: S.Number,
  executor: FrlmExecutorKind,
  input: S.String,
  purpose: S.String,
  signatureRef: S.String,
  subQueryId: S.String,
})
export type FrlmSubQuery = typeof FrlmSubQuery.Type

export const FrlmSubQueryUsage = S.Struct({
  inputTokens: S.Number,
  outputTokens: S.Number,
  totalTokens: S.Number,
})
export type FrlmSubQueryUsage = typeof FrlmSubQueryUsage.Type

export const FrlmSubQueryReturn = S.Struct({
  evidenceRefs: S.Array(S.String),
  output: S.String,
  subQueryId: S.String,
  usage: S.optional(FrlmSubQueryUsage),
})
export type FrlmSubQueryReturn = typeof FrlmSubQueryReturn.Type

export const FrlmConductorRunInput = S.Struct({
  environment: FrlmEnvironment,
  objective: S.String,
  requestedAt: S.String,
  runId: S.String,
  policy: FrlmSchedulerPolicy,
})
export type FrlmConductorRunInput = typeof FrlmConductorRunInput.Type

export type FrlmSubQueryResult = Readonly<
  | {
      status: 'returned'
      query: FrlmSubQuery
      value: FrlmSubQueryReturn
    }
  | {
      status: 'failed'
      query: FrlmSubQuery
      reason: string
    }
>

export type FrlmConductorEvent = Readonly<{
  at: string
  eventId: string
  kind: FrlmConductorEventKind
  runId: string
  subQueryId: string | null
  refs: ReadonlyArray<string>
  summary: string
}>

export type FrlmCompositionInput = Readonly<{
  run: FrlmConductorRunInput
  subQueries: ReadonlyArray<FrlmSubQuery>
  returns: ReadonlyArray<FrlmSubQueryReturn>
}>

export type FrlmConductorRun = Readonly<{
  answer: string | null
  compositionPrompt: string
  events: ReadonlyArray<FrlmConductorEvent>
  policy: FrlmSchedulerPolicy
  returns: ReadonlyArray<FrlmSubQueryReturn>
  runId: string
  status: FrlmConductorStatus
  subQueries: ReadonlyArray<FrlmSubQuery>
  usage: FrlmSubQueryUsage
}>

export class FrlmConductorError extends S.TaggedErrorClass<FrlmConductorError>()(
  'FrlmConductorError',
  {
    reason: S.String,
  },
) {}

export type FrlmPlanner = (
  input: FrlmConductorRunInput,
) => Effect.Effect<ReadonlyArray<FrlmSubQuery>, FrlmConductorError>

export type FrlmLeafExecutor = (
  subQuery: FrlmSubQuery,
  input: FrlmConductorRunInput,
) => Effect.Effect<FrlmSubQueryReturn, FrlmConductorError>

export type FrlmComposer = (
  input: FrlmCompositionInput,
) => Effect.Effect<string, FrlmConductorError>

export type FrlmConductorDependencies = Readonly<{
  compose: FrlmComposer
  execute: FrlmLeafExecutor
  plan: FrlmPlanner
}>

const unique = <A>(values: ReadonlyArray<A>): ReadonlyArray<A> => [
  ...new Set(values),
]

const sumUsage = (
  returns: ReadonlyArray<FrlmSubQueryReturn>,
): FrlmSubQueryUsage =>
  returns.reduce(
    (acc, item) => ({
      inputTokens: acc.inputTokens + (item.usage?.inputTokens ?? 0),
      outputTokens: acc.outputTokens + (item.usage?.outputTokens ?? 0),
      totalTokens: acc.totalTokens + (item.usage?.totalTokens ?? 0),
    }),
    { inputTokens: 0, outputTokens: 0, totalTokens: 0 },
  )

const event = (input: {
  at: string
  index: number
  kind: FrlmConductorEventKind
  refs?: ReadonlyArray<string>
  runId: string
  subQueryId?: string | null
  summary: string
}): FrlmConductorEvent => ({
  at: input.at,
  eventId: `${input.runId}.event.${String(input.index).padStart(3, '0')}`,
  kind: input.kind,
  refs: input.refs ?? [],
  runId: input.runId,
  subQueryId: input.subQueryId ?? null,
  summary: input.summary,
})

const validatePolicy = (
  input: FrlmConductorRunInput,
): Effect.Effect<void, FrlmConductorError> =>
  Effect.gen(function* () {
    if (input.objective.trim().length === 0) {
      yield* new FrlmConductorError({ reason: 'objective is required' })
    }
    if (input.policy.maxSubQueries < 1) {
      yield* new FrlmConductorError({ reason: 'maxSubQueries must be >= 1' })
    }
    if (input.policy.maxParallelism < 1) {
      yield* new FrlmConductorError({ reason: 'maxParallelism must be >= 1' })
    }
    if (input.policy.quorum < 1) {
      yield* new FrlmConductorError({ reason: 'quorum must be >= 1' })
    }
    if (input.policy.quorum > input.policy.maxSubQueries) {
      yield* new FrlmConductorError({
        reason: 'quorum cannot exceed maxSubQueries',
      })
    }
    if (input.policy.allowedExecutors.length === 0) {
      yield* new FrlmConductorError({
        reason: 'at least one executor must be allowed',
      })
    }
  })

const validateSubQueries = (
  input: FrlmConductorRunInput,
  subQueries: ReadonlyArray<FrlmSubQuery>,
): Effect.Effect<ReadonlyArray<FrlmSubQuery>, FrlmConductorError> =>
  Effect.gen(function* () {
    if (subQueries.length === 0) {
      yield* new FrlmConductorError({
        reason: 'planner returned no subqueries',
      })
    }
    if (subQueries.length > input.policy.maxSubQueries) {
      yield* new FrlmConductorError({
        reason: 'planner exceeded maxSubQueries',
      })
    }
    const allowed = new Set(input.policy.allowedExecutors)
    const ids = new Set<string>()
    for (const subQuery of subQueries) {
      if (ids.has(subQuery.subQueryId)) {
        yield* new FrlmConductorError({
          reason: `duplicate subQueryId ${subQuery.subQueryId}`,
        })
      }
      ids.add(subQuery.subQueryId)
      if (!allowed.has(subQuery.executor)) {
        yield* new FrlmConductorError({
          reason: `executor ${subQuery.executor} is not allowed`,
        })
      }
      if (subQuery.depth > input.policy.maxDepth) {
        yield* new FrlmConductorError({
          reason: `subquery ${subQuery.subQueryId} exceeds maxDepth`,
        })
      }
      if (!input.policy.blueprintSignatureRefs.includes(subQuery.signatureRef)) {
        yield* new FrlmConductorError({
          reason: `subquery ${subQuery.subQueryId} uses an ungoverned Blueprint signature`,
        })
      }
    }
    return subQueries
  })

const executeSubQuery = (
  execute: FrlmLeafExecutor,
  input: FrlmConductorRunInput,
  subQuery: FrlmSubQuery,
): Effect.Effect<FrlmSubQueryResult> =>
  execute(subQuery, input).pipe(
    Effect.map(value => ({
      query: subQuery,
      status: 'returned' as const,
      value,
    })),
    Effect.catch(error =>
      Effect.succeed({
        query: subQuery,
        reason: error.reason,
        status: 'failed' as const,
      }),
    ),
  )

export const buildFrlmCompositionPrompt = (
  input: FrlmCompositionInput,
): string => {
  const contextVars = Object.entries(input.run.environment.contextVars)
    .map(([key, value]) => `${key}: ${value}`)
    .join('\n')
  const fragments = input.run.environment.fragments
    .map(fragment => `[${fragment.fragmentRef}]\n${fragment.text}`)
    .join('\n\n')
  const returns = input.returns
    .map(result => {
      const query = input.subQueries.find(
        subQuery => subQuery.subQueryId === result.subQueryId,
      )
      return [
        `SubQuery: ${result.subQueryId}`,
        `Purpose: ${query?.purpose ?? 'unknown'}`,
        `Signature: ${query?.signatureRef ?? RLM_LEAF_EXECUTOR_SIGNATURE_REF}`,
        `Evidence: ${result.evidenceRefs.join(', ')}`,
        result.output,
      ].join('\n')
    })
    .join('\n\n')

  return [
    'Compose the final Artanis operator answer from the recursive language model subquery returns.',
    `Objective: ${input.run.objective}`,
    `Run: ${input.run.runId}`,
    `Blueprint signatures: ${input.run.policy.blueprintSignatureRefs.join(', ')}`,
    `Policy evidence: ${input.run.policy.evidenceRefs.join(', ')}`,
    '',
    'Environment variables:',
    contextVars.length === 0 ? '(none)' : contextVars,
    '',
    'Environment fragments:',
    fragments.length === 0 ? '(none)' : fragments,
    '',
    'Subquery returns:',
    returns.length === 0 ? '(none)' : returns,
    '',
    'Return one coherent answer. Cite public-safe refs only. Do not claim execution authority, payout authority, public-claim authority, or training-promotion authority from this run.',
  ].join('\n')
}

export const makeDefaultFrlmComposer = (): FrlmComposer => input =>
  Effect.succeed(buildFrlmCompositionPrompt(input))

export class FrlmConductor {
  constructor(private readonly dependencies: FrlmConductorDependencies) {}

  run(
    input: FrlmConductorRunInput,
  ): Effect.Effect<FrlmConductorRun, FrlmConductorError> {
    const dependencies = this.dependencies
    return Effect.gen(function* () {
      yield* validatePolicy(input)
      const planned = yield* dependencies.plan(input)
      const subQueries = yield* validateSubQueries(input, planned)
      const initEvent = event({
        at: input.requestedAt,
        index: 0,
        kind: 'Run.Init',
        refs: [
          FRLM_CONDUCTOR_SIGNATURE_REF,
          ...input.policy.blueprintSignatureRefs,
          ...input.policy.evidenceRefs,
        ],
        runId: input.runId,
        summary: `Initialized FRLM conductor run with ${subQueries.length} subqueries and quorum ${input.policy.quorum}.`,
      })
      const submitEvents = subQueries.map((subQuery, index) =>
        event({
          at: input.requestedAt,
          index: index + 1,
          kind: 'SubQuery.Submit',
          refs: [subQuery.signatureRef],
          runId: input.runId,
          subQueryId: subQuery.subQueryId,
          summary: `${subQuery.executor} leaf submitted for ${subQuery.purpose}.`,
        }),
      )

      const results = yield* Effect.forEach(
        subQueries,
        subQuery => executeSubQuery(dependencies.execute, input, subQuery),
        { concurrency: input.policy.maxParallelism },
      )
      const returnedResults = results.filter(result => result.status === 'returned')
      const returns = returnedResults.map(result => result.value)
      const returnEvents = results.map((result, index) =>
        event({
          at: input.requestedAt,
          index: submitEvents.length + index + 1,
          kind: 'SubQuery.Return',
          refs:
            result.status === 'returned'
              ? result.value.evidenceRefs
              : ['blocker.frlm_conductor.leaf_executor_failed'],
          runId: input.runId,
          subQueryId: result.query.subQueryId,
          summary:
            result.status === 'returned'
              ? `${result.query.subQueryId} returned ${result.value.output.length} chars.`
              : `${result.query.subQueryId} failed: ${result.reason}`,
        }),
      )

      const compositionPrompt = buildFrlmCompositionPrompt({
        returns,
        run: input,
        subQueries,
      })
      const quorumMet = returns.length >= input.policy.quorum
      const answer = quorumMet
        ? yield* dependencies.compose({ returns, run: input, subQueries })
        : null
      const status: FrlmConductorStatus = quorumMet
        ? 'completed'
        : 'blocked_quorum_not_met'
      const doneEvent = event({
        at: input.requestedAt,
        index: submitEvents.length + returnEvents.length + 1,
        kind: 'Run.Done',
        refs: unique([
          status === 'completed'
            ? 'evidence.frlm_conductor.quorum_met'
            : 'blocker.frlm_conductor.quorum_not_met',
          ...returns.flatMap(result => result.evidenceRefs),
        ]),
        runId: input.runId,
        summary:
          status === 'completed'
            ? `FRLM conductor completed with ${returns.length}/${subQueries.length} returns.`
            : `FRLM conductor blocked with ${returns.length}/${input.policy.quorum} quorum returns.`,
      })

      return {
        answer,
        compositionPrompt,
        events: [initEvent, ...submitEvents, ...returnEvents, doneEvent],
        policy: input.policy,
        returns,
        runId: input.runId,
        status,
        subQueries,
        usage: sumUsage(returns),
      }
    })
  }
}

export const makeFrlmConductor = (
  dependencies: FrlmConductorDependencies,
): FrlmConductor => new FrlmConductor(dependencies)

export const makeStaticFrlmPlanner =
  (subQueries: ReadonlyArray<FrlmSubQuery>): FrlmPlanner =>
  () =>
    Effect.succeed(subQueries)

export const defaultArtanisFrlmPolicy = (input?: {
  allowedExecutors?: ReadonlyArray<FrlmExecutorKind>
  evidenceRefs?: ReadonlyArray<string>
  maxSubQueries?: number
  quorum?: number
}): FrlmSchedulerPolicy => ({
  allowedExecutors: [...(input?.allowedExecutors ?? ['local', 'codex'])],
  blueprintSignatureRefs: [
    FRLM_CONDUCTOR_SIGNATURE_REF,
    RLM_LEAF_EXECUTOR_SIGNATURE_REF,
    FRLM_BLUEPRINT_EVIDENCE_ONLY_SIGNATURE_REF,
  ],
  budget: {
    maxInputTokens: 64_000,
    maxOutputTokens: 64_000,
    maxWallClockMs: 120_000,
  },
  evidenceRefs: [
    'evidence.autonomous_ops_v1.blueprint_signature_governance',
    ...(input?.evidenceRefs ?? []),
  ],
  maxDepth: 2,
  maxParallelism: 4,
  maxSubQueries: input?.maxSubQueries ?? 4,
  quorum: input?.quorum ?? 2,
})
