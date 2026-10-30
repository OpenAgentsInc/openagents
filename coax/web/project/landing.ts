// The /new-land mock-up's data: each project's tasks from its relay (verified
// and folded as snapshot.ts does) and its recent commits from git, written as
// Coax tables into apps/openagents-web/newland-data.coax. A fixture: the page
// draws what this captured until it reads the relay itself.
//
//   bun landing.ts
import { readFileSync, writeFileSync } from "node:fs"
import { join } from "node:path"
import { npubEncode } from "nostr-effect/nip19"
import { createHash } from "node:crypto"
import { Relay } from "./relay"
import { filters, fold, followups, KIND_ISSUE, type Folded, type ProjectConfig } from "./model"
import { pylons, pylonNow, pylonRows, PYLON_COLUMNS } from "./pylons"

const ROOT = join(import.meta.dir, "../../..")
// The places on the grid, by cluster: one owner's stack together, the worlds together.
const PLACES: [string, number, number, number][] = [
  // slug, cluster, x, z (grid units): the stack to the left, the worlds to the right, as seen from the south-east
  ["coax", 0, -4.2, 4.2],
  ["psionic", 0, -12.7, 3.5],
  ["openagents-desktop", 0, -5.3, 13.1],
  ["coder", 0, 4.9, 6.4],
  ["verse", 1, -0.4, -10.3],
  ["planetary-villa", 1, 8.8, -10.3],
]
const clean = (s: string) => s.replace(/[|\n\\"]/g, " ").replace(/[^\x20-\x7e]/g, "").trim()
const cut = (s: string, n: number) => (s.length > n ? s.slice(0, n - 3).trimEnd() + "..." : s)

// Which project a changed file belongs to, by the folders each owns (most specific first); "" for none.
// Commits are never matched by their message.
const WORLDS = ["everglade", "fairy-forest", "icecream", "deadgears", "haunt"]
const REALM_PAGES = /^apps\/openagents-web\/(realm|realms\/|everglade|fairy|icecream|deadgears|garden|thegrid|waterlab|characters|fixture|droprealm|myrealm|builder|verse|room|world|place|save|socket|textkeys)/
function owner(file: string): string {
  if (file.startsWith("verse/planetary-villa/") || /^apps\/openagents-web\/villa/.test(file)) return "planetary-villa"
  if (/^coax\/web\/goldens\/(hecate|villa)/.test(file)) return "planetary-villa"
  // the realms' goldens and docs are the worlds', though they sit in coax/web and docs/
  if (WORLDS.some((w) => file.startsWith(`verse/${w}/`)) || REALM_PAGES.test(file) || file.startsWith("coax/web/goldens/") || file.startsWith("docs/verse/")) return "verse"
  if (file.startsWith("coax/") || file.startsWith("verse/")) return "coax"   // the language and the Verse engine
  if (file.startsWith("psionic/") || file.startsWith("crates/psionic/") || file.startsWith("docs/psionic/")) return "psionic"
  if (file.startsWith("apps/openagents-desktop/")) return "openagents-desktop"
  if (file.startsWith("docs/coder/")) return "coder"
  return ""
}
// Each recent commit goes to the project owning most of its files.
const owned = new Map<string, string[]>()   // slug -> "hash|unix|author|subject", newest first
const byHash = new Map<string, string>()     // full hash -> its row
{
  const out = Bun.spawnSync(["git", "-C", ROOT, "log", "-n", "3000", "--name-only", "--format=@@%H|%at|%an|%s"]).stdout.toString()
  for (const block of out.split("@@").filter(Boolean)) {
    const [head, ...files] = block.trim().split("\n")
    const count = new Map<string, number>()
    for (const f of files) { const o = owner(f.trim()); if (o) count.set(o, (count.get(o) ?? 0) + 1) }
    const [h, ...rest] = head.split("|")
    const row = `${h.slice(0, 10)}|${rest.join("|")}`
    byHash.set(h, row)
    const best = [...count.entries()].sort((a, b) => b[1] - a[1])[0]
    if (best) { if (!owned.has(best[0])) owned.set(best[0], []); owned.get(best[0])!.push(row) }
  }
}
const recorded = new Map<string, string>()   // short hash -> the project whose task recorded it (done --commit)
const taskRows: string[][] = [], rows0: [string, string][] = []
const projects: string[] = [], working: string[] = [], feed: [number, string][] = [], commits: string[] = []
for (const [k, [slug, cluster, x, z]] of PLACES.entries()) {
  const dir = join(ROOT, "apps/openagents-web/projects", slug)
  const config: ProjectConfig = JSON.parse(readFileSync(join(dir, "project.json"), "utf8"))
  const pubkey: string = (config as any).pubkey
  const relay = await Relay.open(config.relay, null)
  const first = await relay.all(filters(pubkey, config))
  const ids = first.filter((e) => e.kind === KIND_ISSUE).map((e) => e.id)
  const more = []
  for (let i = 0; i < ids.length; i += 50) more.push(...await relay.all(followups(pubkey, ids.slice(i, i + 50))))
  relay.close()
  const f: Folded = fold(pubkey, config, [...first, ...more])
  const fin = (s: string) => s === "resolved" || s === "closed"
  const tasks = f.issues.filter((i) => i.kind === "task")
  const done = tasks.filter((i) => fin(i.status)).length
  // the milestone being worked now: the first unfinished one in rank order with work under it
  const ms = f.issues.filter((i) => i.kind === "milestone" && !fin(i.status)).sort((a, b) => (a.rank < b.rank ? -1 : 1))
  let mname = "", mdone = 0, mtotal = 0
  for (const m of ms) {
    const under = tasks.filter((t) => t.parent === m.id)
    if (!under.length || under.every((t) => fin(t.status))) continue
    mname = clean(m.title); mdone = under.filter((t) => fin(t.status)).length; mtotal = under.length
    if (under.some((t) => t.claimed_by)) break
    break
  }
  const on = tasks.filter((t) => t.claimed_by && !fin(t.status))
  // one agent per worker name, on its newest claim
  const seen = new Set<string>()
  for (const t of on.sort((a, b) => b.claimed_at - a.claimed_at)) {
    if (seen.has(t.claimed_by)) continue
    seen.add(t.claimed_by)
    working.push(`${clean(t.claimed_by)}|${k}|${cut(clean(t.title), 60)}`)
    feed.push([t.claimed_at, `${k}|WORKING|${clean(t.claimed_by)}|${cut(clean(t.title), 60)}`])
  }
  for (const t of tasks.filter((t) => fin(t.status)).sort((a, b) => b.status_at - a.status_at).slice(0, 3))
    feed.push([t.status_at, `${k}|DONE||${cut(clean(t.title), 60)}`])
  // its commits: the ones its tasks recorded when they were done, then the ones touching what it owns
  const mine: string[] = []
  for (const t of tasks) {
    if (!t.commit) continue
    const full = [...byHash.keys()].find((h) => h.startsWith(t.commit.slice(0, 10)))
    if (full) { mine.push(byHash.get(full)!); recorded.set(full.slice(0, 10), slug) }
  }
  taskRows.push(mine)
  rows0.push([slug, `${slug}|${clean(config.name)}|${clean((config as any).owner ?? "")}|${cluster}|${x}|${z}|${done}|${tasks.length}|${cut(mname, 48)}|${mdone}|${mtotal}|${on.length}`])
  console.error(`landing: ${slug} ${done}/${tasks.length} done, ${on.length} working`)
}
// With every project's recorded commits known, each project's history: its
// recorded ones, then the ones its folders own that no other project recorded.
for (const [k, [slug, row]] of rows0.entries()) {
  const mine = [...taskRows[k]]
  for (const r of owned.get(slug) ?? []) if (!mine.includes(r)) mine.push(r)
  const log = mine.filter((r) => { const o = recorded.get(r.split("|")[0]); return !o || o === slug })
    .sort((a, b) => Number(b.split("|")[1]) - Number(a.split("|")[1])).slice(0, 48).reverse()
  for (const line of log) {
    const [h, , an, ...s] = line.split("|")
    commits.push(`${k}|${h}|${clean(an)}|${cut(clean(s.join(" ")), 64)}`)
  }
  for (const line of log.slice(-2)) {
    const [h, at, , ...s] = line.split("|")
    feed.push([Number(at), `${k}|COMMIT|${h}|${cut(clean(s.join(" ")), 60)}`])
  }
  projects.push(`${row}|${log.length}`)
  console.error(`landing: ${slug} ${log.length} commits (${taskRows[k].length} recorded by its tasks)`)
}
feed.sort((a, b) => b[0] - a[0])
// The people are simulated: made-up names, each with a key made from its name, shown short.
const NAMES = ["ada", "rui", "noor", "theo", "lena", "sam", "ivo", "zuri", "kai", "mira", "otto", "bea", "finn", "yuki"]
const people = NAMES.map((n) => {
  const npub = npubEncode(createHash("sha256").update(`new-land person ${n}`).digest("hex"))
  return `${n}|${npub.slice(0, 9)}...${npub.slice(-4)}`
})
// The Pylons: the real beacons on the relay (pylons.ts PYLON_SOURCE; "demo" for the made-up machines).
const places = new Map(PLACES.map(([slug, , x, z], k) => [slug, [k, x, z] as [number, number, number]]))
const pylonList = await pylons("wss://relay.openagents.com", (url) => Relay.open(url, null))
const pylonTable = pylonRows(pylonList, places, pylonNow(pylonList))
console.error(`landing: ${pylonTable.length} pylons (${pylonList.filter((p) => p.demo).length} demo)`)
const lit = (rows: string[]) => JSON.stringify(rows.map((r) => r + "\n").join(""))
const out = join(ROOT, "apps/openagents-web/newland-data.coax")
writeFileSync(out, [
  "// Generated by coax/web/project/landing.ts: do not edit; run it.",
  `// Captured ${new Date().toISOString().slice(0, 16)}Z from each project's relay and git.`,
  "// LAND_PROJECTS: slug|name|owner|cluster|x|z|done|tasks|milestone|milestone done|milestone tasks|working|commits",
  `LAND_PROJECTS :: ${lit(projects)};`,
  "// LAND_WORKING: agent|project|task",
  `LAND_WORKING :: ${lit(working)};`,
  "// LAND_FEED, newest first: project|WORKING, DONE or COMMIT|who or hash|text",
  `LAND_FEED :: ${lit(feed.slice(0, 24).map((f) => f[1]))};`,
  "// LAND_PEOPLE (simulated): name|short npub",
  `LAND_PEOPLE :: ${lit(people)};`,
  "// LAND_COMMITS, oldest first per project: project|hash|author|subject",
  `LAND_COMMITS :: ${lit(commits)};`,
  `// LAND_PYLONS, from coax/web/project/pylons.ts (a demo row says demo): ${PYLON_COLUMNS}`,
  `LAND_PYLONS :: ${lit(pylonTable)};`,
  "",
].join("\n"))
console.error(`landing: wrote ${out}`)
process.exit(0)
