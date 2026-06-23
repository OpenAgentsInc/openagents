#!/usr/bin/env bun
// One-shot ENTRYPOINT for the out-of-Worker acceptance runner (EPIC #6017).
//
// Runs the SAME canonical harness the daemon runs, but against a LOCAL artifact file and
// for exactly ONE job — no lease loop. Two uses:
//   1. Local proof / manual replay: run a committed artifact through the real headless
//      suite and print the verdict (exit non-zero when not verified), like the worker
//      `acceptance-runner/cli.ts` but exercising the full `runAcceptanceJob` job path
//      (artifact resolve -> run -> build verdict payload).
//   2. End-to-end smoke against a live callback: with --callback-url + a token it POSTs
//      the produced verdict to the gateway verdict callback (the real delivery the
//      daemon does), so you can prove the receipt backfills `verified:true` without
//      standing up the lease loop.
//
// Usage:
//   bun src/run-once.ts <artifact.html> [--request-id <id>] [--served-model <m>]
//                       [--worker <w>] [--callback-url <url>] [--json]
//   ACCEPTANCE_VERDICT_CALLBACK_TOKEN=<tok> bun src/run-once.ts art.html --callback-url <url>
//
// Prereq: `bunx playwright install chromium`.

import { readFile } from 'node:fs/promises'
import process from 'node:process'

import {
  type RunnerTransport,
  AcceptanceJobMessage,
  crossyRoadAcceptanceSpec,
  makeFetchVerdictPoster,
  runAcceptanceJob,
} from './harness-bridge'

type Args = Readonly<{
  artifactPath: string
  requestId: string
  servedModel: string
  worker: string
  callbackUrl: string | undefined
  json: boolean
}>

const parseArgs = (argv: ReadonlyArray<string>): Args | undefined => {
  const positional: string[] = []
  const flags = new Map<string, string>()
  let json = false
  for (let i = 0; i < argv.length; i += 1) {
    const arg = argv[i]!
    if (arg === '--json') {
      json = true
    } else if (arg.startsWith('--')) {
      const value = argv[i + 1]
      if (value === undefined) return undefined
      flags.set(arg.slice(2), value)
      i += 1
    } else {
      positional.push(arg)
    }
  }
  const artifactPath = positional[0]
  if (artifactPath === undefined) return undefined
  return {
    artifactPath,
    callbackUrl: flags.get('callback-url'),
    json,
    requestId:
      flags.get('request-id') ?? 'acceptance.runner.run-once.local-proof.v1',
    servedModel: flags.get('served-model') ?? 'openagents/khala-code',
    worker: flags.get('worker') ?? 'khala-code-crossy-road-verifier',
  }
}

const main = async (): Promise<void> => {
  const args = parseArgs(process.argv.slice(2))
  if (args === undefined) {
    console.error(
      'Usage: bun src/run-once.ts <artifact.html> [--request-id id] ' +
        '[--served-model m] [--worker w] [--callback-url url] [--json]',
    )
    process.exit(2)
    return
  }

  const html = await readFile(args.artifactPath, 'utf8')
  const spec = crossyRoadAcceptanceSpec()

  // Build the SAME job message the gateway dispatch would enqueue. The artifact ref is a
  // local marker here; the transport's resolver returns the file bytes directly.
  const message = AcceptanceJobMessage.make({
    artifactRef: `file://${args.artifactPath}`,
    meteringReceiptRef: null,
    requestId: args.requestId,
    schemaVersion: 'openagents.inference.acceptance_job.v1',
    servedModel: args.servedModel,
    spec: {
      checks: spec.checks,
      kind: spec.kind,
      params: spec.params,
      rubricRef: spec.rubricRef,
    },
    worker: args.worker,
  })

  // Local transport: resolve the artifact from the already-read file, and either POST the
  // verdict to a live callback (the real delivery) or record it locally (proof print).
  let recorded = false
  const token = process.env.ACCEPTANCE_VERDICT_CALLBACK_TOKEN
  const postVerdict: RunnerTransport['postVerdict'] =
    args.callbackUrl !== undefined && token !== undefined && token.trim() !== ''
      ? makeFetchVerdictPoster({
          bearerToken: token,
          callbackUrl: args.callbackUrl,
        })
      : async () => {
          recorded = true
        }

  const transport: RunnerTransport = {
    postVerdict,
    resolveArtifact: async () => html,
  }

  const result = await runAcceptanceJob(transport, message)
  const { verdict } = result

  if (args.json) {
    console.log(JSON.stringify({ delivered: result.delivered, verdict }, null, 2))
  } else {
    console.log('=== EXECUTED ACCEPTANCE VERDICT (run-once) ===')
    console.log('requestId:', args.requestId)
    console.log('verified:', verdict.verified)
    console.log('scalarReward:', verdict.scalarReward)
    console.log(
      'passed:',
      `${verdict.passedChecks.length}/${verdict.checks.length}`,
    )
    console.log('failedChecks:', verdict.failedChecks.join(', ') || '(none)')
    for (const check of verdict.checks) {
      console.log(`  [${check.passed ? 'PASS' : 'FAIL'}] ${check.id}: ${check.detail}`)
    }
    if (args.callbackUrl !== undefined) {
      console.log('callbackUrl:', args.callbackUrl)
      console.log('delivered:', result.delivered)
    } else if (recorded) {
      console.log('(no --callback-url: verdict not delivered; printed only)')
    }
  }

  process.exit(verdict.verified ? 0 : 1)
}

if (import.meta.main) {
  await main()
}
