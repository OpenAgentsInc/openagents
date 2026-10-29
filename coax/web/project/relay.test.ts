import { expect, test } from "bun:test"
import { Relay } from "./relay"

function capped(rows: any[], cap: number) {
  const r = Object.create(Relay.prototype) as Relay
  const queries: any[] = []
  r.query = async ([filter]: any[]) => {
    queries.push(filter)
    return rows.filter(e => (filter.until === undefined || e.created_at <= filter.until) &&
      (!filter.kinds || filter.kinds.includes(e.kind))).slice(0, Math.min(cap, filter.limit))
  }
  return { r, queries }
}
const row = (at: number, kind = 1621, suffix = "") => ({ id: `${at}-${kind}-${suffix}`, created_at: at, kind })

test("a relay cap below requested limit does not hide older issues", async () => {
  const rows = Array.from({ length: 155 }, (_, i) => row(200 - i))
  const { r, queries } = capped(rows, 100)
  expect((await r.all([{ kinds: [1621] }], 200)).map(e => e.id)).toEqual(rows.map(e => e.id))
  expect(queries.length).toBeGreaterThan(1)
})
test("overlapping filters still scan their older pages", async () => {
  const rows = [row(9, 1), row(8, 1), row(7, 2), row(6, 2)]
  const { r } = capped(rows, 2)
  expect((await r.all([{ kinds: [1] }, { kinds: [1, 2] }])).length).toBe(4)
})
test("a stalled timestamp cohort fails rather than claiming exhaustion", async () => {
  const { r } = capped([row(9, 1, "a"), row(9, 1, "b"), row(9, 1, "c"), row(8, 1)], 2)
  await expect(r.all([{ kinds: [1] }])).rejects.toThrow("timestamp")
})
test("pagination budget refuses instead of returning a partial list", async () => {
  const { r } = capped(Array.from({ length: 400 }, (_, i) => row(500 - i)), 1)
  await expect(r.all([{}])).rejects.toThrow("pagination budget")
})

// A relay that misbehaves the way relay.openagents.com does under load: it
// refuses REQs and EVENTs with "rate-limited:", drops connections, and
// stays silent. The client must retry and never answer short or post twice.
function flaky(script: { req?: string[]; ok?: string[]; dropFirst?: boolean }) {
  const stored = new Map<string, any>()
  const stats = { reqs: 0, events: 0, conns: 0 }
  const server = Bun.serve({
    port: 0,
    fetch(req, s) { return s.upgrade(req) ? undefined : new Response("no", { status: 400 }) },
    websocket: {
      open() { stats.conns++ },
      message(ws, raw) {
        const [type, a, b] = JSON.parse(String(raw))
        if (type === "REQ") {
          stats.reqs++
          const act = script.req?.shift()
          if (act === "drop") return ws.close()
          if (act === "silent") return
          if (act) return ws.send(JSON.stringify(["CLOSED", a, act]))
          for (const e of stored.values()) if (!b.kinds || b.kinds.includes(e.kind)) ws.send(JSON.stringify(["EVENT", a, e]))
          ws.send(JSON.stringify(["EOSE", a, ["finish"]]))
        } else if (type === "EVENT") {
          stats.events++
          const act = script.ok?.shift()
          if (act === "drop") return ws.close()
          if (act === "silent") { stored.set(a.id, a); return }
          if (act) return ws.send(JSON.stringify(["OK", a.id, false, act]))
          const dup = stored.has(a.id)
          stored.set(a.id, a)
          ws.send(JSON.stringify(["OK", a.id, true, dup ? "duplicate: have it" : ""]))
        }
      },
    },
  })
  return { url: `ws://127.0.0.1:${server.port}`, stored, stats, stop: () => server.stop(true) }
}
const fast = { tries: 5, baseMs: 5, maxMs: 20, log: () => {} }
const ev = (id: string, kind = 1111) => ({ id, kind, created_at: 1, pubkey: "p", tags: [], content: "", sig: "s" }) as any

test("a rate-limited or dropped query is retried, not read as empty", async () => {
  const f = flaky({ req: ["rate-limited: REQ rate exceeded", "drop", "error: busy"] })
  f.stored.set("x", ev("x"))
  const r = await Relay.open(f.url, null)
  r.retry = fast
  expect((await r.all([{ kinds: [1111] }])).map((e) => e.id)).toEqual(["x"])
  expect(f.stats.reqs).toBe(4)
  r.close(); f.stop()
})
test("a refusal that won't lift fails at once with the relay's reason", async () => {
  const f = flaky({ req: ["restricted: too many filters"] })
  const r = await Relay.open(f.url, null)
  r.retry = fast
  await expect(r.query([{}])).rejects.toThrow("restricted: too many filters")
  expect(f.stats.reqs).toBe(1)
  r.close(); f.stop()
})
test("a publish retried after a timeout, a drop or a rate limit is stored once", async () => {
  const f = flaky({ ok: ["rate-limited: event rate exceeded", "silent", "drop"] })
  const r = await Relay.open(f.url, null, 200)
  r.retry = fast
  expect(await r.publish(ev("e1"))).toStartWith("duplicate:")
  expect(f.stats.events).toBe(4)
  expect([...f.stored.keys()]).toEqual(["e1"])
  r.close(); f.stop()
})
test("giving up says so plainly", async () => {
  const f = flaky({ req: Array(10).fill("rate-limited: REQ rate exceeded") })
  const r = await Relay.open(f.url, null)
  r.retry = fast
  await expect(r.query([{}])).rejects.toThrow(/gave up reading after 5 tries \(query refused: rate-limited/)
  r.close(); f.stop()
})
