/**
 * SM-4 owned Effect-shaped agent runtime seed for Sarah.
 *
 * Replaces eve for the HTTP turn/tool path. Tools call the same service
 * modules the eve tools used. Shaped to converge with agent_definition.v1 /
 * ai_employee.v1 later (P4) — not a Sarah-only fork forever.
 */

import { captureOpenAgentsIntake } from "../services/openagents-sales-client.ts"
import {
  createOpenAgentsCheckoutLink,
  createOpenAgentsHumanHandoff,
} from "../services/openagents-sales-client.ts"
import {
  appendOpenAgentsCrmActivity,
  upsertOpenAgentsCrmContact,
} from "../services/openagents-crm-client.ts"
import { evaluateDealRules } from "../services/deal-rules.ts"
import {
  generateSarahGemmaReply,
  isSarahInferenceBusyError,
  sarahInferenceArmed,
  sarahInferenceTransport,
} from "../services/google-inference.ts"
import type { GemmaContent } from "../services/google-inference.ts"
import {
  CROSS_PROSPECT_MEMORY_REFUSAL_REPLY,
  getProspectMemoryContext,
  isCrossProspectMemoryProbe,
} from "../services/prospect-memory.ts"
import { getSarahAccountPromptLine } from "../services/account-link.ts"
import { maybeSemanticCacheAnswer } from "../services/semantic-answer-cache.ts"
import {
  liveStats,
  maybeEcosystemGrounding,
  planCatalog,
  promiseLookup,
} from "../services/ecosystem-tools.ts"
import { buildCustomerBlueprintDraft } from "../services/customer-blueprint.ts"
import { getSarahRealtimeInstructions } from "../services/sarah-instructions.ts"
import {
  getSarahSessionTranscript,
  recordSarahToolReceipt,
  recordSarahTranscriptTurn,
} from "../services/session-index.ts"
import {
  formatInstructedToolReply,
  instructedJsonToolProtocolPrompt,
  instructedJsonToolsArmed,
  parseInstructedJsonToolCall,
} from "../services/instructed-json-tools.ts"
import { readFile } from "node:fs/promises"
import path from "node:path"
import { fileURLToPath } from "node:url"

const root = fileURLToPath(new URL("../..", import.meta.url))

export type OwnedSarahTurnInput = {
  message: string
  threadId?: string
  prospectRef?: string
  toolCall?: { toolName: string; args: unknown; toolCallId?: string }
}

export type OwnedSarahToolResult = {
  toolCallId: string
  toolName: string
  ok: boolean
  output: unknown
}

export type OwnedSarahTurnResult = {
  runtime: "owned_effect_seed"
  /**
   * "khala_gateway_live" (KHS-1) or "google_gemma_live" when a real model
   * produced the reply; "semantic_cache" for KHS-6 cache hits.
   */
  modelPath:
    | "khala_gateway_live"
    | "google_gemma_live"
    | "seed_echo"
    | "deterministic_guard"
    | "semantic_cache"
  model?: string
  modelError?: string
  ok: boolean
  reply: string
  threadId: string
  toolResults: OwnedSarahToolResult[]
  personaPreview: string
}

async function loadPersona(): Promise<string> {
  try {
    return await readFile(path.join(root, "agent/instructions.md"), "utf8")
  } catch {
    return "You are Sarah, an OpenAgents sales assistant. Disclose you are AI."
  }
}

