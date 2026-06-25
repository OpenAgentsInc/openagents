// Headless render harness for the public `/gym` live run follow-along (#6261).
//
// Mounts the REAL `Gym.view(initGymModel())` Foldkit view via a minimal Foldkit
// program, so the capture script exercises the actual page composition — the
// live Gym run follow-along panel (in-progress label + three-effect run field +
// accessible text/table mirror) — NOT a re-implementation, without the app's
// auth bootstrap or any network.

import { Schema as S } from 'effect'
import { Runtime } from 'foldkit'
import { html } from 'foldkit/html'
import type { Document } from 'foldkit/html'

import { initGymModel } from '../src/page/loggedOut/gym/flow'
import * as Gym from '../src/page/loggedOut/page/gym'

const Model = S.Struct({ mounted: S.Boolean })
type Model = typeof Model.Type
type Message = Readonly<{ _tag: 'Noop' }>

const view = (): Document => {
  const h = html<Message>()
  return {
    title: 'Gym run progress capture',
    body: h.div([h.Id('gym-run-progress-capture-root')], [Gym.view(initGymModel())]),
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
