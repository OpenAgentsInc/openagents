/**
 * Typed, durable, migratable desktop preferences (CUT-24 criterion 1, #8704).
 *
 * The daily coding app persists a single versioned preferences document at
 * `<userData>/preferences.json` (mode 0600, written by
 * `desktop-preferences-host.ts`). This contract owns the SCHEMA and the
 * MIGRATION CHAIN; the host owns the file IO.
 *
 * Scope note (honest, per the issue's nine listed keys):
 * - `theme` is intentionally NOT a preference: the app is the single fixed
 *   Protoss-blue `khalaTheme` with no light variant and no runtime theme switch
 *   (see workspace policy "uniform StarCraft blue everywhere"). It is recorded
 *   here as a fixed, read-only fact rather than a mutable field.
 * - `keybindings` already have a typed durable store (`desktop-command-bindings`
 *   → `<userData>/commands/bindings.json`); this document does not duplicate
 *   them, it references that store as the authority.
 * - The remaining seven keys — density, font, reduced-motion, provider-defaults,
 *   privacy, notifications, and update preferences — get typed durable schemas
 *   HERE. Density, font, and reduced-motion are genuinely consumed (a scaled
 *   theme + a reduced-motion root attribute, see `desktop-preferences-effects`).
 *   Provider-defaults, privacy, notifications, and update preferences are
 *   durable and surfaced, consumed where a real effect already exists and
 *   otherwise carried honestly until their consumer lands.
 *
 * Security posture: every field is a bounded enum or boolean or a PUBLIC-SAFE
 * account ref (`[A-Za-z0-9][A-Za-z0-9._-]{0,79}`). No secrets, tokens, prompts,
 * file contents, or free-form user text are ever stored here.
 */
import { Schema } from "effect"

export const DESKTOP_PREFERENCES_SCHEMA_ID =
  "openagents.desktop.preferences.store.v2" as const

/** Additive IPC channels (main ↔ renderer). Public-safe payloads only. */
export const DesktopPreferencesGetChannel = "openagents-desktop/preferences-get" as const
export const DesktopPreferencesUpdateChannel = "openagents-desktop/preferences-update" as const
export const DesktopPreferencesResetChannel = "openagents-desktop/preferences-reset" as const

/** Current on-disk document version. Bump + add a migration when the shape changes. */
export const DESKTOP_PREFERENCES_VERSION = 2 as const

// ---------------------------------------------------------------------------
// Field vocabularies (bounded enums — never free text).
// ---------------------------------------------------------------------------

/** UI density → scales spacing/control tokens (see effects module). */
export const desktopDensityValues = ["comfortable", "cozy", "compact"] as const
export type DesktopDensity = (typeof desktopDensityValues)[number]

/** Font size → scales the type-scale tokens (see effects module). */
export const desktopFontScaleValues = ["small", "default", "large", "x-large"] as const
export type DesktopFontScale = (typeof desktopFontScaleValues)[number]

/**
 * Reduced motion: `system` defers to the OS `prefers-reduced-motion` media
 * query; `always`/`never` are an explicit user override honored by the app CSS
 * regardless of the OS setting.
 */
export const desktopReducedMotionValues = ["system", "always", "never"] as const
export type DesktopReducedMotion = (typeof desktopReducedMotionValues)[number]

/** Default coding provider for a new turn. `auto` keeps existing routing. */
export const desktopDefaultProviderValues = ["auto", "codex", "claude", "fable"] as const
export type DesktopDefaultProvider = (typeof desktopDefaultProviderValues)[number]

/** Update channel. `stable` takes the `latest` badge; `rc` opts into pre-releases. */
export const desktopUpdateChannelValues = ["stable", "rc"] as const
export type DesktopUpdateChannel = (typeof desktopUpdateChannelValues)[number]

/** A public-safe account ref (never a token). Mirrors the accounts contract charset. */
const PublicAccountRef = Schema.String.check(
  Schema.isMaxLength(80),
  Schema.isPattern(/^[A-Za-z0-9][A-Za-z0-9._-]{0,79}$/),
)

// ---------------------------------------------------------------------------
// Section schemas.
// ---------------------------------------------------------------------------

