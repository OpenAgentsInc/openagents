#!/usr/bin/env bun
import { mkdir, rm, writeFile } from "node:fs/promises"
import { dirname, join, resolve } from "node:path"

import { chromium, type Browser, type Page } from "playwright"

export type ComposerVisualViewport = Readonly<{
  name: "desktop" | "mobile"
  width: number
  height: number
}>

export type ComposerVisualTarget = Readonly<{
  name: "khala-code-desktop" | "openagents-khala-chat" | "openagents-autopilot-hud"
  app: "khala-code-desktop" | "openagents-web"
  path: string
  composerSelector: string
  inputSelector: string
  footerSelector: string
  canvasSelector: string | null
}>

export type ComposerVisualPlan = Readonly<{
  prompt: string
  targets: ReadonlyArray<ComposerVisualTarget>
  viewports: ReadonlyArray<ComposerVisualViewport>
}>

type Rect = Readonly<{
  x: number
  y: number
  width: number
  height: number
}>

type CanvasProbe = Readonly<{
  found: boolean
  width: number
  height: number
  sampledPixels: number
  nonBlankPixels: number
}>

type GeometryProbe = Readonly<{
  composer: Rect
  footer: Rect
  input: Rect
}>

export type ComposerVisualCaptureResult = Readonly<{
  target: string
  viewport: string
  screenshot: string
  geometry: GeometryProbe
  canvas: CanvasProbe | null
  reducedMotion: boolean
}>

export const COMPOSER_VISUAL_SAFE_PROMPT =
  "Synthetic visual smoke prompt: summarize the public onboarding flow."

export const composerVisualPlan = (): ComposerVisualPlan => ({
  prompt: COMPOSER_VISUAL_SAFE_PROMPT,
  targets: [
    {
      name: "khala-code-desktop",
      app: "khala-code-desktop",
      path: "/",
      composerSelector: "#composer-form",
      inputSelector: "#composer-input",
      footerSelector: "#composer-form .oa-ai-command-composer-footer",
      canvasSelector: "#composer-hud canvas",
    },
    {
      name: "openagents-khala-chat",
      app: "openagents-web",
      path: "/chat",
      composerSelector: "[data-khala-chat-composer]",
      inputSelector: "[data-oa-command-composer-textarea]",
      footerSelector:
        "[data-khala-chat-composer] .oa-ai-command-composer-footer",
      canvasSelector: null,
    },
    {
      name: "openagents-autopilot-hud",
      app: "openagents-web",
      path: "/autopilot",
      composerSelector: "[data-autopilot-onboarding-composer]",
      inputSelector:
        "[data-autopilot-onboarding-composer] .oa-ai-prompt-input-textarea",
      footerSelector:
        "[data-autopilot-onboarding-composer] .oa-ai-prompt-input-footer",
      canvasSelector: "oa-landing-squares",
    },
  ],
  viewports: [
    { name: "desktop", width: 1280, height: 800 },
    { name: "mobile", width: 390, height: 844 },
  ],
})

export const validatePublicSafeComposerPrompt = (prompt: string): boolean => {
  const lower = prompt.toLowerCase()
  return (
    !lower.includes("secret") &&
    !lower.includes("token") &&
    !lower.includes("private") &&
    !lower.includes("/users/") &&
    !lower.includes("~/.") &&
    !/[A-Za-z0-9_=-]{32,}/.test(prompt)
  )
}

export const assertComposerGeometry = (probe: GeometryProbe): void => {
  if (probe.composer.width < 280 || probe.composer.height < 72) {
    throw new Error("composer frame is too small for a stable capture")
  }
  if (probe.input.width <= 0 || probe.input.height <= 0) {
    throw new Error("composer input is not visible")
  }
  if (probe.footer.width <= 0 || probe.footer.height <= 0) {
    throw new Error("composer footer controls are not visible")
  }
  const footerBottom = probe.footer.y + probe.footer.height
  const composerBottom = probe.composer.y + probe.composer.height
  if (footerBottom > composerBottom + 1) {
    throw new Error("composer footer controls overflow the composer frame")
  }
  const footerStartsBeforeInputEnds =
    probe.footer.y < probe.input.y + probe.input.height - 1
  const columnsOverlap =
    probe.footer.x < probe.input.x + probe.input.width &&
    probe.input.x < probe.footer.x + probe.footer.width
  if (footerStartsBeforeInputEnds && columnsOverlap) {
    throw new Error("composer footer controls overlap the input")
  }
}

