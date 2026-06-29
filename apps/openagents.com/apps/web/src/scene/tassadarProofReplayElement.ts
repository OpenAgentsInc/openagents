import {
  FIRST_REAL_SETTLEMENT_REPLAY_SLUG,
  OPENAGENTS_PUBLIC_ORIGIN,
  type ProofReplayBundle,
  type ReplayCameraMode,
  type ReplayCameraPose,
  type ReplayEvent,
  type ReplayPlaybackState,
  type ReplayRenderPlan,
  type ReplaySourceRef,
  type ReplayVector3,
  activeReplayEventsAt,
  assertProofReplayBundleShipmentGate,
  assertReplayPlanSourceCoverage,
  buildReplayRenderPlan,
  cameraPoseFor,
  initialReplayPlaybackState,
  interpolateActorPosition,
  proofReplayBundleEndpointForSlug,
  reduceReplayClock,
} from '@openagentsinc/proof-replay'
import {
  type ProofReplayActorDefinition,
  type ProofReplayCameraPose as ThreeEffectProofReplayCameraPose,
  type ProofReplayEventDefinition,
  type ProofReplayFlowDefinition,
  type ProofReplayStageDefinition,
  type ProofReplayVisualizationFrame,
  type ProofReplayVisualizationHandle,
  type ProofReplayVisualizationOptions,
  mountProofReplayVisualization,
} from '@openagentsinc/three-effect/core'
import { Effect, Schema as S } from 'effect'
import { define as defineCustomElement } from 'foldkit/customElement'
import type { Attribute, Html } from 'foldkit/html'

import { currentUnixMs } from '../time-format.js'

export const TASSADAR_PROOF_REPLAY_TAG = 'oa-tassadar-proof-replay'
export const TASSADAR_REPLAY_ORIGIN_DATA_KEY = 'replay-origin'
export const TASSADAR_REPLAY_SLUG_DATA_KEY = 'replay-slug'
export { FIRST_REAL_SETTLEMENT_REPLAY_SLUG, OPENAGENTS_PUBLIC_ORIGIN }

type ReplayDataState = 'loading' | 'ok' | 'error'
type ReplayPresentationMode = 'interactive' | 'social'
type ReplayCameraOverride = Readonly<{
  fov?: number
  position?: ReplayVector3
  target?: ReplayVector3
}>
type DrivenReplayCameraPose = ReplayCameraPose & Readonly<{ fov?: number }>
type DriveReplayFrameInput = Readonly<{
  cameraMode?: ReplayCameraMode
  cameraPose?: ReplayCameraOverride
  second?: number
}>

const SOCIAL_TITLE = 'Tassadar Run 1: first real Bitcoin settlement'
const SOCIAL_SUBTITLE =
  'Verified work -> owner gate -> Spark zap -> public receipt'
const SOCIAL_FINAL_RECEIPT_REF =
  'receipt.nexus.tassadar_run_settlement...v6.20260618'
const SOCIAL_DEFAULT_DURATION_SECOND = 60

const CAMERA_MODES: ReadonlyArray<ReplayCameraMode> = [
  'director_track',
  'overview',
  'follow_actor',
  'orbit_proof',
  'zap_focus',
  'free_camera',
]

const isReplayCameraMode = (value: unknown): value is ReplayCameraMode =>
  typeof value === 'string' && CAMERA_MODES.includes(value as ReplayCameraMode)

const SPEEDS = [0.5, 1, 1.5, 2, 4] as const

