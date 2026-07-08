#!/usr/bin/env bun
// OB-3 (#8560): fleet lane — run the LG-1 agent-readiness prober across
// pending pipeline rows and render one hosted, tokenized, public-safe
// 15-step report per prospect in bulk.
//
// WHY A DOMAIN MAP FILE: `business_pipeline_rows` deliberately never stores
// a prospect's domain (see the privacy-boundary comments in migrations
// 0294/0295/0296 and `business-pipeline-queue.ts`'s `readMetrics`
// `privacyBoundary.excludes`) — Apollo enrichment (including the domain)
// stays outside the committed D1 schema; the pipeline is the system of
// record for STAGE, not for prospect PII/domain. That means this fleet lane
// cannot resolve `pipelineRef -> domain` from the database alone. The
// caller (an operator, or an agent with live Apollo MCP access) supplies
// that mapping explicitly as a local, never-committed JSON file. This
// script is the mechanism; wiring it to a live Apollo pull is a separate,
// explicitly out-of-repo step.
//
// Usage:
//   bun run scripts/agent-readiness-fleet-report-run.ts \
//     --domain-map ./prospect-domains.local.json \
//     --api-base https://openagents.com \
//     --admin-token "$OPENAGENTS_ADMIN_API_TOKEN" \
//     [--limit 100] [--concurrency 4] [--timeout-ms 15000] \
//     [--output-dir ./out] [--dry-run]
//
// `prospect-domains.local.json` shape: { "<pipelineRef>": "<domain>", ... }
//
// Exit code: 0 only if every mapped row produced a hosted report. Any
// per-row failure is reported honestly (never silently dropped) and the
// script exits 1 so a fleet run's receipts always reflect reality.

import { mkdir } from 'node:fs/promises'
import { join } from 'node:path'

import { scanAgentReadinessDomain, type AgentReadinessReport } from '@openagentsinc/agent-readiness'

type DomainMap = Readonly<Record<string, string>>

type FleetReportReceipt = Readonly<{
  pipelineRef: string
  domain: string
  ok: true
  reportToken: string
  url: string
  score: number
  grade: string
  receiptRef: string
}>

type FleetReportFailure = Readonly<{
  pipelineRef: string
  domain: string
  ok: false
  stage: 'scan' | 'create'
  reason: string
}>

type FleetReportOutcome = FleetReportReceipt | FleetReportFailure

const args = process.argv.slice(2)

const option = (name: string): string | undefined => {
  const index = args.indexOf(name)
  if (index < 0) return undefined
  const value = args[index + 1]
  return value === undefined || value.startsWith('--') ? undefined : value
}

const flag = (name: string): boolean => args.includes(name)

const help = (): string =>
  [
    'Usage: bun run scripts/agent-readiness-fleet-report-run.ts [options]',
    '',
    'OB-3 (#8560) fleet lane: scans each pipelineRef->domain pair with the',
    'real LG-1 agent-readiness prober, renders the OB-3 15-step assessment,',
    'and creates a hosted public-safe tokenized report per prospect via the',
    'operator API. Never fabricates findings — every score/gap comes from a',
    'real public HTTP scan of the prospect\'s own domain.',
    '',
    'Options:',
    '  --domain-map <file>     Required. JSON { pipelineRef: domain }.',
    '  --api-base <url>        Default https://openagents.com.',
    '  --admin-token <token>   Default $OPENAGENTS_ADMIN_API_TOKEN.',
    '  --limit <n>             Only process the first n rows (default: all).',
    '  --concurrency <n>       Bounded parallel scans (default: 4).',
    '  --timeout-ms <n>        Per-domain scan timeout (default: 15000).',
    '  --output-dir <dir>      Write receipts.json + a human summary there.',
    '  --dry-run               Scan + render only; do not POST/create reports.',
    '  --help',
  ].join('\n')

if (flag('--help')) {
  process.stdout.write(`${help()}\n`)
  process.exit(0)
}

const domainMapPath = option('--domain-map')
if (domainMapPath === undefined) {
  process.stderr.write('Missing required --domain-map <file>.\n\n')
  process.stderr.write(`${help()}\n`)
  process.exit(1)
}

const apiBase = option('--api-base') ?? 'https://openagents.com'
const adminToken = option('--admin-token') ?? Bun.env.OPENAGENTS_ADMIN_API_TOKEN
const limit = Number(option('--limit') ?? Number.POSITIVE_INFINITY)
const concurrency = Math.max(1, Number(option('--concurrency') ?? 4))
const timeoutMs = Number(option('--timeout-ms') ?? 15_000)
const outputDir = option('--output-dir')
const dryRun = flag('--dry-run')

