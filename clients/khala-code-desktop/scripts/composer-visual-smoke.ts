#!/usr/bin/env bun
import { mkdir, rm, writeFile } from "node:fs/promises"
import { dirname, join, resolve } from "node:path"

import { chromium, type Browser, type Page } from "playwright"
import {
  findKhalaQaAvailablePort as findAvailablePort,
  installKhalaQaConsoleErrorOracle,
  startKhalaQaViteServer as startViteServer,
  waitForKhalaQaHttp as waitForHttp,
} from "@openagentsinc/khala-qa-harness/desktop-smoke-helpers"
import {
  assertKhalaVisualBaseline,
  type KhalaVisualBaselineResult,
} from "@openagentsinc/khala-qa-harness/visual-baseline"
import {
  defaultKhalaCodeVisualBaselineOptions,
  khalaCodeVisualBaselineOptionsFromArgs,
  type KhalaCodeVisualBaselineOptions,
} from "./visual-baseline-options"
import { installKhalaCodeVisualSmokeRpcMocks } from "./visual-smoke-rpc-mocks"

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

export type FocusProbe = Readonly<{
  activeElementMatchesInput: boolean
  borderColor: string
  boxShadow: string
  focusedBorderColor: string
  hasVisibleFrame: boolean
}>

export type ReducedMotionProbe = Readonly<{
  matchesMedia: boolean
  transitionDurationMs: number
}>

export type CanvasProbe = Readonly<{
  found: boolean
  width: number
  height: number
  sampledPixels: number
  nonBlankPixels: number
}>

export type GeometryProbe = Readonly<{
  composer: Rect
  footerChildren: ReadonlyArray<Rect>
  footer: Rect
  input: Rect
  viewport: Rect
}>

export type ComposerVisualCaptureResult = Readonly<{
  target: string
  viewport: string
  screenshot: string
  visualBaseline: KhalaVisualBaselineResult
  geometry: GeometryProbe
  focus: FocusProbe
  canvas: CanvasProbe | null
  reducedMotionProbe: ReducedMotionProbe
  reducedMotion: boolean
}>

export const COMPOSER_VISUAL_SAFE_PROMPT =
  "Synthetic visual smoke prompt: summarize the public onboarding flow."
export const COMPOSER_VISUAL_SMOKE_HARNESS = "preview_ui_codex_harness_shell"

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

const khalaPreviewFallbackPorts = (preferredPort: number): ReadonlyArray<number> =>
  Array.from({ length: 10 }, (_, index) => 50021 + index)
    .filter(port => port !== preferredPort)

