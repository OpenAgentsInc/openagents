import { z } from "zod"

export const demoSalesContextInputSchema = z.object({
  topic: z.string().min(1).default("OpenAgents sales conversation"),
})

export async function execute(input: unknown) {
  const parsed = demoSalesContextInputSchema.parse(input)
  return {
    topic: parsed.topic,
    packs: ["starter", "growth", "enterprise"],
    publicSafe: true,
  }
}
