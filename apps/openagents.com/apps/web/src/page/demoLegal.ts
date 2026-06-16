import { Array } from 'effect'
import type { Html } from 'foldkit/html'
import { html } from 'foldkit/html'

import { demoLegalSceneView } from '../scene/demoLegalScene'
import * as Ui from '../ui'
import type { PublicHeaderAuthState } from './publicHeader'
import * as PublicHeader from './publicHeader'

// Demo business landing page for the legal vertical.
//
// Showcases the Forge "legal MVP" (Design Sprint 4 follow-up + ROADMAP E3) as
// polished cards that flutter in and stay anchored to a 3D constellation scene
// via three-effect's htmlOverlay primitives (the drei `<Html>` equivalent —
// see scene/demoLegalScene.ts). Honest, demo-framed, dark-only. The theme is
// review-gated, source-linked, human-in-the-loop legal work — NOT an "AI lawyer"
// or a static "legal AI dashboard". No real client data.

const pageShellClass = 'relative h-dvh overflow-hidden bg-[#000] text-[#f1efe8]'

// Cards start hidden; the scene clock flutters them in (opacity/translate/
// scale/rotation) once the GL backdrop mounts. See demoLegalScene.ts.
const cardBaseClass =
  'pointer-events-auto w-[min(86vw,360px)] border border-[#1f2937] bg-[#05070b]/90 p-4 shadow-[0_18px_50px_-20px_rgba(0,0,0,0.9)] backdrop-blur-sm'

const cardLabelClass =
  'm-0 font-mono text-[0.7rem] uppercase tracking-[0.12em] text-[#7da3d9]'

const pillClass =
  'inline-flex items-center gap-1 border border-[#1f2937] bg-white/[0.03] px-2 py-1 font-mono text-[0.7rem] text-white/70'

const card = <Message>(input: {
  anchor: string
  children: ReadonlyArray<Html>
  extraClass?: string
}): Html => {
  const h = html<Message>()

  return h.div(
    [
      h.DataAttribute('anchor', input.anchor),
      h.DataAttribute('demo-legal-card', input.anchor),
      Ui.className<Message>(
        `${cardBaseClass}${input.extraClass === undefined ? '' : ` ${input.extraClass}`}`,
      ),
      // Hidden until the scene clock positions + flutters it in.
      h.Style({ opacity: '0', position: 'absolute' }),
    ],
    input.children,
  )
}

const checklistItem = <Message>(label: string): Html => {
  const h = html<Message>()

  return h.li(
    [
      Ui.className<Message>(
        'flex items-start gap-2 text-[0.8rem] text-white/75',
      ),
    ],
    [
      h.span([Ui.className<Message>('mt-[2px] text-[#7da3d9]')], ['[ ]']),
      h.span([], [label]),
    ],
  )
}

const commandBarCard = <Message>(): Html => {
  const h = html<Message>()

  return card<Message>({
    anchor: 'command-bar',
    extraClass: 'w-[min(90vw,520px)]',
    children: [
      h.p([Ui.className<Message>(cardLabelClass)], ['Command bar']),
      h.div(
        [
          Ui.className<Message>(
            'mt-2 flex items-center gap-2 border border-[#243044] bg-black/60 px-3 py-2',
          ),
        ],
        [
          h.span([Ui.className<Message>('font-mono text-[#7da3d9]')], ['>']),
          h.span(
            [Ui.className<Message>('text-[0.92rem] text-[#f1efe8]')],
            ['I need an NDA for a Texas startup talking to a vendor.'],
          ),
        ],
      ),
      h.p(
        [Ui.className<Message>('mt-2 text-[0.78rem] text-white/55')],
        [
          'Talk or type. Forge prepares the work surface — you stay in strategic counsel mode.',
        ],
      ),
    ],
  })
}

const quickActionsCard = <Message>(): Html => {
  const h = html<Message>()

  const actions = ['Find a form', 'Prepare a consult', 'Review this draft']

  return card<Message>({
    anchor: 'quick-actions',
    extraClass: 'w-[min(90vw,520px)]',
    children: [
      h.p([Ui.className<Message>(cardLabelClass)], ['Quick actions']),
      h.div(
        [Ui.className<Message>('mt-2 flex flex-wrap gap-2')],
        Array.map(actions, action =>
          h.button(
            [
              h.Type('button'),
              Ui.className<Message>(
                `${pillClass} transition-colors hover:border-[#3a72b0] hover:text-white`,
              ),
            ],
            [action],
          ),
        ),
      ),
    ],
  })
}

