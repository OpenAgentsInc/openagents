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

import { define as defineCustomElement } from 'foldkit/customElement'
import {
  registerTrainingRunElement,
  trainingRunTagName,
} from '@openagentsinc/three-effect/foldkit'
import type { Attribute, Html } from 'foldkit/html'

import {
  type TassadarRunPublicSummary,
  tassadarRunVisualizationOptions,
} from './tassadarRunSnapshot'

export const TASSADAR_RUN_TAG = 'oa-tassadar-run'
export const TASSADAR_RUN_SUMMARY_ENDPOINT = '/api/public/tassadar-run-summary'

export type TassadarRunDataState = 'loading' | 'ok' | 'empty' | 'error'

const HOST_STYLE =
  ':host{position:absolute;inset:0;display:block;background:#000;color:#f1efe8}' +
  '.mount{position:absolute;inset:0}' +
  '.overlay{position:absolute;inset:0;display:flex;align-items:center;justify-content:center;' +
  'padding:2rem;text-align:center;font-family:ui-monospace,SFMono-Regular,Menlo,monospace}' +
  '.overlay p{margin:0;max-width:48ch;font-size:0.95rem;line-height:1.6;color:rgba(241,239,232,0.6)}' +
  '.overlay .label{display:block;margin-bottom:0.4rem;font-size:0.7rem;letter-spacing:0.08em;' +
  'text-transform:uppercase;color:rgba(241,239,232,0.35)}'

const isIdle = (summary: TassadarRunPublicSummary): boolean =>
  summary.emptyState?.idle === true

// Pure: classify a fetched summary into the data-state the scene should show.
// Idle → 'empty' (still renders the honest planned/zeroed scene). Otherwise 'ok'.
export const dataStateForSummary = (
  summary: TassadarRunPublicSummary,
): Exclude<TassadarRunDataState, 'loading' | 'error'> =>
  isIdle(summary) ? 'empty' : 'ok'

const makeClass = (): CustomElementConstructor =>
  class extends HTMLElement {
    #shadow: ShadowRoot | null = null
    #abort: AbortController | null = null

    connectedCallback(): void {
      const shadow = this.shadowRoot ?? this.attachShadow({ mode: 'open' })
      this.#shadow = shadow
      this.#renderLoading()
      this.#abort = new AbortController()
      void this.#load(this.#abort.signal)
    }

    disconnectedCallback(): void {
      this.#abort?.abort()
      this.#abort = null
      this.#shadow?.replaceChildren()
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
      registerTrainingRunElement()
      const run = document.createElement(trainingRunTagName) as HTMLElement & {
        visualization?: unknown
      }
      run.style.position = 'absolute'
      run.style.inset = '0'
      // The training-run element reads its `visualization` property reactively.
      run.visualization = tassadarRunVisualizationOptions(summary)
      base.mount.append(run)
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
