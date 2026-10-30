// The Pylons on the /new-land Grid: one row per Pylon, shaped by its NIP-PYLON
// beacon (kind 30200, openagents.pylon-beacon.v1, the content that
// psionic/lib/pylon/beacon.coax writes and reads) plus what a reader counts
// from the Pylon's own events (26910 answers and 27010 status) on the relay.
//
// THE REAL FEED PLUGS IN HERE: landing.ts calls `pylons()`. Today that returns
// DEMO_PYLONS (every row demo: true). When chapter 19's Pylons publish beacons
// to the relay, set PYLON_SOURCE to "relay": `readPylons` reads the 30200
// beacons (verified, admitted as NIP-PYLON's Validation says, newest
// generation per pylon) and the 26910/27010 counts, and the page draws those
// instead. Nothing in newland.coax changes: it draws whatever LAND_PYLONS holds.
import { createHash } from "node:crypto"
import { npubEncode } from "nostr-effect/nip19"
import { verifyEvent, type Event } from "nostr-effect/pure"
import type { Relay } from "./relay"

export const PYLON_SOURCE: "demo" | "relay" = "demo"

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
  jobs_24h: number
  failed_24h: number        // 27010s: refused, failed or cancelled
  avg_job_s: number         // 0: none answered
  last_job_at: number       // 0: none
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
type Demo = Omit<Pylon, "demo" | "provider" | "settlement" | "price_hint_msat" | "observed_at" | "bay"> & { heard: number }
const DEMO: Demo[] = [
  { pylon: "studio-gpu", label: "Studio GPU", status: "online", family: "gpu", tier: "medium", memory_gb: 16, slots: { total: 1, free: 0 },
    models: ["elephant-1"], lanes: ["cj-decision"], pools: [], since: ago(6 * 3600 + 720), heard: 18,
    jobs_24h: 41, failed_24h: 1, avg_job_s: 3, last_job_at: ago(12) },
  { pylon: "mac-m5-max", label: "Mac M5 Max", status: "online", family: "unified-memory", tier: "xl", memory_gb: 128, slots: { total: 2, free: 1 },
    models: ["qwen3.5-0.8b-q8_0", "gemma-4-e4b-it-q4_k_m"], lanes: ["chat"], pools: [], since: ago(4860), heard: 28,
    jobs_24h: 9, failed_24h: 1, avg_job_s: 32, last_job_at: ago(39) },
  { pylon: "l4-elephant", label: "L4 Elephant", status: "online", family: "gpu", tier: "large", memory_gb: 24, slots: { total: 4, free: 1 },
    models: ["elephant-1"], lanes: ["cj-decision"], pools: [], since: ago(2 * 86400 + 3 * 3600), heard: 9,
    jobs_24h: 212, failed_24h: 3, avg_job_s: 2, last_job_at: ago(4) },
  { pylon: "a100-mammoth", label: "A100 Mammoth", status: "online", family: "gpu", tier: "xl", memory_gb: 80, slots: { total: 4, free: 0 },
    models: ["mammoth-1"], lanes: ["cj-decision"], pools: [], since: ago(19 * 3600), heard: 6,
    jobs_24h: 96, failed_24h: 2, avg_job_s: 9, last_job_at: ago(7) },
  { pylon: "epyc-9654", label: "EPYC 9654", status: "online", family: "cpu", tier: "xl", memory_gb: 384, slots: { total: 8, free: 6 },
    models: ["elephant-1"], lanes: ["cj-decision"], pools: [], since: ago(5 * 86400), heard: 41,
    jobs_24h: 18, failed_24h: 0, avg_job_s: 14, last_job_at: ago(260) },
  { pylon: "mac-mini-m4", label: "Mac mini M4", status: "draining", family: "unified-memory", tier: "large", memory_gb: 32, slots: { total: 1, free: 1 },
    models: ["elephant-1"], lanes: ["cj-decision"], pools: [], since: ago(3 * 3600 + 600), heard: 52,
    jobs_24h: 5, failed_24h: 0, avg_job_s: 6, last_job_at: ago(1320) },
  { pylon: "r760-rack", label: "Dell R760", status: "offline", family: "cpu", tier: "large", memory_gb: 256, slots: { total: 4, free: 4 },
    models: ["elephant-1"], lanes: ["cj-decision"], pools: [], since: ago(9 * 3600), heard: 2460,
    jobs_24h: 31, failed_24h: 4, avg_job_s: 21, last_job_at: ago(2700) },
  { pylon: "coderos-4080", label: "CoderOS RTX 4080", status: "online", family: "gpu", tier: "medium", memory_gb: 16, slots: { total: 2, free: 2 },
    models: ["gemma-4-e4b-it-q4_k_m"], lanes: ["chat"], pools: ["coder"], since: ago(10 * 3600 + 1080), heard: 14,
    jobs_24h: 0, failed_24h: 0, avg_job_s: 0, last_job_at: 0 },
  { pylon: "rtx-5090", label: "RTX 5090 tower", status: "online", family: "gpu", tier: "large", memory_gb: 32, slots: { total: 2, free: 1 },
    models: ["qwen3.6-27b-q4_k_m"], lanes: ["chat"], pools: ["coder"], since: ago(28 * 3600), heard: 21,
    jobs_24h: 57, failed_24h: 1, avg_job_s: 11, last_job_at: ago(66) },
]
export const DEMO_PYLONS: Pylon[] = DEMO.map(({ heard, ...d }) => ({
  ...d, demo: true, provider: demoKey(d.pylon), settlement: ["free-v1"], price_hint_msat: null,
  observed_at: DEMO_NOW - heard, bay: bayOf(d.pools),
}))

