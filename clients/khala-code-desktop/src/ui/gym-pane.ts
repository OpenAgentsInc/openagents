import { iconView } from "@openagentsinc/ui/icon"
import { Effect, Fiber, Queue, Schema as S, Stream } from "effect"
import { Runtime, Subscription } from "foldkit"
import type { Command } from "foldkit"
import type { Document, Html } from "foldkit/html"
import { html } from "foldkit/html"
import { m } from "foldkit/message"
import type { MakeRuntimeReturn } from "foldkit/runtime"
import { ts } from "foldkit/schema"

import { GraphSpec } from "@openagentsinc/arbiter-effect/core"
import { khalaGymGraphFigure } from "./gym-graph-renderer"

const GymPaneDetail = S.Struct({
  label: S.String,
  value: S.String,
})
export type GymPaneDetail = typeof GymPaneDetail.Type

const GymPaneActiveParameters = S.Struct({
  actionSubmissionProposalRef: S.optional(S.String),
  blockerRefs: S.Array(S.String),
  candidateManifestRef: S.optional(S.String),
  candidateRef: S.optional(S.String),
  caveatRefs: S.Array(S.String),
  parameterRef: S.String,
  schemaVersion: S.Literal("openagents.khala.fleet_delegation.parameters.v0"),
  source: S.Literals(["admitted_candidate", "default"]),
})
export type GymPaneActiveParameters = typeof GymPaneActiveParameters.Type

const GymPaneLoadedState = S.Struct({
  phase: S.Literal("loaded"),
  title: S.String,
  runRef: S.String,
  status: S.String,
  refs: S.Array(S.String),
  activeParameters: S.optional(GymPaneActiveParameters),
  details: S.optional(S.Array(GymPaneDetail)),
  graph: S.optional(GraphSpec),
})
export type GymPaneLoadedState = typeof GymPaneLoadedState.Type

const GymPaneBlockedState = S.Struct({
  phase: S.Literal("blocked"),
  title: S.String,
  blockerRefs: S.Array(S.String),
  activeParameters: S.optional(GymPaneActiveParameters),
  details: S.optional(S.Array(GymPaneDetail)),
  graph: S.optional(GraphSpec),
})
export type GymPaneBlockedState = typeof GymPaneBlockedState.Type

const GymPaneEmptyState = S.Struct({
  phase: S.Literal("empty"),
  activeParameters: S.optional(GymPaneActiveParameters),
})

const GymPaneState = S.Union([
  GymPaneEmptyState,
  GymPaneLoadedState,
  GymPaneBlockedState,
])
export type GymPaneState = typeof GymPaneState.Type

export type GymPaneHandle = Readonly<{
  setState: (state: GymPaneState) => void
  setVisible: (visible: boolean) => void
  snapshot: () => GymPaneState
}>

const GymPaneModel = S.Struct({
  mountId: S.String,
  reducedMotion: S.Boolean,
  state: GymPaneState,
})
type GymPaneModel = typeof GymPaneModel.Type

const GymPaneHostPortMessage = S.Union([
  ts("HostGymPaneSetState", {
    state: GymPaneState,
  }),
  ts("HostGymPaneSetReducedMotion", {
    reducedMotion: S.Boolean,
  }),
])
type GymPaneHostPortMessage = typeof GymPaneHostPortMessage.Type

const GymPaneReceivedHostPort = m("GymPaneReceivedHostPort", {
  message: GymPaneHostPortMessage,
})
const GymPaneMounted = m("GymPaneMounted")

const GymPaneMessage = S.Union([
  GymPaneReceivedHostPort,
  GymPaneMounted,
])
type GymPaneMessage = typeof GymPaneMessage.Type

type GymPaneHostPort = Readonly<{
  send: (message: unknown) => GymPaneHostPortMessage
  subscribe: (listener: (message: GymPaneHostPortMessage) => void) => () => void
}>

const defaultGymPaneState: GymPaneState = { phase: "empty" }
const noCommands: ReadonlyArray<Command.Command<GymPaneMessage>> = []

const makeGymPaneHostPort = (): GymPaneHostPort => {
  const decode = S.decodeUnknownSync(GymPaneHostPortMessage)
  const listeners = new Set<(message: GymPaneHostPortMessage) => void>()
  return {
    send: (message) => {
      const decoded = decode(message)
      for (const listener of listeners) listener(decoded)
      return decoded
    },
    subscribe: (listener) => {
      listeners.add(listener)
      return () => listeners.delete(listener)
    },
  }
}

const mountedStream = Stream.make(GymPaneMounted())

const hostPortStream = (
  port: GymPaneHostPort,
): Stream.Stream<GymPaneMessage> =>
  Stream.callback<GymPaneMessage>((queue) =>
    Effect.acquireRelease(
      Effect.sync(() =>
        port.subscribe((message) => {
          Queue.offerUnsafe(queue, GymPaneReceivedHostPort({ message }))
        }),
      ),
      (unsubscribe) => Effect.sync(unsubscribe),
    ),
  )

