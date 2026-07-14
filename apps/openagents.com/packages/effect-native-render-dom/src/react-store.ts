import { Deferred, Effect, Scope, Stream } from "effect"
import type { View } from "@effect-native/core"

export type ReactViewSnapshot =
  | { readonly status: "loading"; readonly revision: 0 }
  | { readonly status: "ready"; readonly revision: number; readonly view: View }
  | { readonly status: "failed"; readonly revision: number; readonly message: string }

export interface ReactViewStore {
  readonly getSnapshot: () => ReactViewSnapshot
  readonly getServerSnapshot: () => ReactViewSnapshot
  readonly subscribe: (listener: () => void) => () => void
  readonly firstCommit: Effect.Effect<void>
  readonly activeSubscribers: () => number
}

const loadingSnapshot: ReactViewSnapshot = { status: "loading", revision: 0 }

export const makeReactViewStore = (
  viewStream: Stream.Stream<View>
): Effect.Effect<ReactViewStore, never, Scope.Scope> =>
  Effect.gen(function*() {
    let snapshot: ReactViewSnapshot = loadingSnapshot
    let revision = 0
    const listeners = new Set<() => void>()
    const firstCommit = yield* Deferred.make<void>()
    yield* Effect.addFinalizer(() => Effect.sync(() => listeners.clear()))
    const publish = (next: ReactViewSnapshot): void => {
      snapshot = next
      for (const listener of listeners) listener()
      Effect.runFork(Deferred.succeed(firstCommit, undefined))
    }
    yield* viewStream.pipe(
      Stream.runForEach((view) => Effect.sync(() => {
        revision += 1
        publish({ status: "ready", revision, view })
      })),
      Effect.catchCause(() => Effect.sync(() => {
        revision += 1
        publish({ status: "failed", revision, message: "Effect Native view stream failed" })
      })),
      Effect.forkScoped
    )
    return {
      getSnapshot: () => snapshot,
      getServerSnapshot: () => snapshot,
      subscribe: (listener) => {
        listeners.add(listener)
        return () => listeners.delete(listener)
      },
      firstCommit: Deferred.await(firstCommit),
      activeSubscribers: () => listeners.size
    }
  })
