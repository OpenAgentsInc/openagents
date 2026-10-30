// The realm host's one table (apps/openagents-web/realms.json) into what
// the site builds from it (docs/verse/realm-host.md):
//
//   apps/openagents-web/realms.coax           the galaxy's worlds (verse.coax loads it)
//   apps/openagents-web/realms/<slug>/index.html  each realm's page: its module, assets and save slot
//
// Adding a realm is data: a world with a realm in realms.json, its module
// (<module>.coax, <module>-face.c, <module>.wgsl, built by build.sh) and
// its realms/<slug>/index.md. The build runs this first; --check fails when
// the written files are behind the table, without writing.
//
//   bun coax/web/realms.ts           write
//   bun coax/web/realms.ts --check   compare only
import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs"
import { join } from "node:path"
import { LOADER_CSS, loaderMarkup } from "./loader.js"

const SITE = join(import.meta.dir, "../../apps/openagents-web")
const KINDS = { locked: 0, open: 1, soon: 2 } as const
// golden: what the release walk (goldens.ts) asks of the realm's page: the least share of
// its pixels lit, how long it settles before its shot, how many 8 px blocks may differ.
export type Golden = { lit: number; settle: number; blocks: number }
// style: the realm's art style (docs/verse/art-styles.md); style_views: the hashes the style check shoots it at (style-check.ts); zones: parts of it drawn in another;
// clock: how its time of day runs (cycle, fixed:T with T 0 midnight to 1, or real: the visitor's time).
export const STYLES = ["lowpoly", "painterly", "photoreal", "flat"] as const
export type Style = (typeof STYLES)[number]
// ruleset: the rules a realm plays by (verse/rules/srd5.coax), read by its page as ruleset=; none casts nothing.
export const RULESETS = ["srd5"]
export type Zone = { id: string; style?: Style }
export type Realm = { slug: string; title: string; description: string; alt: string; module: string; slot: string; slot_was?: string; voice?: string; ruleset?: string; uploads?: string; card?: string; style: Style; style_views?: string[]; zones?: Zone[]; clock: string; assets: string[]; golden: Golden }
export type World = { id: string; name: string; kind: keyof typeof KINDS; listed?: boolean; orbit: { r: number; a: number; size: number }; realm?: Realm }

const ID = /^[a-z0-9][a-z0-9-]{0,47}$/
export const CLOCK = /^(cycle|real|fixed:(0(\.\d{1,4})?|1(\.0{1,4})?))$/
// What wgsl.py writes into a realm's WGSL from the style presets: a painterly, photoreal or flat
// realm draws with it. Required once verse/style.coax exists (as-presets).
export const STYLE_BLOCK = "// ---- generated: verse/style.coax"
const STYLE_SOURCE = join(import.meta.dir, "../../verse/style.coax")
const PACKS = join(import.meta.dir, "../../verse/packs")
// Painterly realms whose pack isn't painted yet: each leaves this set with its pack task
// (Everglade: eg-paint-pack, #f300). A pack that names a style never passes by it.
const PACK_STYLE_PENDING = new Set(["everglade"])
/** The table, checked: unique ids, slugs and slots; an open world has a realm and a realm a module,
 *  a style, its zones' ids once each, and a clock; a painterly, photoreal or flat realm's WGSL has the style block. */