const ndaDraftCard = <Message>(): Html => {
  const h = html<Message>()

  return card<Message>({
    anchor: 'nda-draft',
    children: [
      h.div(
        [Ui.className<Message>('flex items-center justify-between gap-2')],
        [
          h.p([Ui.className<Message>(cardLabelClass)], ['NDA draft']),
          h.span(
            [
              Ui.className<Message>(
                'border border-[#3a72b0]/60 px-2 py-[2px] font-mono text-[0.65rem] uppercase tracking-[0.1em] text-[#7da3d9]',
              ),
            ],
            ['Draft only'],
          ),
        ],
      ),
      h.p(
        [Ui.className<Message>('mt-1 text-[0.8rem] text-white/60')],
        ['Mutual NDA · Texas · startup ↔ vendor'],
      ),
      h.p(
        [Ui.className<Message>('mt-2 text-[0.78rem] text-white/70')],
        [
          'Form pulled from your library; clauses adapted in the form’s style. New beta-access language flagged for attorney review.',
        ],
      ),
      h.p(
        [Ui.className<Message>('mt-2 font-mono text-[0.72rem] text-white/45')],
        ['Source: library/nda-mutual-tx.docx · option B'],
      ),
    ],
  })
}

const reviewChecklistCard = <Message>(): Html => {
  const h = html<Message>()

  return card<Message>({
    anchor: 'review-checklist',
    children: [
      h.p([Ui.className<Message>(cardLabelClass)], ['Lawyer review checklist']),
      h.ul(
        [Ui.className<Message>('mt-2 flex flex-col gap-[6px]')],
        [
          checklistItem<Message>('Confirm parties and signing authority'),
          checklistItem<Message>('Verify Texas governing-law clause'),
          checklistItem<Message>('Review beta-access materials carve-out'),
          checklistItem<Message>('Set confidentiality term and survival'),
        ],
      ),
      h.p(
        [Ui.className<Message>('mt-2 text-[0.74rem] text-white/45')],
        ['Nothing relied on until you check it off.'],
      ),
    ],
  })
}

const timeEntryCard = <Message>(): Html => {
  const h = html<Message>()

  return card<Message>({
    anchor: 'time-entry',
    children: [
      h.div(
        [Ui.className<Message>('flex items-center justify-between gap-2')],
        [
          h.p([Ui.className<Message>(cardLabelClass)], ['Time entry']),
          h.span(
            [
              Ui.className<Message>(
                'border border-amber-500/50 px-2 py-[2px] font-mono text-[0.65rem] uppercase tracking-[0.1em] text-amber-300',
              ),
            ],
            ['Pending approval'],
          ),
        ],
      ),
      h.p(
        [Ui.className<Message>('mt-2 text-[0.82rem] text-white/80')],
        ['0.4 hr — NDA prep & clause adaptation (vendor matter)'],
      ),
      h.p(
        [Ui.className<Message>('mt-2 text-[0.78rem] text-white/60')],
        [
          'Follow-up task: send intake questions to client — not sent until you approve.',
        ],
      ),
    ],
  })
}

const matterWorkspaceCard = <Message>(): Html => {
  const h = html<Message>()

  const column = (title: string, items: ReadonlyArray<string>): Html =>
    h.div(
      [Ui.className<Message>('flex flex-col gap-1')],
      [
        h.p(
          [
            Ui.className<Message>(
              'm-0 font-mono text-[0.66rem] uppercase tracking-[0.1em] text-white/40',
            ),
          ],
          [title],
        ),
        ...Array.map(items, item =>
          h.p([Ui.className<Message>('text-[0.74rem] text-white/70')], [item]),
        ),
      ],
    )

  return card<Message>({
    anchor: 'matter-workspace',
    children: [
      h.p([Ui.className<Message>(cardLabelClass)], ['Matter workspace']),
      h.div(
        [Ui.className<Message>('mt-2 grid grid-cols-2 gap-3')],
        [
          column('Sources', ['Client note', 'NDA template']),
          column('Assumptions', ['Mutual NDA', 'TX governing law']),
          column('Drafts', ['NDA v0.1 (draft)']),
          column('Review notes', ['Beta carve-out — needs review']),
        ],
      ),
    ],
  })
}

