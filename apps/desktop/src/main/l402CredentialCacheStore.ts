import {
  decodeL402CredentialSync,
  type CredentialCacheLookup,
  type L402Credential,
} from "@openagentsinc/lightning-effect";
import { Effect } from "effect";

import { DesktopSecureStorageService, type DesktopSecureStorageApi } from "./desktopSecureStorage";

type CacheKey = string;

type CacheEntry = Readonly<{
  readonly credential: L402Credential;
  readonly expiresAtMs: number;
}>;

type PersistedStore = Readonly<{
  readonly version: 1;
  readonly entries: Record<CacheKey, CacheEntry>;
}>;

const STORE_SECRET_KEY = "lightning.l402.credential-cache.v1";
const STORE_VERSION = 1 as const;

const emptyStore = (): PersistedStore => ({
  version: STORE_VERSION,
  entries: {},
});

const toCacheKey = (host: string, scope: string): CacheKey =>
  `${host.trim().toLowerCase()}::${scope.trim().toLowerCase()}`;

const parseStore = (raw: string): PersistedStore => {
  const parsed = JSON.parse(raw) as unknown;
  if (!parsed || typeof parsed !== "object") throw new Error("invalid_store_shape");
  const record = parsed as { readonly version?: unknown; readonly entries?: unknown };
  if (record.version !== STORE_VERSION) throw new Error("invalid_store_version");
  if (!record.entries || typeof record.entries !== "object") throw new Error("invalid_store_entries");

  const entries: Record<string, CacheEntry> = {};
  for (const [key, value] of Object.entries(record.entries as Record<string, unknown>)) {
    if (!value || typeof value !== "object") continue;
    const entry = value as { readonly credential?: unknown; readonly expiresAtMs?: unknown };
    if (typeof entry.expiresAtMs !== "number" || !Number.isFinite(entry.expiresAtMs)) continue;

    try {
      const credential = decodeL402CredentialSync(entry.credential);
      entries[key] = {
        credential,
        expiresAtMs: Math.max(0, Math.floor(entry.expiresAtMs)),
      };
    } catch {
      // Skip invalid entries; next write will compact.
      continue;
    }
  }

  return {
    version: STORE_VERSION,
    entries,
  };
};

const loadStore = (storage: DesktopSecureStorageApi): Effect.Effect<PersistedStore> =>
  storage.getSecret(STORE_SECRET_KEY).pipe(
    Effect.map((raw) => {
      if (!raw) return emptyStore();
      try {
        return parseStore(raw);
      } catch {
        return emptyStore();
      }
    }),
    Effect.catchAll(() => Effect.succeed(emptyStore())),
  );

const persistStore = (
  storage: DesktopSecureStorageApi,
  store: PersistedStore,
): Effect.Effect<void> =>
  storage
    .setSecret(STORE_SECRET_KEY, `${JSON.stringify(store)}\n`)
    .pipe(Effect.catchAll(() => Effect.void));

export type L402CredentialCacheStoreApi = Readonly<{
  readonly getByHost: (
    host: string,
    scope: string,
    nowMs: number,
  ) => Effect.Effect<CredentialCacheLookup>;
  readonly putByHost: (
    host: string,
    scope: string,
    credential: L402Credential,
    options?: { readonly ttlMs?: number },
  ) => Effect.Effect<void>;
  readonly markInvalid: (host: string, scope: string) => Effect.Effect<void>;
  readonly clearHost: (host: string, scope: string) => Effect.Effect<void>;
}>;

export const makeL402CredentialCacheStore = Effect.gen(function* () {
  const storage = yield* DesktopSecureStorageService;
  const defaultTtlMs = 10 * 60 * 1000;

  const getByHost: L402CredentialCacheStoreApi["getByHost"] = (host, scope, nowMs) =>
    loadStore(storage).pipe(
      Effect.map((store) => {
        const entry = store.entries[toCacheKey(host, scope)];
        if (!entry) return { _tag: "miss" as const };
        if (nowMs >= entry.expiresAtMs) {
          return { _tag: "stale" as const, credential: entry.credential };
        }
        return { _tag: "hit" as const, credential: entry.credential };
      }),
    );

  const putByHost: L402CredentialCacheStoreApi["putByHost"] = (host, scope, credential, options) =>
    loadStore(storage).pipe(
      Effect.flatMap((store) => {
        // IPC callers may provide unknown/untrusted shapes; validate at runtime.
        try {
          decodeL402CredentialSync(credential);
        } catch {
          return Effect.void;
        }

        const ttlMs = Math.max(0, Math.floor(options?.ttlMs ?? defaultTtlMs));
        const expiresAtMs = credential.issuedAtMs + ttlMs;
        const nextEntries = {
          ...store.entries,
          [toCacheKey(host, scope)]: {
            credential,
            expiresAtMs,
          },
        };
        return persistStore(storage, {
          version: STORE_VERSION,
          entries: nextEntries,
        });
      }),
    );

  const removeByHost = (host: string, scope: string) =>
    loadStore(storage).pipe(
      Effect.flatMap((store) => {
        const key = toCacheKey(host, scope);
        if (!(key in store.entries)) return Effect.void;
        const nextEntries = { ...store.entries };
        delete nextEntries[key];
        return persistStore(storage, {
          version: STORE_VERSION,
          entries: nextEntries,
        });
      }),
    );

  return {
    getByHost,
    putByHost,
    markInvalid: removeByHost,
    clearHost: removeByHost,
  } satisfies L402CredentialCacheStoreApi;
});
