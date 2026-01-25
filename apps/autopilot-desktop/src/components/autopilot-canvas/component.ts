import { Effect, Schema } from "effect"
import { listen } from "@tauri-apps/api/event"
import { nanoid } from "nanoid"
import {
  applyPatch,
  renderTree,
  resolveAction,
  executeActionWithErrorHandling,
  setByPath,
  type Action,
  type DataModel,
  type JsonPatch,
  type UITree,
  type Component,
  makeEzRegistry,
  mountEzRuntimeWith,
} from "../../effuse/index.js"
import { UiEventSchema } from "../../contracts/tauri.js"
import type { UiEvent } from "../../gen/tauri-contracts"
import {
  connectUnifiedAgent,
  getCurrentDirectory,
  sendUnifiedMessage,
  startUnifiedSession,
} from "../../ipc/unified.js"
import { effuseCatalog, componentRegistry } from "../catalog.js"
import { createSetupTree } from "./setup-tree.js"

type AutopilotCanvasState = {
  tree: UITree
  dataModel: DataModel
  sessionId: string | null
  status: "idle" | "running" | "error"
  error: string | null
}

const createInitialData = (): DataModel => ({
  workspace: { path: "" },
  task: { prompt: "" },
  session: { id: null },
  status: { phase: "Idle" },
})

const updateDataModel = (
  current: AutopilotCanvasState,
  path: string,
  value: unknown
): AutopilotCanvasState => {
  const nextModel = structuredClone(current.dataModel) as DataModel
  setByPath(nextModel, path, value)
  return { ...current, dataModel: nextModel }
}

export const AutopilotCanvasComponent: Component<AutopilotCanvasState, never> = {
  id: "autopilot-canvas",

  initialState: () => ({
    tree: createSetupTree(),
    dataModel: createInitialData(),
    sessionId: null,
    status: "idle",
    error: null,
  }),

  render: (ctx) =>
    Effect.gen(function* () {
      const state = yield* ctx.state.get
      const validated = effuseCatalog.validateTree(state.tree)
      const tree = validated.success && validated.data ? validated.data : createSetupTree()
      return renderTree(tree, componentRegistry, { dataModel: state.dataModel })
    }),

  setupEvents: (ctx) =>
    Effect.gen(function* () {
      const setData = (path: string, value: unknown) => {
        Effect.runFork(
          ctx.state.update((current) => updateDataModel(current, path, value))
        )
      }

      const runActionByName = async (name: string, params: Record<string, unknown>) => {
        if (name === "ui.set") {
          const path = typeof params.path === "string" ? params.path : ""
          if (!path) {
            throw new Error("ui.set requires a path")
          }
          setData(path, params.value)
          return
        }

        if (name === "ui.start") {
          const workspacePath =
            typeof params.workspacePath === "string" ? params.workspacePath.trim() : ""
          if (!workspacePath) {
            throw new Error("Working directory is required")
          }

          const prompt = typeof params.prompt === "string" ? params.prompt.trim() : ""
          const workspaceId = nanoid()
          const connect = await Effect.runPromise(
            connectUnifiedAgent({
              agentIdStr: "Adjutant",
              workspacePath,
              workspaceId,
            })
          )

          await Effect.runPromise(
            startUnifiedSession({
              sessionId: connect.sessionId,
              workspacePath,
            })
          )

          if (prompt) {
            await Effect.runPromise(
              sendUnifiedMessage({
                sessionId: connect.sessionId,
                text: prompt,
              })
            )
          }

          Effect.runFork(
            ctx.state.update((current) => {
              const next = updateDataModel(current, "/status/phase", "Running")
              const withSession = updateDataModel(
                { ...next, sessionId: connect.sessionId, status: "running", error: null },
                "/session/id",
                connect.sessionId
              )
              return withSession
            })
          )
          return
        }

        if (name === "ui.refresh") {
          return
        }

        throw new Error(`Unknown action: ${name}`)
      }

      const handleAction = (action: Action, params: Record<string, string>) =>
        Effect.gen(function* () {
          const state = yield* ctx.state.get
          const resolutionModel = {
            ...state.dataModel,
            __event: { params },
          }

          const resolved = resolveAction(action, resolutionModel)
          if (resolved.confirm) {
            const confirmMessage = resolved.confirm.title
              ? `${resolved.confirm.title}\n${resolved.confirm.message}`
              : resolved.confirm.message
            if (!window.confirm(confirmMessage)) {
              return
            }
          }

          yield* Effect.tryPromise({
            try: () =>
              executeActionWithErrorHandling({
                action: resolved,
                handler: (payload) => runActionByName(resolved.name, payload as Record<string, unknown>),
                setData,
                executeAction: (name) => runActionByName(name, {}),
              }),
            catch: (error) => new Error(String(error)),
          })
        })

      const registry = makeEzRegistry([
        [
          "ui.action",
          ({ params }) =>
            Effect.gen(function* () {
              const actionJson = params.action
              if (!actionJson) {
                return
              }
              const action = yield* Effect.try({
                try: () => JSON.parse(actionJson) as Action,
                catch: (error) => new Error(String(error)),
              }).pipe(
                Effect.catchAll((error) => {
                  console.warn("[AutopilotCanvas] Failed to parse action:", error)
                  return Effect.succeed(null)
                })
              )

              if (!action) {
                return
              }

              yield* handleAction(action, params)
            }),
        ],
      ])

      yield* mountEzRuntimeWith(ctx.container, registry)

      yield* getCurrentDirectory().pipe(
        Effect.tap((cwd) => {
          setData("/workspace/path", cwd)
        }),
        Effect.catchAll(() => Effect.void)
      )

      const handleUiEvent = (uiEvent: UiEvent) =>
        ctx.state.update((current) => {
          if (current.sessionId && uiEvent.session_id !== current.sessionId) {
            return current
          }

          if (uiEvent.type === "UiTreeReset") {
            const validated = effuseCatalog.validateTree(uiEvent.tree)
            if (!validated.success || !validated.data) {
              return { ...current, error: "Received invalid UI tree", status: "error" }
            }
            return { ...current, tree: validated.data }
          }

          if (uiEvent.type === "UiPatch") {
            const patch = uiEvent.patch as JsonPatch
            return { ...current, tree: applyPatch(current.tree, patch) }
          }

          if (uiEvent.type === "UiDataUpdate") {
            return updateDataModel(current, uiEvent.path, uiEvent.value)
          }

          return current
        })

      const unlisten = yield* Effect.tryPromise({
        try: () =>
          listen<unknown>("ui-event", (event) => {
            Effect.runFork(
              Schema.decodeUnknown(UiEventSchema)(event.payload).pipe(
                Effect.tap((uiEvent) => handleUiEvent(uiEvent))
              )
            )
          }),
        catch: (error) => new Error(String(error)),
      })

      yield* Effect.addFinalizer(() => Effect.promise(() => unlisten()))
    }),
}
