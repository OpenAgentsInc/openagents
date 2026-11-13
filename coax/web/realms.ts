// The realm host's one table (apps/openagents-web/realms.json) into what
// the site builds from it (docs/metaverse/realm-host.md):
//
//   apps/openagents-web/realms.coax           the galaxy's worlds (verse.coax loads it)
//   apps/openagents-web/realms/<slug>/index.html  each realm's page: its module, assets and save slot
//   apps/openagents-web/realms/list.coax      /worlds' rows (worlds-web.coax): every open realm, listed or not
//   apps/openagents-web/realms/index.md       /worlds as text
//   apps/openagents-web/realms/index.html     /worlds: the page, its links for agents and readers without a GPU
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
import { execFileSync } from "node:child_process"
import { LOADER_CSS, loaderMarkup } from "./loader.js"
import { brief, renderedCard, DEFAULT_IMAGE } from "./og"

const SITE = join(import.meta.dir, "../../apps/openagents-web")
let renderProfiles: Map<string, number> | undefined
// Renderer identity comes from the compiled Coax catalog. A fork keeps this
// base profile even when its own realm/save-slot slug changes.
export function renderProfile(slug: string): number {
  if (!renderProfiles) {
    const root = join(import.meta.dir, "../..")
    const rows = execFileSync(join(root, "coax/web/tool.sh"), ["metaverse-engine/render/build.coax", "--profile-table"], { cwd: root, encoding: "utf8" }).trim().split("\n")
    renderProfiles = new Map(rows.map((row) => {
      const match = /^(\d+) ([a-z0-9-]+)$/.exec(row)
      if (!match) throw new Error("invalid compiled renderer profile row")
      return [match[2], Number(match[1])]
    }))
  }
  const index = renderProfiles.get(slug)
  if (index === undefined) throw new Error(`no compiled renderer profile for ${slug}`)
  return index
}
// Page adoption is decided in Coax, including explicitly tracked legacy
// consumers. Unknown identities fail rather than receiving a default profile.
const renderAttributeRows = new Map<string, string>()
export function renderAttributes(slug: string, module: string): string {
  const key = JSON.stringify([slug, module])
  const cached = renderAttributeRows.get(key)
  if (cached !== undefined) return cached
  const root = join(import.meta.dir, "../..")
  const row = execFileSync(join(root, "coax/web/tool.sh"), ["metaverse-engine/render/page-render.coax", slug, module], { cwd: root, encoding: "utf8" })
  renderAttributeRows.set(key, row)
  return row
}
const KINDS = { locked: 0, open: 1, soon: 2 } as const
// golden: what the release walk (goldens.ts) asks of the realm's page: the least share of
// its pixels lit, how long it settles before its shot, how many 8 px blocks may differ.
export type Golden = { lit: number; settle: number; blocks: number }
// style: the realm's art style (docs/metaverse/art-styles.md); style_views: the hashes the style check shoots it at (style-check.ts); zones: parts of it drawn in another;
// clock: how its time of day runs (cycle, fixed:T with T 0 midnight to 1, or real: the visitor's time).
export const STYLES = ["lowpoly", "painterly", "photoreal", "flat", "anime"] as const
export type Style = (typeof STYLES)[number]
// variant: an anime realm's or zone's look (metaverse/style-preset.coax ANIME_*); none is pastoral. Only anime has variants.
export const ANIME_VARIANTS = ["pastoral", "action", "dusk", "neon"] as const
export type AnimeVariant = (typeof ANIME_VARIANTS)[number]
// ruleset: the rules a realm plays by (metaverse/rules/srd5.coax), read by its page as ruleset=; none casts nothing.
export const RULESETS = ["srd5"]
export type Zone = { id: string; style?: Style; variant?: AnimeVariant }
// share: the line a link preview shows (at most 200 characters; else the description, shortened).
// page: the module's data-page (coaxial_page), when one module draws several pages: the Grid is the homepage's (newland, page 1).
// project_asset: an owner-local asset (local:/private/NAME.EXT) that ?project=SLUG swaps for /private/SLUG.EXT
// before the host starts (Animation Lab's project bundles).
// short: a top-level path that serves the same page too (/druid), written to DIR/index.html for the site bucket.
// place_of: the realm this page opens a place in (Character Create, at Everglade's glade, #13018): one
// world, so the page is that realm's, its module, slot and room; only its address and its first place differ.
export type Realm = { slug: string; short?: string; place_of?: string; title: string; description: string; share?: string; alt: string; module: string; page?: number; slot: string; slot_was?: string; ui_font_src?: string; ui_font_family?: string; ui_title_font_src?: string; ui_title_font_family?: string; ui_title_font_weight?: string; ui_title_font_scale?: number; voice?: string; project_asset?: string; ruleset?: string; uploads?: string; timing?: boolean; arrival?: "portal"; domain?: Domain; card?: string; added?: string; line?: string; style: Style; variant?: AnimeVariant; style_views?: string[]; zones?: Zone[]; clock: string; assets: string[]; golden: Golden }
// domain: the realm on a domain of its own (docs/metaverse/everglade-domain.md): a front door at the
// domain's / (realms/<slug>/domain/index.html, an MMORPG landing page in plain HTML, an owner
// decision 2026-11-08), /play (the realm's page under the domain's name) and a 404. Its pictures,
// icon and wordmark (coax/web/everglade-wordmark.coax) and index.md sit beside them, made by hand.
export type Feature = { title: string; line: string }
export type Shot = { src: string; alt: string }
export type Domain = { host: string; title: string; description: string; tagline: string; entry: string; create?: string; features: Feature[]; shots: Shot[] }
export type World = { id: string; name: string; kind: keyof typeof KINDS; listed?: boolean; orbit: { r: number; a: number; size: number }; realm?: Realm }

