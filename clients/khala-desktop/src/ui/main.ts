import { Electroview } from "electrobun/view"

import type { AppleFmSidecarPublicStatus } from "../bun/apple-fm-sidecar.js"
import {
  KHALA_OPERATOR_POLL_INTERVAL_MS,
  type DashboardAccount,
  type DashboardPylon,
  type DashboardSession,
  type KhalaDesktopDashboard,
} from "../shared/operator-dashboard.js"
import {
  KHALA_DESKTOP_RPC_MAX_REQUEST_TIME_MS,
  type KhalaDesktopRPCSchema,
} from "../shared/rpc.js"
import "./styles.css"

const rpc = Electroview.defineRPC<KhalaDesktopRPCSchema>({
  maxRequestTime: KHALA_DESKTOP_RPC_MAX_REQUEST_TIME_MS,
  handlers: {
    requests: {},
    messages: {},
  },
})

new Electroview({ rpc })

type ViewState =
  | { readonly status: "loading" }
  | {
      readonly status: "ready"
      readonly appleFmSidecar: AppleFmSidecarPublicStatus | null
      readonly dashboard: KhalaDesktopDashboard
    }
  | { readonly status: "error"; readonly error: string; readonly observedAt: string }

const app = document.querySelector<HTMLElement>("#app")
if (app === null) {
  throw new Error("Missing #app mount")
}

const formatNumber = (value: number): string =>
  new Intl.NumberFormat("en-US").format(Math.max(0, Math.trunc(value)))

const formatTime = (value: string | null): string => {
  if (value === null || value.trim() === "") return "not seen"
  const date = new Date(value)
  return Number.isFinite(date.getTime())
    ? date.toLocaleString([], { dateStyle: "medium", timeStyle: "short" })
    : value
}

const formatElapsed = (elapsedMs: number): string => {
  const minutes = Math.max(0, Math.round(elapsedMs / 60_000))
  if (minutes < 60) return `${minutes}m`
  return `${Math.floor(minutes / 60)}h ${minutes % 60}m`
}

const readinessLabel = (readiness: DashboardAccount["readiness"]): string =>
  readiness.replace(/_/g, " ")

const escapeHtml = (value: string): string =>
  value.replace(/[&<>"']/g, character => {
    switch (character) {
      case "&":
        return "&amp;"
      case "<":
        return "&lt;"
      case ">":
        return "&gt;"
      case "\"":
        return "&quot;"
      default:
        return "&#39;"
    }
  })

const pylonRow = (pylon: DashboardPylon): string => `
  <tr>
    <td><code>${escapeHtml(pylon.pylonRef)}</code></td>
    <td><span class="pill ${pylon.heartbeatFresh ? "pill-ready" : "pill-warn"}">${pylon.heartbeatFresh ? "online" : "stale"}</span></td>
    <td>${escapeHtml(pylon.status)}</td>
    <td>${formatTime(pylon.latestHeartbeatAt)}</td>
    <td class="num">${formatNumber(pylon.readySlots)}</td>
    <td class="num">${formatNumber(pylon.busySlots)}</td>
    <td class="num">${formatNumber(pylon.queuedSlots)}</td>
  </tr>`

const accountRow = (account: DashboardAccount): string => {
  const reset = account.resetAt === null ? "available" : formatTime(account.resetAt)
  const used = account.usedPercent === null ? "n/a" : `${Math.round(account.usedPercent)}%`
  return `
    <tr>
      <td><span class="provider">${escapeHtml(account.provider)}</span></td>
      <td><code>${escapeHtml(account.accountRef)}</code></td>
      <td>${escapeHtml(account.email ?? "email not published")}</td>
      <td><span class="pill pill-${account.readiness === "ready" ? "ready" : "warn"}">${escapeHtml(readinessLabel(account.readiness))}</span></td>
      <td class="num">${escapeHtml(used)}</td>
      <td>${escapeHtml(reset)}</td>
    </tr>`
}

const sessionRow = (session: DashboardSession): string => `
  <tr>
    <td><code>${escapeHtml(session.assignmentRef)}</code></td>
    <td>${escapeHtml(session.state)}</td>
    <td>${escapeHtml(session.provider ?? "unknown")}</td>
    <td><code>${escapeHtml(session.accountRef ?? "unreported")}</code></td>
    <td>${escapeHtml(session.jobKind)}</td>
    <td><code>${escapeHtml(session.pylonRef)}</code></td>
    <td class="num">${formatElapsed(session.elapsedMs)}</td>
    <td class="num">${session.tokenCount === null ? "pending" : formatNumber(session.tokenCount)}</td>
  </tr>`

const emptyRow = (message: string, columns: number): string =>
  `<tr><td class="empty-row" colspan="${columns}">${escapeHtml(message)}</td></tr>`

const sidecarPanel = (status: AppleFmSidecarPublicStatus | null): string => {
  if (status === null) {
    return `
      <section class="panel">
        <div class="section-heading">
          <h2>Local Apple FM</h2>
          <span>checking</span>
        </div>
        <div class="detail-grid">
          <article><span>State</span><strong>checking</strong></article>
          <article><span>Authority</span><strong>Pylon required</strong></article>
          <article><span>Blockers</span><strong>pending</strong></article>
        </div>
      </section>`
  }

  const blockerText =
    status.blockerRefs.length === 0 ? "none" : status.blockerRefs.join(", ")
  const source = status.helperSource === null ? "none" : status.helperSource
  return `
    <section class="panel">
      <div class="section-heading">
        <h2>Local Apple FM</h2>
        <span>${escapeHtml(formatTime(status.observedAt))}</span>
      </div>
      <div class="detail-grid">
        <article><span>State</span><strong><span class="pill ${status.available ? "pill-ready" : "pill-warn"}">${escapeHtml(status.state.replace(/_/g, " "))}</span></strong></article>
        <article><span>Helper</span><strong>${escapeHtml(source.replace(/_/g, " "))}</strong></article>
        <article><span>Owner process</span><strong>${status.launchedByApp ? "launched by app" : "not launched"}</strong></article>
      </div>
      <p class="panel-note">${escapeHtml(status.message)}</p>
      <p class="panel-note">Blockers: ${escapeHtml(blockerText)}</p>
    </section>`
}