const HOST_STYLE =
  ':host{position:absolute;inset:0;display:block;background:#000;color:#f1efe8;font-family:ui-monospace,SFMono-Regular,Menlo,monospace}' +
  '.shell{position:absolute;inset:0;display:grid;grid-template-rows:auto minmax(0,1fr) auto;background:radial-gradient(circle at 50% 48%,rgba(44,162,143,0.16),transparent 34%),#000;overflow:hidden}' +
  '.shell.social{grid-template-rows:minmax(0,1fr);background:#000}' +
  '.top{position:relative;z-index:8;display:flex;justify-content:space-between;gap:1rem;padding:0.9rem 1rem 0;color:#f1efe8;pointer-events:auto}' +
  '.shell.social .top{position:absolute;left:1.4rem;right:1.4rem;top:1rem;padding:0;z-index:6}' +
  '.top dl{display:grid;grid-template-columns:repeat(4,minmax(0,auto));gap:0.55rem 1rem;margin:0;min-width:0}.top div{min-width:0}.top dt{margin:0 0 0.16rem;font-size:0.58rem;letter-spacing:0.08em;text-transform:uppercase;color:rgba(255,255,255,0.34)}' +
  '.top dd{margin:0;max-width:min(20rem,26vw);overflow-wrap:anywhere;white-space:normal;font-size:0.72rem;line-height:1.25;color:rgba(255,255,255,0.78);text-shadow:0 1px 8px rgba(0,0,0,0.9)}' +
  '.top a{position:relative;z-index:9;align-self:start;justify-self:end;width:max-content;margin-left:auto;white-space:nowrap;pointer-events:auto;color:rgba(255,255,255,0.86);font-size:0.72rem;text-underline-offset:0.2rem;touch-action:manipulation}.top a:hover{color:#fff}' +
  '.world{position:relative;min-height:0;margin:0 1rem;border-top:1px solid rgba(255,255,255,0.08);border-bottom:1px solid rgba(255,255,255,0.08);overflow:hidden;perspective:900px}' +
  '.shell.social .world{margin:0;border:0;min-height:100%;aspect-ratio:16/9;background:#000}' +
  '.webgl-mount{position:absolute;inset:0;width:100%;height:100%;background:#000}' +
  '.caption{position:absolute;left:50%;bottom:1rem;z-index:4;max-width:min(52rem,calc(100% - 2rem));transform:translateX(-50%);padding:0.5rem 0.75rem;background:rgba(0,0,0,0.44);border:1px solid rgba(255,255,255,0.1);backdrop-filter:blur(12px);font-size:0.78rem;line-height:1.45;color:rgba(255,255,255,0.82);text-align:center}' +
  '.social-hud{position:absolute;inset:0;z-index:5;pointer-events:none;color:#fff;text-shadow:0 1px 16px rgba(0,0,0,0.88)}.social-title{position:absolute;left:4.2%;top:6.4%;max-width:43rem}.social-title h1{margin:0;font-size:3.15rem;line-height:1.02;font-weight:760;letter-spacing:0;color:#fff}.social-title p{margin:0.42rem 0 0;font-size:1rem;line-height:1.3;color:rgba(255,255,255,0.78)}.social-time{position:absolute;right:4.2%;top:7.2%;font-size:1.05rem;color:rgba(255,255,255,0.82)}.social-beat{position:absolute;left:4.2%;right:4.2%;bottom:6.2%;display:flex;align-items:flex-end;justify-content:space-between;gap:1.2rem}.social-beat p{max-width:52rem;margin:0;font-size:1.18rem;line-height:1.36;color:rgba(255,255,255,0.84)}.social-progress{width:min(16rem,26vw);height:0.18rem;background:rgba(255,255,255,0.22);overflow:hidden}.social-progress span{display:block;height:100%;background:#fff;box-shadow:0 0 1rem rgba(255,255,255,0.78)}.end-card{position:absolute;right:4.2%;bottom:13%;width:min(30rem,38vw);border:1px solid rgba(255,255,255,0.16);background:rgba(0,0,0,0.52);padding:0.78rem 0.92rem;backdrop-filter:blur(14px)}.end-card strong{display:block;margin-bottom:0.38rem;font-size:1.58rem;line-height:1.08;color:#fff}.end-card span,.end-card a{display:block;overflow-wrap:anywhere;font-size:0.82rem;line-height:1.35;color:rgba(255,255,255,0.78);pointer-events:auto}.end-card a{margin-top:0.42rem;text-underline-offset:0.2rem;color:rgba(255,255,255,0.92)}' +
  '.bottom{position:relative;z-index:6;display:grid;grid-template-columns:minmax(0,1fr) minmax(20rem,28rem);gap:0.8rem;padding:0.75rem 1rem 1rem;min-height:12rem;max-height:36vh;pointer-events:auto}.controls,.events,.inspector{position:relative;z-index:7;pointer-events:auto}.controls{display:grid;grid-template-columns:auto minmax(8rem,1fr) auto auto;gap:0.5rem;align-items:center;margin-bottom:0.55rem}.controls button,.controls select,.events button{border:1px solid rgba(255,255,255,0.14);background:rgba(255,255,255,0.07);color:#f1efe8;font:inherit;font-size:0.72rem}.controls button{min-width:4.2rem;padding:0.42rem 0.58rem;touch-action:manipulation}.controls select{padding:0.38rem 0.46rem}.controls input{min-width:0;accent-color:#f1efe8}' +
  '.events{min-height:0;overflow:auto;border-top:1px solid rgba(255,255,255,0.08);padding-top:0.45rem}.events ol{display:grid;grid-template-columns:repeat(auto-fit,minmax(13rem,1fr));gap:0.35rem;margin:0;padding:0;list-style:none}.events button{display:grid;gap:0.18rem;width:100%;padding:0.45rem 0.5rem;text-align:left}.events button[aria-current=true]{border-color:rgba(255,241,151,0.5);background:rgba(255,241,151,0.09)}.events time{font-size:0.58rem;color:rgba(255,255,255,0.38)}.events span{font-size:0.66rem;line-height:1.28;color:rgba(255,255,255,0.72)}' +
  '.inspector{min-height:0;overflow:auto;border:1px solid rgba(255,255,255,0.11);background:rgba(0,0,0,0.38);padding:0.65rem 0.7rem;backdrop-filter:blur(12px)}.inspector h2{margin:0 0 0.5rem;font-size:0.84rem;line-height:1.25;font-weight:650;color:#fff}.inspector dl{display:grid;gap:0.42rem;margin:0}.inspector dt{margin:0 0 0.12rem;font-size:0.56rem;letter-spacing:0.08em;text-transform:uppercase;color:rgba(255,255,255,0.34)}.inspector dd{margin:0;overflow-wrap:anywhere;font-size:0.67rem;line-height:1.38;color:rgba(255,255,255,0.68)}.inspector a{color:rgba(255,255,255,0.9);text-underline-offset:0.18rem}.inspector a:hover{color:#fff}.source-list{display:grid;gap:0.2rem;margin:0;padding:0;list-style:none}.source-list li{overflow-wrap:anywhere}' +
  '.overlay{position:absolute;inset:0;display:flex;align-items:center;justify-content:center;padding:2rem;text-align:center;pointer-events:none}.overlay p{max-width:48ch;margin:0;border:1px solid rgba(255,255,255,0.1);background:rgba(0,0,0,0.42);padding:0.8rem 1rem;font-size:0.86rem;line-height:1.55;color:rgba(255,255,255,0.66);backdrop-filter:blur(12px)}.overlay .label{display:block;margin-bottom:0.36rem;font-size:0.58rem;letter-spacing:0.08em;text-transform:uppercase;color:rgba(255,255,255,0.36)}' +
  '@media (max-width:820px){.top{display:grid}.top dl{grid-template-columns:repeat(2,minmax(0,1fr))}.bottom{grid-template-columns:1fr;max-height:48vh}.controls{grid-template-columns:auto minmax(0,1fr);}.world{margin:0}.top dd{max-width:none}.social-title h1{font-size:2.2rem}.social-title p,.social-time,.social-beat p{font-size:0.86rem}.end-card{width:min(23rem,44vw)}.end-card strong{font-size:1.12rem}.end-card span,.end-card a{font-size:0.68rem}}'

