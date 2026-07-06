/**
 * CFG-9 (#8524): `ExecutionContext` for the Cloud Run monolith.
 *
 * `ctx.waitUntil` on Workers extends the invocation lifetime; on a long-lived
 * Bun server we instead track every background promise in a shared set so
 * that (a) rejections are logged instead of crashing the process and (b) a
 * graceful shutdown can await the stragglers before exiting.
 */

export type BackgroundTasks = Readonly<{
  add: (promise: Promise<unknown>) => void
  /** Number of currently-tracked background promises. */
  size: () => number
  /** Await every tracked promise (used on SIGTERM drain and in tests). */
  drain: () => Promise<void>
}>

export const makeBackgroundTasks = (
  log: (event: string, error: unknown) => void = (event, error) =>
    console.error(`[cloudrun] ${event}`, error),
): BackgroundTasks => {
  const pending = new Set<Promise<unknown>>()

  return {
    add: promise => {
      const tracked = promise.then(
        () => undefined,
        error => {
          log('background_task_failed', error)
        },
      )
      pending.add(tracked)
      void tracked.finally(() => {
        pending.delete(tracked)
      })
    },
    drain: async () => {
      // Tasks may enqueue further tasks; loop until quiescent.
      while (pending.size > 0) {
        await Promise.allSettled([...pending])
      }
    },
    size: () => pending.size,
  }
}

export const makeExecutionContext = (
  tasks: BackgroundTasks,
): ExecutionContext =>
  ({
    passThroughOnException: () => undefined,
    props: {},
    waitUntil: (promise: Promise<unknown>) => {
      tasks.add(promise)
    },
  }) as ExecutionContext
