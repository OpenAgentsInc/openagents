import { define as defineCustomElement } from 'foldkit/customElement'
import type { Attribute, Html } from 'foldkit/html'
import { html } from 'foldkit/html'

import * as Ui from '../ui'
import type { PublicHeaderAuthState } from './publicHeader'
import * as PublicHeader from './publicHeader'
import { currentUnixMs } from '../time-format'

type AccountUsageWindow = Readonly<{
  cap: number | null
  label: 'hourly' | 'weekly'
  percentUsed: number
  remaining: number | null
  used: number | null
}>

type AccountStatusEntry = Readonly<{
  accountRefHash: string
  cooldownExpiresAt: string | null
  isRateLimited: boolean
  manualResetsRemaining: number | null
  provider: string
  windows: ReadonlyArray<AccountUsageWindow>
}>

type AccountsStatusResponse = Readonly<{
  accounts: ReadonlyArray<AccountStatusEntry>
  observedAt: string
}>

type DashboardState =
  | Readonly<{ tag: 'loading' }>
  | Readonly<{ tag: 'unauthorized'; status: number }>
  | Readonly<{ tag: 'failed'; message: string }>
  | Readonly<{
      tag: 'loaded'
      response: AccountsStatusResponse
      resettingAccountRefHash: string | null
    }>

const dashboardTagName = 'oa-artanis-accounts-dashboard'

const dashboardElement = defineCustomElement({
  events: {},
  properties: {},
  tag: dashboardTagName,
})

const numberFormatter = new Intl.NumberFormat('en-US')

const formatNumber = (value: number | null): string =>
  value === null ? '-' : numberFormatter.format(value)

const escapeHtml = (value: string): string =>
  value
    .replaceAll('&', '&amp;')
    .replaceAll('<', '&lt;')
    .replaceAll('>', '&gt;')
    .replaceAll('"', '&quot;')
    .replaceAll("'", '&#39;')

const labelForProvider = (provider: string): string =>
  provider === 'codex'
    ? 'Codex'
    : provider === 'claude'
      ? 'Claude'
      : provider

const shortAccountRef = (ref: string): string =>
  ref.length <= 18 ? ref : `${ref.slice(0, 14)}...${ref.slice(-4)}`

const countdownLabel = (expiresAt: string | null, now = currentUnixMs()): string => {
  if (expiresAt === null) {
    return 'available'
  }

  const remainingMs = Date.parse(expiresAt) - now

  if (!Number.isFinite(remainingMs) || remainingMs <= 0) {
    return 'ready now'
  }

  const totalSeconds = Math.ceil(remainingMs / 1000)
  const minutes = Math.floor(totalSeconds / 60)
  const seconds = totalSeconds % 60

  return `${minutes}:${String(seconds).padStart(2, '0')}`
}

const responseIsAccountsStatus = (
  value: unknown,
): value is AccountsStatusResponse => {
  if (typeof value !== 'object' || value === null) {
    return false
  }

  const candidate = value as Record<string, unknown>
  return (
    typeof candidate.observedAt === 'string' &&
    Array.isArray(candidate.accounts)
  )
}

const resetResponseStatus = (value: unknown): AccountsStatusResponse | null => {
  if (typeof value !== 'object' || value === null) {
    return null
  }

  const status = (value as Record<string, unknown>).status
  return responseIsAccountsStatus(status) ? status : null
}

