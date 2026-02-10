export type OvernightCliOptions = {
  readonly baseUrl: string
  readonly verify: boolean
  readonly runE2e: boolean
  readonly e2eGrep: string
}

export type OvernightEnv = {
  readonly OA_DSE_ADMIN_SECRET: string
  readonly EFFUSE_TEST_E2E_BYPASS_SECRET?: string | undefined
}

export type ParsedOvernightArgs =
  | { readonly ok: true; readonly options: OvernightCliOptions }
  | { readonly ok: false; readonly error: string; readonly usage: string }

const USAGE = `Usage:
  bun run apps/web/scripts/dse-overnight.ts --base-url <url>

Required env:
  OA_DSE_ADMIN_SECRET=...

Optional env (prod E2E):
  EFFUSE_TEST_E2E_BYPASS_SECRET=...

Options:
  --base-url <url>        Base URL for the Worker (e.g. http://localhost:3000, https://openagents.com)
  --verify | --no-verify  Run local verification commands (default: on for localhost, off otherwise)
  --e2e | --no-e2e        Run prod E2E smoke (default: on for prod-ish URLs, off otherwise)
  --e2e-grep <regex>      Regex for prod E2E grep (default: DSE recap visibility test)
`

export const isLocalhostBaseUrl = (baseUrl: string): boolean => {
  try {
    const u = new URL(baseUrl)
    const host = u.hostname
    return host === "localhost" || host === "127.0.0.1" || host === "0.0.0.0"
  } catch {
    return false
  }
}

export const isProdishBaseUrl = (baseUrl: string): boolean => {
  try {
    const u = new URL(baseUrl)
    const host = u.hostname
    return host === "openagents.com" || host.endsWith(".openagents.com") || host.endsWith(".openagents.workers.dev")
  } catch {
    return false
  }
}

const takeArgValue = (argv: ReadonlyArray<string>, i: number): string | null => {
  const v = argv[i + 1]
  if (!v) return null
  if (v.startsWith("--")) return null
  return v
}

export const parseOvernightArgs = (argv: ReadonlyArray<string>): ParsedOvernightArgs => {
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

  let verifyFlag: boolean | null = null
  if (args.includes("--verify")) verifyFlag = true
  if (args.includes("--no-verify")) verifyFlag = false

  let e2eFlag: boolean | null = null
  if (args.includes("--e2e")) e2eFlag = true
  if (args.includes("--no-e2e")) e2eFlag = false

  const rawE2eGrep =
    args.find((a) => a.startsWith("--e2e-grep="))?.slice("--e2e-grep=".length) ??
    (() => {
      const i = args.indexOf("--e2e-grep")
      if (i < 0) return null
      return takeArgValue(args, i)
    })()

  const e2eGrep =
    typeof rawE2eGrep === "string" && rawE2eGrep.trim().length > 0
      ? rawE2eGrep.trim()
      : "apps-web\\\\.prod\\\\.autopilot\\\\.dse-canary-recap-shows-debug-card-and-trace"

  const verifyDefault = isLocalhostBaseUrl(baseUrl)
  const runE2eDefault = isProdishBaseUrl(baseUrl)

  return {
    ok: true,
    options: {
      baseUrl,
      verify: verifyFlag ?? verifyDefault,
      runE2e: e2eFlag ?? runE2eDefault,
      e2eGrep,
    },
  }
}

export type OpsEventLevel = "info" | "warn" | "error"

export type OpsClient = {
  readonly startRun: (input: {
    readonly commitSha?: string | undefined
    readonly baseUrl?: string | undefined
    readonly signatureIds?: ReadonlyArray<string> | undefined
    readonly notes?: string | undefined
    readonly links?: unknown
  }) => Promise<{ readonly runId: string }>
  readonly event: (input: {
    readonly runId: string
    readonly level: OpsEventLevel
    readonly phase?: string | undefined
    readonly message: string
    readonly json?: unknown
    readonly tsMs?: number | undefined
  }) => Promise<void>
  readonly finishRun: (input: {
    readonly runId: string
    readonly status: "finished" | "failed"
    readonly summaryJson?: unknown
  }) => Promise<void>
}

