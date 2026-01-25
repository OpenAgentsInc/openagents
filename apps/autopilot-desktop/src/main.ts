/**
 * Main entry point - Effuse counter example
 */

import { Effect } from "effect"
import { mountComponent, EffuseLive } from "./effuse/index.js"
import { UnifiedStreamComponent } from "./components/unified-stream/index.js"
import { setupStorybookListener } from "./effuse-storybook/index.js"

const program = Effect.gen(function* () {
  const container = document.getElementById("root")
  if (!container) {
    throw new Error("Root element not found")
  }

  // Initialize Storybook listener in Dev mode
  if (import.meta.env.DEV) {
    setupStorybookListener()
  }

  yield* mountComponent(UnifiedStreamComponent, container)

  // Keep the scoped fibers (event handlers, state watchers) alive.
  yield* Effect.never
})

Effect.runPromise(
  program.pipe(
    Effect.provide(EffuseLive),
    Effect.scoped
  )
).catch((error) => {
  console.error("Failed to mount Effuse component:", error)
})