export const assertCanvasProbe = (
  targetName: string,
  probe: CanvasProbe | null,
): void => {
  if (probe === null) return
  if (!probe.found) throw new Error(`${targetName} canvas was not found`)
  if (probe.width < 10 || probe.height < 10) {
    throw new Error(`${targetName} canvas has invalid geometry`)
  }
  if (probe.sampledPixels < 1 || probe.nonBlankPixels < 1) {
    throw new Error(`${targetName} canvas/HUD pixels are blank`)
  }
}

export async function runComposerVisualSmoke(
  options: Readonly<{
    outDir: string
    keepServers?: boolean
  }>,
): Promise<ReadonlyArray<ComposerVisualCaptureResult>> {
  const plan = composerVisualPlan()
  if (!validatePublicSafeComposerPrompt(plan.prompt)) {
    throw new Error("composer visual smoke prompt failed the public-safe guard")
  }

  await rm(options.outDir, { force: true, recursive: true })
  await mkdir(options.outDir, { recursive: true })

  const repoRoot = resolve(import.meta.dir, "../../..")
  const khalaCodePort = 50021
  const openagentsPort = 5192
  const servers = [
    startViteServer({
      cwd: join(repoRoot, "clients/khala-code-desktop"),
      port: khalaCodePort,
      label: "khala-code-desktop",
    }),
    startViteServer({
      cwd: join(repoRoot, "apps/openagents.com/apps/web"),
      port: openagentsPort,
      label: "openagents-web",
    }),
  ]

  let browser: Browser | null = null
  try {
    await Promise.all([
      waitForHttp(`http://127.0.0.1:${khalaCodePort}/`),
      waitForHttp(`http://127.0.0.1:${openagentsPort}/`),
    ])
    browser = await chromium.launch({ headless: true })
    const results: ComposerVisualCaptureResult[] = []
    for (const target of plan.targets) {
      for (const viewport of plan.viewports) {
        const baseUrl =
          target.app === "khala-code-desktop"
            ? `http://127.0.0.1:${khalaCodePort}`
            : `http://127.0.0.1:${openagentsPort}`
        const reducedMotion = viewport.name === "mobile"
        const page = await browser.newPage({
          viewport: { width: viewport.width, height: viewport.height },
          colorScheme: "dark",
          reducedMotion: reducedMotion ? "reduce" : "no-preference",
        })
        try {
          const result = await captureTarget(page, {
            baseUrl,
            outDir: options.outDir,
            prompt: plan.prompt,
            reducedMotion,
            target,
            viewport,
          })
          results.push(result)
        } finally {
          await page.close()
        }
      }
    }
    const summaryPath = join(options.outDir, "summary.json")
    await writeFile(summaryPath, `${JSON.stringify({ results }, null, 2)}\n`)
    return results
  } finally {
    if (browser !== null) await browser.close()
    if (options.keepServers !== true) {
      for (const server of servers) server.kill()
    }
  }
}

