import { FleetPublicRef, SyncScope, fleetRunScope } from "@openagentsinc/khala-sync"
import { Schema } from "effect"

import {
  MAX_SARAH_FLEET_PROJECTION_STATE_BYTES,
  SarahFleetProjectionReducerError,
  decodeSarahFleetProjectionState,
  type SarahFleetProjectionPersistence,
  type SarahFleetProjectionState,
} from "./fleet-sync-projection-store.ts"

export const SARAH_FLEET_BROWSER_STORAGE_PREFIX =
  "openagents.sarah.fleet_projection.v1:"

export type SarahFleetBrowserStorage = Readonly<{
  getItem: (key: string) => string | null
  setItem: (key: string, value: string) => void
  removeItem: (key: string) => void
}>

export type SarahFleetBrowserPersistence = SarahFleetProjectionPersistence &
  Readonly<{
    clear: (scope: typeof SyncScope.Type) => Promise<void>
  }>

const fail = (
  reason:
    | "invalid_scope"
    | "invalid_state"
    | "foreign_scope"
    | "persistence_failed",
): never => {
  throw new SarahFleetProjectionReducerError(reason)
}

const exactFleetScope = (raw: string): typeof SyncScope.Type => {
  const match =
    /^scope\.fleet_run\.([A-Za-z0-9](?:[A-Za-z0-9._:-]*[A-Za-z0-9])?)$/.exec(
      raw,
    )
  if (match?.[1] === undefined) return fail("invalid_scope")
  try {
    const runRef = Schema.decodeUnknownSync(FleetPublicRef)(match[1])
    const scope = Schema.decodeUnknownSync(SyncScope)(raw)
    if (fleetRunScope(runRef) !== scope) return fail("invalid_scope")
    return scope
  } catch (error) {
    if (error instanceof SarahFleetProjectionReducerError) throw error
    return fail("invalid_scope")
  }
}

const keyOf = (scope: typeof SyncScope.Type): string =>
  `${SARAH_FLEET_BROWSER_STORAGE_PREFIX}${scope}`

const byteLength = (value: string): number =>
  new TextEncoder().encode(value).byteLength

/**
 * Local, allowlisted persistence for Sarah's owner-safe fleet projection.
 * Only the strict projection-state schema is stored; no token, prompt, raw
 * event, command output, or arbitrary caller field crosses this boundary.
 */
export const makeSarahFleetBrowserPersistence = (
  storage?: SarahFleetBrowserStorage,
): SarahFleetBrowserPersistence => {
  let selected: SarahFleetBrowserStorage
  try {
    selected = storage ?? globalThis.localStorage
  } catch {
    return fail("persistence_failed")
  }
  if (selected === undefined) return fail("persistence_failed")
  return {
    load: async (rawScope) => {
      const scope = exactFleetScope(rawScope)
      let serialized: string | null
      try {
        serialized = selected.getItem(keyOf(scope))
      } catch {
        return fail("persistence_failed")
      }
      if (serialized === null) return null
      if (byteLength(serialized) > MAX_SARAH_FLEET_PROJECTION_STATE_BYTES) {
        return fail("invalid_state")
      }
      let raw: unknown
      try {
        raw = JSON.parse(serialized) as unknown
      } catch {
        return fail("invalid_state")
      }
      const state = decodeSarahFleetProjectionState(raw)
      if (state.scope !== scope) return fail("foreign_scope")
      return state
    },

    save: async (rawState: SarahFleetProjectionState) => {
      const state = decodeSarahFleetProjectionState(rawState)
      const scope = exactFleetScope(state.scope)
      const serialized = JSON.stringify(state)
      if (byteLength(serialized) > MAX_SARAH_FLEET_PROJECTION_STATE_BYTES) {
        return fail("invalid_state")
      }
      try {
        selected.setItem(keyOf(scope), serialized)
      } catch {
        return fail("persistence_failed")
      }
    },

    clear: async (rawScope) => {
      const scope = exactFleetScope(rawScope)
      try {
        selected.removeItem(keyOf(scope))
      } catch {
        return fail("persistence_failed")
      }
    },
  }
}
