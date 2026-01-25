import { invoke } from "@tauri-apps/api/core"
import { listen } from "@tauri-apps/api/event"

type CodexDoctorResponse = {
  ok: boolean
  codexBin: string | null
  version: string | null
  appServerOk: boolean
  details: string | null
  path: string | null
}

type WorkspaceConnectionResponse = {
  success: boolean
  message: string
  workspaceId: string
}

type WorkspaceConnectionStatusResponse = {
  workspaceId: string
  connected: boolean
}

type AppServerEvent = {
  workspace_id: string
  message: {
    method?: string
    params?: unknown
    [key: string]: unknown
  }
}

const root = document.getElementById("root")
if (!root) {
  throw new Error("Root element not found")
}

root.innerHTML = `
  <div class="app">
    <header class="header">
      <div class="title">Autopilot</div>
      <div class="status-pill" id="overall-status" data-state="warn">Checking</div>
    </header>
    <main class="main">
      <section class="panel">
        <div class="panel-head">
          <h1>Codex App-Server</h1>
          <button id="refresh-doctor">Refresh</button>
        </div>
        <div class="grid">
          <div class="stat">
            <span class="label">CLI</span>
            <span class="value" id="codex-cli">-</span>
          </div>
          <div class="stat">
            <span class="label">App-Server</span>
            <span class="value" id="codex-appserver">-</span>
          </div>
          <div class="stat">
            <span class="label">Binary</span>
            <span class="value mono" id="codex-bin">-</span>
          </div>
        </div>
        <div class="hint" id="codex-detail"></div>
      </section>
      <section class="panel">
        <div class="panel-head">
          <h2>Workspace</h2>
          <div class="actions">
            <button id="connect-btn">Connect</button>
            <button id="disconnect-btn" class="secondary">Disconnect</button>
          </div>
        </div>
        <label class="field">
          <span>Working directory</span>
          <input id="workspace-path" type="text" placeholder="/path/to/workspace" />
        </label>
        <div class="grid">
          <div class="stat">
            <span class="label">Connection</span>
            <span class="value" id="workspace-status">Disconnected</span>
          </div>
          <div class="stat">
            <span class="label">Workspace ID</span>
            <span class="value mono" id="workspace-id">-</span>
          </div>
        </div>
        <div class="hint" id="workspace-message"></div>
      </section>
      <section class="panel">
        <div class="panel-head">
          <h2>Last App-Server Event</h2>
          <span class="subtle" id="event-time">Waiting</span>
        </div>
        <pre id="event-log" class="event-log">Waiting for app-server events...</pre>
      </section>
    </main>
  </div>
`

const elements = {
  overallStatus: document.getElementById("overall-status") as HTMLDivElement,
  codexCli: document.getElementById("codex-cli") as HTMLSpanElement,
  codexAppServer: document.getElementById("codex-appserver") as HTMLSpanElement,
  codexBin: document.getElementById("codex-bin") as HTMLSpanElement,
  codexDetail: document.getElementById("codex-detail") as HTMLDivElement,
  refreshDoctor: document.getElementById("refresh-doctor") as HTMLButtonElement,
  workspacePath: document.getElementById("workspace-path") as HTMLInputElement,
  workspaceStatus: document.getElementById("workspace-status") as HTMLSpanElement,
  workspaceId: document.getElementById("workspace-id") as HTMLSpanElement,
  workspaceMessage: document.getElementById("workspace-message") as HTMLDivElement,
  connectBtn: document.getElementById("connect-btn") as HTMLButtonElement,
  disconnectBtn: document.getElementById("disconnect-btn") as HTMLButtonElement,
  eventLog: document.getElementById("event-log") as HTMLPreElement,
  eventTime: document.getElementById("event-time") as HTMLSpanElement,
}

const state = {
  workspaceId: loadWorkspaceId(),
  connected: false,
  doctorOk: false,
  appServerOk: false,
}

elements.workspaceId.textContent = state.workspaceId

function loadWorkspaceId(): string {
  const stored = window.localStorage.getItem("autopilotWorkspaceId")
  if (stored) {
    return stored
  }
  const generated =
    typeof crypto !== "undefined" && "randomUUID" in crypto
      ? crypto.randomUUID()
      : `workspace-${Date.now()}`
  window.localStorage.setItem("autopilotWorkspaceId", generated)
  return generated
}

function setStatusPill(level: "ok" | "warn" | "error", label: string): void {
  elements.overallStatus.dataset.state = level
  elements.overallStatus.textContent = label
}

function updateOverallStatus(): void {
  if (!state.doctorOk || !state.appServerOk) {
    setStatusPill("error", "Codex not ready")
    return
  }
  if (state.connected) {
    setStatusPill("ok", "App-server connected")
    return
  }
  setStatusPill("warn", "Ready to connect")
}

function setWorkspaceStatus(connected: boolean): void {
  elements.workspaceStatus.textContent = connected ? "Connected" : "Disconnected"
}

function setWorkspaceMessage(message: string): void {
  elements.workspaceMessage.textContent = message
}