const makeGymPaneSubscriptions = (port: GymPaneHostPort) =>
  Subscription.make<GymPaneModel, GymPaneMessage>()(() => ({
    mounted: Subscription.persistent(mountedStream),
    hostPort: Subscription.persistent(hostPortStream(port)),
  }))

const update = (
  model: GymPaneModel,
  message: GymPaneMessage,
): readonly [GymPaneModel, ReadonlyArray<Command.Command<GymPaneMessage>>] => {
  switch (message._tag) {
    case "GymPaneReceivedHostPort":
      switch (message.message._tag) {
        case "HostGymPaneSetState":
          return [{ ...model, state: message.message.state }, noCommands]
        case "HostGymPaneSetReducedMotion":
          return [{ ...model, reducedMotion: message.message.reducedMotion }, noCommands]
      }
    case "GymPaneMounted":
      return [model, noCommands]
  }
}

const stateIcon = (state: string): "Check" | "Circle" | "Eye" | "Warning" => {
  if (state === "loaded" || state === "proposal_ready") return "Check"
  if (state === "blocked") return "Warning"
  if (state === "readonly") return "Eye"
  return "Circle"
}

const activeParameterDetails = (
  activeParameters: GymPaneActiveParameters,
): ReadonlyArray<GymPaneDetail> => [
  { label: "source", value: activeParameters.source },
  { label: "parameterRef", value: activeParameters.parameterRef },
  ...(activeParameters.candidateManifestRef === undefined
    ? []
    : [{ label: "candidateManifestRef", value: activeParameters.candidateManifestRef }]),
  ...(activeParameters.candidateRef === undefined
    ? []
    : [{ label: "candidateRef", value: activeParameters.candidateRef }]),
  ...(activeParameters.actionSubmissionProposalRef === undefined
    ? []
    : [{
        label: "actionSubmissionProposalRef",
        value: activeParameters.actionSubmissionProposalRef,
      }]),
  ...(activeParameters.blockerRefs.length === 0
    ? []
    : [{ label: "blockerRefs", value: activeParameters.blockerRefs.join(" ") }]),
  ...(activeParameters.caveatRefs.length === 0
    ? []
    : [{ label: "caveatRefs", value: activeParameters.caveatRefs.join(" ") }]),
]

const detailList = (
  h: ReturnType<typeof html<GymPaneMessage>>,
  details: ReadonlyArray<GymPaneDetail> | undefined,
): Html => {
  if (details === undefined || details.length === 0) return null
  return h.dl([h.Class("khala-gym-detail-grid")], details.map(item =>
    h.div([h.Class("khala-gym-detail")], [
      h.dt([h.Class("khala-gym-detail-label")], [item.label]),
      h.dd([h.Class("khala-gym-detail-value")], [item.value]),
    ]),
  ))
}

const badge = (
  h: ReturnType<typeof html<GymPaneMessage>>,
  state: string,
  label: string,
): Html =>
  h.span([
    h.Class("khala-gym-badge"),
    h.DataAttribute("state", state),
  ], [
    iconView<GymPaneMessage>(stateIcon(state), "khala-gym-badge-icon"),
    h.span([], [label]),
  ])

const sectionHeader = (
  h: ReturnType<typeof html<GymPaneMessage>>,
  title: string,
  meta?: string,
): Html =>
  h.div([h.Class("khala-gym-section-header")], [
    h.h3([h.Class("khala-gym-section-title")], [title]),
    meta === undefined
      ? null
      : h.span([h.Class("khala-gym-section-meta")], [meta]),
  ])

const refList = (
  h: ReturnType<typeof html<GymPaneMessage>>,
  refs: ReadonlyArray<string>,
): Html =>
  h.div([h.Class("khala-gym-ref-list")], refs.map(ref =>
    h.span([
      h.Class("khala-gym-ref"),
      h.DataAttribute("gym-ref", ref),
    ], [ref]),
  ))

const graphFigure = (
  model: GymPaneModel,
  graph: GymPaneLoadedState["graph"] | GymPaneBlockedState["graph"],
): Html =>
  graph === undefined
    ? null
    : khalaGymGraphFigure<GymPaneMessage>({
        projection: graph,
        options: { reducedMotion: model.reducedMotion },
      })

const renderActiveParameters = (
  h: ReturnType<typeof html<GymPaneMessage>>,
  activeParameters: GymPaneActiveParameters | undefined,
): Html => {
  if (activeParameters === undefined) return null
  const state =
    activeParameters.source === "admitted_candidate" ? "loaded" : "empty"
  return h.section([
    h.Class("khala-gym-section khala-gym-parameters"),
  ], [
    sectionHeader(h, "Active delegation parameters", activeParameters.source),
    h.article([
      h.Class("khala-gym-state"),
      h.DataAttribute("state", state),
    ], [
      badge(h, state, activeParameters.source),
      h.span([h.Class("khala-gym-run-ref")], [activeParameters.parameterRef]),
      detailList(h, activeParameterDetails(activeParameters)),
    ]),
  ])
}

