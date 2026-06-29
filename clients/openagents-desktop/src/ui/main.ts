import { Electroview } from "electrobun/view"

import {
  type CodingProcess,
  OPENAGENTS_DESKTOP_CODING_POLL_INTERVAL_MS,
  type CodingStatusResult,
  type CodingSupervisorEvent,
} from "../shared/coding-status"
import {
  type DesktopPylon,
  OPENAGENTS_DESKTOP_PYLON_POLL_INTERVAL_MS,
  type PylonStatusResult,
} from "../shared/pylon-status"
import {
  OPENAGENTS_DESKTOP_RPC_MAX_REQUEST_TIME_MS,
  type OpenAgentsDesktopRPCSchema,
} from "../shared/rpc"
import { mountLandingSquares } from "./landing-squares"
import "./styles.css"

const rpc = Electroview.defineRPC<OpenAgentsDesktopRPCSchema>({
  maxRequestTime: OPENAGENTS_DESKTOP_RPC_MAX_REQUEST_TIME_MS,
  handlers: {
    requests: {},
    messages: {},
  },
})

new Electroview({ rpc })

const requireElement = <T extends Element>(selector: string): T => {
  const element = document.querySelector<T>(selector)
  if (element === null) throw new Error(`Missing ${selector}`)
  return element
}

const shell = requireElement<HTMLElement>(".openagents-shell")
const scene = requireElement<HTMLElement>("#openagents-scene")
const codingStatus = requireElement<HTMLButtonElement>("#coding-status")
const codingCount = requireElement<HTMLElement>("#coding-count")
const pylonStatus = requireElement<HTMLButtonElement>("#pylon-status")
const pylonCount = requireElement<HTMLElement>("#pylon-count")
const pylonsPage = requireElement<HTMLElement>("#pylons-page")
const pylonsBack = requireElement<HTMLButtonElement>("#pylons-back")
const pylonsSummary = requireElement<HTMLElement>("#pylons-summary")
const pylonsList = requireElement<HTMLElement>("#pylons-list")
const createPylonButton = requireElement<HTMLButtonElement>("#create-pylon")
const pylonActionStatus = requireElement<HTMLElement>("#pylon-action-status")
const codingPage = requireElement<HTMLElement>("#coding-page")
const codingBack = requireElement<HTMLButtonElement>("#coding-back")
const codingObserved = requireElement<HTMLElement>("#coding-observed")
const codingSummary = requireElement<HTMLElement>("#coding-summary")
const codingMetricCodex = requireElement<HTMLElement>("#coding-metric-codex")
const codingMetricBurning = requireElement<HTMLElement>("#coding-metric-burning")
const codingMetricKhala = requireElement<HTMLElement>("#coding-metric-khala")
const codingMetricReady = requireElement<HTMLElement>("#coding-metric-ready")
const codingList = requireElement<HTMLElement>("#coding-list")
const codingEvents = requireElement<HTMLElement>("#coding-events")
const codingDispatchSummary = requireElement<HTMLElement>(
  "#coding-dispatch-summary",
)

const prefersReducedMotion = globalThis.matchMedia?.(
  "(prefers-reduced-motion: reduce)",
).matches ?? false

const handle = mountLandingSquares(scene, {
  animate: !prefersReducedMotion,
  pose: "landing",
})

globalThis.addEventListener("pagehide", () => {
  handle.dispose()
})

const formatCount = (value: number): string =>
  new Intl.NumberFormat("en-US").format(Math.max(0, Math.trunc(value)))

const formatTimestamp = (value: string | null, label: string | null): string => {
  if (label !== null) return label
  if (value === null) return "No heartbeat"
  const date = new Date(value)
  if (!Number.isFinite(date.getTime())) return value
  return new Intl.DateTimeFormat("en-US", {
    dateStyle: "medium",
    timeStyle: "short",
  }).format(date)
}

const pylonIsOnline = (pylon: DesktopPylon): boolean =>
  pylon.heartbeatFresh || pylon.status.trim().toLowerCase() === "online"

const pylonStatusLabel = (pylon: DesktopPylon): string =>
  pylonIsOnline(pylon) ? "online" : pylon.status || "unknown"

const slotCell = (label: string, value: number): HTMLElement => {
  const cell = document.createElement("span")
  cell.className = "pylon-slot"

  const labelEl = document.createElement("span")
  labelEl.className = "pylon-slot-label"
  labelEl.textContent = label

  const valueEl = document.createElement("strong")
  valueEl.textContent = formatCount(value)

  cell.append(labelEl, valueEl)
  return cell
}