export type FetchLike = (input: RequestInfo | URL, init?: RequestInit) => Promise<Response>

export type RunCommandResult = {
  readonly ok: boolean
  readonly code: number | null
  readonly stdout: string
  readonly stderr: string
  readonly durationMs: number
  readonly timedOut: boolean
}

export type RunCommand = (input: {
  readonly cwd: string
  readonly command: string
  readonly args: ReadonlyArray<string>
  readonly env?: Record<string, string | undefined>
  readonly timeoutMs: number
}) => Promise<RunCommandResult>

const truncate = (s: string, max: number): string => (s.length <= max ? s : `${s.slice(0, max)}…(truncated)`)

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

export const makeOpsClient = (input: {
  readonly baseUrl: string
  readonly adminSecret: string
  readonly fetchFn: FetchLike
  readonly timeoutMs?: number | undefined
}): OpsClient => {
  const baseUrl = input.baseUrl.replace(/\/+$/, "")
  const timeoutMs = typeof input.timeoutMs === "number" ? input.timeoutMs : 15_000

  const post = async (path: string, body: unknown): Promise<any> => {
    const { response, json } = await fetchJsonWithTimeout(input.fetchFn, {
      url: `${baseUrl}${path}`,
      timeoutMs,
      init: {
        method: "POST",
        cache: "no-store",
        headers: {
          "content-type": "application/json; charset=utf-8",
          authorization: `Bearer ${input.adminSecret}`,
        },
        body: JSON.stringify(body),
      },
    })

    if (!response.ok || !json || json.ok !== true) {
      const msg = json && typeof json.error === "string" ? json.error : `HTTP ${response.status}`
      throw new Error(`dse_ops_http_error path=${path} ${msg}`)
    }
    return json
  }

  return {
    startRun: async ({ commitSha, baseUrl, signatureIds, notes, links }) => {
      const json = await post("/api/dse/ops/run/start", {
        ...(commitSha ? { commitSha } : {}),
        ...(baseUrl ? { baseUrl } : {}),
        ...(signatureIds ? { signatureIds } : {}),
        ...(notes ? { notes } : {}),
        ...(links === undefined ? {} : { links }),
      })
      const runId = typeof json.runId === "string" ? json.runId : ""
      if (!runId) throw new Error("dse_ops_invalid_start_response")
      return { runId }
    },
    event: async ({ runId, level, phase, message, json, tsMs }) => {
      await post("/api/dse/ops/run/event", {
        runId,
        level,
        ...(phase ? { phase } : {}),
        message,
        ...(json === undefined ? {} : { json }),
        ...(typeof tsMs === "number" ? { tsMs } : {}),
      })
    },
    finishRun: async ({ runId, status, summaryJson }) => {
      await post("/api/dse/ops/run/finish", {
        runId,
        status,
        ...(summaryJson === undefined ? {} : { summaryJson }),
      })
    },
  }
}

type DseAdminClient = {
  readonly importExamples: (input: {
    readonly signatureId: string
    readonly jsonl: string
    readonly opsRunId?: string | undefined
    readonly source?: string | undefined
    readonly tagsAppend?: ReadonlyArray<string> | undefined
  }) => Promise<{ readonly requestId: string | null; readonly result: any }>
  readonly compile: (input: { readonly signatureId: string }) => Promise<{ readonly requestId: string | null; readonly result: any }>
  readonly canaryStart: (input: {
    readonly signatureId: string
    readonly canary_compiled_id: string
    readonly rolloutPct: number
    readonly salt: string
    readonly minHoldoutDelta?: number | undefined
    readonly requireHoldout?: boolean | undefined
    readonly minSamples?: number | undefined
    readonly maxErrorRate?: number | undefined
    readonly reason?: string | undefined
  }) => Promise<{ readonly requestId: string | null; readonly result: any }>
  readonly canaryStatus: (input: { readonly signatureId: string }) => Promise<{ readonly requestId: string | null; readonly result: any }>
  readonly canaryStop: (input: { readonly signatureId: string; readonly reason?: string | undefined }) => Promise<{ readonly requestId: string | null; readonly result: any }>
  readonly promote: (input: {
    readonly signatureId: string
    readonly compiled_id: string
    readonly minHoldoutDelta?: number | undefined
    readonly requireHoldout?: boolean | undefined
  }) => Promise<{ readonly requestId: string | null; readonly result: any }>
  readonly ensureExerciseThread: () => Promise<{ readonly requestId: string | null; readonly result: any }>
  readonly exercisePredict: (input: {
    readonly signatureId: string
    readonly threadId: string
    readonly count: number
    readonly split?: string | undefined
    readonly limit?: number | undefined
  }) => Promise<{ readonly requestId: string | null; readonly result: any }>
}

