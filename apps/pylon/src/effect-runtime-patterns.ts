import { Context, Effect, Layer, Schedule } from "effect"

export type PylonEffectRuntimePolicyShape = {
  readonly gitOperationMaxRetries: number
  readonly gitOperationBaseDelayMs: number
  readonly gitOperationMaxDelayMs: number
  readonly durableObjectMaxRetries: number
  readonly externalHttpMaxRetries: number
  readonly walletAdjacentMaxRetries: number
  readonly publicProjectionMaxRetries: number
}

export const PYLON_EFFECT_RUNTIME_POLICY_DEFAULTS: PylonEffectRuntimePolicyShape = {
  durableObjectMaxRetries: 3,
  externalHttpMaxRetries: 3,
  gitOperationBaseDelayMs: 40,
  gitOperationMaxDelayMs: 1_500,
  gitOperationMaxRetries: 5,
  publicProjectionMaxRetries: 3,
  walletAdjacentMaxRetries: 2,
}

export class PylonEffectRuntimePolicy extends Context.Service<
  PylonEffectRuntimePolicy,
  PylonEffectRuntimePolicyShape
>()("PylonEffectRuntimePolicy") {
  static readonly Default = Layer.succeed(PylonEffectRuntimePolicy, PYLON_EFFECT_RUNTIME_POLICY_DEFAULTS)
}

export function boundedExponentialSchedule(input: {
  readonly baseDelayMs: number
  readonly maxDelayMs?: number
  readonly retries: number
}) {
  return Schedule.exponential(`${input.baseDelayMs} millis`).pipe(Schedule.both(Schedule.recurs(input.retries)))
}

export function gitOperationRetrySchedule(policy: PylonEffectRuntimePolicyShape = PYLON_EFFECT_RUNTIME_POLICY_DEFAULTS) {
  return boundedExponentialSchedule({
    baseDelayMs: policy.gitOperationBaseDelayMs,
    maxDelayMs: policy.gitOperationMaxDelayMs,
    retries: policy.gitOperationMaxRetries,
  })
}

export function externalHttpRetrySchedule(policy: PylonEffectRuntimePolicyShape = PYLON_EFFECT_RUNTIME_POLICY_DEFAULTS) {
  return boundedExponentialSchedule({
    baseDelayMs: 250,
    maxDelayMs: 5_000,
    retries: policy.externalHttpMaxRetries,
  })
}

export function durableObjectRetrySchedule(policy: PylonEffectRuntimePolicyShape = PYLON_EFFECT_RUNTIME_POLICY_DEFAULTS) {
  return boundedExponentialSchedule({
    baseDelayMs: 25,
    maxDelayMs: 1_000,
    retries: policy.durableObjectMaxRetries,
  })
}

export function walletAdjacentRetrySchedule(policy: PylonEffectRuntimePolicyShape = PYLON_EFFECT_RUNTIME_POLICY_DEFAULTS) {
  return boundedExponentialSchedule({
    baseDelayMs: 500,
    maxDelayMs: 10_000,
    retries: policy.walletAdjacentMaxRetries,
  })
}

export function publicProjectionRetrySchedule(policy: PylonEffectRuntimePolicyShape = PYLON_EFFECT_RUNTIME_POLICY_DEFAULTS) {
  return boundedExponentialSchedule({
    baseDelayMs: 100,
    maxDelayMs: 2_000,
    retries: policy.publicProjectionMaxRetries,
  })
}

export function scopedResource<A, E, R, R2>(
  acquire: Effect.Effect<A, E, R>,
  release: (resource: A) => Effect.Effect<unknown, never, R2>,
) {
  return Effect.acquireRelease(acquire, (resource) => release(resource))
}

export function withScopedResource<A, E, R, R2, B, E2, R3>(
  acquire: Effect.Effect<A, E, R>,
  release: (resource: A) => Effect.Effect<unknown, never, R2>,
  use: (resource: A) => Effect.Effect<B, E2, R3>,
) {
  return scopedResource(acquire, release).pipe(Effect.flatMap(use), Effect.scoped)
}
