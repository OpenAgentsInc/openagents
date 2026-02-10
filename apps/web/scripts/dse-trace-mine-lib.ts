export type TraceMineCliOptions = {
  readonly baseUrl: string
  readonly signatureId: string
  readonly limit: number
  readonly split: "train" | "dev" | "holdout" | "test"
  readonly tags: ReadonlyArray<string>
  readonly strategyId: string | null
  readonly resultTag: "Ok" | "Error" | null
  readonly requireRlmTrace: boolean
  readonly dryRun: boolean
  readonly concurrency: number
}

export type TraceMineEnv = {
  readonly OA_DSE_ADMIN_SECRET: string
}

export type ParsedTraceMineArgs =
  | { readonly ok: true; readonly options: TraceMineCliOptions }
  | { readonly ok: false; readonly error: string; readonly usage: string }

const USAGE = `Usage:
  bun run apps/web/scripts/dse-trace-mine.ts --base-url <url> --signature-id <id>

Required env:
  OA_DSE_ADMIN_SECRET=...

Options:
  --base-url <url>          Base URL for the Worker (e.g. http://localhost:3000, https://openagents.com)
  --signature-id <id>       Signature id to mine (e.g. @openagents/autopilot/canary/RecapThread.v1)
  --limit <n>               Max receipts to scan (default: 50, max: 200)
  --split <train|dev|holdout|test>  Split for exported examples (default: train)
  --strategy-id <id>        Filter receipts by strategy id (default: rlm_lite.v1)
  --result-tag <Ok|Error>   Filter receipts by result tag (default: Ok)
  --require-rlm-trace       Require rlm trace blob id (default: on)
  --no-require-rlm-trace    Disable rlm trace requirement
  --tag <tag>               Add a tag to exported examples (repeatable)
  --tags <a,b,c>            Add tags (comma separated)
  --dry-run                 Do not write to Convex (calls trace export with dryRun=true)
  --concurrency <n>         Concurrent exports (default: 3, max: 10)
`

const takeArgValue = (argv: ReadonlyArray<string>, i: number): string | null => {
  const v = argv[i + 1]
  if (!v) return null
  if (v.startsWith("--")) return null
  return v
}

const parseIntBounded = (raw: unknown, fallback: number, min: number, max: number): number => {
  const n = typeof raw === "string" && raw.trim().length > 0 ? Number(raw) : NaN
  if (!Number.isFinite(n)) return fallback
  return Math.max(min, Math.min(max, Math.floor(n)))
}

const parseSplit = (raw: string | null): "train" | "dev" | "holdout" | "test" => {
  if (raw === "train" || raw === "dev" || raw === "holdout" || raw === "test") return raw
  return "train"
}

const normalizeTag = (t: unknown): string | null => {
  if (typeof t !== "string") return null
  const s = t.trim()
  if (!s) return null
  if (s.length > 80) return s.slice(0, 80)
  return s
}

const parseTags = (argv: ReadonlyArray<string>): string[] => {
  const out: string[] = []

  for (let i = 0; i < argv.length; i++) {
    const a = argv[i]
    if (a === "--tag") {
      const v = takeArgValue(argv, i)
      const t = normalizeTag(v)
      if (t) out.push(t)
    }
    if (a.startsWith("--tags=")) {
      const rest = a.slice("--tags=".length)
      for (const p of rest.split(",")) {
        const t = normalizeTag(p)
        if (t) out.push(t)
      }
    }
    if (a === "--tags") {
      const v = takeArgValue(argv, i)
      if (!v) continue
      for (const p of v.split(",")) {
        const t = normalizeTag(p)
        if (t) out.push(t)
      }
    }
  }

  // Ensure the mining tag is always present (bounded + stable).
  out.push("trace_mined")

  return Array.from(new Set(out)).slice(0, 50)
}

