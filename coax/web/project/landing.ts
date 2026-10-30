// The public homepage warm start: shared Coax Nostr verifies and folds each project's tasks from its relay and its recent commits from git, written as
// Coax tables into apps/openagents-web/newland-data.coax. A fixture: the page
// draws what this captured until it reads the relay itself.
//
//   bun landing.ts
import { readFileSync, writeFileSync, renameSync, rmSync, mkdirSync } from "node:fs"
import { join } from "node:path"
import { PublicRelay, reader, projectFilters, followupFilters } from "./landing-reader"
import { pylons, pylonNow, pylonRows, PYLON_COLUMNS } from "./pylons"

const ROOT = join(import.meta.dir, "../../..")
const args=process.argv.slice(2)
const flag=(name:string)=>{const at=args.indexOf(name);return at<0?null:args[at+1]}
const output=flag("--out")??join(ROOT,"apps/openagents-web/newland-data.coax")
const relayOverride=flag("--relay")
const known:Record<string,{key:string;ids:string[]}>= {}
let previous:typeof known={}
try { const old=readFileSync(output,"utf8").match(/^LAND_KNOWN_ISSUES :: (".*");$/m);if(old)previous=JSON.parse(JSON.parse(old[1])) } catch(e) {if((e as any).code!=="ENOENT")throw e}
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
// The seed: every admitted event as the page's cache entry (landing-cache.coax), so a first visit asks
// the relay only for what is newer than this capture. synced: each project's newest event here.
const seedEntries: string[] = [], seedSynced: number[] = [], seedFolds: string[] = []
for (const [k, [slug, cluster, x, z]] of PLACES.entries()) {
  const dir = join(ROOT, "apps/openagents-web/projects", slug)
  const config = JSON.parse(readFileSync(join(dir, "project.json"), "utf8"))
  const pubkey: string = (config as any).pubkey
  const relay = await PublicRelay.open(relayOverride??config.relay)
  const first = await relay.all(projectFilters(pubkey, config))
  if(previous[slug] && previous[slug].key!==pubkey)throw Error(`landing: ${slug} changed identity; explicit capture migration is required`)
  const seenRoots=new Set(first.map(e=>e.id))
  const missing=(previous[slug]?.ids??[]).filter(id=>!seenRoots.has(id))
  for(let i=0;i<missing.length;i+=50)first.push(...await relay.all([reader.filter(7,"",JSON.stringify(missing.slice(i,i+50)))]))
  const ids = first.filter((e) => e.kind === 1621).map((e) => e.id)
  const more = []
  for (let i = 0; i < ids.length; i += 50) more.push(...await relay.all(followupFilters(pubkey, ids.slice(i, i + 50))))
  relay.close()
  const f = reader.fold([...first, ...more], pubkey, config.slug,previous[slug]?.ids??[])
  const held = [...new Map([...first, ...more].map((e) => [e.id, e])).values()]
  for (const event of held) seedEntries.push(reader.cacheEntry(event, k))
  seedSynced[k] = Math.max(0, ...held.map((e) => e.created_at))
  // and the fold the page draws until something newer comes in, keyed as landing-cache.coax hc_key keys it
  const fold = JSON.stringify(f), key = held.reduce((a, e) => (a + parseInt(e.id.slice(0, 10), 16)) % 999999999989, 0)
  seedFolds.push(`P${k} ${String(held.length).padStart(7, "0")} ${String(key).padStart(13, "0")} ${String(Buffer.byteLength(fold)).padStart(7, "0")}\n${fold}\n`)
  known[slug]={key:pubkey,ids:f.issues.map(i=>i.id)}
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
// People are shown only when a real public presence source is integrated.
const people: string[] = []
// The Pylons: the real beacons on the relay (pylons.ts PYLON_SOURCE; "demo" for the made-up machines).
const places = new Map(PLACES.map(([slug, , x, z], k) => [slug, [k, x, z] as [number, number, number]]))
const pylonList = await pylons(relayOverride??"wss://relay.openagents.com", (url) => PublicRelay.open(url))
const pylonTable = pylonRows(pylonList, places, pylonNow(pylonList))
console.error(`landing: ${pylonTable.length} pylons (${pylonList.filter((p) => p.demo).length} demo)`)
const lit = (rows: string[]) => JSON.stringify(rows.map((r) => r + "\n").join(""))
const out = output
const captured = Math.floor(Date.now() / 1000)
// Parts the page's inbox holds whole (1 MiB, with the HTTP status line), the synced lines last.
const parts: string[] = []
{
  const synced = seedSynced.map((t, k) => t > 0 ? `S${k} ${String(t).padStart(10, "0")}\n` : "").join("")
  let part = "", size = 0
  for (const entry of [...seedEntries, ...seedFolds, synced]) {
    const n = Buffer.byteLength(entry)
    if (size > 0 && size + n > 900000) { parts.push(part); part = ""; size = 0 }
    part += entry; size += n
  }
  if (size > 0) parts.push(part)
}
const temporary = out + `.${process.pid}`
writeFileSync(temporary, [
  "// Generated by coax/web/project/landing.ts: do not edit; run it.",
  `// Captured ${new Date().toISOString().slice(0, 16)}Z from each project's relay and git.`,
  `LAND_CAPTURED :: ${captured};`,
  "// LAND_SEED_PARTS: build/home-seed/<LAND_CAPTURED>-<i>.txt, this capture's admitted events (landing-cache.coax)",
  `LAND_SEED_PARTS :: ${relayOverride || out !== join(ROOT, "apps/openagents-web/newland-data.coax") ? 0 : parts.length};`,
  `LAND_KNOWN_ISSUES :: ${JSON.stringify(JSON.stringify(known))};`,
  "// LAND_PROJECTS: slug|name|owner|cluster|x|z|done|tasks|milestone|milestone done|milestone tasks|working|commits",
  `LAND_PROJECTS :: ${lit(projects)};`,
  "// LAND_WORKING: agent|project|task",
  `LAND_WORKING :: ${lit(working)};`,
  "// LAND_FEED, newest first: project|WORKING, DONE or COMMIT|who or hash|text",
  `LAND_FEED :: ${lit(feed.slice(0, 24).map((f) => f[1]))};`,
  "// LAND_PEOPLE: empty; no simulated people",
  `LAND_PEOPLE :: ${lit(people)};`,
  "// LAND_COMMITS, oldest first per project: project|hash|author|subject",
  `LAND_COMMITS :: ${lit(commits)};`,
  `// LAND_PYLONS, from coax/web/project/pylons.ts (a demo row says demo): ${PYLON_COLUMNS}`,
  `LAND_PYLONS :: ${lit(pylonTable)};`,
  "",
].join("\n"))
// Public identities for the runtime watcher are generated from the exact
// successfully captured project configuration, rather than copied by hand.
if (!relayOverride && out === join(ROOT, "apps/openagents-web/newland-data.coax")) {
  const configs = PLACES.map(([slug]) => JSON.parse(readFileSync(join(ROOT,"apps/openagents-web/projects",slug,"project.json"),"utf8")))
  if (configs.some(c=>c.relay!==configs[0].relay)) throw Error("home: featured projects need an explicit relay routing migration")
  const target = join(ROOT,"apps/openagents-web/newland-targets.coax"), staged=target+`.${process.pid}`
  writeFileSync(staged,["// Generated public homepage identities. No signing keys.",
    `HOME_PROJECTS :: ${PLACES.length};`,
    `HOME_SLUGS :: [${PLACES.length}]string.[${PLACES.map(([slug])=>JSON.stringify(slug)).join(",")}];`,
    `HOME_KEYS :: [${PLACES.length}]string.[${configs.map(c=>JSON.stringify(c.pubkey)).join(",")}];`,
    `HOME_REPOS :: [${PLACES.length}]string.[${configs.map(c=>JSON.stringify(c.repo.d)).join(",")}];`,
    `HOME_KEY_JSON :: ${JSON.stringify(JSON.stringify(configs.map(c=>c.pubkey)))};`,
    `HOME_RELAY :: ${JSON.stringify(configs[0].relay)};`,""].join("\n"))
  renameSync(staged,target)
}
if (!relayOverride && out === join(ROOT, "apps/openagents-web/newland-data.coax")) {
  const seed = join(ROOT, "apps/openagents-web/build/home-seed")
  rmSync(seed, { recursive: true, force: true }); mkdirSync(seed, { recursive: true })
  parts.forEach((part, i) => writeFileSync(join(seed, `${captured}-${i}.txt`), part))
  console.error(`landing: seed ${seedEntries.length} events in ${parts.length} parts, ${parts.reduce((a, p) => a + Buffer.byteLength(p), 0)} bytes`)
}
renameSync(temporary, out)
console.error(`landing: wrote verified capture ${out}`)
process.exit(0)
