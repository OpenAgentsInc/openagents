import type { Html } from 'foldkit/html'
import { html } from 'foldkit/html'

import * as Ui from '../ui'
import type { PublicHeaderAuthState } from './publicHeader'
import * as PublicHeader from './publicHeader'

// Public `openagents.com/download` page (AO-5, #5446 / EPIC #5441).
//
// Re-surfaces a discoverable, signed-DMG download for the Autopilot Desktop app
// after the homepage "Download Autopilot" CTA was removed on 2026-06-18
// (`b85391e2b`, which refocused the homepage on Pylon-CLI). A non-technical
// human who watched the launch video now has a friendly product page to find
// the app, instead of GitHub spelunking.
//
// HONEST-SCOPE GATE (owner-flippable). The signed + notarized DMG that exists
// today (`autopilot-desktop-v1.0.0-rc.3`) PREDATES the Phase 1+2 auto-onboarding
// chain (AO-1..AO-4: self-register -> configure node -> presence/payout/Tassadar
// -> identity choice + wizard). Those land on `main` but are NOT yet in any
// signed build. So the "one-click, open-it-and-it-does-everything" experience is
// NOT live until the owner builds + signs + notarizes a fresh DMG from current
// `main` and updates the asset URL below.
//
// `DOWNLOAD_ONE_CLICK_READY` is that switch:
//   - `false` (current): the page is discoverable, the download link works, but
//     the copy is honest that this build boots an isolated node and does NOT yet
//     auto-onboard. No "one-click does everything" claim is made.
//   - `true` (owner flips after shipping the fresh signed DMG): the copy
//     promotes the real one-click auto-onboarding experience.
// Owner steps to go fully live are documented in
// `docs/launch/2026-06-18-autopilot-desktop-availability-audit.md` (§4) and in
// the on-page "For the owner" note below.
//
// Naming note: keep copy generic/honest. Do not change homepage marketing copy
// here -- this is a dedicated page plus a single discoverable link.

// Owner-gated switch. Flip to `true` ONLY once a fresh signed + notarized DMG
// built from current `main` (AO-1..AO-4 included) is published and
// `AUTOPILOT_DESKTOP_DMG_URL` points at it.
export const DOWNLOAD_ONE_CLICK_READY = false

// The currently published, signed + notarized macOS Apple Silicon DMG.
// Verified asset on GitHub release `autopilot-desktop-v1.0.0-rc.3`.
// NOTE (owner): when you publish the fresh auto-onboarding build, replace this
// URL with the new signed asset and flip `DOWNLOAD_ONE_CLICK_READY` to `true`.
export const AUTOPILOT_DESKTOP_DMG_URL =
  'https://github.com/OpenAgentsInc/openagents/releases/download/autopilot-desktop-v1.0.0-rc.3/AutopilotDesktop-1.0.0-rc.3-macos-arm64.dmg'

export const AUTOPILOT_DESKTOP_RELEASE_URL =
  'https://github.com/OpenAgentsInc/openagents/releases/tag/autopilot-desktop-v1.0.0-rc.3'

const PYLON_INSTALL_COMMAND = 'npx @openagentsinc/pylon'

const pageShellClass = 'h-dvh overflow-auto bg-[#000] text-[#f1efe8]'

const sectionLabelClass = 'm-0 font-mono text-base text-white/35 sm:text-sm'

const cardClass = 'grid gap-3 border border-[#222] bg-[#010102] p-5 sm:p-6'

const heroView = <Message>(): Html => {
  const h = html<Message>()

  return h.section(
    [Ui.className<Message>('grid gap-3 border-b border-[#222] pb-8')],
    [
      h.p([Ui.className<Message>(sectionLabelClass)], ['Autopilot Desktop']),
      h.h1(
        [
          Ui.className<Message>(
            'm-0 text-balance text-3xl font-medium tracking-normal text-[#f1efe8] sm:text-4xl',
          ),
        ],
        ['Download Autopilot for Mac'],
      ),
      h.p(
        [Ui.className<Message>('mt-1 max-w-[68ch] text-base/7 text-white/65')],
        [
          DOWNLOAD_ONE_CLICK_READY
            ? 'Install the signed macOS app, open it, and it sets up your node and joins the run for you. No terminal required.'
            : 'The signed macOS app is available now. The one-click auto-onboarding experience is shipping in the next signed build — see the status note below before you install.',
        ],
      ),
    ],
  )
}