const ID = /^[a-z0-9][a-z0-9-]{0,47}$/
// added: when the realm opened, Chicago time to the minute (/worlds' RECENT); line: its one line there.
export const ADDED = /^20\d\d-[01]\d-[0-3]\dT[0-2]\d:[0-5]\d$/
export const CLOCK = /^(cycle|real|fixed:(0(\.\d{1,4})?|1(\.0{1,4})?))$/
// What coax wgsl writes into a realm's WGSL from the style presets: a painterly, photoreal, flat or
// anime realm draws with it. Required once metaverse/style.coax exists (as-presets).
export const STYLE_BLOCK = "// ---- generated: metaverse/style.coax"
// A module coax wgsl writes whole carries the style as its own functions (coax/web/wgsl-module.coax).
export const STYLE_MODULE = "fn coax_p_metaverse_style_"
// Or the module's Coax source (its first line names it, from the repo root) loads the presets,
// for a flat realm that draws with its own light and calls none of them (Orbit League).
const MODULE_SOURCE = /^\/\/ ---- generated: (\S+\.coax) \(the module\)/
export function styled(text: string, root = join(import.meta.dir, "../..")): boolean {
  if (text.includes(STYLE_BLOCK) || text.includes(STYLE_MODULE)) return true
  const m = MODULE_SOURCE.exec(text)
  if (!m || m[1].includes("..")) return false
  const src = join(root, m[1])
  return existsSync(src) && /#load "[^"]*verse\/style\.coax";/.test(readFileSync(src, "utf8"))
}
const STYLE_SOURCE = join(import.meta.dir, "../../metaverse/style.coax")
const PACKS = join(import.meta.dir, "../../metaverse/packs")
// Painterly realms whose pack isn't painted yet: each leaves this set with its pack task
// (Everglade: eg-paint-pack, #f300). A pack that names a style never passes by it.
const PACK_STYLE_PENDING = new Set(["everglade"])
/** The table, checked: unique ids, slugs and slots; an open world has a realm and a realm a module,
 *  a style (an anime one maybe a variant), its zones' ids once each, and a clock; a painterly, photoreal, flat or
 *  anime realm's WGSL has the style block. */
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
    once(`slug ${r.slug}`); if (!r.place_of) once(`slot ${r.slot}`)
    if (r.slot_was) once(`slot ${r.slot_was}`)
    if (!STYLES.includes(r.style)) throw new Error(`realms.json: ${r.slug}: style is one of ${STYLES.join(", ")}, not ${JSON.stringify(r.style)}`)
    variant(r.slug, r.style, r.variant)
    const zones = new Set<string>()
    for (const z of r.zones ?? []) {
      if (!ID.test(z?.id) || (z.style !== undefined && !STYLES.includes(z.style))) throw new Error(`realms.json: ${r.slug}: zone ${JSON.stringify(z?.id)}: an id and a style (${STYLES.join(", ")})`)
      if (zones.has(z.id)) throw new Error(`realms.json: ${r.slug}: zone ${z.id} twice`)
      zones.add(z.id)
      variant(`${r.slug}: zone ${z.id}`, z.style ?? r.style, z.variant)
    }
    if (typeof r.clock !== "string" || !CLOCK.test(r.clock)) throw new Error(`realms.json: ${r.slug}: clock is cycle, real or fixed:T (T 0 to 1), not ${JSON.stringify(r.clock)}`)
    if (styleBlock && [r.style, ...(r.zones ?? []).map((z) => z.style)].some((st) => st === "painterly" || st === "photoreal" || st === "flat" || st === "anime")) {
      // the shader the page loads: the one its row names (metaverse-engine/render/page-render.coax data-wgsl), else its module's
      const named = /data-wgsl="\/([^"]+)"/.exec(renderAttributes(r.slug, r.module))?.[1]
      const wgsl = join(site, named ?? `${r.module}.wgsl`)
      if (!existsSync(wgsl) || !styled(readFileSync(wgsl, "utf8"))) throw new Error(`realms.json: ${r.slug} is drawn ${r.style === "lowpoly" ? "partly painterly or photoreal" : r.style} but ${r.module}.wgsl has no style block (coax/web/wgsl-block.coax metaverse/style.coax ... --into it)`)
    }
    // its pack's style (metaverse/packs/<pack>.json "style", none is lowpoly) is one the realm draws in
    for (const a of r.assets) {
      const pack = /^pack:.*\/([a-z0-9-]+)\.(?:pack|vpk3)$/.exec(a)?.[1]
      const manifest = pack && join(packs, `${pack}.json`)
      if (!manifest || !existsSync(manifest)) continue
      const declared = JSON.parse(readFileSync(manifest, "utf8")).style, ps = declared ?? "lowpoly"
      const drawn = [r.style, ...(r.zones ?? []).map((z) => z.style ?? r.style)]
      if (!drawn.includes(ps) && !(declared === undefined && PACK_STYLE_PENDING.has(r.slug)))
        throw new Error(`realms.json: ${r.slug} is drawn ${drawn.join(" and ")} but its pack ${pack} is ${ps} (metaverse/packs/${pack}.json "style")`)
    }
    // its ruleset (metaverse/rules/<ruleset>.coax): a realm without one casts nothing
    if (r.share !== undefined && !(typeof r.share === "string" && r.share.length >= 20 && r.share.length <= 200)) throw new Error(`realms.json: ${r.slug}: share is a line of 20 to 200 characters`)
    if (r.ruleset !== undefined && !RULESETS.includes(r.ruleset)) throw new Error(`realms.json: ${r.slug}: ruleset is one of ${RULESETS.join(", ")}, not ${JSON.stringify(r.ruleset)}`)
    if (w.kind === "open" && w.listed !== false && (typeof r.added !== "string" || !ADDED.test(r.added)))
      throw new Error(`realms.json: ${r.slug}: a listed realm needs added, when it opened (YYYY-MM-DDTHH:MM, Chicago)`)
    if (r.added !== undefined && !ADDED.test(r.added)) throw new Error(`realms.json: ${r.slug}: added is YYYY-MM-DDTHH:MM`)
    if (w.kind === "open" && w.listed !== false && (typeof r.line !== "string" || !/^[^|\n"\\]{1,60}$/.test(r.line)))
      throw new Error(`realms.json: ${r.slug}: a listed realm needs line, its one line on /worlds (at most 60, no | or ")`)
    for (const a of r.assets) if (!/^[a-z]+:\S+$/.test(a)) throw new Error(`realms.json: ${r.slug}: asset ${a}`)
    if (r.short !== undefined && (!/^\/[a-z][a-z0-9-]{1,31}$/.test(r.short) || ["/worlds", "/realms", "/realm", "/coaxial", "/verse", "/metaverse"].includes(r.short))) throw new Error(`realms.json: ${r.slug}: short is a free top-level path such as /druid, not ${JSON.stringify(r.short)}`)
    if (r.short !== undefined) once(`short ${r.short}`)
    const g = r.golden
    if (!g || !(g.lit > 0 && g.lit < 1) || !(g.settle >= 0 && g.settle <= 20000) || !(Number.isInteger(g.blocks) && g.blocks >= 0)) throw new Error(`realms.json: ${r.slug}: golden needs lit (0 to 1), settle (ms) and blocks`)
  }
  for (const w of list) if (w.realm?.domain) {
    domainChecked(w.realm)
    // create: the place in this realm whose studio /play opens when the browser keeps no character yet
    const c = w.realm.domain.create
    if (c !== undefined && !list.some((x) => x.realm?.slug === c && x.realm.place_of === w.realm!.slug && x.realm.module === w.realm!.module))
      throw new Error(`realms.json: ${w.realm.slug}: domain: create is a place of it (place_of) on its module`)
  }
  // a place in another realm is that realm's page: its module and its slot
  for (const w of list) {
    const r = w.realm
    if (!r?.place_of) continue
    const home = list.find((o) => o.realm?.slug === r.place_of && !o.realm.place_of)?.realm
    if (!home || home.module !== r.module || home.slot !== r.slot) throw new Error(`realms.json: ${r.slug}: place_of ${r.place_of} is a realm with the same module and slot`)
  }
  return list
}

/** A realm's or zone's variant: none, or one of ANIME_VARIANTS when what it names is drawn anime. */
function variant(what: string, style: string, v: unknown) {
  if (v === undefined) return
  if (style !== "anime") throw new Error(`realms.json: ${what}: a variant is anime's, but it is drawn ${style}`)
  if (!ANIME_VARIANTS.includes(v as AnimeVariant)) throw new Error(`realms.json: ${what}: variant is one of ${ANIME_VARIANTS.join(", ")}, not ${JSON.stringify(v)}`)
}
// A zone in data-zones: "id:style", and for an anime zone "id:anime:variant" (its own, the realm's, or pastoral).
export const zoneFact = (r: Realm) => (z: Zone) => {
  const st = z.style ?? r.style
  return st === "anime" ? `${z.id}:anime:${z.variant ?? r.variant ?? "pastoral"}` : `${z.id}:${st}`
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
    `PLANET_PATH :: ${col("string", (w) => lit(w.realm ? `/worlds/${w.realm.slug}` : ""))};`,
    "// Its art style (docs/metaverse/art-styles.md; a world without a realm is lowpoly) and its clock.",
    "PLANET_LOWPOLY :: 0; PLANET_PAINTERLY :: 1; PLANET_PHOTOREAL :: 2; PLANET_FLAT :: 3; PLANET_ANIME :: 4;   // metaverse/style-preset.coax STYLE_*",
    `PLANET_STYLE :: ${col("int", (w) => `PLANET_${(w.realm?.style ?? "lowpoly").toUpperCase()}`)};`,
    `PLANET_CLOCK :: ${col("string", (w) => lit(w.realm?.clock ?? ""))};`,
    "",
  ].join("\n")
}

