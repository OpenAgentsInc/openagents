/**
 * Single-writer tab election over the Web Locks API (KS-5.4, the Notion
 * WASM-SQLite pattern): every tab requests one exclusive named lock and
 * HOLDS it for its lifetime once granted. The browser queues contenders
 * FIFO, so exactly one tab is the writer at any moment; when the writer
 * tab dies (crash, close, navigation) its lock is released by the browser
 * and the next queued tab is elected automatically — no heartbeats.
 *
 * In v1 all store operations route through the single storage worker
 * that owns the one `opfs-sahpool` connection, so the election is
 * bookkeeping plus the seam for the v2 dedicated-worker fallback (per-tab
 * workers, where the elected tab's worker becomes the writer) — see the
 * package README's read-scaling follow-up.
 *
 * `LockManagerLike` is structural (no DOM lib): production passes
 * `navigator.locks`; tests pass a fake.
 */

export interface LockManagerLike {
  readonly request: (
    name: string,
    options: { readonly mode: "exclusive"; readonly signal?: AbortSignal },
    callback: (lock: unknown) => Promise<unknown> | unknown,
  ) => Promise<unknown>
}

export interface WriterElection {
  /**
   * Resolves `true` when this tab acquires the writer lock; `false` if
   * {@link WriterElection.release} ran (or the request aborted) first.
   */
  readonly becameWriter: Promise<boolean>
  /** Whether this tab currently holds the writer lock. */
  readonly isWriter: () => boolean
  /**
   * Give up the held lock (electing the next queued tab) or withdraw the
   * pending request. Idempotent.
   */
  readonly release: () => void
}

export const electWriter = (
  locks: LockManagerLike,
  lockName: string,
): WriterElection => {
  let winner = false
  let released = false
  let releaseHeld: (() => void) | null = null
  const controller = new AbortController()

  let resolveBecameWriter!: (value: boolean) => void
  const becameWriter = new Promise<boolean>((resolve) => {
    resolveBecameWriter = resolve
  })

  void Promise.resolve(
    locks.request(
      lockName,
      { mode: "exclusive", signal: controller.signal },
      (lock) => {
        if (released || lock === null) {
          resolveBecameWriter(false)
          return undefined // hand the lock straight back
        }
        winner = true
        resolveBecameWriter(true)
        // Hold the lock until release(): the returned promise's lifetime
        // IS the lock's lifetime (Web Locks contract).
        return new Promise<void>((resolve) => {
          releaseHeld = resolve
        })
      },
    ),
  ).catch(() => {
    // Aborted before grant (or the lock manager failed): not the writer.
    resolveBecameWriter(false)
  })

  return {
    becameWriter,
    isWriter: () => winner && !released,
    release: () => {
      if (released) return
      released = true
      if (releaseHeld !== null) {
        releaseHeld()
      } else {
        controller.abort()
      }
      resolveBecameWriter(false) // no-op if already resolved true
    },
  }
}
