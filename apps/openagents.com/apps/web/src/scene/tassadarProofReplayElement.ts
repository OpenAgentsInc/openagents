import {
  activeReplayEventsAt,
  assertReplayPlanSourceCoverage,
  buildReplayRenderPlan,
  cameraPoseFor,
  initialReplayPlaybackState,
  interpolateActorPosition,
  reduceReplayClock,
  type ProofReplayBundle,
  type ReplayCameraMode,
  type ReplayEvent,
  type ReplayPlaybackState,
  type ReplayRenderPlan,
  type ReplaySourceRef,
  type ReplayVector3,
} from '@openagentsinc/proof-replay'
import { define as defineCustomElement } from 'foldkit/customElement'
import type { Attribute, Html } from 'foldkit/html'

import { currentUnixMs } from '../time-format'

export const TASSADAR_PROOF_REPLAY_TAG = 'oa-tassadar-proof-replay'
export const TASSADAR_REPLAY_SLUG_DATA_KEY = 'replay-slug'
export const FIRST_REAL_SETTLEMENT_REPLAY_SLUG = 'first-real-settlement'
export const TASSADAR_FIRST_REAL_SETTLEMENT_REPLAY_ENDPOINT =
  '/api/public/tassadar-replays/first-real-settlement'

type ReplayDataState = 'loading' | 'ok' | 'error'
type ReplayPresentationMode = 'interactive' | 'social'

const SOCIAL_TITLE = 'Tassadar Run 1: first real Bitcoin settlement'
const SOCIAL_SUBTITLE =
  'Verified work -> owner gate -> Spark zap -> public receipt'
const SOCIAL_FINAL_RECEIPT_REF =
  'receipt.nexus.tassadar_run_settlement...v6.20260618'
const SOCIAL_DEFAULT_DURATION_SECOND = 60
const SOCIAL_CANVAS_WIDTH = 1280
const SOCIAL_CANVAS_HEIGHT = 720

const CAMERA_MODES: ReadonlyArray<ReplayCameraMode> = [
  'director_track',
  'overview',
  'follow_actor',
  'orbit_proof',
  'zap_focus',
  'free_camera',
]

const SPEEDS = [0.5, 1, 1.5, 2, 4] as const