const downloadButton = <Message>(): Html => {
  const h = html<Message>()

  return h.a(
    [
      h.Href(AUTOPILOT_DESKTOP_DMG_URL),
      h.DataAttribute('cta', 'download-autopilot'),
      Ui.className<Message>(
        'inline-flex min-h-11 items-center justify-center border border-[#f1efe8] bg-[#f1efe8] px-5 font-mono text-[0.8125rem] font-semibold text-[#000] hover:bg-white',
      ),
    ],
    ['Download for Mac (Apple Silicon)'],
  )
}

const macCardView = <Message>(): Html => {
  const h = html<Message>()

  return h.section(
    [h.DataAttribute('download-platform', 'macos'), Ui.className<Message>(cardClass)],
    [
      h.p(
        [
          Ui.className<Message>(
            'm-0 font-mono text-[0.6875rem] uppercase text-white/35',
          ),
        ],
        ['macOS · Apple Silicon'],
      ),
      h.h2(
        [Ui.className<Message>('m-0 text-lg font-medium text-[#f1efe8]')],
        ['Signed + notarized .dmg'],
      ),
      h.p(
        [Ui.className<Message>('m-0 text-base/7 text-white/60')],
        [
          'Built with an Apple Developer ID and notarized, so macOS Gatekeeper opens it without warnings. Apple Silicon (M-series) Macs.',
        ],
      ),
      downloadButton<Message>(),
      h.a(
        [
          h.Href(AUTOPILOT_DESKTOP_RELEASE_URL),
          Ui.className<Message>(
            'font-mono text-xs text-white/45 underline underline-offset-2 hover:text-white/70',
          ),
        ],
        ['View the release on GitHub'],
      ),
    ],
  )
}

// Honest status note. Always rendered; copy depends on the owner gate.
const statusNoteView = <Message>(): Html => {
  const h = html<Message>()

  return h.section(
    [
      h.DataAttribute('download-status', DOWNLOAD_ONE_CLICK_READY ? 'live' : 'gated'),
      Ui.className<Message>(
        'm-0 grid gap-2 border border-[#ffb400]/40 bg-[#1a1403] px-4 py-3 text-sm/6 text-white/80',
      ),
    ],
    DOWNLOAD_ONE_CLICK_READY
      ? [
          h.p(
            [Ui.className<Message>('m-0 font-mono text-xs uppercase text-[#ffb400]')],
            ['Ready'],
          ),
          h.p(
            [Ui.className<Message>('m-0')],
            [
              'This build runs the full first-run flow automatically: it creates an identity, registers, brings up your node, and joins the run.',
            ],
          ),
        ]
      : [
          h.p(
            [Ui.className<Message>('m-0 font-mono text-xs uppercase text-[#ffb400]')],
            ['Status: auto-onboarding not in this build yet'],
          ),
          h.p(
            [Ui.className<Message>('m-0')],
            [
              'The currently published .dmg is the latest signed release, but it predates the new auto-onboarding flow. If you install it today it boots an isolated node — it does not yet self-register or join the run for you on its own. The next signed build will. If you want to start contributing right now, run a Pylon node instead.',
            ],
          ),
        ],
  )
}

