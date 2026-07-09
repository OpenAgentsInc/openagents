/**
 * SM-1 Effect service layers for Sarah domain dependencies.
 * Keeps HTTP handlers thin and wires CRM/sales/config as Context.Tag services.
 */

import { Context, Effect, Layer } from "effect"
import { getSarahRealtimeInstructions } from "./sarah-instructions.ts"
import { listSarahProspectSessions } from "./session-index.ts"
import { enqueueSarahEmailDraft } from "./crm-email-rail.ts"
import { evaluateDealRules } from "./deal-rules.ts"
import { runOwnedSarahTurn } from "../agent-runtime/owned-runtime.ts"

export class SarahInstructions extends Context.Service<
  SarahInstructions,
  {
    readonly realtime: (
      crmContext?: string | null,
    ) => Effect.Effect<string>
  }
>()("sarah/Instructions") {}

export class SarahSessions extends Context.Service<
  SarahSessions,
  {
    readonly listProspects: () => Effect.Effect<unknown>
  }
>()("sarah/Sessions") {}

export class SarahEmailRail extends Context.Service<
  SarahEmailRail,
  {
    readonly enqueueDraft: (
      input: Parameters<typeof enqueueSarahEmailDraft>[0],
    ) => Effect.Effect<Awaited<ReturnType<typeof enqueueSarahEmailDraft>>>
  }
>()("sarah/EmailRail") {}

export class SarahDealRules extends Context.Service<
  SarahDealRules,
  {
    readonly evaluate: (
      input: Parameters<typeof evaluateDealRules>[0],
    ) => Effect.Effect<ReturnType<typeof evaluateDealRules>>
  }
>()("sarah/DealRules") {}

export class SarahAgentRuntime extends Context.Service<
  SarahAgentRuntime,
  {
    readonly turn: (
      input: Parameters<typeof runOwnedSarahTurn>[0],
    ) => Effect.Effect<Awaited<ReturnType<typeof runOwnedSarahTurn>>>
  }
>()("sarah/AgentRuntime") {}

export const SarahInstructionsLive = Layer.succeed(SarahInstructions, {
  realtime: (crmContext) =>
    Effect.promise(() => getSarahRealtimeInstructions(crmContext)),
})

export const SarahSessionsLive = Layer.succeed(SarahSessions, {
  listProspects: () => Effect.promise(() => listSarahProspectSessions()),
})

export const SarahEmailRailLive = Layer.succeed(SarahEmailRail, {
  enqueueDraft: (input) => Effect.promise(() => enqueueSarahEmailDraft(input)),
})

export const SarahDealRulesLive = Layer.succeed(SarahDealRules, {
  evaluate: (input) => Effect.sync(() => evaluateDealRules(input)),
})

export const SarahAgentRuntimeLive = Layer.succeed(SarahAgentRuntime, {
  turn: (input) => Effect.promise(() => runOwnedSarahTurn(input)),
})

export const SarahAppLive = Layer.mergeAll(
  SarahInstructionsLive,
  SarahSessionsLive,
  SarahEmailRailLive,
  SarahDealRulesLive,
  SarahAgentRuntimeLive,
)
