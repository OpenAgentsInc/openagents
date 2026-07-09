import {
  dealRulesEvaluateInputSchema,
  evaluateDealRules,
} from "../../src/services/deal-rules.ts"

export { dealRulesEvaluateInputSchema }

export async function execute(input: unknown) {
  return evaluateDealRules(dealRulesEvaluateInputSchema.parse(input))
}
