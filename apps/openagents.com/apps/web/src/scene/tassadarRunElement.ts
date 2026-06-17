// Live Tassadar run scene element (#5118, epic #5112).
//
// A self-fetching custom element that, on connect, fetches the public-safe run
// summary from `GET /api/public/tassadar-run-summary` (no auth, #5114), maps it
// through the merged snapshot adapter (`tassadarRunVisualizationOptions`, #5113),
// and mounts the `oa-training-run` WebGL element with the resulting options —
// the real run, breathing.
//
// RECEIPT-FIRST: this element NEVER fabricates metrics. A just-launched / idle
// run (the endpoint's `emptyState.idle`) flows through the adapter as honest
// zeros and renders the empty/planned scene. A non-200 response or network
// failure renders a graceful, honest error message — never faked numbers.
//
// State machine: loading → ok (mount scene) | empty (idle honest scene) | error.
// The data-state attribute is exposed for tests and styling. Dark-only.
import type { TrainingRunNodeSelection } from '@openagentsinc/three-effect/core'
import {
  registerTrainingRunElement,
  trainingRunTagName,
} from '@openagentsinc/three-effect/foldkit'
import { define as defineCustomElement } from 'foldkit/customElement'
import type { Attribute, Html } from 'foldkit/html'

import {
  type TassadarRunPublicSummary,
  tassadarRunVisualizationOptions,
} from './tassadarRunSnapshot'

export const TASSADAR_RUN_TAG = 'oa-tassadar-run'
export const TASSADAR_RUN_SUMMARY_ENDPOINT = '/api/public/tassadar-run-summary'

export type TassadarRunDataState = 'loading' | 'ok' | 'empty' | 'error'
export type TassadarRunProofLink = Readonly<{
  href: string
  label: string
  ref: string
}>

const HOST_STYLE =
  ':host{position:absolute;inset:0;display:block;background:#000;color:#f1efe8}' +
  '.mount{position:absolute;inset:0}' +
  '.overlay{position:absolute;inset:0;display:flex;align-items:center;justify-content:center;' +
  'padding:2rem;text-align:center;font-family:ui-monospace,SFMono-Regular,Menlo,monospace}' +
  '.overlay p{margin:0;max-width:48ch;font-size:0.95rem;line-height:1.6;color:rgba(241,239,232,0.6)}' +
  '.overlay .label{display:block;margin-bottom:0.4rem;font-size:0.7rem;letter-spacing:0.08em;' +
  'text-transform:uppercase;color:rgba(241,239,232,0.35)}' +
  '.status{position:absolute;top:0.75rem;left:0.75rem;right:0.75rem;z-index:3;display:flex;align-items:flex-start;' +
  'justify-content:space-between;gap:0.75rem;border:1px solid rgba(255,255,255,0.14);background:rgba(0,0,0,0.68);' +
  'padding:0.7rem 0.8rem;font-family:ui-monospace,SFMono-Regular,Menlo,monospace;color:#f1efe8;backdrop-filter:blur(10px)}' +
  '.status dl{display:grid;grid-template-columns:repeat(5,minmax(0,auto));gap:0.55rem 1rem;margin:0;min-width:0}' +
  '.status div{min-width:0}.status dt{margin:0 0 0.18rem;font-size:0.6rem;letter-spacing:0.08em;text-transform:uppercase;color:rgba(255,255,255,0.35)}' +
  '.status dd{margin:0;max-width:18rem;overflow:hidden;text-overflow:ellipsis;white-space:nowrap;font-size:0.72rem;color:rgba(255,255,255,0.78)}' +
  '.status button{min-height:2rem;border:1px solid rgba(255,255,255,0.18);background:rgba(255,255,255,0.06);padding:0 0.65rem;' +
  'font:inherit;font-size:0.7rem;color:rgba(255,255,255,0.82);cursor:pointer}' +
  '.status button:focus-visible{outline:2px solid rgba(114,191,255,0.9);outline-offset:2px}' +
  '@media (max-width:720px){.status{display:grid}.status dl{grid-template-columns:repeat(2,minmax(0,1fr))}.status dd{max-width:none}.status button{justify-self:start}}' +
  '.selection{position:absolute;right:1rem;bottom:1rem;z-index:2;max-width:min(26rem,calc(100% - 2rem));' +
  'border:1px solid rgba(255,255,255,0.14);background:rgba(0,0,0,0.78);padding:0.75rem 0.875rem;' +
  'font-family:ui-monospace,SFMono-Regular,Menlo,monospace;color:#f1efe8;backdrop-filter:blur(10px)}' +
  '.selection strong{display:block;margin-bottom:0.25rem;font-size:0.78rem;font-weight:600;color:rgba(255,255,255,0.88)}' +
  '.selection p{margin:0;font-size:0.72rem;line-height:1.45;color:rgba(255,255,255,0.55)}' +
  '.selection a{display:inline-flex;margin-top:0.55rem;font-size:0.72rem;color:rgba(255,255,255,0.86);text-underline-offset:0.18rem}' +
  '.selection a:hover{color:#fff}'

