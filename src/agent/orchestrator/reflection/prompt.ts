/**
 * Reflection Prompt Templates
 *
 * Prompts for generating and injecting reflections.
 */
import type { FailureContext, Reflection } from "./schema.js";

// ============================================================================
// Generation Prompt
// ============================================================================

/**
 * Prompt template for generating a reflection from a failure context.
 */
export const REFLECTION_GENERATION_PROMPT = `You are analyzing a failed coding task attempt. Your goal is to generate a concise reflection that will help the next attempt succeed.

## Failed Subtask
{subtaskDescription}

## Attempt Number
{attemptNumber}

## Failure Type
{failureType}

## Error Output
\`\`\`
{errorOutput}
\`\`\`

## Previous Reflections (avoid repeating these)
{previousReflections}

---

Generate a concise reflection analyzing this failure:

1. **What went wrong**: Identify the root cause in 1-2 sentences. Be specific about the technical issue.
2. **What to do differently**: Suggest a concrete alternative approach in 1-2 sentences.
3. **Action items**: List 2-3 specific steps for the next attempt.

Respond ONLY with valid JSON in this exact format:
{
  "category": "root_cause" | "misconception" | "environment" | "approach_error" | "edge_case" | "verification",
  "analysis": "What went wrong...",
  "suggestion": "What to do differently...",
  "actionItems": ["Step 1", "Step 2"],
  "confidence": 0.8
}

Choose the category that best describes the failure:
- root_cause: The fundamental technical cause
- misconception: Agent misunderstood requirements or context
- environment: Dependencies, config, or environment issues
- approach_error: Wrong strategy or approach taken
- edge_case: Missed an edge case or boundary condition
- verification: Test/verification-specific issue`;

/**
 * Build the reflection generation prompt from a failure context.
 */
export const buildGenerationPrompt = (ctx: FailureContext): string => {
  const previousReflectionsText =
    ctx.previousReflections.length > 0
      ? ctx.previousReflections.map((r, i) => `${i + 1}. ${r}`).join("\n")
      : "None";

  return REFLECTION_GENERATION_PROMPT.replace("{subtaskDescription}", ctx.subtaskDescription.slice(0, 1000))
    .replace("{attemptNumber}", String(ctx.attemptNumber))
    .replace("{failureType}", ctx.failureType)
    .replace("{errorOutput}", ctx.errorOutput.slice(0, 2000))
    .replace("{previousReflections}", previousReflectionsText);
};

// ============================================================================
// Injection Prompt
// ============================================================================

/**
 * Format reflections for injection into a subagent retry prompt.
 */
export const formatReflectionsForPrompt = (reflections: Reflection[]): string => {
  if (reflections.length === 0) return "";

  const reflectionItems = reflections
    .map(
      (r) => `
### Attempt ${r.attemptNumber}
- **What went wrong**: ${r.analysis}
- **What to do differently**: ${r.suggestion}
- **Action items**:
${r.actionItems.map((a) => `  - ${a}`).join("\n")}`
    )
    .join("\n");

  return `
## Reflections from Previous Attempts

Learn from these insights before proceeding:
${reflectionItems}

---

**IMPORTANT**: You MUST address these issues. Do NOT repeat the same mistakes.
`;
};

// ============================================================================
// Response Parsing
// ============================================================================

/**
 * Parsed reflection response from LLM.
 */
export interface ParsedReflectionResponse {
  category: string;
  analysis: string;
  suggestion: string;
  actionItems: string[];
  confidence: number;
}

/**
 * Parse the LLM response into a reflection.
 * Handles JSON extraction from potentially wrapped response.
 */
export const parseReflectionResponse = (response: string): ParsedReflectionResponse | null => {
  try {
    // Try to extract JSON from the response
    const jsonMatch = response.match(/\{[\s\S]*\}/);
    if (!jsonMatch) {
      return null;
    }

    const parsed = JSON.parse(jsonMatch[0]) as Partial<ParsedReflectionResponse>;

    // Validate required fields
    if (!parsed.category || !parsed.analysis || !parsed.suggestion) {
      return null;
    }

    return {
      category: parsed.category,
      analysis: parsed.analysis,
      suggestion: parsed.suggestion,
      actionItems: Array.isArray(parsed.actionItems) ? parsed.actionItems : [],
      confidence: typeof parsed.confidence === "number" ? parsed.confidence : 0.5,
    };
  } catch {
    return null;
  }
};