// ---- the real feed: beacons from the relay

const FAMILIES = new Set<string>(["unified-memory", "gpu", "cpu"])
const TIERS = new Set<string>(["small", "medium", "large", "xl"])
const STATUSES = new Set<string>(["online", "draining", "offline"])
const tag = (e: Event, name: string) => e.tags.filter((t) => t[0] === name)

/** A 30200 event as a Pylon, or null when NIP-PYLON's checks refuse it
 * (the same checks as pb_read in psionic/lib/pylon/beacon.coax, less the x
 * digest, which the caller's relay read already ties to the signed content). */
export function fromBeacon(e: Event, now: number): Omit<Pylon, "jobs_24h" | "failed_24h" | "avg_job_s" | "last_job_at"> | null {
  if (e.kind !== PB_KIND || !verifyEvent(e)) return null
  const d = tag(e, "d"), t = tag(e, "t")
  if (d.length !== 1 || t.length !== 1 || t[0][1] !== PB_MARKER) return null
  let c: any
  try { c = JSON.parse(e.content) } catch { return null }
  if (c?.v !== PB_VERSION || c.provider !== e.pubkey || c.pylon !== d[0][1]) return null
  if (!STATUSES.has(c.status) || !FAMILIES.has(c.class?.family) || !TIERS.has(c.class?.tier)) return null
  if (!(c.valid_until > now) || c.observed_at > now + 30) return null   // stale or from the future
  const services: any[] = Array.isArray(c.services) ? c.services : []
  const prices = services.map((s) => s.price_hint_msat).filter((p) => typeof p === "number")
  const pools: string[] = Array.isArray(c.pools) ? c.pools : []
  return {
    demo: false, provider: e.pubkey, pylon: c.pylon, label: String(c.label ?? c.pylon), status: c.status,
    family: c.class.family, tier: c.class.tier, memory_gb: Number(c.class.memory_gb) || 0,
    slots: { total: Number(c.slots?.total) || 0, free: Number(c.slots?.free) || 0 },
    models: [...new Set(services.map((s) => String(s.model ?? "").split("@")[0]).filter(Boolean))],
    lanes: [...new Set(services.flatMap((s) => (Array.isArray(s.lanes) ? s.lanes : []).map(String)))],
    settlement: Array.isArray(c.settlement) ? c.settlement.map(String) : [], pools,
    price_hint_msat: prices.length ? Math.min(...prices) : null,
    since: Number(c.since) || 0, observed_at: Number(c.observed_at) || 0, bay: bayOf(pools),
  }
}

/** Every Pylon with a live beacon on the relay, its newest generation, and its last day's work. */
export async function readPylons(relay: Relay, now = Math.floor(Date.now() / 1000)): Promise<Pylon[]> {
  const beacons = await relay.all([{ kinds: [PB_KIND], "#t": [PB_MARKER] }])
  const best = new Map<string, { gen: number; p: NonNullable<ReturnType<typeof fromBeacon>> }>()
  for (const e of beacons) {
    const p = fromBeacon(e, now)
    if (!p) continue
    const key = `${p.provider}:${p.pylon}`, gen = Number(JSON.parse(e.content).generation) || 0
    if (!best.has(key) || best.get(key)!.gen < gen) best.set(key, { gen, p })
  }
  const out: Pylon[] = []
  for (const { p } of best.values()) {
    const day = now - 86400
    const work = await relay.all([{ kinds: [KIND_ANSWER, KIND_STATUS], authors: [p.provider], since: day }])
    const answers = work.filter((e) => e.kind === KIND_ANSWER)
    // a 27010 is a refusal, an error or a cancel (its body is encrypted to the buyer; the count is public)
    const failed = work.filter((e) => e.kind === KIND_STATUS).length
    // a job's time: from the request it answers (its e tag) to the answer
    const asks = new Map((await relay.all([{ ids: answers.flatMap((a) => tag(a, "e").map((x) => x[1])).slice(0, 500) }]))
      .map((r) => [r.id, r.created_at] as const))
    const spans = answers.map((a) => a.created_at - (asks.get(tag(a, "e")[0]?.[1] ?? "") ?? a.created_at)).filter((s) => s >= 0)
    out.push({ ...p, jobs_24h: answers.length, failed_24h: failed,
      avg_job_s: spans.length ? Math.round(spans.reduce((a, b) => a + b, 0) / spans.length) : 0,
      last_job_at: answers.reduce((m, a) => Math.max(m, a.created_at), 0) })
  }
  return out
}

/** The Pylons landing.ts writes: the demo today, the relay's once PYLON_SOURCE says so. */
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
const short = (hex: string) => { const n = npubEncode(hex); return `${n.slice(0, 9)}...${n.slice(-4)}` }

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
        p.jobs_24h, p.failed_24h, p.avg_job_s, p.last_job_at ? Math.max(0, now - p.last_job_at) : -1,
      ].join("|"))
    })
  }
  return rows
}
export const PYLON_COLUMNS =
  "source (demo or beacon)|project|x|z|pylon|label|npub|status|family|tier|memory_gb|slots|free|models|lanes|settlement|pools|price_hint_msat (-1 none)|online s|heard s|jobs 24h|failed 24h|avg job s|last job s (-1 none)"
/** The clock rows are aged against: the demo's own, or now. */
export const pylonNow = (list: Pylon[]) => (list.every((p) => p.demo) ? DEMO_NOW : Math.floor(Date.now() / 1000))
