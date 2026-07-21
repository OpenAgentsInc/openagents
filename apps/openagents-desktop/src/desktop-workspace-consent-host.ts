/**
 * Durable one-time workspace-consent host (#9157 — main only).
 *
 * Persists a single record at `<userData>/workspace-consent.json` so the
 * first-run "Choose your workspace folder" step is asked ONCE, not on every
 * launch. The decision (grant + chosen folder, or decline) survives relaunch.
 *
 * Security posture (mirrors desktop-preferences-host):
 * - Written mode 0600 (owner read/write only) via temp+rename, then re-chmod'd.
 * - The record only ever holds a bounded status enum, one directory path, and a
 *   timestamp — never secrets — so read/write is never sensitive.
 * - A missing, corrupt, partial, or future-shaped file NEVER throws: the read
 *   returns null (treated as "not yet decided") and the caller re-onboards.
 */
import { chmodSync, mkdirSync, readFileSync, renameSync, writeFileSync } from "node:fs"
import path from "node:path"

import { Schema } from "effect"

import {
  DESKTOP_WORKSPACE_CONSENT_SCHEMA_ID,
  DESKTOP_WORKSPACE_CONSENT_VERSION,
  DesktopWorkspaceConsentSchema,
  type DesktopWorkspaceConsent,
  type DesktopWorkspaceConsentStatus,
} from "./desktop-workspace-consent.ts"

const OWNER_ONLY = 0o600

const decodeConsent = Schema.decodeUnknownSync(DesktopWorkspaceConsentSchema)

export type DesktopWorkspaceConsentStore = Readonly<{
  /** The decided record, or null when nothing valid has been persisted yet. */
  snapshot: () => DesktopWorkspaceConsent | null
  /** Persist a decision and return the stored record. */
  record: (input: Readonly<{
    status: DesktopWorkspaceConsentStatus
    workspaceRoot: string | null
    decidedAt: string
  }>) => DesktopWorkspaceConsent
}>

export const openDesktopWorkspaceConsentStore = (filePath: string): DesktopWorkspaceConsentStore => {
  const read = (): DesktopWorkspaceConsent | null => {
    try {
      return decodeConsent(JSON.parse(readFileSync(filePath, "utf8")))
    } catch {
      // Missing, corrupt, partial, or future-shaped: treat as "not decided".
      return null
    }
  }

  const writeDocument = (value: DesktopWorkspaceConsent): void => {
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
    record: (input) => {
      const value: DesktopWorkspaceConsent = {
        schemaId: DESKTOP_WORKSPACE_CONSENT_SCHEMA_ID,
        version: DESKTOP_WORKSPACE_CONSENT_VERSION,
        status: input.status,
        workspaceRoot: input.status === "granted" ? input.workspaceRoot : null,
        decidedAt: input.decidedAt,
      }
      writeDocument(value)
      return value
    },
  }
}
