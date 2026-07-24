/**
 * Minimal supervised service lifecycle for omega-effectd.
 *
 * FA-01 extracts the engine and proves start/stop outside Electron.
 * FA-02 wires Rust supervision and the framed protocol.
 */

import { mkdirSync } from "node:fs"

import {
  resolveFullAutoDir,
  type OmegaEffectdPaths,
} from "./paths.ts"
import { FULL_AUTO_RUN_ACTIVE_LIMIT } from "./engine/full-auto-run-registry.ts"
import { FULL_AUTO_NON_OVERRIDABLE_GUARDRAILS } from "./engine/full-auto-reconcile.ts"

export type OmegaEffectdServiceOptions = Readonly<{
  paths: OmegaEffectdPaths
  env?: Readonly<Record<string, string | undefined>>
}>

export type OmegaEffectdService = Readonly<{
  paths: OmegaEffectdPaths
  activeRunLimit: typeof FULL_AUTO_RUN_ACTIVE_LIMIT
  nonOverridableGuardrails: typeof FULL_AUTO_NON_OVERRIDABLE_GUARDRAILS
  start: () => Promise<void>
  stop: () => Promise<void>
  health: () => Readonly<{ ok: true; status: "running" | "stopped"; dataRoot: string }>
}>

export const createOmegaEffectdService = (
  options: OmegaEffectdServiceOptions,
): OmegaEffectdService => {
  let running = false
  const fullAutoDir = resolveFullAutoDir(options.paths)

  return {
    paths: options.paths,
    activeRunLimit: FULL_AUTO_RUN_ACTIVE_LIMIT,
    nonOverridableGuardrails: FULL_AUTO_NON_OVERRIDABLE_GUARDRAILS,
    start: async () => {
      mkdirSync(fullAutoDir, { recursive: true, mode: 0o700 })
      running = true
    },
    stop: async () => {
      running = false
    },
    health: () => ({
      ok: true as const,
      status: running ? ("running" as const) : ("stopped" as const),
      dataRoot: options.paths.dataRoot,
    }),
  }
}
