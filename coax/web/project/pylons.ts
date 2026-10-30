// The Pylons on the /new-land Grid: one row per Pylon, shaped by its NIP-PYLON
// beacon (kind 30200, openagents.pylon-beacon.v1, the content that
// psionic/lib/pylon/beacon.coax writes and reads) plus what a reader counts
// from the Pylon's own events (26910 answers and 27010 status) on the relay.
//
// THE REAL FEED PLUGS IN HERE: landing.ts calls `pylons()`. PYLON_SOURCE is
// "relay" since 2026-10-30 (real Pylons publish beacons there; "demo" brings
// back DEMO_PYLONS, every row demo: true): `readPylons` reads the 30200
// beacons (verified, admitted as NIP-PYLON's Validation says, newest
// generation per pylon) and the 26910/27010 counts, and the page draws those
// instead. Nothing in newland.coax changes: it draws whatever LAND_PYLONS holds.
import { createHash } from "node:crypto"
import { reader, type PublicEvent as Event, type PublicRelay as Relay } from "./landing-reader"

export const PYLON_SOURCE: "demo" | "relay" = "relay"

export const PB_KIND = 30200
export const PB_MARKER = "oa:pylon-beacon:v1"
export const PB_VERSION = "openagents.pylon-beacon.v1"
export const KIND_ANSWER = 26910
export const KIND_STATUS = 27010

export type PylonStatus = "online" | "draining" | "offline"
export type Family = "unified-memory" | "gpu" | "cpu"

/** One Pylon as the page draws it. Beacon fields keep the beacon's names. */
export type Pylon = {
  demo: boolean
  generation: number
  valid_until: number
  provider: string          // the Pylon's key, hex
  pylon: string             // its d tag
  label: string
  status: PylonStatus
  family: Family
  tier: "small" | "medium" | "large" | "xl"
  memory_gb: number
  slots: { total: number; free: number }
  models: string[]          // services[].model, without the @sha256 digest
  lanes: string[]           // services[].lanes, each once
  settlement: string[]
  pools: string[]
  price_hint_msat: number | null
  since: number             // unix seconds: online since
  observed_at: number
  // counted from the Pylon's own events over the last 24 hours
  answers_24h: number
  statuses_24h: number        // 27010s: progress, refusal, failure or cancellation
  avg_reply_s: number         // -1: no verified timing; zero is a measured delay
  last_answer_at: number       // 0: none
  bay: string               // the project slug whose bay it stands in
}

// Which project's bay a Pylon stands in, by its first pool (an exact pool name,
// never matched on text); every Pylon runs Psionic's engine, so that is the default.
const POOL_BAY: Record<string, string> = { coder: "coder" }
export const bayOf = (pools: string[]) => POOL_BAY[pools[0] ?? ""] ?? "psionic"

// ---- the demo: made-up machines, each with a key made from its name

