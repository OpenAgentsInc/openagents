import { Effect } from "effect"
import { EffuseLive, mountComponent } from "./effuse/index.js"
import { StatusDashboardComponent } from "./components/status-dashboard/index.js"
import { setupStorybookListener } from "./effuse-storybook/index.js"

const program = Effect.gen(function* () {
  const container = document.getElementById("root")
  if (!container) {
    throw new Error("Root element not found")
  }

  if (import.meta.env.DEV) {
    setupStorybookListener()
  }

  yield* mountComponent(StatusDashboardComponent, container)

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
