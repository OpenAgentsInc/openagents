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
import type {
  TrainingRunNodeSelection,
  TrainingRunVisualizationOptions,
} from '@openagentsinc/three-effect/core'
import {
  registerTrainingRunElement,
  trainingRunTagName,
} from '@openagentsinc/three-effect/foldkit'
import { define as defineCustomElement } from 'foldkit/customElement'
import type { Attribute, Html } from 'foldkit/html'

import {
  type PublicTassadarSettlementRow,
  type TassadarRunPublicSummary,
  tassadarRunVisualizationOptions,
} from './tassadarRunSnapshot'

export const TASSADAR_RUN_TAG = 'oa-tassadar-run'
export const TASSADAR_RUN_SUMMARY_ENDPOINT = '/api/public/tassadar-run-summary'
const TASSADAR_ENTER_WORLD_SELECTOR = '[data-tassadar-enter-world]'

export type TassadarRunDataState = 'loading' | 'ok' | 'empty' | 'error'
export type TassadarRunProofLink = Readonly<{
  caveats: ReadonlyArray<string>
  href: string
  kind: string
  label: string
  ref: string
  sourceRefs: ReadonlyArray<string>
  state: string
}>

const HOST_STYLE =
  ':host{position:absolute;inset:0;display:block;background:#000;color:#f1efe8}' +
  '.mount{position:absolute;inset:0}' +
  '.overlay{position:absolute;inset:0;display:flex;align-items:center;justify-content:center;' +
  'padding:2rem;text-align:center;font-family:ui-monospace,SFMono-Regular,Menlo,monospace;pointer-events:none}' +
  '.overlay p{margin:0;max-width:48ch;border:1px solid rgba(255,255,255,0.1);background:rgba(0,0,0,0.34);padding:0.8rem 1rem;font-size:0.95rem;line-height:1.6;color:rgba(241,239,232,0.64);backdrop-filter:blur(12px)}' +
  '.overlay .label{display:block;margin-bottom:0.4rem;font-size:0.7rem;letter-spacing:0.08em;' +
  'text-transform:uppercase;color:rgba(241,239,232,0.35)}' +
  '.walk-controls{position:absolute;left:1rem;bottom:1rem;z-index:3;display:grid;max-width:min(14rem,calc(100% - 2rem));' +
  'border:1px solid rgba(255,255,255,0.12);background:rgba(0,0,0,0.28);padding:0.55rem 0.6rem;' +
  'font-family:ui-monospace,SFMono-Regular,Menlo,monospace;color:#f1efe8;backdrop-filter:blur(14px);box-shadow:0 0.75rem 2rem rgba(0,0,0,0.24);transition:opacity 140ms ease}' +
  '.walk-controls button{min-height:2rem;border:1px solid rgba(255,255,255,0.2);background:rgba(255,255,255,0.08);padding:0 0.7rem;' +
  'font:inherit;font-size:0.72rem;color:rgba(255,255,255,0.88);cursor:pointer;text-align:left}' +
  '.walk-controls button:focus-visible{outline:2px solid rgba(114,191,255,0.9);outline-offset:2px}' +
  ':host([data-pointer-lock="locked"]) .walk-controls{opacity:0.22;pointer-events:none}' +
  '.status{position:absolute;top:0.75rem;left:0.75rem;right:0.75rem;z-index:3;display:flex;align-items:flex-start;' +
  'justify-content:space-between;gap:0.75rem;border:1px solid rgba(255,255,255,0.12);background:rgba(0,0,0,0.34);' +
  'padding:0.65rem 0.75rem;font-family:ui-monospace,SFMono-Regular,Menlo,monospace;color:#f1efe8;backdrop-filter:blur(14px);box-shadow:0 0.75rem 2rem rgba(0,0,0,0.24)}' +
  '.status dl{display:grid;grid-template-columns:repeat(5,minmax(0,auto));gap:0.55rem 1rem;margin:0;min-width:0}' +
  '.status div{min-width:0}.status dt{margin:0 0 0.18rem;font-size:0.6rem;letter-spacing:0.08em;text-transform:uppercase;color:rgba(255,255,255,0.35)}' +
  '.status dd{margin:0;max-width:18rem;overflow:hidden;text-overflow:ellipsis;white-space:nowrap;font-size:0.72rem;color:rgba(255,255,255,0.78)}' +
  '.status button{min-height:2rem;border:1px solid rgba(255,255,255,0.18);background:rgba(255,255,255,0.06);padding:0 0.65rem;' +
  'font:inherit;font-size:0.7rem;color:rgba(255,255,255,0.82);cursor:pointer}' +
  '.status button:focus-visible{outline:2px solid rgba(114,191,255,0.9);outline-offset:2px}' +
  '@media (max-width:720px){.status{display:grid}.status dl{grid-template-columns:repeat(2,minmax(0,1fr))}.status dd{max-width:none}.status button{justify-self:start}}' +
  '.selection{position:absolute;right:1rem;bottom:1rem;z-index:2;max-width:min(26rem,calc(100% - 2rem));' +
  'border:1px solid rgba(255,255,255,0.12);background:rgba(0,0,0,0.42);padding:0.75rem 0.875rem;' +
  'font-family:ui-monospace,SFMono-Regular,Menlo,monospace;color:#f1efe8;backdrop-filter:blur(14px);box-shadow:0 0.75rem 2rem rgba(0,0,0,0.28)}' +
  '.selection strong{display:block;margin-bottom:0.25rem;font-size:0.78rem;font-weight:600;color:rgba(255,255,255,0.88)}' +
  '.selection p{margin:0;font-size:0.72rem;line-height:1.45;color:rgba(255,255,255,0.55)}' +
  '.selection dl{display:grid;gap:0.45rem;margin:0.55rem 0 0}.selection dt{margin:0 0 0.12rem;font-size:0.58rem;letter-spacing:0.08em;text-transform:uppercase;color:rgba(255,255,255,0.35)}' +
  '.selection dd{margin:0;overflow-wrap:anywhere;font-size:0.7rem;line-height:1.4;color:rgba(255,255,255,0.66)}' +
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