const isIdle = (summary: TassadarRunPublicSummary): boolean =>
  summary.emptyState?.idle === true

// Pure: classify a fetched summary into the data-state the scene should show.
// Idle → 'empty' (still renders the honest planned/zeroed scene). Otherwise 'ok'.
export const dataStateForSummary = (
  summary: TassadarRunPublicSummary,
): Exclude<TassadarRunDataState, 'loading' | 'error'> =>
  isIdle(summary) ? 'empty' : 'ok'

const publicTrainingRunHref = (summary: TassadarRunPublicSummary): string => {
  const runRef = summary.runRef ?? 'run.tassadar.executor.20260615'
  return `/api/public/training/runs/${encodeURIComponent(runRef)}`
}

const focusedTrainingRunHref = (
  summary: TassadarRunPublicSummary,
  ref: string,
): string =>
  `${publicTrainingRunHref(summary)}?focusRef=${encodeURIComponent(ref)}`

const receiptHref = (ref: string): string =>
  `/api/forum/receipts/${encodeURIComponent(ref)}`

const firstRef = (
  refs: ReadonlyArray<string> | undefined,
): string | undefined =>
  Array.isArray(refs)
    ? refs.map(ref => ref.trim()).find(ref => ref.length > 0)
    : undefined

const metricNumber = (
  metric: { readonly value?: number } | undefined,
): number =>
  metric !== undefined &&
  typeof metric.value === 'number' &&
  Number.isFinite(metric.value)
    ? metric.value
    : 0

const textOrUnknown = (value: string | undefined): string => {
  const text = value?.trim()
  return text === undefined || text.length === 0 ? 'unknown' : text
}

const generatedAtText = (summary: TassadarRunPublicSummary): string =>
  textOrUnknown(summary.generatedAt)

const stalenessText = (summary: TassadarRunPublicSummary): string => {
  const staleness = summary.staleness
  if (staleness === undefined) return 'unknown'
  const contract = textOrUnknown(staleness.contractVersion)
  const composition = textOrUnknown(staleness.composition)
  const max =
    typeof staleness.maxStalenessSeconds === 'number' &&
    Number.isFinite(staleness.maxStalenessSeconds)
      ? `${staleness.maxStalenessSeconds}s`
      : 'unknown'
  return `${contract} / ${composition} / max ${max}`
}

const linkForRef = (
  summary: TassadarRunPublicSummary,
  label: string,
  ref: string | undefined,
): TassadarRunProofLink | null =>
  ref === undefined
    ? null
    : {
        href: ref.startsWith('receipt.')
          ? receiptHref(ref)
          : focusedTrainingRunHref(summary, ref),
        label,
        ref,
      }

const replayPairForSelection = (
  summary: TassadarRunPublicSummary,
  selection: TrainingRunNodeSelection,
) =>
  summary.realGradient?.verifiedReplayPairs?.find(
    pair =>
      pair.workerRef === selection.id || pair.validatorRef === selection.id,
  )

const leaderboardRowForSelection = (
  summary: TassadarRunPublicSummary,
  selection: TrainingRunNodeSelection,
) =>
  summary.realGradient?.leaderboardRows?.find(
    row => row.pylonRef === selection.id,
  )