function setDoctorDetail(message: string): void {
  elements.codexDetail.textContent = message
}

async function refreshDoctor(): Promise<void> {
  setDoctorDetail("Checking codex CLI...")
  try {
    const response = await invoke<CodexDoctorResponse>("codex_doctor", {
      codex_bin: null,
    })
    state.doctorOk = response.ok
    state.appServerOk = response.appServerOk
    elements.codexCli.textContent = response.version
      ? `ok (${response.version})`
      : "not found"
    elements.codexAppServer.textContent = response.appServerOk
      ? "ready"
      : "unavailable"
    elements.codexBin.textContent = response.codexBin ?? "default"
    setDoctorDetail(response.details ?? "")
  } catch (error) {
    state.doctorOk = false
    state.appServerOk = false
    elements.codexCli.textContent = "error"
    elements.codexAppServer.textContent = "error"
    elements.codexBin.textContent = "-"
    setDoctorDetail(String(error))
  }
  updateOverallStatus()
}

async function refreshWorkspaceStatus(): Promise<void> {
  try {
    const response = await invoke<WorkspaceConnectionStatusResponse>(
      "get_workspace_connection_status",
      { workspace_id: state.workspaceId }
    )
    state.connected = response.connected
    setWorkspaceStatus(response.connected)
  } catch (error) {
    state.connected = false
    setWorkspaceStatus(false)
    setWorkspaceMessage(`Status check failed: ${String(error)}`)
  }
  updateOverallStatus()
}

async function connectWorkspace(): Promise<void> {
  const workspacePath = elements.workspacePath.value.trim()
  if (!workspacePath) {
    setWorkspaceMessage("Enter a working directory first.")
    return
  }
  elements.connectBtn.disabled = true
  setWorkspaceMessage("Connecting to app-server...")
  try {
    const response = await invoke<WorkspaceConnectionResponse>(
      "connect_workspace",
      {
        workspace_id: state.workspaceId,
        workspace_path: workspacePath,
        codex_bin: null,
      }
    )
    state.connected = response.success
    setWorkspaceStatus(state.connected)
    setWorkspaceMessage(response.message)
  } catch (error) {
    state.connected = false
    setWorkspaceStatus(false)
    setWorkspaceMessage(`Connection failed: ${String(error)}`)
  } finally {
    elements.connectBtn.disabled = false
    updateOverallStatus()
  }
}

async function disconnectWorkspace(): Promise<void> {
  elements.disconnectBtn.disabled = true
  setWorkspaceMessage("Disconnecting...")
  try {
    const response = await invoke<WorkspaceConnectionResponse>(
      "disconnect_workspace",
      { workspace_id: state.workspaceId }
    )
    state.connected = false
    setWorkspaceStatus(false)
    setWorkspaceMessage(response.message)
  } catch (error) {
    setWorkspaceMessage(`Disconnect failed: ${String(error)}`)
  } finally {
    elements.disconnectBtn.disabled = false
    updateOverallStatus()
  }
}

function formatEvent(payload: AppServerEvent): string {
  const method = payload.message?.method ?? "unknown"
  const params = payload.message?.params
  const time = new Date().toLocaleTimeString()
  const header = `[${time}] ${method}`
  if (params === undefined) {
    return header
  }
  return `${header}\n${JSON.stringify(params, null, 2)}`
}

function setEventLog(payload: AppServerEvent): void {
  elements.eventLog.textContent = formatEvent(payload)
  elements.eventTime.textContent = new Date().toLocaleTimeString()
}

function handleAppServerEvent(payload: AppServerEvent): void {
  if (payload.workspace_id !== state.workspaceId) {
    return
  }
  setEventLog(payload)
  if (payload.message?.method === "codex/connected") {
    state.connected = true
    setWorkspaceStatus(true)
    updateOverallStatus()
  }
}

function registerListeners(): void {
  elements.refreshDoctor.addEventListener("click", () => {
    void refreshDoctor()
  })
  elements.connectBtn.addEventListener("click", () => {
    void connectWorkspace()
  })
  elements.disconnectBtn.addEventListener("click", () => {
    void disconnectWorkspace()
  })
  elements.workspacePath.addEventListener("input", () => {
    window.localStorage.setItem(
      "autopilotWorkspacePath",
      elements.workspacePath.value
    )
  })
}

async function loadDefaults(): Promise<void> {
  const storedPath = window.localStorage.getItem("autopilotWorkspacePath")
  if (storedPath) {
    elements.workspacePath.value = storedPath
  } else {
    try {
      const current = await invoke<string>("get_current_directory")
      elements.workspacePath.value = current
      window.localStorage.setItem("autopilotWorkspacePath", current)
    } catch (error) {
      setWorkspaceMessage(`Failed to read current directory: ${String(error)}`)
    }
  }
}

async function boot(): Promise<void> {
  registerListeners()
  await loadDefaults()
  await refreshDoctor()
  await refreshWorkspaceStatus()
  await listen<AppServerEvent>("app-server-event", (event) => {
    handleAppServerEvent(event.payload)
  })
}

void boot()
