// Autopilot Concierge tool registry SEAM (issue #6148).
//
// WHAT THIS IS — AND WHAT IT IS NOT
// ---------------------------------
// Concierge is more than a system prompt: it is the agent a customer (or another
// program) talks to to put their business on Autopilot, with a BOUNDED,
// purpose-built tool set. This module is the TYPED DECLARATION of that bounded
// set — the closed catalog of tools Concierge is permitted to use, each with its
// review/spend posture and a typed argument schema.
//
// IT DOES NOT EXECUTE ANY TOOL. This PR ships the SEAM only:
//   - the closed enum of tool ids,
//   - each tool's typed args schema (Effect Schema),
//   - each tool's review/mutation/spend classification, and
//   - a pure resolver/guard so a future executor can dispatch only declared,
//     review-gated tools.
//
// LIVE TOOL EXECUTION IS DEFERRED (NEEDS-FOLLOWUP). Wiring real web
// search/enrichment, prefilled-workspace seeding, checkout/credit kickoff, and
// CRM writes — each behind its human-review/consent gate and the existing
// gateway auth + metering + receipt boundaries — is a separate, larger change.
// Nothing here calls a provider, mutates state, or spends. A caller that tries
// to "run" a tool gets a typed `not_implemented` outcome, never a side effect.
//
// WHY A CLOSED, TYPED SET (workspace semantic-routing rule)
// ---------------------------------------------------------
// Tool selection is a bounded enum + typed args, never an ad-hoc string match on
// model intent. The model may only NAME a tool from this catalog; the gateway
// validates the args against the tool's schema before anything could ever run.
// Each mutating/spending tool is `humanReviewGated: true`, so even when live
// execution lands it cannot mutate or spend without the reviewed surface that
// owns that action explicitly performing it.

import { Schema as S } from 'effect'

// The closed set of Concierge tool ids. Adding a tool is a deliberate, reviewed
// catalog bump — never a model invention.
export const AUTOPILOT_CONCIERGE_TOOL_IDS = [
  'web_search_enrichment',
  'prefilled_workspace_seeding',
  'checkout_credit_kickoff',
  'crm_write',
] as const

export type AutopilotConciergeToolId =
  (typeof AUTOPILOT_CONCIERGE_TOOL_IDS)[number]

// The effect class of a tool: does invoking it READ, MUTATE state, or SPEND
// money? Drives the review gate — `mutate`/`spend` tools are always
// human-review-gated.
export type ConciergeToolEffectClass = 'read' | 'mutate' | 'spend'

// ---------------------------------------------------------------------------
// Typed argument schemas (one per tool). These are the contract a future
// executor validates BEFORE any dispatch. Bounded fields only — no free-form
// system-prompt/overlay text (that would re-open the injection hole the
// server-owned vertical enum closed).
// ---------------------------------------------------------------------------

// web_search_enrichment — look up public, non-private context about the
// business (read-only). `query` is the bounded search string; `maxResults`
// caps breadth.
export const WebSearchEnrichmentArgs = S.Struct({
  query: S.NonEmptyString,
  maxResults: S.optionalKey(S.Int.check(S.isGreaterThan(0))),
})

// prefilled_workspace_seeding — seed a workspace with public-safe facts already
// gathered in the interview. Mutating (creates/updates a workspace), so
// review-gated. `seededFacts` are the public-safe facts to seed.
export const PrefilledWorkspaceSeedingArgs = S.Struct({
  workspaceRef: S.NonEmptyString,
  seededFacts: S.Array(S.NonEmptyString),
})

// checkout_credit_kickoff — kick off the credit/checkout flow (e.g. "$500 in
// credits"). SPENDING, so review-gated. `amountCents` is the bounded positive
// kickoff amount; `label` is the CTA copy.
export const CheckoutCreditKickoffArgs = S.Struct({
  amountCents: S.Int.check(S.isGreaterThan(0)),
  label: S.NonEmptyString,
})

// crm_write — write the intake outcome (contact + spec summary) to the CRM.
// Mutating, so review-gated. Bounded fields only.
export const CrmWriteArgs = S.Struct({
  contactName: S.NonEmptyString,
  contactEmail: S.optionalKey(S.String),
  summary: S.NonEmptyString,
})

// A typed Concierge tool declaration. `argsSchema` is `unknown`-typed at the map
// level (the schemas above differ per tool); `validateArgs` decodes per-tool.
export type AutopilotConciergeTool = Readonly<{
  id: AutopilotConciergeToolId
  // Human/agent-readable purpose (documentation, surfaced to callers).
  description: string
  // read | mutate | spend — drives the review gate.
  effectClass: ConciergeToolEffectClass
  // True when invoking the tool requires an explicit human-review/consent gate.
  // Always true for `mutate`/`spend`. A `read` tool may still be gated by data
  // practices (consent before private data); here read tools are public-only and
  // ungated.
  humanReviewGated: boolean
  // Pure args validation. Returns the typed args on success or undefined on a
  // schema violation (NEVER throws). A future executor calls this before any
  // dispatch so only well-formed, in-catalog calls could ever run.
  validateArgs: (raw: unknown) => Record<string, unknown> | undefined
}>

const decodeArgs =
  <A>(schema: S.Decoder<A>) =>
  (raw: unknown): Record<string, unknown> | undefined => {
    try {
      return S.decodeUnknownSync(schema)(raw) as Record<string, unknown>
    } catch {
      return undefined
    }
  }