const isNexusPylonReceiptRef = (ref: string): boolean =>
  ref.startsWith('receipt.nexus.') ||
  ref.startsWith('receipt.nexus_') ||
  ref.startsWith('receipt.nexus-pylon.')

const nexusPylonReceiptHref = (ref: string): string =>
  `/api/public/nexus-pylon/receipts/${encodeURIComponent(ref)}`

const receiptHref = (ref: string): string =>
  isNexusPylonReceiptRef(ref)
    ? nexusPylonReceiptHref(ref)
    : `/api/forum/receipts/${encodeURIComponent(ref)}`

const firstRef = (
  refs: ReadonlyArray<string> | undefined,
): string | undefined =>
  Array.isArray(refs)
    ? refs.map(ref => ref.trim()).find(ref => ref.length > 0)
    : undefined

const publicRefs = (
  refs: ReadonlyArray<string> | undefined,
): ReadonlyArray<string> =>
  Array.isArray(refs)
    ? refs.map(ref => ref.trim()).filter(ref => ref.length > 0)
    : []

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

const settlementReceiptRef = (
  row: PublicTassadarSettlementRow,
): string | undefined => {
  const ref = row.receiptRef?.trim()
  return ref === undefined || ref === '' ? undefined : ref
}

const settlementRows = (
  summary: TassadarRunPublicSummary,
): ReadonlyArray<PublicTassadarSettlementRow> =>
  Array.isArray(summary.settlementRows) ? summary.settlementRows : []

const firstSettlementRow = (
  summary: TassadarRunPublicSummary,
): PublicTassadarSettlementRow | undefined =>
  settlementRows(summary).find(row => settlementReceiptRef(row) !== undefined)

const settlementRowForRef = (
  summary: TassadarRunPublicSummary,
  ref: string,
): PublicTassadarSettlementRow | undefined =>
  settlementRows(summary).find(row => settlementReceiptRef(row) === ref)

const proofDetail = (
  input: Readonly<{
    caveats?: ReadonlyArray<string>
    href: string
    kind: string
    label: string
    ref: string
    sourceRefs?: ReadonlyArray<string>
    state?: string
  }>,
): TassadarRunProofLink => ({
  caveats: publicRefs(input.caveats),
  href: input.href,
  kind: input.kind,
  label: input.label,
  ref: input.ref,
  sourceRefs: publicRefs(input.sourceRefs),
  state: input.state ?? 'linked',
})