const clamp = (value: number, min: number, max: number): number =>
  Math.min(max, Math.max(min, Number.isFinite(value) ? value : min))

const formatSecond = (second: number): string =>
  `${Math.round(second * 10) / 10}s`

const addPressHandler = (element: HTMLElement, handler: () => void): void => {
  let pointerHandled = false
  element.addEventListener('pointerdown', event => {
    pointerHandled = true
    event.preventDefault()
    event.stopPropagation()
    handler()
  })
  element.addEventListener('click', event => {
    event.preventDefault()
    event.stopPropagation()
    if (pointerHandled) {
      pointerHandled = false
      return
    }
    handler()
  })
}

const navigateToTassadar = (): void => {
  if (typeof window === 'undefined') return
  window.location.assign('/tassadar')
}

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
  const requested = Number(
    params.get('duration') ?? SOCIAL_DEFAULT_DURATION_SECOND,
  )
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

const uniqueRefs = (refs: ReadonlyArray<string | undefined>): ReadonlyArray<string> =>
  [...new Set(refs.filter((ref): ref is string => ref !== undefined && ref.trim() !== ''))]

const cameraPoseForFrame = (
  plan: ReplayRenderPlan,
  second: number,
  cameraMode: ReplayCameraMode,
  override: ReplayCameraOverride | null,
): DrivenReplayCameraPose => {
  const base = cameraPoseFor(
    plan,
    second,
    cameraMode === 'director_track' ? undefined : cameraMode,
  )
  return {
    ...base,
    ...(override?.fov === undefined ? {} : { fov: override.fov }),
    position: override?.position ?? base.position,
    target: override?.target ?? base.target,
  }
}