export const assertComposerGeometry = (probe: GeometryProbe): void => {
  if (probe.composer.width < 280 || probe.composer.height < 72) {
    throw new Error("composer frame is too small for a stable capture")
  }
  if (probe.composer.x < -1 || probe.composer.y < -1) {
    throw new Error("composer frame is clipped outside the viewport")
  }
  const composerRight = probe.composer.x + probe.composer.width
  const viewportRight = probe.viewport.x + probe.viewport.width
  if (composerRight > viewportRight + 1) {
    throw new Error("composer frame overflows the viewport width")
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
  for (let index = 0; index < probe.footerChildren.length; index += 1) {
    const left = probe.footerChildren[index]
    if (left === undefined) continue
    for (
      let compareIndex = index + 1;
      compareIndex < probe.footerChildren.length;
      compareIndex += 1
    ) {
      const right = probe.footerChildren[compareIndex]
      if (right === undefined) continue
      const overlapX =
        left.x < right.x + right.width - 1 &&
        right.x < left.x + left.width - 1
      const overlapY =
        left.y < right.y + right.height - 1 &&
        right.y < left.y + left.height - 1
      if (overlapX && overlapY) {
        throw new Error("composer footer controls overlap each other")
      }
    }
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

export const assertFocusProbe = (
  targetName: string,
  probe: FocusProbe,
): void => {
  if (!probe.activeElementMatchesInput) {
    throw new Error(`${targetName} input did not retain focus`)
  }
  if (!probe.hasVisibleFrame) {
    throw new Error(`${targetName} composer frame has no visible focus border`)
  }
  if (
    probe.borderColor === "rgba(0, 0, 0, 0)" &&
    (probe.boxShadow === "" || probe.boxShadow === "none")
  ) {
    throw new Error(`${targetName} composer focus frame is transparent`)
  }
}

export const assertReducedMotionProbe = (
  targetName: string,
  probe: ReducedMotionProbe,
  expectedReducedMotion: boolean,
): void => {
  if (probe.matchesMedia !== expectedReducedMotion) {
    throw new Error(`${targetName} reduced-motion media query mismatch`)
  }
  if (expectedReducedMotion && probe.transitionDurationMs > 0) {
    throw new Error(`${targetName} keeps transitions under reduced motion`)
  }
}

export async function runComposerVisualSmoke(
  options: Readonly<{
    outDir: string
    keepServers?: boolean
    visualBaseline?: KhalaCodeVisualBaselineOptions
  }>,
): Promise<ReadonlyArray<ComposerVisualCaptureResult>> {
  const plan = composerVisualPlan()
  if (!validatePublicSafeComposerPrompt(plan.prompt)) {
    throw new Error("composer visual smoke prompt failed the public-safe guard")
  }

  await rm(options.outDir, { force: true, recursive: true })
  await mkdir(options.outDir, { recursive: true })

  const repoRoot = resolve(import.meta.dir, "../../..")
  const khalaCodePort = await findAvailablePort(50021, khalaPreviewFallbackPorts(50021))
  const openagentsPort = await findAvailablePort(5192)
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
    const visualBaseline = options.visualBaseline ?? defaultKhalaCodeVisualBaselineOptions()
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
        const consoleOracle = installKhalaQaConsoleErrorOracle(page, {
          label: `${COMPOSER_VISUAL_SMOKE_HARNESS}.${target.name}.${viewport.name}`,
        })
        try {
          if (target.app === "khala-code-desktop") {
            await installKhalaCodeVisualSmokeRpcMocks(page)
          }
          const result = await captureTarget(page, {
            baseUrl,
            outDir: options.outDir,
            prompt: plan.prompt,
            reducedMotion,
            target,
            visualBaseline,
            viewport,
          })
          consoleOracle.assertNoUnexpected()
          results.push(result)
        } catch (error) {
          consoleOracle.assertNoUnexpected()
          throw error
        } finally {
          await page.close()
        }
      }
    }
    const summaryPath = join(options.outDir, "summary.json")
    await writeFile(summaryPath, `${JSON.stringify({
      harness: COMPOSER_VISUAL_SMOKE_HARNESS,
      results,
    }, null, 2)}\n`)
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
    visualBaseline: KhalaCodeVisualBaselineOptions
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
    const footer = document.querySelector(selectors.footerSelector)
    if (!(footer instanceof HTMLElement)) {
      throw new Error(`missing visual smoke selector: ${selectors.footerSelector}`)
    }
    const childRects = Array.from(footer.children)
      .filter((element): element is HTMLElement => element instanceof HTMLElement)
      .filter(element => {
        const style = window.getComputedStyle(element)
        return style.display !== "none" && style.visibility !== "hidden"
      })
      .map(element => {
        const rect = element.getBoundingClientRect()
        return {
          x: rect.x,
          y: rect.y,
          width: rect.width,
          height: rect.height,
        }
      })
      .filter(rect => rect.width > 0 && rect.height > 0)
    return {
      composer: rectFor(selectors.composerSelector),
      footer: rectFor(selectors.footerSelector),
      footerChildren: childRects,
      input: rectFor(selectors.inputSelector),
      viewport: {
        x: 0,
        y: 0,
        width: window.innerWidth,
        height: window.innerHeight,
      },
    }
  }, input.target)
  assertComposerGeometry(geometry)

  const focus = await page.evaluate(selectors => {
    const input = document.querySelector(selectors.inputSelector)
    const composer = document.querySelector(selectors.composerSelector)
    const frame =
      composer?.querySelector(".oa-ai-command-composer-frame") ??
      composer?.querySelector(".oa-ai-prompt-input") ??
      composer
    if (!(frame instanceof HTMLElement)) {
      throw new Error("missing composer focus frame")
    }
    const style = window.getComputedStyle(frame)
    const focusedBorderColor = style.getPropertyValue(
      "--oa-command-composer-focus",
    ).trim()
    const borderWidths = [
      style.borderTopWidth,
      style.borderRightWidth,
      style.borderBottomWidth,
      style.borderLeftWidth,
    ].map(value => Number.parseFloat(value))
    return {
      activeElementMatchesInput: input === document.activeElement,
      borderColor: style.borderTopColor,
      boxShadow: style.boxShadow,
      focusedBorderColor,
      hasVisibleFrame:
        borderWidths.some(width => Number.isFinite(width) && width > 0) &&
        style.borderTopStyle !== "none" &&
        style.borderTopColor !== "rgba(0, 0, 0, 0)",
    }
  }, input.target)
  assertFocusProbe(input.target.name, focus)

  const reducedMotionProbe = await page.evaluate(selectors => {
    const composer = document.querySelector(selectors.composerSelector)
    const frame =
      composer?.querySelector(".oa-ai-command-composer-frame") ??
      composer?.querySelector(".oa-ai-prompt-input") ??
      composer
    if (!(frame instanceof HTMLElement)) {
      throw new Error("missing composer motion frame")
    }
    const style = window.getComputedStyle(frame)
    const durations = style.transitionDuration
      .split(",")
      .map(value => value.trim())
      .map(value =>
        value.endsWith("ms")
          ? Number.parseFloat(value)
          : value.endsWith("s")
            ? Number.parseFloat(value) * 1000
            : Number.parseFloat(value),
      )
      .filter(value => Number.isFinite(value))
    return {
      matchesMedia: window.matchMedia("(prefers-reduced-motion: reduce)")
        .matches,
      transitionDurationMs: Math.max(0, ...durations),
    }
  }, input.target)
  assertReducedMotionProbe(
    input.target.name,
    reducedMotionProbe,
    input.reducedMotion,
  )

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
  await page.screenshot({
    animations: "disabled",
    caret: "hide",
    fullPage: false,
    path: screenshot,
  })
  const visualBaseline = await assertKhalaVisualBaseline({
    baselineDir: input.visualBaseline.baselineDir,
    bless: input.visualBaseline.bless,
    capture: {
      colorScheme: "dark",
      harness: COMPOSER_VISUAL_SMOKE_HARNESS,
      id: `composer.${input.target.name}.${input.viewport.name}`,
      reducedMotion: input.reducedMotion ? "reduce" : "no-preference",
      screenshotPath: screenshot,
      viewport: input.viewport.name,
    },
    requireBaseline: input.visualBaseline.requireBaseline,
  })

  return {
    target: input.target.name,
    viewport: input.viewport.name,
    screenshot,
    visualBaseline,
    geometry,
    focus,
    canvas,
    reducedMotionProbe,
    reducedMotion: input.reducedMotion,
  }
}

async function screenshotPixelProbe(
  page: Page,
  selector: string,
): Promise<CanvasProbe> {
  const buffer = await page.locator(selector).first().screenshot({
    animations: "disabled",
    caret: "hide",
  })
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

if (import.meta.main) {
  const args = Bun.argv.slice(2)
  const outDir =
    argValue(args, "--out") ??
    resolve("var/khala-code-desktop/composer-visual-smoke")
  try {
    const results = await runComposerVisualSmoke({
      outDir,
      visualBaseline: khalaCodeVisualBaselineOptionsFromArgs(args),
    })
    console.log(JSON.stringify({
      harness: COMPOSER_VISUAL_SMOKE_HARNESS,
      ok: true,
      outDir,
      results,
    }, null, 2))
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