const dashboardCss = `
:host {
  display: block;
  color: #f1efe8;
  font-family: 'Berkeley Mono', ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace;
}
* { box-sizing: border-box; }
.shell { display: grid; gap: 1rem; }
.summary {
  display: grid;
  grid-template-columns: repeat(4, minmax(0, 1fr));
  gap: 0.75rem;
}
.metric {
  min-height: 5rem;
  display: grid;
  align-content: space-between;
  border: 1px solid rgba(255, 255, 255, 0.1);
  background: #050505;
  padding: 0.75rem;
}
.metric-label {
  color: rgba(255, 255, 255, 0.46);
  font-size: 0.68rem;
  text-transform: uppercase;
}
.metric-value {
  color: #f1efe8;
  font-size: 1.35rem;
  font-weight: 700;
  font-variant-numeric: tabular-nums;
}
.toolbar {
  display: flex;
  align-items: end;
  justify-content: space-between;
  gap: 1rem;
  border: 1px solid rgba(255, 255, 255, 0.1);
  background: #010102;
  padding: 0.875rem;
}
.toolbar-title {
  margin: 0;
  color: #ffffff;
  font-size: 0.95rem;
  font-weight: 700;
}
.toolbar-detail {
  margin: 0.25rem 0 0;
  color: rgba(255, 255, 255, 0.52);
  font-size: 0.76rem;
}
.refresh,
.reset {
  min-height: 2.25rem;
  font: inherit;
  font-size: 0.72rem;
  font-weight: 700;
  cursor: pointer;
}
.refresh {
  border: 1px solid rgba(255, 180, 0, 0.45);
  background: rgba(255, 180, 0, 0.12);
  color: #ffe0a3;
  padding: 0 0.75rem;
}
.refresh:hover { background: rgba(255, 180, 0, 0.18); }
.refresh:focus-visible,
.reset:focus-visible {
  outline: 2px solid #ffb400;
  outline-offset: 2px;
}
.grid {
  display: grid;
  border: 1px solid rgba(255, 255, 255, 0.1);
  background: #010102;
}
.header,
.row {
  display: grid;
  grid-template-columns: minmax(9rem, 1.15fr) minmax(5rem, 0.55fr) minmax(8rem, 0.8fr) minmax(13rem, 1.1fr) minmax(13rem, 1.1fr) minmax(9rem, 0.85fr);
  gap: 0.75rem;
  align-items: center;
}
.header {
  padding: 0.75rem 1rem;
  color: rgba(255, 255, 255, 0.45);
  font-size: 0.68rem;
  text-transform: uppercase;
}
.row {
  min-height: 7.25rem;
  border-top: 1px solid rgba(255, 255, 255, 0.1);
  padding: 1rem;
  color: rgba(255, 255, 255, 0.68);
  font-size: 0.78rem;
}
.account { min-width: 0; display: grid; gap: 0.35rem; }
.account-ref {
  overflow-wrap: anywhere;
  color: #f1efe8;
  font-weight: 700;
}
.account-full {
  color: rgba(255, 255, 255, 0.36);
  font-size: 0.68rem;
}
.status {
  width: fit-content;
  display: inline-flex;
  align-items: center;
  gap: 0.45rem;
  border: 1px solid rgba(0, 200, 83, 0.35);
  background: rgba(0, 200, 83, 0.12);
  color: #a8f5c2;
  padding: 0.35rem 0.5rem;
  font-size: 0.7rem;
  font-weight: 700;
}
.status::before {
  width: 0.5rem;
  height: 0.5rem;
  border-radius: 999px;
  background: #00c853;
  content: '';
}
.status.limited {
  border-color: rgba(211, 47, 47, 0.45);
  background: rgba(211, 47, 47, 0.13);
  color: #ffb4b4;
}
.status.limited::before { background: #d32f2f; }
.window { display: grid; gap: 0.45rem; }
.window-head {
  display: flex;
  justify-content: space-between;
  gap: 0.75rem;
  color: rgba(255, 255, 255, 0.56);
  font-size: 0.7rem;
  text-transform: uppercase;
}
.bar {
  height: 0.5rem;
  overflow: hidden;
  border: 1px solid rgba(255, 255, 255, 0.1);
  background: #000000;
}
.fill {
  height: 100%;
  width: var(--fill);
  background: #2979ff;
  transition: width 180ms ease-out;
}
.fill.warn { background: #ff6f00; }
.usage {
  color: rgba(255, 255, 255, 0.46);
  font-size: 0.68rem;
}
.reset-cell { display: grid; gap: 0.5rem; }
.reset {
  border: 1px solid rgba(255, 255, 255, 0.16);
  background: #0b0b0b;
  color: #f1efe8;
  padding: 0 0.65rem;
}
.reset:hover:not(:disabled) { background: #141414; }
.reset:disabled {
  cursor: not-allowed;
  color: rgba(255, 255, 255, 0.36);
}
.reset-detail {
  color: rgba(255, 255, 255, 0.42);
  font-size: 0.68rem;
}
.state {
  border: 1px solid rgba(255, 255, 255, 0.1);
  background: #050505;
  padding: 1rem;
  color: rgba(255, 255, 255, 0.62);
  font-size: 0.85rem;
  line-height: 1.55;
}
.state strong { color: #f1efe8; }
@media (max-width: 980px) {
  .summary { grid-template-columns: repeat(2, minmax(0, 1fr)); }
  .header { display: none; }
  .row { grid-template-columns: 1fr; }
}
@media (max-width: 560px) {
  .summary { grid-template-columns: 1fr; }
  .toolbar { display: grid; }
}
@media (prefers-reduced-motion: reduce) {
  .fill { transition: none; }
}
`