async function runTool(
  toolName: string,
  args: unknown,
  context: { prospectRef?: string } = {},
): Promise<{ ok: boolean; output: unknown }> {
  try {
    switch (toolName) {
      case "intake_capture":
        return { ok: true, output: await captureOpenAgentsIntake(args as never) }
      case "crm_contact_upsert":
        return {
          ok: true,
          output: await upsertOpenAgentsCrmContact(args as never),
        }
      case "crm_activity_append":
        return {
          ok: true,
          output: await appendOpenAgentsCrmActivity(args as never),
        }
      case "deal_rules_evaluate":
        return { ok: true, output: evaluateDealRules(args as never) }
      case "checkout_link_create":
        return {
          ok: true,
          output: await createOpenAgentsCheckoutLink(args as never),
        }
      case "human_handoff":
        return {
          ok: true,
          output: await createOpenAgentsHumanHandoff(args as never),
        }
      case "demo_sales_context":
        return {
          ok: true,
          output: {
            packs: ["starter", "growth", "enterprise"],
            publicSafe: true,
          },
        }
      // KHS-9 (#8608) live ecosystem product-truth tools — public
      // openagents.com surfaces, fail-soft, registry safe-copy only.
      case "promise_lookup": {
        const query =
          typeof (args as { query?: unknown })?.query === "string"
            ? (args as { query: string }).query
            : ""
        const result = await promiseLookup(query)
        return { ok: result.ok, output: result }
      }
      case "live_stats": {
        const result = await liveStats()
        return { ok: result.ok, output: result }
      }
      case "plan_catalog": {
        const result = await planCatalog()
        return { ok: result.ok, output: result }
      }
      // KHS-9 (#8608) customer Blueprint draft — per-prospect scoped (the
      // ref comes from the request context, never another prospect's args).
      case "customer_blueprint_draft": {
        const prospectRef =
          context.prospectRef ??
          (typeof (args as { prospectRef?: unknown })?.prospectRef === "string"
            ? (args as { prospectRef: string }).prospectRef
            : "")
        const result = await buildCustomerBlueprintDraft(prospectRef)
        return { ok: result.ok, output: result }
      }
      default:
        return {
          ok: false,
          output: { error: `unknown_tool:${toolName}` },
        }
    }
  } catch (error) {
    return {
      ok: false,
      output: {
        error: error instanceof Error ? error.message : String(error),
      },
    }
  }
}

