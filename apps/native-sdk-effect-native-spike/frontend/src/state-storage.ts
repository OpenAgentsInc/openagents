import { Schema } from "@effect-native/core/effect";

import { fixtureSessions, initialSpikeState, type SpikeState } from "./program.ts";

const storageFormatVersion = 1;
const storageBytesLimit = 32 * 1024;

const StoredSpikeStateSchema = Schema.Struct({
  formatVersion: Schema.Literal(storageFormatVersion),
  state: Schema.Struct({
    workspace: Schema.Literals(["chat", "home", "settings"]),
    selectedSessionRef: Schema.NullOr(Schema.String),
    input: Schema.String,
    messages: Schema.Array(Schema.Struct({
      key: Schema.String,
      role: Schema.Literals(["user", "assistant", "system"]),
      text: Schema.String,
      timestamp: Schema.String,
    })),
    pending: Schema.Boolean,
    revision: Schema.Number,
  }),
});

const decodeStoredSpikeState = Schema.decodeUnknownSync(StoredSpikeStateSchema);

export interface SpikeStorage {
  readonly getItem: (key: string) => string | null;
  readonly setItem: (key: string, value: string) => void;
}

const boundedNamespace = (value: string): string =>
  /^[A-Za-z0-9._-]{1,80}$/u.test(value) ? value : "default";

export const spikeStorageNamespace = (url: string): string => {
  try {
    const parsed = new URL(url);
    const query = parsed.searchParams.get("assurance-run");
    const fragment = new URLSearchParams(parsed.hash.slice(1)).get("assurance-run");
    return boundedNamespace(query ?? fragment ?? "default");
  } catch {
    return "default";
  }
};

export const spikeStorageKey = (namespace: string): string =>
  `openagents.native-sdk-effect-native-spike.state.v1.${boundedNamespace(namespace)}`;

const stateIsBounded = (state: SpikeState): boolean =>
  Number.isSafeInteger(state.revision) &&
  state.revision >= 1 &&
  state.revision < Number.MAX_SAFE_INTEGER &&
  state.input.length <= 4_000 &&
  state.messages.length <= 200 &&
  state.messages.every((message) =>
    message.key.length <= 160 &&
    message.text.length <= 4_000 &&
    message.timestamp.length <= 80
  ) &&
  (state.selectedSessionRef === null || fixtureSessions.some((session) => session.ref === state.selectedSessionRef));

/**
 * Restores only the small Effect-owned fixture projection. A new revision on
 * each document boot lets the native mirror distinguish a real reload from a
 * stale projection while pending work always fails closed to stopped.
 */
export const restoreSpikeState = (storage: SpikeStorage, namespace: string): SpikeState => {
  const fallback = initialSpikeState();
  try {
    const bytes = storage.getItem(spikeStorageKey(namespace));
    if (bytes === null || new TextEncoder().encode(bytes).length > storageBytesLimit) return fallback;
    const decoded = decodeStoredSpikeState(JSON.parse(bytes)).state;
    if (!stateIsBounded(decoded)) return fallback;
    return { ...decoded, pending: false, revision: decoded.revision + 1 };
  } catch {
    return fallback;
  }
};

export const persistSpikeState = (
  storage: SpikeStorage,
  namespace: string,
  state: SpikeState,
): boolean => {
  if (!stateIsBounded(state)) return false;
  try {
    const bytes = JSON.stringify({ formatVersion: storageFormatVersion, state });
    if (new TextEncoder().encode(bytes).length > storageBytesLimit) return false;
    storage.setItem(spikeStorageKey(namespace), bytes);
    return true;
  } catch {
    return false;
  }
};
