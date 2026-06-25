// Headless render harness for the public `/gym` live run follow-along (#6261).
//
// Mounts the REAL `Gym.view` Foldkit view via a minimal Foldkit program with a
// LOADED run-progress model, so the capture script exercises the actual page
// composition — the live Gym run follow-along panel (counts, pass-rate over
// completed, in-progress / not-decision-grade markers, accessible per-run text
// mirror) — NOT a re-implementation, without the app's auth bootstrap or any
// network. The run used here is a test-local mirror of the public-safe
// projection, not a shipped fixture: the live app renders real runs fetched
// from `GET /api/public/gym/run-progress`.

import { Schema as S } from 'effect'
import { Runtime } from 'foldkit'
import { html } from 'foldkit/html'
import type { Document } from 'foldkit/html'

import { initGymModel } from '../src/page/loggedOut/gym/flow'
import {
  GYM_RUN_PROGRESS_SCHEMA,
  type GymRunProgress,
} from '../src/page/loggedOut/gym/runProgress'
import { LoadedPublicGymRunProgress } from '../src/page/loggedOut/model'
import * as Gym from '../src/page/loggedOut/page/gym'

const Model = S.Struct({ mounted: S.Boolean })
type Model = typeof Model.Type
type Message = Readonly<{ _tag: 'Noop' }>

const captureRun: GymRunProgress = {
  schemaVersion: GYM_RUN_PROGRESS_SCHEMA,
  runRef: 'run.gym.terminal_bench.glm52-reap-baseline',
  jobRef: 'job.gym.harbor_terminal_bench.glm52-reap-baseline',
  configId: 'gym.terminal_bench.glm52-reap-baseline',
  environmentRef: 'terminal-bench',
  datasetRef: 'terminal-bench@2.0',
  runner: 'harbor',
  agent: 'terminus-2',
  profile: {
    profileRef: 'glm-reap-504b-g4-tp4-mtp2-rp105',
    publicLabel: 'GLM-5.2 REAP 504B speculative decoding',
    model: 'openagents/glm-5.2-reap-504b',
    attribution: 'Z.ai GLM-5.2 REAP',
    hardwareProfile: 'hydralisk-g4-4x-rtx-pro-6000',
    contextWindowTokens: 250_000,
  },
  phase: 'running',
  decisionGrade: false,
  inProgress: true,
  publication: 'web_authorized',
  counts: {
    officialDenominator: 89,
    completed: 18,
    completedPassed: 10,
    completedFailed: 8,
    running: 1,
    pending: 70,
    error: 5,
    cancelled: 0,
  },
  passRateOverCompleted: 10 / 18,
  completionFraction: 18 / 89,
  tokens: {
    promptTokens: 61_966_392,
    completionTokens: 296_537,
    totalTokens: 62_262_929,
  },
  elapsedMs: null,
  lastUpdatedAt: '2026-06-25T18:13:35.081Z',
  caveatRefs: [],
  blockerRefs: [],
}

const view = (): Document => {
  const h = html<Message>()
  return {
    title: 'Gym run progress capture',
    body: h.div(
      [h.Id('gym-run-progress-capture-root')],
      [Gym.view(initGymModel(), LoadedPublicGymRunProgress({ runs: [captureRun] }))],
    ),
  }
}

const container = document.getElementById('root')
if (container === null) throw new Error('missing #root container')

const program = Runtime.makeProgram<Model, Message>({
  Model,
  container,
  init: () => [{ mounted: true }, []],
  update: (model: Model) => [model, []],
  view,
})

Runtime.run(program)

declare global {
  interface Window {
    __gymRunProgressCaptureMounted?: boolean
  }
}

window.__gymRunProgressCaptureMounted = true