// /worlds (worlds-web.coax): the listed realms, in the table's order.
export function listed(list: World[]): Realm[] {
  return list.filter((w) => w.kind === "open" && w.listed !== false && w.realm).map((w) => w.realm!)
}
// The open realms /worlds doesn't list: someone who may edit one still finds it there, with its name, line and picture.
export function unlisted(list: World[]): Realm[] {
  return list.filter((w) => w.kind === "open" && w.listed === false && w.realm).map((w) => w.realm!)
}
// A realm's picture in a list: its link preview's card (a hand-made one, else the one og-render.ts renders) when the
// site has that file, as a path here; none for the site's default card, which shows nothing of the realm.
export function picture(r: Realm, site = SITE): string {
  const card = r.card ?? renderedCard(`realms/${r.slug}`)
  if (card === DEFAULT_IMAGE || !/^\/[a-z0-9/_.-]+\.(jpg|png|webp)$/.test(card)) return ""
  return existsSync(join(site, card.replace(/^\/worlds(?=\/)/, "/realms"))) ? card : ""
}
const listRow = (r: Realm) => `${r.slug}|${r.title}|${r.line ?? ""}|${r.added ?? ""}|${r.style}|${picture(r)}\n`
export function listCoax(list: World[]): string {
  return [
    "// Written by coax/web/realms.ts from realms.json: change the table, not this file.",
    "// /worlds' rows (worlds-web.coax), the listed realms in the table's order:",
    "// slug | title | its one line | when it opened (Chicago, YYYY-MM-DDTHH:MM) | art style | its picture (a path here, or none)",
    `REALMS_ROWS :: ${JSON.stringify(listed(list).map(listRow).join(""))};`,
    "// The open realms it doesn't list, the same columns: shown only to someone who may edit one.",
    `REALMS_UNLISTED :: ${JSON.stringify(unlisted(list).map(listRow).join(""))};`,
    "",
  ].join("\n")
}
const MONTHS = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"]
export function listMarkdown(list: World[]): string {
  const day = (a: string) => `${MONTHS[Number(a.slice(5, 7)) - 1]} ${Number(a.slice(8, 10))}, ${a.slice(0, 4)}`
  const rows = listed(list).sort((a, b) => (a.added! < b.added! ? 1 : a.added! > b.added! ? -1 : 0))
  return [
    "# Worlds",
    "",
    "Worlds on the OpenAgents Metaverse you play in the browser, newest first. The page sorts them by most recent, most visited, most played and A to Z; visits leave out our own team and automation.",
    "",
    ...rows.map((r) => `- [${r.title}](https://openagents.com/worlds/${r.slug}): ${r.line}. Opened ${day(r.added!)}.`),
    "",
    "Also: [the Metaverse](https://openagents.com/metaverse), [build your own world](https://openagents.com/worlds/new).",
    "",
  ].join("\n")
}
// /worlds: a page of the kit beside the site's side (worlds-web.coax, docs/app-shell.md). Until its module draws,
// and for agents, search engines and readers without it, the page is its name and the worlds' links.
export function listPage(list: World[]): string {
  const links = listed(list).map((r) => `<li><a href="/worlds/${r.slug}">${esc(r.title)}</a></li>`).join("\n")
  return `<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width,initial-scale=1">
<title>Worlds · OpenAgents</title>
<meta name="description" content="Every world on the OpenAgents Metaverse you can play in the browser, sorted by most popular, most recent, most played or A to Z.">
<meta property="og:type" content="website">
<meta property="og:site_name" content="OpenAgents">
<meta property="og:title" content="Worlds">
<meta property="og:description" content="Every world on the OpenAgents Metaverse you can play in the browser.">
<meta property="og:url" content="https://openagents.com/worlds">
<meta name="twitter:card" content="summary_large_image">
<meta name="theme-color" content="#000000">
<link rel="icon" type="image/svg+xml" href="/favicon.svg">
<link rel="alternate" type="text/markdown" href="/worlds/index.md">
<style>
html,body{margin:0;background:#000;color:#f2f2f2}
body{font-family:ui-sans-serif,system-ui,-apple-system,"Segoe UI",sans-serif;font-size:14px;line-height:1.6}
</style>
</head>
<body>
<main id="page-root">
<h1>Worlds</h1>
<ul>
${links}
</ul>
<p><a href="/worlds/index.md">Worlds as text</a></p>
</main>
<p id="side-root"></p>
<script type="module" src="/coaxial/dom-host.js" data-app="worlds-web" data-root="page-root"></script>
<script type="module" src="/coaxial/dom-host.js?app=chat-web&amp;root=side-root&amp;path=/side/worlds"></script>
</body>
</html>
`
}

