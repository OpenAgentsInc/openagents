import { mkdirSync, mkdtempSync, readFileSync, writeFileSync } from "node:fs"
import { tmpdir } from "node:os"
import path from "node:path"

import { MvpProofArg, mvpProofRequiredSteps, resolveMvpProofCommand, type MvpProofJournalEntry } from "../src/mvp-proof.ts"

const root = mkdtempSync(path.join(tmpdir(), "openagents-desktop-mvp-proof-"))
const workspace = path.join(root, "workspace")
const userData = path.join(root, "user-data")
const receipts = process.env.OPENAGENTS_DESKTOP_MVP_PROOF_DIR?.trim() || path.join(root, "receipts")
const specPath = "specs/mvp-proof.product-spec.md"
mkdirSync(path.join(workspace, "specs"), { recursive: true })
mkdirSync(path.join(workspace, "mvp-proof"), { recursive: true })

writeFileSync(path.join(workspace, specPath), `---
spec_format_version: "0.1"
title: "OpenAgents Desktop MVP Acceptance Fixture"
artifact_type: "prd"
spec_revision: 1
author: "OpenAgents"
created_at: "2026-07-13T00:00:00Z"
updated_at: "2026-07-13T00:00:00Z"
---

## Problem

The signed workroom needs a bounded real-Codex acceptance journey.

## Hypothesis

If exact root and child packets produce independently checked artifacts, the workroom can prove its execution spine.

## Scope

\`\`\`productspec-scope
in:
  - two bounded local proof artifacts
out:
  - production repository mutation
cut:
  - fabricated owner acceptance
\`\`\`

## Acceptance Criteria

- **FX-AC-01:** Create \`mvp-proof/root-output.txt\` containing exactly \`root packet complete\` followed by one newline, run a shell assertion over the exact bytes, and report the artifact evidence without changing this ProductSpec.
- **FX-AC-02:** Use the native Codex child-agent tool to delegate creation of \`mvp-proof/child-output.txt\` containing exactly \`child packet complete\` followed by one newline; inspect the child result, run a shell assertion over the exact bytes, and report the artifact evidence without changing this ProductSpec.

## Success Metrics

\`\`\`productspec-success-metrics
- id: mvp_fixture_integrity
  metric: independently_verified_root_and_child_artifacts
  target: "100%"
  window: every release acceptance run
  segment: isolated signed Desktop proof workspaces
  source: public_safe_mvp_proof_journal
\`\`\`
`)

for (const args of [
  ["init", "-b", "main"],
  ["config", "user.name", "OpenAgents MVP Proof"],
  ["config", "user.email", "mvp-proof@openagents.local"],
  ["add", "."],
  ["commit", "-m", "fixture: initialize MVP proof workspace"],
]) {
  const result = Bun.spawnSync(["git", ...args], { cwd: workspace, stdout: "pipe", stderr: "pipe" })
  if (result.exitCode !== 0) throw new Error(`MVP proof workspace setup failed at git ${args[0]}`)
}

const installedExecutable = process.env.OPENAGENTS_DESKTOP_MVP_PROOF_APP?.trim()
const packageRoot = path.resolve(import.meta.dir, "..")
const launch = async (phase: "initial" | "restart"): Promise<number> => {
  const command = [
    ...resolveMvpProofCommand(installedExecutable, packageRoot),
    MvpProofArg,
    `--openagents-mvp-proof-user-data=${userData}`,
    `--openagents-mvp-proof-workspace=${workspace}`,
    `--openagents-mvp-proof-receipts=${receipts}`,
    `--openagents-mvp-proof-spec=${specPath}`,
    `--openagents-mvp-proof-phase=${phase}`,
  ]
  const child = Bun.spawn(command, {
    cwd: packageRoot,
    env: {
      ...process.env,
      OPENAGENTS_DESKTOP_MVP_PROOF: "1",
      OPENAGENTS_DESKTOP_MVP_PROOF_DIR: receipts,
      OPENAGENTS_DESKTOP_MVP_PROOF_SPEC_PATH: specPath,
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
