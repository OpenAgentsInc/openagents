// Fractional rank keys: the queue's order without numbers.
//
// A rank is a string of base-62 digits ("0"-"9", "A"-"Z", "a"-"z": ASCII
// order, so comparing bytes compares ranks) read as a fraction after a
// radix point, and never ending in "0". Between any two ranks there is
// another, so moving a task rewrites only its own rank: nothing else is
// renumbered. Repeated inserts at one spot lengthen keys slowly (about one
// digit per 6 inserts); `rebalance` spreads a whole list evenly when a key
// passes LONG digits. (David Greenspan's "Implementing Fractional Indexing".)

export const DIGITS = "0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz"
const BASE = DIGITS.length
export const LONG = 10

const digit = (c: string) => {
  const d = DIGITS.indexOf(c)
  if (d < 0) throw new Error(`not a rank digit: ${JSON.stringify(c)}`)
  return d
}

export function valid(rank: string): boolean {
  if (rank.length === 0 || rank.endsWith("0")) return false
  for (const c of rank) if (DIGITS.indexOf(c) < 0) return false
  return true
}

/** A rank strictly between a and b; a = "" is the start, b = null the end. */
export function between(a: string, b: string | null): string {
  if (a !== "" && !valid(a)) throw new Error(`bad rank ${a}`)
  if (b !== null && !valid(b)) throw new Error(`bad rank ${b}`)
  if (b !== null && a >= b) throw new Error(`ranks out of order: ${a} >= ${b}`)
  return midpoint(a, b)
}

function midpoint(a: string, b: string | null): string {
  if (b !== null) {
    // the common prefix, reading a as padded with zeros
    let n = 0
    while ((a[n] ?? "0") === b[n]) n++
    if (n > 0) return b.slice(0, n) + midpoint(a.slice(n), b.slice(n))
  }
  const da = a ? digit(a[0]) : 0
  const db = b !== null ? digit(b[0]) : BASE
  if (db - da > 1) return DIGITS[Math.round((da + db) / 2)]
  // adjacent digits: b's first digit alone is between, if b goes on
  if (b !== null && b.length > 1) return b.slice(0, 1)
  return DIGITS[da] + midpoint(a.slice(1), null)
}

/** n evenly spread ranks, as short as they can be. */
export function spread(n: number): string[] {
  let width = 1
  while (BASE ** width < n + 1) width++
  if ((BASE ** width) / (n + 1) < 4 && width < 8) width++   // leave room between neighbours
  const out: string[] = []
  for (let i = 1; i <= n; i++) {
    let v = Math.floor((i * BASE ** width) / (n + 1))
    let s = ""
    for (let k = 0; k < width; k++) { s = DIGITS[v % BASE] + s; v = Math.floor(v / BASE) }
    out.push(s.replace(/0+$/, ""))
  }
  return out
}

/** k ranks evenly between a and b (exclusive), by halving: as short as bisection allows. */
export function spreadBetween(a: string, b: string | null, k: number): string[] {
  if (k <= 0) return []
  const mid = between(a, b)
  const left = Math.floor((k - 1) / 2)
  return [...spreadBetween(a, mid, left), mid, ...spreadBetween(mid, b, k - 1 - left)]
}

/** The ranks in order, respread if any has grown past LONG digits. */
export function rebalance(ranks: string[]): { ranks: string[]; changed: boolean } {
  if (!ranks.some((r) => r.length > LONG)) return { ranks, changed: false }
  return { ranks: spread(ranks.length), changed: true }
}

/** The rank for an item moved to sit just before or after a neighbour in a sorted list. */
export function placed(sorted: string[], neighbour: number, where: "before" | "after"): string {
  if (where === "before") return between(neighbour > 0 ? sorted[neighbour - 1] : "", sorted[neighbour])
  return between(sorted[neighbour], neighbour + 1 < sorted.length ? sorted[neighbour + 1] : null)
}

/** The relay's tag limit per event (RS_MAX_TAGS in coax/lib/nostr/relay_server.coax). */
export const RELAY_MAX_TAGS = 256
/** Tags an order event carries besides its items: d, a and alt. */
export const ORDER_FIXED_TAGS = 3

type Orderable = { id: string; slug: string; rank: string; status: string }
const done = (s: string) => s === "resolved" || s === "closed"

/** The order the queue keeps: open items only (a finished item has nothing left to order, and
 * leaving it out keeps the order event under the relay's tag limit), ranked items by rank, then
 * unranked items in work-file order after them. A reopened item comes back unranked, at the end. */
export function queueOrder(issues: Orderable[], workSlugs: string[]): { id: string; rank: string }[] {
  const open = issues.filter((i) => !done(i.status))
  const ranked = open.filter((i) => i.rank).sort((a, b) => (a.rank < b.rank ? -1 : 1)).map((i) => ({ id: i.id, rank: i.rank }))
  const bySlug = new Map(open.map((i) => [i.slug, i]))
  const last = ranked.length ? ranked[ranked.length - 1].rank : ""
  const fresh = workSlugs.map((s) => bySlug.get(s)).filter((i): i is Orderable => !!i && !i.rank)
  const keys = spreadBetween(last, null, fresh.length)
  fresh.forEach((i, k) => ranked.push({ id: i.id, rank: keys[k] }))
  const r = rebalance(ranked.map((e) => e.rank))
  if (r.changed) ranked.forEach((e, i) => (e.rank = r.ranks[i]))
  return ranked
}

/** Refuses an order the relay would refuse, before signing it. */
export function checkOrderSize(n: number) {
  if (n + ORDER_FIXED_TAGS > RELAY_MAX_TAGS)
    throw new Error(`${n} open items: an order event holds at most ${RELAY_MAX_TAGS - ORDER_FIXED_TAGS} (relay tag limit); close or split the project`)
}
