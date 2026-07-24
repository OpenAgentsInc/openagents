#!/usr/bin/env node
/**
 * omega-effectd CLI entry.
 *
 * Usage:
 *   OPENAGENTS_OMEGA_EFFECTD_DATA_ROOT=/path omega-effectd
 *
 * FA-02 replaces this process with Rust-supervised lifecycle. FA-01 proves
 * the Node entry can start and stop with an injected data root.
 */

import { createOmegaEffectdService } from "../service.ts"
import { FULL_AUTO_RUN_ACTIVE_LIMIT } from "../engine/full-auto-run-registry.ts"

const dataRoot = process.env.OPENAGENTS_OMEGA_EFFECTD_DATA_ROOT?.trim()
if (!dataRoot) {
  console.error("OPENAGENTS_OMEGA_EFFECTD_DATA_ROOT is required")
  process.exit(2)
}

const service = createOmegaEffectdService({ paths: { dataRoot }, env: process.env })

const shutdown = async (code: number): Promise<void> => {
  await service.stop()
  process.exit(code)
}

process.on("SIGINT", () => {
  void shutdown(0)
})
process.on("SIGTERM", () => {
  void shutdown(0)
})

await service.start()
console.log(
  JSON.stringify({
    service: "omega-effectd",
    version: "0.1.0",
    status: "running",
    dataRoot,
    activeRunLimit: FULL_AUTO_RUN_ACTIVE_LIMIT,
  }),
)

// Keep the process alive until signal. FA-02 attaches the framed protocol.
await new Promise<void>(() => {})