const HOST_STYLE =
  ':host{position:absolute;inset:0;display:block;background:#000;color:#f1efe8;font-family:ui-monospace,SFMono-Regular,Menlo,monospace}' +
  '.shell{position:absolute;inset:0;display:grid;grid-template-rows:auto minmax(0,1fr) auto;background:radial-gradient(circle at 50% 48%,rgba(44,162,143,0.16),transparent 34%),#000;overflow:hidden}' +
  '.shell.social{grid-template-rows:minmax(0,1fr);background:#000}' +
  '.top{z-index:4;display:flex;justify-content:space-between;gap:1rem;padding:0.9rem 1rem 0;color:#f1efe8;pointer-events:none}' +
  '.shell.social .top{position:absolute;left:1.4rem;right:1.4rem;top:1rem;padding:0;z-index:6}' +
  '.top dl{display:grid;grid-template-columns:repeat(4,minmax(0,auto));gap:0.55rem 1rem;margin:0;min-width:0}.top div{min-width:0}.top dt{margin:0 0 0.16rem;font-size:0.58rem;letter-spacing:0.08em;text-transform:uppercase;color:rgba(255,255,255,0.34)}' +
  '.top dd{margin:0;max-width:20rem;overflow:hidden;text-overflow:ellipsis;white-space:nowrap;font-size:0.72rem;color:rgba(255,255,255,0.78);text-shadow:0 1px 8px rgba(0,0,0,0.9)}' +
  '.top a{pointer-events:auto;color:rgba(255,255,255,0.86);font-size:0.72rem;text-underline-offset:0.2rem}.top a:hover{color:#fff}' +
  '.world{position:relative;min-height:0;margin:0 1rem;border-top:1px solid rgba(255,255,255,0.08);border-bottom:1px solid rgba(255,255,255,0.08);overflow:hidden;perspective:900px}' +
  '.shell.social .world{margin:0;border:0;min-height:100%;aspect-ratio:16/9;background:#000}' +
  '.social-canvas{position:absolute;inset:0;width:100%;height:100%;opacity:0.92}' +
  '.plane{position:absolute;inset:7% 6% 8%;transform-style:preserve-3d}.grid{position:absolute;inset:0;background:linear-gradient(rgba(255,255,255,0.045) 1px,transparent 1px),linear-gradient(90deg,rgba(255,255,255,0.045) 1px,transparent 1px);background-size:7.5% 10%;mask-image:radial-gradient(circle at 50% 50%,#000 0 62%,transparent 82%);opacity:0.58}' +
  '.shell.social .plane{inset:9% 7% 10%}.shell.social .grid{opacity:0.36}' +
  '.stage,.actor,.zap,.marker{position:absolute;transform:translate(-50%,-50%);border:0;color:#f1efe8;font:inherit;text-align:center}' +
  '.stage{min-width:5.2rem;max-width:9.5rem;padding:0;background:transparent;text-shadow:0 1px 10px rgba(0,0,0,0.9);cursor:pointer}.stage .orb{display:block;width:2.15rem;height:2.15rem;margin:0 auto 0.34rem;border:1px solid rgba(255,255,255,0.24);border-radius:999px;background:rgba(255,255,255,0.08);box-shadow:0 0 1.2rem rgba(42,181,161,0.32)}' +
  '.stage span:last-child{display:block;overflow:hidden;text-overflow:ellipsis;font-size:0.67rem;line-height:1.18;color:rgba(255,255,255,0.72)}.stage.core .orb{width:4.8rem;height:4.8rem;background:radial-gradient(circle,rgba(249,255,232,0.92),rgba(42,181,161,0.38) 42%,rgba(42,181,161,0.02) 72%);box-shadow:0 0 3.4rem rgba(249,255,232,0.72),0 0 8rem rgba(42,181,161,0.28)}.stage.core span:last-child{font-size:1rem;color:#fff}' +
  '.stage.proof_gate .orb{background:rgba(96,163,255,0.18);box-shadow:0 0 1.6rem rgba(96,163,255,0.36)}.stage.settlement_terminal .orb{background:rgba(255,193,83,0.18);box-shadow:0 0 1.8rem rgba(255,193,83,0.32)}.stage.registry_marker .orb{background:rgba(255,255,255,0.06);box-shadow:0 0 1.2rem rgba(255,255,255,0.14)}' +
  '.actor{z-index:2;min-width:5.8rem;background:transparent;text-shadow:0 1px 10px rgba(0,0,0,0.85);cursor:pointer}.actor .avatar{display:block;width:2.4rem;height:2.4rem;margin:0 auto 0.26rem;border-radius:999px;border:1px solid rgba(255,255,255,0.32);background:radial-gradient(circle at 35% 28%,#fff,rgba(42,181,161,0.72) 38%,rgba(42,181,161,0.12) 78%);box-shadow:0 0 1.4rem rgba(42,181,161,0.38)}.actor.validator .avatar{background:radial-gradient(circle at 35% 28%,#fff,rgba(96,163,255,0.7) 38%,rgba(96,163,255,0.12) 78%)}.actor.settlement_terminal .avatar,.actor.operator_gate .avatar{background:radial-gradient(circle at 35% 28%,#fff,rgba(255,193,83,0.74) 38%,rgba(255,193,83,0.12) 78%)}' +
  '.actor strong{display:block;overflow:hidden;text-overflow:ellipsis;font-size:0.64rem;font-weight:600;color:rgba(255,255,255,0.86)}.actor em{display:block;margin-top:0.12rem;font-style:normal;font-size:0.55rem;letter-spacing:0.08em;text-transform:uppercase;color:rgba(255,255,255,0.42)}' +
  '.zap{z-index:1;left:50%;top:50%;width:min(42rem,70%);height:0.2rem;background:linear-gradient(90deg,transparent,rgba(255,241,151,0.18),rgba(255,241,151,0.92),rgba(255,241,151,0.18),transparent);box-shadow:0 0 1.4rem rgba(255,241,151,0.72);animation:zapPulse 1.2s infinite}.zap span{position:absolute;left:50%;top:-1.5rem;transform:translateX(-50%);white-space:nowrap;font-size:0.76rem;color:#fff;text-shadow:0 0 1rem rgba(255,241,151,0.9)}' +
  '.marker{z-index:3;right:8%;top:28%;left:auto;transform:none;max-width:16rem;border-left:2px solid rgba(255,112,112,0.75);padding-left:0.6rem;text-align:left;font-size:0.66rem;line-height:1.35;color:rgba(255,205,205,0.84);text-shadow:0 1px 8px rgba(0,0,0,0.9)}.marker.simulation{top:40%;border-left-color:rgba(255,255,255,0.42);color:rgba(255,255,255,0.68)}' +
  '.caption{position:absolute;left:50%;bottom:1rem;z-index:4;max-width:min(52rem,calc(100% - 2rem));transform:translateX(-50%);padding:0.5rem 0.75rem;background:rgba(0,0,0,0.44);border:1px solid rgba(255,255,255,0.1);backdrop-filter:blur(12px);font-size:0.78rem;line-height:1.45;color:rgba(255,255,255,0.82);text-align:center}' +
  '.social-hud{position:absolute;inset:0;z-index:5;pointer-events:none;color:#fff;text-shadow:0 1px 16px rgba(0,0,0,0.88)}.social-title{position:absolute;left:4.2%;top:6.8%;max-width:56rem}.social-title h1{margin:0;font-size:4rem;line-height:1.02;font-weight:760;letter-spacing:0;color:#fff}.social-title p{margin:0.52rem 0 0;font-size:1.16rem;line-height:1.3;color:rgba(255,255,255,0.78)}.social-time{position:absolute;right:4.2%;top:7.2%;font-size:1.05rem;color:rgba(255,255,255,0.82)}.social-beat{position:absolute;left:4.2%;right:4.2%;bottom:6.2%;display:flex;align-items:flex-end;justify-content:space-between;gap:1.2rem}.social-beat p{max-width:52rem;margin:0;font-size:1.18rem;line-height:1.36;color:rgba(255,255,255,0.84)}.social-progress{width:min(16rem,26vw);height:0.18rem;background:rgba(255,255,255,0.22);overflow:hidden}.social-progress span{display:block;height:100%;background:#fff;box-shadow:0 0 1rem rgba(255,255,255,0.78)}.end-card{position:absolute;right:4.2%;bottom:13%;width:min(30rem,38vw);border:1px solid rgba(255,255,255,0.16);background:rgba(0,0,0,0.52);padding:0.78rem 0.92rem;backdrop-filter:blur(14px)}.end-card strong{display:block;margin-bottom:0.38rem;font-size:1.58rem;line-height:1.08;color:#fff}.end-card span,.end-card a{display:block;overflow-wrap:anywhere;font-size:0.82rem;line-height:1.35;color:rgba(255,255,255,0.78);pointer-events:auto}.end-card a{margin-top:0.42rem;text-underline-offset:0.2rem;color:rgba(255,255,255,0.92)}' +
  '.bottom{z-index:4;display:grid;grid-template-columns:minmax(0,1fr) minmax(20rem,28rem);gap:0.8rem;padding:0.75rem 1rem 1rem;min-height:12rem;max-height:36vh}.controls{display:grid;grid-template-columns:auto minmax(8rem,1fr) auto auto;gap:0.5rem;align-items:center;margin-bottom:0.55rem}.controls button,.controls select,.events button{border:1px solid rgba(255,255,255,0.14);background:rgba(255,255,255,0.07);color:#f1efe8;font:inherit;font-size:0.72rem}.controls button{min-width:4.2rem;padding:0.42rem 0.58rem}.controls select{padding:0.38rem 0.46rem}.controls input{min-width:0;accent-color:#f1efe8}' +
  '.events{min-height:0;overflow:auto;border-top:1px solid rgba(255,255,255,0.08);padding-top:0.45rem}.events ol{display:grid;grid-template-columns:repeat(auto-fit,minmax(13rem,1fr));gap:0.35rem;margin:0;padding:0;list-style:none}.events button{display:grid;gap:0.18rem;width:100%;padding:0.45rem 0.5rem;text-align:left}.events button[aria-current=true]{border-color:rgba(255,241,151,0.5);background:rgba(255,241,151,0.09)}.events time{font-size:0.58rem;color:rgba(255,255,255,0.38)}.events span{font-size:0.66rem;line-height:1.28;color:rgba(255,255,255,0.72)}' +
  '.inspector{min-height:0;overflow:auto;border:1px solid rgba(255,255,255,0.11);background:rgba(0,0,0,0.38);padding:0.65rem 0.7rem;backdrop-filter:blur(12px)}.inspector h2{margin:0 0 0.5rem;font-size:0.84rem;line-height:1.25;font-weight:650;color:#fff}.inspector dl{display:grid;gap:0.42rem;margin:0}.inspector dt{margin:0 0 0.12rem;font-size:0.56rem;letter-spacing:0.08em;text-transform:uppercase;color:rgba(255,255,255,0.34)}.inspector dd{margin:0;overflow-wrap:anywhere;font-size:0.67rem;line-height:1.38;color:rgba(255,255,255,0.68)}.inspector a{color:rgba(255,255,255,0.9);text-underline-offset:0.18rem}.inspector a:hover{color:#fff}.source-list{display:grid;gap:0.2rem;margin:0;padding:0;list-style:none}.source-list li{overflow-wrap:anywhere}' +
  '.overlay{position:absolute;inset:0;display:flex;align-items:center;justify-content:center;padding:2rem;text-align:center;pointer-events:none}.overlay p{max-width:48ch;margin:0;border:1px solid rgba(255,255,255,0.1);background:rgba(0,0,0,0.42);padding:0.8rem 1rem;font-size:0.86rem;line-height:1.55;color:rgba(255,255,255,0.66);backdrop-filter:blur(12px)}.overlay .label{display:block;margin-bottom:0.36rem;font-size:0.58rem;letter-spacing:0.08em;text-transform:uppercase;color:rgba(255,255,255,0.36)}' +
  '@keyframes zapPulse{0%,100%{opacity:0.72;filter:blur(0)}50%{opacity:1;filter:blur(0.5px)}}' +
  '@media (max-width:820px){.top{display:grid}.top dl{grid-template-columns:repeat(2,minmax(0,1fr))}.bottom{grid-template-columns:1fr;max-height:48vh}.controls{grid-template-columns:auto minmax(0,1fr);}.world{margin:0}.stage:not(.core) span:last-child,.actor strong{max-width:5.8rem}.top dd{max-width:none}.social-title h1{font-size:2.2rem}.social-title p,.social-time,.social-beat p{font-size:0.86rem}.end-card{width:min(23rem,44vw)}.end-card strong{font-size:1.12rem}.end-card span,.end-card a{font-size:0.68rem}}'

