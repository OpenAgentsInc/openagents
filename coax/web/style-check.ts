// The style check (docs/verse/art-styles.md, 15. Judging "looks painterly"): every realm of a style
// shot headless at its named views (realms.json `style_views`, hashes such as "road") at noon, dusk
// and night, at 1440x900 and 390x844, measured (style-measures.ts), and laid out beside the style
// fixture in coax/web/style/style-<slug>.png for the owner's and Kiki's judgment.
//   bun coax/web/style-check.ts BASE [--style painterly] [--only SLUG] [--out DIR]
// Exits 1 when a view fails a measure (the release walk reports it; see coax/web.md).
import { readFileSync, writeFileSync, mkdirSync, mkdtempSync, existsSync } from "node:fs"
import { join } from "node:path"
import { tmpdir } from "node:os"
import { decodePng, encodePng, measure, sheet, type Image, type Clock } from "./style-measures"

const args = process.argv.slice(2)
const base = (args[0] ?? "").replace(/\/$/, "")
if (!base.startsWith("http")) { console.error("usage: bun coax/web/style-check.ts BASE [--style painterly] [--only SLUG] [--out DIR]"); process.exit(2) }
const opt = (name: string, d: string) => { const i = args.indexOf(name); return i > 0 ? args[i + 1] : d }
const style = opt("--style", "painterly"), only = opt("--only", ""), out = opt("--out", join(import.meta.dir, "style"))
const CLOCKS: [string, Clock][] = [["0.5", "day"], ["0.75", "day"], ["0", "night"]]
const SIZES = ["1440x900", "390x844"]
const realms = JSON.parse(readFileSync(join(import.meta.dir, "../../apps/openagents-web/realms.json"), "utf8"))
const list: { slug: string; views: string[] }[] = (realms.worlds ?? realms).flatMap((w: any) => w.realm && w.realm.style === style && (!only || w.realm.slug === only)
  ? [{ slug: w.realm.slug, views: w.realm.style_views ?? [""] }] : [])
const tmp = mkdtempSync(join(tmpdir(), "coax-style-"))
let shots = 0
async function shoot(url: string, size: string): Promise<Image> {
  const file = join(tmp, `${shots++}.png`)
  // a headless Chrome that doesn't come up (a busy machine) gets two more tries
  for (let tries = 0; tries < 3 && !existsSync(file); tries++) {
    await Bun.spawn(["bun", join(import.meta.dir, "shot.ts"), url, file, size, "9000"], { stdout: "ignore", stderr: "ignore" }).exited
  }
  if (!existsSync(file)) throw new Error(`style check: no shot of ${url} at ${size}`)
  return decodePng(readFileSync(file))
}
// a few at a time: each shot is its own headless Chrome
async function pool<T>(jobs: (() => Promise<T>)[], n = 2): Promise<T[]> {
  const res: T[] = new Array(jobs.length); let next = 0
  await Promise.all(Array.from({ length: n }, async () => { while (next < jobs.length) { const k = next++; res[k] = await jobs[k]() } }))
  return res
}
mkdirSync(out, { recursive: true })
let failed = 0
const fixture = await pool(CLOCKS.map(([c]) => () => shoot(`${base}/realms/fixture?style=${style}&clock=${c}&golden`, "1440x900")))
for (const r of list) {
  const jobs: { name: string; clock: Clock; size: string; run: () => Promise<Image> }[] = []
  for (const v of r.views) for (const [c, clock] of CLOCKS) for (const size of SIZES)
    jobs.push({ name: `${v || "spawn"} at ${c}, ${size}`, clock, size, run: () => shoot(`${base}/realms/${r.slug}/?clock=${c}&golden${v ? `#${v}` : ""}`, size) })
  const images = await pool(jobs.map((j) => j.run))
  console.log(`${r.slug} (${style})`)
  jobs.forEach((j, k) => {
    const m = measure(images[k], j.clock)
    const line = `fine ${m.fine.toFixed(3)}, ink ${(m.ink * 100).toFixed(2)}%, black ${(m.black * 100).toFixed(2)}%, saturation ${m.sat.toFixed(2)}`
    console.log(`  ${m.fails.length ? "FAIL" : "ok  "}  ${j.name}: ${m.fails.length ? m.fails.join("; ") : line}`)
    if (m.fails.length) failed++
  })
  // the sheet: the fixture at each clock, then each view's desktop shots at each clock
  const desktop = images.filter((_, k) => jobs[k].size === SIZES[0])
  const file = join(out, `style-${r.slug}.png`)
  writeFileSync(file, encodePng(sheet([...fixture, ...desktop], CLOCKS.length, 480, 300)))
  console.log(`  sheet ${file}`)
}
console.log(failed ? `style check: ${failed} views failed` : "style check: passed")
process.exit(failed ? 1 : 0)