export const DesktopAppearancePreferencesSchema = Schema.Struct({
  density: Schema.Literals(desktopDensityValues),
  fontScale: Schema.Literals(desktopFontScaleValues),
  reducedMotion: Schema.Literals(desktopReducedMotionValues),
})
export type DesktopAppearancePreferences = typeof DesktopAppearancePreferencesSchema.Type

export const DesktopProviderDefaultsSchema = Schema.Struct({
  defaultProvider: Schema.Literals(desktopDefaultProviderValues),
  /** Public-safe preferred Codex account ref, or null for "no preference". */
  defaultCodexAccountRef: Schema.NullOr(PublicAccountRef),
  /** Public-safe preferred Claude account ref, or null for "no preference". */
  defaultClaudeAccountRef: Schema.NullOr(PublicAccountRef),
})
export type DesktopProviderDefaults = typeof DesktopProviderDefaultsSchema.Type

export const DesktopPrivacyPreferencesSchema = Schema.Struct({
  /**
   * Always redact diagnostics exports. Defaults ON and, for the current build,
   * is effectively pinned: the diagnostics export path only ever emits the
   * redacted bundle, so a `false` here cannot leak secrets.
   */
  redactDiagnosticsExport: Schema.Boolean,
  /** Off by default. Analytics expansion is an explicit non-goal for CUT-24. */
  shareCrashDiagnostics: Schema.Boolean,
})
export type DesktopPrivacyPreferences = typeof DesktopPrivacyPreferencesSchema.Type

export const DesktopNotificationPreferencesSchema = Schema.Struct({
  /** Show the in-app attention badge/count for children that need attention. */
  attentionBadge: Schema.Boolean,
  /** Announce background task/agent completion (ref-only; never prompt/code). */
  taskCompletion: Schema.Boolean,
  /** Only surface notifications when the app window is not focused. */
  onlyWhenUnfocused: Schema.Boolean,
})
export type DesktopNotificationPreferences = typeof DesktopNotificationPreferencesSchema.Type

export const DesktopUpdatePreferencesSchema = Schema.Struct({
  channel: Schema.Literals(desktopUpdateChannelValues),
  /** Periodically check for updates. */
  autoCheck: Schema.Boolean,
  /** Download an available update in the background (still user-applied). */
  autoDownload: Schema.Boolean,
})
export type DesktopUpdatePreferences = typeof DesktopUpdatePreferencesSchema.Type

/** Durable shell presentation preferences. Domain navigation remains elsewhere. */
export const DesktopPresentationPreferencesSchema = Schema.Struct({
  sidebarCollapsed: Schema.Boolean,
})
export type DesktopPresentationPreferences = typeof DesktopPresentationPreferencesSchema.Type

// ---------------------------------------------------------------------------
// Root document (v2).
// ---------------------------------------------------------------------------

export const DesktopPreferencesSchema = Schema.Struct({
  schema: Schema.Literal(DESKTOP_PREFERENCES_SCHEMA_ID),
  version: Schema.Literal(DESKTOP_PREFERENCES_VERSION),
  appearance: DesktopAppearancePreferencesSchema,
  providerDefaults: DesktopProviderDefaultsSchema,
  privacy: DesktopPrivacyPreferencesSchema,
  notifications: DesktopNotificationPreferencesSchema,
  updates: DesktopUpdatePreferencesSchema,
  presentation: DesktopPresentationPreferencesSchema,
})
export type DesktopPreferences = typeof DesktopPreferencesSchema.Type

/** The single canonical default document. */
export const defaultDesktopPreferences = (): DesktopPreferences => ({
  schema: DESKTOP_PREFERENCES_SCHEMA_ID,
  version: DESKTOP_PREFERENCES_VERSION,
  appearance: {
    density: "comfortable",
    fontScale: "default",
    reducedMotion: "system",
  },
  providerDefaults: {
    defaultProvider: "auto",
    defaultCodexAccountRef: null,
    defaultClaudeAccountRef: null,
  },
  privacy: {
    redactDiagnosticsExport: true,
    shareCrashDiagnostics: false,
  },
  notifications: {
    attentionBadge: true,
    taskCompletion: true,
    onlyWhenUnfocused: true,
  },
  updates: {
    channel: "stable",
    autoCheck: true,
    autoDownload: false,
  },
  presentation: {
    sidebarCollapsed: false,
  },
})

const decodeExit = Schema.decodeUnknownExit(DesktopPreferencesSchema)

