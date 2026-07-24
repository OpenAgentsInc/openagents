/**
 * Omega / Desktop data-root injection for omega-effectd.
 *
 * Full Auto durable files live under `{dataRoot}/full-auto/`.
 * Never use a Zed root or a hard-coded Electron userData path inside the
 * engine. Callers inject the channel data root.
 */

import path from "node:path"

export type OmegaEffectdPaths = Readonly<{
  /** Absolute Omega or Desktop application data root. */
  dataRoot: string
}>

export const FULL_AUTO_DIR_NAME = "full-auto" as const
export const FULL_AUTO_RUNS_FILE = "runs.json" as const
export const FULL_AUTO_RUN_REPORTS_FILE = "run-reports.json" as const
export const FULL_AUTO_CONTROL_FILE = "control.json" as const
export const FULL_AUTO_REGISTRY_FILE = "registry.json" as const

export const resolveFullAutoDir = (paths: OmegaEffectdPaths): string =>
  path.join(paths.dataRoot, FULL_AUTO_DIR_NAME)

export const resolveFullAutoRunsPath = (paths: OmegaEffectdPaths): string =>
  path.join(resolveFullAutoDir(paths), FULL_AUTO_RUNS_FILE)

export const resolveFullAutoRunReportsPath = (paths: OmegaEffectdPaths): string =>
  path.join(resolveFullAutoDir(paths), FULL_AUTO_RUN_REPORTS_FILE)

export const resolveFullAutoControlPath = (paths: OmegaEffectdPaths): string =>
  path.join(resolveFullAutoDir(paths), FULL_AUTO_CONTROL_FILE)

export const resolveFullAutoRegistryPath = (paths: OmegaEffectdPaths): string =>
  path.join(resolveFullAutoDir(paths), FULL_AUTO_REGISTRY_FILE)
