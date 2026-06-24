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

const Model = S.Struct({
  which: S.Literals([
    'eval',
    'run',
    'run-refuted',
    // #6190: the multi-target matrix run pages.
    'run-multitarget',
    'run-multitarget-block',
  ]),
})
type Model = typeof Model.Type
type Message = Readonly<{ _tag: 'Noop' }>

const session: Session = {
  userId: 'user_1',
  email: 'operator@openagents.com',
  name: 'Operator',
}

const params = new URLSearchParams(window.location.search)
const requested = params.get('view')
// #6192: `run-refuted` renders the REFUTED fixture run so the capture can prove
// a false claim renders as a refuted verdict (not a fake pass) on /pro.
// #6190: `run-multitarget` / `run-multitarget-block` render the per-target matrix
// run pages so the capture can prove per-target results show on /pro (incl. a
// read-only target blocking a mutating step honestly).
type Which =
  | 'eval'
  | 'run'
  | 'run-refuted'
  | 'run-multitarget'
  | 'run-multitarget-block'
const which: Which =
  requested === 'run'
    ? 'run'
    : requested === 'run-refuted'
      ? 'run-refuted'
      : requested === 'run-multitarget'
        ? 'run-multitarget'
        : requested === 'run-multitarget-block'
          ? 'run-multitarget-block'
          : 'eval'

// The REFUTED run fixture id (the FALSE redirect claim).
const REFUTED_RUN_ID = 'login-redirect-claim-refuted'
// The multi-target fixture run ids (#6190).
const MULTI_TARGET_RUN_ID = 'login-multi-target'
const MULTI_TARGET_BLOCK_RUN_ID = 'submit-login-multi-target'

const runIdFor = (w: Which): string | undefined => {
  switch (w) {
    case 'run':
      return listProRuns()[0]!.id
    case 'run-refuted':
      return REFUTED_RUN_ID
    case 'run-multitarget':
      return MULTI_TARGET_RUN_ID
    case 'run-multitarget-block':
      return MULTI_TARGET_BLOCK_RUN_ID
    default:
      return undefined
  }
}

const view = (model: Model): Document => {
  const h = html<Message>()
  const runId = runIdFor(model.which)
  const body =
    runId !== undefined
      ? runDetailView(session, runId)
      : evalDetailView(session, listProEvals()[0]!.id)
  return {
    title: model.which === 'eval' ? 'Pro eval' : `Pro ${model.which}`,
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