export const makeDseAdminClient = (input: {
  readonly baseUrl: string
  readonly adminSecret: string
  readonly fetchFn: FetchLike
}): DseAdminClient => {
  const baseUrl = input.baseUrl.replace(/\/+$/, "")

  const post = async (path: string, body: unknown, timeoutMs: number): Promise<{ readonly requestId: string | null; readonly result: any }> => {
    const { response, json, requestId } = await fetchJsonWithTimeout(input.fetchFn, {
      url: `${baseUrl}${path}`,
      timeoutMs,
      init: {
        method: "POST",
        cache: "no-store",
        headers: {
          "content-type": "application/json; charset=utf-8",
          authorization: `Bearer ${input.adminSecret}`,
        },
        body: JSON.stringify(body),
      },
    })

    if (!response.ok || !json || json.ok !== true) {
      const msg = json && typeof json.error === "string" ? json.error : `HTTP ${response.status}`
      throw new Error(`dse_admin_http_error path=${path} ${msg}`)
    }
    return { requestId, result: json }
  }

  const get = async (path: string, timeoutMs: number): Promise<{ readonly requestId: string | null; readonly result: any }> => {
    const { response, json, requestId } = await fetchJsonWithTimeout(input.fetchFn, {
      url: `${baseUrl}${path}`,
      timeoutMs,
      init: {
        method: "GET",
        cache: "no-store",
        headers: {
          "content-type": "application/json; charset=utf-8",
          authorization: `Bearer ${input.adminSecret}`,
        },
      },
    })

    if (!response.ok || !json || json.ok !== true) {
      const msg = json && typeof json.error === "string" ? json.error : `HTTP ${response.status}`
      throw new Error(`dse_admin_http_error path=${path} ${msg}`)
    }
    return { requestId, result: json }
  }

  return {
    importExamples: async ({ signatureId, jsonl, opsRunId, source, tagsAppend }) =>
      post(
        "/api/dse/examples/import",
        {
          signatureId,
          jsonl,
          ...(opsRunId ? { opsRunId } : {}),
          ...(source ? { source } : {}),
          ...(tagsAppend ? { tagsAppend } : {}),
        },
        60_000,
      ),
    compile: async ({ signatureId }) => post("/api/dse/compile", { signatureId }, 10 * 60_000),
    canaryStart: async (body) => post("/api/dse/canary/start", body, 10 * 60_000),
    canaryStatus: async ({ signatureId }) =>
      get(`/api/dse/canary/status?signatureId=${encodeURIComponent(signatureId)}`, 15_000),
    canaryStop: async ({ signatureId, reason }) =>
      post("/api/dse/canary/stop", { signatureId, ...(reason ? { reason } : {}) }, 60_000),
    promote: async (body) => post("/api/dse/promote", body, 10 * 60_000),
    ensureExerciseThread: async () => post("/api/dse/exercise/thread/ensure", {}, 60_000),
    exercisePredict: async ({ signatureId, threadId, count, split, limit }) =>
      post(
        "/api/dse/exercise/predict",
        { signatureId, threadId, count, ...(split ? { split } : {}), ...(typeof limit === "number" ? { limit } : {}) },
        10 * 60_000,
      ),
  }
}

