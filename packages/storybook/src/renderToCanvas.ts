/**
 * @since 1.0.0
 */

import type { RenderContext } from "@storybook/types"
import type { Fx } from "@typed/fx/Fx"
import { renderToLayer } from "@typed/template"
import * as Effect from "effect/Effect"
import * as Fiber from "effect/Fiber"
import * as Runtime from "effect/Runtime"
import type { TypedRenderer } from "./types.js"

/**
 * Main render function that integrates Typed components with Storybook
 *
 * @since 1.0.0
 * @category Rendering
 */
// Store the current fiber for cleanup
let currentFiber: Fiber.RuntimeFiber<any, any> | null = null

export const renderToCanvas = async (
  renderContext: RenderContext<TypedRenderer>,
  canvasElement: HTMLElement
) => {
  const { showError, showMain, storyFn } = renderContext

  try {
    // Clean up any existing fiber
    if (currentFiber) {
      await Runtime.runPromise(Runtime.defaultRuntime)(Fiber.interrupt(currentFiber))
      currentFiber = null
    }

    // Clear the canvas
    canvasElement.innerHTML = ""

    // Get the story result
    const storyResult = storyFn()

    // Check if this is an HTML element or a Typed Fx
    if (storyResult instanceof HTMLElement) {
      // Direct HTML element - just append it
      canvasElement.appendChild(storyResult)
      showMain()
      return
    }

    // Handle Typed Fx components
    if (storyResult && typeof storyResult === "object" && "_tag" in storyResult) {
      const runtime = Runtime.defaultRuntime

      // Create a render effect
      const renderEffect = Effect.gen(function*() {
        // Create a container for the Typed component
        const container = document.createElement("div")
        container.style.width = "100%"
        container.style.height = "100%"
        canvasElement.appendChild(container)

        // Use renderToLayer to render the Fx
        const layer = yield* renderToLayer(storyResult as Fx<any, never, any>, container)

        // Return the layer for potential cleanup
        return layer
      })

      // Run the effect as a fiber
      const fiber = await Runtime.runFork(runtime)(renderEffect)
      currentFiber = fiber as any

      // Wait for initial render
      await Runtime.runPromise(runtime)(Fiber.join(fiber))

      showMain()
    } else {
      // Fallback for unknown types
      const element = document.createElement("div")
      element.style.padding = "20px"
      element.style.fontFamily = "'Berkeley Mono', monospace"
      element.style.backgroundColor = "#000000"
      element.style.color = "#ffffff"
      element.textContent = "Unknown story type"
      canvasElement.appendChild(element)
      showMain()
    }
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
  // Interrupt any running fiber
  if (currentFiber) {
    await Runtime.runPromise(Runtime.defaultRuntime)(Fiber.interrupt(currentFiber))
    currentFiber = null
  }
}
