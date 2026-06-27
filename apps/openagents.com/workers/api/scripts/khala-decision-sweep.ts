// Runnable owner-armed Khala-vs-Fireworks/Vertex decision sweep (Open Question #5
// suite / #6307).
//
// This is NOT part of the unit suite — it makes real network calls and is GATED on
// credentials, so `bun run test` / `check:deploy` never require it. It drives the
// already-tested sweep harness (`real-sweep-runner.ts`) against live providers and
// emits the public-safe `decisionGrade` report.
//
// THREE modes, selected by which credentials are present (no-spend by default):
//
//   1. KHALA-ONLY (no third-party spend) — runs NOW with only an
//      OPENAGENTS_AGENT_TOKEN. Produces the Khala side of the suite over the
//      realistic shapes; the report is honestly `decisionGrade:false` (no billable
//      comparator). This is the part that runs unattended.
//
//   2. OWNER-ARMED REAL (spendful) — when the owner ALSO provides FIREWORKS_API_KEY
//      and the Vertex transport credentials AND the explicit arm env
//      (OA_BENCH_OWNER_CONFIRM=1 + OA_BENCH_OWNER_APPROVAL_REF=... +
//      OA_BENCH_BUDGET_CAP_MSAT=... + OA_BENCH_MAX_BILLABLE_SAMPLES=...). Only then
//      does it run the billable comparators and earn `decisionGrade:true`.
//
//   3. SKIP — no agent token: prints the NEEDS-OWNER arm instructions and exits 0.
//
// Usage:
//   bun run apps/openagents.com/workers/api/scripts/khala-decision-sweep.ts
//
// It NEVER prints a key/token. Report output is the public-safe report JSON only.
import { readFileSync } from 'node:fs'
import { homedir } from 'node:os'
import { join } from 'node:path'

import { buildBenchmarkReport, checkReportPublicSafety } from '../src/inference/benchmark/report'
import {
  KHALA_ONLY_DECISION_SLICE,
  KHALA_VS_FIREWORKS_VERTEX_DECISION_SUITE,
} from '../src/inference/benchmark/real-sweep-config'
import type { RealLaneTransport } from '../src/inference/benchmark/real-lane-executor'
import {
  makeKhalaPublicTransport,
  makeOpenAICompatibleTransport,
} from '../src/inference/benchmark/real-lane-transports'
import {
  RealSweepNotArmedError,
  runRealSweep,
} from '../src/inference/benchmark/real-sweep-runner'
import type { RealSweepPreflightOptions } from '../src/inference/benchmark/real-sweep-plan'

