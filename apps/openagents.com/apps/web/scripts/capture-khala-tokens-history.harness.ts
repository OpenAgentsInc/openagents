// Headless render harness for the "Khala Tokens Served" history chart (#6227).
//
// Boots a minimal real Foldkit program whose view is the actual
// `khalaTokensServedPanel` (the live counter + the hand-rolled SVG history
// chart) so the screenshot exercises the production view code, not a
// re-implementation. The query string selects the state to render: `loaded`
// (sample series) or `empty`. Served by Vite (see capture-khala-tokens-history.ts).

import { Effect, Schema as S } from 'effect'
import { Runtime } from 'foldkit'
import { type Document, html } from 'foldkit/html'

import * as Home from '../src/page/loggedOut/page/home'
import {
  IdlePublicKhalaTokensServed,
  LoadedPublicKhalaTokensServed,
  LoadedPublicKhalaTokensServedHistory,
  PublicKhalaTokensServed,
  PublicKhalaTokensServedHistory,
} from '../src/page/loggedOut/model'

const sampleSeries = [
  { day: '2026-05-26', tokensServed: 18_200 },
  { day: '2026-05-27', tokensServed: 24_700 },
  { day: '2026-05-28', tokensServed: 9_400 },
  { day: '2026-05-29', tokensServed: 41_900 },
  { day: '2026-05-30', tokensServed: 33_100 },
  { day: '2026-05-31', tokensServed: 52_600 },
  { day: '2026-06-01', tokensServed: 12_000 },
  { day: '2026-06-02', tokensServed: 28_800 },
  { day: '2026-06-03', tokensServed: 0 },
  { day: '2026-06-04', tokensServed: 47_300 },
  { day: '2026-06-05', tokensServed: 61_500 },
  { day: '2026-06-06', tokensServed: 39_900 },
  { day: '2026-06-07', tokensServed: 70_200 },
  { day: '2026-06-08', tokensServed: 55_400 },
  { day: '2026-06-09', tokensServed: 22_100 },
  { day: '2026-06-10', tokensServed: 84_700 },
  { day: '2026-06-11', tokensServed: 66_300 },
  { day: '2026-06-12', tokensServed: 31_800 },
  { day: '2026-06-13', tokensServed: 49_500 },
  { day: '2026-06-14', tokensServed: 12_900 },
  { day: '2026-06-15', tokensServed: 58_100 },
  { day: '2026-06-16', tokensServed: 73_400 },
  { day: '2026-06-17', tokensServed: 90_000 },
  { day: '2026-06-18', tokensServed: 44_600 },
  { day: '2026-06-19', tokensServed: 67_200 },
  { day: '2026-06-20', tokensServed: 38_500 },
  { day: '2026-06-21', tokensServed: 81_900 },
  { day: '2026-06-22', tokensServed: 25_300 },
  { day: '2026-06-23', tokensServed: 96_250 },
  { day: '2026-06-24', tokensServed: 51_700 },
]

const variant = new URLSearchParams(window.location.search).get('state') ?? 'loaded'

const totalServed = sampleSeries.reduce(
  (sum, point) => sum + point.tokensServed,
  0,
)

const Model = S.Struct({})
type Model = typeof Model.Type

const Message = S.Union([S.Struct({ _tag: S.Literal('Noop') })])
type Message = typeof Message.Type

const counter =
  variant === 'empty'
    ? IdlePublicKhalaTokensServed()
    : LoadedPublicKhalaTokensServed({
        served: PublicKhalaTokensServed.make({
          tokensServed: totalServed,
          generatedAt: '2026-06-24T12:00:00.000Z',
        }),
      })

const history = LoadedPublicKhalaTokensServedHistory({
  history: PublicKhalaTokensServedHistory.make({
    window: '30d',
    bucket: 'day',
    series: variant === 'empty' ? [] : sampleSeries,
  }),
})

const view = (): Document => {
  const h = html<Message>()

  return {
    title: 'Khala Tokens Served history capture',
    body: h.div(
      [
        h.Attribute(
          'style',
          'background:#000;padding:32px;max-width:880px;margin:0 auto;font-family:ui-monospace,monospace;',
        ),
      ],
      [Home.khalaTokensServedPanel(counter, history)],
    ),
  }
}

const program = Runtime.makeProgram({
  Model,
  Flags: S.Struct({}),
  flags: Effect.succeed({}),
  init: (): readonly [Model, ReadonlyArray<never>] => [{}, []],
  update: (model: Model): readonly [Model, ReadonlyArray<never>] => [model, []],
  view,
  container: document.getElementById('root'),
  devTools: { Message },
})

Runtime.run(program)
Reflect.set(window, '__khalaHistoryMounted', true)