const tryGetCommitSha = async (runCommand: RunCommand): Promise<string | undefined> => {
  try {
    const res = await runCommand({
      cwd: ".",
      command: "git",
      args: ["rev-parse", "HEAD"],
      timeoutMs: 5_000,
    })
    if (!res.ok) return undefined
    const sha = res.stdout.trim()
    if (!sha) return undefined
    return sha.slice(0, 80)
  } catch {
    return undefined
  }
}

export type OvernightRunSummary = {
  readonly ok: boolean
  readonly runId: string
  readonly baseUrl: string
  readonly verify: { readonly ran: boolean; readonly ok: boolean }
  readonly e2e: { readonly ran: boolean; readonly ok: boolean }
  readonly phase5?: unknown
  readonly durationsMs: {
    readonly total: number
    readonly verify?: number | undefined
    readonly e2e?: number | undefined
  }
  readonly errors: ReadonlyArray<string>
}

const stableBucket100 = (key: string): number => {
  // FNV-1a 32-bit hash, then mod 100. Must match Worker implementation.
  let h = 2166136261
  for (let i = 0; i < key.length; i++) {
    h ^= key.charCodeAt(i)
    h = Math.imul(h, 16777619)
  }
  return (h >>> 0) % 100
}

const sleepMs = (ms: number): Promise<void> => new Promise((r) => setTimeout(r, Math.max(0, ms)))

