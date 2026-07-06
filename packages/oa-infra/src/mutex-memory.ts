/**
 * In-memory Mutex backend — per-name FIFO promise chain, single-process
 * only. Reference implementation and default test Layer.
 */
import { Effect, Layer } from "effect"
import { Mutex, type MutexShape } from "./mutex.ts"

export const makeMemoryMutex = (): MutexShape => {
  /** Tail of the wait chain per name; resolved = lock free. */
  const tails = new Map<string, Promise<void>>()

  const acquire = (name: string): Promise<() => void> => {
    const previous = tails.get(name) ?? Promise.resolve()
    let release!: () => void
    const gate = new Promise<void>((resolve) => {
      release = resolve
    })
    tails.set(
      name,
      previous.then(() => gate),
    )
    return previous.then(() => release)
  }

  const withLock = <A, E, R>(name: string, effect: Effect.Effect<A, E, R>) =>
    Effect.acquireUseRelease(
      Effect.promise(() => acquire(name)),
      () => effect,
      (release) => Effect.sync(release),
    )

  return { withLock }
}

export const layerMemory = (): Layer.Layer<Mutex> => Layer.sync(Mutex, makeMemoryMutex)