const settlementProofDetail = (
  row: PublicTassadarSettlementRow,
): TassadarRunProofLink | null => {
  const ref = settlementReceiptRef(row)
  if (ref === undefined) return null
  const movementMode = row.movementMode ?? 'unknown'
  const state = row.state ?? 'unknown'
  const realBitcoinMoved = row.realBitcoinMoved === true
  const amount =
    typeof row.amountSats === 'number' && Number.isFinite(row.amountSats)
      ? `${row.amountSats} sats`
      : 'amount unknown'

  return proofDetail({
    caveats: [
      `Amount: ${amount}`,
      realBitcoinMoved
        ? 'Receipt claims real Bitcoin movement.'
        : 'Simulation-backed settlement record; this does not prove real Bitcoin moved.',
    ],
    href: row.apiUrl ?? nexusPylonReceiptHref(ref),
    kind: row.receiptKind ?? 'settlement_recorded',
    label: 'Settlement receipt',
    ref,
    sourceRefs: row.sourceRefs ?? [],
    state: `${state}; ${movementMode}; real bitcoin moved: ${
      realBitcoinMoved ? 'yes' : 'no'
    }`,
  })
}

const linkForRef = (
  summary: TassadarRunPublicSummary,
  label: string,
  ref: string | undefined,
): TassadarRunProofLink | null =>
  ref === undefined
    ? null
    : ref.startsWith('receipt.')
      ? settlementRowForRef(summary, ref) === undefined
        ? proofDetail({
            href: receiptHref(ref),
            kind: isNexusPylonReceiptRef(ref)
              ? 'nexus_pylon_receipt'
              : 'forum_receipt',
            label,
            ref,
            state: 'linked',
          })
        : settlementProofDetail(settlementRowForRef(summary, ref)!)
      : proofDetail({
          href: focusedTrainingRunHref(summary, ref),
          kind: 'training_ref',
          label,
          ref,
          state: 'linked',
        })

const replayPairForSelection = (
  summary: TassadarRunPublicSummary,
  selection: TrainingRunNodeSelection,
) =>
  summary.realGradient?.verifiedReplayPairs?.find(
    pair =>
      pair.workerRef === selection.id || pair.validatorRef === selection.id,
  )

