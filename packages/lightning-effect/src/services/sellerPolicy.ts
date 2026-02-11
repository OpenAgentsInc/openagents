import { Context, Effect } from "effect"

import type {
  L402AuthorizationVerificationResult,
  L402ChallengeIssueRequest,
  PaywallDefinition,
} from "../contracts/seller.js"
import type { SellerPolicyViolationError } from "../errors/lightningErrors.js"

export type SellerPolicyServiceApi = Readonly<{
  readonly ensureChallengeAllowed: (
    paywall: PaywallDefinition,
    request: L402ChallengeIssueRequest,
  ) => Effect.Effect<void, SellerPolicyViolationError>
  readonly ensureAuthorizationAllowed: (
    paywall: PaywallDefinition,
    verification: L402AuthorizationVerificationResult,
  ) => Effect.Effect<void, SellerPolicyViolationError>
}>

export class SellerPolicyService extends Context.Tag(
  "@openagents/lightning-effect/SellerPolicyService",
)<SellerPolicyService, SellerPolicyServiceApi>() {}
