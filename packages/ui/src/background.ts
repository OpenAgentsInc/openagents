import { clsx } from 'clsx'
import type { Html } from 'foldkit/html'
import { html } from 'foldkit/html'

// A single light beam: a tall white→transparent gradient sliver, fanned from a
// shared top-center origin and softened with blur. `extra` sets width, rotation,
// and per-beam opacity so a set of beams reads as volumetric light rays.
const beam = <Message>(extra: string): Html => {
  const h = html<Message>()

  return h.div(
    [
      h.Class(
        clsx(
          'absolute left-1/2 top-[-25%] h-[170%] origin-top -translate-x-1/2 bg-gradient-to-b from-white/50 to-transparent blur',
          extra,
        ),
      ),
    ],
    [],
  )
}

// Decorative light-rays background layer: a soft top glow plus a fan of beams,
// absolutely positioned to fill (and clip to) the nearest relatively-positioned
// ancestor. Pure CSS/Foldkit so it renders anywhere Html does (library preview,
// login screen) and ships without a WebGL canvas. Honors the dark contract:
// faint white light on pure black, never color-only meaning.
export const lightRays = <Message>(
  input: { className?: string } = {},
): Html => {
  const h = html<Message>()

  return h.div(
    [
      h.AriaHidden(true),
      h.Class(
        clsx(
          'pointer-events-none absolute inset-0 overflow-hidden',
          input.className,
        ),
      ),
    ],
    [
      h.div(
        [
          h.Class(
            'absolute left-1/2 top-[-30%] h-[80%] w-[90%] -translate-x-1/2 rounded-full bg-white/5 blur-3xl',
          ),
        ],
        [],
      ),
      beam<Message>('w-px -rotate-[32deg] opacity-20'),
      beam<Message>('w-0.5 -rotate-[20deg] opacity-30'),
      beam<Message>('w-1 -rotate-[9deg] opacity-40'),
      beam<Message>('w-1.5 rotate-0 opacity-40'),
      beam<Message>('w-1 rotate-[9deg] opacity-40'),
      beam<Message>('w-0.5 rotate-[20deg] opacity-30'),
      beam<Message>('w-px rotate-[32deg] opacity-20'),
    ],
  )
}
