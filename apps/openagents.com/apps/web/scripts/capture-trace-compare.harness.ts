// Headless render harness for the public `/trace/compare/{ids}` view (issue
// #6211 — the real "chill-evals": compare N agent traces, shareable).
//
// Mounts the REAL `TraceCompare.view` Foldkit view with the committed sample
// trace-set via a minimal Foldkit program, so the capture script exercises the
// actual page composition (header + comparison table + deltas + deep links) —
// NOT a re-implementation — without the app's auth bootstrap or any network.
//
// The view to mount is chosen by `?view=found|unknown|empty` on the URL.

import { Schema as S } from 'effect'
import { Runtime } from 'foldkit'
import { html } from 'foldkit/html'
import type { Document } from 'foldkit/html'

import { TraceCompareRoute } from '../src/route'
import * as TraceCompare from '../src/page/trace-compare'
import {
  SAMPLE_COMPARE_BASELINE_UUID,
  SAMPLE_COMPARE_PATH_IDS,
} from '../src/page/trace-compare/sample'

const Model = S.Struct({
  which: S.Literals(['found', 'unknown', 'empty']),
})
type Model = typeof Model.Type
type Message = Readonly<{ _tag: 'Noop' }>

const params = new URLSearchParams(window.location.search)
const requested = params.get('view')
const which: Model['which'] =
  requested === 'unknown'
    ? 'unknown'
    : requested === 'empty'
      ? 'empty'
      : 'found'

const idsFor = (model: Model): string => {
  switch (model.which) {
    case 'found':
      return SAMPLE_COMPARE_PATH_IDS
    case 'unknown':
      return `${SAMPLE_COMPARE_BASELINE_UUID},does-not-exist-0000`
    case 'empty':
      return ' , , '
  }
}

const view = (model: Model): Document => {
  const h = html<Message>()
  const body = TraceCompare.view<Message>(
    TraceCompareRoute({ ids: idsFor(model) }),
    { _tag: 'LoggedOut' },
  )
  return {
    title: `Trace compare capture (${model.which})`,
    body: h.div([h.Id('trace-compare-capture-root')], [body]),
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

declare global {
  interface Window {
    __traceCompareCaptureMounted?: boolean
  }
}

window.__traceCompareCaptureMounted = true
