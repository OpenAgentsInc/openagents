import { existsSync, openSync, readSync, closeSync, statSync } from "node:fs"

import {
  verifyPackagedAppleFmBridge,
  type AppleFmBridgeProbe,
} from "../src/shared/apple-fm-packaging.js"

const fsProbe: AppleFmBridgeProbe = (helperPath) => {
  if (!existsSync(helperPath)) {
    return { executable: false, exists: false, nativeExecutable: false, nonEmpty: false }
  }
  const stat = statSync(helperPath)
  const nativeExecutable = stat.isFile() && isMachOExecutable(helperPath)
  return {
    executable: stat.isFile() && (stat.mode & 0o100) !== 0,
    exists: true,
    nativeExecutable,
    nonEmpty: stat.size > 0,
  }
}

const isMachOExecutable = (path: string): boolean => {
  const fd = openSync(path, "r")
  try {
    const buffer = Buffer.alloc(4)
    const bytesRead = readSync(fd, buffer, 0, 4, 0)
    if (bytesRead !== 4) return false
    const magic = buffer.toString("hex")
    return [
      "feedface",
      "feedfacf",
      "cefaedfe",
      "cffaedfe",
      "cafebabe",
      "bebafeca",
    ].includes(magic)
  } finally {
    closeSync(fd)
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
    "The Khala desktop Apple build must bundle the native foundation-bridge helper before notarization.",
    "Checked candidates:",
    ...result.failures.map((failure) => `- ${failure.bundleDir}: ${failure.reason}`),
  ].join("\n"),
)
process.exit(1)
