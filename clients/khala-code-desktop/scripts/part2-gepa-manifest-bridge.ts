#!/usr/bin/env bun
import { mkdir, readFile, writeFile } from "node:fs/promises"
import { dirname } from "node:path"

import { runMutaliskKhalaDelegationNoUiBridge } from "../../../apps/openagents.com/workers/api/src/inference/gym/mutalisk-khala-delegation-bridge"

export async function runPart2GepaManifestBridgeCli(
  argv: ReadonlyArray<string> = Bun.argv.slice(2),
): Promise<number> {
  const summaryPath = argValue(argv, "--summary")
  const outPath = argValue(argv, "--out") ?? "out/khala-gepa-bridge-proof.json"
  const apiBase = argValue(argv, "--api-base")
  const operatorToken =
    argValue(argv, "--operator-token") ??
    process.env[argValue(argv, "--operator-token-env") ?? "OPENAGENTS_OPERATOR_BEARER_TOKEN"]
  if (summaryPath === undefined) {
    usage()
    return 2
  }

  const rawSummary = JSON.parse(await readFile(summaryPath, "utf8"))
  if (apiBase !== undefined) {
    if (operatorToken === undefined || operatorToken.trim() === "") {
      console.error(
        "Missing operator bearer token. Set OPENAGENTS_OPERATOR_BEARER_TOKEN or pass --operator-token-env <ENV>.",
      )
      return 2
    }
    const proof = await runWorkerBridge(apiBase, operatorToken, rawSummary)
    await mkdir(dirname(outPath), { recursive: true })
    await writeFile(outPath, `${JSON.stringify(proof, null, 2)}\n`, "utf8")
    const workerRun =
      proof.run !== null && typeof proof.run === "object"
        ? (proof.run as Record<string, unknown>)
        : undefined

    console.log("OpenAgents Mutalisk Gym Worker bridge: PASS")
    console.log(`runRef=${proof.runRef}`)
    console.log(`stage=${workerRun?.latestStage}`)
    console.log(`candidateManifestRef=${proof.candidateManifestRef}`)
    console.log(`candidateRef=${proof.candidateRef}`)
    console.log(`metricValueBps=${proof.metricValueBps}`)
    console.log(`admissionDecision=${proof.admissionDecision}`)
    console.log(`decisionGrade=${proof.decisionGrade}`)
    if (typeof proof.actionSubmissionProposalRef === "string") {
      console.log(`actionSubmissionProposalRef=${proof.actionSubmissionProposalRef}`)
    }
    console.log(`proof=${outPath}`)
    return 0
  }

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
      "  bun clients/khala-code-desktop/scripts/part2-gepa-manifest-bridge.ts --summary <mutalisk-summary.json> --api-base <https://openagents.com> [--operator-token-env <ENV>] [--out <proof.json>]",
      "",
      "This no-UI bridge reads the public-safe Mutalisk khala.fleet.delegation manifest summary,",
      "projects it into the Gym-shaped OpenAgents admission seam, and writes a public-safe proof JSON.",
      "With --api-base it first creates a durable Worker run and ingests the summary into that runRef.",
    ].join("\n"),
  )
}

async function runWorkerBridge(
  apiBase: string,
  operatorToken: string,
  rawSummary: Record<string, unknown>,
): Promise<Record<string, unknown>> {
  const base = apiBase.replace(/\/+$/, "")
  const create = await postJson(
    `${base}/api/operator/gym/mutalisk-khala-delegation/runs`,
    {
      baseModuleRef:
        typeof rawSummary.baseModuleRef === "string"
          ? rawSummary.baseModuleRef
          : undefined,
      refSeed:
        typeof rawSummary.candidateRef === "string"
          ? rawSummary.candidateRef
          : undefined,
    },
    operatorToken,
  )
  const runRef = typeof create.runRef === "string" ? create.runRef : undefined
  if (runRef === undefined) {
    throw new Error("Worker bridge did not return a runRef.")
  }
  return postJson(
    `${base}/api/operator/gym/mutalisk-khala-delegation/summary`,
    {
      manifestSummary: rawSummary,
      runRef,
    },
    operatorToken,
  )
}

async function postJson(
  url: string,
  body: Record<string, unknown>,
  operatorToken: string,
): Promise<Record<string, unknown>> {
  const response = await fetch(url, {
    body: JSON.stringify(body),
    headers: {
      authorization: `Bearer ${operatorToken}`,
      "content-type": "application/json",
    },
    method: "POST",
  })
  const text = await response.text()
  const parsed = text.trim() === "" ? {} : JSON.parse(text)
  if (!response.ok) {
    throw new Error(
      `Worker bridge request failed (${response.status}): ${JSON.stringify(parsed)}`,
    )
  }
  return parsed
}