const renderEmpty = (
  h: ReturnType<typeof html<GymPaneMessage>>,
  state: Extract<GymPaneState, { readonly phase: "empty" }>,
): ReadonlyArray<Html> => [
  h.section([h.Class("khala-gym-section")], [
    sectionHeader(h, "Delegation visibility"),
    h.div([
      h.Class("khala-gym-state"),
      h.DataAttribute("state", "empty"),
    ], [
      badge(h, "empty", "No proof"),
      h.p([h.Class("khala-gym-empty")], ["No Gym proof loaded."]),
    ]),
  ]),
  renderActiveParameters(h, state.activeParameters),
]

const renderLoaded = (
  h: ReturnType<typeof html<GymPaneMessage>>,
  model: GymPaneModel,
  state: GymPaneLoadedState,
): ReadonlyArray<Html> => [
  h.section([h.Class("khala-gym-section")], [
    sectionHeader(h, state.title, state.status),
    h.article([
      h.Class("khala-gym-state"),
      h.DataAttribute("state", "loaded"),
    ], [
      badge(h, "loaded", "Loaded"),
      h.span([h.Class("khala-gym-run-ref")], [state.runRef]),
      refList(h, state.refs),
      detailList(h, state.details),
      graphFigure(model, state.graph),
    ]),
  ]),
  renderActiveParameters(h, state.activeParameters),
]

const renderBlocked = (
  h: ReturnType<typeof html<GymPaneMessage>>,
  model: GymPaneModel,
  state: GymPaneBlockedState,
): ReadonlyArray<Html> => [
  h.section([h.Class("khala-gym-section")], [
    sectionHeader(h, state.title, `${state.blockerRefs.length} blockers`),
    h.article([
      h.Class("khala-gym-state"),
      h.DataAttribute("state", "blocked"),
    ], [
      badge(h, "blocked", "Blocked"),
      refList(h, state.blockerRefs),
      detailList(h, state.details),
      graphFigure(model, state.graph),
    ]),
  ]),
  renderActiveParameters(h, state.activeParameters),
]

const view = (model: GymPaneModel): Document => {
  const h = html<GymPaneMessage>()
  const stateBody =
    model.state.phase === "loaded"
      ? renderLoaded(h, model, model.state)
      : model.state.phase === "blocked"
        ? renderBlocked(h, model, model.state)
        : renderEmpty(h, model.state)

  return {
    title: "Khala Code",
    body: h.section([
      h.Class("khala-gym-foldkit"),
      h.DataAttribute("gym-pane-mount-id", model.mountId),
    ], [
      h.header([h.Class("khala-gym-header")], [
        h.div([h.Class("khala-gym-title-group")], [
          h.h2([h.Class("khala-gym-title")], ["Gym"]),
          h.p([h.Class("khala-gym-subtitle")], ["Arbiter delegation graph"]),
        ]),
        badge(h, "readonly", "Read-only"),
      ]),
      h.div([h.Class("khala-gym-body")], stateBody),
    ]),
  }
}

const nextMountId = (() => {
  let seq = 0
  return () => {
    seq += 1
    return `khala-code-gym-pane-${seq}`
  }
})()

const initialReducedMotion = (): boolean =>
  typeof matchMedia === "function" &&
  matchMedia("(prefers-reduced-motion: reduce)").matches

const startRuntimeFiber = (program: MakeRuntimeReturn): Fiber.Fiber<void> =>
  Effect.runFork(program.start())

export const mountGymPane = (
  container: HTMLElement,
  initialState: GymPaneState = defaultGymPaneState,
): GymPaneHandle => {
  let state = initialState
  const mountId = nextMountId()
  const port = makeGymPaneHostPort()
  const runtimeContainer = document.createElement("div")
  runtimeContainer.id = mountId
  runtimeContainer.dataset.khalaCodeGymPaneRuntime = "true"
  container.replaceChildren(runtimeContainer)

  const program = Runtime.makeProgram({
    Model: GymPaneModel,
    init: () => [
      {
        mountId,
        reducedMotion: initialReducedMotion(),
        state,
      },
      [],
    ],
    update,
    view,
    subscriptions: makeGymPaneSubscriptions(port),
    container: runtimeContainer,
    devTools: {
      show: "Development",
      Message: GymPaneMessage,
    },
    crash: {
      report: ({ error }) => {
        console.error("[khala-code-desktop/gym-pane] crash:", error)
      },
    },
  })
  startRuntimeFiber(program)

  return {
    setState: next => {
      state = next
      port.send({ _tag: "HostGymPaneSetState", state: next })
      port.send({ _tag: "HostGymPaneSetReducedMotion", reducedMotion: initialReducedMotion() })
    },
    setVisible: visible => {
      container.hidden = !visible
    },
    snapshot: () => state,
  }
}
