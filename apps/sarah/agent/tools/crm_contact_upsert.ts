import {
  crmContactUpsertInputSchema,
  upsertOpenAgentsCrmContact,
} from "../../src/services/openagents-crm-client.ts"

export { crmContactUpsertInputSchema }

export async function execute(input: unknown) {
  return upsertOpenAgentsCrmContact(crmContactUpsertInputSchema.parse(input))
}
