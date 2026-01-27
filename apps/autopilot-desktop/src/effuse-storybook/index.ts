/**
 * Main Entry for Storybook Integration
 */

import { Effect, Layer, Scope } from "effect"
import { StorybookServiceLive, StorybookService } from "./app/state"
import { StorybookOverlay } from "./app/overlay"
import { EffuseLive } from "../effuse/index"

let isMounted = false

export const setupStorybookListener = () => {
  if (isMounted) return
  isMounted = true

  const program = Effect.gen(function* () {
    const service = yield* StorybookService

    // Create Mount Point
    const container = document.createElement("div")
    container.id = "effuse-storybook-root"
    container.className = "storybook-root"

    const ensureContainer = () => {
      const host = document.getElementById("effuse-storybook-host")
      if (host) {
        if (container.parentElement !== host) {
          host.appendChild(container)
        }
        return
      }
      if (!document.body.contains(container)) {
        document.body.appendChild(container)
      }
    }

    ensureContainer()

    const observer = new MutationObserver(() => {
      ensureContainer()
    })

    observer.observe(document.body, { childList: true, subtree: true })

    // Mount Overlay
    const overlay = yield* StorybookOverlay.mount(container)

    // Listen for Cmd+4, Escape, or Ctrl+Shift+S
    const handleKeydown = (e: KeyboardEvent) => {
      // DEBUG: Log ALL keydowns
      // console.log("[Storybook] Keydown:", e.key, e.code, e.ctrlKey, e.shiftKey)

      if ((e.metaKey && e.code === "Digit4") || (e.ctrlKey && e.shiftKey && e.code === "KeyS")) {
        console.log("[Storybook] Toggle triggered")
        e.preventDefault()
        e.stopPropagation() // Stop other listeners
        Effect.runFork(
          service.toggle.pipe(
            Effect.tap(() => Effect.sync(() => ensureContainer())),
            Effect.flatMap(() => overlay.refresh),
            Effect.catchAll(err => Effect.sync(() => console.error("[Storybook] Toggle failed:", err)))
          )
        )
      } else if (e.key === "Escape") {
        // Only close if open
        Effect.runFork(
          service.get.pipe(
            Effect.flatMap(state => {
              if (state.isOpen) {
                console.log("[Storybook] Escape triggered close")
                // We just want to close, so we toggle if it IS open.
                // Or better, add a specific `close` method to service, but toggle works for now if we guard.
                return service.toggle.pipe(Effect.flatMap(() => overlay.refresh))
              }
              return Effect.void
            }),
            Effect.catchAll(err => Effect.sync(() => console.error("[Storybook] Escape failed:", err)))
          )
        )
      }
    }

    // Capture phase to ensure we get it first
    window.addEventListener("keydown", handleKeydown, true)
    console.log("[Storybook] Event listener attached to window (capture phase)")

    // Cleanup
    yield* Effect.addFinalizer(() =>
      Effect.sync(() => {
        window.removeEventListener("keydown", handleKeydown, true)
        observer.disconnect()
        container.remove()
      })
    )

    // Keep the listener alive
    yield* Effect.never
  })

  // Run the program with the Service Layer
  const runnable = program.pipe(
    Effect.provide(StorybookServiceLive),
    Effect.provide(EffuseLive),
    Effect.scoped
  )

  Effect.runFork(runnable)
  console.log("[Storybook] Listener active. Press Cmd+4 to toggle.")
}
