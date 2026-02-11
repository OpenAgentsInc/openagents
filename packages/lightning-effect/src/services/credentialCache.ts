import { Context, Effect, Option } from "effect"

import { L402Credential } from "../contracts/l402.js"

export type CredentialCacheApi = Readonly<{
  readonly getByHost: (host: string) => Effect.Effect<Option.Option<L402Credential>>
  readonly putByHost: (host: string, credential: L402Credential) => Effect.Effect<void>
  readonly clearHost: (host: string) => Effect.Effect<void>
}>

export class CredentialCacheService extends Context.Tag("@openagents/lightning-effect/CredentialCacheService")<
  CredentialCacheService,
  CredentialCacheApi
>() {}
