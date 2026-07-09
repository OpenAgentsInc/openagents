import {
  appendOpenAgentsCrmActivity,
  crmActivityAppendInputSchema,
} from "../../src/services/openagents-crm-client.ts"

export { crmActivityAppendInputSchema }

export async function execute(input: unknown) {
  return appendOpenAgentsCrmActivity(crmActivityAppendInputSchema.parse(input))
}
