// Own an Effect resource Scope from an ordinary React component. React only
// signals mount/unmount; Effect remains the lifecycle authority. Strict Mode
// replay closes the first Scope before reacquiring.
//
// Ported verbatim from the vendored
// `apps/openagents.com/packages/effect-native-render-dom/src/react.ts`
// (`useEffectNativeScopedEffect`) when Effect Native was removed (#9325
// packet 3). The hook is plain `effect` + React — nothing about it belonged
// to the component/renderer layer that was deleted.

import { Effect, Exit, Fiber, Scope } from 'effect'
import { useEffect } from 'react'

export type ScopedEffectOptions = Readonly<{
  onError?: (cause: unknown) => void
}>

export const useScopedEffect = (
  makeEffect: () => Effect.Effect<unknown, unknown, Scope.Scope>,
  dependencies: ReadonlyArray<unknown>,
  options: ScopedEffectOptions = {},
): void => {
  useEffect(() => {
    const scope = Effect.runSync(Scope.make())
    const fiber = Effect.runFork(makeEffect().pipe(Scope.provide(scope)))
    Effect.runFork(
      Fiber.await(fiber).pipe(
        Effect.flatMap(exit =>
          exit._tag === 'Failure'
            ? Effect.sync(() => options.onError?.(exit.cause))
            : Effect.void,
        ),
      ),
    )
    return () => {
      Effect.runFork(Fiber.interrupt(fiber))
      Effect.runFork(Scope.close(scope, Exit.void))
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, dependencies)
}
