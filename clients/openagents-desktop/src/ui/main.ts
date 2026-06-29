import { Electroview } from "electrobun/view"

import {
  apmSummaryText,
  calculateApmStats,
  formatApm,
  type ApmStats,
} from "../shared/apm"
import {
  type CodingCodexSession,
  emptyManagerResumeSnapshot,
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
import {
  type AssignmentTokenUsageVerification,
  type TokenAccountingStatusResult,
} from "../shared/token-accounting"
import { mountLandingSquares } from "./landing-squares"
import { renderMessageBody } from "./transcript-render"
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
const codingApm = requireElement<HTMLButtonElement>("#coding-apm")
const codingObserved = requireElement<HTMLElement>("#coding-observed")
const codingSummary = requireElement<HTMLElement>("#coding-summary")
const codingMetricCodex = requireElement<HTMLElement>("#coding-metric-codex")
const codingMetricBurning = requireElement<HTMLElement>("#coding-metric-burning")
const codingMetricKhala = requireElement<HTMLElement>("#coding-metric-khala")
const codingMetricReady = requireElement<HTMLElement>("#coding-metric-ready")
const codingMetricTokenFailures = requireElement<HTMLElement>(
  "#coding-metric-token-failures",
)
const codingManagerSummary = requireElement<HTMLElement>(
  "#coding-manager-summary",
)
const codingManagerStatus = requireElement<HTMLElement>(
  "#coding-manager-status",
)
const codingManagerWarnings = requireElement<HTMLElement>(
  "#coding-manager-warnings",
)
const codingCopyStatus = requireElement<HTMLButtonElement>("#coding-copy-status")
const codingCopyJson = requireElement<HTMLButtonElement>("#coding-copy-json")
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
const codingTokenVerification = requireElement<HTMLElement>(
  "#coding-token-verification",
)
const codingEvents = requireElement<HTMLElement>("#coding-events")
const codingDispatchSummary = requireElement<HTMLElement>(
  "#coding-dispatch-summary",
)
const tokenAccountingSummary = requireElement<HTMLElement>(
  "#token-accounting-summary",
)
const tokenReplay = requireElement<HTMLButtonElement>("#token-replay")
const tokenSpoolList = requireElement<HTMLElement>("#token-spool-list")
const tokenReplayStatus = requireElement<HTMLElement>("#token-replay-status")
const apmPage = requireElement<HTMLElement>("#apm-page")
const apmBack = requireElement<HTMLButtonElement>("#apm-back")
const apmObserved = requireElement<HTMLElement>("#apm-observed")
const apmCurrent = requireElement<HTMLElement>("#apm-current")
const apmCurrentDetail = requireElement<HTMLElement>("#apm-current-detail")
const apmChart = requireElement<HTMLElement>("#apm-chart")
const apmRecent = requireElement<HTMLElement>("#apm-recent")
const apmPeak = requireElement<HTMLElement>("#apm-peak")
const apmActions = requireElement<HTMLElement>("#apm-actions")
const apmActionsDetail = requireElement<HTMLElement>("#apm-actions-detail")
const apmSummary = requireElement<HTMLElement>("#apm-summary")
const apmSessionList = requireElement<HTMLElement>("#apm-session-list")

let latestCodingResult: CodingStatusResult | null = null
let latestTokenAccounting: TokenAccountingStatusResult | null = null
let selectedSessionPath: string | null = null
const tokenVerificationCache = new Map<string, AssignmentTokenUsageVerification>()
const tokenVerificationInFlight = new Set<string>()

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

const formatDecimal = (value: number): string =>
  new Intl.NumberFormat("en-US", {
    maximumFractionDigits: 1,
    minimumFractionDigits: 1,
  }).format(Number.isFinite(value) ? Math.max(0, value) : 0)

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

const formatAbsoluteTimestamp = (value: string | null): string => {
  if (value === null) return "No timestamp"
  const date = new Date(value)
  if (!Number.isFinite(date.getTime())) return value
  return new Intl.DateTimeFormat("en-US", {
    month: "short",
    day: "numeric",
    hour: "numeric",
    minute: "2-digit",
    second: "2-digit",
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
    session.assignmentRef === null ? null : session.assignmentRef.slice(-12),
    session.pid === null ? null : `PID ${formatCount(session.pid)}`,
    session.elapsed === null ? null : `elapsed ${session.elapsed}`,
    formatRelativeTimestamp(session.modifiedAt),
  ].filter((value): value is string => value !== null)
  return parts.join(" · ")
}

type SessionLifecycleState =
  | "accepted"
  | "active"
  | "blocked"
  | "counted"
  | "idle"
  | "recent"
  | "rejected"
  | "verifying"

const verificationForSession = (
  session: CodingCodexSession,
): AssignmentTokenUsageVerification | null =>
  session.assignmentRef === null
    ? null
    : tokenVerificationCache.get(session.assignmentRef) ?? null

const sessionLifecycleState = (
  session: CodingCodexSession,
): SessionLifecycleState => {
  if (session.active || session.status === "active") return "active"
  const verification = verificationForSession(session)
  if (verification?.ok === true) return "counted"
  if (verification?.ok === false) return "blocked"
  if (
    session.assignmentRef !== null &&
    tokenVerificationInFlight.has(session.assignmentRef)
  ) {
    return "verifying"
  }
  const closeoutStatus = session.closeout.status?.toLowerCase()
  if (closeoutStatus === "accepted") return "accepted"
  if (closeoutStatus === "rejected") return "rejected"
  if (session.status === "recent") return "recent"
  return "idle"
}

const sessionLifecycleLabel = (session: CodingCodexSession): string => {
  const state = sessionLifecycleState(session)
  switch (state) {
    case "accepted":
      return "Accepted"
    case "active":
      return "Active"
    case "blocked":
      return "Proof blocked"
    case "counted":
      return "Counted"
    case "rejected":
      return "Rejected"
    case "verifying":
      return "Verifying"
    case "recent":
      return "Recent"
    case "idle":
      return "Idle"
  }
}

const sessionStatusRank = (session: CodingCodexSession): number => {
  const state = sessionLifecycleState(session)
  if (state === "active") return 0
  if (session.status === "recent") return 1
  if (state === "counted" || state === "accepted" || state === "rejected") {
    return 2
  }
  return 3
}

const sessionModifiedAtMs = (session: CodingCodexSession): number => {
  const millis = Date.parse(session.modifiedAt)
  return Number.isFinite(millis) ? millis : 0
}

const visibleCodingSessions = (
  sessions: readonly CodingCodexSession[],
): readonly CodingCodexSession[] =>
  [...sessions].sort((left, right) => {
    const statusDelta = sessionStatusRank(left) - sessionStatusRank(right)
    if (statusDelta !== 0) return statusDelta
    return sessionModifiedAtMs(right) - sessionModifiedAtMs(left)
  })

const sessionOutcome = (session: CodingCodexSession): string | null => {
  const parts = [
    session.pullRequestRef === null ? null : `PR #${session.pullRequestRef}`,
    session.issueRef === null ? null : `issue #${session.issueRef}`,
  ]
  const verification = verificationForSession(session)
  if (verification !== null) {
    parts.push(
      verification.ok
        ? `${formatCount(verification.totalTokens)} tokens counted`
        : `token proof blocked: ${verification.blockerRef}`,
    )
  } else if (
    session.assignmentRef !== null &&
    tokenVerificationInFlight.has(session.assignmentRef)
  ) {
    parts.push("token proof verifying")
  } else if (session.closeout.status !== null) {
    parts.push(`closeout ${session.closeout.status}`)
  }
  if (session.lastEvent.name !== null) parts.push(session.lastEvent.name)
  const text = parts.filter((value): value is string => value !== null).join(" · ")
  return text === "" ? null : text
}

const sessionPreview = (session: CodingCodexSession): string =>
  session.messages.at(-1)?.text.split("\n")[0]?.trim() ??
  compactPath(session.cwd) ??
  session.path

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
  button.dataset.state = sessionLifecycleState(session)
  button.dataset.selected =
    selectedSessionPath === session.path ? "true" : "false"
  button.addEventListener("click", () => selectSession(session))

  const title = document.createElement("strong")
  title.textContent = session.title

  const meta = document.createElement("span")
  meta.textContent = sessionSubtitle(session)

  const status = document.createElement("span")
  status.className = "coding-session-status"
  status.textContent = sessionLifecycleLabel(session).toUpperCase()

  const outcomeText = sessionOutcome(session)
  const outcome =
    outcomeText === null ? null : document.createElement("span")
  if (outcome !== null) {
    outcome.className = "coding-session-outcome"
    outcome.textContent = outcomeText
  }

  if (variant === "chip") {
    if (outcome === null) {
      button.append(title, meta, status)
    } else {
      button.append(title, meta, outcome, status)
    }
  } else {
    const preview = document.createElement("span")
    preview.className = "coding-session-preview"
    preview.textContent = sessionPreview(session)
    if (outcome === null) {
      button.append(title, meta, preview, status)
    } else {
      button.append(title, meta, outcome, preview, status)
    }
  }

  return button
}

const messageRow = (message: CodingTranscriptMessage): HTMLElement => {
  const row = document.createElement("article")
  row.className = "coding-message"
  row.dataset.role = message.role
  row.dataset.status = message.status

  const heading = document.createElement("div")
  heading.className = "coding-message-heading"

  const marker = document.createElement("span")
  marker.className = "coding-message-marker"
  marker.setAttribute("aria-hidden", "true")

  const label = document.createElement("div")
  label.className = "coding-message-label"

  const role = document.createElement("strong")
  role.textContent = message.title

  const kind = document.createElement("span")
  kind.textContent = message.kind

  label.append(role, kind)

  const chips = document.createElement("div")
  chips.className = "coding-message-chips"

  if (message.detail !== null) {
    const detail = document.createElement("span")
    detail.className = "coding-message-chip"
    detail.textContent = message.detail
    chips.append(detail)
  }

  if (message.status !== "info") {
    const status = document.createElement("span")
    status.className = "coding-message-chip"
    status.dataset.status = message.status
    status.textContent = message.status
    chips.append(status)
  }

  const time = document.createElement("time")
  time.className = "coding-message-time"
  if (message.timestamp !== null) time.dateTime = message.timestamp
  time.textContent = formatAbsoluteTimestamp(message.timestamp)

  const meta = document.createElement("span")
  meta.className = "coding-message-relative-time"
  meta.textContent = formatRelativeTimestamp(message.timestamp)

  const body = document.createElement("div")
  body.className = "coding-message-body"
  body.append(
    ...renderMessageBody({
      text: message.text,
      role: message.role,
      status: message.status,
    }),
  )

  const metaGroup = document.createElement("div")
  metaGroup.className = "coding-message-meta"
  metaGroup.append(chips, time, meta)

  heading.append(marker, label, metaGroup)
  row.append(heading, body)
  return row
}

const detailChip = (label: string, value: string | null): HTMLElement => {
  const chip = document.createElement("span")
  chip.className = "coding-detail-chip"
  const name = document.createElement("strong")
  name.textContent = label
  const body = document.createElement("code")
  body.textContent = value === null || value.trim() === "" ? "unknown" : value
  chip.append(name, body)
  return chip
}

const sessionDetailChips = (session: CodingCodexSession): readonly HTMLElement[] => {
  const lastEvent =
    session.lastEvent.name === null
      ? null
      : `${session.lastEvent.name}${
          session.lastEvent.ageSeconds === null
            ? ""
            : ` · ${formatCount(session.lastEvent.ageSeconds)}s ago`
        }`
  const closeout =
    session.closeout.closeoutRef ??
    session.closeout.status ??
    (session.closeout.blockerRefs.length === 0
      ? null
      : session.closeout.blockerRefs.join(", "))
  const pullIssue = [
    session.pullRequestRef === null ? null : `PR #${session.pullRequestRef}`,
    session.issueRef === null ? null : `issue #${session.issueRef}`,
  ].filter((value): value is string => value !== null).join(" · ")
  const workspace =
    session.cwd ??
    (session.assignmentRef === null
      ? session.path
      : `pylon khala status --assignment-ref ${session.assignmentRef} --json`)
  return [
    detailChip("assignment", session.assignmentRef),
    detailChip("account", session.accountRef),
    detailChip("PR/issue", pullIssue === "" ? null : pullIssue),
    detailChip("workspace", workspace),
    detailChip("PID", session.pid === null ? null : String(session.pid)),
    detailChip("elapsed", session.elapsed),
    detailChip("last event", lastEvent),
    detailChip("closeout", closeout),
  ]
}

const renderCodingTranscript = (
  session: CodingCodexSession | null,
): void => {
  codingTranscriptMessages.replaceChildren()
  renderAssignmentTokenVerification(session)
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

  const details = document.createElement("div")
  details.className = "coding-session-details"
  details.append(...sessionDetailChips(session))
  codingTranscriptMessages.append(details)

  if (session.messages.length === 0) {
    const empty = document.createElement("div")
    empty.className = "coding-empty"
    empty.textContent = "No transcript messages yet."
    codingTranscriptMessages.append(empty)
    return
  }

  codingTranscriptMessages.append(...session.messages.map(messageRow))
}

const tokenUsageText = (verification: AssignmentTokenUsageVerification): string => {
  if (!verification.ok) return verification.blockerRef
  return `${formatCount(verification.totalTokens)} tokens · ${formatCount(verification.rowCount)} rows · exact`
}

const queueAssignmentTokenVerification = (assignmentRef: string): void => {
  if (
    tokenVerificationCache.has(assignmentRef) ||
    tokenVerificationInFlight.has(assignmentRef)
  ) {
    return
  }

  tokenVerificationInFlight.add(assignmentRef)
  void rpc.request
    .verifyAssignmentTokenUsage(assignmentRef)
    .then(result => {
      tokenVerificationCache.set(assignmentRef, result)
    })
    .catch(error => {
      tokenVerificationCache.set(assignmentRef, {
        ok: false,
        assignmentRef,
        blockerRef: "blocker.desktop_token_accounting.proof_unavailable",
        error: error instanceof Error ? error.message : String(error),
        observedAt: new Date().toISOString(),
      })
    })
    .finally(() => {
      tokenVerificationInFlight.delete(assignmentRef)
      if (latestCodingResult !== null) renderCodingStatus(latestCodingResult)
    })
}

const renderAssignmentTokenVerification = (
  session: CodingCodexSession | null,
): void => {
  codingTokenVerification.replaceChildren()
  if (session === null || session.assignmentRef === null) {
    const empty = document.createElement("div")
    empty.className = "token-verification-empty"
    empty.textContent = "No assignment ref detected for this session."
    codingTokenVerification.append(empty)
    return
  }

  const assignmentRef = session.assignmentRef
  const row = document.createElement("article")
  row.className = "token-verification-row"
  row.dataset.state = "pending"

  const heading = document.createElement("div")
  heading.className = "token-verification-heading"

  const title = document.createElement("strong")
  title.textContent = assignmentRef

  const status = document.createElement("span")
  const cached = tokenVerificationCache.get(assignmentRef)
  if (cached === undefined) {
    status.textContent = tokenVerificationInFlight.has(assignmentRef)
      ? "Verifying"
      : "Queued"
  } else {
    row.dataset.state = cached.ok ? "ok" : "blocked"
    status.textContent = cached.ok ? "Verified" : "Blocked"
  }

  heading.append(title, status)

  const body = document.createElement("p")
  body.textContent =
    cached === undefined
      ? "Checking assignment token usage event proof."
      : tokenUsageText(cached)

  row.append(heading, body)
  codingTokenVerification.append(row)

  if (cached === undefined) queueAssignmentTokenVerification(assignmentRef)
}

const renderTokenAccounting = (result: TokenAccountingStatusResult): void => {
  latestTokenAccounting = result
  const spool = result.spool
  codingMetricTokenFailures.textContent = formatCount(spool.lineCount)
  tokenReplay.disabled = spool.lineCount === 0
  tokenAccountingSummary.textContent =
    spool.lineCount === 0
      ? "No failure spool"
      : `${formatCount(spool.lineCount)} reports · ${formatCount(spool.byteLength)} bytes`

  tokenSpoolList.replaceChildren()
  if (spool.lineCount === 0) {
    const empty = document.createElement("div")
    empty.className = "token-spool-empty"
    empty.textContent = "No unposted Codex turn reports."
    tokenSpoolList.append(empty)
    return
  }

  tokenSpoolList.append(
    ...spool.reports.slice(0, 6).map(report => {
      const row = document.createElement("article")
      row.className = "token-spool-row"

      const title = document.createElement("strong")
      title.textContent = report.assignmentRef

      const meta = document.createElement("span")
      meta.textContent = [
        report.turnIndex === null ? null : `turn ${formatCount(report.turnIndex)}`,
        report.totalTokens > 0 ? `${formatCount(report.totalTokens)} tokens` : null,
        formatRelativeTimestamp(report.observedAt),
      ].filter((value): value is string => value !== null).join(" · ")

      const error = document.createElement("p")
      error.textContent = report.error

      row.append(title, meta, error)
      return row
    }),
  )
}

const renderCodingSessions = (
  sessions: readonly CodingCodexSession[],
): void => {
  const visibleSessions = visibleCodingSessions(sessions)
  const activeSessions = visibleSessions.filter(
    session => session.active || session.status === "active",
  )

  for (const session of visibleSessions.slice(0, 8)) {
    if (
      session.assignmentRef !== null &&
      (session.status === "recent" || session.closeout.status !== null)
    ) {
      queueAssignmentTokenVerification(session.assignmentRef)
    }
  }

  if (
    visibleSessions.length > 0 &&
    (selectedSessionPath === null ||
      !visibleSessions.some(session => session.path === selectedSessionPath))
  ) {
    selectedSessionPath = visibleSessions[0].path
  }

  const selectedSession =
    visibleSessions.find(session => session.path === selectedSessionPath) ?? null

  codingActiveList.replaceChildren()
  const topSessions = activeSessions.slice(0, 8)
  if (topSessions.length === 0) {
    const empty = document.createElement("div")
    empty.className = "coding-empty coding-active-empty"
    empty.textContent =
      visibleSessions.length === 0
        ? "No Codex session transcripts found."
        : "No live Codex exec processes. Recent and completed sessions are listed below."
    codingActiveList.append(empty)
  } else {
    codingActiveList.append(
      ...topSessions.map(session => sessionButton(session, "chip")),
    )
  }

  codingList.replaceChildren()
  if (visibleSessions.length === 0) {
    const empty = document.createElement("div")
    empty.className = "coding-empty"
    empty.textContent = "No Codex session rollouts found."
    codingList.append(empty)
  } else {
    codingList.append(
      ...visibleSessions.map(session => sessionButton(session, "row")),
    )
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

const copyText = async (value: string, button: HTMLButtonElement): Promise<void> => {
  const original = button.textContent ?? "Copy"
  try {
    await globalThis.navigator.clipboard.writeText(value)
    button.textContent = "Copied"
  } catch {
    button.textContent = "Copy Failed"
  } finally {
    globalThis.setTimeout(() => {
      button.textContent = original
    }, 1_200)
  }
}

const renderCodingStatus = (result: CodingStatusResult): void => {
  latestCodingResult = result
  const summary = result.summary
  const manager = result.managerResume
  codingCount.textContent = `Live Codex: ${formatCount(summary.codexExecCount)}`
  const statusState =
    summary.codexExecCount > 0
      ? summary.burningCodexCount > 0
        ? "online"
        : "empty"
      : summary.assignmentRunnerCount > 0
        ? "empty"
        : "unknown"
  codingStatus.dataset.state = statusState
  codingStatus.title = result.ok
    ? `Open Coding. ${formatCount(summary.codexExecCount)} live Codex exec processes, ${formatCount(summary.assignmentRunnerCount)} assignment runners.`
    : result.error

  codingObserved.textContent = formatTimestamp(result.observedAt, "Local now")
  codingSummary.textContent = `Live Codex: ${formatCount(summary.codexExecCount)}`
  codingMetricCodex.textContent = formatCount(summary.codexExecCount)
  codingMetricBurning.textContent = formatCount(summary.burningCodexCount)
  codingMetricKhala.textContent = formatCount(summary.assignmentRunnerCount)
  codingMetricReady.textContent =
    summary.readyCodex === null ? "-" : formatCount(summary.readyCodex)
  if (latestTokenAccounting === null) {
    codingMetricTokenFailures.textContent = "0"
  }

  codingManagerSummary.textContent = [
    `Assigned ${formatCount(manager.activeAssignments.length)}`,
    `Runners ${formatCount(summary.assignmentRunnerCount)}`,
    `Requests ${formatCount(summary.khalaRequestCount)}`,
    `Locks ${formatCount(manager.queueLocks.filter(lock => lock.type === "lock").length)}`,
    `Token failures ${formatCount(manager.tokenFailures.failureCount)}`,
    `Warnings ${formatCount(manager.warnings.length)}`,
  ].join(" · ")
  codingManagerStatus.textContent = manager.statusBlock
  codingManagerWarnings.replaceChildren()
  if (manager.warnings.length === 0) {
    const empty = document.createElement("div")
    empty.className = "coding-empty coding-manager-empty"
    empty.textContent = "No mismatch warnings."
    codingManagerWarnings.append(empty)
  } else {
    codingManagerWarnings.append(
      ...manager.warnings.slice(0, 6).map(warning => {
        const row = document.createElement("article")
        row.className = "coding-manager-warning"
        row.dataset.severity = warning.severity
        const code = document.createElement("strong")
        code.textContent = warning.code
        const message = document.createElement("span")
        message.textContent = warning.message
        row.append(code, message)
        return row
      }),
    )
  }

  renderCodingSessions(result.sessions)
  renderApm(result)

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

const chartPath = (
  points: readonly { readonly x: number; readonly y: number }[],
): string =>
  points
    .map((point, index) =>
      `${index === 0 ? "M" : "L"} ${formatDecimal(point.x)} ${formatDecimal(point.y)}`,
    )
    .join(" ")

const renderApmChart = (stats: ApmStats): void => {
  apmChart.replaceChildren()
  apmChart.setAttribute("aria-label", apmSummaryText(stats))

  if (stats.series.every(point => point.actionCount === 0)) {
    const empty = document.createElement("div")
    empty.className = "apm-chart-empty"
    empty.textContent = "No timestamped Coding actions in the last hour."
    apmChart.append(empty)
    return
  }

  const width = 720
  const height = 190
  const padding = 18
  const maxApm = Math.max(1, stats.peakApm)
  const points = stats.series.map((point, index) => ({
    x:
      padding +
      (index / Math.max(1, stats.series.length - 1)) * (width - padding * 2),
    y:
      height -
      padding -
      (point.apm / maxApm) * (height - padding * 2),
  }))
  const line = chartPath(points)
  const area = `${line} L ${width - padding} ${height - padding} L ${padding} ${height - padding} Z`

  const svg = document.createElementNS("http://www.w3.org/2000/svg", "svg")
  svg.setAttribute("viewBox", `0 0 ${width} ${height}`)
  svg.setAttribute("role", "presentation")
  svg.classList.add("apm-chart-svg")

  const areaPath = document.createElementNS("http://www.w3.org/2000/svg", "path")
  areaPath.setAttribute("d", area)
  areaPath.setAttribute("class", "apm-chart-area")

  const linePath = document.createElementNS("http://www.w3.org/2000/svg", "path")
  linePath.setAttribute("d", line)
  linePath.setAttribute("class", "apm-chart-line")

  const axis = document.createElementNS("http://www.w3.org/2000/svg", "line")
  axis.setAttribute("x1", String(padding))
  axis.setAttribute("x2", String(width - padding))
  axis.setAttribute("y1", String(height - padding))
  axis.setAttribute("y2", String(height - padding))
  axis.setAttribute("class", "apm-chart-axis")

  svg.append(areaPath, linePath, axis)
  points.forEach((point, index) => {
    const source = stats.series[index]
    if (source.actionCount === 0) return
    const dot = document.createElementNS("http://www.w3.org/2000/svg", "circle")
    dot.setAttribute("cx", formatDecimal(point.x))
    dot.setAttribute("cy", formatDecimal(point.y))
    dot.setAttribute("r", "3.5")
    dot.setAttribute("class", "apm-chart-dot")
    const title = document.createElementNS("http://www.w3.org/2000/svg", "title")
    title.textContent = `${source.label}: ${formatApm(source.apm)} APM, ${formatCount(source.actionCount)} actions`
    dot.append(title)
    svg.append(dot)
  })

  const fallback = document.createElement("p")
  fallback.className = "apm-chart-fallback"
  fallback.textContent = stats.series
    .filter(point => point.actionCount > 0)
    .map(point => `${point.label} ${formatApm(point.apm)} APM`)
    .join(" · ")

  apmChart.append(svg, fallback)
}

const renderApmSessions = (sessions: readonly CodingCodexSession[]): void => {
  apmSessionList.replaceChildren()
  const rows = visibleCodingSessions(sessions).slice(0, 8)
  if (rows.length === 0) {
    const empty = document.createElement("div")
    empty.className = "apm-session-empty"
    empty.textContent = "No Coding sessions loaded."
    apmSessionList.append(empty)
    return
  }

  apmSessionList.append(
    ...rows.map(session => {
      const stats = calculateApmStats([session])
      const row = document.createElement("article")
      row.className = "apm-session-row"
      row.dataset.state = session.status

      const identity = document.createElement("div")
      identity.className = "apm-session-identity"

      const title = document.createElement("strong")
      title.textContent = session.title

      const meta = document.createElement("span")
      meta.textContent = sessionSubtitle(session)

      identity.append(title, meta)

      const value = document.createElement("div")
      value.className = "apm-session-value"
      const apm = document.createElement("strong")
      apm.textContent = formatApm(stats.currentApm)
      const label = document.createElement("span")
      label.textContent = `${formatCount(stats.actionCount)} actions`
      value.append(apm, label)

      row.append(identity, value)
      return row
    }),
  )
}

const renderApm = (result: CodingStatusResult): void => {
  const stats = calculateApmStats(result.sessions)
  apmObserved.textContent = formatTimestamp(result.observedAt, "Local now")
  apmCurrent.textContent = formatApm(stats.currentApm)
  apmCurrentDetail.textContent =
    stats.actionCount === 0
      ? "Waiting for timestamped Coding session actions."
      : `${formatCount(stats.actionCount)} actions from ${formatCount(stats.sessionCount)} sessions since ${
          stats.firstActionAt === null
            ? "the first loaded action"
            : formatAbsoluteTimestamp(stats.firstActionAt)
        }.`
  apmRecent.textContent = formatApm(stats.recentApm)
  apmPeak.textContent = formatApm(stats.peakApm)
  apmActions.textContent = formatCount(stats.actionCount)
  apmActionsDetail.textContent =
    stats.lastActionAt === null
      ? "No loaded Coding actions yet."
      : `Last action ${formatRelativeTimestamp(stats.lastActionAt)}. Active sessions ${formatCount(stats.activeSessionCount)}.`
  apmSummary.textContent = apmSummaryText(stats)
  renderApmChart(stats)
  renderApmSessions(result.sessions)
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
      managerResume: emptyManagerResumeSnapshot(new Date().toISOString()),
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

const loadTokenAccounting = async (): Promise<void> => {
  try {
    renderTokenAccounting(await rpc.request.tokenAccountingStatus())
  } catch (error) {
    tokenAccountingSummary.textContent = "Token accounting unavailable"
    tokenReplayStatus.textContent =
      error instanceof Error ? error.message : String(error)
  }
}

type DesktopRoute = "apm" | "coding" | "landing" | "pylons"

const routeFromLocation = (): DesktopRoute => {
  const route = globalThis.location.hash.replace(/^#\/?/, "")
  return route === "pylons" || route === "coding" || route === "apm"
    ? route
    : "landing"
}

const applyRoute = (route: DesktopRoute): void => {
  shell.dataset.route = route
  pylonsPage.hidden = route !== "pylons"
  codingPage.hidden = route !== "coding"
  apmPage.hidden = route !== "apm"
  codingStatus.setAttribute(
    "aria-expanded",
    route === "coding" || route === "apm" ? "true" : "false",
  )
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
codingApm.addEventListener("click", () => navigateTo("apm"))
apmBack.addEventListener("click", () => navigateTo("coding"))
pylonsBack.addEventListener("click", () => navigateTo("landing"))
codingCopyStatus.addEventListener("click", () => {
  if (latestCodingResult === null) return
  void copyText(latestCodingResult.managerResume.statusBlock, codingCopyStatus)
})
codingCopyJson.addEventListener("click", () => {
  if (latestCodingResult === null) return
  void copyText(
    JSON.stringify(latestCodingResult.managerResume, null, 2),
    codingCopyJson,
  )
})
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

let isReplayingTokenFailures = false
tokenReplay.addEventListener("click", () => {
  if (isReplayingTokenFailures) return

  isReplayingTokenFailures = true
  tokenReplay.disabled = true
  tokenReplayStatus.textContent = "Replaying unposted turn reports..."

  void rpc.request
    .replayTokenFailures()
    .then(result => {
      tokenReplayStatus.textContent = result.ok
        ? result.archivedPath === null
          ? "No reports needed replay."
          : `Replayed ${formatCount(result.replayedReports)} reports and archived the spool.`
        : `Replay stopped after ${formatCount(result.replayedReports)} reports: ${result.error}`
      renderTokenAccounting({
        ok: true,
        observedAt: result.observedAt,
        spool: result.spool,
      })
      tokenVerificationCache.clear()
      if (latestCodingResult !== null) renderCodingStatus(latestCodingResult)
    })
    .catch(error => {
      tokenReplayStatus.textContent = `Replay failed: ${
        error instanceof Error ? error.message : String(error)
      }`
    })
    .finally(() => {
      isReplayingTokenFailures = false
      tokenReplay.disabled =
        latestTokenAccounting === null ||
        latestTokenAccounting.spool.lineCount === 0
    })
})

applyRoute(routeFromLocation())
void loadCodingStatus()
void loadPylonStatus()
void loadTokenAccounting()
globalThis.setInterval(
  () => void loadCodingStatus(),
  OPENAGENTS_DESKTOP_CODING_POLL_INTERVAL_MS,
)
globalThis.setInterval(
  () => void loadPylonStatus(),
  OPENAGENTS_DESKTOP_PYLON_POLL_INTERVAL_MS,
)
globalThis.setInterval(
  () => void loadTokenAccounting(),
  OPENAGENTS_DESKTOP_CODING_POLL_INTERVAL_MS,
)
