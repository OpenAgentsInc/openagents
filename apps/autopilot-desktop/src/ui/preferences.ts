// Functional Settings preferences — #5472 (EPIC #5461).
//
// THE PREFERENCES SEAM (read this before adding a setting)
// ────────────────────────────────────────────────────────────────────────────
// Settings used to be informational only (install-readiness + connection were
// live; Theme / Notifications / Updates were static copy with no controls).
// This module is the single home for the *interactive* preferences and their
// local persistence, so the hot central Foldkit files (model.ts / update.ts /
// view.ts) only need MINIMAL, append-style edits:
//   - model.ts holds the chosen values as ordinary fields (one literal each).
//   - update.ts maps each Changed* message to a field write + a PersistPreferences
//     command (defined in commands.ts) — no new RPC verb, no Bun contract change.
//   - view.ts renders the controls and reads model.themePreference for the live
//     theme attribute.
//
// Persistence is LOCAL and refs-only: the four small enum/boolean choices are
// written to `localStorage` under one namespaced key. We deliberately do NOT
// persist (or even read) anything that could overwrite the Pylon home, identity,
// account refs, or any node-owned authority — preferences are presentation +
// spawn-form defaults only. `localStorage` is guarded so the pure reducer/view
// stay testable under `bun test` (no DOM): a missing global is a silent no-op.
//
// Honest scope: Theme and the spawn defaults take real effect immediately. The
// in-app notification center toggle takes real effect (it gates the Settings
// notification panel). The *OS* notification channel fires from the Bun poll
// loop (`src/bun/notifier.ts`) and is NOT yet user-gateable from here — the
// Settings copy says so rather than pretending a control exists.

import { Option, Schema as S } from "effect"

// The spawn adapter/lane literals MUST match the Model's spawn fields so a saved
// default can seed them directly. Kept here as the single source the settings
// controls and the seed-at-init path both read.
export const ThemePreference = S.Literals(["dark", "light"])
export type ThemePreference = typeof ThemePreference.Type

export const DefaultAdapter = S.Literals(["codex", "claude_agent", "apple_fm"])
export type DefaultAdapter = typeof DefaultAdapter.Type

export const DefaultLane = S.Literals(["auto", "local", "cloud-gcp", "cloud-shc"])
export type DefaultLane = typeof DefaultLane.Type

// #5485 (EPIC #5474): the OpenAgents inference-gateway fallback preference.
//   • "auto" — when there is no usable own Claude/Codex auth, route coding-turn
//              inference through the OpenAgents gateway (pay-as-you-go credits).
//              The conversion-friendly default: a fresh user with no own keys
//              can still run coding turns. BYO-auth always wins when present.
//   • "off"  — never auto-route through the gateway; require own auth.
// Presentation/spawn-default only — the API key + credit ledger live server-
// side; this just expresses the user's routing intent.
export const GatewayInferenceFallback = S.Literals(["auto", "off"])
export type GatewayInferenceFallback = typeof GatewayInferenceFallback.Type

// The persisted, presentation-only preference record. Refs-only: NO identity,
// home, token, or account-ref fields ever live here (those are node authority).
export const Preferences = S.Struct({
  theme: ThemePreference,
  defaultAdapter: DefaultAdapter,
  defaultLane: DefaultLane,
  // Whether the in-app Settings notification panel is shown. A real local
  // effect (the panel honours it); does NOT gate the Bun-side OS channel.
  showNotificationPanel: S.Boolean,
  // #5485: route coding inference through the OpenAgents gateway when there is
  // no usable own auth (the default). "off" requires own Claude/Codex auth.
  gatewayInferenceFallback: GatewayInferenceFallback,
})
export type Preferences = typeof Preferences.Type

export const defaultPreferences: Preferences = {
  theme: "dark",
  defaultAdapter: "codex",
  defaultLane: "auto",
  showNotificationPanel: true,
  gatewayInferenceFallback: "auto",
}

// One namespaced key; bump the suffix if the shape ever changes incompatibly.
// v2 (#5485): added `gatewayInferenceFallback`. An old v1 blob simply decodes to
// defaults under the new required field, which is the desired "auto" fallback.
export const PREFERENCES_STORAGE_KEY = "autopilot-desktop.preferences.v2"

// Decode to an Option (the codebase idiom for opaque/persisted payloads): None
// on any shape mismatch, so corruption falls back to defaults cleanly.
const decodePreferences = S.decodeUnknownOption(Preferences)

// Best-effort localStorage access. Returns null when there is no DOM (tests /
// the Bun host) so callers degrade to defaults rather than throwing.
const storage = (): Storage | null => {
  try {
    // `localStorage` is a global in the Electrobun webview; absent under bun test.
    return typeof localStorage === "undefined" ? null : localStorage
  } catch {
    // Some sandboxes throw on access (e.g. blocked storage). Treat as absent.
    return null
  }
}

// Load saved preferences, falling back to defaults on absence/corruption. Pure
// w.r.t. the rest of the app — never throws, never partially-applies a bad blob.
export const loadPreferences = (): Preferences => {
  const store = storage()
  if (store === null) return defaultPreferences
  const raw = (() => {
    try {
      return store.getItem(PREFERENCES_STORAGE_KEY)
    } catch {
      return null
    }
  })()
  if (raw === null || raw.trim() === "") return defaultPreferences
  let parsed: unknown
  try {
    parsed = JSON.parse(raw)
  } catch {
    return defaultPreferences
  }
  return Option.getOrElse(decodePreferences(parsed), () => defaultPreferences)
}

// Persist preferences. Best-effort: a storage failure must never break the UI,
// so we swallow it (the in-memory model already reflects the choice this turn).
export const savePreferences = (preferences: Preferences): void => {
  const store = storage()
  if (store === null) return
  try {
    store.setItem(PREFERENCES_STORAGE_KEY, JSON.stringify(preferences))
  } catch {
    // ignore — local persistence is a convenience, not an invariant.
  }
}

// The `data-theme` attribute value the app shell carries so the (central) CSS
// can restyle for light mode. `dark` is the canonical default and matches the
// hard-coded dark palette already in index.html, so a missing attribute and
// `data-theme="dark"` look identical.
export const themeAttr = (theme: ThemePreference): string => theme