/** Decode a fully-formed current document, or null if it is not exactly valid. */
export const decodeDesktopPreferences = (value: unknown): DesktopPreferences | null => {
  const decoded = decodeExit(value)
  return decoded._tag === "Success" ? decoded.value : null
}

// ---------------------------------------------------------------------------
// Migration chain.
//
// `migrateDesktopPreferences` is total: any input (missing, corrupt, partial,
// legacy-shaped, future-versioned) resolves to a valid current document. It
// never throws. It reports what it did so the host can re-persist on upgrade
// and so tests can assert the exact migration path.
// ---------------------------------------------------------------------------

export type DesktopPreferencesMigrationOrigin =
  | "current" // already a valid current-version document
  | "defaults" // unusable input → seeded from defaults
  | "merged" // partial/dirty current-version input → per-field defaults filled
  | "legacy_v0" // pre-versioned flat blob lifted into the current shape
  | "legacy_v1" // nested v1 document lifted into v2 with presentation defaults
  | "downgraded" // future version → unknown fields dropped, known ones kept

export type DesktopPreferencesMigrationResult = Readonly<{
  preferences: DesktopPreferences
  origin: DesktopPreferencesMigrationOrigin
  /** True when the on-disk bytes should be rewritten to the normalized form. */
  changed: boolean
  /** The version we read from, or null when none was present. */
  fromVersion: number | null
}>

const asRecord = (value: unknown): Record<string, unknown> | null =>
  typeof value === "object" && value !== null && !Array.isArray(value)
    ? (value as Record<string, unknown>)
    : null

const oneOf = <T extends string>(
  value: unknown,
  allowed: ReadonlyArray<T>,
  fallback: T,
): T => (typeof value === "string" && (allowed as ReadonlyArray<string>).includes(value) ? (value as T) : fallback)

const bool = (value: unknown, fallback: boolean): boolean =>
  typeof value === "boolean" ? value : fallback

const ref = (value: unknown): string | null =>
  typeof value === "string" && /^[A-Za-z0-9][A-Za-z0-9._-]{0,79}$/.test(value) ? value : null

/**
 * Field-wise normalize an arbitrary record into a full v2 document, filling
 * every missing/invalid field from defaults. Used for both `merged` (dirty current)
 * and `legacy_v0` (flat blob) paths — the difference is only WHERE we look for
 * each field, handled by the two accessor sets below.
 */
const normalize = (
  appearance: Record<string, unknown>,
  providerDefaults: Record<string, unknown>,
  privacy: Record<string, unknown>,
  notifications: Record<string, unknown>,
  updates: Record<string, unknown>,
  presentation: Record<string, unknown>,
): DesktopPreferences => {
  const d = defaultDesktopPreferences()
  return {
    schema: DESKTOP_PREFERENCES_SCHEMA_ID,
    version: DESKTOP_PREFERENCES_VERSION,
    appearance: {
      density: oneOf(appearance.density, desktopDensityValues, d.appearance.density),
      fontScale: oneOf(appearance.fontScale, desktopFontScaleValues, d.appearance.fontScale),
      reducedMotion: oneOf(appearance.reducedMotion, desktopReducedMotionValues, d.appearance.reducedMotion),
    },
    providerDefaults: {
      defaultProvider: oneOf(providerDefaults.defaultProvider, desktopDefaultProviderValues, d.providerDefaults.defaultProvider),
      defaultCodexAccountRef: ref(providerDefaults.defaultCodexAccountRef),
      defaultClaudeAccountRef: ref(providerDefaults.defaultClaudeAccountRef),
    },
    privacy: {
      redactDiagnosticsExport: bool(privacy.redactDiagnosticsExport, d.privacy.redactDiagnosticsExport),
      shareCrashDiagnostics: bool(privacy.shareCrashDiagnostics, d.privacy.shareCrashDiagnostics),
    },
    notifications: {
      attentionBadge: bool(notifications.attentionBadge, d.notifications.attentionBadge),
      taskCompletion: bool(notifications.taskCompletion, d.notifications.taskCompletion),
      onlyWhenUnfocused: bool(notifications.onlyWhenUnfocused, d.notifications.onlyWhenUnfocused),
    },
    updates: {
      channel: oneOf(updates.channel, desktopUpdateChannelValues, d.updates.channel),
      autoCheck: bool(updates.autoCheck, d.updates.autoCheck),
      autoDownload: bool(updates.autoDownload, d.updates.autoDownload),
    },
    presentation: {
      sidebarCollapsed: bool(presentation.sidebarCollapsed, d.presentation.sidebarCollapsed),
    },
  }
}

