#!/usr/bin/env bun
import { mkdir, readFile, writeFile } from "node:fs/promises"
import { dirname } from "node:path"

import { runMutaliskKhalaDelegationNoUiBridge } from "../../../apps/openagents.com/workers/api/src/inference/gym/mutalisk-khala-delegation-bridge"

export async function runPart2GepaManifestBridgeCli(
  argv: ReadonlyArray<string> = Bun.argv.slice(2),
): Promise<number> {
  const summaryPath = argValue(argv, "--summary")
  const outPath = argValue(argv, "--out") ?? "out/khala-gepa-bridge-proof.json"
  if (summaryPath === undefined) {
    usage()
    return 2
  }

  const rawSummary = JSON.parse(await readFile(summaryPath, "utf8"))
  const proof = runMutaliskKhalaDelegationNoUiBridge(rawSummary, {
    observedAt: new Date().toISOString(),
  })
  await mkdir(dirname(outPath), { recursive: true })
  await writeFile(outPath, `${JSON.stringify(proof, null, 2)}\n`, "utf8")

  console.log("OpenAgents Mutalisk Gym bridge: PASS")
  console.log(`runRef=${proof.job.runRef}`)
  console.log(`jobRef=${proof.job.jobRef}`)
  console.log(`stage=${proof.progress.at(-1)?.stage}`)
  console.log(`candidateManifestRef=${proof.candidateManifestRef}`)
  console.log(`candidateRef=${proof.candidateRef}`)
  console.log(`metricValueBps=${proof.metricValueBps}`)
  console.log(`admissionDecision=${proof.admissionDecision}`)
  console.log(`decisionGrade=${proof.decisionGrade}`)
  if (proof.actionSubmissionProposalRef !== null) {
    console.log(`actionSubmissionProposalRef=${proof.actionSubmissionProposalRef}`)
  }
  if (proof.blockerRefs.length > 0) {
    console.log(`blockerRefs=${proof.blockerRefs.join(",")}`)
  }
  console.log(`proof=${outPath}`)
  return 0
}

if (import.meta.main) {
  process.exit(await runPart2GepaManifestBridgeCli())
}

function argValue(args: ReadonlyArray<string>, name: string): string | undefined {
  const index = args.indexOf(name)
  if (index === -1) {
    return undefined
  }
  return args[index + 1]
}

function usage(): void {
  console.error(
    [
      "Usage:",
      "  bun clients/khala-code-desktop/scripts/part2-gepa-manifest-bridge.ts --summary <mutalisk-summary.json> [--out <proof.json>]",
      "",
      "This no-UI bridge reads the public-safe Mutalisk khala.fleet.delegation manifest summary,",
      "projects it into the Gym-shaped OpenAgents admission seam, and writes a public-safe proof JSON.",
    ].join("\n"),
  )
}
