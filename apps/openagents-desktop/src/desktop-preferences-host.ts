/**
 * Durable desktop preferences host (CUT-24 criterion 1, #8704 — main only).
 *
 * Reads/writes the single versioned preferences document to a private JSON file
 * under the app userData root, migrating on read and re-persisting when the
 * on-disk bytes were an older/dirty/legacy shape.
 *
 * Security posture (mirrors mcp-config-host / command-bindings):
 * - Written mode 0600 (owner read/write only) via temp+rename, then re-chmod'd.
 * - The document only ever holds bounded enums, booleans, and public-safe
 *   account refs — never secrets — so read/write is never sensitive.
 * - A missing, corrupt, partial, legacy, or future-versioned file NEVER throws:
 *   `migrateDesktopPreferences` is total and the store falls back to defaults.
 */
import { chmodSync, mkdirSync, readFileSync, renameSync, writeFileSync } from "node:fs"
import path from "node:path"

import {
  defaultDesktopPreferences,
  migrateDesktopPreferences,
  type DesktopPreferences,
  type DesktopPreferencesMigrationOrigin,
  type DesktopPreferencesPatch,
} from "./desktop-preferences-contract.ts"

const OWNER_ONLY = 0o600

export type { DesktopPreferencesPatch }

export type DesktopPreferencesStore = Readonly<{
  /** The migrated current-version document (always valid). */
  snapshot: () => DesktopPreferences
  /** Merge a bounded patch, re-validate through the migrator, persist, return it. */
  update: (patch: DesktopPreferencesPatch) => DesktopPreferences
  /** Reset to the canonical defaults and persist. */
  reset: () => DesktopPreferences
  /** Diagnostic: how the last on-disk read was interpreted. */
  lastOrigin: () => DesktopPreferencesMigrationOrigin | null
}>

export const openDesktopPreferencesStore = (filePath: string): DesktopPreferencesStore => {
  let lastOrigin: DesktopPreferencesMigrationOrigin | null = null

  const read = (): DesktopPreferences => {
    let raw: unknown
    try {
      raw = JSON.parse(readFileSync(filePath, "utf8"))
    } catch {
      const result = migrateDesktopPreferences(undefined)
      lastOrigin = result.origin
      return result.preferences
    }
    const result = migrateDesktopPreferences(raw)
    lastOrigin = result.origin
    // Self-heal: if the bytes were older/dirty/legacy, rewrite the normalized form.
    if (result.changed) writeDocument(result.preferences)
    return result.preferences
  }

  const writeDocument = (value: DesktopPreferences): void => {
    mkdirSync(path.dirname(filePath), { recursive: true, mode: 0o700 })
    try {
      chmodSync(path.dirname(filePath), 0o700)
    } catch {
      // best-effort dir hardening
    }
    const temporary = `${filePath}.tmp`
    writeFileSync(temporary, `${JSON.stringify(value, null, 2)}\n`, { mode: OWNER_ONLY })
    try {
      chmodSync(temporary, OWNER_ONLY)
    } catch {
      // best-effort
    }
    renameSync(temporary, filePath)
    try {
      chmodSync(filePath, OWNER_ONLY)
    } catch {
      // rename preserved the temp's 0600 mode already
    }
  }

  return {
    snapshot: () => read(),
    update: (patch) => {
      const current = read()
      // Apply the patch, then run it back THROUGH the migrator so any bad value
      // in the patch is field-normalized (never trusted raw).
      const merged: DesktopPreferences = {
        ...current,
        appearance: { ...current.appearance, ...(patch.appearance ?? {}) },
        providerDefaults: { ...current.providerDefaults, ...(patch.providerDefaults ?? {}) },
        privacy: { ...current.privacy, ...(patch.privacy ?? {}) },
        notifications: { ...current.notifications, ...(patch.notifications ?? {}) },
        updates: { ...current.updates, ...(patch.updates ?? {}) },
        presentation: { ...current.presentation, ...(patch.presentation ?? {}) },
      }
      const normalized = migrateDesktopPreferences(merged).preferences
      writeDocument(normalized)
      return normalized
    },
    reset: () => {
      const next = defaultDesktopPreferences()
      writeDocument(next)
      lastOrigin = "current"
      return next
    },
    lastOrigin: () => lastOrigin,
  }
}
