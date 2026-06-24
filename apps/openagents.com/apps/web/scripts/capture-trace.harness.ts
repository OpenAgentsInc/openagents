// Headless render harness for the public `/trace/{uuid}` page (issue #6209).
//
// Mounts the REAL `Trace.view` Foldkit view with the committed sample trajectory
// via a minimal Foldkit program, so the capture script exercises the actual page
// composition (header + timeline + tool calls + observations + video + metrics)
// — NOT a re-implementation — without the app's auth bootstrap or any network.
//
// The view to mount is chosen by `?view=found|not-found|skeleton` on the URL.

import { Schema as S } from 'effect'
import { Runtime } from 'foldkit'
import { html } from 'foldkit/html'
import type { Document } from 'foldkit/html'

import { TraceRoute } from '../src/route'
import * as Trace from '../src/page/trace'
import { SAMPLE_TRACE_UUID } from '../src/page/trace/sample'

const Model = S.Struct({
  which: S.Literals(['found', 'not-found', 'skeleton']),
})
type Model = typeof Model.Type
type Message = Readonly<{ _tag: 'Noop' }>

const params = new URLSearchParams(window.location.search)
const requested = params.get('view')
const which: Model['which'] =
  requested === 'not-found'
    ? 'not-found'
    : requested === 'skeleton'
      ? 'skeleton'
      : 'found'

const view = (model: Model): Document => {
  const h = html<Message>()
  const body =
    model.which === 'skeleton'
      ? h.div(
          [Ui_pageShellClass()],
          [Trace.skeletonArticle<Message>()],
        )
      : Trace.view<Message>(
          TraceRoute({
            uuid:
              model.which === 'not-found'
                ? 'does-not-exist-0000'
                : SAMPLE_TRACE_UUID,
          }),
          { _tag: 'LoggedOut' },
        )
  return {
    title: `Trace capture (${model.which})`,
    body: h.div([h.Id('trace-capture-root')], [body]),
  }
}

// The skeleton harness wraps the bare article in a black page shell so the
// screenshot matches the real page background.
const Ui_pageShellClass = () =>
  html<Message>().Class('min-h-dvh bg-[#000] text-[#f1efe8]')

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
    __traceCaptureMounted?: boolean
  }
}

window.__traceCaptureMounted = true
