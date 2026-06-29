// Gym — GPT-OSS playground controller element (#6167).
//
// A self-contained custom element that owns the interactive `/gym/oss` surface:
// the prompt input (presets + custom), the N-samples and concurrency dials, the
// optional ramp toggle, the Run button, and the live result cards / aggregate
// table / scene. It runs entirely client-side against the streaming gateway via
// the PURE runner (`runner.ts`) + live stream seam (`stream.ts`), so the giant
// logged-in TEA model is untouched. The route that mounts it is auth/owner-gated
// (a logged-in route); the runner enforces the hard in-flight cap.
//
// The PURE form/runner/rendering helpers below are unit-tested offline; the
// element wiring is thin DOM plumbing over them.

import { define as defineCustomElement } from 'foldkit/customElement'
import type { Attribute, Html } from 'foldkit/html'

import {
  registerGymOssSceneElement,
  type SceneFrame,
  type SceneRequest,
} from '../../../scene/gymOssSceneElement'
import {
  CONCURRENCY_OPTIONS,
  GPT_OSS_MODEL_ID,
  MAX_IN_FLIGHT,
  aggregateSamples,
  clampConcurrency,
  formatMeasured,
  formatSummaryNumber,
  isMeasured,
  rampSweepSteps,
  runConcurrent,
  runRampSweep,
  type RampStepResult,
  type SampleAggregate,
  type SampleResult,
  type SampleStream,
} from './runner'
import { liveSampleStream } from './stream'

export const GYM_OSS_CONTROLLER_TAG = 'oa-gym-oss-controller'

export const PROMPT_PRESETS: ReadonlyArray<{ id: string; label: string; prompt: string }> = [
  {
    id: 'short',
    label: 'Short answer',
    prompt: 'In one sentence, what is a Lightning Network payment channel?',
  },
  {
    id: 'reasoning',
    label: 'Step reasoning',
    prompt: 'Explain how to compute a P99 latency from a list of samples, step by step.',
  },
  {
    id: 'code',
    label: 'Code sketch',
    prompt: 'Write a small TypeScript function that returns the nearest-rank percentile of an array.',
  },
]

// ---------------------------------------------------------------------------
// Form model + clamping (pure, tested).
// ---------------------------------------------------------------------------

export type GymOssForm = Readonly<{
  prompt: string
  samples: number
  concurrency: number
  ramp: boolean
}>

export const DEFAULT_FORM: GymOssForm = {
  prompt: PROMPT_PRESETS[0]?.prompt ?? '',
  samples: 5,
  concurrency: 4,
  ramp: false,
}

// Clamp the sample count into a sane, percentile-readable bound. >=1; capped so
// a ramp can't queue an unbounded amount.
export const clampSamples = (requested: number): number =>
  Math.min(64, Math.max(1, Math.trunc(requested)))

export const normalizeForm = (form: GymOssForm): GymOssForm => ({
  prompt: form.prompt,
  samples: clampSamples(form.samples),
  concurrency: clampConcurrency(form.concurrency),
  ramp: form.ramp,
})

// The plan a Run produces: either a single run at a concurrency, or a ramp sweep
// of concurrency steps. PURE description of WHAT will run (used for the offline
// runner tests + the live element).
export type RunPlan =
  | Readonly<{ kind: 'single'; samples: number; concurrency: number }>
  | Readonly<{ kind: 'ramp'; samples: number; steps: ReadonlyArray<number> }>

export const planRun = (form: GymOssForm): RunPlan => {
  const normalized = normalizeForm(form)
  if (normalized.ramp) {
    return {
      kind: 'ramp',
      samples: normalized.samples,
      steps: rampSweepSteps(normalized.concurrency),
    }
  }
  return {
    kind: 'single',
    samples: normalized.samples,
    concurrency: normalized.concurrency,
  }
}

// ---------------------------------------------------------------------------
// Run execution over an injectable stream (so the element is testable offline).
// ---------------------------------------------------------------------------

export type RunOutcome =
  | Readonly<{ kind: 'single'; samples: ReadonlyArray<SampleResult>; aggregate: SampleAggregate }>
  | Readonly<{ kind: 'ramp'; steps: ReadonlyArray<RampStepResult> }>