const dailyBriefCard = <Message>(): Html => {
  const h = html<Message>()

  return card<Message>({
    anchor: 'daily-brief',
    children: [
      h.p([Ui.className<Message>(cardLabelClass)], ['Daily brief']),
      h.p(
        [Ui.className<Message>('mt-2 text-[0.82rem] text-white/80')],
        ['Which consults or matters are missing information?'],
      ),
      h.ul(
        [Ui.className<Message>('mt-2 flex flex-col gap-1')],
        [
          h.li(
            [Ui.className<Message>('text-[0.76rem] text-white/65')],
            ['Vendor NDA — awaiting confidential-materials answer'],
          ),
          h.li(
            [Ui.className<Message>('text-[0.76rem] text-white/65')],
            ['Shareholder agreement — missing entity details'],
          ),
        ],
      ),
      h.p(
        [Ui.className<Message>('mt-2 text-[0.72rem] text-white/40')],
        ['Built from selected sources only — no blanket mailbox access.'],
      ),
    ],
  })
}

export const view = <Message>(
  authState: PublicHeaderAuthState<Message>,
): Html => {
  const h = html<Message>()

  const cards: ReadonlyArray<Html> = [
    commandBarCard<Message>(),
    quickActionsCard<Message>(),
    ndaDraftCard<Message>(),
    reviewChecklistCard<Message>(),
    timeEntryCard<Message>(),
    matterWorkspaceCard<Message>(),
    dailyBriefCard<Message>(),
  ]

  return h.div(
    [Ui.className<Message>(pageShellClass)],
    [
      // Header sits above the GL backdrop.
      h.div(
        [Ui.className<Message>('relative z-30')],
        [PublicHeader.view(authState)],
      ),
      // The workbench: GL constellation backdrop (pointer-none) with the
      // 3D-anchored legal cards projected over it via three-effect htmlOverlay.
      h.main(
        [
          h.AriaLabel('Legal workbench demo'),
          Ui.className<Message>('relative h-[calc(100dvh-64px)] w-full'),
        ],
        [
          // three-effect constellation scene + htmlOverlay driver.
          demoLegalSceneView<Message>(),
          // Intro copy, centered behind the assembling cards.
          h.div(
            [
              Ui.className<Message>(
                // z-30 keeps the headline above the z-20 card overlay so the
                // intro text is never obscured, even if a card's projected
                // anchor lands under it. pointer-events-none lets clicks pass
                // through to the cards beneath.
                'pointer-events-none absolute inset-x-0 top-6 z-30 mx-auto w-[min(100%,820px)] px-4 text-center',
              ),
            ],
            [
              h.p(
                [
                  Ui.className<Message>(
                    'm-0 font-mono text-[0.72rem] uppercase tracking-[0.14em] text-white/40',
                  ),
                ],
                ['Demo · legal vertical · Forge legal MVP'],
              ),
              h.h1(
                [
                  Ui.className<Message>(
                    'mx-auto mt-2 max-w-[18ch] text-balance text-2xl font-medium tracking-normal text-[#f1efe8] sm:text-4xl',
                  ),
                ],
                [
                  'Stay in strategic counsel mode — Forge prepares the work surface.',
                ],
              ),
            ],
          ),
          // The anchored card overlay. Each child carries data-anchor and is
          // positioned/fluttered imperatively by the scene (demoLegalScene.ts).
          h.div(
            [
              h.DataAttribute('demo-legal-overlay', ''),
              Ui.className<Message>(
                'pointer-events-none absolute inset-0 z-20',
              ),
            ],
            cards,
          ),
          // Honest footer framing.
          h.p(
            [
              Ui.className<Message>(
                'pointer-events-none absolute inset-x-0 bottom-4 z-10 text-center font-mono text-[0.7rem] text-white/35',
              ),
            ],
            [
              'Demo only — not a live legal product. Review-gated, source-linked, human-in-the-loop. No real client data.',
            ],
          ),
        ],
      ),
    ],
  )
}
