// Link previews (Open Graph and Twitter cards) for every page openagents.com serves
// (docs/web/og-tags.md). One place decides a page's tags, so a new page, realm or
// project gets a complete preview without anyone writing meta tags by hand:
//
//   title        og:title, else the page's <title> without " · OpenAgents"
//   description  og:description, else <meta name="description">
//   url          og:url, else https://openagents.com/<its directory>
//   image        og:image, else its hand-made card (its directory's og.jpg or card.jpg),
//                else its rendered card (og/<its directory>.jpg, og-render.ts), else the
//                branded default (og/default.jpg)
//   alt          og:image:alt, else said from the title
//   and og:type, og:site_name, og:image:width/height, twitter:card (large image),
//   twitter:title/description/image, filled in from the above.
//
// deploy.sh runs it on the staged site: every page gets the tags it lacks, and
// og/pages.txt lists every page's preview for the workbench (/og). It refuses
// (exit 1) a page without a title or description, an image that isn't there or
// isn't a 1200x630 JPEG or PNG, and a realm (realms.json) or project
// (projects/<slug>/project.json) without its own card. COAXIAL_OG_DRAFT=1 turns
// the refusals into warnings (og-render.ts stages a site that way to render the
// cards that are missing).
//
//   bun coax/web/og.ts SITE_DIR APP_DIR   fill and check a staged site
//   bun coax/web/og.ts --report APP_DIR   what each page's preview would be, from the sources
import { existsSync, readFileSync, readdirSync, statSync, writeFileSync, mkdirSync } from "node:fs"
import { dirname, join, relative, sep } from "node:path"

export const ORIGIN = "https://openagents.com"
export const DEFAULT_IMAGE = "/og/default.jpg"
export const W = 1200, H = 630, MAX_BYTES = 300 * 1024
export type Kind = "realm" | "project" | "page"
export type Source = "page" | "hand" | "rendered" | "default"
export type Preview = {
  path: string; kind: Kind; noindex: boolean
  title: string; description: string; url: string
  image: string; alt: string; source: Source
  width: number; height: number; bytes: number
  warnings: string[]; errors: string[]
  progress?: string   // a project's tasks, "DONE OPEN READY" (projects/list.coax), for its card
}