// Execute a plan over a SampleStream. PURE w.r.t. scheduling (every sample
// resolves). Used by the element with the live stream and by tests with a fake.
export const executeRun = async (
  plan: RunPlan,
  stream: SampleStream,
  signal?: AbortSignal,
): Promise<RunOutcome> => {
  if (plan.kind === 'ramp') {
    const steps = await runRampSweep({
      samples: plan.samples,
      // The top of the ramp is the largest step.
      topConcurrency: plan.steps[plan.steps.length - 1] ?? 1,
      stream,
      ...(signal === undefined ? {} : { signal }),
    })
    return { kind: 'ramp', steps }
  }
  const samples = await runConcurrent({
    samples: plan.samples,
    concurrency: plan.concurrency,
    stream,
    ...(signal === undefined ? {} : { signal }),
  })
  return { kind: 'single', samples, aggregate: aggregateSamples(samples) }
}

// ---------------------------------------------------------------------------
// Live scene frame from in-flight samples (pure).
// ---------------------------------------------------------------------------

export const sceneFrameForSamples = (
  samples: ReadonlyArray<SampleResult | undefined>,
  total: number,
): SceneFrame => {
  const requests: Array<SceneRequest> = []
  for (let index = 0; index < total; index += 1) {
    const sample = samples[index]
    if (sample === undefined) {
      requests.push({ index, status: 'running', perceivedTps: null })
    } else {
      requests.push({
        index,
        status: sample.status,
        perceivedTps: isMeasured(sample.perceivedTps) ? sample.perceivedTps : null,
      })
    }
  }
  const measured = requests
    .map(request => request.perceivedTps)
    .filter((value): value is number => value !== null)
  return {
    requests,
    aggregateTps: measured.length === 0 ? null : measured.reduce((s, v) => s + v, 0),
  }
}

// ---------------------------------------------------------------------------
// DOM rendering helpers (pure HTML strings; the element fills the shadow root).
// ---------------------------------------------------------------------------

const esc = (value: string): string =>
  value
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;')

const summaryRow = (label: string, summary: ReturnType<typeof aggregateSamples>['ttftMs'], unit: string, digits: number): string =>
  `<tr>
    <td class="metric-label">${esc(label)}</td>
    <td>${esc(formatSummaryNumber(summary.p50, { unit, digits }))}</td>
    <td>${esc(formatSummaryNumber(summary.p90, { unit, digits }))}</td>
    <td>${esc(formatSummaryNumber(summary.p99, { unit, digits }))}</td>
    <td>${esc(formatSummaryNumber(summary.mean, { unit, digits }))}</td>
    <td>${summary.sampleCount}</td>
  </tr>`

export const renderAggregateTable = (aggregate: SampleAggregate): string =>
  `<table class="agg">
    <thead>
      <tr><th>Metric</th><th>P50</th><th>P90</th><th>P99</th><th>Mean</th><th>n</th></tr>
    </thead>
    <tbody>
      ${summaryRow('TTFT', aggregate.ttftMs, 'ms', 0)}
      ${summaryRow('Tokens/sec', aggregate.perceivedTps, 'tok/s', 1)}
      ${summaryRow('Inter-token latency', aggregate.interTokenLatencyMs, 'ms', 1)}
      ${summaryRow('Total wall-clock', aggregate.totalWallClockMs, 'ms', 0)}
      ${summaryRow('Completion tokens', aggregate.completionTokens, '', 0)}
    </tbody>
  </table>
  <p class="agg-throughput">Aggregate throughput: ${esc(
    aggregate.aggregateTps === null
      ? 'not measured'
      : `${aggregate.aggregateTps.toFixed(1)} tok/s across ${aggregate.okSamples} samples`,
  )}</p>
  <p class="agg-fail">${aggregate.failedSamples > 0 ? `${aggregate.failedSamples} sample(s) failed (shown as failures, not latency).` : 'No failed samples.'}</p>`

