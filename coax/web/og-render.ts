// Renders the link-preview cards (docs/web/og-tags.md) a page without a hand-made one
// shares as, into apps/openagents-web/og/ (committed, so a release needs no browser):
// og/<page's directory>.jpg (the homepage's og/home.jpg) and og/default.jpg, the
// branded card for a page that has none yet. Each is /og#card=PATH (og.coax) captured
// at 1200x630 in headless Chrome with WebGPU (no window, no sound) and saved as a JPEG
// under 300 KB. A realm's card is a still of the realm itself under its name: the
// realm is shot first, at its best moment without its HUD (STILLS), and its card drawn over that shot.
//
//   bun coax/web/og-render.ts                 the missing cards (a stage is built for it)
//   bun coax/web/og-render.ts --all           every rendered card again (a project's tasks done move)
//   bun coax/web/og-render.ts /realms/x ...   these pages' cards
//   ... --stage DIR                           use a site staged already (COAXIAL_STAGE_TO=DIR deploy.sh)
//
// A page with its own og:image, og.jpg or card.jpg, or a realm with a "card" in
// realms.json, keeps it: nothing here touches a hand-made card.
import { existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, statSync } from "node:fs"
import { tmpdir } from "node:os"
import { dirname, join } from "node:path"
import { MAX_BYTES, renderedCard, size } from "./og"

const here = import.meta.dir, repo = join(here, "../.."), app = join(repo, "apps/openagents-web")
const args = process.argv.slice(2)
const at = args.indexOf("--stage")
let stage = at >= 0 ? args[at + 1] : ""
const all = args.includes("--all")
const named = args.filter((a, i) => a.startsWith("/") && args[i - 1] !== "--stage")
const run = (cmd: string[], env: Record<string, string> = {}) => {
  const p = Bun.spawnSync(cmd, { cwd: repo, env: { ...process.env, ...env }, stdout: "pipe", stderr: "pipe" })
  if (p.exitCode !== 0) throw new Error(`${cmd.join(" ")}: ${p.stderr.toString().slice(-2000)}`)
  return p.stdout.toString()
}

if (!stage) {
  stage = mkdtempSync(join(tmpdir(), "og-stage-"))
  console.log(`staging the site (draft: a card may be missing) into ${stage} ...`)
  run([join(here, "deploy.sh"), app, "openagents-web-staging"], { COAXIAL_STAGE_TO: stage, COAXIAL_OG_DRAFT: "1" })
}
const site = join(stage, "site")
const rows = readFileSync(join(site, "og/pages.txt"), "utf8").split("\n").filter((l) => l && !l.startsWith("#")).map((l) => l.split("|"))
const realms: Record<string, any> = Object.fromEntries(JSON.parse(readFileSync(join(app, "realms.json"), "utf8")).worlds
  .filter((w: any) => w.realm).map((w: any) => [w.realm.slug, w.realm]))
