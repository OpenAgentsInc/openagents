// MANDATORY PIXEL PROOF for #6033 (the crackling arc must actually RENDER).
//
// Bundles the harness (which mounts the ACTUAL desktop `verseSceneVisualization`
// path through three-effect's `mountTrainingRunVisualization`), serves it, drives
// headless chromium to BOTH the spawned (?spawn=1) and the un-spawned (?spawn=0)
// world, lets several animated frames accumulate, screenshots both, and asserts
// the spawned world has a meaningful cluster of bright crackling-arc pixels that
// the un-spawned world does NOT — i.e. the arc is visibly present.
//
// Saves the spawned screenshot to tests/headless/verse-spawned-arc.headless.png
// so it can be eyeballed and committed to the PR.
//
// Usage: bun run tests/headless/capture-verse-arc-headless.ts

import { createServer } from "node:http"
import { readFile } from "node:fs/promises"
import { join, dirname, extname } from "node:path"
import { fileURLToPath } from "node:url"

import { chromium } from "playwright"

const here = dirname(fileURLToPath(import.meta.url))
const harnessEntry = join(here, "verse-spawned-arc-harness.ts")
const bundleOut = join(here, "verse-spawned-arc-harness.bundle.js")
const screenshotPath = join(here, "verse-spawned-arc.headless.png")
const baselinePath = join(here, "verse-spawned-arc.baseline.headless.png")

const host = "127.0.0.1"
const port = Number(process.env.VERSE_ARC_PORT ?? "5188")

// ── 1. Bundle the harness (view.ts + three-effect, browser target) ──────────
const build = await Bun.build({
  entrypoints: [harnessEntry],
  outdir: here,
  naming: "verse-spawned-arc-harness.bundle.js",
  target: "browser",
  sourcemap: "none",
  minify: false,
})
if (!build.success) {
  console.error("Failed to bundle the verse-spawned-arc harness.")
  for (const log of build.logs) console.error(log)
  process.exit(1)
}

// ── 2. Tiny static server for the harness dir ───────────────────────────────
const contentType = (path: string): string => {
  switch (extname(path)) {
    case ".html":
      return "text/html; charset=utf-8"
    case ".js":
      return "text/javascript; charset=utf-8"
    case ".png":
      return "image/png"
    default:
      return "application/octet-stream"
  }
}

const server = createServer(async (req, res) => {
  try {
    const url = new URL(req.url ?? "/", `http://${host}:${port}`)
    const name = url.pathname === "/" ? "/verse-spawned-arc.html" : url.pathname
    const body = await readFile(join(here, name))
    res.writeHead(200, { "content-type": contentType(name) })
    res.end(body)
  } catch {
    res.writeHead(404)
    res.end("not found")
  }
})
await new Promise<void>((resolve) => server.listen(port, host, resolve))

const fail = (message: string): never => {
  console.error(message)
  server.close()
  process.exit(1)
}

// ── 3. Drive chromium, count bright crackling-arc (cyan/blue) pixels ────────
type ArcAnalysis = {
  ok: boolean
  brightArc: number
  total: number
  ratio: number
  width: number
  height: number
}

const analysePage = async (
  spawn: boolean,
  savePath: string | null,
): Promise<{ boot: unknown; analysis: ArcAnalysis }> => {
  const browser = await chromium.launch({ headless: true })
  try {
    const page = await browser.newPage({ viewport: { width: 1024, height: 768 } })
    const errors: string[] = []
    page.on("console", (m) => {
      if (m.type() === "error") errors.push(m.text())
    })
    page.on("pageerror", (e) => errors.push(String(e)))

    const url = `http://${host}:${port}/verse-spawned-arc.html?spawn=${spawn ? "1" : "0"}`
    await page.goto(url, { waitUntil: "networkidle" })
    await page.waitForSelector("#scene canvas", { timeout: 15_000 })
    const boot = await page.evaluate(
      () => (globalThis as unknown as { __verseArcScene?: unknown }).__verseArcScene,
    )

    // Let the crackling animation accumulate several frames.
    await page.waitForTimeout(1800)

    const shot = await page.screenshot()
    if (savePath !== null) await page.screenshot({ path: savePath })

    // Count bright crackling-arc pixels: the arc strands are bright cyan/blue
    // (~#93c5fd / #f8fafc) — pixels where blue is high AND blue dominates over
    // red, on a near-black world. This isolates the arc from the dim grey world
    // geometry and the warm pylon/world tones.
    const analysis = await page.evaluate(async (pngBytes) => {
      const blob = new Blob([new Uint8Array(pngBytes)], { type: "image/png" })
      const bitmap = await createImageBitmap(blob)
      const probe = document.createElement("canvas")
      probe.width = bitmap.width
      probe.height = bitmap.height
      const ctx = probe.getContext("2d")
      if (ctx === null) {
        return {
          ok: false,
          brightArc: 0,
          total: 0,
          ratio: 0,
          width: 0,
          height: 0,
        }
      }
      ctx.drawImage(bitmap, 0, 0)
      const { data } = ctx.getImageData(0, 0, probe.width, probe.height)
      let brightArc = 0
      let total = 0
      for (let i = 0; i < data.length; i += 4) {
        total += 1
        const r = data[i]!
        const g = data[i + 1]!
        const b = data[i + 2]!
        // bright + cool/blue-leaning + clearly above the #050505 world floor.
        if (b > 150 && g > 120 && b >= r && r + g + b > 360) brightArc += 1
      }
      return {
        ok: brightArc > 0,
        brightArc,
        total,
        ratio: total === 0 ? 0 : brightArc / total,
        width: probe.width,
        height: probe.height,
      }
    }, Array.from(shot))

    if (errors.length > 0) console.warn(`page errors (spawn=${spawn}):`, errors)
    return { boot, analysis }
  } finally {
    await browser.close()
  }
}

try {
  const spawned = await analysePage(true, screenshotPath)
  const baseline = await analysePage(false, baselinePath)

  console.log("spawned boot hook:", JSON.stringify(spawned.boot))
  console.log("spawned arc analysis:", JSON.stringify(spawned.analysis))
  console.log("baseline (no-spawn) analysis:", JSON.stringify(baseline.analysis))

  const boot = spawned.boot as { arcBeamCount?: number; mounted?: boolean } | undefined
  if (boot?.mounted !== true) fail("Harness did not mount the visualization.")
  if ((boot?.arcBeamCount ?? 0) < 1) {
    fail("Spawned visualization has no crackling_arc beam.")
  }

  // The arc must add a meaningful cluster of bright pixels that the identical
  // world WITHOUT the spawned scene does not have.
  const ARC_PIXEL_FLOOR = 400
  if (spawned.analysis.brightArc < ARC_PIXEL_FLOOR) {
    fail(
      `Crackling arc not visibly present: only ${spawned.analysis.brightArc} bright arc pixels (floor ${ARC_PIXEL_FLOOR}).`,
    )
  }
  if (spawned.analysis.brightArc <= baseline.analysis.brightArc) {
    fail(
      `Spawned arc (${spawned.analysis.brightArc}) is not brighter than the no-spawn baseline (${baseline.analysis.brightArc}).`,
    )
  }

  const delta = spawned.analysis.brightArc - baseline.analysis.brightArc
  console.log(
    `PASS: visible crackling arc. ${spawned.analysis.brightArc} bright arc pixels ` +
      `(baseline ${baseline.analysis.brightArc}, delta +${delta}). Screenshot: ${screenshotPath}`,
  )
} finally {
  server.close()
}
