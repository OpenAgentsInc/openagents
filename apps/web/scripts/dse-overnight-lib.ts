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
): Promise<{ readonly response: Response; readonly json: any }> => {
  const controller = new AbortController()
  const t = setTimeout(() => controller.abort(), Math.max(1, input.timeoutMs))
  try {
    const response = await fetchFn(input.url, { ...input.init, signal: controller.signal })
    const json = await response.json().catch(() => null)
    return { response, json }
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
  readonly durationsMs: {
    readonly total: number
    readonly verify?: number | undefined
    readonly e2e?: number | undefined
  }
  readonly errors: ReadonlyArray<string>
}

export const runOvernight = async (input: {
  readonly options: OvernightCliOptions
  readonly env: OvernightEnv
  readonly fetchFn: FetchLike
  readonly runCommand: RunCommand
  readonly nowMs?: (() => number) | undefined
}): Promise<OvernightRunSummary> => {
  const nowMs = input.nowMs ?? (() => Date.now())
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

  const commitSha = await tryGetCommitSha(input.runCommand)

  let verifyOk = true
  let e2eOk = true
  let status: "finished" | "failed" = "finished"

  let verifyDuration: number | undefined
  let e2eDuration: number | undefined

  const runIdRes = await ops.startRun({
    commitSha,
    baseUrl: input.options.baseUrl,
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

    if (input.options.runE2e) {
      const e0 = nowMs()
      await emit({ level: "info", phase: "phase2.e2e.start", message: "prod e2e smoke starting" })

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
          input.options.e2eGrep,
        ],
        env: {
          EFFUSE_TEST_E2E_BYPASS_SECRET: input.env.EFFUSE_TEST_E2E_BYPASS_SECRET,
        },
        timeoutMs: 10 * 60_000,
      })

      e2eDuration = nowMs() - e0

      await emit({
        level: res.ok ? "info" : "error",
        phase: "phase2.e2e.result",
        message: `${res.ok ? "ok" : "failed"} prod e2e smoke`,
        json: {
          ok: res.ok,
          code: res.code,
          timedOut: res.timedOut,
          durationMs: res.durationMs,
          grep: input.options.e2eGrep,
          stdout: truncate(res.stdout, 12_000),
          stderr: truncate(res.stderr, 12_000),
        },
      })

      if (!res.ok) {
        e2eOk = false
        throw new Error("e2e_failed")
      }
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
