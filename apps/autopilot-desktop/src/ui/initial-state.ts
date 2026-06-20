import type { Command } from "foldkit"

import type { Message } from "./message"
import { LoadOnboardingStatus } from "./commands"
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

// VERSE HOME (owner directive, 2026-06-20): the app launches into the Verse
// chat/world surface. The fallback shell and the full Code/Supervise panes are
// still mounted and reachable, but the first paint is Pylons + Tassadar + one
// chat bar instead of a coding target selector.
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
    // The hands-off default surface: the Verse world plus one chat bar.
    pane: "chat",
    onboardingPending: true,
  })

  // Warm the character-creation/Pylon readiness projection on first paint so
  // the Verse can show an honest ready/loading/blocker state without requiring
  // the user to open another pane.
  return [model, [LoadOnboardingStatus()]]
}
