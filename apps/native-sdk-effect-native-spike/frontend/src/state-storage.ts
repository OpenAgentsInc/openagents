import { Schema } from "@effect-native/core/effect"

import {
  fixtureSessions,
  isFixtureSessionRef,
  type NativeStartupState,
  type NativeWorkspace,
  type SpikeState,
} from "./program.ts"

const storageFormatVersion = 2
const storageBytesLimit = 4 * 1024

const StoredSpikeStateSchema = Schema.Struct({
  formatVersion: Schema.Literal(storageFormatVersion),
  state: Schema.Struct({
    revision: Schema.Number,
    acknowledgedNativeSequence: Schema.Number,
    workspace: Schema.Literals(["chat", "home", "settings"]),
    selectedSessionRef: Schema.NullOr(Schema.Literals([
      "session.parity",
      "session.renderer",
      "session.audit",
    ])),
  }),
})

const decodeStoredSpikeState = Schema.decodeUnknownSync(StoredSpikeStateSchema)

export interface SpikeStorage {
  readonly getItem: (key: string) => string | null
  readonly setItem: (key: string, value: string) => void
}

const boundedNamespace = (value: string): string =>
  /^[A-Za-z0-9._-]{1,80}$/u.test(value) ? value : "default"

export const spikeStorageNamespace = (url: string): string => {
  try {
    const parsed = new URL(url)
    const query = parsed.searchParams.get("assurance-run")
    const fragment = new URLSearchParams(parsed.hash.slice(1)).get("assurance-run")
    return boundedNamespace(query ?? fragment ?? "default")
  } catch {
    return "default"
  }
}

export const spikeStorageKey = (namespace: string): string =>
  `openagents.native-sdk-effect-native-spike.state.v2.${boundedNamespace(namespace)}`

const defaultStartupState = (): NativeStartupState => ({
  revision: 1,
  acknowledgedNativeSequence: 0,
  workspace: "chat",
  selectedSessionRef: fixtureSessions[0].ref,
})

const stateIsBounded = (state: NativeStartupState): boolean =>
  Number.isSafeInteger(state.revision) &&
  state.revision >= 1 &&
  state.revision < Number.MAX_SAFE_INTEGER &&
  Number.isSafeInteger(state.acknowledgedNativeSequence) &&
  state.acknowledgedNativeSequence >= 0 &&
  (state.selectedSessionRef === null || isFixtureSessionRef(state.selectedSessionRef))

/** Restore only the bounded rail projection, never the evolving Desktop state graph. */
export const restoreSpikeState = (storage: SpikeStorage, namespace: string): NativeStartupState => {
  const fallback = defaultStartupState()
  try {
    const bytes = storage.getItem(spikeStorageKey(namespace))
    if (bytes === null || new TextEncoder().encode(bytes).length > storageBytesLimit) return fallback
    const decoded = decodeStoredSpikeState(JSON.parse(bytes)).state
    if (!stateIsBounded(decoded)) return fallback
    return { ...decoded, revision: decoded.revision + 1 }
  } catch {
    return fallback
  }
}

const persistedWorkspace = (workspace: SpikeState["workspace"]): NativeWorkspace | null =>
  workspace === "chat" || workspace === "home" || workspace === "settings" ? workspace : null

export const persistSpikeState = (
  storage: SpikeStorage,
  namespace: string,
  state: SpikeState,
  revision: number,
  acknowledgedNativeSequence = 0,
): boolean => {
  const workspace = persistedWorkspace(state.workspace)
  const candidate: NativeStartupState = {
    revision,
    acknowledgedNativeSequence,
    workspace: workspace ?? "chat",
    selectedSessionRef: isFixtureSessionRef(state.activeThreadId) ? state.activeThreadId : null,
  }
  if (workspace === null || !stateIsBounded(candidate)) return false
  try {
    const bytes = JSON.stringify({ formatVersion: storageFormatVersion, state: candidate })
    if (new TextEncoder().encode(bytes).length > storageBytesLimit) return false
    storage.setItem(spikeStorageKey(namespace), bytes)
    return true
  } catch {
    return false
  }
}