const demoKey = (pylon: string) => createHash("sha256").update(`new-land demo pylon ${pylon}`).digest("hex")
const DEMO_NOW = 1793318400   // the capture clock the demo ages are measured from
const ago = (s: number) => DEMO_NOW - s
type Demo = Omit<Pylon, "demo" | "provider" | "settlement" | "price_hint_msat" | "observed_at" | "bay" | "generation" | "valid_until"> & { heard: number }
const DEMO: Demo[] = [
  { pylon: "studio-gpu", label: "Studio GPU", status: "online", family: "gpu", tier: "medium", memory_gb: 16, slots: { total: 1, free: 0 },
    models: ["elephant-1"], lanes: ["cj-decision"], pools: [], since: ago(6 * 3600 + 720), heard: 18,
    answers_24h: 41, statuses_24h: 1, avg_reply_s: 3, last_answer_at: ago(12) },
  { pylon: "mac-m5-max", label: "Mac M5 Max", status: "online", family: "unified-memory", tier: "xl", memory_gb: 128, slots: { total: 2, free: 1 },
    models: ["qwen3.5-0.8b-q8_0", "gemma-4-e4b-it-q4_k_m"], lanes: ["chat"], pools: [], since: ago(4860), heard: 28,
    answers_24h: 9, statuses_24h: 1, avg_reply_s: 32, last_answer_at: ago(39) },
  { pylon: "l4-elephant", label: "L4 Elephant", status: "online", family: "gpu", tier: "large", memory_gb: 24, slots: { total: 4, free: 1 },
    models: ["elephant-1"], lanes: ["cj-decision"], pools: [], since: ago(2 * 86400 + 3 * 3600), heard: 9,
    answers_24h: 212, statuses_24h: 3, avg_reply_s: 2, last_answer_at: ago(4) },
  { pylon: "a100-mammoth", label: "A100 Mammoth", status: "online", family: "gpu", tier: "xl", memory_gb: 80, slots: { total: 4, free: 0 },
    models: ["mammoth-1"], lanes: ["cj-decision"], pools: [], since: ago(19 * 3600), heard: 6,
    answers_24h: 96, statuses_24h: 2, avg_reply_s: 9, last_answer_at: ago(7) },
  { pylon: "epyc-9654", label: "EPYC 9654", status: "online", family: "cpu", tier: "xl", memory_gb: 384, slots: { total: 8, free: 6 },
    models: ["elephant-1"], lanes: ["cj-decision"], pools: [], since: ago(5 * 86400), heard: 41,
    answers_24h: 18, statuses_24h: 0, avg_reply_s: 14, last_answer_at: ago(260) },
  { pylon: "mac-mini-m4", label: "Mac mini M4", status: "draining", family: "unified-memory", tier: "large", memory_gb: 32, slots: { total: 1, free: 1 },
    models: ["elephant-1"], lanes: ["cj-decision"], pools: [], since: ago(3 * 3600 + 600), heard: 52,
    answers_24h: 5, statuses_24h: 0, avg_reply_s: 6, last_answer_at: ago(1320) },
  { pylon: "r760-rack", label: "Dell R760", status: "offline", family: "cpu", tier: "large", memory_gb: 256, slots: { total: 4, free: 4 },
    models: ["elephant-1"], lanes: ["cj-decision"], pools: [], since: ago(9 * 3600), heard: 2460,
    answers_24h: 31, statuses_24h: 4, avg_reply_s: 21, last_answer_at: ago(2700) },
  { pylon: "coderos-4080", label: "CoderOS RTX 4080", status: "online", family: "gpu", tier: "medium", memory_gb: 16, slots: { total: 2, free: 2 },
    models: ["gemma-4-e4b-it-q4_k_m"], lanes: ["chat"], pools: ["coder"], since: ago(10 * 3600 + 1080), heard: 14,
    answers_24h: 0, statuses_24h: 0, avg_reply_s: 0, last_answer_at: 0 },
  { pylon: "rtx-5090", label: "RTX 5090 tower", status: "online", family: "gpu", tier: "large", memory_gb: 32, slots: { total: 2, free: 1 },
    models: ["qwen3.6-27b-q4_k_m"], lanes: ["chat"], pools: ["coder"], since: ago(28 * 3600), heard: 21,
    answers_24h: 57, statuses_24h: 1, avg_reply_s: 11, last_answer_at: ago(66) },
]
export const DEMO_PYLONS: Pylon[] = DEMO.map(({ heard, ...d }) => ({
  ...d, demo: true, generation: 0, valid_until: 0, provider: demoKey(d.pylon), settlement: ["free-v1"], price_hint_msat: null,
  observed_at: DEMO_NOW - heard, bay: bayOf(d.pools),
}))

// ---- the real feed: beacons from the relay

const tag = (e: Event, name: string) => e.tags.filter((t) => t[0] === name)

/** Projection of content admitted by the actual shared Coax verifier,
 * SHA-256, pb_read and pb_fresh. An optional owner auth is not an
 * authenticated ownership claim. No independent TS Nostr admission remains. */
export function fromBeacon(e: Event, now: number): Omit<Pylon, "answers_24h" | "statuses_24h" | "avg_reply_s" | "last_answer_at"> | null {
  const c = reader.beacon(e, now)
  if (!c) return null
  const services = c.services as any[]
  const prices = services.map((s) => s.price_hint_msat).filter((p) => typeof p === "number")
  return {
    demo: false, generation: c.generation, valid_until: c.valid_until,
    provider: c.provider, pylon: c.pylon, label: c.label, status: c.status,
    family: c.class.family, tier: c.class.tier, memory_gb: c.class.memory_gb,
    slots: { total: c.slots.total, free: c.slots.free },
    models: [...new Set<string>(services.map((s) => s.model.split("@")[0]))],
    lanes: [...new Set<string>(services.flatMap((s) => s.lanes))],
    settlement: c.settlement, pools: c.pools,
    price_hint_msat: prices.length ? Math.min(...prices) : null,
    since: c.since, observed_at: c.observed_at, bay: bayOf(c.pools),
  }
}

