import type { Html } from 'foldkit/html'
import { html } from 'foldkit/html'

import * as Ui from '../ui'
import type { PublicHeaderAuthState } from './publicHeader'
import * as PublicHeader from './publicHeader'

export const KHALA_CODE_DOWNLOAD_ROUTE_PATH = '/code/download'
export const KHALA_CODE_DOWNLOAD_COUNTER_ENDPOINT =
  '/api/public/khala-code/download-counts'
export const KHALA_CODE_PROMISE_ID = 'khala_code.desktop_codex_wrapper.v1'
export const KHALA_CODE_PROMISE_URL = '/api/public/product-promises'
export const KHALA_CODE_DESKTOP_PRODUCT = 'khala-code-desktop'
export const KHALA_CODE_RELEASE_FEED_URL =
  'https://updates.openagents.com/desktop/khala-code-desktop/rc/feed.json'
export const KHALA_CODE_CLI_INSTALL_COMMAND =
  'npm install -g @openagentsinc/khala'
export const CODEX_INSTALL_COMMAND = 'npm install -g @openai/codex'
export const CODEX_LOGIN_COMMAND = 'codex login'
export const KHALA_CODE_SOURCE_BUILD_COMMANDS =
  'git clone --depth 1 https://github.com/OpenAgentsInc/openagents\n' +
  'cd openagents\n' +
  'bun install\n' +
  'bun run dev:khala-code-desktop'

export const KHALA_CODE_DOWNLOAD_COPY_GATE = {
  gateRef: 'gate.public.khala_code.download_copy.v1',
  promiseId: KHALA_CODE_PROMISE_ID,
  state: 'yellow',
  safeCopy:
    'Khala Code wraps the user-owned local Codex install. The public npm khala CLI install path is available; the desktop DMG path is release-lane ready but pending a signed public artifact and outside-user evidence.',
  unsafeCopy:
    'Do not claim Khala Code is downloadable, installed by outside users, or usable without a working Codex install and login. Do not describe the free/paid plan economics as live.',
} as const

export type KhalaCodeDownloadCopyViolation = Readonly<{
  phraseRef: string
  matchedText: string
}>

export const KHALA_CODE_DOWNLOAD_DENIED_COPY_PATTERNS: ReadonlyArray<
  Readonly<{ phraseRef: string; pattern: RegExp }>
> = [
  {
    phraseRef: 'phrase.public.khala_code.downloadable_now',
    pattern:
      /\b(?:khala code|desktop app|desktop dmg)\b[^.!?\n]{0,80}\b(?:downloadable|available now|ready to download|download now)\b/i,
  },
  {
    phraseRef: 'phrase.public.khala_code.works_without_codex',
    pattern:
      /\b(?:works|runs|usable|use)\b[^.!?\n]{0,80}\bwithout\b[^.!?\n]{0,20}\bcodex\b/i,
  },
  {
    phraseRef: 'phrase.public.khala_code.live_plan_economics',
    pattern:
      /\b(?:free|paid)\s+plan\b[^.!?\n]{0,120}\b(?:live|launched|purchasable|earning|pays? you|revenue)\b/i,
  },
]

export const khalaCodeDownloadCopyViolations = (
  text: string,
): ReadonlyArray<KhalaCodeDownloadCopyViolation> =>
  KHALA_CODE_DOWNLOAD_DENIED_COPY_PATTERNS.flatMap(rule => {
    const match = rule.pattern.exec(text)
    return match === null
      ? []
      : [{ phraseRef: rule.phraseRef, matchedText: match[0] }]
  })

const pageShellClass = 'h-dvh overflow-auto bg-[#000] text-[#f1efe8]'
const sectionLabelClass = 'm-0 font-mono text-[0.75rem] uppercase text-white/35'
const panelClass = 'grid gap-3 border border-[#222] bg-[#010102] p-5 sm:p-6'

const commandBlock = <Message>(command: string, cta: string): Html => {
  const h = html<Message>()

  return h.pre(
    [
      h.DataAttribute('cta', cta),
      Ui.className<Message>(
        'm-0 w-full select-all overflow-x-auto border border-white/15 bg-[#030303] px-3 py-3 text-left font-mono text-[0.8125rem] leading-6 text-[#d6f6ff]',
      ),
    ],
    [h.code([], [command])],
  )
}