// A realm a maker can claim has a tip page (realms/<slug>/tip); only its page asks oa-api who made it,
// so the others don't log a 404 for every visitor (host.js, data-maker).
export const hasMaker = (slug: string, site = SITE) => existsSync(join(site, "realms", slug, "tip", "index.html"))
const esc = (s: string) => s.replace(/&/g, "&amp;").replace(/"/g, "&quot;").replace(/</g, "&lt;")
// A page opened through Everglade's portal on the homepage (?via=portal, newland-portal.coax): until
// the engine draws, the loading screen is the portal's light (everglade/scene/gpu/portal.coax's colours at the
// hand-over's scale), still moving: a slow swirl and rings running out, on the compositor, so it
// moves while the engine compiles. No black panel, but the bar stays, low on a dark strip, so a
// slow first load shows real progress (owner, 2026-11-09); the engine then
// draws the same light and opens the world through it (everglade/client/create.coax). A failure still
// says what failed.
const PORTAL_ARRIVAL_CSS = `html.portal.gpu,html.portal.gpu body{background:#86c39d}
html.portal.gpu #coaxial-load{overflow:hidden;background:radial-gradient(ellipse at 50% 50%,#a1e2be 0%,#91d5b1 40%,#87cfac 75%,#7fcaaa 100%)}
html.portal.gpu #coaxial-load::before,html.portal.gpu #coaxial-load::after{content:"";position:absolute;left:50%;top:50%;width:250vmax;height:250vmax;margin:-125vmax 0 0 -125vmax;pointer-events:none;will-change:transform,opacity}
html.portal.gpu #coaxial-load::before{background:conic-gradient(from 0deg,rgba(60,160,140,.42),rgba(205,245,220,.30),rgba(55,140,130,.42),rgba(240,215,150,.26),rgba(60,160,140,.42),rgba(190,240,225,.30),rgba(60,160,140,.42));filter:blur(28px);animation:portal-swirl 4s linear infinite}
html.portal.gpu #coaxial-load::after{background:radial-gradient(circle at 50% 50%,rgba(240,225,170,.35) 0,rgba(240,225,170,0) 9vmax),repeating-radial-gradient(circle at 50% 50%,rgba(205,248,228,0) 0,rgba(205,248,228,0) 2.4vmax,rgba(205,248,228,.22) 3.4vmax,rgba(205,248,228,0) 4.4vmax),conic-gradient(from 90deg,rgba(40,120,115,.22),rgba(40,120,115,0),rgba(40,120,115,.22),rgba(40,120,115,0),rgba(40,120,115,.22));animation:portal-rings 2.4s linear infinite}
html.portal.gpu #coaxial-load:not([data-state=fail]) .lf{position:absolute;z-index:1;left:50%;bottom:max(40px,8vh);transform:translateX(-50%);width:min(360px,calc(100% - 32px));padding:12px 16px 4px;background:rgba(8,28,24,.55);clip-path:none;border-radius:2px}
html.portal.gpu #coaxial-load:not([data-state=fail]) .lf::before,html.portal.gpu #coaxial-load:not([data-state=fail]) :is(.c1,.c2,.ln){display:none}
html.portal.gpu #coaxial-load .lt{color:#e6f4ec}
@keyframes portal-swirl{from{transform:rotate(0deg) scale(1)}50%{transform:rotate(180deg) scale(1.12)}to{transform:rotate(360deg) scale(1)}}
@keyframes portal-rings{0%{transform:rotate(0deg) scale(1)}50%{transform:rotate(-90deg) scale(1.16)}100%{transform:rotate(-180deg) scale(1)}}
@media (prefers-reduced-motion:reduce){html.portal.gpu #coaxial-load::before,html.portal.gpu #coaxial-load::after{animation:none}}`

export function page(r: Realm, isListed = true): string {
  const url = `https://openagents.com/worlds/${r.slug}`, t = esc(r.title), d = esc(r.description), share = esc(r.share ?? brief(r.description))
  // its link preview (docs/web/og-tags.md): its hand-made card, else the one og-render.ts renders from a still of it
  const card = `https://openagents.com${r.card ?? renderedCard(`realms/${r.slug}`)}`
  const alt = esc(r.card === DEFAULT_IMAGE ? "OpenAgents" : r.card ? r.alt : `${r.title}: a still of the world, with its name`)
  const slot = ` data-realm="${r.place_of ?? r.slug}" data-slot="${r.slot}"` + (r.slot_was ? ` data-slot-was="${r.slot_was}"` : "")
    + (r.page ? ` data-page="${r.page}"` : "")
    + (r.ui_font_src ? ` data-ui-font-src="${esc(r.ui_font_src)}" data-ui-font-family="${esc(r.ui_font_family ?? "RealmUIFont")}"` : "")
    // a HUD style's title face (docs/metaverse/hud-styles.md): the atlas's second face, beside the reading face
    + (r.ui_font_src && r.ui_title_font_src ? ` data-ui-title-font-src="${esc(r.ui_title_font_src)}" data-ui-title-font-family="${esc(r.ui_title_font_family ?? "RealmUITitleFont")}" data-ui-title-font-weight="${esc(r.ui_title_font_weight ?? "400")}"` + (r.ui_title_font_scale ? ` data-ui-title-font-scale="${esc(String(r.ui_title_font_scale))}"` : "") : "")
    + (r.voice ? ` data-voice="${r.voice}"` : "")
    + (r.ruleset ? ` data-ruleset="${r.ruleset}"` : "")
    + (hasMaker(r.slug) ? ` data-maker="1"` : "")
    + (r.uploads ? ` data-uploads="${esc(r.uploads)}"` : "")
    + (r.timing ? ` data-timing="1"` : "")   // GPU timestamps for its HUD (coax/web/host.js, message 610)
    + ` data-style="${r.style}"` + (r.style === "anime" ? ` data-variant="${r.variant ?? "pastoral"}"` : "") + ` data-clock="${r.clock}"`
    + (r.zones?.length ? ` data-zones="${r.zones.map(zoneFact(r)).join(" ")}"` : "")
  const assets = r.assets.length ? ` data-assets="\n${r.assets.map((a) => `  ${a}`).join("\n")}"` : ""
  return `<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width,initial-scale=1,viewport-fit=cover">
<title>${t} · OpenAgents</title>
<meta name="description" content="${d}">${isListed ? "" : `
<meta name="robots" content="noindex, nofollow">`}
<meta property="og:type" content="website">
<meta property="og:site_name" content="OpenAgents">
<meta property="og:title" content="${t}">
<meta property="og:description" content="${share}">
<meta property="og:url" content="${url}">
<meta property="og:image" content="${card}">
<meta property="og:image:width" content="1200">
<meta property="og:image:height" content="630">
<meta property="og:image:alt" content="${alt}">
<meta name="twitter:card" content="summary_large_image">
<meta name="twitter:title" content="${t}">
<meta name="twitter:description" content="${share}">
<meta name="twitter:image" content="${card}">
<meta name="theme-color" content="#000000">
<link rel="icon" type="image/svg+xml" href="/favicon.svg">
<link rel="alternate" type="text/markdown" href="/worlds/${r.slug}/index.md">
<script>if ("gpu" in navigator) document.documentElement.classList.add("gpu")${r.arrival === "portal" ? `
if (/[?&]via=portal(&|$)/.test(location.search)) document.documentElement.classList.add("portal")` : ""}</script>
<style>
html,body{margin:0;height:100%;background:#000;color:#ededed}
body{overflow:hidden;font-family:ui-sans-serif,system-ui,-apple-system,"Segoe UI",Roboto,"Helvetica Neue",Arial,sans-serif;font-size:14px;line-height:1.5}
canvas{position:fixed;inset:0;width:100%;height:100%;touch-action:none;user-select:none;-webkit-user-select:none;-webkit-touch-callout:none}
canvas[data-cursor=pointer]{cursor:pointer}
canvas[data-cursor=crosshair]{cursor:crosshair}
canvas[data-cursor=none]{cursor:none}
main{position:relative;max-width:640px;margin:0 auto;padding:24px 16px}
html:not(.gpu) body{overflow:auto}
a{color:#ff8c1f}
/* When the GPU draws the page, the text stays for agents, search engines and screen readers only. */
.gpu main{position:absolute;width:1px;height:1px;overflow:hidden;clip-path:inset(50%);white-space:nowrap;padding:0}
${LOADER_CSS}${r.arrival === "portal" ? "\n" + PORTAL_ARRIVAL_CSS : ""}
</style>
</head>
<body>
<canvas id="coaxial" aria-hidden="true"></canvas>
${loaderMarkup(r.title, `/worlds/${r.slug}/index.md`)}
<main>
<h1>${t}</h1><p><a href="/worlds/${r.slug}/index.md">${t}</a></p>${existsSync(join(SITE, "realms", r.slug, "map.fallback")) ? "\n" + readFileSync(join(SITE, "realms", r.slug, "map.fallback"), "utf8").trimEnd() : ""}
</main>
<script type="module" src="/coaxial/host.js" data-app="${r.module}"${renderAttributes(r.slug, r.module)}${slot}${assets}></script>
${r.project_asset ? projectScript(r.module, r.project_asset) : ""}</body>
</html>
`
}

// ---- A realm's own domain (docs/metaverse/everglade-domain.md) ----
// What sits beside the generated pages, made by hand: the Markdown twin, the icon, the wordmark,
// the hero (wide and phone) and the link-preview card.
export const DOMAIN_HAND = ["index.md", "icon.svg", "wordmark.svg", "hero.jpg", "hero-phone.jpg", "card.jpg"]
const HOST = /^(?=.{4,253}$)([a-z0-9]([a-z0-9-]{0,61}[a-z0-9])?\.)+[a-z]{2,63}$/
function domainChecked(r: Realm) {
  const d = r.domain!, at = `realms.json: ${r.slug}: domain`
  if (typeof d.host !== "string" || !HOST.test(d.host) || /openagents/.test(d.host)) throw new Error(`${at}: host is a plain domain name, not ${JSON.stringify(d.host)}`)
  if (!/^[^<>"|\n]{1,40}$/.test(d.title ?? "")) throw new Error(`${at}: title, at most 40`)
  if (!/^[^<>"\n]{50,200}$/.test(d.description ?? "")) throw new Error(`${at}: description, 50 to 200 characters`)
  if (!/^[^<>"\n]{10,80}$/.test(d.tagline ?? "")) throw new Error(`${at}: tagline, one line of 10 to 80`)
  // entry: the domain's one address for play (/play), with no query: the front door opens it with ?via=portal
  if (!/^\/[a-z0-9-]+$/.test(d.entry ?? "")) throw new Error(`${at}: entry is a path on the domain itself, such as /play`)
  if (!Array.isArray(d.features) || d.features.length < 3 || d.features.length > 4 || d.features.some((f) => !/^[^<>"\n]{3,32}$/.test(f?.title ?? "") || !/^[^<>"\n]{10,90}$/.test(f?.line ?? "")))
    throw new Error(`${at}: features, 3 or 4 of a title (at most 32) and a line (at most 90)`)
  if (!Array.isArray(d.shots) || d.shots.length < 2 || d.shots.some((p) => !/^[a-z0-9-]+\.jpg$/.test(p?.src ?? "") || !/^[^<>"\n]{10,120}$/.test(p?.alt ?? "")))
    throw new Error(`${at}: shots, at least 2 JPEGs beside the pages, each with its alt`)
  // nothing on the domain's pages leads to openagents.com
  for (const v of [d.title, d.description, d.tagline, ...d.features.flatMap((f) => [f.title, f.line]), ...d.shots.map((p) => p.alt)])
    if (/openagents\.com/i.test(v)) throw new Error(`${at}: names openagents.com`)
}

// The domain's head: its own name, description, preview, icon and colour; nothing of OpenAgents.
function domainHead(r: Realm, title: string, path: string, robots: string): string {
  const d = r.domain!, base = `https://${d.host}`, dir = `/worlds/${r.slug}/domain`
  return `<meta charset="utf-8">
<meta name="viewport" content="width=device-width,initial-scale=1,viewport-fit=cover">
<title>${esc(title)}</title>
<meta name="description" content="${esc(d.description)}">${robots ? `\n<meta name="robots" content="${robots}">` : ""}
<link rel="canonical" href="${base}${path}">
<meta property="og:type" content="website">
<meta property="og:site_name" content="${esc(d.title)}">
<meta property="og:title" content="${esc(title)}">
<meta property="og:description" content="${esc(d.tagline)}">
<meta property="og:url" content="${base}${path}">
<meta property="og:image" content="${base}${dir}/card.jpg">
<meta property="og:image:width" content="1200">
<meta property="og:image:height" content="630">
<meta property="og:image:alt" content="${esc(d.title)}: ${esc(d.tagline)}">
<meta name="twitter:card" content="summary_large_image">
<meta name="twitter:title" content="${esc(title)}">
<meta name="twitter:description" content="${esc(d.tagline)}">
<meta name="twitter:image" content="${base}${dir}/card.jpg">
<meta name="theme-color" content="#0d1a10">
<link rel="icon" type="image/svg+xml" href="${dir}/icon.svg">
<link rel="alternate" type="text/markdown" href="${dir}/index.md">`
}

// The front door: a full-bleed hero (a capture of the world) with the wordmark, the tagline and
// PLAY NOW; the features; the screenshots; a footer. The entry page (d.entry) opens out of sight
// under it as soon as it is up, as zone-host.js opens a page in place: its engine, pack and first
// frame, then its world waits paused. PLAY NOW fades it in over the page, with no page load.
// Without WebGPU, or when the entry failed, PLAY NOW is a link to it.
function frontDoor(r: Realm): string {
  const d = r.domain!, dir = `/worlds/${r.slug}/domain`
  const features = d.features.map((f) => `<li class="in"><b>${esc(f.title)}</b><span>${esc(f.line)}</span></li>`).join("\n")
  const shots = d.shots.map((p) => `<li class="in"><img src="${dir}/${p.src}" alt="${esc(p.alt)}" width="960" height="510" loading="lazy" decoding="async"></li>`).join("\n")
  return `<!doctype html>
<html lang="en">
<head>
${domainHead(r, `${d.title}: ${d.tagline}`, "/", "")}
<link rel="preload" as="image" href="${dir}/hero.jpg" media="(min-width: 701px)">
<link rel="preload" as="image" href="${dir}/hero-phone.jpg" media="(max-width: 700px)">
<style>
:root{--ink:#f6efd9;--dim:#c9c1a8;--gold:#f1d38a;--deep:#0d1a10;--line:rgba(246,239,217,.14)}
*{box-sizing:border-box}
html{background:var(--deep);color:var(--ink);-webkit-text-size-adjust:100%}
body{margin:0;font:400 16px/1.5 ui-serif,Georgia,"Times New Roman",serif}
#door{position:relative;z-index:1;background:var(--deep)}
.hero{position:relative;min-height:100vh;min-height:100svh;display:flex;flex-direction:column;align-items:center;justify-content:flex-end;padding:0 16px max(9vh,56px);text-align:center;overflow:hidden;background:#3c5a2a url(${dir}/hero.jpg) center 40%/cover no-repeat}
.hero::after{content:"";position:absolute;inset:0;background:linear-gradient(180deg,rgba(13,26,16,.05) 0,rgba(13,26,16,.1) 45%,rgba(13,26,16,.78) 82%,var(--deep) 100%);pointer-events:none}
.hero>*{position:relative;z-index:1}
.mark{width:min(88vw,760px);height:auto;filter:drop-shadow(0 6px 18px rgba(0,0,0,.55))}
.tag{margin:6px 0 26px;font-size:clamp(17px,2.4vw,22px);color:var(--ink);text-shadow:0 2px 10px rgba(0,0,0,.7)}
.play{display:inline-block;min-width:min(78vw,300px);padding:18px 40px;border:1px solid #fff3c9;border-radius:6px;background:linear-gradient(180deg,#f6dc93,#c9913f);color:#25170a;font:700 20px/1 ui-sans-serif,system-ui,-apple-system,"Segoe UI",Roboto,sans-serif;letter-spacing:.16em;text-decoration:none;box-shadow:0 8px 30px rgba(0,0,0,.45),inset 0 1px 0 rgba(255,255,255,.6);transition:transform .2s ease,box-shadow .2s ease,filter .2s ease}
.play:hover,.play:focus-visible{transform:translateY(-2px);filter:brightness(1.06);box-shadow:0 12px 36px rgba(0,0,0,.5),0 0 0 3px rgba(246,220,147,.35);outline:none}
.free{margin:14px 0 0;font:500 13px/1.4 ui-sans-serif,system-ui,sans-serif;letter-spacing:.08em;color:var(--dim);text-shadow:0 1px 6px rgba(0,0,0,.8)}
section{max-width:1120px;margin:0 auto;padding:56px 16px}
h2{margin:0 0 22px;font-weight:400;font-size:clamp(24px,3.2vw,32px);text-align:center;color:var(--gold)}
ul{list-style:none;margin:0;padding:0}
.feat{display:grid;grid-template-columns:repeat(4,1fr);gap:16px}
.feat li{padding:22px 18px;border:1px solid var(--line);border-radius:8px;background:rgba(246,239,217,.03)}
.feat b{display:block;margin-bottom:6px;font-size:20px;font-weight:400;color:var(--gold)}
.feat span{color:var(--dim);font-size:15px}
.shots{display:grid;grid-template-columns:repeat(${d.shots.length},1fr);gap:14px}
.shots img{display:block;width:100%;height:auto;border-radius:8px;border:1px solid var(--line)}
.strip{padding-top:0}.again{text-align:center;padding-top:8px}
footer{padding:28px 16px 36px;text-align:center;color:#8f8a78;font:400 12px/1.6 ui-sans-serif,system-ui,sans-serif;letter-spacing:.04em;border-top:1px solid var(--line)}
@media (max-width:700px){.hero{background-image:url(${dir}/hero-phone.jpg)}.feat{grid-template-columns:1fr 1fr;gap:10px}.feat li{padding:16px 14px}.feat b{font-size:18px}
.shots{display:flex;overflow-x:auto;scroll-snap-type:x mandatory;margin:0 -16px;padding:0 16px 6px}.shots li{flex:0 0 84%;scroll-snap-align:center}section{padding:40px 16px}}
/* Nothing pops: everything fades in, the hero at once, the rest as it scrolls into view. */
.hero>*{animation:up .9s ease-out both}.hero>:nth-child(2){animation-delay:.25s}.hero>:nth-child(3){animation-delay:.5s}.hero>:nth-child(4){animation-delay:.7s}
@keyframes up{from{opacity:0;transform:translateY(10px)}to{opacity:1;transform:none}}
.js .in{opacity:0;transform:translateY(12px);transition:opacity .8s ease-out,transform .8s ease-out}
.js .in.seen{opacity:1;transform:none}
#door.out{opacity:0;transition:opacity .5s ease-in}
@media (prefers-reduced-motion:reduce){.hero>*{animation:none}.js .in{opacity:1;transform:none;transition:none}}
</style>
</head>
<body>
<div id="door">
<header class="hero">
<h1 style="margin:0"><img class="mark" src="${dir}/wordmark.svg" alt="${esc(d.title)}" width="760" height="100"></h1>
<p class="tag">${esc(d.tagline)}</p>
<a class="play" href="${d.entry}" data-play>PLAY NOW</a>
<p class="free">Free in your browser. No download.</p>
</header>
<section>
<h2 class="in">Your adventure starts in the glade</h2>
<ul class="feat">
${features}
</ul>
</section>
<section class="strip">
<ul class="shots">
${shots}
</ul>
</section>
<section class="again"><a class="play in" href="${d.entry}" data-play>PLAY NOW</a></section>
<footer>${esc(d.title)} · Hosted by OpenAgents</footer>
</div>
<script>
// Firm reason: 1 (browser glue: the entry page opened in place under the front door, as zone-host.js does)
(() => {
  const ENTRY = ${JSON.stringify(d.entry)}, TITLE = ${JSON.stringify(d.title)};   // the address bar says ENTRY all the way in
  const door = document.getElementById("door"), root = document.documentElement;
  root.classList.add("js");
  // What the front door records (analytics.coax's batch shape, beaconed to the same ingest, in this
  // tab's session as the game's host.js names it, page everglade2): its page view, a session start for
  // a new tab, and each PLAY NOW, with or without WebGPU. No words, nothing personal: the anonymous
  // ids, the screen, where the visit came from (a site and path, no query).
  const id = (store, key) => { const make = () => [...crypto.getRandomValues(new Uint8Array(8))].map((b) => b.toString(16).padStart(2, "0")).join("");
    try { const v = store.getItem(key); if (v) return [v, false]; const n = make(); store.setItem(key, n); return [n, true]; } catch { return [make(), true]; } };
  const [tab, fresh] = id(sessionStorage, "coaxial.tab"), [pid] = id(localStorage, "coaxial.visitor");
  const gpu = "gpu" in navigator, mobile = matchMedia("(pointer: coarse)").matches;
  const track = (ev, p) => {
    if (!/^[0-9a-f]{8,64}$/.test(tab) || !navigator.sendBeacon) return;
    const body = { sid: "c" + tab, events: [{ ev, page: "everglade2", ct: Date.now(), p }] };
    if (navigator.webdriver) body.auto = "webdriver";
    try { navigator.sendBeacon("/api/analytics/events", new Blob([JSON.stringify(body)], { type: "text/plain" })); } catch {}
  };
  let ref = ""; try { if (document.referrer) { const r = new URL(document.referrer); ref = r.host === location.host ? r.pathname : r.host; } } catch {}
  if (fresh) { let tz = ""; try { tz = Intl.DateTimeFormat().resolvedOptions().timeZone || ""; } catch {}
    track("session-start", { pid, rev: "door", mobile, gpu, sw: screen.width, sh: screen.height, vw: innerWidth, vh: innerHeight, dpr: devicePixelRatio || 1, lang: navigator.language || "", tz, ref, entry: "everglade2", landing: true }); }
  track("page-view", { pid, from: ref, path: location.pathname, ...(location.search.length > 1 ? { q: location.search.slice(1, 81) } : {}) });
  const io = "IntersectionObserver" in window ? new IntersectionObserver((es) => { for (const e of es) if (e.isIntersecting) { e.target.classList.add("seen"); io.unobserve(e.target); } }, { rootMargin: "0px 0px -8% 0px" }) : null;
  for (const el of document.querySelectorAll(".in")) io ? io.observe(el) : el.classList.add("seen");
  let z = null;
  const ev = globalThis.everglade = { state: () => (z ? z.state : -1) };
  const look = () => {
    let win = null, doc = null;
    try { win = z.frame.contentWindow; doc = z.frame.contentDocument; } catch { return 2; }
    if (!win || !doc || !doc.documentElement) return 0;
    const load = doc.getElementById("coaxial-load");
    if (load && load.dataset.state === "fail") return 2;
    if (z.loaded && !doc.querySelector("script[data-app]")) return 2;
    const up = !load || load.classList.contains("gone") || Number(load.getAttribute("aria-valuenow")) >= 100;
    return win.coaxial && win.coaxial.sceneReady && up ? 1 : 0;
  };
  const poll = () => {
    if (!z || z.state !== 0) return;
    z.state = look();
    if (z.state === 1) { try { z.frame.contentWindow.coaxial.setWorldPaused(true); z.paused = true; } catch {} }
    if (z.state !== 0) clearInterval(z.timer);
  };
  // The entry, loading out of sight under the front door (which is opaque over it).
  const warm = () => {
    if (z || !("gpu" in navigator)) return;
    const frame = document.createElement("iframe");
    frame.dataset.zone = "1"; frame.title = TITLE; frame.tabIndex = -1; frame.inert = true;
    frame.setAttribute("aria-hidden", "true");
    frame.allow = "fullscreen; autoplay; microphone; camera; clipboard-read; clipboard-write; gamepad; screen-wake-lock";
    frame.style.cssText = "position:fixed;inset:0;width:100%;height:100%;border:0;margin:0;padding:0;background:#000;pointer-events:none;opacity:0";
    z = { frame, state: 0, loaded: false, paused: false, timer: 0, t0: performance.now() };
    frame.addEventListener("load", () => { z.loaded = true; poll(); });
    frame.src = ENTRY + "?via=portal";
    document.body.prepend(frame);
    z.timer = setInterval(poll, 150);
  };
  // PLAY NOW: the entry fades in where it is (its own loading screen, if it is not up yet), takes
  // the address, the title and the input; the front door goes. Otherwise it is a plain link.
  const play = (e) => {
    track("play-now-click", { id: "door", at: e.currentTarget.closest(".again") ? "foot" : "hero", gpu });
    if (e.button || e.metaKey || e.ctrlKey || e.shiftKey) return;
    warm();
    if (!z || z.state === 2) return;
    let win = null; try { win = z.frame.contentWindow; } catch {}
    if (!win) return;
    e.preventDefault();
    clearInterval(z.timer);
    ev.pressed = performance.now();
    try { if (z.paused) win.coaxial.setWorldPaused(false); } catch {}
    try { win.coaxial.shown && win.coaxial.shown(); } catch {}
    const frame = z.frame;
    frame.inert = false; frame.removeAttribute("aria-hidden"); frame.tabIndex = 0;
    frame.style.zIndex = "3"; frame.style.pointerEvents = "auto";
    const still = matchMedia("(prefers-reduced-motion: reduce)").matches;
    let done = false;
    const over = () => {
      if (done) return; done = true;
      door.remove(); document.body.style.overflow = "hidden";
      try { history.pushState(null, "", ENTRY); } catch {}
      document.title = TITLE;
      addEventListener("popstate", () => location.reload());
      frame.focus(); try { win.focus(); } catch {}
      ev.shown = performance.now();
    };
    door.classList.add("out");
    requestAnimationFrame(() => requestAnimationFrame(() => {
      if (still) { frame.style.opacity = "1"; over(); return; }
      frame.style.transition = "opacity 500ms ease-in-out"; frame.style.opacity = "1";
      frame.addEventListener("transitionend", over, { once: true });
      setTimeout(over, 700);
    }));
  };
  for (const a of document.querySelectorAll("[data-play]")) { a.addEventListener("click", play); a.addEventListener("pointerenter", warm, { once: true }); a.addEventListener("focus", warm, { once: true }); }
  // as soon as the front door has painted
  const soon = () => setTimeout(warm, 300);
  if (document.readyState === "complete") soon(); else addEventListener("load", soon, { once: true });
})();
</script>
</body>
</html>
`
}

// /play: the realm's own page under the domain's name, icon and preview: one address for all of it.
// With a create place (d.create), the page also carries that place's arrival and interface font: the
// module opens its studio when this browser keeps no character yet, else the world where you stood.
function domainPlay(r: Realm, list: World[] = worlds()): string {
  const d = r.domain!, c = d.create ? list.find((w) => w.realm?.slug === d.create)?.realm : undefined
  const html = page(c ? { ...r, arrival: c.arrival, ui_font_src: c.ui_font_src, ui_font_family: c.ui_font_family, ui_title_font_src: c.ui_title_font_src, ui_title_font_family: c.ui_title_font_family, ui_title_font_weight: c.ui_title_font_weight } : r, false)
  const head = html.slice(html.indexOf("<head>") + 7, html.indexOf("<script>"))
  return html.replace(head, domainHead(r, d.title, "/play", "noindex, nofollow") + "\n")
    .replaceAll(`/worlds/${r.slug}/index.md`, `/worlds/${r.slug}/domain/index.md`)
    .replace(/(<b class="ln">)[^<]*(<\/b>)/, `$1${esc(d.title)}$2`).replace(/aria-label="Loading [^"]*"/, `aria-label="Loading ${esc(d.title)}"`)
    .replace(/READ ABOUT [^<]*/, `READ ABOUT ${esc(d.title)}`).replace(/<h1>[^]*?<\/h1><p><a href="([^"]+)">[^<]*<\/a>/, `<h1>${esc(d.title)}</h1><p><a href="$1">${esc(d.title)}</a>`)
}

// The domain's 404: any other address there (the bucket's own 404 page hands over to it on this host).
function domainNotFound(r: Realm): string {
  const d = r.domain!, dir = `/worlds/${r.slug}/domain`
  return `<!doctype html>
<html lang="en">
<head>
${domainHead(r, `Not found · ${d.title}`, "/", "noindex")}
<style>
html{background:#0d1a10;color:#f6efd9}
body{margin:0;min-height:100vh;min-height:100svh;display:flex;flex-direction:column;align-items:center;justify-content:center;padding:0 16px;text-align:center;font:400 17px/1.5 ui-serif,Georgia,serif;background:linear-gradient(180deg,rgba(13,26,16,.55),#0d1a10 85%),url(${dir}/hero-phone.jpg) center/cover}
img{width:min(80vw,520px);height:auto;filter:drop-shadow(0 6px 18px rgba(0,0,0,.55));animation:in .8s ease-out both}
p{margin:10px 0 26px;color:#c9c1a8;animation:in .8s .2s ease-out both}
a{display:inline-block;padding:14px 30px;border:1px solid #fff3c9;border-radius:6px;background:linear-gradient(180deg,#f6dc93,#c9913f);color:#25170a;font:700 16px/1 ui-sans-serif,system-ui,sans-serif;letter-spacing:.14em;text-decoration:none;animation:in .8s .4s ease-out both}
@keyframes in{from{opacity:0}to{opacity:1}}
@media (prefers-reduced-motion:reduce){img,p,a{animation:none}}
</style>
</head>
<body>
<h1 style="margin:0"><img src="${dir}/wordmark.svg" alt="${esc(d.title)}" width="520" height="68"></h1>
<p>404: this path leads nowhere in the forest.</p>
<a href="/">BACK TO ${esc(d.title.toUpperCase())}</a>
</body>
</html>
`
}

export function domainPages(r: Realm, list: World[] = worlds()): [string, string][] {
  return [["index.html", frontDoor(r)], ["play/index.html", domainPlay(r, list)], ["404.html", domainNotFound(r)]]
}

// The ?project= swap for a realm's project_asset: an inline script after the host's module script runs
// before it (module scripts wait for the document), so the host reads the swapped address.
function projectScript(module: string, asset: string): string {
  const m = /^\/private\/[a-z0-9-]+(\.[a-z]+)$/.exec(asset)
  if (!m) throw new Error(`realms.ts: project_asset ${asset} is not /private/NAME.EXT`)
  return `
<script>
// Firm reason: 1 (bind an owner-local project asset before the browser host starts).
const project = new URLSearchParams(location.search).get("project");
if (project && /^[a-z0-9-]{1,64}$/.test(project)) {
  const entry = document.querySelector("script[data-app=${module}]");
  entry.dataset.assets = entry.dataset.assets.replace("${asset}", "/private/" + project + "${m[1]}");
}
</script>
`
}

if (import.meta.main) {
  const check = process.argv.includes("--check")
  const list = worlds()
  const out: [string, string][] = [[join(SITE, "realms.coax"), coax(list)], [join(SITE, "realms", "list.coax"), listCoax(list)],
    [join(SITE, "realms", "index.md"), listMarkdown(list)], [join(SITE, "realms", "index.html"), listPage(list)]]
  for (const w of list) if (w.realm) out.push([join(SITE, "realms", w.realm.slug, "index.html"), page(w.realm, w.listed !== false)])
  // short: the same page at a top-level path (openagents.com/druid); the site bucket serves DIR/index.html for /DIR
  for (const w of list) if (w.realm?.short) out.push([join(SITE, w.realm.short.slice(1), "index.html"), page(w.realm, w.listed !== false)])
  for (const w of list) if (w.realm?.domain) out.push(...domainPages(w.realm, list).map(([f, t]) => [join(SITE, "realms", w.realm!.slug, "domain", f), t] as [string, string]))
  const behind: string[] = []
  for (const [file, text] of out) {
    if (existsSync(file) && readFileSync(file, "utf8") === text) continue
    if (check) { behind.push(file); continue }
    mkdirSync(join(file, ".."), { recursive: true })
    writeFileSync(file, text)
  }
  for (const w of list) if (w.realm && !existsSync(join(SITE, "realms", w.realm.slug, "index.md"))) behind.push(`realms/${w.realm.slug}/index.md (write it by hand)`)
  for (const w of list) for (const f of w.realm?.domain ? [...DOMAIN_HAND, ...w.realm.domain.shots.map((p) => p.src)] : []) if (!existsSync(join(SITE, "realms", w.realm!.slug, "domain", f))) behind.push(`realms/${w.realm!.slug}/domain/${f} (made by hand)`)
  if (behind.length) { console.error(`realms.ts: behind realms.json:\n  ${behind.join("\n  ")}${check ? "\n  run: bun coax/web/realms.ts" : ""}`); process.exit(1) }
}