export const renderSampleCard = (sample: SampleResult): string => {
  const failed = sample.status === 'failed'
  return `<div class="card ${failed ? 'card-failed' : ''}">
    <div class="card-head">#${sample.index + 1} ${failed ? 'FAILED' : 'OK'}</div>
    ${
      failed
        ? `<div class="card-error">${esc(sample.error ?? 'failed')}</div>`
        : `<div class="card-metric">TTFT ${esc(formatMeasured(sample.ttftMs, { unit: 'ms' }))} <span class="src">(${sample.source.ttft})</span></div>
           <div class="card-metric">TPS ${esc(formatMeasured(sample.perceivedTps, { unit: 'tok/s', digits: 1 }))} <span class="src">(${sample.source.tps})</span></div>
           <div class="card-metric">ITL ${esc(formatMeasured(sample.interTokenLatencyMs, { unit: 'ms', digits: 1 }))}</div>
           <div class="card-metric">Wall ${esc(formatMeasured(sample.totalWallClockMs, { unit: 'ms' }))} <span class="src">(${sample.source.totalWallClock})</span></div>`
    }
  </div>`
}

export const renderRampChart = (steps: ReadonlyArray<RampStepResult>): string => {
  const rows = steps
    .map(step => {
      const ttft = formatSummaryNumber(step.aggregate.ttftMs.p50, { unit: 'ms', digits: 0 })
      const tps = formatSummaryNumber(step.aggregate.perceivedTps.p50, { unit: 'tok/s', digits: 1 })
      const agg =
        step.aggregate.aggregateTps === null
          ? 'not measured'
          : `${step.aggregate.aggregateTps.toFixed(1)} tok/s`
      return `<tr><td>c=${step.concurrency}</td><td>${esc(ttft)}</td><td>${esc(tps)}</td><td>${esc(agg)}</td></tr>`
    })
    .join('')
  return `<table class="ramp">
    <thead><tr><th>Concurrency</th><th>TTFT P50</th><th>TPS P50</th><th>Aggregate</th></tr></thead>
    <tbody>${rows}</tbody>
  </table>`
}

// ---------------------------------------------------------------------------
// Element wiring.
// ---------------------------------------------------------------------------

const controllerCss = `
:host { display: block; font-family: 'Berkeley Mono', ui-monospace, monospace; color: #d2dbe6; }
.panel { border: 1px solid rgba(58,123,255,0.18); border-radius: 12px; background: rgba(10,14,20,0.92); padding: 1rem; margin-top: 1rem; }
.field { display: grid; gap: 0.35rem; margin-bottom: 0.75rem; }
.field label { font-size: 0.7rem; text-transform: uppercase; letter-spacing: 0.16em; color: #8fb6ff; }
textarea, select { width: 100%; background: #03050a; border: 1px solid rgba(58,123,255,0.25); color: #f1efe8; font-family: inherit; padding: 0.5rem; border-radius: 8px; }
textarea { min-height: 4rem; resize: vertical; }
.row { display: flex; gap: 0.75rem; flex-wrap: wrap; align-items: end; }
.presets { display: flex; gap: 0.5rem; flex-wrap: wrap; margin-bottom: 0.75rem; }
button { background: rgba(58,123,255,0.18); border: 1px solid rgba(79,208,255,0.45); color: #bcd4ff; font-family: inherit; padding: 0.45rem 0.9rem; border-radius: 999px; cursor: pointer; text-transform: uppercase; letter-spacing: 0.12em; font-size: 0.72rem; }
button[disabled] { opacity: 0.5; cursor: not-allowed; }
.run-btn { background: rgba(79,208,255,0.22); }
.cards { display: grid; grid-template-columns: repeat(auto-fill, minmax(150px, 1fr)); gap: 0.5rem; margin-top: 1rem; }
.card { border: 1px solid rgba(58,123,255,0.2); border-radius: 8px; padding: 0.5rem; background: rgba(5,7,12,0.8); font-size: 0.72rem; }
.card-failed { border-color: rgba(255,122,122,0.5); }
.card-head { color: #4fd0ff; font-weight: 600; margin-bottom: 0.3rem; }
.card-failed .card-head { color: #ff9c9c; }
.card-error { color: #ff9c9c; }
.src { color: #6a82a8; font-size: 0.62rem; }
table { width: 100%; border-collapse: collapse; margin-top: 0.75rem; font-size: 0.72rem; }
th, td { text-align: left; padding: 0.3rem 0.5rem; border-bottom: 1px solid rgba(58,123,255,0.12); }
th { color: #8fb6ff; text-transform: uppercase; letter-spacing: 0.1em; font-size: 0.62rem; }
.metric-label { color: #bcd4ff; }
.agg-throughput { color: #7cf0ff; margin-top: 0.5rem; font-size: 0.78rem; }
.agg-fail { color: #8fb6ff; font-size: 0.72rem; }
.status { color: #8fb6ff; font-size: 0.72rem; margin-top: 0.5rem; }
.lane { font-size: 0.62rem; color: #6a82a8; letter-spacing: 0.12em; text-transform: uppercase; }
`

