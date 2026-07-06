/**
 * CFG-9 (#8524): Durable Object namespace shims for the Cloud Run monolith.
 *
 * `makeUnavailableDurableObjectNamespace`: id/stub construction stays
 * synchronous (matching the DO API), and every call ON the stub rejects
 * with the typed `BindingUnavailableError` → 503 pattern. Used for the DO
 * classes that have no owned replacement yet (SYNC_ROOM,
 * AGENT_DEFINITION_SCHEDULER, EVENT_LEDGER_OWNER, and the MDK containers
 * until the owner-gated CFG-15 service URLs are flipped).
 *
 * (The former in-memory INFERENCE_DURABLE_STREAM shim was deleted when
 * CFG-6 landed the config-driven Postgres DurableStream backend.)
 */

import { BindingUnavailableError } from './binding-unavailable'

const unavailableStub = (binding: string): DurableObjectStub =>
  new Proxy({} as DurableObjectStub, {
    get(_target, property) {
      if (typeof property !== 'string' || property === 'then') {
        return undefined
      }
      if (property === 'id' || property === 'name') return undefined
      return (..._args: ReadonlyArray<unknown>) =>
        Promise.reject(
          new BindingUnavailableError({
            binding,
            operation: `stub.${property}`,
          }),
        )
    },
  })

export const makeUnavailableDurableObjectNamespace = (
  binding: string,
): DurableObjectNamespace => {
  const namespace = {
    get: (_id: unknown) => unavailableStub(binding),
    getByName: (_name: string) => unavailableStub(binding),
    idFromName: (name: string) =>
      ({ name, toString: () => `${binding}:${name}` }) as unknown,
    idFromString: (id: string) =>
      ({ toString: () => id }) as unknown,
    jurisdiction: () => namespace,
    newUniqueId: () =>
      ({ toString: () => `${binding}:${crypto.randomUUID()}` }) as unknown,
  }
  return namespace as unknown as DurableObjectNamespace
}
