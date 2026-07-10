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
import { randomUUID } from "node:crypto"
import { BrowserWindow, app, dialog, ipcMain, session } from "electron"

import { FleetStageChannel, decodeFleetStageRequest, unavailableFleetStageResult } from "./fleet-contract.ts"
import { submitFleetBrief } from "./fleet-control.ts"
import { completeChatTurn } from "./chat-service.ts"
import { DesktopChatTurnChannel, DesktopNewThreadChannel, DesktopOpenThreadChannel, DesktopThreadsChannel, decode, DesktopThreadRequestSchema, DesktopTurnRequestSchema, type DesktopMessage } from "./chat-contract.ts"
import { makeThreadStore } from "./thread-store.ts"
import { DesktopWorkspaceChooseChannel, DesktopWorkspaceFilesChannel, DesktopWorkspaceReadChannel, DesktopWorkspaceSummaryChannel, decodeWorkspaceFileRequest } from "./workspace-contract.ts"
import { inspectWorkspace, readWorkspaceFile } from "./workspace-service.ts"

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

/**
 * A deliberately single-purpose IPC boundary. The preload validates first and
 * main validates again; the renderer never gets filesystem, token, arbitrary
 * command, or loopback request authority.
 */
ipcMain.handle(FleetStageChannel, async (_event, value: unknown) => {
  const request = decodeFleetStageRequest(value)
  return request === null ? unavailableFleetStageResult() : submitFleetBrief(request)
})

