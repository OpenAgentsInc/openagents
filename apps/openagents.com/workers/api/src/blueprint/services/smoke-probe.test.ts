import { Effect, Schema as S } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  BLUEPRINT_DEPLOYED_PROBE_PLAN,
  BLUEPRINT_NO_NETWORK_SMOKE_PLAN,
  BlueprintSmokeProbePlan as BlueprintSmokeProbePlanSchema,
  blueprintSmokeProbePlanIsSecretSafe,
  makeBlueprintSmokeProbeFakeLayer,
  runBlueprintSmokeProbePlan,
} from './smoke-probe'

describe('Blueprint smoke/probe discipline', () => {
  test('runs the no-network smoke plan through a fake Effect layer', async () => {
    const result = await Effect.runPromise(
      runBlueprintSmokeProbePlan(BLUEPRINT_NO_NETWORK_SMOKE_PLAN).pipe(
        Effect.provide(makeBlueprintSmokeProbeFakeLayer()),
      ),
    )

    expect(result).toMatchObject({
      id: 'blueprint_smoke_plan.no_network.v1',
      retainedFailureRefs: [],
      safeForLogs: true,
      status: 'passed',
    })
    expect(result.results).toHaveLength(1)
    expect(BLUEPRINT_NO_NETWORK_SMOKE_PLAN.probes[0]?.networkAllowed).toBe(false)
  })

  test('projects fake failures as retained failure refs', async () => {
    const result = await Effect.runPromise(
      runBlueprintSmokeProbePlan(BLUEPRINT_NO_NETWORK_SMOKE_PLAN).pipe(
        Effect.provide(
          makeBlueprintSmokeProbeFakeLayer({
            'blueprint_probe.fake.registry_projection': 'failed',
          }),
        ),
      ),
    )

    expect(result).toMatchObject({
      retainedFailureRefs: ['retained_failure.blueprint.registry_projection'],
      status: 'failed',
    })
    expect(result.results[0]?.evidenceRefs).toEqual([
      'retained_failure.blueprint.registry_projection',
    ])
  })

  test('keeps deployed Worker, D1, Resend, and runner probes secret-safe', () => {
    expect(
      S.decodeUnknownSync(BlueprintSmokeProbePlanSchema)(
        BLUEPRINT_DEPLOYED_PROBE_PLAN,
      ),
    ).toEqual(BLUEPRINT_DEPLOYED_PROBE_PLAN)
    expect(blueprintSmokeProbePlanIsSecretSafe(BLUEPRINT_DEPLOYED_PROBE_PLAN))
      .toBe(true)
    expect(BLUEPRINT_DEPLOYED_PROBE_PLAN.probes.map(probe => probe.target))
      .toEqual(['worker_http', 'd1', 'resend', 'runner'])
  })
})
