import { define as defineCustomElement } from 'foldkit/customElement'
import type { Attribute, Html } from 'foldkit/html'
import { slotText } from 'slot-text'

import {
  fetchPylonStats,
  readInitialPylonStatsSnapshot,
  type PylonStatsSnapshot,
} from './pylonNetworkStats'

// #5050: live network stats overlaid on the homepage, using the SAME slot-text
// digit-roll as the countdown (pylonCountdownElement) so updating numbers roll.
// Sits behind the countdown for now; when the countdown is removed at launch
// this overlay (plus the activity-lit pylon) becomes the homepage. Lives in its
// own shadow root so the slot-text CSS is scoped here.

export const pylonStatsTagName = 'oa-pylon-stats'

const STATS: ReadonlyArray<{ key: string; label: string }> = [
  { key: 'online', label: 'pylons online' },
  { key: 'working', label: 'work-ready now' },
  { key: 'sats24h', label: 'sats settled · 24h' },
  { key: 'training', label: 'training contributors' },
]

const pos = (value: number | null | undefined): number =>
  typeof value === 'number' && Number.isFinite(value) && value > 0 ? Math.floor(value) : 0

const sumSats24h = (snapshot: PylonStatsSnapshot): number => {
  const publicReal = pos(snapshot.publicRealSatsSettled24h)
  if (publicReal > 0) return publicReal

  const m = snapshot.nip90MarketSettlementStats
  if (!m) return 0
  return pos(m.compute?.satsSettled24h) + pos(m.data?.satsSettled24h) + pos(m.labor?.satsSettled24h)
}

const statValues = (snapshot: PylonStatsSnapshot | null): Record<string, string> => {
  const fmt = (n: number): string => n.toLocaleString('en-US')
  if (snapshot === null) return { online: '0', working: '0', sats24h: '0', training: '0' }
  return {
    online: fmt(pos(snapshot.pylonsOnlineNow)),
    working: fmt(pos(snapshot.pylonsAssignmentReadyNow)),
    sats24h: fmt(sumSats24h(snapshot)),
    training: fmt(pos(snapshot.trainingModelProgressContributors)),
  }
}

// slot-text structural CSS (same as the countdown element) so the digit roll
// works inside this element's shadow root.
const slotTextCss = `
.slot-text { display: inline-flex; white-space: pre; }
.char-slot { position: relative; display: inline-flex; flex: none; justify-content: center; overflow: hidden; overflow-x: visible; overflow-y: clip; line-height: 1.3; vertical-align: bottom; }
.char-slot.is-resizing { overflow-x: clip; }
.char-sizer { visibility: hidden; white-space: pre; }
.char-face { position: absolute; inset: 0; display: flex; align-items: center; justify-content: center; white-space: pre; will-change: transform; }
`

const hostCss = `
:host { position: absolute; inset: 0; display: block; pointer-events: none; }
.stats-overlay { position: absolute; inset: 0; display: flex; align-items: flex-end; justify-content: center; padding: clamp(1.5rem, 5vw, 4rem); }
.stats-row { display: flex; flex-wrap: wrap; gap: clamp(1rem, 4vw, 3rem); align-items: flex-end; opacity: 0.92; }
.stat { display: flex; flex-direction: column; gap: 0.2rem; align-items: center; color: #d7d8e5; }
.stat-value { font-size: clamp(1.1rem, 2.4vw, 1.9rem); font-weight: 600; font-variant-numeric: tabular-nums; color: #d6f6ff; }
.stat-label { font-size: 0.65rem; letter-spacing: 0.14em; text-transform: uppercase; color: #8a8c93; }
`

export type PylonStatsHandle = Readonly<{ dispose: () => void }>

export const mountPylonStats = (
  root: HTMLElement,
  options: {
    fetchFn?: typeof fetch
    initialSnapshot?: PylonStatsSnapshot
    intervalMs?: number
  } = {},
): PylonStatsHandle => {
  const controllers: Record<string, ReturnType<typeof slotText>> = {}
  const initialValues =
    options.initialSnapshot === undefined
      ? null
      : statValues(options.initialSnapshot)
  const LOADING = '…'
  for (const stat of STATS) {
    const valueEl = root.querySelector<HTMLElement>(`[data-stat-value="${stat.key}"]`)
    const initialText = initialValues?.[stat.key] ?? LOADING
    if (valueEl) controllers[stat.key] = slotText(valueEl, initialText, {})
  }

  let disposed = false
  const apply = async (): Promise<void> => {
    const snapshot = await fetchPylonStats(options.fetchFn)
    if (disposed) return
    const values = statValues(snapshot)
    for (const stat of STATS) controllers[stat.key]?.set(values[stat.key] ?? '0')
  }
  void apply()
  // #5050: poll every 3s for near-realtime "pylons join/leave" updates. The
  // server caches the snapshot (~4s TTL) so this is cheap; each poll returns the
  // latest cached value instantly instead of recomputing.
  const timer = setInterval(() => void apply(), options.intervalMs ?? 3_000)

  return {
    dispose: () => {
      if (disposed) return
      disposed = true
      clearInterval(timer)
      for (const key of Object.keys(controllers)) controllers[key]?.destroy()
    },
  }
}

const pylonStatsElement = defineCustomElement({ events: {}, properties: {}, tag: pylonStatsTagName })

const makePylonStatsElement = (): CustomElementConstructor =>
  class PylonStatsElement extends HTMLElement {
    #handle: PylonStatsHandle | null = null

    connectedCallback(): void {
      if (this.#handle !== null) return
      const shadow = this.shadowRoot ?? this.attachShadow({ mode: 'open' })
      shadow.replaceChildren()

      const style = document.createElement('style')
      style.textContent = `${hostCss}\n${slotTextCss}`

      const overlay = document.createElement('div')
      overlay.className = 'stats-overlay'
      const row = document.createElement('div')
      row.className = 'stats-row'
      for (const stat of STATS) {
        const cell = document.createElement('div')
        cell.className = 'stat'
        const value = document.createElement('div')
        value.className = 'stat-value'
        value.setAttribute('data-stat-value', stat.key)
        const label = document.createElement('div')
        label.className = 'stat-label'
        label.textContent = stat.label
        cell.append(value, label)
        row.append(cell)
      }
      overlay.append(row)
      shadow.append(style, overlay)

      const initialSnapshot = readInitialPylonStatsSnapshot()
      this.#handle = mountPylonStats(overlay, {
        ...(initialSnapshot === null ? {} : { initialSnapshot }),
      })
    }

    disconnectedCallback(): void {
      if (this.#handle === null) return
      this.#handle.dispose()
      this.#handle = null
    }
  }

export const registerPylonStatsElement = (): void => {
  if (typeof customElements === 'undefined') return
  if (typeof HTMLElement === 'undefined') return
  if (customElements.get(pylonStatsTagName) !== undefined) return
  customElements.define(pylonStatsTagName, makePylonStatsElement())
}

export const pylonStatsView = <Message>(
  attributes: ReadonlyArray<Attribute<Message>> = [],
): Html => {
  registerPylonStatsElement()
  const element = pylonStatsElement.withMessage<Message>()
  return element(attributes, [])
}