const rejectedReplayPairForSelection = (
  summary: TassadarRunPublicSummary,
  selection: TrainingRunNodeSelection,
) =>
  summary.realGradient?.rejectedReplayPairs?.find(
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

const corpusTraceForSelection = (
  summary: TassadarRunPublicSummary,
  selection: TrainingRunNodeSelection,
): string | undefined =>
  publicRefs(summary.corpus?.traceRefs).find(ref => ref === selection.id)

export const proofLinkForSelection = (
  summary: TassadarRunPublicSummary,
  selection: TrainingRunNodeSelection,
): TassadarRunProofLink | null => {
  const settlement = settlementRowForRef(summary, selection.id)
  if (settlement !== undefined) {
    return settlementProofDetail(settlement)
  }

  const pair = replayPairForSelection(summary, selection)
  if (pair !== undefined) {
    return linkForRef(summary, 'Verified replay challenge', pair.challengeRef)
  }

  const rejectedPair = rejectedReplayPairForSelection(summary, selection)
  if (rejectedPair !== undefined) {
    return linkForRef(
      summary,
      'Rejected replay challenge',
      rejectedPair.challengeRef,
    )
  }

  const row = leaderboardRowForSelection(summary, selection)
  if (row !== undefined) {
    const contributorSettlement = settlementRows(summary).find(
      settlementRow => settlementRow.contributorRef === row.pylonRef,
    )
    if (contributorSettlement !== undefined) {
      return settlementProofDetail(contributorSettlement)
    }
    return linkForRef(summary, 'Pylon evidence', firstRef(row.sourceRefs))
  }

  const corpusTraceRef = corpusTraceForSelection(summary, selection)
  if (corpusTraceRef !== undefined) {
    return linkForRef(summary, 'Accepted trace corpus ref', corpusTraceRef)
  }

  if (selection.id === 'run') {
    return proofDetail({
      href: publicTrainingRunHref(summary),
      kind: 'training_run',
      label: 'Public run projection',
      ref: summary.runRef ?? 'run.tassadar.executor.20260615',
      state: summary.runState ?? 'unknown',
    })
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
    const settlement = firstSettlementRow(summary)
    if (settlement !== undefined) {
      return settlementProofDetail(settlement)
    }
    return linkForRef(summary, 'Receipt', firstRef(summary.receiptRefs))
  }

  if (selection.id === 'settlement' || selection.role === 'rung') {
    const settlement = firstSettlementRow(summary)
    if (settlement !== undefined) {
      return settlementProofDetail(settlement)
    }
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
        this.#renderScene(summary)
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
    #renderScene(summary: TassadarRunPublicSummary): void {
      const base = this.#base()
      if (base === null) return
      this.setAttribute('data-state', dataStateForSummary(summary))
      this.setAttribute('data-pointer-lock', 'released')
      registerTrainingRunElement()
      this.#renderWalkControls(base.mount)
      const run = document.createElement(trainingRunTagName) as HTMLElement & {
        visualization?: unknown
      }
      run.style.position = 'absolute'
      run.style.inset = '0'
      // The training-run element reads its `visualization` property reactively.
      const visualization: TrainingRunVisualizationOptions = {
        ...tassadarRunVisualizationOptions(summary),
        cameraMode: 'perspective_walk',
        controller: 'wasd_mouselook',
        walkController: {
          bounds: { minX: -8, maxX: 8, minZ: -8, maxZ: 8 },
          eyeHeight: 1.65,
          lockSelector: TASSADAR_ENTER_WORLD_SELECTOR,
          movementSpeed: 4.5,
          sprintMultiplier: 1.8,
          onLockChange: locked => {
            this.setAttribute(
              'data-pointer-lock',
              locked ? 'locked' : 'released',
            )
          },
        },
      }
      run.visualization = visualization
      run.addEventListener('node-selected', event => {
        const detail = (event as CustomEvent<unknown>).detail
        if (!isTrainingRunNodeSelection(detail)) return
        const proofLink = proofLinkForSelection(summary, detail)
        this.#renderSelection(base.mount, detail, proofLink)
      })
      base.mount.append(run)
      this.#renderStatus(base.mount, summary)
    }

    #renderWalkControls(mount: HTMLDivElement): void {
      mount.querySelector('.walk-controls')?.remove()
      const panel = document.createElement('aside')
      panel.className = 'walk-controls'
      panel.setAttribute('aria-label', 'Tassadar run navigation')
      const enter = document.createElement('button')
      enter.type = 'button'
      enter.setAttribute('data-tassadar-enter-world', '')
      enter.textContent = 'Enter run'
      panel.append(enter)
      mount.append(panel)
    }

    #renderStatus(
      mount: HTMLDivElement,
      summary: TassadarRunPublicSummary,
    ): void {
      const panel = document.createElement('aside')
      panel.className = 'status'
      panel.setAttribute('aria-label', 'Live Tassadar snapshot status')
      const list = document.createElement('dl')
      const rows: ReadonlyArray<readonly [string, string]> = [
        ['Run', textOrUnknown(summary.runRef)],
        ['State', textOrUnknown(summary.runState)],
        ['Updated', generatedAtText(summary)],
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
          : `${proofLink.label}`
      panel.append(title, detail)
      if (proofLink !== null) {
        const list = document.createElement('dl')
        const rows: ReadonlyArray<readonly [string, string]> = [
          ['Kind', proofLink.kind],
          ['State', proofLink.state],
          ['Ref', proofLink.ref],
          ['Route', proofLink.href],
          [
            'Caveats',
            proofLink.caveats.length === 0
              ? 'none'
              : proofLink.caveats.join(' | '),
          ],
          [
            'Source refs',
            proofLink.sourceRefs.length === 0
              ? 'none'
              : proofLink.sourceRefs.slice(0, 6).join(' | '),
          ],
        ]
        for (const [term, value] of rows) {
          const item = document.createElement('div')
          const dt = document.createElement('dt')
          dt.textContent = term
          const dd = document.createElement('dd')
          dd.textContent = value
          item.append(dt, dd)
          list.append(item)
        }
        const link = document.createElement('a')
        link.href = proofLink.href
        link.target = '_blank'
        link.rel = 'noopener noreferrer'
        link.textContent = 'Open proof'
        panel.append(list, link)
      }
      mount.append(panel)
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