async function captureTarget(
  page: Page,
  input: Readonly<{
    baseUrl: string
    outDir: string
    prompt: string
    reducedMotion: boolean
    target: ComposerVisualTarget
    viewport: ComposerVisualViewport
  }>,
): Promise<ComposerVisualCaptureResult> {
  await page.goto(`${input.baseUrl}${input.target.path}`, {
    waitUntil: "domcontentloaded",
  })
  await page.locator(input.target.composerSelector).waitFor({ state: "visible" })
  const inputLocator = page.locator(input.target.inputSelector).first()
  await inputLocator.fill(input.prompt)
  await inputLocator.focus()
  await page.waitForTimeout(input.reducedMotion ? 350 : 750)

  const geometry = await page.evaluate(selectors => {
    const rectFor = (selector: string): Rect => {
      const element = document.querySelector(selector)
      if (!(element instanceof HTMLElement)) {
        throw new Error(`missing visual smoke selector: ${selector}`)
      }
      const rect = element.getBoundingClientRect()
      return {
        x: rect.x,
        y: rect.y,
        width: rect.width,
        height: rect.height,
      }
    }
    return {
      composer: rectFor(selectors.composerSelector),
      footer: rectFor(selectors.footerSelector),
      input: rectFor(selectors.inputSelector),
    }
  }, input.target)
  assertComposerGeometry(geometry)

  let canvas =
    input.target.canvasSelector === null
      ? null
      : await page.evaluate(canvasSelector => {
          const selected = document.querySelector(canvasSelector)
          const canvas =
            selected instanceof HTMLCanvasElement
              ? selected
              : selected?.shadowRoot?.querySelector("canvas") ?? null
          if (!(canvas instanceof HTMLCanvasElement)) {
            return {
              found: false,
              height: 0,
              nonBlankPixels: 0,
              sampledPixels: 0,
              width: 0,
            }
          }
          const width = canvas.width
          const height = canvas.height
          const sampleWidth = Math.max(1, Math.min(48, width))
          const sampleHeight = Math.max(1, Math.min(48, height))
          const regions = [
            [0, 0],
            [Math.max(0, width - sampleWidth), 0],
            [0, Math.max(0, height - sampleHeight)],
            [
              Math.max(0, width - sampleWidth),
              Math.max(0, height - sampleHeight),
            ],
            [
              Math.max(0, Math.floor((width - sampleWidth) / 2)),
              Math.max(0, Math.floor((height - sampleHeight) / 2)),
            ],
          ] as const
          const webgl =
            canvas.getContext("webgl2", { preserveDrawingBuffer: true }) ??
            canvas.getContext("webgl", { preserveDrawingBuffer: true })
          if (webgl !== null) {
            const pixels = new Uint8Array(regions.length * sampleWidth * sampleHeight * 4)
            regions.forEach(([x, y], index) => {
              const offset = index * sampleWidth * sampleHeight * 4
              webgl.readPixels(
                x,
                y,
                sampleWidth,
                sampleHeight,
                webgl.RGBA,
                webgl.UNSIGNED_BYTE,
                pixels.subarray(offset, offset + sampleWidth * sampleHeight * 4),
              )
            })
            return canvasProbeFromPixels(width, height, pixels)
          }
          const context = canvas.getContext("2d")
          if (context === null) {
            return {
              found: true,
              height,
              nonBlankPixels: 0,
              sampledPixels: 0,
              width,
            }
          }
          const pixels = new Uint8ClampedArray(regions.length * sampleWidth * sampleHeight * 4)
          regions.forEach(([x, y], index) => {
            const image = context.getImageData(x, y, sampleWidth, sampleHeight)
            pixels.set(image.data, index * sampleWidth * sampleHeight * 4)
          })
          return canvasProbeFromPixels(width, height, pixels)

          function canvasProbeFromPixels(
            canvasWidth: number,
            canvasHeight: number,
            pixels: Uint8Array | Uint8ClampedArray,
          ): CanvasProbe {
            let nonBlankPixels = 0
            for (let index = 0; index < pixels.length; index += 4) {
              const visible = pixels[index + 3] > 0
              const lit =
                pixels[index] > 8 || pixels[index + 1] > 8 || pixels[index + 2] > 8
              if (visible && lit) {
                nonBlankPixels += 1
              }
            }
            return {
              found: true,
              height: canvasHeight,
              nonBlankPixels,
              sampledPixels: pixels.length / 4,
              width: canvasWidth,
            }
          }
        }, input.target.canvasSelector)
  if (canvas !== null && canvas.found && canvas.nonBlankPixels < 1) {
    canvas = await screenshotPixelProbe(page, input.target.composerSelector)
  }
  assertCanvasProbe(input.target.name, canvas)

  const screenshot = join(
    input.outDir,
    `${input.target.name}-${input.viewport.name}.png`,
  )
  await mkdir(dirname(screenshot), { recursive: true })
  await page.screenshot({ fullPage: false, path: screenshot })

  return {
    target: input.target.name,
    viewport: input.viewport.name,
    screenshot,
    geometry,
    canvas,
    reducedMotion: input.reducedMotion,
  }
}

