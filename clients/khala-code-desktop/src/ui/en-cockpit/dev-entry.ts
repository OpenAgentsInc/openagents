import { Effect } from "@effect-native/core/effect"

import { enCockpitFixtureStatus } from "./cockpit-fixture"
import { mountEnCockpitSurface } from "./cockpit-mount"

// Dev-only entry for the Effect Native fleet cockpit proof (MH-7 / EN-5).
//
// This coexists with the shipping shell exactly the way EN-1's `/stage1` route
// coexists with `/`: it is a SEPARATE page (`en-cockpit.html`), reachable via
// `vite dev` (e.g. `bun run dev:hmr` → http://localhost:5173/en-cockpit.html),
// and it is NOT included in the packaged Electrobun build (`build:ui` only
// emits `index.html`). It never touches or replaces any existing desktop
// screen — it renders one real fleet cockpit screen through the real Effect
// Native DOM renderer against a live-shaped fixture.
const bootEnCockpitDevPage = (): void => {
  const root = document.getElementById("en-cockpit-root")
  if (root === null) return

  const log = document.getElementById("en-cockpit-intent-log")
  const appendLog = (line: string): void => {
    if (log === null) return
    const entry = document.createElement("div")
    entry.className = "en-cockpit-intent-log-entry"
    entry.textContent = line
    log.prepend(entry)
  }

  // Fork a scoped program: the mounted surface's scope stays open for the
  // page lifetime (`Effect.never`), so we never tear the cockpit down. Using
  // `runFork` (not `runPromise`) is the correct boundary for a long-lived
  // mounted surface.
  Effect.runFork(
    Effect.scoped(
      Effect.gen(function* () {
        yield* mountEnCockpitSurface(root, {
          initialStatus: enCockpitFixtureStatus,
          onFleetIntent: (intent) => {
            const summary =
              intent.kind === "fleet_run_control"
                ? `${intent.kind}: ${intent.action}`
                : intent.kind === "approval_decision"
                  ? `${intent.kind}: ${intent.decision} ${intent.approvalRef}`
                  : intent.kind === "worker_selection"
                    ? `${intent.kind}: ${intent.workerKind}`
                    : intent.kind
            appendLog(`${intent.intentId} — ${summary}`)
          },
        })
        return yield* Effect.never
      }).pipe(
        Effect.catchCause((cause: unknown) =>
          Effect.sync(() => appendLog(`mount error: ${String(cause)}`)),
        ),
      ),
    ),
  )
}

bootEnCockpitDevPage()
