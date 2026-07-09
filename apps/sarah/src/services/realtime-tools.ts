import { z } from "zod"

import * as checkoutLinkCreate from "../../agent/tools/checkout_link_create.ts"
import * as crmActivityAppend from "../../agent/tools/crm_activity_append.ts"
import * as crmContactUpsert from "../../agent/tools/crm_contact_upsert.ts"
import * as dealRulesEvaluate from "../../agent/tools/deal_rules_evaluate.ts"
import * as demoSalesContext from "../../agent/tools/demo_sales_context.ts"
import * as humanHandoff from "../../agent/tools/human_handoff.ts"
import * as intakeCapture from "../../agent/tools/intake_capture.ts"

const sarahRealtimeTools = {
  checkout_link_create: checkoutLinkCreate,
  crm_activity_append: crmActivityAppend,
  crm_contact_upsert: crmContactUpsert,
  deal_rules_evaluate: dealRulesEvaluate,
  demo_sales_context: demoSalesContext,
  human_handoff: humanHandoff,
  intake_capture: intakeCapture,
} as const

const sarahRealtimeToolInputSchemas = {
  checkout_link_create: checkoutLinkCreate.checkoutLinkCreateInputSchema,
  crm_activity_append: crmActivityAppend.crmActivityAppendInputSchema,
  crm_contact_upsert: crmContactUpsert.crmContactUpsertInputSchema,
  deal_rules_evaluate: dealRulesEvaluate.dealRulesEvaluateInputSchema,
  demo_sales_context: demoSalesContext.demoSalesContextInputSchema,
  human_handoff: humanHandoff.humanHandoffInputSchema,
  intake_capture: intakeCapture.intakeCaptureInputSchema,
} as const

/** Public tool definitions for the realtime token response (schema-only). */
export async function getSarahRealtimeToolDefinitions() {
  return Object.entries(sarahRealtimeToolInputSchemas).map(([name, schema]) => ({
    name,
    description: `Sarah tool ${name}`,
    parameters: schema,
  }))
}

export const realtimeToolCallSchema = z.object({
  toolCallId: z.string().min(1),
  toolName: z.string().min(1),
  args: z.unknown(),
})

export async function executeSarahRealtimeTool({
  toolCallId,
  toolName,
  args,
}: z.infer<typeof realtimeToolCallSchema>) {
  const tool = sarahRealtimeTools[toolName as keyof typeof sarahRealtimeTools]
  if (!tool) {
    return {
      ok: false,
      error: `Unknown Sarah tool: ${toolName}`,
      toolCallId,
      toolName,
    }
  }

  const inputSchema =
    sarahRealtimeToolInputSchemas[
      toolName as keyof typeof sarahRealtimeToolInputSchemas
    ]
  const parsedInput = inputSchema.safeParse(args)
  if (!parsedInput.success) {
    return {
      ok: false,
      error: "Invalid Sarah tool input.",
      issues: parsedInput.error.issues,
      toolCallId,
      toolName,
    }
  }

  const output = await tool.execute(parsedInput.data)
  return {
    ok: true,
    toolCallId,
    toolName,
    output,
  }
}
