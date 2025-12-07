/**
 * Effuse - Effect-Native UI Framework
 *
 * A lightweight, type-safe UI framework for the OpenAgents mainview,
 * inspired by Typed but simplified for our desktop HUD use case.
 *
 * @example
 * ```typescript
 * import { Effect } from "effect"
 * import { html, mountWidget, EffuseLive, type Widget } from "./effuse/index.js"
 *
 * interface CounterState {
 *   count: number
 * }
 *
 * type CounterEvent =
 *   | { type: "increment" }
 *   | { type: "decrement" }
 *
 * const CounterWidget: Widget<CounterState, CounterEvent> = {
 *   id: "counter",
 *   initialState: () => ({ count: 0 }),
 *
 *   render: (ctx) =>
 *     Effect.gen(function* () {
 *       const { count } = yield* ctx.state.get
 *       return html`
 *         <div class="counter">
 *           <span>Count: ${count}</span>
 *           <button data-action="decrement">-</button>
 *           <button data-action="increment">+</button>
 *         </div>
 *       `
 *     }),
 *
 *   setupEvents: (ctx) =>
 *     Effect.gen(function* () {
 *       yield* ctx.dom.delegate(ctx.container, "[data-action]", "click", (e, target) => {
 *         const action = (target as HTMLElement).dataset.action
 *         if (action === "increment") ctx.emit({ type: "increment" })
 *         if (action === "decrement") ctx.emit({ type: "decrement" })
 *       })
 *     }),
 *
 *   handleEvent: (event, ctx) =>
 *     Effect.gen(function* () {
 *       switch (event.type) {
 *         case "increment":
 *           yield* ctx.state.update(s => ({ count: s.count + 1 }))
 *           break
 *         case "decrement":
 *           yield* ctx.state.update(s => ({ count: s.count - 1 }))
 *           break
 *       }
 *     }),
 * }
 *
 * // Mount the widget
 * const program = Effect.gen(function* () {
 *   const container = document.getElementById("counter")!
 *   yield* mountWidget(CounterWidget, container)
 * })
 *
 * Effect.runPromise(
 *   program.pipe(
 *     Effect.provide(EffuseLive),
 *     Effect.scoped
 *   )
 * )
 * ```
 */

// Template system
export { html, rawHtml, joinTemplates } from "./template/html.js"
export type { TemplateResult, TemplateValue } from "./template/types.js"
export { isTemplateResult } from "./template/types.js"
export { escapeHtml } from "./template/escape.js"

// State management
export type { StateCell } from "./state/cell.js"
export { makeCell, readonly } from "./state/cell.js"

// Services
export { DomServiceTag, DomError, type DomService } from "./services/dom.js"
export { StateServiceTag, type StateService } from "./services/state.js"
export {
  SocketServiceTag,
  SocketError,
  type SocketService,
  type StartTBRunOptions,
  type AssignTaskOptions,
} from "./services/socket.js"

// Service implementations
export { DomServiceLive, DomServiceScoped } from "./services/dom-live.js"
export { StateServiceLive } from "./services/state-live.js"
export {
  SocketServiceLive,
  SocketServiceFromClient,
  SocketServiceDefault,
} from "./services/socket-live.js"

// Widget system
export type {
  Widget,
  WidgetContext,
  MountedWidget,
  WidgetState,
  WidgetEvent,
  WidgetRequirements,
} from "./widget/types.js"
export { mountWidget, mountWidgetById, mountWidgets } from "./widget/mount.js"

// Layers
export { EffuseLive, EffuseLiveNoSocket } from "./layers/live.js"
export { makeTestLayer, makeCustomTestLayer, type TestLayerResult } from "./layers/test.js"

// Widgets
export { APMWidget, type APMState, type APMEvent, initialAPMState } from "./widgets/apm-widget.js"
export {
  TrajectoryPaneWidget,
  type TrajectoryPaneState,
  type TrajectoryPaneEvent,
  initialTrajectoryPaneState,
} from "./widgets/trajectory-pane.js"
export {
  ContainerPanesWidget,
  type ContainerPanesState,
  type ContainerPanesEvent,
  type ContainerPane,
  type ContainerOutputLine,
  type ContainerStreamType,
  initialContainerPanesState,
} from "./widgets/container-panes.js"
export {
  TBOutputWidget,
  type TBOutputState,
  type TBOutputEvent,
  type TBOutputLine,
  type TBOutputSource,
  initialTBOutputState,
} from "./widgets/tb-output.js"
export {
  MCTasksWidget,
  type MCTasksState,
  type MCTasksEvent,
  type MCTask,
  initialMCTasksState,
} from "./widgets/mc-tasks.js"
export {
  TBControlsWidget,
  type TBControlsState,
  type TBControlsEvent,
  type TBSuiteInfo,
  type TBTaskInfo,
  initialTBControlsState,
} from "./widgets/tb-controls.js"
export {
  CategoryTreeWidget,
  type CategoryTreeState,
  type CategoryTreeEvent,
  type CategoryData,
  type TBTaskData,
  type TBTaskStatus,
  initialCategoryTreeState,
} from "./widgets/category-tree.js"
export {
  HFTrajectoryListWidget,
  type HFTrajectoryListState,
  type HFTrajectoryListEvent,
  type TrajectoryMetadata,
  initialHFTrajectoryListState,
} from "./widgets/hf-trajectory-list.js"
export {
  HFTrajectoryDetailWidget,
  type HFTrajectoryDetailState,
  type HFTrajectoryDetailEvent,
  initialHFTrajectoryDetailState,
} from "./widgets/hf-trajectory-detail.js"
