/**
 * OpenAgents Desktop — Electron main process (#8574).
 *
 * Scaffolded from the pinned MIT-licensed LuanRoger/electron-shadcn template
 * (see UPSTREAM.md) and hardened per the issue's mandatory first-commit
 * boundary: contextIsolation on, nodeIntegration OFF, sandbox ON, no webview,
 * deny-by-default permissions/navigation/window-open, and a minimal
 * contextBridge preload (no ipcRenderer, no MessagePort/oRPC bridge, no
 * updater, no devtools installer).
 *
 * Plain TypeScript, bundled by `scripts/build.ts` (Bun) into `dist/`.
 */
import path from "node:path"
import { BrowserWindow, app, session } from "electron"

const here = import.meta.dirname
const smokeMode = process.env.OPENAGENTS_DESKTOP_SMOKE === "1"

// Interim development identity ONLY. The frozen macOS bundle ID / Windows
// AppUserModelId / deep-link scheme / userData path / update channel are an
// owner decision (issue #8574 scope 1, NEEDS_OWNER) before the first packaged
// build. Never reuse com.openagents.khala.code.desktop or khala-code://.
app.setName("OpenAgents Desktop")
app.setPath("userData", path.join(app.getPath("appData"), "OpenAgentsDesktopDev"))

const hardenSession = (): void => {
  // Deny-by-default: this shell requests no runtime permissions.
  session.defaultSession.setPermissionRequestHandler((_webContents, _permission, callback) => {
    callback(false)
  })
}

// Deny-by-default for every WebContents: no navigation away from the bundled
// renderer, no window.open, no <webview> attachment.
app.on("web-contents-created", (_event, contents) => {
  contents.on("will-navigate", (event) => {
    event.preventDefault()
  })
  contents.on("will-attach-webview", (event) => {
    event.preventDefault()
  })
  contents.setWindowOpenHandler(() => ({ action: "deny" }))
})

const createWindow = (): BrowserWindow => {
  const window = new BrowserWindow({
    width: 1200,
    height: 800,
    minWidth: 720,
    minHeight: 480,
    backgroundColor: "#03060b",
    show: false,
    title: "OpenAgents Desktop",
    webPreferences: {
      contextIsolation: true,
      nodeIntegration: false,
      nodeIntegrationInSubFrames: false,
      sandbox: true,
      webviewTag: false,
      webSecurity: true,
      devTools: !app.isPackaged,
      spellcheck: false,
      backgroundThrottling: false,
      preload: path.join(here, "preload.cjs"),
    },
  })
  window.once("ready-to-show", () => {
    window.show()
  })
  void window.loadFile(path.join(here, "renderer/index.html"))
  return window
}

/**
 * Smoke mode (`bun run smoke`): proves the Effect Native intent loop runs
 * inside the real Electron renderer — finds the catalog-rendered "Ping loop"
 * button, clicks it, and asserts the loop-proof badge and transcript
 * re-rendered. Exits 0/1 for CI/owner verification.
 */
const smokeScript = `(async () => {
  const wait = (ms) => new Promise((resolve) => setTimeout(resolve, ms))
  const find = (selector) => document.querySelector(selector)
  const deadline = Date.now() + 15000
  while (Date.now() < deadline && find('[data-en-key="shell-ping"]') === null) {
    await wait(100)
  }
  const button = find('[data-en-key="shell-ping"]')
  if (button === null) return { ok: false, reason: "ping button never mounted" }
  const badgeText = () => {
    const badge = find('[data-en-key="shell-ping-count"]')
    return badge === null ? null : badge.textContent
  }
  const before = badgeText()
  const notesBefore = document.querySelectorAll('[data-en-key="shell-transcript"] [data-en-tag="Card"]').length
  button.click()
  const proofDeadline = Date.now() + 5000
  while (Date.now() < proofDeadline && badgeText() === before) {
    await wait(50)
  }
  const after = badgeText()
  const notesAfter = document.querySelectorAll('[data-en-key="shell-transcript"] [data-en-tag="Card"]').length
  return {
    ok: after !== before && after !== null && after.includes("1") && notesAfter === notesBefore + 1,
    before,
    after,
    notesBefore,
    notesAfter,
  }
})()`

const runSmoke = (window: BrowserWindow): void => {
  const timeout = setTimeout(() => {
    console.error("[openagents-desktop smoke] TIMEOUT waiting for renderer")
    app.exit(1)
  }, 30_000)
  window.webContents.once("did-finish-load", () => {
    void (async () => {
      try {
        const result: unknown = await window.webContents.executeJavaScript(smokeScript, true)
        clearTimeout(timeout)
        const ok =
          typeof result === "object" &&
          result !== null &&
          (result as { ok?: unknown }).ok === true
        if (ok) {
          console.log("[openagents-desktop smoke] OK", JSON.stringify(result))
          app.exit(0)
        } else {
          console.error("[openagents-desktop smoke] FAILED", JSON.stringify(result))
          app.exit(1)
        }
      } catch (error) {
        clearTimeout(timeout)
        console.error("[openagents-desktop smoke] ERROR", error)
        app.exit(1)
      }
    })()
  })
}

void app.whenReady().then(() => {
  hardenSession()
  const window = createWindow()
  if (smokeMode) runSmoke(window)
})

app.on("window-all-closed", () => {
  if (process.platform !== "darwin" || smokeMode) {
    app.quit()
  }
})

app.on("activate", () => {
  if (BrowserWindow.getAllWindows().length === 0) {
    createWindow()
  }
})
