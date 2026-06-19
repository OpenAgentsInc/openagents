// Metering-hook seam for the inference gateway (EPIC #5474, #5476).
//
// This is the single typed point where #5477 (credits, metering & billing) will
// decrement credits from the provider `usage` object — receipt-first, never an
// estimate (gateway business doc §4; INVARIANTS.md "Canonical Token Usage
// Ledger"). #5476 ships only a no-op/log stub so the route works end-to-end and
// the seam has a stable shape to plug into.
//
// The hook receives the authenticated account, the served model + adapter, and
// the real provider usage AFTER a completion finishes (or, for streams, after
// the terminal usage frame arrives). It does NOT itself move money; #5477 wires
// the real implementation against the existing credit ledger
// (`payments-ledger.ts`, `agent_balances`) and the pricing/multiplier engine
// (#5478). Returning a typed receipt-ref keeps the route's response honest about
// whether metering is live (stub => `metered: false`).

import { Effect } from 'effect'

import { workerLogEntry } from '../observability'
import { type InferenceUsage } from './provider-adapter'

// Context handed to the metering hook when a request completes.
export type MeteringContext = Readonly<{
  // Authenticated account ref (e.g. "agent:<id>"), the principal whose balance
  // #5477 will decrement.
  accountRef: string
  // The model alias the customer requested.
  requestedModel: string
  // The provider-native model actually served.
  servedModel: string
  // The adapter id that served the request (provider-capacity attribution).
  adapterId: string
  // Receipt-first usage from the provider response.
  usage: InferenceUsage
  // Whether the request was streamed.
  streamed: boolean
}>

// Outcome of a metering attempt. The stub returns `metered: false`; #5477's
// implementation returns `metered: true` with a real ledger receipt ref once
// credits are decremented.
export type MeteringOutcome = Readonly<{
  metered: boolean
  // Public-safe ledger/usage receipt ref when metering is live; null for the
  // stub. Never a raw amount, destination, or payment material here.
  receiptRef: string | null
}>

// The metering-hook contract. #5477 provides the live implementation.
export type MeteringHook = (
  context: MeteringContext,
) => Effect.Effect<MeteringOutcome>

// No-op stub for #5476. Logs the usage it WOULD meter (public-safe: account
// ref, model, adapter, token counts only — no prompts, no payment material) and
// reports `metered: false`. Swap this for the real hook in #5477.
export const stubMeteringHook: MeteringHook = (context: MeteringContext) =>
  Effect.gen(function* () {
    // Public-safe, bounded diagnostic only (token counts + refs, never prompt
    // or response content), through the redacted observability helper.
    yield* Effect.logInfo(
      workerLogEntry('inference.metering.stub', {
        accountRef: context.accountRef,
        adapterId: context.adapterId,
        completionTokens: context.usage.completionTokens,
        promptTokens: context.usage.promptTokens,
        requestedModel: context.requestedModel,
        servedModel: context.servedModel,
        streamed: context.streamed,
        totalTokens: context.usage.totalTokens,
      }),
    )
    return { metered: false, receiptRef: null } satisfies MeteringOutcome
  })
