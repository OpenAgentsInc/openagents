#!/usr/bin/env bun

import {
  runFleetRunSmokeFromEnv,
  writeFleetRunSmokeResult,
  type FleetRunSmokeEnv,
  type FleetRunSmokePlan,
} from "../src/fleet-run-live-smoke"

const result = await runFleetRunSmokeFromEnv("sustained", {
  createManager: async (env: FleetRunSmokeEnv, _plan: FleetRunSmokePlan) => {
    const module = await import("../../../clients/khala-code-desktop/src/bun/khala-fleet-tools.ts")
    return new module.DefaultKhalaFleetRunSupervisorManager({ env })
  },
})

writeFleetRunSmokeResult(result)
process.exit(result.ok ? 0 : 2)
