#!/usr/bin/env node

import { readFileSync } from "node:fs"
import { join } from "node:path"

const root = new URL("..", import.meta.url).pathname
const files = [
  "scripts/codex-supervisor/codex-supervisor.sh",
  "scripts/claude-supervisor/claude-supervisor.sh",
  "scripts/codex-supervisor/lockout.sh",
  "scripts/claude-supervisor/launch.sh",
  "scripts/codex-supervisor/launch.sh",
].map((path) => join(root, path))

const forbidden = [
  {
    pattern: /\bDESIRED_FILE\b/,
    reason: "desired fleet slots must be read from the FleetRun row, not a shell file",
  },
  {
    pattern: /\bPAUSE_FILE\b/,
    reason: "pause/resume state must be the FleetRun state, not a shell file",
  },
  {
    pattern: /\/claims\b|claim\.\*/,
    reason: "live work claims must use pylon_orchestration_work_claims, not filesystem claim directories",
  },
  {
    pattern: /mkdir\s+["']?\$?\{?[^"'\n}]*claim/i,
    reason: "claim acquisition must be the orchestration store's atomic insert",
  },
  {
    pattern: /rm\s+-rf\s+["']?\$?\(?[^"'\n)]*claim/i,
    reason: "claim release must update the orchestration store",
  },
]

let failed = false

for (const file of files) {
  const source = readFileSync(file, "utf8")
  for (const { pattern, reason } of forbidden) {
    if (pattern.test(source)) {
      console.error(`supervisor-store-bypass: ${file} matches ${pattern}: ${reason}`)
      failed = true
    }
  }
}

for (const file of files.slice(0, 3)) {
  const source = readFileSync(file, "utf8")
  if (!source.includes("supervisor-state.ts") && !source.includes("SUP_ORCHESTRATION_STATE_BIN")) {
    console.error(`supervisor-store-bypass: ${file} does not route run/claim state through supervisor-state.ts`)
    failed = true
  }
}

// FC-2 node activation is durable orchestration authority too. It must keep
// arming state in the canonical SQLite store rather than regressing to a JSON,
// shell, or ad-hoc filesystem switch beside the supervisor state.
const activationFile = join(root, "src/node/fleet-run-activation.ts")
const activationSource = readFileSync(activationFile, "utf8")
if (
  !activationSource.includes("openPylonFleetRunRuntime") ||
  !activationSource.includes("setFleetRunActivation") ||
  !activationSource.includes("listFleetRunActivations")
) {
  console.error(
    "supervisor-store-bypass: FleetRun activation does not route armed state through the canonical orchestration store",
  )
  failed = true
}
if (/writeFile|appendFile|fleet-run[^\n]*\.json/i.test(activationSource)) {
  console.error(
    "supervisor-store-bypass: FleetRun activation must not persist an armed-state filesystem sidecar",
  )
  failed = true
}

if (failed) process.exit(1)
console.log("supervisor-store-bypass: OK")
