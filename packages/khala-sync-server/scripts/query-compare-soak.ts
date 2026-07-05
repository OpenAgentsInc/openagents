#!/usr/bin/env bun
/**
 * Compare-mode soak observability query (#8282 shared follow-up).
 *
 * WHY THIS EXISTS. Proving a `compare` read is safe to flip to real
 * Postgres serving requires a genuine multi-hour-or-longer soak with ZERO
 * mismatches. Before this script, the only way to observe that was a
 * `wrangler tail` piped to one agent's terminal for one session — not a
 * real soak, invisible once the session ends, and silently vacuous for
 * near-zero-traffic domains (a "clean" tail on zero requests proves
 * nothing). `../src/compare-soak-metrics.ts` records one durable Cloudflare
 * Analytics Engine data point per compare-mode read; this script queries
 * that dataset via the Analytics Engine SQL API
 * (https://api.cloudflare.com/client/v4/accounts/{account_id}/analytics_engine/sql)
 * and reports, per domain, over a time window:
 *   - total compare-mode reads served
 *   - mismatch count (the drift that blocks a flip)
 *   - error count (shadow Postgres read itself failed — not a comparable
 *     mismatch, but still real traffic, so a domain never reads as
 *     vacuous just because its shadow reads are erroring)
 *   - VACUOUS: true when the domain has ZERO rows in the window at all —
 *     the exact failure mode the #8361 supervision pass hit manually
 *     (`omni_public_proof_bundles` has no organic traffic, so a clean
 *     `wrangler tail` there proved nothing). A domain absent from the
 *     query result is indistinguishable from "worked perfectly" unless
 *     this script explicitly cross-references the known domain list and
 *     flags the gap.
 *
 * This is READ-ONLY observability. It never flips a domain's read flag —
 * that decision still needs real elapsed time after this script (and the
 * metrics pipeline it queries) land, plus a human/ops call recorded on the
 * epic per docs/khala-sync/RUNBOOK.md.
 *
 * Usage (from packages/khala-sync-server/):
 *   CLOUDFLARE_API_TOKEN=<token> CLOUDFLARE_ACCOUNT_ID=<id> \
 *     bun scripts/query-compare-soak.ts [--hours <n>] [--dataset <name>] \
 *       [--domains <comma,separated,list>] [--json]
 *
 * Options:
 *   --hours <n>      Lookback window in hours (default 6).
 *   --dataset <name> Analytics Engine dataset name (default
 *                    khala_sync_compare_soak — the production binding's
 *                    dataset; pass khala_sync_compare_soak_staging for
 *                    staging).
 *   --domains <csv>  Known domain slugs to report on, comma-separated
 *                    (default: the currently-wired KS-8 domains — see
 *                    KNOWN_COMPARE_SOAK_DOMAINS below). A domain absent
 *                    from the query result is reported VACUOUS.
 *   --account-id <id>   Cloudflare account id (default $CLOUDFLARE_ACCOUNT_ID).
 *   --api-token <token> Cloudflare API token (default $CLOUDFLARE_API_TOKEN).
 *                        Needs "Account Analytics Read" permission.
 *   --json           Print the raw per-domain report as JSON instead of
 *                     the human-readable table.
 *   --help           Show this help.
 *
 * The owner's Cloudflare API token normally lives in
 * ~/work/.secrets/cloudflare-openagents.env (CLOUDFLARE_API_TOKEN) — see
 * docs/khala-sync/RUNBOOK.md's "Compare-mode soak observability" section.
 * The account id is visible via `wrangler whoami`.
 */

/**
 * The domain slugs every currently-wired compare-mode call site records
 * under (`domain` field of `CompareSoakSample` — see
 * ../src/compare-soak-metrics.ts and each call site in
 * apps/openagents.com/workers/api/src/*.ts). Keep this list in sync with
 * new domains as they get wired — see docs/khala-sync/RUNBOOK.md.
 */
export const KNOWN_COMPARE_SOAK_DOMAINS: ReadonlyArray<string> = [
  "entitlements_gate",
  "entitlements_non_gate",
  "supervision",
  "artanis",
  "billing",
  "forge",
]

export type CompareSoakDomainReport = Readonly<{
  domain: string
  /** true when the domain had ZERO compare-mode reads in the window — a "clean" run here is VACUOUS, not evidence. */
  vacuous: boolean
  totalReads: number
  matches: number
  mismatches: number
  errors: number
  windowStart: string | null
  windowEnd: string | null
}>

export type CompareSoakQueryReport = Readonly<{
  dataset: string
  hours: number
  queriedAt: string
  domains: ReadonlyArray<CompareSoakDomainReport>
}>

/** One raw row shape as the Analytics Engine SQL API returns it. */
type RawSoakRow = Readonly<{
  domain: string
  total_reads: number | string
  matches: number | string
  mismatches: number | string
  errors: number | string
  window_start: string | null
  window_end: string | null
}>

