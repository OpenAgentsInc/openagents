/**
 * @since 1.0.0
 */

import type { RenderContext } from "@storybook/types"
import type { TypedRenderer } from "./types.js"

/**
 * Storybook v3 renderer.
 * @since 1.0.0
 */
export async function renderToCanvas(
  { showError, showMain, storyContext, storyFn }: RenderContext<TypedRenderer>,
  rootElement: TypedRenderer["canvasElement"]
) {
  try {
    // Clear the canvas
    rootElement.innerHTML = ""

    // Get the story result - it should be an Fx
    storyFn(storyContext)
    
    // For now, create a placeholder that shows we received the Fx
    const placeholder = document.createElement("div")
    placeholder.style.padding = "20px"
    placeholder.style.fontFamily = "'Berkeley Mono', monospace"
    placeholder.style.backgroundColor = "#000000"
    placeholder.style.color = "#ffffff"
    placeholder.style.border = "1px solid #ffffff"
    placeholder.textContent = "Typed component rendered (renderToCanvas needs implementation)"
    
    rootElement.appendChild(placeholder)
    
    showMain()
  } catch (error) {
    console.error("Render error:", error)
    showError({ 
      title: "Render Error", 
      description: error instanceof Error ? error.message : String(error) 
    })
  }
}