const threeEffectCameraPose = (
  pose: DrivenReplayCameraPose,
): ThreeEffectProofReplayCameraPose => ({
  cameraRef: pose.cameraRef,
  ...(pose.fov === undefined ? {} : { fov: pose.fov }),
  mode: pose.mode,
  position: pose.position,
  second: pose.second,
  sourceRefs: pose.sourceRefs,
  target: pose.target,
})

const stageDefinitionsFor = (
  plan: ReplayRenderPlan,
): ReadonlyArray<ProofReplayStageDefinition> =>
  plan.stagePlacements.map(stage => ({
    id: stage.ref,
    kind: stage.kind,
    label: stage.label,
    position: stage.position,
    sourceRefs: stage.sourceRefs,
  }))

const actorDefinitionsAt = (
  bundle: ProofReplayBundle,
  plan: ReplayRenderPlan,
  second: number,
): ReadonlyArray<ProofReplayActorDefinition> =>
  plan.actorTracks.map(track => {
    const actor = bundle.actors.find(value => value.actorRef === track.actorRef)
    const activeFrame = [...track.keyframes]
      .reverse()
      .find(frame => frame.second <= second)
    return {
      id: track.actorRef,
      label: actor?.displayName ?? track.actorRef,
      position: interpolateActorPosition(track, second),
      role: actor?.avatarRole ?? 'agent',
      sourceRefs: uniqueRefs([
        actor?.pylonRef,
        ...(activeFrame?.sourceRefs ?? []),
      ]),
      state: activeFrame?.state ?? 'idle',
    }
  })

const eventDefinitionsFor = (
  events: ReadonlyArray<ReplayEvent>,
): ReadonlyArray<ProofReplayEventDefinition> =>
  events.map(event => ({
    actorIds: event.actorRefs,
    ...(event.amountSats === undefined ? {} : { amountSats: event.amountSats }),
    id: event.eventRef,
    kind: event.kind,
    label: event.displayText,
    ...(event.rail === undefined ? {} : { rail: event.rail }),
    second: event.timelineSecond,
    sourceRefs: event.sourceRefs,
    targetIds: event.targetRefs,
  }))