const statusSummary = (accounts: ReadonlyArray<AccountStatusEntry>) => {
  const limited = accounts.filter(account => account.isRateLimited).length
  const available = accounts.length - limited
  const manualResets = accounts.reduce(
    (total, account) => total + (account.manualResetsRemaining ?? 0),
    0,
  )
  const knownWindows = accounts.flatMap(account => account.windows)
  const averageUsage =
    knownWindows.length === 0
      ? 0
      : Math.round(
          knownWindows.reduce(
            (total, window) => total + window.percentUsed,
            0,
          ) / knownWindows.length,
        )

  return { available, averageUsage, limited, manualResets }
}

const renderWindow = (
  entry: AccountStatusEntry,
  window: AccountUsageWindow,
): string => {
  const percent = Math.max(0, Math.min(100, window.percentUsed))
  const fillClass = percent >= 80 ? 'fill warn' : 'fill'
  const usage =
    window.cap === null
      ? 'No cap reported'
      : `${formatNumber(window.used)} / ${formatNumber(window.cap)} used, ${formatNumber(window.remaining)} left`

  return `
    <div class="window">
      <div class="window-head">
        <span>${window.label}</span>
        <span>${percent}%</span>
      </div>
      <div class="bar" role="meter" aria-label="${escapeHtml(entry.provider)} ${window.label} token usage ${percent}%" aria-valuemin="0" aria-valuemax="100" aria-valuenow="${percent}">
        <div class="${fillClass}" style="--fill: ${percent}%"></div>
      </div>
      <div class="usage">${usage}</div>
    </div>
  `
}

const renderRow = (
  entry: AccountStatusEntry,
  resettingAccountRefHash: string | null,
): string => {
  const resetCount =
    entry.manualResetsRemaining === null
      ? 'N'
      : String(entry.manualResetsRemaining)
  const resetDisabled =
    resettingAccountRefHash === entry.accountRefHash ||
    entry.manualResetsRemaining === 0
  const resetLabel =
    resettingAccountRefHash === entry.accountRefHash
      ? 'Resetting'
      : `Manual Reset (${resetCount} left)`
  const statusClass = entry.isRateLimited ? 'status limited' : 'status'
  const statusText = entry.isRateLimited ? 'rate-limited' : 'available'
  const accountRefHash = escapeHtml(entry.accountRefHash)
  const provider = escapeHtml(labelForProvider(entry.provider))

  return `
    <div class="row" role="row" data-account-ref="${accountRefHash}">
      <div class="account" role="cell">
        <div class="account-ref">${escapeHtml(shortAccountRef(entry.accountRefHash))}</div>
        <div class="account-full">${accountRefHash}</div>
      </div>
      <div role="cell">${provider}</div>
      <div role="cell">
        <span class="${statusClass}"><span>${statusText}</span></span>
      </div>
      ${entry.windows.map(window => `<div role="cell">${renderWindow(entry, window)}</div>`).join('')}
      <div class="reset-cell" role="cell">
        <button class="reset" type="button" data-reset-account="${accountRefHash}" ${resetDisabled ? 'disabled' : ''}>${resetLabel}</button>
        <div class="reset-detail" data-countdown-for="${accountRefHash}">${entry.isRateLimited ? countdownLabel(entry.cooldownExpiresAt) : 'available'}</div>
      </div>
    </div>
  `
}

