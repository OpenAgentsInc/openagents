import { Electroview } from "electrobun/view"

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
const pylonStatus = requireElement<HTMLButtonElement>("#pylon-status")
const pylonCount = requireElement<HTMLElement>("#pylon-count")
const pylonsPage = requireElement<HTMLElement>("#pylons-page")
const pylonsBack = requireElement<HTMLButtonElement>("#pylons-back")
const pylonsSummary = requireElement<HTMLElement>("#pylons-summary")
const pylonsList = requireElement<HTMLElement>("#pylons-list")
const createPylonButton = requireElement<HTMLButtonElement>("#create-pylon")
const pylonActionStatus = requireElement<HTMLElement>("#pylon-action-status")

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

type DesktopRoute = "landing" | "pylons"

const routeFromLocation = (): DesktopRoute =>
  globalThis.location.hash.replace(/^#\/?/, "") === "pylons"
    ? "pylons"
    : "landing"

const applyRoute = (route: DesktopRoute): void => {
  shell.dataset.route = route
  pylonsPage.hidden = route !== "pylons"
  pylonStatus.setAttribute("aria-expanded", route === "pylons" ? "true" : "false")
  handle.setPose(route === "pylons" ? "pylons" : "landing")
}

const navigateTo = (route: DesktopRoute): void => {
  if (route === "pylons") {
    if (globalThis.location.hash !== "#pylons") {
      globalThis.location.hash = "pylons"
    }
    applyRoute("pylons")
    return
  }

  if (globalThis.location.hash !== "") {
    globalThis.history.pushState({}, "", `${globalThis.location.pathname}${globalThis.location.search}`)
  }
  applyRoute("landing")
}

pylonStatus.addEventListener("click", () => navigateTo("pylons"))
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
void loadPylonStatus()
globalThis.setInterval(
  () => void loadPylonStatus(),
  OPENAGENTS_DESKTOP_PYLON_POLL_INTERVAL_MS,
)
