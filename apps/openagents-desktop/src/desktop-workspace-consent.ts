/**
 * One-time workspace-consent contract for OpenAgents Desktop (#9157).
 *
 * macOS will not let a custom dialog pre-grant arbitrary folder access, but it
 * DOES grant scoped, TCC-prompt-free access to a folder the user selects
 * through the native open panel — the selection IS the consent. Rather than
 * eagerly enumerate protected folders (Desktop/Documents/Downloads) or start at
 * the filesystem root — which fires one TCC dialog per protected location — the
 * app asks ONCE, on first run, for a workspace folder and then operates within
 * that scope.
 *
 * This module owns the durable SCHEMA and the pure DECISION for that flow. The
 * host (`desktop-workspace-consent-host.ts`) owns file IO; Electron main owns
 * the native dialog and the workspace selection it drives.
 *
 * Security posture: the record only ever holds a bounded status enum, a single
 * chosen directory path, and a timestamp — never secrets, tokens, prompts, or
 * file contents.
 */
import { Schema } from "effect"

export const DESKTOP_WORKSPACE_CONSENT_SCHEMA_ID =
  "openagents.desktop.workspace-consent.v1" as const

export const DESKTOP_WORKSPACE_CONSENT_VERSION = 1 as const

/**
 * `granted`: the user picked a workspace folder through the native open panel;
 * `workspaceRoot` names it. `declined`: the user cancelled the one-time panel;
 * the app respects the decision, keeps a safe fallback workspace, and never
 * nags again.
 */
export const desktopWorkspaceConsentStatusValues = ["granted", "declined"] as const
export type DesktopWorkspaceConsentStatus = (typeof desktopWorkspaceConsentStatusValues)[number]

export const DesktopWorkspaceConsentSchema = Schema.Struct({
  schemaId: Schema.Literal(DESKTOP_WORKSPACE_CONSENT_SCHEMA_ID),
  version: Schema.Literal(DESKTOP_WORKSPACE_CONSENT_VERSION),
  status: Schema.Literals(desktopWorkspaceConsentStatusValues),
  /** The granted workspace folder, or null when the panel was declined. */
  workspaceRoot: Schema.NullOr(Schema.String),
  decidedAt: Schema.String,
})
export type DesktopWorkspaceConsent = typeof DesktopWorkspaceConsentSchema.Type

/**
 * The startup action the one-time consent flow must take.
 *
 * - `SkipOnboarding`: automation/fixture/isolated launches never prompt.
 * - `UseConsentedWorkspace`: a prior grant still points at a real directory.
 * - `UseFallbackWorkspace`: the user declined earlier; use the safe fallback.
 * - `RequestConsent`: first run (or a granted folder that vanished) — show the
 *   native open panel once, then persist the outcome.
 */
export type DesktopWorkspaceConsentPlan =
  | { readonly _tag: "SkipOnboarding", readonly workspaceRoot: string }
  | { readonly _tag: "UseConsentedWorkspace", readonly workspaceRoot: string }
  | { readonly _tag: "UseFallbackWorkspace", readonly workspaceRoot: string }
  | { readonly _tag: "RequestConsent", readonly defaultPath: string }

export const desktopWorkspaceConsentPlan = (input: Readonly<{
  interactiveLaunch: boolean
  consent: DesktopWorkspaceConsent | null
  launchFallbackRoot: string
  isDirectory: (candidate: string) => boolean
}>): DesktopWorkspaceConsentPlan => {
  if (!input.interactiveLaunch) {
    return { _tag: "SkipOnboarding", workspaceRoot: input.launchFallbackRoot }
  }
  const consent = input.consent
  if (consent === null) {
    return { _tag: "RequestConsent", defaultPath: input.launchFallbackRoot }
  }
  if (
    consent.status === "granted" &&
    consent.workspaceRoot !== null &&
    input.isDirectory(consent.workspaceRoot)
  ) {
    return { _tag: "UseConsentedWorkspace", workspaceRoot: consent.workspaceRoot }
  }
  if (consent.status === "granted") {
    // A previously granted folder that no longer exists: ask once more rather
    // than silently falling back to a protected default.
    return { _tag: "RequestConsent", defaultPath: input.launchFallbackRoot }
  }
  return { _tag: "UseFallbackWorkspace", workspaceRoot: input.launchFallbackRoot }
}