const threads = () => makeThreadStore(path.join(app.getPath("userData"), "threads.json"))
let workspaceRoot = path.resolve(process.env.OPENAGENTS_DESKTOP_WORKSPACE ?? process.cwd())
const workspaceSnapshot = () => {
  try { return inspectWorkspace(workspaceRoot) } catch { return null }
}
ipcMain.handle(DesktopWorkspaceSummaryChannel, () => workspaceSnapshot())
ipcMain.handle(DesktopWorkspaceFilesChannel, () => workspaceSnapshot())
ipcMain.handle(DesktopWorkspaceChooseChannel, async () => {
  const result = await dialog.showOpenDialog({ properties: ["openDirectory", "createDirectory"] })
  if (result.canceled || result.filePaths[0] === undefined) return workspaceSnapshot()
  workspaceRoot = path.resolve(result.filePaths[0])
  return workspaceSnapshot()
})
ipcMain.handle(DesktopWorkspaceReadChannel, (_event, value: unknown) => {
  const request = decodeWorkspaceFileRequest(value)
  return request === null ? null : readWorkspaceFile(workspaceRoot, request.path)
})
ipcMain.handle(DesktopThreadsChannel, () => threads().list())
ipcMain.handle(DesktopNewThreadChannel, () => threads().newThread())
ipcMain.handle(DesktopOpenThreadChannel, (_event, value: unknown) => {
  const request = decode(DesktopThreadRequestSchema, value) as { id: string } | null
  return request === null ? null : threads().open(request.id)
})
ipcMain.handle(DesktopChatTurnChannel, async (_event, value: unknown) => {
  const request = decode(DesktopTurnRequestSchema, value) as { id: string; message: string } | null
  if (request === null || request.message.trim() === "" || request.message.length > 8_000) {
    return { ok: false, error: "That message could not be sent." }
  }
  const store = threads()
  const user: DesktopMessage = { key: randomUUID(), role: "user", text: request.message.trim(), timestamp: new Date().toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" }) }
  const saved = store.append(request.id, user)
  if (saved === null) return { ok: false, error: "That conversation no longer exists." }
  try {
    const text = await completeChatTurn(saved.notes)
    const thread = store.append(saved.id, { key: randomUUID(), role: "assistant", text, timestamp: new Date().toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" }) })
    return { ok: true, thread }
  } catch (error) {
    return { ok: false, error: error instanceof Error ? error.message : "The model request failed." }
  }
})

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
 * inside the real Electron renderer — types into the catalog-rendered
 * composer, submits, and asserts the message row appended AND the composer
 * cleared (the v29 clear-on-submit contract, effect-native#72). When
 * `OPENAGENTS_DESKTOP_SMOKE_SHOTS` names a directory, it captures pixel
 * receipts (shell / composer-typed / composer-cleared). Exits 0/1.
 */
const smokeShotsDir = process.env.OPENAGENTS_DESKTOP_SMOKE_SHOTS

const smokeWaitForShell = `(async () => {
  const wait = (ms) => new Promise((resolve) => setTimeout(resolve, ms))
  const deadline = Date.now() + 15000
  while (Date.now() < deadline && document.querySelector('[data-en-key="shell-input"] input') === null) {
    await wait(100)
  }
  return document.querySelector('[data-en-key="shell-input"] input') !== null
})()`

const smokeTypeIntoComposer = `(() => {
  const input = document.querySelector('[data-en-key="shell-input"] input')
  if (input === null) return { ok: false, reason: "composer input never mounted" }
  input.focus()
  input.value = "Pixel-proof: real chat rows on the shared catalog"
  input.dispatchEvent(new Event("input", { bubbles: true }))
  return { ok: true, typed: input.value }
})()`

const smokeOpenFleetDesk = `(async () => {
  const wait = (ms) => new Promise((resolve) => setTimeout(resolve, ms))
  const button = document.querySelector('[data-en-key="shell-fleet-toggle"]')
  if (button === null) return { ok: false, reason: "Fleet toggle never mounted" }
  button.click()
  const deadline = Date.now() + 20_000
  while (Date.now() < deadline && document.querySelector('[data-en-key="fleet-desk"]') === null) {
    await wait(50)
  }
  const objective = document.querySelector('[data-en-key="fleet-objective"] input')
  const dispatch = document.querySelector('[data-en-key="fleet-stage-request"]')
  const status = document.querySelector('[data-en-key="fleet-authority-status"]')
  return {
    ok: objective !== null && dispatch !== null && status !== null && status.textContent === "Draft",
    status: status === null ? null : status.textContent,
  }
})()`

const smokeSubmitComposer = `(async () => {
  const wait = (ms) => new Promise((resolve) => setTimeout(resolve, ms))
  const input = document.querySelector('[data-en-key="shell-input"] input')
  if (input === null) return { ok: false, reason: "composer input never mounted" }
  const messageCount = () =>
    document.querySelectorAll('[data-en-key="shell-transcript"] [data-en-message]').length
  const messagesBefore = messageCount()
  input.dispatchEvent(new KeyboardEvent("keydown", { key: "Enter", bubbles: true }))
  const deadline = Date.now() + 5000
  while (Date.now() < deadline && (messageCount() < messagesBefore + 2 || input.value !== "")) {
    await wait(50)
  }
  const userRow = document.querySelector(
    '[data-en-key="shell-transcript"] [data-en-message][data-en-role="user"]'
  )
  const responseRow = Array.from(
    document.querySelectorAll('[data-en-key="shell-transcript"] [data-en-message][data-en-role="assistant"], [data-en-key="shell-transcript"] [data-en-message][data-en-role="system"]')
  ).at(-1)
  const sender = userRow === null ? null : userRow.querySelector('[data-en-role="sender"]')
  const body = userRow === null ? null : userRow.querySelector('[data-en-role="body"]')
  const responseSender = responseRow === undefined ? null : responseRow.querySelector('[data-en-role="sender"]')
  return {
    ok:
      messageCount() === messagesBefore + 2 &&
      input.value === "" &&
      sender !== null && sender.textContent === "YOU" &&
      body !== null && body.textContent.includes("Pixel-proof") &&
      !body.textContent.includes("YOU") &&
      responseSender !== null && (responseSender.textContent === "ASSISTANT" || responseSender.textContent === "SYSTEM"),
    messagesBefore,
    messagesAfter: messageCount(),
    inputAfterSubmit: input.value,
    senderChip: sender === null ? null : sender.textContent,
    responseSenderChip: responseSender === null ? null : responseSender.textContent,
  }
})()`

const smokePingLoop = `(async () => {
  const wait = (ms) => new Promise((resolve) => setTimeout(resolve, ms))
  const button = document.querySelector('[data-en-key="shell-ping"]')
  if (button === null) return { ok: false, reason: "ping button never mounted" }
  const badgeText = () => {
    const badge = document.querySelector('[data-en-key="shell-ping-count"]')
    return badge === null ? null : badge.textContent
  }
  const before = badgeText()
  const messageCount = () =>
    document.querySelectorAll('[data-en-key="shell-transcript"] [data-en-message]').length
  const notesBefore = messageCount()
  button.click()
  const deadline = Date.now() + 5000
  while (Date.now() < deadline && badgeText() === before) {
    await wait(50)
  }
  const after = badgeText()
  return {
    ok: after !== before && after !== null && after.includes("1") && messageCount() === notesBefore + 1,
    before,
    after,
    notesBefore,
    notesAfter: messageCount(),
  }
})()`

const captureShot = async (window: BrowserWindow, name: string): Promise<void> => {
  if (smokeShotsDir === undefined || smokeShotsDir === "") return
  const image = await window.webContents.capturePage()
  const { mkdirSync, writeFileSync } = await import("node:fs")
  mkdirSync(smokeShotsDir, { recursive: true })
  writeFileSync(path.join(smokeShotsDir, `${name}.png`), image.toPNG())
  console.log(`[openagents-desktop smoke] shot ${name}.png`)
}

const runSmoke = (window: BrowserWindow): void => {
  const timeout = setTimeout(() => {
    console.error("[openagents-desktop smoke] TIMEOUT waiting for renderer")
    app.exit(1)
  }, 45_000)
  window.webContents.once("did-finish-load", () => {
    void (async () => {
      const step = async (name: string, script: string): Promise<void> => {
        const result: unknown = await window.webContents.executeJavaScript(script, true)
        const ok =
          result === true ||
          (typeof result === "object" && result !== null && (result as { ok?: unknown }).ok === true)
        if (!ok) {
          clearTimeout(timeout)
          console.error(`[openagents-desktop smoke] FAILED ${name}`, JSON.stringify(result))
          app.exit(1)
          return
        }
        console.log(`[openagents-desktop smoke] ${name} OK`, JSON.stringify(result))
      }
      try {
        await step("shell-mounted", smokeWaitForShell)
        await captureShot(window, "01-shell")
        await step("composer-typed", smokeTypeIntoComposer)
        await captureShot(window, "02-composer-typed")
        await step("composer-submit-clears", smokeSubmitComposer)
        await captureShot(window, "03-composer-cleared")
        clearTimeout(timeout)
        console.log("[openagents-desktop smoke] OK")
        app.exit(0)
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
