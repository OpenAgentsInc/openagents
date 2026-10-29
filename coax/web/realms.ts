// The realm host's one table (apps/openagents-web/realms.json) into what
// the site builds from it (docs/verse/realm-host.md):
//
//   apps/openagents-web/realms.coax           the galaxy's worlds (verse.coax loads it)
//   apps/openagents-web/realm/<slug>/index.html  each realm's page: its module, assets and save slot
//
// Adding a realm is data: a world with a realm in realms.json, its module
// (<module>.coax, <module>-face.c, <module>.wgsl, built by build.sh) and
// its realm/<slug>/index.md. The build runs this first; --check fails when
// the written files are behind the table, without writing.
//
//   bun coax/web/realms.ts           write
//   bun coax/web/realms.ts --check   compare only
import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs"
import { join } from "node:path"

const SITE = join(import.meta.dir, "../../apps/openagents-web")
const KINDS = { locked: 0, open: 1, soon: 2 } as const
type Realm = { slug: string; title: string; description: string; alt: string; module: string; slot: string; slot_was?: string; assets: string[] }
type World = { id: string; name: string; kind: keyof typeof KINDS; listed?: boolean; orbit: { r: number; a: number; size: number }; realm?: Realm }

const ID = /^[a-z0-9][a-z0-9-]{0,47}$/
/** The table, checked: unique ids, slugs and slots; an open world has a realm and a realm a module. */
export function worlds(file = join(SITE, "realms.json")): World[] {
  const list: World[] = JSON.parse(readFileSync(file, "utf8")).worlds
  const seen = new Set<string>()
  const once = (what: string) => { if (seen.has(what)) throw new Error(`realms.json: ${what} twice`); seen.add(what) }
  for (const w of list) {
    if (!ID.test(w.id) || !(w.kind in KINDS)) throw new Error(`realms.json: bad world ${JSON.stringify(w.id)}`)
    if (!/^[A-Z0-9 '.-]{1,40}$/.test(w.name)) throw new Error(`realms.json: ${w.id}: the name is capitals, at most 40`)
    for (const k of ["r", "a", "size"] as const) if (!Number.isFinite(w.orbit?.[k])) throw new Error(`realms.json: ${w.id}: orbit.${k}`)
    once(`world ${w.id}`)
    if (w.kind === "open" && !w.realm) throw new Error(`realms.json: ${w.id} is open but has no realm`)
    const r = w.realm
    if (!r) continue
    if (!ID.test(r.slug) || !ID.test(r.slot) || !/^[a-z][a-z0-9-]{0,31}$/.test(r.module)) throw new Error(`realms.json: ${w.id}: slug, slot or module`)
    once(`slug ${r.slug}`); once(`slot ${r.slot}`)
    if (r.slot_was) once(`slot ${r.slot_was}`)
    for (const a of r.assets) if (!/^[a-z]+:\S+$/.test(a)) throw new Error(`realms.json: ${r.slug}: asset ${a}`)
  }
  return list
}

const lit = (s: string) => JSON.stringify(s)
const num = (n: number) => (Number.isInteger(n) ? n.toFixed(0) : String(n))
export function coax(list: World[]): string {
  const col = (type: string, f: (w: World) => string) => `[PLANETS] ${type}.[${list.map(f).join(", ")}]`
  return [
    "// Written by coax/web/realms.ts from realms.json: change the table, not this file.",
    "// The galaxy's worlds in order (verse.coax): id, name, kind, orbit, whether",
    "// the galaxy shows it (an unlisted one with ?realms=all) and the page it opens.",
    `PLANETS :: ${list.length};`,
    "KIND_LOCKED :: 0; KIND_OPEN :: 1; KIND_SOON :: 2;",
    `PLANET_ID :: ${col("string", (w) => lit(w.id))};`,
    `PLANET_NAME :: ${col("string", (w) => lit(w.name))};`,
    `PLANET_KIND :: ${col("int", (w) => ["KIND_LOCKED", "KIND_OPEN", "KIND_SOON"][KINDS[w.kind]])};`,
    `PLANET_R :: ${col("f32", (w) => num(w.orbit.r))};`,
    `PLANET_A :: ${col("f32", (w) => num(w.orbit.a))};`,
    `PLANET_SIZE :: ${col("f32", (w) => num(w.orbit.size))};`,
    `PLANET_LISTED :: ${col("bool", (w) => String(w.listed !== false))};`,
    `PLANET_PATH :: ${col("string", (w) => lit(w.realm ? `/realm/${w.realm.slug}` : ""))};`,
    "",
  ].join("\n")
}

const esc = (s: string) => s.replace(/&/g, "&amp;").replace(/"/g, "&quot;").replace(/</g, "&lt;")
export function page(r: Realm): string {
  const url = `https://openagents.com/realm/${r.slug}`, t = esc(r.title), d = esc(r.description)
  const slot = ` data-realm="${r.slug}" data-slot="${r.slot}"` + (r.slot_was ? ` data-slot-was="${r.slot_was}"` : "")
  const assets = r.assets.length ? ` data-assets="\n${r.assets.map((a) => `  ${a}`).join("\n")}"` : ""
  return `<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width,initial-scale=1,viewport-fit=cover">
<title>${t} · OpenAgents</title>
<meta name="description" content="${d}">
<meta property="og:type" content="website">
<meta property="og:site_name" content="OpenAgents">
<meta property="og:title" content="${t}">
<meta property="og:description" content="${d}">
<meta property="og:url" content="${url}">
<meta property="og:image" content="https://openagents.com/verse/og-verse.jpg">
<meta property="og:image:width" content="1200">
<meta property="og:image:height" content="630">
<meta property="og:image:alt" content="${esc(r.alt)}">
<meta name="twitter:card" content="summary_large_image">
<meta name="twitter:title" content="${t}">
<meta name="twitter:description" content="${d}">
<meta name="twitter:image" content="https://openagents.com/verse/og-verse.jpg">
<meta name="theme-color" content="#000000">
<link rel="icon" type="image/svg+xml" href="/favicon.svg">
<link rel="alternate" type="text/markdown" href="/realm/${r.slug}/index.md">
<script>if ("gpu" in navigator) document.documentElement.classList.add("gpu")</script>
<style>
html,body{margin:0;height:100%;background:#000;color:#ededed}
body{overflow:hidden;font-family:ui-sans-serif,system-ui,-apple-system,"Segoe UI",Roboto,"Helvetica Neue",Arial,sans-serif;font-size:14px;line-height:1.5}
canvas{position:fixed;inset:0;width:100%;height:100%;touch-action:none;user-select:none;-webkit-user-select:none;-webkit-touch-callout:none}
canvas[data-cursor=pointer]{cursor:pointer}
main{position:relative;max-width:640px;margin:0 auto;padding:24px 16px}
html:not(.gpu) body{overflow:auto}
a{color:#ff8c1f}
/* When the GPU draws the page, the text stays for agents, search engines and screen readers only. */
.gpu main{position:absolute;width:1px;height:1px;overflow:hidden;clip-path:inset(50%);white-space:nowrap;padding:0}
</style>
</head>
<body>
<canvas id="coaxial" aria-hidden="true"></canvas>
<main>
<h1>${t}</h1><p><a href="/realm/${r.slug}/index.md">${t}</a></p>
</main>
<script type="module" src="/coaxial/host.js" data-app="${r.module}"${slot}${assets}></script>
</body>
</html>
`
}

if (import.meta.main) {
  const check = process.argv.includes("--check")
  const list = worlds()
  const out: [string, string][] = [[join(SITE, "realms.coax"), coax(list)]]
  for (const w of list) if (w.realm) out.push([join(SITE, "realm", w.realm.slug, "index.html"), page(w.realm)])
  const behind: string[] = []
  for (const [file, text] of out) {
    if (existsSync(file) && readFileSync(file, "utf8") === text) continue
    if (check) { behind.push(file); continue }
    mkdirSync(join(file, ".."), { recursive: true })
    writeFileSync(file, text)
  }
  for (const w of list) if (w.realm && !existsSync(join(SITE, "realm", w.realm.slug, "index.md"))) behind.push(`realm/${w.realm.slug}/index.md (write it by hand)`)
  if (behind.length) { console.error(`realms.ts: behind realms.json:\n  ${behind.join("\n  ")}${check ? "\n  run: bun coax/web/realms.ts" : ""}`); process.exit(1) }
}
