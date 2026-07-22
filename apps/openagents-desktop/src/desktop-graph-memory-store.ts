import {
  GraphMemoryPersistenceError,
  GraphMemoryStore,
  makeGraphMemoryStore,
  type GraphMemoryScope,
  type GraphMemoryStateStore,
} from "@openagentsinc/agent-experience-memory";
import { Effect, Layer } from "effect";

import {
  DesktopGraphMemoryPersistenceError,
  openDesktopGraphMemoryPersistence,
  type DesktopGraphMemoryPersistence,
} from "./desktop-graph-memory-persistence.js";
import type { SafeStorageLike } from "./desktop-session-vault.js";
import type { SqliteDatabase } from "@openagentsinc/sqlite-runtime";

const persistenceError = (operation: string, error: unknown): GraphMemoryPersistenceError =>
  new GraphMemoryPersistenceError({
    operation,
    reason:
      error instanceof DesktopGraphMemoryPersistenceError && error.reason === "invalid_state"
        ? "invalid_state"
        : "unavailable",
    detailSafe:
      error instanceof DesktopGraphMemoryPersistenceError
        ? error.message
        : "Desktop graph memory persistence is unavailable.",
  });

const persistenceScope = (scope: GraphMemoryScope) => ({
  ownerScope: scope.owner,
  projectScope: scope.project,
});

const revisionOf = (value: unknown): number => {
  const revision =
    typeof value === "object" && value !== null && "revision" in value ? value.revision : undefined;
  if (typeof revision !== "number" || !Number.isSafeInteger(revision) || revision < 0) {
    throw new DesktopGraphMemoryPersistenceError(
      "invalid_state",
      "The graph memory envelope revision is invalid.",
    );
  }
  return revision;
};

const stateStoreFromPersistence = (
  persistence: DesktopGraphMemoryPersistence,
): GraphMemoryStateStore => ({
  enabled: persistence.enabled,
  load: (scope) =>
    Effect.try({
      try: () => {
        const stored = persistence.load(persistenceScope(scope));
        if (stored === null) return null;
        const parsed = JSON.parse(stored.payload) as unknown;
        if (revisionOf(parsed) !== stored.revision) {
          throw new DesktopGraphMemoryPersistenceError(
            "invalid_state",
            "The sealed graph memory revision does not match its envelope.",
          );
        }
        return parsed;
      },
      catch: (error) => persistenceError("GraphMemoryStateStore.load", error),
    }),
  compareAndSet: (scope, expectedRevision, next) =>
    Effect.try({
      try: () =>
        persistence.compareAndSet(persistenceScope(scope), expectedRevision, {
          revision: revisionOf(next),
          payload: JSON.stringify(next),
        }),
      catch: (error) => persistenceError("GraphMemoryStateStore.compareAndSet", error),
    }),
  reads: Effect.sync(() => persistence.stats().reads),
  writes: Effect.sync(() => persistence.stats().writes),
});

export type DesktopGraphMemoryStore = Readonly<{
  persistence: DesktopGraphMemoryPersistence;
  stateStore: GraphMemoryStateStore;
  layer: Layer.Layer<GraphMemoryStore>;
  close: () => void;
}>;

/**
 * Compose the portable graph-memory lifecycle with the encrypted Desktop
 * persistence adapter. The portable service owns all graph semantics. This
 * host adapter owns only private storage and compare-and-set durability.
 */
export const openDesktopGraphMemoryStore = (
  input: Readonly<{
    enabled: boolean;
    databasePath: string;
    safeStorage: SafeStorageLike;
    openDatabase?: (databasePath: string) => SqliteDatabase;
  }>,
): DesktopGraphMemoryStore => {
  const persistence = openDesktopGraphMemoryPersistence(input);
  const stateStore = stateStoreFromPersistence(persistence);
  return {
    persistence,
    stateStore,
    layer: Layer.effect(GraphMemoryStore, makeGraphMemoryStore(stateStore)),
    close: persistence.close,
  };
};
