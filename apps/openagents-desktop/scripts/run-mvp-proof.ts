import { Runtime } from "@openagentsinc/runtime-platform"
import { mkdirSync, mkdtempSync, readFileSync, writeFileSync } from "node:fs"
import { tmpdir } from "node:os"
import path from "node:path"

import { MvpProofArg, mvpProofRequiredSteps, resolveMvpProofCommand, type MvpProofJournalEntry } from "../src/mvp-proof.ts"

const root = mkdtempSync(path.join(tmpdir(), "openagents-desktop-mvp-proof-"))
const workspace = path.join(root, "workspace")
const userData = path.join(root, "user-data")
const receipts = process.env.OPENAGENTS_DESKTOP_MVP_PROOF_DIR?.trim() || path.join(root, "receipts")
mkdirSync(path.join(workspace, "mvp-proof"), { recursive: true })
writeFileSync(path.join(workspace, "README.md"), "# OpenAgents Desktop MVP proof workspace\n")

for (const args of [
  ["init", "-b", "main"],
  ["config", "user.name", "OpenAgents MVP Proof"],
  ["config", "user.email", "mvp-proof@openagents.local"],
  ["add", "."],
  ["commit", "-m", "fixture: initialize MVP proof workspace"],
]) {
  const result = Runtime.spawnSync(["git", ...args], { cwd: workspace, stdout: "pipe", stderr: "pipe" })
  if (result.exitCode !== 0) throw new Error(`MVP proof workspace setup failed at git ${args[0]}`)
}

const installedExecutable = process.env.OPENAGENTS_DESKTOP_MVP_PROOF_APP?.trim()
const packageRoot = path.resolve(import.meta.dirname, "..")
const launch = async (phase: "initial" | "restart"): Promise<number> => {
  const command = [
    ...resolveMvpProofCommand(installedExecutable, packageRoot),
    MvpProofArg,
    `--openagents-mvp-proof-user-data=${userData}`,
    `--openagents-mvp-proof-workspace=${workspace}`,
    `--openagents-mvp-proof-receipts=${receipts}`,
    `--openagents-mvp-proof-phase=${phase}`,
  ]
  const child = Runtime.spawn(command, {
    cwd: packageRoot,
    env: {
      ...process.env,
      OPENAGENTS_DESKTOP_MVP_PROOF: "1",
      OPENAGENTS_DESKTOP_MVP_PROOF_DIR: receipts,
      OPENAGENTS_DESKTOP_MVP_PROOF_PHASE: phase,
      OPENAGENTS_DESKTOP_ISOLATED_APP_PROOF: "1",
      OPENAGENTS_DESKTOP_ISOLATED_WORKSPACE_ROOT: workspace,
      OPENAGENTS_DESKTOP_USER_DATA: userData,
    },
    stderr: "inherit",
    stdout: "inherit",
  })
  return child.exited
}
let childCode = await launch("initial")
if (childCode === 75) childCode = await launch("restart")

let journal: ReadonlyArray<MvpProofJournalEntry> = []
try {
  const decoded = JSON.parse(readFileSync(path.join(receipts, "journal.json"), "utf8"))
  if (Array.isArray(decoded)) journal = decoded as ReadonlyArray<MvpProofJournalEntry>
} catch { /* the missing journal fails every required step below */ }

const failed = mvpProofRequiredSteps.filter(step => !journal.some(entry => entry.step === step && entry.ok))
if (childCode !== 0 || failed.length > 0) {
  console.error(`[openagents-desktop mvp-proof] acceptance FAILED child=${childCode} failed=${failed.join(",") || "summary"} receipts=${receipts}`)
  process.exit(1)
}
console.log(`[openagents-desktop mvp-proof] acceptance PASSED receipts=${receipts}`)