const ENT: Record<string, string> = { amp: "&", quot: '"', lt: "<", gt: ">", apos: "'", "#x27": "'", "#39": "'" }
export const unescape = (s: string) => s.replace(/&(amp|quot|lt|gt|apos|#x27|#39);/g, (_, e) => ENT[e])
export const escape = (s: string) => s.replace(/&/g, "&amp;").replace(/"/g, "&quot;").replace(/</g, "&lt;")

/** The page's meta tags (property or name), unescaped, and its <title>. */
export function tags(html: string): { title: string; meta: Map<string, string> } {
  const head = html.split(/<\/head>/i)[0]
  const meta = new Map<string, string>()
  for (const m of head.matchAll(/<meta\s+(?:property|name)="([^"]+)"\s+content="([^"]*)"\s*\/?>/g)) if (!meta.has(m[1])) meta.set(m[1], unescape(m[2]))
  const title = unescape(/<title>([^<]*)<\/title>/i.exec(head)?.[1]?.trim() ?? "")
  return { title, meta }
}

/** A description for a preview: the whole of it when it fits in 200 characters, else its first
 *  sentences that do, else cut at a word with an ellipsis. */
export function brief(text: string, max = 200): string {
  const t = text.trim()
  if (t.length <= max) return t
  let cut = ""
  for (const m of t.matchAll(/[^.!?]+[.!?]+(\s|$)/g)) { if ((cut + m[0]).trim().length > max) break; cut += m[0] }
  if (cut.trim().length >= 60) return cut.trim()
  const w = t.slice(0, max - 1).replace(/\s+\S*$/, "")
  return w.replace(/[,;:\s]+$/, "") + "…"
}

/** A page's title without the site's name after it. */
export const bare = (title: string) => title.replace(/\s*[·—|-]\s*(OpenAgents( project)?|The Verse)\s*$/, "").trim()

/** Width and height of a JPEG or PNG, or null. */
export function size(bytes: Uint8Array): { w: number; h: number; type: "jpeg" | "png" } | null {
  if (bytes[0] === 0x89 && bytes[1] === 0x50 && bytes.length > 24) {
    const v = new DataView(bytes.buffer, bytes.byteOffset)
    return { w: v.getUint32(16), h: v.getUint32(20), type: "png" }
  }
  if (bytes[0] !== 0xff || bytes[1] !== 0xd8) return null
  let i = 2
  while (i + 9 < bytes.length) {
    if (bytes[i] !== 0xff) { i++; continue }
    const m = bytes[i + 1], len = (bytes[i + 2] << 8) | bytes[i + 3]
    if (m >= 0xc0 && m <= 0xcf && m !== 0xc4 && m !== 0xc8 && m !== 0xcc)
      return { h: (bytes[i + 5] << 8) | bytes[i + 6], w: (bytes[i + 7] << 8) | bytes[i + 8], type: "jpeg" }
    i += 2 + len
  }
  return null
}

const realmSlugs = (app: string): Set<string> => {
  const f = join(app, "realms.json")
  if (!existsSync(f)) return new Set()
  return new Set(JSON.parse(readFileSync(f, "utf8")).worlds.map((w: any) => w.realm?.slug).filter(Boolean))
}
const kindOf = (app: string, dir: string, realms: Set<string>): Kind => {
  const p = dir.split("/")
  if (p[0] === "realms" && p.length === 2 && realms.has(p[1])) return "realm"
  if (p[0] === "projects" && p.length === 2 && existsSync(join(app, dir, "project.json"))) return "project"
  return "page"
}
/** Each listed project's tasks (projects/list.coax, list.ts): slug -> "DONE OPEN READY". */
export function progress(app: string): Map<string, string> {
  const f = join(app, "projects", "list.coax"), out = new Map<string, string>()
  if (!existsSync(f)) return out
  const lit = /PROJECTS_ROWS :: "((?:[^"\\]|\\.)*)";/.exec(readFileSync(f, "utf8"))?.[1] ?? ""
  for (const row of lit.replace(/\\n/g, "\n").replace(/\\(.)/g, "$1").split("\n")) {
    const c = row.split("|")
    if (c.length > 7 && c[0]) out.set(c[0], `${c[7]} ${c[5]} ${c[6]}`)
  }
  return out
}

/** Where a page's rendered card lives (og-render.ts writes it): og/<dir>.jpg, the homepage's og/home.jpg. */
export const renderedCard = (dir: string) => `/og/${dir || "home"}.jpg`

