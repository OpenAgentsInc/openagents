#!/usr/bin/env node
/**
 * omega-effectd CLI entry (framed stdio protocol for Omega Rust supervisor).
 *
 * Usage:
 *   OPENAGENTS_OMEGA_EFFECTD_DATA_ROOT=/path omega-effectd
 *
 * Speaks `openagents.omega.effectd.v1` newline-framed JSON on stdin/stdout.
 * Status for humans goes to stderr only.
 */

import { createOmegaEffectdService } from "../service.ts"
import { createOmegaEffectdFramedServer } from "../protocol/server.ts"
import { FULL_AUTO_RUN_ACTIVE_LIMIT } from "../engine/full-auto-run-registry.ts"
import { OMEGA_EFFECTD_SERVICE_VERSION } from "../protocol/framed.ts"

const dataRoot = process.env.OPENAGENTS_OMEGA_EFFECTD_DATA_ROOT?.trim()
if (!dataRoot) {
  console.error("OPENAGENTS_OMEGA_EFFECTD_DATA_ROOT is required")
  process.exit(2)
}

const service = createOmegaEffectdService({ paths: { dataRoot }, env: process.env })
const framed = createOmegaEffectdFramedServer(service, { dataRoot })

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

console.error(
  JSON.stringify({
    service: "omega-effectd",
    version: OMEGA_EFFECTD_SERVICE_VERSION,
    status: "listening",
    protocol: "openagents.omega.effectd.v1",
    dataRoot,
    activeRunLimit: FULL_AUTO_RUN_ACTIVE_LIMIT,
  }),
)

await framed.serveStdio()
await service.stop()
