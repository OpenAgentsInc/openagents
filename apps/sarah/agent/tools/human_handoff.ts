import {
  createOpenAgentsHumanHandoff,
  humanHandoffInputSchema,
} from "../../src/services/openagents-sales-client.ts"

export { humanHandoffInputSchema }

export async function execute(input: unknown) {
  return createOpenAgentsHumanHandoff(humanHandoffInputSchema.parse(input))
}
