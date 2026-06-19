import type { Command } from "foldkit"

import type { Message } from "./message"
import { initialModel, Model } from "./model"
// #5472: load the locally-persisted Settings preferences and apply them to the
// initial model so theme + spawn defaults take effect from app entry. Loaded
// here (the real app entry), not in `initialModel`, so the shared neutral base
// stays deterministic for the view/update tests (which never touch storage).
import { loadPreferences } from "./preferences"

type InitialRuntimeState = readonly [
  Model,
  ReadonlyArray<Command.Command<Message>>,
]

// ZERO-BASE SHELL (owner directive, 2026-06-19): the app launches to a black
// screen with NOTHING on it except the bottom text bar — the `shell` pane. We
// deliberately fire NO warm-up commands at entry (no onboarding/proof/activity/
// gateway loaders), so the default screen stays quiet and black and nothing
// pulls the user onto another surface. Every pane the old UI had is KEPT and
// still mounts; each warms ITS OWN projections lazily on open (the NavigatedTo
// handler in update.ts already loads per-pane data when a pane is entered via
// the Cmd-K palette / "open panes"). One thing at a time.
export const initialRuntimeState = (): InitialRuntimeState => {
  const preferences = loadPreferences()
  const model = Model.make({
    ...initialModel,
    // #5472: apply the saved preferences. `defaultAdapter`/`defaultLane` ALSO
    // seed the live spawn fields so the saved default is what spawn/composer/
    // chat use without any extra wiring. Identity / Pylon home are untouched.
    themePreference: preferences.theme,
    defaultAdapter: preferences.defaultAdapter,
    defaultLane: preferences.defaultLane,
    showNotificationPanel: preferences.showNotificationPanel,
    // #5485: apply the saved gateway-fallback intent so the routing decision
    // (own-auth vs gateway) honours it once the user opens a coding surface.
    gatewayInferenceFallback: preferences.gatewayInferenceFallback,
    spawnAdapter: preferences.defaultAdapter,
    spawnLane: preferences.defaultLane,
    // The dead-simple default surface: black + the bottom text bar.
    pane: "shell",
  })

  // No entry commands: the shell needs nothing loaded to render its black
  // screen + input. Panes warm lazily on open (NavigatedTo in update.ts).
  return [model, []]
}