const makeControllerElement = (): CustomElementConstructor =>
  class GymOssControllerElement extends HTMLElement {
    #form: GymOssForm = DEFAULT_FORM
    #running = false
    #abort: AbortController | null = null
    #streamFactory: (form: GymOssForm) => SampleStream = form =>
      liveSampleStream({ prompt: form.prompt })

    // Test seam: inject a fake stream factory so the element can run offline.
    set streamFactory(value: unknown) {
      if (typeof value === 'function') {
        this.#streamFactory = value as (form: GymOssForm) => SampleStream
      }
    }

    connectedCallback(): void {
      // Ensure the nested live scene element is defined before we render it into
      // the shadow root so its `frame` property accepts pushed frames.
      registerGymOssSceneElement()
      const shadow = this.shadowRoot ?? this.attachShadow({ mode: 'open' })
      shadow.replaceChildren()
      const style = document.createElement('style')
      style.textContent = controllerCss
      const root = document.createElement('div')
      root.setAttribute('data-component', 'gym-oss-controller')
      shadow.append(style, root)
      this.#renderForm(root)
    }

    disconnectedCallback(): void {
      this.#abort?.abort()
    }

    #renderForm(root: HTMLElement): void {
      const presets = PROMPT_PRESETS.map(
        preset =>
          `<button type="button" data-preset="${preset.id}">${esc(preset.label)}</button>`,
      ).join('')
      const concurrencyOptions = CONCURRENCY_OPTIONS.map(
        value =>
          `<option value="${value}" ${value === this.#form.concurrency ? 'selected' : ''}>${value}</option>`,
      ).join('')
      root.innerHTML = `
        <div class="panel">
          <div class="lane">Lane: ${esc(GPT_OSS_MODEL_ID)} (hourly · no per-call balance gate · cap ${MAX_IN_FLIGHT} in flight)</div>
          <div class="presets">${presets}</div>
          <div class="field">
            <label for="oss-prompt">Prompt</label>
            <textarea id="oss-prompt" data-field="prompt">${esc(this.#form.prompt)}</textarea>
          </div>
          <div class="row">
            <div class="field">
              <label for="oss-samples">Samples</label>
              <input id="oss-samples" type="number" min="1" max="64" value="${this.#form.samples}" data-field="samples" />
            </div>
            <div class="field">
              <label for="oss-concurrency">Concurrency</label>
              <select id="oss-concurrency" data-field="concurrency">${concurrencyOptions}</select>
            </div>
            <div class="field">
              <label for="oss-ramp">Ramp 1→2→4→8</label>
              <input id="oss-ramp" type="checkbox" data-field="ramp" ${this.#form.ramp ? 'checked' : ''} />
            </div>
            <button type="button" class="run-btn" data-action="run">Run</button>
          </div>
          <oa-gym-oss-scene data-role="scene"></oa-gym-oss-scene>
          <div class="status" data-role="status">Ready.</div>
          <div data-role="results"></div>
        </div>
      `
      // The nested scene element manages its own canvas; we push frames via its
      // `frame` property after a run completes.
      this.#wireForm(root)
    }

    #wireForm(root: HTMLElement): void {
      root.querySelectorAll<HTMLButtonElement>('[data-preset]').forEach(button => {
        button.addEventListener('click', () => {
          const preset = PROMPT_PRESETS.find(p => p.id === button.dataset.preset)
          if (preset !== undefined) {
            this.#form = { ...this.#form, prompt: preset.prompt }
            const textarea = root.querySelector<HTMLTextAreaElement>('[data-field="prompt"]')
            if (textarea !== null) {
              textarea.value = preset.prompt
            }
          }
        })
      })
      root.querySelector<HTMLTextAreaElement>('[data-field="prompt"]')?.addEventListener('input', event => {
        this.#form = { ...this.#form, prompt: (event.target as HTMLTextAreaElement).value }
      })
      root.querySelector<HTMLInputElement>('[data-field="samples"]')?.addEventListener('input', event => {
        this.#form = { ...this.#form, samples: Number((event.target as HTMLInputElement).value) }
      })
      root.querySelector<HTMLSelectElement>('[data-field="concurrency"]')?.addEventListener('change', event => {
        this.#form = { ...this.#form, concurrency: Number((event.target as HTMLSelectElement).value) }
      })
      root.querySelector<HTMLInputElement>('[data-field="ramp"]')?.addEventListener('change', event => {
        this.#form = { ...this.#form, ramp: (event.target as HTMLInputElement).checked }
      })
      root.querySelector<HTMLButtonElement>('[data-action="run"]')?.addEventListener('click', () => {
        void this.#run(root)
      })
    }

    async #run(root: HTMLElement): Promise<void> {
      if (this.#running) {
        return
      }
      this.#running = true
      const runButton = root.querySelector<HTMLButtonElement>('[data-action="run"]')
      const status = root.querySelector<HTMLElement>('[data-role="status"]')
      const results = root.querySelector<HTMLElement>('[data-role="results"]')
      if (runButton !== null) {
        runButton.disabled = true
      }
      if (status !== null) {
        status.textContent = 'Running…'
      }
      this.#abort = new AbortController()
      const plan = planRun(this.#form)
      const stream = this.#streamFactory(normalizeForm(this.#form))
      try {
        const outcome = await executeRun(plan, stream, this.#abort.signal)
        if (results !== null) {
          results.innerHTML = this.#renderOutcome(outcome)
        }
        if (status !== null) {
          status.textContent = 'Done.'
        }
        this.#pushFinalScene(root, outcome)
      } catch (cause) {
        if (status !== null) {
          status.textContent = `Run error: ${cause instanceof Error ? cause.message : 'unknown'}`
        }
      } finally {
        this.#running = false
        if (runButton !== null) {
          runButton.disabled = false
        }
      }
    }

    #pushFinalScene(root: HTMLElement, outcome: RunOutcome): void {
      const sceneEl = root.querySelector('[data-role="scene"]') as
        | (HTMLElement & { frame?: SceneFrame })
        | null
      if (sceneEl === null) {
        return
      }
      const samples =
        outcome.kind === 'single'
          ? outcome.samples
          : (outcome.steps[outcome.steps.length - 1]?.samples ?? [])
      sceneEl.frame = sceneFrameForSamples(samples, samples.length)
    }

    #renderOutcome(outcome: RunOutcome): string {
      if (outcome.kind === 'ramp') {
        const cards = (outcome.steps[outcome.steps.length - 1]?.samples ?? [])
          .map(renderSampleCard)
          .join('')
        return `${renderRampChart(outcome.steps)}<div class="cards">${cards}</div>`
      }
      const cards = outcome.samples.map(renderSampleCard).join('')
      return `${renderAggregateTable(outcome.aggregate)}<div class="cards">${cards}</div>`
    }
  }

export const registerGymOssControllerElement = (): void => {
  if (typeof customElements === 'undefined') {
    return
  }
  if (typeof HTMLElement === 'undefined') {
    return
  }
  if (customElements.get(GYM_OSS_CONTROLLER_TAG) !== undefined) {
    return
  }
  customElements.define(GYM_OSS_CONTROLLER_TAG, makeControllerElement())
}

const gymOssControllerElement = defineCustomElement({
  events: {},
  properties: {},
  tag: GYM_OSS_CONTROLLER_TAG,
})

// Foldkit view wrapper: lazily registers the element and the nested scene, then
// embeds the controller custom element in the page. The element owns all its own
// interactive state, so the page does not thread new state through the logged-in
// TEA model.
export const gymOssControllerView = <Message>(
  attributes: ReadonlyArray<Attribute<Message>> = [],
): Html => {
  registerGymOssSceneElement()
  registerGymOssControllerElement()
  const element = gymOssControllerElement.withMessage<Message>()
  return element(attributes, [])
}
