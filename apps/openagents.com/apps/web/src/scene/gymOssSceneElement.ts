// Gym — GPT-OSS live latency scene (#6167).
//
// A tasteful, house-styled (psionic blue, mono) live visualization of the
// in-flight GPT-OSS requests: one vertical bar per in-flight request whose fill
// rate is proportional to its perceived tokens/sec, plus an aggregate-throughput
// meter (sum of tokens/sec across in-flight requests). It reuses the
// fan-out-arcs / cost-meter visual language from `scene/*` and
// `docs/khala/khala-in-the-world.md`, rendered to a 2D canvas (the issue
// allows `three-effect` OR canvas; canvas keeps the live meter deterministic and
// testable, and degrades cleanly where no 2D context exists, e.g. headless DOM).
//
// The PURE visual mapping (`buildSceneFrame`) turns a live frame into drawable
// geometry and is unit-tested offline; the canvas draw is guarded behind a
// context check so the element is safe to mount in a DOM without canvas support.
//
// IDENTITY: the scene labels are the neutral lane label only — never the raw
// upstream id, never CoT (#6156).

import { define as defineCustomElement } from 'foldkit/customElement'
import type { Attribute, Html } from 'foldkit/html'

export const GYM_OSS_SCENE_TAG = 'oa-gym-oss-scene'

// House palette (psionic blue on void) — see root DESIGN.md and khala.ts.
const COLORS = {
  void: '#05070c',
  grid: 'rgba(58,123,255,0.10)',
  barIdle: 'rgba(58,123,255,0.22)',
  barFill: '#4fd0ff',
  barFailed: 'rgba(255,122,122,0.55)',
  meterTrack: 'rgba(58,123,255,0.18)',
  meterFill: '#7cf0ff',
  text: '#bcd4ff',
} as const

// The live state the scene draws: one entry per in-flight (or just-finished)
// request, plus the aggregate throughput.
export type SceneRequest = Readonly<{
  index: number
  status: 'running' | 'ok' | 'failed'
  // perceived tokens/sec (measured), or null when not yet/never measured. The
  // bar fill is proportional to this; null reads as an empty (idle) bar, NEVER a
  // fabricated fill.
  perceivedTps: number | null
}>

export type SceneFrame = Readonly<{
  requests: ReadonlyArray<SceneRequest>
  // Aggregate throughput (sum of tokens/sec across in-flight requests), or null
  // when nothing has measured TPS yet.
  aggregateTps: number | null
}>

// ---------------------------------------------------------------------------
// Pure visual mapping (tested offline).
// ---------------------------------------------------------------------------

export type SceneBar = Readonly<{
  index: number
  // [0,1] fill fraction relative to the busiest bar in the frame; 0 for idle /
  // unmeasured (never fabricated).
  fillFraction: number
  status: SceneRequest['status']
}>

export type SceneGeometry = Readonly<{
  bars: ReadonlyArray<SceneBar>
  // [0,1] aggregate meter fraction relative to the frame's aggregate ceiling
  // (the running max aggregate); 0 when nothing measured.
  meterFraction: number
}>

// Map a live frame into drawable geometry. PURE. The bar fill is each request's
// TPS normalized to the busiest request in the frame, so the scene reads as
// relative throughput. An unmeasured/failed request is an empty bar (honest
// absence, never a fabricated fill). `aggregateCeiling` lets the caller keep the
// aggregate meter monotonic across frames (pass the running max).
export const buildSceneFrame = (
  frame: SceneFrame,
  aggregateCeiling: number | null,
): SceneGeometry => {
  const measuredTps = frame.requests
    .map(request => request.perceivedTps)
    .filter((value): value is number => value !== null && value > 0)
  const peak = measuredTps.length === 0 ? 0 : Math.max(...measuredTps)

  const bars = frame.requests.map((request): SceneBar => {
    const tps = request.perceivedTps
    const fillFraction =
      tps === null || tps <= 0 || peak <= 0
        ? 0
        : Math.min(1, tps / peak)
    return { index: request.index, fillFraction, status: request.status }
  })

  const ceiling =
    aggregateCeiling !== null && aggregateCeiling > 0
      ? aggregateCeiling
      : frame.aggregateTps !== null && frame.aggregateTps > 0
        ? frame.aggregateTps
        : 0
  const meterFraction =
    frame.aggregateTps === null || ceiling <= 0
      ? 0
      : Math.min(1, frame.aggregateTps / ceiling)

  return { bars, meterFraction }
}

// ---------------------------------------------------------------------------
// Canvas rendering (guarded; degrades cleanly without a 2D context).
// ---------------------------------------------------------------------------

