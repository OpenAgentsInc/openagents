import { describe, expect, test } from "bun:test"
import { Effect } from "effect"
import {
  CRON_AUTH_HEADER,
  compileDispatchTable,
  dueTasks,
  makeCronHandler,
  type CronDispatchTable,
  type CronTickReport,
} from "./scheduler.ts"

const utc = (y: number, m: number, d: number, hh: number, mm: number) =>
  new Date(Date.UTC(y, m - 1, d, hh, mm))

const tick = (
  handler: (request: Request) => Promise<Response>,
  init?: RequestInit & { path?: string },
) =>
  handler(
    new Request(`http://internal${init?.path ?? "/internal/cron"}`, {
      method: "POST",
      ...init,
    }),
  )

describe("dispatch table", () => {
  test("compileDispatchTable rejects duplicate names and bad cron", () => {
    expect(() =>
      compileDispatchTable([
        { name: "a", cron: "* * * * *", run: () => Effect.void },
        { name: "a", cron: "* * * * *", run: () => Effect.void },
      ]),
    ).toThrow(/duplicate/)
    expect(() =>
      compileDispatchTable([{ name: "bad", cron: "not a cron", run: () => Effect.void }]),
    ).toThrow(/invalid cron/)
  })

  test("dueTasks is pure and picks only matching tasks", () => {
    const table: CronDispatchTable = [
      { name: "every-minute", cron: "* * * * *", run: () => Effect.void },
      { name: "hourly", cron: "0 * * * *", run: () => Effect.void },
      { name: "nightly", cron: "30 2 * * *", run: () => Effect.void },
    ]
    expect(dueTasks(table, utc(2026, 7, 6, 2, 30)).map((t) => t.name)).toEqual([
      "every-minute",
      "nightly",
    ])
    expect(dueTasks(table, utc(2026, 7, 6, 3, 0)).map((t) => t.name)).toEqual([
      "every-minute",
      "hourly",
    ])
  })
})

describe("makeCronHandler", () => {
  test("runs due tasks and reports per-task results", async () => {
    const ran: Array<string> = []
    const handler = makeCronHandler({
      table: [
        {
          name: "due",
          cron: "15 10 * * *",
          run: () => Effect.sync(() => void ran.push("due")),
        },
        {
          name: "not-due",
          cron: "0 0 1 1 *",
          run: () => Effect.sync(() => void ran.push("not-due")),
        },
      ],
      now: () => utc(2026, 7, 6, 10, 15),
    })
    const response = await tick(handler)
    expect(response.status).toBe(200)
    const report = (await response.json()) as CronTickReport
    expect(report.at).toBe("2026-07-06T10:15:00.000Z")
    expect(report.due).toEqual(["due"])
    expect(report.results).toEqual([{ name: "due", ok: true }])
    expect(ran).toEqual(["due"])
  })

  test("a failing task yields 500 with the error reported; others still run", async () => {
    const ran: Array<string> = []
    const handler = makeCronHandler({
      table: [
        { name: "boom", cron: "* * * * *", run: () => Effect.fail("kaput" as const) },
        {
          name: "fine",
          cron: "* * * * *",
          run: () => Effect.sync(() => void ran.push("fine")),
        },
      ],
      now: () => utc(2026, 7, 6, 0, 0),
    })
    const response = await tick(handler)
    expect(response.status).toBe(500)
    const report = (await response.json()) as CronTickReport
    expect(report.results.length).toBe(2)
    expect(report.results[0]?.ok).toBe(false)
    expect(report.results[0]?.error).toContain("kaput")
    expect(report.results[1]).toEqual({ name: "fine", ok: true })
    expect(ran).toEqual(["fine"])
  })

  test("no due tasks is a clean 200", async () => {
    const handler = makeCronHandler({
      table: [{ name: "rare", cron: "0 0 29 2 *", run: () => Effect.void }],
      now: () => utc(2026, 7, 6, 12, 0),
    })
    const response = await tick(handler)
    expect(response.status).toBe(200)
    const report = (await response.json()) as CronTickReport
    expect(report.due).toEqual([])
  })

  test("wrong path 404, wrong method 405", async () => {
    const handler = makeCronHandler({
      table: [],
      now: () => utc(2026, 7, 6, 0, 0),
    })
    expect((await tick(handler, { path: "/elsewhere" })).status).toBe(404)
    expect(
      (await handler(new Request("http://internal/internal/cron", { method: "GET" }))).status,
    ).toBe(405)
  })

  test("authToken gates the tick", async () => {
    let ran = 0
    const handler = makeCronHandler({
      table: [
        { name: "guarded", cron: "* * * * *", run: () => Effect.sync(() => void ran++) },
      ],
      authToken: "sekrit",
      now: () => utc(2026, 7, 6, 0, 0),
    })
    expect((await tick(handler)).status).toBe(401)
    expect(
      (await tick(handler, { headers: { [CRON_AUTH_HEADER]: "wrong" } })).status,
    ).toBe(401)
    expect(ran).toBe(0)
    const ok = await tick(handler, { headers: { [CRON_AUTH_HEADER]: "sekrit" } })
    expect(ok.status).toBe(200)
    expect(ran).toBe(1)
  })

  test("custom path option", async () => {
    const handler = makeCronHandler({
      table: [],
      path: "/hooks/cron",
      now: () => utc(2026, 7, 6, 0, 0),
    })
    expect((await tick(handler, { path: "/hooks/cron" })).status).toBe(200)
    expect((await tick(handler)).status).toBe(404)
  })
})
