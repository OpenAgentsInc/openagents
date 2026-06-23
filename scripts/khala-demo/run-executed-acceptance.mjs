#!/usr/bin/env bun
// Harness glue: run a Khala north-star HTML artifact through the REAL executed
// acceptance runner, then through verifyKhalaCodeCompletion, and print the executed
// verdict (EPIC #6017, M8 #6016).
//
// This script ONLY drives the existing runner/verifier under
// `apps/openagents.com/workers/api/src/inference/` — it does NOT edit them. A concurrent
// check:architecture lane owns that source; this harness lives in scripts/khala-demo/.
//
// What it does (the "verified must mean we ran it" principle, executed for real):
//   1. Reads a single-file HTML artifact (default: the preserved north-star run output
//      reconstructed from the verified prod SSE stream).
//   2. Derives the crossy-road acceptance spec from intent.
//   3. Runs the headless Playwright/chromium acceptance suite against the LIVE page
//      (load -> no errors -> click PLAY -> press forward N -> read exposed state).
//   4. Feeds the executed AcceptanceVerdict through verifyKhalaCodeCompletion to get the
//      EXECUTED khala-code verdict (executed:true, verification, scalarReward).
//   5. Prints both verdicts as JSON and exits non-zero when not verified.
//
// Prereq: `bunx playwright install chromium`.
//
// Usage:
//   bun scripts/khala-demo/run-executed-acceptance.mjs [artifact.html] [--json]
//   cat artifact.html | bun scripts/khala-demo/run-executed-acceptance.mjs -

import { readFile } from 'node:fs/promises'
import { fileURLToPath } from 'node:url'
import { dirname, resolve } from 'node:path'
import process from 'node:process'

const here = dirname(fileURLToPath(import.meta.url))
const repoRoot = resolve(here, '..', '..')
const apiSrc = resolve(
  repoRoot,
  'apps/openagents.com/workers/api/src/inference',
)

export const DEFAULT_ARTIFACT = resolve(
  here,
  'artifacts/khala-crossy-road-northstar-run.v1.html',
)

// Run the real executed acceptance suite + khala-code verifier against an HTML artifact
// string. Imports the runner/verifier from the api source READ-ONLY (no edits). Returns
// { acceptanceVerdict, khalaCodeVerdict }. Requires chromium (Playwright).
export const runExecutedAcceptance = async (artifactHtml, requestId) => {
  const { crossyRoadAcceptanceSpec } = await import(
    resolve(apiSrc, 'acceptance-spec.ts')
  )
  const { runAcceptanceSuite } = await import(
    resolve(apiSrc, 'acceptance-runner/runner.ts')
  )
  const { verifyKhalaCodeCompletion } = await import(
    resolve(apiSrc, 'khala-code-verifier.ts')
  )

  const spec = crossyRoadAcceptanceSpec()
  const acceptance = await runAcceptanceSuite({ artifactHtml, spec })
  const khalaCodeVerdict = verifyKhalaCodeCompletion({
    content: artifactHtml,
    requestId:
      requestId ?? 'khala.northstar.crossy_road.executed_acceptance.v1',
    servedModel: 'openagents/khala-code',
    worker: 'khala-code-crossy-road-verifier',
    acceptance,
  })
  return { acceptanceVerdict: acceptance, khalaCodeVerdict }
}

const readStdin = async () => {
  const chunks = []
  for await (const chunk of process.stdin) chunks.push(Buffer.from(chunk))
  return Buffer.concat(chunks).toString('utf8')
}

const main = async () => {
  const args = process.argv.slice(2)
  const jsonOnly = args.includes('--json')
  const pathArg = args.find(arg => arg !== '--json')
  const artifactPath = pathArg ?? DEFAULT_ARTIFACT

  const artifactHtml =
    artifactPath === '-' ? await readStdin() : await readFile(artifactPath, 'utf8')

  const { acceptanceVerdict: acceptance, khalaCodeVerdict: verdict } =
    await runExecutedAcceptance(artifactHtml)

  const out = {
    artifactPath,
    acceptanceVerdict: acceptance,
    khalaCodeVerdict: verdict,
  }

  if (jsonOnly) {
    console.log(JSON.stringify(out, null, 2))
  } else {
    console.log('=== EXECUTED ACCEPTANCE VERDICT ===')
    console.log('executed:', acceptance.executed)
    console.log('verified:', acceptance.verified)
    console.log('scalarReward:', acceptance.scalarReward)
    console.log('passedChecks:', acceptance.passedChecks.join(', ') || '(none)')
    console.log('failedChecks:', acceptance.failedChecks.join(', ') || '(none)')
    console.log('--- per-check detail ---')
    for (const check of acceptance.checks) {
      console.log(`  [${check.passed ? 'PASS' : 'FAIL'}] ${check.id}: ${check.detail}`)
    }
    console.log('consoleErrors:', acceptance.consoleErrors.length)
    console.log('pageErrors:', acceptance.pageErrors.length)
    console.log('')
    console.log('=== KHALA-CODE VERDICT (verifyKhalaCodeCompletion) ===')
    console.log('executed:', verdict.executed)
    console.log('verification:', verdict.verification)
    console.log('verified:', verdict.verified)
    console.log('scalarReward:', verdict.scalarReward)
    console.log('summary:', verdict.summary)
    console.log('artifact.bytes:', verdict.artifact.bytes)
    console.log('artifact.fingerprint:', verdict.artifact.fingerprint)
  }

  process.exit(verdict.verified ? 0 : 1)
}

// Only run as a script (not when imported by the test).
if (import.meta.main) {
  await main()
}
