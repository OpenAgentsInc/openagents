import { Context, Effect, Layer, Schema as S } from 'effect'

export const BlueprintSmokeProbeTarget = S.Literals([
  'd1',
  'resend',
  'runner',
  'worker_http',
])
export type BlueprintSmokeProbeTarget = typeof BlueprintSmokeProbeTarget.Type

export const BlueprintSmokeProbeMode = S.Literals([
  'deployed',
  'no_network_fake',
])
export type BlueprintSmokeProbeMode = typeof BlueprintSmokeProbeMode.Type

export const BlueprintSmokeProbeSecretPolicy = S.Literals([
  'redacted_refs_only',
])
export type BlueprintSmokeProbeSecretPolicy =
  typeof BlueprintSmokeProbeSecretPolicy.Type

export const BlueprintSmokeProbeStatus = S.Literals([
  'failed',
  'passed',
  'skipped',
])
export type BlueprintSmokeProbeStatus = typeof BlueprintSmokeProbeStatus.Type

export const BlueprintSmokeProbeSpec = S.Struct({
  descriptionRef: S.String,
  evidenceRef: S.String,
  id: S.String,
  mode: BlueprintSmokeProbeMode,
  networkAllowed: S.Boolean,
  retainedFailureRef: S.String,
  secretPolicy: BlueprintSmokeProbeSecretPolicy,
  target: BlueprintSmokeProbeTarget,
})
export type BlueprintSmokeProbeSpec = typeof BlueprintSmokeProbeSpec.Type

export const BlueprintSmokeProbeResult = S.Struct({
  evidenceRefs: S.Array(S.String),
  id: S.String,
  retainedFailureRefs: S.Array(S.String),
  safeForLogs: S.Boolean,
  status: BlueprintSmokeProbeStatus,
  target: BlueprintSmokeProbeTarget,
})
export type BlueprintSmokeProbeResult = typeof BlueprintSmokeProbeResult.Type

export const BlueprintSmokeProbePlan = S.Struct({
  id: S.String,
  probes: S.Array(BlueprintSmokeProbeSpec),
  safeForLogs: S.Boolean,
})
export type BlueprintSmokeProbePlan = typeof BlueprintSmokeProbePlan.Type

export const BlueprintSmokeProbePlanResult = S.Struct({
  evidenceRefs: S.Array(S.String),
  id: S.String,
  retainedFailureRefs: S.Array(S.String),
  results: S.Array(BlueprintSmokeProbeResult),
  safeForLogs: S.Boolean,
  status: BlueprintSmokeProbeStatus,
})
export type BlueprintSmokeProbePlanResult =
  typeof BlueprintSmokeProbePlanResult.Type

export class BlueprintSmokeProbeFailure extends S.TaggedErrorClass<BlueprintSmokeProbeFailure>()(
  'BlueprintSmokeProbeFailure',
  {
    id: S.String,
    reasonRef: S.String,
    target: BlueprintSmokeProbeTarget,
  },
) {}

export type BlueprintSmokeProbeExecutorShape = Readonly<{
  runProbe: (
    spec: BlueprintSmokeProbeSpec,
  ) => Effect.Effect<BlueprintSmokeProbeResult, BlueprintSmokeProbeFailure>
}>

export class BlueprintSmokeProbeExecutor extends Context.Service<
  BlueprintSmokeProbeExecutor,
  BlueprintSmokeProbeExecutorShape
>()('@openagentsinc/autopilot-omega/BlueprintSmokeProbeExecutor') {}

export const BLUEPRINT_NO_NETWORK_SMOKE_PLAN: BlueprintSmokeProbePlan = {
  id: 'blueprint_smoke_plan.no_network.v1',
  probes: [
    {
      descriptionRef: 'probe.blueprint.registry_projection.decode',
      evidenceRef: 'evidence.blueprint.registry_projection.test',
      id: 'blueprint_probe.fake.registry_projection',
      mode: 'no_network_fake',
      networkAllowed: false,
      retainedFailureRef: 'retained_failure.blueprint.registry_projection',
      secretPolicy: 'redacted_refs_only',
      target: 'worker_http',
    },
  ],
  safeForLogs: true,
}