const activeVisualEventsAt = (
  bundle: ProofReplayBundle,
  second: number,
): ReadonlyArray<ReplayEvent> =>
  activeReplayEventsAt(bundle, second)
    .filter(event => second - event.timelineSecond <= 9)
    .slice(-4)

const flowDefinitionsFor = (
  bundle: ProofReplayBundle,
): ReadonlyArray<ProofReplayFlowDefinition> =>
  bundle.flows.map(flow => ({
    fromId: flow.fromRef,
    id: flow.flowRef,
    kind: flow.flowKind,
    sourceRefs: flow.sourceRefs,
    toId: flow.toRef,
  }))

const proofReplayVisualizationOptionsFor = (
  bundle: ProofReplayBundle,
  plan: ReplayRenderPlan,
  playback: ReplayPlaybackState,
  camera: DrivenReplayCameraPose,
  labels: boolean,
): ProofReplayVisualizationOptions => ({
  actors: actorDefinitionsAt(bundle, plan, playback.second),
  camera: threeEffectCameraPose(camera),
  durationSecond: playback.durationSecond,
  events: eventDefinitionsFor(plan.orderedEvents),
  flows: flowDefinitionsFor(bundle),
  labels,
  stages: stageDefinitionsFor(plan),
  title: bundle.title,
})

const proofReplayVisualizationFrameFor = (
  bundle: ProofReplayBundle,
  plan: ReplayRenderPlan,
  playback: ReplayPlaybackState,
  camera: DrivenReplayCameraPose,
): ProofReplayVisualizationFrame => ({
  activeEvents: eventDefinitionsFor(activeVisualEventsAt(bundle, playback.second)),
  actors: actorDefinitionsAt(bundle, plan, playback.second),
  camera: threeEffectCameraPose(camera),
  second: playback.second,
})

