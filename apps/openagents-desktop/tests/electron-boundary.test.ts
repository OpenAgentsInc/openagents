/**
 * Mechanical Electron + Effect Native boundary oracle (#8574 scope 4/5).
 *
 * Source-level assertions that fail loudly if the hardened boundary or the
 * EN-only renderer discipline regresses: sandbox/contextIsolation posture in
 * the main process, a bridge-only preload with no ipcRenderer/MessagePort,
 * no Electron or Node authority inside the renderer, and no starter/parallel
 * UI architectures (React, shadcn, Zod, oRPC, TanStack) in the app at all.
 */
import { describe, expect, test } from "bun:test"
import { readFileSync, readdirSync } from "node:fs"
import path from "node:path"

const appRoot = path.resolve(import.meta.dir, "..")
const read = (relative: string): string => readFileSync(path.join(appRoot, relative), "utf8")

/**
 * The negative oracles scan CODE, not prose: doc comments legitimately name
 * the banned APIs while explaining why they are banned.
 */
const stripComments = (source: string): string =>
  source.replace(/\/\*[\s\S]*?\*\//g, "").replace(/^\s*\/\/.*$/gm, "")

describe("Electron boundary (issue #8574 mandatory first-scaffold hardening)", () => {
  const main = stripComments(read("src/main.ts"))

  test("renderer window is sandboxed with node integration off", () => {
    expect(main).toContain("contextIsolation: true")
    expect(main).toContain("nodeIntegration: false")
    expect(main).toContain("sandbox: true")
    expect(main).toContain("webviewTag: false")
    expect(main).toContain("webSecurity: true")
  })

  test("uses the built mobile icon for the window and macOS Dock", () => {
    expect(main).toContain('"assets", "openagents-icon.png"')
    expect(main).toContain("icon: desktopIconPath")
    expect(main).toContain("app.dock?.setIcon(desktopIconPath)")
  })

  test("deny-by-default permission, navigation, window-open, and webview handlers", () => {
    expect(main).toContain("setPermissionRequestHandler")
    expect(main).toContain("will-navigate")
    expect(main).toContain("will-attach-webview")
    expect(main).toContain('setWindowOpenHandler(() => ({ action: "deny" }))')
  })

  test("no template updater, publisher target, or devtools installer survives", () => {
    expect(main).not.toContain("updateElectronApp")
    expect(main).not.toContain("electron-devtools-installer")
    expect(main).not.toContain("REACT_DEVELOPER_TOOLS")
  })

  test("no legacy Khala Code identity is reused", () => {
    for (const file of ["src/main.ts", "package.json", "index.html"]) {
      const source = stripComments(read(file))
      expect(source).not.toContain("com.openagents.khala.code.desktop")
      expect(source).not.toContain("khala-code://")
    }
  })

  test("preload exposes fixed typed capabilities only — no raw IPC or MessagePort", () => {
    const preload = stripComments(read("src/preload.cts"))
    expect(preload).toContain("contextBridge.exposeInMainWorld")
    expect(preload).toContain("ipcRenderer.invoke(FleetStageChannel, request)")
    expect(preload).toContain("DesktopWorkspaceChooseChannel")
    expect(preload).toContain("decodeWorkspaceFileRequest")
    expect(preload).toContain("decodeWorkspaceSaveRequest")
    expect(preload).toContain("decodeWorkspaceGitDiffRequest")
    expect(preload).not.toContain("ipcRenderer.send")
    expect(preload).not.toContain("ipcRenderer.on")
    expect(preload).not.toContain("ipcRenderer.remove")
    expect(preload).not.toContain("MessagePort")
    expect(preload).not.toContain('require("node:')
  })

  test("main exposes fixed validated channels rather than arbitrary command authority", () => {
    expect(main).toContain("ipcMain.handle(FleetStageChannel")
    expect(main).toContain("decodeFleetStageRequest(value)")
    expect(main).toContain("decodeWorkspaceFileRequest(value)")
    expect(main).toContain("decodeWorkspaceSaveRequest(value)")
    expect(main).toContain("decodeWorkspaceGitDiffRequest(value)")
    expect(main).not.toContain("ipcMain.on(")
  })

  test("workspace filesystem authority starts only after an explicit directory choice", () => {
    expect(main).toContain("let workspaceRoot: string | null = null")
    expect(main).toContain('properties: ["openDirectory", "createDirectory"]')
    expect(main).not.toContain("OPENAGENTS_DESKTOP_WORKSPACE")
  })

  test("renderer CSP is restrictive (no remote script/connect surface)", () => {
    const html = read("index.html")
    expect(html).toContain("Content-Security-Policy")
    expect(html).toContain("default-src 'none'")
    expect(html).toContain("script-src 'self'")
    expect(html).toContain("connect-src 'none'")
  })

  test("recent-chat sidebar CSS is plain text and never restores card chrome", () => {
    const css = read("src/renderer/app.css")
    expect(css).not.toContain('[data-en-key^="sidebar-thread-"],')
    expect(css).toContain('[data-en-key^="sidebar-thread-"][data-en-tag="Button"]')
    expect(css).toContain('[data-en-key^="sidebar-thread-time-"][data-en-tag="Text"]')
    expect(css).toContain("background: transparent !important")
    expect(css).toContain("border-radius: 0 !important")
  })
})

describe("Effect Native renderer boundary (no parallel UI architecture)", () => {
  const rendererDir = path.join(appRoot, "src/renderer")
  const rendererSources = readdirSync(rendererDir)
    .filter((name) => name.endsWith(".ts") && !name.endsWith(".test.ts"))
    .map((name) => ({
      name,
      source: stripComments(readFileSync(path.join(rendererDir, name), "utf8")),
    }))

  test("renderer imports only the shared EN catalog and sibling modules", () => {
    const allowed = /^(@effect-native\/(core|core\/effect|render-dom|tokens)|(\.\.\/|\.\/)[a-z-]+\.ts)$/
    for (const { name, source } of rendererSources) {
      const specifiers = [...source.matchAll(/from\s+"([^"]+)"/g)].map((match) => match[1]!)
      for (const specifier of specifiers) {
        expect(specifier).toMatch(allowed)
      }
    }
  })

  test("renderer never touches Electron or Node builtins", () => {
    for (const { name, source } of rendererSources) {
      expect(source).not.toContain('from "electron"')
      expect(source).not.toContain('from "node:')
      expect(source).not.toContain("process.")
    }
  })

  test("no starter application semantics return (shadcn/React/Zod/oRPC/TanStack)", () => {
    const manifest = JSON.parse(read("package.json")) as {
      dependencies?: Record<string, string>
      devDependencies?: Record<string, string>
    }
    const dependencyNames = [
      ...Object.keys(manifest.dependencies ?? {}),
      ...Object.keys(manifest.devDependencies ?? {}),
    ]
    const banned = [/^react(-dom)?$/, /^zod$/, /@orpc\//, /@tanstack\//, /^shadcn$/, /radix/, /tailwind/]
    for (const name of dependencyNames) {
      for (const pattern of banned) {
        expect(name).not.toMatch(pattern)
      }
    }
  })

  test("no legacy desktop app import (greenfield law: extract, not inherit)", () => {
    for (const { name, source } of rendererSources) {
      expect(source).not.toContain("khala-code-desktop")
      expect(source).not.toContain("electrobun")
    }
  })
})

describe("OpenAI Apps SDK icon catalog", () => {
  test("Effect Native DOM resolves its closed icon contract through the shared catalog", () => {
    const domRenderer = read("../openagents.com/packages/effect-native-render-dom/src/index.ts")
    const catalog = read("../openagents.com/packages/effect-native-core/src/index.ts")
    expect(domRenderer).toContain('from "@openagentsinc/ui/icon"')
    expect(domRenderer).toContain('name === "Compose" ? "ChatCompose" : name')
    expect(domRenderer).toContain("openAiIconSvg(assetName)")
    expect(catalog).toContain('"ChatCompose"')
    expect(catalog).toContain('"Agent"')
  })
})

describe("Effect Native Liquid Glass lowering", () => {
  test("desktop backdrop and glass surfaces are authored in the catalog, not CSS-only", () => {
    const shell = read("src/renderer/shell.ts")
    const domRenderer = read("../openagents.com/packages/effect-native-render-dom/src/index.ts")
    expect(shell).toContain("BackgroundGradient(")
    expect(shell).toContain('surface: "glass"')
    expect(domRenderer).toContain("mobile SwiftUI")
    expect(domRenderer).toContain("blur(28px) saturate(1.35)")
  })
})
