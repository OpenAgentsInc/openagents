// Replay R-1 headless render spike — page entry.
//
// Mounts the EXISTING proof-replay scene custom element
// (`oa-tassadar-proof-replay`, src/scene/tassadarProofReplayElement.ts),
// driven by the EXISTING proof-replay primitives (buildReplayRenderPlan /
// cameraPoseFor), and exposes two programmatic hooks for the headless driver:
//
//   window.setReplaySecond(s: number): Promise<void>
//   window.setCamera(mode: ReplayCameraMode): Promise<void>
//
// The scene element is interactive (its widget suite is what the owner does NOT
// want in a clip), but for this spike we drive it programmatically through its
// real shadow-DOM controls so the screenshot reflects the genuine rendered
// scene at a chosen moment + camera, with no human clicking.

import {
  buildReplayRenderPlan,
  cameraPoseFor,
  type ReplayCameraMode,
} from '@openagentsinc/proof-replay'

import {
  tassadarProofReplayView,
  TASSADAR_PROOF_REPLAY_TAG,
} from '../../src/scene/tassadarProofReplayElement'
import { spikeReplayBundle } from './fixture-bundle'

// Calling the view has the side effect of registering the custom element
// (`register()` -> customElements.define). We discard the returned Foldkit Html
// and mount our own host below; we only need the registration.
void tassadarProofReplayView()

declare global {
  interface Window {
    setReplaySecond: (second: number) => Promise<void>
    setCamera: (mode: ReplayCameraMode) => Promise<void>
    replaySpikeReady: boolean
    replaySpikeCameraPose: unknown
  }
}

const SCENE_ID = 'replay-spike-scene'

const sceneEl = (): HTMLElement => {
  const el = document.getElementById(SCENE_ID)
  if (el === null) throw new Error('spike scene element missing')
  return el
}

const shadowControl = (selector: string): HTMLElement | null => {
  const root = sceneEl().shadowRoot
  if (root === null) return null
  return root.querySelector(selector)
}

const waitFrame = (): Promise<void> =>
  new Promise(resolve => requestAnimationFrame(() => resolve()))

const mount = async (): Promise<void> => {
  await customElements.whenDefined(TASSADAR_PROOF_REPLAY_TAG)
  const host = document.createElement(TASSADAR_PROOF_REPLAY_TAG)
  host.id = SCENE_ID
  host.style.position = 'absolute'
  host.style.inset = '0'
  document.body.appendChild(host)
  // The element exposes a `bundle` property setter that renders the scene from
  // the EXISTING proof-replay render plan. This is the real scene, not a stub.
  ;(host as unknown as { bundle: unknown }).bundle = spikeReplayBundle
}

window.setReplaySecond = async (second: number): Promise<void> => {
  // Drive the real scrubber control the scene renders, so playback time is set
  // through the scene's own clock reducer.
  const scrub = shadowControl('[data-replay-control="scrub"]') as
    | HTMLInputElement
    | null
  if (scrub === null) throw new Error('scrubber control not found in scene')
  scrub.value = String(second)
  scrub.dispatchEvent(new Event('input', { bubbles: true }))
  // Record the camera pose the proof-replay math computes for this moment, so
  // the spike can report whether the camera model is exercised (it is — as
  // data; see the report on whether it drives a viewpoint).
  const plan = buildReplayRenderPlan(spikeReplayBundle)
  window.replaySpikeCameraPose = cameraPoseFor(plan, second)
  await waitFrame()
}

window.setCamera = async (mode: ReplayCameraMode): Promise<void> => {
  const select = shadowControl('[data-replay-control="camera"]') as
    | HTMLSelectElement
    | null
  if (select === null) throw new Error('camera control not found in scene')
  select.value = mode
  select.dispatchEvent(new Event('change', { bubbles: true }))
  await waitFrame()
}

// Mount, then signal readiness once the scene has painted at least one frame.
void (async () => {
  await mount()
  await waitFrame()
  await waitFrame()
  window.replaySpikeReady = true
})()