const pylonAlternativeView = <Message>(): Html => {
  const h = html<Message>()

  return h.section(
    [Ui.className<Message>(cardClass)],
    [
      h.p(
        [
          Ui.className<Message>(
            'm-0 font-mono text-[0.6875rem] uppercase text-white/35',
          ),
        ],
        ['For agents + operators'],
      ),
      h.h2(
        [Ui.className<Message>('m-0 text-lg font-medium text-[#f1efe8]')],
        ['Run a Pylon node from the terminal'],
      ),
      h.p(
        [Ui.className<Message>('m-0 text-base/7 text-white/60')],
        ['The contributor path that works today. Paste this to your coding agent or run it yourself.'],
      ),
      h.pre(
        [
          h.DataAttribute('cta', 'install-pylon-command'),
          Ui.className<Message>(
            'm-0 w-full select-all overflow-x-auto border border-white/15 bg-[#030303] px-3 py-2 text-left font-mono text-[0.8rem] leading-none text-[#d6f6ff]',
          ),
        ],
        [h.code([], [PYLON_INSTALL_COMMAND])],
      ),
    ],
  )
}

const platformStatusView = <Message>(): Html => {
  const h = html<Message>()

  const row = (platform: string, status: string): Html =>
    h.div(
      [
        Ui.className<Message>(
          'flex items-baseline justify-between gap-4 border-b border-[#1a1a1a] py-2 last:border-b-0',
        ),
      ],
      [
        h.span(
          [Ui.className<Message>('font-mono text-sm text-[#f1efe8]')],
          [platform],
        ),
        h.span(
          [Ui.className<Message>('text-right font-mono text-xs text-white/45')],
          [status],
        ),
      ],
    )

  return h.section(
    [Ui.className<Message>(cardClass)],
    [
      h.p(
        [
          Ui.className<Message>(
            'm-0 font-mono text-[0.6875rem] uppercase text-white/35',
          ),
        ],
        ['Platform availability'],
      ),
      h.div(
        [Ui.className<Message>('grid')],
        [
          row('macOS · Apple Silicon', 'Available now (signed + notarized)'),
          row('macOS · Intel', 'Not published yet'),
          row('Windows', 'Pending the Authenticode signing certificate'),
          row('Linux', 'Not published yet'),
        ],
      ),
    ],
  )
}

// Owner-facing note (intentionally on-page and honest). It documents exactly
// what the owner must do to flip this page to the live one-click experience.
const ownerNoteView = <Message>(): Html => {
  const h = html<Message>()

  return h.section(
    [
      Ui.className<Message>(
        'm-0 grid gap-1.5 border border-[#222] bg-[#010102] px-4 py-3 font-mono text-xs text-white/40',
      ),
    ],
    [
      h.p(
        [Ui.className<Message>('m-0 uppercase tracking-wide text-white/35')],
        ['For the owner — to make one-click live'],
      ),
      h.p(
        [Ui.className<Message>('m-0 text-white/45')],
        [
          'Build + sign + notarize a fresh DMG from current main (with AO-1..AO-4), publish it, update AUTOPILOT_DESKTOP_DMG_URL in page/download.ts, then set DOWNLOAD_ONE_CLICK_READY = true. See docs/launch/2026-06-18-autopilot-desktop-availability-audit.md §4.',
        ],
      ),
    ],
  )
}

export const view = <Message>(
  authState: PublicHeaderAuthState<Message>,
): Html => {
  const h = html<Message>()

  return h.div(
    [h.DataAttribute('route', 'download'), Ui.className<Message>(pageShellClass)],
    [
      PublicHeader.view(authState),
      h.main(
        [
          h.AriaLabel('Download'),
          Ui.className<Message>(
            'mx-auto grid w-[min(100%,920px)] gap-8 px-4 py-8 lg:grid-cols-[minmax(0,1fr)_minmax(20rem,24rem)]',
          ),
        ],
        [
          h.div(
            [Ui.className<Message>('grid content-start gap-6')],
            [
              heroView<Message>(),
              statusNoteView<Message>(),
              macCardView<Message>(),
              pylonAlternativeView<Message>(),
            ],
          ),
          h.div(
            [Ui.className<Message>('grid content-start gap-6')],
            [platformStatusView<Message>(), ownerNoteView<Message>()],
          ),
        ],
      ),
    ],
  )
}