export const proofLinkForSelection = (
  summary: TassadarRunPublicSummary,
  selection: TrainingRunNodeSelection,
): TassadarRunProofLink | null => {
  const pair = replayPairForSelection(summary, selection)
  if (pair !== undefined) {
    return linkForRef(summary, 'Verified replay challenge', pair.challengeRef)
  }

  const row = leaderboardRowForSelection(summary, selection)
  if (row !== undefined) {
    return linkForRef(summary, 'Pylon evidence', firstRef(row.sourceRefs))
  }

  if (selection.id === 'run') {
    return {
      href: publicTrainingRunHref(summary),
      label: 'Public run projection',
      ref: summary.runRef ?? 'run.tassadar.executor.20260615',
    }
  }

  if (selection.id === 'training_window' || selection.id === 'active') {
    return linkForRef(
      summary,
      'Training window',
      summary.windows?.[0]?.windowRef,
    )
  }

  if (selection.id === 'freivalds' || selection.role === 'proof') {
    return linkForRef(
      summary,
      'Verification proof',
      firstRef(summary.corpus?.verdictRefs) ??
        firstRef(summary.corpus?.traceRefs) ??
        firstRef(
          summary.realGradient?.closeoutRequirement?.freivaldsCommitmentRefs,
        ),
    )
  }

  if (selection.id === 'receipt' || selection.role === 'receipt') {
    return linkForRef(summary, 'Receipt', firstRef(summary.receiptRefs))
  }

  if (selection.id === 'settlement' || selection.role === 'rung') {
    return metricNumber(summary.metrics?.providerConfirmedSettledPayoutSats) > 0
      ? linkForRef(summary, 'Settlement receipt', firstRef(summary.receiptRefs))
      : null
  }

  return null
}

const isTrainingRunNodeSelection = (
  value: unknown,
): value is TrainingRunNodeSelection => {
  if (typeof value !== 'object' || value === null) {
    return false
  }
  const record = value as Record<string, unknown>
  return (
    typeof record.detail === 'string' &&
    typeof record.id === 'string' &&
    typeof record.label === 'string' &&
    typeof record.role === 'string' &&
    typeof record.status === 'string'
  )
}

