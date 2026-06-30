import type { KhalaGymGraphProjection } from "./gym-graph-projection"
import { renderKhalaGymGraphHtml } from "./gym-graph-renderer"

export type GymPaneLoadedState = Readonly<{
  phase: "loaded"
  title: string
  runRef: string
  status: string
  refs: ReadonlyArray<string>
  graph?: KhalaGymGraphProjection
}>

export type GymPaneBlockedState = Readonly<{
  phase: "blocked"
  title: string
  blockerRefs: ReadonlyArray<string>
}>

export type GymPaneState =
  | { readonly phase: "empty" }
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

const renderEmpty = (container: HTMLElement): void => {
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
  if (state.graph !== undefined) {
    const template = document.createElement("template")
    template.innerHTML = renderKhalaGymGraphHtml(state.graph, {
      reducedMotion:
        typeof matchMedia === "function" &&
        matchMedia("(prefers-reduced-motion: reduce)").matches,
    }).html
    body.append(template.content.cloneNode(true))
  }
  section.append(body)
  container.append(section)
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
  section.append(body)
  container.append(section)
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
    renderEmpty(body)
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
