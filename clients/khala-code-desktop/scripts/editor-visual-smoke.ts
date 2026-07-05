#!/usr/bin/env bun
import { mkdir, rm, writeFile } from "node:fs/promises"
import { basename, dirname, join, relative, resolve } from "node:path"

import { chromium, type Browser, type Page } from "playwright"
import {
  assertKhalaQaVisibleRect as assertVisibleRect,
  findKhalaQaAvailablePort as findAvailablePort,
  installKhalaQaConsoleErrorOracle,
  khalaQaRectsOverlap as rectsOverlap,
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
import {
  assertKhalaCodePagePublicSafe,
  assertKhalaCodePublicSafeValue,
} from "./public-safety-oracle"
import { installKhalaCodeVisualSmokeRpcMocks } from "./visual-smoke-rpc-mocks"

export type EditorVisualViewport = Readonly<{
  name: "desktop"
  width: number
  height: number
}>

type Rect = Readonly<{
  x: number
  y: number
  width: number
  height: number
}>

export type EditorVisualGeometry = Readonly<{
  editorPanel: Rect
  monacoHost: Rect
  sourcePane: Rect
  treePane: Rect
  treeRow: Rect
  viewport: Rect
}>

export type EditorMonacoDomProbe = Readonly<{
  hasFixtureSource: boolean
  lineCount: number
  text: string
}>

export type EditorPixelProbe = Readonly<{
  height: number
  nonBlankPixels: number
  sampledPixels: number
  width: number
}>

export type EditorVisualCaptureResult = Readonly<{
  chatDraftPreserved: boolean
  geometry: EditorVisualGeometry
  monacoProbe: EditorMonacoDomProbe
  monacoResourceCountAfterOpen: number
  monacoResourceCountBeforeOpen: number
  pixelProbe: EditorPixelProbe
  screenshot: string
  visualBaseline: KhalaVisualBaselineResult
  viewport: EditorVisualViewport["name"]
}>

export const EDITOR_VISUAL_SMOKE_HARNESS =
  "khala_code_phase1_editor_visual_smoke"

const providerId = "editor-visual-smoke-provider"
const rootPath = "/fixture"
const srcPath = "/fixture/src"
const readmePath = "/fixture/README.md"
const mainPath = "/fixture/src/main.ts"
const fixtureSource = [
  "export const answer = 42",
  "",
  "export function greet(name: string): string {",
  "  return `hello ${name}`",
  "}",
  "",
].join("\n")

export const editorVisualSmokeViewports = (): ReadonlyArray<EditorVisualViewport> => [
  { name: "desktop", width: 1280, height: 800 },
]

const khalaPreviewFallbackPorts = (preferredPort: number): ReadonlyArray<number> =>
  Array.from({ length: 10 }, (_, index) => 50021 + index)
    .filter(port => port !== preferredPort)

export const assertEditorVisualGeometry = (
  geometry: EditorVisualGeometry,
): void => {
  assertVisibleRect("Editor panel", geometry.editorPanel, geometry.viewport)
  assertVisibleRect("Editor tree", geometry.treePane, geometry.viewport)
  assertVisibleRect("Editor source pane", geometry.sourcePane, geometry.viewport)
  assertVisibleRect("Editor Monaco host", geometry.monacoHost, geometry.viewport)
  assertVisibleRect("Editor file row", geometry.treeRow, geometry.viewport)

  if (geometry.treePane.width < 180) {
    throw new Error("Editor tree pane is too narrow for stable file rows")
  }
  if (geometry.sourcePane.width < 480) {
    throw new Error("Editor source pane is too narrow for readable source")
  }
  if (geometry.monacoHost.height < 320) {
    throw new Error("Editor Monaco host is too short for a nonblank source capture")
  }
  if (rectsOverlap(geometry.treePane, geometry.sourcePane)) {
    throw new Error("Editor tree and source panes overlap")
  }
}

export const assertEditorMonacoDomProbe = (
  probe: EditorMonacoDomProbe,
): void => {
  if (probe.lineCount < 3) {
    throw new Error("Editor Monaco source rendered too few lines")
  }
  if (!probe.hasFixtureSource) {
    throw new Error("Editor Monaco source did not render the fixture source")
  }
}

export const assertEditorPixelProbe = (probe: EditorPixelProbe): void => {
  if (probe.width < 480 || probe.height < 320) {
    throw new Error("Editor screenshot probe is too small")
  }
  if (probe.sampledPixels < 1 || probe.nonBlankPixels < 1) {
    throw new Error("Editor screenshot probe is blank")
  }
}

export async function runEditorVisualSmoke(
  options: Readonly<{
    keepServer?: boolean
    outDir: string
    visualBaseline?: KhalaCodeVisualBaselineOptions
  }>,
): Promise<ReadonlyArray<EditorVisualCaptureResult>> {
  await rm(options.outDir, { force: true, recursive: true })
  await mkdir(options.outDir, { recursive: true })

  const repoRoot = resolve(import.meta.dir, "../../..")
  const port = await findAvailablePort(50026, khalaPreviewFallbackPorts(50026))
  const server = startViteServer({
    cwd: join(repoRoot, "clients/khala-code-desktop"),
    label: "khala-code-desktop-editor",
    port,
  })
  let browser: Browser | null = null
  try {
    await waitForHttp(`http://127.0.0.1:${port}/`)
    browser = await chromium.launch({ headless: true })
    const results: EditorVisualCaptureResult[] = []
    const visualBaseline = options.visualBaseline ?? defaultKhalaCodeVisualBaselineOptions()
    for (const viewport of editorVisualSmokeViewports()) {
      const page = await browser.newPage({
        colorScheme: "dark",
        reducedMotion: "no-preference",
        viewport: { height: viewport.height, width: viewport.width },
      })
      const consoleOracle = installKhalaQaConsoleErrorOracle(page, {
        label: `${EDITOR_VISUAL_SMOKE_HARNESS}.${viewport.name}`,
      })
      try {
        await installKhalaCodeVisualSmokeRpcMocks(page, {
          overrides: editorVisualSmokeRpcOverrides(),
        })
        const result = await captureEditor(page, {
          baseUrl: `http://127.0.0.1:${port}`,
          outDir: options.outDir,
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
    const summary = {
      harness: EDITOR_VISUAL_SMOKE_HARNESS,
      results,
    }
    assertKhalaCodePublicSafeValue(summary, "Editor visual smoke summary")
    await writeFile(join(options.outDir, "summary.json"), `${JSON.stringify(summary, null, 2)}\n`)
    return results
  } finally {
    if (browser !== null) await browser.close()
    if (options.keepServer !== true) server.kill()
  }
}

async function captureEditor(
  page: Page,
  input: Readonly<{
    baseUrl: string
    outDir: string
    visualBaseline: KhalaCodeVisualBaselineOptions
    viewport: EditorVisualViewport
  }>,
): Promise<EditorVisualCaptureResult> {
  await page.goto(`${input.baseUrl}/`, { waitUntil: "domcontentloaded" })
  await page.locator("#composer-input").waitFor({ state: "visible" })
  await page.locator("#composer-input").fill("Synthetic editor smoke draft")
  const chatDraft = await page.locator("#composer-input").evaluate(element => {
    if (element instanceof HTMLInputElement || element instanceof HTMLTextAreaElement) {
      return element.value
    }
    return element.textContent ?? ""
  })
  const chatDraftPreserved = chatDraft.includes("Synthetic editor smoke draft")
  if (!chatDraftPreserved) {
    throw new Error("Editor visual smoke expected Chat composer to remain usable before Editor opens")
  }
  const monacoResourcesBeforeOpen = await monacoResourceNames(page)
  if (monacoResourcesBeforeOpen.length > 0) {
    throw new Error("Editor visual smoke loaded Monaco before opening Editor")
  }

  await page.locator('[data-khala-code-hotbar-value="editor"]').click()
  await page.locator("#editor-panel").waitFor({ state: "visible" })
  await page.locator(`[data-path="${srcPath}"]`).waitFor({ state: "visible" })
  await page.locator(`[data-path="${srcPath}"]`).click()
  await page.locator(`[data-path="${mainPath}"]`).waitFor({ state: "visible" })
  await page.locator(`[data-path="${mainPath}"]`).click()
  await page.locator(".khala-code-editor-source-monaco .monaco-editor").waitFor({
    state: "visible",
    timeout: 20_000,
  })
  await page.locator(".monaco-editor .view-line").first().waitFor({
    state: "visible",
    timeout: 20_000,
  })
  await page.waitForTimeout(250)

  const capture = await page.evaluate(() => {
    const rectFor = (selector: string): Rect => {
      const element = document.querySelector(selector)
      if (!(element instanceof HTMLElement)) {
        throw new Error(`missing editor visual selector: ${selector}`)
      }
      const rect = element.getBoundingClientRect()
      return {
        height: rect.height,
        width: rect.width,
        x: rect.x,
        y: rect.y,
      }
    }
    const lineText = Array.from(document.querySelectorAll(".monaco-editor .view-line"))
      .map(line => line.textContent ?? "")
      .join("\n")
    return {
      geometry: {
        editorPanel: rectFor("#editor-panel"),
        monacoHost: rectFor(".khala-code-editor-source-monaco"),
        sourcePane: rectFor(".khala-code-editor-source-pane"),
        treePane: rectFor(".khala-code-editor-tree-pane"),
        treeRow: rectFor('[data-path="/fixture/src/main.ts"]'),
        viewport: {
          height: window.innerHeight,
          width: window.innerWidth,
          x: 0,
          y: 0,
        },
      },
      monacoProbe: {
        hasFixtureSource:
          lineText.includes("answer") &&
          lineText.includes("greet") &&
          lineText.includes("name"),
        lineCount: document.querySelectorAll(".monaco-editor .view-line").length,
        text: lineText.slice(0, 180),
      },
    }
  })
  assertEditorVisualGeometry(capture.geometry)
  assertEditorMonacoDomProbe(capture.monacoProbe)
  await assertKhalaCodePagePublicSafe(page, "Editor visual smoke")
  assertKhalaCodePublicSafeValue(capture, "Editor visual smoke metadata")

  const pixelProbe = await screenshotPixelProbe(page, ".khala-code-editor-source-pane")
  assertEditorPixelProbe(pixelProbe)

  const monacoResourcesAfterOpen = await monacoResourceNames(page)
  if (monacoResourcesAfterOpen.length <= monacoResourcesBeforeOpen.length) {
    throw new Error("Editor visual smoke did not observe Monaco lazy-load resources")
  }

  const screenshot = join(
    input.outDir,
    `khala-code-editor-${input.viewport.name}.png`,
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
      harness: EDITOR_VISUAL_SMOKE_HARNESS,
      id: `editor.${input.viewport.name}`,
      reducedMotion: "no-preference",
      screenshotPath: screenshot,
      viewport: input.viewport.name,
    },
    requireBaseline: input.visualBaseline.requireBaseline,
  })

  const result = {
    chatDraftPreserved,
    geometry: capture.geometry,
    monacoProbe: capture.monacoProbe,
    monacoResourceCountAfterOpen: monacoResourcesAfterOpen.length,
    monacoResourceCountBeforeOpen: monacoResourcesBeforeOpen.length,
    pixelProbe,
    screenshot: basename(screenshot),
    visualBaseline,
    viewport: input.viewport.name,
  }
  assertKhalaCodePublicSafeValue(result, "Editor visual smoke result")
  return result
}

function editorVisualSmokeRpcOverrides() {
  return {
    editorDirectoryRead: ({ args }: { readonly args: readonly unknown[] }) => {
      const request = requestObject(args[0])
      const path = typeof request.path === "string" ? request.path : rootPath
      if (path === srcPath) {
        return {
          entries: [
            editorNode(mainPath, "file", {
              depth: 2,
              name: "main.ts",
              parentPath: srcPath,
              sizeBytes: fixtureSource.length,
            }),
          ],
          node: {
            ...editorNode(srcPath, "directory", {
              depth: 1,
              name: "src",
              parentPath: rootPath,
            }),
            childrenLoaded: true,
          },
          ok: true,
          providerId,
          rootPath,
          truncated: false,
        }
      }
      return {
        entries: [
          editorNode(srcPath, "directory", { depth: 1, name: "src", parentPath: rootPath }),
          editorNode(readmePath, "file", {
            depth: 1,
            name: "README.md",
            parentPath: rootPath,
            sizeBytes: 38,
          }),
        ],
        node: {
          ...editorNode(rootPath, "directory", {
            depth: 0,
            name: "fixture",
            parentPath: null,
          }),
          childrenLoaded: true,
        },
        ok: true,
        providerId,
        rootPath,
        truncated: false,
      }
    },
    editorFileRead: ({ args }: { readonly args: readonly unknown[] }) => {
      const request = requestObject(args[0])
      if (request.path !== mainPath) {
        return {
          error: {
            code: "not_found",
            message: "Fixture file was not found.",
            path: typeof request.path === "string" ? request.path : undefined,
            providerId,
          },
          ok: false,
        }
      }
      return {
        content: fixtureSource,
        encoding: "utf8",
        mtime: null,
        ok: true,
        path: mainPath,
        providerId,
        rootPath,
        sizeBytes: fixtureSource.length,
      }
    },
    editorWorkspaceRead: () => ({
      ok: true,
      roots: [{
        label: "fixture",
        path: rootPath,
        providerId,
        readonly: true,
      }],
    }),
  }
}

const editorNode = (
  path: string,
  kind: "directory" | "file",
  input: Readonly<{
    depth: number
    name: string
    parentPath: string | null
    sizeBytes?: number | null
  }>,
) => ({
  childrenLoaded: false,
  depth: input.depth,
  kind,
  mtime: null,
  name: input.name,
  parentPath: input.parentPath,
  path,
  providerId,
  readonly: true,
  rootPath,
  sizeBytes: input.sizeBytes ?? (kind === "file" ? 0 : null),
  symlink: false,
})

const requestObject = (value: unknown): Record<string, unknown> =>
  value !== null && typeof value === "object" ? value as Record<string, unknown> : {}

const monacoResourceNames = async (page: Page): Promise<ReadonlyArray<string>> =>
  await page.evaluate(() =>
    performance
      .getEntriesByType("resource")
      .map(entry => entry.name)
      .filter(name =>
        /monaco-editor|editor\.api|editor\.main|\.worker|codicon/i.test(name),
      ),
  )

async function screenshotPixelProbe(
  page: Page,
  selector: string,
): Promise<EditorPixelProbe> {
  const buffer = await page.locator(selector).first().screenshot({
    animations: "disabled",
    caret: "hide",
  })
  const dataUrl = `data:image/png;base64,${buffer.toString("base64")}`
  return await page.evaluate(async url => {
    const image = new Image()
    const loaded = new Promise<void>((resolve, reject) => {
      image.onload = () => resolve()
      image.onerror = () => reject(new Error("failed to decode editor screenshot"))
    })
    image.src = url
    await loaded
    const canvas = document.createElement("canvas")
    canvas.width = image.naturalWidth
    canvas.height = image.naturalHeight
    const context = canvas.getContext("2d")
    if (context === null) {
      return {
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
    resolve("var/khala-code-desktop/editor-visual-smoke")
  try {
    const results = await runEditorVisualSmoke({
      outDir,
      visualBaseline: khalaCodeVisualBaselineOptionsFromArgs(args),
    })
    const cliSummary = {
      harness: EDITOR_VISUAL_SMOKE_HARNESS,
      ok: true,
      outDir: relative(process.cwd(), outDir) || ".",
      results,
    }
    assertKhalaCodePublicSafeValue(cliSummary, "Editor visual smoke CLI summary")
    console.log(JSON.stringify(cliSummary, null, 2))
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
