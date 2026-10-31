import { expect, test } from "bun:test"
import { between, checkOrderSize, LONG, ORDER_FIXED_TAGS, placed, queueOrder, RELAY_MAX_TAGS, rebalance, spread, spreadBetween, valid } from "./rank"

test("a rank lies strictly between its neighbours, at the ends too", () => {
  for (const [a, b] of [["", null], ["", "V"], ["V", null], ["1", "2"], ["1", "11"], ["1z", "2"], ["az", "b"], ["0001", "0002"], ["y", "z"], ["z", null], ["", "01"]] as const) {
    const m = between(a, b)
    expect(valid(m)).toBe(true)
    expect(m > a).toBe(true)
    if (b !== null) expect(m < b).toBe(true)
  }
  expect(() => between("2", "1")).toThrow()
  expect(() => between("10", null)).toThrow()   // never ends in 0
})

test("spread gives sorted, distinct, short ranks", () => {
  for (const n of [1, 2, 5, 61, 62, 200, 5000]) {
    const r = spread(n)
    expect(r.length).toBe(n)
    for (let i = 0; i < n; i++) {
      expect(valid(r[i])).toBe(true)
      if (i > 0) expect(r[i] > r[i - 1]).toBe(true)
    }
    expect(Math.max(...r.map((s) => s.length))).toBeLessThanOrEqual(4)
  }
})

test("inserting at the same spot grows keys slowly; rebalance resets them", () => {
  let lo = "", hi: string | null = "1"
  for (let i = 0; i < 40; i++) hi = between(lo, hi)
  expect(hi!.length).toBeLessThanOrEqual(9)
  const r = rebalance(["1", "2", "0000000000001"])
  expect(r.changed).toBe(true)
  expect(rebalance(spread(10)).changed).toBe(false)
})

test("a thousand random moves keep the list sorted and its keys short", () => {
  let seed = 7
  const rand = (n: number) => { seed = (seed * 1103515245 + 12345) % 2147483648; return seed % n }
  let items = spread(40).map((rank, i) => ({ id: i, rank }))
  let rebalances = 0
  for (let k = 0; k < 1000; k++) {
    const sorted = [...items].sort((x, y) => (x.rank < y.rank ? -1 : 1))
    const from = rand(sorted.length)
    const moving = sorted[from]
    const rest = sorted.filter((_, i) => i !== from)
    const to = rand(rest.length)
    moving.rank = placed(rest.map((x) => x.rank), to, rand(2) ? "before" : "after")
    items = [...rest, moving]
    const ranks = [...items].sort((x, y) => (x.rank < y.rank ? -1 : 1)).map((x) => x.rank)
    expect(new Set(ranks).size).toBe(ranks.length)
    const r = rebalance(ranks)
    if (r.changed) {
      rebalances++
      const order = [...items].sort((x, y) => (x.rank < y.rank ? -1 : 1))
      order.forEach((x, i) => (x.rank = r.ranks[i]))
    }
    expect(Math.max(...items.map((x) => x.rank.length))).toBeLessThanOrEqual(LONG + 1)
  }
  expect(rebalances).toBeLessThan(50)
})

test("a batch appended at the end stays short", () => {
  const r = spreadBetween("V", null, 60)
  expect(r.length).toBe(60)
  for (let i = 1; i < r.length; i++) expect(r[i] > r[i - 1]).toBe(true)
  expect(r[0] > "V").toBe(true)
  expect(Math.max(...r.map((s) => s.length))).toBeLessThanOrEqual(3)
  expect(spreadBetween("1", "2", 0)).toEqual([])
})

test("the order leaves finished items out and fits the relay's tag limit", () => {
  // ~300 items, most of them done: only the open ones are ordered
  const issues = Array.from({ length: 300 }, (_, i) => ({
    id: `id${i}`, slug: `s${i}`, rank: i < 260 ? spread(260)[i] : "", status: i % 5 === 0 || i >= 260 ? "open" : i % 2 ? "resolved" : "closed" }))
  const order = queueOrder(issues, issues.map((i) => i.slug))
  const open = issues.filter((i) => i.status === "open")
  expect(order.map((e) => e.id).sort()).toEqual(open.map((i) => i.id).sort())
  expect(order.length + ORDER_FIXED_TAGS).toBeLessThanOrEqual(RELAY_MAX_TAGS)
  for (let i = 1; i < order.length; i++) expect(order[i].rank > order[i - 1].rank).toBe(true)
  // the unranked new ones come last, in work-file order
  expect(order.slice(-40).map((e) => e.id)).toEqual(issues.slice(260).map((i) => i.id))
  // an order the relay would refuse is refused before signing
  expect(() => checkOrderSize(RELAY_MAX_TAGS - ORDER_FIXED_TAGS)).not.toThrow()
  expect(() => checkOrderSize(RELAY_MAX_TAGS - ORDER_FIXED_TAGS + 1)).toThrow(/tag limit/)
})