const readSecretLine = (file: string, key: string): string | undefined => {
  try {
    const contents = readFileSync(file, 'utf8')
    for (const line of contents.split('\n')) {
      const match = line.match(
        new RegExp(`^\\s*(?:export\\s+)?${key}\\s*=\\s*(.+)$`),
      )
      if (match) {
        return match[1]?.trim().replace(/^["']|["']$/g, '')
      }
    }
  } catch {
    return undefined
  }
  return undefined
}

const resolveSecret = (
  envKey: string,
  secretFile: string,
  secretKey: string,
): string | undefined => {
  const fromEnv = process.env[envKey]?.trim()
  if (fromEnv) {
    return fromEnv
  }
  return readSecretLine(join(homedir(), 'work', '.secrets', secretFile), secretKey)
}

const NEEDS_OWNER = `
NEEDS-OWNER: arm the spendful Khala-vs-Fireworks/Vertex decision sweep (#6307).

The Khala side runs at no third-party cost with only an agent token. To produce
the FIRST decisionGrade:true cross-provider report, the OWNER must arm the
billable comparators and confirm the spend gate:

  1. Credentials (kept in ~/work/.secrets, never committed):
     - OPENAGENTS_AGENT_TOKEN        (Khala /api/v1 caller; no third-party cost)
     - FIREWORKS_API_KEY             (~/work/.secrets/fireworks.env)  [BILLABLE]
     - VERTEX_API_BASE_URL + VERTEX_API_KEY  (Vertex OpenAI-compatible)  [BILLABLE]
       (or a Vertex SA + region per the vertex runbook; wire as an
        OpenAI-compatible transport.)

  2. Explicit arm env (the preflight refuses to spend without ALL of these):
     - OA_BENCH_OWNER_CONFIRM=1
     - OA_BENCH_OWNER_APPROVAL_REF="<public-safe owner approval ref>"
     - OA_BENCH_BUDGET_CAP_MSAT=<positive msat cap>
     - OA_BENCH_MAX_BILLABLE_SAMPLES=<>= expanded billable cell-sample count>
       (the full suite expands to 320 executable samples total: 80 Khala
        own-capacity samples + 240 billable comparator samples across Fireworks
        and two Vertex lanes, 80 per billable lane.)

  3. Refresh the realistic traffic shapes from the live token_usage_events ledger
     so each shape's observed evidence ref + count reflect CURRENT Khala traffic
     (the shapes shipped here are seeded from the 2026-06-25 observed export).

  4. Re-run this script with all of the above set. It will:
     - preflight (owner gate + realistic-traffic evidence + budget/sample caps),
     - run Khala + Fireworks + Vertex over the realistic shapes,
     - emit the public-safe decisionGrade:true report.

Until armed, this run produced ONLY the Khala-side report (decisionGrade:false).
`.trim()

const main = async (): Promise<void> => {
  const deps = { fetch: globalThis.fetch, now: () => Date.now() }
  const agentToken = process.env['OPENAGENTS_AGENT_TOKEN']?.trim()

  if (!agentToken) {
    console.log('[khala-decision-sweep] no OPENAGENTS_AGENT_TOKEN — skipping live run.')
    console.log(NEEDS_OWNER)
    process.exit(0)
  }

  const fireworksKey = resolveSecret('FIREWORKS_API_KEY', 'fireworks.env', 'FIREWORKS_API_KEY')
  const vertexBaseUrl = process.env['VERTEX_API_BASE_URL']?.trim()
  const vertexKey = resolveSecret('VERTEX_API_KEY', 'vertex.env', 'VERTEX_API_KEY')

  const ownerConfirm = process.env['OA_BENCH_OWNER_CONFIRM'] === '1'
  const ownerApprovalRef = process.env['OA_BENCH_OWNER_APPROVAL_REF']?.trim()
  const budgetCapMsat = Number(process.env['OA_BENCH_BUDGET_CAP_MSAT'])
  const maxBillableSamples = Number(process.env['OA_BENCH_MAX_BILLABLE_SAMPLES'])

  const billableArmed =
    ownerConfirm &&
    ownerApprovalRef !== undefined &&
    ownerApprovalRef !== '' &&
    Number.isFinite(budgetCapMsat) &&
    budgetCapMsat > 0 &&
    Number.isFinite(maxBillableSamples) &&
    maxBillableSamples > 0 &&
    fireworksKey !== undefined &&
    vertexBaseUrl !== undefined &&
    vertexKey !== undefined

  const khalaTransport = makeKhalaPublicTransport(deps, { agentToken })

  if (!billableArmed) {
    // KHALA-ONLY no-spend run. Bound the live sample count by default so an
    // unattended proof run does not flood production with 80 sequential calls;
    // OA_BENCH_SAMPLE_LIMIT overrides samplesPerCell (default 1 for a smoke).
    const sampleLimit = Number(process.env['OA_BENCH_SAMPLE_LIMIT'] ?? '1')
    const slice = {
      ...KHALA_ONLY_DECISION_SLICE,
      samplesPerCell:
        Number.isFinite(sampleLimit) && sampleLimit > 0
          ? Math.floor(sampleLimit)
          : 1,
    }
    console.log(
      `[khala-decision-sweep] running Khala-only slice (no third-party spend, ${slice.samplesPerCell} sample(s)/cell).`,
    )
    const runSet = await runRealSweep({
      config: slice,
      preflight: {
        ownerConfirmed: true,
        ownerApprovalRef: 'khala-only-no-spend-slice',
        budgetCapMsat: 1,
        maxBillableSamples: 100_000,
      } satisfies RealSweepPreflightOptions,
      transports: [khalaTransport],
    })
    const report = buildBenchmarkReport(runSet)
    const safety = checkReportPublicSafety(report)
    console.log(JSON.stringify({ report, publicSafety: safety }, null, 2))
    console.log(NEEDS_OWNER)
    process.exit(safety.safe ? 0 : 1)
  }

  // OWNER-ARMED REAL spendful run.
  console.log('[khala-decision-sweep] OWNER-ARMED real sweep: Khala + Fireworks + Vertex (BILLABLE).')
  const fireworksTransport = makeOpenAICompatibleTransport({
    lane: 'fireworks',
    billable: true,
    baseUrl: 'https://api.fireworks.ai/inference/v1',
    apiKey: fireworksKey,
    wireModelRef: 'accounts/fireworks/models/deepseek-v4-flash',
    rateCard: { perKPromptMsat: 1500, perKCompletionMsat: 4500, cachedPromptBilledFraction: 0.5 },
    region: 'us-central',
    deps,
  })
  const vertexTransport: RealLaneTransport = makeOpenAICompatibleTransport({
    lane: 'vertex-anthropic',
    billable: true,
    baseUrl: vertexBaseUrl!,
    apiKey: vertexKey,
    rateCard: { perKPromptMsat: 6000, perKCompletionMsat: 30000, cachedPromptBilledFraction: 0.5 },
    region: 'us-central1',
    deps,
  })

  try {
    const runSet = await runRealSweep({
      config: KHALA_VS_FIREWORKS_VERTEX_DECISION_SUITE,
      preflight: {
        ownerConfirmed: true,
        ownerApprovalRef: ownerApprovalRef!,
        budgetCapMsat,
        maxBillableSamples,
      },
      transports: [khalaTransport, fireworksTransport, vertexTransport],
    })
    const report = buildBenchmarkReport(runSet)
    const safety = checkReportPublicSafety(report)
    console.log(JSON.stringify({ report, publicSafety: safety }, null, 2))
    process.exit(safety.safe && report.decisionGrade ? 0 : 1)
  } catch (error) {
    if (error instanceof RealSweepNotArmedError) {
      console.error('[khala-decision-sweep] preflight blocked the spend:')
      console.error(JSON.stringify(error.preflight.blockers, null, 2))
      console.error(NEEDS_OWNER)
      process.exit(1)
    }
    throw error
  }
}

void main()
