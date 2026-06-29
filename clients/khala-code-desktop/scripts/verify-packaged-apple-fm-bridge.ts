import { existsSync, statSync } from "node:fs"

import {
  verifyPackagedAppleFmBridge,
  type AppleFmBridgeProbe,
} from "../src/shared/apple-fm-packaging.js"

const fsProbe: AppleFmBridgeProbe = (helperPath) => {
  if (!existsSync(helperPath)) {
    return { exists: false, nonEmpty: false, executable: false }
  }
  const stat = statSync(helperPath)
  return {
    exists: true,
    nonEmpty: stat.size > 0,
    executable: stat.isFile() && (stat.mode & 0o100) !== 0,
  }
}

const result = verifyPackagedAppleFmBridge({ probe: fsProbe })

if (result.ok) {
  console.log(
    `Packaged Apple FM bridge helper verified (${result.verifiedEnv}): ${result.verifiedPath}`,
  )
  process.exit(0)
}

console.error(
  [
    "Packaged Apple FM bridge helper is missing or unusable.",
    "Khala Desktop Apple builds must bundle foundation-bridge before signing.",
    "Checked candidates:",
    ...result.failures.map((failure) => `- ${failure.bundleDir}: ${failure.reason}`),
  ].join("\n"),
)
process.exit(1)
