/**
 * Effuse - Effect-Native UI Framework
 *
 * A lightweight, type-safe UI framework for the OpenAgents mainview,
 * inspired by Typed but simplified for our desktop HUD use case.
 *
 * @example
 * ```typescript
 * import { Effect } from "effect"
 * import { html, mountComponent, EffuseLive, type Component } from "./effuse/index.js"
 *
 * interface CounterState {
 *   count: number
 * }
 *
 * type CounterEvent =
 *   | { type: "increment" }
 *   | { type: "decrement" }
 *
 * const CounterComponent: Component<CounterState, CounterEvent> = {
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
 * // Mount the component
 * const program = Effect.gen(function* () {
 *   const container = document.getElementById("counter")!
 *   yield* mountComponent(CounterComponent, container)
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

// Component system
export type {
  Component,
  ComponentContext,
  MountedComponent,
  ComponentState,
  ComponentEvent,
  ComponentRequirements,
} from "./component/types.js"
export { mountComponent, mountComponentById, mountComponents } from "./component/mount.js"

// Layers
export { EffuseLive, EffuseLiveNoSocket } from "./layers/live.js"
export { makeTestLayer, makeCustomTestLayer, type TestLayerResult } from "./layers/test.js"

// HMR (Hot Module Replacement)
export {
  saveComponentState,
  loadComponentState,
  hasComponentState,
  clearAllState,
  getHMRVersion,
  bumpHMRVersion,
} from "./hmr/index.js"

// Components
export { APMComponent, type APMState, type APMEvent, initialAPMState } from "./components/apm-widget.js"
export {
  TrajectoryPaneComponent,
  type TrajectoryPaneState,
  type TrajectoryPaneEvent,
  initialTrajectoryPaneState,
} from "./components/trajectory-pane.js"
export {
  IntroCardComponent,
  type IntroCardState,
  type IntroCardEvent,
} from "./components/intro-card.js"
export {
  ThreeBackgroundComponent,
  type ThreeBackgroundState,
  type ThreeBackgroundEvent,
} from "./components/three-background.js"
export {
  AgentGraphComponent,
  type AgentGraphState,
  type AgentGraphEvent,
} from "./components/agent-graph/index.js"
export {
  TestGenGraphComponent,
  type TestGenGraphState,
  type TestGenGraphEvent,
} from "./components/testgen-graph/index.js"
export {
  ContainerPanesComponent,
  type ContainerPanesState,
  type ContainerPanesEvent,
  type ContainerPane,
  type ContainerOutputLine,
  type ContainerStreamType,
  initialContainerPanesState,
} from "./components/container-panes.js"
export {
  TBOutputComponent,
  type TBOutputState,
  type TBOutputEvent,
  type TBOutputLine,
  type TBOutputSource,
  initialTBOutputState,
} from "./components/tb-output.js"
export {
  MCTasksComponent,
  type MCTasksState,
  type MCTasksEvent,
  type MCTask,
  initialMCTasksState,
} from "./components/mc-tasks.js"
export {
  TBControlsComponent,
  type TBControlsState,
  type TBControlsEvent,
  type TBSuiteInfo,
  type TBTaskInfo,
  initialTBControlsState,
} from "./components/tb-controls.js"
export {
  CategoryTreeComponent,
  type CategoryTreeState,
  type CategoryTreeEvent,
  type CategoryData,
  type TBTaskData,
  type TBTaskStatus,
  initialCategoryTreeState,
} from "./components/category-tree.js"
export {
  HFTrajectoryListComponent,
  type HFTrajectoryListState,
  type HFTrajectoryListEvent,
  type TrajectoryMetadata,
  initialHFTrajectoryListState,
} from "./components/hf-trajectory-list.js"
export {
  HFTrajectoryDetailComponent,
  type HFTrajectoryDetailState,
  type HFTrajectoryDetailEvent,
  initialHFTrajectoryDetailState,
} from "./components/hf-trajectory-detail.js"
export * from "./components/tb-command-center/index.js"

// Components
export {
  renderThreadContainer,
  renderThreadItem,
  type ThreadItem,
  type ProgressData,
  type ReflectionData,
  type TestData,
  type CompleteData,
  type ErrorData,
  type ThreadItemState,
  type ThreadOptions,
} from "./components/atif-thread.js"