// The closed Concierge tool catalog. The KEYS are the closed enum; a tool id not
// in this map is rejected. Single source of truth — the enum, the schemas, and
// the prompt all derive from it.
export const AUTOPILOT_CONCIERGE_TOOLS: Readonly<
  Record<AutopilotConciergeToolId, AutopilotConciergeTool>
> = {
  web_search_enrichment: {
    description:
      'Look up public, non-private context about the business to enrich the intake. Read-only; public sources only.',
    effectClass: 'read',
    humanReviewGated: false,
    id: 'web_search_enrichment',
    validateArgs: decodeArgs(WebSearchEnrichmentArgs),
  },
  prefilled_workspace_seeding: {
    description:
      'Seed a prefilled workspace with public-safe facts gathered in the interview. Mutating; human-review-gated.',
    effectClass: 'mutate',
    humanReviewGated: true,
    id: 'prefilled_workspace_seeding',
    validateArgs: decodeArgs(PrefilledWorkspaceSeedingArgs),
  },
  checkout_credit_kickoff: {
    description:
      'Kick off the credit/checkout flow for the first quick win. Spending; human-review-gated. Never charges from this seam.',
    effectClass: 'spend',
    humanReviewGated: true,
    id: 'checkout_credit_kickoff',
    validateArgs: decodeArgs(CheckoutCreditKickoffArgs),
  },
  crm_write: {
    description:
      'Write the intake outcome (contact + spec summary) to the CRM. Mutating; human-review-gated.',
    effectClass: 'mutate',
    humanReviewGated: true,
    id: 'crm_write',
    validateArgs: decodeArgs(CrmWriteArgs),
  },
}

export const isAutopilotConciergeTool = (
  id: string,
): id is AutopilotConciergeToolId =>
  Object.prototype.hasOwnProperty.call(AUTOPILOT_CONCIERGE_TOOLS, id)

export const getAutopilotConciergeTool = (
  id: string,
): AutopilotConciergeTool | undefined =>
  isAutopilotConciergeTool(id) ? AUTOPILOT_CONCIERGE_TOOLS[id] : undefined

// ---------------------------------------------------------------------------
// The DEFERRED execution seam. Live execution is NOT implemented in this PR.
// `runConciergeTool` always returns a typed `not_implemented` outcome (or
// `invalid_args` / `unknown_tool` on a malformed/unknown request), and NEVER
// produces a side effect, a provider call, a mutation, or a charge. A future PR
// replaces the body with the real, review-gated, metered execution — gated
// behind a flag and behind each tool's `humanReviewGated`/consent boundary —
// without changing this typed contract.
// ---------------------------------------------------------------------------

export type ConciergeToolOutcome =
  | Readonly<{ status: 'not_implemented'; tool: AutopilotConciergeToolId }>
  | Readonly<{
      status: 'invalid_args'
      tool: AutopilotConciergeToolId
      detail: string
    }>
  | Readonly<{ status: 'unknown_tool'; tool: string }>

// Resolve + validate a tool invocation WITHOUT executing it. This is the only
// entry a future executor needs; today it proves the call is well-formed and
// in-catalog, then returns `not_implemented` (the honest deferral). NEEDS-FOLLOWUP:
// wire real execution behind the review gate + gateway auth/metering/receipts.
export const runConciergeTool = (
  toolId: string,
  rawArgs: unknown,
): ConciergeToolOutcome => {
  const tool = getAutopilotConciergeTool(toolId)
  if (tool === undefined) {
    return { status: 'unknown_tool', tool: toolId }
  }
  const validated = tool.validateArgs(rawArgs)
  if (validated === undefined) {
    return {
      detail: `args did not match the ${tool.id} schema`,
      status: 'invalid_args',
      tool: tool.id,
    }
  }
  // DEFERRED: no dispatch, no mutation, no spend. Honest typed deferral.
  return { status: 'not_implemented', tool: tool.id }
}

// A public-safe, agent-readable description of the bounded tool set Concierge
// DECLARES. Surfaced on the disclosure block so a programmatic consumer knows
// the tool surface exists and that mutating/spending tools are review-gated and
// not yet live. No args, no secrets — declaration only.
export type ConciergeToolDeclaration = Readonly<{
  id: AutopilotConciergeToolId
  effect_class: ConciergeToolEffectClass
  human_review_gated: boolean
  // Honest execution status: every tool is a declared seam, not yet live.
  status: 'declared_not_executed'
}>

export const autopilotConciergeToolDeclarations =
  (): ReadonlyArray<ConciergeToolDeclaration> =>
    AUTOPILOT_CONCIERGE_TOOL_IDS.map(id => {
      const tool = AUTOPILOT_CONCIERGE_TOOLS[id]
      return {
        effect_class: tool.effectClass,
        human_review_gated: tool.humanReviewGated,
        id: tool.id,
        status: 'declared_not_executed' as const,
      }
    })

// The system-prompt block declaring the bounded tool set to the model. It tells
// the model the tools EXIST but are not yet executable from this surface, so it
// never promises a tool ran. Kept here so the prompt and the catalog cannot
// drift.
export const AUTOPILOT_CONCIERGE_TOOLS_PROMPT = [
  'BOUNDED TOOL SET (declared, not yet executable from this surface).',
  `Concierge has a closed, purpose-built tool set: ${AUTOPILOT_CONCIERGE_TOOL_IDS.join(', ')}.`,
  'These tools are DECLARED but not live from this endpoint yet. Do not claim a tool ran, do not promise a checkout, CRM write, workspace seeding, or enrichment actually happened.',
  'When a tool would help, describe what it WOULD do and that it is human-review-gated where it mutates state or spends, then proceed with the interview. Never invent a tool outside that list.',
].join(' ')