export const parseTraceMineArgs = (argv: ReadonlyArray<string>): ParsedTraceMineArgs => {
  const args = [...argv]

  const rawBaseUrl =
    args.find((a) => a.startsWith("--base-url="))?.slice("--base-url=".length) ??
    (() => {
      const i = args.indexOf("--base-url")
      if (i < 0) return null
      return takeArgValue(args, i)
    })()

  const baseUrl = typeof rawBaseUrl === "string" ? rawBaseUrl.trim() : ""
  if (!baseUrl) return { ok: false, error: "missing --base-url", usage: USAGE }

  const rawSignatureId =
    args.find((a) => a.startsWith("--signature-id="))?.slice("--signature-id=".length) ??
    (() => {
      const i = args.indexOf("--signature-id")
      if (i < 0) return null
      return takeArgValue(args, i)
    })()

  const signatureId = typeof rawSignatureId === "string" ? rawSignatureId.trim() : ""
  if (!signatureId) return { ok: false, error: "missing --signature-id", usage: USAGE }

  const rawLimit =
    args.find((a) => a.startsWith("--limit="))?.slice("--limit=".length) ??
    (() => {
      const i = args.indexOf("--limit")
      if (i < 0) return null
      return takeArgValue(args, i)
    })()

  const limit = parseIntBounded(rawLimit, 50, 0, 200)

  const rawSplit =
    args.find((a) => a.startsWith("--split="))?.slice("--split=".length) ??
    (() => {
      const i = args.indexOf("--split")
      if (i < 0) return null
      return takeArgValue(args, i)
    })()

  const split = parseSplit(typeof rawSplit === "string" ? rawSplit.trim() : null)

  const rawStrategyId =
    args.find((a) => a.startsWith("--strategy-id="))?.slice("--strategy-id=".length) ??
    (() => {
      const i = args.indexOf("--strategy-id")
      if (i < 0) return null
      return takeArgValue(args, i)
    })()

  const strategyId =
    rawStrategyId === null || rawStrategyId === undefined
      ? "rlm_lite.v1"
      : typeof rawStrategyId === "string" && rawStrategyId.trim().length > 0
        ? rawStrategyId.trim()
        : null

  const rawResultTag =
    args.find((a) => a.startsWith("--result-tag="))?.slice("--result-tag=".length) ??
    (() => {
      const i = args.indexOf("--result-tag")
      if (i < 0) return null
      return takeArgValue(args, i)
    })()

  const resultTag =
    rawResultTag === null || rawResultTag === undefined
      ? ("Ok" as const)
      : rawResultTag === "Ok" || rawResultTag === "Error"
        ? (rawResultTag as "Ok" | "Error")
        : null

  let requireRlmTrace: boolean | null = null
  if (args.includes("--require-rlm-trace")) requireRlmTrace = true
  if (args.includes("--no-require-rlm-trace")) requireRlmTrace = false

  const dryRun = args.includes("--dry-run")

  const rawConcurrency =
    args.find((a) => a.startsWith("--concurrency="))?.slice("--concurrency=".length) ??
    (() => {
      const i = args.indexOf("--concurrency")
      if (i < 0) return null
      return takeArgValue(args, i)
    })()

  const concurrency = parseIntBounded(rawConcurrency, 3, 1, 10)

  const tags = parseTags(args)

  return {
    ok: true,
    options: {
      baseUrl,
      signatureId,
      limit,
      split,
      tags,
      strategyId,
      resultTag,
      requireRlmTrace: requireRlmTrace ?? true,
      dryRun,
      concurrency,
    },
  }
}

export type FetchLike = (input: RequestInfo | URL, init?: RequestInit) => Promise<Response>

type ReceiptListRow = {
  readonly receiptId: string
  readonly signatureId: string
  readonly compiled_id: string
  readonly threadId: string
  readonly runId: string
  readonly createdAtMs: number
  readonly strategyId: string | null
  readonly resultTag: "Ok" | "Error" | null
  readonly rlmTraceBlobId: string | null
  readonly rlmTraceEventCount: number | null
}

export type TraceMineResultRow = {
  readonly receiptId: string
  readonly ok: boolean
  readonly exampleId?: string | undefined
  readonly existed?: boolean | undefined
  readonly error?: string | undefined
  readonly requestId?: string | null | undefined
}

export type TraceMineSummary = {
  readonly ok: boolean
  readonly signatureId: string
  readonly totalReceipts: number
  readonly exported: number
  readonly skipped: number
  readonly failed: number
  readonly dryRun: boolean
  readonly results: ReadonlyArray<TraceMineResultRow>
}

const fetchJsonWithTimeout = async (
  fetchFn: FetchLike,
  input: { readonly url: string; readonly init: RequestInit; readonly timeoutMs: number },
): Promise<{ readonly response: Response; readonly json: any; readonly requestId: string | null }> => {
  const controller = new AbortController()
  const t = setTimeout(() => controller.abort(), Math.max(1, input.timeoutMs))
  try {
    const response = await fetchFn(input.url, { ...input.init, signal: controller.signal })
    const json = await response.json().catch(() => null)
    const requestId = response.headers.get("x-oa-request-id")
    return { response, json, requestId: typeof requestId === "string" && requestId.length > 0 ? requestId : null }
  } finally {
    clearTimeout(t)
  }
}

const truncate = (s: string, max: number): string => (s.length <= max ? s : `${s.slice(0, max)}…(truncated)`)

