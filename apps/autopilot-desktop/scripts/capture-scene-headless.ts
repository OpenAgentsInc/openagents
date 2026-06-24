import { mkdirSync, writeFileSync } from "node:fs"

import { chromium } from "playwright"

import {
  decodePng,
  renderVisualizationAndProbe,
  resolveChromePathOrNull,
} from "../src/testing/headless-pixel.js"

import {
  captureOutputDir,
  parseSceneCaptureArgs,
  type SceneCaptureTarget,
} from "./isolated-scenes/capture-target.js"
import {
  assertSceneRendered,
  defaultSceneRenderSignature,
} from "./isolated-scenes/render-gate.js"

const parsed = parseSceneCaptureArgs(process.argv.slice(2))
if (!parsed.ok) {
  console.error(parsed.message)
  process.exit(1)
}

const ensureChrome = (): void => {
  if (resolveChromePathOrNull() === null) {
    console.error("No Chromium binary found. Set CHROME_PATH to a Chromium-family binary.")
    process.exit(2)
  }
}

const captureRegisteredScene = async (
  target: Extract<SceneCaptureTarget, { kind: "registered-scene" }>,
): Promise<Record<string, unknown>> => {
  ensureChrome()
  mkdirSync(captureOutputDir(target), { recursive: true })
  const result = await renderVisualizationAndProbe({
    entryModulePath: target.scene.entryModulePath,
    width: target.scene.defaultWidth,
    height: target.scene.defaultHeight,
    frameSteps: target.scene.defaultFrameSteps,
    frameDeltaMs: target.scene.defaultFrameDeltaMs,
    ...(target.pageQuery === undefined ? {} : { pageQuery: target.pageQuery }),
  })
  const renderGate = assertSceneRendered(result.image, target.scene.renderSignature)
  writeFileSync(target.outputPath, Buffer.from(result.screenshotBase64, "base64"))
  return {
    ok: true,
    kind: target.kind,
    scene: target.scene.name,
    outputPath: target.outputPath,
    canvas: { width: result.canvasWidth, height: result.canvasHeight },
    framesAdvanced: result.framesAdvanced,
    renderGate,
  }
}

const captureUrl = async (
  target: Extract<SceneCaptureTarget, { kind: "url" }>,
): Promise<Record<string, unknown>> => {
  ensureChrome()
  mkdirSync(captureOutputDir(target), { recursive: true })
  const browser = await chromium.launch({ headless: true })
  try {
    const page = await browser.newPage({ viewport: { width: 960, height: 540 } })
    await page.goto(target.url, { waitUntil: "networkidle" })
    await page.waitForSelector("#scene canvas, oa-training-run canvas, canvas", {
      timeout: 15_000,
    })
    const waitMs = Number(process.env.OA_SCENE_CAPTURE_WAIT_MS ?? "1800")
    if (Number.isFinite(waitMs) && waitMs > 0) await page.waitForTimeout(waitMs)
    const screenshot = await page.screenshot()
    const renderGate = assertSceneRendered(
      decodePng(Buffer.from(screenshot)),
      defaultSceneRenderSignature,
    )
    writeFileSync(target.outputPath, screenshot)
    return {
      ok: true,
      kind: target.kind,
      url: target.url,
      outputPath: target.outputPath,
      waitMs,
      renderGate,
    }
  } finally {
    await browser.close()
  }
}

const summary =
  parsed.target.kind === "registered-scene"
    ? await captureRegisteredScene(parsed.target)
    : await captureUrl(parsed.target)

console.log(JSON.stringify(summary, null, 2))
process.exit(0)