const renderLoaded = (
  response: AccountsStatusResponse,
  resettingAccountRefHash: string | null,
): string => {
  const summary = statusSummary(response.accounts)

  return `
    <div class="shell">
      <div class="summary" aria-label="Account summary">
        <div class="metric"><div class="metric-label">accounts</div><div class="metric-value">${response.accounts.length} / 9</div></div>
        <div class="metric"><div class="metric-label">available</div><div class="metric-value">${summary.available}</div></div>
        <div class="metric"><div class="metric-label">rate-limited</div><div class="metric-value">${summary.limited}</div></div>
        <div class="metric"><div class="metric-label">manual resets</div><div class="metric-value">${summary.manualResets}</div></div>
      </div>
      <div class="toolbar">
        <div>
          <h2 class="toolbar-title">Account status</h2>
          <p class="toolbar-detail">Observed ${escapeHtml(response.observedAt)}. Average usage ${summary.averageUsage}%.</p>
        </div>
        <button class="refresh" type="button" data-refresh>Refresh</button>
      </div>
      <div class="grid" role="table" aria-label="Operator account status">
        <div class="header" role="row">
          <div role="columnheader">Account</div>
          <div role="columnheader">Provider</div>
          <div role="columnheader">State</div>
          <div role="columnheader">Hourly</div>
          <div role="columnheader">Weekly</div>
          <div role="columnheader">Reset</div>
        </div>
        ${response.accounts.length === 0 ? '<div class="state">No operator account rows are available.</div>' : response.accounts.map(entry => renderRow(entry, resettingAccountRefHash)).join('')}
      </div>
    </div>
  `
}

const renderState = (state: DashboardState): string => {
  if (state.tag === 'loading') {
    return '<div class="state"><strong>Loading account status.</strong></div>'
  }

  if (state.tag === 'unauthorized') {
    return `<div class="state"><strong>Unauthorized.</strong> This owner-only account dashboard is not available for the current session. (${state.status})</div>`
  }

  if (state.tag === 'failed') {
    return `<div class="state"><strong>Status unavailable.</strong> ${escapeHtml(state.message)}</div>`
  }

  return renderLoaded(state.response, state.resettingAccountRefHash)
}

const makeDashboardElement = (): CustomElementConstructor =>
  class ArtanisAccountsDashboardElement extends HTMLElement {
    #state: DashboardState = { tag: 'loading' }
    #timer: number | null = null

    connectedCallback(): void {
      const shadow = this.shadowRoot ?? this.attachShadow({ mode: 'open' })

      if (shadow.childNodes.length === 0) {
        const style = document.createElement('style')
        style.textContent = dashboardCss
        const root = document.createElement('div')
        root.setAttribute('data-root', '')
        shadow.append(style, root)
      }

      this.#render()
      void this.#load()
      this.#timer = window.setInterval(() => this.#updateCountdowns(), 1000)
    }

    disconnectedCallback(): void {
      if (this.#timer !== null) {
        window.clearInterval(this.#timer)
        this.#timer = null
      }
    }

    async #load(): Promise<void> {
      this.#state = { tag: 'loading' }
      this.#render()

      try {
        const response = await fetch('/api/operator/accounts/status', {
          cache: 'no-store',
          credentials: 'include',
          headers: { accept: 'application/json' },
        })

        if (response.status === 401 || response.status === 403) {
          this.#state = { tag: 'unauthorized', status: response.status }
          this.#render()
          return
        }

        if (!response.ok) {
          this.#state = {
            message: `GET /api/operator/accounts/status returned ${response.status}.`,
            tag: 'failed',
          }
          this.#render()
          return
        }

        const body = await response.json()
        this.#state = responseIsAccountsStatus(body)
          ? { response: body, resettingAccountRefHash: null, tag: 'loaded' }
          : {
              message: 'The status payload did not match the account schema.',
              tag: 'failed',
            }
        this.#render()
      } catch (error) {
        this.#state = {
          message:
            error instanceof Error ? error.message : 'Unknown fetch error.',
          tag: 'failed',
        }
        this.#render()
      }
    }

    async #reset(accountRefHash: string): Promise<void> {
      if (this.#state.tag !== 'loaded') {
        return
      }

      this.#state = {
        response: this.#state.response,
        resettingAccountRefHash: accountRefHash,
        tag: 'loaded',
      }
      this.#render()

      try {
        const response = await fetch('/api/operator/accounts/reset', {
          body: JSON.stringify({ accountRefHash }),
          cache: 'no-store',
          credentials: 'include',
          headers: {
            accept: 'application/json',
            'content-type': 'application/json',
          },
          method: 'POST',
        })

        if (response.status === 401 || response.status === 403) {
          this.#state = { tag: 'unauthorized', status: response.status }
          this.#render()
          return
        }

        if (!response.ok) {
          this.#state = {
            message: `POST /api/operator/accounts/reset returned ${response.status}.`,
            tag: 'failed',
          }
          this.#render()
          return
        }

        const status = resetResponseStatus(await response.json())
        this.#state =
          status === null
            ? {
                message: 'The reset payload did not include refreshed status.',
                tag: 'failed',
              }
            : { response: status, resettingAccountRefHash: null, tag: 'loaded' }
        this.#render()
      } catch (error) {
        this.#state = {
          message:
            error instanceof Error ? error.message : 'Unknown reset error.',
          tag: 'failed',
        }
        this.#render()
      }
    }

    #render(): void {
      const root = this.shadowRoot?.querySelector('[data-root]')

      if (!(root instanceof HTMLElement)) {
        return
      }

      root.innerHTML = renderState(this.#state)
      root
        .querySelector('[data-refresh]')
        ?.addEventListener('click', () => void this.#load())
      root.querySelectorAll('[data-reset-account]').forEach(button => {
        button.addEventListener('click', () => {
          const accountRefHash = button.getAttribute('data-reset-account')

          if (accountRefHash !== null) {
            void this.#reset(accountRefHash)
          }
        })
      })
      this.#updateCountdowns()
    }

    #updateCountdowns(): void {
      if (this.#state.tag !== 'loaded') {
        return
      }

      const accounts = new Map(
        this.#state.response.accounts.map(account => [
          account.accountRefHash,
          account,
        ]),
      )

      this.shadowRoot
        ?.querySelectorAll('[data-countdown-for]')
        .forEach(countdown => {
          const accountRefHash = countdown.getAttribute('data-countdown-for')
          const account =
            accountRefHash === null ? undefined : accounts.get(accountRefHash)

          if (account !== undefined) {
            countdown.textContent = account.isRateLimited
              ? countdownLabel(account.cooldownExpiresAt)
              : 'available'
          }
        })
    }
  }

