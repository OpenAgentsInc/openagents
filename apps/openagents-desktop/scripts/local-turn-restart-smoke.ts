import { existsSync, mkdtempSync, rmSync } from "node:fs"
import { tmpdir } from "node:os"
import path from "node:path"

const appRoot = path.resolve(import.meta.dir, "..")
const userData = mkdtempSync(path.join(tmpdir(), "openagents-desktop-turn-restart-"))
const packagedBinary = path.join(appRoot, "out", "OpenAgents-darwin-arm64", "OpenAgents.app", "Contents", "MacOS", "OpenAgents")
const electronBinary = path.join(appRoot, "node_modules", ".bin", "electron")
const command = process.platform === "darwin" && existsSync(packagedBinary)
  ? [packagedBinary]
  : [electronBinary, "."]

const runPhase = (phase: "seed" | "recover"): void => {
  const result = Bun.spawnSync({
    cmd: command,
    cwd: appRoot,
    env: {
      ...process.env,
      OPENAGENTS_DESKTOP_SMOKE: "0",
      OPENAGENTS_DESKTOP_USER_DATA: userData,
      OPENAGENTS_DESKTOP_LOCAL_TURN_RESTART_PROBE: phase,
    },
    stdout: "inherit",
    stderr: "inherit",
  })
  if (result.exitCode !== 0) throw new Error(`Electron restart smoke ${phase} phase exited ${result.exitCode}`)
}

try {
  runPhase("seed")
  runPhase("recover")
  console.log("[openagents-desktop local-turn-restart] two-process smoke OK")
} finally {
  rmSync(userData, { recursive: true, force: true })
}
