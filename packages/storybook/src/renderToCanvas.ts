/**
 * @since 1.0.0
 */

import type { RenderContext } from "@storybook/types"
import * as Effect from "effect/Effect"
import * as Runtime from "effect/Runtime"
import type { TypedRenderer } from "./types.js"

/**
 * Main render function that integrates Typed components with Storybook
 *
 * @since 1.0.0
 * @category Rendering
 */
export const renderToCanvas = async (
  renderContext: RenderContext<TypedRenderer>,
  canvasElement: HTMLElement
) => {
  const { showError, showMain, storyFn } = renderContext

  try {
    // Clear the canvas
    canvasElement.innerHTML = ""

    // Get the story result
    const storyResult = storyFn()
    console.log("Story result:", storyResult)

    // Create a simple runtime for rendering
    const runtime = Runtime.defaultRuntime

    // For now, create a simple HTML render
    const renderEffect = Effect.gen(function*() {
      // Basic rendering - we'll improve this as we iterate
      const element = document.createElement("div")
      element.style.padding = "20px"
      element.style.fontFamily = "'Berkeley Mono', monospace"
      element.style.backgroundColor = "#000000"
      element.style.color = "#ffffff"
      element.innerHTML = "Story component placeholder - this will be replaced with actual Typed rendering"

      canvasElement.appendChild(element)

      yield* Effect.void
      return "rendered"
    })

    // Run the effect
    await Runtime.runPromise(runtime)(renderEffect)

    showMain()
  } catch (error) {
    console.error("Storybook render error:", error)
    const errorObj = error instanceof Error ? error : new Error(String(error))
    showError({
      title: "Render Error",
      description: errorObj.message
    })
  }
}

/**
 * Cleanup function called when Storybook unmounts
 *
 * @since 1.0.0
 * @category Lifecycle
 */
export const cleanup = async () => {
  // Cleanup logic if needed
}