/** One page's preview: what its tags say, filled in, and what is wrong with it. */
export function preview(html: string, dir: string, app: string, files: string, realms = realmSlugs(app)): Preview {
  const { title, meta } = tags(html)
  const kind = kindOf(app, dir, realms)
  const noindex = /noindex/.test(meta.get("robots") ?? "")
  const warnings: string[] = [], errors: string[] = []
  const t = meta.get("og:title") || bare(title)
  const desc = meta.get("og:description") || brief(meta.get("description") || "")
  const url = meta.get("og:url") || `${ORIGIN}/${dir}`.replace(/\/$/, "")
  let image = meta.get("og:image") ?? "", source: Source = "page"
  if (image.startsWith(ORIGIN)) image = image.slice(ORIGIN.length)
  if (!image) {
    const own = ["og.jpg", "card.jpg"].map((f) => `/${dir ? dir + "/" : ""}${f}`).find((f) => existsSync(join(files, f)))
    if (own) { image = own; source = "hand" }
    else if (existsSync(join(files, renderedCard(dir)))) { image = renderedCard(dir); source = "rendered" }
    else { image = DEFAULT_IMAGE; source = "default" }
  } else if (image === renderedCard(dir)) source = "rendered"
  else if (dir && image.startsWith(`/${dir}/`)) source = "hand"
  else if (image === DEFAULT_IMAGE) source = "default"
  const alt = meta.get("og:image:alt") || (source === "default" ? "OpenAgents" : `${t}, on OpenAgents`)
  if (!t) errors.push("no title")
  if (!desc) (noindex ? warnings : errors).push("no description")
  if (t.length > 60) warnings.push(`title is ${t.length} characters (previews cut near 60)`)
  if (desc && desc.length > 200) warnings.push(`description is ${desc.length} characters (previews cut near 200)`)
  if (desc && desc.length < 50 && !noindex) warnings.push(`description is only ${desc.length} characters`)
  if (url !== `${ORIGIN}/${dir}`.replace(/\/$/, "") && !(dir === "" && url === ORIGIN + "/")) warnings.push(`og:url is ${url}, not this page`)
  if (meta.get("twitter:card") && meta.get("twitter:card") !== "summary_large_image") warnings.push(`twitter:card is ${meta.get("twitter:card")}: a small square preview`)
  let width = 0, height = 0, bytes = 0
  const file = image.startsWith("/") ? join(files, image) : ""
  if (!file) warnings.push(`image is off the site: ${image}`)
  else if (!existsSync(file)) errors.push(`image ${image} is not on the site`)
  else {
    const b = new Uint8Array(readFileSync(file)); bytes = b.length
    const s = size(b)
    if (!s) errors.push(`image ${image} is not a JPEG or PNG`)
    else {
      width = s.w; height = s.h
      if (s.w !== W || s.h !== H) (Math.abs(s.w / s.h - W / H) < 0.02 && s.w >= 600 ? warnings : errors).push(`image is ${s.w}x${s.h}, not ${W}x${H}`)
      if (bytes > MAX_BYTES) (source === "rendered" ? errors : warnings).push(`image is ${Math.round(bytes / 1024)} KB (over 300)`)
    }
  }
  if (source === "default" && (kind === "realm" || kind === "project") && !noindex)
    errors.push(`a ${kind} without its own card: run bun coax/web/og-render.ts`)
  else if (source === "default" && !noindex) warnings.push("the branded default image: render its own with og-render.ts")
  const out: Preview = { path: "/" + dir, kind, noindex, title: t, description: desc, url, image, alt, source, width, height, bytes, warnings, errors }
  if (kind === "project") out.progress = progress(app).get(dir.split("/")[1]) ?? ""
  return out
}

/** The page with every tag it lacks, after its last meta tag in <head>. */
export function fill(html: string, p: Preview): string {
  const { meta } = tags(html)
  const want: [string, string, string][] = [
    ["property", "og:type", "website"], ["property", "og:site_name", "OpenAgents"],
    ["property", "og:title", p.title], ["property", "og:description", p.description], ["property", "og:url", p.url],
    ["property", "og:image", p.image.startsWith("/") ? ORIGIN + p.image : p.image],
    ["property", "og:image:width", String(p.width || W)], ["property", "og:image:height", String(p.height || H)],
    ["property", "og:image:alt", p.alt],
    ["name", "twitter:card", "summary_large_image"], ["name", "twitter:title", p.title],
    ["name", "twitter:description", p.description], ["name", "twitter:image", p.image.startsWith("/") ? ORIGIN + p.image : p.image],
  ]
  const add = want.filter(([, k, v]) => !meta.has(k) && v).map(([a, k, v]) => `<meta ${a}="${k}" content="${escape(v)}">\n`).join("")
  if (!add) return html
  const head = html.search(/<\/head>/i)
  if (head < 0) return html
  const lastMeta = html.slice(0, head).lastIndexOf("<meta ")
  const at = lastMeta >= 0 ? html.indexOf("\n", lastMeta) + 1 : head
  return html.slice(0, at) + add + html.slice(at)
}

const field = (s: string) => s.replace(/[|\n\r]/g, " ")
/** The workbench's table (og/pages.txt): a page a line, fields split by |, problems by ;. */
export function table(list: Preview[]): string {
  const rows = list.map((p) => [p.path, p.kind, p.noindex ? "noindex" : "", p.title, p.description, p.url, p.image, p.alt, p.source,
    String(p.width), String(p.height), String(Math.round(p.bytes / 1024)), p.errors.join("; "), p.warnings.join("; "), p.progress ?? ""].map(field).join("|"))
  return "# path|kind|noindex|title|description|url|image|alt|source|width|height|kb|errors|warnings|progress\n" + rows.join("\n") + "\n"
}

