import { Electroview } from "electrobun/view"

import {
  type CodexAccountStatusResult,
  type DesktopCodexAccount,
  type DesktopCodexAccountReadiness,
  OPENAGENTS_DESKTOP_ACCOUNT_POLL_INTERVAL_MS,
} from "../shared/account-status"
import {
  type CodingCodexSession,
  OPENAGENTS_DESKTOP_CODING_POLL_INTERVAL_MS,
  type CodingStatusResult,
  type CodingSupervisorEvent,
  type CodingTranscriptMessage,
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
const codingAccountsSummary = requireElement<HTMLElement>(
  "#coding-accounts-summary",
)
const codingAccountsRecheck = requireElement<HTMLButtonElement>(
  "#coding-accounts-recheck",
)
const codingAccountsList = requireElement<HTMLElement>("#coding-accounts-list")
const codingAccountsActionStatus = requireElement<HTMLElement>(
  "#coding-accounts-action-status",
)
const codingActiveList = requireElement<HTMLElement>("#coding-active-list")
const codingList = requireElement<HTMLElement>("#coding-list")
const codingTranscriptTitle = requireElement<HTMLElement>(
  "#coding-transcript-title",
)
const codingTranscriptMeta = requireElement<HTMLElement>(
  "#coding-transcript-meta",
)
const codingTranscriptCount = requireElement<HTMLElement>(
  "#coding-transcript-count",
)
const codingTranscriptMessages = requireElement<HTMLElement>(
  "#coding-transcript-messages",
)
const codingEvents = requireElement<HTMLElement>("#coding-events")
const codingDispatchSummary = requireElement<HTMLElement>(
  "#coding-dispatch-summary",
)

let latestCodingResult: CodingStatusResult | null = null
let selectedSessionPath: string | null = null

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

const formatRelativeTimestamp = (value: string | null): string => {
  if (value === null) return "No timestamp"
  const millis = Date.parse(value)
  if (!Number.isFinite(millis)) return value
  const seconds = Math.max(0, Math.round((Date.now() - millis) / 1000))
  if (seconds < 60) return "Just now"
  const minutes = Math.round(seconds / 60)
  if (minutes < 60) return `${formatCount(minutes)}m ago`
  const hours = Math.round(minutes / 60)
  if (hours < 48) return `${formatCount(hours)}h ago`
  return `${formatCount(Math.round(hours / 24))}d ago`
}

const formatPercent = (value: number | null): string =>
  value === null
    ? "-"
    : `${Math.max(0, Math.min(100, value)).toFixed(value % 1 === 0 ? 0 : 1)}%`

const readinessLabel = (value: DesktopCodexAccountReadiness): string => {
  switch (value) {
    case "ready":
      return "ready"
    case "credentials_missing":
      return "credentials missing"
    case "credentials_revoked":
      return "credentials revoked"
    case "disabled_by_config":
      return "disabled"
    case "platform_unsupported":
      return "unsupported"
    case "rate_limited":
      return "rate limited"
    case "sdk_missing":
      return "SDK missing"
    case "usage_limited":
      return "usage limited"
    case "unknown":
      return "unknown"
  }
}

const accountResetLabel = (account: DesktopCodexAccount): string => {
  if (account.resetAt !== null) return `Reset ${formatTimestamp(account.resetAt, null)}`
  const windowReset = account.windows.find(window => window.resetAt !== null)?.resetAt ?? null
  return windowReset === null ? "No reset time" : `Window ${formatTimestamp(windowReset, null)}`
}

const accountUsageLabel = (account: DesktopCodexAccount): string => {
  const windows = account.windows
    .slice(0, 2)
    .map(window => `${window.label} ${formatPercent(window.usedPercent)}`)
  if (windows.length > 0) return windows.join(" · ")
  if (account.totalTokens !== null) return `${formatCount(account.totalTokens)} tokens`
  return "No usage window"
}

let isRecheckingAccounts = false
let resettingAccountRef: string | null = null

const setAccountActionStatus = (value: string): void => {
  codingAccountsActionStatus.textContent = value
}

const codexAccountRow = (account: DesktopCodexAccount): HTMLElement => {
  const row = document.createElement("article")
  row.className = "coding-account-row"
  row.dataset.state =
    account.readiness === "ready"
      ? "ready"
      : account.readiness === "rate_limited" || account.readiness === "usage_limited"
        ? "limited"
        : "blocked"

  const identity = document.createElement("div")
  identity.className = "coding-account-identity"

  const title = document.createElement("strong")
  title.textContent = account.accountRef ?? account.accountRefHash

  const meta = document.createElement("span")
  meta.textContent = [
    accountUsageLabel(account),
    accountResetLabel(account),
    account.manualResetsRemaining === null
      ? null
      : `${formatCount(account.manualResetsRemaining)} manual resets`,
  ]
    .filter((value): value is string => value !== null)
    .join(" · ")

  identity.append(title, meta)

  const readiness = document.createElement("span")
  readiness.className = "coding-account-readiness"
  readiness.textContent = readinessLabel(account.readiness)

  const action = document.createElement("button")
  action.className = "coding-account-action"
  action.type = "button"
  action.textContent = account.resetAvailable ? "Reset" : "Reset unavailable"
  action.disabled = !account.resetAvailable || resettingAccountRef !== null
  action.hidden = !account.resetSupported
  const localAccountRef = account.accountRef
  if (localAccountRef !== null) {
    action.addEventListener("click", () => {
      if (!account.resetAvailable || resettingAccountRef !== null) return
      resettingAccountRef = localAccountRef
      setAccountActionStatus(`Resetting ${localAccountRef}...`)
      void rpc.request
        .codexAccountReset(localAccountRef)
        .then(result => {
          if (result.ok) {
            setAccountActionStatus(`${localAccountRef} reset rechecked.`)
            renderCodexAccountStatus({
              ok: true,
              accounts: result.accounts,
              observedAt: result.observedAt,
              source: "local-pylon",
            })
            return
          }
          setAccountActionStatus(`Could not reset ${localAccountRef}: ${result.error}`)
        })
        .catch(error => {
          setAccountActionStatus(
            `Could not reset ${localAccountRef}: ${
              error instanceof Error ? error.message : String(error)
            }`,
          )
        })
        .finally(() => {
          resettingAccountRef = null
          void loadCodexAccountStatus()
        })
    })
  }

  row.append(identity, readiness, action)
  return row
}

const renderCodexAccountStatus = (result: CodexAccountStatusResult): void => {
  const readyCount = result.accounts.filter(account => account.readiness === "ready").length
  const limitedCount = result.accounts.filter(account =>
    account.readiness === "rate_limited" || account.readiness === "usage_limited",
  ).length
  codingAccountsSummary.textContent = result.ok
    ? `${formatCount(readyCount)} ready · ${formatCount(limitedCount)} limited`
    : result.error
  codingAccountsList.replaceChildren()
  if (result.accounts.length === 0) {
    const empty = document.createElement("div")
    empty.className = "coding-empty"
    empty.textContent = result.ok
      ? result.notice ?? "No Codex accounts reported yet."
      : result.error
    codingAccountsList.append(empty)
    return
  }
  codingAccountsList.append(...result.accounts.map(codexAccountRow))
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

const compactPath = (value: string | null): string | null => {
  if (value === null) return null
  const marker = "/.pylon-fable/cache/codex-agent-tasks/"
  const markerIndex = value.indexOf(marker)
  if (markerIndex !== -1) {
    return value.slice(markerIndex + marker.length)
  }
  const parts = value.split("/").filter(part => part !== "")
  return parts.length <= 3 ? value : `.../${parts.slice(-3).join("/")}`
}

const sessionSubtitle = (session: CodingCodexSession): string => {
  const parts = [
    session.accountRef,
    session.issueRef === null ? null : `#${session.issueRef}`,
    session.pid === null ? null : `PID ${formatCount(session.pid)}`,
    formatRelativeTimestamp(session.modifiedAt),
  ].filter((value): value is string => value !== null)
  return parts.join(" · ")
}

const selectSession = (session: CodingCodexSession): void => {
  selectedSessionPath = session.path
  if (latestCodingResult !== null) renderCodingStatus(latestCodingResult)
}

const sessionButton = (
  session: CodingCodexSession,
  variant: "chip" | "row",
): HTMLButtonElement => {
  const button = document.createElement("button")
  button.className =
    variant === "chip" ? "coding-active-session" : "coding-session-row"
  button.type = "button"
  button.dataset.state = session.status
  button.dataset.selected =
    selectedSessionPath === session.path ? "true" : "false"
  button.addEventListener("click", () => selectSession(session))

  const title = document.createElement("strong")
  title.textContent = session.title

  const meta = document.createElement("span")
  meta.textContent = sessionSubtitle(session)

  const status = document.createElement("span")
  status.className = "coding-session-status"
  status.textContent = session.status

  if (variant === "chip") {
    button.append(title, meta)
  } else {
    const preview = document.createElement("span")
    preview.className = "coding-session-preview"
    preview.textContent =
      session.messages.at(-1)?.text.split("\n")[0]?.trim() ??
      compactPath(session.cwd) ??
      session.path
    button.append(title, meta, preview, status)
  }

  return button
}

const messageRow = (message: CodingTranscriptMessage): HTMLElement => {
  const row = document.createElement("article")
  row.className = "coding-message"
  row.dataset.role = message.role

  const heading = document.createElement("div")
  heading.className = "coding-message-heading"

  const role = document.createElement("strong")
  role.textContent = message.role

  const meta = document.createElement("span")
  meta.textContent = [
    message.kind,
    formatRelativeTimestamp(message.timestamp),
  ].join(" · ")

  const body = document.createElement("p")
  body.textContent = message.text

  heading.append(role, meta)
  row.append(heading, body)
  return row
}

const renderCodingTranscript = (
  session: CodingCodexSession | null,
): void => {
  codingTranscriptMessages.replaceChildren()
  if (session === null) {
    codingTranscriptTitle.textContent = "No session selected"
    codingTranscriptMeta.textContent = "Click a Codex instance"
    codingTranscriptCount.textContent = "0 messages"
    const empty = document.createElement("div")
    empty.className = "coding-empty"
    empty.textContent = "No Codex session transcript loaded."
    codingTranscriptMessages.append(empty)
    return
  }

  codingTranscriptTitle.textContent = session.title
  codingTranscriptMeta.textContent = [
    sessionSubtitle(session),
    compactPath(session.cwd),
  ]
    .filter((value): value is string => value !== null && value !== "")
    .join(" · ")
  codingTranscriptCount.textContent = `${formatCount(session.messageCount)} messages`

  if (session.messages.length === 0) {
    const empty = document.createElement("div")
    empty.className = "coding-empty"
    empty.textContent = "No transcript messages yet."
    codingTranscriptMessages.append(empty)
    return
  }

  codingTranscriptMessages.append(...session.messages.map(messageRow))
}

const renderCodingSessions = (
  sessions: readonly CodingCodexSession[],
): void => {
  if (
    sessions.length > 0 &&
    (selectedSessionPath === null ||
      !sessions.some(session => session.path === selectedSessionPath))
  ) {
    selectedSessionPath = sessions[0].path
  }

  const selectedSession =
    sessions.find(session => session.path === selectedSessionPath) ?? null

  codingActiveList.replaceChildren()
  const activeSessions = sessions.filter(session => session.active).slice(0, 8)
  if (activeSessions.length === 0) {
    const empty = document.createElement("div")
    empty.className = "coding-empty coding-active-empty"
    empty.textContent = "No active Codex sessions."
    codingActiveList.append(empty)
  } else {
    codingActiveList.append(
      ...activeSessions.map(session => sessionButton(session, "chip")),
    )
  }

  codingList.replaceChildren()
  if (sessions.length === 0) {
    const empty = document.createElement("div")
    empty.className = "coding-empty"
    empty.textContent = "No Codex session rollouts found."
    codingList.append(empty)
  } else {
    codingList.append(...sessions.map(session => sessionButton(session, "row")))
  }

  renderCodingTranscript(selectedSession)
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
  latestCodingResult = result
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

  renderCodingSessions(result.sessions)

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
      sessions: [],
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
