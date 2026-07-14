import { Runtime } from "@openagentsinc/runtime-platform"
import { mkdtempSync, readFileSync } from "node:fs"
import { tmpdir } from "node:os"
import path from "node:path"

import type { LiveProofJournalEntry } from "../src/live-proof.ts"
import { liveProofJournalVerdict } from "../src/live-proof-verdict.ts"

const outDir = process.env.OPENAGENTS_DESKTOP_LIVE_PROOF_DIR?.trim() ||
  mkdtempSync(path.join(tmpdir(), "openagents-desktop-live-proof-"))

const child = Runtime.spawn(["electron", "."], {
  cwd: path.resolve(import.meta.dirname, ".."),
  env: {
    ...process.env,
    OPENAGENTS_DESKTOP_LIVE_PROOF: "1",
    OPENAGENTS_DESKTOP_LIVE_PROOF_DIR: outDir,
  },
  stderr: "inherit",
  stdout: "inherit",
})
const childCode = await child.exited

let journal: ReadonlyArray<LiveProofJournalEntry> = []
try {
  const decoded = JSON.parse(readFileSync(path.join(outDir, "journal.json"), "utf8"))
  if (Array.isArray(decoded)) journal = decoded as ReadonlyArray<LiveProofJournalEntry>
} catch {
  // Missing or malformed journal is represented by every required step missing.
}

const verdict = liveProofJournalVerdict(journal)
if (childCode !== 0 || !verdict.ok) {
  console.error(
    `[openagents-desktop live-proof] acceptance FAILED child=${childCode} failed=${verdict.failed.join(",") || "none"} missing=${verdict.missing.join(",") || "none"} receipts=${outDir}`,
  )
  process.exit(1)
}

console.log(`[openagents-desktop live-proof] acceptance PASSED receipts=${outDir}`)
