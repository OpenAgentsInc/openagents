// Headless render harness for the /pro pages (issue 6184).
//
// Mounts the REAL `evalDetailView` / `runDetailView` Foldkit views with a
// fixture session via a minimal Foldkit program, so the capture script
// exercises the actual page composition (the shared Pro primitives) — NOT a
// re-implementation — without the app's auth bootstrap or any network.
//
// The view to mount is chosen by `?view=eval|run` on the harness URL.

import { Schema as S } from 'effect'
import { Runtime } from 'foldkit'
import { html } from 'foldkit/html'
import type { Document } from 'foldkit/html'

import type { Session } from '../src/domain/session'
import { evalDetailView } from '../src/page/loggedIn/page/pro-evals'
import {
  listProEvals,
  listProRuns,
} from '../src/page/loggedIn/page/pro-readmodel'
import { runDetailView } from '../src/page/loggedIn/page/pro-runs'

const Model = S.Struct({ which: S.Literals(['eval', 'run']) })
type Model = typeof Model.Type
type Message = Readonly<{ _tag: 'Noop' }>

const session: Session = {
  userId: 'user_1',
  email: 'operator@openagents.com',
  name: 'Operator',
}

const params = new URLSearchParams(window.location.search)
const which: 'eval' | 'run' = params.get('view') === 'run' ? 'run' : 'eval'

const view = (model: Model): Document => {
  const h = html<Message>()
  const body =
    model.which === 'run'
      ? runDetailView(session, listProRuns()[0]!.id)
      : evalDetailView(session, listProEvals()[0]!.id)
  return {
    title: model.which === 'run' ? 'Pro run' : 'Pro eval',
    body: h.div([h.Id('pro-capture-root')], [body]),
  }
}

const container = document.getElementById('root')
if (container === null) throw new Error('missing #root container')

const program = Runtime.makeProgram<Model, Message>({
  Model,
  container,
  init: () => [{ which }, []],
  update: (model: Model) => [model, []],
  view,
})

Runtime.run(program)
;(window as unknown as { __proCaptureMounted?: boolean }).__proCaptureMounted =
  true
