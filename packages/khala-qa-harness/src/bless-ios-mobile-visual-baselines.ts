/**
 * Bless (or verify) captured iOS-simulator Maestro screenshots into the owned
 * `openagents.khala_visual_baselines.v1` engine (QAM-4, #8539).
 *
 * Reads every `<id>.png` in the candidate dir (each filename is the public-safe
 * baseline id, e.g. `khala.mobile.screen.settings.iphone-17-pro.dark`), runs it
 * through `runKhalaMobileVisualTier` — blessing (copy PNG into the manifest,
 * record a `blessed` result) by default, or comparing against the committed
 * baseline with `--verify` (fails on any `changed`/`missing`, the nightly
 * regression check) — then writes a
 * `openagents.khala_mobile.visual_tier_report.v1` report.
 *
 * The id's trailing two dot-segments are the viewport and color scheme
 * (`...iphone-17-pro.dark`). This lives beside the engine on purpose: blessing
 * is a shared QA-harness concern and the Expo app must not depend on the
 * harness. Drive it from `clients/khala-mobile/scripts/mobile-visual-tier-run.sh`,
 * only from real captured simulator truth, never from fixtures.
 *
 * Usage:
 *   bun packages/khala-qa-harness/src/bless-ios-mobile-visual-baselines.ts \
 *       <candidateDir> <reportPath> [--verify]
 */
import { readdir, mkdir, writeFile } from "node:fs/promises"
import { basename, join, resolve } from "node:path"

import {
  runKhalaMobileVisualTier,
  type KhalaMobileVisualTierCapture,
} from "./mobile-visual-tier.js"

const repoRoot = resolve(import.meta.dir, "../../..")
const baselineDir = join(repoRoot, "docs/khala-code/receipts/qam-4-baselines")

const [, , rawCandidateDir, rawReportPath, ...rest] = process.argv
if (rawCandidateDir === undefined || rawReportPath === undefined) {
  console.error(
    "Usage: bun src/bless-ios-mobile-visual-baselines.ts <candidateDir> <reportPath> [--verify]",
  )
  process.exit(2)
}
const verify = rest.includes("--verify")
const candidateDir = resolve(rawCandidateDir)
const reportPath = resolve(rawReportPath)

const files = (await readdir(candidateDir)).filter(name => name.endsWith(".png")).sort()
if (files.length === 0) {
  console.error(`No .png captures found in ${candidateDir}`)
  process.exit(1)
}

const captures: KhalaMobileVisualTierCapture[] = []
for (const file of files) {
  const id = basename(file, ".png")
  const segments = id.split(".")
  const colorScheme = segments[segments.length - 1] === "light" ? "light" : "dark"
  const device = segments[segments.length - 2] ?? "iphone-17-pro"
  const png = Buffer.from(await Bun.file(join(candidateDir, file)).arrayBuffer())
  captures.push({ colorScheme, device, id, png, source: "maestro-checkpoint" })
}

const report = await runKhalaMobileVisualTier({
  baselineDir,
  candidateDir: join(candidateDir, ".tier-candidates"),
  captures,
  ...(verify ? { requireBaseline: true } : { bless: true }),
  simulatorTruth: "captured",
})

await mkdir(resolve(reportPath, ".."), { recursive: true })
await writeFile(reportPath, `${JSON.stringify(report, null, 2)}\n`)

for (const result of report.results) {
  console.log(`  ${result.status.padEnd(9)} ${result.id}`)
}
console.log(`\n[bless-ios-mobile-visual-baselines] ${verify ? "verify" : "bless"} report: ${reportPath}`)
console.log(`[bless-ios-mobile-visual-baselines] ok=${report.ok}`)
if (!report.ok) process.exit(1)
