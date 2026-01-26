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
  send: boolean
}

type StatusState = {
  workspaceId: string
  workspacePath: string
  workspaceConnected: boolean
  workspaceMessage: string
  doctor: DoctorState
  lastEventText: string
  lastEventTime: string
  lastUpdated: string
  commandInput: string
  messageInput: string
  threadId: string | null
  busy: BusyState
}

type StatusEvent =
  | { type: "RefreshDoctor" }
  | { type: "ConnectWorkspace" }
  | { type: "DisconnectWorkspace" }
  | { type: "UpdateWorkspacePath"; path: string }
  | { type: "UpdateCommandInput"; value: string }
  | { type: "SubmitCommand"; value: string }
  | { type: "UpdateMessageInput"; value: string }
  | { type: "SubmitMessage"; value: string }
  | { type: "RefreshWorkspaceStatus" }
  | { type: "AppServerEvent"; payload: AppServerEvent }

const workspaceIdKey = "autopilotWorkspaceId"
const workspacePathKey = "autopilotWorkspacePath"

const nowTime = () => new Date().toLocaleTimeString()

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
  const time = nowTime()
  const header = `[${time}] ${method}`
  if (params === undefined) {
    return { time, text: header }
  }
  return { time, text: `${header}\n${JSON.stringify(params, null, 2)}` }
}

const extractThreadId = (value: unknown): string | null => {
  if (!value || typeof value !== "object") {
    return null
  }
  const readContainer = (container: unknown): string | null => {
    if (!container || typeof container !== "object") {
      return null
    }
    const record = container as Record<string, unknown>
    const direct = record.threadId ?? record.thread_id
    if (typeof direct === "string") {
      return direct
    }
    const thread = record.thread
    if (thread && typeof thread === "object") {
      const id = (thread as Record<string, unknown>).id
      if (typeof id === "string") {
        return id
      }
    }
    return null
  }
  const record = value as Record<string, unknown>
  return (
    readContainer(record) ||
    readContainer(record.result) ||
    readContainer(record.params)
  )
}

