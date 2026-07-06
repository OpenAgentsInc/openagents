/**
 * CFG-9 (#8524): typed degrade shim for Worker bindings that have no owned
 * replacement yet on the Cloud Run monolith.
 *
 * Same doctrine as the CFG-8 `artifacts-binding.ts` pattern (#8516): absence
 * degrades PER-CALL as an ordinary async rejection with a typed error, never
 * a synchronous crash at store-construction/wiring time. Routes that are
 * hard-bound to a dead binding surface a typed `service_unavailable` 503
 * through `responseForBindingUnavailable` in the server entry.
 */

import { Data } from 'effect'

export class BindingUnavailableError extends Data.TaggedError(
  'BindingUnavailableError',
)<{
  readonly binding: string
  readonly operation: string
}> {
  override get message(): string {
    return `${this.binding} binding is not available on the Cloud Run monolith (CFG-9, #8524): ${this.operation}`
  }
}

export const isBindingUnavailableError = (
  error: unknown,
): error is BindingUnavailableError =>
  typeof error === 'object' &&
  error !== null &&
  '_tag' in error &&
  (error as { _tag?: unknown })._tag === 'BindingUnavailableError'

/**
 * A proxy whose every string-keyed member is an async function rejecting with
 * `BindingUnavailableError`. `then` (and every symbol-keyed access) resolves
 * to `undefined` so the proxy is never treated as a thenable.
 */
export const unavailableBinding = <T>(binding: string): T =>
  new Proxy(
    {},
    {
      get(_target, property) {
        if (typeof property !== 'string' || property === 'then') {
          return undefined
        }

        return (..._args: ReadonlyArray<unknown>) =>
          Promise.reject(
            new BindingUnavailableError({ binding, operation: property }),
          )
      },
    },
  ) as T

/** Typed 503 body for routes that surfaced a dead binding. */
export const responseForBindingUnavailable = (
  error: BindingUnavailableError,
): Response =>
  Response.json(
    {
      error: 'service_unavailable',
      binding: error.binding,
      operation: error.operation,
      detail:
        'This route depends on a backend that has not been migrated to the Cloud Run monolith yet (CFG-9, #8524).',
    },
    { status: 503, headers: { 'cache-control': 'no-store' } },
  )