/** Every Pylon with a live beacon on the relay, its newest generation, and its last day's work. */
export async function readPylons(relay: Relay, now = Math.floor(Date.now() / 1000)): Promise<Pylon[]> {
  const beacons = await relay.all([reader.filter(5)])
  const best = new Map<string, { gen: number; p: NonNullable<ReturnType<typeof fromBeacon>> }>()
  for (const e of beacons) {
    const p = fromBeacon(e, now)
    if (!p) continue
    const key = `${p.provider}:${p.pylon}`, gen = p.generation
    if (!best.has(key) || reader.newer(gen, p.observed_at, best.get(key)!.gen, best.get(key)!.p.observed_at)) best.set(key, { gen, p })
  }
  const out: Pylon[] = []
  for (const { p } of best.values()) {
    const work = await relay.all([reader.filter(6, p.provider, String(now))])
    const answers = work.filter((e) => e.kind === KIND_ANSWER)
    const ids = [...new Set(answers.flatMap((a) => tag(a, "e").map((x) => x[1])))]
    const requests: Event[] = []
    for (let i = 0; i < ids.length; i += 50)
      requests.push(...await relay.all([reader.filter(7, "", JSON.stringify(ids.slice(i, i + 50)))]))
    const stats = reader.stats(work, requests, p.provider, now)
    // These are signed provider-wide answer/status observations. A status
    // may be progress, error or cancellation; its encrypted body is private.
    out.push({ ...p, answers_24h: stats.answers, statuses_24h: stats.statuses,
      avg_reply_s: stats.average, last_answer_at: stats.last })
  }
  return out
}

/** The Pylons landing.ts writes: the relay's, or the demo when PYLON_SOURCE says so. */
export async function pylons(relayUrl: string, open: (url: string) => Promise<Relay>): Promise<Pylon[]> {
  if (PYLON_SOURCE === "demo") return DEMO_PYLONS
  const relay = await open(relayUrl)
  try { return await readPylons(relay) } finally { relay.close() }
}

// ---- the table: where each stands, and its row

/** A bay: rows of three, back from the front corner, offset from its project in
 * the view's diagonals (u = x - z across the screen, w = x + z toward the eye). */
const BAYS: Record<string, { du: number; dw: number }> = { psionic: { du: 4.4, dw: -3.6 }, coder: { du: 6.2, dw: -0.6 } }
const ACROSS = 3, STEP = 1.9

const clean = (s: string) => s.replace(/[|\n\\"]/g, " ").replace(/[^\x20-\x7e]/g, "").trim()
const short = (hex: string) => { const n = reader.npub(hex); return `${n.slice(0, 9)}...${n.slice(-4)}` }

/** LAND_PYLONS rows: places is slug -> [project index, x, z]; each bay fills in arrival order (since). */
export function pylonRows(list: Pylon[], places: Map<string, [number, number, number]>, now: number): string[] {
  const rows: string[] = []
  const bays = new Map<string, Pylon[]>()
  for (const p of list) { if (!places.has(p.bay)) continue; if (!bays.has(p.bay)) bays.set(p.bay, []); bays.get(p.bay)!.push(p) }
  for (const [slug, ps] of bays) {
    const [k, px, pz] = places.get(slug)!
    const b = BAYS[slug] ?? { du: 6, dw: -5 }
    ps.sort((a, b) => a.since - b.since || a.pylon.localeCompare(b.pylon))
    ps.forEach((p, i) => {
      const u = px - pz + b.du + (i % ACROSS) * STEP, w = px + pz + b.dw - Math.floor(i / ACROSS) * STEP
      const x = ((u + w) / 2).toFixed(2), z = ((w - u) / 2).toFixed(2)
      rows.push([
        p.demo ? "demo" : "beacon", k, x, z, clean(p.pylon), clean(p.label).slice(0, 40), short(p.provider), p.status,
        p.family, p.tier, p.memory_gb, p.slots.total, p.slots.free,
        p.models.map(clean).join(", "), p.lanes.map(clean).join(", "), p.settlement.map(clean).join(", "), p.pools.map(clean).join(", "),
        p.price_hint_msat ?? -1, Math.max(0, now - p.since), Math.max(0, now - p.observed_at),
        p.answers_24h, p.statuses_24h, p.avg_reply_s, p.last_answer_at ? Math.max(0, now - p.last_answer_at) : -1,
        p.demo ? -1 : Math.max(0, p.valid_until - now),
      ].join("|"))
    })
  }
  return rows
}
export const PYLON_COLUMNS =
  "source (demo or beacon)|project|x|z|pylon|label|npub|status|family|tier|memory_gb|slots|free|models|lanes|settlement|pools|price_hint_msat (-1 none)|online s|heard s|signed provider answers 24h|signed provider status updates 24h|mean reply delay s (-1 unknown)|last answer s (-1 none)|beacon valid for s (-1 demo)"
/** The clock rows are aged against: the demo's own, or now. */
export const pylonNow = (list: Pylon[]) => (list.length > 0 && list.every((p) => p.demo) ? DEMO_NOW : Math.floor(Date.now() / 1000))