/**
 * A single section-object of a patch (or {} if the input is not an object).
 * Used at the IPC boundary so a hostile non-object patch can never corrupt the
 * merge; the host still re-normalizes the merged result through the migrator.
 */
export type DesktopPreferencesPatch = {
  readonly appearance?: Record<string, unknown>
  readonly providerDefaults?: Record<string, unknown>
  readonly privacy?: Record<string, unknown>
  readonly notifications?: Record<string, unknown>
  readonly updates?: Record<string, unknown>
  readonly presentation?: Record<string, unknown>
}

const sectionKeys = ["appearance", "providerDefaults", "privacy", "notifications", "updates", "presentation"] as const

/** Decode an untrusted patch: keep only known sections that are plain objects. */
export const decodeDesktopPreferencesPatch = (value: unknown): DesktopPreferencesPatch => {
  const record = asRecord(value)
  if (record === null) return {}
  const patch: Record<string, Record<string, unknown>> = {}
  for (const key of sectionKeys) {
    const section = asRecord(record[key])
    if (section !== null) patch[key] = section
  }
  return patch
}

export const migrateDesktopPreferences = (raw: unknown): DesktopPreferencesMigrationResult => {
  const record = asRecord(raw)
  if (record === null) {
    return { preferences: defaultDesktopPreferences(), origin: "defaults", changed: true, fromVersion: null }
  }

  const rawVersion = typeof record.version === "number" ? record.version : null

  // Exact current-version document: accept as-is if it decodes cleanly.
  if (rawVersion === DESKTOP_PREFERENCES_VERSION) {
    const clean = decodeDesktopPreferences(record)
    if (clean !== null) {
      return { preferences: clean, origin: "current", changed: false, fromVersion: rawVersion }
    }
    // Dirty current document: normalize per-field so the host rewrites it.
    const normalized = normalize(
      asRecord(record.appearance) ?? {},
      asRecord(record.providerDefaults) ?? {},
      asRecord(record.privacy) ?? {},
      asRecord(record.notifications) ?? {},
      asRecord(record.updates) ?? {},
      asRecord(record.presentation) ?? {},
    )
    return { preferences: normalized, origin: "merged", changed: true, fromVersion: rawVersion }
  }

  // Future version: keep the known fields, drop everything unknown, re-stamp
  // to the current version.
  if (rawVersion !== null && rawVersion > DESKTOP_PREFERENCES_VERSION) {
    const normalized = normalize(
      asRecord(record.appearance) ?? {},
      asRecord(record.providerDefaults) ?? {},
      asRecord(record.privacy) ?? {},
      asRecord(record.notifications) ?? {},
      asRecord(record.updates) ?? {},
      asRecord(record.presentation) ?? {},
    )
    return { preferences: normalized, origin: "downgraded", changed: true, fromVersion: rawVersion }
  }

  // Version 1 used the same nested sections but had no presentation section.
  // Preserve every valid v1 value and seed the new bounded presentation state.
  if (rawVersion === 1) {
    const normalized = normalize(
      asRecord(record.appearance) ?? {},
      asRecord(record.providerDefaults) ?? {},
      asRecord(record.privacy) ?? {},
      asRecord(record.notifications) ?? {},
      asRecord(record.updates) ?? {},
      {},
    )
    return { preferences: normalized, origin: "legacy_v1", changed: true, fromVersion: rawVersion }
  }

  // Legacy pre-versioned (v0 or no version): a flat blob where appearance keys
  // may live at the top level. Lift them into the current nested shape.
  const normalized = normalize(
    { ...(asRecord(record.appearance) ?? {}), density: record.density, fontScale: record.fontScale, reducedMotion: record.reducedMotion },
    asRecord(record.providerDefaults) ?? {},
    asRecord(record.privacy) ?? {},
    asRecord(record.notifications) ?? {},
    asRecord(record.updates) ?? {},
    asRecord(record.presentation) ?? {},
  )
  return { preferences: normalized, origin: "legacy_v0", changed: true, fromVersion: rawVersion }
}
