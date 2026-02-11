import { Context, Effect } from "effect"

import type {
  L402ChallengeIssueRequest,
  L402ChallengeIssueResult,
  PaywallDefinition,
} from "../contracts/seller.js"

export type InvoiceIssuerApi = Readonly<{
  readonly issueChallenge: (
    request: L402ChallengeIssueRequest,
    paywall: PaywallDefinition,
  ) => Effect.Effect<L402ChallengeIssueResult>
}>

export class InvoiceIssuerService extends Context.Tag(
  "@openagents/lightning-effect/InvoiceIssuerService",
)<InvoiceIssuerService, InvoiceIssuerApi>() {}