const clamp = (value: number, min: number, max: number): number =>
  Math.min(max, Math.max(min, Number.isFinite(value) ? value : min))

const formatSecond = (second: number): string =>
  `${Math.round(second * 10) / 10}s`

const textOrUnknown = (value: string | undefined): string => {
  const text = value?.trim()
  return text === undefined || text.length === 0 ? 'unknown' : text
}

const stageClassFor = (kind: string): string =>
  kind.replace(/[^a-z0-9_-]/gi, '_').toLowerCase()

const projectPoint = (position: ReplayVector3): Readonly<{
  left: string
  top: string
}> => ({
  left: `${clamp(50 + position.x * 5.8, 7, 93)}%`,
  top: `${clamp(50 + position.z * 4.9 - position.y * 3.5, 9, 91)}%`,
})

const sourceRecordMap = (
  bundle: ProofReplayBundle,
): ReadonlyMap<string, ReplaySourceRef> =>
  new Map(bundle.sourceRefs.map(source => [source.ref, source]))

const hrefForSourceRef = (
  ref: string,
  sourceRecords: ReadonlyMap<string, ReplaySourceRef>,
): string | undefined => {
  const source = sourceRecords.get(ref)
  if (source?.url !== undefined) return source.url
  if (ref.startsWith('https://')) return ref
  if (ref.startsWith('/api/')) return ref
  if (ref.startsWith('receipt.nexus.')) {
    return `/api/public/nexus-pylon/receipts/${encodeURIComponent(ref)}`
  }
  return undefined
}