const registerDashboardElement = (): void => {
  if (typeof customElements === 'undefined') {
    return
  }

  if (typeof HTMLElement === 'undefined') {
    return
  }

  if (customElements.get(dashboardTagName) !== undefined) {
    return
  }

  customElements.define(dashboardTagName, makeDashboardElement())
}

const dashboardView = <Message>(
  attributes: ReadonlyArray<Attribute<Message>> = [],
): Html => {
  registerDashboardElement()
  return dashboardElement.withMessage<Message>()(attributes, [])
}

export const view = <Message>(
  authState: PublicHeaderAuthState<Message>,
): Html => {
  const h = html<Message>()

  return h.div(
    [Ui.className<Message>('min-h-screen bg-black text-[#f1efe8]')],
    [
      PublicHeader.view(authState),
      h.main(
        [
          Ui.className<Message>(
            'mx-auto grid w-[min(100%,1180px)] gap-6 px-4 py-8 sm:px-6 lg:px-8',
          ),
        ],
        [
          h.header(
            [Ui.className<Message>('grid gap-3 border-b border-white/10 pb-5')],
            [
              h.div(
                [
                  Ui.className<Message>(
                    'text-[0.7rem] uppercase tracking-wide text-white/45',
                  ),
                ],
                ['Artanis / accounts'],
              ),
              h.h1(
                [
                  Ui.className<Message>(
                    'm-0 text-2xl font-semibold tracking-normal text-[#f1efe8] sm:text-3xl',
                  ),
                ],
                ['Operator account observability'],
              ),
              h.p(
                [
                  Ui.className<Message>(
                    'm-0 max-w-[72ch] text-sm leading-6 text-white/60',
                  ),
                ],
                [
                  'Owner-only status for Codex and Claude coding accounts: live cooldowns, usage windows, and manual reset controls.',
                ],
              ),
            ],
          ),
          dashboardView<Message>([
            h.AriaLabel('Operator account observability dashboard'),
          ]),
          h.p(
            [
              Ui.className<Message>(
                'm-0 max-w-[76ch] border border-white/10 bg-[#050505] p-3 text-xs leading-5 text-white/45',
              ),
            ],
            [
              'This surface is operator evidence and control only. It does not grant dispatch, spend, settlement, provider-account ownership transfer, or cross-owner routing authority.',
            ],
          ),
        ],
      ),
    ],
  )
}
