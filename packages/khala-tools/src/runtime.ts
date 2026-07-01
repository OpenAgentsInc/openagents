import { Context, Effect, Layer, Random } from "effect"

export type KhalaToolRuntimeServiceShape = {
  readonly currentTimeMillis: Effect.Effect<number, never>
  readonly eventId: (prefix: string) => Effect.Effect<string, never>
  readonly randomIdPart: (length: number) => Effect.Effect<string, never>
  readonly sleep: (ms: number) => Effect.Effect<void, never>
}

const randomAlphabet = "0123456789abcdefghijklmnopqrstuvwxyz"

export class KhalaToolRuntimeService extends Context.Service<
  KhalaToolRuntimeService,
  KhalaToolRuntimeServiceShape
>()("@openagentsinc/khala-tools/KhalaToolRuntimeService") {
  static readonly Default = Layer.succeed(KhalaToolRuntimeService, makeKhalaToolRuntimeService())
}

export const KhalaToolRuntimeLive = KhalaToolRuntimeService.Default

export function makeKhalaToolRuntimeService(): KhalaToolRuntimeServiceShape {
  return {
    currentTimeMillis: Effect.clockWith(clock => clock.currentTimeMillis),
    eventId: prefix =>
      Effect.gen(function* () {
        const now = yield* Effect.clockWith(clock => clock.currentTimeMillis)
        const suffix = yield* randomIdPart(8)
        return `${prefix}.${now.toString(36)}.${suffix}`
      }),
    randomIdPart,
    sleep: ms => Effect.sleep(`${Math.max(0, Math.trunc(ms))} millis`),
  }
}

export function makeDeterministicKhalaToolRuntimeService(input: {
  readonly nowMs?: number
  readonly seed?: string
} = {}): KhalaToolRuntimeServiceShape {
  let now = input.nowMs ?? 0
  let state = hashSeed(input.seed ?? "khala-tools")
  const next = (): number => {
    state = (Math.imul(state, 1664525) + 1013904223) >>> 0
    return state
  }
  return {
    currentTimeMillis: Effect.sync(() => now),
    eventId: prefix =>
      Effect.sync(() => {
        const suffix = Array.from({ length: 8 }, () => randomAlphabet[next() % randomAlphabet.length]).join("")
        return `${prefix}.${now.toString(36)}.${suffix}`
      }),
    randomIdPart: length =>
      Effect.sync(() => Array.from(
        { length: Math.max(0, Math.trunc(length)) },
        () => randomAlphabet[next() % randomAlphabet.length],
      ).join("")),
    sleep: ms =>
      Effect.sync(() => {
        now += Math.max(0, Math.trunc(ms))
      }),
  }
}

function randomIdPart(length: number): Effect.Effect<string, never> {
  return Effect.gen(function* () {
    const chars: string[] = []
    for (let index = 0; index < Math.max(0, Math.trunc(length)); index += 1) {
      const offset = yield* Random.nextIntBetween(0, randomAlphabet.length - 1)
      chars.push(randomAlphabet[offset] ?? "0")
    }
    return chars.join("")
  })
}

function hashSeed(seed: string): number {
  let hash = 2166136261
  for (const char of seed) {
    hash ^= char.charCodeAt(0)
    hash = Math.imul(hash, 16777619)
  }
  return hash >>> 0
}