const latestCaptionAt = (
  plan: ReplayRenderPlan,
  second: number,
): string | undefined =>
  [...plan.captions].reverse().find(caption => caption.timelineSecond <= second)
    ?.text

const activePaymentEvent = (
  events: ReadonlyArray<ReplayEvent>,
): ReplayEvent | undefined =>
  [...events].reverse().find(event => event.kind === 'payment_zap_confirmed')

const activeBlockedEvent = (
  events: ReadonlyArray<ReplayEvent>,
): ReplayEvent | undefined =>
  [...events]
    .reverse()
    .find(event => event.kind === 'settlement_blocked_closed')

const activeSimulationEvent = (
  events: ReadonlyArray<ReplayEvent>,
): ReplayEvent | undefined =>
  [...events].reverse().find(event => event.kind === 'payment_zap_simulated')

const replayEndpointForSlug = (slug: string): string =>
  slug === FIRST_REAL_SETTLEMENT_REPLAY_SLUG
    ? TASSADAR_FIRST_REAL_SETTLEMENT_REPLAY_ENDPOINT
    : `/api/public/proof-replays?ref=${encodeURIComponent(slug)}`

const queryParamsFromLocation = (): URLSearchParams => {
  if (typeof window === 'undefined') return new URLSearchParams()
  return new URLSearchParams(window.location.search)
}

const presentationModeFromLocation = (): ReplayPresentationMode => {
  const params = queryParamsFromLocation()
  return params.get('camera') === 'social' || params.get('hud') === 'social'
    ? 'social'
    : 'interactive'
}

const socialDurationSecondFromLocation = (fallback: number): number => {
  const params = queryParamsFromLocation()
  const requested = Number(params.get('duration') ?? SOCIAL_DEFAULT_DURATION_SECOND)
  return clamp(
    Number.isFinite(requested) ? requested : fallback,
    45,
    SOCIAL_DEFAULT_DURATION_SECOND,
  )
}

const socialStartSecondFromLocation = (durationSecond: number): number => {
  const requested = Number(queryParamsFromLocation().get('start') ?? '0')
  return clamp(requested, 0, durationSecond)
}

const socialProgressPercent = (playback: ReplayPlaybackState): number =>
  clamp((playback.second / playback.durationSecond) * 100, 0, 100)

const socialBeatText = (
  plan: ReplayRenderPlan,
  playback: ReplayPlaybackState,
): string => {
  const caption = latestCaptionAt(plan, playback.second)
  if (caption !== undefined) return caption
  const event = [...plan.orderedEvents]
    .reverse()
    .find(value => value.timelineSecond <= playback.second)
  return event?.displayText ?? 'Public replay begins from receipt-backed proof.'
}

const socialReceiptHref = (bundle: ProofReplayBundle): string => {
  const sourceRecords = sourceRecordMap(bundle)
  for (const source of bundle.sourceRefs) {
    if (source.kind === 'receipt') {
      const href = hrefForSourceRef(source.ref, sourceRecords)
      if (href !== undefined) return href
    }
  }
  for (const source of bundle.sourceRefs) {
    if (source.ref.includes('/api/public/nexus-pylon/receipts/')) {
      const href = hrefForSourceRef(source.ref, sourceRecords)
      if (href !== undefined) return href
    }
  }
  return '/tassadar/replay/first-real-settlement'
}

