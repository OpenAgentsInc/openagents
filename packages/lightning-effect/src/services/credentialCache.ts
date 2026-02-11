import { Context, Effect } from "effect"

import type { L402Credential } from "../contracts/l402.js"

export type CredentialCacheLookup =
  | Readonly<{ readonly _tag: "miss" }>
  | Readonly<{ readonly _tag: "hit"; readonly credential: L402Credential }>
  | Readonly<{ readonly _tag: "stale"; readonly credential: L402Credential }>

export type CredentialCacheApi = Readonly<{
  readonly getByHost: (
    host: string,
    scope: string,
    nowMs: number,
  ) => Effect.Effect<CredentialCacheLookup>
  readonly putByHost: (
    host: string,
    scope: string,
    credential: L402Credential,
    options?: { readonly ttlMs?: number },
  ) => Effect.Effect<void>
  readonly markInvalid: (host: string, scope: string) => Effect.Effect<void>
  readonly clearHost: (host: string, scope: string) => Effect.Effect<void>
}>

export class CredentialCacheService extends Context.Tag("@openagents/lightning-effect/CredentialCacheService")<
  CredentialCacheService,
  CredentialCacheApi
>() {}
