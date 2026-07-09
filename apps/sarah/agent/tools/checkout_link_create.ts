import {
  checkoutLinkCreateInputSchema,
  createOpenAgentsCheckoutLink,
} from "../../src/services/openagents-sales-client.ts"

export { checkoutLinkCreateInputSchema }

export async function execute(input: unknown) {
  return createOpenAgentsCheckoutLink(checkoutLinkCreateInputSchema.parse(input))
}