const makeClass = (): CustomElementConstructor =>
  class extends HTMLElement {
    #abort: AbortController | null = null
    #bundle: ProofReplayBundle | null = null
    #cameraMode: ReplayCameraMode = 'director_track'
    #lastTickAt = 0
    #playback: ReplayPlaybackState | null = null
    #plan: ReplayRenderPlan | null = null
    #selectedRef: string | null = null
    #shadow: ShadowRoot | null = null
    #timer: number | null = null

    connectedCallback(): void {
      this.#shadow = this.shadowRoot ?? this.attachShadow({ mode: 'open' })
      this.#refresh()
    }

    disconnectedCallback(): void {
      this.#abort?.abort()
      this.#abort = null
      this.#stopTimer()
      this.#shadow?.replaceChildren()
    }

    #refresh(): void {
      this.#abort?.abort()
      this.#stopTimer()
      this.#renderLoading()
      this.#abort = new AbortController()
      void this.#load(this.#abort.signal)
    }

    async #load(signal: AbortSignal): Promise<void> {
      try {
        const response = await fetch(replayEndpointForSlug(this.#replaySlug()), {
          headers: { accept: 'application/json' },
          signal,
        })
        if (signal.aborted) return
        if (!response.ok) {
          this.#renderError(`Replay bundle unavailable (HTTP ${response.status}).`)
          return
        }
        const bundle = (await response.json()) as ProofReplayBundle
        if (signal.aborted) return
        this.#renderBundle(bundle)
      } catch {
        if (signal.aborted) return
        this.#renderError('Could not load the public proof replay bundle.')
      }
    }

    #replaySlug(): string {
      return (
        this.getAttribute(`data-${TASSADAR_REPLAY_SLUG_DATA_KEY}`)?.trim() ??
        FIRST_REAL_SETTLEMENT_REPLAY_SLUG
      )
    }

    #base(): HTMLDivElement | null {
      const shadow = this.#shadow
      if (shadow === null) return null
      shadow.replaceChildren()
      const style = document.createElement('style')
      style.textContent = HOST_STYLE
      const shell = document.createElement('div')
      shell.className = 'shell'
      shadow.append(style, shell)
      return shell
    }

    #renderLoading(): void {
      const shell = this.#base()
      if (shell === null) return
      this.#setDataState('loading')
      shell.append(this.#overlay('Proof replay', 'Loading public replay bundle.'))
    }

    #renderError(message: string): void {
      const shell = this.#base()
      if (shell === null) return
      this.#setDataState('error')
      shell.append(this.#overlay('Proof replay error', message))
    }

    #renderBundle(bundle: ProofReplayBundle): void {
      const plan = buildReplayRenderPlan(bundle)
      assertReplayPlanSourceCoverage(plan)
      const presentationMode = presentationModeFromLocation()
      const initialPlayback = initialReplayPlaybackState(bundle)
      const socialDurationSecond =
        presentationMode === 'social'
          ? socialDurationSecondFromLocation(initialPlayback.durationSecond)
          : initialPlayback.durationSecond
      const socialStartSecond =
        presentationMode === 'social'
          ? socialStartSecondFromLocation(socialDurationSecond)
          : initialPlayback.second
      this.#bundle = bundle
      this.#plan = plan
      this.#playback = {
        ...initialPlayback,
        durationSecond: socialDurationSecond,
        isPlaying: presentationMode === 'social',
        second: socialStartSecond,
      }
      this.#selectedRef = plan.orderedEvents[0]?.eventRef ?? null
      this.#cameraMode = 'director_track'
      this.#renderCurrent()
      if (presentationMode === 'social') this.#startTimer()
    }

    #renderCurrent(): void {
      const bundle = this.#bundle
      const plan = this.#plan
      const playback = this.#playback
      const shell = this.#base()
      if (bundle === null || plan === null || playback === null || shell === null) {
        return
      }

      this.#setDataState('ok')
      const presentationMode = presentationModeFromLocation()
      shell.classList.toggle('social', presentationMode === 'social')
      this.setAttribute('data-replay-second', playback.second.toFixed(1))
      this.setAttribute('data-replay-camera', this.#cameraMode)
      this.setAttribute('data-replay-presentation', presentationMode)
      if (presentationMode === 'social') {
        this.setAttribute('data-social-duration', String(playback.durationSecond))
        this.setAttribute('data-social-hud', 'social')
        shell.append(this.#renderWorld(bundle, plan, playback, presentationMode))
        return
      }
      this.removeAttribute('data-social-duration')
      this.removeAttribute('data-social-hud')

      shell.append(
        this.#renderTop(bundle, playback),
        this.#renderWorld(bundle, plan, playback, presentationMode),
        this.#renderBottom(bundle, plan, playback),
      )
    }

    #setDataState(state: ReplayDataState): void {
      this.setAttribute('data-state', state)
    }

    #renderTop(
      bundle: ProofReplayBundle,
      playback: ReplayPlaybackState,
    ): HTMLElement {
      const panel = document.createElement('header')
      panel.className = 'top'
      const list = document.createElement('dl')
      const rows: ReadonlyArray<readonly [string, string]> = [
        ['Replay', bundle.title],
        ['Bundle', bundle.bundleRef],
        ['Moment', bundle.socialDisplayTime ?? 'June 17, 8:38pm CT'],
        ['Clock', `${formatSecond(playback.second)} / ${formatSecond(playback.durationSecond)}`],
      ]
      for (const [termText, detailText] of rows) {
        const row = document.createElement('div')
        const term = document.createElement('dt')
        term.textContent = termText
        const detail = document.createElement('dd')
        detail.textContent = detailText
        row.append(term, detail)
        list.append(row)
      }
      const live = document.createElement('a')
      live.href = '/tassadar'
      live.textContent = 'Live Tassadar'
      panel.append(list, live)
      return panel
    }

    #renderWorld(
      bundle: ProofReplayBundle,
      plan: ReplayRenderPlan,
      playback: ReplayPlaybackState,
      presentationMode: ReplayPresentationMode,
    ): HTMLElement {
      const world = document.createElement('section')
      world.className = 'world'
      world.setAttribute(
        'aria-label',
        presentationMode === 'social'
          ? 'Social proof replay stage'
          : '3D proof replay stage',
      )
      world.setAttribute('data-replay-stage', 'first-real-settlement')
      world.setAttribute('data-camera-mode', this.#cameraMode)
      world.setAttribute('data-replay-presentation', presentationMode)

      if (presentationMode === 'social') {
        const canvas = document.createElement('canvas')
        canvas.className = 'social-canvas'
        canvas.width = SOCIAL_CANVAS_WIDTH
        canvas.height = SOCIAL_CANVAS_HEIGHT
        canvas.setAttribute('data-social-canvas', 'nonblank')
        canvas.setAttribute('aria-hidden', 'true')
        this.#drawSocialCanvas(canvas, playback)
        world.append(canvas)
      }

      const plane = document.createElement('div')
      plane.className = 'plane'
      const grid = document.createElement('div')
      grid.className = 'grid'
      plane.append(grid)

      for (const stage of plan.stagePlacements) {
        plane.append(this.#stageButton(stage.ref, stage.label, stage.kind, stage.position))
      }

      for (const track of plan.actorTracks) {
        const actor = bundle.actors.find(value => value.actorRef === track.actorRef)
        const position = interpolateActorPosition(track, playback.second)
        const activeFrame = [...track.keyframes]
          .reverse()
          .find(frame => frame.second <= playback.second)
        plane.append(
          this.#actorButton(
            track.actorRef,
            actor?.displayName ?? track.actorRef,
            actor?.avatarRole ?? 'agent',
            position,
            activeFrame?.state ?? 'idle',
          ),
        )
      }

      const activeEvents = activeReplayEventsAt(bundle, playback.second)
      const confirmedZap = activePaymentEvent(activeEvents)
      if (confirmedZap !== undefined) {
        const zap = document.createElement('div')
        zap.className = 'zap'
        zap.setAttribute('data-replay-zap', 'confirmed')
        const label = document.createElement('span')
        label.textContent = `${confirmedZap.amountSats ?? 1_000} sats ${textOrUnknown(confirmedZap.rail)}`
        zap.append(label)
        plane.append(zap)
      }

      const blocked = activeBlockedEvent(activeEvents)
      if (blocked !== undefined && confirmedZap === undefined) {
        const marker = document.createElement('div')
        marker.className = 'marker blocked'
        marker.setAttribute('data-replay-marker', 'blocked')
        marker.textContent = `${blocked.displayText} ${blocked.caveat ?? ''}`.trim()
        plane.append(marker)
      }

      const simulation = activeSimulationEvent(activeEvents)
      if (simulation !== undefined && confirmedZap === undefined) {
        const marker = document.createElement('div')
        marker.className = 'marker simulation'
        marker.setAttribute('data-replay-marker', 'simulation')
        marker.textContent = `${simulation.amountSats ?? 0} sats simulation, not payment`
        plane.append(marker)
      }

      const pose = cameraPoseFor(
        plan,
        playback.second,
        this.#cameraMode === 'director_track' ? undefined : this.#cameraMode,
      )
      world.setAttribute(
        'data-camera-pose',
        `${pose.position.x.toFixed(2)},${pose.position.y.toFixed(2)},${pose.position.z.toFixed(2)}`,
      )
      world.setAttribute(
        'data-camera-target',
        `${pose.target.x.toFixed(2)},${pose.target.y.toFixed(2)},${pose.target.z.toFixed(2)}`,
      )

      world.append(plane)
      const captionText = latestCaptionAt(plan, playback.second)
      if (captionText !== undefined) {
        const caption = document.createElement('div')
        caption.className = 'caption'
        caption.textContent = captionText
        world.append(caption)
      }
      if (presentationMode === 'social') {
        world.append(this.#renderSocialHud(bundle, plan, playback))
      }
      return world
    }

    #drawSocialCanvas(
      canvas: HTMLCanvasElement,
      playback: ReplayPlaybackState,
    ): void {
      let context: CanvasRenderingContext2D | null = null
      try {
        context = canvas.getContext('2d')
      } catch {
        context = null
      }
      if (context === null) return

      const width = canvas.width
      const height = canvas.height
      context.clearRect(0, 0, width, height)
      const background = context.createLinearGradient(0, 0, width, height)
      background.addColorStop(0, '#050605')
      background.addColorStop(0.52, '#06120f')
      background.addColorStop(1, '#000000')
      context.fillStyle = background
      context.fillRect(0, 0, width, height)

      const glow = context.createRadialGradient(
        width * 0.5,
        height * 0.52,
        10,
        width * 0.5,
        height * 0.52,
        height * 0.58,
      )
      glow.addColorStop(0, 'rgba(249,255,232,0.78)')
      glow.addColorStop(0.28, 'rgba(42,181,161,0.2)')
      glow.addColorStop(1, 'rgba(42,181,161,0)')
      context.fillStyle = glow
      context.fillRect(0, 0, width, height)

      context.strokeStyle = 'rgba(255,255,255,0.08)'
      context.lineWidth = 1
      for (let x = 0; x <= width; x += width / 16) {
        context.beginPath()
        context.moveTo(x, height * 0.2)
        context.lineTo(width * 0.5 + (x - width * 0.5) * 0.72, height * 0.9)
        context.stroke()
      }
      for (let y = height * 0.32; y <= height * 0.9; y += height / 14) {
        context.beginPath()
        context.moveTo(width * 0.08, y)
        context.lineTo(width * 0.92, y)
        context.stroke()
      }

      const progress = socialProgressPercent(playback) / 100
      context.strokeStyle = 'rgba(255,241,151,0.32)'
      context.lineWidth = 4
      context.beginPath()
      context.arc(
        width * 0.5,
        height * 0.52,
        height * 0.18 + progress * height * 0.05,
        -Math.PI * 0.5,
        -Math.PI * 0.5 + Math.PI * 2 * progress,
      )
      context.stroke()
    }

    #renderSocialHud(
      bundle: ProofReplayBundle,
      plan: ReplayRenderPlan,
      playback: ReplayPlaybackState,
    ): HTMLElement {
      const hud = document.createElement('div')
      hud.className = 'social-hud'
      hud.setAttribute('data-social-hud', 'social')

      const title = document.createElement('div')
      title.className = 'social-title'
      const heading = document.createElement('h1')
      heading.textContent = SOCIAL_TITLE
      const subtitle = document.createElement('p')
      subtitle.textContent = SOCIAL_SUBTITLE
      title.append(heading, subtitle)

      const time = document.createElement('div')
      time.className = 'social-time'
      time.textContent = bundle.socialDisplayTime ?? '8:38pm, June 17'

      const beat = document.createElement('div')
      beat.className = 'social-beat'
      const caption = document.createElement('p')
      caption.textContent = socialBeatText(plan, playback)
      const progress = document.createElement('div')
      progress.className = 'social-progress'
      progress.setAttribute('aria-hidden', 'true')
      const fill = document.createElement('span')
      fill.style.width = `${socialProgressPercent(playback).toFixed(1)}%`
      progress.append(fill)
      beat.append(caption, progress)

      hud.append(title, time, beat)
      if (playback.second >= playback.durationSecond - 8) {
        hud.append(this.#renderSocialEndCard(bundle))
      }
      return hud
    }

    #renderSocialEndCard(bundle: ProofReplayBundle): HTMLElement {
      const card = document.createElement('aside')
      card.className = 'end-card'
      card.setAttribute('data-social-end-card', 'settled')
      const amount = document.createElement('strong')
      amount.textContent = '1,000 sats settled'
      const moved = document.createElement('span')
      moved.textContent = 'realBitcoinMoved:true'
      const receipt = document.createElement('span')
      receipt.textContent = SOCIAL_FINAL_RECEIPT_REF
      const link = document.createElement('a')
      link.href = socialReceiptHref(bundle)
      link.textContent = 'Public receipt'
      card.append(amount, moved, receipt, link)
      return card
    }

    #stageButton(
      ref: string,
      label: string,
      kind: string,
      position: ReplayVector3,
    ): HTMLButtonElement {
      const button = document.createElement('button')
      button.type = 'button'
      button.className = `stage ${stageClassFor(kind)}`
      button.setAttribute('data-replay-target-ref', ref)
      const point = projectPoint(position)
      button.style.left = point.left
      button.style.top = point.top
      const orb = document.createElement('span')
      orb.className = 'orb'
      const text = document.createElement('span')
      text.textContent = label
      button.append(orb, text)
      button.addEventListener('click', () => {
        this.#selectedRef = ref
        this.#renderCurrent()
      })
      return button
    }

    #actorButton(
      ref: string,
      label: string,
      role: string,
      position: ReplayVector3,
      state: string,
    ): HTMLButtonElement {
      const button = document.createElement('button')
      button.type = 'button'
      button.className = `actor ${stageClassFor(role)}`
      button.setAttribute('data-replay-actor-ref', ref)
      button.setAttribute('data-replay-actor-state', state)
      const point = projectPoint(position)
      button.style.left = point.left
      button.style.top = point.top
      const avatar = document.createElement('span')
      avatar.className = 'avatar'
      const name = document.createElement('strong')
      name.textContent = label
      const mode = document.createElement('em')
      mode.textContent = state
      button.append(avatar, name, mode)
      button.addEventListener('click', () => {
        this.#selectedRef = ref
        this.#renderCurrent()
      })
      return button
    }

    #renderBottom(
      bundle: ProofReplayBundle,
      plan: ReplayRenderPlan,
      playback: ReplayPlaybackState,
    ): HTMLElement {
      const bottom = document.createElement('footer')
      bottom.className = 'bottom'
      const left = document.createElement('div')
      left.append(
        this.#renderControls(playback),
        this.#renderEventList(plan, playback),
      )
      bottom.append(left, this.#renderInspector(bundle, plan))
      return bottom
    }

    #renderControls(playback: ReplayPlaybackState): HTMLElement {
      const controls = document.createElement('div')
      controls.className = 'controls'
      controls.setAttribute('aria-label', 'Replay controls')

      const toggle = document.createElement('button')
      toggle.type = 'button'
      toggle.setAttribute('data-replay-control', 'play')
      toggle.textContent = playback.isPlaying ? 'Pause' : 'Play'
      toggle.addEventListener('click', () => {
        playback.isPlaying ? this.#pause() : this.#play()
      })

      const scrub = document.createElement('input')
      scrub.type = 'range'
      scrub.min = '0'
      scrub.max = String(playback.durationSecond)
      scrub.step = '0.1'
      scrub.value = String(playback.second)
      scrub.setAttribute('data-replay-control', 'scrub')
      scrub.addEventListener('input', () => {
        this.#setPlayback(reduceReplayClock(playback, {
          second: Number(scrub.value),
          type: 'seek',
        }))
      })

      const speed = document.createElement('select')
      speed.setAttribute('data-replay-control', 'speed')
      for (const value of SPEEDS) {
        const option = document.createElement('option')
        option.value = String(value)
        option.textContent = `${value}x`
        option.selected = playback.playbackRate === value
        speed.append(option)
      }
      speed.addEventListener('change', () => {
        this.#setPlayback(reduceReplayClock(playback, {
          playbackRate: Number(speed.value),
          type: 'set_speed',
        }))
      })

      const camera = document.createElement('select')
      camera.setAttribute('data-replay-control', 'camera')
      for (const mode of CAMERA_MODES) {
        const option = document.createElement('option')
        option.value = mode
        option.textContent = mode.replace(/_/g, ' ')
        option.selected = this.#cameraMode === mode
        camera.append(option)
      }
      camera.addEventListener('change', () => {
        this.#cameraMode = camera.value as ReplayCameraMode
        this.#renderCurrent()
      })

      controls.append(toggle, scrub, speed, camera)
      return controls
    }

    #renderEventList(
      plan: ReplayRenderPlan,
      playback: ReplayPlaybackState,
    ): HTMLElement {
      const panel = document.createElement('section')
      panel.className = 'events'
      panel.setAttribute('aria-label', 'Replay event list')
      const list = document.createElement('ol')
      for (const event of plan.orderedEvents) {
        const item = document.createElement('li')
        const button = document.createElement('button')
        button.type = 'button'
        button.setAttribute('data-replay-event-ref', event.eventRef)
        button.setAttribute(
          'aria-current',
          String(this.#selectedRef === event.eventRef),
        )
        const time = document.createElement('time')
        time.textContent = formatSecond(event.timelineSecond)
        const text = document.createElement('span')
        text.textContent = event.displayText
        button.append(time, text)
        button.addEventListener('click', () => {
          this.#selectedRef = event.eventRef
          this.#setPlayback(reduceReplayClock(playback, {
            second: event.timelineSecond,
            type: 'seek',
          }))
        })
        item.append(button)
        list.append(item)
      }
      panel.append(list)
      return panel
    }

    #renderInspector(
      bundle: ProofReplayBundle,
      plan: ReplayRenderPlan,
    ): HTMLElement {
      const inspector = document.createElement('aside')
      inspector.className = 'inspector'
      inspector.setAttribute('aria-label', 'Replay source inspector')

      const target = plan.hitTargets.find(
        hitTarget => hitTarget.targetRef === this.#selectedRef,
      )
      const event = plan.orderedEvents.find(
        value => value.eventRef === this.#selectedRef,
      )
      const title = document.createElement('h2')
      title.textContent = event?.displayText ?? target?.label ?? 'Replay source'
      inspector.append(title)

      const rows: ReadonlyArray<readonly [string, string | ReadonlyArray<string>]> = [
        ['Kind', event?.kind ?? target?.kind ?? 'bundle'],
        ['State', event?.stateAfter ?? 'evidence presentation'],
        ['Time', event === undefined ? 'n/a' : formatSecond(event.timelineSecond)],
        [
          'Sats',
          event?.amountSats === undefined
            ? 'n/a'
            : `${event.amountSats} sats ${event.rail ?? ''}`.trim(),
        ],
        ['Caveat', event?.caveat ?? 'public refs only'],
        ['Source refs', event?.sourceRefs ?? target?.sourceRefs ?? bundle.sourceRefs.map(source => source.ref)],
      ]
      const list = document.createElement('dl')
      const sourceRecords = sourceRecordMap(bundle)
      for (const [termText, detailValue] of rows) {
        const row = document.createElement('div')
        const term = document.createElement('dt')
        term.textContent = termText
        const detail = document.createElement('dd')
        if (typeof detailValue === 'string') {
          detail.textContent = detailValue
        } else {
          detail.append(this.#renderSourceList(detailValue, sourceRecords))
        }
        row.append(term, detail)
        list.append(row)
      }
      inspector.append(list)
      return inspector
    }

    #renderSourceList(
      refs: ReadonlyArray<string>,
      sourceRecords: ReadonlyMap<string, ReplaySourceRef>,
    ): HTMLElement {
      const list = document.createElement('ul')
      list.className = 'source-list'
      for (const ref of refs.slice(0, 10)) {
        const item = document.createElement('li')
        const href = hrefForSourceRef(ref, sourceRecords)
        if (href === undefined) {
          item.textContent = ref
        } else {
          const link = document.createElement('a')
          link.href = href
          link.target = '_blank'
          link.rel = 'noopener noreferrer'
          link.textContent = ref
          item.append(link)
        }
        list.append(item)
      }
      return list
    }

    #play(): void {
      const playback = this.#playback
      if (playback === null) return
      this.#setPlayback(reduceReplayClock(playback, { type: 'play' }), false)
      this.#startTimer()
      this.#renderCurrent()
    }

    #pause(): void {
      const playback = this.#playback
      if (playback === null) return
      this.#setPlayback(reduceReplayClock(playback, { type: 'pause' }), false)
      this.#stopTimer()
      this.#renderCurrent()
    }

    #setPlayback(next: ReplayPlaybackState, render = true): void {
      this.#playback = next
      if (!next.isPlaying || next.second >= next.durationSecond) {
        this.#stopTimer()
        this.#playback =
          next.second >= next.durationSecond ? { ...next, isPlaying: false } : next
      }
      if (render) this.#renderCurrent()
    }

    #startTimer(): void {
      this.#stopTimer()
      this.#lastTickAt = currentUnixMs()
      this.#timer = window.setInterval(() => {
        const playback = this.#playback
        if (playback === null) return
        const now = currentUnixMs()
        const deltaSecond = Math.max(0, now - this.#lastTickAt) / 1_000
        this.#lastTickAt = now
        this.#setPlayback(
          reduceReplayClock(playback, { deltaSecond, type: 'tick' }),
        )
      }, 100)
    }

    #stopTimer(): void {
      if (this.#timer === null) return
      window.clearInterval(this.#timer)
      this.#timer = null
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
  if (customElements.get(TASSADAR_PROOF_REPLAY_TAG) !== undefined) return
  customElements.define(TASSADAR_PROOF_REPLAY_TAG, makeClass())
}

const element = defineCustomElement({
  events: {},
  properties: {},
  tag: TASSADAR_PROOF_REPLAY_TAG,
})

export const tassadarProofReplayView = <Message>(
  attributes: ReadonlyArray<Attribute<Message>> = [],
): Html => {
  register()
  return element.withMessage<Message>()(attributes, [])
}
