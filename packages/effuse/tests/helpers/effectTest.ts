import { it } from "@effect/vitest"
import { Effect } from "effect"
import type { TestOptions } from "vitest"

import { DomServiceLive, DomServiceTag, type DomService } from "../../src/index.ts"

export const itLivePromise = (
  name: string,
  fn: () => Promise<unknown>,
  timeout?: number | TestOptions,
): void =>
  it.live(
    name,
    () =>
      Effect.promise(fn).pipe(
        Effect.asVoid,
      ),
    timeout,
  )

export const withDom =
  (dom: DomService = DomServiceLive) =>
  <A, E, R>(effect: Effect.Effect<A, E, R>) =>
    effect.pipe(Effect.provideService(DomServiceTag, dom))
