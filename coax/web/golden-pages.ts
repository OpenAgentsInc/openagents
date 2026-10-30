// The pages the release walk (goldens.ts) compares with their goldens, and
// what each one's look is built from, so a commit can be tied to the pages
// it may have changed (golden-check.sh) and a page that fails its golden to
// the commits that most likely broke it (release.sh's hold-back).
//
// A page's inputs, as repository paths (a path ending in / is a directory):
//   shared   what every page is built or walked with: the host and its
//            scripts, the build and deploy steps, the realm table, the walk
//            itself, and the Coax compiler and libraries (coax/, not its
//            docs or tests)
//   its own  its golden, its module (apps/openagents-web/M.coax, M-face.c,
//            M.wgsl), the files those #load or #include in the app
//            directory, the Coax libraries build.sh links into build/M.wasm,
//            its page's directory and its assets; a realm's also the engine
//            (verse/), whose functions build.sh writes into the realms' shaders
// Generous on purpose: a page named here that didn't change costs a render,
// a page missed would let a break through.
import { existsSync, readFileSync, statSync } from "node:fs"
import { dirname, join, normalize } from "node:path"
import { worlds } from "./realms"

// GOLDEN_PAGES_ROOT: another checkout's pages and inputs, with this file's
// rules (golden-check.sh on a commit from before this file existed).
const ROOT = process.env.GOLDEN_PAGES_ROOT || join(import.meta.dir, "../..")
const APP = "apps/openagents-web"
// Goldens are renders, and Metal on a Mac and Vulkan on Linux light pixels a
// little differently, so each platform's walk compares with its own set:
// coax/web/goldens/ (the Mac's) or coax/web/goldens/linux/. GOLDENS_PLATFORM
// (mac or linux) picks one by hand.
export const GOLDEN_DIR = (process.env.GOLDENS_PLATFORM ?? (process.platform === "darwin" ? "mac" : "linux")) === "mac" ? "coax/web/goldens" : "coax/web/goldens/linux"

// path: its address; lit: the least share of pixels lit; settle: ms before its
// shot; blocks: how many 8 px blocks may change; module: the app it runs; size: its viewport in css px
// when not the walk's 1280x800 (a phone's 390x844: the phone tier, art-styles.md 14).
export type Page = { path: string; lit: number; settle: number; blocks: number; module: string; size?: [number, number] }
export type Pages = { pages: Record<string, Page>; byAddress: Record<string, Page>; open: ReturnType<typeof worlds>; unlisted: Set<string> }

export function goldenPages(file = join(ROOT, APP, "realms.json")): Pages {
  const pages: Record<string, Page> = { landing: { path: "/verse", lit: 0.004, settle: 2500, blocks: 4, module: "verse" },
    galaxy: { path: "/verse/galaxy", lit: 0.01, settle: 2500, blocks: 8, module: "verse" } }
  // Every realm, and the open worlds in the order the galaxy's arrows step
  // through them with ?realms=all (realms.json's order).
  const open = worlds(file, { site: join(ROOT, APP), packs: join(ROOT, "verse/packs"), styleBlock: existsSync(join(ROOT, "verse/style.coax")) }).filter((w) => w.kind === "open" && w.realm)
  for (const w of open) pages[w.realm!.slug] = { path: `/realms/${w.realm!.slug}`, ...w.realm!.golden, module: w.realm!.module }
  // Realms the galaxy doesn't list yet: with no golden, their shot waits in pending/ (golden-policy.ts).
  const unlisted = new Set(open.filter((w) => w.listed === false).map((w) => w.realm!.slug))
  // Pages of their own the galaxy doesn't open (not in realms.json), walked by their address.
  const byAddress: Record<string, Page> = { garden: { path: "/realms/garden", lit: 0.5, settle: 3000, blocks: 6, module: "garden" },
    "realms-new": { path: "/realms/new", lit: 0.004, settle: 2500, blocks: 4, module: "builder" } }
  byAddress["pylon"] = {path:"/pylon",lit:0.004,settle:1500,blocks:4,module:"pylon"}
  byAddress["pylon-phone"] = {path:"/pylon",lit:0.004,settle:1500,blocks:4,module:"pylon",size:[390,844]}
  // NIP guides are public Coaxial pages too. Every route must start and draw
  // during staging and production checks, with its own reviewed golden.
  const nipCatalog = join(ROOT, APP, "nip/catalog.json")
  if (existsSync(nipCatalog)) {
    const nips = JSON.parse(readFileSync(nipCatalog, "utf8")).pages as { number: number }[]
    byAddress["nip-index"] = { path: "/nip", lit: 0.004, settle: 1200, blocks: 4, module: "nip-guide" }
    for (const nip of nips) {
      const number = String(nip.number).padStart(2, "0")
      byAddress[`nip-${number}`] = { path: `/nip/${number}`, lit: 0.004, settle: nip.number === 1 ? 2200 : 1200, blocks: 4, module: nip.number === 1 ? "nip01" : "nip-guide" }
    }
    byAddress["nip-01-phone"] = {path:"/nip/01",lit:0.004,settle:2200,blocks:4,module:"nip01",size:[390,844]}
    for (const [name, path] of [["nip-index-phone", "/nip"], ["nip-44-phone", "/nip/44"], ["nip-49-phone", "/nip/49"]])
      byAddress[name] = { path, lit: 0.004, settle: 1500, blocks: 4, module: "nip-guide", size: [390, 844] }
  }
  // The style fixture (docs/verse/art-styles.md): the fixture realm in the painterly preset at each clock key.
  for (const c of ["0", "0.25", "0.5", "0.75"])
    byAddress[`fixture-painterly-${c}`] = { path: `/realms/fixture?style=painterly&clock=${c}`, lit: 0.5, settle: 3000, blocks: 6, module: "fixture" }
  // and on a phone (390x844 css px: the phone tier, art-styles.md 14) at each clock key
  for (const c of ["0", "0.25", "0.5", "0.75"])
    byAddress[`fixture-painterly-phone-${c}`] = { path: `/realms/fixture?style=painterly&clock=${c}`, lit: 0.5, settle: 3000, blocks: 6, module: "fixture", size: [390, 844] }
  // and backlit at noon, the sun put behind the blob tree (?sun=, ?sunup=), for the light shafts (spec 6)
  byAddress["fixture-painterly-shafts"] = { path: "/realms/fixture?style=painterly&clock=0.5&sun=156&sunup=9.5", lit: 0.5, settle: 3000, blocks: 6, module: "fixture" }
  for (const [name, page] of Object.entries(byAddress)) pages[name] ??= page
  return { pages, byAddress, open, unlisted }
}