export const runOvernight = async (input: {
  readonly options: OvernightCliOptions
  readonly env: OvernightEnv
  readonly fetchFn: FetchLike
  readonly runCommand: RunCommand
  readonly nowMs?: (() => number) | undefined
  readonly readTextFile?: ((path: string) => Promise<string>) | undefined
  readonly sleep?: ((ms: number) => Promise<void>) | undefined
}): Promise<OvernightRunSummary> => {
  const nowMs = input.nowMs ?? (() => Date.now())
  const doSleep = input.sleep ?? sleepMs
  const startedAt = nowMs()
  const errors: string[] = []

  if (!input.env.OA_DSE_ADMIN_SECRET || input.env.OA_DSE_ADMIN_SECRET.length === 0) {
    throw new Error("missing_OA_DSE_ADMIN_SECRET")
  }

  const ops = makeOpsClient({
    baseUrl: input.options.baseUrl,
    adminSecret: input.env.OA_DSE_ADMIN_SECRET,
    fetchFn: input.fetchFn,
  })

  const dse = makeDseAdminClient({
    baseUrl: input.options.baseUrl,
    adminSecret: input.env.OA_DSE_ADMIN_SECRET,
    fetchFn: input.fetchFn,
  })

  const commitSha = await tryGetCommitSha(input.runCommand)

  let verifyOk = true
  let e2eOk = true
  let status: "finished" | "failed" = "finished"

  let verifyDuration: number | undefined
  let e2eDuration: number | undefined

  let phase5Summary: any | undefined

  const runIdRes = await ops.startRun({
    commitSha,
    baseUrl: input.options.baseUrl,
    signatureIds: ["@openagents/autopilot/blueprint/SelectTool.v1"],
  })
  const runId = runIdRes.runId

  const emit = async (e: {
    readonly level: OpsEventLevel
    readonly phase: string
    readonly message: string
    readonly json?: unknown
  }): Promise<void> => {
    try {
      await ops.event({ runId, level: e.level, phase: e.phase, message: e.message, json: e.json })
    } catch (err) {
      // Best-effort: keep progressing even if Convex writes fail; we still must finish.
      errors.push(`event_emit_failed phase=${e.phase} err=${String(err)}`)
    }
  }

  const finish = async (status: "finished" | "failed", summaryJson: unknown): Promise<void> => {
    await emit({
      level: status === "failed" ? "error" : "info",
      phase: "phase2.finish",
      message: `overnight script finishing status=${status}`,
      json: summaryJson,
    })
    try {
      await ops.finishRun({ runId, status, summaryJson })
    } catch (err) {
      // Nothing else we can do; surface in summary.
      errors.push(`finish_failed err=${String(err)}`)
    }
  }

  try {
    // Preflight: secrets.
    if (input.options.runE2e && !input.env.EFFUSE_TEST_E2E_BYPASS_SECRET) {
      throw new Error("missing_EFFUSE_TEST_E2E_BYPASS_SECRET")
    }

    await emit({
      level: "info",
      phase: "phase2.start",
      message: "overnight script started",
      json: {
        baseUrl: input.options.baseUrl,
        commitSha,
        verify: input.options.verify,
        runE2e: input.options.runE2e,
        e2eGrep: input.options.e2eGrep,
      },
    })

    if (input.options.verify) {
      const v0 = nowMs()
      await emit({ level: "info", phase: "phase2.verify.start", message: "local verification starting" })

      const steps: Array<{ readonly cwd: string; readonly command: string; readonly args: string[]; readonly timeoutMs: number }> =
        [
          { cwd: "packages/dse", command: "bun", args: ["test"], timeoutMs: 5 * 60_000 },
          { cwd: "packages/dse", command: "bun", args: ["run", "typecheck"], timeoutMs: 2 * 60_000 },
          { cwd: "apps/web", command: "npm", args: ["run", "lint"], timeoutMs: 5 * 60_000 },
          { cwd: "apps/web", command: "npm", args: ["test"], timeoutMs: 5 * 60_000 },
        ]

      for (const s of steps) {
        await emit({
          level: "info",
          phase: "phase2.verify.step",
          message: `run ${s.command} ${s.args.join(" ")} (cwd=${s.cwd})`,
        })
        const res = await input.runCommand({
          cwd: s.cwd,
          command: s.command,
          args: s.args,
          timeoutMs: s.timeoutMs,
        })
        await emit({
          level: res.ok ? "info" : "error",
          phase: "phase2.verify.result",
          message: `${res.ok ? "ok" : "failed"} ${s.command} ${s.args.join(" ")} (cwd=${s.cwd})`,
          json: {
            ok: res.ok,
            code: res.code,
            timedOut: res.timedOut,
            durationMs: res.durationMs,
            stdout: truncate(res.stdout, 8_000),
            stderr: truncate(res.stderr, 8_000),
          },
        })
        if (!res.ok) {
          verifyOk = false
          throw new Error(`verify_failed cmd=${s.command}`)
        }
      }

      verifyDuration = nowMs() - v0
      await emit({
        level: "info",
        phase: "phase2.verify.finish",
        message: "local verification complete",
        json: { durationMs: verifyDuration },
      })
    }

    // Phase 5: compile -> canary -> traffic -> monitor -> promote/stop (safe-by-default).
    const PHASE5_SIGNATURE_ID = "@openagents/autopilot/blueprint/SelectTool.v1"
    const DATASET_FIXTURE_PATH = "docs/autopilot/fixtures/dse-selecttool.dataset.v1.jsonl"

    const phase5: any = {
      signatureId: PHASE5_SIGNATURE_ID,
      datasetFixturePath: DATASET_FIXTURE_PATH,
    }

    let canaryStarted: { signatureId: string } | null = null
    let canaryStopAttempted = false
    let canarySalt: string | null = null
    const canaryRolloutPct = 20
    const canaryMinSamples = 20
    const canaryMaxErrorRate = 0.2
    let exerciseThreadId: string | null = null
    let compiledId: string | null = null
    let compileJobHash: string | null = null
    let compileDatasetHash: string | null = null

    const cleanupCanary = async (reason: string): Promise<void> => {
      if (!canaryStarted) return
      if (canaryStopAttempted) return
      canaryStopAttempted = true
      try {
        await emit({ level: "warn", phase: "phase5.canary.stop.start", message: "stopping canary", json: { reason } })
        const res = await dse.canaryStop({ signatureId: canaryStarted.signatureId, reason })
        await emit({
          level: "info",
          phase: "phase5.canary.stop.result",
          message: "canary stop ok",
          json: { requestId: res.requestId, result: res.result },
        })
      } catch (err) {
        errors.push(`canary_stop_failed err=${String(err)}`)
        await emit({
          level: "error",
          phase: "phase5.canary.stop.error",
          message: "canary stop failed",
          json: { error: String(err) },
        })
      }
    }

    try {
      await emit({ level: "info", phase: "phase5.start", message: "phase 5 starting" })

      const readText = input.readTextFile ?? (async (path: string) => (await import("node:fs/promises")).readFile(path, "utf8"))
      const datasetJsonl = await readText(DATASET_FIXTURE_PATH)

      await emit({ level: "info", phase: "phase5.dataset.import.start", message: "importing dataset fixture" })
      const importRes = await dse.importExamples({
        signatureId: PHASE5_SIGNATURE_ID,
        jsonl: datasetJsonl,
        opsRunId: runId,
        source: "fixture:dse-selecttool.dataset.v1",
        tagsAppend: ["overnight:phase5"],
      })
      phase5.datasetImport = { requestId: importRes.requestId, result: importRes.result }
      await emit({
        level: "info",
        phase: "phase5.dataset.import.result",
        message: "dataset import ok",
        json: { requestId: importRes.requestId, result: importRes.result },
      })

      await emit({ level: "info", phase: "phase5.compile.start", message: "compile starting" })
      const compileRes = await dse.compile({ signatureId: PHASE5_SIGNATURE_ID })
      phase5.compile = { requestId: compileRes.requestId, result: compileRes.result }
      await emit({
        level: "info",
        phase: "phase5.compile.result",
        message: "compile ok",
        json: { requestId: compileRes.requestId, result: compileRes.result },
      })

      compiledId = typeof compileRes.result?.compiled_id === "string" ? compileRes.result.compiled_id : null
      compileJobHash = typeof compileRes.result?.jobHash === "string" ? compileRes.result.jobHash : null
      compileDatasetHash = typeof compileRes.result?.datasetHash === "string" ? compileRes.result.datasetHash : null
      if (!compiledId || !compileJobHash || !compileDatasetHash) throw new Error("invalid_compile_response")

      const threadRes = await dse.ensureExerciseThread()
      exerciseThreadId = typeof threadRes.result?.threadId === "string" ? threadRes.result.threadId : null
      phase5.exerciseThread = { requestId: threadRes.requestId, result: threadRes.result }
      await emit({
        level: "info",
        phase: "phase5.exercise.thread",
        message: "exercise thread ensured",
        json: { requestId: threadRes.requestId, result: threadRes.result },
      })
      if (!exerciseThreadId) throw new Error("missing_exercise_thread")

      // Pick a canary salt that guarantees this thread falls into the canary bucket.
      const maxSaltAttempts = 500
      let found: { salt: string; bucket: number; attempts: number } | null = null
      for (let i = 0; i < maxSaltAttempts; i++) {
        const salt = crypto.randomUUID()
        const bucket = stableBucket100(`${salt}:${exerciseThreadId}:${PHASE5_SIGNATURE_ID}`)
        if (bucket < canaryRolloutPct) {
          found = { salt, bucket, attempts: i + 1 }
          break
        }
      }
      if (!found) throw new Error("canary_salt_search_failed")
      canarySalt = found.salt
      phase5.canarySalt = found
      await emit({
        level: "info",
        phase: "phase5.canary.salt",
        message: "selected canary salt for deterministic canary thread",
        json: found,
      })

      await emit({ level: "info", phase: "phase5.canary.start.start", message: "starting canary" })
      const canaryStartRes = await dse.canaryStart({
        signatureId: PHASE5_SIGNATURE_ID,
        canary_compiled_id: compiledId,
        rolloutPct: canaryRolloutPct,
        salt: canarySalt,
        minHoldoutDelta: 0,
        requireHoldout: true,
        minSamples: canaryMinSamples,
        maxErrorRate: canaryMaxErrorRate,
        reason: `opsRunId=${runId}`,
      })
      canaryStarted = { signatureId: PHASE5_SIGNATURE_ID }
      phase5.canaryStart = { requestId: canaryStartRes.requestId, result: canaryStartRes.result }
      await emit({
        level: "info",
        phase: "phase5.canary.start.result",
        message: "canary started",
        json: { requestId: canaryStartRes.requestId, result: canaryStartRes.result },
      })

      await emit({ level: "info", phase: "phase5.exercise.start", message: "generating canary traffic (exercise)" })
      const exerciseRes = await dse.exercisePredict({
        signatureId: PHASE5_SIGNATURE_ID,
        threadId: exerciseThreadId,
        count: canaryMinSamples,
        split: "train",
        limit: 200,
      })
      phase5.exercise = { requestId: exerciseRes.requestId, result: exerciseRes.result }
      await emit({
        level: "info",
        phase: "phase5.exercise.result",
        message: "exercise finished",
        json: { requestId: exerciseRes.requestId, result: exerciseRes.result },
      })

      // Monitor canary counters until minSamples or auto-stop or timeout.
      const pollStartedAt = nowMs()
      const pollTimeoutMs = 3 * 60_000
      const pollIntervalMs = 2_000
      let lastStatus: any = null
      while (true) {
        const statusRes = await dse.canaryStatus({ signatureId: PHASE5_SIGNATURE_ID })
        lastStatus = { requestId: statusRes.requestId, result: statusRes.result }
        const canary = statusRes.result?.canary ?? null
        const okCount = typeof canary?.okCount === "number" ? canary.okCount : 0
        const errorCount = typeof canary?.errorCount === "number" ? canary.errorCount : 0
        const minSamples = typeof canary?.minSamples === "number" ? canary.minSamples : canaryMinSamples
        const maxErrorRate = typeof canary?.maxErrorRate === "number" ? canary.maxErrorRate : canaryMaxErrorRate
        const total = okCount + errorCount

        await emit({
          level: "info",
          phase: "phase5.canary.poll",
          message: "canary status",
          json: {
            requestId: statusRes.requestId,
            present: Boolean(canary),
            okCount,
            errorCount,
            total,
            minSamples,
            maxErrorRate,
          },
        })

        if (!canary) break
        if (total >= minSamples) break
        if (nowMs() - pollStartedAt > pollTimeoutMs) throw new Error("canary_poll_timeout")
        await doSleep(pollIntervalMs)
      }

      phase5.canaryFinal = lastStatus
      const canaryFinal = lastStatus?.result?.canary ?? null
      const okCount = typeof canaryFinal?.okCount === "number" ? canaryFinal.okCount : 0
      const errorCount = typeof canaryFinal?.errorCount === "number" ? canaryFinal.errorCount : 0
      const minSamples = typeof canaryFinal?.minSamples === "number" ? canaryFinal.minSamples : canaryMinSamples
      const maxErrorRate = typeof canaryFinal?.maxErrorRate === "number" ? canaryFinal.maxErrorRate : canaryMaxErrorRate
      const total = okCount + errorCount
      const errorRate = total === 0 ? 0 : errorCount / total

      const canaryClean = Boolean(canaryFinal) && total >= minSamples && errorRate <= maxErrorRate
      phase5.canaryHealth = { okCount, errorCount, total, minSamples, maxErrorRate, errorRate, canaryClean }
      await emit({
        level: canaryClean ? "info" : "warn",
        phase: "phase5.canary.health",
        message: canaryClean ? "canary clean" : "canary not clean",
        json: phase5.canaryHealth,
      })

      if (!canaryClean) {
        await cleanupCanary(`canary_not_clean total=${total} errorRate=${errorRate.toFixed(3)}`)
        throw new Error("canary_not_clean")
      }

      if (input.options.runE2e) {
        const e0 = nowMs()
        await emit({ level: "info", phase: "phase5.e2e.start", message: "prod e2e starting" })

        const res = await input.runCommand({
          cwd: "apps/web",
          command: "npm",
          args: [
            "run",
            "test:e2e",
            "--",
            "--base-url",
            input.options.baseUrl,
            "--tag",
            "prod",
            "--grep",
            "apps-web\\\\.prod\\\\.autopilot",
          ],
          env: {
            EFFUSE_TEST_E2E_BYPASS_SECRET: input.env.EFFUSE_TEST_E2E_BYPASS_SECRET,
          },
          timeoutMs: 15 * 60_000,
        })

        e2eDuration = nowMs() - e0

        phase5.e2e = {
          ok: res.ok,
          code: res.code,
          timedOut: res.timedOut,
          durationMs: res.durationMs,
          stdout: truncate(res.stdout, 12_000),
          stderr: truncate(res.stderr, 12_000),
        }

        await emit({
          level: res.ok ? "info" : "error",
          phase: "phase5.e2e.result",
          message: `${res.ok ? "ok" : "failed"} prod e2e`,
          json: phase5.e2e,
        })

        if (!res.ok) {
          e2eOk = false
          await cleanupCanary("e2e_failed")
          throw new Error("e2e_failed")
        }
      }

      await emit({ level: "info", phase: "phase5.promote.start", message: "promoting canary artifact" })
      const promoteRes = await dse.promote({
        signatureId: PHASE5_SIGNATURE_ID,
        compiled_id: compiledId,
        minHoldoutDelta: 0,
        requireHoldout: true,
      })
      phase5.promote = { requestId: promoteRes.requestId, result: promoteRes.result }
      await emit({
        level: "info",
        phase: "phase5.promote.result",
        message: "promote ok",
        json: { requestId: promoteRes.requestId, result: promoteRes.result },
      })

      // Promote clears canary best-effort; ensure it's stopped for safety.
      await cleanupCanary("promoted")

      phase5.links = {
        signatureId: PHASE5_SIGNATURE_ID,
        compileReportKey: { signatureId: PHASE5_SIGNATURE_ID, jobHash: compileJobHash, datasetHash: compileDatasetHash },
        compiled_id: compiledId,
        exerciseThreadId,
      }
      await emit({ level: "info", phase: "phase5.finish", message: "phase 5 complete", json: phase5.links })
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err)
      phase5.error = msg
      await emit({ level: "error", phase: "phase5.error", message: "phase 5 failed", json: { error: msg } })
      await cleanupCanary(`phase5_failed err=${msg}`)
      throw err
    } finally {
      phase5Summary = phase5
    }
  } catch (err) {
    status = "failed"
    const msg = err instanceof Error ? err.message : String(err)
    errors.push(msg)
    await emit({
      level: "error",
      phase: "phase2.error",
      message: "overnight script failed",
      json: { error: msg },
    })
  } finally {
    const summary: OvernightRunSummary = {
      ok: status === "finished",
      runId,
      baseUrl: input.options.baseUrl,
      verify: { ran: input.options.verify, ok: input.options.verify ? verifyOk : true },
      e2e: { ran: input.options.runE2e, ok: input.options.runE2e ? e2eOk : true },
      ...(phase5Summary ? { phase5: phase5Summary } : {}),
      durationsMs: {
        total: nowMs() - startedAt,
        ...(verifyDuration !== undefined ? { verify: verifyDuration } : {}),
        ...(e2eDuration !== undefined ? { e2e: e2eDuration } : {}),
      },
      errors,
    }

    await finish(status, summary)
  }

  return {
    ok: status === "finished",
    runId,
    baseUrl: input.options.baseUrl,
    verify: { ran: input.options.verify, ok: input.options.verify ? verifyOk : true },
    e2e: { ran: input.options.runE2e, ok: input.options.runE2e ? e2eOk : true },
    durationsMs: {
      total: nowMs() - startedAt,
      ...(verifyDuration !== undefined ? { verify: verifyDuration } : {}),
      ...(e2eDuration !== undefined ? { e2e: e2eDuration } : {}),
    },
    errors,
  }
}

export const validateEnv = (env: Record<string, string | undefined>): OvernightEnv | null => {
  const adminSecret = env.OA_DSE_ADMIN_SECRET
  if (typeof adminSecret !== "string" || adminSecret.length === 0) return null
  return {
    OA_DSE_ADMIN_SECRET: adminSecret,
    EFFUSE_TEST_E2E_BYPASS_SECRET: env.EFFUSE_TEST_E2E_BYPASS_SECRET,
  }
}

export const usage = (): string => USAGE