const pylonRow = (pylon: DesktopPylon): HTMLElement => {
  const row = document.createElement("article")
  row.className = "pylon-row"
  row.dataset.state = pylonIsOnline(pylon) ? "online" : "stale"

  const identity = document.createElement("div")
  identity.className = "pylon-row-identity"

  const ref = document.createElement("strong")
  ref.textContent = pylon.pylonRef

  const heartbeat = document.createElement("span")
  heartbeat.textContent = formatTimestamp(
    pylon.latestHeartbeatAt,
    pylon.latestHeartbeatLabel,
  )

  identity.append(ref, heartbeat)

  const status = document.createElement("span")
  status.className = "pylon-row-status"
  status.textContent = pylonStatusLabel(pylon)

  const slots = document.createElement("div")
  slots.className = "pylon-row-slots"
  slots.append(
    slotCell("Ready", pylon.readySlots),
    slotCell("Busy", pylon.busySlots),
    slotCell("Queued", pylon.queuedSlots),
  )

  row.append(identity, status, slots)
  return row
}

const renderPylonsPage = (result: PylonStatusResult): void => {
  pylonsSummary.textContent = `Pylons: ${formatCount(result.count)}`
  pylonsList.replaceChildren()

  if (result.pylons.length === 0) {
    const empty = document.createElement("div")
    empty.className = "pylon-empty"
    empty.textContent = result.ok
      ? result.notice ?? "No pylons connected."
      : result.error
    pylonsList.append(empty)
    return
  }

  pylonsList.append(...result.pylons.map(pylonRow))
}

const processRow = (process: CodingProcess): HTMLElement => {
  const row = document.createElement("article")
  row.className = "coding-row"
  row.dataset.state = process.status

  const identity = document.createElement("div")
  identity.className = "coding-row-identity"

  const label = document.createElement("strong")
  label.textContent = process.label

  const meta = document.createElement("span")
  meta.textContent = `PID ${formatCount(process.pid)} · ${process.age} · ${process.cpuPercent.toFixed(1)}% CPU`

  identity.append(label, meta)

  const status = document.createElement("span")
  status.className = "coding-row-status"
  status.textContent = process.status

  row.append(identity, status)
  return row
}

const eventRow = (event: CodingSupervisorEvent): HTMLElement => {
  const row = document.createElement("article")
  row.className = "coding-event"

  const status = document.createElement("strong")
  status.textContent = event.status

  const text = document.createElement("span")
  const slot = event.slot === null ? "" : `slot ${formatCount(event.slot)} `
  const account = event.accountRef === null ? "" : `${event.accountRef} `
  const issue = event.issueRef === null ? "" : `#${event.issueRef} `
  text.textContent = `${slot}${account}${issue}${event.text}`.trim()

  row.append(status, text)
  return row
}

const renderCodingStatus = (result: CodingStatusResult): void => {
  const summary = result.summary
  codingCount.textContent = `Coding: ${formatCount(summary.codexExecCount)}`
  codingStatus.dataset.state =
    summary.codexExecCount > 0
      ? summary.burningCodexCount > 0
        ? "online"
        : "empty"
      : "unknown"
  codingStatus.title = result.ok ? "Open Coding" : result.error

  codingObserved.textContent = formatTimestamp(result.observedAt, "Local now")
  codingSummary.textContent = `Codex: ${formatCount(summary.codexExecCount)}`
  codingMetricCodex.textContent = formatCount(summary.codexExecCount)
  codingMetricBurning.textContent = formatCount(summary.burningCodexCount)
  codingMetricKhala.textContent = formatCount(summary.khalaRequestCount)
  codingMetricReady.textContent =
    summary.readyCodex === null ? "-" : formatCount(summary.readyCodex)

  codingList.replaceChildren()
  const visibleProcesses = result.processes
    .filter(process => process.kind !== "supervisor")
    .slice(0, 12)
  if (visibleProcesses.length === 0) {
    const empty = document.createElement("div")
    empty.className = "coding-empty"
    empty.textContent = result.ok
      ? "No live coding agent processes."
      : result.error
    codingList.append(empty)
  } else {
    codingList.append(...visibleProcesses.map(processRow))
  }

  codingDispatchSummary.textContent = [
    `OK ${formatCount(summary.okRecent)}`,
    `No dispatch ${formatCount(summary.noDispatchRecent)}`,
    `Lockout ${formatCount(summary.lockoutRecent)}`,
    summary.desiredSlots === null
      ? null
      : `Desired ${formatCount(summary.desiredSlots)}`,
    `Claims ${formatCount(summary.claimCount)}`,
    summary.openIssueCount === null
      ? null
      : `Issues ${formatCount(summary.openIssueCount)}`,
    `Vertex ${formatCount(summary.vertexBurnCount)}`,
  ]
    .filter((value): value is string => value !== null)
    .join(" · ")

  codingEvents.replaceChildren()
  if (result.events.length === 0) {
    const empty = document.createElement("div")
    empty.className = "coding-empty"
    empty.textContent = "No recent supervisor events."
    codingEvents.append(empty)
  } else {
    codingEvents.append(...result.events.map(eventRow))
  }

}