const toNumber = (value: number | string | null | undefined): number => {
  if (value === null || value === undefined) return 0
  const n = typeof value === "number" ? value : Number.parseFloat(value)
  return Number.isFinite(n) ? n : 0
}

/**
 * Pure report builder — cross-references the raw per-domain query rows
 * against the full known-domain list so a domain with NO traffic is
 * reported VACUOUS rather than silently omitted. Unit-tested without any
 * real Cloudflare API call.
 */
export const buildCompareSoakReport = (input: {
  dataset: string
  hours: number
  queriedAt: string
  rows: ReadonlyArray<RawSoakRow>
  knownDomains?: ReadonlyArray<string> | undefined
}): CompareSoakQueryReport => {
  const knownDomains = input.knownDomains ?? KNOWN_COMPARE_SOAK_DOMAINS
  const byDomain = new Map(input.rows.map(row => [row.domain, row]))
  // Report every known domain (vacuous if absent), PLUS any domain the
  // query returned that isn't in the known list yet (a newly-wired domain
  // the operator forgot to add to KNOWN_COMPARE_SOAK_DOMAINS / --domains).
  const domainSlugs = Array.from(
    new Set([...knownDomains, ...input.rows.map(row => row.domain)]),
  ).sort((a, b) => (a < b ? -1 : a > b ? 1 : 0))

  const domains: CompareSoakDomainReport[] = domainSlugs.map(domain => {
    const row = byDomain.get(domain)
    if (row === undefined) {
      return {
        domain,
        errors: 0,
        matches: 0,
        mismatches: 0,
        totalReads: 0,
        vacuous: true,
        windowEnd: null,
        windowStart: null,
      }
    }
    const totalReads = toNumber(row.total_reads)
    return {
      domain,
      errors: toNumber(row.errors),
      matches: toNumber(row.matches),
      mismatches: toNumber(row.mismatches),
      totalReads,
      vacuous: totalReads === 0,
      windowEnd: row.window_end,
      windowStart: row.window_start,
    }
  })

  return { dataset: input.dataset, domains, hours: input.hours, queriedAt: input.queriedAt }
}

/** The SQL sent to the Analytics Engine SQL API. */
export const compareSoakSql = (dataset: string, hours: number): string =>
  `SELECT blob1 AS domain, SUM(double1) AS total_reads, SUM(double2) AS matches, ` +
  `SUM(double3) AS mismatches, SUM(double4) AS errors, ` +
  `MIN(timestamp) AS window_start, MAX(timestamp) AS window_end ` +
  `FROM ${dataset} WHERE timestamp > NOW() - INTERVAL '${hours}' HOUR GROUP BY blob1 ORDER BY blob1`

export type FetchLike = (
  url: string,
  init: { method: string; headers: Record<string, string>; body: string },
) => Promise<{ ok: boolean; status: number; text: () => Promise<string> }>

/**
 * Query the Analytics Engine SQL API for one dataset over the last `hours`
 * hours. Returns the cross-referenced per-domain report. Throws on a
 * non-2xx response (the CLI wrapper turns that into a clear exit code and
 * message — never a silent empty report that could misread as "clean").
 */
export const queryCompareSoak = async (input: {
  accountId: string
  apiToken: string
  dataset: string
  hours: number
  knownDomains?: ReadonlyArray<string> | undefined
  fetchImpl?: FetchLike | undefined
  now?: (() => Date) | undefined
}): Promise<CompareSoakQueryReport> => {
  const fetchImpl = input.fetchImpl ?? (globalThis.fetch as unknown as FetchLike)
  const now = input.now ?? (() => new Date())
  const sql = compareSoakSql(input.dataset, input.hours)
  const response = await fetchImpl(
    `https://api.cloudflare.com/client/v4/accounts/${input.accountId}/analytics_engine/sql`,
    {
      body: sql,
      headers: {
        Authorization: `Bearer ${input.apiToken}`,
        "Content-Type": "text/plain",
      },
      method: "POST",
    },
  )
  const bodyText = await response.text()
  if (!response.ok) {
    throw new Error(
      `Analytics Engine SQL API returned ${response.status}: ${bodyText.slice(0, 500)}`,
    )
  }
  let parsed: unknown
  try {
    parsed = JSON.parse(bodyText)
  } catch (error) {
    throw new Error(
      `Analytics Engine SQL API returned non-JSON body: ${bodyText.slice(0, 500)} (${error instanceof Error ? error.message : String(error)})`,
    )
  }
  const rows = extractRows(parsed)
  return buildCompareSoakReport({
    dataset: input.dataset,
    hours: input.hours,
    knownDomains: input.knownDomains,
    queriedAt: now().toISOString(),
    rows,
  })
}

/**
 * The Analytics Engine SQL API returns ClickHouse-style JSON:
 * `{ meta: [...], data: [{col: value, ...}, ...], rows: N, ... }`. Extract
 * the `data` array defensively (an empty/missing array means zero matching
 * rows — every configured domain will then report VACUOUS, which is the
 * correct and intended signal, not an error).
 */