export async function runOwnedSarahTurn(
  input: OwnedSarahTurnInput,
): Promise<OwnedSarahTurnResult> {
  const persona = await loadPersona()
  const threadId =
    input.threadId ??
    (input.prospectRef
      ? `prospect:${input.prospectRef}`
      : `thread:${crypto.randomUUID()}`)
  const toolResults: OwnedSarahTurnResult["toolResults"] = []
  const runAndRecordTool = async (
    toolName: string,
    args: unknown,
    toolCallId = `tool.${crypto.randomUUID()}`,
  ) => {
    const result = await runTool(toolName, args, {
      ...(input.prospectRef ? { prospectRef: input.prospectRef } : {}),
    })
    const recorded = {
      toolCallId,
      toolName,
      ok: result.ok,
      output: result.output,
    }
    toolResults.push(recorded)
    if (input.prospectRef) {
      await recordSarahToolReceipt({
        prospectRef: input.prospectRef,
        sessionId: threadId,
        threadId,
        toolCallId,
        toolName,
        result: { ok: result.ok, output: result.output },
      })
    }
    return result
  }

  if (input.toolCall?.toolName) {
    await runAndRecordTool(
      input.toolCall.toolName,
      input.toolCall.args,
      input.toolCall.toolCallId,
    )
  }

  const message = input.message.trim()
  let reply: string
  let modelPath: OwnedSarahTurnResult["modelPath"] = "seed_echo"
  let model: string | undefined
  let modelError: string | undefined

  // KHS-6 (#8605): flag-gated semantic answer cache — always null unless
  // SARAH_SEMANTIC_CACHE=1; its internal pricing guard runs before matching.
  const semanticCached =
    !input.toolCall && message ? await maybeSemanticCacheAnswer(message) : null

  if (toolResults.length > 0) {
    reply = `Tool ${toolResults[0]!.toolName} ${toolResults[0]!.ok ? "completed" : "failed"}.`
    modelPath = "deterministic_guard"
  } else if (!message) {
    reply =
      "I'm Sarah, an AI sales assistant for OpenAgents. How can I help you evaluate OpenAgents?"
    modelPath = "deterministic_guard"
  } else if (isCrossProspectMemoryProbe(message)) {
    // KHS-3: cross-prospect memory probes never reach the model.
    reply = CROSS_PROSPECT_MEMORY_REFUSAL_REPLY
    modelPath = "deterministic_guard"
  } else if (/price|discount|deal/i.test(message)) {
    // Hard no-improvised-pricing law: Gemma's text lane has no native tool
    // calling, so pricing never reaches the model here. Deal-rule evaluation,
    // checkout tracing, and handoff stay on the tool/voice paths.
    reply =
      "I only quote public pack prices and owner-approved parameters — I won't improvise discounts. I can evaluate deal rules or open a human handoff."
    modelPath = "deterministic_guard"
  } else if (semanticCached) {
    reply = semanticCached.answer
    modelPath = "semantic_cache"
  } else if (sarahInferenceArmed()) {
    // KHS-2 (#8601): prospect memory prepends AFTER the guards above — the
    // pricing guard always runs before the model regardless of memory.
    let system = await getSarahRealtimeInstructions()
    if (input.prospectRef) {
      const memory = await getProspectMemoryContext(input.prospectRef)
      if (memory) system = `${memory}\n\n${system}`
    }
    // KHS-7 (#8606): one account-awareness line (linked identity, or a gentle
    // may-suggest-once for engaged anonymous prospects). Code-side assembly
    // only — the owner-managed base context is untouched, and this runs after
    // the deterministic guards above, so it can never reach the pricing lane.
    const accountLine = await getSarahAccountPromptLine(input.prospectRef)
    if (accountLine) system = `${system}\n\n${accountLine}`
    // KHS-9 (#8608): flag-gated live-product-truth grounding
    // (SARAH_ECOSYSTEM_GROUNDING=1). Embedding-matched intents only, appended
    // AFTER the deterministic guards above — never on the pricing lane.
    const grounding = await maybeEcosystemGrounding(message)
    if (grounding) system = `${system}\n\n${grounding}`
    // AV-3 residual (#8598): optional instructed-JSON tool protocol for Gemma
    // (no native function calling). Flag-gated; pricing tools never allowed.
    if (instructedJsonToolsArmed()) {
      system = `${system}\n\n${instructedJsonToolProtocolPrompt()}`
    }
    const contents: GemmaContent[] = []
    if (input.prospectRef) {
      const history = await getSarahSessionTranscript({
        prospectRef: input.prospectRef,
        sessionId: threadId,
      })
      for (const turn of history) {
        if (!turn.text.trim()) continue
        contents.push({
          role: turn.role === "assistant" ? "model" : "user",
          parts: [{ text: turn.text }],
        })
      }
    }
    contents.push({ role: "user", parts: [{ text: message }] })

    const result = await generateSarahGemmaReply({ system, contents })
    if (result.ok) {
      reply = result.reply
      modelPath =
        sarahInferenceTransport() === "khala_gateway"
          ? "khala_gateway_live"
          : "google_gemma_live"
      model = result.model

      // If the model asked for a tool via instructed JSON, execute it once
      // (no recursive model loop in v1 — keep deterministic after the tool).
      if (instructedJsonToolsArmed() && !input.toolCall) {
        const instructed = parseInstructedJsonToolCall(result.reply)
        if (instructed) {
          const toolResult = await runAndRecordTool(
            instructed.toolName,
            instructed.args,
            `tool.${crypto.randomUUID()}`,
          )
          reply = formatInstructedToolReply(
            instructed.toolName,
            toolResult.ok,
            toolResult.output,
          )
          modelPath = "deterministic_guard"
        }
      }
    } else {
      modelError = result.error
      reply = isSarahInferenceBusyError(result.error)
        ? "I'm handling a lot of conversations right now — give me about a minute and ask again, or leave your email and I'll follow up."
        : "I'm having trouble reaching my model right now — please try again in a moment, or leave your email and I'll follow up."
    }
  } else {
    reply = `Thanks — I heard you. (Owned runtime seed; model path not armed.) You said: ${message.slice(0, 280)}`
  }

  // Every prospect-attached turn is recorded — guard refusals and fallbacks
  // included (owner directive 2026-07-09: save all conversation turns).
  if (input.prospectRef && message) {
    const shared = {
      prospectRef: input.prospectRef,
      sessionId: threadId,
      threadId,
    }
    await recordSarahTranscriptTurn({
      ...shared,
      turn: { modality: "text", role: "user", sourceEvent: "text_turn", text: message },
    })
    await recordSarahTranscriptTurn({
      ...shared,
      turn: { modality: "text", role: "assistant", sourceEvent: "text_turn", text: reply },
    })
  }

  return {
    runtime: "owned_effect_seed",
    modelPath,
    ...(model ? { model } : {}),
    ...(modelError ? { modelError } : {}),
    ok: true,
    reply,
    threadId,
    toolResults,
    personaPreview: persona.slice(0, 200),
  }
}

export const SARAH_OWNED_TOOL_INVENTORY = [
  "intake_capture",
  "crm_contact_upsert",
  "crm_activity_append",
  "deal_rules_evaluate",
  "human_handoff",
  "checkout_link_create",
  "demo_sales_context",
  // KHS-9 (#8608): live ecosystem product truth + customer Blueprint drafts.
  "promise_lookup",
  "live_stats",
  "plan_catalog",
  "customer_blueprint_draft",
] as const
