/**
 * Scheduler — owned cron-dispatch contract (CFG-2, issue #8517, audit §5).
 *
 * The contract is deliberately vendor-free: a PURE dispatch table (name +
 * cron expression + Effect to run) and an HTTP handler factory. ANY
 * minute-tick source — Cloud Scheduler, systemd timers, crontab + curl, a
 * test — POSTs to the handler (conventionally mounted at `/internal/cron`)
 * and the handler runs whichever tasks are due at that instant. Swapping
 * the tick source never touches app code.
 *
 * Task expectations:
 * - Task effects must be fully provided (`R = never`) — compose Layers into
 *   the table at app wiring time.
 * - Ticks arrive at minute resolution (UTC) and MAY be retried by the tick
 *   source on non-2xx, so tasks must be idempotent per minute.
 */
import { Effect, Exit } from "effect"
import { cronScheduleMatches, parseCron, type CronSchedule } from "./cron.ts"

export interface CronTask {
  /** Unique within the table. */
  readonly name: string
  /** 5-field cron expression, UTC (see src/cron.ts for the dialect). */
  readonly cron: string
  readonly run: () => Effect.Effect<unknown, unknown>
}

export type CronDispatchTable = ReadonlyArray<CronTask>

interface CompiledTask {
  readonly task: CronTask
  readonly schedule: CronSchedule
}

/**
 * Validate a dispatch table eagerly. Throws (defect) on invalid cron
 * expressions or duplicate names — a broken table is a config bug that
 * should fail deploy, not a runtime condition.
 */
export const compileDispatchTable = (table: CronDispatchTable): ReadonlyArray<CompiledTask> => {
  const seen = new Set<string>()
  return table.map((task) => {
    if (seen.has(task.name)) {
      throw new Error(`duplicate cron task name ${JSON.stringify(task.name)}`)
    }
    seen.add(task.name)
    return { task, schedule: parseCron(task.cron) }
  })
}

/** Pure: which tasks fire at this instant (minute resolution, UTC)? */
export const dueTasks = (table: CronDispatchTable, at: Date): ReadonlyArray<CronTask> =>
  compileDispatchTable(table)
    .filter(({ schedule }) => cronScheduleMatches(schedule, at))
    .map(({ task }) => task)

export interface CronTaskResult {
  readonly name: string
  readonly ok: boolean
  readonly error?: string
}

export interface CronTickReport {
  readonly at: string
  readonly due: ReadonlyArray<string>
  readonly results: ReadonlyArray<CronTaskResult>
}

export interface CronHandlerOptions {
  readonly table: CronDispatchTable
  /** Pathname the handler answers on. Default "/internal/cron". */
  readonly path?: string
  /**
   * Optional shared secret; when set, requests must carry it in the
   * `x-oa-cron-token` header. (Prefer platform auth — e.g. OIDC on the tick
   * source — where available; this is defense in depth.)
   */
  readonly authToken?: string
  /** Clock override for tests. Default `() => new Date()`. */
  readonly now?: () => Date
}

export const CRON_DEFAULT_PATH = "/internal/cron"
export const CRON_AUTH_HEADER = "x-oa-cron-token"

const json = (status: number, body: unknown): Response =>
  new Response(JSON.stringify(body), {
    status,
    headers: { "content-type": "application/json" },
  })

/**
 * Build a fetch-shaped handler (`Request -> Promise<Response>`) that runs
 * the due tasks on each POST tick. Mount it in any HTTP server (Bun.serve,
 * Cloud Run service, test harness).
 *
 * Responses: 404 wrong path, 405 non-POST, 401 bad/missing token, 200 all
 * due tasks succeeded, 500 at least one failed (so retrying tick sources
 * re-fire; tasks are idempotent per minute by contract).
 */
export const makeCronHandler = (
  options: CronHandlerOptions,
): ((request: Request) => Promise<Response>) => {
  const compiled = compileDispatchTable(options.table)
  const path = options.path ?? CRON_DEFAULT_PATH
  const now = options.now ?? (() => new Date())

  return async (request: Request): Promise<Response> => {
    const url = new URL(request.url)
    if (url.pathname !== path) return json(404, { error: "not found" })
    if (request.method !== "POST") return json(405, { error: "method not allowed" })
    if (
      options.authToken !== undefined &&
      request.headers.get(CRON_AUTH_HEADER) !== options.authToken
    ) {
      return json(401, { error: "unauthorized" })
    }

    const at = now()
    const due = compiled.filter(({ schedule }) => cronScheduleMatches(schedule, at))
    const results: Array<CronTaskResult> = []
    for (const { task } of due) {
      const exit = await Effect.runPromiseExit(Effect.suspend(() => task.run()))
      results.push(
        Exit.isSuccess(exit)
          ? { name: task.name, ok: true }
          : { name: task.name, ok: false, error: String(exit.cause) },
      )
    }

    const report: CronTickReport = {
      at: at.toISOString(),
      due: due.map(({ task }) => task.name),
      results,
    }
    const failed = results.some((result) => !result.ok)
    return json(failed ? 500 : 200, report)
  }
}