const drawScene = (
  ctx: CanvasRenderingContext2D,
  width: number,
  height: number,
  geometry: SceneGeometry,
): void => {
  ctx.clearRect(0, 0, width, height)
  ctx.fillStyle = COLORS.void
  ctx.fillRect(0, 0, width, height)

  // Aggregate meter along the bottom.
  const meterHeight = 10
  const meterY = height - meterHeight - 6
  ctx.fillStyle = COLORS.meterTrack
  ctx.fillRect(6, meterY, width - 12, meterHeight)
  ctx.fillStyle = COLORS.meterFill
  ctx.fillRect(6, meterY, (width - 12) * geometry.meterFraction, meterHeight)

  // Bars across the top region.
  const count = geometry.bars.length
  if (count === 0) {
    return
  }
  const region = meterY - 12
  const gap = 6
  const barWidth = Math.max(2, (width - 12 - gap * (count - 1)) / count)
  geometry.bars.forEach((bar, i) => {
    const x = 6 + i * (barWidth + gap)
    ctx.fillStyle = COLORS.barIdle
    ctx.fillRect(x, 6, barWidth, region - 6)
    const fillHeight = (region - 6) * bar.fillFraction
    ctx.fillStyle = bar.status === 'failed' ? COLORS.barFailed : COLORS.barFill
    ctx.fillRect(x, 6 + (region - 6) - fillHeight, barWidth, fillHeight)
  })
}

export type GymOssSceneHandle = Readonly<{
  push: (frame: SceneFrame) => void
  dispose: () => void
}>

// Mount the scene into a host element holding a <canvas>. Returns a handle whose
// `push` re-renders for a new live frame. Safe to call where no 2D context
// exists — it simply keeps the latest frame without drawing.
export const mountGymOssScene = (canvas: HTMLCanvasElement): GymOssSceneHandle => {
  let aggregateCeiling: number | null = null
  let disposed = false
  const ctx =
    typeof canvas.getContext === 'function' ? canvas.getContext('2d') : null

  const push = (frame: SceneFrame): void => {
    if (disposed) {
      return
    }
    if (frame.aggregateTps !== null) {
      aggregateCeiling =
        aggregateCeiling === null
          ? frame.aggregateTps
          : Math.max(aggregateCeiling, frame.aggregateTps)
    }
    const geometry = buildSceneFrame(frame, aggregateCeiling)
    if (ctx === null) {
      return
    }
    drawScene(ctx, canvas.width || 600, canvas.height || 200, geometry)
  }

  return {
    push,
    dispose: () => {
      disposed = true
    },
  }
}

// ---------------------------------------------------------------------------
// Custom element + Foldkit view wrapper.
// ---------------------------------------------------------------------------

const gymOssSceneElement = defineCustomElement({
  events: {},
  properties: {},
  tag: GYM_OSS_SCENE_TAG,
})

const hostCss = `
:host { display: block; width: 100%; }
canvas { display: block; width: 100%; height: 200px; border: 1px solid rgba(58,123,255,0.18); border-radius: 12px; background: ${COLORS.void}; }
`

const makeGymOssSceneElement = (): CustomElementConstructor =>
  class GymOssSceneElement extends HTMLElement {
    #handle: GymOssSceneHandle | null = null

    // The live frame is pushed by the page as a property.
    set frame(value: unknown) {
      if (this.#handle === null) {
        return
      }
      if (value !== null && typeof value === 'object') {
        this.#handle.push(value as SceneFrame)
      }
    }

    connectedCallback(): void {
      if (this.#handle !== null) {
        return
      }
      const shadow = this.shadowRoot ?? this.attachShadow({ mode: 'open' })
      shadow.replaceChildren()
      const style = document.createElement('style')
      style.textContent = hostCss
      const canvas = document.createElement('canvas')
      canvas.width = 600
      canvas.height = 200
      shadow.append(style, canvas)
      this.#handle = mountGymOssScene(canvas)
    }

    disconnectedCallback(): void {
      if (this.#handle === null) {
        return
      }
      this.#handle.dispose()
      this.#handle = null
    }
  }

export const registerGymOssSceneElement = (): void => {
  if (typeof customElements === 'undefined') {
    return
  }
  if (typeof HTMLElement === 'undefined') {
    return
  }
  if (customElements.get(GYM_OSS_SCENE_TAG) !== undefined) {
    return
  }
  customElements.define(GYM_OSS_SCENE_TAG, makeGymOssSceneElement())
}

export const gymOssSceneView = <Message>(
  attributes: ReadonlyArray<Attribute<Message>> = [],
): Html => {
  registerGymOssSceneElement()
  const element = gymOssSceneElement.withMessage<Message>()
  return element(attributes, [])
}
