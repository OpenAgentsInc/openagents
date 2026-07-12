/**
 * Diagnostics host (CUT-24 criterion 4, #8704 — main only).
 *
 * Thin, injectable orchestrator: main collects the live health inputs and the
 * recovery callbacks, this host builds the public-safe report, runs recovery
 * actions, and writes the REDACTED export bundle to disk. Kept injectable so it
 * is unit-testable without Electron and so main's wiring stays small.
 *
 * The exported bundle is written mode 0600 and passed through
 * `redactDiagnosticsReport` first — even a regressed builder cannot leak a
 * secret into the artifact. The returned notice is public-safe and NEVER
 * contains the saved path.
 */
import { chmodSync, mkdirSync, renameSync, writeFileSync } from "node:fs"
import path from "node:path"

import { redactDiagnosticsReport, type DiagnosticsReport } from "./diagnostics-contract.ts"
import { buildDiagnosticsReport, type DiagnosticsInputs } from "./diagnostics-report.ts"
import { diagnosticsActions, type DiagnosticsAction } from "./diagnostics-contract.ts"

const OWNER_ONLY = 0o600

export type DiagnosticsActionResult = Readonly<{ ok: boolean; notice?: string }>

/** A recovery callback for one action, returning a public-safe notice. */
export type DiagnosticsRecovery = Partial<Record<DiagnosticsAction, () => Promise<DiagnosticsActionResult>>>

export type DiagnosticsHostConfig = Readonly<{
  /** Collect the live health inputs from the main-process surfaces (may be async). */
  collectInputs: () => DiagnosticsInputs | Promise<DiagnosticsInputs>
  /** Directory to write redacted export bundles into (created 0700 on demand). */
  exportDir: string
  /** Recovery/restart callbacks by action; unmapped actions are no-ops. */
  recovery?: DiagnosticsRecovery
  /** Clock (injected for tests). */
  now?: () => number
}>

export type DiagnosticsHost = Readonly<{
  gather: () => Promise<DiagnosticsReport>
  exportRedacted: () => Promise<DiagnosticsActionResult>
  runAction: (action: DiagnosticsAction) => Promise<DiagnosticsActionResult>
}>

export const makeDiagnosticsHost = (config: DiagnosticsHostConfig): DiagnosticsHost => {
  const gather = async (): Promise<DiagnosticsReport> => buildDiagnosticsReport(await config.collectInputs())

  return {
    gather,
    exportRedacted: async () => {
      // ALWAYS redact before writing — this is the privacy gate.
      const redacted = redactDiagnosticsReport(await gather())
      try {
        mkdirSync(config.exportDir, { recursive: true, mode: 0o700 })
        try {
          chmodSync(config.exportDir, 0o700)
        } catch {
          // best-effort
        }
        const stamp = new Date((config.now ?? Date.now)()).toISOString().replace(/[:.]/g, "-")
        const file = path.join(config.exportDir, `diagnostics-${stamp}.json`)
        const temporary = `${file}.tmp`
        writeFileSync(temporary, `${JSON.stringify(redacted, null, 2)}\n`, { mode: OWNER_ONLY })
        try {
          chmodSync(temporary, OWNER_ONLY)
        } catch {
          // best-effort
        }
        renameSync(temporary, file)
        // NOTE: the saved path is intentionally NOT returned (public-safe notice only).
        return { ok: true, notice: "Redacted diagnostics written to the app data folder" }
      } catch {
        return { ok: false, notice: "Could not write the diagnostics export" }
      }
    },
    runAction: async (action) => {
      if (!(diagnosticsActions as ReadonlyArray<string>).includes(action)) {
        return { ok: false, notice: "Unknown action" }
      }
      const callback = config.recovery?.[action]
      if (callback === undefined) {
        return { ok: false, notice: "No recovery action available" }
      }
      try {
        return await callback()
      } catch {
        return { ok: false, notice: "Recovery action failed" }
      }
    },
  }
}