export function worlds(file = join(SITE, "realms.json"), opts: { site?: string; styleBlock?: boolean; packs?: string } = {}): World[] {
  const site = opts.site ?? SITE, styleBlock = opts.styleBlock ?? existsSync(STYLE_SOURCE), packs = opts.packs ?? PACKS
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
    if (!STYLES.includes(r.style)) throw new Error(`realms.json: ${r.slug}: style is one of ${STYLES.join(", ")}, not ${JSON.stringify(r.style)}`)
    const zones = new Set<string>()
    for (const z of r.zones ?? []) {
      if (!ID.test(z?.id) || (z.style !== undefined && !STYLES.includes(z.style))) throw new Error(`realms.json: ${r.slug}: zone ${JSON.stringify(z?.id)}: an id and a style (${STYLES.join(", ")})`)
      if (zones.has(z.id)) throw new Error(`realms.json: ${r.slug}: zone ${z.id} twice`)
      zones.add(z.id)
    }
    if (typeof r.clock !== "string" || !CLOCK.test(r.clock)) throw new Error(`realms.json: ${r.slug}: clock is cycle, real or fixed:T (T 0 to 1), not ${JSON.stringify(r.clock)}`)
    if (styleBlock && [r.style, ...(r.zones ?? []).map((z) => z.style)].some((st) => st === "painterly" || st === "photoreal" || st === "flat")) {
      const wgsl = join(site, `${r.module}.wgsl`)
      if (!existsSync(wgsl) || !readFileSync(wgsl, "utf8").includes(STYLE_BLOCK)) throw new Error(`realms.json: ${r.slug} is drawn ${r.style === "lowpoly" ? "partly painterly or photoreal" : r.style} but ${r.module}.wgsl has no style block (coax/web/wgsl.py verse/style.coax ... --into it)`)
    }
    // its pack's style (verse/packs/<pack>.json "style", none is lowpoly) is one the realm draws in
    for (const a of r.assets) {
      const pack = /^pack:.*\/([a-z0-9-]+)\.pack$/.exec(a)?.[1]
      const manifest = pack && join(packs, `${pack}.json`)
      if (!manifest || !existsSync(manifest)) continue
      const declared = JSON.parse(readFileSync(manifest, "utf8")).style, ps = declared ?? "lowpoly"
      const drawn = [r.style, ...(r.zones ?? []).map((z) => z.style ?? r.style)]
      if (!drawn.includes(ps) && !(declared === undefined && PACK_STYLE_PENDING.has(r.slug)))
        throw new Error(`realms.json: ${r.slug} is drawn ${drawn.join(" and ")} but its pack ${pack} is ${ps} (verse/packs/${pack}.json "style")`)
    }
    // its ruleset (verse/rules/<ruleset>.coax): a realm without one casts nothing
    if (r.ruleset !== undefined && !RULESETS.includes(r.ruleset)) throw new Error(`realms.json: ${r.slug}: ruleset is one of ${RULESETS.join(", ")}, not ${JSON.stringify(r.ruleset)}`)
    for (const a of r.assets) if (!/^[a-z]+:\S+$/.test(a)) throw new Error(`realms.json: ${r.slug}: asset ${a}`)
    const g = r.golden
    if (!g || !(g.lit > 0 && g.lit < 1) || !(g.settle >= 0 && g.settle <= 20000) || !(Number.isInteger(g.blocks) && g.blocks >= 0)) throw new Error(`realms.json: ${r.slug}: golden needs lit (0 to 1), settle (ms) and blocks`)
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
    `PLANET_PATH :: ${col("string", (w) => lit(w.realm ? `/realms/${w.realm.slug}` : ""))};`,
    "// Its art style (docs/verse/art-styles.md; a world without a realm is lowpoly) and its clock.",
    "PLANET_LOWPOLY :: 0; PLANET_PAINTERLY :: 1; PLANET_PHOTOREAL :: 2; PLANET_FLAT :: 3;   // verse/style-preset.coax STYLE_*",
    `PLANET_STYLE :: ${col("int", (w) => `PLANET_${(w.realm?.style ?? "lowpoly").toUpperCase()}`)};`,
    `PLANET_CLOCK :: ${col("string", (w) => lit(w.realm?.clock ?? ""))};`,
    "",
  ].join("\n")
}

const esc = (s: string) => s.replace(/&/g, "&amp;").replace(/"/g, "&quot;").replace(/</g, "&lt;")
export function page(r: Realm): string {
  const url = `https://openagents.com/realms/${r.slug}`, t = esc(r.title), d = esc(r.description)
  const card = `https://openagents.com${r.card ?? "/verse/og-verse.jpg"}`
  const slot = ` data-realm="${r.slug}" data-slot="${r.slot}"` + (r.slot_was ? ` data-slot-was="${r.slot_was}"` : "")
    + (r.voice ? ` data-voice="${r.voice}"` : "")
    + (r.ruleset ? ` data-ruleset="${r.ruleset}"` : "")
    + (r.uploads ? ` data-uploads="${esc(r.uploads)}"` : "")
    + ` data-style="${r.style}" data-clock="${r.clock}"`
    + (r.zones?.length ? ` data-zones="${r.zones.map((z) => `${z.id}:${z.style ?? r.style}`).join(" ")}"` : "")
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
<meta property="og:image" content="${card}">
<meta property="og:image:width" content="1200">
<meta property="og:image:height" content="630">
<meta property="og:image:alt" content="${esc(r.alt)}">
<meta name="twitter:card" content="summary_large_image">
<meta name="twitter:title" content="${t}">
<meta name="twitter:description" content="${d}">
<meta name="twitter:image" content="${card}">
<meta name="theme-color" content="#000000">
<link rel="icon" type="image/svg+xml" href="/favicon.svg">
<link rel="alternate" type="text/markdown" href="/realms/${r.slug}/index.md">
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
${LOADER_CSS}
</style>
</head>
<body>
<canvas id="coaxial" aria-hidden="true"></canvas>
${loaderMarkup(r.title, `/realms/${r.slug}/index.md`)}
<main>
<h1>${t}</h1><p><a href="/realms/${r.slug}/index.md">${t}</a></p>
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
  for (const w of list) if (w.realm) out.push([join(SITE, "realms", w.realm.slug, "index.html"), page(w.realm)])
  const behind: string[] = []
  for (const [file, text] of out) {
    if (existsSync(file) && readFileSync(file, "utf8") === text) continue
    if (check) { behind.push(file); continue }
    mkdirSync(join(file, ".."), { recursive: true })
    writeFileSync(file, text)
  }
  for (const w of list) if (w.realm && !existsSync(join(SITE, "realms", w.realm.slug, "index.md"))) behind.push(`realms/${w.realm.slug}/index.md (write it by hand)`)
  if (behind.length) { console.error(`realms.ts: behind realms.json:\n  ${behind.join("\n  ")}${check ? "\n  run: bun coax/web/realms.ts" : ""}`); process.exit(1) }
}