const makeClass = (): CustomElementConstructor =>
  class extends HTMLElement {
    #abort: AbortController | null = null
    #bundle: ProofReplayBundle | null = null
    #cameraOverride: ReplayCameraOverride | null = null
    #cameraMode: ReplayCameraMode = 'director_track'
    #hasProvidedBundle = false
    #lastTickAt = 0
    #playback: ReplayPlaybackState | null = null
    #plan: ReplayRenderPlan | null = null
    #proofHandle: ProofReplayVisualizationHandle | null = null
    #providedBundle: ProofReplayBundle | null = null
    #selectedRef: string | null = null
    #shadow: ShadowRoot | null = null
    #timer: number | null = null

    get bundle(): ProofReplayBundle | null {
      return this.#providedBundle
    }

    set bundle(value: unknown) {
      if (value === undefined || value === null) {
        this.#providedBundle = null
        this.#hasProvidedBundle = false
        return
      }

      this.#providedBundle = value as ProofReplayBundle
      this.#hasProvidedBundle = true
      this.#abort?.abort()
      this.#abort = null
      this.#stopTimer()
      if (this.isConnected) this.#renderProvidedBundle()
    }

    driveReplayFrame(input: DriveReplayFrameInput): DrivenReplayCameraPose | null {
      const plan = this.#plan
      const playback = this.#playback
      if (plan === null || playback === null) return null

      if (isReplayCameraMode(input.cameraMode)) {
        this.#cameraMode = input.cameraMode
      }
      this.#cameraOverride = input.cameraPose ?? null

      const requestedSecond =
        typeof input.second === 'number' ? input.second : playback.second
      const paused = reduceReplayClock(playback, { type: 'pause' })
      this.#setPlayback(
        reduceReplayClock(paused, {
          second: requestedSecond,
          type: 'seek',
        }),
        false,
      )
      this.#renderCurrent()

      const current = this.#playback ?? playback
      return cameraPoseForFrame(
        plan,
        current.second,
        this.#cameraMode,
        this.#cameraOverride,
      )
    }

    connectedCallback(): void {
      this.#shadow = this.shadowRoot ?? this.attachShadow({ mode: 'open' })
      if (this.#renderProvidedBundle()) return
      this.#refresh()
    }

    disconnectedCallback(): void {
      this.#abort?.abort()
      this.#abort = null
      this.#stopTimer()
      this.#disposeProofScene()
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
        const response = await fetch(
          proofReplayBundleEndpointForSlug(
            this.#replaySlug(),
            this.#replayOrigin(),
          ),
          {
            headers: { accept: 'application/json' },
            signal,
          },
        )
        if (signal.aborted) return
        if (!response.ok) {
          this.#renderError(
            `Replay bundle unavailable (HTTP ${response.status}).`,
          )
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

    #renderProvidedBundle(): boolean {
      if (!this.#hasProvidedBundle || this.#providedBundle === null)
        return false
      try {
        this.#renderBundle(this.#providedBundle)
      } catch {
        this.#renderError(
          'Loaded proof replay bundle failed the public-safe replay gate.',
        )
      }
      return true
    }

    #replaySlug(): string {
      return (
        this.getAttribute(`data-${TASSADAR_REPLAY_SLUG_DATA_KEY}`)?.trim() ??
        FIRST_REAL_SETTLEMENT_REPLAY_SLUG
      )
    }

    #replayOrigin(): string | undefined {
      const origin = this.getAttribute(
        `data-${TASSADAR_REPLAY_ORIGIN_DATA_KEY}`,
      )?.trim()
      return origin === undefined || origin === '' ? undefined : origin
    }

    #base(): HTMLDivElement | null {
      const shadow = this.#shadow
      if (shadow === null) return null
      this.#disposeProofScene()
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
      shell.append(
        this.#overlay('Proof replay', 'Loading public replay bundle.'),
      )
    }

    #renderError(message: string): void {
      const shell = this.#base()
      if (shell === null) return
      this.#setDataState('error')
      shell.append(this.#overlay('Proof replay error', message))
    }

    #renderBundle(bundle: ProofReplayBundle): void {
      assertProofReplayBundleShipmentGate(bundle)
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
      this.#cameraOverride = null
      this.#renderCurrent()
      if (presentationMode === 'social') this.#startTimer()
    }

    #renderCurrent(): void {
      const bundle = this.#bundle
      const plan = this.#plan
      const playback = this.#playback
      const shell = this.#base()
      if (
        bundle === null ||
        plan === null ||
        playback === null ||
        shell === null
      ) {
        return
      }

      this.#setDataState('ok')
      const presentationMode = presentationModeFromLocation()
      shell.classList.toggle('social', presentationMode === 'social')
      this.setAttribute('data-replay-second', playback.second.toFixed(1))
      this.setAttribute('data-replay-camera', this.#cameraMode)
      this.setAttribute('data-replay-presentation', presentationMode)
      if (presentationMode === 'social') {
        this.setAttribute(
          'data-social-duration',
          String(playback.durationSecond),
        )
        this.setAttribute('data-social-hud', 'social')
        shell.append(
          this.#renderWorld(bundle, plan, playback, presentationMode),
        )
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
        [
          'Clock',
          `${formatSecond(playback.second)} / ${formatSecond(playback.durationSecond)}`,
        ],
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
      live.setAttribute('data-replay-control', 'live-tassadar')
      live.textContent = 'Live Tassadar'
      addPressHandler(live, navigateToTassadar)
      panel.append(list, live)
      return panel
    }

    #disposeProofScene(): void {
      const handle = this.#proofHandle
      if (handle === null) return
      Effect.runSync(handle.dispose)
      this.#proofHandle = null
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

      const pose = cameraPoseForFrame(
        plan,
        playback.second,
        this.#cameraMode,
        this.#cameraOverride,
      )
      world.setAttribute(
        'data-camera-pose',
        `${pose.position.x.toFixed(2)},${pose.position.y.toFixed(2)},${pose.position.z.toFixed(2)}`,
      )
      world.setAttribute(
        'data-camera-target',
        `${pose.target.x.toFixed(2)},${pose.target.y.toFixed(2)},${pose.target.z.toFixed(2)}`,
      )
      if (pose.fov !== undefined) {
        world.setAttribute('data-camera-fov', pose.fov.toFixed(2))
      } else {
        world.removeAttribute('data-camera-fov')
      }

      const mount = document.createElement('div')
      mount.className = 'webgl-mount'
      mount.setAttribute('data-proof-replay-webgl-mount', 'true')
      mount.setAttribute('aria-hidden', 'true')
      world.append(mount)
      try {
        const handle = Effect.runSync(
          mountProofReplayVisualization(
            mount,
            proofReplayVisualizationOptionsFor(
              bundle,
              plan,
              playback,
              pose,
              presentationMode !== 'social',
            ),
          ),
        )
        handle.setFrame(
          proofReplayVisualizationFrameFor(bundle, plan, playback, pose),
        )
        this.#proofHandle = handle
        world.setAttribute(
          'data-proof-replay-webgl',
          handle.webglAvailable ? 'available' : 'unavailable',
        )
      } catch {
        world.setAttribute('data-proof-replay-webgl', 'error')
        world.append(
          this.#overlay('Proof replay renderer', 'WebGL scene unavailable.'),
        )
      }
      const captionText = latestCaptionAt(plan, playback.second)
      if (captionText !== undefined && presentationMode !== 'social') {
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
      addPressHandler(toggle, () => {
        const current = this.#playback
        if (current === null) return
        current.isPlaying ? this.#pause() : this.#play()
      })

      const scrub = document.createElement('input')
      scrub.type = 'range'
      scrub.min = '0'
      scrub.max = String(playback.durationSecond)
      scrub.step = '0.1'
      scrub.value = String(playback.second)
      scrub.setAttribute('data-replay-control', 'scrub')
      scrub.addEventListener('input', () => {
        const current = this.#playback ?? playback
        this.#setPlayback(
          reduceReplayClock(current, {
            second: Number(scrub.value),
            type: 'seek',
          }),
        )
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
        const current = this.#playback ?? playback
        this.#setPlayback(
          reduceReplayClock(current, {
            playbackRate: Number(speed.value),
            type: 'set_speed',
          }),
        )
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
        addPressHandler(button, () => {
          this.#selectedRef = event.eventRef
          const current = this.#playback ?? playback
          this.#setPlayback(
            reduceReplayClock(current, {
              second: event.timelineSecond,
              type: 'seek',
            }),
          )
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

      const rows: ReadonlyArray<
        readonly [string, string | ReadonlyArray<string>]
      > = [
        ['Kind', event?.kind ?? target?.kind ?? 'bundle'],
        ['State', event?.stateAfter ?? 'evidence presentation'],
        [
          'Time',
          event === undefined ? 'n/a' : formatSecond(event.timelineSecond),
        ],
        [
          'Sats',
          event?.amountSats === undefined
            ? 'n/a'
            : `${event.amountSats} sats ${event.rail ?? ''}`.trim(),
        ],
        ['Caveat', event?.caveat ?? 'public refs only'],
        [
          'Source refs',
          event?.sourceRefs ??
            target?.sourceRefs ??
            bundle.sourceRefs.map(source => source.ref),
        ],
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
          next.second >= next.durationSecond
            ? { ...next, isPlaying: false }
            : next
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
  properties: {
    bundle: S.Unknown,
  },
  tag: TASSADAR_PROOF_REPLAY_TAG,
})

export const tassadarProofReplayView = <Message>(
  attributes: ReadonlyArray<Attribute<Message>> = [],
  bundle?: ProofReplayBundle | null,
): Html => {
  register()
  const replayElement = element.withMessage<Message>()
  return replayElement(
    bundle === undefined || bundle === null
      ? attributes
      : [...attributes, replayElement.Bundle(bundle)],
    [],
  )
}
