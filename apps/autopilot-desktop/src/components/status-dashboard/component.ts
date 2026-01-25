import { Effect } from "effect"
import { invoke } from "@tauri-apps/api/core"
import { listen } from "@tauri-apps/api/event"
import type { Component } from "../../effuse/index.js"
import { html } from "../../effuse/index.js"

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

type DoctorState = {
  ok: boolean
  appServerOk: boolean
  version: string | null
  codexBin: string | null
  detail: string
}

type BusyState = {
  doctor: boolean
  connect: boolean
  disconnect: boolean
}

type StatusState = {
  workspaceId: string
  workspacePath: string
  workspaceConnected: boolean
  workspaceMessage: string
  doctor: DoctorState
  lastEventText: string
  lastEventTime: string
  busy: BusyState
}

type StatusEvent =
  | { type: "RefreshDoctor" }
  | { type: "ConnectWorkspace" }
  | { type: "DisconnectWorkspace" }
  | { type: "UpdateWorkspacePath"; path: string }
  | { type: "RefreshWorkspaceStatus" }
  | { type: "AppServerEvent"; payload: AppServerEvent }

const workspaceIdKey = "autopilotWorkspaceId"
const workspacePathKey = "autopilotWorkspacePath"

const loadWorkspaceId = (): string => {
  const stored = window.localStorage.getItem(workspaceIdKey)
  if (stored) {
    return stored
  }
  const generated =
    typeof crypto !== "undefined" && "randomUUID" in crypto
      ? crypto.randomUUID()
      : `workspace-${Date.now()}`
  window.localStorage.setItem(workspaceIdKey, generated)
  return generated
}

const formatEvent = (payload: AppServerEvent) => {
  const method = payload.message?.method ?? "unknown"
  const params = payload.message?.params
  const time = new Date().toLocaleTimeString()
  const header = `[${time}] ${method}`
  if (params === undefined) {
    return { time, text: header }
  }
  return { time, text: `${header}\n${JSON.stringify(params, null, 2)}` }
}

const deriveStatus = (state: StatusState) => {
  if (!state.doctor.ok || !state.doctor.appServerOk) {
    return { level: "error" as const, label: "Codex not ready" }
  }
  if (state.workspaceConnected) {
    return { level: "ok" as const, label: "App-server connected" }
  }
  return { level: "warn" as const, label: "Ready to connect" }
}

const invokeCommand = <T>(command: string, payload?: Record<string, unknown>) =>
  Effect.tryPromise({
    try: () => invoke<T>(command, payload),
    catch: (error) => new Error(String(error)),
  })