if (!dryRun && (adminToken === undefined || adminToken === '')) {
  process.stderr.write(
    'Missing --admin-token (or $OPENAGENTS_ADMIN_API_TOKEN). Pass --dry-run to scan without creating hosted reports.\n',
  )
  process.exit(1)
}

const { readFile } = await import('node:fs/promises')
const domainMap = JSON.parse(await readFile(domainMapPath, 'utf8')) as DomainMap
const entries = Object.entries(domainMap).slice(
  0,
  Number.isFinite(limit) ? limit : undefined,
)

if (entries.length === 0) {
  process.stderr.write('Domain map is empty — nothing to run.\n')
  process.exit(1)
}

const createReport = async (
  pipelineRef: string,
  report: AgentReadinessReport,
): Promise<FleetReportOutcome> => {
  const response = await fetch(`${apiBase}/api/operator/agent-readiness/reports`, {
    method: 'POST',
    headers: {
      authorization: `Bearer ${adminToken}`,
      'content-type': 'application/json',
    },
    body: JSON.stringify({ pipelineRef, report }),
  })
  const body = (await response.json().catch(() => ({}))) as Record<string, unknown>
  if (!response.ok) {
    return {
      pipelineRef,
      domain: report.domain,
      ok: false,
      stage: 'create',
      reason: `HTTP ${response.status}: ${JSON.stringify(body)}`,
    }
  }
  const created = (body.report ?? {}) as Record<string, unknown>
  return {
    pipelineRef,
    domain: report.domain,
    ok: true,
    reportToken: String(created.reportToken ?? ''),
    url: String(created.url ?? ''),
    score: Number(created.score ?? report.score),
    grade: String(created.grade ?? report.grade),
    receiptRef: String(created.receiptRef ?? ''),
  }
}

const runOne = async ([pipelineRef, domain]: readonly [string, string]): Promise<FleetReportOutcome> => {
  let report: AgentReadinessReport
  try {
    report = await scanAgentReadinessDomain(domain, { timeoutMs })
  } catch (error) {
    return {
      pipelineRef,
      domain,
      ok: false,
      stage: 'scan',
      reason: error instanceof Error ? error.message : String(error),
    }
  }

  if (dryRun) {
    return {
      pipelineRef,
      domain,
      ok: true,
      reportToken: '(dry-run)',
      url: '(dry-run)',
      score: report.score,
      grade: report.grade,
      receiptRef: '(dry-run)',
    }
  }

  return createReport(pipelineRef, report)
}

// Bounded-concurrency pool — sequential probes per domain (agent-readiness's
// own scanner is already polite/rate-limited per domain), N domains in
// flight at once across the fleet run.
const runPool = async (
  items: ReadonlyArray<readonly [string, string]>,
  poolSize: number,
): Promise<ReadonlyArray<FleetReportOutcome>> => {
  const results = new Array<FleetReportOutcome>(items.length)
  let cursor = 0
  const worker = async (): Promise<void> => {
    while (cursor < items.length) {
      const index = cursor
      cursor += 1
      const item = items[index]
      if (item === undefined) continue
      results[index] = await runOne(item)
      process.stderr.write(
        `[agent-readiness-fleet] ${index + 1}/${items.length} pipelineRef=${item[0]} domain=${item[1]} -> ${
          results[index]?.ok ? 'ok' : `FAILED(${(results[index] as FleetReportFailure).stage})`
        }\n`,
      )
    }
  }
  await Promise.all(Array.from({ length: Math.min(poolSize, items.length) }, worker))
  return results
}

const outcomes = await runPool(entries as ReadonlyArray<readonly [string, string]>, concurrency)

const successes = outcomes.filter((outcome): outcome is FleetReportReceipt => outcome.ok)
const failures = outcomes.filter((outcome): outcome is FleetReportFailure => !outcome.ok)

const summary = {
  schemaVersion: 'openagents.agent_readiness_fleet_run_receipt.v1' as const,
  generatedAt: new Date().toISOString(),
  requested: entries.length,
  succeeded: successes.length,
  failed: failures.length,
  dryRun,
  receipts: successes,
  failures,
  sourceRefs: [
    'github:OpenAgentsInc/openagents#8560',
    'docs/fable/2026-07-07-palantir-institutional-sovereignty-smb-analysis.md',
  ],
}

if (outputDir !== undefined) {
  await mkdir(outputDir, { recursive: true })
  await Bun.write(
    join(outputDir, 'agent-readiness-fleet-run-receipts.json'),
    `${JSON.stringify(summary, null, 2)}\n`,
  )
}

process.stdout.write(`${JSON.stringify(summary, null, 2)}\n`)
process.stderr.write(
  `[agent-readiness-fleet] requested=${summary.requested} succeeded=${summary.succeeded} failed=${summary.failed}${dryRun ? ' (dry-run: no reports created)' : ''}\n`,
)

process.exit(failures.length === 0 ? 0 : 1)
