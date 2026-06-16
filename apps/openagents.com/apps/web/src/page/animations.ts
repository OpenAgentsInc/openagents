import { Array } from 'effect'
import type { Html } from 'foldkit/html'
import { html } from 'foldkit/html'

import { lightBeamsView } from '../scene/lightBeamsElement'
import { particlesView } from '../scene/animations/particles'
import { wireframeView } from '../scene/animations/wireframe'
import * as Ui from '../ui'
import type { PublicHeaderAuthState } from './publicHeader'
import * as PublicHeader from './publicHeader'

const pageShellClass = 'min-h-dvh bg-[#000] text-[#f1efe8]'

// A three.js animation playground: a grid of self-contained WebGL experiments,
// each a custom element rendered in a framed tile. Add an experiment by writing
// a `mount` + `makeAnimationView` in scene/animations and dropping its view here.

const tile = <Message>(input: {
  title: string
  source: string
  node: Html
}): Html => {
  const h = html<Message>()

  return h.section(
    [Ui.className<Message>('border border-[#222] bg-white/[0.02] p-3')],
    [
      h.div(
        [
          Ui.className<Message>(
            'mb-2 flex flex-wrap items-baseline justify-between gap-2',
          ),
        ],
        [
          h.p(
            [
              Ui.className<Message>(
                'm-0 font-mono text-[0.75rem] uppercase tracking-[0.08em] text-white/45',
              ),
            ],
            [input.title],
          ),
          h.code(
            [Ui.className<Message>('font-mono text-[0.75rem] text-white/35')],
            [input.source],
          ),
        ],
      ),
      h.div(
        [
          Ui.className<Message>(
            'relative h-[320px] overflow-hidden border border-[#1a1a1a] bg-[#000]',
          ),
        ],
        [input.node],
      ),
    ],
  )
}

export const view = <Message>(
  authState: PublicHeaderAuthState<Message>,
): Html => {
  const h = html<Message>()

  const experiments: ReadonlyArray<{
    title: string
    source: string
    node: Html
  }> = [
    {
      node: lightBeamsView<Message>(),
      source: 'oa-light-beams',
      title: 'Light beams',
    },
    {
      node: wireframeView<Message>(),
      source: 'oa-anim-wireframe',
      title: 'Wireframe',
    },
    {
      node: particlesView<Message>(),
      source: 'oa-anim-particles',
      title: 'Particles',
    },
  ]

  return h.div(
    [Ui.className<Message>(pageShellClass)],
    [
      PublicHeader.view(authState),
      h.main(
        [
          h.AriaLabel('Animation playground'),
          Ui.className<Message>(
            'mx-auto w-[min(100%,1120px)] px-4 py-8',
          ),
        ],
        [
          h.p(
            [
              Ui.className<Message>(
                'mb-3 font-mono text-base text-white/35 sm:text-sm',
              ),
            ],
            ['Internal - three.js animation playground'],
          ),
          h.h1(
            [
              Ui.className<Message>(
                'm-0 text-balance text-3xl font-medium tracking-normal text-[#f1efe8] sm:text-4xl',
              ),
            ],
            ['Animations'],
          ),
          h.p(
            [
              Ui.className<Message>('mt-3 max-w-[76ch] text-base/7 text-white/60'),
            ],
            [
              'A scratch space for three.js / WebGL experiments. Each tile is a self-contained custom element from scene/animations; add one with a mount function + makeAnimationView.',
            ],
          ),
          h.div(
            [
              Ui.className<Message>(
                'mt-8 grid gap-4 sm:grid-cols-2',
              ),
            ],
            Array.map(experiments, experiment => tile<Message>(experiment)),
          ),
        ],
      ),
    ],
  )
}