const statusPill = <Message>(tone: 'ready' | 'gated', label: string): Html => {
  const h = html<Message>()
  const toneClass =
    tone === 'ready'
      ? 'border-[#00c853]/45 bg-[#001d0b] text-[#8fffb9]'
      : 'border-[#ffb400]/45 bg-[#1a1403] text-[#ffd36a]'

  return h.span(
    [
      Ui.className<Message>(
        `inline-flex w-fit items-center border px-2 py-1 font-mono text-[0.6875rem] uppercase ${toneClass}`,
      ),
    ],
    [label],
  )
}

const heroView = <Message>(): Html => {
  const h = html<Message>()

  return h.section(
    [Ui.className<Message>('grid gap-4 border-b border-[#222] pb-7')],
    [
      h.a(
        [
          h.Href('/code'),
          Ui.className<Message>(
            'w-fit font-mono text-[0.75rem] uppercase text-[#8fb6ff] underline underline-offset-2 hover:text-white',
          ),
        ],
        ['<- Khala Code'],
      ),
      h.div(
        [Ui.className<Message>('grid gap-2')],
        [
          h.p([Ui.className<Message>(sectionLabelClass)], ['Khala Code install truth']),
          h.h1(
            [
              Ui.className<Message>(
                'm-0 text-balance text-3xl font-medium tracking-normal text-[#f1efe8] sm:text-4xl',
              ),
            ],
            ['Install paths, with the Codex requirement kept visible'],
          ),
          h.p(
            [Ui.className<Message>('m-0 max-w-[72ch] text-base/7 text-white/65')],
            [
              'Khala Code is the OpenAgents coding app around your own local Codex install. The default desktop harness requires the Codex CLI and a signed-in primary Codex home before it can run coding turns.',
            ],
          ),
        ],
      ),
    ],
  )
}

const codexPrereqView = <Message>(): Html => {
  const h = html<Message>()

  return h.section(
    [h.DataAttribute('install-step', 'codex-required'), Ui.className<Message>(panelClass)],
    [
      h.p([Ui.className<Message>(sectionLabelClass)], ['Required first']),
      h.h2([Ui.className<Message>('m-0 text-lg font-medium')], ['Install and sign in to Codex']),
      h.p(
        [Ui.className<Message>('m-0 text-base/7 text-white/60')],
        [
          'Khala Code does not bundle or replace Codex Core. Run the Codex install and login yourself for the primary user Codex home.',
        ],
      ),
      commandBlock<Message>(
        `${CODEX_INSTALL_COMMAND}\n${CODEX_LOGIN_COMMAND}`,
        'install-codex-command',
      ),
    ],
  )
}

const desktopPathView = <Message>(): Html => {
  const h = html<Message>()

  return h.section(
    [h.DataAttribute('install-path', 'desktop-dmg'), Ui.className<Message>(panelClass)],
    [
      h.div(
        [Ui.className<Message>('flex flex-wrap items-center justify-between gap-3')],
        [
          h.p([Ui.className<Message>(sectionLabelClass)], ['Desktop DMG']),
          statusPill<Message>('gated', 'public artifact pending'),
        ],
      ),
      h.h2([Ui.className<Message>('m-0 text-lg font-medium')], ['macOS release lane']),
      h.p(
        [Ui.className<Message>('m-0 text-base/7 text-white/60')],
        [
          'The Khala Code desktop release lane exists for signed/notarized macOS builds, but no public signed DMG receipt is recorded here yet. Use the source build path until a public artifact appears with owner release receipts.',
        ],
      ),
      h.dl(
        [Ui.className<Message>('m-0 grid gap-2 border-t border-white/10 pt-3 font-mono text-xs')],
        [
          h.div(
            [Ui.className<Message>('grid gap-1 sm:grid-cols-[8rem_1fr]')],
            [
              h.dt([Ui.className<Message>('text-white/35')], ['Product']),
              h.dd([Ui.className<Message>('m-0 text-white/70')], [KHALA_CODE_DESKTOP_PRODUCT]),
            ],
          ),
          h.div(
            [Ui.className<Message>('grid gap-1 sm:grid-cols-[8rem_1fr]')],
            [
              h.dt([Ui.className<Message>('text-white/35')], ['Feed']),
              h.dd(
                [Ui.className<Message>('m-0 break-all text-white/70')],
                [KHALA_CODE_RELEASE_FEED_URL],
              ),
            ],
          ),
        ],
      ),
    ],
  )
}