export const BLUEPRINT_DEPLOYED_PROBE_PLAN: BlueprintSmokeProbePlan = {
  id: 'blueprint_probe_plan.deployed.v1',
  probes: [
    {
      descriptionRef: 'probe.worker.session_endpoint',
      evidenceRef: 'evidence.worker.session_endpoint_smoke',
      id: 'blueprint_probe.deployed.worker_http.session',
      mode: 'deployed',
      networkAllowed: true,
      retainedFailureRef: 'retained_failure.worker.session_endpoint',
      secretPolicy: 'redacted_refs_only',
      target: 'worker_http',
    },
    {
      descriptionRef: 'probe.d1.program_run_repository',
      evidenceRef: 'evidence.d1.program_run_repository_smoke',
      id: 'blueprint_probe.deployed.d1.program_run_repository',
      mode: 'deployed',
      networkAllowed: true,
      retainedFailureRef: 'retained_failure.d1.program_run_repository',
      secretPolicy: 'redacted_refs_only',
      target: 'd1',
    },
    {
      descriptionRef: 'probe.resend.review_ready_dry_run',
      evidenceRef: 'evidence.resend.review_ready_dry_run_smoke',
      id: 'blueprint_probe.deployed.resend.review_ready_dry_run',
      mode: 'deployed',
      networkAllowed: true,
      retainedFailureRef: 'retained_failure.resend.review_ready_dry_run',
      secretPolicy: 'redacted_refs_only',
      target: 'resend',
    },
    {
      descriptionRef: 'probe.runner.dispatch_dry_run',
      evidenceRef: 'evidence.runner.dispatch_dry_run_smoke',
      id: 'blueprint_probe.deployed.runner.dispatch_dry_run',
      mode: 'deployed',
      networkAllowed: true,
      retainedFailureRef: 'retained_failure.runner.dispatch_dry_run',
      secretPolicy: 'redacted_refs_only',
      target: 'runner',
    },
  ],
  safeForLogs: true,
}

const uniqueRefs = (refs: ReadonlyArray<string>): ReadonlyArray<string> => [
  ...new Set(refs),
]

export const blueprintSmokeProbePlanIsSecretSafe = (
  plan: BlueprintSmokeProbePlan,
): boolean =>
  plan.safeForLogs &&
  plan.probes.every(
    probe =>
      probe.secretPolicy === 'redacted_refs_only' &&
      !probe.id.toLowerCase().includes('secret') &&
      !probe.evidenceRef.toLowerCase().includes('secret') &&
      !probe.retainedFailureRef.toLowerCase().includes('secret'),
  )

const resultFromFailure = (
  spec: BlueprintSmokeProbeSpec,
  failure: BlueprintSmokeProbeFailure,
): BlueprintSmokeProbeResult => ({
  evidenceRefs: [failure.reasonRef],
  id: spec.id,
  retainedFailureRefs: [spec.retainedFailureRef],
  safeForLogs: true,
  status: 'failed',
  target: spec.target,
})

export const runBlueprintSmokeProbePlan = (
  plan: BlueprintSmokeProbePlan,
): Effect.Effect<BlueprintSmokeProbePlanResult, never, BlueprintSmokeProbeExecutor> =>
  Effect.gen(function* () {
    const executor = yield* BlueprintSmokeProbeExecutor
    const results = yield* Effect.forEach(plan.probes, spec =>
      executor.runProbe(spec).pipe(
        Effect.catchTag('BlueprintSmokeProbeFailure', failure =>
          Effect.succeed(resultFromFailure(spec, failure)),
        ),
      ),
    )
    const evidenceRefs = uniqueRefs(results.flatMap(result => result.evidenceRefs))
    const retainedFailureRefs = uniqueRefs(
      results.flatMap(result => result.retainedFailureRefs),
    )

    return {
      evidenceRefs,
      id: plan.id,
      retainedFailureRefs,
      results,
      safeForLogs: plan.safeForLogs && results.every(result => result.safeForLogs),
      status: results.some(result => result.status === 'failed')
        ? 'failed'
        : 'passed',
    }
  })

export const makeBlueprintSmokeProbeFakeLayer = (
  outcomes: Readonly<Record<string, BlueprintSmokeProbeStatus>> = {},
) =>
  Layer.succeed(BlueprintSmokeProbeExecutor, {
    runProbe: spec => {
      const status = outcomes[spec.id] ?? 'passed'

      if (status === 'failed') {
        return Effect.fail(
          new BlueprintSmokeProbeFailure({
            id: spec.id,
            reasonRef: spec.retainedFailureRef,
            target: spec.target,
          }),
        )
      }

      return Effect.succeed({
        evidenceRefs: [spec.evidenceRef],
        id: spec.id,
        retainedFailureRefs: [],
        safeForLogs: true,
        status,
        target: spec.target,
      })
    },
  })
