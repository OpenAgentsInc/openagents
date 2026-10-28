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
