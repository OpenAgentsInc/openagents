import { Context, Effect } from "effect"

import { PolicyCheckInput, SpendPolicy } from "../contracts/policy.js"
import { BudgetExceededError, DomainNotAllowedError } from "../errors/lightningErrors.js"

export type SpendPolicyServiceApi = Readonly<{
  readonly policy: SpendPolicy
  readonly ensureRequestAllowed: (
    input: PolicyCheckInput,
  ) => Effect.Effect<void, BudgetExceededError | DomainNotAllowedError>
}>

export class SpendPolicyService extends Context.Tag("@openagents/lightning-effect/SpendPolicyService")<
  SpendPolicyService,
  SpendPolicyServiceApi
>() {}