async function screenshotPixelProbe(
  page: Page,
  selector: string,
): Promise<CanvasProbe> {
  const buffer = await page.locator(selector).first().screenshot()
  const dataUrl = `data:image/png;base64,${buffer.toString("base64")}`
  return await page.evaluate(async url => {
    const image = new Image()
    const loaded = new Promise<void>((resolve, reject) => {
      image.onload = () => resolve()
      image.onerror = () => reject(new Error("failed to decode screenshot"))
    })
    image.src = url
    await loaded
    const canvas = document.createElement("canvas")
    canvas.width = image.naturalWidth
    canvas.height = image.naturalHeight
    const context = canvas.getContext("2d")
    if (context === null) {
      return {
        found: true,
        height: canvas.height,
        nonBlankPixels: 0,
        sampledPixels: 0,
        width: canvas.width,
      }
    }
    context.drawImage(image, 0, 0)
    const imageData = context.getImageData(0, 0, canvas.width, canvas.height)
    let nonBlankPixels = 0
    for (let index = 0; index < imageData.data.length; index += 4) {
      const visible = imageData.data[index + 3] > 0
      const lit =
        imageData.data[index] > 8 ||
        imageData.data[index + 1] > 8 ||
        imageData.data[index + 2] > 8
      if (visible && lit) nonBlankPixels += 1
    }
    return {
      found: true,
      height: canvas.height,
      nonBlankPixels,
      sampledPixels: imageData.data.length / 4,
      width: canvas.width,
    }
  }, dataUrl)
}

type ViteServer = Readonly<{
  kill: () => void
}>

function startViteServer(
  input: Readonly<{ cwd: string; port: number; label: string }>,
): ViteServer {
  const proc = Bun.spawn(
    [
      "bunx",
      "vite",
      "--host",
      "127.0.0.1",
      "--port",
      String(input.port),
      "--strictPort",
    ],
    {
      cwd: input.cwd,
      stderr: "pipe",
      stdout: "pipe",
    },
  )
  void streamServerOutput(input.label, proc.stdout)
  void streamServerOutput(input.label, proc.stderr)
  return {
    kill: () => {
      proc.kill()
    },
  }
}

async function streamServerOutput(
  label: string,
  stream: ReadableStream<Uint8Array>,
): Promise<void> {
  const reader = stream.getReader()
  const decoder = new TextDecoder()
  try {
    for (;;) {
      const chunk = await reader.read()
      if (chunk.done) return
      const text = decoder.decode(chunk.value, { stream: true })
      for (const line of text.split("\n")) {
        if (line.trim().length > 0) console.error(`[${label}] ${line}`)
      }
    }
  } catch {
    // Server output is diagnostic only.
  }
}

async function waitForHttp(url: string): Promise<void> {
  const deadline = Date.now() + 30_000
  let lastError: unknown = null
  while (Date.now() < deadline) {
    try {
      const response = await fetch(url)
      if (response.ok) return
      lastError = new Error(`${url} returned ${response.status}`)
    } catch (error) {
      lastError = error
    }
    await Bun.sleep(250)
  }
  throw new Error(`Timed out waiting for ${url}: ${String(lastError)}`)
}

if (import.meta.main) {
  const outDir =
    argValue(Bun.argv.slice(2), "--out") ??
    resolve("var/khala-code-desktop/composer-visual-smoke")
  try {
    const results = await runComposerVisualSmoke({ outDir })
    console.log(JSON.stringify({ ok: true, outDir, results }, null, 2))
  } catch (error) {
    console.error(error instanceof Error ? error.stack ?? error.message : error)
    process.exit(1)
  }
}

function argValue(args: ReadonlyArray<string>, name: string): string | undefined {
  const index = args.indexOf(name)
  if (index === -1) return undefined
  return args[index + 1]
}