const renderPylonStatus = (result: PylonStatusResult): void => {
  pylonCount.textContent = `Pylons: ${formatCount(result.count)}`
  pylonStatus.dataset.state =
    result.ok && result.count > 0 ? "online" : result.ok ? "empty" : "unknown"
  pylonStatus.title = result.ok ? "Open Pylons" : result.error
  renderPylonsPage(result)
}

const loadPylonStatus = async (): Promise<void> => {
  try {
    renderPylonStatus(await rpc.request.pylonStatus())
  } catch (error) {
    renderPylonStatus({
      ok: false,
      count: 0,
      pylons: [],
      error: error instanceof Error ? error.message : String(error),
      observedAt: new Date().toISOString(),
    })
  }
}

const loadCodingStatus = async (): Promise<void> => {
  try {
    renderCodingStatus(await rpc.request.codingStatus())
  } catch (error) {
    renderCodingStatus({
      ok: false,
      events: [],
      processes: [],
      summary: {
        assignmentRunnerCount: 0,
        burningCodexCount: 0,
        claimCount: 0,
        codexExecCount: 0,
        desiredSlots: null,
        khalaRequestCount: 0,
        lastDispatchAt: null,
        lockoutRecent: 0,
        noDispatchRecent: 0,
        okRecent: 0,
        openIssueCount: null,
        pylonNodeCount: 0,
        readyCodex: null,
        standingPylonCount: 0,
        supervisorCount: 0,
        vertexBurnCount: 0,
      },
      error: error instanceof Error ? error.message : String(error),
      observedAt: new Date().toISOString(),
    })
  }
}

type DesktopRoute = "coding" | "landing" | "pylons"

const routeFromLocation = (): DesktopRoute => {
  const route = globalThis.location.hash.replace(/^#\/?/, "")
  return route === "pylons" || route === "coding" ? route : "landing"
}

const applyRoute = (route: DesktopRoute): void => {
  shell.dataset.route = route
  pylonsPage.hidden = route !== "pylons"
  codingPage.hidden = route !== "coding"
  codingStatus.setAttribute("aria-expanded", route === "coding" ? "true" : "false")
  pylonStatus.setAttribute("aria-expanded", route === "pylons" ? "true" : "false")
  handle.setPose(route === "landing" ? "landing" : "pylons")
}

const navigateTo = (route: DesktopRoute): void => {
  if (route !== "landing") {
    const hash = `#${route}`
    if (globalThis.location.hash !== hash) {
      globalThis.location.hash = route
    }
    applyRoute(route)
    return
  }

  if (globalThis.location.hash !== "") {
    globalThis.history.pushState({}, "", `${globalThis.location.pathname}${globalThis.location.search}`)
  }
  applyRoute("landing")
}

codingStatus.addEventListener("click", () => navigateTo("coding"))
pylonStatus.addEventListener("click", () => navigateTo("pylons"))
codingBack.addEventListener("click", () => navigateTo("landing"))
pylonsBack.addEventListener("click", () => navigateTo("landing"))
globalThis.addEventListener("hashchange", () => applyRoute(routeFromLocation()))
globalThis.addEventListener("popstate", () => applyRoute(routeFromLocation()))

let isCreatingPylon = false
createPylonButton.addEventListener("click", () => {
  if (isCreatingPylon) return

  isCreatingPylon = true
  createPylonButton.disabled = true
  pylonActionStatus.textContent = "Starting Pylon..."

  void rpc.request
    .createPylon()
    .then(result => {
      pylonActionStatus.textContent = result.ok
        ? result.pid === null
          ? "Pylon started."
          : `Pylon started. PID ${formatCount(result.pid)}.`
        : `Could not create Pylon: ${result.error}`
    })
    .catch(error => {
      pylonActionStatus.textContent = `Could not create Pylon: ${
        error instanceof Error ? error.message : String(error)
      }`
    })
    .finally(() => {
      isCreatingPylon = false
      createPylonButton.disabled = false
      void loadPylonStatus()
    })
})

applyRoute(routeFromLocation())
void loadCodingStatus()
void loadPylonStatus()
globalThis.setInterval(
  () => void loadCodingStatus(),
  OPENAGENTS_DESKTOP_CODING_POLL_INTERVAL_MS,
)
globalThis.setInterval(
  () => void loadPylonStatus(),
  OPENAGENTS_DESKTOP_PYLON_POLL_INTERVAL_MS,
)
