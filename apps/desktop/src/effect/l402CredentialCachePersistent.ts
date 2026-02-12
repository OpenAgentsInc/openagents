import {
  CredentialCacheService,
  type CredentialCacheLookup,
  type L402Credential,
} from "@openagentsinc/lightning-effect";
import { Effect, Layer } from "effect";

const bridge = () => {
  if (typeof window === "undefined") return undefined;
  return window.openAgentsDesktop?.l402CredentialCache;
};

const normalizeCredential = (value: unknown): L402Credential | null => {
  if (!value || typeof value !== "object") return null;
  const record = value as Record<string, unknown>;
  if (typeof record.host !== "string" || record.host.trim().length === 0) return null;
  if (typeof record.macaroon !== "string" || record.macaroon.trim().length === 0) return null;
  if (typeof record.preimageHex !== "string" || record.preimageHex.trim().length === 0) return null;
  if (typeof record.amountMsats !== "number" || !Number.isFinite(record.amountMsats)) return null;
  if (typeof record.issuedAtMs !== "number" || !Number.isFinite(record.issuedAtMs)) return null;

  const scope =
    typeof record.scope === "string" && record.scope.trim().length > 0 ? record.scope.trim() : undefined;

  const base: Omit<L402Credential, "scope"> = {
    host: record.host.trim().toLowerCase(),
    macaroon: record.macaroon.trim(),
    preimageHex: record.preimageHex.trim().toLowerCase(),
    amountMsats: Math.max(0, Math.floor(record.amountMsats)),
    issuedAtMs: Math.max(0, Math.floor(record.issuedAtMs)),
  };

  if (scope) {
    return {
      ...base,
      scope,
    };
  }

  return base;
};

const normalizeLookup = (value: unknown): CredentialCacheLookup => {
  if (!value || typeof value !== "object") return { _tag: "miss" as const };
  const record = value as Record<string, unknown>;
  if (record._tag === "miss") return { _tag: "miss" as const };
  if (record._tag === "hit") {
    const credential = normalizeCredential(record.credential);
    if (!credential) return { _tag: "miss" as const };
    return { _tag: "hit" as const, credential };
  }
  if (record._tag === "stale") {
    const credential = normalizeCredential(record.credential);
    if (!credential) return { _tag: "miss" as const };
    return { _tag: "stale" as const, credential };
  }
  return { _tag: "miss" as const };
};

export const CredentialCacheDesktopPersistentLayer = Layer.succeed(
  CredentialCacheService,
  CredentialCacheService.of({
    getByHost: (host, scope, nowMs) =>
      Effect.promise(async () => {
        const api = bridge();
        if (!api) return { _tag: "miss" as const };
        return normalizeLookup(
          await api.getByHost({
            host,
            scope,
            nowMs,
          }),
        );
      }).pipe(Effect.catchAll(() => Effect.succeed({ _tag: "miss" as const }))),

    putByHost: (host, scope, credential, options) =>
      Effect.promise(async () => {
        const api = bridge();
        if (!api) return;
        await api.putByHost({
          host,
          scope,
          credential,
          ...(options ? { options } : {}),
        });
      }).pipe(Effect.catchAll(() => Effect.void)),

    markInvalid: (host, scope) =>
      Effect.promise(async () => {
        const api = bridge();
        if (!api) return;
        await api.markInvalid({ host, scope });
      }).pipe(Effect.catchAll(() => Effect.void)),

    clearHost: (host, scope) =>
      Effect.promise(async () => {
        const api = bridge();
        if (!api) return;
        await api.clearHost({ host, scope });
      }).pipe(Effect.catchAll(() => Effect.void)),
  }),
);