/** What every page is built or walked with: files, and directories (ending in /). */
export const SHARED = ["coax/web/host.js", "coax/web/loader.js", "coax/web/voice.js", "coax/web/entropy.js", "coax/web/entropy-import.js", "coax/web/drop.js",
  "coax/web/uploads.js", "coax/web/deploy.sh", "coax/web/strip.ts", "coax/web/wasm.sh", "coax/web/wgsl.py", "coax/web/realms.ts",
  "coax/web/goldens.ts", "coax/web/golden-pages.ts", "coax/web/golden-policy.ts", "coax/web/unlock/", "coax/web/voice/",
  `${APP}/build.sh`, `${APP}/realms.json`, `${APP}/realms.coax`, "coax/"]
// What can't change a page: docs, tests, and coax/web's other tools (only the files above count there).
const notInput = (p: string) => /\.md$|(^|\/)tests?\/|\.test\./.test(p) || (p.startsWith("coax/web/") && !SHARED.some((s) => s !== "coax/" && (s.endsWith("/") ? p.startsWith(s) : p === s)))

const read = (rel: string) => { try { return readFileSync(join(ROOT, rel), "utf8") } catch { return "" } }
const isFile = (rel: string) => { try { return statSync(join(ROOT, rel)).isFile() } catch { return false } }
// The files a Coax file #loads and a C file #includes, from the app directory, transitively.
function sources(start: string[], out = new Set<string>()) {
  for (const rel of start) {
    if (out.has(rel) || !isFile(rel)) continue
    out.add(rel)
    const text = read(rel), next: string[] = []
    for (const m of text.matchAll(/^\s*#(?:load|include)\s+"([^"]+)"/gm)) next.push(normalize(join(dirname(rel), m[1])))
    sources(next, out)
  }
  return out
}
// The Coax libraries build.sh links into build/M.wasm ("$m/x.coax" is verse/x.coax, "$here/x" the app's).
function linked(module: string, build = read(`${APP}/build.sh`)) {
  const out: string[] = []
  for (const line of build.replace(/\\\n/g, " ").split("\n")) {
    if (!line.includes(`build/${module}.wasm`)) continue
    for (const m of line.matchAll(/"\$(m|here)\/([^"$]+\.coax)"/g)) out.push(m[1] === "m" ? `verse/${m[2]}` : `${APP}/${m[2]}`)
  }
  return out
}

/** A page's own inputs (SHARED aside). */
export function pageInputs(name: string, p: Pages = goldenPages()): string[] {
  const page = p.pages[name]
  if (!page) return []
  const m = page.module, out = new Set<string>()
  for (const f of sources([`${APP}/${m}.coax`, `${APP}/${m}-face.c`, ...linked(m)])) out.add(f)
  out.add(`${APP}/${m}.wgsl`)
  out.add(`coax/web/goldens/${name}.png`)   // a wrong golden fails the page as surely as a changed page
  out.add(`coax/web/goldens/linux/${name}.png`)
  const dir = page.path.split("?")[0].replace(/^\//, "")
  out.add(`${APP}/${dir}/`)
  const realm = p.open.find((w) => w.realm!.slug === name)?.realm
  // an asset is "kind:/path", or several paths apart by "|"
  for (const a of realm?.assets ?? []) for (const part of a.replace(/^[a-z]+:/, "").split("|")) if (part.startsWith("/")) out.add(`${APP}${part}`)
  if (m !== "verse") out.add("verse/")
  return [...out].sort()
}

// A page's directory holds its Markdown twin too, which no one sees drawn.
const under = (file: string, input: string) => input.endsWith("/") ? file.startsWith(input) && !file.endsWith(".md") : file === input
const sharedHit = (file: string) => SHARED.some((s) => under(file, s)) && !notInput(file)

/** The pages a set of changed files may have changed, in the walk's order. */
export function affected(changed: string[], p: Pages = goldenPages()): string[] {
  const names = Object.keys(p.pages)
  if (changed.some(sharedHit)) return names
  return names.filter((n) => { const ins = pageInputs(n, p); return changed.some((f) => ins.some((i) => under(f, i))) })
}

/** Why a commit's files may change a page: "own" (an input of its own: its module, what that loads or links, its
 *  page, assets or golden), "engine" (only the rest of verse/, which every realm takes), "shared", or "" (they can't). */
export type Why = "own" | "engine" | "shared"
export function touches(name: string, files: string[], p: Pages = goldenPages()): Why | "" {
  const ins = pageInputs(name, p)
  if (files.some((f) => ins.some((i) => i !== "verse/" && under(f, i)))) return "own"
  if (files.some((f) => under(f, "verse/") && ins.includes("verse/"))) return "engine"
  return files.some(sharedHit) ? "shared" : ""
}

/** The bucket objects that are a page's bytes: /verse is verse (the bare path's page) and verse/index.html. */
export function pageObjects(path: string): string {
  const dir = path.split("?")[0].replace(/^\/|\/$/g, "").replace(/[.*+?^${}()|[\]\\]/g, "\\$&")
  return dir ? `^${dir}(/index\\.html)?$` : "^index\\.html$"
}

// The commits of a range that may have changed a page, newest first; the
// likeliest is the newest touching its own inputs, else the engine, else what
// every page shares.
export type Suspect = { hash: string; author: string; subject: string; why: Why }
export function suspects(name: string, commits: { hash: string; author: string; subject: string; files: string[] }[], p: Pages = goldenPages()): Suspect[] {
  const out: Suspect[] = []
  for (const c of commits) { const why = touches(name, c.files, p); if (why) out.push({ hash: c.hash, author: c.author, subject: c.subject, why }) }
  return (["own", "engine", "shared"] as const).flatMap((why) => out.filter((s) => s.why === why))
}

// ---- git: the commits of a range with their files
export function commitsIn(range: string, cwd = ROOT) {
  const r = Bun.spawnSync(["git", "-C", cwd, "log", "--no-merges", "--format=%x00%H%x09%an%x09%s", "--name-only", range], { stdout: "pipe", stderr: "pipe" })
  if (r.exitCode !== 0) throw new Error(`git log ${range}: ${r.stderr.toString().trim()}`)
  return r.stdout.toString().split("\0").filter((s) => s.trim()).map((chunk) => {
    const [head, ...files] = chunk.split("\n")
    const [hash, author, subject] = head.split("\t")
    return { hash, author, subject, files: files.map((f) => f.trim()).filter(Boolean) }
  })
}

//   bun golden-pages.ts affected RANGE           the pages RANGE (BASE..HEAD) may have changed, one a line
//   bun golden-pages.ts blame RANGE NAME...      each page's likeliest breaking commit in RANGE: NAME<TAB>HASH<TAB>AUTHOR<TAB>WHY<TAB>SUBJECT
//   bun golden-pages.ts objects NAME...          the bucket objects that are those pages' bytes, as one regex
if (import.meta.main) {
  const [verb, ...rest] = process.argv.slice(2)
  const p = goldenPages()
  if (verb === "affected") {
    const files = [...new Set(commitsIn(rest[0]).flatMap((c) => c.files))]
    for (const n of affected(files, p)) console.log(n)
  } else if (verb === "blame") {
    const commits = commitsIn(rest[0])
    for (const name of rest.slice(1)) {
      const s = suspects(name, commits, p)[0]
      console.log(s ? [name, s.hash, s.author, s.why, s.subject].join("\t") : [name, "", "", "", "no commit in the range touches its inputs"].join("\t"))
    }
  } else if (verb === "objects") {
    console.log(rest.filter((n) => p.pages[n]).map((n) => pageObjects(p.pages[n].path)).join("|"))
  } else {
    console.error("usage: bun golden-pages.ts affected RANGE | blame RANGE NAME... | objects NAME..."); process.exit(2)
  }
}