const extractRows = (parsed: unknown): ReadonlyArray<RawSoakRow> => {
  if (
    parsed !== null &&
    typeof parsed === "object" &&
    Array.isArray((parsed as { data?: unknown }).data)
  ) {
    return (parsed as { data: unknown[] }).data as unknown as RawSoakRow[]
  }
  return []
}

// ---------------------------------------------------------------------------
// Rendering
// ---------------------------------------------------------------------------

const pad = (value: string, width: number): string =>
  value.length >= width ? value : value + " ".repeat(width - value.length)

export const renderCompareSoakReportTable = (report: CompareSoakQueryReport): string => {
  const header = [
    `Compare-mode soak report — dataset "${report.dataset}", last ${report.hours}h, queried ${report.queriedAt}`,
    "",
    pad("DOMAIN", 22) + pad("TOTAL", 10) + pad("MATCH", 10) + pad("MISMATCH", 10) + pad("ERROR", 8) + "STATUS",
  ]
  const rows = report.domains.map(d => {
    const status = d.vacuous
      ? "VACUOUS — no compare-mode traffic in window"
      : d.mismatches > 0
        ? "MISMATCHES — do NOT flip"
        : d.errors > 0
          ? "clean (shadow read errors present)"
          : "clean"
    return (
      pad(d.domain, 22) +
      pad(String(d.totalReads), 10) +
      pad(String(d.matches), 10) +
      pad(String(d.mismatches), 10) +
      pad(String(d.errors), 8) +
      status
    )
  })
  return [...header, ...rows].join("\n")
}

// ---------------------------------------------------------------------------
// CLI
// ---------------------------------------------------------------------------

const USAGE = `Usage: bun scripts/query-compare-soak.ts [options]

Options:
  --hours <n>          Lookback window in hours (default 6).
  --dataset <name>     Analytics Engine dataset (default khala_sync_compare_soak).
  --domains <csv>      Comma-separated known domain slugs (default: the wired KS-8 domains).
  --account-id <id>    Cloudflare account id (default $CLOUDFLARE_ACCOUNT_ID).
  --api-token <token>  Cloudflare API token (default $CLOUDFLARE_API_TOKEN).
  --json               Print JSON instead of a table.
  --help                Show this help.
`

const main = async (argv: ReadonlyArray<string>): Promise<number> => {
  let hours = 6
  let dataset = "khala_sync_compare_soak"
  let domains: ReadonlyArray<string> | undefined
  let accountId = process.env["CLOUDFLARE_ACCOUNT_ID"]
  let apiToken = process.env["CLOUDFLARE_API_TOKEN"]
  let json = false

  for (let i = 0; i < argv.length; i++) {
    const arg = argv[i]
    if (arg === "--hours") {
      const raw = argv[++i]
      hours = Number.parseFloat(raw ?? "")
      if (!Number.isFinite(hours) || hours <= 0) {
        console.error("error: --hours requires a positive number\n")
        console.error(USAGE)
        return 2
      }
    } else if (arg === "--dataset") {
      dataset = argv[++i] ?? dataset
    } else if (arg === "--domains") {
      domains = (argv[++i] ?? "").split(",").map(s => s.trim()).filter(s => s.length > 0)
    } else if (arg === "--account-id") {
      accountId = argv[++i]
    } else if (arg === "--api-token") {
      apiToken = argv[++i]
    } else if (arg === "--json") {
      json = true
    } else if (arg === "--help" || arg === "-h") {
      console.log(USAGE)
      return 0
    } else {
      console.error(`error: unknown argument ${JSON.stringify(arg)}\n`)
      console.error(USAGE)
      return 2
    }
  }

  if (accountId === undefined || accountId === "") {
    console.error(
      "✘ query-compare-soak: no Cloudflare account id — pass --account-id or set " +
        "CLOUDFLARE_ACCOUNT_ID (see `wrangler whoami`)",
    )
    return 2
  }
  if (apiToken === undefined || apiToken === "") {
    console.error(
      "✘ query-compare-soak: no Cloudflare API token — pass --api-token or set " +
        "CLOUDFLARE_API_TOKEN (owner secret: ~/work/.secrets/cloudflare-openagents.env — " +
        'needs "Account Analytics Read" permission)',
    )
    return 2
  }

  try {
    const report = await queryCompareSoak({ accountId, apiToken, dataset, hours, knownDomains: domains })
    if (json) {
      console.log(JSON.stringify(report, null, 2))
    } else {
      console.log(renderCompareSoakReportTable(report))
    }
    return 0
  } catch (error) {
    console.error(
      `✘ query-compare-soak: ${error instanceof Error ? error.message : String(error)}`,
    )
    return 1
  }
}

if (import.meta.main) {
  process.exit(await main(process.argv.slice(2)))
}