const deriveStatus = (state: StatusState) => {
  if (!state.doctor.ok || !state.doctor.appServerOk) {
    return { level: "error" as const, label: "NOT READY" }
  }
  if (state.workspaceConnected) {
    return { level: "ok" as const, label: "CONNECTED" }
  }
  return { level: "warn" as const, label: "READY" }
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
    lastEventTime: "--",
    lastUpdated: nowTime(),
    commandInput: "",
    messageInput: "",
    threadId: null,
    busy: {
      doctor: false,
      connect: false,
      disconnect: false,
      send: false,
    },
  }),

  render: (ctx) =>
    Effect.gen(function* () {
      const state = yield* ctx.state.get
      const status = deriveStatus(state)
      const cliStatus = state.doctor.version
        ? `OK ${state.doctor.version}`
        : "MISSING"
      const appServerStatus = state.doctor.appServerOk ? "READY" : "DOWN"
      const connectionLabel = state.workspaceConnected ? "CONNECTED" : "DISCONNECTED"
      const connectDisabled = state.busy.connect
      const disconnectDisabled = state.busy.disconnect || !state.workspaceConnected
      const refreshDisabled = state.busy.doctor
      const sendDisabled = state.busy.send
      const threadLabel = state.threadId ?? "--"

      return html`
        <div class="terminal">
          <div class="status-strip">
            <div class="status-item">
              <span class="status-label">Status</span>
              <span class="status-value ${status.level}">${status.label}</span>
            </div>
            <div class="status-item">
              <span class="status-label">Workspace</span>
              <span class="status-value mono">${state.workspaceId}</span>
            </div>
            <div class="status-item">
              <span class="status-label">Updated</span>
              <span class="status-value">${state.lastUpdated}</span>
            </div>
          </div>

          <div class="command-bar">
            <span class="command-label">Command</span>
            <input
              id="command-input"
              class="command-input"
              type="text"
              placeholder="connect | disconnect | doctor | cd /path"
              value="${state.commandInput}"
            />
            <div class="command-hints">F2 CONNECT | F3 DISCONNECT | F5 DOCTOR | F12 STORYBOOK</div>
          </div>

          <div class="grid">
            <section class="panel">
              <div class="panel-title">System</div>
              <div class="panel-body">
                <div class="table">
                  <div class="label">CLI</div>
                  <div class="value ${state.doctor.ok ? "ok" : "error"}">${cliStatus}</div>
                  <div class="label">App-Server</div>
                  <div class="value ${state.doctor.appServerOk ? "ok" : "error"}">${appServerStatus}</div>
                  <div class="label">Binary</div>
                  <div class="value mono">${state.doctor.codexBin ?? "default"}</div>
                </div>
                <div class="note">${state.doctor.detail || "No diagnostics."}</div>
                <div class="actions">
                  <button
                    class="btn"
                    data-action="refresh-doctor"
                    ${refreshDisabled ? "disabled" : ""}
                  >
                    DOCTOR
                  </button>
                </div>
              </div>
            </section>

            <section class="panel">
              <div class="panel-title">Workspace</div>
              <div class="panel-body">
                <label class="field">
                  <span class="label">Working Dir</span>
                  <input
                    id="workspace-path"
                    class="input"
                    type="text"
                    placeholder="/path/to/workspace"
                    value="${state.workspacePath}"
                  />
                </label>
                <div class="actions">
                  <button
                    class="btn primary"
                    data-action="connect"
                    ${connectDisabled ? "disabled" : ""}
                  >
                    CONNECT
                  </button>
                  <button
                    class="btn secondary"
                    data-action="disconnect"
                    ${disconnectDisabled ? "disabled" : ""}
                  >
                    DISCONNECT
                  </button>
                </div>
                <div class="table">
                  <div class="label">Connection</div>
                  <div class="value ${state.workspaceConnected ? "ok" : "error"}">${connectionLabel}</div>
                  <div class="label">Last Event</div>
                  <div class="value">${state.lastEventTime}</div>
                  <div class="label">Thread</div>
                  <div class="value mono">${threadLabel}</div>
                </div>
                <div class="note">${state.workspaceMessage || ""}</div>
              </div>
            </section>

            <section class="panel">
              <div class="panel-title">App-Server Feed</div>
              <div class="panel-body">
                <pre class="event-log">${state.lastEventText}</pre>
              </div>
            </section>

            <section class="panel">
              <div class="panel-title">Summary</div>
              <div class="panel-body">
                <div class="table">
                  <div class="label">Overall</div>
                  <div class="value ${status.level}">${status.label}</div>
                  <div class="label">Workspace</div>
                  <div class="value ${state.workspaceConnected ? "ok" : "error"}">${connectionLabel}</div>
                  <div class="label">Event Time</div>
                  <div class="value">${state.lastEventTime}</div>
                  <div class="label">Thread</div>
                  <div class="value mono">${threadLabel}</div>
                  <div class="label">Update</div>
                  <div class="value">${state.lastUpdated}</div>
                </div>
                <div class="note">${state.workspaceMessage || state.doctor.detail || ""}</div>
              </div>
            </section>
          </div>

          <div class="compose-bar">
            <div class="compose-inner">
              <span class="compose-label">Input</span>
              <input
                id="message-input"
                class="compose-input"
                type="text"
                placeholder="Type a message or /command"
                value="${state.messageInput}"
              />
              <button
                class="compose-submit"
                data-action="send-message"
                ${sendDisabled ? "disabled" : ""}
              >
                SEND
              </button>
            </div>
            <div class="compose-hints">Enter to send | /connect /disconnect /doctor /cd /new /help</div>
          </div>

          <div class="status-strip bottom">
            <div class="status-item">
              <span class="status-label">App-Server</span>
              <span class="status-value ${state.doctor.appServerOk ? "ok" : "error"}">${appServerStatus}</span>
            </div>
            <div class="status-item">
              <span class="status-label">CLI</span>
              <span class="status-value ${state.doctor.ok ? "ok" : "error"}">${cliStatus}</span>
            </div>
            <div class="status-item">
              <span class="status-label">Last Event</span>
              <span class="status-value">${state.lastEventTime}</span>
            </div>
          </div>
        </div>
      `
    }),

  handleEvent: (event, ctx) =>
    Effect.gen(function* () {
      const runCommand = (commandText: string) =>
        Effect.gen(function* () {
          const command = commandText.trim()
          if (!command) {
            return
          }
          const [rawHead, ...rest] = command.split(/\s+/)
          const head = rawHead.toLowerCase()
          const arg = rest.join(" ").trim()

          if (head === "connect") {
            yield* ctx.emit({ type: "ConnectWorkspace" })
            return
          }
          if (head === "disconnect") {
            yield* ctx.emit({ type: "DisconnectWorkspace" })
            return
          }
          if (head === "doctor" || head === "refresh") {
            yield* ctx.emit({ type: "RefreshDoctor" })
            yield* ctx.emit({ type: "RefreshWorkspaceStatus" })
            return
          }
          if (head === "cd" || head === "cwd") {
            if (!arg) {
              yield* ctx.state.update((state) => ({
                ...state,
                workspaceMessage: "Command requires a path.",
                lastUpdated: nowTime(),
              }))
              return
            }
            window.localStorage.setItem(workspacePathKey, arg)
            yield* ctx.state.update((state) => ({
              ...state,
              workspacePath: arg,
              workspaceMessage: `Working dir set to ${arg}`,
              lastUpdated: nowTime(),
            }))
            return
          }
          if (head === "new" || head === "thread") {
            yield* ctx.state.update((state) => ({
              ...state,
              threadId: null,
              workspaceMessage: "Next send will start a new thread.",
              lastUpdated: nowTime(),
            }))
            return
          }
          if (head === "help" || head === "?") {
            yield* ctx.state.update((state) => ({
              ...state,
              workspaceMessage:
                "Commands: /connect /disconnect /doctor /cd <path> /new /help",
              lastUpdated: nowTime(),
            }))
            return
          }

          yield* ctx.state.update((state) => ({
            ...state,
            workspaceMessage: `Unknown command: ${command}`,
            lastUpdated: nowTime(),
          }))
        })

      if (event.type === "UpdateWorkspacePath") {
        yield* ctx.state.update((current) => ({
          ...current,
          workspacePath: event.path,
        }))
        return
      }

      if (event.type === "UpdateCommandInput") {
        yield* ctx.state.update((current) => ({
          ...current,
          commandInput: event.value,
        }))
        return
      }

      if (event.type === "UpdateMessageInput") {
        yield* ctx.state.update((current) => ({
          ...current,
          messageInput: event.value,
        }))
        return
      }

      if (event.type === "SubmitCommand") {
        const command = event.value.trim()
        if (!command) {
          yield* ctx.state.update((state) => ({
            ...state,
            commandInput: "",
          }))
          return
        }

        yield* runCommand(command)

        yield* ctx.state.update((state) => ({
          ...state,
          commandInput: "",
        }))
        return
      }

      if (event.type === "SubmitMessage") {
        const rawInput = event.value.trim()
        if (!rawInput) {
          yield* ctx.state.update((state) => ({
            ...state,
            messageInput: "",
          }))
          return
        }

        if (rawInput.startsWith("/")) {
          yield* runCommand(rawInput.slice(1))
          yield* ctx.state.update((state) => ({
            ...state,
            messageInput: "",
          }))
          return
        }

        const current = yield* ctx.state.get
        if (!current.workspaceConnected) {
          yield* ctx.state.update((state) => ({
            ...state,
            workspaceMessage: "Connect the workspace before sending a message.",
            lastUpdated: nowTime(),
          }))
          return
        }

        yield* ctx.state.update((state) => ({
          ...state,
          busy: { ...state.busy, send: true },
          workspaceMessage: "Sending message...",
        }))

        let threadId = current.threadId
        if (!threadId) {
          const response = yield* invokeCommand<unknown>("start_thread", {
            workspaceId: current.workspaceId,
          })
          threadId = extractThreadId(response)
          if (!threadId) {
            yield* ctx.state.update((state) => ({
              ...state,
              busy: { ...state.busy, send: false },
              workspaceMessage: "Failed to start a new thread.",
              lastUpdated: nowTime(),
            }))
            return
          }
          yield* ctx.state.update((state) => ({
            ...state,
            threadId,
          }))
        }

        yield* invokeCommand<unknown>("send_user_message", {
          workspaceId: current.workspaceId,
          threadId,
          text: rawInput,
          model: null,
          accessMode: null,
        }).pipe(
          Effect.tap(() =>
            ctx.state.update((state) => ({
              ...state,
              busy: { ...state.busy, send: false },
              messageInput: "",
              workspaceMessage: "Message sent.",
              lastUpdated: nowTime(),
            }))
          ),
          Effect.catchAll((error) =>
            ctx.state.update((state) => ({
              ...state,
              busy: { ...state.busy, send: false },
              workspaceMessage: `Send failed: ${String(error)}`,
              lastUpdated: nowTime(),
            }))
          ),
          Effect.asVoid
        )
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
              lastUpdated: nowTime(),
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
              lastUpdated: nowTime(),
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
              lastUpdated: nowTime(),
            }))
          ),
          Effect.catchAll((error) =>
            ctx.state.update((state) => ({
              ...state,
              workspaceConnected: false,
              workspaceMessage: `Status check failed: ${String(error)}`,
              lastUpdated: nowTime(),
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
            lastUpdated: nowTime(),
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
              threadId: response.success ? null : state.threadId,
              lastUpdated: nowTime(),
            }))
          ),
          Effect.catchAll((error) =>
            ctx.state.update((state) => ({
              ...state,
              workspaceConnected: false,
              workspaceMessage: `Connection failed: ${String(error)}`,
              busy: { ...state.busy, connect: false },
              lastUpdated: nowTime(),
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
              threadId: null,
              lastUpdated: nowTime(),
            }))
          ),
          Effect.catchAll((error) =>
            ctx.state.update((state) => ({
              ...state,
              workspaceMessage: `Disconnect failed: ${String(error)}`,
              busy: { ...state.busy, disconnect: false },
              lastUpdated: nowTime(),
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
        const eventThreadId = extractThreadId(event.payload.message)
        const shouldUpdateThread =
          event.payload.message?.method === "thread/started" ||
          current.threadId === null
        yield* ctx.state.update((state) => ({
          ...state,
          lastEventText: formatted.text,
          lastEventTime: formatted.time,
          lastUpdated: formatted.time,
          workspaceConnected:
            event.payload.message?.method === "codex/connected"
              ? true
              : state.workspaceConnected,
          threadId:
            shouldUpdateThread && eventThreadId ? eventThreadId : state.threadId,
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
          void event
          const value = (target as HTMLInputElement).value
          window.localStorage.setItem(workspacePathKey, value)
          emit({ type: "UpdateWorkspacePath", path: value })
        }
      )

      yield* ctx.dom.delegate(
        ctx.container,
        "#command-input",
        "input",
        (event, target) => {
          void event
          const value = (target as HTMLInputElement).value
          emit({ type: "UpdateCommandInput", value })
        }
      )

      yield* ctx.dom.delegate(
        ctx.container,
        "#message-input",
        "input",
        (event, target) => {
          void event
          const value = (target as HTMLInputElement).value
          emit({ type: "UpdateMessageInput", value })
        }
      )

      yield* ctx.dom.delegate(
        ctx.container,
        "#command-input",
        "keydown",
        (event, target) => {
          const keyEvent = event as KeyboardEvent
          if (keyEvent.key === "Enter") {
            keyEvent.preventDefault()
            emit({
              type: "SubmitCommand",
              value: (target as HTMLInputElement).value,
            })
          }
        }
      )

      yield* ctx.dom.delegate(
        ctx.container,
        "#message-input",
        "keydown",
        (event, target) => {
          const keyEvent = event as KeyboardEvent
          if (keyEvent.key === "Enter") {
            keyEvent.preventDefault()
            emit({
              type: "SubmitMessage",
              value: (target as HTMLInputElement).value,
            })
          }
        }
      )

      yield* ctx.dom.delegate(
        ctx.container,
        "[data-action=\"send-message\"]",
        "click",
        () => {
          const input = ctx.container.querySelector(
            "#message-input"
          ) as HTMLInputElement | null
          emit({
            type: "SubmitMessage",
            value: input?.value ?? "",
          })
        }
      )

      const handleKeydown = (event: KeyboardEvent) => {
        if (event.key === "F2") {
          event.preventDefault()
          emit({ type: "ConnectWorkspace" })
        }
        if (event.key === "F3") {
          event.preventDefault()
          emit({ type: "DisconnectWorkspace" })
        }
        if (event.key === "F5") {
          event.preventDefault()
          emit({ type: "RefreshDoctor" })
          emit({ type: "RefreshWorkspaceStatus" })
        }
      }

      window.addEventListener("keydown", handleKeydown)

      yield* Effect.addFinalizer(() =>
        Effect.sync(() => window.removeEventListener("keydown", handleKeydown))
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
