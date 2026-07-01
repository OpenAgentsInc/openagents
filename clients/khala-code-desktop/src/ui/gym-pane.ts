import type { KhalaGymGraphProjection } from "./gym-graph-projection"
import { renderKhalaGymGraphHtml } from "./gym-graph-renderer"

export type GymPaneDetail = Readonly<{
  label: string
  value: string
}>

export type GymPaneActiveParameters = Readonly<{
  actionSubmissionProposalRef?: string
  blockerRefs: ReadonlyArray<string>
  candidateManifestRef?: string
  candidateRef?: string
  caveatRefs: ReadonlyArray<string>
  parameterRef: string
  schemaVersion: "openagents.khala.fleet_delegation.parameters.v0"
  source: "admitted_candidate" | "default"
}>

export type GymPaneLoadedState = Readonly<{
  phase: "loaded"
  title: string
  runRef: string
  status: string
  refs: ReadonlyArray<string>
  activeParameters?: GymPaneActiveParameters
  details?: ReadonlyArray<GymPaneDetail>
  graph?: KhalaGymGraphProjection
}>

export type GymPaneBlockedState = Readonly<{
  phase: "blocked"
  title: string
  blockerRefs: ReadonlyArray<string>
  activeParameters?: GymPaneActiveParameters
  details?: ReadonlyArray<GymPaneDetail>
  graph?: KhalaGymGraphProjection
}>

export type GymPaneState =
  | { readonly phase: "empty"; readonly activeParameters?: GymPaneActiveParameters }
  | GymPaneLoadedState
  | GymPaneBlockedState

export type GymPaneHandle = Readonly<{
  setState: (state: GymPaneState) => void
  setVisible: (visible: boolean) => void
  snapshot: () => GymPaneState
}>

const defaultGymPaneState: GymPaneState = { phase: "empty" }

const el = <K extends keyof HTMLElementTagNameMap>(
  tag: K,
  className?: string,
  text?: string,
): HTMLElementTagNameMap[K] => {
  const node = document.createElement(tag)
  if (className !== undefined) node.className = className
  if (text !== undefined) node.textContent = text
  return node
}

const badge = (state: string, label: string): HTMLElement => {
  const node = el("span", "khala-gym-badge")
  node.dataset.state = state
  node.append(el("span", "khala-gym-dot"), el("span", undefined, label))
  return node
}

const sectionHeader = (title: string, meta?: string): HTMLElement => {
  const header = el("div", "khala-gym-section-header")
  header.append(el("h3", "khala-gym-section-title", title))
  if (meta !== undefined) header.append(el("span", "khala-gym-section-meta", meta))
  return header
}

const refList = (refs: ReadonlyArray<string>): HTMLElement => {
  const list = el("div", "khala-gym-ref-list")
  for (const ref of refs) {
    const chip = el("span", "khala-gym-ref", ref)
    chip.dataset.gymRef = ref
    list.append(chip)
  }
  return list
}

const detailList = (
  details: ReadonlyArray<GymPaneDetail> | undefined,
): HTMLElement | null => {
  if (details === undefined || details.length === 0) return null
  const list = el("dl", "khala-gym-detail-grid")
  for (const item of details) {
    const row = el("div", "khala-gym-detail")
    row.append(
      el("dt", "khala-gym-detail-label", item.label),
      el("dd", "khala-gym-detail-value", item.value),
    )
    list.append(row)
  }
  return list
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

const renderActiveParameters = (
  container: HTMLElement,
  activeParameters: GymPaneActiveParameters | undefined,
): void => {
  if (activeParameters === undefined) return
  const section = el("section", "khala-gym-section khala-gym-parameters")
  section.append(sectionHeader("Active delegation parameters", activeParameters.source))
  const body = el("article", "khala-gym-state")
  const state =
    activeParameters.source === "admitted_candidate" ? "loaded" : "empty"
  body.dataset.state = state
  body.append(
    badge(state, activeParameters.source),
    el("span", "khala-gym-run-ref", activeParameters.parameterRef),
  )
  const details = detailList(activeParameterDetails(activeParameters))
  if (details !== null) body.append(details)
  section.append(body)
  container.append(section)
}

const appendGraph = (
  body: HTMLElement,
  graph: KhalaGymGraphProjection | undefined,
): void => {
  if (graph === undefined) return
  const template = document.createElement("template")
  template.innerHTML = renderKhalaGymGraphHtml(graph, {
    reducedMotion:
      typeof matchMedia === "function" &&
      matchMedia("(prefers-reduced-motion: reduce)").matches,
  }).html
  body.append(template.content.cloneNode(true))
}

const renderEmpty = (
  container: HTMLElement,
  state: Extract<GymPaneState, { readonly phase: "empty" }>,
): void => {
  const section = el("section", "khala-gym-section")
  section.append(sectionHeader("Delegation visibility"))
  const empty = el("div", "khala-gym-state")
  empty.dataset.state = "empty"
  empty.append(
    badge("empty", "No proof"),
    el("p", "khala-gym-empty", "No Gym proof loaded."),
  )
  section.append(empty)
  container.append(section)
  renderActiveParameters(container, state.activeParameters)
}

const renderLoaded = (
  container: HTMLElement,
  state: GymPaneLoadedState,
): void => {
  const section = el("section", "khala-gym-section")
  section.append(sectionHeader(state.title, state.status))
  const body = el("article", "khala-gym-state")
  body.dataset.state = "loaded"
  body.append(
    badge("loaded", "Loaded"),
    el("span", "khala-gym-run-ref", state.runRef),
    refList(state.refs),
  )
  const details = detailList(state.details)
  if (details !== null) body.append(details)
  appendGraph(body, state.graph)
  section.append(body)
  container.append(section)
  renderActiveParameters(container, state.activeParameters)
}

const renderBlocked = (
  container: HTMLElement,
  state: GymPaneBlockedState,
): void => {
  const section = el("section", "khala-gym-section")
  section.append(sectionHeader(state.title, `${state.blockerRefs.length} blockers`))
  const body = el("article", "khala-gym-state")
  body.dataset.state = "blocked"
  body.append(
    badge("blocked", "Blocked"),
    refList(state.blockerRefs),
  )
  const details = detailList(state.details)
  if (details !== null) body.append(details)
  appendGraph(body, state.graph)
  section.append(body)
  container.append(section)
  renderActiveParameters(container, state.activeParameters)
}

const render = (container: HTMLElement, state: GymPaneState): void => {
  container.replaceChildren()

  const header = el("header", "khala-gym-header")
  const titleGroup = el("div", "khala-gym-title-group")
  titleGroup.append(
    el("h2", "khala-gym-title", "Gym"),
    el("p", "khala-gym-subtitle", "Arbiter delegation graph"),
  )
  header.append(titleGroup)
  header.append(badge("readonly", "Read-only"))
  container.append(header)

  const body = el("div", "khala-gym-body")
  if (state.phase === "loaded") {
    renderLoaded(body, state)
  } else if (state.phase === "blocked") {
    renderBlocked(body, state)
  } else {
    renderEmpty(body, state)
  }
  container.append(body)
}

export const mountGymPane = (
  container: HTMLElement,
  initialState: GymPaneState = defaultGymPaneState,
): GymPaneHandle => {
  let state = initialState
  render(container, state)

  return {
    setState: next => {
      state = next
      render(container, state)
    },
    setVisible: visible => {
      container.hidden = !visible
    },
    snapshot: () => state,
  }
}