// How each realm's still is taken: its best moment, without the HUD. A staged moment
// (?stage=highlight, docs/verse/clips.md) is stepped `skip` seconds of game time in; ?card=1 asks
// the game to leave its HUD out. A realm without it is shot big (its HUD small at the edges)
// and cropped to [x, y, w, h] clear of it. Anything not listed: golden mode at 1680x638.
const STILLS: Record<string, { query?: string; skip?: number; size?: string; crop?: number[] }> = {
  "hold-the-stair": { query: "?stage=highlight&card=1", skip: 10 },
  "orbit-league": { query: "?stage=highlight&card=1", skip: 1.65 },
  "belt-2200": { query: "?stage=highlight&card=1", skip: 2.5 },
  "dead-gears": { query: "?golden&card=1&hour=17.5" },
  "the-grid": { size: "2520x957", crop: [150, 0, 2160, 822] },
  "fixture": { size: "2520x957", crop: [560, 170, 1400, 532] },
  "drop": { size: "2520x957", crop: [500, 200, 1520, 578] },
  "built": { size: "2520x957", crop: [260, 70, 2000, 760] },
  "sandbox": { size: "2520x957", crop: [220, 0, 2000, 760] },
}
const dirOf = (path: string) => path.replace(/^\//, "")
// A page's card is ours to render when it isn't hand-made: its image is the default or its rendered card.
const ours = (r: string[]) => !r[2] && (r[8] === "default" || r[8] === "rendered") && r[0] !== "/404"
let targets = rows.filter((r) => named.length ? named.includes(r[0]) : ours(r) && (all || r[8] === "default" || !existsSync(join(app, renderedCard(dirOf(r[0]))))))
const wantDefault = named.length === 0 && (all || !existsSync(join(app, "og/default.jpg")))
if (!targets.length && !wantDefault) { console.log("og-render: every card is there"); process.exit(0) }

const port = 4700 + Math.floor(Math.random() * 200)
const server = Bun.spawn(["bun", join(here, "golden-serve.ts"), site, String(port)], { stdout: "ignore", stderr: "ignore" })
const base = `http://golden.localhost:${port}`
const work = mkdtempSync(join(tmpdir(), "og-render-"))
const shot = (url: string, out: string, wh: string, wait: number) => run([join(here, "tool.sh"), "shot", url, out, wh, String(wait)])
// PNG to a JPEG under 300 KB, sized to w x h (sips: macOS).
const jpeg = (png: string, out: string, w: number, h: number) => {
  mkdirSync(dirname(out), { recursive: true })
  for (const q of [82, 74, 66, 58]) {
    run(["sips", "-s", "format", "jpeg", "-s", "formatOptions", String(q), "-z", String(h), String(w), png, "--out", out])
    if (statSync(out).size <= MAX_BYTES) return
  }
}
try {
  for (let i = 0; i < 100; i++) { try { await fetch(base + "/og/pages.txt"); break } catch { await Bun.sleep(100) } }
  let n = 0
  for (const r of targets) {
    const [path, kind] = r
    const card = join(app, renderedCard(dirOf(path)))
    if (kind === "realm") {
      const slug = path.split("/")[2], realm = realms[slug], how = STILLS[slug] ?? {}
      const png = join(work, `${slug}-still.png`), [w, h] = (how.size ?? "1680x638").split("x").map(Number)
      if (how.skip !== undefined) {
        // a staged moment, stepped in fixed game time (clip.ts): the same frame every render
        const frames = join(work, `${slug}-frames`)
        run(["bun", join(here, "clip.ts"), `${base}${path}${how.query}`, join(work, `${slug}.mp4`), "--size", how.size ?? "1680x638",
          "--seconds", "0.034", "--fps", "30", "--skip", String(how.skip), "--frames", frames])
        run(["cp", join(frames, "f00000.png"), png])
      } else {
        shot(`${base}${path}${how.query ?? "?golden"}`, png, `${w}x${h}`, Math.max(6000, (realm?.golden?.settle ?? 0) + 5000))
      }
      // the part of the shot clear of the realm's HUD, then fitted to the card's 1200x456 band
      if (how.crop) { const [x, y, cw, ch] = how.crop; run(["sips", "-c", String(ch), String(cw), "--cropOffset", String(y), String(x), png, "--out", png]) }
      jpeg(png, join(site, `og/stills/${slug}.jpg`), 1200, 456)
    }
    const png = join(work, `card-${n}.png`)
    shot(`${base}/og#card=${path}`, png, "1200x630", kind === "realm" ? 3500 : 2500)
    jpeg(png, card, 1200, 630)
    const s = size(new Uint8Array(readFileSync(card)))
    console.log(`  ${path.padEnd(32)} ${renderedCard(dirOf(path))}  ${s?.w}x${s?.h}  ${Math.round(statSync(card).size / 1024)} KB`)
    n++
  }
  if (wantDefault) {
    const png = join(work, "default.png")
    shot(`${base}/og#card=default`, png, "1200x630", 2500)
    jpeg(png, join(app, "og/default.jpg"), 1200, 630)
    console.log(`  (default)                        /og/default.jpg  ${Math.round(statSync(join(app, "og/default.jpg")).size / 1024)} KB`)
  }
  console.log(`og-render: ${n} card${n === 1 ? "" : "s"} in apps/openagents-web/og/: look at them at /og, then commit them`)
} finally {
  server.kill()
  rmSync(work, { recursive: true, force: true })
}
