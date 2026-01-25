/**
 * Rendering Logic: Normalizes Render Functions & Applies Decorators
 */

import type { CsfStory, StoryRenderOutput, StoryContext } from "../../csf/csf"
import { isTemplateResult } from "../../../effuse/template/types"

// Simple template stringifier (duplicated to avoid complex dependencies for now)
const templateToString = (template: any): string => {
  if (!template || !template.parts) return ""
  return template.parts
    .map((part: any) => {
      switch (part._tag) {
        case "Text": return part.value
        case "Html": return part.value
        case "Template": return templateToString(part.value)
        default: return ""
      }
    })
    .join("")
}

/**
 * Normalizes a render output to a standard shape { html, swapMode }
 * Handles: string, { html }, Promise<string>, Promise<{ html }>
 */
export async function resolveRenderOutput(
  output: StoryRenderOutput
): Promise<{ html: string; swapMode: "inner" | "outer" | "morph" }> {
  // Debug output
  // console.log("[Storybook] Resolving render output:", output)

  const result = await Promise.resolve(output)

  if (typeof result === "string") {
    return { html: result, swapMode: "inner" }
  }

  // Handle Effuse TemplateResult
  if (isTemplateResult(result)) {
     return { html: templateToString(result), swapMode: "inner" }
  }

  // Handle existing object shape { html, swapMode }
  if (result && typeof result === "object" && "html" in result) {
    return {
        html: (result as any).html,
        swapMode: (result as any).swapMode || "inner",
    }
  }

  return { html: `undefined (unknown output type: ${typeof result})`, swapMode: "inner" }
}

/**
 * Composes the final story function by wrapping the base render with decorators.
 * Decorators are applied "outside-in" (like Storybook), meaning the first decorator
 * in the array wraps the rest.
 */
export function composeStory(story: CsfStory): (ctx: StoryContext) => Promise<StoryRenderOutput> {
  const { render, decorators } = story

  // The innermost function calls the actual story render
  let composedFn = (ctx: StoryContext) => render(ctx.args, ctx)

  // Apply decorators in reverse order so the first one ends up on the outside
  if (decorators && decorators.length > 0) {
    for (let i = decorators.length - 1; i >= 0; i--) {
      const decorator = decorators[i]
      const previousFn = composedFn
      composedFn = (ctx: StoryContext) => decorator((args, c) => previousFn({ ...c, args }), ctx)
    }
  }

  return async (ctx: StoryContext) => composedFn(ctx)
}
