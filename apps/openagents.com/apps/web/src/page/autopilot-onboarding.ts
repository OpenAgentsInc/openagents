import { Option } from 'effect'
import type { Html } from 'foldkit/html'
import { html } from 'foldkit/html'

import * as Ui from '../ui'
import type { PublicHeaderAuthState } from './publicHeader'
import * as PublicHeader from './publicHeader'

// Public `openagents.com/autopilot` onboarding entry point.
//
// This is the route-shape placeholder. It serves the SPA shell for both
// `/autopilot` and `/autopilot/<vertical>` (e.g. `/autopilot/legal`) for
// logged-out users and for logged-in users without a workspace. Logged-in
// users with a workspace resolve to the existing cockpit upstream in the
// router, so they never reach this page.
//
// The rich onboarding flow (conversation + streamed components + 3D scene) is
// a separate issue (#6129) that expands this page. Keep this minimal: a
// heading + intro composed from the centralized `@openagentsinc/ui` theme so
// #6129 has a clean, well-structured base to build on. Do NOT hand-roll
// styling or duplicate tokens.

const pageShellClass = 'h-dvh overflow-auto bg-[#000] text-[#f1efe8]'

const verticalLabels: Readonly<Record<string, string>> = {
  legal: 'legal',
}

const introForVertical = (maybeVertical: Option.Option<string>): string =>
  Option.match(maybeVertical, {
    onNone: () =>
      'Tell Autopilot what you want done. We set up a workspace, then agents do the work with a human-review gate before anything ships.',
    onSome: vertical => {
      const label = verticalLabels[vertical] ?? vertical
      return `Tell Autopilot what you want done for your ${label} work. We set up a workspace, then agents do the work with a human-review gate before anything ships.`
    },
  })

export const onboardingShell = <Message>(
  maybeVertical: Option.Option<string>,
): Html => {
  const h = html<Message>()

  return Ui.publicLandingThemeShell<Message>({
    preference: 'dark',
    mode: 'dark',
    className: 'min-h-[calc(100dvh-3.5rem)]',
    attrs: [h.DataAttribute('autopilot-onboarding-shell', '')],
    children: [
      h.main(
        [
          h.AriaLabel('Autopilot onboarding'),
          Ui.className<Message>(
            'mx-auto grid w-[min(100%,720px)] content-start gap-4 px-4 py-16',
          ),
        ],
        [
          h.p(
            [
              Ui.className<Message>(
                'm-0 font-mono text-[0.72rem] uppercase tracking-[0.14em] text-white/40',
              ),
            ],
            ['Autopilot'],
          ),
          h.h1(
            [
              Ui.className<Message>(
                'm-0 text-balance text-3xl font-medium tracking-normal text-[#f1efe8] sm:text-4xl',
              ),
            ],
            ['Put an AI workforce to work'],
          ),
          h.p(
            [Ui.className<Message>('m-0 text-base text-white/70')],
            [introForVertical(maybeVertical)],
          ),
        ],
      ),
    ],
  })
}

export const view = <Message>(
  authState: PublicHeaderAuthState<Message>,
  maybeVertical: Option.Option<string> = Option.none(),
): Html => {
  const h = html<Message>()

  return h.div(
    [Ui.className<Message>(pageShellClass)],
    [PublicHeader.view(authState), onboardingShell<Message>(maybeVertical)],
  )
}
