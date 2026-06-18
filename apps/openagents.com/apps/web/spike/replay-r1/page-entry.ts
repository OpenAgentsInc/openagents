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
  type ProofReplayBundle,
  type ReplayCameraMode,
  type ReplayCameraPose,
  type ReplayVector3,
  buildReplayRenderPlan,
  cameraPoseFor,
} from '@openagentsinc/proof-replay'

import {
  TASSADAR_PROOF_REPLAY_TAG,
  tassadarProofReplayView,
} from '../../src/scene/tassadarProofReplayElement'
import { spikeReplayBundle } from './fixture-bundle'

// Calling the view has the side effect of registering the custom element
// (`register()` -> customElements.define). We discard the returned Foldkit Html
// and mount our own host below; we only need the registration.
void tassadarProofReplayView()

declare global {
  interface Window {
    driveReplayFrame: (input: {
      second?: number
      cameraMode?: ReplayCameraMode
      cameraPose?: {
        fov?: number
        position?: ReplayVector3
        target?: ReplayVector3
      }
    }) => Promise<(ReplayCameraPose & { fov?: number }) | null>
    loadReplayBundle: (bundle: ProofReplayBundle) => Promise<void>
    setReplaySecond: (second: number) => Promise<void>
    setCamera: (mode: ReplayCameraMode) => Promise<void>
    replaySpikeReady: boolean
    replaySpikeCameraPose: unknown
  }
}

const SCENE_ID = 'replay-spike-scene'
let currentReplayBundle: ProofReplayBundle = spikeReplayBundle

type ReplaySceneElement = HTMLElement & {
  bundle?: unknown
  driveReplayFrame?: (input: {
    second?: number
    cameraMode?: ReplayCameraMode
    cameraPose?: {
      fov?: number
      position?: ReplayVector3
      target?: ReplayVector3
    }
  }) => (ReplayCameraPose & { fov?: number }) | null
}

type ReplayCameraOverride = {
  fov?: number
  position?: ReplayVector3
  target?: ReplayVector3
}

const sceneEl = (): ReplaySceneElement => {
  const el = document.getElementById(SCENE_ID)
  if (el === null) throw new Error('spike scene element missing')
  return el as ReplaySceneElement
}

const waitFrame = (): Promise<void> =>
  new Promise(resolve => requestAnimationFrame(() => resolve()))

const applyCameraOverride = (
  pose: ReplayCameraPose,
  override?: ReplayCameraOverride,
): ReplayCameraPose & { fov?: number } => ({
  ...pose,
  ...(override?.fov === undefined ? {} : { fov: override.fov }),
  position: override?.position ?? pose.position,
  target: override?.target ?? pose.target,
})

const mount = async (): Promise<void> => {
  await customElements.whenDefined(TASSADAR_PROOF_REPLAY_TAG)
  const host = document.createElement(TASSADAR_PROOF_REPLAY_TAG)
  host.id = SCENE_ID
  host.style.position = 'absolute'
  host.style.inset = '0'
  document.body.appendChild(host)
  // The element exposes a `bundle` property setter that renders the scene from
  // the EXISTING proof-replay render plan. This is the real scene, not a stub.
  ;(host as ReplaySceneElement).bundle = spikeReplayBundle
}

window.loadReplayBundle = async (bundle: ProofReplayBundle): Promise<void> => {
  currentReplayBundle = bundle
  sceneEl().bundle = bundle
  await waitFrame()
}

window.driveReplayFrame = async (input: {
  second?: number
  cameraMode?: ReplayCameraMode
  cameraPose?: {
    fov?: number
    position?: ReplayVector3
    target?: ReplayVector3
  }
}): Promise<(ReplayCameraPose & { fov?: number }) | null> => {
  const pose =
    sceneEl().driveReplayFrame?.(input) ??
    applyCameraOverride(
      cameraPoseFor(
        buildReplayRenderPlan(currentReplayBundle),
        input.second ?? 0,
        input.cameraMode,
      ),
      input.cameraPose,
    )
  window.replaySpikeCameraPose = pose
  await waitFrame()
  return pose
}

window.setReplaySecond = async (second: number): Promise<void> => {
  await window.driveReplayFrame({ second })
}

window.setCamera = async (mode: ReplayCameraMode): Promise<void> => {
  await window.driveReplayFrame({ cameraMode: mode })
}

// Mount, then signal readiness once the scene has painted at least one frame.
void (async () => {
  await mount()
  await waitFrame()
  await waitFrame()
  window.replaySpikeReady = true
})()