const cliPathView = <Message>(): Html => {
  const h = html<Message>()

  return h.section(
    [h.DataAttribute('install-path', 'khala-cli'), Ui.className<Message>(panelClass)],
    [
      h.div(
        [Ui.className<Message>('flex flex-wrap items-center justify-between gap-3')],
        [
          h.p([Ui.className<Message>(sectionLabelClass)], ['Terminal']),
          statusPill<Message>('ready', 'npm package'),
        ],
      ),
      h.h2([Ui.className<Message>('m-0 text-lg font-medium')], ['Install the khala CLI']),
      h.p(
        [Ui.className<Message>('m-0 text-base/7 text-white/60')],
        [
          'The CLI is the public terminal path for Khala chat, Codex account connection, and fleet commands. Fleet coding still requires your own Codex account.',
        ],
      ),
      commandBlock<Message>(KHALA_CODE_CLI_INSTALL_COMMAND, 'install-khala-cli-command'),
    ],
  )
}

const sourcePathView = <Message>(): Html => {
  const h = html<Message>()

  return h.section(
    [h.DataAttribute('install-path', 'source-build'), Ui.className<Message>(panelClass)],
    [
      h.p([Ui.className<Message>(sectionLabelClass)], ['Desktop from source']),
      h.h2([Ui.className<Message>('m-0 text-lg font-medium')], ['Run Khala Code from the repo']),
      h.p(
        [Ui.className<Message>('m-0 text-base/7 text-white/60')],
        [
          'This is the supported desktop path while the signed DMG remains receipt-gated. Clone shallow and install workspace dependencies at the repo root.',
        ],
      ),
      commandBlock<Message>(
        KHALA_CODE_SOURCE_BUILD_COMMANDS,
        'run-khala-code-source-command',
      ),
    ],
  )
}

const countView = <Message>(): Html => {
  const h = html<Message>()

  return h.section(
    [
      h.DataAttribute('download-counter', 'khala-code'),
      Ui.className<Message>(panelClass),
    ],
    [
      h.p([Ui.className<Message>(sectionLabelClass)], ['Counter']),
      h.h2([Ui.className<Message>('m-0 text-lg font-medium')], ['Exact rows only']),
      h.p(
        [Ui.className<Message>('m-0 text-base/7 text-white/60')],
        [
          'The public counter endpoint exposes only exact download ledger rows. If there are no rows, it returns an empty counts array instead of a synthesized number.',
        ],
      ),
      h.a(
        [
          h.Href(KHALA_CODE_DOWNLOAD_COUNTER_ENDPOINT),
          Ui.className<Message>(
            'break-all font-mono text-xs text-[#8fb6ff] underline underline-offset-2 hover:text-white',
          ),
        ],
        [KHALA_CODE_DOWNLOAD_COUNTER_ENDPOINT],
      ),
    ],
  )
}

const promiseGateView = <Message>(): Html => {
  const h = html<Message>()

  return h.section(
    [
      h.DataAttribute('promise-gate', KHALA_CODE_PROMISE_ID),
      Ui.className<Message>(
        'grid gap-2 border border-[#ffb400]/40 bg-[#1a1403] px-4 py-3 text-sm/6 text-white/80',
      ),
    ],
    [
      h.p([Ui.className<Message>('m-0 font-mono text-xs uppercase text-[#ffb400]')], ['Copy gate']),
      h.p([Ui.className<Message>('m-0')], [KHALA_CODE_DOWNLOAD_COPY_GATE.safeCopy]),
      h.a(
        [
          h.Href(KHALA_CODE_PROMISE_URL),
          Ui.className<Message>(
            'w-fit font-mono text-xs text-[#ffdd8a] underline underline-offset-2 hover:text-white',
          ),
        ],
        ['Product promise registry'],
      ),
    ],
  )
}

export const view = <Message>(
  authState: PublicHeaderAuthState<Message>,
): Html => {
  const h = html<Message>()

  return h.div(
    [
      h.DataAttribute('route', 'khala-code-download'),
      Ui.className<Message>(pageShellClass),
    ],
    [
      PublicHeader.view(authState),
      h.main(
        [
          h.AriaLabel('Khala Code install paths'),
          Ui.className<Message>(
            'mx-auto grid w-[min(100%,980px)] gap-8 px-4 py-8 lg:grid-cols-[minmax(0,1fr)_minmax(20rem,24rem)]',
          ),
        ],
        [
          h.div(
            [Ui.className<Message>('grid content-start gap-6')],
            [
              heroView<Message>(),
              codexPrereqView<Message>(),
              desktopPathView<Message>(),
              cliPathView<Message>(),
              sourcePathView<Message>(),
            ],
          ),
          h.div(
            [Ui.className<Message>('grid content-start gap-6')],
            [promiseGateView<Message>(), countView<Message>()],
          ),
        ],
      ),
    ],
  )
}