export const StatusDashboardComponent: Component<StatusState, StatusEvent> = {
  id: "status-dashboard",

  initialState: () => ({
    workspaceId: loadWorkspaceId(),
    workspacePath: "",
    workspaceConnected: false,
    workspaceMessage: "",
    doctor: {
      ok: false,
      appServerOk: false,
      version: null,
      codexBin: null,
      detail: "",
    },
    lastEventText: "Waiting for app-server events...",
    lastEventTime: "Waiting",
    busy: {
      doctor: false,
      connect: false,
      disconnect: false,
    },
  }),

  render: (ctx) =>
    Effect.gen(function* () {
      const state = yield* ctx.state.get
      const status = deriveStatus(state)
      const cliStatus = state.doctor.version
        ? `ok (${state.doctor.version})`
        : "not found"
      const appServerStatus = state.doctor.appServerOk ? "ready" : "unavailable"
      const connectDisabled = state.busy.connect
      const disconnectDisabled = state.busy.disconnect || !state.workspaceConnected
      const refreshDisabled = state.busy.doctor

      return html`
        <div class="app">
          <header class="header">
            <div class="title">Autopilot</div>
            <div class="status-pill" data-state="${status.level}">${status.label}</div>
          </header>
          <main class="main">
            <section class="panel">
              <div class="panel-head">
                <h1>Codex App-Server</h1>
                <button data-action="refresh-doctor" ${refreshDisabled ? "disabled" : ""}>Refresh</button>
              </div>
              <div class="grid">
                <div class="stat">
                  <span class="label">CLI</span>
                  <span class="value">${cliStatus}</span>
                </div>
                <div class="stat">
                  <span class="label">App-Server</span>
                  <span class="value">${appServerStatus}</span>
                </div>
                <div class="stat">
                  <span class="label">Binary</span>
                  <span class="value mono">${state.doctor.codexBin ?? "default"}</span>
                </div>
              </div>
              <div class="hint">${state.doctor.detail}</div>
            </section>
            <section class="panel">
              <div class="panel-head">
                <h2>Workspace</h2>
                <div class="actions">
                  <button data-action="connect" ${connectDisabled ? "disabled" : ""}>Connect</button>
                  <button data-action="disconnect" class="secondary" ${disconnectDisabled ? "disabled" : ""}>Disconnect</button>
                </div>
              </div>
              <label class="field">
                <span>Working directory</span>
                <input id="workspace-path" type="text" value="${state.workspacePath}" placeholder="/path/to/workspace" />
              </label>
              <div class="grid">
                <div class="stat">
                  <span class="label">Connection</span>
                  <span class="value">${state.workspaceConnected ? "Connected" : "Disconnected"}</span>
                </div>
                <div class="stat">
                  <span class="label">Workspace ID</span>
                  <span class="value mono">${state.workspaceId}</span>
                </div>
              </div>
              <div class="hint">${state.workspaceMessage}</div>
            </section>
            <section class="panel">
              <div class="panel-head">
                <h2>Last App-Server Event</h2>
                <span class="subtle">${state.lastEventTime}</span>
              </div>
              <pre class="event-log">${state.lastEventText}</pre>
            </section>
          </main>
        </div>
      `
    }),

  handleEvent: (event, ctx) =>
    Effect.gen(function* () {
      if (event.type === "UpdateWorkspacePath") {
        yield* ctx.state.update((current) => ({
          ...current,
          workspacePath: event.path,
        }))
        return
      }

      if (event.type === "RefreshDoctor") {
        yield* ctx.state.update((current) => ({
          ...current,
          busy: { ...current.busy, doctor: true },
          doctor: { ...current.doctor, detail: "Checking codex CLI..." },
        }))

        yield* invokeCommand<CodexDoctorResponse>("codex_doctor", {
          codexBin: null,
        }).pipe(
          Effect.tap((response) =>
            ctx.state.update((current) => ({
              ...current,
              doctor: {
                ok: response.ok,
                appServerOk: response.appServerOk,
                version: response.version,
                codexBin: response.codexBin,
                detail: response.details ?? "",
              },
              busy: { ...current.busy, doctor: false },
            }))
          ),
          Effect.catchAll((error) =>
            ctx.state.update((current) => ({
              ...current,
              doctor: {
                ok: false,
                appServerOk: false,
                version: null,
                codexBin: null,
                detail: String(error),
              },
              busy: { ...current.busy, doctor: false },
            }))
          ),
          Effect.asVoid
        )
        return
      }

      if (event.type === "RefreshWorkspaceStatus") {
        const current = yield* ctx.state.get
        yield* invokeCommand<WorkspaceConnectionStatusResponse>(
          "get_workspace_connection_status",
          { workspaceId: current.workspaceId }
        ).pipe(
          Effect.tap((response) =>
            ctx.state.update((state) => ({
              ...state,
              workspaceConnected: response.connected,
            }))
          ),
          Effect.catchAll((error) =>
            ctx.state.update((state) => ({
              ...state,
              workspaceConnected: false,
              workspaceMessage: `Status check failed: ${String(error)}`,
            }))
          ),
          Effect.asVoid
        )
        return
      }

      if (event.type === "ConnectWorkspace") {
        const current = yield* ctx.state.get
        const workspacePath = current.workspacePath.trim()
        if (!workspacePath) {
          yield* ctx.state.update((state) => ({
            ...state,
            workspaceMessage: "Enter a working directory first.",
          }))
          return
        }

        yield* ctx.state.update((state) => ({
          ...state,
          busy: { ...state.busy, connect: true },
          workspaceMessage: "Connecting to app-server...",
        }))

        yield* invokeCommand<WorkspaceConnectionResponse>("connect_workspace", {
          workspaceId: current.workspaceId,
          workspacePath,
          codexBin: null,
        }).pipe(
          Effect.tap((response) =>
            ctx.state.update((state) => ({
              ...state,
              workspaceConnected: response.success,
              workspaceMessage: response.message,
              busy: { ...state.busy, connect: false },
            }))
          ),
          Effect.catchAll((error) =>
            ctx.state.update((state) => ({
              ...state,
              workspaceConnected: false,
              workspaceMessage: `Connection failed: ${String(error)}`,
              busy: { ...state.busy, connect: false },
            }))
          ),
          Effect.asVoid
        )
        return
      }

      if (event.type === "DisconnectWorkspace") {
        const current = yield* ctx.state.get
        yield* ctx.state.update((state) => ({
          ...state,
          busy: { ...state.busy, disconnect: true },
          workspaceMessage: "Disconnecting...",
        }))

        yield* invokeCommand<WorkspaceConnectionResponse>("disconnect_workspace", {
          workspaceId: current.workspaceId,
        }).pipe(
          Effect.tap((response) =>
            ctx.state.update((state) => ({
              ...state,
              workspaceConnected: false,
              workspaceMessage: response.message,
              busy: { ...state.busy, disconnect: false },
            }))
          ),
          Effect.catchAll((error) =>
            ctx.state.update((state) => ({
              ...state,
              workspaceMessage: `Disconnect failed: ${String(error)}`,
              busy: { ...state.busy, disconnect: false },
            }))
          ),
          Effect.asVoid
        )
        return
      }

      if (event.type === "AppServerEvent") {
        const current = yield* ctx.state.get
        if (event.payload.workspace_id !== current.workspaceId) {
          return
        }
        const formatted = formatEvent(event.payload)
        yield* ctx.state.update((state) => ({
          ...state,
          lastEventText: formatted.text,
          lastEventTime: formatted.time,
          workspaceConnected:
            event.payload.message?.method === "codex/connected"
              ? true
              : state.workspaceConnected,
        }))
      }
    }),

  setupEvents: (ctx) =>
    Effect.gen(function* () {
      const emit = (event: StatusEvent) => {
        Effect.runFork(ctx.emit(event))
      }

      yield* ctx.dom.delegate(
        ctx.container,
        "[data-action=\"refresh-doctor\"]",
        "click",
        () => emit({ type: "RefreshDoctor" })
      )

      yield* ctx.dom.delegate(
        ctx.container,
        "[data-action=\"connect\"]",
        "click",
        () => emit({ type: "ConnectWorkspace" })
      )

      yield* ctx.dom.delegate(
        ctx.container,
        "[data-action=\"disconnect\"]",
        "click",
        () => emit({ type: "DisconnectWorkspace" })
      )

      yield* ctx.dom.delegate(
        ctx.container,
        "#workspace-path",
        "input",
        (event, target) => {
          const value = (target as HTMLInputElement).value
          window.localStorage.setItem(workspacePathKey, value)
          emit({ type: "UpdateWorkspacePath", path: value })
        }
      )

      const unlisten = yield* Effect.tryPromise({
        try: () =>
          listen<AppServerEvent>("app-server-event", (event) => {
            emit({ type: "AppServerEvent", payload: event.payload })
          }),
        catch: (error) => new Error(String(error)),
      })

      yield* Effect.addFinalizer(() => Effect.sync(() => unlisten()))

      const initialize = Effect.gen(function* () {
        const storedPath = window.localStorage.getItem(workspacePathKey)
        if (storedPath && storedPath.trim()) {
          yield* ctx.state.update((state) => ({
            ...state,
            workspacePath: storedPath,
          }))
        } else {
          yield* invokeCommand<string>("get_current_directory").pipe(
            Effect.tap((cwd) =>
              ctx.state.update((state) => ({
                ...state,
                workspacePath: cwd,
              }))
            ),
            Effect.tap((cwd) =>
              Effect.sync(() => window.localStorage.setItem(workspacePathKey, cwd))
            ),
            Effect.catchAll((error) =>
              ctx.state.update((state) => ({
                ...state,
                workspaceMessage: `Failed to read current directory: ${String(error)}`,
              }))
            ),
            Effect.asVoid
          )
        }

        yield* ctx.emit({ type: "RefreshDoctor" })
        yield* ctx.emit({ type: "RefreshWorkspaceStatus" })
      })

      yield* Effect.forkScoped(initialize)
    }),
}
