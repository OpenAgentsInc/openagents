import {
  captureOpenAgentsIntake,
  intakeCaptureInputSchema,
} from "../../src/services/openagents-sales-client.ts"

export { intakeCaptureInputSchema }

export async function execute(input: unknown) {
  return captureOpenAgentsIntake(intakeCaptureInputSchema.parse(input))
}
