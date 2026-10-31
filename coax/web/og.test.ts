// coax/web/og.ts: a page's link preview, filled in and checked (docs/web/og-tags.md).
import { expect, test } from "bun:test"
import { mkdirSync, mkdtempSync, writeFileSync } from "node:fs"
import { tmpdir } from "node:os"
import { join } from "node:path"
import { bare, brief, fill, preview, size, tags } from "./og"

// A JPEG's start with a frame header saying w x h (enough for size()).
const jpeg = (w: number, h: number) => new Uint8Array([0xff, 0xd8, 0xff, 0xc0, 0, 17, 8, h >> 8, h & 255, w >> 8, w & 255, 3, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0])
const page = (head: string) => `<!doctype html>\n<html>\n<head>\n<meta charset="utf-8">\n${head}\n</head>\n<body></body>\n</html>\n`
function site() {
  const app = mkdtempSync(join(tmpdir(), "og-test-"))
  mkdirSync(join(app, "og/projects"), { recursive: true })
  mkdirSync(join(app, "projects/coax"), { recursive: true })
  writeFileSync(join(app, "projects/coax/project.json"), "{}")
  writeFileSync(join(app, "og/default.jpg"), jpeg(1200, 630))
  writeFileSync(join(app, "realms.json"), JSON.stringify({ worlds: [{ id: "x", realm: { slug: "dead-gears" } }] }))
  return app
}

test("titles lose the site's name; long lines are cut at a sentence, else a word", () => {
  expect(bare("Coax · OpenAgents project")).toBe("Coax")
  expect(bare("Connect a Pylon — OpenAgents")).toBe("Connect a Pylon")
  expect(bare("Galaxy · The Verse")).toBe("Galaxy")
  const two = "A".repeat(120) + ". " + "B".repeat(120) + "."
  expect(brief(two)).toBe("A".repeat(120) + ".")
  const words = Array(60).fill("word").join(" ")
  expect(brief(words).length).toBeLessThanOrEqual(200)
  expect(brief(words).endsWith("…")).toBe(true)
})

test("a page with only a title and description gets every tag, with the branded card", () => {
  const app = site()
  const html = page(`<title>Pylon · OpenAgents</title>\n<meta name="description" content="View OpenAgents Pylons &amp; signed network activity, verified in Coax.">`)
  const p = preview(html, "pylon", app, app)
  expect(p.errors).toEqual([])
  expect(p.source).toBe("default")
  expect(p.warnings.some((w) => w.includes("branded default"))).toBe(true)
  const t = tags(fill(html, p)).meta
  expect(t.get("og:title")).toBe("Pylon")
  expect(t.get("og:description")).toBe("View OpenAgents Pylons & signed network activity, verified in Coax.")
  expect(t.get("og:url")).toBe("https://openagents.com/pylon")
  expect(t.get("og:image")).toBe("https://openagents.com/og/default.jpg")
  expect(t.get("twitter:card")).toBe("summary_large_image")
  expect(t.get("twitter:image")).toBe("https://openagents.com/og/default.jpg")
  expect(fill(fill(html, p), p)).toBe(fill(html, p))   // nothing twice
})

test("a project or realm on the default card, a missing image, a wrong size and no description are refused", () => {
  const app = site()
  const proj = preview(page(`<title>Coax · OpenAgents project</title>\n<meta name="description" content="A systems language for real-time code that compiles to C11.">`), "projects/coax", app, app)
  expect(proj.kind).toBe("project")
  expect(proj.errors.join()).toContain("a project without its own card")
  writeFileSync(join(app, "og/projects/coax.jpg"), jpeg(1200, 630))
  const ok = preview(page(`<title>Coax · OpenAgents project</title>\n<meta name="description" content="A systems language for real-time code that compiles to C11.">`), "projects/coax", app, app)
  expect(ok.errors).toEqual([])
  expect(ok.source).toBe("rendered")
  const realm = preview(page(`<title>Dead Gears · OpenAgents</title>\n<meta name="description" content="Hold a ship's hold against three waves, then repair the reactor.">\n<meta property="og:image" content="https://openagents.com/og/realms/dead-gears.jpg">`), "realms/dead-gears", app, app)
  expect(realm.kind).toBe("realm")
  expect(realm.errors.join()).toContain("not on the site")
  writeFileSync(join(app, "og/small.jpg"), jpeg(400, 400))
  const small = preview(page(`<title>X</title>\n<meta name="description" content="A page whose card is a small square picture, not a link card.">\n<meta property="og:image" content="/og/small.jpg">`), "x", app, app)
  expect(small.errors.join()).toContain("400x400")
  expect(preview(page(`<title>Y</title>`), "y", app, app).errors).toContain("no description")
  expect(preview(page(`<title>Y</title>\n<meta name="robots" content="noindex">`), "y", app, app).errors).toEqual([])
})

test("size reads a JPEG's and a PNG's dimensions", () => {
  expect(size(jpeg(1200, 630))).toEqual({ w: 1200, h: 630, type: "jpeg" })
  const png = new Uint8Array(32); png.set([0x89, 0x50, 0x4e, 0x47]); new DataView(png.buffer).setUint32(16, 1200); new DataView(png.buffer).setUint32(20, 630)
  expect(size(png)).toEqual({ w: 1200, h: 630, type: "png" })
  expect(size(new Uint8Array([1, 2, 3]))).toBeNull()
})
