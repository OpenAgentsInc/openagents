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
import { readFile } from "node:fs/promises"
import path from "node:path"
import { fileURLToPath } from "node:url"

const root = fileURLToPath(new URL("../..", import.meta.url))

export type OwnedSarahTurnInput = {
  message: string
  threadId?: string
  prospectRef?: string
  toolCall?: { toolName: string; args: unknown }
}

export type OwnedSarahTurnResult = {
  runtime: "owned_effect_seed"
  ok: boolean
  reply: string
  threadId: string
  toolResults: Array<{ toolName: string; ok: boolean; output: unknown }>
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

  if (input.toolCall?.toolName) {
    const result = await runTool(input.toolCall.toolName, input.toolCall.args)
    toolResults.push({
      toolName: input.toolCall.toolName,
      ok: result.ok,
      output: result.output,
    })
  }

  const message = input.message.trim()
  let reply: string
  if (toolResults.length > 0) {
    reply = `Tool ${toolResults[0]!.toolName} ${toolResults[0]!.ok ? "completed" : "failed"}.`
  } else if (!message) {
    reply =
      "I'm Sarah, an AI sales assistant for OpenAgents. How can I help you evaluate OpenAgents?"
  } else if (/price|discount|deal/i.test(message)) {
    reply =
      "I only quote public pack prices and owner-approved parameters — I won't improvise discounts. I can evaluate deal rules or open a human handoff."
  } else {
    reply = `Thanks — I heard you. (Owned runtime seed; full model provider path is env-armed.) You said: ${message.slice(0, 280)}`
  }

  return {
    runtime: "owned_effect_seed",
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
] as const