const asReceiptListRow = (u: unknown): ReceiptListRow | null => {
  const r = u as any
  const receiptId = typeof r?.receiptId === "string" ? r.receiptId : ""
  const signatureId = typeof r?.signatureId === "string" ? r.signatureId : ""
  const compiled_id = typeof r?.compiled_id === "string" ? r.compiled_id : ""
  const threadId = typeof r?.threadId === "string" ? r.threadId : ""
  const runId = typeof r?.runId === "string" ? r.runId : ""
  const createdAtMs = typeof r?.createdAtMs === "number" && Number.isFinite(r.createdAtMs) ? r.createdAtMs : 0
  const strategyId = typeof r?.strategyId === "string" ? r.strategyId : null
  const resultTag = r?.resultTag === "Ok" || r?.resultTag === "Error" ? (r.resultTag as "Ok" | "Error") : null
  const rlmTraceBlobId = typeof r?.rlmTraceBlobId === "string" ? r.rlmTraceBlobId : null
  const rlmTraceEventCount =
    typeof r?.rlmTraceEventCount === "number" && Number.isFinite(r.rlmTraceEventCount) ? r.rlmTraceEventCount : null
  if (!receiptId || !signatureId) return null
  return {
    receiptId,
    signatureId,
    compiled_id,
    threadId,
    runId,
    createdAtMs,
    strategyId,
    resultTag,
    rlmTraceBlobId,
    rlmTraceEventCount,
  }
}

export const runTraceMine = async (input: {
  readonly options: TraceMineCliOptions
  readonly env: TraceMineEnv
  readonly fetchFn: FetchLike
  readonly timeoutMs?: number | undefined
}): Promise<TraceMineSummary> => {
  const baseUrl = input.options.baseUrl.replace(/\/+$/, "")
  const timeoutMs = typeof input.timeoutMs === "number" ? input.timeoutMs : 30_000

  const authHeader = `Bearer ${input.env.OA_DSE_ADMIN_SECRET}`

  const params = new URLSearchParams()
  params.set("signatureId", input.options.signatureId)
  params.set("limit", String(input.options.limit))
  if (input.options.requireRlmTrace) params.set("requireRlmTrace", "1")
  if (input.options.resultTag) params.set("resultTag", input.options.resultTag)
  if (input.options.strategyId) params.set("strategyId", input.options.strategyId)

  const listUrl = `${baseUrl}/api/dse/receipts/list?${params.toString()}`

  const listRes = await fetchJsonWithTimeout(input.fetchFn, {
    url: listUrl,
    timeoutMs,
    init: {
      method: "GET",
      cache: "no-store",
      headers: {
        authorization: authHeader,
      },
    },
  })

  if (!listRes.response.ok || !listRes.json || listRes.json.ok !== true) {
    const msg = listRes.json && typeof listRes.json.error === "string" ? listRes.json.error : `HTTP ${listRes.response.status}`
    throw new Error(`dse_receipts_list_failed ${msg}`)
  }

  const rawRows = Array.isArray(listRes.json.receipts) ? (listRes.json.receipts as unknown[]) : []
  const receipts = rawRows.map(asReceiptListRow).filter((r): r is ReceiptListRow => Boolean(r))

  const results: TraceMineResultRow[] = []
  let exported = 0
  let skipped = 0
  let failed = 0

  const exportOne = async (r: ReceiptListRow): Promise<void> => {
    if (!r.receiptId) {
      skipped++
      return
    }

    const body = {
      receiptId: r.receiptId,
      split: input.options.split,
      tags: [...input.options.tags],
      dryRun: input.options.dryRun,
    }

    const { response, json, requestId } = await fetchJsonWithTimeout(input.fetchFn, {
      url: `${baseUrl}/api/dse/trace/export`,
      timeoutMs,
      init: {
        method: "POST",
        cache: "no-store",
        headers: {
          "content-type": "application/json; charset=utf-8",
          authorization: authHeader,
        },
        body: JSON.stringify(body),
      },
    })

    if (!response.ok || !json || json.ok !== true) {
      failed++
      const msg = json && typeof json.error === "string" ? json.error : `HTTP ${response.status}`
      results.push({ receiptId: r.receiptId, ok: false, error: truncate(msg, 300), requestId })
      return
    }

    exported++
    results.push({
      receiptId: r.receiptId,
      ok: true,
      exampleId: typeof json.exampleId === "string" ? json.exampleId : undefined,
      existed: typeof json.existed === "boolean" ? json.existed : undefined,
      requestId,
    })
  }

  // Simple bounded worker pool.
  let i = 0
  const n = Math.max(1, Math.min(10, Math.floor(input.options.concurrency)))

  const worker = async (): Promise<void> => {
    while (true) {
      const idx = i++
      if (idx >= receipts.length) return
      await exportOne(receipts[idx]!)
    }
  }

  await Promise.all(Array.from({ length: n }, () => worker()))

  const ok = failed === 0

  return {
    ok,
    signatureId: input.options.signatureId,
    totalReceipts: receipts.length,
    exported,
    skipped,
    failed,
    dryRun: input.options.dryRun,
    results: results.slice(0, 200),
  }
}