/** Every page's HTML under root: its directory (posix, "" the homepage) and file. 404.html is the error page. */
export function pages(root: string, opts: { skip?: (dir: string) => boolean } = {}): { dir: string; file: string }[] {
  const out: { dir: string; file: string }[] = []
  const walk = (d: string) => {
    for (const f of readdirSync(d).sort()) {
      const p = join(d, f)
      if (f.startsWith(".") || f === "build" || f === "node_modules" || f === "work") continue
      if (statSync(p).isDirectory()) { if (!existsSync(join(p, "DEV-ONLY"))) walk(p); continue }
      if (f === "index.html") out.push({ dir: relative(root, d).split(sep).join("/"), file: p })
    }
  }
  walk(root)
  return out
}

/** Pages whose previews repeat another's image get a warning naming how many share it. */
function shared(list: Preview[]) {
  const by = new Map<string, number>()
  for (const p of list) if (!p.noindex) by.set(p.image, (by.get(p.image) ?? 0) + 1)
  for (const p of list) {
    const n = by.get(p.image) ?? 0
    if (!p.noindex && n > 1 && p.source !== "default") p.warnings.push(`its image is shared with ${n - 1} other page${n > 2 ? "s" : ""}`)
  }
}

if (import.meta.main) {
  const args = process.argv.slice(2)
  const draft = process.env.COAXIAL_OG_DRAFT === "1"
  if (args[0] === "--report") {
    const app = args[1]
    const list = pages(app).map(({ dir, file }) => preview(readFileSync(file, "utf8"), dir, app, app))
    shared(list)
    process.stdout.write(table(list))
    process.exit(0)
  }
  const [site, app] = args
  if (!site || !app) { console.error("usage: bun coax/web/og.ts SITE_DIR APP_DIR | --report APP_DIR"); process.exit(2) }
  const realms = realmSlugs(app)
  const list: Preview[] = []
  let filled = 0
  for (const { dir, file } of pages(site)) {
    const html = readFileSync(file, "utf8")
    const p = preview(html, dir, app, site, realms)
    list.push(p)
    const next = fill(html, p)
    if (next !== html) {
      filled++
      writeFileSync(file, next)
      // the bare-path copy deploy.sh made of a directory's page (/projects/coax)
      if (dir && existsSync(join(site, dir + ".page"))) writeFileSync(join(site, dir + ".page"), next)
    }
  }
  // the error page answers any address: its preview points home
  const nf = join(site, "404.html")
  if (existsSync(nf)) {
    const html = readFileSync(nf, "utf8"), p = preview(html, "404", app, site, realms)
    p.path = "/404"; p.url = ORIGIN; p.warnings = p.warnings.filter((w) => !w.startsWith("og:url"))
    const next = fill(html, p)
    if (next !== html) writeFileSync(nf, next)
    list.push(p)
  }
  shared(list)
  // the workbench's order: pages people share first (the homepage, then by address), noindex ones last
  list.sort((a, b) => Number(a.noindex) - Number(b.noindex) || (a.path === "/" ? -1 : b.path === "/" ? 1 : a.path.localeCompare(b.path)))
  mkdirSync(join(site, "og"), { recursive: true })
  writeFileSync(join(site, "og", "pages.txt"), table(list))
  const bad = list.filter((p) => p.errors.length)
  const warned = list.filter((p) => p.warnings.length).length
  console.log(`  og       ${list.length} pages, ${filled} filled in, ${warned} with warnings (og/pages.txt, /og)`)
  if (bad.length) {
    for (const p of bad) console.error(`og: ${p.path}: ${p.errors.join("; ")}`)
    if (!draft) { console.error("og: refused: fix these, or COAXIAL_OG_DRAFT=1 to stage anyway (docs/web/og-tags.md)"); process.exit(1) }
  }
}