const renderDashboard = (
  dashboard: KhalaDesktopDashboard,
  appleFmSidecar: AppleFmSidecarPublicStatus | null,
): string => `
  <section class="hero">
    <div>
      <p class="kicker">Khala operator desktop</p>
      <h1>Fleet control surface</h1>
      <p class="lede">Live pylon presence, Codex and Claude account health, active assignment load, and token accounting from the owner operator APIs.</p>
    </div>
    <div class="source">
      <span>Updated</span>
      <strong>${escapeHtml(formatTime(dashboard.generatedAt))}</strong>
      <small>${escapeHtml(dashboard.source.baseUrl)}</small>
    </div>
  </section>

  <section class="metrics" aria-label="Fleet summary">
    <article><span>Ready slots</span><strong>${formatNumber(dashboard.totals.readySlots)}</strong></article>
    <article><span>Busy slots</span><strong>${formatNumber(dashboard.totals.busySlots)}</strong></article>
    <article><span>Ready accounts</span><strong>${formatNumber(dashboard.totals.readyAccounts)}</strong></article>
    <article><span>Tokens today</span><strong>${formatNumber(dashboard.totals.tokensToday)}</strong></article>
  </section>

  ${sidecarPanel(appleFmSidecar)}

  <section class="panel">
    <div class="section-heading">
      <h2>Connected Pylons</h2>
      <span>${formatNumber(dashboard.pylons.length)} registered</span>
    </div>
    <div class="table-wrap">
      <table>
        <thead><tr><th>Pylon</th><th>Heartbeat</th><th>Status</th><th>Last seen</th><th>Ready</th><th>Busy</th><th>Queued</th></tr></thead>
        <tbody>${dashboard.pylons.length === 0 ? emptyRow("No connected pylons in this owner scope.", 7) : dashboard.pylons.map(pylonRow).join("")}</tbody>
      </table>
    </div>
  </section>

  <section class="panel">
    <div class="section-heading">
      <h2>Coding Accounts</h2>
      <span>Codex and Claude</span>
    </div>
    <div class="table-wrap">
      <table>
        <thead><tr><th>Provider</th><th>Ref</th><th>Email</th><th>Readiness</th><th>Used</th><th>Reset</th></tr></thead>
        <tbody>${dashboard.accounts.length === 0 ? emptyRow("No coding accounts reported by /api/operator/accounts/status.", 6) : dashboard.accounts.map(accountRow).join("")}</tbody>
      </table>
    </div>
  </section>

  <section class="panel">
    <div class="section-heading">
      <h2>Active Sessions</h2>
      <span>${formatNumber(dashboard.totals.activeAssignments)} running</span>
    </div>
    <div class="table-wrap">
      <table>
        <thead><tr><th>Assignment</th><th>State</th><th>Provider</th><th>Account</th><th>Kind</th><th>Pylon</th><th>Elapsed</th><th>Tokens</th></tr></thead>
        <tbody>${dashboard.sessions.length === 0 ? emptyRow("No active coding sessions are currently leased.", 8) : dashboard.sessions.map(sessionRow).join("")}</tbody>
      </table>
    </div>
  </section>`

const render = (state: ViewState): void => {
  if (state.status === "loading") {
    app.innerHTML = `
      <section class="hero skeleton-hero">
        <div><p class="kicker">Khala operator desktop</p><h1>Loading fleet</h1><p class="lede">Polling owner operator APIs.</p></div>
      </section>
      <section class="metrics">${Array.from({ length: 4 }, () => "<article class=\"skeleton\"></article>").join("")}</section>`
    return
  }

  if (state.status === "error") {
    app.innerHTML = `
      <section class="hero">
        <div>
          <p class="kicker">Khala operator desktop</p>
          <h1>Fleet data unavailable</h1>
          <p class="lede">${escapeHtml(state.error)}</p>
        </div>
        <div class="source"><span>Observed</span><strong>${escapeHtml(formatTime(state.observedAt))}</strong></div>
      </section>
      <section class="panel"><p class="empty-row">Set an owner token in the Bun host environment and reopen the app.</p></section>`
    return
  }

  app.innerHTML = renderDashboard(state.dashboard, state.appleFmSidecar)
}

const load = async (): Promise<void> => {
  const [result, appleFmSidecar] = await Promise.all([
    rpc.request.operatorDashboard(),
    rpc.request.appleFmSidecarStatus().catch(() => null),
  ])
  if (result.ok) {
    render({ status: "ready", appleFmSidecar, dashboard: result.dashboard })
  } else {
    render({ status: "error", error: result.error, observedAt: result.observedAt })
  }
}

render({ status: "loading" })
void load()
setInterval(() => void load(), KHALA_OPERATOR_POLL_INTERVAL_MS)

document.addEventListener("click", event => {
  const target = event.target
  if (!(target instanceof HTMLAnchorElement)) return
  const href = target.href
  if (/^https?:\/\//i.test(href)) {
    event.preventDefault()
    void rpc.request.openExternal({ url: href })
  }
})