const makeClass = (): CustomElementConstructor =>
  class extends HTMLElement {
    #shadow: ShadowRoot | null = null
    #abort: AbortController | null = null

    connectedCallback(): void {
      const shadow = this.shadowRoot ?? this.attachShadow({ mode: 'open' })
      this.#shadow = shadow
      this.#refresh()
    }

    disconnectedCallback(): void {
      this.#abort?.abort()
      this.#abort = null
      this.#shadow?.replaceChildren()
    }

    #refresh(): void {
      this.#abort?.abort()
      this.#renderLoading()
      this.#abort = new AbortController()
      void this.#load(this.#abort.signal)
    }

    async #load(signal: AbortSignal): Promise<void> {
      try {
        const response = await fetch(TASSADAR_RUN_SUMMARY_ENDPOINT, {
          headers: { accept: 'application/json' },
          signal,
        })
        if (signal.aborted) return
        if (!response.ok) {
          this.#renderError(
            `Run summary unavailable (HTTP ${response.status}). The live ` +
              'projection is not reachable right now.',
          )
          return
        }
        const summary = (await response.json()) as TassadarRunPublicSummary
        if (signal.aborted) return
        this.#renderScene(summary, new Date())
      } catch (error) {
        if (signal.aborted) return
        this.#renderError(
          'Could not load the live run summary. The projection endpoint did ' +
            'not respond.',
        )
        // Keep the failure visible to operators without faking any metric.
        void error
      }
    }

    #base(): { shadow: ShadowRoot; mount: HTMLDivElement } | null {
      const shadow = this.#shadow
      if (shadow === null) return null
      shadow.replaceChildren()
      const style = document.createElement('style')
      style.textContent = HOST_STYLE
      const mount = document.createElement('div')
      mount.className = 'mount'
      shadow.append(style, mount)
      return { shadow, mount }
    }

    #renderLoading(): void {
      const base = this.#base()
      if (base === null) return
      this.setAttribute('data-state', 'loading')
      base.mount.append(
        this.#overlay('Live data', 'Loading the live Tassadar run projection…'),
      )
    }

    #renderError(message: string): void {
      const base = this.#base()
      if (base === null) return
      this.setAttribute('data-state', 'error')
      base.mount.append(this.#overlay('Live data — error', message))
    }

    // Receipt-first: idle summaries still render the real (zeroed) scene; we do
    // not substitute placeholder numbers. Only the data-state differs so callers
    // and tests can distinguish a just-launched run from a populated one.
    #renderScene(summary: TassadarRunPublicSummary, fetchedAt: Date): void {
      const base = this.#base()
      if (base === null) return
      this.setAttribute('data-state', dataStateForSummary(summary))
      registerTrainingRunElement()
      const run = document.createElement(trainingRunTagName) as HTMLElement & {
        visualization?: unknown
      }
      run.style.position = 'absolute'
      run.style.inset = '0'
      // The training-run element reads its `visualization` property reactively.
      run.visualization = tassadarRunVisualizationOptions(summary)
      run.addEventListener('node-selected', event => {
        const detail = (event as CustomEvent<unknown>).detail
        if (!isTrainingRunNodeSelection(detail)) return
        const proofLink = proofLinkForSelection(summary, detail)
        this.#renderSelection(base.mount, detail, proofLink)
        if (proofLink !== null) {
          this.#openProofLink(proofLink)
        }
      })
      base.mount.append(run)
      this.#renderStatus(base.mount, summary, fetchedAt)
    }

    #renderStatus(
      mount: HTMLDivElement,
      summary: TassadarRunPublicSummary,
      fetchedAt: Date,
    ): void {
      const panel = document.createElement('aside')
      panel.className = 'status'
      panel.setAttribute('aria-label', 'Live Tassadar snapshot status')
      const list = document.createElement('dl')
      const rows: ReadonlyArray<readonly [string, string]> = [
        ['Run', textOrUnknown(summary.runRef)],
        ['State', textOrUnknown(summary.runState)],
        ['Generated', generatedAtText(summary)],
        ['Staleness', stalenessText(summary)],
        ['Browser fetched', fetchedAt.toISOString()],
      ]
      for (const [label, value] of rows) {
        const item = document.createElement('div')
        const term = document.createElement('dt')
        term.textContent = label
        const detail = document.createElement('dd')
        detail.textContent = value
        item.append(term, detail)
        list.append(item)
      }
      const refresh = document.createElement('button')
      refresh.type = 'button'
      refresh.textContent = 'Refresh snapshot'
      refresh.addEventListener('click', () => this.#refresh())
      panel.append(list, refresh)
      mount.append(panel)
    }

    #renderSelection(
      mount: HTMLDivElement,
      selection: TrainingRunNodeSelection,
      proofLink: TassadarRunProofLink | null,
    ): void {
      mount.querySelector('.selection')?.remove()
      const panel = document.createElement('aside')
      panel.className = 'selection'
      panel.setAttribute(
        'data-proof-state',
        proofLink === null ? 'unlinked' : 'linked',
      )
      const title = document.createElement('strong')
      title.textContent = selection.label
      const detail = document.createElement('p')
      detail.textContent =
        proofLink === null
          ? `${selection.detail}. No public proof ref is linked yet.`
          : `${proofLink.label}: ${proofLink.ref}`
      panel.append(title, detail)
      if (proofLink !== null) {
        const link = document.createElement('a')
        link.href = proofLink.href
        link.target = '_blank'
        link.rel = 'noopener noreferrer'
        link.textContent = 'Open proof'
        panel.append(link)
      }
      mount.append(panel)
    }

    #openProofLink(proofLink: TassadarRunProofLink): void {
      if (typeof window === 'undefined') return
      if (typeof window.open !== 'function') return
      window.open(proofLink.href, '_blank', 'noopener,noreferrer')
    }

    #overlay(label: string, message: string): HTMLDivElement {
      const overlay = document.createElement('div')
      overlay.className = 'overlay'
      const text = document.createElement('p')
      const labelEl = document.createElement('span')
      labelEl.className = 'label'
      labelEl.textContent = label
      text.append(labelEl, document.createTextNode(message))
      overlay.append(text)
      return overlay
    }
  }

const register = (): void => {
  if (typeof customElements === 'undefined') return
  if (typeof HTMLElement === 'undefined') return
  if (customElements.get(TASSADAR_RUN_TAG) !== undefined) return
  customElements.define(TASSADAR_RUN_TAG, makeClass())
}

const element = defineCustomElement({
  events: {},
  properties: {},
  tag: TASSADAR_RUN_TAG,
})

// Foldkit view helper: renders the self-fetching `<oa-tassadar-run>` element.
export const tassadarRunView = <Message>(
  attributes: ReadonlyArray<Attribute<Message>> = [],
): Html => {
  register()
  return element.withMessage<Message>()(attributes, [])
}
