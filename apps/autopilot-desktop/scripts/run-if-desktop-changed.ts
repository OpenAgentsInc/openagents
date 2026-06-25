// Desktop release lane gate: run the desktop verify only when desktop files
// actually changed vs origin/main (plus uncommitted changes). This keeps the
// heavy Electrobun build + headless Verse smoke out of unrelated paths while
// still gating real desktop changes on it.
//
// Usage:
//   bun scripts/run-if-desktop-changed.ts -- <command> [args...]
// Force-run regardless of diff:
//   OA_FORCE_DESKTOP_VERIFY=1 bun scripts/run-if-desktop-changed.ts -- <command>

import { execSync } from "node:child_process"

const argv = process.argv.slice(2)
const separatorIndex = argv.indexOf("--")
if (separatorIndex === -1 || separatorIndex === argv.length - 1) {
  console.error(
    "run-if-desktop-changed: usage: bun scripts/run-if-desktop-changed.ts -- <command> [args...]",
  )
  process.exit(2)
}
const command = argv.slice(separatorIndex + 1)

const git = (cmd: string): string =>
  execSync(`git ${cmd}`, { encoding: "utf8" }).trim()

const desktopChanged = (): boolean => {
  if (process.env.OA_FORCE_DESKTOP_VERIFY === "1") return true
  try {
    git("fetch origin main --quiet")
  } catch {
    // Offline / no remote: be conservative and run the verify.
    return true
  }
  const ranges = ["origin/main...HEAD", "HEAD"]
  const changed = ranges
    .flatMap(range => {
      try {
        return git(`diff --name-only ${range}`).split("\n")
      } catch {
        return [] as Array<string>
      }
    })
    .filter(Boolean)
  return changed.some(path =>
    path.startsWith("apps/autopilot-desktop/") ||
    path.startsWith("packages/three-effect") ||
    path.startsWith("packages/world-client") ||
    path.startsWith("packages/world-contract") ||
    path.startsWith("packages/autopilot-ui") ||
    path.startsWith("packages/input-bindings"),
  )
}

if (!desktopChanged()) {
  console.error(
    "[desktop-verify] SKIPPED: no apps/autopilot-desktop/** (or desktop-feeding package) changes vs origin/main. Set OA_FORCE_DESKTOP_VERIFY=1 to run anyway.",
  )
  process.exit(0)
}

console.error("[desktop-verify] desktop changes detected — running verify lane.")
const child = Bun.spawn({
  cmd: command,
  stdin: "inherit",
  stdout: "inherit",
  stderr: "inherit",
})
process.exit((await child.exited) ?? 1)
