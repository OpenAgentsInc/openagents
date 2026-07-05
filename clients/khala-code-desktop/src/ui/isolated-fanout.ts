/**
 * Shared per-item isolation for "map an independent async operation over a
 * list of unrelated items" call sites (turn steering across concurrent
 * Codex turns, resolving multiple composer attachments, etc.).
 *
 * A bare `Promise.all(items.map(run))` rejects as soon as ANY item rejects,
 * discarding the already-resolved results of every sibling item. That
 * collapses independent per-item success/failure into one opaque error and
 * silently drops visibility into (or delivery for) the items that actually
 * succeeded — the same failure class documented in
 * `docs/2026-07-05-promise-all-cron-landmine-audit.md`.
 *
 * `settleFanout` isolates each item's outcome so callers can always see
 * exactly which items succeeded and which failed, even when some reject.
 */

export type FanoutOutcome<T, R> =
  | { readonly item: T; readonly ok: true; readonly value: R }
  | { readonly item: T; readonly ok: false; readonly error: string }

export const settleFanout = async <T, R>(
  items: readonly T[],
  run: (item: T) => Promise<R>,
): Promise<ReadonlyArray<FanoutOutcome<T, R>>> => {
  return Promise.all(items.map(async (item): Promise<FanoutOutcome<T, R>> => {
    try {
      const value = await run(item)
      return { item, ok: true, value }
    } catch (error) {
      return { item, ok: false, error: error instanceof Error ? error.message : String(error) }
    }
  }))
}
